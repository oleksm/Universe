//! Traffic control: the world service that grants or refuses permission to
//! dock at a station, land at a spaceport or fly through a gate, and lets a
//! clearance lapse when the ship strays too far. A rule of the world, not of
//! physics: nothing moves because of it.

use glam::DVec3;
use serde::{Deserialize, Serialize};

use crate::events::ClearanceKind;
use crate::ship::Ship;
use crate::spaceport;
use crate::system::{BodyKind, StarSystem};

/// Docking clearance is granted within this range of the station (m).
pub const DOCK_RANGE: f64 = 50_000.0;
/// Landing clearance is granted within this many planet radii of the pad.
pub const LAND_RANGE_RADII: f64 = 20.0;
/// Transit clearance is granted within this range of the gate (m).
pub const TRANSIT_RANGE: f64 = 50_000.0;

/// A place a ship can be cleared for, in its current star system.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Facility {
    /// Index into the system's bodies.
    Station(usize),
    /// Index into the system's spaceports.
    Spaceport(usize),
    /// Index into the system's bodies (a ring gate).
    Gate(usize),
    /// Index into the system's bodies (an asteroid field's remnant): no one
    /// controls traffic there.
    Asteroid(usize),
    /// Index into the system's bodies (a rig, see `rigs`): no traffic control;
    /// a ship comes alongside by hand.
    Rig(usize),
}

impl Facility {
    /// What a clearance for it is for (none at an asteroid).
    pub fn kind(self) -> Option<ClearanceKind> {
        match self {
            Facility::Station(_) => Some(ClearanceKind::Dock),
            Facility::Spaceport(_) => Some(ClearanceKind::Land),
            Facility::Gate(_) => Some(ClearanceKind::Transit),
            Facility::Asteroid(_) | Facility::Rig(_) => None,
        }
    }

    /// Display name.
    pub fn name(self, sys: &StarSystem) -> String {
        match self {
            Facility::Station(b) => sys.bodies.get(b).map_or_else(String::new, |b| b.name.clone()),
            Facility::Spaceport(p) => sys.spaceports.get(p).map_or_else(String::new, |p| format!("{} ({})", p.name, sys.bodies[p.body].name)),
            Facility::Gate(b) => sys.bodies.get(b).map_or_else(String::new, |b| b.name.clone()),
            Facility::Asteroid(b) => sys.fields.iter().find(|f| f.body == b).map_or_else(String::new, |f| f.name.clone()),
            Facility::Rig(b) => sys.bodies.get(b).map_or_else(String::new, |b| b.name.clone()),
        }
    }

    /// Where it is at `t` (`positions` at `t`), if it exists in `sys`.
    pub fn position(self, sys: &StarSystem, t: f64, positions: &[DVec3]) -> Option<DVec3> {
        match self {
            Facility::Station(b) => (sys.bodies.get(b)?.kind == BodyKind::Station).then(|| positions[b]),
            Facility::Spaceport(p) => {
                sys.spaceports.get(p)?;
                Some(spaceport::pad_position(sys, p, t, positions))
            }
            Facility::Gate(b) => (sys.bodies.get(b)?.kind == BodyKind::Gate).then(|| positions[b]),
            Facility::Asteroid(b) => sys.bodies.get(b)?.kind.is_rock().then(|| positions[b]),
            Facility::Rig(b) => (sys.bodies.get(b)?.kind == BodyKind::Rig).then(|| positions[b]),
        }
    }

    /// How far out clearance is granted (m).
    pub fn clearance_range(self, sys: &StarSystem) -> f64 {
        match self {
            Facility::Station(_) => DOCK_RANGE,
            Facility::Spaceport(p) => sys.bodies[sys.spaceports[p].body].rail.radius * LAND_RANGE_RADII,
            Facility::Gate(_) => TRANSIT_RANGE,
            Facility::Asteroid(_) | Facility::Rig(_) => 0.0,
        }
    }
}

/// The facility a ship is docked or landed at (a station's slot, a port's
/// pads), if any: a physical fact.
pub fn docked_at(sys: &StarSystem, ship: &Ship) -> Option<Facility> {
    let crate::ship::ShipState::Landed { body, local_position, .. } = ship.state else { return None };
    if sys.bodies[body].kind == BodyKind::Station {
        return Some(Facility::Station(body));
    }
    if sys.bodies[body].kind == BodyKind::Rig {
        return Some(Facility::Rig(body));
    }
    sys.port_at(body, local_position.normalize()).map(Facility::Spaceport)
}

/// Every market place in a star system: its stations and spaceports.
pub fn facilities(sys: &StarSystem) -> Vec<Facility> {
    let stations = sys.bodies.iter().enumerate().filter(|(_, b)| b.kind == BodyKind::Station).map(|(i, _)| Facility::Station(i));
    let rigs = sys.bodies.iter().enumerate().filter(|(_, b)| b.kind == BodyKind::Rig).map(|(i, _)| Facility::Rig(i));
    stations.chain((0..sys.spaceports.len()).map(Facility::Spaceport)).chain(rigs).collect()
}
