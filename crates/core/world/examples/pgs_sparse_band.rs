#[allow(dead_code)]
mod contact_core;
#[allow(dead_code)]
mod crease;
mod sparse_band;
use glam::DVec3;
use serde_json::{Value, json};
use universe_world::worlds::{pgs::Surface, sha256};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let a = std::env::args().skip(1).collect::<Vec<_>>();
    if !(4..=5).contains(&a.len()) {
        return Err("source features plan output".into());
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
    let trace: Option<Value> = a
        .get(4)
        .map(|p| serde_json::from_slice(&std::fs::read(p).unwrap()).unwrap());
    let mut regressions = vec![];
    for f in features["features"].as_array().unwrap() {
        let id = f["id"].as_str().unwrap();
        let v = &f["queries"][0]["unit_direction"];
        let c = DVec3::new(
            v[0].as_f64().unwrap(),
            v[1].as_f64().unwrap(),
            v[2].as_f64().unwrap(),
        );
        if let Some(trace) = &trace {
            if let Some(w) = trace["results"]
                .as_array()
                .unwrap()
                .iter()
                .find(|w| w["feature"] == id)
            {
                regressions.push(json!({"feature":id,"trace":sparse_band::regression_trace(&s,c,&w["collapse_witnesses"])?}));
            }
        }
        let clock = std::time::Instant::now();
        match sparse_band::build(&s, c, id) {
            Ok(g) => {
                let blob = serde_json::to_vec(&g)?;
                std::fs::write(out.join(format!("{id}.json")), &blob)?;
                results.push(json!({"feature":id,"status":"built_for_audit","vertices":g["vertices"],"triangles":g["triangles"],"outer_constraints":g["outer_constraints"],"gpu_bytes":g["estimated_gpu_bytes"],"serialized_bytes":blob.len(),"cpu_ms":g["cpu_ms"],"chain":g["chain"],"geometry_sha256":sha256(&blob)}));
            }
            Err(reason) => {
                results.push(json!({"feature":id,"status":"refused","witness":reason,"cpu_ms":clock.elapsed().as_secs_f64()*1000.}));
            }
        }
    }
    std::fs::write(
        out.join("regressions.json"),
        serde_json::to_vec_pretty(&regressions)?,
    )?;
    std::fs::write(
        out.join("results.json"),
        serde_json::to_vec_pretty(&json!({"source_sha256":sha256(&bytes),"results":results}))?,
    )?;
    Ok(())
}
