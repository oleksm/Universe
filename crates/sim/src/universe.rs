use std::rc::Rc;

use glam::{DQuat, DVec3};
use serde::{Deserialize, Serialize};
use universe_world::traffic;
use universe_world::{
    BodyKind, Controls, HyperdriveCommand, Ship, ShipCommands, ShipEvent, ShipState, StarSystem, StepResult, TrafficEvent, World,
};

use crate::avionics::{Avionics, Clearance, NavTarget, Phase};
use crate::computer::{self, Computer};
use crate::docking::{self, DockingStatus};
use crate::gate::{self, GateStatus};
use crate::landing::{self, LandingStatus, PadFrame};
use crate::route::{self, Route, Stop};
use crate::rng::Rng;
use crate::{GateFrame, StationFrame};

/// What happened, for the pilot (and the traffic statistics): the world's
/// physical events and traffic control's, and the avionics' own.
#[derive(Clone, Debug)]
pub enum Event {
    /// Something physically happened to the ship.
    Ship(ShipEvent),
    /// Traffic control said something.
    Traffic(TrafficEvent),
    /// Dropped out of hyperdrive at the nav target.
    HyperdriveArrived { target: String },
    /// The route autopilot reached a stop (1-based number, name).
    RouteStop { number: usize, name: String },
    RouteComplete,
    /// A route stop can't be reached (no gate path, or it no longer exists).
    RouteBlocked { reason: String },
    Autopilot { on: bool },
    NavTargetSet { name: Option<String> },
    /// The avionics can't do what was asked.
    Refused { reason: String },
}

/// Live guidance for the current clearance.
#[derive(Clone, Debug)]
pub enum Approach {
    Dock { station: usize, status: DockingStatus },
    Land { port: usize, status: Box<LandingStatus> },
    Transit { gate: usize, status: GateStatus },
}

/// Everything needed to restore a game. Star systems regenerate from the seed.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct UniverseSave {
    pub seed: u64,
    pub time: f64,
    pub ship: Ship,
    pub ship_system: usize,
    #[serde(default)]
    pub route: Route,
    #[serde(default)]
    pub avionics: Avionics,
}

pub struct Universe {
    /// The galaxy, its gate network, the clock and the star systems.
    pub world: World,
    pub ship: Ship,
    /// Galaxy index of the system the ship is in; ship coordinates are relative to its star.
    pub ship_system: usize,
    /// The ship's navigation state: target, clearance, autopilots.
    pub avionics: Avionics,
    /// The ship's multi-stop route, and the route autopilot's progress.
    pub route: Route,
    pub events: Vec<Event>,
    /// Other ships (settlers), each flying its own route.
    pub crafts: Vec<Craft>,
    pub traffic: TrafficStats,
    /// Recent craft crashes (most recent last, capped).
    pub crash_log: Vec<CrashReport>,
    positions: Vec<DVec3>,
    /// Where the hyperdrive autopilot is steering (for debugging).
    pub debug_way: Option<DVec3>,
}

/// Another ship in the world, flown by the same code as the player's: its
/// state is swapped into the `Universe` for its turn (see `step_world`).
pub struct Craft {
    pub name: String,
    pub ship: Ship,
    /// Galaxy index of the system it's in.
    pub system: usize,
    pub avionics: Avionics,
    pub route: Route,
    /// Seed of its current route (a new one is made when it finishes).
    pub route_seed: u64,
    events: Vec<Event>,
}

/// Totals across all crafts.
#[derive(Clone, Copy, Debug, Default)]
pub struct TrafficStats {
    pub stops: u64,
    pub transits: u64,
    pub crashes: u64,
    pub routes_completed: u64,
}

/// What a ship was doing when it crashed (for diagnosing autopilots).
#[derive(Clone, Debug)]
pub struct CrashReport {
    pub craft: String,
    pub body: String,
    pub system: usize,
    pub time: f64,
    pub target: Option<NavTarget>,
    pub clearance: Option<Clearance>,
    pub hyperdrive: bool,
    pub departing: bool,
    pub speed: f64,
}

