//! Crew: a person aboard a ship, who can leave the pilot's seat, walk
//! through the ship, and step out onto the ground once it has landed.
//!
//! A person walks on what's really there (`walk`): a modelled hull's own
//! surfaces inside and out (its carved spaces, its floors), the ground,
//! buildings, other ships.
//!
//! Where a person is:
//! - in the seat (flying the ship);
//! - aboard a ship in flight: standing in it, in its frame, on magnetic boots
//!   (a coasting ship is in free fall; the boots hold you to its floors). The
//!   ship flies on with whatever its devices were last told, autopilot included;
//! - outside: in a body's rotating frame, under its real gravity (walk, run,
//!   jump), kept out of the sea. A landed ship is part of that ground: walking
//!   about in it, you're outside in this sense.
//!
//! The hatch opens only on a ship resting on a body's surface (not docked in
//! a station, not in flight: no spacewalks yet).

use glam::{DQuat, DVec3};
use serde::{Deserialize, Serialize};

use crate::ship::{facing, Ship, ShipState};
use crate::system::{BodyKind, StarSystem};
use crate::walk::{Collider, Stride, Walker};

/// Eye height above the feet (m).
pub const EYE: f64 = 1.7;
/// Walking and running speed (m/s).
pub const WALK: f64 = 1.6;
pub const RUN: f64 = 4.5;
/// Take-off speed of a jump (m/s).
pub const JUMP: f64 = 3.0;
/// How close to something you have to be to use it (m).
pub const REACH: f64 = 1.6;
/// What the boots hold you to a floor with, aboard in flight (m/s²: as a world's gravity).
const BOOTS: f64 = 9.81;
/// Climbing a ladder (m/s).
pub const CLIMB: f64 = 1.0;

/// Where a person is.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub enum Place {
    /// In the pilot's seat.
    #[default]
    Seat,
    /// Standing in the ship in flight: feet at `position` (ship frame),
    /// moving at `velocity` (the same), looking `yaw` (about the ship's up
    /// axis, 0 = forward) and `pitch` (rad).
    Aboard {
        position: DVec3,
        #[serde(default)]
        velocity: DVec3,
        yaw: f64,
        pitch: f64,
    },
    /// On `body`: feet at `position` in its rotating frame, moving at
    /// `velocity` (same frame), looking `yaw` (from the local north, toward
    /// east) and `pitch`.
    Outside { body: usize, position: DVec3, velocity: DVec3, yaw: f64, pitch: f64 },
}

/// A person: for now, the pilot.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Person {
    pub place: Place,
}

/// What the person does this frame.
#[derive(Clone, Copy, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct WalkCommands {
    /// -1..1: back/forward and left/right.
    pub forward: f64,
    pub right: f64,
    pub run: bool,
    pub jump: bool,
    /// Turn the head (rad): left is positive yaw, up is positive pitch.
    pub yaw: f64,
    pub pitch: f64,
    /// Use what's in reach: the seat, the hatch.
    pub interact: bool,
}

/// What happened to a person.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum CrewEvent {
    StoodUp,
    SatDown,
    SteppedOutside { body: String },
    CameAboard,
    /// The hatch won't open, and why.
    HatchRefused { reason: String },
}

/// What the person could use now (for the pilot's prompt).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Reach {
    Seat,
    Hatch,
    Ramp,
    /// The vending machine at spaceport `0`'s pads.
    Vending(usize),
}

/// The local north and east on a body at `up` (its frame): north toward the
/// body's +Y pole.
fn tangent(up: DVec3) -> (DVec3, DVec3) {
    let north = (DVec3::Y - up * up.y).try_normalize().unwrap_or_else(|| (DVec3::Z - up * up.z).normalize());
    let east = north.cross(up);
    (north, east)
}

/// Can the ship's hatch be opened (resting on a body's surface)? The body, if so.
pub fn hatch_body(sys: &StarSystem, ship: &Ship) -> Result<usize, String> {
    match ship.state {
        ShipState::Landed { body, .. } if sys.bodies[body].kind == BodyKind::Station => Err("DOCKED - NO GANGWAY YET".into()),
        ShipState::Landed { body, .. } => Ok(body),
        _ => Err("IN FLIGHT - NO SPACEWALKS".into()),
    }
}

/// How far aft a stair from a hull's own `hatch` runs to the ground (m).
const STAIR_RUN: f64 = 8.0;

