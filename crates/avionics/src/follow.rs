//! Follow: keep at a range from something, or orbit it, and stay with it as
//! it moves. The anchor is another ship (as the radar measures it: the
//! caller passes its position and velocity each frame) or a station or a
//! gate (from the charts). The program flies the ship the way a pilot
//! would: thrusters for small corrections with the nose on the anchor,
//! turning to burn the main engine when more is needed, braking in time to
//! stop where it should be. Its feedforward is the anchor's own acceleration
//! (estimated from its motion, as fire control does), less the gravity on
//! the ship, so it falls with the anchor and burns only for the difference.
//!
//! It holds the stick while engaged; any other autopilot taking over, leaving
//! the system, the hyperdrive, landing, or losing the anchor ends it.

use glam::DVec3;
use serde::{Deserialize, Serialize};
use universe_physics::Collider;
use universe_world::ship::{facing, Controls, ShipCommands};
use universe_world::StarSystem;

use crate::avionics::{target_position, Avionics};
use crate::bus::Bus;
use crate::docking::attitude;
use crate::events::Event;
use crate::fire_control::Track;
use crate::hunter::{avoid, thrust_for};
use crate::nav::NavTarget;

/// The ranges the pilot picks from (m).
pub const RANGES: [f64; 6] = [1_000.0, 2_000.0, 3_000.0, 5_000.0, 10_000.0, 20_000.0];
/// No closer than this to another ship (m).
const MIN_SHIP_RANGE: f64 = 500.0;
/// And this far beyond a station's or gate's structure (m).
const MIN_STRUCTURE_GAP: f64 = 1_000.0;
/// Fastest it closes or opens the range (m/s).
const MAX_CLOSING: f64 = 300.0;
/// Fastest orbit (m/s), and the share of the thrusters the turn may take.
const MAX_ORBIT: f64 = 150.0;
const ORBIT_ACCEL_SHARE: f64 = 0.4;
/// How hard it corrects its velocity (1/s).
const GAIN: f64 = 0.6;

/// What to follow.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum Anchor {
    /// A ship, by its id (the player's 0, craft i: i + 1).
    Ship(usize),
    /// A station or a gate.
    Place(NavTarget),
}

/// How to follow it.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum Manoeuvre {
    /// Hold station at this range (m), wherever it goes.
    KeepAt(f64),
    /// Circle it at this radius (m).
    Orbit(f64),
}

impl Manoeuvre {
    pub fn range(self) -> f64 {
        match self {
            Manoeuvre::KeepAt(r) | Manoeuvre::Orbit(r) => r,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Manoeuvre::KeepAt(_) => "KEEP AT",
            Manoeuvre::Orbit(_) => "ORBIT",
        }
    }
}

/// The follow program engaged.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct Follow {
    pub anchor: Anchor,
    pub manoeuvre: Manoeuvre,
    /// The orbit's plane (its normal), chosen when it starts.
    #[serde(default)]
    pub axis: Option<DVec3>,
    /// The track on the anchor (for its acceleration).
    #[serde(skip)]
    pub track: Option<Track>,
}

/// The closest a manoeuvre may be flown round `anchor` (m).
pub fn min_range(sys: &StarSystem, anchor: Anchor) -> f64 {
    match anchor {
        Anchor::Ship(_) => MIN_SHIP_RANGE,
        Anchor::Place(t) => {
            let size = match t {
                NavTarget::Station(b) | NavTarget::Gate(b) => match &sys.bodies[b].rail.collider {
                    Collider::Polytope(p) => p.bound * p.scale,
                    Collider::Ring(r) => r.radius + r.tube,
                    _ => 0.0,
                },
                NavTarget::Spaceport(_) => 0.0,
            };
            size + MIN_STRUCTURE_GAP
        }
    }
}

/// The preset range nearest `distance` (m), no closer than `min`.
pub fn nearest_range(distance: f64, min: f64) -> f64 {
    RANGES.iter().copied().filter(|&r| r >= min).min_by(|a, b| (a - distance).abs().total_cmp(&(b - distance).abs())).unwrap_or(min).max(min)
}

/// The next preset range out from `range` (back to the first past the last), no closer than `min`.
pub fn next_range(range: f64, min: f64) -> f64 {
    RANGES.iter().copied().filter(|&r| r >= min).find(|&r| r > range + 1.0).unwrap_or_else(|| RANGES[0].max(min))
}

/// Speed toward closing `gap` (m): as fast as it can still brake to a stop
/// there (`brake`, m/s^2), gentler close in; negative to open the range.
fn closing_speed(gap: f64, brake: f64) -> f64 {
    let s = (2.0 * brake * gap.abs()).sqrt().min(0.3 * gap.abs()).min(MAX_CLOSING);
    s.copysign(gap)
}

/// The orbit speed for radius `r` (m/s): the turn takes a share of the thrusters.
pub fn orbit_speed(r: f64, side_accel: f64) -> f64 {
    (ORBIT_ACCEL_SHARE * side_accel * r).sqrt().min(MAX_ORBIT)
}

