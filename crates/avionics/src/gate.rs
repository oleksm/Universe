//! Gate run guidance and the transit autopilot. (The gate itself, its frame
//! and the gate device are the world's: `universe_world::gate`.)
//!
//! The run-in goes along the ring's axis — the one way through, toward the
//! star it leads to — through the opening, at a steady speed well under the
//! gate's limit. From the far side, the way round is back through the
//! empty ring (against its axis it takes no one anywhere).

use glam::DVec3;
use universe_world::gate::{GATE_RADIUS, RING_TUBE};
use universe_world::ship::facing;
use universe_world::{GateFrame, Ship};

use crate::docking::{attitude, gain, Command, Guidance};
use crate::nav::{Clearance, Phase};

/// Recommended speed through the ring (m/s).
pub const TRANSIT_SPEED: f64 = 100.0;
/// Where the run-in starts, along the axis from the ring (m).
pub const APPROACH_DISTANCE: f64 = 4000.0;

/// Lined up on the axis on its entry side, close enough for the run-in.
pub fn in_final_zone(frame: &GateFrame, pos: DVec3) -> bool {
    let (side, h) = frame.side(pos);
    let lateral = ((pos - frame.center) - frame.axis() * (pos - frame.center).dot(frame.axis())).length();
    side < 0.0 && h < APPROACH_DISTANCE + 500.0 && lateral < GATE_RADIUS * 0.5
}

/// The side a run-in starts from: behind the ring, going along its axis.
const ENTRY: f64 = -1.0;

/// Where to go, relative to the gate. The run-in goes through the ring from
/// its entry side; from the far side, first back through the empty ring
/// along the axis (not round the ring's tube).
/// `accel` is the thruster acceleration available (m/s^2).
pub fn guidance(frame: &GateFrame, pos: DVec3, final_run: bool, accel: f64) -> Guidance {
    let axis = frame.axis();
    let r = pos - frame.center;
    let (side, h) = frame.side(pos);
    let lateral = r - axis * r.dot(axis);
    let inward = axis;
    if final_run {
        let correct = (-lateral * 0.2).clamp_length_max(20.0);
        return Guidance {
            desired_velocity: inward * TRANSIT_SPEED + correct,
            waypoint: frame.center + inward * 300.0,
            waypoint_dir: inward,
            final_run: true,
        };
    }
    let approach = frame.center + axis * ENTRY * APPROACH_DISTANCE;
    // On the far side, off the axis: onto it first, then back through the middle.
    let approach = if side != ENTRY && lateral.length() > GATE_RADIUS * 0.4 { frame.center + axis * h.max(1000.0) } else { approach };
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
    /// Distance before the opening's plane, on its entry side (m; negative:
    /// past it, on the far side).
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
    let h = if side == ENTRY { h } else { -h };
    let v = ship.velocity - frame.velocity;
    let offset = (r - axis * r.dot(axis)).length();
    let final_run = if clearance.autopilot { clearance.phase == Phase::Final } else { in_final_zone(frame, ship.position) };
    GateStatus {
        phase: clearance.phase,
        autopilot: clearance.autopilot,
        range: r.length(),
        distance: h,
        offset,
        closing: v.dot(axis),
        speed: v.length(),
        relative_velocity: v,
        in_corridor: offset < GATE_RADIUS - RING_TUBE - 200.0,
        guidance: guidance(frame, ship.position, final_run, ship.side_accel()),
    }
}

/// The transit autopilot: thrusters only, nose pointed along the run. Its
/// command holds for `h` seconds.
pub fn autopilot(frame: &GateFrame, ship: &Ship, phase: Phase, may_enter: bool, h: f64) -> Command {
    let axis = frame.axis();
    let inward = axis;
    let v = ship.velocity - frame.velocity;
    let to_approach = (frame.center + axis * ENTRY * APPROACH_DISTANCE).distance(ship.position);
    let aligned = ship.forward().dot(inward) > 0.995;
    let phase = match phase {
        Phase::Approach if to_approach < 100.0 && v.length() < 8.0 => Phase::Align,
        Phase::Align if aligned && may_enter => Phase::Final,
        Phase::Final if !in_final_zone(frame, ship.position) => Phase::Approach,
        p @ (Phase::Approach | Phase::Align | Phase::Final) => p,
        _ => Phase::Approach,
    };
    let g = guidance(frame, ship.position, phase == Phase::Final, ship.side_accel());
    let accel = ((g.desired_velocity - v) * gain(1.2, h)).clamp_length_max(ship.side_accel());
    let rcs = ship.thruster_command(ship.orientation.inverse() * accel);
    // Nose: at the gate while travelling, then along the run-in (roll kept level
    // with the ring's own frame).
    let target = match phase {
        Phase::Approach => facing(frame.center - ship.position, frame.rotation * DVec3::Z),
        _ => facing(inward, frame.rotation * DVec3::Z),
    };
    Command { controls: attitude(ship, target, DVec3::ZERO, h), throttle: 0.0, rcs, phase, attitude: target }
}
