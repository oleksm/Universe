//! A world drawn from its vector lines (the planet graph's root, the `planet-unfold-tiles` package its
//! record's `ground` names in the worlds store), as the planet lab's anchor model draws it: its level lines, shores, rivers, peaks and
//! lows as lines on the globe (`lines_draw`), over flat ground, land grey and water near black
//! (`lines_colour`, in place of its bake's `globe_color.jpg`). Water is where the shore round a pixel
//! has its low side there: each shore filled on its inside, the smaller of its two sides on the sphere,
//! largest first, so a pixel ends up with the innermost shore around it.

use super::{direction, store, Equirect, LonLat, Package};
use rayon::prelude::*;
use std::f64::consts::PI;

/// The painted image's size: 4.9 km a pixel at the equator.
const W: usize = 8192;
const H: usize = 4096;

/// Body `key`'s lines: the root of the ground its record names (`ground`, a `planet-unfold-tiles`
/// package in the worlds store, checked by its manifest's hash). None: it has none (or the store
/// lacks them: said).
fn open(key: &str) -> Option<Vec<Line>> {
    let g = crate::registry::registry().body(key)?.ground.as_ref()?;
    let lines = (|| -> Result<Vec<Line>, String> {
        let folder = store().ok_or("no worlds store (set UNIVERSE_WORLDS)")?.join(&g.path);
        let package = Package::open(folder.clone(), &g.manifest_sha256)?;
        // (The root tile, as the manifest names it.)
        let manifest: serde_json::Value = serde_json::from_slice(&std::fs::read(folder.join("manifest.json")).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
        let root = manifest.get("l0").and_then(|v| v.as_str()).unwrap_or("L0_0_0.lines");
        load(&package.read(root)?)
    })();
    lines.map_err(|e| eprintln!("{key}: no lines: {e}")).ok()
}

/// Body `key`'s ground under its lines: land grey, water near black.
pub fn lines_colour(key: &str) -> Option<Equirect> {
    let rgba = paint(&open(key)?, W, H);
    Some(Equirect { width: W, height: H, rgb: rgba.chunks(4).flat_map(|p| [p[0], p[1], p[2]]).collect() })
}

/// Lines to draw on a unit globe: vertex positions, their colours (linear RGBA), and the edges between them.
pub struct LinesDraw {
    pub positions: Vec<[f32; 3]>,
    pub colors: Vec<[f32; 4]>,
    pub edges: Vec<[u32; 2]>,
    /// The highest peaks and the lowest lows (`PINS` of each), to pin with their heights.
    pub pins: Vec<Pin>,
}

/// A peak or a low to pin: its body direction (unit), its height (m from the sea), which.
pub struct Pin {
    pub dir: glam::DVec3,
    pub z: f32,
    pub peak: bool,
}

/// How many of the highest peaks and of the lowest lows are pinned.
const PINS: usize = 5;

/// The anchor model's colours (sRGB): shores white, land's level lines tan (paler above 2,000 m), the
/// sea floor's blue, rivers pale blue, dry traces brown, peaks red and lows blue.
const SHORE: [u8; 3] = [255, 255, 255];
const LAND_LOW: [u8; 3] = [150, 120, 78];
const LAND_HIGH: [u8; 3] = [226, 192, 130];
const SEA_FLOOR: [u8; 3] = [70, 112, 186];
const RIVER: [u8; 3] = [120, 180, 240];
const DRY_TRACE: [u8; 3] = [138, 112, 80];
const PEAK: [u8; 3] = [232, 64, 52];
const LOW: [u8; 3] = [64, 120, 240];
/// A peak's or low's mark: a ring this wide (radians) round it.
const MARK: f64 = 0.004;

/// Body `key`'s lines as the anchor model draws them, on a globe of radius `lift` (1: the datum).
pub fn lines_draw(key: &str, lift: f64) -> Option<LinesDraw> {
    let lines = open(key)?;
    let lin = |c: [u8; 3]| {
        let f = |v: u8| {
            let v = v as f32 / 255.0;
            if v <= 0.04045 { v / 12.92 } else { ((v + 0.055) / 1.055).powf(2.4) }
        };
        [f(c[0]), f(c[1]), f(c[2]), 1.0]
    };
    let at = |lon: f64, lat: f64| (direction(LonLat { lon, lat }) * lift).as_vec3().to_array();
    let mut d = LinesDraw { positions: Vec::new(), colors: Vec::new(), edges: Vec::new(), pins: Vec::new() };
    for peak in [true, false] {
        let flag = if peak { FLAG_PEAK } else { FLAG_LOW };
        let mut anchors: Vec<&Line> = lines.iter().filter(|l| l.kind == KIND_ANCHOR && l.flags & flag != 0 && !l.pts.is_empty()).collect();
        anchors.sort_by(|a, b| if peak { b.z.total_cmp(&a.z) } else { a.z.total_cmp(&b.z) });
        d.pins.extend(anchors.iter().take(PINS).map(|l| Pin { dir: direction(LonLat { lon: l.pts[0].0, lat: l.pts[0].1 }), z: l.z, peak }));
    }
    for l in &lines {
        let colour = match l.kind {
            KIND_COAST => SHORE,
            KIND_CONTOUR if l.z < 0.0 => SEA_FLOOR,
            KIND_CONTOUR if l.z > 2000.0 => LAND_HIGH,
            KIND_CONTOUR => LAND_LOW,
            KIND_RIVER if l.flags & DRY != 0 => DRY_TRACE,
            KIND_RIVER => RIVER,
            KIND_ANCHOR if l.flags & FLAG_PEAK != 0 => PEAK,
            KIND_ANCHOR if l.flags & FLAG_LOW != 0 => LOW,
            _ => continue,
        };
        let c = lin(colour);
        let first = d.positions.len() as u32;
        if l.kind == KIND_ANCHOR {
            // (A small ring round it, in the plane across it.)
            let Some(&(lon, lat)) = l.pts.first() else { continue };
            let n = direction(LonLat { lon, lat });
            let u = n.any_orthonormal_vector();
            let v = n.cross(u);
            for k in 0..8 {
                let a = k as f64 / 8.0 * std::f64::consts::TAU;
                d.positions.push(((n + (u * a.cos() + v * a.sin()) * MARK).normalize() * lift).as_vec3().to_array());
                d.colors.push(c);
                d.edges.push([first + k, first + (k + 1) % 8]);
            }
            continue;
        }
        for &(lon, lat) in &l.pts {
            d.positions.push(at(lon, lat));
            d.colors.push(c);
        }
        let n = l.pts.len() as u32;
        for k in 1..n {
            d.edges.push([first + k - 1, first + k]);
        }
        if l.flags & CLOSED != 0 && n > 2 {
            d.edges.push([first + n - 1, first]);
        }
    }
    Some(d)
}

/// The same as RGBA rows (width, height, bytes), for the near maps.
pub fn lines_colour_rgba(key: &str) -> Option<(usize, usize, Vec<u8>)> {
    let e = lines_colour(key)?;
    Some((e.width, e.height, e.rgb.chunks(3).flat_map(|p| [p[0], p[1], p[2], 255]).collect()))
}

const KIND_CONTOUR: u8 = 0;
const KIND_RIVER: u8 = 2;
const KIND_COAST: u8 = 4;
const KIND_ANCHOR: u8 = 10;
const CLOSED: u16 = 1;
const DRY: u16 = 1 << 3;
const FLAG_PEAK: u16 = 1 << 5;
const FLAG_LOW: u16 = 1 << 6;

struct Line {
    kind: u8,
    flags: u16,
    /// (lon, lat) in degrees.
    pts: Vec<(f64, f64)>,
    /// Metres from the sea.
    z: f32,
}

/// The lines of a PTL2 file.
fn load(b: &[u8]) -> Result<Vec<Line>, String> {
    if b.len() < 59 || &b[0..4] != b"PTL2" {
        return Err("not a PTL2 line file".into());
    }
    let u32_at = |at: usize| u32::from_le_bytes(b[at..at + 4].try_into().unwrap());
    let f64_at = |at: usize| f64::from_le_bytes(b[at..at + 8].try_into().unwrap());
    let f32_at = |at: usize| f32::from_le_bytes(b[at..at + 4].try_into().unwrap());
    let mut at = 55;
    let n = u32_at(at) as usize;
    at += 4;
    let mut lines = Vec::with_capacity(n);
    for _ in 0..n {
        if at + 61 > b.len() {
            return Err("the file ends early".into());
        }
        let kind = b[at];
        let flags = u16::from_le_bytes([b[at + 9], b[at + 10]]);
        at += 1 + 8 + 2 + 2 + 4 + 32 + 8;
        let nv = u32_at(at) as usize;
        at += 4;
        if at + nv * 28 > b.len() {
            return Err("the file ends early".into());
        }
        let pts = (0..nv).map(|k| (f64_at(at + k * 28), f64_at(at + k * 28 + 8))).collect();
        let z = if nv > 0 { f32_at(at + 16) } else { 0.0 };
        at += nv * 28;
        lines.push(Line { kind, flags, pts, z });
    }
    Ok(lines)
}

/// A closed line ready to fill: its inside as a plane polygon in unwrapped longitude, whether that
/// polygon is the inside or its complement, the inside's area and whether the inside is the high side.
struct Loop {
    poly: Vec<(f64, f64)>,
    complement: bool,
    area: f64,
    inside_high: bool,
}

fn prepare(l: &Line) -> Option<Loop> {
    if l.pts.len() < 3 {
        return None;
    }
    // Longitudes unwrapped so the line runs continuously.
    let mut poly = Vec::with_capacity(l.pts.len() + 4);
    let mut lon = l.pts[0].0;
    poly.push((lon, l.pts[0].1));
    for w in l.pts.windows(2) {
        let mut d = w[1].0 - w[0].0;
        d -= 360.0 * (d / 360.0).round();
        lon += d;
        poly.push((lon, w[1].1));
    }
    let mut d = l.pts[0].0 - l.pts[l.pts.len() - 1].0;
    d -= 360.0 * (d / 360.0).round();
    let net = lon + d - l.pts[0].0;
    let winding = (net / 360.0).round() as i32;
    // The area on the line's left: the integral of (s - sin lat) d lon round it, s the winding's sign.
    let s = winding.signum() as f64;
    let mut left = 0.0;
    let n = poly.len();
    for k in 0..n {
        let (a, b) = (poly[k], if k + 1 < n { poly[k + 1] } else { (poly[0].0 + net, poly[0].1) });
        let lat_mid = ((a.1 + b.1) * 0.5).to_radians();
        left += (s - lat_mid.sin()) * (b.0 - a.0).to_radians();
    }
    let left = left.rem_euclid(4.0 * PI);
    let inside_high = left <= 2.0 * PI;
    let area = left.min(4.0 * PI - left);
    let complement;
    if winding == 0 {
        // The plane polygon's own area on the sphere says whether it is the inside.
        let mut a = 0.0;
        for k in 0..n {
            let (p, q) = (poly[k], poly[(k + 1) % n]);
            a -= ((p.1 + q.1) * 0.5).to_radians().sin() * (q.0 - p.0).to_radians();
        }
        complement = a.abs() > 2.0 * PI;
    } else {
        // Round a pole: closed over the pole on the inside's side.
        let north_is_left = winding > 0;
        let inside_north = north_is_left == inside_high;
        let pole = if inside_north { 90.0 } else { -90.0 };
        let end = poly[0].0 + net;
        poly.push((end, poly[0].1));
        poly.push((end, pole));
        poly.push((poly[0].0, pole));
        complement = false;
    }
    Some(Loop { poly, complement, area, inside_high })
}

/// Fills each row with the index of the innermost loop around each pixel (u32::MAX: none).
fn innermost(loops: &[Loop], w: usize, h: usize) -> Vec<u32> {
    // Each edge's row crossings, by row: (loop index, x).
    let mut rows: Vec<Vec<(u32, f64)>> = vec![Vec::new(); h];
    let (sx, sy) = (w as f64 / 360.0, h as f64 / 180.0);
    for (i, lp) in loops.iter().enumerate() {
        let n = lp.poly.len();
        for k in 0..n {
            let (a, b) = (lp.poly[k], lp.poly[(k + 1) % n]);
            let (x0, y0, x1, y1) = ((a.0 + 180.0) * sx, (90.0 - a.1) * sy, (b.0 + 180.0) * sx, (90.0 - b.1) * sy);
            let (lo, hi) = (y0.min(y1), y0.max(y1));
            let r0 = (lo - 0.5).ceil().max(0.0) as usize;
            let r1 = ((hi - 0.5).ceil().max(0.0) as usize).min(h);
            for r in r0..r1 {
                let yc = r as f64 + 0.5;
                let t = (yc - y0) / (y1 - y0);
                rows[r].push((i as u32, x0 + t * (x1 - x0)));
            }
        }
    }
    let complements: Vec<u32> = (0..loops.len() as u32).filter(|&i| loops[i as usize].complement).collect();
    let mut out = vec![u32::MAX; w * h];
    out.par_chunks_mut(w).zip(rows.par_iter_mut()).for_each(|(row, xs)| {
        xs.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.total_cmp(&b.1)));
        let mut mask = vec![false; w];
        let mut c = 0;
        let mut ci = 0;
        // Loops in order (largest inside first); a complement loop with no crossing in this row still fills it.
        loop {
            let next_x = xs.get(c).map(|e| e.0);
            let next_c = complements.get(ci).copied();
            let i = match (next_x, next_c) {
                (None, None) => break,
                (Some(a), Some(b)) => a.min(b),
                (Some(a), None) => a,
                (None, Some(b)) => b,
            };
            if next_c == Some(i) {
                ci += 1;
            }
            let start = c;
            while c < xs.len() && xs[c].0 == i {
                c += 1;
            }
            let lp = &loops[i as usize];
            mask.iter_mut().for_each(|m| *m = false);
            for pair in xs[start..c].chunks(2) {
                if pair.len() < 2 {
                    continue;
                }
                let (a, b) = (pair[0].1, pair[1].1);
                let (c0, c1) = ((a - 0.5).ceil() as i64, (b - 0.5).ceil() as i64);
                for x in c0..c1.min(c0 + w as i64) {
                    mask[x.rem_euclid(w as i64) as usize] = true;
                }
            }
            for (o, &m) in row.iter_mut().zip(&mask) {
                if m != lp.complement {
                    *o = i;
                }
            }
        }
    });
    out
}

