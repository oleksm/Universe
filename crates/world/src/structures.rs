//! The rules each structure's owner registers with the core (see `rules`):
//! today written here for every station, spaceport and gate of a system as
//! it's generated; with the services (re-architecture R4), each owner — a
//! station's traffic control, a port, a gate authority — registers its own.

use glam::DVec3;

use crate::events::ShipEvent;
use crate::galaxy::Galaxy;
use crate::names::star_name;
use crate::rules::{Part, Pose, Release, Rule, Rules, Says};
use crate::spaceport::{LAND_SPEED, PAD_RADIUS};
use crate::station::{BUMP_SPEED, DOCKED_HEIGHT, MAX_DOCK_SPEED, MAX_ROLL_ERROR, STATION_SIZE};
use crate::system::{BodyKind, StarSystem};

/// The rules of every part of `sys`: stations' docking ports and hulls,
/// worlds' ground (with their spaceports' pads) and seas, gates' openings
/// and rings.
pub fn rules(galaxy: &Galaxy, sys: &StarSystem) -> Rules {
    let mut r = Rules::default();
    for (i, b) in sys.bodies.iter().enumerate() {
        let name = b.name.clone();
        match b.kind {
            BodyKind::Station => {
                // The docking port: slowly, lined up with the slot (nose into
                // it, wings along it), held in the slot.
                let port = format!("{name} docking port");
                r.set(
                    i,
                    Part::Slot,
                    Rule::Lock {
                        name: port,
                        max_speed: MAX_DOCK_SPEED,
                        pose: Pose::Slot { position: DVec3::Y * STATION_SIZE * DOCKED_HEIGHT, nose: DVec3::NEG_Y, wings: DVec3::X, max_roll: MAX_ROLL_ERROR },
                        says: Says::Always(ShipEvent::Landed { body: name.clone(), station: true }),
                        otherwise: name.clone(),
                    },
                );
                r.set(i, Part::Hull, Rule::Bounce { name: format!("{name} hull"), max_speed: BUMP_SPEED, otherwise: name.clone() });
                // Launch: out along the axis, nose first, at 40 m/s.
                r.set_release(i, Release::Eject { axis: DVec3::Y, wings: DVec3::X, distance: STATION_SIZE + 150.0, speed: 40.0, says: ShipEvent::Launched { station: name.clone() } });
            }
            BodyKind::Gate => {
                let to = b.link.unwrap_or(sys.index);
                let says = ShipEvent::GateEntered { to: star_name(galaxy.stars[to].seed) };
                r.set(i, Part::Opening, Rule::Transit { name: name.clone(), max_speed: crate::gate::MAX_TRANSIT_SPEED, to, says, otherwise: name.clone() });
                r.set(i, Part::Ring, Rule::Wreck { name: format!("{name} ring"), cause: name.clone() });
            }
            kind if kind.landable() => {
                // The ground takes a gentle touchdown anywhere; on a port's
                // pads, the port says so.
                let zones = sys
                    .spaceports
                    .iter()
                    .filter(|sp| sp.body == i)
                    .map(|sp| (sp.direction, PAD_RADIUS / b.rail.radius, ShipEvent::LandedAtPort { port: sp.name.clone() }))
                    .collect();
                r.set(
                    i,
                    Part::Ground,
                    Rule::Lock {
                        name: format!("{name} ground"),
                        max_speed: LAND_SPEED,
                        pose: Pose::Ground,
                        says: Says::Zones { zones, otherwise: ShipEvent::Landed { body: name.clone(), station: false } },
                        otherwise: name.clone(),
                    },
                );
                r.set_release(i, Release::LiftOff { speed: 3.0, lift: 2.0, says: ShipEvent::TookOff });
                r.set(i, Part::Sea, Rule::Wreck { name: format!("{name} sea"), cause: format!("{name} ocean") });
            }
            _ => {}
        }
    }
    r
}