/// The way out of `ship`, landed (ship frame): the top of its stair and
/// where its foot reaches, horizontally (it meets the ground below that).
/// A modelled hull's belly hatch, a stair from it the way the hatch points
/// (down its ramp), or aft; otherwise from its port side, amidships.
pub fn stair(ship: &Ship) -> (DVec3, DVec3) {
    let shape = ship.spec().shape();
    match shape.nodes(crate::shape::Role::Hatch).next() {
        Some(h) => {
            // (Down its ramp, the way the node points; straight down, aft.)
            let along = DVec3::new(h.dir.x, 0.0, h.dir.z).try_normalize().unwrap_or(DVec3::Z);
            (h.at, h.at + along * STAIR_RUN)
        }
        None => {
            let (lo, _) = shape.mesh.extent();
            (DVec3::new(lo.x, lo.y, 0.0), DVec3::new(lo.x - STAIR_RUN, lo.y, 0.0))
        }
    }
}

/// The ship's surfaces to walk on, if it's modelled.
fn walk_mesh(ship: &Ship) -> Option<&'static crate::walk::WalkMesh> {
    ship.spec().shape().walk.as_deref()
}

/// The floor under a point of `ship` (its frame), within `below` m: where feet would stand.
fn floor_under(ship: &Ship, at: DVec3, below: f64) -> Option<DVec3> {
    let mesh = walk_mesh(ship)?;
    let colliders = [Collider::Mesh { mesh, at: DVec3::ZERO, rot: DQuat::IDENTITY }];
    crate::walk::ray(&colliders, at, DVec3::NEG_Y, below).filter(|&(_, n)| n.y > crate::walk::SLOPE).map(|(d, _)| at - DVec3::Y * d)
}

/// Where you stand getting up from the seat (ship frame): the floor under it.
pub fn beside_seat(ship: &Ship) -> Option<DVec3> {
    let seat = ship.spec().shape().nodes(crate::shape::Role::Cockpit).next()?;
    floor_under(ship, seat.at, 30.0)
}

/// Where you stand coming in by the hatch (ship frame): the floor at its
/// top (a ramp's: a step down it, swung as far as `ramp`).
pub fn inside_hatch(ship: &Ship, ramp: f64) -> Option<DVec3> {
    let (top, foot) = stair(ship);
    let out = DVec3::new(foot.x - top.x, 0.0, foot.z - top.z).normalize_or_zero();
    let at = top + out * if walks_out(ship) { 0.8 } else { 0.0 } + DVec3::Y * 0.5;
    let mut colliders = Vec::new();
    ship_colliders(ship, DVec3::ZERO, DQuat::IDENTITY, ramp, &mut colliders);
    crate::walk::ray(&colliders, at, DVec3::NEG_Y, 3.0).filter(|&(_, n)| n.y > crate::walk::SLOPE).map(|(d, _)| at - DVec3::Y * d)
}

/// Where `ship` is in `body`'s rotating frame (its centre, its turn).
fn placed(sys: &StarSystem, ship: &Ship, body: usize, t: f64, center: DVec3) -> (DVec3, DQuat) {
    let inv = sys.bodies[body].rotation(t).inverse();
    (inv * (ship.position - center), inv * ship.orientation)
}

/// Where the ramp's foot is on `body` (its frame, on the ground), for a
/// ship landed on it.
fn ramp_foot(sys: &StarSystem, ship: &Ship, body: usize, t: f64, center: DVec3) -> DVec3 {
    let (at, rot) = placed(sys, ship, body, t, center);
    let dir = (at + rot * stair(ship).1).normalize();
    dir * sys.bodies[body].surface_radius(dir)
}

/// Does `ship` have a ramp to walk down (and up), rather than a hatch you use?
pub fn walks_out(ship: &Ship) -> bool {
    ship.spec().shape().ramp.is_some()
}

/// Is `p` (ship frame) within `ship`'s bounds?
fn within(ship: &Ship, p: DVec3) -> bool {
    walk_mesh(ship).is_some_and(|m| p.cmpge(m.lo).all() && p.cmple(m.hi).all())
}

/// How far down `ship`'s ramp is swung (rad): landed where its hatch opens,
/// till its end meets the ground (level with its feet); else shut.
pub fn ramp_angle(sys: &StarSystem, ship: &Ship) -> f64 {
    let shape = ship.spec().shape();
    match &shape.ramp {
        Some(r) if hatch_body(sys, ship).is_ok() => {
            let ground = shape.nodes(crate::shape::Role::Gear).map(|g| g.at.y).fold(f64::INFINITY, f64::min);
            if !ground.is_finite() || r.length <= 0.0 {
                return 0.0;
            }
            ((r.hinge.y - ground) / r.length).clamp(0.0, 1.0).asin()
        }
        _ => 0.0,
    }
}

