//! Crew: a person aboard a ship, who can leave the pilot's seat, walk
//! through the ship, and step out onto the ground once it has landed.
//!
//! Where a person is:
//! - in the seat (flying the ship);
//! - aboard: standing in the ship, in its frame, on magnetic boots (a coasting
//!   ship is in free fall; the boots hold you to the deck). The ship flies on
//!   with whatever its devices were last told, autopilot included;
//! - outside: on a body's surface, in the body's rotating frame, under its
//!   real gravity (walk, run, jump), kept out of the sea.
//!
//! The hatch opens only on a ship resting on a body's surface (not docked in
//! a station, not in flight: no spacewalks yet).

use glam::{DQuat, DVec3};
use serde::{Deserialize, Serialize};

use crate::ship::{facing, Ship, ShipState};
use crate::system::{BodyKind, StarSystem};

/// The deck, in the ship's frame (y, m): feet stand on it.
pub const DECK: f64 = -2.4;
/// Headroom above the deck (m).
pub const HEADROOM: f64 = 3.2;
/// Eye height above the feet (m).
pub const EYE: f64 = 1.7;
/// How wide a person is (m), for walls.
pub const BODY_RADIUS: f64 = 0.35;
/// Walking and running speed (m/s).
pub const WALK: f64 = 1.6;
pub const RUN: f64 = 4.5;
/// Take-off speed of a jump (m/s).
pub const JUMP: f64 = 3.0;
/// How close to something you have to be to use it (m).
pub const REACH: f64 = 1.6;

/// A room of the ship's interior: a box from the deck up, `x0..x1` by
/// `z0..z1` in the ship's frame (m; -Z is forward).
#[derive(Clone, Copy, Debug)]
pub struct Room {
    pub x0: f64,
    pub x1: f64,
    pub z0: f64,
    pub z1: f64,
}

/// The ship's interior: the cockpit at the front, a corridor, the cabin with
/// the hatch in its port wall.
pub const ROOMS: [Room; 3] = [
    Room { x0: -3.0, x1: 3.0, z0: -13.0, z1: -6.0 },
    Room { x0: -1.1, x1: 1.1, z0: -6.0, z1: 2.0 },
    Room { x0: -5.0, x1: 5.0, z0: 2.0, z1: 10.0 },
];
/// The pilot's seat, and where you stand when you get up from it (deck, ship frame).
pub const SEAT: DVec3 = DVec3::new(0.0, DECK, -10.5);
pub const BESIDE_SEAT: DVec3 = DVec3::new(0.0, DECK, -9.0);
/// The hatch (in the cabin's port wall, x = -5), from inside.
pub const HATCH: DVec3 = DVec3::new(-4.6, DECK, 6.0);
/// Where the ramp from the hatch meets the ground, beside the ship (ship
/// frame, horizontally; clear of the wing).
pub const RAMP_FOOT: DVec3 = DVec3::new(-15.0, DECK, 6.0);

/// Where a person is.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub enum Place {
    /// In the pilot's seat.
    #[default]
    Seat,
    /// Standing in the ship: feet at `position` (ship frame), looking `yaw`
    /// (about the ship's up axis, 0 = forward) and `pitch` (rad).
    Aboard { position: DVec3, yaw: f64, pitch: f64 },
    /// On the surface of `body`: feet at `position` in its rotating frame,
    /// moving at `velocity` (same frame), looking `yaw` (from the local
    /// north, toward east) and `pitch`.
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
}

