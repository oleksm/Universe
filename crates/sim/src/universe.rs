use std::collections::HashMap;
use std::rc::Rc;

use glam::{DQuat, DVec3};
use serde::{Deserialize, Serialize};

use crate::docking::{self, Contact, DockingStatus, StationFrame};
use crate::landing::{self, LandingStatus, PadFrame};
use crate::galaxy::{Galaxy, StarClass, GALAXY_STARS};
use crate::gate::{self, Crossing, GateFrame};
use crate::names::star_name;
use crate::route::{self, Route, Stop};
use crate::rng::Rng;
use crate::ship::{self, upright, Clearance, Controls, NavTarget, Phase, Ship, ShipState};
use crate::system::{BodyKind, Ephemeris, StarSystem};
use crate::units::{SUN_RADIUS, LIGHT_YEAR};

/// Upper bound on integration substeps per frame; beyond this, time warp is limited.
const MAX_SUBSTEPS: u32 = 2000;
/// Neighbouring stars checked for hyperdrive obstacles and system hand-over.
const NEIGHBOURS: usize = 24;
/// Hyperdrive drops out this close to a targeted station (m).
const HYPER_ARRIVE_STATION: f64 = 20_000.0;
/// Hyperdrive drops out this close to a targeted spaceport (m).
const HYPER_ARRIVE_PORT: f64 = 120_000.0;
/// The hyperdrive autopilot aims this far above a targeted pad (m).
const HYPER_PORT_ALTITUDE: f64 = 100_000.0;
/// Without a target, hyperdrive drops out at least this far above a planet or moon (m).
const HYPER_PLANET_MARGIN: f64 = 1_000_000.0;

/// Where a hyperdrive jump to the nav target ends.
struct HyperAim {
    /// The target itself (station center or pad).
    target: DVec3,
    /// Drop out within this distance of `target`.
    arrive: f64,
    /// Where the hyperdrive autopilot heads for.
    aim: DVec3,
    /// The body the target sits on or orbits, which may block the way.
    body: usize,
    /// Velocity to match on arrival.
    velocity: DVec3,
    /// The frame hyperdrive motion is relative to: the target itself, or for
    /// a spaceport its planet (the ground's rotation only matters close in).
    frame_velocity: DVec3,
    name: String,
}

#[derive(Clone, Debug)]
pub enum Event {
    Landed { body: String, station: bool },
    TookOff,
    Crashed { body: String },
    Respawned,
    EnteredSystem { name: String },
    HyperdriveEngaged,
    HyperdriveDisengaged,
    /// Dropped out of hyperdrive at the nav target.
    HyperdriveArrived { target: String },
    /// Entered a gate, heading for another system.
    GateEntered { to: String },
    /// Came out of the paired gate.
    GateArrived { system: String },
    /// The route autopilot reached a stop (1-based number, name).
    RouteStop { number: usize, name: String },
    RouteComplete,
    /// A route stop can't be reached (no gate path, or it no longer exists).
    RouteBlocked { reason: String },
    /// Went through a gate faster than it can take.
    GateTooFast { speed: f64 },
    ClearanceGranted { target: String, kind: ClearanceKind },
    ClearanceDenied { reason: String },
    ClearanceCancelled,
    Autopilot { on: bool },
    NavTargetSet { name: Option<String> },
    LandedAtPort { port: String },
    /// Gentle scrape against a station hull.
    Bumped,
    Launched { station: String },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClearanceKind {
    Dock,
    Land,
    Transit,
}

/// Live guidance for the current clearance.
#[derive(Clone, Debug)]
pub enum Approach {
    Dock { station: usize, status: DockingStatus },
    Land { port: usize, status: Box<LandingStatus> },
    Transit { gate: usize, status: gate::GateStatus },
}

#[derive(Clone, Copy, Debug, Default)]
pub struct StepResult {
    /// Simulated seconds this step.
    pub simulated: f64,
    /// True if the requested warp could not be reached.
    pub warp_limited: bool,
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
}

pub struct Universe {
    pub galaxy: Galaxy,
    /// Seconds since the epoch.
    pub time: f64,
    pub ship: Ship,
    /// Galaxy index of the system the ship is in; ship coordinates are relative to its star.
    pub ship_system: usize,
    pub home_system: usize,
    /// Gate links between star systems (galaxy indices).
    pub gate_links: Vec<(usize, usize)>,
    /// The ship's multi-stop route, and the route autopilot's progress.
    pub route: Route,
    pub events: Vec<Event>,
    /// Other ships (settlers), each flying its own route.
    pub crafts: Vec<Craft>,
    pub traffic: TrafficStats,
    /// Recent craft crashes (most recent last, capped).
    pub crash_log: Vec<CrashReport>,
    systems: HashMap<usize, Rc<StarSystem>>,
    neighbours: Vec<usize>,
    positions: Vec<DVec3>,
    /// Body snapshots per system for the current moment, shared by every ship there.
    ephemerides: HashMap<usize, (f64, Rc<Ephemeris>)>,
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
    pub route: Route,
    /// Seed of its current route (a new one is made when it finishes).
    pub route_seed: u64,
    neighbours: Vec<usize>,
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
        let galaxy = Galaxy::generate(seed, GALAXY_STARS);
        let mut u = Self {
            galaxy,
            time: 0.0,
            ship: Ship::new(DVec3::ZERO, DVec3::ZERO, DQuat::IDENTITY),
            ship_system: 0,
            home_system: 0,
            gate_links: Vec::new(),
            route: Route::default(),
            events: Vec::new(),
            crafts: Vec::new(),
            traffic: TrafficStats::default(),
            crash_log: Vec::new(),
            ephemerides: HashMap::new(),
            debug_way: None,
            systems: HashMap::new(),
            neighbours: Vec::new(),
            positions: Vec::new(),
        };
        u.home_system = u.find_home(seed);
        u.build_gate_network();
        u.respawn();
        u.events.clear();
        u
    }

    /// A sun-like star in the galactic disc with a station around a rocky world.
    fn find_home(&mut self, seed: u64) -> usize {
        let mut rng = Rng::new(seed ^ 0x686f_6d65);
        let n = self.galaxy.stars.len();
        for _ in 0..10_000 {
            let i = (rng.next_u64() % n as u64) as usize;
            let s = &self.galaxy.stars[i];
            let r = (s.position.x.powi(2) + s.position.z.powi(2)).sqrt();
            if !matches!(s.class, StarClass::G | StarClass::K) || !(2500.0..5000.0).contains(&r) {
                continue;
            }
            let sys = self.system(i);
            if sys.station().is_some() && sys.planet_count() >= 4 {
                return i;
            }
        }
        0
    }

    /// Link the home system and its 4 nearest neighbours with gates: a
    /// spanning tree (each system to the nearest already-linked one), plus up
    /// to two extra short links for loops. Every system gets 1-3 gates.
    fn build_gate_network(&mut self) {
        const MAX_GATES: usize = 3;
        let mut nodes = vec![self.home_system];
        nodes.extend(self.galaxy.nearest(self.home_system, 4));
        let dist = |a: usize, b: usize| self.galaxy.stars[a].position.distance(self.galaxy.stars[b].position);
        let mut degree: HashMap<usize, usize> = HashMap::new();
        let mut links: Vec<(usize, usize)> = Vec::new();
        for k in 1..nodes.len() {
            let n = nodes[k];
            let best = nodes[..k]
                .iter()
                .copied()
                .filter(|m| degree.get(m).copied().unwrap_or(0) < MAX_GATES)
                .min_by(|&a, &b| dist(a, n).total_cmp(&dist(b, n)))
                .unwrap_or(nodes[0]);
            links.push((best, n));
            *degree.entry(best).or_default() += 1;
            *degree.entry(n).or_default() += 1;
        }
        let mut pairs: Vec<(usize, usize)> =
            nodes.iter().flat_map(|&a| nodes.iter().filter(move |&&b| b > a).map(move |&b| (a, b))).collect();
        pairs.sort_by(|p, q| dist(p.0, p.1).total_cmp(&dist(q.0, q.1)));
        let mut extra = 0;
        for (a, b) in pairs {
            let linked = links.iter().any(|&(x, y)| (x, y) == (a, b) || (y, x) == (a, b));
            let room = |n: usize| degree.get(&n).copied().unwrap_or(0) < MAX_GATES;
            if extra < 2 && !linked && room(a) && room(b) {
                links.push((a, b));
                *degree.entry(a).or_default() += 1;
                *degree.entry(b).or_default() += 1;
                extra += 1;
            }
        }
        self.gate_links = links;
        // Systems generated before the network existed need their gates.
        self.systems.clear();
    }

    /// Systems linked to `i` by gates, with their names.
    pub fn gate_links_of(&self, i: usize) -> Vec<(usize, String)> {
        self.gate_links
            .iter()
            .filter_map(|&(a, b)| if a == i { Some(b) } else if b == i { Some(a) } else { None })
            .map(|j| (j, star_name(self.galaxy.stars[j].seed)))
            .collect()
    }

    /// Get (generating and caching if needed) the star system at galaxy index `i`.
    pub fn system(&mut self, i: usize) -> Rc<StarSystem> {
        if self.systems.len() > 64 {
            let keep = self.ship_system;
            self.systems.retain(|&k, _| k == keep);
        }
        if !self.systems.contains_key(&i) {
            let star = &self.galaxy.stars[i];
            let mut sys = StarSystem::generate(i, star);
            let links = self.gate_links_of(i);
            if !links.is_empty() {
                sys.add_gates(&links, star.seed);
            }
            self.systems.insert(i, Rc::new(sys));
        }
        self.systems[&i].clone()
    }

    pub fn ship_system(&mut self) -> Rc<StarSystem> {
        self.system(self.ship_system)
    }