impl Universe {
    pub fn new(seed: u64) -> Self {
        let mut u = Self {
            world: World::new(seed),
            ship: Ship::new(DVec3::ZERO, DVec3::ZERO, DQuat::IDENTITY),
            ship_system: 0,
            avionics: Avionics::default(),
            route: Route::default(),
            events: Vec::new(),
            crafts: Vec::new(),
            traffic: TrafficStats::default(),
            crash_log: Vec::new(),
            debug_way: None,
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

    /// Report the world's events for the ship, and let its avionics take note.
    fn record(&mut self, events: Vec<ShipEvent>) {
        for e in events {
            self.avionics.observe(&e);
            self.events.push(Event::Ship(e));
        }
    }

    /// Give the ship's devices new commands now (see `World::command`).
    pub fn command(&mut self, c: &ShipCommands) {
        let mut events = Vec::new();
        self.world.command(&mut self.ship, self.ship_system, c, &mut events);
        self.record(events);
    }

    /// Change some of the engine and thruster settings, keeping the rest.
    fn set_controls(&mut self, change: impl FnOnce(&mut ShipCommands)) {
        let mut c = self.ship.holding();
        change(&mut c);
        self.command(&c);
    }

    /// Put a new ship next to the home station, matching its orbit.
    pub fn respawn(&mut self) {
        let mut events = Vec::new();
        self.world.respawn(&mut self.ship, &mut self.ship_system, &mut events);
        self.record(events);
    }

    pub fn toggle_hyperdrive(&mut self) {
        if !self.ship.is_flying() {
            return;
        }
        let engage = !self.ship.hyperdrive;
        // Drop out co-moving with the nav target if it's close, otherwise
        // with whatever dominates gravity here.
        let exit_velocity = if engage {
            None
        } else {
            let sys = self.ship_system();
            sys.positions(self.world.time, &mut self.positions);
            let aim = self.avionics.nav_target.and_then(|t| computer::hyper_aim(&sys, t, self.world.time, &self.positions, self.ship.position));
            computer::exit_velocity(aim.as_ref(), self.ship.position)
        };
        let orders = HyperdriveCommand { engage, exit_velocity, ..Default::default() };
        self.command(&ShipCommands { hyperdrive: Some(orders), ..self.ship.holding() });
        // (Both ways, the drive starts from zero throttle, and the pilot has the stick.)
        self.avionics.hyper_autopilot = false;
    }

    /// Advance the universe by `real_dt * warp` seconds (less if warp is limited).
    pub fn step(&mut self, real_dt: f64, warp: f64, controls: &Controls) -> StepResult {
        self.route_step();
        if !self.route.active {
            self.autopilot_hyperjump();
        }
        // The pilot's stick turns the ship, unless a computer is flying it.
        let computer_flies = match self.ship.state {
            ShipState::Flying if self.ship.hyperdrive => self.avionics.hyper_autopilot,
            ShipState::Flying => self.avionics.autopilot_engaged(),
            _ => false,
        };
        let commands = ShipCommands { turn: (!computer_flies).then_some(*controls), ..self.ship.holding() };
        let mut events = Vec::new();
        let mut computer = Computer::new(&mut self.avionics, &mut self.debug_way);
        let result = self.world.step_ship(&mut self.ship, &mut self.ship_system, &commands, &mut computer, real_dt, warp, &mut events);
        let arrived = computer.arrived;
        self.record(events);
        if let Some(target) = arrived {
            self.events.push(Event::HyperdriveArrived { target });
        }
        self.check_clearance();
        result
    }

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

    /// Lock (or clear) the navigation target.
    pub fn set_nav_target(&mut self, target: Option<NavTarget>) {
        if self.avionics.clearance.is_some_and(|c| Some(c.target) != target) {
            self.avionics.clearance = None;
            self.set_controls(|c| c.rcs = DVec3::ZERO);
            self.events.push(Event::Traffic(TrafficEvent::ClearanceCancelled));
        }
        self.avionics.nav_target = target;
        let name = target.map(|t| self.target_name(t));
        self.events.push(Event::NavTargetSet { name });
    }

    /// Ask traffic control for permission to dock/land at the locked nav
    /// target (or the nearest station).
    pub fn request_clearance(&mut self) -> bool {
        let sys = self.ship_system();
        let t = self.world.time;
        sys.positions(t, &mut self.positions);
        let target = self.avionics.nav_target.or_else(|| traffic::nearest_station(&sys, self.ship.position, &self.positions));
        match traffic::request(&sys, &self.ship, target, t, &self.positions) {
            Ok(target) => {
                self.avionics.clearance = Some(Clearance { target, autopilot: false, phase: Phase::Approach });
                self.events.push(Event::Traffic(TrafficEvent::ClearanceGranted { target: target.name(&sys), kind: target.kind() }));
                true
            }
            Err(reason) => {
                self.events.push(Event::Traffic(TrafficEvent::ClearanceDenied { reason }));
                false
            }
        }
    }

    /// Engage or release the autopilot. In hyperdrive it steers to the nav
    /// target; otherwise it docks or lands (requesting clearance if needed).
    pub fn toggle_autopilot(&mut self) {
        if self.ship.hyperdrive {
            if self.avionics.nav_target.is_none() && !self.avionics.hyper_autopilot {
                self.events.push(Event::Refused { reason: "LOCK A NAV TARGET FIRST (M)".into() });
                return;
            }
            self.avionics.hyper_autopilot = !self.avionics.hyper_autopilot;
            self.events.push(Event::Autopilot { on: self.avionics.hyper_autopilot });
            return;
        }
        if self.avionics.clearance.is_none() && !self.request_clearance() {
            return;
        }
        let Some(c) = &mut self.avionics.clearance else { return };
        c.autopilot = !c.autopilot;
        c.phase = Phase::Approach;
        let on = c.autopilot;
        if !on {
            self.set_controls(|c| {
                c.rcs = DVec3::ZERO;
                c.throttle = 0.0;
            });
        }
        self.events.push(Event::Autopilot { on });
    }

    /// Clearance lapses if the ship wanders far away or the target vanishes
    /// (traffic control's rule).
    fn check_clearance(&mut self) {
        let Some(c) = self.avionics.clearance else { return };
        if !self.ship.is_flying() {
            return;
        }
        let sys = self.ship_system();
        sys.positions(self.world.time, &mut self.positions);
        if traffic::lapsed(&sys, &self.ship, c.target, self.world.time, &self.positions) {
            self.avionics.clearance = None;
            self.set_controls(|c| c.rcs = DVec3::ZERO);
            self.events.push(Event::Traffic(TrafficEvent::ClearanceCancelled));
        }
    }

    /// Advance the whole world: the player's ship, then every craft, all from
    /// the same moment; the clock moves once.
    pub fn step_world(&mut self, real_dt: f64, warp: f64, controls: &Controls) -> StepResult {
        let t0 = self.world.time;
        let result = self.step(real_dt, warp, controls);
        let t1 = self.world.time;
        for i in 0..self.crafts.len() {
            self.world.time = t0;
            self.swap_craft(i);
            let before = (self.avionics.nav_target, self.avionics.clearance, self.ship.hyperdrive, self.route.departing, self.ship.velocity);
            self.step(real_dt, warp, &Controls::default());
            let crashed = self.events.iter().find_map(|e| match e {
                Event::Ship(ShipEvent::Crashed { body }) => Some(body.clone()),
                _ => None,
            });
            if let Some(body) = crashed {
                let sys = self.ship_system();
                let speed = sys.bodies.iter().position(|b| b.name == body).map_or(0.0, |b| (before.4 - sys.velocity(b, self.world.time)).length());
                self.crash_log.push(CrashReport {
                    craft: self.crafts[i].name.clone(),
                    body,
                    system: self.ship_system,
                    time: self.world.time,
                    target: before.0,
                    clearance: before.1,
                    hyperdrive: before.2,
                    departing: before.3,
                    speed,
                });
                if self.crash_log.len() > 50 {
                    self.crash_log.remove(0);
                }
            }
            self.swap_craft(i);
            self.tally_craft(i);
        }
        self.world.time = t1;
        result
    }

    /// Exchange the player's per-ship state with craft `i`'s.
    fn swap_craft(&mut self, i: usize) {
        let c = &mut self.crafts[i];
        std::mem::swap(&mut self.ship, &mut c.ship);
        std::mem::swap(&mut self.ship_system, &mut c.system);
        std::mem::swap(&mut self.avionics, &mut c.avionics);
        std::mem::swap(&mut self.route, &mut c.route);
        std::mem::swap(&mut self.events, &mut c.events);
    }

    /// Count what happened to craft `i`, and give it a new route when done.
    fn tally_craft(&mut self, i: usize) {
        let events = std::mem::take(&mut self.crafts[i].events);
        for e in events {
            match e {
                Event::RouteStop { .. } => self.traffic.stops += 1,
                Event::Ship(ShipEvent::GateEntered { .. }) => self.traffic.transits += 1,
                Event::Ship(ShipEvent::Crashed { .. }) => self.traffic.crashes += 1,
                Event::RouteComplete => self.traffic.routes_completed += 1,
                _ => {}
            }
        }
        let c = &self.crafts[i];
        if !c.route.active && matches!(c.ship.state, ShipState::Landed { .. }) {
            let seed = crate::rng::mix(c.route_seed, 1);
            let mut stops = self.settler_route(seed, 10);
            // Start from where it is: skip a first stop that's right here.
            if stops.first().is_some_and(|s| s.system == self.crafts[i].system) {
                stops.rotate_left(1);
            }
            let c = &mut self.crafts[i];
            c.route = Route { stops, next: 0, active: true, dwell_until: None, departing: false };
            c.route_seed = seed;
        }
    }

    /// Settlers: `count` crafts, each on its own reproducible route (from
    /// `seed`), starting docked or landed at its first stop with staggered
    /// departures.
    pub fn spawn_settlers(&mut self, count: usize, seed: u64) {
        let mut rng = Rng::new(seed);
        for i in 0..count {
            let route_seed = crate::rng::mix(seed, i as u64);
            let stops = self.settler_route(route_seed, 10);
            let Some(&first) = stops.first() else { continue };
            let ship = self.world.ship_at(first.system, first.target);
            let route = Route { stops, next: 0, active: true, dwell_until: Some(self.world.time + rng.range(0.0, 600.0)), departing: false };
            self.crafts.push(Craft {
                name: format!("Settler {}", self.crafts.len() + 1),
                ship,
                system: first.system,
                avionics: Avionics::default(),
                route,
                route_seed,
                events: Vec::new(),
            });
        }
    }

    /// Start or stop the route autopilot.
    pub fn toggle_route(&mut self) {
        if self.route.stops.is_empty() {
            return;
        }
        self.route.active = !self.route.active;
        if !self.route.active {
            if let Some(c) = &mut self.avionics.clearance {
                c.autopilot = false;
            }
            self.avionics.hyper_autopilot = false;
            self.set_controls(|c| {
                c.rcs = DVec3::ZERO;
                c.throttle = 0.0;
            });
        }
        self.events.push(Event::Autopilot { on: self.route.active });
    }

    /// A reproducible route of `count` stops (stations and spaceports) across
    /// the gate network, from a seed: same seed, same route.
    pub fn settler_route(&mut self, seed: u64, count: usize) -> Vec<Stop> {
        let mut systems: Vec<usize> = self.world.gate_links.iter().flat_map(|&(a, b)| [a, b]).collect();
        systems.sort();
        systems.dedup();
        let mut candidates = Vec::new();
        for s in systems {
            let sys = self.system(s);
            for (i, b) in sys.bodies.iter().enumerate() {
                if b.kind == BodyKind::Station {
                    candidates.push(Stop { system: s, target: NavTarget::Station(i) });
                }
            }
            for i in 0..sys.spaceports.len() {
                candidates.push(Stop { system: s, target: NavTarget::Spaceport(i) });
            }
        }
        // Shuffle (Fisher-Yates) and take the first `count`: no repeats while
        // there are enough destinations, then cycle through again.
        let mut rng = Rng::new(crate::rng::mix(self.world.galaxy.seed, seed));
        for i in (1..candidates.len()).rev() {
            let j = (rng.next_u64() % (i as u64 + 1)) as usize;
            candidates.swap(i, j);
        }
        let mut stops: Vec<Stop> = Vec::new();
        for pick in candidates.iter().cycle().take(count * 2) {
            if stops.len() == count {
                break;
            }
            if stops.last() != Some(pick) {
                stops.push(*pick);
            }
        }
        stops
    }

    /// Name of a stop, with its system if it's elsewhere.
    pub fn stop_name(&mut self, stop: Stop) -> String {
        let sys = self.system(stop.system);
        let name = match stop.target {
            NavTarget::Station(b) | NavTarget::Gate(b) => sys.bodies.get(b).map_or_else(String::new, |b| b.name.clone()),
            NavTarget::Spaceport(p) => sys.spaceports.get(p).map_or_else(String::new, |p| p.name.clone()),
        };
        format!("{name} ({})", sys.name)
    }

    /// Is the ship, landed on `body` at `local_position`, at this target?
    fn landed_at(&mut self, target: NavTarget, body: usize, local_position: DVec3) -> bool {
        let sys = self.ship_system();
        match target {
            NavTarget::Station(s) => s == body,
            NavTarget::Spaceport(p) => sys.on_pad(p, body, local_position.normalize()),
            NavTarget::Gate(_) => false,
        }
    }

    /// Height above the ground of the dominant body (m), if it has a surface.
    fn ground_altitude(&mut self) -> f64 {
        let sys = self.ship_system();
        sys.positions(self.world.time, &mut self.positions);
        let d = sys.dominant(self.ship.position, &self.positions);
        let b = &sys.bodies[d];
        self.ship.position.distance(self.positions[d]) - b.surface_radius_at(self.positions[d], self.ship.position, self.world.time)
    }

    /// The route autopilot: leave, cross systems through gates, travel by
    /// hyperdrive, dock or land with the regular autopilots, wait, repeat.
    fn route_step(&mut self) {
        if !self.route.active {
            return;
        }
        let Some(stop) = self.route.current() else {
            self.route.active = false;
            self.events.push(Event::RouteComplete);
            return;
        };
        match self.ship.state.clone() {
            ShipState::Destroyed { .. } | ShipState::Transit { .. } => {}
            ShipState::Landed { body, local_position, .. } => {
                if self.ship_system == stop.system && self.landed_at(stop.target, body, local_position) {
                    match self.route.dwell_until {
                        None => {
                            self.route.dwell_until = Some(self.world.time + route::DWELL);
                            let name = self.stop_name(stop);
                            self.events.push(Event::RouteStop { number: self.route.next + 1, name });
                        }
                        Some(t) if self.world.time >= t => {
                            self.route.dwell_until = None;
                            self.route.next += 1;
                            if self.route.next >= self.route.stops.len() {
                                self.route.active = false;
                                self.events.push(Event::RouteComplete);
                                return;
                            }
                            self.leave(body);
                        }
                        Some(_) => {}
                    }
                } else {
                    self.leave(body);
                }
            }
            ShipState::Flying => self.route_fly(stop),
        }
    }

    /// How close the hyperdrive gets before an autopilot takes over (m). The
    /// hyperdrive drops out a little closer still (120 km / 20 km).
    fn hyperjump_limit(target: NavTarget) -> f64 {
        match target {
            NavTarget::Spaceport(_) => 200_000.0,
            NavTarget::Station(_) | NavTarget::Gate(_) => 30_000.0,
        }
    }

    /// With the dock/land/gate autopilot on and the target far away, cover
    /// the distance by hyperdrive first (its autopilot steers around planets),
    /// then carry on: the clearance and autopilot stay on through the jump.
    fn autopilot_hyperjump(&mut self) {
        let Some(c) = self.avionics.clearance.filter(|c| c.autopilot) else { return };
        if !self.ship.is_flying() || self.ship.hyperdrive {
            return;
        }
        let Some(at) = self.target_position(c.target) else { return };
        if at.distance(self.ship.position) > Self::hyperjump_limit(c.target) {
            self.avionics.nav_target = Some(c.target);
            self.toggle_hyperdrive();
            self.avionics.hyper_autopilot = true;
            self.set_controls(|c| c.throttle = 1.0);
        }
    }

    /// Launch from a station, or lift off a surface and climb.
    fn leave(&mut self, body: usize) {
        let station = self.ship_system().bodies[body].kind == BodyKind::Station;
        if station {
            self.set_controls(|c| c.throttle = 0.2);
        } else {
            self.set_controls(|c| c.rcs = DVec3::Y);
            self.route.departing = true;
        }
    }

    fn route_fly(&mut self, stop: Stop) {
        if self.ship.hyperdrive {
            return; // the hyperdrive autopilot flies and drops out on arrival
        }
        if self.route.departing {
            // Straight up on the lift thrusters until clear of the ground.
            if self.ground_altitude() < 3000.0 {
                self.set_controls(|c| {
                    c.rcs = DVec3::Y;
                    c.throttle = 0.0;
                });
                return;
            }
            self.route.departing = false;
            self.set_controls(|c| c.rcs = DVec3::ZERO);
        }
        // Next hop: the stop itself, or the gate toward its system.
        let hop = if self.ship_system == stop.system {
            stop.target
        } else {
            let Some(next) = route::gate_path(&self.world.gate_links, self.ship_system, stop.system).and_then(|p| p.first().copied()) else {
                self.route.active = false;
                self.events.push(Event::RouteBlocked { reason: "NO GATE PATH".into() });
                return;
            };
            let Some(g) = self.ship_system().gate_to(next) else {
                self.route.active = false;
                self.events.push(Event::RouteBlocked { reason: "GATE MISSING".into() });
                return;
            };
            NavTarget::Gate(g)
        };
        if self.avionics.nav_target != Some(hop) {
            self.avionics.nav_target = Some(hop);
            self.avionics.clearance = None;
        }
        let Some(at) = self.target_position(hop) else {
            self.route.active = false;
            self.events.push(Event::RouteBlocked { reason: "STOP NOT FOUND".into() });
            return;
        };
        // Hyperdrive (which steers around anything in the way) until close:
        // the landing and docking approaches only mind their own target.
        let far = at.distance(self.ship.position) > Self::hyperjump_limit(hop);
        match self.avionics.clearance {
            Some(c) if !c.autopilot => self.toggle_autopilot(),
            Some(_) => {}
            None if far => {
                // Far: hyperdrive there, steered by its autopilot.
                self.toggle_hyperdrive();
                self.avionics.hyper_autopilot = true;
                self.set_controls(|c| c.throttle = 1.0);
            }
            None => {
                if self.request_clearance() {
                    self.toggle_autopilot();
                }
            }
        }
    }

    /// Guidance numbers for the HUD, if cleared to dock or land.
    pub fn approach(&mut self) -> Option<Approach> {
        let c = self.avionics.clearance?;
        let sys = self.ship_system();
        sys.positions(self.world.time, &mut self.positions);
        Some(match c.target {
            NavTarget::Station(station) => {
                let frame = StationFrame::new(&sys, station, self.world.time, &self.positions);
                Approach::Dock { station, status: docking::status(&frame, &self.ship, &c) }
            }
            NavTarget::Spaceport(port) => {
                let pad = PadFrame::new(&sys, port, self.world.time, &self.positions);
                let terrain = sys.bodies[sys.spaceports[port].body].terrain.as_ref();
                Approach::Land { port, status: Box::new(landing::status(&pad, &self.ship, &c, terrain)) }
            }
            NavTarget::Gate(g) => {
                let frame = GateFrame::new(&sys, g, self.world.time, &self.positions);
                Approach::Transit { gate: g, status: gate::status(&frame, &self.ship, &c) }
            }
        })
    }

    /// The flight plan to the cleared target: what to do from here, as the
    /// autopilot would do it.
    pub fn plan(&mut self) -> Option<crate::plan::Plan> {
        let c = self.avionics.clearance?;
        if !self.ship.is_flying() || self.ship.hyperdrive {
            return None;
        }
        let sys = self.ship_system();
        Some(crate::plan::plan(&sys, &self.ship, c.target, c.phase, self.world.time))
    }

    /// Docking guidance only (convenience for tests and tools).
    pub fn docking_status(&mut self) -> Option<(usize, DockingStatus)> {
        match self.approach()? {
            Approach::Dock { station, status } => Some((station, status)),
            Approach::Land { .. } | Approach::Transit { .. } => None,
        }
    }

    /// Distance in light years between two stars.
    pub fn distance_ly(&self, a: usize, b: usize) -> f64 {
        self.world.distance_ly(a, b)
    }

    pub fn save(&self) -> UniverseSave {
        UniverseSave {
            seed: self.world.galaxy.seed,
            time: self.world.time,
            ship: self.ship.clone(),
            ship_system: self.ship_system,
            route: self.route.clone(),
            avionics: self.avionics.clone(),
        }
    }

    pub fn load(&mut self, save: UniverseSave) {
        if save.seed != self.world.galaxy.seed {
            *self = Universe::new(save.seed);
        }
        self.world.time = save.time;
        self.ship = save.ship;
        self.ship_system = save.ship_system.min(self.world.galaxy.stars.len() - 1);
        self.route = save.route;
        self.avionics = save.avionics;
        self.events.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Fly the docking computer from `setup`'s position; returns simulated seconds to dock.
    fn autodock(mut u: Universe) -> f64 {
        u.toggle_autopilot();
        assert!(u.avionics.clearance.is_some_and(|d| d.autopilot), "clearance should be granted: {:?}", u.events);
        let start = u.world.time;
        let mut phase = Phase::Approach;
        for _ in 0..(60 * 60 * 3) {
            u.step(1.0 / 60.0, 10.0, &Controls::default());
            if let Some(d) = u.avionics.clearance
                && d.phase != phase
            {
                phase = d.phase;
                eprintln!("t+{:.0}s phase {:?} status {:?}", u.world.time - start, phase, u.docking_status().map(|s| s.1));
            }
            match u.ship.state {
                ShipState::Landed { .. } => return u.world.time - start,
                ShipState::Destroyed { .. } => panic!("crashed: {:?}", u.events),
                ShipState::Flying | ShipState::Transit { .. } => {}
            }
        }
        panic!("did not dock within 30 min; status {:?}", u.docking_status());
    }

    #[test]
    fn docking_computer_docks_from_spawn() {
        let secs = autodock(Universe::new(42));
        eprintln!("docked after {secs:.0} s");
    }

    #[test]
    fn docking_computer_goes_around_from_behind() {
        // Start on the far side of the station from the slot.
        let mut u = Universe::new(42);
        let sys = u.ship_system();
        let station = sys.station().unwrap();
        let mut pos = Vec::new();
        sys.positions(u.world.time, &mut pos);
        let frame = StationFrame::new(&sys, station, u.world.time, &pos);
        u.ship.position = frame.center - frame.axis() * 3000.0 + frame.slot_long() * 200.0;
        u.ship.velocity = frame.velocity;
        let secs = autodock(u);
        eprintln!("docked from behind after {secs:.0} s");
    }

    /// Autopilot to the home planet's spaceport; returns simulated seconds to touchdown.
    fn autoland(mut u: Universe, warp: f64) -> f64 {
        let sys = u.ship_system();
        let planet = sys.bodies[sys.station().unwrap()].rail.parent.unwrap();
        let port = sys.spaceports.iter().position(|p| p.body == planet).expect("home planet has a spaceport");
        u.set_nav_target(Some(NavTarget::Spaceport(port)));
        u.toggle_autopilot();
        assert!(u.avionics.clearance.is_some_and(|c| c.autopilot), "landing clearance: {:?}", u.events);
        let start = u.world.time;
        let mut phase = Phase::Approach;
        let mut next_report = 0.0;
        for _ in 0..(60 * 60 * 20) {
            u.step(1.0 / 60.0, warp, &Controls::default());
            let hd_events: Vec<Event> = u.events.iter().filter(|e| matches!(e, Event::Ship(ShipEvent::HyperdriveEngaged) | Event::Ship(ShipEvent::HyperdriveDisengaged) | Event::HyperdriveArrived { .. })).cloned().collect();
            for e in hd_events {
                let sys = u.ship_system();
                let mut pos = Vec::new();
                sys.positions(u.world.time, &mut pos);
                let dom = sys.dominant(u.ship.position, &pos);
                let alt = u.ship.position.distance(pos[dom]) - sys.bodies[dom].rail.radius;
                let pad = PadFrame::new(&sys, port, u.world.time, &pos);
                eprintln!("  t+{:>5.0}s {e:?}: near {} alt {:.0} km, pad {:.0} km", u.world.time - start, sys.bodies[dom].name, alt / 1000.0, pad.pad.distance(u.ship.position) / 1000.0);
            }
            u.events.retain(|e| !matches!(e, Event::Ship(ShipEvent::HyperdriveEngaged) | Event::Ship(ShipEvent::HyperdriveDisengaged) | Event::HyperdriveArrived { .. }));
            if let Some(Approach::Land { status, .. }) = u.approach()
                && (status.phase != phase || u.world.time - start > next_report)
            {
                phase = status.phase;
                next_report = u.world.time - start + 120.0;
                let surface_alt = (u.ship.position - status.pad.body_center).length() - status.pad.body_radius;
                eprintln!(
                    "t+{:>5.0}s {:?} range {:>10.0} surf-alt {:>8.0} want {:>6.0} m/s speed {:>6.0} vspd {:>7.1} hspd {:>7.1} tilt {:>4.0}deg thr {:.2}",
                    u.world.time - start,
                    status.phase,
                    status.range,
                    surface_alt,
                    status.guidance.desired_velocity.length(),
                    status.relative_velocity.length(),
                    status.vertical_speed,
                    status.horizontal_speed,
                    status.tilt.to_degrees(),
                    u.ship.throttle
                );
            }
            match u.ship.state {
                ShipState::Landed { .. } => {
                    assert!(
                        u.events.iter().any(|e| matches!(e, Event::Ship(ShipEvent::LandedAtPort { .. }))),
                        "landed off the pad: {:?}",
                        u.events
                    );
                    return u.world.time - start;
                }
                ShipState::Destroyed { .. } => panic!("crashed: {:?}", u.events),
                ShipState::Flying | ShipState::Transit { .. } => {}
            }
        }
        panic!("did not land; approach {:?}", u.approach());
    }

    #[test]
    fn autopilot_lands_from_orbit() {
        let secs = autoland(Universe::new(42), 20.0);
        eprintln!("landed after {secs:.0} s");
    }

    #[test]
    fn autopilot_lands_in_the_game_universe() {
        let secs = autoland(Universe::new(1984), 20.0);
        eprintln!("game universe: landed after {secs:.0} s");
    }

    #[test]
    fn autopilot_lands_from_far_side_of_planet() {
        let mut u = Universe::new(42);
        let sys = u.ship_system();
        let planet = sys.bodies[sys.station().unwrap()].rail.parent.unwrap();
        let port = sys.spaceports.iter().position(|p| p.body == planet).unwrap();
        let mut pos = Vec::new();
        sys.positions(u.world.time, &mut pos);
        let pad = PadFrame::new(&sys, port, u.world.time, &pos);
        // Hover-still (in the rotating frame) 1000 km up, over the antipode.
        u.ship.position = pad.body_center - pad.up * (pad.body_radius + 1.0e6);
        u.ship.velocity = pad.frame_velocity(u.ship.position);
        let secs = autoland(u, 50.0);
        eprintln!("landed from the far side after {secs:.0} s");
    }

    #[test]
    fn autopilot_lands_from_high_above() {
        let mut u = Universe::new(42);
        let sys = u.ship_system();
        let planet = sys.bodies[sys.station().unwrap()].rail.parent.unwrap();
        let port = sys.spaceports.iter().position(|p| p.body == planet).unwrap();
        let mut pos = Vec::new();
        sys.positions(u.world.time, &mut pos);
        let pad = PadFrame::new(&sys, port, u.world.time, &pos);
        u.ship.position = pad.pad + pad.up * 50_000.0;
        u.ship.velocity = pad.frame_velocity(u.ship.position);
        autoland(u, 10.0);
    }

    /// A spaceport on a different planet of the home system than the one we start at.
    fn far_port(u: &mut Universe) -> usize {
        let sys = u.ship_system();
        let home_planet = sys.bodies[sys.station().unwrap()].rail.parent.unwrap();
        sys.spaceports.iter().position(|p| p.body != home_planet && sys.bodies[p.body].rail.parent == Some(0)).expect("another planet with a port")
    }

    /// A port on another planet whose pad faces the ship (a straight line to it is clear).
    fn facing_port(u: &mut Universe) -> usize {
        let sys = u.ship_system();
        let home_planet = sys.bodies[sys.station().unwrap()].rail.parent.unwrap();
        let mut pos = Vec::new();
        sys.positions(u.world.time, &mut pos);
        (0..sys.spaceports.len())
            .filter(|&i| sys.spaceports[i].body != home_planet)
            .max_by(|&a, &b| {
                let facing = |i: usize| {
                    let pad = PadFrame::new(&sys, i, u.world.time, &pos);
                    pad.up.dot((u.ship.position - pad.body_center).normalize())
                };
                facing(a).total_cmp(&facing(b))
            })
            .unwrap()
    }

    /// Run hyperdrive until it drops out; returns the distance to the port's pad.
    /// With `pilot_aims`, the nose is re-pointed at the pad every frame, like a
    /// pilot keeping the target marker on the crosshair.
    fn hyper_until_arrival(u: &mut Universe, port: usize, pilot_aims: bool) -> f64 {
        for _ in 0..(60 * 600) {
            if pilot_aims && u.ship.hyperdrive {
                let at = u.target_position(NavTarget::Spaceport(port)).unwrap();
                u.ship.orientation = DQuat::from_rotation_arc(DVec3::NEG_Z, (at - u.ship.position).normalize());
            }
            u.step(1.0 / 60.0, 1.0, &Controls::default());
            if !u.ship.hyperdrive {
                break;
            }
        }
        assert!(!u.ship.hyperdrive, "hyperdrive should have dropped out");
        assert!(u.ship.is_flying(), "{:?}", u.events);
        let sys = u.ship_system();
        let mut pos = Vec::new();
        sys.positions(u.world.time, &mut pos);
        PadFrame::new(&sys, port, u.world.time, &pos).pad.distance(u.ship.position)
    }

    #[test]
    fn manual_hyperdrive_at_a_port_drops_out_there_with_clearance() {
        let mut u = Universe::new(42);
        let port = facing_port(&mut u);
        u.set_nav_target(Some(NavTarget::Spaceport(port)));
        // Point the nose at the pad, as a pilot would at the target marker.
        let at = u.target_position(NavTarget::Spaceport(port)).unwrap();
        u.ship.orientation = DQuat::from_rotation_arc(DVec3::NEG_Z, (at - u.ship.position).normalize());
        u.toggle_hyperdrive();
        u.ship.throttle = 1.0;
        let dist = hyper_until_arrival(&mut u, port, true);
        eprintln!("manual: dropped out {:.0} km from the pad; events {:?}", dist / 1000.0, u.events);
        assert!(dist < computer::HYPER_ARRIVE_PORT, "dropped out too far: {dist}");
        assert!(u.events.iter().any(|e| matches!(e, Event::HyperdriveArrived { .. })));
        assert!(u.avionics.clearance.is_none(), "clearance is the pilot's call, not automatic");
    }

    #[test]
    fn hyperdrive_autopilot_steers_to_the_port() {
        let mut u = Universe::new(42);
        let port = far_port(&mut u);
        u.set_nav_target(Some(NavTarget::Spaceport(port)));
        u.toggle_hyperdrive();
        u.toggle_autopilot();
        assert!(u.avionics.hyper_autopilot);
        u.ship.throttle = 1.0;
        let dist = hyper_until_arrival(&mut u, port, false);
        eprintln!("autopilot: dropped out {:.0} km from the pad", dist / 1000.0);
        assert!(dist < computer::HYPER_ARRIVE_PORT);
    }

    #[test]
    fn plan_reaches_the_pad_and_the_slot() {
        // Landing from orbit.
        let mut u = Universe::new(42);
        let sys = u.ship_system();
        let planet = sys.bodies[sys.station().unwrap()].rail.parent.unwrap();
        let port = sys.spaceports.iter().position(|p| p.body == planet).unwrap();
        u.set_nav_target(Some(NavTarget::Spaceport(port)));
        assert!(u.request_clearance());
        let start = std::time::Instant::now();
        let plan = u.plan().unwrap();
        eprintln!(
            "landing plan: {} points, {:.0} s, arrives {}, first action {:?}, built in {:?}",
            plan.points.len(),
            plan.points.last().unwrap().time,
            plan.arrives,
            plan.points[0].action,
            start.elapsed()
        );
        assert!(plan.arrives, "landing plan should reach the pad");

        // Docking from the spawn point.
        let mut u = Universe::new(42);
        u.set_nav_target(None);
        assert!(u.request_clearance());
        let plan = u.plan().unwrap();
        eprintln!("docking plan: {} points, {:.0} s, arrives {}", plan.points.len(), plan.points.last().unwrap().time, plan.arrives);
        assert!(plan.arrives, "docking plan should reach the slot");
    }

    /// Position and roll-free "up" of a plan at absolute time `at`.
    fn plan_at(plan: &crate::plan::Plan, start: f64, at: f64) -> (DVec3, DVec3) {
        let rel = at - start;
        let i = plan.points.iter().position(|p| p.time >= rel).unwrap_or(plan.points.len() - 1).max(1);
        let (a, b) = (plan.points[i - 1], plan.points[i]);
        let u = ((rel - a.time) / (b.time - a.time).max(1e-9)).clamp(0.0, 1.0);
        (a.position.lerp(b.position, u), a.orientation.slerp(b.orientation, u) * DVec3::Y)
    }

    #[test]
    fn plan_stays_put_while_the_autopilot_follows_it() {
        for label in ["landing", "docking"] {
            let mut u = Universe::new(1984);
            let sys = u.ship_system();
            let station = sys.station().unwrap();
            let planet = sys.bodies[station].rail.parent.unwrap();
            // Plans are drawn relative to the target as it is when planned:
            // this is that reference body, and its spin (zero for stations).
            let (body, omega) = if label == "landing" {
                let port = sys.spaceports.iter().position(|p| p.body == planet).unwrap();
                u.set_nav_target(Some(NavTarget::Spaceport(port)));
                // Within hyperjump range, so the landing autopilot flies it all.
                let mut pos = Vec::new();
                sys.positions(u.world.time, &mut pos);
                let pad = PadFrame::new(&sys, port, u.world.time, &pos);
                u.ship.position = pad.pad + (pad.up + pad.up.any_orthonormal_vector() * 0.5).normalize() * 150_000.0;
                u.ship.velocity = pad.frame_velocity(u.ship.position);
                (planet, sys.bodies[planet].angular_velocity())
            } else {
                (station, DVec3::ZERO)
            };
            u.toggle_autopilot();
            let mut pos = Vec::new();
            let t0 = u.world.time;
            sys.positions(t0, &mut pos);
            let c0 = pos[body];
            let first = u.plan().unwrap();
            for _ in 0..600 {
                u.step(1.0 / 60.0, 1.0, &Controls::default());
            }
            let t1 = u.world.time;
            sys.positions(t1, &mut pos);
            let c1 = pos[body];
            let second = u.plan().unwrap();
            let end = t0 + first.points.last().unwrap().time.min(second.points.last().unwrap().time + (t1 - t0));
            // Re-express the first plan in the second plan's reference: move with
            // the body, and turn with it for the extra time.
            let spin = DQuat::from_scaled_axis(omega * (t1 - t0));
            let mut worst_pos: f64 = 0.0;
            let mut worst_roll: f64 = 0.0;
            let mut rolled = 0;
            for k in 1..=10 {
                let at = t1 + (end - t1) * k as f64 / 10.0;
                let (p0, up0) = plan_at(&first, t0, at);
                let (p1, up1) = plan_at(&second, t1, at);
                let p0 = c1 + spin * (p0 - c0);
                let up0 = spin * up0;
                let scale = p1.distance(c1 + spin * (u.ship.position - c1)).max(100.0).min(p1.distance(u.ship.position).max(100.0));
                worst_pos = worst_pos.max(p0.distance(p1) / scale);
                let roll = up0.angle_between(up1).to_degrees();
                worst_roll = worst_roll.max(roll);
                if roll > 10.0 {
                    rolled += 1;
                }
            }
            eprintln!("{label}: plans 10 s apart differ by at most {:.1}% of distance, {:.1} deg of roll", worst_pos * 100.0, worst_roll);
            assert!(worst_pos < 0.05, "{label}: plan moved {:.1}%", worst_pos * 100.0);
            // A planned turn (e.g. from routing around the planet to heading
            // straight in) can land a few seconds apart in two plans; allow
            // one such sample, but frames must otherwise hold their attitude.
            assert!(rolled <= 1, "{label}: frames rolled at {rolled} of 10 samples (worst {worst_roll:.1} deg)");
        }
    }

    #[test]
    fn autopilot_flies_through_a_gate_and_keeps_its_motion() {
        let mut u = Universe::new(1984);
        let home = u.world.home_system;
        let sys = u.ship_system();
        let (dest, _) = u.gate_links_of(home)[0].clone();
        let g = sys.gate_to(dest).unwrap();
        // Start 30 km from the gate, co-moving with it.
        let mut pos = Vec::new();
        sys.positions(u.world.time, &mut pos);
        let frame = GateFrame::new(&sys, g, u.world.time, &pos);
        u.ship.position = frame.center + (frame.rotation * DVec3::new(1.0, 0.5, 0.3)).normalize() * 30_000.0;
        u.ship.velocity = frame.velocity;
        u.set_nav_target(Some(NavTarget::Gate(g)));
        u.toggle_autopilot();
        assert!(u.avionics.clearance.is_some_and(|c| c.autopilot), "{:?}", u.events);
        let plan_eta = u.plan().unwrap();
        assert!(plan_eta.arrives, "the plan should reach the gate");
        let mut entered = None;
        for _ in 0..(60 * 60 * 20) {
            u.step(1.0 / 60.0, 10.0, &Controls::default());
            if let ShipState::Transit { local_velocity, .. } = u.ship.state
                && entered.is_none()
            {
                entered = Some(local_velocity);
            }
            if entered.is_some() && u.ship.is_flying() {
                break;
            }
            assert!(!matches!(u.ship.state, ShipState::Destroyed { .. }), "crashed: {:?}", u.events);
        }
        let entry_local = entered.expect("should have entered the gate");
        assert_eq!(u.ship_system, dest, "arrived in the linked system");
        // Same motion relative to the arrival gate.
        let sys = u.ship_system();
        let back = sys.gate_to(home).unwrap();
        sys.positions(u.world.time, &mut pos);
        let out = GateFrame::new(&sys, back, u.world.time, &pos);
        let exit_local = out.rotation.inverse() * (u.ship.velocity - out.velocity);
        eprintln!("entry {entry_local:.1?} exit {exit_local:.1?}; plan eta {:.0} s", plan_eta.points.last().unwrap().time);
        assert!((exit_local - entry_local).length() < 1.0);
        assert!(u.ship.position.distance(out.center) < universe_world::gate::GATE_RADIUS, "came out of the ring");
        assert!(u.events.iter().any(|e| matches!(e, Event::Ship(ShipEvent::GateArrived { .. }))));
    }

    #[test]
    fn hyperdrive_catches_a_moving_gate_at_part_throttle_and_stops_with_it() {
        let mut u = Universe::new(1984);
        let home = u.world.home_system;
        let sys = u.ship_system();
        let (dest, _) = u.gate_links_of(home)[0].clone();
        let g = sys.gate_to(dest).unwrap();
        u.set_nav_target(Some(NavTarget::Gate(g)));
        u.toggle_hyperdrive();
        u.ship.throttle = 0.3;
        for _ in 0..(60 * 600) {
            let at = u.target_position(NavTarget::Gate(g)).unwrap();
            u.ship.orientation = DQuat::from_rotation_arc(DVec3::NEG_Z, (at - u.ship.position).normalize());
            u.step(1.0 / 60.0, 1.0, &Controls::default());
            if !u.ship.hyperdrive {
                break;
            }
        }
        assert!(!u.ship.hyperdrive, "should arrive at the gate at 30% throttle");
        let mut pos = Vec::new();
        sys.positions(u.world.time, &mut pos);
        let rel = u.ship.velocity - sys.velocity(g, u.world.time);
        let dist = pos[g].distance(u.ship.position);
        eprintln!("arrived {:.1} km from the gate, drifting {:.2} m/s relative", dist / 1000.0, rel.length());
        assert!(dist < 20_000.0);
        assert!(rel.length() < 1.0, "should be at rest relative to the gate");

        // Manual drop-out near the target also matches it.
        u.toggle_hyperdrive();
        u.ship.throttle = 0.1;
        for _ in 0..30 {
            u.step(1.0 / 60.0, 1.0, &Controls::default());
        }
        u.toggle_hyperdrive();
        let rel = u.ship.velocity - sys.velocity(g, u.world.time);
        assert!(rel.length() < 1.0, "manual drop-out near the gate matches it: {:.1} m/s", rel.length());
    }

    #[test]
    fn settler_routes_are_reproducible() {
        let mut u = Universe::new(1984);
        let a = u.settler_route(7, 10);
        let b = u.settler_route(7, 10);
        let c = u.settler_route(8, 10);
        assert_eq!(a.len(), 10);
        assert_eq!(a, b, "same seed, same route");
        assert_ne!(a, c, "different seed, different route");
        assert!(a.windows(2).all(|w| w[0] != w[1]));
        let systems: std::collections::HashSet<usize> = a.iter().map(|s| s.system).collect();
        eprintln!("route 7 visits {} systems: {:?}", systems.len(), a.iter().map(|s| (s.system, s.target)).collect::<Vec<_>>());
    }

    /// Fly a route headless; returns (stops reached, completed).
    fn fly_route(u: &mut Universe, warp: f64, max_frames: usize) -> (usize, bool) {
        u.toggle_route();
        let (mut reached, mut done) = (0, false);
        for frame in 0..max_frames {
            u.step(1.0 / 60.0, warp, &Controls::default());
            for e in std::mem::take(&mut u.events) {
                match &e {
                    Event::RouteStop { .. } => reached += 1,
                    Event::RouteComplete => done = true,
                    Event::Ship(ShipEvent::Crashed { .. }) | Event::RouteBlocked { .. } => panic!("route failed at frame {frame}: {e:?}"),
                    _ => {}
                }
                if matches!(e, Event::RouteStop { .. } | Event::RouteComplete | Event::Ship(ShipEvent::GateEntered { .. }) | Event::Ship(ShipEvent::GateArrived { .. }) | Event::Ship(ShipEvent::Landed { .. }) | Event::Ship(ShipEvent::LandedAtPort { .. }))
                {
                    eprintln!("  t+{:>7.0} s (frame {frame:>6}): {e:?}", u.world.time);
                }
            }
            if done {
                break;
            }
        }
        (reached, done)
    }

    /// A whole 10-stop settler route, headless.
    /// `cargo test -p universe-sim --release settler_route_flies -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn settler_route_flies_to_completion() {
        let mut u = Universe::new(1984);
        u.route.stops = u.settler_route(7, 10);
        let names: Vec<String> = u.route.stops.clone().into_iter().map(|s| u.stop_name(s)).collect();
        eprintln!("route: {names:#?}");
        let t0 = u.world.time;
        let (reached, done) = fly_route(&mut u, 20.0, 60 * 60 * 60 * 3);
        eprintln!("reached {reached} stops, complete {done}, {:.1} game hours", (u.world.time - t0) / 3600.0);
        assert!(done);
    }

    #[test]
    fn route_autopilot_lands_docks_and_crosses_a_gate() {
        let mut u = Universe::new(1984);
        let home = u.world.home_system;
        let sys = u.ship_system();
        let station = sys.station().unwrap();
        let planet = sys.bodies[station].rail.parent.unwrap();
        let port = sys.spaceports.iter().position(|p| p.body == planet).unwrap();
        let (next, _) = u.gate_links_of(home)[0].clone();
        let there = u.system(next);
        let far = there
            .station()
            .map(|s| Stop { system: next, target: NavTarget::Station(s) })
            .unwrap_or(Stop { system: next, target: NavTarget::Spaceport(0) });
        u.route.stops = vec![
            Stop { system: home, target: NavTarget::Spaceport(port) },
            Stop { system: home, target: NavTarget::Station(station) },
            far,
        ];
        let (reached, done) = fly_route(&mut u, 10.0, 60 * 60 * 90);
        assert_eq!(reached, 3, "all stops reached");
        assert!(done);
        assert_eq!(u.ship_system, next);
    }

    /// Run many settlers headless: frame cost and what they got up to.
    /// `cargo test -p universe-sim --release settlers_fly -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn settlers_fly() {
        for (count, warp, frames) in [(100, 20.0, 60 * 60 * 30), (1000, 20.0, 60 * 60 * 3), (1000, 2.0, 60 * 30)] {
            let mut u = Universe::new(1984);
            u.spawn_settlers(count, 99);
            let t0 = u.world.time;
            let start = std::time::Instant::now();
            for _ in 0..frames {
                u.step_world(1.0 / 60.0, warp, &Controls::default());
            }
            let ms = start.elapsed().as_secs_f64() * 1000.0 / frames as f64;
            let flying = u.crafts.iter().filter(|c| c.ship.is_flying()).count();
            for r in &u.crash_log {
                eprintln!("    crash: {r:?}");
            }
            eprintln!(
                "{count} settlers at {warp}x for {:.1} game h: {ms:.2} ms/frame; stops {}, transits {}, routes done {}, crashes {}; {flying} flying now",
                (u.world.time - t0) / 3600.0,
                u.traffic.stops,
                u.traffic.transits,
                u.traffic.routes_completed,
                u.traffic.crashes
            );
        }
    }

    /// Autoland at every moon port in the gate network, from 150 km out.
    #[test]
    #[ignore]
    fn autoland_on_every_moon_port() {
        let base = Universe::new(1984);
        let mut systems: Vec<usize> = base.world.gate_links.iter().flat_map(|&(a, b)| [a, b]).collect();
        systems.sort();
        systems.dedup();
        for s in systems {
            let mut probe = Universe::new(1984);
            let sys = probe.system(s);
            for (p, sp) in sys.spaceports.iter().enumerate() {
                if sys.bodies[sp.body].kind != BodyKind::Moon {
                    continue;
                }
                let mut u = Universe::new(1984);
                u.ship_system = s;
                let sys = u.ship_system();
                let mut pos = Vec::new();
                sys.positions(u.world.time, &mut pos);
                let pad = PadFrame::new(&sys, p, u.world.time, &pos);
                let side = pad.up.any_orthonormal_vector();
                u.ship.position = pad.pad + (pad.up * 0.6 + side * 0.8).normalize() * 150_000.0;
                u.ship.velocity = pad.frame_velocity(u.ship.position);
                u.ship.state = ShipState::Flying;
                u.set_nav_target(Some(NavTarget::Spaceport(p)));
                u.toggle_autopilot();
                let t0 = u.world.time;
                let mut outcome = "timeout".to_string();
                for _ in 0..(60 * 60 * 20) {
                    u.step(1.0 / 60.0, 10.0, &Controls::default());
                    if let Some(e) = u.events.iter().find(|e| matches!(e, Event::Ship(ShipEvent::Crashed { .. }) | Event::Ship(ShipEvent::LandedAtPort { .. }) | Event::Ship(ShipEvent::Landed { .. }))).cloned() {
                        let st = u.approach();
                        outcome = format!("{e:?} after {:.0} s; last status {:?}", u.world.time - t0, st.map(|a| match a {
                            Approach::Land { status, .. } => (status.phase, status.altitude as i64, status.vertical_speed as i64, status.horizontal_speed as i64),
                            _ => (Phase::Approach, 0, 0, 0),
                        }));
                        break;
                    }
                    u.events.clear();
                }
                let b = &sys.bodies[sp.body];
                eprintln!("{} on {} (R {:.0} km, g {:.2}): {outcome}", sp.name, b.name, b.rail.radius / 1000.0, b.rail.mu / (b.rail.radius * b.rail.radius));
            }
        }
    }

    #[test]
    fn settlers_are_reproducible_and_move() {
        let run = || {
            let mut u = Universe::new(1984);
            u.spawn_settlers(10, 5);
            for _ in 0..(60 * 60 * 2) {
                u.step_world(1.0 / 60.0, 20.0, &Controls::default());
            }
            (u.traffic.stops, u.traffic.crashes, u.crafts.iter().map(|c| (c.system, c.route.next)).collect::<Vec<_>>())
        };
        let (a, b) = (run(), run());
        eprintln!("10 settlers, 0.7 game h: {} stops, {} crashes", a.0, a.1);
        assert_eq!(a, b, "same seed and steps, same world");
        assert!(a.0 >= 10, "settlers should be visiting stops");
        assert_eq!(a.1, 0, "no crashes");
    }

    #[test]
    fn save_round_trips_through_json() {
        let mut u = Universe::new(7);
        u.ship.throttle = 0.5;
        let station = u.ship_system().station().unwrap();
        u.set_nav_target(Some(NavTarget::Station(station)));
        for _ in 0..60 {
            u.step(1.0 / 60.0, 100.0, &Controls::default());
        }
        let json = serde_json::to_string(&u.save()).unwrap();
        let mut restored = Universe::new(7);
        restored.load(serde_json::from_str(&json).unwrap());
        assert_eq!(restored.world.time, u.world.time);
        assert_eq!(restored.ship.position, u.ship.position);
        assert_eq!(restored.ship_system, u.ship_system);
        assert_eq!(restored.avionics.nav_target, u.avionics.nav_target);
    }
}

#[cfg(test)]
mod probe {
    use super::*;

