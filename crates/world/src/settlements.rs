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

/// A lot of land and who owns it (a Maker House company's key).
#[derive(Clone, Debug, Deserialize)]
pub struct Parcel {
    pub number: u32,
    pub owner: String,
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
