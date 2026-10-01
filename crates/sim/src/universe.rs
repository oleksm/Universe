//! The universe as the game runs it: the world, the player's ship with its
//! avionics, and the settlers' crafts with theirs, advanced one tick at a
//! time in a fixed order — the player's ship first, then every craft, each
//! from the same moment (see `vessel` for what a ship's turn is).

use std::sync::Arc;

use glam::{DQuat, DVec3};
use universe_avionics::route::{self, Stop};
use universe_avionics::{Approach, Avionics, Event, NavTarget, Plan};
use universe_world::{Controls, Facility, Person, Ship, ShipCommands, ShipEvent, ShipState, StarSystem, StepResult, WalkCommands, World};

use crate::traffic::{CrashReport, Craft, TrafficStats};
use crate::vessel::Vessel;


/// The longest tick (game seconds): pilots act once a tick.
pub const TICK: f64 = 1.0 / 60.0 + 1e-9;
/// The most ticks a step may take (beyond it, under heavy warp, ticks stretch).
pub const TICK_BUDGET: usize = 8;

/// A ship on its final run this close to the station or gate lets the next one start (m).
const CORRIDOR_RELEASE: f64 = 1_500.0;


/// What a new pilot starts with (credits).
pub const STARTING_CREDITS: f64 = 1000.0;

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
    /// The pilot's money (credits).
    pub credits: f64,
    /// The flight recorder: every ship's last seconds, and the wrecks filed (see `recorder`).
    pub recorder: crate::recorder::Recorder,
    /// Recent trades by settlers, most recent last.
    pub trade_log: Vec<crate::commerce::TradeRecord>,
    positions: Vec<DVec3>,
    /// Aggressed ships flying this frame: system and position (who's worth judging).
    pub(crate) aggressors: Vec<(usize, DVec3)>,
    /// Every ship as it was at the start of the frame (by combat id), and
    /// when that was: what each sees of the others, so a ship stepped earlier
    /// in the frame isn't seen where it will be at its end.
    pub(crate) snaps: Vec<crate::traffic::Snap>,
    pub(crate) snap_time: f64,
    /// Ticks run so far.
    pub tick: u64,
    /// Crafts' commands reach their devices this many ticks after they're
    /// given (0: at once). Tests set it to the lag pilots will have once they
    /// run apart from the world.
    pub command_delay: usize,
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
            credits: STARTING_CREDITS,
            recorder: Default::default(),
            trade_log: Vec::new(),
            aggressors: Vec::new(),
            snaps: Vec::new(),
            snap_time: f64::NAN,
            tick: 0,
            command_delay: 0,
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
    pub fn system(&mut self, i: usize) -> Arc<StarSystem> {
        self.world.system(i)
    }

    pub fn ship_system(&mut self) -> Arc<StarSystem> {
        self.world.system(self.ship_system)
    }

    /// Distance in light years between two stars.
    pub fn distance_ly(&self, a: usize, b: usize) -> f64 {
        self.world.distance_ly(a, b)
    }

    /// The player's ship and its avionics, and the world they're in.
    pub(crate) fn player(&mut self) -> (&mut World, Vessel<'_>) {
        let vessel = Vessel { id: crate::combat::PLAYER, ship: &mut self.ship, system: &mut self.ship_system, avionics: &mut self.avionics, events: &mut self.events };
        (&mut self.world, vessel)
    }

    /// Advance the player's ship by `real_dt * warp` seconds (less if warp is
    /// limited), the pilot's stick at `controls`. The world clock moves with it.
    pub fn step(&mut self, real_dt: f64, warp: f64, controls: &Controls) -> StepResult {
        let before = self.events.len();
        // Following something: the program has the stick.
        let stick = self.player_follow();
        let controls = stick.as_ref().unwrap_or(controls);
        let (world, mut player) = self.player();
        let result = player.tick(world, controls, real_dt, warp);
        let fresh: Vec<Event> = self.events[before..].to_vec();
        self.traffic_events(crate::combat::PLAYER, &fresh);
        // Wrecked on something (collisions and weapons are filed by the combat phase).
        let crashed = self.events.iter().find_map(|e| match e {
            Event::Ship(ShipEvent::Crashed { body }) if !matches!(body.as_str(), "COLLISION" | "GUNFIRE" | "LASER FIRE") => Some(body.clone()),
            _ => None,
        });
        if let Some(body) = crashed {
            self.recorder.file(self.world.time, crate::combat::PLAYER, "YOU".into(), body, None);
        }
        result
    }

    /// Advance the whole world by `real_dt` real seconds at `warp`, in ticks
    /// of at most `TICK` game seconds (pilots act once a tick, so their
    /// control rate stays in game time whatever the warp), up to
    /// `TICK_BUDGET` ticks; past that, ticks stretch and the step says so.
    pub fn step_world(&mut self, real_dt: f64, warp: f64, controls: &Controls) -> StepResult {
        let wanted = (real_dt * warp / TICK).ceil().max(1.0);
        let n = wanted.min(TICK_BUDGET as f64) as usize;
        let mut result = StepResult::default();
        for _ in 0..n {
            let r = self.tick(real_dt / n as f64, warp, controls);
            result.simulated += r.simulated;
            result.warp_limited |= r.warp_limited;
        }
        result.warp_limited |= wanted > TICK_BUDGET as f64;
        result
    }

    /// One tick: the player's ship, then every craft, all from the same
    /// moment; the clock moves once (as far as the player's ship went).
    fn tick(&mut self, real_dt: f64, warp: f64, controls: &Controls) -> StepResult {
        self.tick += 1;
        let t0 = self.world.time;
        universe_prof::time("sim/snapshot", || self.snapshot());
        let result = universe_prof::time("sim/player", || self.step(real_dt, warp, controls));
        let t1 = self.world.time;
        {
            let _p = universe_prof::scope("sim/crafts");
            // (Each craft keeps its own clock from t0; the world's stays at t1.)
            self.fly_crafts(t0, real_dt, warp);
        }
        self.world.time = t1;
        universe_prof::time("sim/combat", || self.combat(t1 - t0));
        universe_prof::time("sim/traffic presence", || self.traffic_presence());
        universe_prof::time("sim/recorder", || self.record());
        result
    }

    /// Craft `i`'s avionics, connected to its ship, for a request (as its pilot would).
    pub(crate) fn craft_run<R>(&mut self, i: usize, f: impl FnOnce(&mut Avionics, &mut crate::vessel::Link, &mut Vec<Event>) -> R) -> R {
        let c = &mut self.crafts[i];
        let mut events = Vec::new();
        let mut vessel = Vessel { id: crate::combat::craft_id(i), ship: &mut c.ship, system: &mut c.system, avionics: &mut c.avionics, events: &mut events };
        vessel.run(&mut self.world, f)
    }

    /// Lock craft `i`'s nav target.
    pub fn craft_set_nav_target(&mut self, i: usize, target: Option<NavTarget>) {
        self.craft_run(i, |a, link, events| a.set_nav_target(link, target, events));
    }

    /// Craft `i` asks traffic control for clearance (to its nav target, else the nearest station).
    pub fn craft_request_clearance(&mut self, i: usize) -> bool {
        self.craft_run(i, |a, link, events| a.request_clearance(link, events))
    }

    /// Craft `i` engages (or releases) its autopilot.
    pub fn craft_toggle_autopilot(&mut self, i: usize) {
        self.craft_run(i, |a, link, events| a.toggle_autopilot(link, events));
    }

    /// The flight recorder's sample of every ship, when one is due.
    fn record(&mut self) {
        let now = self.world.time;
        if !self.recorder.due(now) {
            return;
        }
        self.recorder.record(crate::combat::PLAYER, crate::recorder::Sample::of(now, self.ship_system, &self.ship, &self.avionics));
        for (i, c) in self.crafts.iter().enumerate() {
            self.recorder.record(crate::combat::craft_id(i), crate::recorder::Sample::of(now, c.system, &c.ship, &c.avionics));
        }
        self.recorder.sampled(now);
    }

    /// What traffic control needs to hear from a ship's events this tick:
    /// its holds end when its clearance does, when it docks or goes through a
    /// gate, and when it's wrecked, replaced or leaves the system.
    pub(crate) fn traffic_events(&mut self, id: usize, events: &[Event]) {
        let done = events.iter().any(|e| {
            matches!(
                e,
                Event::Traffic(universe_world::TrafficEvent::ClearanceCancelled)
                    | Event::Ship(
                        ShipEvent::Crashed { .. }
                            | ShipEvent::Respawned
                            | ShipEvent::GateEntered { .. }
                            | ShipEvent::EnteredSystem { .. }
                            | ShipEvent::Landed { station: true, .. }
                    )
            )
        });
        if done {
            self.world.traffic.release(id);
        }
    }

    /// Traffic control's view of where every ship is, this frame: who stands
    /// on a pad or is in the column over it (below 5 km, within 800 m), and
    /// who is done with the corridor it holds (on its final run within
    /// `CORRIDOR_RELEASE`, or not cleared for it and 3 km clear of it).
    fn traffic_presence(&mut self) {
        use rayon::prelude::*;
        use universe_avionics::nav::Phase;
        use universe_avionics::NavTarget;
        let now = self.world.time;
        // Each system's ports, worked out once: where each is now and how
        // its planet is turned.
        struct Port {
            center: DVec3,
            unturn: DQuat,
            radius: f64,
            direction: DVec3,
        }
        type Seen = (Arc<StarSystem>, Arc<Vec<DVec3>>, Vec<Port>);
        // Corridors held, by ship: (system, body).
        let mut held: std::collections::HashMap<usize, Vec<(usize, usize)>> = std::collections::HashMap::new();
        for (system, body, ship) in self.world.traffic.corridors_held() {
            held.entry(ship).or_default().push((system, body));
        }
        // Who's where (on the ground, or flying in normal space).
        type Where = (usize, usize, DVec3, bool, Option<(NavTarget, Phase)>);
        let ships: Vec<Where> = std::iter::once((crate::combat::PLAYER, self.ship_system, &self.ship, &self.avionics))
            .chain(self.crafts.iter().enumerate().map(|(i, c)| (crate::combat::craft_id(i), c.system, &c.ship, &c.avionics)))
            .filter(|(_, _, s, _)| matches!(s.state, ShipState::Landed { .. }) || (s.is_flying() && !s.hyperdrive))
            .map(|(id, system, s, a)| (id, system, s.position, matches!(s.state, ShipState::Landed { .. }), a.clearance.map(|c| (c.target, c.phase))))
            .collect();
        let mut systems: std::collections::HashMap<usize, Seen> = std::collections::HashMap::new();
        for &(_, system, ..) in &ships {
            if let std::collections::hash_map::Entry::Vacant(e) = systems.entry(system) {
                let sys = self.world.system(system);
                let positions = self.world.rails_now(system);
                let ports = sys
                    .spaceports
                    .iter()
                    .map(|sp| Port { center: positions[sp.body], unturn: sys.bodies[sp.body].rotation(now).inverse(), radius: sys.bodies[sp.body].rail.radius, direction: sp.direction })
                    .collect();
                e.insert((sys, positions, ports));
            }
        }
        // Each ship's facts, side by side.
        let present: Vec<universe_world::pads::Presence> = ships
            .par_iter()
            .filter_map(|&(id, system, pos, landed, clearance)| {
                let (sys, positions, ports) = &systems[&system];
                let mut p = universe_world::pads::Presence { ship: id, system, ..Default::default() };
                // Pads: on one, or in the column over it.
                for (port, sp) in ports.iter().enumerate() {
                    let off = pos - sp.center;
                    if off.length() - sp.radius > 5_000.0 {
                        continue;
                    }
                    let dir = (sp.unturn * off).normalize();
                    if dir.angle_between(sp.direction) * sp.radius > 800.0 {
                        continue;
                    }
                    let nearest = (0..universe_world::spaceport::PADS).min_by(|&a, &b| {
                        let d = |k| universe_world::spaceport::pad_direction(sys, port, k).angle_between(dir);
                        d(a).total_cmp(&d(b))
                    });
                    p.pad = nearest.map(|k| (port, k));
                }
                // Corridors it holds and is done with.
                for &(s, b) in held.get(&id).map_or(&[][..], |v| &v[..]) {
                    if s != system {
                        continue;
                    }
                    let d = positions[b].distance(pos);
                    let cleared_for = clearance.filter(|(t, _)| matches!(t, NavTarget::Station(x) | NavTarget::Gate(x) if *x == b));
                    let done = match cleared_for {
                        Some((_, Phase::Final)) => d < CORRIDOR_RELEASE,
                        Some(_) => false,
                        None => !landed && d > 3_000.0,
                    };
                    if done {
                        p.clear_of.push(b);
                    }
                }
                (p.pad.is_some() || !p.clear_of.is_empty()).then_some(p)
            })
            .collect();
        self.world.traffic.presence(&present);
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

    /// The markets of the system we're in (visible from anywhere in it), with names.
    pub fn markets(&mut self) -> Vec<(Facility, String)> {
        let sys = self.ship_system();
        universe_world::market::facilities(&sys).into_iter().map(|f| (f, f.name(&sys))).collect()
    }

    /// The market we're docked or landed at, if any.
    pub fn docked_market(&mut self) -> Option<Facility> {
        let sys = self.ship_system();
        universe_world::market::docked_at(&sys, &self.ship)
    }

    /// A market in our system: its quotes, and what it bans.
    pub fn market_quotes(&mut self, f: Facility) -> (Vec<universe_world::market::Quote>, Vec<universe_world::goods::Category>) {
        let system = self.ship_system;
        let banned = self.world.market(system, f).map(|m| m.banned.clone()).unwrap_or_default();
        (self.world.quotes(system, f), banned)
    }

    /// A market's quote for one item (listed, or of a kind it wants), if any.
    pub fn quote_for(&mut self, f: Facility, item: usize) -> Option<universe_world::market::Quote> {
        self.world.quote_for(self.ship_system, f, item)
    }

    /// Buy (`units` > 0) or sell (< 0) `item` at the market `f` we're docked
    /// at: credits paid (negative: received), or why not.
    pub fn trade(&mut self, f: Facility, item: usize, units: i64) -> Result<f64, String> {
        let mut credits = self.credits;
        let r = self.world.trade(self.ship_system, f, item, units, &mut self.ship, &mut credits);
        self.credits = credits;
        if let Ok(amount) = r {
            let sys = self.ship_system();
            let record = crate::commerce::TradeRecord {
                time: self.world.time,
                system: self.ship_system,
                market: f.name(&sys),
                trader: "YOU".into(),
                deal: if units > 0 { crate::commerce::Deal::Bought } else { crate::commerce::Deal::Sold },
                bought: units > 0,
                item: self.world.goods[item].name.to_uppercase(),
                units: units.unsigned_abs() as u32,
                amount: amount.abs(),
                cargo: self.ship.cargo,
                credits: self.credits,
            };
            self.log_trade(record);
        }
        r
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
        let rules = self.world.rules_of(self.ship_system);
        self.avionics.plan(&sys, &rules, &self.ship, self.world.time)
    }

    /// Docking guidance only (convenience for tests and tools).
    pub fn docking_status(&mut self) -> Option<(usize, universe_avionics::DockingStatus)> {
        match self.approach()? {
            Approach::Dock { station, status } => Some((station, status)),
            Approach::Land { .. } | Approach::Transit { .. } => None,
        }
    }
}

/// The universe can move to its own thread (the world engine runs apart from the client).
#[allow(dead_code)]
fn universe_is_send() {
    fn send<T: Send>() {}
    send::<Universe>();
}
