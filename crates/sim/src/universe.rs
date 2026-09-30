//! The universe as the game runs it: the world, the player's ship with its
//! avionics, and the settlers' crafts with theirs, advanced one tick at a
//! time in a fixed order — the player's ship first, then every craft, each
//! from the same moment (see `vessel` for what a ship's turn is).

use std::rc::Rc;

use glam::{DQuat, DVec3};
use universe_avionics::route::{self, Stop};
use universe_avionics::{Approach, Avionics, Event, NavTarget, Plan};
use universe_world::{Controls, Person, Ship, ShipCommands, ShipEvent, ShipState, StarSystem, StepResult, WalkCommands, World};

use crate::traffic::{CrashReport, Craft, TrafficStats};
use crate::vessel::Vessel;

pub struct Universe {
    /// The galaxy, its gate network, the clock and the star systems.
    pub world: World,
    pub ship: Ship,
    /// Galaxy index of the system the ship is in; ship coordinates are relative to its star.
    pub ship_system: usize,
    /// The ship's avionics: nav target, clearance, autopilots, route.
    pub avionics: Avionics,
    /// What happened to the player's ship, for the pilot (the game takes them).
    pub events: Vec<Event>,
    /// Other ships (settlers), each flying its own route.
    pub crafts: Vec<Craft>,
    pub traffic: TrafficStats,
    /// Recent craft crashes (most recent last, capped).
    pub crash_log: Vec<CrashReport>,
    /// The pilot: in the seat, or on foot.
    pub crew: Person,
    /// Recent kills by weapons fire, most recent last.
    pub kills: Vec<crate::combat::Kill>,
    positions: Vec<DVec3>,
}

impl Universe {
    pub fn new(seed: u64) -> Self {
        let mut u = Self {
            world: World::new(seed),
            ship: Ship::new(DVec3::ZERO, DVec3::ZERO, DQuat::IDENTITY),
            ship_system: 0,
            avionics: Avionics::default(),
            events: Vec::new(),
            crafts: Vec::new(),
            traffic: TrafficStats::default(),
            crash_log: Vec::new(),
            crew: Person::default(),
            kills: Vec::new(),
            positions: Vec::new(),
        };
        u.respawn();
        u.events.clear();
        u
    }

    /// Systems linked to `i` by gates, with their names.
    pub fn gate_links_of(&self, i: usize) -> Vec<(usize, String)> {
        self.world.gate_links_of(i)
    }

    /// Get (generating and caching if needed) the star system at galaxy index `i`.
    pub fn system(&mut self, i: usize) -> Rc<StarSystem> {
        self.world.system(i)
    }

    pub fn ship_system(&mut self) -> Rc<StarSystem> {
        self.world.system(self.ship_system)
    }

    /// Distance in light years between two stars.
    pub fn distance_ly(&self, a: usize, b: usize) -> f64 {
        self.world.distance_ly(a, b)
    }

    /// The player's ship and its avionics, and the world they're in.
    fn player(&mut self) -> (&mut World, Vessel<'_>) {
        let vessel = Vessel { ship: &mut self.ship, system: &mut self.ship_system, avionics: &mut self.avionics, events: &mut self.events };
        (&mut self.world, vessel)
    }

    /// Advance the player's ship by `real_dt * warp` seconds (less if warp is
    /// limited), the pilot's stick at `controls`. The world clock moves with it.
    pub fn step(&mut self, real_dt: f64, warp: f64, controls: &Controls) -> StepResult {
        let (world, mut player) = self.player();
        player.tick(world, controls, real_dt, warp)
    }

    /// Advance the whole world: the player's ship, then every craft, all from
    /// the same moment; the clock moves once (as far as the player's ship went).
    pub fn step_world(&mut self, real_dt: f64, warp: f64, controls: &Controls) -> StepResult {
        let t0 = self.world.time;
        let result = self.step(real_dt, warp, controls);
        let t1 = self.world.time;
        for i in 0..self.crafts.len() {
            self.world.time = t0;
            self.fly_craft(i, real_dt, warp);
        }
        self.world.time = t1;
        self.combat(t1 - t0);
        result
    }

    // The pilot's requests, to the ship's avionics (or, for `command`,
    // straight to its devices), taking effect at once.