impl Avionics {
    /// Start following `anchor` (or change how): any other autopilot stops.
    pub fn follow(&mut self, bus: &mut impl Bus, anchor: Anchor, manoeuvre: Manoeuvre, events: &mut Vec<Event>) {
        if !bus.ship().is_flying() || bus.ship().hyperdrive {
            events.push(Event::Refused { reason: "FOLLOW: FLYING IN NORMAL SPACE ONLY".into() });
            return;
        }
        if let Anchor::Place(NavTarget::Spaceport(_)) = anchor {
            events.push(Event::Refused { reason: "FOLLOW: SHIPS, STATIONS AND GATES ONLY".into() });
            return;
        }
        let sys = bus.star_system();
        let manoeuvre = match manoeuvre {
            Manoeuvre::KeepAt(r) => Manoeuvre::KeepAt(r.max(min_range(&sys, anchor))),
            Manoeuvre::Orbit(r) => Manoeuvre::Orbit(r.max(min_range(&sys, anchor))),
        };
        if let Some(c) = &mut self.clearance {
            c.autopilot = false;
        }
        self.route.active = false;
        self.hyper_autopilot = false;
        let same = self.following.is_some_and(|f| f.anchor == anchor);
        let (axis, track) = if same { self.following.map_or((None, None), |f| (f.axis, f.track)) } else { (None, None) };
        let axis = if matches!(manoeuvre, Manoeuvre::Orbit(_)) { axis } else { None };
        self.following = Some(Follow { anchor, manoeuvre, axis, track });
        events.push(Event::Following { what: Some((manoeuvre.label(), manoeuvre.range())) });
    }

    /// Stop following (the engines idle).
    pub fn stop_following(&mut self, bus: &mut impl Bus, events: &mut Vec<Event>) {
        if self.following.take().is_some() {
            self.set_controls(bus, events, |c| {
                c.rcs = DVec3::ZERO;
                c.throttle = 0.0;
            });
            events.push(Event::Following { what: None });
        }
    }

    /// The follow program for the frame: the devices' commands, and the
    /// stick (None if not following). `mark` is where an anchoring ship is
    /// and how it moves, if it's still in sight (a place is charted).
    pub fn follow_step(&mut self, bus: &mut impl Bus, mark: Option<(DVec3, DVec3)>, events: &mut Vec<Event>) -> Option<Controls> {
        let mut f = self.following?;
        let ship = bus.ship().clone();
        if !ship.is_flying() || ship.hyperdrive {
            self.following = None;
            events.push(Event::Following { what: None });
            return None;
        }
        let now = bus.time();
        let (sys, positions) = bus.positions();
        let mark = match f.anchor {
            Anchor::Ship(_) => mark,
            Anchor::Place(t) => target_position(bus, t).map(|p| (p, place_velocity(&sys, t, now))),
        };
        let Some((at, velocity)) = mark else {
            self.stop_following(bus, events);
            events.push(Event::Refused { reason: "FOLLOW: TARGET LOST".into() });
            return None;
        };
        let id = match f.anchor {
            Anchor::Ship(id) => id,
            Anchor::Place(t) => usize::MAX - place_body(t),
        };
        Track::update(&mut f.track, id, at, velocity, now);
        let anchor_accel = f.track.map_or(DVec3::ZERO, |t| t.acceleration);

        let r = at - ship.position;
        let d = r.length().max(1.0);
        let dir = r / d;
        let v_rel = ship.velocity - velocity;
        let brake = 0.5 * ship.main_accel();
        let range = f.manoeuvre.range();
        let mut desired = dir * closing_speed(d - range, brake);
        let mut accel = DVec3::ZERO;
        if let Manoeuvre::Orbit(_) = f.manoeuvre {
            // The plane: as it's moving now, if it is, else any.
            let axis = *f.axis.get_or_insert_with(|| {
                let n = (-dir).cross(v_rel);
                if n.length() > 1e-3 * v_rel.length().max(1.0) && v_rel.length() > 1.0 { n.normalize() } else { dir.any_orthonormal_vector() }
            });
            let out = -dir;
            let height = out.dot(axis) * d;
            let tangent = axis.cross(out).normalize_or(dir.any_orthonormal_vector());
            let speed = orbit_speed(range, ship.side_accel());
            desired += tangent * speed - axis * closing_speed(height, brake);
            // Centripetal: what turns the path round the anchor.
            accel += dir * (speed * speed / d);
        }
        let gravity = sys.gravity(ship.position, &positions);
        accel += (desired - v_rel) * GAIN + anchor_accel - gravity;
        accel += avoid(&sys, &ship, &positions, now);
        self.following = Some(f);

        // Small corrections on the thrusters, nose on the anchor; more, and it turns to burn.
        let (throttle, rcs, nose) = if accel.length() < 0.9 * ship.side_accel() {
            let (_, rcs, _) = thrust_for(&ship, accel, dir);
            (0.0, rcs, dir)
        } else {
            thrust_for(&ship, accel, dir)
        };
        let c = ShipCommands { throttle, rcs, ..ship.holding() };
        self.command(bus, &c, events);
        Some(attitude(&ship, facing(nose, ship.orientation * DVec3::Y), DVec3::ZERO, 1.0 / 60.0))
    }
}

/// The body of a station or gate.
fn place_body(t: NavTarget) -> usize {
    match t {
        NavTarget::Station(b) | NavTarget::Gate(b) => b,
        NavTarget::Spaceport(_) => unreachable!("follow anchors are stations and gates"),
    }
}

/// How a station or gate moves now.
fn place_velocity(sys: &StarSystem, t: NavTarget, now: f64) -> DVec3 {
    sys.velocity(place_body(t), now)
}