    /// `cargo test -p universe-sim --release bench_navigation -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn bench_navigation_costs() {
        use std::hint::black_box;
        use std::time::Instant;
        let per = |label: &str, n: u32, f: &mut dyn FnMut()| {
            let t = Instant::now();
            for _ in 0..n {
                f();
            }
            let us = t.elapsed().as_secs_f64() * 1e6 / n as f64;
            eprintln!("{label:<44} {us:>10.2} us");
            us
        };

        let mut u = Universe::new(1984);
        let sys = u.ship_system();
        eprintln!("home system: {} bodies", sys.bodies.len());
        let mut pos = Vec::new();
        let mut t = 0.0;
        per("body positions (all bodies, one time)", 100_000, &mut || {
            t += 1.0;
            sys.positions(black_box(t), &mut pos);
        });
        let p = u.ship.position;
        per("gravity sum at a point", 100_000, &mut || {
            black_box(sys.gravity(black_box(p), &pos));
        });
        let planet = sys.bodies[sys.station().unwrap()].rail.parent.unwrap();
        let body = &sys.bodies[planet];
        let terrain = body.terrain.as_ref().unwrap();
        let mut d = DVec3::new(0.3, 0.4, 0.5).normalize();
        per("terrain height lookup", 100_000, &mut || {
            d = (d + DVec3::splat(1e-4)).normalize();
            black_box(terrain.surface(d));
        });

