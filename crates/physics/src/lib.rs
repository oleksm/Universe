//! The physics kernel: bodies on Kepler rails, rigid bodies moved by gravity and
//! applied forces, colliders that report facts, and a few explicit operations.
//!
//! It knows nothing about what the bodies are, who owns them or what flies them:
//! the same integrator, substeps and contact tests for everything. Whatever
//! pushes a body does so through the applied acceleration a `Driver` returns;
//! whatever a contact *means* is decided by the caller from the `Fact`.
//! Deterministic: no clocks, no randomness, a fixed order of evaluation.

pub mod body;
pub mod collide;
pub mod integrate;
pub mod ops;
pub mod orbit;
pub mod query;
pub mod rails;
pub mod surface;
#[cfg(test)]
mod testkit;

pub use body::RigidBody;
pub use collide::{Collider, Contact, CutOut, Fact, Feature, Polytope, Ring, RingCrossing};
pub use integrate::{integrate, leapfrog, Driver, Outcome, Response, Span};
pub use ops::{bounce, relocate, Relative, Weld};
pub use orbit::Orbit;
pub use query::{dominant, gravity, pull, segment_distance, simulate};
pub use rails::{positions, velocity, Ephemeris, Frame, OnRails, RailBody};
pub use surface::{max_radius, surface_radius, surface_radius_at, Surface};
