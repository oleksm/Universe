//! Opt-in compiled worker preflight; writes portable geometry, never installs it.
mod crease;
use glam::DVec3;
use serde_json::{Value, json};
use std::sync::atomic::AtomicU64;
use universe_world::worlds::{pgs::Surface, sha256};
fn dir(v: &Value) -> DVec3 {
    DVec3::new(
        v[0].as_f64().unwrap(),
        v[1].as_f64().unwrap(),
        v[2].as_f64().unwrap(),
    )
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let a = std::env::args().skip(1).collect::<Vec<_>>();
    if a.len() != 4 {
        return Err("source features plan output-dir".into());
    }
    let bytes = std::fs::read(&a[0])?;
    let plan: Value = serde_json::from_slice(&std::fs::read(&a[2])?)?;
    if sha256(&bytes) != plan["source_sha256"].as_str().ok_or("source binding")? {
        return Err("source hash".into());
    }
    let clock = std::time::Instant::now();
    let s = Surface::read(&bytes)?;
    let decode_ms = clock.elapsed().as_secs_f64() * 1000.;
    let features: Value = serde_json::from_slice(&std::fs::read(&a[1])?)?;
    let r = plan["radius_m"].as_f64().ok_or("radius")?;
    let mut centres = features["features"]
        .as_array()
        .ok_or("features")?
        .iter()
        .map(|f| {
            (
                f["id"].as_str().unwrap().to_string(),
                dir(&f["queries"][0]["unit_direction"]),
            )
        })
        .collect::<Vec<_>>();
    // Construct the same large-face control, but build still seeds via query.
    let shore = &features["features"][0];
    let large = shore["faces"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| s.face_geometry(f.as_u64().unwrap() as u32).unwrap())
        .max_by(|a, b| {
            let size = |p: [DVec3; 3]| (p[1] - p[0]).cross(p[2] - p[0]).length_squared();
            size(a.positions).total_cmp(&size(b.positions))
        })
        .unwrap();
    centres.push((
        "large_native_face_centroid".into(),
        large.positions.iter().sum::<DVec3>().normalize(),
    ));
    let output = std::path::Path::new(&a[3]);
    std::fs::create_dir_all(output)?;
    let generation = AtomicU64::new(1);
    let mut results = vec![];
    for (id, c) in centres {
        let g = crease::build(&s, c, r, &id, &generation, 1)?;
        let blob = serde_json::to_vec(&g)?;
        if blob.len() > 2097152 {
            return Err("serialized cap".into());
        }
        std::fs::write(output.join(format!("{id}.json")), &blob)?;
        results.push(json!({"feature":id,"geometry_sha256":sha256(&blob),"worker_ms":g.cpu_ms,"seed_owner":g.seed_owner,"search_faces":g.search_faces,"vertices":g.vertices,"triangles":g.triangles,"gpu_expanded_bytes":g.gpu_expanded_bytes,"serialized_bytes":blob.len(),"retained_cpu_capacity_bytes":g.retained_cpu_capacity_bytes,"seam_max_m":g.seam_max_m}));
    }
    let stale = crease::build(&s, DVec3::X, r, "stale", &generation, 0)
        .err()
        .ok_or("stale accepted")?;
    std::fs::write(
        output.join("results.json"),
        serde_json::to_vec_pretty(
            &json!({"source_sha256":sha256(&bytes),"decode_ms":decode_ms,"results":results,"stale_refusal":stale,"scope":"compiled preflight; no GPU allocations or runtime publication"}),
        )?,
    )?;
    Ok(())
}
