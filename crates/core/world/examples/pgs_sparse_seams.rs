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
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let a = std::env::args().skip(1).collect::<Vec<_>>();
    if a.len() != 5 {
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
        let c = v(&f["queries"][0]["unit_direction"]);
        let chart = Chart::new(c, 6281370.);
        let (old, _) = grid::retained(&s, &chart)?;
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
        let mut strict_missing = 0;
        let mut tolerant_missing = 0;
        let mut count = 0;
        let mut worst = Value::Null;
        for &(a, b) in &edges {
            for fraction in [0., 0.25, 0.5, 0.75, 1.] {
                count += 1;
                if count > 300000 {
                    return Err("seam sample cap".into());
                }
                let p = positions[a] * (1. - fraction) + positions[b] * fraction;
                let q = chart.project(p)?;
                let d = p.normalize();
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
                        tolerant += 1;
                        let error = (grid::radial(tr, d) - p.length()).abs();
                        if error > max {
                            max = error;
                            worst = json!({"edge":[a,b],"fraction":fraction,"chart_xy":q.to_array(),"radial_seam_m":error,"outside_triangle_m":(-minimum).max(0.)});
                        }
                    }
                }
                if strict == 0 {
                    strict_missing += 1
                }
                if tolerant == 0 {
                    tolerant_missing += 1
                }
            }
        }
        results.push(json!({"feature":id,"geometry_sha256":sha256(&bytes),"outer_edges":edges.len(),"samples":count,"max_radial_seam_m":max,"strict_support_missing":strict_missing,"support_missing_within_10um":tolerant_missing,"worst":worst,"scope":"physical f32 outer edges sampled at endpoints, quarters and midpoint against reconstructed retained planes; strict misses retained;10um support tolerance is a diagnostic, not welding"}));
    }
    std::fs::write(
        &a[4],
        serde_json::to_vec_pretty(&json!({"source_sha256":sha256(&b),"results":results}))?,
    )?;
    Ok(())
}
