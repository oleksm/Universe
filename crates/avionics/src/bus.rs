//! The avionics' connection to their ship: sensors in, device commands out.
//!
//! Whoever runs the ship (the orchestration) provides the bus. Through it the
//! avionics see the ship and the world around it, read-only, and send
//! `ShipCommands` to the devices; nothing else. What the devices then did is
//! reported back as the world's physical events.

use std::rc::Rc;

use glam::DVec3;
use universe_world::{Ship, ShipCommands, ShipEvent, StarSystem};

pub trait Bus {
    /// The ship itself, as its sensors report it.
    fn ship(&self) -> &Ship;

    /// Galaxy index of the star system the ship is in.
    fn system(&self) -> usize;

    /// The star system the ship is in (its charts).
    fn star_system(&mut self) -> Rc<StarSystem>;

    /// World time now (s).
    fn time(&self) -> f64;

    /// Gate links between star systems (galaxy indices).
    fn gate_links(&self) -> &[(usize, usize)];

    /// Give the devices new commands now; the physical events that followed.
    fn command(&mut self, c: &ShipCommands) -> Vec<ShipEvent>;

    /// Where the ship's star system's bodies are now.
    fn positions(&mut self) -> (Rc<StarSystem>, Vec<DVec3>) {
        let sys = self.star_system();
        let mut positions = Vec::with_capacity(sys.bodies.len());
        sys.positions(self.time(), &mut positions);
        (sys, positions)
    }
}
