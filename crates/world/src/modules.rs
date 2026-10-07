//! Modules: what a hull is fitted with (the registry's equipment, SFO 16). Each is a
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
    /// Makes power (W) from the material it `burns`, at `efficiency` (the rest heat).
    PowerPlant { output: f64, efficiency: f64, burns: String },
    /// The main drive: the thrust of a full-share nozzle (N).
    Drive { thrust: f64, exhaust: f64, efficiency: f64, burns: String },
    /// Translation and attitude thrusters: the thrust of a full-share nozzle (N).
    Thrusters { thrust: f64, exhaust: f64, efficiency: f64, burns: String },
    /// The belly lift: the thrust of a full-share nozzle (N).
    Lift { thrust: f64, exhaust: f64, efficiency: f64, burns: String },
    /// Holds fuel (kg): the material it `holds`.
    Tank { capacity: f64, holds: String },
    /// A capacitor bank: stores energy (J), taken in or given out at up to `rate` (W).
    Capacitor { capacity: f64, rate: f64 },
    /// Holds cargo (kg).
    Rack { capacity: f64 },
    /// A passenger cabin: seats, with their life support (in a cargo slot).
    Cabin { seats: u32 },
    /// The hyperdrive: its field's `efficiency` (the share of the fuel's energy
    /// that goes into the field; the rest is heat), and its top speed (m/s).
    Hyperdrive { efficiency: f64, top_speed: f64 },
    /// Flies the ship by wire: how fast it lets it turn (rad/s): pitch and yaw, roll.
    FlightComputer { turn_rate: f64, roll_rate: f64 },
    Transponder,
    /// Sensors: how far they see a ship (m).
    /// `resolves`: how far it makes out a rock, for its size (m per m), and
    /// `survey_range` the farthest it looks (m): 0, it doesn't survey.
    Sensors { range: f64, resolves: f64, survey_range: f64 },
    /// A comm (the hypernet, `docs/hypernet.md`): it hears what happens within
    /// `capture` (m), links to another comm within `link` (m; the shorter of
    /// the two decides), passes a message on after `lag` (s), and handles
    /// `capacity` messages a second.
    Comm { capture: f64, link: f64, lag: f64, capacity: f64 },
    /// A gate relay (fitted to a gate ring): links its system's net to its
    /// twin's through the lane's tube (in capsules: `hypernet::capsule_time`), handling a message
    /// in `lag` (s) more, `capacity` messages a second, throwing a batch every
    /// `cadence` s (capsules can't pass each other in the flow: they go by turns).
    GateRelay { lag: f64, capacity: f64, cadence: f64 },
    /// A hyper relay (space structures): links its site to others through
    /// hyperspace (Dogma's hyper-signal), handling a message in `lag` (s),
    /// `capacity` messages a second, throwing a batch every `cadence` s.
    HyperRelay { lag: f64, capacity: f64, cadence: f64 },
    LifeSupport,
    /// A gun, a laser (see `weapons::Gun`, `weapons::Laser`).
    Gun(crate::weapons::Gun),
    Laser(crate::weapons::Laser),
    /// The anchor and excavator (see `mining::Rig`).
    MiningRig(crate::mining::Rig),
    /// Runs these autopilots; and its hyperdrive interlock: never closer
    /// than `interlock` m to a body's highest ground (0: it has none).
    NavComputer {
        features: Vec<Feature>,
        #[serde(default)]
        interlock: f64,
        /// Its hyperdrive governor (1/s): held to this times the distance to
        /// the nearest surface (slow close to bodies; 0: none).
        #[serde(default)]
        governor: f64,
    },
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

/// Gear a module brings that something checks for.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Gear {
    Gun,
    Laser,
    MiningRig,
    Hyperdrive,
}

