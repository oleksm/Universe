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
    /// Something done, said back (sworn to a faction, say).
    Notice { text: String },
    /// Fuel bought: tonnes, for credits.
    Refuelled { tonnes: f64, credits: f64 },
    /// A slot refitted at a station: what went in (None: emptied), and the
    /// credits it cost (less what the module taken out fetched).
    Refitted { slot: String, module: Option<String>, credits: f64 },
    /// A new ship bought at a shipyard, the old one traded in: what it cost.
    BoughtShip { name: String, credits: f64 },
    /// The ship's trim set at a shipyard.
    Trimmed,
    /// Passengers taken aboard; landed to settle, for their fares.
    PassengersBoarded { count: u32 },
    PassengersLanded { count: u32, credits: f64 },
    /// Bought from a vending machine: what, for how much, and how it is.
    Vended { what: String, credits: f64, note: String },
    /// The hull mended at a station: what it cost, and how sound it is now (0..1).
    Repaired { credits: f64, hull: f64 },
    /// A ship lost, replaced by the insurer: what the excess cost (None: it
    /// couldn't be paid, or the loss was refused, and the replacement is the
    /// basic ship); the offence it refused for; the port it was delivered to
    /// (empty: none, left by the home station).
    Insured { excess: Option<f64>, refused: Option<String>, at: String },
    /// Charged with an offence under the law of the system named, and what it gave for it.
    Charged { offence: String, system: String, penalties: String },
    /// A bounty paid us for bringing the ship named down.
    Bounty { credits: f64, on: String },
}
