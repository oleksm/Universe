//! What the avionics report to the pilot: the world's physical events and
//! traffic control's, as they heard them, and their own.

use universe_world::{ShipEvent, TrafficEvent};

#[derive(Clone, Debug)]
pub enum Event {
    /// Something physically happened to the ship.
    Ship(ShipEvent),
    /// Traffic control said something.
    Traffic(TrafficEvent),
    /// Dropped out of hyperdrive at the nav target.
    HyperdriveArrived { target: String },
    /// The route autopilot reached a stop (1-based number, name).
    RouteStop { number: usize, name: String },
    RouteComplete,
    /// A route stop can't be reached (no gate path, or it no longer exists).
    RouteBlocked { reason: String },
    Autopilot { on: bool },
    NavTargetSet { name: Option<String> },
    /// The avionics can't do what was asked.
    Refused { reason: String },
}
