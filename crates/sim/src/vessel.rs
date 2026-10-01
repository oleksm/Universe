//! One ship's turn in the tick. Every ship — the player's and each settler's
//! — goes through the same steps in the same order:
//!
//! 1. its avionics run their per-frame programs (the route autopilot, the
//!    hyperjump), sending `ShipCommands` over the bus;
//! 2. the pilot's stick (or nothing, for a settler) becomes the frame's
//!    commands, unless a computer is flying the ship;
//! 3. the world steps the ship: the commands set its devices, the physics
//!    kernel moves it (the flight computer commanding the devices between
//!    substeps), and the world's device and contact rules judge the facts
//!    the kernel reports;
//! 4. the physical events that came of it go to the avionics (which take
//!    note: a clearance ends on landing, a target is forgotten on leaving
//!    the system…) and on to the pilot's feed;
//! 5. the avionics conclude the frame (a hyperdrive arrival, a clearance
//!    that has lapsed).

use std::sync::Arc;

use universe_avionics::{Avionics, Bus, Event};
use universe_protocol::PadGrant;
use universe_services::TrafficControl;
use universe_world::{Controls, Ship, ShipCommands, ShipEvent, StarSystem, StepResult, World};

/// A ship and the avionics flying it, borrowed for their turn.
pub(crate) struct Vessel<'a> {
    /// Its id (the player's ship 0, craft i: i + 1), as traffic control knows it.
    pub id: usize,
    pub ship: &'a mut Ship,
    /// Galaxy index of the system it's in.
    pub system: &'a mut usize,
    pub avionics: &'a mut Avionics,
    /// The pilot's feed: what happened, as the avionics report it.
    pub events: &'a mut Vec<Event>,
    /// Traffic control, for the pilot's requests.
    pub atc: &'a mut TrafficControl,
}

/// The avionics' bus to the ship being stepped: its sensors read the ship
/// and the world, its commands go to the world's devices.
pub(crate) struct Link<'a> {
    world: &'a mut World,
    ship: &'a mut Ship,
    system: usize,
    id: usize,
    atc: &'a mut TrafficControl,
    /// What the devices did, for the feed.
    happened: Vec<ShipEvent>,
}

impl Bus for Link<'_> {
    fn ship(&self) -> &Ship {
        self.ship
    }

    fn system(&self) -> usize {
        self.system
    }

    fn star_system(&mut self) -> Arc<StarSystem> {
        self.world.system(self.system)
    }

    fn time(&self) -> f64 {
        self.world.time
    }

    fn gate_links(&self) -> &[(usize, usize)] {
        &self.world.gate_links
    }

    fn id(&self) -> usize {
        self.id
    }

    fn request_pad(&mut self, port: usize) -> PadGrant {
        let now = self.world.time;
        self.atc.request_pad(self.system, port, self.id, now)
    }

    fn request_corridor(&mut self, body: usize) -> Option<usize> {
        let now = self.world.time;
        self.atc.request_corridor(self.system, body, self.id, now)
    }

    fn turrets(&mut self) -> Vec<(glam::DVec3, glam::DVec3)> {
        self.world.turret_motions(self.system).into_iter().map(|(_, p, v)| (p, v)).collect()
    }

    fn positions(&mut self) -> (Arc<StarSystem>, Vec<glam::DVec3>) {
        let sys = self.world.system(self.system);
        (sys, (*self.world.rails_now(self.system)).clone())
    }

    fn actuate(&mut self, c: &ShipCommands) {
        self.world.command(self.ship, self.system, c, &mut self.happened);
    }

    fn feed(&mut self) -> Vec<ShipEvent> {
        std::mem::take(&mut self.happened)
    }

    fn request_clearance(&mut self, target: Option<universe_avionics::NavTarget>) -> Result<universe_avionics::NavTarget, String> {
        clearance(self.world, self.ship, self.system, target, self.world.time)
    }

    fn clearance_holds(&mut self, target: universe_avionics::NavTarget) -> bool {
        holds(self.world, self.ship, self.system, target, self.world.time)
    }
}

/// Traffic control on a clearance request from `ship` in `system` at `t`
/// (no target: the nearest station). (Its rules; see `world::traffic`.)
fn clearance(world: &World, ship: &Ship, system: usize, target: Option<universe_avionics::NavTarget>, t: f64) -> Result<universe_avionics::NavTarget, String> {
    let sys = world.system(system);
    let positions = world.rails_at(system, t);
    let target = target.or_else(|| universe_services::atc::nearest_station(&sys, ship.position, &positions));
    universe_services::atc::request(&sys, ship, target, t, &positions)
}

/// Does traffic control still stand by `ship`'s clearance for `target`?
fn holds(world: &World, ship: &Ship, system: usize, target: universe_avionics::NavTarget, t: f64) -> bool {
    let sys = world.system(system);
    let positions = world.rails_at(system, t);
    !universe_services::atc::lapsed(&sys, ship, target, t, &positions)
}

