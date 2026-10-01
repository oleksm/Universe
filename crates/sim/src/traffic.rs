//! Traffic: the settlers' crafts — other ships in the world, flown by the
//! same code as the player's, each by its own avionics along its own
//! reproducible route — with the totals of what they got up to, and a log of
//! their crashes (for diagnosing autopilots).

use glam::DVec3;
use universe_avionics::hunter::Sighting;
use universe_avionics::route::{Route, Stop};
use universe_avionics::{Avionics, Clearance, Event, NavTarget};
use universe_world::radar::RADAR_RANGE;
use universe_world::{BodyKind, Ship, ShipEvent, ShipState};

use crate::rng::Rng;
use crate::universe::Universe;

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
    /// A trader (see `commerce`): buys and sells at its stops.
    pub trader: bool,
    /// What it paid per unit for what it carries (its own reckoning; its
    /// money and hold are in the ledger).
    pub paid: std::collections::BTreeMap<usize, f64>,
    /// Its commands on their way to the devices (see `Universe::command_delay`).
    pub inbox: crate::vessel::Inbox,
}

/// Totals across all crafts.
#[derive(Clone, Copy, Debug, Default)]
pub struct TrafficStats {
    pub stops: u64,
    pub transits: u64,
    pub crashes: u64,
    pub routes_completed: u64,
    /// Crafts destroyed by weapons fire.
    pub shot_down: u64,
    /// Hunts by pirates: begun, and ended in a kill.
    pub hunts: u64,
    pub pirate_kills: u64,
    /// Lawful ships taking on an aggressor (each ship counted), and aggressors they shot down.
    pub defences: u64,
    pub aggressors_downed: u64,
    /// Ship-to-ship collisions (each ship's side counted), and ships wrecked by them.
    pub collisions: u64,
    pub collision_losses: u64,
    /// Trades by settlers, and the credits that changed hands.
    pub trades: u64,
    pub turnover: f64,
}

/// A ship as it was at the start of the frame (see `Universe::snaps`).
#[derive(Clone, Copy, Debug)]
pub(crate) struct Snap {
    pub system: usize,
    pub position: DVec3,
    pub velocity: DVec3,
    pub flying: bool,
    pub landed: bool,
    pub destroyed: bool,
    pub transit: bool,
    pub hyperdrive: bool,
    pub hull: f64,
    pub aggressed: bool,
    pub pirate: bool,
}

impl Snap {
    fn of(system: usize, ship: &Ship, pirate: bool, aggressed: bool) -> Self {
        Snap {
            system,
            position: ship.position,
            velocity: ship.velocity,
            flying: ship.is_flying(),
            landed: matches!(ship.state, ShipState::Landed { .. }),
            destroyed: matches!(ship.state, ShipState::Destroyed { .. }),
            transit: matches!(ship.state, ShipState::Transit { .. }),
            hyperdrive: ship.hyperdrive,
            hull: ship.hull,
            aggressed,
            pirate,
        }
    }
}

