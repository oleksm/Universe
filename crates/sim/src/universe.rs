use std::rc::Rc;

use glam::{DQuat, DVec3};
use serde::{Deserialize, Serialize};
use universe_avionics::route::{self, Route, Stop};
use universe_avionics::{Approach, Avionics, Bus, Clearance, Event, NavTarget, Plan};
use universe_world::{BodyKind, Controls, Ship, ShipCommands, ShipEvent, ShipState, StarSystem, StepResult, World};

use crate::rng::Rng;

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
    /// The ship's avionics: nav target, clearance, autopilots, route.
    pub avionics: Avionics,
    pub events: Vec<Event>,
    /// Other ships (settlers), each flying its own route.
    pub crafts: Vec<Craft>,
    pub traffic: TrafficStats,
    /// Recent craft crashes (most recent last, capped).
    pub crash_log: Vec<CrashReport>,
    positions: Vec<DVec3>,
}

/// Another ship in the world, flown by the same code as the player's: its
/// state is swapped into the `Universe` for its turn (see `step_world`).
pub struct Craft {
    pub name: String,
    pub ship: Ship,
    /// Galaxy index of the system it's in.
    pub system: usize,
    /// Its avionics, flying its route.
    pub avionics: Avionics,
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

/// The avionics' bus to the ship being stepped: its sensors read the ship
/// and the world, its commands go to the world's devices.
struct Link<'a> {
    world: &'a mut World,
    ship: &'a mut Ship,
    system: usize,
}

impl Bus for Link<'_> {
    fn ship(&self) -> &Ship {
        self.ship
    }

    fn system(&self) -> usize {
        self.system
    }

    fn star_system(&mut self) -> Rc<StarSystem> {
        self.world.system(self.system)
    }

    fn time(&self) -> f64 {
        self.world.time
    }

    fn gate_links(&self) -> &[(usize, usize)] {
        &self.world.gate_links
    }

    fn command(&mut self, c: &ShipCommands) -> Vec<ShipEvent> {
        let mut events = Vec::new();
        self.world.command(self.ship, self.system, c, &mut events);
        events
    }
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

    /// Run `f` on the ship's avionics, connected to the ship.
    fn with_avionics<R>(&mut self, f: impl FnOnce(&mut Avionics, &mut Link, &mut Vec<Event>) -> R) -> R {
        let mut link = Link { world: &mut self.world, ship: &mut self.ship, system: self.ship_system };
        f(&mut self.avionics, &mut link, &mut self.events)
    }

    /// Give the ship's devices new commands now (see `World::command`).
    pub fn command(&mut self, c: &ShipCommands) {
        self.with_avionics(|a, link, events| a.command(link, c, events));
    }

    /// Put a new ship next to the home station, matching its orbit.
    pub fn respawn(&mut self) {
        let mut events = Vec::new();
        self.world.respawn(&mut self.ship, &mut self.ship_system, &mut events);
        self.avionics.record(events, &mut self.events);
    }

    pub fn toggle_hyperdrive(&mut self) {
        self.with_avionics(|a, link, events| a.toggle_hyperdrive(link, events));
    }

    /// Advance the universe by `real_dt * warp` seconds (less if warp is limited).
    pub fn step(&mut self, real_dt: f64, warp: f64, controls: &Controls) -> StepResult {
        self.with_avionics(|a, link, events| a.prepare(link, events));
        // The pilot's stick turns the ship, unless a computer is flying it.
        let commands = ShipCommands { turn: (!self.avionics.flies(&self.ship)).then_some(*controls), ..self.ship.holding() };
        let mut events = Vec::new();
        let mut computer = self.avionics.computer();
        let result = self.world.step_ship(&mut self.ship, &mut self.ship_system, &commands, &mut computer, real_dt, warp, &mut events);
        let arrived = computer.arrived.take();
        self.avionics.record(events, &mut self.events);
        self.with_avionics(|a, link, events| a.conclude(link, arrived, events));
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
        self.with_avionics(|a, link, events| a.set_nav_target(link, target, events));
    }

    /// Ask traffic control for permission to dock/land at the locked nav
    /// target (or the nearest station).
    pub fn request_clearance(&mut self) -> bool {
        self.with_avionics(|a, link, events| a.request_clearance(link, events))
    }

    /// Engage or release the autopilot. In hyperdrive it steers to the nav
    /// target; otherwise it docks or lands (requesting clearance if needed).
    pub fn toggle_autopilot(&mut self) {
        self.with_avionics(|a, link, events| a.toggle_autopilot(link, events));
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
            let a = &self.avionics;
            let before = (a.nav_target, a.clearance, self.ship.hyperdrive, a.route.departing, self.ship.velocity);
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
        if !c.avionics.route.active && matches!(c.ship.state, ShipState::Landed { .. }) {
            let seed = crate::rng::mix(c.route_seed, 1);
            let mut stops = self.settler_route(seed, 10);
            // Start from where it is: skip a first stop that's right here.
            if stops.first().is_some_and(|s| s.system == self.crafts[i].system) {
                stops.rotate_left(1);
            }
            let c = &mut self.crafts[i];
            c.avionics.route = Route { stops, next: 0, active: true, dwell_until: None, departing: false };
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
                avionics: Avionics { route, ..Avionics::default() },
                route_seed,
                events: Vec::new(),
            });
        }
    }

    /// Start or stop the route autopilot.
    pub fn toggle_route(&mut self) {
        self.with_avionics(|a, link, events| a.toggle_route(link, events));
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
            route: self.avionics.route.clone(),
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
        self.avionics = Avionics { route: save.route, ..save.avionics };
        self.events.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use universe_avionics::hyperdrive::HYPER_ARRIVE_PORT;
    use universe_avionics::{PadFrame, Phase};
    use universe_world::{GateFrame, StationFrame};

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
        assert!(dist < HYPER_ARRIVE_PORT, "dropped out too far: {dist}");
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
        assert!(dist < HYPER_ARRIVE_PORT);
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
    fn plan_at(plan: &Plan, start: f64, at: f64) -> (DVec3, DVec3) {
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
        u.avionics.route.stops = u.settler_route(7, 10);
        let names: Vec<String> = u.avionics.route.stops.clone().into_iter().map(|s| u.stop_name(s)).collect();
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
        u.avionics.route.stops = vec![
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
            (u.traffic.stops, u.traffic.crashes, u.crafts.iter().map(|c| (c.system, c.avionics.route.next)).collect::<Vec<_>>())
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
    use universe_avionics::{docking, PadFrame};
    use universe_world::{GateFrame, StationFrame};

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
                let way = u.avionics.debug_way.unwrap_or(DVec3::ZERO);
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