    /// Put the ship next to the home station, matching its orbit.
    pub fn respawn(&mut self) {
        self.ship_system = self.home_system;
        self.neighbours = self.galaxy.nearest(self.ship_system, NEIGHBOURS);
        let sys = self.ship_system();
        let station = sys.station().unwrap_or(0);
        sys.positions(self.time, &mut self.positions);
        let pos = self.positions[station];
        let vel = sys.velocity(station, self.time);
        let parent = sys.bodies[station].parent.unwrap_or(0);
        let rel_vel = vel - sys.velocity(parent, self.time);
        let prograde = rel_vel.normalize();
        let radial = (pos - self.positions[parent]).normalize();

        // 4 km behind the station on the same orbit, nose pointing at it.
        let ship_pos = pos - prograde * 4000.0;
        let orientation = upright(radial, prograde);
        self.ship = Ship::new(ship_pos, vel, orientation);
        self.events.push(Event::Respawned);
    }

    pub fn toggle_hyperdrive(&mut self) {
        if !self.ship.is_flying() {
            return;
        }
        if self.ship.hyperdrive {
            // Drop out co-moving with the nav target if it's close, otherwise
            // with whatever dominates gravity here.
            let sys = self.ship_system();
            sys.positions(self.time, &mut self.positions);
            let near_target = self
                .ship
                .nav_target
                .and_then(|t| self.hyper_aim(&sys, t))
                .filter(|a| a.target.distance(self.ship.position) < 1.0e6);
            self.ship.velocity = match near_target {
                Some(a) => a.velocity,
                None => sys.velocity(sys.dominant(self.ship.position, &self.positions), self.time),
            };
            self.ship.hyperdrive = false;
            self.events.push(Event::HyperdriveDisengaged);
        } else {
            self.ship.hyperdrive = true;
            self.events.push(Event::HyperdriveEngaged);
        }
        // Both ways, start from zero throttle: hyperdrive speed follows the
        // throttle, so carrying full main-engine throttle in (or out) would
        // fling the ship away.
        self.ship.throttle = 0.0;
        self.ship.hyper_autopilot = false;
    }


    /// Advance the universe by `real_dt * warp` seconds (less if warp is limited).
    pub fn step(&mut self, real_dt: f64, warp: f64, controls: &Controls) -> StepResult {
        self.route_step();
        if !self.route.active {
            self.autopilot_hyperjump();
        }
        let sys = self.ship_system();
        let result = match self.ship.state.clone() {
            ShipState::Destroyed { respawn_in } => {
                let left = respawn_in - real_dt;
                self.ship.state = ShipState::Destroyed { respawn_in: left };
                if left <= 0.0 {
                    self.respawn();
                }
                self.advance_clock(real_dt * warp)
            }
            ShipState::Landed { body, local_position, local_orientation } => {
                self.landed_step(&sys, real_dt, warp, controls, body, local_position, local_orientation)
            }
            ShipState::Transit { to, from, remaining, local_velocity, local_offset, local_orientation } => {
                // The transit takes a few real seconds; the world clock keeps its pace.
                let result = self.advance_clock(real_dt * warp);
                let left = remaining - real_dt;
                if left > 0.0 {
                    self.ship.state = ShipState::Transit { to, from, remaining: left, local_velocity, local_offset, local_orientation };
                } else {
                    self.arrive_through_gate(to, from, local_velocity, local_offset, local_orientation);
                }
                result
            }
            ShipState::Flying if self.ship.hyperdrive => {
                if !self.ship.hyper_autopilot {
                    self.ship.steer(controls, real_dt);
                }
                self.hyperdrive_step(&sys, real_dt, warp)
            }
            ShipState::Flying => {
                if !self.autopilot_engaged() {
                    self.ship.steer(controls, real_dt);
                }
                self.flight_step(&sys, real_dt * warp)
            }
        };
        self.check_system_handover();
        self.check_clearance();
        result
    }

    fn autopilot_engaged(&self) -> bool {
        self.ship.clearance.is_some_and(|c| c.autopilot)
    }

    /// Display name of a target in the ship's system.
    pub fn target_name(&mut self, target: NavTarget) -> String {
        let sys = self.ship_system();
        match target {
            NavTarget::Station(b) => sys.bodies.get(b).map_or_else(String::new, |b| b.name.clone()),
            NavTarget::Spaceport(p) => sys.spaceports.get(p).map_or_else(String::new, |p| {
                format!("{} ({})", p.name, sys.bodies[p.body].name)
            }),
            NavTarget::Gate(b) => sys.bodies.get(b).map_or_else(String::new, |b| b.name.clone()),
        }
    }

    /// Current world position of a target, and whether it's still valid.
    pub fn target_position(&mut self, target: NavTarget) -> Option<DVec3> {
        let sys = self.ship_system();
        sys.positions(self.time, &mut self.positions);
        match target {
            NavTarget::Station(b) => (sys.bodies.get(b)?.kind == BodyKind::Station).then(|| self.positions[b]),
            NavTarget::Spaceport(p) => {
                sys.spaceports.get(p)?;
                Some(PadFrame::new(&sys, p, self.time, &self.positions).pad)
            }
            NavTarget::Gate(b) => (sys.bodies.get(b)?.kind == BodyKind::Gate).then(|| self.positions[b]),
        }
    }

    /// How far out clearance is granted for a target (m).
    fn clearance_range(&mut self, target: NavTarget) -> f64 {
        match target {
            NavTarget::Station(_) => docking::CLEARANCE_RANGE,
            NavTarget::Spaceport(p) => {
                let sys = self.ship_system();
                sys.bodies[sys.spaceports[p].body].radius * landing::CLEARANCE_RADII
            }
            NavTarget::Gate(_) => gate::CLEARANCE_RANGE,
        }
    }

    /// Lock (or clear) the navigation target.
    pub fn set_nav_target(&mut self, target: Option<NavTarget>) {
        if self.ship.clearance.is_some_and(|c| Some(c.target) != target) {
            self.ship.clearance = None;
            self.ship.rcs = DVec3::ZERO;
            self.events.push(Event::ClearanceCancelled);
        }
        self.ship.nav_target = target;
        let name = target.map(|t| self.target_name(t));
        self.events.push(Event::NavTargetSet { name });
    }

