//! The celestial registry (`standards/Celestial`): the seed's settings, the
//! kinds of asteroid, the systems written out.

use serde::{Deserialize, Serialize};

use crate::{Basis, Provenance};

/// The seed's settings (`seeding.*`): how the seed makes the world.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Seeding {
    pub galaxy: GalaxySeeding,
}

/// `seeding.galaxy`: the world as a whole.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GalaxySeeding {
    pub identity: NamedIdentity,
    pub galaxy: Galaxy,
    #[serde(default)]
    pub basis: Vec<Basis>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NamedIdentity {
    pub key: String,
    pub name: String,
    #[serde(default)]
    pub about: Option<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Galaxy {
    /// The number the whole world is made from.
    pub seed: u64,
    /// The home system, by its key.
    pub home: String,
    /// The side of the charted region, a cube round home (m).
    pub region: f64,
    /// Stars for each cubic metre.
    pub star_density: f64,
    /// The side of the cubes the stars are made in (m).
    pub sector: f64,
    pub class_mix: ClassMix,
}

/// The share of stars of each spectral class, by number.
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[allow(non_snake_case)]
pub struct ClassMix {
    pub O: f64,
    pub B: f64,
    pub A: f64,
    pub F: f64,
    pub G: f64,
    pub K: f64,
    pub M: f64,
}

/// `rock-class.*`: a kind of asteroid, by what it is made of.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RockClass {
    pub identity: RockClassIdentity,
    #[serde(default)]
    pub physical: Option<RockPhysical>,
    #[serde(default)]
    pub composition: Composition,
    #[serde(default)]
    pub found: Found,
    #[serde(default)]
    pub mining: Option<Mining>,
    #[serde(default)]
    pub basis: Vec<Basis>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RockClassIdentity {
    pub key: String,
    /// Its label in the game, for one the game has.
    #[serde(default)]
    pub label: Option<String>,
    pub name: String,
    /// Its spectral type, as astronomers class asteroids.
    #[serde(default)]
    pub letter: Option<String>,
    #[serde(default)]
    pub about: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RockPhysical {
    /// As a rubble pile (kg/m³).
    #[serde(default)]
    pub density_rubble: Option<f64>,
    /// As one solid piece (kg/m³).
    #[serde(default)]
    pub density_monolith: Option<f64>,
    /// The share of light it reflects.
    #[serde(default)]
    pub albedo: Option<f64>,
    /// Red, green, blue, each 0 to 1.
    #[serde(default)]
    pub colour: Option<[f64; 3]>,
}

/// What it is made of, by mass: each a range from a lean family to a rich one
/// (the rest is silicate rock).
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Composition {
    #[serde(default)]
    pub water: Option<[f64; 2]>,
    #[serde(default)]
    pub organics: Option<[f64; 2]>,
    #[serde(default)]
    pub metal: Option<[f64; 2]>,
    #[serde(default)]
    pub volatiles: Option<[f64; 2]>,
    /// Platinum-group metals.
    #[serde(default)]
    pub pgm: Option<[f64; 2]>,
}

/// How much of what is there is of this class, by where a body formed.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Found {
    #[serde(default)]
    pub warm: Option<f64>,
    #[serde(default)]
    pub frost_line: Option<f64>,
    #[serde(default)]
    pub cold: Option<f64>,
    #[serde(default)]
    pub trojans: Option<f64>,
    #[serde(default)]
    pub outer: Option<f64>,
    #[serde(default)]
    pub falls: Option<f64>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Mining {
    /// J to break a kg loose from a solid piece.
    #[serde(default)]
    pub cut_energy: Option<f64>,
    /// The good it yields when dug, by key.
    #[serde(default)]
    pub yields: Option<String>,
    /// What it yields instead where it is rich enough.
    #[serde(default)]
    pub rich_yields: Option<String>,
    /// The share of platinum-group metals above which it yields that instead.
    #[serde(default)]
    pub rich_above: Option<f64>,
}

/// `system.*`: a star system written out.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct System {
    pub provenance: Provenance,
    pub identity: SystemIdentity,
    #[serde(default)]
    pub position: Option<SystemPosition>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SystemIdentity {
    pub key: String,
    pub name: String,
    /// Its number among the seed's stars.
    #[serde(default)]
    pub index: Option<u64>,
    #[serde(default)]
    pub about: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SystemPosition {
    /// From the home star, [x, y, z] in the galaxy's frame (m).
    #[serde(default)]
    pub from_home: Option<[f64; 3]>,
    /// From the home star (m).
    #[serde(default)]
    pub distance: Option<f64>,
}

/// Whether the game makes a kind of thing yet.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum InGame {
    Made,
    Partly,
    #[serde(rename = "not made")]
    NotMade,
}

/// `body.<system>.<name>`: a star, planet, moon or small body.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Body {
    pub provenance: Provenance,
    #[serde(default)]
    pub in_game: Option<InGame>,
    pub identity: BodyIdentity,
    #[serde(default)]
    pub star: Option<Star>,
    #[serde(default)]
    pub orbit: Option<BodyOrbit>,
    #[serde(default)]
    pub physical: BodyPhysical,
    #[serde(default)]
    pub bulk: Option<Bulk>,
    #[serde(default)]
    pub interior: Option<Interior>,
    #[serde(default)]
    pub magnetic: Option<Magnetic>,
    #[serde(default)]
    pub crust: Option<Crust>,
    #[serde(default)]
    pub water: Option<Water>,
    #[serde(default)]
    pub surface: Surface,
    #[serde(default)]
    pub atmosphere: Option<Atmosphere>,
    #[serde(default)]
    pub life: Option<Life>,
    #[serde(default)]
    pub rock: Option<BodyRock>,
    #[serde(default)]
    pub basis: Vec<Basis>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BodyIdentity {
    pub key: String,
    pub name: String,
    pub kind: BodyKind,
    /// What it goes round, by key (a star has none).
    #[serde(default)]
    pub parent: Option<String>,
    #[serde(default)]
    pub about: Option<String>,
    /// Its story, in paragraphs.
    #[serde(default)]
    pub story: Vec<String>,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum BodyKind {
    #[serde(rename = "star")]
    Star,
    #[serde(rename = "rocky planet")]
    RockyPlanet,
    #[serde(rename = "gas giant")]
    GasGiant,
    #[serde(rename = "ice giant")]
    IceGiant,
    #[serde(rename = "moon")]
    Moon,
    #[serde(rename = "asteroid")]
    Asteroid,
    #[serde(rename = "dwarf planet")]
    DwarfPlanet,
    #[serde(rename = "comet")]
    Comet,
    #[serde(rename = "centaur")]
    Centaur,
    #[serde(rename = "crossing asteroid")]
    CrossingAsteroid,
    #[serde(rename = "captured moon")]
    CapturedMoon,
}

/// For a star.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Star {
    /// Its spectral class: O, B, A, F, G, K or M.
    #[serde(default)]
    pub class: Option<String>,
    /// All the light it gives off (W).
    #[serde(default)]
    pub luminosity: Option<f64>,
}

/// Its orbit round its parent (m, s; angles in degrees).
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BodyOrbit {
    #[serde(default)]
    pub semi_major_axis: Option<f64>,
    #[serde(default)]
    pub eccentricity: Option<f64>,
    #[serde(default)]
    pub period: Option<f64>,
    #[serde(default)]
    pub inclination: Option<f64>,
    #[serde(default)]
    pub ascending_node: Option<f64>,
    #[serde(default)]
    pub periapsis_argument: Option<f64>,
    #[serde(default)]
    pub mean_anomaly: Option<f64>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BodyPhysical {
    #[serde(default)]
    pub mass: Option<f64>,
    #[serde(default)]
    pub radius: Option<f64>,
    #[serde(default)]
    pub density: Option<f64>,
    /// Worked out (m/s²): written down, not taken.
    #[serde(default)]
    pub gravity: Option<f64>,
    /// Its spin (s).
    #[serde(default)]
    pub day: Option<f64>,
    /// Its axis against its orbit (degrees).
    #[serde(default)]
    pub tilt: Option<f64>,
    #[serde(default)]
    pub albedo: Option<f64>,
    /// The inner and outer radius of its rings (m).
    #[serde(default)]
    pub rings: Option<[f64; 2]>,
    #[serde(default)]
    pub flattening: Option<f64>,
    #[serde(default)]
    pub age: Option<f64>,
}

/// What it is made of, by mass.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Bulk {
    #[serde(default)]
    pub metal: Option<f64>,
    #[serde(default)]
    pub rock: Option<f64>,
    #[serde(default)]
    pub ice: Option<f64>,
    #[serde(default)]
    pub gas: Option<f64>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Interior {
    #[serde(default)]
    pub differentiated: Option<bool>,
    #[serde(default)]
    pub core_radius: Option<f64>,
    #[serde(default)]
    pub core_state: Option<CoreState>,
    #[serde(default)]
    pub core_made_of: Option<String>,
    #[serde(default)]
    pub mantle_made_of: Option<String>,
    #[serde(default)]
    pub crust_thickness: Option<f64>,
    #[serde(default)]
    pub heat_flow: Option<f64>,
    #[serde(default)]
    pub heat_from: Vec<HeatFrom>,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum CoreState {
    #[serde(rename = "solid")]
    Solid,
    #[serde(rename = "liquid")]
    Liquid,
    #[serde(rename = "liquid with a solid centre")]
    LiquidWithSolidCentre,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum HeatFrom {
    Formation,
    Radioactivity,
    Tides,
    Settling,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Magnetic {
    /// At its surface, at the equator (T).
    #[serde(default)]
    pub field: Option<f64>,
    #[serde(default)]
    pub tilt: Option<f64>,
    #[serde(default)]
    pub from: Option<MagneticFrom>,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum MagneticFrom {
    #[serde(rename = "core dynamo")]
    CoreDynamo,
    #[serde(rename = "crust left magnetised")]
    CrustLeftMagnetised,
    #[serde(rename = "induced")]
    Induced,
    #[serde(rename = "none")]
    None,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Crust {
    #[serde(default)]
    pub rocks: Vec<CrustRock>,
    #[serde(default)]
    pub regolith: Option<f64>,
    #[serde(default)]
    pub ores: Vec<CrustOre>,
    #[serde(default)]
    pub tectonics: Option<Tectonics>,
    #[serde(default)]
    pub volcanism: Option<Volcanism>,
    #[serde(default)]
    pub surface_age: Option<f64>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CrustRock {
    pub rock: String,
    pub share: f64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CrustOre {
    pub ore: String,
    #[serde(default)]
    pub share: Option<f64>,
    #[serde(default)]
    pub depth: Option<f64>,
    #[serde(default, rename = "where")]
    pub place: Option<String>,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum Tectonics {
    #[serde(rename = "plates")]
    Plates,
    #[serde(rename = "one plate")]
    OnePlate,
    #[serde(rename = "none")]
    None,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum Volcanism {
    #[serde(rename = "active")]
    Active,
    #[serde(rename = "past")]
    Past,
    #[serde(rename = "none")]
    None,
    #[serde(rename = "ice volcanoes")]
    IceVolcanoes,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Water {
    #[serde(default)]
    pub ocean_cover: Option<f64>,
    #[serde(default)]
    pub ocean_depth: Option<f64>,
    #[serde(default)]
    pub ice_cover: Option<f64>,
    #[serde(default)]
    pub polar_ice: Option<bool>,
    #[serde(default)]
    pub buried_sea: Option<bool>,
    #[serde(default)]
    pub salinity: Option<f64>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Surface {
    #[serde(default)]
    pub terrain: Option<Terrain>,
    /// How far its ground rises and falls (m).
    #[serde(default)]
    pub relief: Option<f64>,
    #[serde(default)]
    pub mean_temperature: Option<f64>,
    #[serde(default)]
    pub temperature_low: Option<f64>,
    #[serde(default)]
    pub temperature_high: Option<f64>,
    #[serde(default)]
    pub highest: Option<f64>,
    #[serde(default)]
    pub lowest: Option<f64>,
    /// Red, green, blue, each 0 to 1.
    #[serde(default)]
    pub colour: Option<[f64; 3]>,
    /// For a frozen body: the file its made landscape is kept in.
    #[serde(default)]
    pub landscape: Option<String>,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Terrain {
    Terran,
    Dry,
    Cratered,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Atmosphere {
    /// At its surface (kg/m³).
    #[serde(default)]
    pub surface_density: Option<f64>,
    /// The height over which the air thins by e (m).
    #[serde(default)]
    pub scale_height: Option<f64>,
    /// Where it is taken to end (m).
    #[serde(default)]
    pub top: Option<f64>,
    #[serde(default)]
    pub surface_pressure: Option<f64>,
    #[serde(default)]
    pub made_of: Vec<Gas>,
    #[serde(default)]
    pub breathable: Option<bool>,
    #[serde(default)]
    pub greenhouse: Option<f64>,
    #[serde(default)]
    pub clouds: Option<f64>,
    #[serde(default)]
    pub wind: Option<f64>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Gas {
    pub gas: String,
    pub share: f64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Life {
    #[serde(default)]
    pub present: Option<LifePresent>,
    #[serde(default)]
    pub soil: Option<Soil>,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum LifePresent {
    #[serde(rename = "none")]
    None,
    #[serde(rename = "microbes")]
    Microbes,
    #[serde(rename = "plants and animals")]
    PlantsAndAnimals,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Soil {
    None,
    Barren,
    Poor,
    Fertile,
}

/// For an asteroid.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BodyRock {
    /// What it is made of, by its spectrum: a rock class, by key.
    #[serde(default)]
    pub class: Option<String>,
    #[serde(default)]
    pub structure: Option<RockStructure>,
    #[serde(default)]
    pub density: Option<f64>,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum RockStructure {
    #[serde(rename = "rubble pile")]
    RubblePile,
    #[serde(rename = "monolith")]
    Monolith,
}

/// `population.<system>.<name>`: many small bodies taken together.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Population {
    pub provenance: Provenance,
    #[serde(default)]
    pub in_game: Option<InGame>,
    pub identity: PopulationIdentity,
    /// For a field the game makes: its rocks.
    #[serde(default)]
    pub rocks: Option<PopulationRocks>,
    /// For a region: how far from the star it lies (m).
    #[serde(default)]
    pub extent: Option<PopulationExtent>,
    #[serde(default)]
    pub basis: Vec<Basis>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PopulationIdentity {
    pub key: String,
    pub name: String,
    pub kind: PopulationKind,
    /// For a field: the named asteroid it gathers round, by key.
    #[serde(default)]
    pub anchor: Option<String>,
    /// For a meteoroid stream: the comet that sheds it, by key.
    #[serde(default)]
    pub parent: Option<String>,
    #[serde(default)]
    pub about: Option<String>,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum PopulationKind {
    #[serde(rename = "family")]
    Family,
    #[serde(rename = "trojan")]
    Trojan,
    #[serde(rename = "outer")]
    Outer,
    #[serde(rename = "scattered disc")]
    ScatteredDisc,
    #[serde(rename = "far cloud")]
    FarCloud,
    #[serde(rename = "meteoroid stream")]
    MeteoroidStream,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PopulationRocks {
    /// A rock class, by key.
    #[serde(default)]
    pub class: Option<String>,
    #[serde(default)]
    pub count: Option<u64>,
    /// How far it spreads round its anchor (m).
    #[serde(default)]
    pub extent: Option<f64>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PopulationExtent {
    #[serde(default)]
    pub inner: Option<f64>,
    #[serde(default)]
    pub outer: Option<f64>,
}
