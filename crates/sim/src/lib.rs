//! Universe orchestration: the world (`universe-world`) with the player's
//! ship and the settlers' crafts in it, each with its avionics — the
//! navigation computer, guidance, autopilots and flight planner — which fly
//! the ships through `ShipCommands`. Deterministic and headless; knows
//! nothing about rendering. (The physics itself is the kernel,
//! `universe-physics`; the avionics move to their own crate later.)

pub mod avionics;
mod computer;
pub mod docking;
pub mod gate;
pub mod landing;
pub mod plan;
pub mod route;
pub mod universe;

pub use avionics::{Avionics, Clearance, NavTarget, Phase};
pub use docking::{DockingStatus, Guidance};
pub use landing::{LandingStatus, PadFrame};
pub use gate::GateStatus;
pub use universe_physics as physics;
pub use universe_physics::Orbit;
pub use universe_world as world;
pub use universe_world::{galaxy, names, rng, ship, system, terrain, units};
pub use universe_world::{
    Body, BodyKind, ClearanceKind, Controls, Galaxy, GalaxyStar, GateFrame, Ground, Ship, ShipCommands, ShipEvent, ShipState, Spaceport,
    StarClass, StarSystem, StationFrame, StepResult, Terrain, TerrainKind, TrafficEvent,
};
pub use plan::{Action, Plan, PlanPoint};
pub use route::{Route, Stop};
pub use universe::{Approach, CrashReport, Craft, Event, TrafficStats, Universe, UniverseSave};
