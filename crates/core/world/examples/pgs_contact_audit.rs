//! Independent canonical checks of saved core/band meshes; no runtime acceptance.
use glam::DVec3;
use serde_json::{Value, json};
use universe_world::worlds::{
    pgs::{Surface, Water},
    sha256,
};
fn vec(v: &Value) -> DVec3 {
    DVec3::new(
        v[0].as_f64().unwrap(),
        v[1].as_f64().unwrap(),
        v[2].as_f64().unwrap(),
    )
}
fn stats(v: &[f64]) -> Value {
    if v.is_empty() {
        return Value::Null;
    }
    let mut a = v
        .iter()
        .filter(|x| x.is_finite())
        .map(|x| x.abs())
        .collect::<Vec<_>>();
    let invalid = v.len() - a.len();
    a.sort_by(f64::total_cmp);
    if a.is_empty() {
        return json!({"count":v.len(),"finite_count":0,"invalid_count":invalid});
    }
    json!({"count":v.len(),"finite_count":a.len(),"invalid_count":invalid,"max":a[a.len()-1],"p99":a[a.len()*99/100],"rms":(a.iter().map(|x|x*x).sum::<f64>()/a.len() as f64).sqrt()})
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let a = std::env::args().skip(1).collect::<Vec<_>>();
    if a.len() != 5 {
        return Err("surface geometry features plan report".into());
    }
    let bytes = std::fs::read(&a[0])?;
    let plan: Value = serde_json::from_slice(&std::fs::read(&a[3])?)?;
    if sha256(&bytes) != plan["source_sha256"].as_str().ok_or("hash")? {
        return Err("hash".into());
    }
    let s = Surface::read(&bytes)?;
    let features: Value = serde_json::from_slice(&std::fs::read(&a[2])?)?;
    let r = 6281370.;
    let mut results = vec![];
    for f in features["features"].as_array().unwrap() {
        let id = f["id"].as_str().unwrap();
        let path = std::path::Path::new(&a[1]).join(format!("{id}.json"));
        if !path.exists() {
            continue;
        }
        let data = std::fs::read(path)?;
        let g: Value = serde_json::from_slice(&data)?;
        let mut core = vec![];
        let mut dry_core = vec![];
        let mut wet_core = vec![];
        let mut band = vec![];
        let mut inner = vec![];
        let mut outer = vec![];
        let mut core_normal = vec![];
        let mut core_interior_same_owner = vec![];
        let mut core_boundary_same_owner = vec![];
        let mut core_other_owner = vec![];
        let mut interior_same_owner = vec![];
        let mut boundary_same_owner = vec![];
        let mut other_owner = vec![];
        let mut band_normal = vec![];
        let mut count = 0;
        let mut degenerate_triangles = 0;
        let mut worst = Value::Null;
        let mut max = 0f64;
        let mut triangles = vec![];
        for t in g["tiles"].as_array().unwrap() {
            let origin = vec(&t["origin"]);
            let v = t["vertices"]
                .as_array()
                .unwrap()
                .iter()
                .map(|p| origin + vec(p).as_vec3().as_dvec3())
                .collect::<Vec<_>>();
            let uv = t["uv"].as_array().unwrap();
            for (ti, tri) in t["triangles"].as_array().unwrap().iter().enumerate() {
                let ids = [0, 1, 2].map(|i| tri[i].as_u64().unwrap() as usize);
                let p = ids.map(|i| v[i]);
                triangles.push(p);
                let cross = (p[1] - p[0]).cross(p[2] - p[0]);
                if !cross.is_finite() || cross.length_squared() == 0. {
                    degenerate_triangles += 1;
                }
                let normal = cross.normalize();
                for w in [
                    [1., 0., 0.],
                    [0., 1., 0.],
                    [0., 0., 1.],
                    [0.5, 0.5, 0.],
                    [0.5, 0., 0.5],
                    [0., 0.5, 0.5],
                    [1. / 3.; 3],
                ] {
                    count += 1;
                    if count > 300000 {
                        return Err("sample cap".into());
                    }
                    let pos = p[0] * w[0] + p[1] * w[1] + p[2] * w[2];
                    let d = pos.normalize();
                    let (q, der) = s.query_differential(d, r)?;
                    let expected = match q.water {
                        Water::Wet { level_m, .. } => level_m,
                        _ => q.height_m,
                    };
                    let error = pos.length() - r - expected;
                    let xy = [0, 1].map(|axis| {
                        (0..3)
                            .map(|j| uv[ids[j]][axis].as_f64().unwrap() * w[j])
                            .sum::<f64>()
                    });
                    let dist = xy[0].abs().max(xy[1].abs());
                    let want = if matches!(q.water, Water::Wet { .. }) {
                        d
                    } else {
                        der.normal
                    };
                    let normal = if normal.dot(d) < 0. { -normal } else { normal };
                    let angle = normal.angle_between(want).to_degrees();
                    let owner = t["provenance"][ti][0].as_u64().unwrap() as u32;
                    if q.face != owner {
                        other_owner.push(angle)
                    } else if w.iter().all(|v| *v > 0.) {
                        interior_same_owner.push(angle)
                    } else {
                        boundary_same_owner.push(angle)
                    }
                    if dist <= 32. + 1e-7 {
                        if q.face != owner {
                            core_other_owner.push(angle)
                        } else if w.iter().all(|v| *v > 0.) {
                            core_interior_same_owner.push(angle)
                        } else {
                            core_boundary_same_owner.push(angle)
                        }
                        core.push(error);
                        core_normal.push(angle);
                        if matches!(q.water, Water::Wet { .. }) {
                            wet_core.push(error)
                        } else {
                            dry_core.push(error)
                        }
                    } else {
                        band.push(error);
                        band_normal.push(angle)
                    }
                    if (dist - 32.).abs() < 1e-7 {
                        inner.push(error)
                    }
                    if (dist - 64.).abs() < 1e-7 {
                        outer.push(error)
                    }
                    if error.abs() > max {
                        max = error.abs();
                        worst = json!({"error_m":error,"chart_xy":xy,"bed_height_m":q.height_m,"visible_height_m":expected,"water":format!("{:?}",q.water)});
                    }
                }
            }
        }
        let mut frozen = vec![];
        for q in f["queries"].as_array().unwrap() {
            let d = vec(&q["unit_direction"]).normalize();
            let sample = s.query(d)?;
            let h = match sample.water {
                Water::Wet { level_m, .. } => level_m,
                _ => sample.height_m,
            };
            let mut hits = vec![];
            for &[a, b, c] in &triangles {
                let n = (b - a).cross(c - a);
                let den = n.dot(d);
                if den.abs() < 1e-20 {
                    continue;
                }
                let radial = n.dot(a) / den;
                let p = d * radial;
                let v = (p - a).cross(c - a).dot(n) / n.length_squared();
                let w = (b - a).cross(p - a).dot(n) / n.length_squared();
                if v >= -1e-7 && w >= -1e-7 && v + w <= 1. + 1e-7 {
                    hits.push(radial - r - h)
                }
            }
            frozen.push(json!({"label":q["label"],"hit_count":hits.len(),"error":stats(&hits),"water":format!("{:?}",sample.water)}));
        }
        results.push(json!({"feature":id,"geometry_sha256":sha256(&data),"sample_count":count,"degenerate_triangles":degenerate_triangles,"core_visible_error_m":stats(&core),"dry_core_error_m":stats(&dry_core),"wet_core_visible_error_m":stats(&wet_core),"band_error_m":stats(&band),"inner_error_m":stats(&inner),"outer_error_m":stats(&outer),"core_normal_attribution":{"interior_same_owner":stats(&core_interior_same_owner),"boundary_same_owner":stats(&core_boundary_same_owner),"different_query_owner":stats(&core_other_owner)},"normal_attribution":{"interior_same_owner":stats(&interior_same_owner),"boundary_same_owner":stats(&boundary_same_owner),"different_query_owner":stats(&other_owner)},"core_normal_degrees":stats(&core_normal),"band_normal_degrees":stats(&band_normal),"worst":worst,"frozen_queries":frozen,"scope":"finite physical f32 triangle samples; wet visible geometry is not dry contact"}));
    }
    std::fs::write(
        &a[4],
        serde_json::to_vec_pretty(&json!({"source_sha256":sha256(&bytes),"results":results}))?,
    )?;
    Ok(())
}
