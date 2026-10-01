//! Traffic: the settlers' crafts — other ships in the world, flown by the
//! same code as the player's, each by its own avionics along its own
//! reproducible route — with the totals of what they got up to, and a log of
//! their crashes (for diagnosing autopilots).

use glam::DVec3;
use universe_avionics::{Clearance, Event, NavTarget};
use std::sync::Arc;

use universe_world::{Ship, ShipCommands, ShipEvent, ShipState};

use crate::universe::Universe;

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
    pub status: crate::contract::Status,
    /// When its pilot last posted (world time), and whether the dead-man rule has cut in since.
    pub last_posted: f64,
    pub dead_man: bool,
    /// Its pilot sleeps till this tick (it's not in the views till then,
    /// unless a message wakes it).
    pub(crate) asleep_until: u64,
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
}

impl Snap {
    fn of(system: usize, ship: &Ship, aggressed: bool) -> Self {
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
        }
    }
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
    /// Whether its route was flying.
    pub route_active: bool,
}

impl Universe {
    /// Ships registered by their operator (a client: see `operator`): each
    /// put on its pad at its first stop, with what a new settler starts with
    /// from the world's account. The world knows each by its name and its
    /// ship, and whatever its pilot later declares.
    pub fn register(&mut self, ships: Vec<crate::contract::Registration>) {
        let logged = ships.clone();
        self.note(|| crate::audit::Input::Op(crate::audit::Op::Register(logged)));
        let now = self.world.time;
        for reg in ships {
            let ship = self.world.ship_on(reg.at.system, reg.at.target, reg.pad);
            self.crafts.push(Craft { name: reg.name, ship, system: reg.at.system, status: Default::default(), last_posted: now, dead_man: false, asleep_until: 0, inbox: Default::default() });
            let me = universe_services::Party::Pilot(crate::combat::craft_id(self.crafts.len() - 1));
            self.ledger.settle(me, universe_services::Asset::Credits, crate::commerce::SETTLER_CREDITS, self.tick, universe_protocol::Cause::Rules);
        }
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
            self.tell(i, crate::contract::Msg::Feed(happened));
        }
    }

    /// Pilots' postings: each due tick's commands into its craft's inbox (a
    /// late one at once; a stale one dropped), its requests to traffic
    /// control, what it reports to the services, and what it shows.
    pub(crate) fn post(&mut self, mut postings: Vec<crate::contract::Posting>) {
        postings.sort_by_key(|p| (p.thought, p.id));
        for p in postings {
            let due = p.due();
            if due + crate::contract::LATE_HORIZON < self.tick {
                self.dropped += 1;
                continue;
            }
            if due < self.tick {
                self.late += 1;
            }
            // A gunner's orders, for its turret's gun.
            if let Some(c) = p.gun {
                self.turret_orders.push_back((due.max(self.tick), p.id, c));
                continue;
            }
            if p.id == crate::combat::PLAYER {
                for r in p.requests {
                    self.request(p.id, r);
                }
                self.player_inbox.post(due.max(self.tick), p.seen, p.devices, p.turn);
                self.player_status = p.status;
                self.log_events(crate::combat::PLAYER, &p.events);
                self.traffic_events(crate::combat::PLAYER, &p.events);
                self.events.extend(p.events);
                continue;
            }
            let i = p.id - 1;
            // Going to sleep first: an answer to its requests wakes it.
            if let Some(t) = p.sleep_until {
                self.crafts[i].asleep_until = t;
            }
            let requests = universe_prof::scope("sim/postings/requests");
            for r in p.requests {
                self.request(p.id, r);
            }
            drop(requests);
            let c = &mut self.crafts[i];
            c.inbox.post(due.max(self.tick), p.seen, p.devices, p.turn);
            c.last_posted = self.world.time;
            c.dead_man = false;
            c.status = p.status;
            let _events = universe_prof::scope("sim/postings/events");
            self.log_events(crate::combat::craft_id(i), &p.events);
            self.traffic_events(crate::combat::craft_id(i), &p.events);
            for e in p.events {
                match e {
                    Event::RouteStop { .. } => self.records.stats.stops += 1,
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
            if c.dead_man || now - c.last_posted < crate::contract::DEAD_MAN || !c.ship.is_flying() {
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

    /// The world as the player's cockpit reads it: the pilots' view, and
    /// the crafts' transponders.
    pub(crate) fn cockpit_view(&mut self, world: Arc<crate::contract::PilotView>) -> crate::contract::CockpitView {
        let now = self.world.time;
        let mut transponders = std::collections::HashMap::new();
        let (system, at) = (self.ship_system, self.ship.position);
        for (i, c) in self.crafts.iter().enumerate().filter(|(_, c)| c.system == system && c.ship.position.distance(at) < universe_world::radar::RADAR_RANGE) {
            let destination = c.status.next_stop.map(|s| universe_avionics::route::stop_name(&self.world.system(s.system), s).to_uppercase());
            transponders.insert(i, crate::contract::Transponder {
                name: c.name.clone(),
                activity: crate::contacts::activity(c),
                destination,
                hull: c.ship.hull,
                aggressed: self.law.aggressed(crate::combat::craft_id(i), now),
            });
        }
        crate::contract::CockpitView { world, transponders }
    }

    /// The world as pilots read it, now.
    pub(crate) fn pilot_view(&mut self, dt: f64) -> crate::contract::PilotView {
        let t = self.world.time;
        let charts = self.charts.get_or_insert_with(|| Arc::new(self.world.charts())).clone();
        let mut systems: Vec<usize> = self.crafts.iter().map(|c| c.system).chain([self.ship_system]).collect();
        systems.sort_unstable();
        systems.dedup();
        let mut rails = std::collections::HashMap::new();
        let mut turrets = std::collections::HashMap::new();
        for s in systems {
            let positions = self.world.rails_at(s, t);
            let sys = self.world.system(s);
            let mut guns = crate::contract::turret_motions(&charts, s, &sys, t, &positions);
            for g in &mut guns {
                g.aim = self.world.turret_gun(g.id).map(|gun| gun.aim);
            }
            turrets.insert(s, Arc::new(guns));
            rails.insert(s, positions);
        }
        // Everyone as they are now (pilots read the newest); full ships only
        // of the pilots awake (and ours).
        self.snapshot();
        self.snapped_at = self.tick;
        let tick = self.tick;
        let mut ships: std::collections::HashMap<usize, (usize, Ship), universe_physics::pairs::CellHash> = Default::default();
        ships.insert(crate::combat::PLAYER, (self.ship_system, self.ship.clone()));
        for (i, c) in self.crafts.iter().enumerate().filter(|(_, c)| c.asleep_until <= tick) {
            ships.insert(crate::combat::craft_id(i), (c.system, c.ship.clone()));
        }
        crate::contract::PilotView {
            tick: self.tick,
            time: t,
            dt,
            charts,
            ships,
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
        let law = &self.law;
        let mut snaps = Vec::with_capacity(self.crafts.len() + 1);
        snaps.push(Snap::of(self.ship_system, &self.ship, law.aggressed(crate::combat::PLAYER, now)));
        use rayon::prelude::*;
        let crafts: Vec<Snap> = self.crafts.par_iter().enumerate().map(|(i, c)| Snap::of(c.system, &c.ship, law.aggressed(crate::combat::craft_id(i), now))).collect();
        snaps.extend(crafts);
        self.snaps = Arc::new(snaps);
        self.snap_time = now;
        self.aggressors = self.snaps.iter().filter(|s| s.aggressed && s.flying && !s.hyperdrive).map(|s| (s.system, s.position)).collect();
    }

}

/// Craft `c`'s step (see `fly_crafts`): its events, and its velocity before.
fn step_craft(world: &universe_world::World, c: &mut Craft, t0: f64, real_dt: f64, warp: f64, tick: u64) -> (Vec<ShipEvent>, DVec3) {
    let velocity = c.ship.velocity;
    let mut happened = Vec::new();
    let turn = c.inbox.deliver(world, &mut c.ship, c.system, t0, tick, &mut happened);
    let commands = ShipCommands { turn, ..c.ship.holding() };
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

