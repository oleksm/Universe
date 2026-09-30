//! Universe simulation core: a seeded galaxy, star systems on Keplerian rails,
//! and a Newtonian ship. Deterministic and headless; knows nothing about rendering.

pub mod docking;
pub mod galaxy;
pub mod gate;
pub mod landing;
pub mod names;
pub mod orbit;
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
pub use orbit::Orbit;
pub use plan::{Action, Plan, PlanPoint};
pub use route::{Route, Stop};
pub use ship::{Clearance, Controls, NavTarget, Phase, Ship, ShipState};
pub use system::{Body, BodyKind, Spaceport, StarSystem};
pub use terrain::{Surface, Terrain, TerrainKind};
pub use universe::{Approach, ClearanceKind, CrashReport, Craft, Event, StepResult, TrafficStats, Universe, UniverseSave};
