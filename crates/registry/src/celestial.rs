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
