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
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Facility {
    /// Index into the system's bodies.
    Station(usize),
    /// Index into the system's spaceports.
    Spaceport(usize),
    /// Index into the system's bodies (a ring gate).
    Gate(usize),
}

impl Facility {
    /// What a clearance for it is for.
    pub fn kind(self) -> ClearanceKind {
        match self {
            Facility::Station(_) => ClearanceKind::Dock,
            Facility::Spaceport(_) => ClearanceKind::Land,
            Facility::Gate(_) => ClearanceKind::Transit,
        }
    }

    /// Display name.
    pub fn name(self, sys: &StarSystem) -> String {
        match self {
            Facility::Station(b) => sys.bodies.get(b).map_or_else(String::new, |b| b.name.clone()),
            Facility::Spaceport(p) => sys.spaceports.get(p).map_or_else(String::new, |p| format!("{} ({})", p.name, sys.bodies[p.body].name)),
            Facility::Gate(b) => sys.bodies.get(b).map_or_else(String::new, |b| b.name.clone()),
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
        }
    }

    /// How far out clearance is granted (m).
    pub fn clearance_range(self, sys: &StarSystem) -> f64 {
        match self {
            Facility::Station(_) => DOCK_RANGE,
            Facility::Spaceport(p) => sys.bodies[sys.spaceports[p].body].rail.radius * LAND_RANGE_RADII,
            Facility::Gate(_) => TRANSIT_RANGE,
        }
    }
}

/// The station nearest to `p`, if the system has one.
pub fn nearest_station(sys: &StarSystem, p: DVec3, positions: &[DVec3]) -> Option<Facility> {
    sys.bodies
        .iter()
        .enumerate()
        .filter(|(_, b)| b.kind == BodyKind::Station)
        .map(|(i, _)| (i, positions[i].distance(p)))
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(i, _)| Facility::Station(i))
}

/// A ship asks for clearance to use `target` (if it names one) at `t`
/// (`positions` at `t`): granted, or refused with the reason.
pub fn request(sys: &StarSystem, ship: &Ship, target: Option<Facility>, t: f64, positions: &[DVec3]) -> Result<Facility, String> {
    if !ship.is_flying() {
        return Err("NOT IN FLIGHT".into());
    }
    if ship.hyperdrive {
        return Err("DISENGAGE HYPERDRIVE FIRST".into());
    }
    if ship.armed {
        return Err("WEAPONS ARMED - DISARM FIRST (B)".into());
    }
    let Some(target) = target else {
        return Err("NO TARGET - PICK ONE ON THE MAP (M)".into());
    };
    let Some(at) = target.position(sys, t, positions) else {
        return Err("TARGET NOT IN THIS SYSTEM".into());
    };
    let range = target.clearance_range(sys);
    if at.distance(ship.position) > range {
        return Err(format!("OUT OF RANGE - CLOSE TO {:.0} KM", range / 1000.0));
    }
    Ok(target)
}

/// A clearance for `target` lapses if the ship wanders more than twice the
/// granting range away, the target is gone, or the ship arms its weapons.
pub fn lapsed(sys: &StarSystem, ship: &Ship, target: Facility, t: f64, positions: &[DVec3]) -> bool {
    if ship.armed {
        return true;
    }
    let range = target.clearance_range(sys);
    target.position(sys, t, positions).is_none_or(|p| p.distance(ship.position) > 2.0 * range)
}

#[cfg(test)]
mod tests {
    use glam::DQuat;

    use super::*;
    use crate::World;

    #[test]
    fn clearance_is_granted_in_range_and_refused_otherwise() {
        let w = World::new(42);
        let sys = w.system(w.home_system);
        let station = Facility::Station(sys.station().unwrap());
        let mut positions = Vec::new();
        sys.positions(w.time, &mut positions);
        let at = station.position(&sys, w.time, &positions).unwrap();
        let mut ship = Ship::new(at + DVec3::X * 4000.0, DVec3::ZERO, DQuat::IDENTITY);
        assert_eq!(request(&sys, &ship, Some(station), w.time, &positions), Ok(station));
        assert_eq!(nearest_station(&sys, ship.position, &positions), Some(station));
        assert!(request(&sys, &ship, None, w.time, &positions).is_err(), "no target, no clearance");
        assert!(request(&sys, &ship, Some(Facility::Gate(0)), w.time, &positions).is_err(), "the star is no gate");

        ship.position = at + DVec3::X * (DOCK_RANGE + 1000.0);
        assert!(request(&sys, &ship, Some(station), w.time, &positions).unwrap_err().starts_with("OUT OF RANGE"));
        assert!(!lapsed(&sys, &ship, station, w.time, &positions), "a granted clearance holds out to twice the range");
        ship.position = at + DVec3::X * (2.0 * DOCK_RANGE + 1000.0);
        assert!(lapsed(&sys, &ship, station, w.time, &positions));

        ship.position = at + DVec3::X * 4000.0;
        ship.hyperdrive = true;
        assert_eq!(request(&sys, &ship, Some(station), w.time, &positions), Err("DISENGAGE HYPERDRIVE FIRST".into()));
        ship.hyperdrive = false;
        ship.state = crate::ship::ShipState::Destroyed { respawn_in: 1.0 };
        assert_eq!(request(&sys, &ship, Some(station), w.time, &positions), Err("NOT IN FLIGHT".into()));
    }
}
