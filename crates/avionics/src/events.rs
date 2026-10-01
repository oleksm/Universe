//! What the avionics report to the pilot: the world's physical events and
//! traffic control's, as they heard them, and their own.

use serde::{Deserialize, Serialize};

use universe_world::{ShipEvent, TrafficEvent, CrewEvent};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Event {
    /// Something physically happened to the ship.
    Ship(ShipEvent),
    /// Traffic control said something.
    Traffic(TrafficEvent),
    /// The pilot, on foot.
    Crew(CrewEvent),
    /// Dropped out of hyperdrive at the nav target.
    HyperdriveArrived { target: String },
    /// The route autopilot reached a stop (1-based number, name).
    RouteStop { number: usize, name: String },
    RouteComplete,
    /// A route stop can't be reached (no gate path, or it no longer exists).
    RouteBlocked { reason: String },
    Autopilot { on: bool },
    NavTargetSet { name: Option<String> },
    /// The follow program: now keeping at / orbiting at this range (m), or off.
    Following { what: Option<(String, f64)> },
    /// The radar lock: on a contact (its name), or released.
    Lock { name: Option<String> },
    /// Asked to lock, with nothing in the beam.
    NothingInBeam,
    /// The locked contact went off the radar.
    ContactLost,
    /// A trade at a market: units (negative: sold) of an item, for credits
    /// (negative: received).
    Traded { item: String, units: i64, credits: f64 },
    /// The avionics can't do what was asked.
    Refused { reason: String },
}