/// The ground under the lines, painted as RGBA: land grey, water near black.
fn paint(lines: &[Line], w: usize, h: usize) -> Vec<u8> {
    let mut shores: Vec<Loop> = lines.iter().filter(|l| l.kind == KIND_COAST && l.flags & CLOSED != 0).filter_map(prepare).collect();
    shores.sort_by(|a, b| b.area.total_cmp(&a.area));
    let mut rgba = vec![0u8; w * h * 4];
    if shores.is_empty() {
        rgba.chunks_mut(4).for_each(|px| px.copy_from_slice(&LAND));
        return rgba;
    }
    let water = innermost(&shores, w, h);
    rgba.par_chunks_mut(4).enumerate().for_each(|(p, px)| {
        // (Outside every shore's inside: the outside of the largest.)
        let (lp, inside) = if water[p] == u32::MAX { (&shores[0], false) } else { (&shores[water[p] as usize], true) };
        px.copy_from_slice(if lp.inside_high != inside { &WATER } else { &LAND });
    });
    rgba
}

/// The ground's two colours (sRGB): land, and water.
const LAND: [u8; 4] = [96, 96, 100, 255];
const WATER: [u8; 4] = [16, 17, 22, 255];

/// A world's ground from its lines: the height anywhere, by the band rule. The nearest level line
/// (a loop or a shore) gives one height and, by which side of it the point is (the high side on its
/// left), which way the ground goes; the nearest line beyond it that way (a level line higher on the
/// high side, lower on the low; a peak above, a low below) gives the other; the height is the straight
/// blend between the two by distance. Exact on every line and anchor; the sea's is the ground under it.
pub struct LineHeights {
    segs: Vec<Seg>,
    /// Segment indices by cell (`CELL_DEG` of latitude and longitude, row 0 at the south pole).
    cells: Vec<Vec<u32>>,
    anchors: Vec<(DVec3, f32, bool)>,
    /// The highest ground (m, unstretched).
    max: f64,
}

