//! Materials: the world's matter as its machines care about it — real
//! substances at their real properties (the registry's materials, SFO 5: the
//! fuels among them). Tanks hold one, reactors and engines burn one. Dogma knows
//! none of them; which exist and what burns them is the world's.

use serde::Deserialize;

/// How a material gives up its energy.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
pub enum Process {
    Fusion,
    Fission,
    /// Burnt with what it carries (fuel and oxidiser together).
    Chemical,
    /// Inert, or reaction mass only.
    None,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Material {
    pub key: String,
    pub name: String,
    /// As stored (kg/m³).
    pub density: f64,
    /// Released by its process (J/kg).
    pub energy: f64,
    pub process: Process,
    /// The kind of goods it trades as ("": not traded).
    pub goods: String,
    pub note: String,
}

impl Material {
    pub(crate) fn check(&self) -> Result<(), String> {
        if !(self.density.is_finite() && self.density > 0.0) {
            return Err(format!("density must be positive ({})", self.density));
        }
        if !(self.energy.is_finite() && self.energy >= 0.0) {
            return Err(format!("energy can't be negative ({})", self.energy));
        }
        if self.process != Process::None && self.energy <= 0.0 {
            return Err("a fuel must release energy".into());
        }
        Ok(())
    }
}

/// The material keyed `key` of the loaded content.
pub fn material(key: &str) -> Option<&'static Material> {
    let c = crate::content::content();
    c.handle::<Material>(key).map(|h| c.get(h))
}

impl Material {
    /// From the registry's record of a material that is burnt for its energy,
    /// or thrown as reaction mass (one with a `fuel` group); None for any other.
    pub fn from_record(reg: &crate::registry::Registry, m: &crate::registry::Material) -> Option<Self> {
        use crate::registry::MaterialFuelRelease as R;
        let fuel = m.fuel.as_ref()?;
        Some(Material {
            key: m.identity.key.clone(),
            name: crate::standards::caps(&m.identity.name),
            density: m.mass.density.unwrap_or(0.0),
            energy: fuel.energy.unwrap_or(0.0),
            process: match fuel.release {
                R::Fusion => Process::Fusion,
                R::Fission => Process::Fission,
                R::Chemical => Process::Chemical,
                R::None => Process::None,
            },
            goods: reg.traded_as(&m.identity.key).unwrap_or_default(),
            note: m.identity.description.clone().unwrap_or_default(),
        })
    }
}
