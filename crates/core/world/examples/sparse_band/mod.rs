use crate::{
    contact_core::{
        Chart,
        grid::{self, Triangle},
    },
    crease,
};
use glam::{DVec2, DVec3};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use universe_world::worlds::pgs::Surface;
#[derive(Clone)]
pub struct Event {
    pub uv: DVec2,
    pub world: DVec3,
    pub ids: Vec<String>,
}
fn edge_key(mut a: [u32; 3], mut b: [u32; 3]) -> String {
    if b < a {
        std::mem::swap(&mut a, &mut b)
    }
    if a[1] == b[1] {
        format!("cube:{}:u:{}", a[0], a[1])
    } else if a[2] == b[2] {
        format!("cube:{}:v:{}", a[0], a[2])
    } else {
        format!("diagonal:{a:?}:{b:?}")
    }
}
/// Exact provenance identifies a shared cube-grid line; proximity never does.
pub fn chain(chart: &Chart, grid: &[Triangle], bound: f64) -> Result<(Vec<Event>, Value), Value> {
    let mut events: BTreeMap<String, Event> = BTreeMap::new();
    let mut contributors = BTreeMap::<String, Value>::new();
    let mut max = 0f64;
    let mut aliases = 0;
    for tri in grid {
        for (i, j) in [(0, 1), (1, 2), (2, 0)] {
            let (a, b) = if tri.tags[i] < tri.tags[j] {
                (i, j)
            } else {
                (j, i)
            };
            for axis in [0, 1] {
                for side in [-bound, bound] {
                    let signed = |p: DVec3| {
                        chart.r * p.dot(if axis == 0 { chart.e } else { chart.n })
                            - side * p.dot(chart.c)
                    };
                    let da = signed(tri.world[a]);
                    let db = signed(tri.world[b]);
                    if (da > 0. && db > 0.) || (da < 0. && db < 0.) || da == db {
                        continue;
                    }
                    let t = da / (da - db);
                    if !(0. ..=1.).contains(&t) {
                        continue;
                    }
                    let pos = tri.world[a] + (tri.world[b] - tri.world[a]) * t;
                    let mut uv = chart.project(pos).map_err(|e| json!({"reason":e}))?;
                    uv[axis] = side;
                    if uv[1 - axis].abs() > bound {
                        continue;
                    }
                    let id = format!(
                        "{}:boundary:{axis}:{side}",
                        edge_key(tri.tags[a], tri.tags[b])
                    );
                    let mut contributor = json!({"patch":tri.patch,"origin":tri.origin.to_array(),"edge_tags":[tri.tags[a],tri.tags[b]],"fraction":t,"world":[tri.world[a].to_array(),tri.world[b].to_array()],"triangle_world":tri.world.map(|p|p.to_array()),"intersection":pos.to_array()});
                    if let Some(trace) = &tri.trace {
                        contributor["eye"] = json!(trace.eye.to_array());
                        contributor["size"] = json!(trace.size);
                        contributor["raw"] =
                            json!([trace.raw[a].to_array(), trace.raw[b].to_array()]);
                        contributor["target"] =
                            json!([trace.target[a].to_array(), trace.target[b].to_array()]);
                        contributor["morph"] = json!([trace.morph[a], trace.morph[b]]);
                    }

                    if let Some(old) = events.get_mut(&id) {
                        let error = old.world.distance(pos);
                        max = max.max(error);
                        aliases += 1;
                        if error > 1e-5 {
                            return Err(
                                json!({"reason":"same lattice boundary disagrees with retained geometry","identity":id,"first_uv":old.uv.to_array(),"next_uv":uv.to_array(),"distance_m":error,"edge_tags":[tri.tags[a],tri.tags[b]],"first":contributors[&id],"second":contributor}),
                            );
                        }
                    } else {
                        contributors.insert(id.clone(), contributor);
                        events.insert(
                            id.clone(),
                            Event {
                                uv,
                                world: pos,
                                ids: vec![id],
                            },
                        );
                    }
                }
            }
        }
    }
    Ok((
        events.into_values().collect(),
        json!({"proven_identity_reuses":aliases,"max_retained_position_disagreement_m":max}),
    ))
}
fn smooth(p: DVec2) -> f64 {
    let s = |x: f64| {
        let t = ((x.abs() - 32.) / 32.).clamp(0., 1.);
        t * t * (3. - 2. * t)
    };
    1. - (1. - s(p.x)) * (1. - s(p.y))
}
fn retained(grid: &[Triangle], p: DVec2, d: DVec3) -> Result<f64, Value> {
    for t in grid {
        if p.x < t.lo.x - 1e-5 || p.x > t.hi.x + 1e-5 || p.y < t.lo.y - 1e-5 || p.y > t.hi.y + 1e-5
        {
            continue;
        }
        let [a, b, c] = [t.uv[0], t.uv[1], t.uv[2]];
        let det = (b - a).perp_dot(c - a);
        let v = (p - a).perp_dot(c - a) / det;
        let w = (b - a).perp_dot(p - a) / det;
        if v >= -1e-6 && w >= -1e-6 && v + w <= 1. + 1e-6 {
            return Ok(grid::radial(t, d));
        }
    }
    Err(json!({"reason":"retained triangle coverage miss","uv":p.to_array()}))
}
fn on_outer(p: DVec2) -> bool {
    p.x.abs() == 64. || p.y.abs() == 64.
}
fn bits(p: [f32; 3]) -> [u32; 3] {
    p.map(f32::to_bits)
}
pub fn build(s: &Surface, c: DVec3, id: &str) -> Result<Value, Value> {
    let r = 6281370.;
    let chart = Chart::new(c, r);
    let base = crease::build(s, c, r, id, &std::sync::atomic::AtomicU64::new(1), 1)
        .map_err(|e| json!({"reason":e}))?;
    let (grid, visits) = grid::retained(s, &chart).map_err(|e| json!({"reason":e}))?;
    build_prepared(s, c, id, base, grid, visits)
}
pub fn build_prepared(
    s: &Surface,
    c: DVec3,
    id: &str,
    base: crease::Pair,
    grid: Vec<Triangle>,
    visits: usize,
) -> Result<Value, Value> {
    let clock = std::time::Instant::now();
    let r = 6281370.;
    let chart = Chart::new(c, r);
    let origin = chart.c * (r + s.query(c).map_err(|e| json!({"reason":e}))?.height_m);
    let (mut events, chain_report) = chain(&chart, &grid, 64.)?;
    // Canonical source/8m boundary vertices are independently required constraints.
    let mut vertices = vec![];
    let mut uv = vec![];
    let mut triangles = vec![];
    let mut provenance = vec![];
    let mut by_uv = BTreeMap::<(u64, u64), usize>::new();
    for tile in &base.tiles {
        let mut remap = vec![];
        for (&p, &xy) in tile.vertices.iter().zip(&tile.uv) {
            let q = DVec2::from_array(xy);
            let key = (xy[0].to_bits(), xy[1].to_bits());
            let idx = if let Some(&i) = by_uv.get(&key) {
                i
            } else {
                let source = DVec3::from_array(tile.origin) + glam::Vec3::from_array(p).as_dvec3();
                let d = chart.direction(q);
                let t = smooth(q);
                let pos = if t == 0. {
                    source
                } else {
                    d * (source.length() * (1. - t) + retained(&grid, q, d)? * t)
                };
                let i = vertices.len();
                vertices.push((pos - origin).as_vec3().to_array());
                uv.push(xy);
                by_uv.insert(key, i);
                if on_outer(q) {
                    events.push(Event {
                        uv: q,
                        world: pos,
                        ids: vec![format!("canonical_cell_boundary:{xy:?}")],
                    });
                }
                i
            };
            remap.push(idx as u32);
        }
        for (ids, owner) in tile.triangles.iter().zip(&tile.provenance) {
            triangles.push(ids.map(|i| remap[i as usize]));
            provenance.push(*owner);
        }
    }
    // Build one ordered chain per outer side; same-identity events were already
    // shared above. Never turn mere f32 coincidence into a topology merge.
    let mut ordered = Vec::new();
    for axis in [0, 1] {
        for side in [-64., 64.] {
            let mut line = events
                .iter()
                .filter(|e| e.uv[axis] == side)
                .cloned()
                .collect::<Vec<_>>();
            line.sort_by(|a, b| a.uv[1 - axis].total_cmp(&b.uv[1 - axis]));
            for e in line {
                if let Some(last) = ordered.last_mut() {
                    let last: &mut Event = last;
                    if last.uv == e.uv {
                        last.ids.extend(e.ids);
                        continue;
                    }
                }
                ordered.push(e);
            }
        }
    }
    let mut physical = BTreeMap::new();
    for e in &ordered {
        let local = (e.world - origin).as_vec3().to_array();
        if let Some(previous) = physical.insert(bits(local), e) {
            let previous: &Event = previous;
            if previous.uv != e.uv {
                return Err(
                    json!({"reason":"distinct required outer constraints collapse in f32","first":{"uv":previous.uv.to_array(),"identities":previous.ids},"second":{"uv":e.uv.to_array(),"identities":e.ids},"local_f32":local,"source_feature":id,"chain":chain_report}),
                );
            }
        }
    }
    for e in &ordered {
        let key = (e.uv.x.to_bits(), e.uv.y.to_bits());
        if !by_uv.contains_key(&key) {
            let i = vertices.len();
            by_uv.insert(key, i);
            uv.push(e.uv.to_array());
            vertices.push((e.world - origin).as_vec3().to_array());
        }
    }
    let mut output = vec![];
    let mut owners = vec![];
    for (tri, owner) in triangles.iter().zip(&provenance) {
        let points = tri.map(|i| DVec2::from_array(uv[i as usize]));
        let mut polygon = vec![];
        let mut split = false;
        for j in 0..3 {
            let a = points[j];
            let b = points[(j + 1) % 3];
            polygon.push(tri[j]);
            if (a.x == b.x && a.x.abs() == 64.) || (a.y == b.y && a.y.abs() == 64.) {
                let delta = b - a;
                let mut inside = ordered
                    .iter()
                    .filter_map(|e| {
                        let p = e.uv;
                        let same = (a.x == b.x && p.x == a.x) || (a.y == b.y && p.y == a.y);
                        let t = (p - a).dot(delta) / delta.length_squared();
                        (same && t > 0. && t < 1.).then_some((t, p))
                    })
                    .collect::<Vec<_>>();
                inside.sort_by(|a, b| a.0.total_cmp(&b.0));
                inside.dedup_by(|a, b| a.1 == b.1);
                for (_, p) in inside {
                    split = true;
                    polygon.push(by_uv[&(p.x.to_bits(), p.y.to_bits())] as u32);
                }
            }
        }
        if split {
            let centre = points.iter().sum::<DVec2>() / 3.;
            let d = chart.direction(centre);
            let source = grid::visible(s, d).map_err(|e| json!({"reason":e}))?;
            let w = smooth(centre);
            let radial = (r + source) * (1. - w) + retained(&grid, centre, d)? * w;
            let i = vertices.len() as u32;
            vertices.push((d * radial - origin).as_vec3().to_array());
            uv.push(centre.to_array());
            for j in 0..polygon.len() {
                output.push([i, polygon[j], polygon[(j + 1) % polygon.len()]]);
                owners.push(*owner);
            }
        } else {
            output.push(*tri);
            owners.push(*owner);
        }
    }
    if vertices.len() > 32768 || output.len() > 32768 {
        return Err(
            json!({"reason":"output geometry cap","vertices":vertices.len(),"triangles":output.len()}),
        );
    }
    for (i, t) in output.iter().enumerate() {
        let p = t.map(|j| glam::Vec3::from_array(vertices[j as usize]).as_dvec3());
        if (p[1] - p[0]).cross(p[2] - p[0]).length_squared() == 0. {
            return Err(
                json!({"reason":"physical triangle collapsed","triangle":i,"ids":t,"uv":t.map(|j|uv[j as usize]),"positions":t.map(|j|vertices[j as usize]),"owner":owners[i]}),
            );
        }
    }
    let fixture = json!({"feature":id,"tiles":[{"origin":origin.to_array(),"vertices":vertices,"uv":uv,"triangles":output,"provenance":owners}],"chain":chain_report,"outer_constraints":ordered.len(),"vertices":vertices.len(),"triangles":output.len(),"retained_triangles":grid.len(),"retained_patch_search":visits,"cpu_ms":clock.elapsed().as_secs_f64()*1000.,"estimated_gpu_bytes":output.len()*168+40});
    if serde_json::to_vec(&fixture).unwrap().len() > 2097152 {
        return Err(json!({"reason":"serialized output cap"}));
    }
    Ok(fixture)
}

