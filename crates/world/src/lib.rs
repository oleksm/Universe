//! The world: what exists and the rules it runs by. Content (a seeded galaxy,
//! star systems on Keplerian rails, terrain, the gate network), structures
//! (stations, spaceports, gates), ships as rigid bodies with devices, the
//! rules for what touching things means (docking port, landing gear, gate
//! device, damage), and world services (traffic control).
//!
//! Built on Dogma (`universe-physics`). Ships are flown only
//! through `ShipCommands`: the world never looks at where a pilot or a flight
//! computer wants to go, only at what it commands the devices to do.
//! Deterministic and headless; knows nothing about rendering.

pub mod belt;
pub mod charts;
pub mod climate;
pub mod collisions;
pub mod conditions;
pub mod content;
pub mod crew;
pub mod deckplan;
pub mod damage;
pub mod design;
pub mod events;
pub mod galaxy;
pub mod goods;
pub mod gate;
pub mod heat;
pub mod hyperdrive;
pub mod hypernet;
pub mod import;
pub mod materials;
pub mod mining;
pub mod missiles;
pub mod modules;
pub mod names;
pub mod radar;
pub mod network;
pub mod port;
pub mod rng;
pub mod shape;
pub mod sheet;
pub mod small_bodies;
pub mod ship;
pub mod celestial;
pub mod settlements;
pub mod spaceport;
pub mod standards;
pub mod thrusters;
pub mod registry;
pub mod rules;
pub mod station;
pub mod structures;
pub mod structures_catalogue;
pub mod system;
pub mod terrain;
#[cfg(test)]
mod testkit;
pub mod traffic;
pub mod trim;
pub mod turrets;
pub mod walk;
pub mod units;
pub mod weapons;
mod world;

pub use crew::{CrewEvent, Person, Place, WalkCommands};
pub use events::{ClearanceKind, ShipEvent, TrafficEvent};
pub use galaxy::{Galaxy, GalaxyStar, StarClass};
pub use radar::{Blip, RADAR_RANGE};
pub use gate::GateFrame;
pub use ship::{Controls, Destination, HyperdriveCommand, Ship, ShipCommands, ShipState, Triggers};
pub use weapons::{Armed, Beam, Impact, Slug};
pub use station::StationFrame;
pub use system::{Body, BodyKind, Spaceport, StarSystem};
pub use terrain::{Ground, Terrain, TerrainKind};
pub use traffic::Facility;
pub use universe_physics as physics;
pub use world::{Devices, StepResult, World, NEIGHBOURS};
