//! The flight planner: fast-forward the ship under the same autopilot that
//! would fly it, with real gravity and moving targets, and record the path.
//!
//! The result is what the pilot should do from here: where the ship goes,
//! which way it faces along the way, and when it burns or coasts. The
//! autopilot flies exactly this, so watching it teaches the procedure.
//!
//! (It still steps the ship with its own simplified integrator; the plan is to
//! simulate a copy through the physics kernel instead, `universe_physics::simulate`.)

use glam::{DQuat, DVec3};
use universe_physics::{Frame, RingCrossing};

use crate::docking::{self, StationFrame, STATION_SIZE};
use crate::gate::{self, GateFrame};
use crate::landing::{self, PadFrame};
use crate::ship::{self, NavTarget, Phase, Ship};
use crate::system::StarSystem;

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

/// Plan from the ship's current state toward `target`.
pub fn plan(sys: &StarSystem, ship: &Ship, target: NavTarget, phase: Phase, now: f64) -> Plan {
    let mut ship = ship.clone();
    let mut phase = phase;
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
    let mut last_dt: f64 = 0.0;
    while out.points.len() < MAX_POINTS && t - now < HORIZON {
        sys.positions(t, &mut positions);
        let gravity = sys.gravity(ship.position, &positions);
        // Command, distance left to the goal, speed relative to the target, target reference point.
        // Command, distance left to the goal, speed relative to the target,
        // target reference point, the velocity guidance wants (absolute), and
        // the target's own acceleration (it falls too).
        let command = |ship: &Ship, phase: Phase| match target {
            NavTarget::Station(s) => {
                let f = StationFrame::new(sys, s, t, &positions);
                let cmd = docking::autopilot(&f, ship, phase);
                let want = f.velocity + docking::guidance(&f, ship.position, cmd.phase == Phase::Final, ship.side_accel()).desired_velocity;
                let fall = sys.gravity(f.center, &positions);
                (cmd, ship.position.distance(f.on_axis(STATION_SIZE * 0.9)), (ship.velocity - f.velocity).length(), f.center, want, fall)
            }
            NavTarget::Gate(g) => {
                let f = GateFrame::new(sys, g, t, &positions);
                let cmd = gate::autopilot(&f, ship, phase);
                let want = f.velocity + gate::guidance(&f, ship.position, cmd.phase == Phase::Final, ship.side_accel()).desired_velocity;
                let fall = sys.gravity(f.center, &positions);
                (cmd, ship.position.distance(f.center), (ship.velocity - f.velocity).length(), f.center, want, fall)
            }
            NavTarget::Spaceport(p) => {
                let pad = PadFrame::new(sys, p, t, &positions);
                let cmd = landing::autopilot(&pad, ship, gravity, phase);
                let frame_v = pad.frame_velocity(ship.position);
                let want = frame_v + landing::guidance(&pad, ship.position, cmd.phase == Phase::Descent, ship.main_accel()).desired_velocity;
                let rel = (ship.velocity - frame_v).length();
                let fall = sys.gravity(pad.body_center, &positions);
                (cmd, ship.position.distance(pad.pad) - ship::SHIP_RADIUS, rel, pad.body_center, want, fall)
            }
        };
        // Turn toward where the autopilot wants the nose, at the ship's real
        // turn rate over this step, then take its command for that attitude.
        let (turn, ..) = command(&ship, phase);
        // Like the attitude controller: full turn rate while far off, then
        // slowing down in proportion to the angle left (rate = 1.5 * angle).
        let angle = ship.orientation.angle_between(turn.attitude);
        let left = {
            let (mut a, mut rem) = (angle, last_dt);
            let linear = ship::TURN_RATE / 1.5;
            if a > linear {
                let t = ((a - linear) / ship::TURN_RATE).min(rem);
                a -= ship::TURN_RATE * t;
                rem -= t;
            }
            a * (-1.5 * rem).exp()
        };
        let can_turn = angle - left;
        ship.orientation = if left < 1e-3 { turn.attitude } else { ship.orientation.slerp(turn.attitude, can_turn / angle) };
        // The ship's rotation rate is what this turn implies (it settles as the
        // turn finishes, or matches a slowly moving target such as a spinning
        // station's slot). Carrying over the rate from when the plan was made
        // kept "aligned" checks failing, and such plans never arrived.
        ship.angular_velocity = if last_dt > 0.0 { DVec3::Z * (can_turn / last_dt) } else { DVec3::ZERO };
        let (cmd, left, rel_speed, center, want, fall) = command(&ship, turn.phase);
        phase = cmd.phase;
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
            aim: undo * turn.attitude,
            action,
            phase,
        });
        if left < 15.0 {
            out.arrives = true;
            break;
        }

        // Step size: coarse far away, but near the goal no longer than the
        // autopilot's own reaction time, or the simulated control oscillates.
        let max_dt = if left < 20_000.0 { 0.5 } else { 60.0 };
        // Also start with short steps, so the first turn and burn are timed right.
        let dt = (left / rel_speed.max(1.0) * 0.05).clamp(0.1, max_dt).min(0.5 + 0.25 * (t - now));
        last_dt = dt;
        // Thrust for this step: whatever reaches the wanted velocity by the end
        // of the step, limited by what the engines can give. Unlike the
        // autopilot's continuous feedback, this stays stable for long steps.
        let lined_up = ship.orientation.angle_between(turn.attitude) < 0.25;
        let capacity = match (cmd.throttle > 0.02 && lined_up, cmd.phase) {
            // The autopilot tops up a burn with the thrusters (they push along too).
            (true, _) => ship.main_accel() + ship.side_accel(),
            (false, Phase::Descent) => ship.lift_accel(),
            _ => ship.side_accel(),
        };
        // Only the gravity *difference* from the target needs thrust: a station
        // falls around its planet just like the ship does.
        let thrust = ((want - ship.velocity) / dt.max(1.0) - (gravity - fall)).clamp_length_max(capacity);
        let prev = ship.position;
        ship.velocity += (gravity + thrust) * (dt * 0.5);
        ship.position += ship.velocity * dt;
        t += dt;
        sys.positions(t, &mut positions);
        ship.velocity += (sys.gravity(ship.position, &positions) + thrust) * (dt * 0.5);

        // A gate plan arrives when the ship passes through the ring.
        if let NavTarget::Gate(g) = target {
            let f = Frame::of(&sys.bodies, g, t, &positions);
            match gate::RING.crossing(&f, prev, ship.position, dt, ship::SHIP_RADIUS) {
                RingCrossing::Through => {
                    out.arrives = true;
                    break;
                }
                RingCrossing::Hit => break,
                RingCrossing::None => {}
            }
        }

        // Hitting something ends the plan (it shouldn't, but show it honestly).
        let hit = sys.bodies.iter().zip(&positions).any(|(b, p)| {
            let d = p.distance(ship.position);
            // Sphere test first; terrain only when inside the tallest mountains.
            !b.kind.artificial() && d < b.max_radius() && d < b.surface_radius_at(*p, ship.position, t)
        });
        if hit {
            break;
        }
    }
    out
}
