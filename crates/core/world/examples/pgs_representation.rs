#[allow(dead_code)]
mod contact_core;
#[allow(dead_code)]
mod crease;
#[allow(dead_code)]
mod sparse_band;
use glam::DVec3;
use serde_json::{Value, json};
use universe_world::worlds::{pgs::Surface, sha256};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let a = std::env::args().skip(1).collect::<Vec<_>>();
    let bytes = std::fs::read(&a[0])?;
    let plan: Value = serde_json::from_slice(&std::fs::read(&a[2])?)?;
    if sha256(&bytes) != plan["source_sha256"].as_str().ok_or("hash")? {
        return Err("hash".into());
    }
    let s = Surface::read(&bytes)?;
    let features: Value = serde_json::from_slice(&std::fs::read(&a[1])?)?;
    let mut out = vec![];
    for f in features["features"].as_array().unwrap() {
        let id = f["id"].as_str().unwrap();
        if !["parent_face_join", "canonical_crest_1", "canonical_crest_2"].contains(&id) {
            continue;
        }
        let v = &f["queries"][0]["unit_direction"];
        let c = DVec3::new(
            v[0].as_f64().unwrap(),
            v[1].as_f64().unwrap(),
            v[2].as_f64().unwrap(),
        );
        let chart = contact_core::Chart::new(c, 6281370.);
        if id == "parent_face_join" {
            let face = s.face_geometry(40805).ok_or("face")?;
            let mut faces = vec![];
            for owner in std::iter::once(40805).chain(face.neighbours) {
                let f = s.face_geometry(owner).ok_or("neighbour")?;
                faces.push(json!({"owner":owner,"positions":f.positions.map(|p|p.to_array()),"uv":f.positions.map(|p|chart.project(p).unwrap().to_array()),"bed":f.bed_heights_m,"body":f.body}));
            }
            out.push(json!({"feature":id,"faces":faces,"chart":{"c":chart.c.to_array(),"n":chart.n.to_array(),"e":chart.e.to_array(),"r":chart.r}}));
        } else {
            let (grid, _) = contact_core::grid::retained(&s, &chart)?;
            out.push(json!({"feature":id,"witness":sparse_band::chain(&chart,&grid,64.).err()}));
        }
    }
    std::fs::write(
        &a[3],
        serde_json::to_vec_pretty(&json!({"source_sha256":sha256(&bytes),"results":out}))?,
    )?;
    Ok(())
}
