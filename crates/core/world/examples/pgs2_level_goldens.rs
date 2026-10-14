//! Revision-5 level goldens through the engine's `Surface` (planets' tests/fixtures/pgs2-v2-levels):
//! every query at its tolerance (owner in the mixture's composed ids, height, categories), and the
//! five refusals, each where it must be refused (at open, or by the level check).
//! cargo run -p universe-world --release --example pgs2_level_goldens -- FIXTURE_DIR
use glam::DVec3;
use serde_json::{Value, json};
use std::path::Path;
use universe_world::worlds::pgs::Surface;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let root = std::env::args().nth(1).ok_or("usage: pgs2_level_goldens FIXTURE_DIR")?;
    let root = Path::new(&root);
    let queries: Value = serde_json::from_slice(&std::fs::read(root.join("queries.json"))?)?;
    let package = root.join(queries["package"].as_str().unwrap());
    let manifest: Value = serde_json::from_slice(&std::fs::read(package.join("planet.json"))?)?;
    let (s, _) = Surface::open_package(&package)?;
    let tol = queries["height_tolerance_m"].as_f64().unwrap();
    let (mut pass, mut failures) = (0, vec![]);
    for q in queries["queries"].as_array().unwrap() {
        let d = DVec3::from_slice(&q["direction"].as_array().unwrap().iter().map(|v| v.as_f64().unwrap()).collect::<Vec<_>>());
        let t = q["tolerance_m"].as_f64().unwrap();
        let r = s.query_to(d, t)?;
        let cat = &q["categories"];
        let cat_ok = match r.categories {
            Some((rock, pattern)) => cat["state"] == "known" && cat["rock"].as_u64() == Some(rock as u64) && cat["pattern"].as_u64() == Some(pattern as u64),
            None => cat["state"] != "known",
        };
        let ok = q["composed_face"].as_u64() == Some(r.face as u64) && (r.height_m - q["height_m"].as_f64().unwrap()).abs() <= tol && cat_ok
            && (r.height_m - q["level0_height_m"].as_f64().unwrap()).abs() <= t + tol;
        if ok { pass += 1 } else if failures.len() < 12 {
            failures.push(json!({"kind":q["kind"],"tolerance_m":t,"want_face":q["composed_face"],"got_face":r.face,"want_h":q["height_m"],"got_h":r.height_m,"owner":q["owner"],"representations":q["representations"]}));
        }
    }
    // Gates 1-2 on the goldens' own levels.
    let mut verified = vec![];
    for r in manifest["regions"].as_array().unwrap().iter().filter(|r| r["levels"].is_array()) {
        verified.push(s.verify_levels(r["id"].as_str().unwrap())?);
    }
    let mut refused = vec![];
    for entry in std::fs::read_dir(root.join("refused"))? {
        let dir = entry?.path();
        let outcome = match Surface::open_package(&dir) {
            Err(e) => json!({"stage":"open","refused":e}),
            Ok((s, _)) => {
                let m: Value = serde_json::from_slice(&std::fs::read(dir.join("planet.json"))?)?;
                let mut res = json!({"stage":"verify","refused":null});
                for r in m["regions"].as_array().unwrap().iter().filter(|r| r["levels"].is_array()) {
                    if let Err(e) = s.verify_levels(r["id"].as_str().unwrap()) { res = json!({"stage":"verify","refused":e}); break; }
                }
                res
            }
        };
        refused.push(json!({"case":dir.file_name().unwrap().to_string_lossy(),"outcome":outcome}));
    }
    refused.sort_by(|a, b| a["case"].as_str().cmp(&b["case"].as_str()));
    let all_refused = refused.iter().all(|r| !r["outcome"]["refused"].is_null());
    println!("{}", serde_json::to_string_pretty(&json!({"queries":queries["queries"].as_array().unwrap().len(),"pass":pass,"failures":failures,"verified":verified,"refused":refused,"all_refused":all_refused}))?);
    Ok(())
}
