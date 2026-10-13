//! Explicit core/band CPU experiment. No runtime publication.
mod contact_core;
#[allow(dead_code)]
mod crease;
use glam::DVec3;
use serde_json::{Value, json};
use universe_world::worlds::{pgs::Surface, sha256};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let a = std::env::args().skip(1).collect::<Vec<_>>();
    if a.len() != 4 {
        return Err("source features plan out-dir".into());
    }
    let bytes = std::fs::read(&a[0])?;
    let plan: Value = serde_json::from_slice(&std::fs::read(&a[2])?)?;
    if sha256(&bytes) != plan["source_sha256"].as_str().ok_or("source binding")? {
        return Err("source hash".into());
    }
    let s = Surface::read(&bytes)?;
    let features: Value = serde_json::from_slice(&std::fs::read(&a[1])?)?;
    let out = std::path::Path::new(&a[3]);
    std::fs::create_dir_all(out)?;
    let mut results = vec![];
    for f in features["features"].as_array().ok_or("features")? {
        let id = f["id"].as_str().unwrap();
        let d = &f["queries"][0]["unit_direction"];
        let c = DVec3::new(
            d[0].as_f64().unwrap(),
            d[1].as_f64().unwrap(),
            d[2].as_f64().unwrap(),
        );
        let clock = std::time::Instant::now();
        let result = match contact_core::build(&s, c, 6281370., id) {
            Ok(g) => {
                let blob = serde_json::to_vec(&g)?;
                let ok = blob.len() <= 2097152;
                if ok {
                    std::fs::write(out.join(format!("{id}.json")), &blob)?;
                }
                json!({"feature":id,"status":if ok{"built_for_audit"}else{"refused"},"reason":if ok{"pending core/seam/topology audits"}else{"serialized output cap"},"serialized_bytes":blob.len(),"vertices":g.vertices,"triangles":g.triangles,"source_search":g.source_search,"retained_triangles":g.retained_triangles,"retained_patch_search":g.retained_patch_search,"area_m2":g.raw_area_m2,"outer_vertex_max_displacement_m":g.outer_vertex_max_displacement_m,"duplicate_disagreement_m":g.retained_grid_max_overlap_disagreement_m,"estimated_gpu_bytes":g.output_gpu_bytes,"cpu_ms":g.cpu_ms,"geometry_sha256":if ok{Some(sha256(&blob))}else{None}})
            }
            Err(e) => {
                json!({"feature":id,"status":"refused","reason":e,"cpu_ms":clock.elapsed().as_secs_f64()*1000.})
            }
        };
        results.push(result);
    }
    std::fs::write(
        out.join("results.json"),
        serde_json::to_vec_pretty(
            &json!({"source_sha256":sha256(&bytes),"results":results,"scope":"CPU retained grid reconstruction plus canonical overlay; no GPU residency/readiness claim"}),
        )?,
    )?;
    Ok(())
}
