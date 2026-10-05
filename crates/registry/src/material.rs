//! `material.*`: a material (SFO 5) at its properties. Every property is
//! optional: a record says what is known.

use serde::{Deserialize, Serialize};

use crate::Basis;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Material {
    pub identity: MaterialIdentity,
    /// For one burnt for its energy, or thrown as reaction mass.
    #[serde(default)]
    pub fuel: Option<Fuel>,
    #[serde(default)]
    pub mass: Option<MaterialMass>,
    #[serde(default)]
    pub mechanical: Option<Mechanical>,
    #[serde(default)]
    pub thermal: Option<Thermal>,
    #[serde(default)]
    pub electrical_magnetic: Option<ElectricalMagnetic>,
    #[serde(default)]
    pub optical: Option<Optical>,
    #[serde(default)]
    pub environment: Option<Environment>,
    #[serde(default)]
    pub making: Option<MaterialMaking>,
    #[serde(default)]
    pub basis: Vec<Basis>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MaterialIdentity {
    pub key: String,
    #[serde(default)]
    pub traded_as: Option<String>,
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    pub class: MaterialClass,
    #[serde(default)]
    pub composition: Vec<crate::Part>,
    #[serde(default)]
    pub form: Vec<Form>,
    #[serde(default)]
    pub grade: Option<String>,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum MaterialClass {
    Metal,
    Alloy,
    Ceramic,
    Glass,
    Polymer,
    Composite,
    Fluid,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Form {
    Ingot,
    Sheet,
    Plate,
    Bar,
    Tube,
    Wire,
    Fibre,
    Fabric,
    Foam,
    Film,
    Tile,
    Casting,
    Forging,
    Powder,
    Fluid,
    Scrap,
    Blank,
    Panel,
}

/// How a fuel gives up its energy, and how much (J/kg).
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Fuel {
    pub release: Release,
    #[serde(default)]
    pub energy: Option<f64>,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Release {
    Fusion,
    Fission,
    /// Burnt with the oxidiser it carries.
    Chemical,
    /// Inert, or reaction mass.
    None,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MaterialMass {
    /// kg/m³.
    #[serde(default)]
    pub density: Option<f64>,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Rating {
    None,
    Poor,
    Fair,
    Good,
    Excellent,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Level {
    None,
    Low,
    Medium,
    High,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Magnetism {
    Ferromagnetic,
    Paramagnetic,
    Diamagnetic,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Joining {
    Welded,
    Brazed,
    Bonded,
    Bolted,
    Riveted,
    Sewn,
}


#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Mechanical {
    #[serde(default)]
    pub youngs_modulus: Option<f64>,
    #[serde(default)]
    pub shear_modulus: Option<f64>,
    #[serde(default)]
    pub poissons_ratio: Option<f64>,
    #[serde(default)]
    pub yield_strength: Option<f64>,
    #[serde(default)]
    pub tensile_strength: Option<f64>,
    #[serde(default)]
    pub compressive_strength: Option<f64>,
    #[serde(default)]
    pub shear_strength: Option<f64>,
    #[serde(default)]
    pub elongation_at_break: Option<f64>,
    #[serde(default)]
    pub hardness: Option<f64>,
    #[serde(default)]
    pub fracture_toughness: Option<f64>,
    #[serde(default)]
    pub impact_toughness: Option<f64>,
    #[serde(default)]
    pub fatigue_limit: Option<f64>,
    #[serde(default)]
    pub creep_resistance: Option<Rating>,
    #[serde(default)]
    pub wear_resistance: Option<Rating>,
    #[serde(default)]
    pub friction_coefficient: Option<f64>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Thermal {
    #[serde(default)]
    pub melting_point: Option<f64>,
    #[serde(default)]
    pub max_service_temperature: Option<f64>,
    #[serde(default)]
    pub min_service_temperature: Option<f64>,
    #[serde(default)]
    pub thermal_conductivity: Option<f64>,
    #[serde(default)]
    pub specific_heat_capacity: Option<f64>,
    #[serde(default)]
    pub thermal_expansion: Option<f64>,
    #[serde(default)]
    pub thermal_shock_resistance: Option<Rating>,
    #[serde(default)]
    pub emissivity: Option<f64>,
    #[serde(default)]
    pub absorptivity: Option<f64>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ElectricalMagnetic {
    #[serde(default)]
    pub electrical_conductivity: Option<f64>,
    #[serde(default)]
    pub dielectric_strength: Option<f64>,
    #[serde(default)]
    pub magnetic: Option<Magnetism>,
    #[serde(default)]
    pub superconducting_temperature: Option<f64>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Optical {
    #[serde(default)]
    pub transparency: Option<f64>,
    #[serde(default)]
    pub refractive_index: Option<f64>,
    #[serde(default)]
    pub reflectivity: Option<f64>,
    #[serde(default)]
    pub colour: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Environment {
    #[serde(default)]
    pub corrosion_resistance: Option<Rating>,
    #[serde(default)]
    pub oxidation_resistance: Option<Rating>,
    #[serde(default)]
    pub chemical_resistance: Option<Rating>,
    #[serde(default)]
    pub outgassing: Option<f64>,
    #[serde(default)]
    pub radiation_resistance: Option<Rating>,
    #[serde(default)]
    pub radiation_shielding: Option<Rating>,
    #[serde(default)]
    pub uv_resistance: Option<Rating>,
    #[serde(default)]
    pub flammability: Option<Level>,
    #[serde(default)]
    pub permeability: Option<Rating>,
    #[serde(default)]
    pub toxicity: Option<Level>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MaterialMaking {
    #[serde(default)]
    pub formability: Option<Rating>,
    #[serde(default)]
    pub machinability: Option<Rating>,
    #[serde(default)]
    pub weldability: Option<Rating>,
    #[serde(default)]
    pub joining: Vec<Joining>,
    #[serde(default)]
    pub castability: Option<Rating>,
    #[serde(default)]
    pub repairability: Option<Rating>,
    #[serde(default)]
    pub recyclability: Option<Rating>,
}
