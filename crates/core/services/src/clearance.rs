//! Whether a system's traffic control clears a ship for a place: one rule, asked by the pilot before it asks
//! (what it sees of itself) and by traffic control before it grants a pad or a corridor (its own records). The
//! system's docks refuse its enemies and those outside its law; a port on ground its lift can't hold the ship
//! over, or its legs stand it on; a gate whose crossing it can't pay for.

use universe_world::traffic::Facility;
use universe_world::{Ship, StarSystem};

/// Why a clearance is refused.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refusal {
    Hostile,
    Outlaw,
    Lift,
    Legs,
    Toll,
}

/// What the system makes of the pilot, and what it can pay: its standing there at or under the line of
/// enemies, outlawed there, its credits.
#[derive(Clone, Copy, Debug, Default)]
pub struct Standing {
    pub hostile: bool,
    pub outlaw: bool,
    pub credits: f64,
}

/// Why `ship` is refused `target` in `sys`, with the words its pilot is told; None: nothing refuses it.
/// `gate_span` (m): how far a gate's crossing goes, if `target` is a gate (its toll by it).
pub fn refusal(sys: &StarSystem, ship: &Ship, target: Facility, standing: &Standing, gate_span: Option<f64>) -> Option<(Refusal, String)> {
    let name = sys.name.to_uppercase();
    if standing.hostile {
        return Some((Refusal::Hostile, format!("REFUSED - {name} TREATS YOU AS AN ENEMY")));
    }
    if standing.outlaw {
        return Some((Refusal::Outlaw, format!("REFUSED - YOU ARE OUTSIDE {name}'S LAW")));
    }
    if let Facility::Spaceport(p) = target
        && let Some(sp) = sys.spaceports.get(p)
    {
        let body = &sys.bodies[sp.body];
        let (lift, legs) = universe_world::legs::ground_check(ship.spec(), ship.mass(), body.surface_gravity());
        if lift < 1.0 {
            return Some((Refusal::Lift, format!("REFUSED - ITS LIFT HOLDS {lift:.2} OF ITS WEIGHT ON {}", body.name.to_uppercase())));
        }
        if legs.is_some_and(|v| v <= 0.0) {
            return Some((Refusal::Legs, format!("REFUSED - ITS LEGS CAN'T STAND ITS WEIGHT ON {}", body.name.to_uppercase())));
        }
    }
    if let (Facility::Gate(_), Some(span)) = (target, gate_span) {
        let toll = universe_world::gate::crossing_toll(ship.mass(), span);
        if standing.credits < toll {
            return Some((Refusal::Toll, format!("REFUSED - THE CROSSING COSTS {toll:.0} CR, YOU HAVE {:.0}", standing.credits)));
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use glam::{DQuat, DVec3};

    fn system() -> StarSystem {
        StarSystem {
            index: 0, name: "Testing".into(), class: universe_world::galaxy::StarClass::G, luminosity: 0.0,
            bodies: vec![universe_world::system::Body {
                key: "test.root".into(), name: "Root".into(), kind: universe_world::system::BodyKind::Star,
                mass: 0.0, color: [0.0; 3], rings: None, link: None, terrain: None, rock: None,
                rail: universe_physics::RailBody {
                    parent: None, orbit: None, mu: 0.0, attracts: false, radius: 0.0, tilt: DQuat::IDENTITY, day: 1.0,
                    collider: universe_physics::Collider::None, atmosphere: None, pulled_by: Vec::new(),
                },
            }],
            spaceports: Vec::new(), fields: Vec::new(), small: 0..0, belts: Vec::new(), belt_seed: 0, patches: Default::default(),
        }
    }

    /// The system's enemies and those outside its law are refused, whatever the pilot says of itself: the
    /// same rule for the pilot and for traffic control.
    #[test]
    fn clearance_refuses_enemies_and_outlaws_and_clears_the_rest() {
        let (sys, ship) = (system(), Ship::new(DVec3::ZERO, DVec3::ZERO, DQuat::IDENTITY));
        let at = Facility::Station(0);
        let ask = |s: Standing| refusal(&sys, &ship, at, &s, None).map(|r| r.0);
        assert_eq!(ask(Standing { hostile: true, ..Default::default() }), Some(Refusal::Hostile));
        assert_eq!(ask(Standing { outlaw: true, ..Default::default() }), Some(Refusal::Outlaw));
        assert_eq!(ask(Standing::default()), None);
        assert!(refusal(&sys, &ship, at, &Standing { hostile: true, ..Default::default() }, None).unwrap().1.contains("TESTING TREATS YOU AS AN ENEMY"));
    }
}