/// A kind of slot: what goes in it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Deserialize)]
pub enum SlotKind {
    Power,
    Drive,
    Thrusters,
    Lift,
    Tank,
    /// Capacitor banks.
    Capacitor,
    Cargo,
    Hyperdrive,
    Computer,
    Transponder,
    Sensors,
    /// The comm.
    Comm,
    /// A gate's relay (structures only).
    Relay,
    LifeSupport,
    /// Guns and lasers.
    Hardpoint,
    /// Mining gear and such.
    Utility,
    /// Nav computers.
    Avionics,
}

impl Does {
    /// The gear it is, if anything checks for it.
    pub fn gear(&self) -> Option<Gear> {
        match self {
            Does::Gun(_) => Some(Gear::Gun),
            Does::Laser(_) => Some(Gear::Laser),
            Does::MiningRig(_) => Some(Gear::MiningRig),
            Does::Hyperdrive { .. } => Some(Gear::Hyperdrive),
            _ => None,
        }
    }

    /// An engine's figures (a drive's, thrusters', lift's): its thrust a
    /// nozzle (N), exhaust velocity (m/s), efficiency (the share of its fuel's
    /// energy that goes into the jet; the rest heat), and what it burns.
    pub fn engine(&self) -> Option<(f64, f64, f64, &str)> {
        match self {
            Does::Drive { thrust, exhaust, efficiency, burns } | Does::Thrusters { thrust, exhaust, efficiency, burns } | Does::Lift { thrust, exhaust, efficiency, burns } => Some((*thrust, *exhaust, *efficiency, burns)),
            _ => None,
        }
    }

    /// A comm's figures, if it's one.
    pub fn comm(&self) -> Option<Comm> {
        match *self {
            Does::Comm { capture, link, lag, capacity } => Some(Comm { capture, link, lag, capacity }),
            _ => None,
        }
    }

    /// The kind of slot it goes in.
    pub fn slot(&self) -> SlotKind {
        match self {
            Does::PowerPlant { .. } => SlotKind::Power,
            Does::Drive { .. } => SlotKind::Drive,
            Does::Thrusters { .. } => SlotKind::Thrusters,
            Does::Lift { .. } => SlotKind::Lift,
            Does::Tank { .. } => SlotKind::Tank,
            Does::Capacitor { .. } => SlotKind::Capacitor,
            Does::Rack { .. } | Does::Cabin { .. } => SlotKind::Cargo,
            Does::Hyperdrive { .. } => SlotKind::Hyperdrive,
            Does::FlightComputer { .. } => SlotKind::Computer,
            Does::Transponder => SlotKind::Transponder,
            Does::Sensors { .. } => SlotKind::Sensors,
            Does::Comm { .. } => SlotKind::Comm,
            Does::GateRelay { .. } | Does::HyperRelay { .. } => SlotKind::Relay,
            Does::LifeSupport => SlotKind::LifeSupport,
            Does::Gun(_) | Does::Laser(_) => SlotKind::Hardpoint,
            Does::MiningRig(_) => SlotKind::Utility,
            Does::NavComputer { .. } => SlotKind::Avionics,
        }
    }
}

/// The slots every ship must have filled to fly: the base blocks.
pub const BASE_BLOCKS: [SlotKind; 9] = [SlotKind::Power, SlotKind::Drive, SlotKind::Thrusters, SlotKind::Tank, SlotKind::Computer, SlotKind::Transponder, SlotKind::Sensors, SlotKind::Comm, SlotKind::LifeSupport];

/// A comm's figures (see `Does::Comm`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Comm {
    pub capture: f64,
    pub link: f64,
    pub lag: f64,
    pub capacity: f64,
}

impl Comm {
    /// The longest link between two comms: the shorter reach decides.
    pub fn link_with(&self, other: &Comm) -> f64 {
        self.link.min(other.link)
    }
}

/// A maker of modules: one of the registry's makers (`org.*`).
#[derive(Clone, Debug, PartialEq)]
pub struct Brand {
    pub key: String,
    pub name: String,
    /// What it's known for.
    pub note: String,
    /// The system it's based in (its index among the galaxy's stars): where
    /// its address is.
    pub home: Option<usize>,
}

