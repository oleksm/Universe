//! Test helpers: one ship in a world, flown by hand.

use std::sync::Arc;

use glam::{DQuat, DVec3};

use crate::events::ShipEvent;
use crate::ship::{Controls, HyperdriveCommand, Ship, ShipCommands};
use crate::system::StarSystem;
use crate::world::{Manual, StepResult, World};

/// A ship in a world, respawned at the home station.
pub struct Probe {
    pub world: World,
    pub ship: Ship,
    pub system: usize,
    pub events: Vec<ShipEvent>,
}

impl Probe {
    pub fn new(seed: u64) -> Self {
        let mut p = Self { world: World::new(seed), ship: Ship::new(DVec3::ZERO, DVec3::ZERO, DQuat::IDENTITY), system: 0, events: Vec::new() };
        p.world.respawn(&mut p.ship, &mut p.system, &mut p.events);
        p.events.clear();
        p
    }

    pub fn sys(&mut self) -> Arc<StarSystem> {
        self.world.system(self.system)
    }

    /// Body positions in the ship's system now.
    pub fn positions(&mut self) -> Vec<DVec3> {
        let mut pos = Vec::new();
        self.sys().positions(self.world.time, &mut pos);
        pos
    }

    /// One frame with the stick centred and no flight computer.
    pub fn step(&mut self, real_dt: f64, warp: f64) -> StepResult {
        let c = ShipCommands { turn: Some(Controls::default()), ..self.ship.holding() };
        self.world.step_ship(&mut self.ship, &mut self.system, &c, &mut Manual, real_dt, warp, &mut self.events)
    }

    pub fn command(&mut self, c: &ShipCommands) {
        self.world.command(&mut self.ship, self.system, c, &mut self.events);
    }

    pub fn set_throttle(&mut self, throttle: f64) {
        self.command(&ShipCommands { throttle, ..self.ship.holding() });
    }

    /// Engage or disengage the hyperdrive (dropping out with the dominant body).
    pub fn toggle_hyperdrive(&mut self) {
        let orders = HyperdriveCommand { engage: !self.ship.hyperdrive, ..Default::default() };
        self.command(&ShipCommands { hyperdrive: Some(orders), ..self.ship.holding() });
    }

    pub fn crashed(&self) -> bool {
        self.events.iter().any(|e| matches!(e, ShipEvent::Crashed { .. }))
    }
}