    /// The pilot on foot (or getting up, sitting down) for `real_dt` real
    /// seconds. A new ship puts the pilot back in its seat.
    pub fn walk(&mut self, c: &WalkCommands, real_dt: f64) {
        if self.events.iter().any(|e| matches!(e, Event::Ship(ShipEvent::Respawned))) || !self.ship.is_flying() && !matches!(self.ship.state, ShipState::Landed { .. }) {
            self.crew = Person::default();
        }
        let sys = self.ship_system();
        sys.positions(self.world.time, &mut self.positions);
        let mut events = Vec::new();
        self.crew.step(&sys, &self.ship, self.world.time, &self.positions, c, real_dt, &mut events);
        self.events.extend(events.into_iter().map(Event::Crew));
    }

    /// Where the pilot's eyes are and which way they look (seated: `seat_eye`).
    pub fn pilot_eye(&mut self, seat_eye: DVec3) -> (DVec3, DQuat) {
        let sys = self.ship_system();
        sys.positions(self.world.time, &mut self.positions);
        self.crew.eye(&sys, &self.ship, self.world.time, &self.positions, seat_eye)
    }

    /// What the pilot on foot could use now.
    pub fn pilot_reach(&mut self) -> Option<universe_world::crew::Reach> {
        let sys = self.ship_system();
        sys.positions(self.world.time, &mut self.positions);
        self.crew.reach(&sys, &self.ship, self.world.time, &self.positions)
    }

    /// Give the ship's devices new commands now (see `World::command`).
    pub fn command(&mut self, c: &ShipCommands) {
        let (world, mut player) = self.player();
        player.run(world, |a, link, events| a.command(link, c, events));
    }

    /// Put a new ship next to the home station, matching its orbit.
    pub fn respawn(&mut self) {
        let mut events = Vec::new();
        self.world.respawn(&mut self.ship, &mut self.ship_system, &mut events);
        self.avionics.record(events, &mut self.events);
    }

    pub fn toggle_hyperdrive(&mut self) {
        let (world, mut player) = self.player();
        player.run(world, |a, link, events| a.toggle_hyperdrive(link, events));
    }

    /// Lock (or clear) the navigation target.
    pub fn set_nav_target(&mut self, target: Option<NavTarget>) {
        let (world, mut player) = self.player();
        player.run(world, |a, link, events| a.set_nav_target(link, target, events));
    }

    /// Ask traffic control for permission to dock/land at the locked nav
    /// target (or the nearest station).
    pub fn request_clearance(&mut self) -> bool {
        let (world, mut player) = self.player();
        player.run(world, |a, link, events| a.request_clearance(link, events))
    }

    /// Give the clearance up (stopping its autopilot).
    pub fn cancel_clearance(&mut self) {
        let (world, mut player) = self.player();
        player.run(world, |a, link, events| a.cancel_clearance(link, events));
    }

    /// Engage or release the autopilot. In hyperdrive it steers to the nav
    /// target; otherwise it docks or lands (requesting clearance if needed).
    pub fn toggle_autopilot(&mut self) {
        let (world, mut player) = self.player();
        player.run(world, |a, link, events| a.toggle_autopilot(link, events));
    }

    /// Start or stop the route autopilot.
    pub fn toggle_route(&mut self) {
        let (world, mut player) = self.player();
        player.run(world, |a, link, events| a.toggle_route(link, events));
    }

    // What the avionics show the pilot.

    /// Display name of a target in the ship's system.
    pub fn target_name(&mut self, target: NavTarget) -> String {
        target.name(&self.ship_system())
    }

    /// Current world position of a target, and whether it's still valid.
    pub fn target_position(&mut self, target: NavTarget) -> Option<DVec3> {
        let sys = self.ship_system();
        sys.positions(self.world.time, &mut self.positions);
        target.position(&sys, self.world.time, &self.positions)
    }

    /// Name of a stop, with its system if it's elsewhere.
    pub fn stop_name(&mut self, stop: Stop) -> String {
        route::stop_name(&self.system(stop.system), stop)
    }

    /// Guidance numbers for the HUD, if cleared to dock or land.
    pub fn approach(&mut self) -> Option<Approach> {
        let sys = self.ship_system();
        sys.positions(self.world.time, &mut self.positions);
        self.avionics.approach(&sys, &self.ship, self.world.time, &self.positions)
    }

    /// The flight plan to the cleared target: what to do from here, as the
    /// autopilot would do it.
    pub fn plan(&mut self) -> Option<Plan> {
        let sys = self.ship_system();
        self.avionics.plan(&sys, &self.ship, self.world.time)
    }

    /// Docking guidance only (convenience for tests and tools).
    pub fn docking_status(&mut self) -> Option<(usize, universe_avionics::DockingStatus)> {
        match self.approach()? {
            Approach::Dock { station, status } => Some((station, status)),
            Approach::Land { .. } | Approach::Transit { .. } => None,
        }
    }
}