impl Brand {
    /// From the registry's record of the maker: its home where its address
    /// is (the settlement's body's system).
    pub fn from_record(reg: &crate::registry::Registry, o: &crate::registry::Org) -> Self {
        let home = o.address.as_ref().and_then(|a| {
            let body = reg.settlements.iter().find(|s| s.identity.key == a.at)?.at.as_deref()?;
            let system = body.split('.').nth(1)?;
            reg.system(&format!("system.{system}"))?.identity.index.map(|i| i as usize)
        });
        Brand { key: o.identity.key.clone(), name: crate::standards::caps(&o.identity.name), note: crate::standards::caps(o.note.as_deref().unwrap_or_default()), home }
    }
}

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
    /// The mount it's built to (SFO 19), by key; its own size (m: length, width,
    /// height), where its record says.
    #[serde(default)]
    pub fits: Option<String>,
    #[serde(default)]
    pub dims: Option<[f64; 3]>,
}

impl Module {
    /// Its box (m: across, up, along the ship): as much room as its volume,
    /// proportioned by what it is — a drive long, a lift flat, racks broad.
    pub fn dims(&self) -> glam::DVec3 {
        let a = match self.does.slot() {
            SlotKind::Drive | SlotKind::Hardpoint => glam::DVec3::new(1.0, 1.0, 2.2),
            SlotKind::Lift => glam::DVec3::new(1.6, 0.5, 1.6),
            SlotKind::Tank | SlotKind::Hyperdrive => glam::DVec3::new(1.0, 1.0, 1.5),
            SlotKind::Cargo => glam::DVec3::new(1.4, 1.0, 1.8),
            SlotKind::Utility => glam::DVec3::new(1.0, 0.6, 1.6),
            _ => glam::DVec3::ONE,
        };
        a * (self.volume / (a.x * a.y * a.z)).cbrt()
    }

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
            Does::PowerPlant { output, efficiency, .. } => positive("output", *output).and(if *efficiency > 0.0 && *efficiency <= 1.0 { Ok(()) } else { Err(format!("efficiency must be in 0..1 ({efficiency})")) }),
            Does::Drive { thrust, exhaust, efficiency, .. } | Does::Thrusters { thrust, exhaust, efficiency, .. } | Does::Lift { thrust, exhaust, efficiency, .. } => {
                positive("thrust", *thrust).and(positive("exhaust", *exhaust)).and(if *efficiency > 0.0 && *efficiency <= 1.0 { Ok(()) } else { Err(format!("efficiency must be in 0..1 ({efficiency})")) })
            }
            // (What a tank holds is checked against its material when content loads.)
            Does::Tank { capacity, .. } | Does::Rack { capacity } => positive("capacity", *capacity),
            Does::Cabin { seats } => positive("seats", *seats as f64),
            Does::FlightComputer { turn_rate, roll_rate } => positive("turn_rate", *turn_rate).and(positive("roll_rate", *roll_rate)),
            Does::Sensors { range, .. } => positive("range", *range),
            Does::Comm { capture, link, lag, capacity } => positive("capture", *capture).and(positive("link", *link)).and(positive("capacity", *capacity)).and(if lag.is_finite() && *lag >= 0.0 { Ok(()) } else { Err(format!("lag can't be negative ({lag})")) }),
            Does::GateRelay { lag, capacity, .. } | Does::HyperRelay { lag, capacity, .. } => positive("capacity", *capacity).and(if lag.is_finite() && *lag >= 0.0 { Ok(()) } else { Err(format!("lag can't be negative ({lag})")) }),
            Does::Hyperdrive { efficiency, top_speed } => positive("top_speed", *top_speed).and(if *efficiency > 0.0 && *efficiency <= 1.0 { Ok(()) } else { Err(format!("efficiency must be in 0..1 ({efficiency})")) }),
            // (Storage can't beat the physics sheet's density.)
            Does::Capacitor { capacity, rate } => positive("capacity", *capacity).and(positive("rate", *rate)).and(if *capacity <= crate::sheet::CAPACITOR_DENSITY * self.mass * 1.001 {
                Ok(())
            } else {
                Err(format!("stores {:.1e} J in {:.0} kg: past the sheet's {:.0e} J/kg", capacity, self.mass, crate::sheet::CAPACITOR_DENSITY))
            }),
            _ => Ok(()),
        }
    }
}

