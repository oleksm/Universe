//! Traffic: the settlers' crafts — other ships in the world, flown by the
//! same code as the player's, each by its own avionics along its own
//! reproducible route — with the totals of what they got up to, and a log of
//! their crashes (for diagnosing autopilots).

use glam::DVec3;
use universe_avionics::route::{Route, Stop};
use universe_avionics::{Avionics, Clearance, Event, NavTarget};
use std::sync::Arc;

use universe_world::{BodyKind, Ship, ShipCommands, ShipEvent, ShipState};

use crate::rng::Rng;
use crate::universe::Universe;

/// Stops on a settler's route.
const ROUTE_STOPS: usize = 10;
/// Crash reports kept (the most recent).
const CRASH_LOG: usize = 50;

/// Another ship in the world: the body (its ship), and what the world
/// knows of the pilot flying it (see `pilots`: the pilot itself is apart).
pub struct Craft {
    pub name: String,
    pub ship: Ship,
    /// Galaxy index of the system it's in.
    pub system: usize,
    /// What its pilot shows (posted with its commands).
    pub status: crate::pilots::Status,
    /// When its pilot last posted (world time), and whether the dead-man rule has cut in since.
    pub last_posted: f64,
    pub dead_man: bool,
    /// A new route has been ordered and not yet taken up.
    pub(crate) route_ordered: bool,
    /// Seed of its current route (a new one is made when it finishes).
    pub route_seed: u64,
    /// A trader (see `commerce`): buys and sells at its stops.
    pub trader: bool,
    /// What it paid per unit for what it carries (its own reckoning; its
    /// money and hold are in the ledger).
    pub paid: std::collections::BTreeMap<usize, f64>,
    /// Its pilot's commands on their way to the devices.
    pub inbox: crate::vessel::Inbox,
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
            let avionics = Avionics { route, pirate, ..Avionics::default() };
            self.crafts.push(Craft {
                // Named for what they do (the number stays each craft's own).
                name: format!("{} {}", if pirate { "Pirate" } else if trader { "Trader" } else { "Settler" }, self.crafts.len() + 1),
                ship,
                system: first.system,
                status: crate::pilots::Status::of(&avionics),
                last_posted: self.world.time,
                dead_man: false,
                route_ordered: false,
                route_seed,
                trader,
                paid: Default::default(),
                inbox: Default::default(),
            });
            self.pool.add(crate::pilots::Pilot::new(avionics));
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

    /// Every craft's step from `t0`, side by side on all cores: the
    /// commands its pilot posted that are due reach its devices, and the
    /// world steps it (each against the world as it was at the frame's
    /// start). Then, in craft order, what came of it.
    pub(crate) fn fly_crafts(&mut self, t0: f64, real_dt: f64, warp: f64) {
        use rayon::prelude::*;
        // What they'll look up, gathered first: read without locks.
        let mut systems: Vec<usize> = self.crafts.iter().map(|c| c.system).collect();
        systems.sort_unstable();
        systems.dedup();
        universe_prof::time("sim/crafts/freeze", || self.world.freeze(&systems, t0, t0 + real_dt * warp));
        let (world, tick) = (&self.world, self.tick);
        let steps: Vec<(Vec<ShipEvent>, DVec3)> = {
            let _p = universe_prof::scope("sim/crafts/side by side");
            self.crafts.par_iter_mut().map(|c| step_craft(world, c, t0, real_dt, warp, tick)).collect()
        };
        self.world.thaw();
        let _p = universe_prof::scope("sim/crafts/in order");
        for (i, (events, velocity)) in steps.into_iter().enumerate() {
            self.after_step(i, events, velocity);
        }
    }

