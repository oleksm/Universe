//! Damage: what destroys a ship, and how long until a new one is delivered.
//! (Where the new one appears is `World::respawn`.)

use glam::DVec3;

use crate::events::ShipEvent;
use crate::ship::{Ship, ShipState};

/// Energy the hull absorbs before it fails (J).
pub const HULL_STRENGTH: f64 = 20.0e6;

/// A hit jams the hyperdrive for this long (game s): under fire, a ship can't
/// simply jump away.
pub const HYPER_JAM: f64 = 15.0;

/// Real seconds from destruction to the replacement ship.
pub const RESPAWN_TIME: f64 = 3.0;

/// The ship is destroyed by `cause` (what it hit): its devices go dead.
pub fn destroy(ship: &mut Ship, cause: &str, events: &mut Vec<ShipEvent>) {
    ship.state = ShipState::Destroyed { respawn_in: RESPAWN_TIME };
    ship.hyperdrive = false;
    ship.throttle = 0.0;
    ship.rcs = DVec3::ZERO;
    events.push(ShipEvent::Crashed { body: cause.to_string() });
}

/// Struck by a weapon of ship `by`: `joules` of damage and a push of
/// `impulse` (N·s). The hull fails when it's used up.
pub fn hit(ship: &mut Ship, joules: f64, impulse: DVec3, by: usize, cause: &str, events: &mut Vec<ShipEvent>) {
    if !matches!(ship.state, ShipState::Flying | ShipState::Landed { .. }) {
        return;
    }
    let damage = joules / HULL_STRENGTH;
    ship.hull = (ship.hull - damage).max(0.0);
    if cause != "COLLISION" {
        ship.hyper_jam = HYPER_JAM;
    }
    if matches!(ship.state, ShipState::Flying) {
        ship.velocity += impulse / ship.mass();
    }
    events.push(ShipEvent::Hit { by, damage, hull: ship.hull, weapon: cause != "COLLISION" });
    if ship.hull <= 0.0 {
        destroy(ship, cause, events);
    }
}
