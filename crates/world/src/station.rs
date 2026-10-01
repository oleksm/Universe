//! Stations: a Coriolis station's shape, its frame, and its docking port.
//!
//! A Coriolis station is a cuboctahedron spinning about its local +Y axis. The
//! docking slot sits in the +Y face, so the approach corridor runs along the
//! spin axis: it never moves sideways, but the slot rolls with the station and
//! a ship has to roll with it.

use glam::{DMat3, DQuat, DVec3};
use universe_physics::{CutOut, Frame, Polytope};

use crate::system::StarSystem;

/// Distance from the station center to its square faces (m). Also the render scale.
pub const STATION_SIZE: f64 = 500.0;
/// Slot half-extents in station units: long axis (local X) and short axis (local Z).
pub const SLOT_HALF: (f64, f64) = (0.3, 0.08);
/// Depth into the slot (station units from center) at which a ship counts as docked.
const DOCKED_DEPTH: f64 = 0.9;
/// Where a docked ship rests: in the slot, this far out along the axis (station units).
pub const DOCKED_HEIGHT: f64 = 0.85;
/// Fastest safe speed through the slot (m/s).
pub const MAX_DOCK_SPEED: f64 = 25.0;
/// Largest roll mismatch with the slot that still fits (radians).
pub const MAX_ROLL_ERROR: f64 = 0.52; // 30 degrees
/// Hull contact slower than this bounces instead of destroying the ship (m/s).
pub const BUMP_SPEED: f64 = 15.0;

/// A station's pose and motion at one instant, in the system frame.
#[derive(Clone, Copy, Debug)]
pub struct StationFrame {
    pub center: DVec3,
    pub velocity: DVec3,
    pub rotation: DQuat,
    pub angular_velocity: DVec3,
}

impl StationFrame {
    pub fn new(sys: &StarSystem, station: usize, t: f64, positions: &[DVec3]) -> Self {
        let f = Frame::of(&sys.bodies, station, t, positions);
        Self { center: f.center, velocity: f.velocity, rotation: f.rotation, angular_velocity: f.angular_velocity }
    }

    /// Docking axis: out of the slot, along the spin axis.
    pub fn axis(&self) -> DVec3 {
        self.rotation * DVec3::Y
    }

    /// The slot's long direction.
    pub fn slot_long(&self) -> DVec3 {
        self.rotation * DVec3::X
    }

    pub fn slot_short(&self) -> DVec3 {
        self.rotation * DVec3::Z
    }

    /// Point `height` meters out along the axis.
    pub fn on_axis(&self, height: f64) -> DVec3 {
        self.center + self.axis() * height
    }

    /// Velocity of the station's material at world point `p` (includes spin).
    pub fn velocity_at(&self, p: DVec3) -> DVec3 {
        self.velocity + self.angular_velocity.cross(p - self.center)
    }

    /// Ship orientation that points into the slot with wings along the slot,
    /// choosing whichever of the two fitting rolls is closer to `current`.
    pub fn docking_orientation(&self, current: DQuat) -> DQuat {
        let forward = -self.axis();
        let mut right = self.slot_long();
        if (current * DVec3::X).dot(right) < 0.0 {
            right = -right;
        }
        let up = right.cross(forward);
        DQuat::from_mat3(&DMat3::from_cols(right, up, -forward))
    }

    /// Angle between the ship's wings and the slot's long axis, 0..90 degrees (radians).
    pub fn roll_error(&self, orientation: DQuat) -> f64 {
        let axis = self.axis();
        let right = orientation * DVec3::X;
        let flat = right - axis * right.dot(axis);
        match flat.try_normalize() {
            Some(flat) => flat.dot(self.slot_long()).abs().clamp(0.0, 1.0).acos(),
            None => std::f64::consts::FRAC_PI_2,
        }
    }
}

/// The station's real shape, for the physics kernel: a cuboctahedron (the
/// intersection of a cube, |x|,|y|,|z| <= 1, and an octahedron,
/// |x|+|y|+|z| <= 2), with the slot cut into its +Y face. Reaching the
/// slot's floor is the docking port's contact; the mouth above it is open.
pub fn hull() -> Polytope {
    Polytope::cuboctahedron(STATION_SIZE).with_cut_out(CutOut { half_x: SLOT_HALF.0, half_z: SLOT_HALF.1, floor: DOCKED_DEPTH })
}

#[cfg(test)]
mod tests {
    use super::*;
    
    use crate::ship::ShipState;
    use crate::testkit::Probe;

    #[test]
    fn launch_from_docked_leaves_along_axis() {
        let mut p = Probe::new(42);
        let station = p.sys().station().unwrap();
        p.ship.state = ShipState::Landed { body: station, local_position: DVec3::Y * 425.0, local_orientation: DQuat::IDENTITY };
        p.set_throttle(0.2);
        p.step(1.0 / 60.0, 1.0);
        assert!(p.ship.is_flying());
        for _ in 0..300 {
            p.step(1.0 / 60.0, 1.0);
        }
        assert!(p.ship.is_flying(), "launch should not hit the station: {:?}", p.events);
    }
}