    /// Nearest station in the ship's system.
    fn nearest_station(&mut self) -> Option<NavTarget> {
        let sys = self.ship_system();
        sys.positions(self.time, &mut self.positions);
        sys.bodies
            .iter()
            .enumerate()
            .filter(|(_, b)| b.kind == BodyKind::Station)
            .map(|(i, _)| (i, self.positions[i].distance(self.ship.position)))
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(i, _)| NavTarget::Station(i))
    }

    /// Ask the locked nav target (or the nearest station) for permission to dock/land.
    pub fn request_clearance(&mut self) -> bool {
        let deny = |u: &mut Self, reason: &str| {
            u.events.push(Event::ClearanceDenied { reason: reason.into() });
            false
        };
        if !self.ship.is_flying() {
            return deny(self, "NOT IN FLIGHT");
        }
        if self.ship.hyperdrive {
            return deny(self, "DISENGAGE HYPERDRIVE FIRST");
        }
        let Some(target) = self.ship.nav_target.or_else(|| self.nearest_station()) else {
            return deny(self, "NO TARGET - PICK ONE ON THE MAP (M)");
        };
        let Some(at) = self.target_position(target) else {
            return deny(self, "TARGET NOT IN THIS SYSTEM");
        };
        let range = self.clearance_range(target);
        if at.distance(self.ship.position) > range {
            return deny(self, &format!("OUT OF RANGE - CLOSE TO {:.0} KM", range / 1000.0));
        }
        let name = self.target_name(target);
        self.ship.clearance = Some(Clearance { target, autopilot: false, phase: Phase::Approach });
        let kind = match target {
            NavTarget::Station(_) => ClearanceKind::Dock,
            NavTarget::Spaceport(_) => ClearanceKind::Land,
            NavTarget::Gate(_) => ClearanceKind::Transit,
        };
        self.events.push(Event::ClearanceGranted { target: name, kind });
        true
    }

    /// Engage or release the autopilot. In hyperdrive it steers to the nav
    /// target; otherwise it docks or lands (requesting clearance if needed).
    pub fn toggle_autopilot(&mut self) {
        if self.ship.hyperdrive {
            if self.ship.nav_target.is_none() && !self.ship.hyper_autopilot {
                self.events.push(Event::ClearanceDenied { reason: "LOCK A NAV TARGET FIRST (M)".into() });
                return;
            }
            self.ship.hyper_autopilot = !self.ship.hyper_autopilot;
            self.events.push(Event::Autopilot { on: self.ship.hyper_autopilot });
            return;
        }
        if self.ship.clearance.is_none() && !self.request_clearance() {
            return;
        }
        if let Some(c) = &mut self.ship.clearance {
            c.autopilot = !c.autopilot;
            c.phase = Phase::Approach;
            let on = c.autopilot;
            if !on {
                self.ship.rcs = DVec3::ZERO;
                self.ship.throttle = 0.0;
            }
            self.events.push(Event::Autopilot { on });
        }
    }

    /// Clearance lapses if the ship wanders far away or the target vanishes.
    fn check_clearance(&mut self) {
        let Some(c) = self.ship.clearance else { return };
        if !self.ship.is_flying() {
            return;
        }
        let range = self.clearance_range(c.target);
        let far = self.target_position(c.target).is_none_or(|p| p.distance(self.ship.position) > 2.0 * range);
        if far {
            self.ship.clearance = None;
            self.ship.rcs = DVec3::ZERO;
            self.events.push(Event::ClearanceCancelled);
        }
    }

    /// The body snapshot for `sys` at time `t`, computed once and shared by
    /// every ship stepping from that moment.
    fn ephemeris(&mut self, sys: &StarSystem, t: f64) -> Rc<Ephemeris> {
        match self.ephemerides.get(&sys.index) {
            Some((at, e)) if *at == t => e.clone(),
            _ => {
                let e = Rc::new(sys.ephemeris(t));
                self.ephemerides.insert(sys.index, (t, e.clone()));
                e
            }
        }
    }

    /// Advance the whole world: the player's ship, then every craft, all from
    /// the same moment; the clock moves once.
    pub fn step_world(&mut self, real_dt: f64, warp: f64, controls: &Controls) -> StepResult {
        let t0 = self.time;
        let result = self.step(real_dt, warp, controls);
        let t1 = self.time;
        for i in 0..self.crafts.len() {
            self.time = t0;
            self.swap_craft(i);
            let before = (self.ship.nav_target, self.ship.clearance, self.ship.hyperdrive, self.route.departing, self.ship.velocity);
            self.step(real_dt, warp, &Controls::default());
            let crashed = self.events.iter().find_map(|e| match e {
                Event::Crashed { body } => Some(body.clone()),
                _ => None,
            });
            if let Some(body) = crashed {
                let sys = self.ship_system();
                let speed = sys.bodies.iter().position(|b| b.name == body).map_or(0.0, |b| (before.4 - sys.velocity(b, self.time)).length());
                self.crash_log.push(CrashReport {
                    craft: self.crafts[i].name.clone(),
                    body,
                    system: self.ship_system,
                    time: self.time,
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
        self.time = t1;
        result
    }

    /// Exchange the player's per-ship state with craft `i`'s.
    fn swap_craft(&mut self, i: usize) {
        let c = &mut self.crafts[i];
        std::mem::swap(&mut self.ship, &mut c.ship);
        std::mem::swap(&mut self.ship_system, &mut c.system);
        std::mem::swap(&mut self.route, &mut c.route);
        std::mem::swap(&mut self.neighbours, &mut c.neighbours);
        std::mem::swap(&mut self.events, &mut c.events);
    }

    /// Count what happened to craft `i`, and give it a new route when done.
    fn tally_craft(&mut self, i: usize) {
        let events = std::mem::take(&mut self.crafts[i].events);
        for e in events {
            match e {
                Event::RouteStop { .. } => self.traffic.stops += 1,
                Event::GateEntered { .. } => self.traffic.transits += 1,
                Event::Crashed { .. } => self.traffic.crashes += 1,
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
            let ship = self.ship_at(first);
            let route = Route { stops, next: 0, active: true, dwell_until: Some(self.time + rng.range(0.0, 600.0)), departing: false };
            let neighbours = self.galaxy.nearest(first.system, NEIGHBOURS);
            self.crafts.push(Craft {
                name: format!("Settler {}", self.crafts.len() + 1),
                ship,
                system: first.system,
                route,
                route_seed,
                neighbours,
                events: Vec::new(),
            });
        }
    }

    /// A ship docked or landed at `stop`.
    fn ship_at(&mut self, stop: Stop) -> Ship {
        let sys = self.system(stop.system);
        sys.positions(self.time, &mut self.positions);
        let mut ship = Ship::new(DVec3::ZERO, DVec3::ZERO, DQuat::IDENTITY);
        let (body, local_position, local_orientation) = match stop.target {
            NavTarget::Station(s) => {
                // In the slot, nose in (the station's local frame: axis +Y, slot along X).
                let docked = DQuat::from_mat3(&glam::DMat3::from_cols(DVec3::X, DVec3::NEG_Z, DVec3::Y));
                (s, DVec3::Y * docking::STATION_SIZE * 0.85, docked)
            }
            NavTarget::Spaceport(p) | NavTarget::Gate(p) => {
                let sp = &sys.spaceports[p.min(sys.spaceports.len().saturating_sub(1))];
                let b = &sys.bodies[sp.body];
                let d = sp.direction;
                (sp.body, d * (b.surface_radius(d) + ship::SHIP_RADIUS), upright(d, d.any_orthonormal_vector()))
            }
        };
        let rot = sys.bodies[body].rotation(self.time);
        ship.position = self.positions[body] + rot * local_position;
        ship.velocity = sys.velocity(body, self.time);
        ship.orientation = rot * local_orientation;
        ship.state = ShipState::Landed { body, local_position, local_orientation };
        ship
    }

    /// Start or stop the route autopilot.
    pub fn toggle_route(&mut self) {
        if self.route.stops.is_empty() {
            return;
        }
        self.route.active = !self.route.active;
        if !self.route.active {
            if let Some(c) = &mut self.ship.clearance {
                c.autopilot = false;
            }
            self.ship.hyper_autopilot = false;
            self.ship.rcs = DVec3::ZERO;
            self.ship.throttle = 0.0;
        }
        self.events.push(Event::Autopilot { on: self.route.active });
    }

    /// A reproducible route of `count` stops (stations and spaceports) across
    /// the gate network, from a seed: same seed, same route.
    pub fn settler_route(&mut self, seed: u64, count: usize) -> Vec<Stop> {
        let mut systems: Vec<usize> = self.gate_links.iter().flat_map(|&(a, b)| [a, b]).collect();
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
        let mut rng = Rng::new(crate::rng::mix(self.galaxy.seed, seed));
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
            NavTarget::Spaceport(p) => sys.spaceports.get(p).is_some_and(|sp| {
                sp.body == body && sp.direction.angle_between(local_position.normalize()) * sys.bodies[body].radius < landing::PAD_RADIUS
            }),
            NavTarget::Gate(_) => false,
        }
    }

    /// Height above the ground of the dominant body (m), if it has a surface.
    fn ground_altitude(&mut self) -> f64 {
        let sys = self.ship_system();
        sys.positions(self.time, &mut self.positions);
        let d = sys.dominant(self.ship.position, &self.positions);
        let b = &sys.bodies[d];
        self.ship.position.distance(self.positions[d]) - b.surface_radius_at(self.positions[d], self.ship.position, self.time)
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
                            self.route.dwell_until = Some(self.time + route::DWELL);
                            let name = self.stop_name(stop);
                            self.events.push(Event::RouteStop { number: self.route.next + 1, name });
                        }
                        Some(t) if self.time >= t => {
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
        let Some(c) = self.ship.clearance.filter(|c| c.autopilot) else { return };
        if !self.ship.is_flying() || self.ship.hyperdrive {
            return;
        }
        let Some(at) = self.target_position(c.target) else { return };
        if at.distance(self.ship.position) > Self::hyperjump_limit(c.target) {
            self.ship.nav_target = Some(c.target);
            self.toggle_hyperdrive();
            self.ship.hyper_autopilot = true;
            self.ship.throttle = 1.0;
        }
    }

    /// Launch from a station, or lift off a surface and climb.
    fn leave(&mut self, body: usize) {
        let station = self.ship_system().bodies[body].kind == BodyKind::Station;
        if station {
            self.ship.throttle = 0.2;
        } else {
            self.ship.rcs = DVec3::Y;
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
                self.ship.rcs = DVec3::Y;
                self.ship.throttle = 0.0;
                return;
            }
            self.route.departing = false;
            self.ship.rcs = DVec3::ZERO;
        }
        // Next hop: the stop itself, or the gate toward its system.
        let hop = if self.ship_system == stop.system {
            stop.target
        } else {
            let Some(next) = route::gate_path(&self.gate_links, self.ship_system, stop.system).and_then(|p| p.first().copied()) else {
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
        if self.ship.nav_target != Some(hop) {
            self.ship.nav_target = Some(hop);
            self.ship.clearance = None;
        }
        let Some(at) = self.target_position(hop) else {
            self.route.active = false;
            self.events.push(Event::RouteBlocked { reason: "STOP NOT FOUND".into() });
            return;
        };
        // Hyperdrive (which steers around anything in the way) until close:
        // the landing and docking approaches only mind their own target.
        let far = at.distance(self.ship.position) > Self::hyperjump_limit(hop);
        match self.ship.clearance {
            Some(c) if !c.autopilot => self.toggle_autopilot(),
            Some(_) => {}
            None if far => {
                // Far: hyperdrive there, steered by its autopilot.
                self.toggle_hyperdrive();
                self.ship.hyper_autopilot = true;
                self.ship.throttle = 1.0;
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
        let c = self.ship.clearance?;
        let sys = self.ship_system();
        sys.positions(self.time, &mut self.positions);
        Some(match c.target {
            NavTarget::Station(station) => {
                let frame = StationFrame::new(&sys, station, self.time, &self.positions);
                Approach::Dock { station, status: docking::status(&frame, &self.ship, &c) }
            }
            NavTarget::Spaceport(port) => {
                let pad = PadFrame::new(&sys, port, self.time, &self.positions);
                let terrain = sys.bodies[sys.spaceports[port].body].terrain.as_ref();
                Approach::Land { port, status: Box::new(landing::status(&pad, &self.ship, &c, terrain)) }
            }
            NavTarget::Gate(g) => {
                let frame = GateFrame::new(&sys, g, self.time, &self.positions);
                Approach::Transit { gate: g, status: gate::status(&frame, &self.ship, &c) }
            }
        })
    }

    /// The flight plan to the cleared target: what to do from here, as the
    /// autopilot would do it.
    pub fn plan(&mut self) -> Option<crate::plan::Plan> {
        let c = self.ship.clearance?;
        if !self.ship.is_flying() || self.ship.hyperdrive {
            return None;
        }
        let sys = self.ship_system();
        Some(crate::plan::plan(&sys, &self.ship, c.target, c.phase, self.time))
    }

    /// Docking guidance only (convenience for tests and tools).
    pub fn docking_status(&mut self) -> Option<(usize, DockingStatus)> {
        match self.approach()? {
            Approach::Dock { station, status } => Some((station, status)),
            Approach::Land { .. } | Approach::Transit { .. } => None,
        }
    }

    fn advance_clock(&mut self, dt: f64) -> StepResult {
        self.time += dt;
        StepResult { simulated: dt, warp_limited: false }
    }

    #[allow(clippy::too_many_arguments)]
    fn landed_step(
        &mut self,
        sys: &StarSystem,
        real_dt: f64,
        warp: f64,
        controls: &Controls,
        body: usize,
        local_position: DVec3,
        local_orientation: DQuat,
    ) -> StepResult {
        let result = self.advance_clock(real_dt * warp);
        let b = &sys.bodies[body];
        let rot = b.rotation(self.time);
        sys.positions(self.time, &mut self.positions);
        let center = self.positions[body];

        // Allow turning in place on the pad.
        self.ship.orientation = rot * local_orientation;
        self.ship.steer(controls, real_dt);
        let local_orientation = rot.inverse() * self.ship.orientation;

        let offset = rot * local_position;
        self.ship.position = center + offset;
        self.ship.velocity = sys.velocity(body, self.time) + b.angular_velocity().cross(offset);
        self.ship.state = ShipState::Landed { body, local_position, local_orientation };

        if b.kind == BodyKind::Station {
            if self.ship.throttle > 0.05 || self.ship.rcs.length() > 0.1 {
                // Launch: shot out of the slot along the axis, nose first.
                let frame = StationFrame::new(sys, body, self.time, &self.positions);
                let axis = frame.axis();
                let out = frame.docking_orientation(self.ship.orientation) * DQuat::from_rotation_y(std::f64::consts::PI);
                self.ship.orientation = out;
                self.ship.position = frame.on_axis(docking::STATION_SIZE + 150.0);
                self.ship.velocity = frame.velocity + axis * 40.0;
                self.ship.angular_velocity = DVec3::ZERO;
                self.ship.state = ShipState::Flying;
                self.events.push(Event::Launched { station: b.name.clone() });
            }
            return result;
        }
        let normal = offset.normalize();
        let nose_up_burn = self.ship.throttle > 0.05 && self.ship.forward().dot(normal) > 0.2;
        let lift = self.ship.rcs.y > 0.1 && (self.ship.orientation * DVec3::Y).dot(normal) > 0.5;
        if nose_up_burn || lift {
            self.ship.velocity += normal * 3.0;
            self.ship.position += normal * 2.0;
            self.ship.state = ShipState::Flying;
            self.events.push(Event::TookOff);
        }
        result
    }

    /// Hyperdrive destination for the nav target, if it's in this system.
    fn hyper_aim(&self, sys: &StarSystem, target: NavTarget) -> Option<HyperAim> {
        match target {
            NavTarget::Station(b) => {
                let body = sys.bodies.get(b).filter(|body| body.kind == BodyKind::Station)?;
                let at = self.positions[b];
                Some(HyperAim {
                    target: at,
                    arrive: HYPER_ARRIVE_STATION,
                    aim: at,
                    body: body.parent.unwrap_or(0),
                    velocity: sys.velocity(b, self.time),
                    frame_velocity: sys.velocity(b, self.time),
                    name: body.name.clone(),
                })
            }
            NavTarget::Gate(b) => {
                let body = sys.bodies.get(b).filter(|body| body.kind == BodyKind::Gate)?;
                let at = self.positions[b];
                Some(HyperAim {
                    target: at,
                    arrive: HYPER_ARRIVE_STATION,
                    aim: at,
                    body: body.parent.unwrap_or(0),
                    velocity: sys.velocity(b, self.time),
                    frame_velocity: sys.velocity(b, self.time),
                    name: body.name.clone(),
                })
            }
            NavTarget::Spaceport(p) => {
                sys.spaceports.get(p)?;
                let pad = PadFrame::new(sys, p, self.time, &self.positions);
                Some(HyperAim {
                    target: pad.pad,
                    arrive: HYPER_ARRIVE_PORT,
                    aim: pad.pad + pad.up * HYPER_PORT_ALTITUDE,
                    body: sys.spaceports[p].body,
                    velocity: pad.frame_velocity(self.ship.position),
                    frame_velocity: pad.body_velocity,
                    name: sys.spaceports[p].name.clone(),
                })
            }
        }
    }

    /// Hyperdrive speed is defined per real second; the world clock (and the
    /// target's own motion) runs at `warp` game seconds per real second.
    fn hyperdrive_step(&mut self, sys: &StarSystem, real_dt: f64, warp: f64) -> StepResult {
        let result = self.advance_clock(real_dt * warp);
        sys.positions(self.time, &mut self.positions);
        let p = self.ship.position;
        let aim = self.ship.nav_target.and_then(|t| self.hyper_aim(sys, t));

        // Arrived at the nav target: drop out moving with it. Asking for
        // clearance is left to the pilot.
        if let Some(a) = &aim
            && p.distance(a.target) < a.arrive
        {
            let (velocity, name) = (a.velocity, a.name.clone());
            self.ship.hyperdrive = false;
            self.ship.hyper_autopilot = false;
            self.ship.throttle = 0.0;
            self.ship.velocity = velocity;
            self.events.push(Event::HyperdriveDisengaged);
            self.events.push(Event::HyperdriveArrived { target: name });
            return result;
        }

        // Nearest surface: (clearance, radius, center, body index or None for other stars).
        let mut nearest = (f64::INFINITY, 0.0, DVec3::ZERO, None);
        for (i, (b, &bp)) in sys.bodies.iter().zip(&self.positions).enumerate() {
            let d = bp.distance(p) - b.radius;
            if !b.kind.artificial() && d < nearest.0 {
                nearest = (d, b.radius, bp, Some(i));
            }
        }
        for &n in &self.neighbours {
            let r = self.galaxy.stars[n].class.radius_suns() * SUN_RADIUS;
            let c = self.galaxy.offset(self.ship_system, n);
            let d = c.distance(p) - r;
            if d < nearest.0 {
                nearest = (d, r, c, None);
            }
        }
        let (clearance, radius, center, body) = nearest;

        // Direction of travel: where the nose points, unless the pilot handed
        // the jump to the hyperdrive autopilot.
        let dir = match (&aim, self.ship.hyper_autopilot) {
            (Some(a), true) => {
                // Swing around whatever is in the way: the first body met along
                // the path, then re-check the detour leg itself (up to three
                // times). (For the target's own planet, "blocked" means dipping
                // below half the aim altitude, so the aim point just above a
                // pad never counts.)
                let first_blocker = |to: DVec3| -> Option<usize> {
                    let seg = to - p;
                    let len2 = seg.length_squared().max(1e-9);
                    let mut best: Option<(f64, usize)> = None;
                    for (i, b) in sys.bodies.iter().enumerate() {
                        if b.kind.artificial() {
                            continue;
                        }
                        let c = self.positions[i];
                        let margin = if i == a.body { 0.5 * HYPER_PORT_ALTITUDE } else { 0.1 * b.radius };
                        if docking::segment_distance(p, to, c) < b.radius + margin {
                            let along = ((c - p).dot(seg) / len2).clamp(0.0, 1.0);
                            if best.is_none_or(|(t, _)| along < t) {
                                best = Some((along, i));
                            }
                        }
                    }
                    best.map(|(_, i)| i)
                };
                // Going around: fly along the curve (tangent to the body) and
                // steer toward a safe radius (at least 1.5 radii), rather than
                // at a point on the far side: a straight line to such a point
                // always starts off heading inward, and chasing it spirals in.
                let way_dir = match first_blocker(a.aim) {
                    None => (a.aim - p).normalize(),
                    Some(i) => {
                        let (c, r) = (self.positions[i], sys.bodies[i].radius);
                        let from = (p - c).normalize();
                        let to = (a.aim - c).normalize();
                        let axis = from.cross(to).try_normalize().unwrap_or_else(|| from.any_orthonormal_vector());
                        let tangent = axis.cross(from);
                        let hold = (p - c).length().max(1.5 * r);
                        let climb = ((hold - (p - c).length()) / (0.2 * hold)).clamp(-1.0, 1.0);
                        (tangent + from * climb).normalize()
                    }
                };
                let way = p + way_dir * 1.0e6;
                self.debug_way = Some(way);
                let dir = (way - p).normalize();
                let target = DQuat::from_rotation_arc(self.ship.forward(), dir) * self.ship.orientation;
                self.ship.steer(&docking::attitude(&self.ship, target, DVec3::ZERO), real_dt);
                dir
            }
            _ => self.ship.forward(),
        };

        // Obstacle dead ahead: drop out at a safe distance instead of crawling
        // toward its surface. Stars get a wide margin, planets and moons a
        // fixed one. Near misses fly on past, and so does heading for a
        // targeted port or station whose line of approach is clear.
        let to = center - p;
        let along = to.dot(dir);
        let miss = (to - dir * along).length();
        let is_star = body.is_none_or(|i| sys.bodies[i].kind == BodyKind::Star);
        let margin = if is_star { 3.0 * radius } else { HYPER_PLANET_MARGIN.max(0.1 * radius) };
        let approach_clear = aim.as_ref().is_some_and(|a| {
            Some(a.body) == body && docking::segment_distance(p, a.target + (a.target - center).normalize() * 1000.0, center) > radius
        });
        // (The hyperdrive autopilot steers around obstacles itself.)
        if !self.ship.hyper_autopilot && clearance < margin && along > 0.0 && miss < 1.5 * radius && !approach_clear {
            self.toggle_hyperdrive();
            return result;
        }

        // Speed grows with room to move: distance to the nearest surface, and
        // to the target if there is one, so we never overshoot it.
        let room = aim.as_ref().map_or(clearance, |a| clearance.min(p.distance(a.target)));
        let speed = ship::HYPER_RATE * room.max(1000.0) * self.ship.throttle.max(0.02);
        // Move relative to the target (or the dominant body): targets ride
        // along with their planets at tens of km/s, and a hyperdrive that
        // ignored that would never catch them.
        let base = match &aim {
            Some(a) => a.frame_velocity,
            None => sys.velocity(sys.dominant(p, &self.positions), self.time),
        };
        self.ship.velocity = base + dir * speed;
        let next = self.ship.position + base * (real_dt * warp) + dir * speed * real_dt.min(0.1);
        // Safety net: never hyperdrive into a planet, moon or star.
        let inside = sys.bodies.iter().zip(&self.positions).any(|(b, &c)| !b.kind.artificial() && c.distance(next) < b.max_radius() + 1000.0);
        if inside {
            self.toggle_hyperdrive();
            return result;
        }
        self.ship.position = next;
        result
    }

    fn flight_step(&mut self, sys: &StarSystem, dt: f64) -> StepResult {
        let mut t = self.time;
        // For short frames, snapshot the bodies once and extrapolate for each
        // substep instead of re-solving every orbit (see `Ephemeris`).
        let ephemeris = (dt <= Ephemeris::SPAN).then(|| self.ephemeris(sys, t));
        let place = |t: f64, out: &mut Vec<DVec3>| match &ephemeris {
            Some(e) => e.positions(t, out),
            None => sys.positions(t, out),
        };
        place(t, &mut self.positions);
        let (mut pos, mut vel) = (self.ship.position, self.ship.velocity);
        let mut remaining = dt;
        let mut steps = 0;
        let mut limited = false;
        // Near a station (or docking), take small steps so contact and control are precise.
        let near_station = sys.bodies.iter().zip(&self.positions).any(|(b, p)| b.kind.artificial() && p.distance(pos) < 30_000.0);
        let fine = near_station || self.ship.clearance.is_some() || self.ship.rcs != DVec3::ZERO;

        while remaining > 1e-9 {
            if steps >= MAX_SUBSTEPS {
                limited = true;
                break;
            }
            let dom = sys.dominant(pos, &self.positions);
            let r = pos.distance(self.positions[dom]);
            let mut h = (0.01 * (r * r * r / sys.bodies[dom].mu).sqrt()).clamp(0.01, 3600.0);
            if fine {
                h = h.min(0.05);
            }
            let h = h.min(remaining);

            if let Some(c) = self.ship.clearance.filter(|c| c.autopilot) {
                self.ship.position = pos;
                self.ship.velocity = vel;
                let cmd = match c.target {
                    NavTarget::Station(station) => {
                        let frame = StationFrame::new(sys, station, t, &self.positions);
                        docking::autopilot(&frame, &self.ship, c.phase)
                    }
                    NavTarget::Spaceport(port) => {
                        let pad = PadFrame::new(sys, port, t, &self.positions);
                        landing::autopilot(&pad, &self.ship, sys.gravity(pos, &self.positions), c.phase)
                    }
                    NavTarget::Gate(g) => {
                        let frame = GateFrame::new(sys, g, t, &self.positions);
                        gate::autopilot(&frame, &self.ship, c.phase)
                    }
                };
                if cmd.phase != c.phase {
                    self.ship.clearance = Some(Clearance { phase: cmd.phase, ..c });
                }
                self.ship.throttle = cmd.throttle;
                self.ship.rcs = cmd.rcs;
                self.ship.steer(&cmd.controls, h);
            }
            // Acceleration = thrust / current mass.
            let thrust = self.ship.forward() * (self.ship.main_accel() * self.ship.throttle)
                + self.ship.orientation * self.ship.thruster_accel(self.ship.rcs);

            // Leapfrog (kick-drift-kick): stable for long orbits.
            let prev = pos;
            vel += (sys.gravity(pos, &self.positions) + thrust) * (h * 0.5);
            pos += vel * h;
            t += h;
            place(t, &mut self.positions);
            vel += (sys.gravity(pos, &self.positions) + thrust) * (h * 0.5);
            remaining -= h;
            steps += 1;

            if let Some(true) = self.gate_crossing(sys, prev, pos, vel, t, h) {
                return StepResult { simulated: dt - remaining, warp_limited: limited };
            }
            if let Some(hit) = self.collision(sys, pos, t) {
                self.ship.position = pos;
                self.ship.velocity = vel;
                self.time = t;
                if sys.bodies[hit].kind == BodyKind::Station {
                    let frame = StationFrame::new(sys, hit, t, &self.positions);
                    match docking::contact(&frame, pos, vel, self.ship.orientation) {
                        Contact::Clear => continue,
                        Contact::Bump(normal) => {
                            // Bounce off gently and keep flying.
                            let surface = frame.velocity_at(pos);
                            let rel = vel - surface;
                            let into = rel.dot(normal).min(0.0);
                            vel = surface + rel - normal * (1.4 * into) + normal * 0.5;
                            pos += normal * 2.0;
                            self.events.push(Event::Bumped);
                            continue;
                        }
                        Contact::Docked => self.dock(sys, hit, &frame),
                        Contact::Crash => self.crash(&sys.bodies[hit].name),
                    }
                } else {
                    self.touch_down(sys, hit, vel, t);
                }
                return StepResult { simulated: dt - remaining, warp_limited: limited };
            }
        }
        self.ship.position = pos;
        self.ship.velocity = vel;
        self.time = t;
        StepResult { simulated: dt - remaining, warp_limited: limited }
    }

    /// Check every nearby gate for a pass through its ring (start a transit)
    /// or a hit on its structure (crash). Returns Some(true) if the flight step ends.
    fn gate_crossing(&mut self, sys: &StarSystem, prev: DVec3, pos: DVec3, vel: DVec3, t: f64, h: f64) -> Option<bool> {
        for (i, b) in sys.bodies.iter().enumerate() {
            if b.kind != BodyKind::Gate || self.positions[i].distance(pos) > gate::GATE_RADIUS * 2.0 {
                continue;
            }
            let frame = GateFrame::new(sys, i, t, &self.positions);
            match gate::crossing(&frame, prev, pos, h) {
                Crossing::None => continue,
                Crossing::Hit => {
                    self.ship.position = pos;
                    self.time = t;
                    self.crash(&b.name);
                    return Some(true);
                }
                Crossing::Through => {
                    self.ship.position = pos;
                    self.time = t;
                    let rel = vel - frame.velocity;
                    if rel.length() > gate::MAX_TRANSIT_SPEED {
                        self.events.push(Event::GateTooFast { speed: rel.length() });
                        self.crash(&b.name);
                        return Some(true);
                    }
                    let local = frame.local(pos);
                    let to = b.link.unwrap_or(self.ship_system);
                    self.ship.state = ShipState::Transit {
                        to,
                        from: self.ship_system,
                        remaining: gate::TRANSIT_TIME,
                        local_velocity: frame.rotation.inverse() * rel,
                        local_offset: DVec3::new(local.x, 0.0, local.z),
                        local_orientation: frame.rotation.inverse() * self.ship.orientation,
                    };
                    self.ship.velocity = vel;
                    self.ship.clearance = None;
                    self.ship.nav_target = None;
                    self.ship.rcs = DVec3::ZERO;
                    self.ship.throttle = 0.0;
                    self.events.push(Event::GateEntered { to: star_name(self.galaxy.stars[to].seed) });
                    return Some(true);
                }
            }
        }
        None
    }

    /// Come out of the gate in system `to` that leads back to `from`, with the
    /// same motion relative to it as we had going into the other one.
    fn arrive_through_gate(&mut self, to: usize, from: usize, local_velocity: DVec3, local_offset: DVec3, local_orientation: DQuat) {
        self.ship_system = to;
        self.neighbours = self.galaxy.nearest(to, NEIGHBOURS);
        let sys = self.ship_system();
        sys.positions(self.time, &mut self.positions);
        let Some(g) = sys.gate_to(from) else {
            // No return gate (shouldn't happen): arrive near the star instead.
            self.respawn();
            return;
        };
        let frame = GateFrame::new(&sys, g, self.time, &self.positions);
        let out = if local_velocity.y >= 0.0 { 1.0 } else { -1.0 };
        let clear = gate::RING_TUBE + ship::SHIP_RADIUS + 50.0;
        self.ship.position = frame.center + frame.rotation * (local_offset + DVec3::Y * out * clear);
        self.ship.velocity = frame.velocity + frame.rotation * local_velocity;
        self.ship.orientation = frame.rotation * local_orientation;
        self.ship.angular_velocity = DVec3::ZERO;
        self.ship.state = ShipState::Flying;
        self.events.push(Event::GateArrived { system: sys.name.clone() });
    }

    /// Broad-phase: which body (if any) might the ship be touching?
    fn collision(&self, sys: &StarSystem, pos: DVec3, t: f64) -> Option<usize> {
        sys.bodies.iter().zip(&self.positions).position(|(b, &bp)| {
            match b.kind {
                BodyKind::Gate => false, // handled by gate_crossing
                // Bounding sphere of the cuboctahedron; the exact test comes later.
                BodyKind::Station => bp.distance(pos) < docking::STATION_SIZE * std::f64::consts::SQRT_2 + ship::SHIP_RADIUS,
                // The terrain under us (cheap sphere test first).
                _ => {
                    let d = bp.distance(pos);
                    d < b.max_radius() + ship::SHIP_RADIUS && d < b.surface_radius_at(bp, pos, t) + ship::SHIP_RADIUS
                }
            }
        })
    }

    fn dock(&mut self, sys: &StarSystem, station: usize, frame: &StationFrame) {
        let rot = frame.rotation;
        let orientation = frame.docking_orientation(self.ship.orientation);
        self.ship.orientation = orientation;
        self.ship.throttle = 0.0;
        self.ship.rcs = DVec3::ZERO;
        self.ship.angular_velocity = DVec3::ZERO;
        self.ship.clearance = None;
        self.ship.state = ShipState::Landed {
            body: station,
            local_position: DVec3::Y * docking::STATION_SIZE * 0.85,
            local_orientation: rot.inverse() * orientation,
        };
        self.events.push(Event::Landed { body: sys.bodies[station].name.clone(), station: true });
    }

    fn crash(&mut self, body: &str) {
        self.ship.state = ShipState::Destroyed { respawn_in: 3.0 };
        self.ship.hyperdrive = false;
        self.ship.throttle = 0.0;
        self.ship.rcs = DVec3::ZERO;
        self.ship.clearance = None;
        self.events.push(Event::Crashed { body: body.to_string() });
    }

    fn touch_down(&mut self, sys: &StarSystem, body: usize, vel: DVec3, t: f64) {
        let b = &sys.bodies[body];
        let center = self.positions[body];
        let offset = self.ship.position - center;
        let surface_vel = sys.velocity(body, t) + b.angular_velocity().cross(offset);
        let impact = (vel - surface_vel).length();

        let rot = b.rotation(t);
        let normal = offset.normalize();
        let local = rot.inverse() * normal;
        let sea = b.terrain.as_ref().is_some_and(|tr| tr.is_ocean(local));
        if sea {
            let name = format!("{} ocean", b.name);
            self.crash(&name);
            return;
        }
        if b.kind.landable() && impact < ship::LAND_SPEED {
            let orientation = upright(normal, self.ship.forward());
            self.ship.orientation = orientation;
            self.ship.throttle = 0.0;
            self.ship.angular_velocity = DVec3::ZERO;
            self.ship.state = ShipState::Landed {
                body,
                local_position: local * (b.surface_radius(local) + ship::SHIP_RADIUS),
                local_orientation: rot.inverse() * orientation,
            };
            self.ship.rcs = DVec3::ZERO;
            self.ship.clearance = None;
            let port = sys
                .spaceports
                .iter()
                .find(|p| p.body == body && p.direction.angle_between(local) * b.radius < landing::PAD_RADIUS);
            match port {
                Some(p) => self.events.push(Event::LandedAtPort { port: p.name.clone() }),
                None => self.events.push(Event::Landed { body: b.name.clone(), station: false }),
            }
        } else {
            let name = b.name.clone();
            self.crash(&name);
        }
    }

    /// When the ship is closer to a neighbouring star than to its own, move it
    /// into that star's frame (a floating origin at interstellar scale).
    fn check_system_handover(&mut self) {
        if !self.ship.is_flying() {
            return;
        }
        let p = self.ship.position;
        let mut best = (self.ship_system, p.length());
        for &n in &self.neighbours {
            let d = self.galaxy.offset(self.ship_system, n).distance(p);
            if d < best.1 {
                best = (n, d);
            }
        }
        if best.0 != self.ship_system {
            let offset = self.galaxy.offset(self.ship_system, best.0);
            self.ship.position -= offset;
            self.ship_system = best.0;
            self.ship.clearance = None;
            self.ship.nav_target = None;
            self.neighbours = self.galaxy.nearest(best.0, NEIGHBOURS);
            let name = self.ship_system().name.clone();
            self.events.push(Event::EnteredSystem { name });
        }
    }

    /// Distance in light years between two stars.
    pub fn distance_ly(&self, a: usize, b: usize) -> f64 {
        self.galaxy.offset(a, b).length() / LIGHT_YEAR
    }

    pub fn save(&self) -> UniverseSave {
        UniverseSave {
            seed: self.galaxy.seed,
            time: self.time,
            ship: self.ship.clone(),
            ship_system: self.ship_system,
            route: self.route.clone(),
        }
    }

    pub fn load(&mut self, save: UniverseSave) {
        if save.seed != self.galaxy.seed {
            *self = Universe::new(save.seed);
        }
        self.time = save.time;
        self.ship = save.ship;
        self.ship_system = save.ship_system.min(self.galaxy.stars.len() - 1);
        self.route = save.route;
        self.neighbours = self.galaxy.nearest(self.ship_system, NEIGHBOURS);
        self.events.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn home_system_has_station_and_planets() {
        let mut u = Universe::new(42);
        let sys = u.ship_system();
        assert!(sys.station().is_some());
        assert!(sys.planet_count() >= 4);
        for (i, b) in sys.bodies.iter().enumerate() {
            if let Some(p) = b.parent {
                assert!(p < i, "parent must precede child");
            }
        }
    }

    #[test]
    fn ship_keeps_orbit_under_warp() {
        let mut u = Universe::new(42);
        let sys = u.ship_system();
        let station = sys.station().unwrap();
        let planet = sys.bodies[station].parent.unwrap();
        let mut pos = Vec::new();
        sys.positions(u.time, &mut pos);
        let r0 = u.ship.position.distance(pos[planet]);
        // Two simulated hours at 1000x.
        for _ in 0..(7200 / 16) {
            u.step(0.016, 1000.0, &Controls::default());
        }
        sys.positions(u.time, &mut pos);
        let r1 = u.ship.position.distance(pos[planet]);
        assert!(u.ship.is_flying());
        assert!((r1 - r0).abs() / r0 < 0.01, "orbit radius drifted: {r0} -> {r1}");
    }

    #[test]
    fn hyperdrive_reaches_neighbour_and_drops_out_safely() {
        let mut u = Universe::new(42);
        let home = u.ship_system;
        let target = u.galaxy.nearest(home, 1)[0];
        let dir = (u.galaxy.offset(home, target) - u.ship.position).normalize();
        u.ship.orientation = DQuat::from_rotation_arc(DVec3::NEG_Z, dir);
        u.ship.throttle = 1.0;
        u.toggle_hyperdrive();
        assert_eq!(u.ship.throttle, 0.0, "engaging hyperdrive resets the throttle");
        u.ship.throttle = 1.0;
        for _ in 0..(120 * 60) {
            u.step(1.0 / 60.0, 1.0, &Controls::default());
            if !u.ship.hyperdrive {
                break;
            }
        }
        assert!(!u.ship.hyperdrive, "should have dropped out on arrival");
        assert_eq!(u.ship.throttle, 0.0, "dropping out resets the throttle");
        assert_eq!(u.ship_system, target);
        let star_radius = u.ship_system().bodies[0].radius;
        assert!(u.ship.position.length() > 2.0 * star_radius, "dropped out too close to the star");
        assert!(u.ship.is_flying());
    }

    /// Fly the docking computer from `setup`'s position; returns simulated seconds to dock.
    fn autodock(mut u: Universe) -> f64 {
        u.toggle_autopilot();
        assert!(u.ship.clearance.is_some_and(|d| d.autopilot), "clearance should be granted: {:?}", u.events);
        let start = u.time;
        let mut phase = Phase::Approach;
        for _ in 0..(60 * 60 * 3) {
            u.step(1.0 / 60.0, 10.0, &Controls::default());
            if let Some(d) = u.ship.clearance
                && d.phase != phase
            {
                phase = d.phase;
                eprintln!("t+{:.0}s phase {:?} status {:?}", u.time - start, phase, u.docking_status().map(|s| s.1));
            }
            match u.ship.state {
                ShipState::Landed { .. } => return u.time - start,
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
        sys.positions(u.time, &mut pos);
        let frame = StationFrame::new(&sys, station, u.time, &pos);
        u.ship.position = frame.center - frame.axis() * 3000.0 + frame.slot_long() * 200.0;
        u.ship.velocity = frame.velocity;
        let secs = autodock(u);
        eprintln!("docked from behind after {secs:.0} s");
    }

    #[test]
    fn fast_hull_contact_crashes_and_slow_bumps() {
        let mut u = Universe::new(42);
        let sys = u.ship_system();
        let station = sys.station().unwrap();
        let mut pos = Vec::new();
        sys.positions(u.time, &mut pos);
        let frame = StationFrame::new(&sys, station, u.time, &pos);
        // Slow: drift onto the slot face near the spin axis, but beside the slot,
        // where the hull barely moves.
        u.ship.position = frame.rotation * (DVec3::new(0.0, 1.03, 0.2) * docking::STATION_SIZE) + frame.center;
        u.ship.velocity = frame.velocity_at(u.ship.position) - frame.axis() * 5.0;
        for _ in 0..180 {
            u.step(1.0 / 60.0, 1.0, &Controls::default());
        }
        assert!(u.ship.is_flying(), "slow contact should bounce: {:?}", u.events);
        // Fast: fly into a spinning side face.
        let side = frame.rotation * DVec3::X;
        assert!(u.events.iter().any(|e| matches!(e, Event::Bumped)));

        u.ship.position = frame_now(&mut u, station).center + side * 700.0;
        let f = frame_now(&mut u, station);
        u.ship.velocity = f.velocity_at(u.ship.position) - (f.rotation * DVec3::X) * 80.0;
        for _ in 0..300 {
            u.step(1.0 / 60.0, 1.0, &Controls::default());
        }
        assert!(matches!(u.ship.state, ShipState::Destroyed { .. }), "fast contact should crash");
    }

    fn frame_now(u: &mut Universe, station: usize) -> StationFrame {
        let sys = u.ship_system();
        let mut pos = Vec::new();
        sys.positions(u.time, &mut pos);
        StationFrame::new(&sys, station, u.time, &pos)
    }

    #[test]
    fn launch_from_docked_leaves_along_axis() {
        let mut u = Universe::new(42);
        let station = u.ship_system().station().unwrap();
        u.ship.state = ShipState::Landed { body: station, local_position: DVec3::Y * 425.0, local_orientation: DQuat::IDENTITY };
        u.ship.throttle = 0.2;
        u.step(1.0 / 60.0, 1.0, &Controls::default());
        assert!(u.ship.is_flying());
        for _ in 0..300 {
            u.step(1.0 / 60.0, 1.0, &Controls::default());
        }
        assert!(u.ship.is_flying(), "launch should not hit the station: {:?}", u.events);
    }

    /// Autopilot to the home planet's spaceport; returns simulated seconds to touchdown.
    fn autoland(mut u: Universe, warp: f64) -> f64 {
        let sys = u.ship_system();
        let planet = sys.bodies[sys.station().unwrap()].parent.unwrap();
        let port = sys.spaceports.iter().position(|p| p.body == planet).expect("home planet has a spaceport");
        u.set_nav_target(Some(NavTarget::Spaceport(port)));
        u.toggle_autopilot();
        assert!(u.ship.clearance.is_some_and(|c| c.autopilot), "landing clearance: {:?}", u.events);
        let start = u.time;
        let mut phase = Phase::Approach;
        let mut next_report = 0.0;
        for _ in 0..(60 * 60 * 20) {
            u.step(1.0 / 60.0, warp, &Controls::default());
            let hd_events: Vec<Event> = u.events.iter().filter(|e| matches!(e, Event::HyperdriveEngaged | Event::HyperdriveDisengaged | Event::HyperdriveArrived { .. })).cloned().collect();
            for e in hd_events {
                let sys = u.ship_system();
                let mut pos = Vec::new();
                sys.positions(u.time, &mut pos);
                let dom = sys.dominant(u.ship.position, &pos);
                let alt = u.ship.position.distance(pos[dom]) - sys.bodies[dom].radius;
                let pad = PadFrame::new(&sys, port, u.time, &pos);
                eprintln!("  t+{:>5.0}s {e:?}: near {} alt {:.0} km, pad {:.0} km", u.time - start, sys.bodies[dom].name, alt / 1000.0, pad.pad.distance(u.ship.position) / 1000.0);
            }
            u.events.retain(|e| !matches!(e, Event::HyperdriveEngaged | Event::HyperdriveDisengaged | Event::HyperdriveArrived { .. }));
            if let Some(Approach::Land { status, .. }) = u.approach()
                && (status.phase != phase || u.time - start > next_report)
            {
                phase = status.phase;
                next_report = u.time - start + 120.0;
                let surface_alt = (u.ship.position - status.pad.body_center).length() - status.pad.body_radius;
                eprintln!(
                    "t+{:>5.0}s {:?} range {:>10.0} surf-alt {:>8.0} want {:>6.0} m/s speed {:>6.0} vspd {:>7.1} hspd {:>7.1} tilt {:>4.0}deg thr {:.2}",
                    u.time - start,
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
                        u.events.iter().any(|e| matches!(e, Event::LandedAtPort { .. })),
                        "landed off the pad: {:?}",
                        u.events
                    );
                    return u.time - start;
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
        let planet = sys.bodies[sys.station().unwrap()].parent.unwrap();
        let port = sys.spaceports.iter().position(|p| p.body == planet).unwrap();
        let mut pos = Vec::new();
        sys.positions(u.time, &mut pos);
        let pad = PadFrame::new(&sys, port, u.time, &pos);
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
        let planet = sys.bodies[sys.station().unwrap()].parent.unwrap();
        let port = sys.spaceports.iter().position(|p| p.body == planet).unwrap();
        let mut pos = Vec::new();
        sys.positions(u.time, &mut pos);
        let pad = PadFrame::new(&sys, port, u.time, &pos);
        u.ship.position = pad.pad + pad.up * 50_000.0;
        u.ship.velocity = pad.frame_velocity(u.ship.position);
        autoland(u, 10.0);
    }

    /// A spaceport on a different planet of the home system than the one we start at.
    fn far_port(u: &mut Universe) -> usize {
        let sys = u.ship_system();
        let home_planet = sys.bodies[sys.station().unwrap()].parent.unwrap();
        sys.spaceports.iter().position(|p| p.body != home_planet && sys.bodies[p.body].parent == Some(0)).expect("another planet with a port")
    }

    /// A port on another planet whose pad faces the ship (a straight line to it is clear).
    fn facing_port(u: &mut Universe) -> usize {
        let sys = u.ship_system();
        let home_planet = sys.bodies[sys.station().unwrap()].parent.unwrap();
        let mut pos = Vec::new();
        sys.positions(u.time, &mut pos);
        (0..sys.spaceports.len())
            .filter(|&i| sys.spaceports[i].body != home_planet)
            .max_by(|&a, &b| {
                let facing = |i: usize| {
                    let pad = PadFrame::new(&sys, i, u.time, &pos);
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
        sys.positions(u.time, &mut pos);
        PadFrame::new(&sys, port, u.time, &pos).pad.distance(u.ship.position)
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
        assert!(dist < HYPER_ARRIVE_PORT, "dropped out too far: {dist}");
        assert!(u.events.iter().any(|e| matches!(e, Event::HyperdriveArrived { .. })));
        assert!(u.ship.clearance.is_none(), "clearance is the pilot's call, not automatic");
    }

    #[test]
    fn hyperdrive_autopilot_steers_to_the_port() {
        let mut u = Universe::new(42);
        let port = far_port(&mut u);
        u.set_nav_target(Some(NavTarget::Spaceport(port)));
        u.toggle_hyperdrive();
        u.toggle_autopilot();
        assert!(u.ship.hyper_autopilot);
        u.ship.throttle = 1.0;
        let dist = hyper_until_arrival(&mut u, port, false);
        eprintln!("autopilot: dropped out {:.0} km from the pad", dist / 1000.0);
        assert!(dist < HYPER_ARRIVE_PORT);
    }

    #[test]
    fn untargeted_hyperdrive_stops_well_short_of_a_planet() {
        let mut u = Universe::new(42);
        let port = far_port(&mut u);
        let body = u.ship_system().spaceports[port].body;
        let sys = u.ship_system();
        let mut pos = Vec::new();
        sys.positions(u.time, &mut pos);
        u.ship.orientation = DQuat::from_rotation_arc(DVec3::NEG_Z, (pos[body] - u.ship.position).normalize());
        u.toggle_hyperdrive();
        u.ship.throttle = 1.0;
        hyper_until_arrival(&mut u, port, false);
        sys.positions(u.time, &mut pos);
        let alt = u.ship.position.distance(pos[body]) - sys.bodies[body].radius;
        eprintln!("untargeted: dropped out at altitude {:.0} km", alt / 1000.0);
        assert!(alt > 0.5 * HYPER_PLANET_MARGIN && alt < 3.0 * HYPER_PLANET_MARGIN, "altitude {alt}");
    }

    #[test]
    fn plan_reaches_the_pad_and_the_slot() {
        // Landing from orbit.
        let mut u = Universe::new(42);
        let sys = u.ship_system();
        let planet = sys.bodies[sys.station().unwrap()].parent.unwrap();
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
            let planet = sys.bodies[station].parent.unwrap();
            // Plans are drawn relative to the target as it is when planned:
            // this is that reference body, and its spin (zero for stations).
            let (body, omega) = if label == "landing" {
                let port = sys.spaceports.iter().position(|p| p.body == planet).unwrap();
                u.set_nav_target(Some(NavTarget::Spaceport(port)));
                // Within hyperjump range, so the landing autopilot flies it all.
                let mut pos = Vec::new();
                sys.positions(u.time, &mut pos);
                let pad = PadFrame::new(&sys, port, u.time, &pos);
                u.ship.position = pad.pad + (pad.up + pad.up.any_orthonormal_vector() * 0.5).normalize() * 150_000.0;
                u.ship.velocity = pad.frame_velocity(u.ship.position);
                (planet, sys.bodies[planet].angular_velocity())
            } else {
                (station, DVec3::ZERO)
            };
            u.toggle_autopilot();
            let mut pos = Vec::new();
            let t0 = u.time;
            sys.positions(t0, &mut pos);
            let c0 = pos[body];
            let first = u.plan().unwrap();
            for _ in 0..600 {
                u.step(1.0 / 60.0, 1.0, &Controls::default());
            }
            let t1 = u.time;
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
    fn gate_network_links_five_systems_with_one_to_three_gates_each() {
        let mut u = Universe::new(1984);
        let mut systems: Vec<usize> = u.gate_links.iter().flat_map(|&(a, b)| [a, b]).collect();
        systems.sort();
        systems.dedup();
        assert_eq!(systems.len(), 5, "links: {:?}", u.gate_links);
        assert!(systems.contains(&u.home_system));
        for &s in &systems {
            let links = u.gate_links_of(s);
            assert!((1..=3).contains(&links.len()), "system {s} has {} gates", links.len());
            let sys = u.system(s);
            for (to, _) in &links {
                let g = sys.gate_to(*to).expect("a gate for every link");
                assert_eq!(sys.bodies[g].kind, BodyKind::Gate);
            }
        }
        // Connected: walk from home.
        let mut seen = vec![u.home_system];
        let mut i = 0;
        while i < seen.len() {
            for (n, _) in u.gate_links_of(seen[i]) {
                if !seen.contains(&n) {
                    seen.push(n);
                }
            }
            i += 1;
        }
        assert_eq!(seen.len(), 5, "network must be connected");
        eprintln!("gate links: {:?}", u.gate_links);
    }

    #[test]
    fn autopilot_flies_through_a_gate_and_keeps_its_motion() {
        let mut u = Universe::new(1984);
        let home = u.home_system;
        let sys = u.ship_system();
        let (dest, _) = u.gate_links_of(home)[0].clone();
        let g = sys.gate_to(dest).unwrap();
        // Start 30 km from the gate, co-moving with it.
        let mut pos = Vec::new();
        sys.positions(u.time, &mut pos);
        let frame = GateFrame::new(&sys, g, u.time, &pos);
        u.ship.position = frame.center + (frame.rotation * DVec3::new(1.0, 0.5, 0.3)).normalize() * 30_000.0;
        u.ship.velocity = frame.velocity;
        u.set_nav_target(Some(NavTarget::Gate(g)));
        u.toggle_autopilot();
        assert!(u.ship.clearance.is_some_and(|c| c.autopilot), "{:?}", u.events);
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
        sys.positions(u.time, &mut pos);
        let out = GateFrame::new(&sys, back, u.time, &pos);
        let exit_local = out.rotation.inverse() * (u.ship.velocity - out.velocity);
        eprintln!("entry {entry_local:.1?} exit {exit_local:.1?}; plan eta {:.0} s", plan_eta.points.last().unwrap().time);
        assert!((exit_local - entry_local).length() < 1.0);
        assert!(u.ship.position.distance(out.center) < gate::GATE_RADIUS, "came out of the ring");
        assert!(u.events.iter().any(|e| matches!(e, Event::GateArrived { .. })));
    }

    #[test]
    fn clipping_the_ring_or_going_too_fast_is_fatal() {
        for (label, offset, speed) in [("ring", gate::GATE_RADIUS, 50.0), ("fast", 0.0, 500.0)] {
            let mut u = Universe::new(1984);
            let home = u.home_system;
            let sys = u.ship_system();
            let (dest, _) = u.gate_links_of(home)[0].clone();
            let g = sys.gate_to(dest).unwrap();
            let mut pos = Vec::new();
            sys.positions(u.time, &mut pos);
            let frame = GateFrame::new(&sys, g, u.time, &pos);
            let axis = frame.axis();
            u.ship.position = frame.center + frame.rotation * DVec3::new(offset, 0.0, 0.0) + axis * 500.0;
            u.ship.velocity = frame.velocity - axis * speed;
            for _ in 0..600 {
                u.step(1.0 / 60.0, 1.0, &Controls::default());
            }
            assert!(u.events.iter().any(|e| matches!(e, Event::Crashed { .. })), "{label}: expected a crash; events {:?}", u.events);
        }
    }

    #[test]
    fn heavier_ship_accelerates_less() {
        let mut u = Universe::new(42);
        let full = u.ship.main_accel();
        assert!((full - 30.0).abs() < 1e-9, "a fully fuelled, empty ship keeps the old 30 m/s^2");
        u.ship.cargo = ship::DRY_MASS + ship::FUEL_CAPACITY; // double the mass
        assert!((u.ship.main_accel() - 15.0).abs() < 1e-9);
        // And it shows in flight: burn for 10 s far from anything.
        let burn = |cargo: f64| {
            let mut u = Universe::new(42);
            u.ship.position = DVec3::new(0.0, 5.0 * crate::units::AU, 0.0);
            u.ship.velocity = DVec3::ZERO;
            u.ship.cargo = cargo;
            u.ship.throttle = 1.0;
            let v0 = u.ship.velocity;
            for _ in 0..600 {
                u.step(1.0 / 60.0, 1.0, &Controls::default());
            }
            (u.ship.velocity - v0).dot(u.ship.forward())
        };
        let (light, heavy) = (burn(0.0), burn(90_000.0));
        eprintln!("10 s full burn: empty {light:.1} m/s, loaded {heavy:.1} m/s");
        assert!((light / heavy - 2.0).abs() < 0.02);
    }

    #[test]
    fn hyperdrive_catches_a_moving_gate_at_part_throttle_and_stops_with_it() {
        let mut u = Universe::new(1984);
        let home = u.home_system;
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
        sys.positions(u.time, &mut pos);
        let rel = u.ship.velocity - sys.velocity(g, u.time);
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
        let rel = u.ship.velocity - sys.velocity(g, u.time);
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
                    Event::Crashed { .. } | Event::RouteBlocked { .. } => panic!("route failed at frame {frame}: {e:?}"),
                    _ => {}
                }
                if matches!(e, Event::RouteStop { .. } | Event::RouteComplete | Event::GateEntered { .. } | Event::GateArrived { .. } | Event::Landed { .. } | Event::LandedAtPort { .. })
                {
                    eprintln!("  t+{:>7.0} s (frame {frame:>6}): {e:?}", u.time);
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
        let t0 = u.time;
        let (reached, done) = fly_route(&mut u, 20.0, 60 * 60 * 60 * 3);
        eprintln!("reached {reached} stops, complete {done}, {:.1} game hours", (u.time - t0) / 3600.0);
        assert!(done);
    }

    #[test]
    fn route_autopilot_lands_docks_and_crosses_a_gate() {
        let mut u = Universe::new(1984);
        let home = u.home_system;
        let sys = u.ship_system();
        let station = sys.station().unwrap();
        let planet = sys.bodies[station].parent.unwrap();
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
            let t0 = u.time;
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
                (u.time - t0) / 3600.0,
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
        let mut systems: Vec<usize> = base.gate_links.iter().flat_map(|&(a, b)| [a, b]).collect();
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
                u.neighbours = u.galaxy.nearest(s, NEIGHBOURS);
                let sys = u.ship_system();
                let mut pos = Vec::new();
                sys.positions(u.time, &mut pos);
                let pad = PadFrame::new(&sys, p, u.time, &pos);
                let side = pad.up.any_orthonormal_vector();
                u.ship.position = pad.pad + (pad.up * 0.6 + side * 0.8).normalize() * 150_000.0;
                u.ship.velocity = pad.frame_velocity(u.ship.position);
                u.ship.state = ShipState::Flying;
                u.set_nav_target(Some(NavTarget::Spaceport(p)));
                u.toggle_autopilot();
                let t0 = u.time;
                let mut outcome = "timeout".to_string();
                for _ in 0..(60 * 60 * 20) {
                    u.step(1.0 / 60.0, 10.0, &Controls::default());
                    if let Some(e) = u.events.iter().find(|e| matches!(e, Event::Crashed { .. } | Event::LandedAtPort { .. } | Event::Landed { .. })).cloned() {
                        let st = u.approach();
                        outcome = format!("{e:?} after {:.0} s; last status {:?}", u.time - t0, st.map(|a| match a {
                            Approach::Land { status, .. } => (status.phase, status.altitude as i64, status.vertical_speed as i64, status.horizontal_speed as i64),
                            _ => (Phase::Approach, 0, 0, 0),
                        }));
                        break;
                    }
                    u.events.clear();
                }
                let b = &sys.bodies[sp.body];
                eprintln!("{} on {} (R {:.0} km, g {:.2}): {outcome}", sp.name, b.name, b.radius / 1000.0, b.mu / (b.radius * b.radius));
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
        for _ in 0..60 {
            u.step(1.0 / 60.0, 100.0, &Controls::default());
        }
        let json = serde_json::to_string(&u.save()).unwrap();
        let mut restored = Universe::new(7);
        restored.load(serde_json::from_str(&json).unwrap());
        assert_eq!(restored.time, u.time);
        assert_eq!(restored.ship.position, u.ship.position);
        assert_eq!(restored.ship_system, u.ship_system);
    }

    #[test]
    fn orbit_state_matches_numeric_derivative() {
        let o = crate::Orbit::new(1.0e9, 0.3, 0.2, 1.0, 2.0, 0.5, 1.0e17);
        let (p0, v) = o.state(1000.0);
        let p1 = o.position(1000.001);
        let numeric = (p1 - p0) / 0.001;
        assert!((numeric - v).length() / v.length() < 1e-4);
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
        let planet = sys.bodies[sys.station().unwrap()].parent.unwrap();
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
        let (dest, _) = v.gate_links_of(v.home_system)[0].clone();
        let g = v.ship_system().gate_to(dest).unwrap();
        v.set_nav_target(Some(NavTarget::Gate(g)));
        v.toggle_hyperdrive();
        v.ship.throttle = 0.3;
        run("hyperdrive (targeted), warp 1", &mut v, 1.0);
        let mut v = Universe::new(1984);
        let sysv = v.ship_system();
        sysv.positions(v.time, &mut pos);
        let up = (v.ship.position - pos[planet]).normalize();
        v.ship.position = pos[planet] + up * (sysv.bodies[planet].surface_radius_at(pos[planet], pos[planet] + up, v.time) + 3000.0);
        v.ship.velocity = sysv.velocity(planet, v.time) + sysv.bodies[planet].angular_velocity().cross(v.ship.position - pos[planet]);
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
            let start = u.time;
            let eta0 = u.plan().map(|p| p.points.last().unwrap().time).unwrap_or(f64::NAN);
            let mut samples = Vec::new();
            for i in 0..(60 * 60 * 30) {
                if i % 600 == 0
                    && let Some(p) = u.plan()
                {
                    samples.push((u.time - start, u.time - start + p.points.last().unwrap().time));
                }
                u.step(1.0 / 60.0, 5.0, &Controls::default());
                if done(&u) {
                    break;
                }
            }
            let actual = u.time - start;
            eprintln!("{label}: planned {eta0:.0} s, actual {actual:.0} s ({:+.0}%)", (actual / eta0 - 1.0) * 100.0);
            for (t, arrive) in samples.iter().step_by((samples.len() / 8).max(1)) {
                eprintln!("    at {t:>6.0} s the plan said arrival at {arrive:>6.0} s");
            }
        };
        let landed = |u: &Universe| matches!(u.ship.state, ShipState::Landed { .. });
        run("docking from spawn", Universe::new(1984), &landed);

        let mut u = Universe::new(1984);
        let sys = u.ship_system();
        let planet = sys.bodies[sys.station().unwrap()].parent.unwrap();
        let port = sys.spaceports.iter().position(|p| p.body == planet).unwrap();
        u.set_nav_target(Some(NavTarget::Spaceport(port)));
        run("landing from orbit", u, &landed);

        let mut u = Universe::new(1984);
        let home = u.home_system;
        let sys = u.ship_system();
        let (dest, _) = u.gate_links_of(home)[0].clone();
        let g = sys.gate_to(dest).unwrap();
        let mut pos = Vec::new();
        sys.positions(u.time, &mut pos);
        let frame = GateFrame::new(&sys, g, u.time, &pos);
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
                let left = p.points.last().unwrap().time - (u.time - p.start);
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
        let home = u.home_system;
        let sys = u.ship_system();
        let (dest, _) = u.gate_links_of(home)[0].clone();
        let g = sys.gate_to(dest).unwrap();
        let mut pos = Vec::new();
        sys.positions(u.time, &mut pos);
        let frame = GateFrame::new(&sys, g, u.time, &pos);
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
        let planet = sys.bodies[sys.station().unwrap()].parent.unwrap();
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
        let planet = sys.bodies[sys.station().unwrap()].parent.unwrap();
        let port = sys.spaceports.iter().position(|p| p.body == planet).unwrap();
        u.set_nav_target(Some(NavTarget::Spaceport(port)));
        u.toggle_autopilot();
        for i in 0..(60 * 300) {
            u.step(1.0 / 60.0, 20.0, &Controls::default());
            if i % 60 == 0 && u.ship.hyperdrive {
                let mut pos = Vec::new();
                sys.positions(u.time, &mut pos);
                let c = pos[planet];
                let r = sys.bodies[planet].radius;
                let alt = |x: DVec3| (x.distance(c) - r) / 1000.0;
                let way = u.debug_way.unwrap_or(DVec3::ZERO);
                let pad = PadFrame::new(&sys, port, u.time, &pos);
                eprintln!("  {:>4}s alt {:>8.0} km  way alt {:>8.0} km  angle to pad {:>5.1} deg  speed {:>8.0} km/s", i / 60, alt(u.ship.position), alt(way), ((u.ship.position - c).angle_between(pad.up)).to_degrees(), u.ship.velocity.length() / 1000.0);
            }
            if !u.ship.hyperdrive && i > 60 * 5 {
                break;
            }
        }
        eprintln!("R = {:.0} km", sys.bodies[planet].radius / 1000.0);
    }

    #[test]
    #[ignore]
    fn debug_landing_start() {
        let mut u = Universe::new(1984);
        let sys = u.ship_system();
        let planet = sys.bodies[sys.station().unwrap()].parent.unwrap();
        let port = sys.spaceports.iter().position(|p| p.body == planet).unwrap();
        u.set_nav_target(Some(NavTarget::Spaceport(port)));
        let mut pos = Vec::new();
        sys.positions(u.time, &mut pos);
        let pad = PadFrame::new(&sys, port, u.time, &pos);
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
        let planet = sys.bodies[sys.station().unwrap()].parent.unwrap();
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
        sys.positions(u.time, &mut pos);
        let f = StationFrame::new(&sys, station, u.time, &pos);
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
