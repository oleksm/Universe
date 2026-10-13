//! CPU transaction on a supplied selected group; no independent post-correction morph.
use crate::contact_core::grid::Triangle;
use glam::DVec3;
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::sync::atomic::{AtomicU64, Ordering};
type Key = ([u32; 4], [u32; 3]);
#[derive(Clone)]
struct Edge {
    patch: [u32; 4],
    a: [u32; 3],
    b: [u32; 3],
    axis: usize,
}
fn rank(p: [u32; 4]) -> (u32, [u32; 4]) {
    (p[1], p)
}
fn line(p: [u32; 4], a: [u32; 3], b: [u32; 3]) -> Option<([u32; 3], usize)> {
    let step = 1u32 << (19 - p[1]);
    let lo = [p[2] * 16 * step, p[3] * 16 * step];
    let hi = [lo[0] + 16 * step, lo[1] + 16 * step];
    for axis in 1..=2 {
        if a[axis] == b[axis] && (a[axis] == lo[axis - 1] || a[axis] == hi[axis - 1]) {
            return Some(([a[0], axis as u32, a[axis]], 3 - axis));
        }
    }
    None
}
pub fn correct(
    grid: &mut [Triangle],
    generation: &AtomicU64,
    expected: u64,
    chart: &crate::contact_core::Chart,
) -> Result<Value, Value> {
    let mut edges: BTreeMap<[u32; 3], Vec<Edge>> = BTreeMap::new();
    let mut points: BTreeMap<Key, DVec3> = BTreeMap::new();
    let mut origins = BTreeMap::new();
    for t in grid.iter() {
        origins.insert(t.patch, t.origin);
        for i in 0..3 {
            let k = (t.patch, t.tags[i]);
            if let Some(p) = points.insert(k, t.world[i]) {
                if p != t.world[i] {
                    return Err(json!({"reason":"same patch vertex inconsistent","key":k}));
                }
            }
        }
    }
    for t in grid.iter() {
        for (i, j) in [(0, 1), (1, 2), (2, 0)] {
            if let Some((key, axis)) = line(t.patch, t.tags[i], t.tags[j]) {
                let (a, b) = if t.tags[i][axis] < t.tags[j][axis] {
                    (t.tags[i], t.tags[j])
                } else {
                    (t.tags[j], t.tags[i])
                };
                edges.entry(key).or_default().push(Edge {
                    patch: t.patch,
                    a,
                    b,
                    axis,
                });
            }
        }
    }
    for es in edges.values_mut() {
        es.sort_by_key(|e| (rank(e.patch), e.a, e.b));
        es.dedup_by_key(|e| (e.patch, e.a, e.b));
    }
    let mut keys = points.keys().copied().collect::<Vec<_>>();
    keys.sort_by_key(|k| (rank(k.0), k.1));
    let mut changed = 0;
    let mut maximum = 0f64;
    let mut dependencies = 0;
    let mut ownerless = 0;
    let mut constraints = Vec::new();
    for k in keys {
        if generation.load(Ordering::Acquire) != expected {
            return Err(json!({"reason":"stale group"}));
        }
        let (patch, tag) = k;
        let mut candidates = vec![];
        for axis in 1..=2 {
            if let Some(es) = edges.get(&[tag[0], axis as u32, tag[axis]]) {
                for e in es {
                    if e.a[e.axis] <= tag[e.axis] && tag[e.axis] <= e.b[e.axis] {
                        candidates.push(e);
                    }
                }
            }
        }
        if candidates.is_empty() {
            continue;
        }
        candidates.sort_by_key(|e| (rank(e.patch), e.a, e.b));
        let e = candidates[0];
        if !candidates.iter().any(|e| e.patch != patch) {
            ownerless += 1;
            continue;
        }
        if rank(e.patch) > rank(patch) {
            return Err(json!({"reason":"owner dependency cycle"}));
        }
        let f = (tag[e.axis] - e.a[e.axis]) as f64 / (e.b[e.axis] - e.a[e.axis]) as f64;
        let a = points[&(e.patch, e.a)];
        let b = points[&(e.patch, e.b)];
        let wanted = a + (b - a) * f;
        let origin = origins[&patch];
        let p = origin + (wanted - origin).as_vec3().as_dvec3();
        let error = p.distance(wanted);
        if error > 1e-5 {
            return Err(json!({"reason":"owner f32 gate","error_m":error,"patch":patch,"tag":tag}));
        }
        let delta = p.distance(points[&k]);
        maximum = maximum.max(delta);
        changed += usize::from(delta > 0.);
        dependencies += usize::from(e.patch != patch);
        points.insert(k, p);
        constraints.push((k, e.patch, e.a, e.b, f));
    }
    // Validate against final owners, including corners whose owner endpoints may change.
    let mut seam = 0f64;
    for (k, p, a, b, f) in constraints {
        let expected = points[&(p, a)].lerp(points[&(p, b)], f);
        let error = points[&k].distance(expected);
        seam = seam.max(error);
        if error > 1e-5 {
            return Err(
                json!({"reason":"final corner dependency disagreement","error_m":error,"key":k}),
            );
        }
    }
    let mut interval_bound = 0f64;
    let mut overlaps = 0;
    let mut unsupported = 0;
    let mut unsupported_inside = 0;
    let mut unsupported_examples = vec![];
    for es in edges.values() {
        for e in es {
            let mut cover = vec![];
            for other in es {
                if other.patch == e.patch {
                    continue;
                }
                let low = e.a[e.axis].max(other.a[e.axis]);
                let high = e.b[e.axis].min(other.b[e.axis]);
                if high <= low {
                    continue;
                }
                cover.push((low, high));
                overlaps += 1;
                for x in [low, high] {
                    let t = (x - e.a[e.axis]) as f64 / (e.b[e.axis] - e.a[e.axis]) as f64;
                    let u =
                        (x - other.a[e.axis]) as f64 / (other.b[e.axis] - other.a[e.axis]) as f64;
                    let a = points[&(e.patch, e.a)].lerp(points[&(e.patch, e.b)], t);
                    let b =
                        points[&(other.patch, other.a)].lerp(points[&(other.patch, other.b)], u);
                    let d = a.distance(b);
                    interval_bound = interval_bound.max(d);
                    if d > 1e-5 {
                        return Err(
                            json!({"reason":"finite overlap separation","error_m":d,"patches":[e.patch,other.patch],"interval":[low,high]}),
                        );
                    }
                }
            }
            cover.sort();
            let mut end = e.a[e.axis];
            for &(a, b) in &cover {
                if a > end {
                    break;
                }
                end = end.max(b)
            }
            if end < e.b[e.axis] {
                unsupported += 1;
                let a = points[&(e.patch, e.a)];
                let b = points[&(e.patch, e.b)];
                let mut lo = 0f64;
                let mut hi = 1f64;
                for axis in [chart.e, chart.n] {
                    for sign in [-1., 1.] {
                        let side = axis * (sign * chart.r) - chart.c * 64.;
                        let da = side.dot(a);
                        let db = side.dot(b);
                        if da > 0. && db > 0. {
                            hi = -1.;
                        } else if da > 0. {
                            lo = lo.max(da / (da - db));
                        } else if db > 0. {
                            hi = hi.min(da / (da - db));
                        }
                    }
                }
                if hi > lo {
                    let low = e.a[e.axis] as f64 + (e.b[e.axis] - e.a[e.axis]) as f64 * lo;
                    let high = e.a[e.axis] as f64 + (e.b[e.axis] - e.a[e.axis]) as f64 * hi;
                    let mut covered = low;
                    for &(a, b) in &cover {
                        if b as f64 <= covered {
                            continue;
                        }
                        if a as f64 > covered {
                            break;
                        }
                        covered = covered.max(b as f64)
                    }
                    if covered < high {
                        unsupported_inside += 1;
                        unsupported_examples.push(json!({"patch":e.patch,"edge":[e.a,e.b],"required_interval":[low,high],"coverage":cover,"world":[a.to_array(),b.to_array()]}));
                    }
                }

                if unsupported_examples.len() < 3 {
                    unsupported_examples
                        .push(json!({"patch":e.patch,"edge":[e.a,e.b],"covered_to":end}));
                }
            }
        }
    }
    if unsupported_inside != 0 {
        return Err(json!({"reason":"missing finite owner inside footprint","unsupported_intersecting_footprint":unsupported_inside,"witnesses":unsupported_examples}));
    }
    if generation.load(Ordering::Acquire) != expected {
        return Err(json!({"reason":"stale group before publication"}));
    }
    // Atomic at caller boundary: mutation happens only after validation.
    for t in grid.iter_mut() {
        for i in 0..3 {
            t.world[i] = points[&(t.patch, t.tags[i])];
        }
    }
    Ok(
        json!({"finite_overlap_bound_m":interval_bound,"directed_overlap_intervals":overlaps,"unsupported_full_intervals":unsupported,"unsupported_intersecting_footprint":unsupported_inside,"unsupported_examples":unsupported_examples,"changed_vertices":changed,"max_correction_m":maximum,"max_owner_parametric_separation_m":seam,"owner_dependencies":dependencies,"single_patch_boundary_vertices":ownerless,"policy":"finite lowest-level/lexicographic owner; corrected endpoints frozen; transaction requires supplied selected group","scope":"CPU group, single-patch exterior intervals not certified resident-neighbour readiness"}),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn missing_incident_owner_refuses_without_partial_mutation() {
        let chart = crate::contact_core::Chart::new(DVec3::X, 6281370.);
        let origin = DVec3::X * chart.r;
        let world = [origin, origin + DVec3::Y, origin + DVec3::Z];
        let uv = world.iter().map(|p| chart.project(*p).unwrap()).collect();
        let mut grid = vec![Triangle { tags:[[0,0,0],[0,0,1],[0,1,0]], patch:[0,19,0,0], origin,trace:None,world,uv,lo:glam::DVec2::splat(-1.),hi:glam::DVec2::ONE }];
        let err = correct(&mut grid, &AtomicU64::new(1), 1, &chart).unwrap_err();
        assert_eq!(err["reason"], "missing finite owner inside footprint");
        assert_eq!(grid[0].world, world);
    }
}
