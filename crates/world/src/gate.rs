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
// (Its constants are the physics sheet's: config/dogma.ron.)
pub use crate::sheet::{GATE_RADIUS, RING_TUBE};
pub use universe_physics::laws::MAX_TRANSIT_SPEED;

/// The ring's shape, for Dogma; its opening is the trigger.
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

    /// As a Dogma frame (a gate doesn't spin), e.g. to relocate a ship between gates.
    pub fn frame(&self) -> Frame {
        Frame { center: self.center, velocity: self.velocity, rotation: self.rotation, angular_velocity: DVec3::ZERO }
    }

    /// The ring's axis (normal to the opening): the way through it, toward
    /// the star it leads to. One way only: crossing against it is just
    /// flying through an empty ring.
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
/// kept from the other gate, just clear of the ring on the far side. The
/// twins face each other, so what went in along one's axis comes out along
/// the other's the opposite way (turned half round about its X): it keeps
/// on through space the way it was going, out on the twin's entry side.
pub fn emerge(frame: &GateFrame, local_velocity: DVec3, local_offset: DVec3, local_orientation: DQuat, rigid: &mut RigidBody) {
    let half = DQuat::from_rotation_x(std::f64::consts::PI);
    let (local_velocity, local_offset, local_orientation) = (half * local_velocity, half * local_offset, half * local_orientation);
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
            // (From its entry side, along its axis: the one way through.)
            p.ship.position = frame.center + frame.rotation * DVec3::new(offset, 0.0, 0.0) - axis * 500.0;
            p.ship.velocity = frame.velocity + axis * speed;
            for _ in 0..600 {
                p.step(1.0 / 60.0, 1.0);
            }
            assert!(p.crashed(), "{label}: expected a crash; events {:?}", p.events);
        }
    }

    #[test]
    fn a_gate_is_one_way_and_faces_its_twin() {
        let mut p = Probe::new(1984);
        let home = p.world.home_system;
        let sys = p.sys();
        let (dest, _) = p.world.gate_links_of(home)[0].clone();
        let g = sys.gate_to(dest).unwrap();
        let pos = p.positions();
        let frame = GateFrame::new(&sys, g, p.world.time, &pos);
        let axis = frame.axis();
        // It faces the star it leads to, and its twin faces back.
        let toward = p.world.galaxy.offset(home, dest).normalize();
        assert!(axis.dot(toward) > 0.999, "{axis} vs {toward}");
        let there = p.world.system(dest);
        let twin = there.gate_to(home).unwrap();
        assert!((there.bodies[twin].rotation(0.0) * DVec3::Y).dot(-toward) > 0.999, "the twin faces back");
        // Against its axis: through an empty ring, still here.
        p.ship.position = frame.center + axis * 300.0;
        p.ship.velocity = frame.velocity - axis * 60.0;
        for _ in 0..600 {
            p.step(1.0 / 60.0, 1.0);
        }
        assert!(p.ship.is_flying() && !p.crashed(), "{:?}", p.events);
        assert!(p.events.iter().any(|e| matches!(e, crate::events::ShipEvent::RuleFired { outcome, .. } if outcome.contains("wrong way"))));
    }
}

#[cfg(test)]
mod clear {
    use super::*;

    #[test]
    fn nothing_large_comes_near_a_gates_path() {
        let w = crate::World::new(1984);
        let mut systems: Vec<usize> = w.gate_links.iter().flat_map(|&(a, b)| [a, b]).collect();
        systems.sort_unstable();
        systems.dedup();
        let mut checked = 0;
        for &s in systems.iter().take(40) {
            let sys = w.system(s);
            let mut pos = Vec::new();
            for g in (0..sys.bodies.len()).filter(|&i| sys.bodies[i].kind == crate::system::BodyKind::Gate) {
                let period = sys.bodies[g].rail.orbit.as_ref().map_or(1.0e6, |o| o.period());
                for k in 0..16 {
                    let t = period * k as f64 / 16.0;
                    sys.positions(t, &mut pos);
                    let f = GateFrame::new(&sys, g, t, &pos);
                    // The run-in and the way out, 8 km either side.
                    for step in -8..=8 {
                        let p = f.center + f.axis() * (step as f64 * 1000.0);
                        for (i, b) in sys.bodies.iter().enumerate() {
                            if i == g {
                                continue;
                            }
                            let field = sys.fields.iter().find(|fl| fl.body == i).map_or(0.0, |fl| fl.extent);
                            let gap = pos[i].distance(p) - b.max_radius() - field;
                            assert!(gap > 50_000.0, "system {s}, {}: {} within {:.0} km of its path", sys.bodies[g].name, b.name, gap / 1000.0);
                        }
                    }
                    checked += 1;
                }
            }
        }
        eprintln!("{checked} gate positions checked in {} systems", systems.len().min(40));
        assert!(checked > 100, "{checked}");
    }
}
