//! The dock/land/gate autopilot: what it commands for the next tick from
//! where the ship is (see `Avionics::fly`, which runs it once a tick, before
//! the world steps the ship with the settings it gave).

use glam::DVec3;
use universe_world::{GateFrame, Ship, StarSystem, StationFrame};

use crate::docking::{self, Command};
use crate::gate;
use crate::landing::{self, PadFrame};
use crate::nav::{NavTarget, PadSlot, Phase};

/// The dock/land/gate autopilot for `target` at `t`, from `phase`, for the
/// next `h` seconds: what it commands, and the phase it has moved on to.
/// What the autopilot knows for one step.
#[derive(Clone, Copy)]
pub struct AutopilotInput<'a> {
    pub sys: &'a StarSystem,
    pub ship: &'a Ship,
    /// Where it's cleared for, in what phase, on which pad.
    pub target: NavTarget,
    pub phase: Phase,
    pub pad: PadSlot,
    /// Traffic control hasn't let it into the corridor (a station's or a
    /// gate's final run) yet: close by, it holds at its own place (this one)
    /// on a ring out beyond the corridor's entry, clear of the others waiting.
    pub wait: Option<usize>,
    /// Now, and how long its command holds (s); the bodies at `t`.
    pub t: f64,
    pub h: f64,
    pub positions: &'a [DVec3],
}

pub fn autopilot(input: &AutopilotInput) -> Command {
    let AutopilotInput { sys, ship, target, phase, pad, wait, t, h, positions } = *input;
    let may_enter = wait.is_none();
    if let Some(slot) = wait
        && matches!(phase, Phase::Approach | Phase::Align)
        && let Some(cmd) = hold_for_corridor(sys, ship, target, slot, t, h, positions)
    {
        return cmd;
    }
    match target {
        NavTarget::Station(station) => {
            let frame = StationFrame::new(sys, station, t, positions);
            docking::autopilot(&frame, ship, phase, may_enter, h)
        }
        NavTarget::Spaceport(port) => match pad {
            // Waiting for a pad: the holding circle.
            crate::nav::PadSlot::Hold(n) => landing::hold(&PadFrame::new(sys, port, t, positions), n, ship, sys.gravity(ship.position, positions), t, h),
            _ => {
                let pad = PadFrame::for_slot(sys, port, pad, t, positions);
                landing::autopilot(&pad, ship, sys.gravity(ship.position, positions), phase, h)
            }
        },
        NavTarget::Gate(g) => {
            let frame = GateFrame::new(sys, g, t, positions);
            gate::autopilot(&frame, ship, phase, may_enter, h)
        }
    }
}

/// Places on the waiting ring: how many, how far out beyond the corridor's
/// entry, and how far off its axis (m).
const WAIT_PLACES: usize = 24;
const WAIT_OUT: f64 = 1500.0;
const WAIT_RING: f64 = 1200.0;
/// Within this of its waiting place a waiting ship holds there (m).
const WAIT_ZONE: f64 = 8000.0;

/// Hold at waiting place `slot` of `target`'s corridor, if close by.
fn hold_for_corridor(sys: &StarSystem, ship: &Ship, target: NavTarget, slot: usize, t: f64, h: f64, positions: &[DVec3]) -> Option<Command> {
    let (center, axis, velocity, entry) = match target {
        NavTarget::Station(s) => {
            let f = StationFrame::new(sys, s, t, positions);
            (f.center, f.axis(), f.velocity, docking::APPROACH_HEIGHT)
        }
        NavTarget::Gate(g) => {
            let f = GateFrame::new(sys, g, t, positions);
            let (side, _) = f.side(ship.position);
            (f.center, f.axis() * side, f.velocity, gate::APPROACH_DISTANCE)
        }
        NavTarget::Spaceport(_) => return None,
    };
    let across = axis.any_orthonormal_vector();
    // Two rings of twelve, the outer one a little further out.
    let (ring, k) = ((slot % WAIT_PLACES) / 12, slot % 12);
    let a = (k as f64 + 0.5 * ring as f64) * std::f64::consts::TAU / 12.0;
    let around = glam::DQuat::from_axis_angle(axis, a) * across;
    let place = center + axis * (entry + WAIT_OUT + 600.0 * ring as f64) + around * (WAIT_RING + 700.0 * ring as f64);
    if place.distance(ship.position) > WAIT_ZONE {
        return None;
    }
    let v = ship.velocity - velocity;
    let d = place - ship.position;
    let desired = d.clamp_length_max((d.length() * 0.05).min(120.0) + 0.0);
    let accel = ((desired - v) * docking::gain(1.0, h)).clamp_length_max(ship.side_accel());
    let rcs = ship.thruster_command(ship.orientation.inverse() * accel);
    let facing_target = universe_world::ship::facing(center - ship.position, axis);
    let controls = docking::attitude(ship, facing_target, DVec3::ZERO, h);
    Some(Command { controls, throttle: 0.0, rcs, phase: Phase::Approach, attitude: facing_target })
}