/// The ship as something to walk on and bump into, placed (`at`, `rot`),
/// its ramp swung down by `ramp` (rad): its own surfaces, or failing that
/// its convex parts.
pub fn ship_colliders(ship: &Ship, at: DVec3, rot: DQuat, ramp: f64, out: &mut Vec<Collider<'_>>) {
    let shape = ship.spec().shape();
    if let Some(r) = &shape.ramp {
        let turn = r.turn(ramp);
        out.push(Collider::Mesh { mesh: &r.walk, at: at + rot * (r.hinge - turn * r.hinge), rot: rot * turn });
    }
    match walk_mesh(ship) {
        Some(mesh) => out.push(Collider::Mesh { mesh, at, rot }),
        None => {
            for (k, planes) in shape.solids.iter().enumerate() {
                out.push(Collider::Convex { planes, at, rot, centre: shape.parts[k].centre, radius: shape.parts[k].radius });
            }
        }
    }
}

impl Person {
    pub fn seated(&self) -> bool {
        self.place == Place::Seat
    }

    /// What's in reach to use.
    pub fn reach(&self, sys: &StarSystem, ship: &Ship, t: f64, positions: &[DVec3]) -> Option<Reach> {
        let near = |feet: DVec3, spot: Option<DVec3>| spot.is_some_and(|s| DVec3::new(feet.x - s.x, 0.0, feet.z - s.z).length() < REACH && (feet.y - s.y).abs() < 1.0);
        match self.place {
            Place::Seat => None,
            Place::Aboard { position, .. } => near(position, beside_seat(ship)).then_some(Reach::Seat).or_else(|| (!walks_out(ship) && near(position, inside_hatch(ship, 0.0))).then_some(Reach::Hatch)),
            Place::Outside { body, position, .. } => {
                if hatch_body(sys, ship) == Ok(body) {
                    // In the ship: its seat, its hatch; outside it: the foot of its stair.
                    let (at, rot) = placed(sys, ship, body, t, positions[body]);
                    let local = rot.inverse() * (position - at);
                    if near(local, beside_seat(ship)) {
                        return Some(Reach::Seat);
                    }
                    if !walks_out(ship) {
                        if near(local, inside_hatch(ship, ramp_angle(sys, ship))) {
                            return Some(Reach::Hatch);
                        }
                        if position.distance(ramp_foot(sys, ship, body, t, positions[body])) < REACH * 2.0 {
                            return Some(Reach::Ramp);
                        }
                    }
                }
                // A spaceport's vending machine, in the middle of its pads.
                (0..sys.spaceports.len()).filter(|&p| sys.spaceports[p].body == body).find(|&p| {
                    let d = crate::spaceport::vending_direction(sys, p);
                    position.distance(d * sys.bodies[body].surface_radius(d)) < REACH * 1.5
                }).map(Reach::Vending)
            }
        }
    }

    /// Where the eyes are (world) and which way they look, for a ship whose
    /// pilot's view is `seat_eye` (world) when seated.
    pub fn eye(&self, sys: &StarSystem, ship: &Ship, t: f64, positions: &[DVec3], seat_eye: DVec3) -> (DVec3, DQuat) {
        match self.place {
            Place::Seat => (seat_eye, ship.orientation),
            Place::Aboard { position, yaw, pitch, .. } => {
                let local = position + DVec3::Y * EYE;
                (ship.position + ship.orientation * local, ship.orientation * DQuat::from_rotation_y(yaw) * DQuat::from_rotation_x(pitch))
            }
            Place::Outside { body, position, yaw, pitch, .. } => {
                let b = &sys.bodies[body];
                let rot = b.rotation(t);
                let up = position.normalize();
                let (north, east) = tangent(up);
                let ahead = north * yaw.cos() - east * yaw.sin();
                let look = facing(ahead, up) * DQuat::from_rotation_x(pitch);
                (positions[body] + rot * (position + up * EYE), rot * look)
            }
        }
    }

    /// Standing at `feet` (ship frame) facing the ship's `yaw`: outside (on
    /// the world it rests on) if it has landed on one; aboard otherwise (in
    /// flight, or docked on a station's deck: the ship's frame, on boots).
    pub fn stand(&mut self, sys: &StarSystem, ship: &Ship, t: f64, positions: &[DVec3], feet: DVec3, yaw: f64) {
        self.place = match hatch_body(sys, ship) {
            Ok(body) => {
                let (at, rot) = placed(sys, ship, body, t, positions[body]);
                let position = at + rot * feet;
                Place::Outside { body, position, velocity: DVec3::ZERO, yaw: body_yaw(position, rot * DQuat::from_rotation_y(yaw) * DVec3::NEG_Z), pitch: 0.0 }
            }
            Err(_) => Place::Aboard { position: feet, velocity: DVec3::ZERO, yaw, pitch: 0.0 },
        };
    }

