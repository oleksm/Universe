//! Definitions every schema shares (`standards/common.schema.yaml`).

use serde::{Deserialize, Serialize};

/// What every physical thing has: what it weighs, the room it takes, what it
/// stands. What a ship hauling it and a warehouse holding it have to know. A
/// record uses those that apply to it.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Physical {
    /// The thing as made, nothing fitted to it or carried in it (kg).
    #[serde(default)]
    pub mass: Option<f64>,
    #[serde(default)]
    pub length: Option<f64>,
    #[serde(default)]
    pub width: Option<f64>,
    #[serde(default)]
    pub height: Option<f64>,
    /// [length, width, height] of the box it ships in, where that differs
    /// from its own size (m).
    #[serde(default)]
    pub envelope: Option<[f64; 3]>,
    /// The space its shape takes (m³).
    #[serde(default)]
    pub volume: Option<f64>,
    /// As stowed broken in a hold, for what is carried in bulk (kg/m³).
    #[serde(default)]
    pub bulk_density: Option<f64>,
    #[serde(default)]
    pub operating_min_temperature: Option<f64>,
    #[serde(default)]
    pub operating_max_temperature: Option<f64>,
    /// The coldest it may be kept or carried at (K).
    #[serde(default)]
    pub storage_min_temperature: Option<f64>,
    /// The hottest it may be kept or carried at (K).
    #[serde(default)]
    pub storage_max_temperature: Option<f64>,
    /// The blow it takes without damage (J).
    #[serde(default)]
    pub impact_resistance: Option<f64>,
    /// The hardest jolt it takes while it is carried (m/s²).
    #[serde(default)]
    pub shock_limit: Option<f64>,
}
