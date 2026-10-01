//! The pilot interface: everything a pilot (the avionics) has to do with the
//! world goes through it — see `docs/rearchitecture.md`.
//!
//! - **Sensors**: its own ship's instruments, its star system (the charts, and
//!   where the bodies are), the time, the defence turrets it can see.
//! - **The feed**: the physical events since it last looked.
//! - **Actuation**: device settings (`ShipCommands`), nothing returned.
//! - **Traffic control**: requests and their replies (clearance, a pad, a
//!   corridor). Traffic control decides; the pilot only asks.
//!
//! Whoever runs the ship (the orchestration) provides the bus. A pilot never
//! sees the world itself.

use std::sync::Arc;

use glam::DVec3;
use universe_protocol::PadGrant;
use crate::nav::NavTarget;
use universe_world::{Ship, ShipCommands, ShipEvent, StarSystem};

pub trait Bus {
    /// The ship itself, as its sensors report it.
    fn ship(&self) -> &Ship;

    /// Galaxy index of the star system the ship is in.
    fn system(&self) -> usize;

    /// The star system the ship is in (its charts).
    fn star_system(&mut self) -> Arc<StarSystem>;

    /// World time now (s).
    fn time(&self) -> f64;

    /// Gate links between star systems (galaxy indices).
    fn gate_links(&self) -> &[(usize, usize)];

    /// The ship's id, as traffic control knows it.
    fn id(&self) -> usize;

    /// Ask traffic control for a pad at spaceport `port` in the ship's system.
    fn request_pad(&mut self, port: universe_world::traffic::Facility) -> PadGrant;

    /// Ask traffic control for the corridor of station or gate `body` (one
    /// ship at a time on its final run or launching): `None` if granted,
    /// else how many are ahead in line.
    fn request_corridor(&mut self, body: usize) -> Option<usize>;

    /// The defence turrets of the ship's system (charted): where they are and how they move.
    /// (Where each is, how it moves, and how far it reaches.)
    fn turrets(&mut self) -> Vec<(DVec3, DVec3, f64)>;

    /// Set the devices (they hold the settings until changed).
    fn actuate(&mut self, c: &ShipCommands);

    /// The physical events since the feed was last read.
    fn feed(&mut self) -> Vec<ShipEvent>;

    /// Ask traffic control for clearance to use `target` (None: the nearest
    /// station): granted (for what), or refused with the reason.
    fn request_clearance(&mut self, target: Option<NavTarget>) -> Result<NavTarget, String>;

    /// Does traffic control still stand by a clearance for `target`?
    fn clearance_holds(&mut self, target: NavTarget) -> bool;

    /// Where the ship's star system's bodies are now.
    fn positions(&mut self) -> (Arc<StarSystem>, Vec<DVec3>) {
        let sys = self.star_system();
        let mut positions = Vec::with_capacity(sys.bodies.len());
        sys.positions(self.time(), &mut positions);
        (sys, positions)
    }
}
