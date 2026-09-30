//! The ship's flight computer while the ship moves: the dock/land/gate
//! autopilot every substep, and the hyperdrive's navigation every frame (see
//! `hyperdrive`). It reads the ship and the star system, keeps its own state
//! in the avionics, and flies the ship only through `ShipCommands` (see
//! `universe_world::FlightComputer`).

use glam::DVec3;
use universe_physics::integrate::FINE_STEP;
use universe_world::{FlightComputer, GateFrame, Ship, ShipCommands, StarSystem, StationFrame};

use crate::avionics::Avionics;
use crate::docking::{self, Command};
use crate::gate;
use crate::hyperdrive;
use crate::landing::{self, PadFrame};
use crate::nav::{Clearance, NavTarget, PadSlot, Phase};

/// The dock/land/gate autopilot for `target` at `t`, from `phase`, for the
/// next `h` seconds: what it commands, and the phase it has moved on to.
/// `wait`: traffic control hasn't let it into the corridor (a station's or a
/// gate's final run) yet: close by, it holds at its own place (this one) on a
/// ring out beyond the corridor's entry, clear of the others waiting.
#[allow(clippy::too_many_arguments)]
pub fn autopilot(sys: &StarSystem, ship: &Ship, target: NavTarget, phase: Phase, pad: PadSlot, wait: Option<usize>, t: f64, h: f64, positions: &[DVec3]) -> Command {
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
        NavTarget::Spaceport(port) => {
            let pad = PadFrame::for_slot(sys, port, pad, t, positions);
            landing::autopilot(&pad, ship, sys.gravity(ship.position, positions), phase, h)
        }
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

/// The flight computer of one ship, for one step.
pub struct Computer<'a> {
    avionics: &'a mut Avionics,
    /// The hyperdrive dropped out at the nav target (its name).
    pub arrived: Option<String>,
}

impl<'a> Computer<'a> {
    pub fn new(avionics: &'a mut Avionics) -> Self {
        Self { avionics, arrived: None }
    }
}

impl FlightComputer for Computer<'_> {
    /// The autopilot reacts every 0.05 s at most, so control is precise.
    fn interval(&self) -> f64 {
        if self.avionics.autopilot_engaged() { FINE_STEP } else { f64::INFINITY }
    }

    /// The dock/land/gate autopilot, if it's flying.
    fn substep(&mut self, sys: &StarSystem, ship: &Ship, t: f64, h: f64, positions: &[DVec3]) -> Option<ShipCommands> {
        let c = self.avionics.clearance.filter(|c| c.autopilot)?;
        let wait = self.avionics.corridor_denied.then_some(self.avionics.wait_place);
        let cmd = autopilot(sys, ship, c.target, c.phase, c.pad, wait, t, h, positions);
        if cmd.phase != c.phase {
            self.avionics.clearance = Some(Clearance { phase: cmd.phase, ..c });
        }
        Some(cmd.commands())
    }

    /// Navigation in hyperdrive (see `hyperdrive::navigate`).
    fn hyperdrive(&mut self, sys: &StarSystem, ship: &Ship, t: f64, positions: &[DVec3]) -> ShipCommands {
        let a = &mut *self.avionics;
        let (commands, arrived) = hyperdrive::navigate(sys, ship, t, positions, a.nav_target, a.hyper_autopilot, &mut a.debug_way);
        if arrived.is_some() {
            self.arrived = arrived;
        }
        commands
    }
}
