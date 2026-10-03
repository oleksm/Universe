//! Ports: where ships set down and park, by `Facility` — a spaceport's
//! pads on a world, or a station's deck. Both have a 4 × 4 grid of pads,
//! a hangar for long stays (out of sight, the pad freed) and taxiing
//! between them. Positions are in the port's body's own frame, where a
//! resting ship's centre is.

use glam::{DQuat, DVec3};

use crate::ship::upright;
use crate::system::{BodyKind, StarSystem};
use crate::traffic::Facility;
use crate::{spaceport, station};

/// The body a port is on (None: not a port).
pub fn body(sys: &StarSystem, port: Facility) -> Option<usize> {
    match port {
        Facility::Station(b) => sys.bodies.get(b).is_some_and(|b| b.kind == BodyKind::Station).then_some(b),
        Facility::Spaceport(p) => sys.spaceports.get(p).map(|sp| sp.body),
        Facility::Gate(_) | Facility::Asteroid(_) => None,
    }
}

/// Where a ship whose centre stands `height` over its feet
/// (`Ship::rest_height`) rests above body-frame point `at` (on the ground,
/// or the deck).
pub fn settle(sys: &StarSystem, port: Facility, at: DVec3, height: f64) -> DVec3 {
    match port {
        Facility::Spaceport(p) => {
            let dir = at.normalize();
            dir * (sys.bodies[sys.spaceports[p].body].surface_radius(dir) + height)
        }
        _ => station::rest(at, height),
    }
}

/// Up, at body-frame point `at` of the port.
pub fn up(port: Facility, at: DVec3) -> DVec3 {
    match port {
        Facility::Spaceport(_) => at.normalize(),
        _ => DVec3::Y,
    }
}

/// Where a ship `height` tall to its centre rests on pad `pad`.
pub fn pad(sys: &StarSystem, port: Facility, pad: usize, height: f64) -> DVec3 {
    match port {
        Facility::Spaceport(p) => settle(sys, port, spaceport::pad_direction(sys, p, pad.min(spaceport::PADS - 1)), height),
        _ => station::rest(station::pad_local(pad.min(spaceport::PADS - 1)), height),
    }
}

/// Where a ship `height` tall to its centre rests in the hangar (out of sight).
pub fn hangar(sys: &StarSystem, port: Facility, height: f64) -> DVec3 {
    match port {
        Facility::Spaceport(p) => settle(sys, port, spaceport::hangar_direction(sys, p), height),
        _ => station::rest(station::hangar_local(), height),
    }
}

/// The pad a ship resting at `at` is on, if any.
pub fn pad_at(sys: &StarSystem, port: Facility, at: DVec3) -> Option<usize> {
    match port {
        Facility::Spaceport(p) => spaceport::pad_at(sys, p, at.normalize()),
        _ => station::pad_at(at),
    }
}

/// The port whose ground (or deck) a ship resting on `body` at `at` is
/// on, if any: a station's deck, or within a spaceport's pad area.
pub fn at(sys: &StarSystem, body: usize, at: DVec3) -> Option<Facility> {
    if sys.bodies[body].kind == BodyKind::Station {
        return Some(Facility::Station(body));
    }
    sys.port_at(body, at.normalize()).map(Facility::Spaceport)
}

/// Resting at `at`, upright, the nose along `heading` (body frame).
pub fn resting(port: Facility, at: DVec3, heading: DVec3) -> DQuat {
    let up = up(port, at);
    let heading = heading - up * heading.dot(up);
    upright(up, heading.try_normalize().unwrap_or_else(|| up.any_orthonormal_vector()))
}