    /// What came of craft `i`'s step, in order: for the services, the
    /// records, and its pilot's feed.
    fn after_step(&mut self, i: usize, happened: Vec<ShipEvent>, velocity: DVec3) {
        let events: Vec<Event> = happened.iter().cloned().map(Event::Ship).collect();
        self.log_events(crate::combat::craft_id(i), &events);
        self.traffic_events(crate::combat::craft_id(i), &events);
        let crashed = happened.iter().find_map(|e| match e {
            ShipEvent::Crashed { body } => Some(body.clone()),
            _ => None,
        });
        if let Some(body) = crashed {
            let now = self.world.time;
            self.recorder.file(now, crate::combat::craft_id(i), self.crafts[i].name.to_uppercase(), body.clone(), None);
            let system = self.crafts[i].system;
            let sys = self.system(system);
            let speed = sys.bodies.iter().position(|x| x.name == body).map_or(0.0, |x| (velocity - sys.velocity(x, now)).length());
            let (c, st) = (&self.crafts[i], &self.crafts[i].status);
            self.crash_log.push(CrashReport {
                craft: c.name.clone(),
                body,
                system,
                time: now,
                target: st.nav_target,
                clearance: st.clearance,
                hyperdrive: c.ship.hyperdrive,
                departing: st.departing,
                speed,
                hunting: st.hunting.is_some(),
                route_active: st.route_active,
            });
            if self.crash_log.len() > CRASH_LOG {
                self.crash_log.remove(0);
            }
        }
        for e in &happened {
            match e {
                ShipEvent::GateEntered { .. } => self.records.stats.transits += 1,
                ShipEvent::Crashed { .. } => self.records.stats.crashes += 1,
                _ => {}
            }
        }
        if !happened.is_empty() {
            self.pool.send(i, crate::pilots::Msg::Feed(happened));
        }
        self.dispatch(i);
    }

    /// Pilots' postings: each due tick's commands into its craft's inbox (a
    /// late one at once; a stale one dropped), its requests to traffic
    /// control, what it reports to the services, and what it shows.
    pub(crate) fn post(&mut self, mut postings: Vec<crate::pilots::Posting>) {
        use universe_avionics::hunter::FLEE_HULL;
        postings.sort_by_key(|p| (p.thought, p.craft));
        for p in postings {
            let due = p.due();
            if due + crate::pilots::LATE_HORIZON < self.tick {
                self.pool.dropped += 1;
                continue;
            }
            if due < self.tick {
                self.pool.late += 1;
            }
            let i = p.craft;
            for r in p.requests {
                r.make(&mut self.atc);
            }
            let c = &mut self.crafts[i];
            c.inbox.post(due.max(self.tick), p.devices, p.turn);
            c.last_posted = self.world.time;
            c.dead_man = false;
            if p.status.route_active {
                c.route_ordered = false;
            }
            c.status = p.status;
            if p.hunt_begun && let Some(h) = c.status.hunting {
                if h.lawful {
                    self.records.stats.defences += 1;
                } else {
                    self.records.stats.hunts += 1;
                }
            }
            // A defender that broke off hurt runs for the guns.
            if p.hunt_end.is_some() && !c.status.pirate && c.ship.hull < FLEE_HULL {
                self.flee(i);
            }
            self.log_events(crate::combat::craft_id(i), &p.events);
            self.traffic_events(crate::combat::craft_id(i), &p.events);
            for e in p.events {
                match e {
                    Event::RouteStop { .. } => {
                        self.records.stats.stops += 1;
                        if self.crafts[i].trader {
                            self.craft_trades(i);
                        }
                    }
                    Event::RouteComplete => self.records.stats.routes_completed += 1,
                    _ => {}
                }
            }
        }
    }

    /// The dead-man rule: a ship whose pilot has posted nothing for
    /// `DEAD_MAN` seconds has its engines cut and its weapons made safe (a
    /// core rule: the hardware does it).
    pub(crate) fn dead_man(&mut self) {
        let now = self.world.time;
        for i in 0..self.crafts.len() {
            let c = &mut self.crafts[i];
            if c.dead_man || now - c.last_posted < crate::pilots::DEAD_MAN || !c.ship.is_flying() {
                continue;
            }
            c.dead_man = true;
            let cut = ShipCommands { throttle: 0.0, rcs: DVec3::ZERO, arm: Some(false), weapons: Some(Default::default()), ..c.ship.holding() };
            let mut happened = Vec::new();
            self.world.command_at(&mut c.ship, c.system, &cut, now, &mut happened);
            let events: Vec<Event> = happened.into_iter().map(Event::Ship).collect();
            self.log_events(crate::combat::craft_id(i), &events);
        }
    }

