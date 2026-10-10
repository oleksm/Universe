//! The structures catalogue (the registry's structures and gate rings): stations, spaceports,
//! outposts and gate rings as products of their makers, like modules and hulls.

use serde::Deserialize;

#[derive(Clone, Debug, PartialEq, Deserialize)]
pub enum StructureKind {
    Station,
    Spaceport,
    Outpost,
    /// An orbital site round a planet or moon: its transceiver and hyper relay.
    Orbital,
    /// A gate ring: its class, the longest throat a pair of them holds (light
    /// years), and the fastest it catches a ship entering it (m/s): faster, the
    /// capture wrecks it.
    GateRing { class: u8, span_ly: f64, capture: f64 },
}

#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Structure {
    pub key: String,
    pub brand: String,
    pub name: String,
    pub kind: StructureKind,
    /// The modules installed (keys): its comm, a gate relay.
    pub fit: Vec<String>,
    pub note: String,
}

impl StructureKind {
    /// On a world's ground (no transceiver or relay works from there).
    pub fn grounded(&self) -> bool {
        matches!(self, StructureKind::Spaceport | StructureKind::Outpost)
    }
}

impl Structure {
    pub(crate) fn check(&self) -> Result<(), String> {
        if let StructureKind::GateRing { span_ly, .. } = self.kind
            && !(span_ly.is_finite() && span_ly > 0.0)
        {
            return Err(format!("a ring's span must be positive ({span_ly})"));
        }
        if let StructureKind::GateRing { capture, .. } = self.kind
            && !(capture.is_finite() && capture > 0.0)
        {
            return Err(format!("a ring's capture speed must be positive ({capture})"));
        }
        Ok(())
    }
}

impl Structure {
    /// The registry's structures (`structure.*`) and gate rings (`gate.*`),
    /// fitted with the equipment the game has (`has`: a key it knows; a
    /// gate's throat coils it doesn't make yet are left off).
    pub fn from_registry(reg: &crate::registry::Registry, has: impl Fn(&str) -> bool) -> Vec<Self> {
        use crate::registry::StructureIdentityKind as K;
        let fit = |f: Vec<(&String, u32)>| f.into_iter().filter(|(item, _)| has(item)).flat_map(|(item, n)| std::iter::repeat_n(item.clone(), n as usize)).collect::<Vec<_>>();
        let structures = reg.structures.iter().map(|s| Structure {
            key: s.identity.key.clone(),
            brand: s.identity.maker.clone(),
            name: crate::standards::caps(&s.identity.name),
            kind: match s.identity.kind {
                K::Station => StructureKind::Station,
                K::Spaceport => StructureKind::Spaceport,
                K::Outpost => StructureKind::Outpost,
                K::Orbital => StructureKind::Orbital,
            },
            fit: fit(s.fit.iter().map(|x| (&x.item, x.count.unwrap_or(1))).collect()),
            note: s.identity.description.clone().unwrap_or_default(),
        });
        let rings = reg.gates.iter().map(|g| {
            let p = &g.performance;
            Structure {
                key: g.identity.key.clone(),
                brand: g.identity.maker.clone().unwrap_or_default(),
                name: crate::standards::caps(&g.identity.name),
                kind: StructureKind::GateRing {
                    class: g.identity.class.unwrap_or(1) as u8,
                    span_ly: p.span.unwrap_or(0.0) / crate::units::LIGHT_YEAR,
                    capture: p.capture_speed.unwrap_or(0.0),
                },
                fit: fit(g.fit.iter().map(|x| (&x.item, x.count)).collect()),
                note: g.identity.description.clone().unwrap_or_default(),
            }
        });
        structures.chain(rings).collect()
    }
}
