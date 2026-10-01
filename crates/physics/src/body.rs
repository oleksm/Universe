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
    /// Radius of its collision sphere (m).
    pub radius: f64,
    /// Ballistic coefficient: mass / (drag coefficient × area) (kg/m²), for
    /// drag in an atmosphere. Zero: no drag.
    pub ballistic: f64,
}

impl RigidBody {
    pub fn new(position: DVec3, velocity: DVec3, orientation: DQuat, radius: f64) -> Self {
        Self { position, velocity, orientation, angular_velocity: DVec3::ZERO, radius, ballistic: 0.0 }
    }
}