    /// The world as pilots read it, now.
    pub(crate) fn pilot_view(&mut self, dt: f64) -> crate::pilots::PilotView {
        let t = self.world.time;
        let charts = self.charts.get_or_insert_with(|| Arc::new(self.world.charts())).clone();
        let mut systems: Vec<usize> = self.crafts.iter().map(|c| c.system).collect();
        systems.sort_unstable();
        systems.dedup();
        let mut rails = std::collections::HashMap::new();
        let mut turrets = std::collections::HashMap::new();
        for s in systems {
            let positions = self.world.rails_at(s, t);
            let sys = self.world.system(s);
            turrets.insert(s, Arc::new(crate::pilots::turret_motions(&charts, s, &sys, t, &positions)));
            rails.insert(s, positions);
        }
        // Everyone as they are now (pilots read the newest).
        self.snapshot();
        crate::pilots::PilotView {
            tick: self.tick,
            time: t,
            dt,
            charts,
            ships: self.crafts.iter().map(|c| (c.system, c.ship.clone())).collect(),
            snaps: self.snaps.clone(),
            aggressors: self.aggressors.clone(),
            board: self.atc.board(),
            rails,
            turrets,
        }
    }

    /// Take the frame's snapshot of every ship, and who's aggressed.
    pub(crate) fn snapshot(&mut self) {
        let now = self.world.time;
        self.snaps.clear();
        let law = &self.law;
        self.snaps.push(Snap::of(self.ship_system, &self.ship, false, law.aggressed(crate::combat::PLAYER, now)));
        let crafts = self.crafts.iter().enumerate().map(|(i, c)| Snap::of(c.system, &c.ship, c.status.pirate, law.aggressed(crate::combat::craft_id(i), now)));
        self.snaps.extend(crafts);
        self.snap_time = now;
        self.aggressors = self.snaps.iter().filter(|s| s.aggressed && s.flying && !s.hyperdrive).map(|s| (s.system, s.position)).collect();
    }

    /// Dispatch: craft `i`, parked with its route done, is given a new one.
    fn dispatch(&mut self, i: usize) {
        let c = &self.crafts[i];
        if c.status.route_active || c.route_ordered || !matches!(c.ship.state, ShipState::Landed { .. }) {
            return;
        }
        let seed = crate::rng::mix(c.route_seed, 1);
        let mut stops = self.settler_route(seed, ROUTE_STOPS);
        // Start from where it is: skip a first stop that's right here.
        if stops.first().is_some_and(|s| s.system == self.crafts[i].system) {
            stops.rotate_left(1);
        }
        let c = &mut self.crafts[i];
        c.route_seed = seed;
        c.route_ordered = true;
        let route = Route { stops, next: 0, active: true, dwell_until: None, departing: false };
        self.pool.send(i, crate::pilots::Msg::Order(crate::pilots::Order::Route(route)));
    }
}

/// Craft `c`'s step (see `fly_crafts`): its events, and its velocity before.
fn step_craft(world: &universe_world::World, c: &mut Craft, t0: f64, real_dt: f64, warp: f64, tick: u64) -> (Vec<ShipEvent>, DVec3) {
    let velocity = c.ship.velocity;
    let mut happened = Vec::new();
    let turn = c.inbox.deliver(world, &mut c.ship, c.system, t0, tick, &mut happened);
    let commands = ShipCommands { turn: turn.flatten(), ..c.ship.holding() };
    let mut clock = t0;
    universe_prof::time("sim/crafts/tick/world step", || world.step_ship_at(&mut clock, &mut c.ship, &mut c.system, &commands, real_dt, warp, &mut happened));
    (happened, velocity)
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

}

