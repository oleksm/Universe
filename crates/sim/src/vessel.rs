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

use std::rc::Rc;

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

    fn star_system(&mut self) -> Rc<StarSystem> {
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

    fn request_corridor(&mut self, body: usize) -> bool {
        self.world.traffic.request_corridor(self.system, body, self.id)
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
        self.run(world, |a, link, events| a.prepare(link, events));
        // The pilot's stick turns the ship, unless a computer is flying it.
        let commands = ShipCommands { turn: (!self.avionics.flies(self.ship)).then_some(*controls), ..self.ship.holding() };
        let mut happened = Vec::new();
        let mut computer = self.avionics.computer();
        let result = world.step_ship(self.ship, self.system, &commands, &mut computer, real_dt, warp, &mut happened);
        let arrived = computer.arrived.take();
        self.avionics.record(happened, self.events);
        self.run(world, |a, link, events| a.conclude(link, arrived, events));
        result
    }
}
