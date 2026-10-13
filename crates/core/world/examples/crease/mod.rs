//! Bounded compiled visual-worker prototype; not enabled in the game.
use glam::{DVec2, DVec3};
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::sync::atomic::{AtomicU64, Ordering};
use universe_world::worlds::pgs::{FaceGeometry, Surface};
const HALF: f64 = 64.;
const STEP: f64 = 8.;
#[derive(Serialize)]
pub struct Tile {
    pub origin: [f64; 3],
    pub vertices: Vec<[f32; 3]>,
    pub uv: Vec<[f64; 2]>,
    pub triangles: Vec<[u32; 3]>,
    pub provenance: Vec<[u32; 2]>,
}
#[derive(Serialize)]
pub struct Pair {
    pub feature: String,
    pub status: &'static str,
    pub seed_owner: u32,
    pub search_faces: usize,
    pub source_faces: usize,
    pub tiles: Vec<Tile>,
    pub vertices: usize,
    pub triangles: usize,
    pub seam_max_m: f64,
    pub seam_points: usize,
    pub cpu_ms: f64,
    pub gpu_expanded_bytes: usize,
    pub retained_cpu_capacity_bytes: usize,
}
pub(super) fn area(p: &[DVec2]) -> f64 {
    if p.len() < 3 {
        return 0.;
    }
    (0..p.len())
        .map(|i| p[i].perp_dot(p[(i + 1) % p.len()]))
        .sum::<f64>()
        .abs()
        * 0.5
}
pub(super) fn rect(mut p: Vec<DVec2>, bounds: [f64; 4]) -> Vec<DVec2> {
    for (axis, bound, sign) in [
        (0, bounds[0], 1.),
        (0, bounds[1], -1.),
        (1, bounds[2], 1.),
        (1, bounds[3], -1.),
    ] {
        let mut out = Vec::with_capacity(8);
        if let Some(&last) = p.last() {
            let mut a = last;
            let mut da = (a[axis] - bound) * sign;
            for b in p {
                let db = (b[axis] - bound) * sign;
                if (da >= 0.) != (db >= 0.) {
                    let mut v = a + (b - a) * (da / (da - db));
                    v[axis] = bound;
                    out.push(v);
                }
                if db >= 0. {
                    out.push(b);
                }
                a = b;
                da = db;
            }
        }
        p = out;
    }
    p
}
pub(super) fn world(f: &FaceGeometry, d: DVec3, radius: f64) -> Result<DVec3, String> {
    let [a, b, c] = f.positions;
    let n = (b - a).cross(c - a);
    let p = d * (n.dot(a) / n.dot(d));
    let v = (p - a).cross(c - a).dot(n) / n.length_squared();
    let w = (b - a).cross(p - a).dot(n) / n.length_squared();
    let [ha, hb, hc] = f.bed_heights_m;
    let h = f
        .water_level_m
        .unwrap_or((1. - v - w) * ha + v * hb + w * hc);
    let p = d * (radius + h);
    if !p.is_finite() {
        return Err("nonfinite embedding".into());
    }
    Ok(p)
}
pub fn build(
    s: &Surface,
    centre: DVec3,
    r: f64,
    id: &str,
    generation: &AtomicU64,
    expected: u64,
) -> Result<Pair, String> {
    let start = std::time::Instant::now();
    let valid = || {
        if generation.load(Ordering::Acquire) == expected {
            Ok(())
        } else {
            Err("stale generation".to_string())
        }
    };
    valid()?;
    if !centre.is_finite() || centre.length_squared() == 0. || !r.is_finite() || r <= 0. {
        return Err("invalid chart".into());
    }
    let c = centre.normalize();
    let n = (DVec3::Y - c * c.y).normalize();
    let e = n.cross(c);
    if !n.is_finite() {
        return Err("polar chart refused".into());
    }
    let direction = |uv: DVec2| (c + e * uv.x / r + n * uv.y / r).normalize();
    let query = s.query(c)?;
    let seed = query.face;

    let mut queue = VecDeque::from([seed]);
    let mut seen = BTreeSet::new();
    let mut hit = BTreeMap::new();
    let mut coverage = 0.;
    while let Some(f) = queue.pop_front() {
        valid()?;
        if seen.contains(&f) {
            continue;
        }
        if seen.len() >= 4096 {
            return Err("search face cap".into());
        }
        seen.insert(f);
        let data = s.face_geometry(f).ok_or("face range")?;
        let mut poly = Vec::with_capacity(3);
        for p in data.positions {
            let den = p.dot(c);
            if den <= 0. {
                return Err("face outside projection hemisphere".into());
            }
            poly.push(DVec2::new(p.dot(e), p.dot(n)) * (r / den));
        }
        let a = area(&rect(poly.clone(), [-HALF, HALF, -HALF, HALF]));
        if a < 1e-12 {
            continue;
        }
        coverage += a;
        queue.extend(data.neighbours.into_iter().filter(|f| !seen.contains(f)));
        hit.insert(f, (data, poly));
    }
    if (coverage - 4. * HALF * HALF).abs() > 1e-4 {
        return Err("coverage area mismatch".into());
    }
    let mut tiles = vec![];
    let mut total_vertices = 0;
    let mut total_triangles = 0;
    let h = query.height_m;
    for (xmin, xmax) in [(-HALF, 0.), (0., HALF)] {
        let origin = direction(DVec2::new((xmin + xmax) * 0.5, 0.)) * (r + h);
        let mut t = Tile {
            origin: origin.to_array(),
            vertices: vec![],
            uv: vec![],
            triangles: vec![],
            provenance: vec![],
        };
        // Independent per-tile cache; no forced seam equality.
        let mut indices = BTreeMap::<(i64, i64), u32>::new();
        for (&face, (data, poly)) in &hit {
            valid()?;
            let lo = poly
                .iter()
                .fold(DVec2::splat(f64::INFINITY), |a, b| a.min(*b));
            let hi = poly
                .iter()
                .fold(DVec2::splat(f64::NEG_INFINITY), |a, b| a.max(*b));
            for ix in ((xmin / STEP) as i32).max((lo.x / STEP).floor() as i32)
                ..((xmax / STEP) as i32).min((hi.x / STEP).ceil() as i32)
            {
                for iy in (-8).max((lo.y / STEP).floor() as i32)..8.min((hi.y / STEP).ceil() as i32)
                {
                    let cut = rect(
                        poly.clone(),
                        [
                            ix as f64 * STEP,
                            (ix + 1) as f64 * STEP,
                            iy as f64 * STEP,
                            (iy + 1) as f64 * STEP,
                        ],
                    );
                    if area(&cut) < 1e-12 {
                        continue;
                    }
                    let mut ids = vec![];
                    for v in cut {
                        let key = ((v.x * 1e8).round() as i64, (v.y * 1e8).round() as i64);
                        let idx = if let Some(&idx) = indices.get(&key) {
                            idx
                        } else {
                            if total_vertices >= 32768 {
                                return Err("vertex cap".into());
                            }
                            let uv = DVec2::new(key.0 as f64, key.1 as f64) / 1e8;
                            let idx = t.vertices.len() as u32;
                            t.vertices.push(
                                (world(data, direction(uv), r)? - origin)
                                    .as_vec3()
                                    .to_array(),
                            );
                            t.uv.push(uv.to_array());
                            indices.insert(key, idx);
                            total_vertices += 1;
                            idx
                        };
                        if ids.last() != Some(&idx) {
                            ids.push(idx);
                        }
                    }
                    if ids.len() > 1 && ids.first() == ids.last() {
                        ids.pop();
                    }
                    for j in 1..ids.len().saturating_sub(1) {
                        let tri = [ids[0], ids[j], ids[j + 1]];
                        if area(&tri.map(|i| DVec2::from_array(t.uv[i as usize]))) < 1e-12 {
                            continue;
                        }
                        if total_triangles >= 32768 {
                            return Err("triangle cap".into());
                        }
                        t.triangles.push(tri);
                        t.provenance.push([face, data.body]);
                        total_triangles += 1;
                    }
                }
            }
        }
        let a = t
            .triangles
            .iter()
            .map(|tri| area(&tri.map(|i| DVec2::from_array(t.uv[i as usize]))))
            .sum::<f64>();
        if (a - (xmax - xmin) * 2. * HALF).abs() > 1e-4 {
            return Err("emitted area mismatch".into());
        }
        tiles.push(t);
    }
    let seams = tiles
        .iter()
        .map(|t| {
            t.uv.iter()
                .zip(&t.vertices)
                .filter(|(uv, _)| uv[0] == 0.)
                .map(|(uv, p)| {
                    (
                        (uv[1] * 1e8).round() as i64,
                        DVec3::from_array(t.origin) + glam::Vec3::from_array(*p).as_dvec3(),
                    )
                })
                .collect::<BTreeMap<_, _>>()
        })
        .collect::<Vec<_>>();
    if !seams[0].keys().eq(seams[1].keys()) {
        return Err("seam split mismatch".into());
    }
    let seam = seams[0]
        .iter()
        .map(|(k, p)| p.distance(seams[1][k]))
        .fold(0., f64::max);
    if seam > 1e-5 {
        return Err(format!("{id}: seam error {seam:e}"));
    }
    valid()?;
    let retained_cpu_capacity_bytes = std::mem::size_of::<Pair>()
        + tiles.capacity() * std::mem::size_of::<Tile>()
        + tiles
            .iter()
            .map(|t| {
                t.vertices.capacity() * 12
                    + t.uv.capacity() * 16
                    + t.triangles.capacity() * 12
                    + t.provenance.capacity() * 8
            })
            .sum::<usize>()
        + id.len();
    Ok(Pair {
        feature: id.into(),
        status: "built",
        seed_owner: seed,
        search_faces: seen.len(),
        source_faces: hit.len(),
        tiles,
        vertices: total_vertices,
        triangles: total_triangles,
        seam_max_m: seam,
        seam_points: seams[0].len(),
        cpu_ms: start.elapsed().as_secs_f64() * 1000.,
        gpu_expanded_bytes: total_triangles * 3 * 56 + 80,
        retained_cpu_capacity_bytes,
    })
}
