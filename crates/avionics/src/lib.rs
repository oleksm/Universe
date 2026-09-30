//! Ship software (`universe-avionics`): the navigation computer (nav target,
//! clearance requests), guidance for docking, landing and gate runs, the
//! autopilots (dock, land, gate, hyperdrive, route) and the flight planner.
//!
//! Avionics are programs running on a ship. They read what its sensors
//! would — the ship, and the world around it, read-only — and write
//! `ShipCommands` to its devices, nothing else (see `Bus` and the world's
//! `FlightComputer`). They can't cheat: an autopilot is exactly as limited as
//! a pilot. The flight planner predicts a flight by simulating a copy of the
//! ship through the physics kernel under the same autopilot.
//! Deterministic and headless.

pub mod avionics;
pub mod bus;
pub mod computer;
pub mod docking;
pub mod events;
pub mod gate;
pub mod hyperdrive;
pub mod landing;
pub mod nav;
pub mod plan;
pub mod route;

pub use avionics::{Approach, Avionics};
pub use bus::Bus;
pub use computer::Computer;
pub use docking::{Command, DockingStatus, Guidance};
pub use events::Event;
pub use gate::GateStatus;
pub use landing::{LandingStatus, PadFrame};
pub use nav::{Clearance, NavTarget, Phase};
pub use plan::{Action, Plan, PlanPoint};
pub use route::{Route, Stop};