/// The heights' stretch for viewing (see `set_stretch`), as f32 bits.
static STRETCH: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0x3f80_0000);

/// Every world's ground from its lines stretched upward this many times, for seeing relief the lines
/// hold too gently to show (1: as they are). The ground is the ground: drawing, landing and collisions
/// all see it stretched.
pub fn set_stretch(k: f64) {
    STRETCH.store((k as f32).to_bits(), std::sync::atomic::Ordering::Relaxed);
}

/// The heights' stretch now (see `set_stretch`).
pub fn stretch() -> f64 {
    f32::from_bits(STRETCH.load(std::sync::atomic::Ordering::Relaxed)) as f64
}

/// A level line's segment: its ends (unit), its height.
struct Seg {
    a: DVec3,
    b: DVec3,
    z: f32,
}

use glam::DVec3;

/// The cell size of the segments' index (degrees).
const CELL_DEG: f64 = 1.0;
const COLS: usize = (360.0 / CELL_DEG) as usize;
const ROWS: usize = (180.0 / CELL_DEG) as usize;
/// The farthest a line is looked for (cells round the point's): about 1,300 km.
const REACH_CELLS: usize = 12;
/// Where no line beyond is in reach: as if the next level were this far beyond the nearest line (m
/// of height, and radians).
const STEP: f32 = 200.0;
const BEYOND: f64 = 0.01;