impl Module {
    /// From the registry's record of a piece of equipment, at `price` (the
    /// game's: prices aren't the registry's). None for a kind of device the
    /// game doesn't make yet (a gate's throat coil).
    pub fn from_record(e: &crate::registry::Equipment, price: f64) -> Option<Self> {
        let does = e.function.handle(&mut Kinds)?;
        Some(Module {
            key: e.identity.key.clone(),
            name: crate::standards::caps(&e.identity.name),
            brand: e.identity.maker.clone(),
            does,
            size: e.size_class.unwrap_or(1) as u8,
            mass: e.physical.mass.unwrap_or(0.0),
            volume: e.physical.volume.unwrap_or(0.0),
            power: e.needs.power.unwrap_or(0.0),
            price,
            fits: e.fits.clone(),
            dims: match (e.physical.length, e.physical.width, e.physical.height) {
                (Some(l), Some(w), Some(h)) => Some([l, w, h]),
                _ => None,
            },
        })
    }
}

/// What the engine makes of each kind of device in the registry (one method a
/// kind, generated from the schema: a new kind there is a method to write here;
/// a figure added to a kind changes nothing until the method reads it).
struct Kinds;

use crate::registry as r;

impl crate::registry::EquipmentFunctionHandler for Kinds {
    type Out = Option<Does>;
    /// A kind the game doesn't make yet (its schema says `x-in-game: "not made"`): nothing a ship does with it.
    fn not_made(&mut self, _: &'static str) -> Self::Out {
        None
    }
    fn power_plant(&mut self, it: &r::EquipmentFunctionPowerPlant) -> Self::Out {
        Some(Does::PowerPlant { output: it.output, efficiency: it.efficiency, burns: it.burns.clone() })
    }
    fn drive(&mut self, it: &r::EquipmentFunctionDrive) -> Self::Out {
        Some(Does::Drive { thrust: it.thrust, exhaust: it.exhaust, efficiency: it.efficiency, burns: it.burns.clone() })
    }
    fn thrusters(&mut self, it: &r::EquipmentFunctionThrusters) -> Self::Out {
        Some(Does::Thrusters { thrust: it.thrust, exhaust: it.exhaust, efficiency: it.efficiency, burns: it.burns.clone() })
    }
    fn lift(&mut self, it: &r::EquipmentFunctionLift) -> Self::Out {
        Some(Does::Lift { thrust: it.thrust, exhaust: it.exhaust, efficiency: it.efficiency, burns: it.burns.clone() })
    }
    fn tank(&mut self, it: &r::EquipmentFunctionTank) -> Self::Out {
        Some(Does::Tank { capacity: it.capacity, holds: it.holds.clone() })
    }
    fn capacitor(&mut self, it: &r::EquipmentFunctionCapacitor) -> Self::Out {
        Some(Does::Capacitor { capacity: it.capacity, rate: it.rate })
    }
    fn rack(&mut self, it: &r::EquipmentFunctionRack) -> Self::Out {
        Some(Does::Rack { capacity: it.capacity })
    }
    fn cabin(&mut self, it: &r::EquipmentFunctionCabin) -> Self::Out {
        Some(Does::Cabin { seats: it.seats as u32 })
    }
    fn hyperdrive(&mut self, it: &r::EquipmentFunctionHyperdrive) -> Self::Out {
        Some(Does::Hyperdrive { efficiency: it.efficiency, top_speed: it.top_speed })
    }
    fn flight_computer(&mut self, it: &r::EquipmentFunctionFlightComputer) -> Self::Out {
        Some(Does::FlightComputer { turn_rate: it.turn_rate, roll_rate: it.roll_rate })
    }
    fn sensors(&mut self, it: &r::EquipmentFunctionSensors) -> Self::Out {
        Some(Does::Sensors { range: it.range, resolves: it.resolves.unwrap_or(0.0), survey_range: it.survey_range.unwrap_or(0.0) })
    }
    fn comm(&mut self, it: &r::EquipmentFunctionComm) -> Self::Out {
        Some(Does::Comm { capture: it.capture, link: it.link, lag: it.lag, capacity: it.capacity })
    }
    fn gate_relay(&mut self, it: &r::EquipmentFunctionGateRelay) -> Self::Out {
        Some(Does::GateRelay { lag: it.lag, capacity: it.capacity, cadence: it.cadence })
    }
    fn hyper_relay(&mut self, it: &r::EquipmentFunctionHyperRelay) -> Self::Out {
        Some(Does::HyperRelay { lag: it.lag, capacity: it.capacity, cadence: it.cadence })
    }
    fn nav_computer(&mut self, it: &r::EquipmentFunctionNavComputer) -> Self::Out {
        use crate::registry::EquipmentFunctionNavComputerFeature as N;
        let features = it
            .features
            .iter()
            .map(|f| match f {
                N::Docking => Feature::Docking,
                N::Landing => Feature::Landing,
                N::Gate => Feature::Gate,
                N::Follow => Feature::Follow,
                N::Hyperdrive => Feature::Hyperdrive,
                N::Route => Feature::Route,
            })
            .collect();
        Some(Does::NavComputer { features, interlock: it.interlock, governor: it.governor })
    }
    fn transponder(&mut self, _: &r::EquipmentFunctionTransponder) -> Self::Out {
        Some(Does::Transponder)
    }
    fn life_support(&mut self, _: &r::EquipmentFunctionLifeSupport) -> Self::Out {
        Some(Does::LifeSupport)
    }
    fn gun(&mut self, it: &r::EquipmentFunctionGun) -> Self::Out {
        let f = |v: Option<f64>| v.unwrap_or(0.0);
        Some(Does::Gun(crate::weapons::Gun { muzzle: f(it.muzzle_speed), rate: f(it.rate), slug_mass: f(it.slug_mass), magazine: it.magazine.unwrap_or(0).max(0) as u32 }))
    }
    fn laser(&mut self, it: &r::EquipmentFunctionLaser) -> Self::Out {
        let f = |v: Option<f64>| v.unwrap_or(0.0);
        Some(Does::Laser(crate::weapons::Laser { power: f(it.beam_power), focus: f(it.focus), range: f(it.range), burn: f(it.burn), cool: f(it.cool), reset: f(it.reset) }))
    }
    fn mining_rig(&mut self, it: &r::EquipmentFunctionMiningRig) -> Self::Out {
        let f = |v: Option<f64>| v.unwrap_or(0.0);
        Some(Does::MiningRig(crate::mining::Rig { excavator_power: f(it.excavator_power), throughput: f(it.throughput), anchor_reach: f(it.anchor_reach), anchor_speed: f(it.anchor_speed) }))
    }
    /// A gate's throat coil: not made yet (the schema is to say so: `x-in-game: not made`).
    fn throat_coil(&mut self, _: &r::EquipmentFunctionThroatCoil) -> Self::Out {
        None
    }
    /// An ore bay as a product: not made yet (`x-in-game: not made`); the game's bay is the hull's own (`HullDef::bay`).
    fn ore_bay(&mut self, _: &r::EquipmentFunctionOreBay) -> Self::Out {
        None
    }
    // Ship fittings (SFO 20): not made yet (`x-in-game: not made`); the studio fits them, the game does not yet run them.
    fn airlock(&mut self, _: &r::EquipmentFunctionAirlock) -> Self::Out {
        None
    }
    fn ramp(&mut self, _: &r::EquipmentFunctionRamp) -> Self::Out {
        None
    }
    fn cargo_lift(&mut self, _: &r::EquipmentFunctionCargoLift) -> Self::Out {
        None
    }
    fn bay_door(&mut self, _: &r::EquipmentFunctionBayDoor) -> Self::Out {
        None
    }
    fn docking(&mut self, _: &r::EquipmentFunctionDocking) -> Self::Out {
        None
    }
    fn handling(&mut self, _: &r::EquipmentFunctionHandling) -> Self::Out {
        None
    }
    fn radiator(&mut self, _: &r::EquipmentFunctionRadiator) -> Self::Out {
        None
    }
    fn heat_exchanger(&mut self, _: &r::EquipmentFunctionHeatExchanger) -> Self::Out {
        None
    }
    fn coolant_loop(&mut self, _: &r::EquipmentFunctionCoolantLoop) -> Self::Out {
        None
    }
    fn heat_sink(&mut self, _: &r::EquipmentFunctionHeatSink) -> Self::Out {
        None
    }
    fn store(&mut self, _: &r::EquipmentFunctionStore) -> Self::Out {
        None
    }
    fn pump(&mut self, _: &r::EquipmentFunctionPump) -> Self::Out {
        None
    }
    fn compressor(&mut self, _: &r::EquipmentFunctionCompressor) -> Self::Out {
        None
    }
    fn port(&mut self, _: &r::EquipmentFunctionPort) -> Self::Out {
        None
    }
    fn window(&mut self, _: &r::EquipmentFunctionWindow) -> Self::Out {
        None
    }
    fn battery(&mut self, _: &r::EquipmentFunctionBattery) -> Self::Out {
        None
    }
    fn solar_array(&mut self, _: &r::EquipmentFunctionSolarArray) -> Self::Out {
        None
    }
    fn switchgear(&mut self, _: &r::EquipmentFunctionSwitchgear) -> Self::Out {
        None
    }
    fn reaction_wheels(&mut self, _: &r::EquipmentFunctionReactionWheels) -> Self::Out {
        None
    }
    fn command_station(&mut self, _: &r::EquipmentFunctionCommandStation) -> Self::Out {
        None
    }
    fn berths(&mut self, _: &r::EquipmentFunctionBerths) -> Self::Out {
        None
    }
    fn galley(&mut self, _: &r::EquipmentFunctionGalley) -> Self::Out {
        None
    }
    fn head(&mut self, _: &r::EquipmentFunctionHead) -> Self::Out {
        None
    }
    fn fire_unit(&mut self, _: &r::EquipmentFunctionFireUnit) -> Self::Out {
        None
    }
    fn pressure_door(&mut self, _: &r::EquipmentFunctionPressureDoor) -> Self::Out {
        None
    }
    fn suit_locker(&mut self, _: &r::EquipmentFunctionSuitLocker) -> Self::Out {
        None
    }
    fn altimeter(&mut self, _: &r::EquipmentFunctionAltimeter) -> Self::Out {
        None
    }
    fn camera(&mut self, _: &r::EquipmentFunctionCamera) -> Self::Out {
        None
    }
    fn engine(&mut self, _: &r::EquipmentFunctionEngine) -> Self::Out {
        None
    }
    fn swivel(&mut self, _: &r::EquipmentFunctionSwivel) -> Self::Out {
        None
    }
    /// A landing leg as a product: not made yet (`x-in-game: not made`); legs are reckoned from a hull's parts (`world::legs`).
    fn landing_gear(&mut self, _: &r::EquipmentFunctionLandingGear) -> Self::Out {
        None
    }
}
