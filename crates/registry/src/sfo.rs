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

/// `standard.<body>.<number>`: a standard, as its body's register holds it.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Standard {
    pub identity: StandardIdentity,
    #[serde(default)]
    pub title: Option<String>,
    /// The record it sits under (its category), by key.
    #[serde(default)]
    pub parent: Option<String>,
    /// A folder in metadata/ whose records sit under this one.
    #[serde(default)]
    pub records: Option<String>,
    /// Why it exists, a paragraph.
    #[serde(default)]
    pub purpose: Option<String>,
    #[serde(default)]
    pub details: Option<Block>,
    #[serde(default)]
    pub sections: Vec<Block>,
    #[serde(default)]
    pub version: Option<u32>,
    #[serde(default)]
    pub status: Option<StandardStatus>,
    #[serde(default)]
    pub scope: Option<String>,
    #[serde(default)]
    pub topics: Vec<String>,
    /// The standards it builds on, as cited (`SFO 2`).
    #[serde(default)]
    pub refs: Vec<String>,
    #[serde(default)]
    pub params: Vec<Param>,
    #[serde(default)]
    pub requires: Vec<Requirement>,
    #[serde(default)]
    pub text: Option<String>,
    #[serde(default)]
    pub licence: Option<Licence>,
    /// When it was published (world time, s).
    #[serde(default)]
    pub published: Option<f64>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StandardIdentity {
    pub key: String,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum StandardStatus {
    Draft,
    Published,
    Superseded,
    Withdrawn,
}

/// Text and/or a table.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Block {
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub text: Option<Text>,
    #[serde(default)]
    pub table: Option<Table>,
}

/// A paragraph, or several.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Text {
    One(String),
    Paragraphs(Vec<String>),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Table {
    pub columns: Vec<String>,
    pub rows: Vec<Vec<String>>,
}

/// One value a standard fixes.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Param {
    pub key: String,
    pub value: ParamValue,
    #[serde(default)]
    pub unit: Option<String>,
    #[serde(default)]
    pub note: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ParamValue {
    Number(f64),
    Range([f64; 2]),
    Text(String),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Requirement {
    pub subject: String,
    pub check: Check,
    pub param: String,
    #[serde(default)]
    pub per: Option<String>,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Check {
    AtMost,
    AtLeast,
    Equals,
    FitsWithin,
    Provides,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Licence {
    Open(OpenLicence),
    Fee { fee: f64 },
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum OpenLicence {
    Open,
}

/// `module.*`: an industrial module (SFO 10), one step of a line.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Module {
    pub identity: ModuleIdentity,
    #[serde(default)]
    pub physical: Physical,
    /// For one that stores or handles.
    #[serde(default)]
    pub capacity: Option<Capacity>,
    /// Everything it can be set to make.
    #[serde(default)]
    pub recipes: Vec<Recipe>,
    /// For a shop module: how much it puts through of whatever it's given.
    #[serde(default)]
    pub throughput: Option<Throughput>,
    /// For one that makes power.
    #[serde(default)]
    pub generation: Option<Generation>,
    /// For one with no recipe: what it draws all the time.
    #[serde(default)]
    pub needs: Option<Needs>,
    #[serde(default)]
    pub basis: Vec<Basis>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModuleIdentity {
    pub key: String,
    pub name: String,
    /// The step it performs.
    pub step: String,
    #[serde(default)]
    pub description: Option<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Capacity {
    /// kg it can store.
    #[serde(default)]
    pub holds: Option<f64>,
    /// m³ it can store.
    #[serde(default)]
    pub volume: Option<f64>,
    /// The kinds of goods it stores, by the game's keys.
    #[serde(default)]
    pub stores: Vec<String>,
    /// kg/s it can move in or out.
    #[serde(default)]
    pub handling: Option<f64>,
}

/// One thing a module can be set to make: what it makes, what that takes
/// (per kg made), what else comes out, how fast, at what power.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Recipe {
    pub makes: String,
    #[serde(default)]
    pub does: Option<String>,
    pub inputs: Vec<Amount>,
    #[serde(default)]
    pub outputs: Vec<Amount>,
    /// kg/s of what it makes, at full rate.
    #[serde(default)]
    pub rate: Option<f64>,
    #[serde(default)]
    pub batch: Option<f64>,
    /// W at full rate.
    #[serde(default)]
    pub power: Option<f64>,
    #[serde(default)]
    pub changeover: Option<Changeover>,
}

/// kg of an item for each kg the recipe makes.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Amount {
    pub item: String,
    pub quantity: f64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Changeover {
    #[serde(default)]
    pub time: Option<f64>,
    #[serde(default)]
    pub loss: Option<f64>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Throughput {
    /// kg/s of what it is making, at full rate.
    pub rate: f64,
    /// W at full rate.
    #[serde(default)]
    pub power: Option<f64>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Generation {
    /// W at full output.
    pub supplies: f64,
    #[serde(default)]
    pub burns: Vec<Burn>,
}

/// What it burns at full output (kg/s).
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Burn {
    pub item: String,
    pub rate: f64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Needs {
    /// W.
    #[serde(default)]
    pub power: Option<f64>,
}

/// `market.*`: one of the categories the markets trade in. Anything physical
/// is traded as one (its `traded_as`).
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Market {
    pub identity: MarketIdentity,
    /// About what one item weighs, for goods the seed names itself (kg).
    #[serde(default)]
    pub unit_mass: Option<f64>,
    /// As stowed in a hold (kg/m³).
    #[serde(default)]
    pub bulk_density: Option<f64>,
    /// kg a person uses of it each second (none: people don't use it up).
    #[serde(default)]
    pub basket: Option<f64>,
    #[serde(default)]
    pub names: Option<MarketNames>,
    #[serde(default)]
    pub basis: Vec<Basis>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MarketIdentity {
    pub key: String,
    pub name: String,
}

/// The words the seed names its goods of a category from.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MarketNames {
    #[serde(default)]
    pub adjectives: Vec<String>,
    #[serde(default)]
    pub nouns: Vec<String>,
}

/// `stock.*`: an item of mill stock: a material in a form and a size.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Stock {
    pub identity: StockIdentity,
    #[serde(default)]
    pub made_from: Vec<crate::MadeFrom>,
    #[serde(default)]
    pub size: Option<StockSize>,
    #[serde(default)]
    pub physical: Physical,
    #[serde(default)]
    pub basis: Vec<Basis>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StockIdentity {
    pub key: String,
    /// The market category it's traded as, by key.
    #[serde(default)]
    pub traded_as: Option<String>,
    pub code: String,
    pub name: String,
    pub form: String,
    #[serde(default)]
    pub temper: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StockSize {
    #[serde(default)]
    pub thickness: Option<f64>,
    #[serde(default)]
    pub diameter: Option<f64>,
    #[serde(default)]
    pub wall: Option<f64>,
}
