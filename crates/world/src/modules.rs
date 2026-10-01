//! Modules: what a hull is fitted with (content: `modules.ron`). Each is a
//! real thing — a mass, a volume, a power draw (or, for a power plant, an
//! output), a price — doing one job: a drive, thrusters, a tank, a rack,
//! the flight computer... A ship's numbers come from its hull's frame and
//! what's fitted in its slots (see `ship::HullDef::build`), never set by hand.
//!
//! Some are **base blocks** every ship must carry to fly at all (a power
//! plant, a main drive, attitude thrusters, a tank, the flight computer, a
//! transponder, sensors, life support); the rest is specialised equipment.

use serde::Deserialize;

/// What a module does, with its numbers.
#[derive(Clone, Debug, PartialEq, Deserialize)]
pub enum Does {
    /// Makes power (W).
    PowerPlant { output: f64 },
    /// The main drive: the thrust of a full-share nozzle (N).
    Drive { thrust: f64 },
    /// Translation and attitude thrusters: the thrust of a full-share nozzle (N).
    Thrusters { thrust: f64 },
    /// The belly lift: the thrust of a full-share nozzle (N).
    Lift { thrust: f64 },
    /// Holds fuel (kg).
    Tank { capacity: f64 },
    /// Holds cargo (kg).
    Rack { capacity: f64 },
    Hyperdrive,
    /// Flies the ship by wire: how fast it lets it turn (rad/s): pitch and yaw, roll.
    FlightComputer { turn_rate: f64, roll_rate: f64 },
    Transponder,
    /// Sensors: how far they see a ship (m).
    Sensors { range: f64 },
    LifeSupport,
    Gun,
    Laser,
    /// The anchor and excavator.
    MiningRig,
    /// Runs these autopilots.
    NavComputer { features: Vec<Feature> },
}

/// An autopilot a nav computer runs.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Deserialize)]
pub enum Feature {
    Docking,
    Landing,
    Gate,
    Follow,
    Hyperdrive,
    Route,
}

/// A kind of slot: what goes in it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Deserialize)]
pub enum SlotKind {
    Power,
    Drive,
    Thrusters,
    Lift,
    Tank,
    Cargo,
    Hyperdrive,
    Computer,
    Transponder,
    Sensors,
    LifeSupport,
    /// Guns and lasers.
    Hardpoint,
    /// Mining gear and such.
    Utility,
    /// Nav computers.
    Avionics,
}

impl Does {
    /// The kind of slot it goes in.
    pub fn slot(&self) -> SlotKind {
        match self {
            Does::PowerPlant { .. } => SlotKind::Power,
            Does::Drive { .. } => SlotKind::Drive,
            Does::Thrusters { .. } => SlotKind::Thrusters,
            Does::Lift { .. } => SlotKind::Lift,
            Does::Tank { .. } => SlotKind::Tank,
            Does::Rack { .. } => SlotKind::Cargo,
            Does::Hyperdrive => SlotKind::Hyperdrive,
            Does::FlightComputer { .. } => SlotKind::Computer,
            Does::Transponder => SlotKind::Transponder,
            Does::Sensors { .. } => SlotKind::Sensors,
            Does::LifeSupport => SlotKind::LifeSupport,
            Does::Gun | Does::Laser => SlotKind::Hardpoint,
            Does::MiningRig => SlotKind::Utility,
            Does::NavComputer { .. } => SlotKind::Avionics,
        }
    }
}

/// The slots every ship must have filled to fly: the base blocks.
pub const BASE_BLOCKS: [SlotKind; 8] = [SlotKind::Power, SlotKind::Drive, SlotKind::Thrusters, SlotKind::Tank, SlotKind::Computer, SlotKind::Transponder, SlotKind::Sensors, SlotKind::LifeSupport];

/// A module of the loaded content.
#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Module {
    pub key: String,
    pub name: String,
    /// Who makes it (brands come later; empty: unbranded).
    #[serde(default)]
    pub brand: String,
    pub does: Does,
    /// Its size class (1..4): it fits a slot at least as big.
    pub size: u8,
    /// Mass (kg) and volume (m³).
    pub mass: f64,
    pub volume: f64,
    /// Power it draws while working (W); a power plant's is its output (`does`).
    #[serde(default)]
    pub power: f64,
    /// Price (credits).
    pub price: f64,
}

impl Module {
    pub(crate) fn check(&self) -> Result<(), String> {
        for (what, v) in [("mass", self.mass), ("volume", self.volume), ("price", self.price)] {
            if !(v.is_finite() && v > 0.0) {
                return Err(format!("{what} must be positive ({v})"));
            }
        }
        if !(1..=4).contains(&self.size) {
            return Err(format!("size must be 1..4 ({})", self.size));
        }
        if !(self.power.is_finite() && self.power >= 0.0) {
            return Err(format!("power can't be negative ({})", self.power));
        }
        let positive = |what: &str, v: f64| if v.is_finite() && v > 0.0 { Ok(()) } else { Err(format!("{what} must be positive ({v})")) };
        match &self.does {
            Does::PowerPlant { output } => positive("output", *output),
            Does::Drive { thrust } | Does::Thrusters { thrust } | Does::Lift { thrust } => positive("thrust", *thrust),
            Does::Tank { capacity } | Does::Rack { capacity } => positive("capacity", *capacity),
            Does::FlightComputer { turn_rate, roll_rate } => positive("turn_rate", *turn_rate).and(positive("roll_rate", *roll_rate)),
            Does::Sensors { range } => positive("range", *range),
            _ => Ok(()),
        }
    }
}