impl LineHeights {
    /// Body `key`'s ground from its lines, made once and shared. None: it has none.
    pub fn of(key: &str) -> Option<std::sync::Arc<LineHeights>> {
        use std::collections::HashMap;
        use std::sync::{Arc, Mutex, OnceLock};
        static MADE: OnceLock<Mutex<HashMap<String, Option<Arc<LineHeights>>>>> = OnceLock::new();
        crate::registry::registry().body(key)?.ground.as_ref()?;
        let mut made = MADE.get_or_init(Default::default).lock().ok()?;
        made.entry(key.to_string()).or_insert_with(|| open(key).map(|l| Arc::new(LineHeights::new(&l)))).clone()
    }

    fn new(lines: &[Line]) -> LineHeights {
        let dir = |p: (f64, f64)| direction(LonLat { lon: p.0, lat: p.1 });
        let mut segs = Vec::new();
        let mut anchors = Vec::new();
        let mut max = 0.0f64;
        for l in lines {
            max = max.max(l.z as f64);
            match l.kind {
                KIND_CONTOUR | KIND_COAST => {
                    let n = l.pts.len();
                    let edges = if l.flags & CLOSED != 0 && n > 2 { n } else { n.saturating_sub(1) };
                    for k in 0..edges {
                        segs.push(Seg { a: dir(l.pts[k]), b: dir(l.pts[(k + 1) % n]), z: l.z });
                    }
                }
                KIND_ANCHOR if !l.pts.is_empty() => anchors.push((dir(l.pts[0]), l.z, l.flags & FLAG_PEAK != 0)),
                _ => {}
            }
        }
        let mut cells = vec![Vec::new(); COLS * ROWS];
        for (i, s) in segs.iter().enumerate() {
            // (Each segment in every cell its ends' box spans: they are short.)
            let (pa, pb) = (super::lon_lat(s.a), super::lon_lat(s.b));
            let (r0, r1) = (row(pa.lat.min(pb.lat)), row(pa.lat.max(pb.lat)));
            let (mut c0, mut c1) = (col(pa.lon), col(pb.lon));
            if c0 > c1 {
                std::mem::swap(&mut c0, &mut c1);
            }
            // (Across the date line: the short way round.)
            let cols: Vec<usize> = if c1 - c0 > COLS / 2 { (c1..COLS).chain(0..=c0).collect() } else { (c0..=c1).collect() };
            for r in r0..=r1 {
                for &c in &cols {
                    cells[r * COLS + c].push(i as u32);
                }
            }
        }
        LineHeights { segs, cells, anchors, max }
    }

