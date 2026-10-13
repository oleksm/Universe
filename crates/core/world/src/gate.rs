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
/// A ring's size, as its records have it (every class alike): its
/// centreline's radius (half `size.opening`), and its structure's half
/// thickness (half `size.thickness`), m.
fn size() -> (f64, f64) {
    static SIZE: std::sync::OnceLock<(f64, f64)> = std::sync::OnceLock::new();
    *SIZE.get_or_init(|| {
        let g = crate::registry::registry().gates.first().expect("the registry has a gate ring");
        (g.size.opening.expect("a ring's opening") / 2.0, g.size.thickness.expect("a ring's thickness") / 2.0)
    })
}

/// What crossing a tube `span` m long costs a ship of `mass` kg at its
/// natural pace (credits): the energy it takes (Dogma's Tube law), at the
/// power price, paid to the gate's administration.
pub fn crossing_toll(mass: f64, span: f64) -> f64 {
    let energy = universe_physics::hyper::tube_crossing_energy(mass, span, universe_physics::hyper::tube_natural_time(mass, span));
    energy / 3.6e9 * crate::goods::POWER_PRICE
}

/// A ring's centreline radius (m).
pub fn gate_radius() -> f64 {
    size().0
}

/// Half a ring's structure's thickness (m).
pub fn ring_tube() -> f64 {
    size().1
}

/// The ring's shape, for Dogma; its opening is the trigger.
pub fn ring() -> Ring {
    Ring { radius: gate_radius(), tube: ring_tube() }
}

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
    let clear = ring_tube() + SHIP_RADIUS + 50.0;
    let arrival = Relative { position: local_offset + DVec3::Y * out * clear, velocity: local_velocity, orientation: local_orientation };
    arrival.place(&frame.frame(), rigid);
    rigid.angular_velocity = DVec3::ZERO;
}

/// The settled systems: those on the gate network (`links`), in order, each once.
pub fn settled(links: &[(usize, usize)]) -> Vec<usize> {
    let mut s: Vec<usize> = links.iter().flat_map(|&(a, b)| [a, b]).collect();
    s.sort_unstable();
    s.dedup();
    s
}