        let frame = 1.0 / 60.0;
        let frames = 600;
        let run = |label: &str, u: &mut Universe, warp: f64| {
            let t = Instant::now();
            for _ in 0..frames {
                u.step(frame, warp, &Controls::default());
            }
            let us = t.elapsed().as_secs_f64() * 1e6 / frames as f64;
            eprintln!("{label:<44} {us:>10.2} us/frame");
        };

        let mut v = Universe::new(1984);
        v.ship.position += DVec3::new(0.0, 0.0, 200_000.0); // away from the station's fine stepping
        run("ship coasting in orbit, warp 1", &mut v, 1.0);
        run("ship coasting in orbit, warp 1000", &mut v, 1000.0);
        let mut v = Universe::new(1984);
        run("ship near a station (fine steps), warp 1", &mut v, 1.0);
        let mut v = Universe::new(1984);
        v.toggle_autopilot();
        run("docking autopilot, warp 1", &mut v, 1.0);
        let mut v = Universe::new(1984);
        let port = v.ship_system().spaceports.iter().position(|sp| sp.body == planet).unwrap();
        v.set_nav_target(Some(NavTarget::Spaceport(port)));
        v.toggle_autopilot();
        run("landing autopilot, warp 1", &mut v, 1.0);
        run("landing autopilot, warp 100", &mut v, 100.0);
        let mut v = Universe::new(1984);
        let (dest, _) = v.gate_links_of(v.world.home_system)[0].clone();
        let g = v.ship_system().gate_to(dest).unwrap();
        v.set_nav_target(Some(NavTarget::Gate(g)));
        v.toggle_hyperdrive();
        v.ship.throttle = 0.3;
        run("hyperdrive (targeted), warp 1", &mut v, 1.0);
        let mut v = Universe::new(1984);
        let sysv = v.ship_system();
        sysv.positions(v.world.time, &mut pos);
        let up = (v.ship.position - pos[planet]).normalize();
        v.ship.position = pos[planet] + up * (sysv.bodies[planet].surface_radius_at(pos[planet], pos[planet] + up, v.world.time) + 3000.0);
        v.ship.velocity = sysv.velocity(planet, v.world.time) + sysv.bodies[planet].angular_velocity().cross(v.ship.position - pos[planet]);
        v.ship.throttle = 0.4;
        v.ship.orientation = crate::ship::facing(up, up.any_orthonormal_vector());
        run("near the ground (terrain collision), warp 1", &mut v, 1.0);

