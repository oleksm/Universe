#[allow(dead_code)]
mod boundary_correction;
#[allow(dead_code)]
mod contact_core;
#[allow(dead_code)]
mod crease;
use contact_core::{Chart, grid};
use glam::{DVec2, DVec3};
use serde_json::{Value, json};
use std::collections::BTreeSet;
use universe_world::worlds::{pgs::Surface, sha256};
fn v(p: &Value) -> DVec3 {
    DVec3::new(
        p[0].as_f64().unwrap(),
        p[1].as_f64().unwrap(),
        p[2].as_f64().unwrap(),
    )
}
fn triangle_distance(p: DVec3, t: &grid::Triangle) -> f64 {
    let [a, b, c] = t.world;
    let n = (b - a).cross(c - a);
    let q = p - n * ((p - a).dot(n) / n.length_squared());
    let v = (q - a).cross(c - a).dot(n) / n.length_squared();
    let w = (b - a).cross(q - a).dot(n) / n.length_squared();
    if v >= 0. && w >= 0. && v + w <= 1. {
        return p.distance(q);
    }
    [(a, b), (b, c), (c, a)]
        .into_iter()
        .map(|(a, b)| {
            let d = b - a;
            p.distance(a + d * ((p - a).dot(d) / d.length_squared()).clamp(0., 1.))
        })
        .fold(f64::INFINITY, f64::min)
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let a = std::env::args().skip(1).collect::<Vec<_>>();
    if !(5..=6).contains(&a.len()) {
        return Err("source geometry features plan report".into());
    }
    let b = std::fs::read(&a[0])?;
    let plan: Value = serde_json::from_slice(&std::fs::read(&a[3])?)?;
    if sha256(&b) != plan["source_sha256"].as_str().ok_or("hash")? {
        return Err("hash".into());
    }
    let s = Surface::read(&b)?;
    let features: Value = serde_json::from_slice(&std::fs::read(&a[2])?)?;
    let mut results = vec![];
    for f in features["features"].as_array().unwrap() {
        let id = f["id"].as_str().unwrap();
        let path = std::path::Path::new(&a[1]).join(format!("{id}.json"));
        if !path.exists() {
            continue;
        }
        let bytes = std::fs::read(path)?;
        let g: Value = serde_json::from_slice(&bytes)?;
        let c = v(g.get("anchor_direction").unwrap_or(&f["queries"][0]["unit_direction"]));
        let chart = Chart::new(c, 6281370.);
        let (mut old, _) = grid::retained_compact(&s, &chart)?;
        if a.len() == 6 {
            boundary_correction::ownership::correct(
                &mut old,
                &std::sync::atomic::AtomicU64::new(1),
                1,
                &chart,
            )
            .map_err(|e| e.to_string())?;
            for t in &mut old {
                t.uv = t.world.iter().map(|p| chart.project(*p).unwrap()).collect();
                t.lo =
                    t.uv.iter()
                        .copied()
                        .fold(DVec2::splat(f64::INFINITY), DVec2::min);
                t.hi =
                    t.uv.iter()
                        .copied()
                        .fold(DVec2::splat(f64::NEG_INFINITY), DVec2::max);
            }
        }
        let t = &g["tiles"][0];
        let origin = v(&t["origin"]);
        let positions = t["vertices"]
            .as_array()
            .unwrap()
            .iter()
            .map(|p| origin + v(p).as_vec3().as_dvec3())
            .collect::<Vec<_>>();
        let uv = t["uv"]
            .as_array()
            .unwrap()
            .iter()
            .map(|p| DVec2::new(p[0].as_f64().unwrap(), p[1].as_f64().unwrap()))
            .collect::<Vec<_>>();
        let mut edges = BTreeSet::new();
        for tri in t["triangles"].as_array().unwrap() {
            let ids = [0, 1, 2].map(|i| tri[i].as_u64().unwrap() as usize);
            for (i, j) in [(0, 1), (1, 2), (2, 0)] {
                let (a, b) = (ids[i], ids[j]);
                if (uv[a].x == uv[b].x && uv[a].x.abs() == 64.)
                    || (uv[a].y == uv[b].y && uv[a].y.abs() == 64.)
                {
                    edges.insert((a.min(b), a.max(b)));
                }
            }
        }
        let mut max = 0f64;
        let mut full_edge_bound = 0f64;
        let mut full_edge_failures = vec![];
        let mut strict_missing = 0;
        let mut tolerant_missing = 0;
        let mut count = 0;
        let mut worst = Value::Null;
        let mut misses = vec![];
        for &(a, b) in &edges {
            let qa = chart.project(positions[a])?;
            let qb = chart.project(positions[b])?;
            let mut bound = f64::INFINITY;
            for tr in &old {
                if qa.min(qb).x < tr.lo.x - 1e-5
                    || qa.max(qb).x > tr.hi.x + 1e-5
                    || qa.min(qb).y < tr.lo.y - 1e-5
                    || qa.max(qb).y > tr.hi.y + 1e-5
                {
                    continue;
                }
                bound = bound.min(
                    triangle_distance(positions[a], tr).max(triangle_distance(positions[b], tr)),
                );
            }
            full_edge_bound = full_edge_bound.max(bound);
            if bound > 1e-5 {
                full_edge_failures.push(
                    json!({"edge":[a,b],"bound_m":if bound.is_finite(){Some(bound)}else{None}}),
                );
            }

            for fraction in [0., 0.25, 0.5, 0.75, 1.] {
                count += 1;
                if count > 300000 {
                    return Err("seam sample cap".into());
                }
                let p = positions[a] * (1. - fraction) + positions[b] * fraction;
                let q = chart.project(p)?;
                let d = p.normalize();
                let mut supports = vec![];
                let mut strict = 0;
                let mut tolerant = 0;
                for tr in &old {
                    if q.x < tr.lo.x - 1e-5
                        || q.x > tr.hi.x + 1e-5
                        || q.y < tr.lo.y - 1e-5
                        || q.y > tr.hi.y + 1e-5
                    {
                        continue;
                    }
                    let orientation = (tr.uv[1] - tr.uv[0]).perp_dot(tr.uv[2] - tr.uv[0]).signum();
                    let minimum = (0..3)
                        .map(|i| {
                            let delta = tr.uv[(i + 1) % 3] - tr.uv[i];
                            orientation * delta.perp_dot(q - tr.uv[i]) / delta.length()
                        })
                        .fold(f64::INFINITY, f64::min);
                    if minimum >= 0. {
                        strict += 1;
                    }
                    if minimum >= -1e-5 {
                        supports.push(json!({"world":tr.world.map(|p|p.to_array()),"uv":tr.uv.iter().map(|p|p.to_array()).collect::<Vec<_>>(),"patch":tr.patch,"tags":tr.tags,"minimum":minimum}));
                        tolerant += 1;
                        let error = (grid::radial(tr, d) - p.length()).abs();
                        if error > max {
                            max = error;
                            worst = json!({"edge":[a,b],"fraction":fraction,"chart_xy":q.to_array(),"radial_seam_m":error,"outside_triangle_m":(-minimum).max(0.)});
                        }
                    }
                }
                if strict == 0 {
                    strict_missing += 1;
                    misses.push(json!({"point":p.to_array(),"query_uv":q.to_array(),"edge":[a,b],"fraction":fraction,"supports":supports}));
                }
                if tolerant == 0 {
                    tolerant_missing += 1
                }
            }
        }
        results.push(json!({"feature":id,"geometry_sha256":sha256(&bytes),"outer_edges":edges.len(),"samples":count,"max_radial_seam_m":max,"complete_edge_support_bound_m":if full_edge_bound.is_finite(){Some(full_edge_bound)}else{None},"complete_edge_failures":full_edge_failures,"strict_support_missing":strict_missing,"misses":misses,"chart":{"c":chart.c.to_array(),"e":chart.e.to_array(),"n":chart.n.to_array(),"r":chart.r},"support_missing_within_10um":tolerant_missing,"worst":worst,"scope":"physical f32 outer edges sampled at endpoints, quarters and midpoint against reconstructed retained planes; strict misses retained;10um support tolerance is a diagnostic, not welding"}));
    }
    std::fs::write(
        &a[4],
        serde_json::to_vec_pretty(&json!({"source_sha256":sha256(&b),"results":results}))?,
    )?;
    Ok(())
}
