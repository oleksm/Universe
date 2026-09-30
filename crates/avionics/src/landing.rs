//! Landing at planetary spaceports: the pad as guidance sees it, guidance,
//! trajectory prediction and the landing autopilot. (The spaceport and the
//! landing gear are the world's: `universe_world::spaceport`.)
//!
//! Guidance works in the planet's rotating frame, where the pad stands still.
//! Far out, it routes over the curve of the planet (never through it) to an
//! entry point above the pad, using the main engine. Close in, the ship turns
//! belly-down and descends vertically on its lift thrusters.

use glam::{DQuat, DVec3};
use universe_physics::{leapfrog, pull, segment_distance};
use universe_world::ship::{facing, upright};
use universe_world::{Ship, StarSystem, Terrain};

use crate::docking::{attitude, gain, Command, Guidance};
use crate::nav::{Clearance, Phase};

/// Height of the entry point above the pad, where the vertical descent starts (m).
pub const ENTRY_ALTITUDE: f64 = 3000.0;
/// Horizontal distance from the pad within which the descent phase applies (m).
const DESCENT_RADIUS: f64 = 2500.0;
/// Top speed while routing around the planet (m/s).
const ROUTE_SPEED: f64 = 3000.0;
/// Lowest cruise altitude when routing around the planet, as a fraction of its radius.
const MIN_ROUTE_ALTITUDE: f64 = 0.05;

/// A pad and its planet at one instant, in the system frame.
#[derive(Clone, Copy, Debug)]
pub struct PadFrame {
    pub body_center: DVec3,
    pub body_velocity: DVec3,
    pub body_radius: f64,
    pub mu: f64,
    pub angular_velocity: DVec3,
    /// Pad center, on the surface.
    pub pad: DVec3,
    /// Local vertical at the pad.
    pub up: DVec3,
    /// Body orientation now (for looking up terrain).
    pub rotation: DQuat,
    /// Highest terrain above the base radius (m).
    pub terrain_top: f64,
}

impl PadFrame {
    pub fn new(sys: &StarSystem, port: usize, t: f64, positions: &[DVec3]) -> Self {
        let sp = &sys.spaceports[port];
        let b = &sys.bodies[sp.body];
        let up = b.rotation(t) * sp.direction;
        let body_center = positions[sp.body];
        Self {
            body_center,
            body_velocity: sys.velocity(sp.body, t),
            body_radius: b.rail.radius,
            mu: b.rail.mu,
            angular_velocity: b.angular_velocity(),
            pad: body_center + up * b.rail.radius,
            up,
            rotation: b.rotation(t),
            terrain_top: b.max_radius() - b.rail.radius,
        }
    }

    /// Velocity of the planet's rotating frame at `p` (the ground, extended upward).
    pub fn frame_velocity(&self, p: DVec3) -> DVec3 {
        self.body_velocity + self.angular_velocity.cross(p - self.body_center)
    }

    pub fn surface_gravity(&self) -> f64 {
        self.mu / (self.body_radius * self.body_radius)
    }

    pub fn entry(&self) -> DVec3 {
        self.pad + self.up * ENTRY_ALTITUDE
    }

    /// Height above the pad (along its vertical) and horizontal offset from it.
    fn split(&self, pos: DVec3) -> (f64, DVec3) {
        let r = pos - self.pad;
        let h = r.dot(self.up);
        (h, r - self.up * h)
    }
}

/// Close enough above the pad to descend vertically.
pub fn in_descent_zone(pad: &PadFrame, pos: DVec3) -> bool {
    let (h, horiz) = pad.split(pos);
    horiz.length() < DESCENT_RADIUS && h < ENTRY_ALTITUDE * 1.5 && h > -50.0
}