/// Regression diagnostics retain the failed physical triangles, not a repaired copy.
pub fn regression_trace(s: &Surface, c: DVec3, witnesses: &Value) -> Result<Value, String> {
    let chart = Chart::new(c, 6281370.);
    let (grid, _) = grid::retained(s, &chart)?;
    let mut regressions = vec![];
    for w in witnesses.as_array().ok_or("collapse witnesses")? {
        let xy = w["uv"].as_array().ok_or("uv")?;
        let positions = w["local_f32"].as_array().ok_or("local f32")?;
        let v = positions
            .iter()
            .map(|p| [0, 1, 2].map(|i| p[i].as_f64().unwrap() as f32))
            .collect::<Vec<_>>();
        let (i, j) = [(0, 1), (0, 2), (1, 2)]
            .into_iter()
            .find(|&(i, j)| bits(v[i]) == bits(v[j]))
            .ok_or("saved collapse not reproduced")?;
        let queries =
            [i, j].map(|i| DVec2::new(xy[i][0].as_f64().unwrap(), xy[i][1].as_f64().unwrap()));
        if queries[0] == queries[1] {
            return Err("expected distinct saved chart constraints".into());
        }
        let face = w["source_provenance"][0].as_u64().unwrap() as u32;
        let f = s.face_geometry(face).ok_or("face")?;
        let source = f
            .positions
            .map(|p| chart.project(p))
            .into_iter()
            .collect::<Result<Vec<_>, _>>()?;
        let mut endpoints = vec![];
        for q in queries {
            let axis = if q.x.abs() == 32. {
                0
            } else if q.y.abs() == 32. {
                1
            } else {
                return Err("saved clipping boundary".into());
            };
            let bound = q[axis];
            let mut candidates = vec![];
            for old in &grid {
                for (a, b) in [(0, 1), (1, 2), (2, 0)] {
                    let da = old.uv[a][axis] - bound;
                    let db = old.uv[b][axis] - bound;
                    if da == db || (da > 0. && db > 0.) || (da < 0. && db < 0.) {
                        continue;
                    }
                    let t = da / (da - db);
                    let mut p = old.uv[a] + (old.uv[b] - old.uv[a]) * t;
                    p[axis] = bound;
                    let distance = p.distance(q);
                    if distance <= 1e-5 {
                        candidates.push((distance, edge_key(old.tags[a], old.tags[b]), p));
                    }
                }
            }
            candidates.sort_by(|a, b| a.0.total_cmp(&b.0));
            let mut ids = candidates.iter().map(|x| x.1.clone()).collect::<Vec<_>>();
            ids.sort();
            ids.dedup();
            let source_edge_distance = (0..3)
                .map(|i| {
                    let a = source[i];
                    let delta = source[(i + 1) % 3] - a;
                    delta.perp_dot(q - a).abs() / delta.length()
                })
                .fold(f64::INFINITY, f64::min);
            endpoints.push(json!({"saved_uv":q.to_array(),"clipping_boundary":{"axis":axis,"bound":bound},"source_face":face,"source_body":f.body,"nearest_source_edge_line_distance_m":source_edge_distance,"retained_id_candidates":ids,"candidates":candidates.iter().take(8).map(|(d,id,p)|json!({"distance_m":d,"identity":id,"computed_uv":p.to_array()})).collect::<Vec<_>>()}));
        }
        let a = endpoints[0]["retained_id_candidates"].as_array().unwrap();
        let b = endpoints[1]["retained_id_candidates"].as_array().unwrap();
        let common = a
            .iter()
            .filter(|id| b.contains(id))
            .cloned()
            .collect::<Vec<_>>();
        regressions.push(json!({"triangle":w["triangle"],"source_provenance":w["source_provenance"],"distinct_uv_same_f32":true,"local_f32":v[i],"endpoints":endpoints,"common_lattice_id_candidates":common,"scope":"near candidate association is diagnostic; only exact lattice provenance may establish identity"}));
    }
    // Evaluate the same edge with opposite incident winding. Canonical endpoint
    // ordering must reproduce the same intersection bits and identities.
    let mut reversal = Value::Null;
    for t in &grid {
        if let Ok((events, _)) = chain(&chart, std::slice::from_ref(t), 64.) {
            if events.is_empty() {
                continue;
            }
            let mut reversed = t.clone();
            reversed.world.reverse();
            reversed.tags.reverse();
            reversed.uv.reverse();
            let (second, report) =
                chain(&chart, &[t.clone(), reversed], 64.).map_err(|e| e.to_string())?;
            if events.len() != second.len()
                || events
                    .iter()
                    .zip(&second)
                    .any(|(a, b)| a.uv != b.uv || a.world != b.world || a.ids != b.ids)
            {
                return Err("edge reversal regression".into());
            }
            reversal = json!({"pass":true,"events":events.len(),"report":report});
            break;
        }
    }
    if reversal.is_null() {
        return Err("missing reversal witness".into());
    }
    Ok(
        json!({"collapses_reproduced":regressions.len(),"witnesses":regressions,"orientation_reuse":reversal}),
    )
}