impl Vessel<'_> {
    /// Run `f` on the avionics, connected to the ship by the bus.
    pub fn run<R>(&mut self, world: &mut World, f: impl FnOnce(&mut Avionics, &mut Link, &mut Vec<Event>) -> R) -> R {
        let mut link = Link { world, ship: self.ship, system: *self.system, id: self.id, atc: &mut *self.atc, happened: Vec::new() };
        f(self.avionics, &mut link, self.events)
    }

    /// The ship's turn: `real_dt` real seconds at `warp`, with the pilot's
    /// `controls` (see the module docs for the order).
    pub fn tick(&mut self, world: &mut World, controls: &Controls, real_dt: f64, warp: f64) -> StepResult {
        let dt = real_dt * warp;
        let program = universe_prof::time("sim/crafts/tick/avionics prepare", || self.run(world, |a, link, events| a.prepare(link, dt, events)));
        // The pilot's stick turns the ship, unless a program is flying it.
        let turn = if self.avionics.flies(self.ship) { program } else { Some(*controls) };
        let commands = ShipCommands { turn, ..self.ship.holding() };
        let mut happened = Vec::new();
        let result = universe_prof::time("sim/crafts/tick/world step", || world.step_ship(self.ship, self.system, &commands, real_dt, warp, &mut happened));
        self.avionics.record(happened, self.events);
        universe_prof::time("sim/crafts/tick/avionics conclude", || self.run(world, |a, link, events| a.conclude(link, events)));
        result
    }
}

/// A traffic control request made while ships step side by side: made in
/// ship order once they're all done (so the outcome doesn't depend on which
/// thread got there first). Meanwhile the ship has last tick's answer.
#[derive(Clone, Copy, Debug)]
pub(crate) enum Request {
    Pad { system: usize, port: usize, ship: usize, now: f64 },
    Corridor { system: usize, body: usize, ship: usize, now: f64 },
}

impl Request {
    pub fn make(self, traffic: &mut TrafficControl) {
        match self {
            Request::Pad { system, port, ship, now } => {
                traffic.request_pad(system, port, ship, now);
            }
            Request::Corridor { system, body, ship, now } => {
                traffic.request_corridor(system, body, ship, now);
            }
        }
    }
}

/// A craft's commands on their way to its devices: with a delay of k ticks
/// (to test pilots against the lag they'll have once they run apart from
/// the world), each takes effect k ticks after it was given. Meanwhile the
/// pilot sees its own last settings (as a ship's controls show what was
/// set, not yet what the devices do).
#[derive(Clone, Debug, Default)]
pub struct Inbox {
    pending: std::collections::VecDeque<(u64, Pending)>,
    /// The pilot's last settings: engine, thrusters.
    intent: Option<(f64, glam::DVec3)>,
}

#[derive(Clone, Debug)]
enum Pending {
    Devices(Box<ShipCommands>),
    Turn(Option<Controls>),
}

impl Inbox {
    /// Hand the devices what's due by `tick`; the turn due, if one is.
    fn deliver(&mut self, world: &World, ship: &mut Ship, system: usize, t: f64, tick: u64, events: &mut Vec<ShipEvent>) -> Option<Option<Controls>> {
        let mut turn = None;
        while self.pending.front().is_some_and(|(due, _)| *due <= tick) {
            match self.pending.pop_front().expect("due").1 {
                Pending::Devices(c) => world.command_at(ship, system, &c, t, events),
                Pending::Turn(c) => turn = Some(c),
            }
        }
        if self.pending.is_empty() {
            self.intent = None;
        }
        turn
    }
}

/// The bus while ships step side by side: the world shared and read-only,
/// the ship's own clock, traffic control's answers as of the last tick (the
/// requests go in `requests`).
pub(crate) struct FrameLink<'a> {
    pub world: &'a World,
    /// Traffic control as it stood at the tick's start (requests are made after).
    pub atc: &'a TrafficControl,
    pub ship: &'a mut Ship,
    pub system: usize,
    pub id: usize,
    pub time: f64,
    pub requests: &'a mut Vec<Request>,
    /// Commands delayed (see `Inbox`): the inbox, the tick now and the delay.
    pub delay: Option<(&'a mut Inbox, u64, u64)>,
    /// The ship as its pilot sees it, with its own last settings (while delayed).
    pub seen: Option<Ship>,
    /// What the devices did, for the feed.
    pub happened: Vec<ShipEvent>,
}

impl FrameLink<'_> {
    fn refresh_seen(&mut self) {
        self.seen = self.delay.as_ref().and_then(|(inbox, _, _)| inbox.intent).map(|(throttle, rcs)| Ship { throttle, rcs, ..self.ship.clone() });
    }
}

