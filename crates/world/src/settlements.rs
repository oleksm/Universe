//! Settlements' ground, as the registry records it (Local Administration,
//! generated into `content/base/settlements.ron` by `tools/standards/build.py`):
//! zones, parcels, streets, power lines, and facilities with their modules laid
//! out on their parcels. Facts only: the world holds what stands where; what is
//! made there is the economy's.
//!
//! Everything is in metres [east, north] of the settlement's position, which is
//! its spaceport's pad grid centre (the game's own `Spaceport::direction`).

use glam::DVec3;
use serde::Deserialize;

/// A settlement with ground recorded, found by its system, body and name
/// (the spaceport's).
#[derive(Clone, Debug, Deserialize)]
pub struct Settlement {
    pub system: String,
    pub body: String,
    pub name: String,
    pub zones: Vec<Zone>,
    pub parcels: Vec<Parcel>,
    pub streets: Vec<Street>,
    pub power_lines: Vec<PowerLine>,
    pub facilities: Vec<Facility>,
}

/// Ground set aside for a use (port, industrial, commercial, civic, residential).
#[derive(Clone, Debug, Deserialize)]
pub struct Zone {
    pub name: String,
    #[serde(rename = "use")]
    pub use_: String,
    pub outline: Vec<(f64, f64)>,
}

/// A lot of land and who owns it (a Maker House company's key; empty:
/// vacant, the land office's to sell).
#[derive(Clone, Debug, Deserialize)]
pub struct Parcel {
    pub number: u32,
    pub owner: String,
    pub owner_name: String,
    pub outline: Vec<(f64, f64)>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Street {
    pub name: String,
    pub line: Vec<(f64, f64)>,
}

/// A power line and the most it can carry (MW).
#[derive(Clone, Debug, Deserialize)]
pub struct PowerLine {
    pub name: String,
    pub capacity: f64,
    pub line: Vec<(f64, f64)>,
}

/// A facility on a parcel: what kind, and its modules where they stand.
#[derive(Clone, Debug, Deserialize)]
pub struct Facility {
    pub name: String,
    pub kind: String,
    pub parcel: u32,
    /// The most it can do, as the registry works it out from its modules:
    /// what each line makes (product, t/h), the power it draws flat out
    /// (MW), the power it can supply (MW), what it can hold (t).
    pub makes: Vec<(String, f64)>,
    pub draws: f64,
    pub supplies: f64,
    pub holds: f64,
    /// What it's built of: (industrial module, how many), in the order laid out.
    pub modules: Vec<(String, u32)>,
    pub blocks: Vec<Block>,
}

/// One industrial module standing on the ground: its centre, its footprint
/// (length and width, m), its height, and which way its length runs (0 east,
/// 90 north).
#[derive(Clone, Debug, Deserialize)]
pub struct Block {
    pub module: String,
    pub centre: (f64, f64),
    pub length: f64,
    pub width: f64,
    pub height: f64,
    pub heading: f64,
}

impl Block {
    /// Its half sizes along east and north (m).
    pub fn half_extent(&self) -> (f64, f64) {
        if self.heading.rem_euclid(180.0) == 90.0 { (self.width / 2.0, self.length / 2.0) } else { (self.length / 2.0, self.width / 2.0) }
    }
}

impl Settlement {
    /// (A parcel's owner is any Maker House company, not only the makers the
    /// game has as brands: not checked here.)
    pub fn check(&self) -> Result<(), String> {
        for f in &self.facilities {
            if !self.parcels.iter().any(|p| p.number == f.parcel) {
                return Err(format!("settlements.ron '{}': {} stands on no parcel {}", self.name, f.name, f.parcel));
            }
        }
        Ok(())
    }
}

/// A point `(east, north)` metres from a settlement at `dir` (unit, the
/// body's frame) on a body of `radius`: its direction (unit), as the pads'
/// are placed (`spaceport::pad_direction`).
pub fn direction(dir: DVec3, radius: f64, at: (f64, f64)) -> DVec3 {
    let (north, east) = crate::spaceport::tangent(dir);
    (dir * radius + east * at.0 + north * at.1).normalize()
}

/// Is `p` inside the polygon `outline` (even-odd)?
pub fn inside(outline: &[(f64, f64)], p: (f64, f64)) -> bool {
    let mut odd = false;
    for k in 0..outline.len() {
        let (a, b) = (outline[k], outline[(k + 1) % outline.len()]);
        if (a.1 > p.1) != (b.1 > p.1) && p.0 < a.0 + (p.1 - a.1) / (b.1 - a.1) * (b.0 - a.0) {
            odd = !odd;
        }
    }
    odd
}

/// An outline's area (m², shoelace).
pub fn area(outline: &[(f64, f64)]) -> f64 {
    (0..outline.len()).map(|k| { let (a, b) = (outline[k], outline[(k + 1) % outline.len()]); a.0 * b.1 - b.0 * a.1 }).sum::<f64>().abs() / 2.0
}

/// An industrial module (the SFO's, SFO 10): what facilities are built of.
#[derive(Clone, Debug, Deserialize)]
pub struct IndustrialModule {
    pub key: String,
    pub name: String,
    /// Its footprint and height (m).
    pub length: f64,
    pub width: f64,
    pub height: f64,
    /// Power it needs, and supplies (MW).
    pub needs: f64,
    pub supplies: f64,
    /// What it holds (t).
    pub holds: f64,
}
