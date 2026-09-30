//! Ring gates linking star systems.
//!
//! A gate is a ring, inertially fixed, orbiting a planet. Flying through its
//! opening (not too fast) sends the ship to the paired gate in the linked
//! system, arriving with the same speed and heading relative to the gate.
//! Hitting the ring structure is fatal.

use glam::{DQuat, DVec3};
use universe_physics::{Frame, Ring};

use crate::docking::{attitude, Command, Guidance};
use crate::ship::{facing, Clearance, Phase, Ship};
use crate::system::StarSystem;

/// Radius of the ring's centerline (m); the opening is a little smaller.
pub const GATE_RADIUS: f64 = 1500.0;
/// Half-thickness of the ring structure (m).
pub const RING_TUBE: f64 = 60.0;
/// Faster than this through a gate and the transit fails (m/s).
pub const MAX_TRANSIT_SPEED: f64 = 300.0;
/// Recommended speed through the ring (m/s).
pub const TRANSIT_SPEED: f64 = 100.0;
/// Where the run-in starts, along the axis from the ring (m).
pub const APPROACH_DISTANCE: f64 = 4000.0;
/// How long the transit between gates takes (real seconds).
pub const TRANSIT_TIME: f64 = 5.0;
/// Transit clearance is granted within this range (m).
pub const CLEARANCE_RANGE: f64 = 50_000.0;
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
    fn side(&self, p: DVec3) -> (f64, f64) {
        let h = (p - self.center).dot(self.axis());
        (if h >= 0.0 { 1.0 } else { -1.0 }, h.abs())
    }
}

/// Lined up on the axis, close enough for the run-in.
pub fn in_final_zone(frame: &GateFrame, pos: DVec3) -> bool {
    let (_, h) = frame.side(pos);
    let lateral = ((pos - frame.center) - frame.axis() * (pos - frame.center).dot(frame.axis())).length();
    h < APPROACH_DISTANCE + 500.0 && lateral < GATE_RADIUS * 0.5
}

/// Where to go, relative to the gate. The run-in goes through the ring from
/// whichever side the ship is on.
/// `accel` is the thruster acceleration available (m/s^2).
pub fn guidance(frame: &GateFrame, pos: DVec3, final_run: bool, accel: f64) -> Guidance {
    let axis = frame.axis();
    let r = pos - frame.center;
    let (side, _) = frame.side(pos);
    let lateral = r - axis * r.dot(axis);
    let inward = -axis * side;
    if final_run {
        let correct = (-lateral * 0.2).clamp_length_max(20.0);
        return Guidance {
            desired_velocity: inward * TRANSIT_SPEED + correct,
            waypoint: frame.center + inward * 300.0,
            waypoint_dir: inward,
            final_run: true,
        };
    }
    let approach = frame.center + axis * side * APPROACH_DISTANCE;
    let d = approach - pos;
    let dist = d.length();
    let top = (2.0 * accel * 0.25 * dist).sqrt().min(250.0);
    let desired_velocity = if dist > 1.0 { d / dist * top.min(dist * 0.5) } else { DVec3::ZERO };
    Guidance { desired_velocity, waypoint: approach, waypoint_dir: inward, final_run: false }
}

/// Numbers for the transit HUD.
#[derive(Clone, Copy, Debug)]
pub struct GateStatus {
    pub phase: Phase,
    pub autopilot: bool,
    /// Distance to the ring's center (m).
    pub range: f64,
    /// Distance from the opening's plane (m).
    pub distance: f64,
    /// Distance from the axis (m).
    pub offset: f64,
    /// Speed toward the opening (m/s).
    pub closing: f64,
    pub speed: f64,
    pub relative_velocity: DVec3,
    /// Lined up to pass through the opening.
    pub in_corridor: bool,
    pub guidance: Guidance,
}

pub fn status(frame: &GateFrame, ship: &Ship, clearance: &Clearance) -> GateStatus {
    let axis = frame.axis();
    let r = ship.position - frame.center;
    let (side, h) = frame.side(ship.position);
    let v = ship.velocity - frame.velocity;
    let offset = (r - axis * r.dot(axis)).length();
    let final_run = if clearance.autopilot { clearance.phase == Phase::Final } else { in_final_zone(frame, ship.position) };
    GateStatus {
        phase: clearance.phase,
        autopilot: clearance.autopilot,
        range: r.length(),
        distance: h,
        offset,
        closing: -v.dot(axis * side),
        speed: v.length(),
        relative_velocity: v,
        in_corridor: offset < GATE_RADIUS - RING_TUBE - 200.0,
        guidance: guidance(frame, ship.position, final_run, ship.side_accel()),
    }
}

/// The transit autopilot: thrusters only, nose pointed along the run.
pub fn autopilot(frame: &GateFrame, ship: &Ship, phase: Phase) -> Command {
    let axis = frame.axis();
    let (side, _) = frame.side(ship.position);
    let inward = -axis * side;
    let v = ship.velocity - frame.velocity;
    let to_approach = (frame.center + axis * side * APPROACH_DISTANCE).distance(ship.position);
    let aligned = ship.forward().dot(inward) > 0.995;
    let phase = match phase {
        Phase::Approach if to_approach < 100.0 && v.length() < 8.0 => Phase::Align,
        Phase::Align if aligned => Phase::Final,
        Phase::Final if !in_final_zone(frame, ship.position) => Phase::Approach,
        p @ (Phase::Approach | Phase::Align | Phase::Final) => p,
        _ => Phase::Approach,
    };
    let g = guidance(frame, ship.position, phase == Phase::Final, ship.side_accel());
    let accel = ((g.desired_velocity - v) * 1.2).clamp_length_max(ship.side_accel());
    let rcs = ship.thruster_command(ship.orientation.inverse() * accel);
    // Nose: at the gate while travelling, then along the run-in (roll kept level
    // with the ring's own frame).
    let target = match phase {
        Phase::Approach => facing(frame.center - ship.position, frame.rotation * DVec3::Z),
        _ => facing(inward, frame.rotation * DVec3::Z),
    };
    Command { controls: attitude(ship, target, DVec3::ZERO), throttle: 0.0, rcs, phase, attitude: target }
}