/// Where to go and how fast, relative to the rotating ground.
/// `main_accel` is the main engine's acceleration (m/s^2); it sets the braking profile.
pub fn guidance(pad: &PadFrame, pos: DVec3, descent: bool, main_accel: f64) -> Guidance {
    let (h, horiz) = pad.split(pos);
    if descent {
        // Drift over the pad and sink, slowing to a gentle touchdown.
        let lateral = (-horiz * 0.08).clamp_length_max(40.0);
        let sink = (h / 15.0).clamp(1.5, 60.0);
        return Guidance { desired_velocity: lateral - pad.up * sink, waypoint: pad.pad, waypoint_dir: -pad.up, final_run: true };
    }

    let (c, radius) = (pad.body_center, pad.body_radius);
    let entry = pad.entry();
    // Is the way clear of the planet and its mountains? Check against a point
    // above the tallest terrain over the pad; the last stretch down to the
    // entry point is over the flattened ground around the port.
    // Over the flattened ground around the port there's nothing to clear.
    let above_terrain = entry + pad.up * pad.terrain_top;
    let over_port = horiz.length() < 30_000.0 && h > -1000.0;
    let clear = over_port || segment_distance(pos, above_terrain, c) > radius + pad.terrain_top + 500.0;
    let brake = 0.4 * (main_accel - pad.surface_gravity()).max(5.0);
    if clear {
        let d = entry - pos;
        let dist = d.length();
        let speed = (2.0 * brake * dist).sqrt().min(ROUTE_SPEED).min(dist * 0.5);
        let desired_velocity = if dist > 1.0 { d / dist * speed } else { DVec3::ZERO };
        return Guidance { desired_velocity, waypoint: entry, waypoint_dir: -pad.up, final_run: false };
    }

    // The straight line would cut through the planet: cruise along the great
    // circle toward the pad, holding altitude (at least a safe minimum), until
    // the direct path to the entry point clears the horizon.
    let from_c = pos - c;
    let r_ship = from_c.length();
    let ship_dir = from_c / r_ship;
    let altitude = r_ship - radius;
    let hold = altitude.max(MIN_ROUTE_ALTITUDE * radius);
    let angle = ship_dir.angle_between(pad.up);
    let axis = ship_dir.cross(pad.up).try_normalize().unwrap_or_else(|| ship_dir.any_orthonormal_vector());
    let along = axis.cross(ship_dir); // horizontal, toward the pad
    let remaining = angle * (radius + hold) + ENTRY_ALTITUDE;
    let speed = (2.0 * brake * remaining).sqrt().min(ROUTE_SPEED);
    let climb = ((hold - altitude) * 0.05).clamp(-200.0, 500.0);
    // Show the path a little way ahead along the circle.
    let ahead = DQuat::from_axis_angle(axis, angle.min(0.3)) * ship_dir;
    Guidance {
        desired_velocity: along * speed + ship_dir * climb,
        waypoint: c + ahead * (radius + hold),
        waypoint_dir: axis.cross(ahead),
        final_run: false,
    }
}

/// Numbers for the landing HUD.
#[derive(Clone, Debug)]
pub struct LandingStatus {
    pub phase: Phase,
    pub autopilot: bool,
    /// Straight-line distance to the pad (m).
    pub range: f64,
    /// Height above the pad (m).
    pub altitude: f64,
    /// Up is positive (m/s), relative to the ground.
    pub vertical_speed: f64,
    pub horizontal_speed: f64,
    pub horizontal_distance: f64,
    /// Angle between the ship's belly-up axis and the local vertical (radians).
    pub tilt: f64,
    /// Velocity relative to the rotating ground (m/s).
    pub relative_velocity: DVec3,
    pub guidance: Guidance,
    /// Free-fall trajectory drawn in the rotating frame (points in the system frame, now).
    pub prediction: Vec<DVec3>,
    /// Where that trajectory hits the ground, if it does.
    pub impact: Option<DVec3>,
    pub pad: PadFrame,
}

