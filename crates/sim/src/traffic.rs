//! Traffic: the settlers' crafts — other ships in the world, flown by the
//! same code as the player's, each by its own avionics along its own
//! reproducible route — with the totals of what they got up to, and a log of
//! their crashes (for diagnosing autopilots).

use universe_avionics::route::{Route, Stop};
use universe_avionics::{Avionics, Clearance, Event, NavTarget};
use universe_world::{BodyKind, Controls, Ship, ShipEvent, ShipState};

use crate::rng::Rng;
use crate::universe::Universe;
use crate::vessel::Vessel;

/// Stops on a settler's route.
const ROUTE_STOPS: usize = 10;
/// Crash reports kept (the most recent).
const CRASH_LOG: usize = 50;

/// Another ship in the world, with the avionics flying it.
pub struct Craft {
    pub name: String,
    pub ship: Ship,
    /// Galaxy index of the system it's in.
    pub system: usize,
    /// Its avionics, flying its route.
    pub avionics: Avionics,
    /// Seed of its current route (a new one is made when it finishes).
    pub route_seed: u64,
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
    /// Settlers: `count` crafts, each on its own reproducible route (from
    /// `seed`), starting docked or landed at its first stop with staggered
    /// departures.
    pub fn spawn_settlers(&mut self, count: usize, seed: u64) {
        let mut rng = Rng::new(seed);
        for i in 0..count {
            let route_seed = crate::rng::mix(seed, i as u64);
            let stops = self.settler_route(route_seed, ROUTE_STOPS);
            let Some(&first) = stops.first() else { continue };
            let ship = self.world.ship_at(first.system, first.target);
            let route = Route { stops, next: 0, active: true, dwell_until: Some(self.world.time + rng.range(0.0, 600.0)), departing: false };
            self.crafts.push(Craft {
                name: format!("Settler {}", self.crafts.len() + 1),
                ship,
                system: first.system,
                avionics: Avionics { route, ..Avionics::default() },
                route_seed,
            });
        }
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

    /// Craft `i`'s turn (no pilot at the stick): step it, log a crash, count
    /// what happened.
    pub(crate) fn fly_craft(&mut self, i: usize, real_dt: f64, warp: f64) {
        let c = &mut self.crafts[i];
        let a = &c.avionics;
        let before = (a.nav_target, a.clearance, c.ship.hyperdrive, a.route.departing, c.ship.velocity);
        let mut events = Vec::new();
        let mut vessel = Vessel { ship: &mut c.ship, system: &mut c.system, avionics: &mut c.avionics, events: &mut events };
        vessel.tick(&mut self.world, &Controls::default(), real_dt, warp);
        let crashed = events.iter().find_map(|e| match e {
            Event::Ship(ShipEvent::Crashed { body }) => Some(body.clone()),
            _ => None,
        });
        if let Some(body) = crashed {
            let system = self.crafts[i].system;
            let sys = self.system(system);
            let speed = sys.bodies.iter().position(|b| b.name == body).map_or(0.0, |b| (before.4 - sys.velocity(b, self.world.time)).length());
            self.crash_log.push(CrashReport {
                craft: self.crafts[i].name.clone(),
                body,
                system,
                time: self.world.time,
                target: before.0,
                clearance: before.1,
                hyperdrive: before.2,
                departing: before.3,
                speed,
            });
            if self.crash_log.len() > CRASH_LOG {
                self.crash_log.remove(0);
            }
        }
        self.tally_craft(i, events);
    }

    /// Count what happened to craft `i`, and give it a new route when done.
    fn tally_craft(&mut self, i: usize, events: Vec<Event>) {
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
            let mut stops = self.settler_route(seed, ROUTE_STOPS);
            // Start from where it is: skip a first stop that's right here.
            if stops.first().is_some_and(|s| s.system == self.crafts[i].system) {
                stops.rotate_left(1);
            }
            let c = &mut self.crafts[i];
            c.avionics.route = Route { stops, next: 0, active: true, dwell_until: None, departing: false };
            c.route_seed = seed;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
