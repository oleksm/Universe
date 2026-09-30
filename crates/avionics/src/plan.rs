//! The flight planner: fly a copy of the ship ahead, under the same
//! autopilot that would fly it, through the physics kernel (`simulate`) with
//! the ship's own devices — real gravity, moving targets, the same contact
//! rules — and record the path.
//!
//! The result is what the pilot should do from here: where the ship goes,
//! which way it faces along the way, and when it burns or coasts. The
//! autopilot flies exactly this, so watching it teaches the procedure. The
//! plan arrives when the world would take the copy in: docked in the slot,
//! landed on the pad, or through the gate.
//!
//! Within the range the autopilot flies itself, the copy flies exactly as the
//! ship would, reacting every fine step, so the plan is the flight. From
//! farther out (where the autopilot would jump by hyperdrive first, which the
//! plan leaves out) it looks ahead in longer substeps.

use glam::{DQuat, DVec3};
use universe_physics::integrate::FINE_STEP;
use universe_physics::{simulate, Ephemeris, Fact, Span};
use universe_world::ship::{Ship, ShipState, SHIP_RADIUS};
use universe_world::station::STATION_SIZE;
use universe_world::{gate as gate_device, spaceport, station};
use universe_world::{Devices, GateFrame, StarSystem, StationFrame};

use crate::avionics::Avionics;
use crate::computer;
use crate::landing::PadFrame;
use crate::nav::{Clearance, NavTarget, Phase};
use crate::route;

/// What the ship is doing at a point of the plan.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Action {
    /// Main engine, at this throttle.
    Burn(f64),
    /// Translation / lift thrusters.
    Thrusters,
    Coast,
}

#[derive(Clone, Copy, Debug)]
pub struct PlanPoint {
    /// Seconds from now.
    pub time: f64,
    /// Where the ship will be, drawn relative to the target as it is now.
    pub position: DVec3,
    /// Planned attitude, in the same frame.
    pub orientation: DQuat,
    /// Where the autopilot wants the nose at this point (the end of any turn in progress).
    pub aim: DQuat,
    pub action: Action,
    pub phase: Phase,
}

#[derive(Clone, Debug, Default)]
pub struct Plan {
    /// Simulation time the plan was made at; point times count from here.
    pub start: f64,
    /// Where the plan's reference was at `start` (station/gate center, or the
    /// planet's center for a spaceport), and its spin. Points are relative to
    /// it: to draw an older plan, move it with the reference (see `anchor`).
    pub center: DVec3,
    pub spin: DVec3,
    pub points: Vec<PlanPoint>,
    /// The plan reaches the slot / pad within the horizon.
    pub arrives: bool,
}

impl Plan {
    /// Map a plan point (made at `start`) to where it is now, given the
    /// reference's current center: carried along with it, and turned with it.
    pub fn anchor(&self, center_now: DVec3, now: f64) -> impl Fn(DVec3) -> DVec3 + '_ {
        let turn = DQuat::from_scaled_axis(self.spin * (now - self.start));
        move |p| center_now + turn * (p - self.center)
    }

    /// Rotation to apply to planned attitudes for the same reason.
    pub fn turn(&self, now: f64) -> DQuat {
        DQuat::from_scaled_axis(self.spin * (now - self.start))
    }

    /// Time until the action changes from what it is now, and the next action.
    pub fn next_change(&self) -> Option<(f64, Action)> {
        let first = self.points.first()?;
        let kind = |a: Action| std::mem::discriminant(&a);
        self.points.iter().find(|p| kind(p.action) != kind(first.action)).map(|p| (p.time, p.action))
    }
}

const MAX_POINTS: usize = 2000;
/// Stop planning this far ahead (s).
const HORIZON: f64 = 6.0 * 3600.0;
/// Plan points are no more than half a second apart within this distance of
/// the goal (m).
const NEAR: f64 = 20_000.0;
/// Beyond where the autopilot would fly itself (it jumps by hyperdrive from
/// farther out: see `route::hyperjump_limit`), the copy looks ahead in longer
/// substeps, up to this (s; the kernel also keeps them short against the
/// orbital time scale). The autopilot's gains stay within reach of such steps
/// (see `docking::gain`).
const FAR_STEP: f64 = 10.0;

