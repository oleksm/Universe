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

/// The ground between modules, and between them and a parcel's edges (m):
/// the same invented rule the registry's build lays facilities out by.
pub const LAYOUT_GAP: f64 = 20.0;

/// Modules on a parcel, as the registry lays them out: in rows, the first
/// along the side facing the nearest of `streets`, each row behind the
/// last, each module's length along its row, `LAYOUT_GAP` round each. Only
/// rectangles squared to east and north are laid out. The blocks, or why
/// they don't fit.
pub fn lay_out(outline: &[(f64, f64)], streets: &[Street], order: &[(&IndustrialModule, u32)]) -> Result<Vec<Block>, String> {
    let (w, e) = outline.iter().fold((f64::MAX, f64::MIN), |m, p| (m.0.min(p.0), m.1.max(p.0)));
    let (sth, nth) = outline.iter().fold((f64::MAX, f64::MIN), |m, p| (m.0.min(p.1), m.1.max(p.1)));
    if outline.len() != 4 || outline.iter().any(|p| (p.0 != w && p.0 != e) || (p.1 != sth && p.1 != nth)) {
        return Err("ONLY A RECTANGLE SQUARED TO EAST AND NORTH IS LAID OUT YET".into());
    }
    // (Its front: the side nearest a street; north if there's none.)
    let gap_to = |p: (f64, f64)| streets.iter().flat_map(|s| s.line.windows(2).map(move |w| segment_distance(p, w[0], w[1]))).fold(f64::MAX, f64::min);
    let sides = [("north", ((w + e) / 2.0, nth)), ("south", ((w + e) / 2.0, sth)), ("east", (e, (sth + nth) / 2.0)), ("west", (w, (sth + nth) / 2.0))];
    let front = if streets.is_empty() { "north" } else { sides.iter().min_by(|a, b| gap_to(a.1).total_cmp(&gap_to(b.1))).map_or("north", |s| s.0) };
    let ns = front == "north" || front == "south";
    let (along, deep) = if ns { (e - w, nth - sth) } else { (nth - sth, e - w) };
    let mut rows: Vec<(Vec<(&IndustrialModule, f64)>, f64)> = Vec::new();
    let (mut row, mut at, mut depth) = (Vec::new(), LAYOUT_GAP, 0.0f64);
    for &(m, n) in order {
        for _ in 0..n {
            if m.length > along - 2.0 * LAYOUT_GAP {
                return Err(format!("A {} ({:.0} M LONG) IS LONGER THAN THE PARCEL IS WIDE", m.name.to_uppercase(), m.length));
            }
            if !row.is_empty() && at + m.length > along - LAYOUT_GAP {
                rows.push((std::mem::take(&mut row), depth));
                (at, depth) = (LAYOUT_GAP, 0.0);
            }
            row.push((m, at));
            at += m.length + LAYOUT_GAP;
            depth = depth.max(m.width);
        }
    }
    if !row.is_empty() {
        rows.push((row, depth));
    }
    let mut blocks = Vec::new();
    let mut back = LAYOUT_GAP;
    for (row, d) in rows {
        for (m, a) in row {
            let (u, v) = (a + m.length / 2.0, back + m.width / 2.0);
            let centre = match front {
                "north" => (w + u, nth - v),
                "south" => (w + u, sth + v),
                "east" => (e - v, sth + u),
                _ => (w + v, sth + u),
            };
            blocks.push(Block { module: m.key.clone(), centre, length: m.length, width: m.width, height: m.height, heading: if ns { 0.0 } else { 90.0 } });
        }
        back += d + LAYOUT_GAP;
    }
    if back > deep + 1e-6 {
        return Err(format!("ITS MODULES NEED {back:.0} M OF DEPTH IN ROWS; THE PARCEL HAS {deep:.0} M"));
    }
    Ok(blocks)
}

/// How far `p` is from the segment `a`-`b` (m).
pub fn segment_distance(p: (f64, f64), a: (f64, f64), b: (f64, f64)) -> f64 {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let len2 = dx * dx + dy * dy;
    let t = if len2 == 0.0 { 0.0 } else { (((p.0 - a.0) * dx + (p.1 - a.1) * dy) / len2).clamp(0.0, 1.0) };
    ((p.0 - (a.0 + t * dx)).powi(2) + (p.1 - (a.1 + t * dy)).powi(2)).sqrt()
}

/// Do two rectangles squared to east and north overlap (by more than touching)?
pub fn rects_overlap(a: &[(f64, f64)], b: &[(f64, f64)]) -> bool {
    let bounds = |o: &[(f64, f64)]| o.iter().fold((f64::MAX, f64::MAX, f64::MIN, f64::MIN), |m, p| (m.0.min(p.0), m.1.min(p.1), m.2.max(p.0), m.3.max(p.1)));
    let (a, b) = (bounds(a), bounds(b));
    a.0 < b.2 && b.0 < a.2 && a.1 < b.3 && b.1 < a.3
}
