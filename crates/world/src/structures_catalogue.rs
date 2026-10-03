//! The structures catalogue (content: `structures.ron`): stations, spaceports,
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
