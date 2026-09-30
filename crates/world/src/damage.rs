//! Damage: what destroys a ship, and how long until a new one is delivered.
//! (Where the new one appears is `World::respawn`.)

use glam::DVec3;

use crate::events::ShipEvent;
use crate::ship::{Ship, ShipState};

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