/// Plan from the ship's current state toward `target`, the autopilot being
/// in `phase`, at world time `now`.
pub fn plan(sys: &StarSystem, ship: &Ship, target: NavTarget, phase: Phase, now: f64) -> Plan {
    let mut ship = ship.clone();
    // The copy's avionics: only the autopilot, flying to the target.
    let mut avionics = Avionics { clearance: Some(Clearance { target, autopilot: true, phase }), ..Avionics::default() };
    let mut t = now;
    let mut positions = Vec::new();
    sys.positions(t, &mut positions);

    // The frame we draw in: the target as it is now.
    let (now_center, now_omega) = match target {
        NavTarget::Station(s) | NavTarget::Gate(s) => (positions[s], DVec3::ZERO),
        NavTarget::Spaceport(p) => {
            let pad = PadFrame::new(sys, p, t, &positions);
            (pad.body_center, pad.angular_velocity)
        }
    };

    let mut out = Plan { start: now, center: now_center, spin: now_omega, ..Default::default() };
    // Rail bodies for the substeps, snapshot at a moment (see `Ephemeris`).
    let mut ephemeris: Option<(f64, Ephemeris)> = None;
    let mut stopped = None;
    let mut events = Vec::new();
    loop {
        sys.positions(t, &mut positions);
        // What the autopilot does here, how far it has to go, and how fast
        // it closes.
        let phase = avionics.clearance.map_or(phase, |c| c.phase);
        let cmd = computer::autopilot(sys, &ship, target, phase, t, FINE_STEP, &positions);
        let (left, rel_speed, center) = progress(sys, &ship, target, t, &positions);
        let action = if cmd.throttle > 0.02 {
            Action::Burn(cmd.throttle)
        } else if cmd.rcs.length() > 0.05 {
            Action::Thrusters
        } else {
            Action::Coast
        };
        // Express in the target's frame as it is now (undo its drift and, for a
        // planet, its rotation since `now`).
        let undo = DQuat::from_scaled_axis(-now_omega * (t - now));
        out.points.push(PlanPoint {
            time: t - now,
            position: now_center + undo * (ship.position - center),
            orientation: undo * ship.orientation,
            aim: undo * cmd.attitude,
            action,
            phase: cmd.phase,
        });
        if let Some(fact) = stopped {
            // The copy touched something: arrived, or (it shouldn't, but show
            // it honestly) hit it.
            out.arrives = arrived(sys, &mut ship, target, &fact, t, &positions);
            break;
        }
        if out.points.len() >= MAX_POINTS || t - now >= HORIZON {
            break;
        }

        // Points: close together near the goal, and at first (so the first
        // turn and burn are timed right), sparse far out.
        let max_dt = if left < NEAR { 0.5 } else { 60.0 };
        let dt = (left / rel_speed.max(1.0) * 0.05).clamp(0.1, max_dt).min(0.5 + 0.25 * (t - now));
        // Substeps: where the autopilot flies, exactly as it does (it reacts
        // every fine step); from farther out, longer ones after the first
        // few seconds. (The kernel keeps them short near stations and gates.)
        let max_h = if left < route::hyperjump_limit(target) { FINE_STEP } else { (0.1 * (t - now)).clamp(FINE_STEP, FAR_STEP) };
        // Fly to the next point. Where the autopilot flies, in pieces short
        // enough for the rail bodies to come from a snapshot, as in flight
        // (see `Ephemeris`); farther out, substeps outgrow a snapshot, so the
        // kernel solves the rails exactly.
        let end = t + dt;
        let far = max_h > FINE_STEP;
        while stopped.is_none() && end - t > 1e-9 {
            let piece = if far { end - t } else { (end - t).min(Ephemeris::SPAN) };
            if !far && ephemeris.as_ref().is_none_or(|(t0, _)| t + piece > t0 + Ephemeris::SPAN) {
                ephemeris = Some((t, sys.ephemeris(t)));
            }
            let rigid = ship.rigid();
            let mut computer = avionics.computer();
            events.clear();
            let mut devices = Devices::new(sys, &mut ship, &mut computer, &mut events);
            let snapshot = if far { None } else { ephemeris.as_ref().map(|(_, e)| e) };
            let (copy, outcome) = simulate(&sys.bodies, snapshot, &rigid, Span { t, dt: piece, max_h }, &mut devices);
            ship.set_rigid(&copy);
            t = outcome.time;
            stopped = outcome.fact;
        }
    }
    out
}

/// How far the ship has to go to reach `target` (m), its speed relative to
/// it (m/s), and where the target's reference is (station or gate center,
/// or the planet's center for a spaceport).
fn progress(sys: &StarSystem, ship: &Ship, target: NavTarget, t: f64, positions: &[DVec3]) -> (f64, f64, DVec3) {
    match target {
        NavTarget::Station(s) => {
            let f = StationFrame::new(sys, s, t, positions);
            (ship.position.distance(f.on_axis(STATION_SIZE * 0.9)), (ship.velocity - f.velocity).length(), f.center)
        }
        NavTarget::Gate(g) => {
            let f = GateFrame::new(sys, g, t, positions);
            (ship.position.distance(f.center), (ship.velocity - f.velocity).length(), f.center)
        }
        NavTarget::Spaceport(p) => {
            let pad = PadFrame::new(sys, p, t, positions);
            let rel = (ship.velocity - pad.frame_velocity(ship.position)).length();
            (ship.position.distance(pad.pad) - SHIP_RADIUS, rel, pad.body_center)
        }
    }
}

