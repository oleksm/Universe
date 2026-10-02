//! Dogma: bodies on Kepler rails, rigid bodies moved by gravity and
//! applied forces, colliders that report facts, and a few explicit operations.
//!
//! It knows nothing about what the bodies are, who owns them or what flies them:
//! the same integrator, substeps and contact tests for everything. Whatever
//! pushes a body does so through the applied acceleration a `Driver` returns;
//! whatever a contact *means* is decided by the caller from the `Fact`.
//! Deterministic: no clocks, no randomness, a fixed order of evaluation.

pub mod atmosphere;
pub mod body;
pub mod collide;
pub mod hyper;
pub mod integrate;
pub mod laws;
pub mod mesh;
pub mod ops;
pub mod orbit;
pub mod pairs;
pub mod projectile;
pub mod query;
pub mod rails;
pub mod sheet;
pub mod surface;
#[cfg(test)]
mod testkit;

pub use atmosphere::{air_at, drag, heat_flux, Atmosphere};
pub use body::{RigidBody, Sphere};
pub use collide::{Blocks, Collider, Contact, Fact, Feature, Ring, RingCrossing};
pub use mesh::{MassProperties, Mesh};
pub use integrate::{integrate, leapfrog, Driver, Outcome, Response, Span};
pub use ops::{bounce, relocate, Relative, Weld};
pub use orbit::Orbit;
pub use pairs::{bounce_pair, contacts, Mover, PairContact};
pub use projectile::{intercept, ray, step_projectile, Hit, Projectile, Target};
pub use query::{dominant, gravity, pull, segment_distance, simulate};
pub use rails::{position, positions, settle, velocity, Ephemeris, Frame, OnRails, RailBody};
pub use surface::{max_radius, surface_radius, surface_radius_at, Surface};