pub fn status(pad: &PadFrame, ship: &Ship, clearance: &Clearance, terrain: Option<&Terrain>) -> LandingStatus {
    let (altitude, horiz) = pad.split(ship.position);
    let v = ship.velocity - pad.frame_velocity(ship.position);
    let local_up = (ship.position - pad.body_center).normalize();
    let descent = if clearance.autopilot { clearance.phase == Phase::Descent } else { in_descent_zone(pad, ship.position) };
    let (prediction, impact) = predict(pad, ship, terrain);
    LandingStatus {
        phase: clearance.phase,
        autopilot: clearance.autopilot,
        range: ship.position.distance(pad.pad),
        altitude,
        vertical_speed: v.dot(pad.up),
        horizontal_speed: (v - pad.up * v.dot(pad.up)).length(),
        horizontal_distance: horiz.length(),
        tilt: (ship.orientation * DVec3::Y).angle_between(local_up),
        relative_velocity: v,
        guidance: guidance(pad, ship.position, descent, ship.main_accel()),
        prediction,
        impact,
        pad: *pad,
    }
}

/// Coast (no thrust) under the planet's gravity for a while, and express the
/// path in the rotating frame so it lines up with the ground as seen now.
pub fn predict(pad: &PadFrame, ship: &Ship, terrain: Option<&Terrain>) -> (Vec<DVec3>, Option<DVec3>) {
    let c = pad.body_center;
    let mut r = ship.position - c;
    let mut v = ship.velocity - pad.body_velocity;
    let altitude = r.length() - pad.body_radius;
    let dt = (altitude / 5000.0).clamp(0.25, 4.0);
    let mut points = vec![ship.position];
    let mut t = 0.0;
    for _ in 0..400 {
        leapfrog(&mut r, &mut v, dt, |r, _| pull(pad.mu, -r));
        t += dt;
        let back = DQuat::from_scaled_axis(-pad.angular_velocity * t);
        // Ground under the predicted point, in the body's frame.
        let ground = pad.body_radius + terrain.map_or(0.0, |tr| tr.surface(pad.rotation.inverse() * (back * r.normalize())));
        if r.length() < ground {
            let hit = c + back * (r.normalize() * ground);
            points.push(hit);
            return (points, Some(hit));
        }
        points.push(c + back * r);
    }
    (points, None)
}

/// The landing autopilot. `gravity` is the gravitational acceleration at the
/// ship. Its command holds for `h` seconds.
pub fn autopilot(pad: &PadFrame, ship: &Ship, gravity: DVec3, phase: Phase, h: f64) -> Command {
    let pos = ship.position;
    let v = ship.velocity - pad.frame_velocity(pos);
    let (_, horiz) = pad.split(pos);
    let phase = match phase {
        Phase::Descent if !in_descent_zone(pad, pos) && horiz.length() > 2.0 * DESCENT_RADIUS => Phase::Approach,
        Phase::Descent => Phase::Descent,
        _ if in_descent_zone(pad, pos) && v.length() < 80.0 => Phase::Descent,
        _ => Phase::Approach,
    };
    let g = guidance(pad, pos, phase == Phase::Descent, ship.main_accel());
    // Thrust needed: close the velocity error, and hold the ship up against gravity.
    let accel = (g.desired_velocity - v) * gain(0.8, h) - gravity;

    if phase == Phase::Descent {
        // Belly down (lift thrusters toward the ground), keep the current heading.
        let up = (pos - pad.body_center).normalize();
        let target = upright(up, ship.forward());
        let controls = attitude(ship, target, pad.angular_velocity, h);
        let rcs = ship.thruster_command(ship.orientation.inverse() * accel);
        return Command { controls, throttle: 0.0, rcs, phase, attitude: target };
    }
    // Point the main engine along the needed acceleration and burn once lined up.
    let dir = accel.try_normalize().unwrap_or_else(|| ship.forward());
    // Roll: keep the ship's top pointing away from the planet.
    let target = facing(dir, pos - pad.body_center);
    let controls = attitude(ship, target, DVec3::ZERO, h);
    let aligned = ship.forward().dot(dir) > 0.97;
    let throttle = if aligned { (accel.length() / ship.main_accel()).min(1.0) } else { 0.0 };
    // Thrusters trim whatever the main engine isn't covering.
    let residual = accel - ship.forward() * (throttle * ship.main_accel());
    let rcs = ship.thruster_command(ship.orientation.inverse() * residual);
    Command { controls, throttle, rcs, phase, attitude: target }
}
