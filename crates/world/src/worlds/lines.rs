//! A world drawn from its vector lines (the planet graph's root, a `planet-unfold-tiles` package in the
//! worlds store), as the planet lab's anchor model draws it: its level lines, shores, rivers, peaks and
//! lows as lines on the globe (`lines_draw`), over flat ground, land grey and water near black
//! (`lines_colour`, in place of its bake's `globe_color.jpg`). Water is where the shore round a pixel
//! has its low side there: each shore filled on its inside, the smaller of its two sides on the sphere,
//! largest first, so a pixel ends up with the innermost shore around it.

use super::{direction, store, Equirect, LonLat, Package};
use rayon::prelude::*;
use std::f64::consts::PI;

/// The worlds whose colour comes from their lines: body key, package (in the store) and its
/// manifest's hash. (Here until the body's record names its lines.)
const PAINTED: &[(&str, &str, &str)] = &[("body.treistun.treistun-e", "worlds/TRE3/ground/earth_s13-graph-20261009b", "05d07e51c8caea77f22d9ca50b72e1b53cbfe7f0e1a46a524e8078b77b0ea308")];

/// The painted image's size: 4.9 km a pixel at the equator.
const W: usize = 8192;
const H: usize = 4096;

/// Body `key`'s lines. None: it has none (or the store lacks them: said).
fn open(key: &str) -> Option<Vec<Line>> {
    let &(_, folder, sha) = PAINTED.iter().find(|p| p.0 == key)?;
    let lines = (|| -> Result<Vec<Line>, String> {
        let package = Package::open(store().ok_or("no worlds store (set UNIVERSE_WORLDS)")?.join(folder), sha)?;
        load(&package.read("L0_0_0.lines")?)
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
const LAND: [u8; 4] = [58, 58, 62, 255];
const WATER: [u8; 4] = [16, 17, 22, 255];
