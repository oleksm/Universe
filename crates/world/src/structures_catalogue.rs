//! The structures catalogue (content: `structures.ron`): stations, spaceports,
//! outposts and gate rings as products of their makers, like modules and hulls.

use serde::Deserialize;

#[derive(Clone, Debug, PartialEq, Deserialize)]
pub enum StructureKind {
    Station,
    Spaceport,
    Outpost,
    /// A world's relay: a constellation round a planet or moon.
    Relay,
    /// A gate ring: its class, and the longest throat a pair of them holds (light years).
    GateRing { class: u8, span_ly: f64 },
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

impl Structure {
    pub(crate) fn check(&self) -> Result<(), String> {
        if let StructureKind::GateRing { span_ly, .. } = self.kind
            && !(span_ly.is_finite() && span_ly > 0.0)
        {
            return Err(format!("a ring's span must be positive ({span_ly})"));
        }
        Ok(())
    }
}
