//! A world's colour painted from its vector lines (the planet graph's root, a `planet-unfold-tiles`
//! package in the worlds store): an equirectangular image in place of its bake's `globe_color.jpg`.
//! Every closed level line (loops and shores) is filled on its inside, the smaller of its two sides on
//! the sphere, largest first, so a pixel ends up with the innermost line around it. Its band is that
//! line's height and which side of it the pixel is on (the high side is on the line's left). Water is
//! the same with the shores alone: a pixel is water where the shore around it has its low side there.
//! Rivers that carry water are drawn over it.

use super::{store, Equirect, Package};
use rayon::prelude::*;
use std::f64::consts::PI;

/// The worlds whose colour comes from their lines: body key, package (in the store) and its
/// manifest's hash. (Here until the body's record names its lines.)
const PAINTED: &[(&str, &str, &str)] = &[("body.treistun.treistun-e", "worlds/TRE3/ground/earth_s13-graph-20261009b", "05d07e51c8caea77f22d9ca50b72e1b53cbfe7f0e1a46a524e8078b77b0ea308")];

/// The painted image's size: 4.9 km a pixel at the equator.
const W: usize = 8192;
const H: usize = 4096;

/// Body `key`'s colour painted from its lines. None: it has none (or the store lacks them: said).
pub fn lines_colour(key: &str) -> Option<Equirect> {
    let &(_, folder, sha) = PAINTED.iter().find(|p| p.0 == key)?;
    let painted = (|| -> Result<Equirect, String> {
        let package = Package::open(store().ok_or("no worlds store (set UNIVERSE_WORLDS)")?.join(folder), sha)?;
        let lines = load(&package.read("L0_0_0.lines")?)?;
        let rgba = paint(&lines, W, H);
        Ok(Equirect { width: W, height: H, rgb: rgba.chunks(4).flat_map(|p| [p[0], p[1], p[2]]).collect() })
    })();
    painted.map_err(|e| eprintln!("{key}: no colour from its lines: {e}")).ok()
}

/// The same as RGBA rows (width, height, bytes), for the near maps.
pub fn lines_colour_rgba(key: &str) -> Option<(usize, usize, Vec<u8>)> {
    let e = lines_colour(key)?;
    Some((e.width, e.height, e.rgb.chunks(3).flat_map(|p| [p[0], p[1], p[2], 255]).collect()))
}

const KIND_CONTOUR: u8 = 0;
const KIND_RIVER: u8 = 2;
const KIND_COAST: u8 = 4;
const CLOSED: u16 = 1;
const DRY: u16 = 1 << 3;

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

/// A closed level line ready to fill: its inside as a plane polygon in unwrapped longitude, whether that
/// polygon is the inside or its complement, the inside's area and whether the inside is the high side.
struct Loop {
    poly: Vec<(f64, f64)>,
    complement: bool,
    area: f64,
    inside_high: bool,
    z: f32,
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
    Some(Loop { poly, complement, area, inside_high, z: l.z })
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

/// The planet's height bands (m, the middle of each 200 m band) and its water, painted as RGBA.
fn paint(lines: &[Line], w: usize, h: usize) -> Vec<u8> {
    const STEP: f32 = 200.0;
    let order = |kinds: &[u8]| {
        let mut v: Vec<Loop> = lines.iter().filter(|l| kinds.contains(&l.kind) && l.flags & CLOSED != 0).filter_map(prepare).collect();
        v.sort_by(|a, b| b.area.total_cmp(&a.area));
        v
    };
    let levels = order(&[KIND_CONTOUR, KIND_COAST]);
    let shores = order(&[KIND_COAST]);
    let band = innermost(&levels, w, h);
    let water = innermost(&shores, w, h);
    // Outside every inside: the outside of the largest.
    let height = |i: u32| -> f32 {
        if i == u32::MAX {
            let lp = &levels[0];
            return lp.z + if lp.inside_high { -STEP * 0.5 } else { STEP * 0.5 };
        }
        let lp = &levels[i as usize];
        lp.z + if lp.inside_high { STEP * 0.5 } else { -STEP * 0.5 }
    };
    let wet = |i: u32| -> Option<f32> {
        let (lp, inside) = if i == u32::MAX { (&shores[0], false) } else { (&shores[i as usize], true) };
        let low_here = lp.inside_high != inside;
        low_here.then_some(lp.z)
    };
    let mut rgba = vec![0u8; w * h * 4];
    rgba.par_chunks_mut(4).enumerate().for_each(|(p, px)| {
        let z = height(band[p]);
        let c = match wet(water[p]) {
            Some(level) => sea(level - z),
            None => land(z),
        };
        px.copy_from_slice(&[c[0], c[1], c[2], 255]);
    });
    // Rivers that carry water.
    let (sx, sy) = (w as f64 / 360.0, h as f64 / 180.0);
    for l in lines.iter().filter(|l| l.kind == KIND_RIVER && l.flags & DRY == 0) {
        for s in l.pts.windows(2) {
            let mut d = s[1].0 - s[0].0;
            d -= 360.0 * (d / 360.0).round();
            let (x0, y0) = ((s[0].0 + 180.0) * sx, (90.0 - s[0].1) * sy);
            let (x1, y1) = (x0 + d * sx, (90.0 - s[1].1) * sy);
            let n = ((x1 - x0).abs().max((y1 - y0).abs()).ceil() as usize).max(1);
            for k in 0..=n {
                let t = k as f64 / n as f64;
                let x = ((x0 + t * (x1 - x0)).floor() as i64).rem_euclid(w as i64) as usize;
                let y = ((y0 + t * (y1 - y0)).floor() as usize).min(h - 1);
                rgba[(y * w + x) * 4..(y * w + x) * 4 + 3].copy_from_slice(&[52, 104, 168]);
            }
        }
    }
    rgba
}

fn mix(stops: &[(f32, [f32; 3])], v: f32) -> [u8; 3] {
    let k = stops.iter().position(|s| v < s.0).unwrap_or(stops.len());
    let c = if k == 0 {
        stops[0].1
    } else if k == stops.len() {
        stops[k - 1].1
    } else {
        let (a, b) = (stops[k - 1], stops[k]);
        let t = (v - a.0) / (b.0 - a.0);
        [0, 1, 2].map(|j| a.1[j] + t * (b.1[j] - a.1[j]))
    };
    c.map(|x| x.round().clamp(0.0, 255.0) as u8)
}

/// Water by depth (m).
fn sea(depth: f32) -> [u8; 3] {
    mix(&[(0.0, [86.0, 150.0, 196.0]), (200.0, [52.0, 112.0, 170.0]), (2000.0, [28.0, 70.0, 132.0]), (5000.0, [14.0, 36.0, 84.0])], depth)
}

/// Land by height (m).
fn land(z: f32) -> [u8; 3] {
    mix(
        &[
            (-500.0, [120.0, 140.0, 96.0]),
            (0.0, [92.0, 140.0, 80.0]),
            (500.0, [134.0, 160.0, 92.0]),
            (1500.0, [196.0, 180.0, 120.0]),
            (3000.0, [150.0, 116.0, 84.0]),
            (5000.0, [120.0, 104.0, 98.0]),
            (6500.0, [236.0, 236.0, 240.0]),
        ],
        z,
    )
}
