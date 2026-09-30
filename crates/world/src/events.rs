//! What the world reports: physical facts about a ship (it landed, docked, was
//! destroyed, went through a gate…), separate from what world services say
//! (traffic control granting or refusing clearance).

/// Something that physically happened to a ship.
#[derive(Clone, Debug, PartialEq)]
pub enum ShipEvent {
    /// Came to rest on a body: docked in a station's slot (`station`), or on the ground.
    Landed { body: String, station: bool },
    /// Came to rest on a spaceport's pad.
    LandedAtPort { port: String },
    TookOff,
    Launched { station: String },
    /// Gentle scrape against a station hull.
    Bumped,
    /// Destroyed, by hitting `body`.
    Crashed { body: String },
    /// A new ship was delivered.
    Respawned,
    /// Crossed into another star system's neighbourhood.
    EnteredSystem { name: String },
    HyperdriveEngaged,
    HyperdriveDisengaged,
    /// Entered a gate, heading for another system.
    GateEntered { to: String },
    /// Came out of the paired gate.
    GateArrived { system: String },
    /// Went through a gate faster than it can take.
    GateTooFast { speed: f64 },
}

/// What traffic control says.
#[derive(Clone, Debug, PartialEq)]
pub enum TrafficEvent {
    ClearanceGranted { target: String, kind: ClearanceKind },
    ClearanceDenied { reason: String },
    /// A clearance lapsed, or was given up.
    ClearanceCancelled,
}

/// What a clearance is for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClearanceKind {
    Dock,
    Land,
    Transit,
}