/// Inside the interior, keeping `BODY_RADIUS` from the walls?
fn inside(p: DVec3) -> bool {
    ROOMS.iter().any(|r| p.x >= r.x0 + BODY_RADIUS && p.x <= r.x1 - BODY_RADIUS && p.z >= r.z0 + BODY_RADIUS && p.z <= r.z1 - BODY_RADIUS)
        || ROOMS.windows(2).any(|w| {
            // Doorways between rooms: the narrower room's width carries through the boundary.
            let (a, b) = (w[0], w[1]);
            let (x0, x1) = (a.x0.max(b.x0), a.x1.min(b.x1));
            let z = a.z1;
            p.x >= x0 + BODY_RADIUS && p.x <= x1 - BODY_RADIUS && (p.z - z).abs() <= BODY_RADIUS + 0.01
        })
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

/// Where the ramp's foot is on `body` (its frame, on the ground), for a
/// ship landed on it.
fn ramp_foot(sys: &StarSystem, ship: &Ship, body: usize, t: f64, center: DVec3) -> DVec3 {
    let b = &sys.bodies[body];
    let world = ship.position + ship.orientation * RAMP_FOOT;
    let local = b.rotation(t).inverse() * (world - center);
    let dir = local.normalize();
    dir * b.surface_radius(dir)
}

impl Person {
    pub fn seated(&self) -> bool {
        self.place == Place::Seat
    }

    /// What's in reach to use.
    pub fn reach(&self, sys: &StarSystem, ship: &Ship, t: f64, positions: &[DVec3]) -> Option<Reach> {
        match self.place {
            Place::Seat => None,
            Place::Aboard { position, .. } => {
                if position.distance(BESIDE_SEAT) < REACH {
                    Some(Reach::Seat)
                } else if position.distance(HATCH) < REACH {
                    Some(Reach::Hatch)
                } else {
                    None
                }
            }
            Place::Outside { body, position, .. } => {
                let foot = ramp_foot(sys, ship, body, t, positions[body]);
                (hatch_body(sys, ship) == Ok(body) && position.distance(foot) < REACH * 2.0).then_some(Reach::Ramp)
            }
        }
    }

    /// Where the eyes are (world) and which way they look, for a ship whose
    /// pilot's view is `seat_eye` (world) when seated.
    pub fn eye(&self, sys: &StarSystem, ship: &Ship, t: f64, positions: &[DVec3], seat_eye: DVec3) -> (DVec3, DQuat) {
        match self.place {
            Place::Seat => (seat_eye, ship.orientation),
            Place::Aboard { position, yaw, pitch } => {
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

    /// One frame of `dt` real seconds: walk, turn, jump, and use what's in
    /// reach. The ship is where the world has just put it.
    #[allow(clippy::too_many_arguments)]
    pub fn step(&mut self, sys: &StarSystem, ship: &Ship, t: f64, positions: &[DVec3], c: &WalkCommands, dt: f64, events: &mut Vec<CrewEvent>) {
        let reach = self.reach(sys, ship, t, positions);
        let speed = if c.run { RUN } else { WALK };
        match &mut self.place {
            Place::Seat => {
                if c.interact {
                    self.place = Place::Aboard { position: BESIDE_SEAT, yaw: 0.0, pitch: 0.0 };
                    events.push(CrewEvent::StoodUp);
                }
            }
            Place::Aboard { position, yaw, pitch } => {
                *yaw += c.yaw;
                *pitch = (*pitch + c.pitch).clamp(-1.4, 1.4);
                // On the deck: move along it, sliding along walls.
                let (s, co) = yaw.sin_cos();
                let (fwd, right) = (DVec3::new(-s, 0.0, -co), DVec3::new(co, 0.0, -s));
                let step = (fwd * c.forward + right * c.right).clamp_length_max(1.0) * speed * dt;
                for d in [step, DVec3::new(step.x, 0.0, 0.0), DVec3::new(0.0, 0.0, step.z)] {
                    if inside(*position + d) {
                        *position += d;
                        break;
                    }
                }
                if c.interact {
                    match reach {
                        Some(Reach::Seat) => {
                            self.place = Place::Seat;
                            events.push(CrewEvent::SatDown);
                        }
                        Some(Reach::Hatch) => match hatch_body(sys, ship) {
                            Ok(body) => {
                                let foot = ramp_foot(sys, ship, body, t, positions[body]);
                                // Face away from the ship (outward along the ramp).
                                let rot = sys.bodies[body].rotation(t);
                                let out = rot.inverse() * (ship.orientation * DVec3::NEG_X);
                                let up = foot.normalize();
                                let (north, east) = tangent(up);
                                let yaw = f64::atan2(-out.dot(east), out.dot(north));
                                self.place = Place::Outside { body, position: foot, velocity: DVec3::ZERO, yaw, pitch: 0.0 };
                                events.push(CrewEvent::SteppedOutside { body: sys.bodies[body].name.clone() });
                            }
                            Err(reason) => events.push(CrewEvent::HatchRefused { reason }),
                        },
                        _ => {}
                    }
                }
            }
            Place::Outside { body, position, velocity, yaw, pitch } => {
                let b = &sys.bodies[*body];
                *yaw += c.yaw;
                *pitch = (*pitch + c.pitch).clamp(-1.4, 1.4);
                let up = position.normalize();
                let r = position.length();
                let ground = b.surface_radius(up);
                let grounded = r <= ground + 0.05;
                let (north, east) = tangent(up);
                let (s, co) = yaw.sin_cos();
                let (fwd, right) = (north * co - east * s, east * co + north * s);
                // Feet on the ground steer; in the air, momentum carries.
                let mut v_up = velocity.dot(up);
                let mut v_side = *velocity - up * v_up;
                if grounded {
                    v_side = (fwd * c.forward + right * c.right).clamp_length_max(1.0) * speed;
                    v_up = v_up.max(0.0);
                    if c.jump {
                        v_up = JUMP;
                    }
                }
                let g = b.rail.mu / (r * r);
                v_up -= g * dt;
                let mut next = *position + (v_side + up * v_up) * dt;
                // Not into the sea.
                let dir = next.normalize();
                if b.terrain.as_ref().is_some_and(|tr| universe_physics::Surface::liquid(tr, dir)) {
                    next = *position + up * (v_up * dt);
                    v_side = DVec3::ZERO;
                }
                let dir = next.normalize();
                let floor = b.surface_radius(dir);
                if next.length() < floor {
                    next = dir * floor;
                    v_up = v_up.max(0.0);
                }
                let up2 = next.normalize();
                *velocity = v_side - up2 * v_side.dot(up2) + up2 * v_up;
                *position = next;
                if c.interact && reach == Some(Reach::Ramp) {
                    // In through the port hatch, facing into the cabin (+X), not back at it.
                    self.place = Place::Aboard { position: HATCH, yaw: -std::f64::consts::FRAC_PI_2, pitch: 0.0 };
                    events.push(CrewEvent::CameAboard);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testkit::Probe;

    fn walk(forward: f64) -> WalkCommands {
        WalkCommands { forward, ..Default::default() }
    }
    fn use_it() -> WalkCommands {
        WalkCommands { interact: true, ..Default::default() }
    }

    #[test]
    fn step_out_on_a_planet_walk_jump_and_come_back_aboard() {
        let mut p = Probe::new(42);
        let sys = p.sys();
        let planet = sys.bodies[sys.station().unwrap()].rail.parent.unwrap();
        let port = sys.spaceports.iter().position(|sp| sp.body == planet).unwrap();
        // Land the ship on the pad.
        let dir = sys.spaceports[port].direction;
        let b = &sys.bodies[planet];
        let rot = b.rotation(p.world.time);
        let local = dir * (b.surface_radius(dir) + crate::ship::SHIP_RADIUS);
        let orientation = crate::ship::upright(rot * dir, rot * dir.any_orthonormal_vector());
        p.ship.state = ShipState::Landed { body: planet, local_position: local, local_orientation: rot.inverse() * orientation };
        let pos = p.positions();
        p.ship.position = pos[planet] + rot * local;
        p.ship.orientation = orientation;

        let mut person = Person { place: Place::Aboard { position: HATCH, yaw: 0.0, pitch: 0.0 } };
        let mut events = Vec::new();
        let t = p.world.time;
        person.step(&sys, &p.ship, t, &pos, &use_it(), 0.02, &mut events);
        let Place::Outside { position: start, .. } = person.place else { panic!("outside: {events:?}") };
        assert!((start.length() - b.surface_radius(start.normalize())).abs() < 0.01, "feet on the ground");
        // Walk 10 s: about 16 m, staying on the ground.
        for _ in 0..500 {
            person.step(&sys, &p.ship, t, &pos, &walk(1.0), 0.02, &mut events);
        }
        let Place::Outside { position, .. } = person.place else { panic!() };
        let moved = position.distance(start);
        assert!((moved - 16.0).abs() < 1.0, "walked {moved:.1} m");
        assert!((position.length() - b.surface_radius(position.normalize())).abs() < 0.05);
        // A jump goes up and comes down.
        person.step(&sys, &p.ship, t, &pos, &WalkCommands { jump: true, ..Default::default() }, 0.02, &mut events);
        let mut top: f64 = 0.0;
        for _ in 0..200 {
            person.step(&sys, &p.ship, t, &pos, &WalkCommands::default(), 0.02, &mut events);
            let Place::Outside { position, .. } = person.place else { panic!() };
            top = top.max(position.length() - b.surface_radius(position.normalize()));
        }
        let g = b.rail.mu / b.rail.radius.powi(2);
        assert!((top - JUMP * JUMP / (2.0 * g)).abs() < 0.1, "jumped {top:.2} m at g = {g:.1}");
        // Back to the ramp, and aboard.
        if let Place::Outside { position, .. } = &mut person.place {
            *position = start;
        }
        person.step(&sys, &p.ship, t, &pos, &use_it(), 0.02, &mut events);
        assert!(matches!(person.place, Place::Aboard { .. }), "{events:?}");
    }
}
