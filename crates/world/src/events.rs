//! What the world reports: physical facts about a ship (it landed, docked, was
//! destroyed, went through a gate…), separate from what world services say
//! (traffic control granting or refusing clearance).

use serde::{Deserialize, Serialize};

/// Something that physically happened to a ship.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum ShipEvent {
    /// Came to rest on a body: docked in a station's slot (`station`), or on the ground.
    Landed { body: String, station: bool },
    /// Came to rest on a spaceport's pad.
    LandedAtPort { port: String },
    /// Moved off the pad into the port's hangar, or out of it onto pad `pad`.
    EnteredHangar { port: String },
    /// Flight systems powered up or down; or a power-down refused, why.
    SystemsOn,
    SystemsOff,
    SystemsRefused { why: String },
    LeftHangar { port: String, pad: usize },
    /// A hangar move that couldn't be made: why.
    HangarRefused { why: String },
    /// Asked of gear that isn't fitted (a gun, a laser, the mining rig, the hyperdrive).
    NotFitted { what: String },
    /// A contact rule fired (see `rules`): whose, and what came of it.
    RuleFired { rule: String, outcome: String },
    TookOff,
    Launched { station: String },
    /// Gentle scrape against a station hull.
    Bumped,
    /// Struck an asteroid (`body`) at `speed` (m/s), losing `damage` of the hull.
    StruckRock { body: String, speed: f64, damage: f64 },
    /// The anchor holds the ship to asteroid `body`.
    Anchored { body: String },
    /// The anchor couldn't hold: why.
    AnchorFailed { why: String },
    /// The tank is dry: no thrust, no hyperdrive.
    OutOfFuel,
    /// The anchor let go (any loose ore in the hopper drifts away).
    AnchorReleased,
    /// A tonne of `item` (an ore) dug out of rock `rock` among field `field`'s
    /// bodies, and into the hold.
    Mined { field: usize, rock: usize, item: usize },
    /// The excavator stopped: why.
    ExcavatorStopped { why: String },
    /// Destroyed, by hitting `body` (or by weapons fire: "GUNFIRE", "LASER FIRE").
    Crashed { body: String },
    /// Opened fire on a ship that wasn't fair game: aggressed until world time `until`.
    Aggressed { until: f64 },
    /// Ran into ship `with`, closing at `speed` (m/s).
    Collided { with: usize, speed: f64 },
    /// Combat mode: the master arm went on (weapons priming), the weapons
    /// are primed and hot, or the master arm went off (safe).
    WeaponsArming,
    WeaponsHot,
    WeaponsSafe,
    /// Struck by a weapon of ship `by`: `damage` of the hull's strength taken,
    /// `hull` left (fractions).
    /// `weapon`: fired on (gun or laser), not a collision.
    Hit { by: usize, damage: f64, hull: f64, weapon: bool },
    /// A new ship was delivered.
    Respawned,
    /// Crossed into another star system's neighbourhood.
    EnteredSystem { name: String },
    HyperdriveEngaged,
    /// The hyperdrive wouldn't engage: jammed by hits, for this many more seconds.
    HyperdriveJammed { seconds: f64 },
    HyperdriveDisengaged,
    /// Entered a gate, heading for another system.
    GateEntered { to: String },
    /// Came out of the paired gate.
    GateArrived { system: String },
    /// Went through a gate faster than it can take.
    GateTooFast { speed: f64 },
}

/// What traffic control says.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum TrafficEvent {
    ClearanceGranted { target: String, kind: ClearanceKind },
    ClearanceDenied { reason: String },
    /// A clearance lapsed, or was given up.
    ClearanceCancelled,
    /// Cleared to land on this pad (0..9, row by row from the north-west).
    PadAssigned { pad: usize },
    /// Cleared to land, but all pads are taken: hold, with this many ahead.
    Holding { ahead: usize },
}

/// What a clearance is for.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ClearanceKind {
    Dock,
    Land,
    Transit,
}
