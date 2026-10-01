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
use universe_world::rules::Rules;
use universe_world::{Devices, GateFrame, StarSystem, StationFrame};

use crate::avionics::Avionics;
use crate::computer;
use crate::landing::{self, PadFrame};
use crate::nav::{Clearance, NavTarget, PadSlot, Phase};
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
    /// Waiting for a pad: the plan ends where the ship joins its place on
    /// the holding circle (laps round it would show nothing new).
    pub holds: bool,
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
/// (see `docking::gain`); short enough that the ship's turning — torque
/// against inertia, as in flight — keeps up with what the autopilot asks.
/// (2 s was too coarse for the winged Drover's slower roll: its copy came
/// down short of the pad, into the sea, while the real flight landed.)
const FAR_STEP: f64 = 1.0;
/// A holding ship this near its place on the circle has joined it (m).
const JOINED: f64 = 400.0;

/// Plan from the ship's current state toward `target`, the autopilot being
/// in `phase`, at world time `now`.
pub fn plan(sys: &StarSystem, rules: &Rules, ship: &Ship, target: NavTarget, phase: Phase, pad: PadSlot, now: f64) -> Plan {
    let mut ship = ship.clone();
    // The copy's avionics: only the autopilot, flying to the target.
    let mut avionics = Avionics { clearance: Some(Clearance { target, autopilot: true, phase, pad }), ..Avionics::default() };
    let mut t = now;
    let mut positions = Vec::new();
    sys.positions(t, &mut positions);

    // The frame we draw in: the target as it is now.
    let (now_center, now_omega) = match target {
        NavTarget::Station(s) | NavTarget::Gate(s) | NavTarget::Asteroid(s) => (positions[s], DVec3::ZERO),
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
        let cmd = computer::autopilot(&computer::AutopilotInput { sys, ship: &ship, target, phase, pad, wait: None, t, h: FINE_STEP, positions: &positions });
        let (left, rel_speed, center) = progress(sys, &ship, target, pad, t, &positions);
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
            out.arrives = arrived(sys, rules, &mut ship, target, &fact, t, &positions);
            break;
        }
        if out.points.len() >= MAX_POINTS || t - now >= HORIZON {
            break;
        }
        // Holding: once at its place on the circle, and moving with it, that's it.
        if let (NavTarget::Spaceport(p), PadSlot::Hold(n)) = (target, pad) {
            let (place, v, _) = landing::hold_place(&PadFrame::new(sys, p, t, &positions), n, t);
            if ship.position.distance(place) < JOINED && (ship.velocity - v).length() < 30.0 {
                out.holds = true;
                break;
            }
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
            // The autopilot's say, held for its control step (as in flight,
            // where it speaks once a tick; out far, a coarser step will do),
            // then the copy flies it.
            let step = if far { max_h.min(end - t) } else { FINE_STEP.min(end - t) };
            if !far && ephemeris.as_ref().is_none_or(|(t0, _)| t + step > t0 + Ephemeris::SPAN) {
                ephemeris = Some((t, sys.ephemeris(t)));
            }
            sys.positions(t, &mut positions);
            if let Some(c) = avionics.clearance {
                let cmd = computer::autopilot(&computer::AutopilotInput { sys, ship: &ship, target, phase: c.phase, pad, wait: None, t, h: step, positions: &positions });
                avionics.clearance = Some(Clearance { phase: cmd.phase, ..c });
                ship.throttle = cmd.throttle;
                ship.rcs = cmd.rcs;
                ship.drive(Some(&cmd.controls), step, true);
            }
            let rigid = ship.rigid();
            events.clear();
            let mut devices = Devices::new(&mut ship, rules, &mut events).among(&sys.bodies);
            let snapshot = if far { None } else { ephemeris.as_ref().map(|(_, e)| e) };
            let (copy, outcome) = simulate(&sys.bodies, snapshot, &rigid, Span { t, dt: step, max_h, contact_step: FINE_STEP }, &mut devices);
            ship.set_rigid(&copy);
            t = outcome.time;
            stopped = outcome.fact;
            // Through a trigger that takes it nowhere (a gate's ring the
            // wrong way: one way only), it flies on, as it would.
            if let Some(fact @ Fact::Trigger { .. }) = &stopped {
                let mut probe = ship.clone();
                let mut said = Vec::new();
                sys.positions(t, &mut positions);
                universe_world::rules::apply(rules, sys, sys.index, &mut probe, fact, t, &positions, &mut said);
                if probe.is_flying() {
                    stopped = None;
                }
            }
        }
    }
    out
}