    /// One frame of `dt` real seconds: walk, turn, jump, and use what's in
    /// reach. The ship is where the world has just put it; `around` is what
    /// else stands near it (buildings, other ships: the body's frame);
    /// `layout` its inside as laid out (its frame), if it has one: walked on,
    /// its ladders climbed.
    #[allow(clippy::too_many_arguments)]
    pub fn step(&mut self, sys: &StarSystem, ship: &Ship, t: f64, positions: &[DVec3], around: &[Collider], layout: Option<&crate::deckplan::Walkable>, c: &WalkCommands, dt: f64, events: &mut Vec<CrewEvent>) {
        // A ship landed on a world is part of its ground (walked about in its
        // frame); one taking off carries whoever stands in it (its frame), as
        // does one docked on a station's deck. (Nobody stands outside on a
        // station: put back in the seat.)
        if let Place::Outside { body, .. } = self.place
            && sys.bodies[body].kind == BodyKind::Station
        {
            self.place = Place::Seat;
        }
        match self.place {
            Place::Aboard { position, yaw, .. } if hatch_body(sys, ship).is_ok() => {
                let pitch = if let Place::Aboard { pitch, .. } = self.place { pitch } else { 0.0 };
                self.stand(sys, ship, t, positions, position, yaw);
                if let Place::Outside { pitch: p, .. } = &mut self.place {
                    *p = pitch;
                }
            }
            Place::Outside { body, position, pitch, .. } if hatch_body(sys, ship) != Ok(body) => {
                let (at, rot) = placed(sys, ship, body, t, positions[body]);
                let local = rot.inverse() * (position - at);
                if within(ship, local) {
                    self.place = Place::Aboard { position: local, velocity: DVec3::ZERO, yaw: 0.0, pitch };
                }
            }
            _ => {}
        }
        let reach = self.reach(sys, ship, t, positions);
        let speed = if c.run { RUN } else { WALK };
        let jump = if c.jump { JUMP } else { 0.0 };
        // On a ladder, forward climbs: up looking level or up, down looking down.
        let looking_down = matches!(self.place, Place::Aboard { pitch, .. } | Place::Outside { pitch, .. } if pitch < -0.35);
        let climb = c.forward * CLIMB * if looking_down { -1.0 } else { 1.0 };
        match &mut self.place {
            Place::Seat => {
                if c.interact {
                    match beside_seat(ship) {
                        Some(feet) => {
                            self.stand(sys, ship, t, positions, feet, 0.0);
                            events.push(CrewEvent::StoodUp);
                        }
                        // (No room modelled to stand in: out by the hatch, if it opens.)
                        None => match hatch_body(sys, ship) {
                            Ok(body) => {
                                self.go_out(sys, ship, t, positions, body);
                                events.push(CrewEvent::SteppedOutside { body: sys.bodies[body].name.clone() });
                            }
                            Err(_) => events.push(CrewEvent::HatchRefused { reason: "NO ROOM TO STAND".into() }),
                        },
                    }
                }
            }
            Place::Aboard { position, velocity, yaw, pitch } => {
                *yaw += c.yaw;
                *pitch = (*pitch + c.pitch).clamp(-1.4, 1.4);
                let (s, co) = yaw.sin_cos();
                let (fwd, right) = (DVec3::new(-s, 0.0, -co), DVec3::new(co, 0.0, -s));
                let wish = (fwd * c.forward + right * c.right).clamp_length_max(1.0) * speed;
                let mut colliders: Vec<Collider> = Vec::new();
                ship_colliders(ship, DVec3::ZERO, DQuat::IDENTITY, ramp_angle(sys, ship), &mut colliders);
                if let Some(l) = layout {
                    colliders.push(Collider::Mesh { mesh: &l.mesh, at: DVec3::ZERO, rot: DQuat::IDENTITY });
                }
                let mut w = Walker { feet: *position, velocity: *velocity };
                let climbable = |p: DVec3| layout.is_some_and(|l| l.climbing(p));
                w.step(&colliders, &|_| DVec3::Y, BOOTS, &climbable, &Stride { wish, jump, climb }, dt);
                *position = w.feet;
                *velocity = w.velocity;
                if c.interact {
                    match reach {
                        Some(Reach::Seat) => {
                            self.place = Place::Seat;
                            events.push(CrewEvent::SatDown);
                        }
                        Some(Reach::Hatch) => {
                            if let Err(reason) = hatch_body(sys, ship) {
                                events.push(CrewEvent::HatchRefused { reason });
                            }
                        }
                        _ => {}
                    }
                }
            }
            Place::Outside { body, position, velocity, yaw, pitch } => {
                let b = &sys.bodies[*body];
                *yaw += c.yaw;
                *pitch = (*pitch + c.pitch).clamp(-1.4, 1.4);
                let up = position.normalize();
                let (north, east) = tangent(up);
                let (s, co) = yaw.sin_cos();
                let (fwd, right) = (north * co - east * s, east * co + north * s);
                let wish = (fwd * c.forward + right * c.right).clamp_length_max(1.0) * speed;
                // The ground, the ship (if it rests here), what stands about.
                let radius = |d: DVec3| b.surface_radius(d);
                let mut colliders: Vec<Collider> = vec![Collider::Ground(&radius)];
                if hatch_body(sys, ship) == Ok(*body) {
                    let (at, rot) = placed(sys, ship, *body, t, positions[*body]);
                    let mut own = Vec::new();
                    ship_colliders(ship, at, rot, ramp_angle(sys, ship), &mut own);
                    colliders.extend(own);
                    if let Some(l) = layout {
                        colliders.push(Collider::Mesh { mesh: &l.mesh, at, rot });
                    }
                }
                colliders.extend(around.iter().copied());
                let r = position.length();
                let g = b.rail.mu / (r * r);
                let before = *position;
                let mut w = Walker { feet: *position, velocity: *velocity };
                // (The ship's ladders, where it rests: in its frame.)
                let (ship_at, ship_rot) = placed(sys, ship, *body, t, positions[*body]);
                let landed_here = hatch_body(sys, ship) == Ok(*body);
                let climbable = |p: DVec3| landed_here && layout.is_some_and(|l| l.climbing(ship_rot.inverse() * (p - ship_at)));
                w.step(&colliders, &|p: DVec3| p.normalize(), g, &climbable, &Stride { wish, jump, climb }, dt);
                // Not into the sea.
                if b.terrain.as_ref().is_some_and(|tr| universe_physics::Surface::liquid(tr, w.feet.normalize())) {
                    let up2 = w.feet.normalize();
                    w.feet = before.normalize() * w.feet.dot(up2);
                    w.velocity = w.feet.normalize() * w.velocity.dot(up2);
                }
                *position = w.feet;
                *velocity = w.velocity;
                if c.interact {
                    match reach {
                        Some(Reach::Seat) => {
                            self.place = Place::Seat;
                            events.push(CrewEvent::SatDown);
                        }
                        Some(Reach::Hatch) => {
                            let body = *body;
                            self.go_out(sys, ship, t, positions, body);
                            events.push(CrewEvent::SteppedOutside { body: sys.bodies[body].name.clone() });
                        }
                        Some(Reach::Ramp) => {
                            // In by the hatch, facing into the ship, away from the stair.
                            if let Some(feet) = inside_hatch(ship, ramp_angle(sys, ship)) {
                                let (top, bottom) = stair(ship);
                                let inward = DVec3::new(top.x - bottom.x, 0.0, top.z - bottom.z);
                                self.stand(sys, ship, t, positions, feet, f64::atan2(-inward.x, -inward.z));
                                events.push(CrewEvent::CameAboard);
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
    }

    /// Out of the hatch and down the stair: at its foot, facing away from the ship.
    fn go_out(&mut self, sys: &StarSystem, ship: &Ship, t: f64, positions: &[DVec3], body: usize) {
        let foot = ramp_foot(sys, ship, body, t, positions[body]);
        let (_, rot) = placed(sys, ship, body, t, positions[body]);
        let (top, bottom) = stair(ship);
        let out = rot * DVec3::new(bottom.x - top.x, 0.0, bottom.z - top.z).normalize_or(DVec3::NEG_X);
        self.place = Place::Outside { body, position: foot, velocity: DVec3::ZERO, yaw: body_yaw(foot, out), pitch: 0.0 };
    }
}

/// The yaw (from north toward east) of direction `d` at `position` on a body.
fn body_yaw(position: DVec3, d: DVec3) -> f64 {
    let (north, east) = tangent(position.normalize());
    f64::atan2(-d.dot(east), d.dot(north))
}
