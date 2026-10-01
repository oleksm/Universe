//! Universe orchestration: the world (`universe-world`) with the player's
//! ship and the settlers' crafts in it, each with its avionics
//! (`universe-avionics`: the navigation computer, guidance, autopilots and
//! flight planner), which fly the ships through `ShipCommands`. Every tick,
//! each ship takes its turn in a fixed order (`vessel`); traffic (the
//! settlers, their totals and crash log) and save/load live here too.
//! Deterministic and headless; knows nothing about rendering. (The physics
//! itself is the kernel, `universe-physics`.)

mod combat;
mod commerce;
mod contacts;
pub mod engine;
pub mod audit;
pub mod cockpit;
pub mod operator;
pub mod pilots;
mod follow;
pub mod recorder;
mod save;
mod traffic;
pub mod universe;
mod vessel;

pub use universe_avionics as avionics;
pub use universe_avionics::{docking, gate, landing, plan, route};
pub use universe_avionics::{
    Action, Approach, Avionics, Clearance, DockingStatus, Event, GateStatus, Guidance, LandingStatus, NavTarget, PadFrame, Phase, Plan, PlanPoint, Route,
    Stop,
};
pub use universe_physics as physics;
pub use universe_protocol as protocol;
pub use universe_services as services;
pub use universe_physics::Orbit;
pub use universe_world as world;
pub use universe_world::{galaxy, names, rng, ship, system, terrain, units};
pub use universe_world::{
    Body, BodyKind, ClearanceKind, Controls, Galaxy, GalaxyStar, GateFrame, Ground, Ship, ShipCommands, ShipEvent, ShipState, Spaceport,
    StarClass, StarSystem, StationFrame, StepResult, Terrain, TerrainKind, TrafficEvent,
};
pub use combat::{craft_id, PLAYER};
pub use universe_services::records::{Deal, Kill, TradeRecord, TrafficStats};
pub use contacts::{Contact, LOCK_BEAM};
pub use follow::FollowKind;
pub use save::UniverseSave;
pub use traffic::{CrashReport, Craft};
pub use engine::{Command, CraftView, EngineHandle, View};
pub use universe::Universe;
