//! Local Administration's records (`standards/LocalAdministration`): the
//! settlements and rigs, and each settlement's ground: zones, parcels,
//! streets, power lines, facilities. Every place is in metres [east, north]
//! of the settlement's position.

use serde::{Deserialize, Serialize};

/// A point on the ground: metres [east, north] of the settlement's position.
pub type Point = [f64; 2];

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct KeyName {
    pub key: String,
    pub name: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct KeyOnly {
    pub key: String,
}

/// `settlement.*` or `rig.*`.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Settlement {
    pub identity: KeyName,
    pub kind: SettlementKind,
    /// The planet or moon it is at, by key.
    #[serde(default)]
    pub at: Option<String>,
    /// Where on its body's surface (degrees).
    #[serde(default)]
    pub position: Option<Position>,
    #[serde(default)]
    pub gate: Option<GatePlace>,
    #[serde(default)]
    pub spin: Option<Spin>,
    /// For a rig: whose it is.
    #[serde(default)]
    pub owner: Option<String>,
    #[serde(default)]
    pub processes: Vec<String>,
    #[serde(default)]
    pub lines: Vec<Line>,
    #[serde(default)]
    pub modules: Vec<ModuleCount>,
    #[serde(default)]
    pub about: Option<String>,
    #[serde(default)]
    pub story: Vec<String>,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum SettlementKind {
    Settlement,
    Rig,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Position {
    pub latitude: f64,
    pub longitude: f64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GatePlace {
    /// The gate ring it is, by key.
    pub ring: String,
    /// The star it leads to, by key.
    pub to: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Spin {
    #[serde(default)]
    pub radius: Option<f64>,
    #[serde(default)]
    pub gravity: Option<f64>,
}

/// A production line: what it is built to make (its modules' recipes lead
/// there), what else they can be set to make, and its modules in order.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Line {
    #[serde(default)]
    pub makes: Option<String>,
    #[serde(default)]
    pub also: Vec<String>,
    pub modules: Vec<ModuleCount>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModuleCount {
    pub module: String,
    pub count: u32,
}

/// `zone.*`: ground set aside for a use.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Zone {
    pub identity: KeyName,
    #[serde(rename = "use")]
    pub zone: crate::ZoneUse,
    pub outline: Vec<Point>,
}

/// `parcel.*`: a lot of land and who owns it.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Parcel {
    pub identity: KeyOnly,
    pub number: u32,
    /// Who owns it, by key (left out: vacant).
    #[serde(default)]
    pub owner: Option<String>,
    #[serde(default)]
    pub address: Option<StreetAddress>,
    pub outline: Vec<Point>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StreetAddress {
    pub street: String,
    pub number: u32,
}

/// `street.*`.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Street {
    pub identity: KeyName,
    pub line: Vec<Point>,
}

/// `power-line.*`: from a facility that makes power to another.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PowerLine {
    pub identity: KeyName,
    pub from: String,
    pub to: String,
    /// The most it can carry (W).
    pub capacity: f64,
    pub line: Vec<Point>,
}

/// `facility.*`: works on a parcel.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Facility {
    pub identity: KeyName,
    pub kind: FacilityKind,
    /// The parcel it stands on, by key.
    pub parcel: String,
    #[serde(default)]
    pub processes: Vec<String>,
    #[serde(default)]
    pub lines: Vec<Line>,
    /// What it's built of where its lines don't say (a power station's, a store's).
    #[serde(default)]
    pub modules: Vec<ModuleCount>,
    /// For a warehouse: the exchange that has approved it.
    #[serde(default)]
    pub exchange: Option<String>,
    #[serde(default)]
    pub parts: Vec<SitePart>,
    #[serde(default)]
    pub pipelines: Vec<Pipeline>,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum FacilityKind {
    Foundry,
    Mill,
    Yard,
    Power,
    Warehouse,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SitePart {
    pub name: String,
    pub kind: String,
    #[serde(default)]
    pub does: Option<String>,
    #[serde(default)]
    pub processes: Vec<String>,
    pub outline: Vec<Point>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Pipeline {
    pub name: String,
    pub carries: String,
    pub from: String,
    pub to: String,
    pub line: Vec<Point>,
}