/// How far the ship has to go to reach `target` (m), its speed relative to
/// it (m/s), and where the target's reference is (station or gate center,
/// or the planet's center for a spaceport).
fn progress(sys: &StarSystem, ship: &Ship, target: NavTarget, pad: PadSlot, t: f64, positions: &[DVec3]) -> (f64, f64, DVec3) {
    match target {
        NavTarget::Station(s) => {
            let f = StationFrame::new(sys, s, t, positions);
            let k = crate::docking::pad_of(pad);
            (ship.position.distance(f.pad(k)), (ship.velocity - f.velocity_at(ship.position)).length(), f.center)
        }
        NavTarget::Gate(g) => {
            let f = GateFrame::new(sys, g, t, positions);
            (ship.position.distance(f.center), (ship.velocity - f.velocity).length(), f.center)
        }
        NavTarget::Asteroid(b) => (ship.position.distance(positions[b]), (ship.velocity - sys.velocity(b, t)).length(), positions[b]),
        NavTarget::Spaceport(p) => {
            let pad = PadFrame::for_slot(sys, p, pad, t, positions);
            let rel = (ship.velocity - pad.frame_velocity(ship.position)).length();
            (ship.position.distance(pad.pad) - SHIP_RADIUS, rel, pad.body_center)
        }
    }
}

/// Whether the world would take the copy in at the target on `fact` (at
/// `t`, bodies at `positions`), by the target's own rule: docked, landed on
/// the pad, or let through the gate. The copy is left as the world leaves it.
fn arrived(sys: &StarSystem, rules: &Rules, ship: &mut Ship, target: NavTarget, fact: &Fact, t: f64, positions: &[DVec3]) -> bool {
    let mut events = Vec::new();
    let body = match fact {
        Fact::Contact(c) => c.body,
        Fact::Trigger { body, .. } => *body,
    };
    let ours = match target {
        NavTarget::Station(s) | NavTarget::Gate(s) => body == s,
        NavTarget::Spaceport(p) => body == sys.spaceports[p].body,
        NavTarget::Asteroid(_) => false,
    };
    if !ours {
        return false;
    }
    universe_world::rules::apply(rules, sys, sys.index, ship, fact, t, positions, &mut events);
    match (target, fact) {
        (NavTarget::Spaceport(p), Fact::Contact(c)) => matches!(ship.state, ShipState::Landed { .. }) && sys.port_at(c.body, c.local) == Some(p),
        (NavTarget::Gate(_), _) => matches!(ship.state, ShipState::Transit { .. }),
        _ => matches!(ship.state, ShipState::Landed { .. }),
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
        let mut avionics = Avionics { clearance: Some(Clearance { target, autopilot: true, phase: Phase::Approach, pad: PadSlot::Center }), ..Avionics::default() };
        let plan = avionics.plan(&sys, &world.rules_of(system), &ship, world.time).expect("flying, not in hyperdrive");
        let start = world.time;
        // As the avionics fly it: the autopilot's say once a tick (here 0.05 s,
        // the planner's control step), held through the world's step.
        let (real_dt, warp) = (1.0 / 60.0, 3.0);
        while ship.is_flying() && world.time - start < 3600.0 {
            let c = avionics.clearance.expect("cleared");
            sys.positions(world.time, &mut positions);
            let cmd = computer::autopilot(&computer::AutopilotInput { sys: &sys, ship: &ship, target, phase: c.phase, pad: c.pad, wait: None, t: world.time, h: real_dt * warp, positions: &positions });
            avionics.clearance = Some(Clearance { phase: cmd.phase, ..c });
            world.step_ship(&mut ship, &mut system, &cmd.commands(), real_dt, warp, &mut events);
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
