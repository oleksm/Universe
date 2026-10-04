//! The Standards Foundry Office's records (`standards/SFO`).

use serde::{Deserialize, Serialize};

use crate::{Basis, Physical};

/// `good.*`: something that moves in bulk through industry (rock as dug, raw
/// matter, what is used up, what one step hands the next, by-products).
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Good {
    pub identity: GoodIdentity,
    #[serde(default)]
    pub physical: Physical,
    /// What a rock is made of: each part (an element or a good, by key) and its
    /// share by mass.
    #[serde(default)]
    pub composition: Vec<Part>,
    #[serde(default)]
    pub source: Option<GoodSource>,
    /// What it is in the game's own files, while those are the game's.
    #[serde(default)]
    pub game: Option<GoodInGame>,
    #[serde(default)]
    pub basis: Vec<Basis>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GoodIdentity {
    pub key: String,
    pub name: String,
    pub kind: GoodKind,
    #[serde(default)]
    pub description: Option<String>,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum GoodKind {
    Rock,
    Raw,
    Consumable,
    Fuel,
    Intermediate,
    Product,
    ByProduct,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Part {
    pub part: String,
    pub share: f64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GoodSource {
    /// For a rock: where in the universe it is found.
    #[serde(default)]
    pub occurs: Option<String>,
    /// For a raw good: the rock it is won from, by key.
    #[serde(default)]
    pub won_from: Option<String>,
    /// Tonnes of it won from each tonne of that rock.
    #[serde(default, rename = "yield")]
    pub yields: Option<f64>,
    #[serde(default)]
    pub note: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GoodInGame {
    /// The kind of goods it is in the game's markets.
    #[serde(default)]
    pub goods: Option<String>,
    /// For a rock: the game's ore it is.
    #[serde(default)]
    pub ore: Option<String>,
}