/// About one settler in `PIRATE_ONE_IN` is a pirate, and as many again traders.
pub const PIRATE_ONE_IN: u64 = 10;

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
    /// A pirate on a hunt (or standing down from one), and whether its route was flying.
    pub hunting: bool,
    pub route_active: bool,
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
            // Spread over the port's pads.
            let ship = self.world.ship_on(first.system, first.target, (route_seed % 9) as usize);
            let route = Route { stops, next: 0, active: true, dwell_until: Some(self.world.time + rng.range(0.0, 600.0)), departing: false };
            // Roles, from the seed (the same settlers every time): one slice
            // pirates, another traders, the rest just travel.
            let role = crate::rng::mix(route_seed, 0x0917_27e5) % PIRATE_ONE_IN;
            let (pirate, trader) = (role == 0, role == 1);
            self.crafts.push(Craft {
                // Named for what they do (the number stays each craft's own).
                name: format!("{} {}", if pirate { "Pirate" } else if trader { "Trader" } else { "Settler" }, self.crafts.len() + 1),
                ship,
                system: first.system,
                avionics: Avionics { route, pirate, ..Avionics::default() },
                route_seed,
                trader,
                paid: Default::default(),
                inbox: Default::default(),
            });
            // What a new settler starts with, from the world's account.
            let me = universe_services::Party::Pilot(crate::combat::craft_id(self.crafts.len() - 1));
            self.ledger.settle(me, universe_services::Asset::Credits, crate::commerce::SETTLER_CREDITS, self.tick, universe_protocol::Cause::Rules);
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

    /// Every craft's turn (no pilot at the stick), from `t0`: side by side on
    /// all cores, each against the world as it was at the frame's start (the
    /// others as the snapshot has them); then, in craft order, what came of
    /// them — traffic control's requests, hunts begun and ended, crashes,
    /// trades.
    pub(crate) fn fly_crafts(&mut self, t0: f64, real_dt: f64, warp: f64) {
        use rayon::prelude::*;
        // What they'll look up, gathered first: read without locks.
        let mut systems: Vec<usize> = self.crafts.iter().map(|c| c.system).collect();
        systems.sort_unstable();
        systems.dedup();
        universe_prof::time("sim/crafts/freeze", || self.world.freeze(&systems, t0, t0 + real_dt * warp));
        let (world, atc, snaps, aggressors) = (&self.world, &self.atc, &self.snaps, &self.aggressors);
        let delay = (self.command_delay > 0).then_some((self.tick, self.command_delay as u64));
        let turns: Vec<Turn> = {
            let _p = universe_prof::scope("sim/crafts/side by side");
            self.crafts.par_iter_mut().enumerate().map(|(i, c)| craft_turn(world, atc, snaps, aggressors, i, c, t0, real_dt, warp, delay)).collect()
        };
        self.world.thaw();
        let _p = universe_prof::scope("sim/crafts/in order");
        for (i, t) in turns.into_iter().enumerate() {
            for r in &t.requests {
                r.make(&mut self.atc);
            }
            self.after_turn(i, t);
        }
    }

    /// What came of craft `i`'s turn, in order.
    fn after_turn(&mut self, i: usize, t: Turn) {
        use universe_avionics::hunter::FLEE_HULL;
        let c = &self.crafts[i];
        if let Some(h) = c.avionics.hunting.filter(|_| !t.was_hunting) {
            if h.lawful {
                self.traffic.defences += 1;
            } else {
                self.traffic.hunts += 1;
            }
        }
        // A defender that broke off hurt runs for the guns.
        if t.end.is_some() && !c.avionics.pirate && c.ship.hull < FLEE_HULL {
            self.flee(i);
        }
        let events = t.events;
        self.traffic_events(crate::combat::craft_id(i), &events);
        let crashed = events.iter().find_map(|e| match e {
            Event::Ship(ShipEvent::Crashed { body }) => Some(body.clone()),
            _ => None,
        });
        if let Some(body) = crashed {
            let now = self.world.time;
            self.recorder.file(now, crate::combat::craft_id(i), self.crafts[i].name.to_uppercase(), body.clone(), None);
            let system = self.crafts[i].system;
            let sys = self.system(system);
            let b = &t.before;
            let speed = sys.bodies.iter().position(|x| x.name == body).map_or(0.0, |x| (b.velocity - sys.velocity(x, now)).length());
            self.crash_log.push(CrashReport {
                craft: self.crafts[i].name.clone(),
                body,
                system,
                time: now,
                target: b.target,
                clearance: b.clearance,
                hyperdrive: b.hyperdrive,
                departing: b.departing,
                speed,
                hunting: b.hunting,
                route_active: b.route_active,
            });
            if self.crash_log.len() > CRASH_LOG {
                self.crash_log.remove(0);
            }
        }
        self.tally_craft(i, events);
    }

    /// Take the frame's snapshot of every ship, and who's aggressed.
    pub(crate) fn snapshot(&mut self) {
        let now = self.world.time;
        self.snaps.clear();
        let law = &self.law;
        self.snaps.push(Snap::of(self.ship_system, &self.ship, false, law.aggressed(crate::combat::PLAYER, now)));
        let crafts = self.crafts.iter().enumerate().map(|(i, c)| Snap::of(c.system, &c.ship, c.avionics.pirate, law.aggressed(crate::combat::craft_id(i), now)));
        self.snaps.extend(crafts);
        self.snap_time = now;
        self.aggressors = self.snaps.iter().filter(|s| s.aggressed && s.flying && !s.hyperdrive).map(|s| (s.system, s.position)).collect();
    }

    /// Count what happened to craft `i`, and give it a new route when done.
    fn tally_craft(&mut self, i: usize, events: Vec<Event>) {
        for e in events {
            match e {
                Event::RouteStop { .. } => {
                    self.traffic.stops += 1;
                    if self.crafts[i].trader {
                        self.craft_trades(i);
                    }
                }
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

/// One craft's turn, as it came out (applied in order after: see `fly_crafts`).
pub(crate) struct Turn {
    events: Vec<Event>,
    requests: Vec<crate::vessel::Request>,
    end: Option<universe_avionics::hunter::HuntEnd>,
    was_hunting: bool,
    before: Before,
}

/// What a craft was doing before its turn (for a crash report).
struct Before {
    target: Option<universe_avionics::NavTarget>,
    clearance: Option<universe_avionics::Clearance>,
    hyperdrive: bool,
    departing: bool,
    velocity: DVec3,
    hunting: bool,
    route_active: bool,
}

/// Craft `i`'s turn against the world as it is (see `fly_crafts`).
#[allow(clippy::too_many_arguments)]
fn craft_turn(world: &universe_world::World, atc: &universe_services::TrafficControl, snaps: &[Snap], aggressors: &[(usize, DVec3)], i: usize, c: &mut Craft, t0: f64, real_dt: f64, warp: f64, delay: Option<(u64, u64)>) -> Turn {
    use universe_avionics::hunter::{may_defend, wants_sightings, DEFEND_RANGE};
    // A pirate looks around first (its radar and its friends' transponders),
    // as does anyone with an aggressor near.
    let threat = may_defend(&c.avionics, &c.ship) && aggressors.iter().any(|&(s, p)| s == c.system && p.distance(c.ship.position) < DEFEND_RANGE);
    let sightings = if threat || wants_sightings(&c.avionics, &c.ship, t0) { universe_prof::time("sim/crafts/sightings", || sightings(world, snaps, i, c.system, c.ship.position, t0)) } else { Vec::new() };
    let mark = match c.avionics.following.map(|f| f.anchor) {
        Some(universe_avionics::follow::Anchor::Ship(id)) => crate::follow::mark_in(snaps, c.system, c.ship.position, id),
        _ => None,
    };
    let a = &c.avionics;
    let before = Before { target: a.nav_target, clearance: a.clearance, hyperdrive: c.ship.hyperdrive, departing: a.route.departing, velocity: c.ship.velocity, hunting: a.hunting.is_some(), route_active: a.route.active };
    let was_hunting = a.hunting.is_some();
    let (mut events, mut requests) = (Vec::new(), Vec::new());
    let mut end = None;
    let delay = delay.map(|(tick, k)| (&mut c.inbox, tick, k));
    crate::vessel::turn(world, atc, crate::combat::craft_id(i), &mut c.ship, &mut c.system, &mut c.avionics, t0, real_dt, warp, &mut events, &mut requests, delay, |a, link, ev| {
        let (stick, e) = universe_prof::time("sim/crafts/hunt", || a.hunt(link, &sightings, ev));
        end = e;
        stick.or_else(|| a.follow_step(link, mark, ev))
    });
    Turn { events, requests, end, was_hunting, before }
}

/// What craft `i`'s radar sees (at `pos` in `system`, at `t`) of everyone
/// else in its system, as the snapshot has them, with who's a pirate (they
/// know their own) and who's sheltered or wrecked.
fn sightings(world: &universe_world::World, snaps: &[Snap], i: usize, system: usize, pos: DVec3, t: f64) -> Vec<Sighting> {
    let me = crate::combat::craft_id(i);
    // Shelter is real: docked or landed, or under a defence turret's guns.
    let guns: Vec<DVec3> = world.turret_motions_at(system, t).into_iter().map(|(_, p, _)| p).collect();
    let sheltered = |s: &Snap| s.landed || guns.iter().any(|p| p.distance(s.position) < universe_world::turrets::TURRET_RANGE + universe_avionics::hunter::SHELTER_MARGIN);
    snaps
        .iter()
        .enumerate()
        .filter(|&(id, s)| id != me && s.system == system && !s.transit && s.position.distance(pos) < RADAR_RANGE)
        .map(|(id, s)| Sighting {
            id,
            position: s.position,
            velocity: s.velocity,
            pirate: s.pirate,
            docked: sheltered(s),
            destroyed: s.destroyed,
            hyperdrive: s.hyperdrive,
            landed: s.landed,
            aggressed: s.aggressed,
            hull: s.hull,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use universe_world::Controls;

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
        for (count, warp, frames) in [(100, 20.0, 60 * 60 * 30), (1000, 20.0, 60 * 60 * 3), (1000, 2.0, 60 * 30), (100, 1.0, 60 * 60 * 60 * 2)].into_iter().filter(|c| std::env::var("ONLY_1X").is_err() || c.1 == 1.0) {
            let mut u = Universe::new(1984);
            u.spawn_settlers(count, 99);
            let t0 = u.world.time;
            let start = std::time::Instant::now();
            for _ in 0..frames {
                u.step_world(1.0 / 60.0, warp, &Controls::default());
            }
            let ms = start.elapsed().as_secs_f64() * 1000.0 / frames as f64;
            let flying = u.crafts.iter().filter(|c| c.ship.is_flying()).count();
            let (mut by_pirates, mut by_traders) = (0, 0);
            for r in &u.crash_log {
                let pirate = u.crafts.iter().any(|c| c.name == r.craft && c.avionics.pirate);
                if pirate { by_pirates += 1 } else { by_traders += 1 }
                eprintln!("    crash ({}): {} into {} at {:.0} m/s, target {:?}, clearance {:?}, hunting {} route {}", if pirate { "pirate" } else { "trader" }, r.craft, r.body, r.speed, r.target, r.clearance.map(|c| c.target), r.hunting, r.route_active);
            }
            eprintln!("  crash log: pirates {by_pirates}, traders {by_traders}");
            eprintln!(
                "{count} settlers at {warp}x for {:.1} game h: {ms:.2} ms/frame; stops {}, transits {}, routes done {}, crashes {}; {flying} flying now; pirates {}: hunts {}, kills {} (shot down {}); collisions {} (wrecked {})",
                (u.world.time - t0) / 3600.0,
                u.traffic.stops,
                u.traffic.transits,
                u.traffic.routes_completed,
                u.traffic.crashes,
                u.crafts.iter().filter(|c| c.avionics.pirate).count(),
                u.traffic.hunts,
                u.traffic.pirate_kills,
                u.traffic.shot_down,
                u.traffic.collisions,
                u.traffic.collision_losses
            );
        }
    }
}

#[cfg(test)]
mod pirate_tests {
    use glam::DVec3;
    use universe_world::{Controls, ShipState};

    use crate::universe::Universe;

    #[test]
    fn a_pirate_hunts_down_a_trader_and_goes_back_to_its_route() {
        let mut u = Universe::new(1984);
        u.spawn_settlers(2, 1);
        // Far from any haven, so the trader can't run for the guns.
        let (sys, pos, vel) = (u.ship_system, u.ship.position + DVec3::new(1.5e6, 0.0, 0.0), u.ship.velocity);
        // Park our own ship far off, out of the way.
        u.ship.position += DVec3::new(0.0, 0.0, 2.0e6);
        for (k, c) in u.crafts.iter_mut().enumerate() {
            c.system = sys;
            c.ship.state = ShipState::Flying;
            c.ship.hyperdrive = false;
            c.ship.position = pos + DVec3::new(8_000.0 * k as f64, 30_000.0, 0.0);
            c.ship.velocity = vel;
            c.avionics.pirate = k == 0;
            c.avionics.route.active = false; // both just drift, the pirate hunts
            c.avionics.route.dwell_until = None;
        }
        let mut hunted_at = None;
        for frame in 0..60 * 600 {
            u.step_world(1.0 / 60.0, 1.0, &Controls::default());
            if hunted_at.is_none() && u.crafts[0].avionics.hunting.is_some() {
                hunted_at = Some(frame);
            }
            if u.traffic.pirate_kills > 0 && u.crafts[0].avionics.hunting.is_none() {
                eprintln!("hunt began frame {hunted_at:?}, kill and stand-down by {:.0} s; pirate hull {:.2}", frame as f64 / 60.0, u.crafts[0].ship.hull);
                assert!(!u.crafts[0].ship.armed, "stood down");
                assert!(u.crafts[0].avionics.route.active, "back to its route");
                return;
            }
        }
        let p = &u.crafts[0].ship;
        panic!(
            "no kill: hunts {} kills {} pirate {:?} {:?} dist {:.0} ammo {} trader hull {:.2}",
            u.traffic.hunts,
            u.traffic.pirate_kills,
            p.state,
            u.crafts[0].avionics.hunting,
            p.position.distance(u.crafts[1].ship.position),
            p.ammo,
            u.crafts[1].ship.hull
        );
    }
}

#[cfg(test)]
mod pirate_debug {
    use glam::DVec3;
    use universe_world::{BodyKind, Controls, GateFrame, ShipState};

    use crate::universe::Universe;

    #[test]
    #[ignore]
    fn debug_pirate_chase_through_gate() {
        let mut u = Universe::new(1984);
        u.spawn_settlers(2, 1);
        let sysi = u.ship_system;
        let sys = u.system(sysi);
        let gate = sys.bodies.iter().position(|b| b.kind == BodyKind::Gate).unwrap();
        let mut pos = Vec::new();
        sys.positions(u.world.time, &mut pos);
        let f = GateFrame::new(&sys, gate, u.world.time, &pos);
        u.ship.position += DVec3::new(0.0, 0.0, 2.0e6);
        let axis = f.axis();
        for (k, c) in u.crafts.iter_mut().enumerate() {
            c.system = sysi;
            c.ship.state = ShipState::Flying;
            c.ship.hyperdrive = false;
            c.ship.position = f.center - axis * (3_000.0 + 4_000.0 * (1 - k) as f64);
            c.ship.velocity = f.velocity + axis * 150.0;
            c.avionics.pirate = k == 0;
            c.avionics.route.active = false;
            c.avionics.route.dwell_until = None;
        }
        for frame in 0..60 * 200 {
            u.step_world(1.0 / 60.0, 1.0, &Controls::default());
            let sys = u.system(sysi);
            sys.positions(u.world.time, &mut pos);
            let p = &u.crafts[0];
            let gc = pos[gate];
            let off = p.ship.position - gc;
            if (frame % 15 == 0 && off.length() < 20_000.0) || !p.ship.is_flying() {
                eprintln!(
                    "t {:.1} d_gate {:.0} closing {:.0} thr {:.2} rcs {:.2} fwd·out {:.2} hunting {:?} state {:?}",
                    frame as f64 / 60.0,
                    off.length(),
                    -(p.ship.velocity - sys.velocity(gate, u.world.time)).dot(off.normalize()),
                    p.ship.throttle,
                    p.ship.rcs,
                    p.ship.forward().dot(off.normalize()),
                    p.avionics.hunting.map(|h| h.target),
                    std::mem::discriminant(&p.ship.state)
                );
            }
            if !p.ship.is_flying() {
                break;
            }
        }
    }
}

#[cfg(test)]
mod collision_debug {
    use universe_world::{Controls, ShipState};

    use crate::universe::Universe;

    /// Where collision wrecks happen: what both ships were doing a frame before.
    #[test]
    #[ignore]
    fn debug_where_collisions_happen() {
        let mut u = Universe::new(1984);
        u.spawn_settlers(100, 99);
        let mut seen = 0;
        let mut before: Vec<String> = Vec::new();
        for frame in 0..60 * 60 * 30 {
            let snap: Vec<String> = u
                .crafts
                .iter()
                .map(|c| {
                    let state = match c.ship.state {
                        ShipState::Flying => format!("flying {:.0} m/s", c.ship.velocity.length()),
                        ShipState::Landed { .. } => "landed".into(),
                        _ => "other".into(),
                    };
                    format!("{} {state} clearance {:?} route next {} dwell {:?}", c.name, c.avionics.clearance.map(|x| (x.target, x.phase, x.pad)), c.avionics.route.next, c.avionics.route.dwell_until.map(|d| d > 0.0))
                })
                .collect();
            u.step_world(1.0 / 60.0, 1.0, &Controls::default());
            let kills: Vec<_> = u.kills.iter().filter(|k| k.weapon == "COLLISION").cloned().collect();
            for k in kills.iter().skip(seen) {
                let v = before.get(k.victim - 1).cloned().unwrap_or_default();
                let w = before.get(k.killer - 1).cloned().unwrap_or_default();
                eprintln!("t {:.0}: {v}  <->  {w}", frame as f64 / 60.0);
            }
            seen = kills.len();
            before = snap;
            if seen > 12 {
                break;
            }
        }
        eprintln!("collision wrecks {}, collisions {}", u.traffic.collision_losses, u.traffic.collisions);
    }
}