    /// The nearest segment to `q` (unit) passing `keep`: its index, the angle to it, and the point
    /// on it nearest.
    fn nearest(&self, q: DVec3, keep: impl Fn(&Seg) -> bool) -> Option<(usize, f64, DVec3)> {
        let p = super::lon_lat(q);
        let (r, c) = (row(p.lat), col(p.lon));
        let cell = CELL_DEG.to_radians();
        let mut best: Option<(usize, f64, DVec3)> = None;
        let mut k = 1;
        loop {
            let shrink = p.lat.to_radians().cos().max(0.02);
            let kc = ((k as f64 / shrink).ceil() as usize).min(COLS / 2);
            for rr in r.saturating_sub(k)..=(r + k).min(ROWS - 1) {
                for dc in -(kc as i64)..=kc as i64 {
                    let cc = (c as i64 + dc).rem_euclid(COLS as i64) as usize;
                    for &i in &self.cells[rr * COLS + cc] {
                        let s = &self.segs[i as usize];
                        if !keep(s) {
                            continue;
                        }
                        let (d, foot) = to_segment(q, s.a, s.b);
                        if best.is_none_or(|b| d < b.1) {
                            best = Some((i as usize, d, foot));
                        }
                    }
                }
            }
            // (Found within the box searched: nothing outside it is nearer.)
            if best.is_some_and(|b| b.1 <= (k as f64 - 0.5) * cell) || k >= REACH_CELLS {
                return best;
            }
            k *= 2;
        }
    }

