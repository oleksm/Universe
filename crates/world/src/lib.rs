//! The world: what exists and the rules it runs by. Content (a seeded galaxy,
//! star systems on Keplerian rails, terrain, the gate network), structures
//! (stations, spaceports, gates), ships as rigid bodies with devices, the
//! rules for what touching things means (docking port, landing gear, gate
//! device, damage), and world services (traffic control).
//!
//! Built on the physics kernel (`universe-physics`). Ships are flown only
//! through `ShipCommands`: the world never looks at where a pilot or a flight
//! computer wants to go, only at what it commands the devices to do.
//! Deterministic and headless; knows nothing about rendering.

pub mod damage;
pub mod events;
pub mod galaxy;
pub mod gate;
pub mod hyperdrive;
pub mod names;
pub mod network;
pub mod rng;
pub mod ship;
pub mod spaceport;
pub mod station;
pub mod system;
pub mod terrain;
#[cfg(test)]
mod testkit;
pub mod traffic;
pub mod units;
mod world;

pub use events::{ClearanceKind, ShipEvent, TrafficEvent};
pub use galaxy::{Galaxy, GalaxyStar, StarClass};
pub use gate::GateFrame;
pub use ship::{Controls, Destination, HyperdriveCommand, Ship, ShipCommands, ShipState};
pub use station::StationFrame;
pub use system::{Body, BodyKind, Spaceport, StarSystem};
pub use terrain::{Ground, Terrain, TerrainKind};
pub use traffic::Facility;
pub use universe_physics as physics;
pub use world::{FlightComputer, Manual, StepResult, World, NEIGHBOURS};
