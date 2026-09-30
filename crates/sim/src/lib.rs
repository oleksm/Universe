//! Universe simulation core: a seeded galaxy, star systems on Keplerian rails,
//! and a Newtonian ship. Deterministic and headless; knows nothing about rendering.
//! The physics itself (rails, integration, contacts) is the kernel, `universe-physics`.

pub mod docking;
pub mod galaxy;
pub mod gate;
pub mod landing;
pub mod names;
pub mod plan;
pub mod rng;
pub mod route;
pub mod ship;
pub mod system;
pub mod terrain;
pub mod units;
pub mod universe;

pub use docking::{DockingStatus, Guidance, StationFrame};
pub use landing::{LandingStatus, PadFrame};
pub use galaxy::{Galaxy, GalaxyStar, StarClass};
pub use gate::{GateFrame, GateStatus};
pub use universe_physics as physics;
pub use universe_physics::Orbit;
pub use plan::{Action, Plan, PlanPoint};
pub use route::{Route, Stop};
pub use ship::{Clearance, Controls, NavTarget, Phase, Ship, ShipState};
pub use system::{Body, BodyKind, Spaceport, StarSystem};
pub use terrain::{Ground, Terrain, TerrainKind};
pub use universe::{Approach, ClearanceKind, CrashReport, Craft, Event, StepResult, TrafficStats, Universe, UniverseSave};
