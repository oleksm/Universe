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
use universe_world::pads::PadGrant;
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
}

/// The avionics' bus to the ship being stepped: its sensors read the ship
/// and the world, its commands go to the world's devices.
pub(crate) struct Link<'a> {
    world: &'a mut World,
    ship: &'a mut Ship,
    system: usize,
    id: usize,
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
        self.world.traffic.request_pad(self.system, port, self.id, now)
    }

    fn request_corridor(&mut self, body: usize) -> Option<usize> {
        let now = self.world.time;
        self.world.traffic.request_corridor(self.system, body, self.id, now)
    }

    fn turrets(&mut self) -> Vec<(glam::DVec3, glam::DVec3)> {
        self.world.turret_motions(self.system).into_iter().map(|(_, p, v)| (p, v)).collect()
    }

    fn positions(&mut self) -> (Arc<StarSystem>, Vec<glam::DVec3>) {
        let sys = self.world.system(self.system);
        (sys, (*self.world.rails_now(self.system)).clone())
    }

    fn command(&mut self, c: &ShipCommands) -> Vec<ShipEvent> {
        let mut events = Vec::new();
        self.world.command(self.ship, self.system, c, &mut events);
        events
    }
}

impl Vessel<'_> {
    /// Run `f` on the avionics, connected to the ship by the bus.
    pub fn run<R>(&mut self, world: &mut World, f: impl FnOnce(&mut Avionics, &mut Link, &mut Vec<Event>) -> R) -> R {
        let mut link = Link { world, ship: self.ship, system: *self.system, id: self.id };
        f(self.avionics, &mut link, self.events)
    }

    /// The ship's turn: `real_dt` real seconds at `warp`, with the pilot's
    /// `controls` (see the module docs for the order).
    pub fn tick(&mut self, world: &mut World, controls: &Controls, real_dt: f64, warp: f64) -> StepResult {
        universe_prof::time("sim/crafts/tick/avionics prepare", || self.run(world, |a, link, events| a.prepare(link, events)));
        // The pilot's stick turns the ship, unless a computer is flying it.
        let commands = ShipCommands { turn: (!self.avionics.flies(self.ship)).then_some(*controls), ..self.ship.holding() };
        let mut happened = Vec::new();
        let mut computer = self.avionics.computer();
        let result = universe_prof::time("sim/crafts/tick/world step", || world.step_ship(self.ship, self.system, &commands, &mut computer, real_dt, warp, &mut happened));
        let arrived = computer.arrived.take();
        self.avionics.record(happened, self.events);
        universe_prof::time("sim/crafts/tick/avionics conclude", || self.run(world, |a, link, events| a.conclude(link, arrived, events)));
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
    pub fn make(self, traffic: &mut universe_world::pads::TrafficControl) {
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

/// The bus while ships step side by side: the world shared and read-only,
/// the ship's own clock, traffic control's answers as of the last tick (the
/// requests go in `requests`).
pub(crate) struct FrameLink<'a> {
    pub world: &'a World,
    pub ship: &'a mut Ship,
    pub system: usize,
    pub id: usize,
    pub time: f64,
    pub requests: &'a mut Vec<Request>,
}

impl Bus for FrameLink<'_> {
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
        self.world.traffic.peek_pad(self.system, port, self.id)
    }

    fn request_corridor(&mut self, body: usize) -> Option<usize> {
        self.requests.push(Request::Corridor { system: self.system, body, ship: self.id, now: self.time });
        self.world.traffic.peek_corridor(self.system, body, self.id)
    }

    fn turrets(&mut self) -> Vec<(glam::DVec3, glam::DVec3)> {
        self.world.turret_motions_at(self.system, self.time).into_iter().map(|(_, p, v)| (p, v)).collect()
    }

    fn positions(&mut self) -> (Arc<StarSystem>, Vec<glam::DVec3>) {
        (self.world.system(self.system), (*self.world.rails_at(self.system, self.time)).clone())
    }

    fn command(&mut self, c: &ShipCommands) -> Vec<ShipEvent> {
        let mut events = Vec::new();
        self.world.command_at(self.ship, self.system, c, self.time, &mut events);
        events
    }
}

/// A ship's turn while ships step side by side (see `Vessel::tick` for the
/// order): from `t0` for `real_dt` at `warp`, its pilot (a program) having
/// first said what the stick does (`stick`: given the bus, it returns it).
/// What happened goes to `events`, traffic requests to `requests`.
#[allow(clippy::too_many_arguments)]
pub(crate) fn turn(
    world: &World,
    id: usize,
    ship: &mut Ship,
    system: &mut usize,
    avionics: &mut Avionics,
    t0: f64,
    real_dt: f64,
    warp: f64,
    events: &mut Vec<Event>,
    requests: &mut Vec<Request>,
    stick: impl FnOnce(&mut Avionics, &mut FrameLink, &mut Vec<Event>) -> Option<Controls>,
) {
    let controls = {
        let mut l = FrameLink { world, ship: &mut *ship, system: *system, id, time: t0, requests: &mut *requests };
        let c = stick(avionics, &mut l, events);
        universe_prof::time("sim/crafts/tick/avionics prepare", || avionics.prepare(&mut l, events));
        c.unwrap_or_default()
    };
    let commands = ShipCommands { turn: (!avionics.flies(ship)).then_some(controls), ..ship.holding() };
    let mut happened = Vec::new();
    let mut clock = t0;
    let mut computer = avionics.computer();
    universe_prof::time("sim/crafts/tick/world step", || world.step_ship_at(&mut clock, ship, system, &commands, &mut computer, real_dt, warp, &mut happened));
    let arrived = computer.arrived.take();
    avionics.record(happened, events);
    let mut l = FrameLink { world, ship, system: *system, id, time: clock, requests };
    universe_prof::time("sim/crafts/tick/avionics conclude", || avionics.conclude(&mut l, arrived, events));
}
