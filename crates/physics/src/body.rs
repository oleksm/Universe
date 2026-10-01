//! Rigid bodies: anything that moves freely under gravity and applied forces.

use glam::{DQuat, DVec3};

#[derive(Clone, Copy, Debug)]
pub struct RigidBody {
    /// Relative to the rail system's root (m).
    pub position: DVec3,
    pub velocity: DVec3,
    /// Local -Z is forward, +Y is up.
    pub orientation: DQuat,
    /// Body-frame angular velocity (rad/s).
    pub angular_velocity: DVec3,
    /// Radius of its collision sphere (m): what passes through a ring (and
    /// what touches anything, if it has no `parts`).
    pub radius: f64,
    /// Its solid shape for contact, as spheres fixed in its frame (empty: the
    /// one sphere of `radius` at its centre).
    pub parts: &'static [Sphere],
    /// Ballistic coefficient: mass / (drag coefficient × area) (kg/m²), for
    /// drag in an atmosphere. Zero: no drag.
    pub ballistic: f64,
}

impl RigidBody {
    pub fn new(position: DVec3, velocity: DVec3, orientation: DQuat, radius: f64) -> Self {
        Self { position, velocity, orientation, angular_velocity: DVec3::ZERO, radius, parts: &[], ballistic: 0.0 }
    }
}

/// A sphere of a body's contact shape: where it is in the body's frame (m), and its radius (m).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Sphere {
    pub at: DVec3,
    pub radius: f64,
}