    /// The highest ground (m), as stretched now.
    pub fn max(&self) -> f64 {
        self.max * stretch()
    }

    /// The ground's height (m from the sea; below 0, the sea floor) at body direction `dir`, as
    /// stretched now (see `set_stretch`).
    pub fn at(&self, dir: DVec3) -> f64 {
        self.at_unstretched(dir) * stretch()
    }

    /// The ground's height as the lines hold it.
    fn at_unstretched(&self, dir: DVec3) -> f64 {
        let q = dir.normalize();
        let Some((ia, da, foot)) = self.nearest(q, |_| true) else { return 0.0 };
        let a = &self.segs[ia];
        // (Which side: the high side is on the line's left, up × along.)
        let left = foot.cross(a.b - a.a);
        let high = (q - foot).dot(left) >= 0.0;
        let za = a.z;
        let beyond = |z: f32| if high { z > za } else { z < za };
        let mut b = self.nearest(q, |s| beyond(s.z)).map(|(i, d, _)| (self.segs[i].z, d));
        for &(p, z, peak) in &self.anchors {
            if peak == high && beyond(z) {
                let d = p.angle_between(q);
                if b.is_none_or(|b| d < b.1) {
                    b = Some((z, d));
                }
            }
        }
        let (zb, db) = b.unwrap_or((if high { za + STEP } else { za - STEP }, da + BEYOND));
        let t = if da + db > 0.0 { da / (da + db) } else { 0.0 };
        za as f64 + (zb - za) as f64 * t
    }
}

fn row(lat: f64) -> usize {
    (((lat + 90.0) / CELL_DEG).floor() as usize).min(ROWS - 1)
}

fn col(lon: f64) -> usize {
    (((lon + 180.0) / CELL_DEG).floor() as i64).rem_euclid(COLS as i64) as usize
}

/// The angle from `q` to the segment `a`–`b` (unit vectors, short), and the point on it nearest.
fn to_segment(q: DVec3, a: DVec3, b: DVec3) -> (f64, DVec3) {
    let ab = b - a;
    let t = if ab.length_squared() > 0.0 { ((q - a).dot(ab) / ab.length_squared()).clamp(0.0, 1.0) } else { 0.0 };
    let foot = (a + ab * t).normalize();
    (foot.angle_between(q), foot)
}

impl std::fmt::Debug for LineHeights {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "LineHeights({} segments, {} anchors)", self.segs.len(), self.anchors.len())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Hoar's ground from its lines (where the store holds them): exact on its lines and anchors,
    /// between neighbouring levels elsewhere, and quick.
    #[test]
    fn ground_from_lines() {
        let Some(g) = LineHeights::of("body.treistun.treistun-e") else { return };
        let lines = open("body.treistun.treistun-e").unwrap();
        for l in lines.iter().filter(|l| l.kind == KIND_CONTOUR).step_by(97).take(40) {
            let p = l.pts[l.pts.len() / 2];
            let h = g.at_unstretched(direction(LonLat { lon: p.0, lat: p.1 }));
            assert!((h - l.z as f64).abs() < 1.0, "on a {} m line: {h}", l.z);
        }
        for &(p, z, _) in g.anchors.iter().step_by(37) {
            assert!((g.at_unstretched(p) - z as f64).abs() < 1.0, "on a {z} m anchor: {}", g.at_unstretched(p));
        }
        let t = std::time::Instant::now();
        let n = 20_000;
        let mut lo = f64::MAX;
        let mut hi = f64::MIN;
        for k in 0..n {
            // (A spiral over the whole sphere.)
            let y = 1.0 - 2.0 * (k as f64 + 0.5) / n as f64;
            let a = k as f64 * 2.399_963;
            let r = (1.0 - y * y).sqrt();
            let h = g.at_unstretched(DVec3::new(r * a.cos(), y, r * a.sin()));
            (lo, hi) = (lo.min(h), hi.max(h));
        }
        let us = t.elapsed().as_secs_f64() * 1e6 / n as f64;
        eprintln!("{us:.1} µs a height; {lo:.0} to {hi:.0} m");
        assert!(lo >= -3_800.0 && hi <= 8_600.0, "{lo} to {hi}");
    }
}
