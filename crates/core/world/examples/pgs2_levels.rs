//! Revision-5 levels through the engine's `Surface`: the reader's rule (which representation
//! answers a tolerance), every level held to level 0 (gates 1 and 2: points byte-identical, the
//! declared deviation re-measured at the overlay's vertices), and a sweep of directions over each
//! region with levels: at the tolerance of each level the answer stays within the declared
//! deviation of level 0, tolerance 0 is level 0 exactly.
//! cargo run -p universe-world --release --example pgs2_levels -- PACKAGE_DIR OUT.json
use glam::DVec3;
use serde_json::{Value, json};
use std::path::Path;
use universe_world::worlds::pgs::Surface;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let (dir, out) = (Path::new(a.first().ok_or("usage: pgs2_levels PACKAGE_DIR OUT.json")?), a.get(1).ok_or("OUT.json")?);
    let manifest: Value = serde_json::from_slice(&std::fs::read(dir.join("planet.json"))?)?;
    let (s, sha) = Surface::open_package(dir)?;
    let mut regions = vec![];
    for r in manifest["regions"].as_array().unwrap().iter().filter(|r| r["levels"].is_array()) {
        let id = r["id"].as_str().unwrap();
        let cap = &r["cap"];
        let centre = DVec3::from_slice(&cap["centre"].as_array().unwrap().iter().map(|v| v.as_f64().unwrap()).collect::<Vec<_>>());
        let radius = cap["radius"].as_f64().unwrap();
        // A grid of directions over the cap (inside and around the footprint).
        let (e1, e2) = centre.any_orthonormal_pair();
        let n = 301;
        let dirs: Vec<DVec3> = (0..n * n).map(|k| {
            let (u, v) = ((k % n) as f64 / (n - 1) as f64 * 2. - 1., (k / n) as f64 / (n - 1) as f64 * 2. - 1.);
            (centre + (e1 * u + e2 * v) * radius).normalize()
        }).collect();
        let fine: Vec<f64> = dirs.iter().map(|&d| s.query(d).map(|q| q.height_m)).collect::<Result<_, _>>()?;
        let mut tolerances: Vec<(String, f64)> = vec![("0".into(), 0.0)];
        for l in r["levels"].as_array().unwrap() {
            tolerances.push((format!("level {} declared", l["level"]), l["deviation_max_m"].as_f64().unwrap()));
        }
        tolerances.push(("root declared".into(), r["root_deviation_max_m"].as_f64().unwrap()));
        let mut sweep = vec![];
        for (name, t) in &tolerances {
            let mut worst = 0f64;
            for (k, &d) in dirs.iter().enumerate() {
                let q = s.query_to(d, *t)?;
                worst = worst.max((q.height_m - fine[k]).abs());
            }
            sweep.push(json!({"tolerance":name,"tolerance_m":t,"directions":dirs.len(),"max_abs_difference_from_level0_m":worst,
                "within_tolerance":worst <= t + 1e-6}));
        }
        let before = s.package_report().unwrap();
        let started = std::time::Instant::now();
        let verify = s.verify_levels(id)?;
        regions.push(json!({"region":id,"sweep":sweep,"verify":verify,"verify_s":started.elapsed().as_secs_f64(),"reads_before_verify":before}));
    }
    let report = json!({"format":"pgs2-levels-check/1","package":dir,"manifest_sha256":sha,"regions":regions});
    std::fs::write(out, serde_json::to_string_pretty(&report)?)?;
    println!("{}", serde_json::to_string_pretty(&report["regions"].as_array().unwrap().iter().map(|r| json!({"region":r["region"],"sweep":r["sweep"],"levels":r["verify"]["levels"]})).collect::<Vec<_>>())?);
    Ok(())
}
