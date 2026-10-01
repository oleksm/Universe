//! Ring gates linking star systems, and the gate device that uses them.
//!
//! A gate is a ring, inertially fixed, orbiting a planet. Flying through its
//! opening (not too fast) sends the ship to the paired gate in the linked
//! system, arriving with the same speed and heading relative to the gate.
//! Hitting the ring structure is fatal.

use glam::{DQuat, DVec3};
use universe_physics::{Frame, Relative, RigidBody, Ring};

use crate::ship::SHIP_RADIUS;
use crate::system::StarSystem;

/// Radius of the ring's centerline (m); the opening is a little smaller.
pub const GATE_RADIUS: f64 = 1500.0;
/// Half-thickness of the ring structure (m).
pub const RING_TUBE: f64 = 60.0;
/// Faster than this through a gate and the transit fails (m/s).
pub const MAX_TRANSIT_SPEED: f64 = 300.0;
/// How long the transit between gates takes (real seconds).
pub const TRANSIT_TIME: f64 = 10.0;
/// The ring's shape, for the physics kernel; its opening is the trigger.
pub const RING: Ring = Ring { radius: GATE_RADIUS, tube: RING_TUBE };

/// A gate's pose and motion at one instant, in the system frame.
#[derive(Clone, Copy, Debug)]
pub struct GateFrame {
    pub center: DVec3,
    pub velocity: DVec3,
    pub rotation: DQuat,
}

impl GateFrame {
    pub fn new(sys: &StarSystem, gate: usize, t: f64, positions: &[DVec3]) -> Self {
        Self { center: positions[gate], velocity: sys.velocity(gate, t), rotation: sys.bodies[gate].rotation(t) }
    }

    /// As a kernel frame (a gate doesn't spin), e.g. to relocate a ship between gates.
    pub fn frame(&self) -> Frame {
        Frame { center: self.center, velocity: self.velocity, rotation: self.rotation, angular_velocity: DVec3::ZERO }
    }

    /// The ring's axis (normal to the opening).
    pub fn axis(&self) -> DVec3 {
        self.rotation * DVec3::Y
    }

    pub fn local(&self, p: DVec3) -> DVec3 {
        self.rotation.inverse() * (p - self.center)
    }

    /// Which side of the ring `p` is on (+1 or -1), and distance from the opening's plane.
    pub fn side(&self, p: DVec3) -> (f64, f64) {
        let h = (p - self.center).dot(self.axis());
        (if h >= 0.0 { 1.0 } else { -1.0 }, h.abs())
    }
}

/// Out of the paired gate at `frame`: relocated to its frame with the motion
/// kept from the other gate, just clear of the ring on the far side.
pub fn emerge(frame: &GateFrame, local_velocity: DVec3, local_offset: DVec3, local_orientation: DQuat, rigid: &mut RigidBody) {
    let out = if local_velocity.y >= 0.0 { 1.0 } else { -1.0 };
    let clear = RING_TUBE + SHIP_RADIUS + 50.0;
    let arrival = Relative { position: local_offset + DVec3::Y * out * clear, velocity: local_velocity, orientation: local_orientation };
    arrival.place(&frame.frame(), rigid);
    rigid.angular_velocity = DVec3::ZERO;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testkit::Probe;

    #[test]
    fn clipping_the_ring_or_going_too_fast_is_fatal() {
        for (label, offset, speed) in [("ring", GATE_RADIUS, 50.0), ("fast", 0.0, 500.0)] {
            let mut p = Probe::new(1984);
            let home = p.world.home_system;
            let sys = p.sys();
            let (dest, _) = p.world.gate_links_of(home)[0].clone();
            let g = sys.gate_to(dest).unwrap();
            let pos = p.positions();
            let frame = GateFrame::new(&sys, g, p.world.time, &pos);
            let axis = frame.axis();
            p.ship.position = frame.center + frame.rotation * DVec3::new(offset, 0.0, 0.0) + axis * 500.0;
            p.ship.velocity = frame.velocity - axis * speed;
            for _ in 0..600 {
                p.step(1.0 / 60.0, 1.0);
            }
            assert!(p.crashed(), "{label}: expected a crash; events {:?}", p.events);
        }
    }
}
