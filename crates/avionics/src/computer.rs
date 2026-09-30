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
use crate::nav::{Clearance, NavTarget, Phase};

/// The dock/land/gate autopilot for `target` at `t`, from `phase`, for the
/// next `h` seconds: what it commands, and the phase it has moved on to.
pub fn autopilot(sys: &StarSystem, ship: &Ship, target: NavTarget, phase: Phase, t: f64, h: f64, positions: &[DVec3]) -> Command {
    match target {
        NavTarget::Station(station) => {
            let frame = StationFrame::new(sys, station, t, positions);
            docking::autopilot(&frame, ship, phase, h)
        }
        NavTarget::Spaceport(port) => {
            let pad = PadFrame::new(sys, port, t, positions);
            landing::autopilot(&pad, ship, sys.gravity(ship.position, positions), phase, h)
        }
        NavTarget::Gate(g) => {
            let frame = GateFrame::new(sys, g, t, positions);
            gate::autopilot(&frame, ship, phase, h)
        }
    }
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
        let cmd = autopilot(sys, ship, c.target, c.phase, t, h, positions);
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