impl Bus for FrameLink<'_> {
    fn ship(&self) -> &Ship {
        self.seen.as_ref().unwrap_or(self.ship)
    }

    fn system(&self) -> usize {
        self.system
    }

    fn star_system(&mut self) -> Arc<StarSystem> {
        self.world.system(self.system)
    }

    fn time(&self) -> f64 {
        self.time
    }

    fn gate_links(&self) -> &[(usize, usize)] {
        &self.world.gate_links
    }

    fn id(&self) -> usize {
        self.id
    }

    fn request_pad(&mut self, port: usize) -> PadGrant {
        self.requests.push(Request::Pad { system: self.system, port, ship: self.id, now: self.time });
        self.atc.peek_pad(self.system, port, self.id)
    }

    fn request_corridor(&mut self, body: usize) -> Option<usize> {
        self.requests.push(Request::Corridor { system: self.system, body, ship: self.id, now: self.time });
        self.atc.peek_corridor(self.system, body, self.id)
    }

    fn turrets(&mut self) -> Vec<(glam::DVec3, glam::DVec3)> {
        self.world.turret_motions_at(self.system, self.time).into_iter().map(|(_, p, v)| (p, v)).collect()
    }

    fn positions(&mut self) -> (Arc<StarSystem>, Vec<glam::DVec3>) {
        (self.world.system(self.system), (*self.world.rails_at(self.system, self.time)).clone())
    }

    fn actuate(&mut self, c: &ShipCommands) {
        match &mut self.delay {
            Some((inbox, tick, k)) => {
                inbox.pending.push_back((*tick + *k, Pending::Devices(Box::new(*c))));
                inbox.intent = Some((c.throttle, c.rcs));
                self.refresh_seen();
            }
            None => self.world.command_at(self.ship, self.system, c, self.time, &mut self.happened),
        }
    }

    fn feed(&mut self) -> Vec<ShipEvent> {
        std::mem::take(&mut self.happened)
    }

    fn request_clearance(&mut self, target: Option<universe_avionics::NavTarget>) -> Result<universe_avionics::NavTarget, String> {
        clearance(self.world, self.ship, self.system, target, self.time)
    }

    fn clearance_holds(&mut self, target: universe_avionics::NavTarget) -> bool {
        holds(self.world, self.ship, self.system, target, self.time)
    }
}

/// A ship's turn while ships step side by side (see `Vessel::tick` for the
/// order): from `t0` for `real_dt` at `warp`, its pilot (a program) having
/// first said what the stick does (`stick`: given the bus, it returns it).
/// What happened goes to `events`, traffic requests to `requests`.
#[allow(clippy::too_many_arguments)]
pub(crate) fn turn(
    world: &World,
    atc: &TrafficControl,
    id: usize,
    ship: &mut Ship,
    system: &mut usize,
    avionics: &mut Avionics,
    t0: f64,
    real_dt: f64,
    warp: f64,
    events: &mut Vec<Event>,
    requests: &mut Vec<Request>,
    delay: Option<(&mut Inbox, u64, u64)>,
    stick: impl FnOnce(&mut Avionics, &mut FrameLink, &mut Vec<Event>) -> Option<Controls>,
) {
    // Delayed commands now due reach the devices first.
    let mut delay = delay;
    let due_turn = match &mut delay {
        Some((inbox, tick, _)) => {
            let mut happened = Vec::new();
            let turn = inbox.deliver(world, ship, *system, t0, *tick, &mut happened);
            avionics.record(happened, events);
            turn
        }
        None => None,
    };
    let delayed = delay.is_some();
    let (stick, program) = {
        let lent = delay.as_mut().map(|(inbox, tick, k)| (&mut **inbox, *tick, *k));
        let mut l = FrameLink { world, atc, ship: &mut *ship, system: *system, id, time: t0, requests: &mut *requests, delay: lent, seen: None, happened: Vec::new() };
        l.refresh_seen();
        let c = stick(avionics, &mut l, events);
        let p = universe_prof::time("sim/crafts/tick/avionics prepare", || avionics.prepare(&mut l, real_dt * warp, events));
        (c, p)
    };
    let turn = if avionics.flies(ship) { program } else { Some(stick.unwrap_or_default()) };
    // Delayed, the stick goes in the inbox too, and the one due now turns the ship.
    let turn = match delay {
        Some((inbox, tick, k)) => {
            inbox.pending.push_back((tick + k, Pending::Turn(turn)));
            due_turn.flatten()
        }
        None if delayed => None,
        None => turn,
    };
    let commands = ShipCommands { turn, ..ship.holding() };
    let mut happened = Vec::new();
    let mut clock = t0;
    universe_prof::time("sim/crafts/tick/world step", || world.step_ship_at(&mut clock, ship, system, &commands, real_dt, warp, &mut happened));
    avionics.record(happened, events);
    let mut l = FrameLink { world, atc, ship, system: *system, id, time: clock, requests, delay: None, seen: None, happened: Vec::new() };
    universe_prof::time("sim/crafts/tick/avionics conclude", || avionics.conclude(&mut l, events));
}