/// Whether the world would take the copy in at the target on `fact` (at
/// `t`, bodies at `positions`): docked by the station's port, landed on the
/// pad, or let through by the gate. The copy is left as the world leaves it.
fn arrived(sys: &StarSystem, ship: &mut Ship, target: NavTarget, fact: &Fact, t: f64, positions: &[DVec3]) -> bool {
    let mut events = Vec::new();
    match (target, fact) {
        (NavTarget::Station(s), Fact::Contact(c)) if c.body == s => {
            station::contact(sys, ship, c, t, positions, &mut events);
            matches!(ship.state, ShipState::Landed { .. })
        }
        (NavTarget::Spaceport(p), Fact::Contact(c)) if c.body == sys.spaceports[p].body => {
            spaceport::touch_down(sys, ship, c, t, &mut events);
            matches!(ship.state, ShipState::Landed { .. }) && sys.port_at(c.body, c.local) == Some(p)
        }
        (NavTarget::Gate(g), Fact::Trigger { body, .. }) if *body == g => {
            let frame = GateFrame::new(sys, g, t, positions);
            gate_device::enter(&frame, ship, sys.index, sys.index).is_ok()
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use glam::DQuat;
    use universe_world::World;

    use super::*;

    /// A ship at the home station of the `seed` universe, cleared for
    /// `target` (from `place`) with the autopilot on: the plan, and how long
    /// the autopilot really takes (flown by the world, frame by frame).
    fn planned_and_flown(seed: u64, target: impl Fn(&StarSystem) -> NavTarget, place: impl Fn(&StarSystem, &mut Ship, f64, &[DVec3])) -> (Plan, f64) {
        let mut world = World::new(seed);
        let (mut ship, mut system, mut events) = (Ship::new(DVec3::ZERO, DVec3::ZERO, DQuat::IDENTITY), 0, Vec::new());
        world.respawn(&mut ship, &mut system, &mut events);
        let sys = world.system(system);
        let mut positions = Vec::new();
        sys.positions(world.time, &mut positions);
        place(&sys, &mut ship, world.time, &positions);
        let target = target(&sys);
        let mut avionics = Avionics { clearance: Some(Clearance { target, autopilot: true, phase: Phase::Approach }), ..Avionics::default() };
        let plan = avionics.plan(&sys, &ship, world.time).expect("flying, not in hyperdrive");
        let start = world.time;
        while ship.is_flying() && world.time - start < 3600.0 {
            let commands = ship.holding();
            let mut computer = avionics.computer();
            world.step_ship(&mut ship, &mut system, &commands, &mut computer, 1.0 / 60.0, 5.0, &mut events);
        }
        assert!(matches!(ship.state, ShipState::Landed { .. }), "the autopilot should get there: {events:?}");
        (plan, world.time - start)
    }

    #[test]
    fn the_plan_is_the_flight() {
        // Docking from where a new ship starts, 4 km behind the station.
        let (plan, flown) = planned_and_flown(1984, |sys| NavTarget::Station(sys.station().unwrap()), |_, _, _, _| {});
        let eta = plan.points.last().unwrap().time;
        eprintln!("docking: planned {eta:.1} s, flown {flown:.1} s");
        assert!(plan.arrives && (eta - flown).abs() < 0.5, "planned {eta:.1} s, flown {flown:.1} s");

        // Landing from 150 km up and to the side of the pad.
        let port = |sys: &StarSystem| {
            let planet = sys.bodies[sys.station().unwrap()].rail.parent.unwrap();
            sys.spaceports.iter().position(|p| p.body == planet).unwrap()
        };
        let (plan, flown) = planned_and_flown(
            1984,
            |sys| NavTarget::Spaceport(port(sys)),
            |sys, ship, t, positions| {
                let pad = PadFrame::new(sys, port(sys), t, positions);
                ship.position = pad.pad + (pad.up + pad.up.any_orthonormal_vector() * 0.5).normalize() * 150_000.0;
                ship.velocity = pad.frame_velocity(ship.position);
            },
        );
        let eta = plan.points.last().unwrap().time;
        eprintln!("landing: planned {eta:.1} s, flown {flown:.1} s");
        assert!(plan.arrives && (eta - flown).abs() < 0.5, "planned {eta:.1} s, flown {flown:.1} s");
    }
}
