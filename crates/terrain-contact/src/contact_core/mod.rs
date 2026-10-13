pub mod grid;
use crate::crease::{Tile, area, rect, world};
use glam::{DVec2, DVec3};
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use universe_world::worlds::pgs::{FaceGeometry, Surface};
pub struct Chart {
    pub c: DVec3,
    pub n: DVec3,
    pub e: DVec3,
    pub r: f64,
}
impl Chart {
    pub fn new(c: DVec3, r: f64) -> Self {
        let c = c.normalize();
        let n = (DVec3::Y - c * c.y).normalize();
        Self {
            c,
            n,
            e: n.cross(c),
            r,
        }
    }
    pub fn direction(&self, p: DVec2) -> DVec3 {
        (self.c + self.e * p.x / self.r + self.n * p.y / self.r).normalize()
    }
    pub fn project(&self, p: DVec3) -> Result<DVec2, String> {
        let den = p.dot(self.c);
        if den <= 0. {
            return Err("hemisphere".into());
        }
        Ok(DVec2::new(p.dot(self.e), p.dot(self.n)) * (self.r / den))
    }
}
fn intersect(mut p: Vec<DVec2>, triangle: &[DVec2]) -> Vec<DVec2> {
    let orientation = (triangle[1] - triangle[0])
        .perp_dot(triangle[2] - triangle[0])
        .signum();
    for i in 0..3 {
        let a = triangle[i];
        let edge = triangle[(i + 1) % 3] - a;
        let signed = |v: DVec2| edge.perp_dot(v - a) * orientation;
        let mut next = vec![];
        if let Some(&last) = p.last() {
            let mut u = last;
            let mut du = signed(u);
            for v in p {
                let dv = signed(v);
                if (du >= 0.) != (dv >= 0.) {
                    next.push(u + (v - u) * (du / (du - dv)));
                }
                if dv >= 0. {
                    next.push(v);
                }
                u = v;
                du = dv;
            }
        }
        p = next;
    }
    p
}
fn taper(p: DVec2) -> f64 {
    let smooth = |x: f64| {
        let t = ((x.abs() - 32.) / 32.).clamp(0., 1.);
        t * t * (3. - 2. * t)
    };
    1. - (1. - smooth(p.x)) * (1. - smooth(p.y))
}
struct Canonical {
    id: u32,
    data: FaceGeometry,
    poly: Vec<DVec2>,
    lo: DVec2,
    hi: DVec2,
}
#[derive(Serialize)]
pub struct Fixture {
    pub feature: String,
    pub tiles: Vec<Tile>,
    pub vertices: usize,
    pub triangles: usize,
    pub source_search: usize,
    pub retained_triangles: usize,
    pub retained_patch_search: usize,
    pub cpu_ms: f64,
    pub raw_area_m2: f64,
    pub outer_vertex_max_displacement_m: f64,
    pub retained_grid_max_overlap_disagreement_m: f64,
    pub output_gpu_bytes: usize,
}
pub fn build(s: &Surface, c: DVec3, r: f64, id: &str) -> Result<Fixture, String> {
    let clock = std::time::Instant::now();
    let chart = Chart::new(c, r);
    let seed = s.query(c)?.face;
    let mut queue = VecDeque::from([seed]);
    let mut seen = BTreeSet::new();
    let mut canonical = vec![];
    while let Some(f) = queue.pop_front() {
        if seen.contains(&f) {
            continue;
        }
        if seen.len() >= 4096 {
            return Err("source face cap".into());
        }
        seen.insert(f);
        let data = s.face_geometry(f).ok_or("face")?;
        let poly = data
            .positions
            .map(|p| chart.project(p))
            .into_iter()
            .collect::<Result<Vec<_>, _>>()?;
        if area(&rect(poly.clone(), [-64., 64., -64., 64.])) < 1e-12 {
            continue;
        }
        let lo = poly
            .iter()
            .copied()
            .fold(DVec2::splat(f64::INFINITY), DVec2::min);
        let hi = poly
            .iter()
            .copied()
            .fold(DVec2::splat(f64::NEG_INFINITY), DVec2::max);
        queue.extend(data.neighbours);
        canonical.push(Canonical {
            id: f,
            data,
            poly,
            lo,
            hi,
        });
    }
    canonical.sort_by_key(|f| f.id);
    let (retained, visits) = grid::retained(s, &chart)?;
    let mut t = Tile {
        origin: (chart.c * (r + s.query(c)?.height_m)).to_array(),
        vertices: vec![],
        uv: vec![],
        triangles: vec![],
        provenance: vec![],
    };
    let origin = DVec3::from_array(t.origin);
    let mut index = BTreeMap::new();
    let mut outer = 0f64;
    let mut duplicate = 0f64;
    let mut total_area = 0f64;
    // The complete canonical/grid overlay makes all outer split positions explicit.
    for old in &retained {
        for f in &canonical {
            if old.hi.x < f.lo.x || old.lo.x > f.hi.x || old.hi.y < f.lo.y || old.lo.y > f.hi.y {
                continue;
            }
            let overlap = intersect(old.uv.clone(), &f.poly);
            if area(&overlap) < 1e-12 {
                continue;
            }
            for (xmin, xmax) in [(-64., -32.), (-32., 0.), (0., 32.), (32., 64.)] {
                for (ymin, ymax) in [(-64., -32.), (-32., 32.), (32., 64.)] {
                    if xmin >= -32. && xmax <= 32. && ymin >= -32. && ymax <= 32. {
                        continue;
                    }
                    let cut = rect(overlap.clone(), [xmin, xmax, ymin, ymax]);
                    if area(&cut) < 1e-12 {
                        continue;
                    }
                    let mut ids = vec![];
                    for p in cut {
                        let key = ((p.x * 1e8).round() as i64, (p.y * 1e8).round() as i64);
                        let p = DVec2::new(key.0 as f64, key.1 as f64) / 1e8;
                        let d = chart.direction(p);
                        let canonical = world(&f.data, d, r)?;
                        let old_r = grid::radial(old, d);
                        let w = taper(p);
                        let pos = d * (canonical.length() * (1. - w) + old_r * w);
                        let local = (pos - origin).as_vec3();
                        if p.x.abs() == 64. || p.y.abs() == 64. {
                            outer = outer.max((origin + local.as_dvec3() - d * old_r).length());
                        }
                        let k = if let Some(&k) = index.get(&key) {
                            let existing = glam::Vec3::from_array(t.vertices[k as usize]);
                            duplicate = duplicate.max((existing - local).length() as f64);
                            k
                        } else {
                            if t.vertices.len() >= 32768 {
                                return Err(format!(
                                    "vertex cap at {} retained triangles; emitted {} triangles",
                                    retained.len(),
                                    t.triangles.len()
                                ));
                            }
                            let k = t.vertices.len() as u32;
                            index.insert(key, k);
                            t.vertices.push(local.to_array());
                            t.uv.push(p.to_array());
                            k
                        };
                        if ids.last() != Some(&k) {
                            ids.push(k)
                        }
                    }
                    if ids.len() > 1 && ids.first() == ids.last() {
                        ids.pop();
                    }
                    for j in 1..ids.len().saturating_sub(1) {
                        let tri = [ids[0], ids[j], ids[j + 1]];
                        let a = area(&tri.map(|k| DVec2::from_array(t.uv[k as usize])));
                        if a < 1e-12 {
                            continue;
                        }
                        if t.triangles.len() >= 32768 {
                            return Err(format!(
                                "triangle cap; retained={}, source={}, vertices={}, area={total_area}",
                                retained.len(),
                                canonical.len(),
                                t.vertices.len()
                            ));
                        }
                        t.triangles.push(tri);
                        t.provenance.push([f.id, f.data.body]);
                        total_area += a;
                    }
                }
            }
        }
    }
    // Core needs canonical edges plus the exact inner-band boundary splits, not
    // the distant renderer's finer triangulation throughout its interior.
    let boundary =
        t.uv.iter()
            .copied()
            .filter(|p| {
                (p[0].abs() == 32. && p[1].abs() <= 32.) || (p[1].abs() == 32. && p[0].abs() <= 32.)
            })
            .map(DVec2::from_array)
            .collect::<Vec<_>>();
    for f in &canonical {
        for ix in -4..4 {
            for iy in -4..4 {
                let cut = rect(
                    f.poly.clone(),
                    [
                        ix as f64 * 8.,
                        (ix + 1) as f64 * 8.,
                        iy as f64 * 8.,
                        (iy + 1) as f64 * 8.,
                    ],
                );
                if area(&cut) < 1e-12 {
                    continue;
                }
                let mut polygon = vec![];
                for j in 0..cut.len() {
                    let a = cut[j];
                    let b = cut[(j + 1) % cut.len()];
                    polygon.push(a);
                    if (a.x == b.x && a.x.abs() == 32.) || (a.y == b.y && a.y.abs() == 32.) {
                        let delta = b - a;
                        let mut split = boundary
                            .iter()
                            .filter_map(|p| {
                                let u = (*p - a).dot(delta) / delta.length_squared();
                                (u > 1e-8 && u < 1. - 1e-8 && delta.perp_dot(*p - a).abs() < 1e-7)
                                    .then_some((u, *p))
                            })
                            .collect::<Vec<_>>();
                        split.sort_by(|a, b| a.0.total_cmp(&b.0));
                        split.dedup_by(|a, b| a.1.distance(b.1) < 1e-8);
                        polygon.extend(split.into_iter().map(|p| p.1));
                    }
                }
                let mut ids = vec![];
                for p in polygon {
                    let key = ((p.x * 1e8).round() as i64, (p.y * 1e8).round() as i64);
                    let p = DVec2::new(key.0 as f64, key.1 as f64) / 1e8;
                    let k = if let Some(&k) = index.get(&key) {
                        k
                    } else {
                        if t.vertices.len() >= 32768 {
                            return Err("core vertex cap".into());
                        }
                        let k = t.vertices.len() as u32;
                        index.insert(key, k);
                        t.uv.push(p.to_array());
                        t.vertices.push(
                            (world(&f.data, chart.direction(p), r)? - origin)
                                .as_vec3()
                                .to_array(),
                        );
                        k
                    };
                    if ids.last() != Some(&k) {
                        ids.push(k)
                    }
                }
                if ids.len() > 1 && ids.first() == ids.last() {
                    ids.pop();
                }
                // Fan from an interior centroid, so collinear boundary split vertices
                // remain actual mesh edges rather than disappearing as degenerate fans.
                let centre = ids
                    .iter()
                    .map(|i| DVec2::from_array(t.uv[*i as usize]))
                    .sum::<DVec2>()
                    / (ids.len() as f64);
                if t.vertices.len() >= 32768 {
                    return Err("core centroid cap".into());
                }
                let k = t.vertices.len() as u32;
                t.uv.push(centre.to_array());
                t.vertices.push(
                    (world(&f.data, chart.direction(centre), r)? - origin)
                        .as_vec3()
                        .to_array(),
                );
                for j in 0..ids.len() {
                    let tri = [k, ids[j], ids[(j + 1) % ids.len()]];
                    let a = area(&tri.map(|i| DVec2::from_array(t.uv[i as usize])));
                    if a < 1e-12 {
                        continue;
                    }
                    if t.triangles.len() >= 32768 {
                        return Err("core triangle cap".into());
                    }
                    t.triangles.push(tri);
                    t.provenance.push([f.id, f.data.body]);
                    total_area += a;
                }
            }
        }
    }
    if (total_area - 16384.).abs() > 1e-3 {
        return Err(format!("overlay area {total_area}"));
    }
    let count = t.triangles.len();
    Ok(Fixture {
        feature: id.into(),
        vertices: t.vertices.len(),
        triangles: count,
        source_search: seen.len(),
        retained_triangles: retained.len(),
        retained_patch_search: visits,
        cpu_ms: clock.elapsed().as_secs_f64() * 1000.,
        raw_area_m2: total_area,
        outer_vertex_max_displacement_m: outer,
        retained_grid_max_overlap_disagreement_m: duplicate,
        output_gpu_bytes: count * 3 * 56 + 40,
        tiles: vec![t],
    })
}