        let mut v = Universe::new(1984);
        v.set_nav_target(Some(NavTarget::Spaceport(port)));
        v.request_clearance();
        per("flight planner (landing from orbit)", 50, &mut || {
            black_box(v.plan());
        });
        let mut v = Universe::new(1984);
        v.request_clearance();
        per("flight planner (docking)", 50, &mut || {
            black_box(v.plan());
        });
        let _ = &mut u;
    }

    /// Plan ETA vs. how long the autopilot really takes.
    /// `cargo test -p universe-sim --release eta_accuracy -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn eta_accuracy() {
        let run = |label: &str, mut u: Universe, done: &dyn Fn(&Universe) -> bool| {
            u.toggle_autopilot();
            let start = u.world.time;
            let eta0 = u.plan().map(|p| p.points.last().unwrap().time).unwrap_or(f64::NAN);
            let mut samples = Vec::new();
            for i in 0..(60 * 60 * 30) {
                if i % 600 == 0
                    && let Some(p) = u.plan()
                {
                    samples.push((u.world.time - start, u.world.time - start + p.points.last().unwrap().time));
                }
                u.step(1.0 / 60.0, 5.0, &Controls::default());
                if done(&u) {
                    break;
                }
            }
            let actual = u.world.time - start;
            eprintln!("{label}: planned {eta0:.0} s, actual {actual:.0} s ({:+.0}%)", (actual / eta0 - 1.0) * 100.0);
            for (t, arrive) in samples.iter().step_by((samples.len() / 8).max(1)) {
                eprintln!("    at {t:>6.0} s the plan said arrival at {arrive:>6.0} s");
            }
        };
        let landed = |u: &Universe| matches!(u.ship.state, ShipState::Landed { .. });
        run("docking from spawn", Universe::new(1984), &landed);

        let mut u = Universe::new(1984);
        let sys = u.ship_system();
        let planet = sys.bodies[sys.station().unwrap()].rail.parent.unwrap();
        let port = sys.spaceports.iter().position(|p| p.body == planet).unwrap();
        u.set_nav_target(Some(NavTarget::Spaceport(port)));
        run("landing from orbit", u, &landed);

        let mut u = Universe::new(1984);
        let home = u.world.home_system;
        let sys = u.ship_system();
        let (dest, _) = u.gate_links_of(home)[0].clone();
        let g = sys.gate_to(dest).unwrap();
        let mut pos = Vec::new();
        sys.positions(u.world.time, &mut pos);
        let frame = GateFrame::new(&sys, g, u.world.time, &pos);
        u.ship.position = frame.center + (frame.rotation * DVec3::new(1.0, 0.5, 0.3)).normalize() * 30_000.0;
        u.ship.velocity = frame.velocity;
        u.set_nav_target(Some(NavTarget::Gate(g)));
        run("gate from 30 km", u, &|u: &Universe| matches!(u.ship.state, ShipState::Transit { .. }));
    }

    /// Reproduce the HUD's ETA countdown frame by frame (60 fps, plan rebuilt
    /// 10×/s) and return the biggest frame-to-frame jump (real seconds).
    fn eta_jumps(label: &str, mut u: Universe, warp: f64, max_frames: usize) -> f64 {
        u.toggle_autopilot();
        // No plan (and no ETA) while the autopilot is in hyperdrive; measure the rest.
        let mut plan = u.plan();
        let mut shown: Vec<f64> = Vec::new();
        for frame in 0..max_frames {
            if frame % 6 == 0 {
                plan = u.plan();
                if let Some(p) = &plan {
                    assert!(p.arrives, "{label}: a plan failed to arrive at frame {frame}");
                } else {
                    shown.clear(); // a hyperjump: the countdown starts fresh afterwards
                }
            }
            if let Some(p) = &plan {
                let left = p.points.last().unwrap().time - (u.world.time - p.start);
                shown.push(left / warp);
            }
            u.step(1.0 / 60.0, warp, &Controls::default());
            if !u.ship.is_flying() {
                break;
            }
        }
        let worst = shown.windows(2).map(|w| (w[1] - w[0]).abs()).fold(0.0, f64::max);
        eprintln!("{label}: {} frames, first ETA {:.0} s, biggest jump {worst:.2} s", shown.len(), shown.first().copied().unwrap_or(0.0));
        worst
    }

    #[test]
    fn eta_counts_down_smoothly() {
        // Docking at the default 2x.
        assert!(eta_jumps("docking", Universe::new(1984), 2.0, 60 * 120) < 0.5);
        // A gate from 30 km.
        let mut u = Universe::new(1984);
        let home = u.world.home_system;
        let sys = u.ship_system();
        let (dest, _) = u.gate_links_of(home)[0].clone();
        let g = sys.gate_to(dest).unwrap();
        let mut pos = Vec::new();
        sys.positions(u.world.time, &mut pos);
        let frame = GateFrame::new(&sys, g, u.world.time, &pos);
        u.ship.position = frame.center + (frame.rotation * DVec3::new(1.0, 0.5, 0.3)).normalize() * 30_000.0;
        u.ship.velocity = frame.velocity;
        u.set_nav_target(Some(NavTarget::Gate(g)));
        assert!(eta_jumps("gate", u, 2.0, 60 * 200) < 0.5);
    }

    #[test]
    #[ignore]
    fn eta_counts_down_smoothly_landing() {
        let mut u = Universe::new(1984);
        let sys = u.ship_system();
        let planet = sys.bodies[sys.station().unwrap()].rail.parent.unwrap();
        let port = sys.spaceports.iter().position(|p| p.body == planet).unwrap();
        u.set_nav_target(Some(NavTarget::Spaceport(port)));
        // Warp 20 to keep the run short; jumps are measured in real seconds at that rate.
        let worst = eta_jumps("landing", u, 20.0, 60 * 60 * 10);
        assert!(worst < 5.0);
    }

    #[test]
    #[ignore]
    fn debug_hyper_path() {
        let mut u = Universe::new(42);
        let sys = u.ship_system();
        let planet = sys.bodies[sys.station().unwrap()].rail.parent.unwrap();
        let port = sys.spaceports.iter().position(|p| p.body == planet).unwrap();
        u.set_nav_target(Some(NavTarget::Spaceport(port)));
        u.toggle_autopilot();
        for i in 0..(60 * 300) {
            u.step(1.0 / 60.0, 20.0, &Controls::default());
            if i % 60 == 0 && u.ship.hyperdrive {
                let mut pos = Vec::new();
                sys.positions(u.world.time, &mut pos);
                let c = pos[planet];
                let r = sys.bodies[planet].rail.radius;
                let alt = |x: DVec3| (x.distance(c) - r) / 1000.0;
                let way = u.debug_way.unwrap_or(DVec3::ZERO);
                let pad = PadFrame::new(&sys, port, u.world.time, &pos);
                eprintln!("  {:>4}s alt {:>8.0} km  way alt {:>8.0} km  angle to pad {:>5.1} deg  speed {:>8.0} km/s", i / 60, alt(u.ship.position), alt(way), ((u.ship.position - c).angle_between(pad.up)).to_degrees(), u.ship.velocity.length() / 1000.0);
            }
            if !u.ship.hyperdrive && i > 60 * 5 {
                break;
            }
        }
        eprintln!("R = {:.0} km", sys.bodies[planet].rail.radius / 1000.0);
    }

    #[test]
    #[ignore]
    fn debug_landing_start() {
        let mut u = Universe::new(1984);
        let sys = u.ship_system();
        let planet = sys.bodies[sys.station().unwrap()].rail.parent.unwrap();
        let port = sys.spaceports.iter().position(|p| p.body == planet).unwrap();
        u.set_nav_target(Some(NavTarget::Spaceport(port)));
        let mut pos = Vec::new();
        sys.positions(u.world.time, &mut pos);
        let pad = PadFrame::new(&sys, port, u.world.time, &pos);
        u.ship.position = pad.pad + (pad.up + pad.up.any_orthonormal_vector() * 0.5).normalize() * 150_000.0;
        u.ship.velocity = pad.frame_velocity(u.ship.position);
        u.toggle_autopilot();
        let plan = u.plan().unwrap();
        for p in plan.points.iter().take(12) {
            eprintln!("  plan t {:>5.1} {:?} {:?}", p.time, p.phase, p.action);
        }
        for i in 0..600 {
            if i % 30 == 0 {
                let aim = u.approach().and_then(|a| match a { Approach::Land { status, .. } => Some(status.guidance.desired_velocity.length()), _ => None });
                eprintln!("  real t {:>5.1} throttle {:.2} rcs {:.2?} hyper {} want {:?}", i as f64 / 60.0, u.ship.throttle, u.ship.rcs, u.ship.hyperdrive, aim);
            }
            u.step(1.0 / 60.0, 1.0, &Controls::default());
        }
    }

    #[test]
    #[ignore]
    fn print_game_landing_plan() {
        let mut u = Universe::new(1984);
        let sys = u.ship_system();
        let planet = sys.bodies[sys.station().unwrap()].rail.parent.unwrap();
        let port = sys.spaceports.iter().position(|p| p.body == planet).unwrap();
        u.set_nav_target(Some(NavTarget::Spaceport(port)));
        assert!(u.request_clearance());
        for frames in [0, 1, 2, 30, 120] {
            let mut v = Universe::new(1984);
            v.set_nav_target(Some(NavTarget::Spaceport(port)));
            v.request_clearance();
            for _ in 0..frames {
                v.step(1.0 / 60.0, 1.0, &Controls::default());
            }
            let p = v.plan().unwrap();
            let l = p.points.last().unwrap();
            eprintln!("after {frames:>3} frames: points {} last t {:.0} arrives {} last phase {:?} action {:?}", p.points.len(), l.time, p.arrives, l.phase, l.action);
        }
        let plan = u.plan().unwrap();
        let last = plan.points.last().unwrap();
        eprintln!("points {} last t {:.0} arrives {} last phase {:?}", plan.points.len(), last.time, plan.arrives, last.phase);
        for p in plan.points.iter().step_by(plan.points.len() / 12 + 1) {
            eprintln!("  t {:>7.0} {:?} {:?}", p.time, p.phase, p.action);
        }
    }

    #[test]
    #[ignore]
    fn print_guidance() {
        let mut u = Universe::new(1984);
        let sys = u.ship_system();
        let station = sys.station().unwrap();
        let mut pos = Vec::new();
        sys.positions(u.world.time, &mut pos);
        let f = StationFrame::new(&sys, station, u.world.time, &pos);
        let to_station = |p: DVec3| (f.center - p).normalize();
        let report = |label: &str, p: DVec3| {
            let g = docking::guidance(&f, p, docking::in_final_zone(&f, p), u.ship.side_accel());
            let r = p - f.center;
            eprintln!(
                "{label:10} height {:7.0} lateral {:7.0} | desired {:6.1} m/s, cos(toward station) {:+.2} | waypoint height {:7.0} lateral {:7.0}",
                r.dot(f.axis()),
                (r - f.axis() * r.dot(f.axis())).length(),
                g.desired_velocity.length(),
                g.desired_velocity.normalize_or_zero().dot(to_station(p)),
                (g.waypoint - f.center).dot(f.axis()),
                ((g.waypoint - f.center) - f.axis() * (g.waypoint - f.center).dot(f.axis())).length(),
            );
        };
        report("spawn", u.ship.position);
        report("side 6km", f.center + f.slot_long() * 6000.0);
        report("above 8km", f.on_axis(8000.0));
        report("above 3km", f.on_axis(3000.0) + f.slot_long() * 100.0);
        report("level 2km", f.center + f.slot_short() * 2000.0);
        report("below 3km", f.center - f.axis() * 3000.0);
        report("far 30km", f.center + (f.slot_long() + f.axis() * 0.2).normalize() * 30000.0);
    }
}
