//! The engine's `Surface` over a revision-4 package: the lab's goldens (queries, ties, refusals),
//! every composed face's neighbours reciprocal and sharing their edge, and the package's reads.
//! cargo run -p universe-world --release --example pgs2_surface -- PGS2_FIXTURE_DIR [PACKAGE_DIR...]
use glam::DVec3;
use serde_json::{Value, json};
use std::path::Path;
use universe_world::worlds::pgs::Surface;

/// Every face: each neighbour names it back, over an edge with the same two end positions.
fn topology(s: &Surface, limit: usize) -> Result<Value, String> {
    let n = s.face_count().min(limit);
    let (mut checked, mut bad) = (0, vec![]);
    for id in 0..n as u32 {
        let f = s.face_geometry(id).ok_or(format!("face {id}"))?;
        for e in 0..3 {
            let g = s.face_geometry(f.neighbours[e]).ok_or(format!("neighbour of {id}"))?;
            let ends = [f.positions[e], f.positions[(e + 1) % 3]];
            let back = (0..3).find(|&k| g.neighbours[k] == id);
            let same = back.is_some_and(|k| [g.positions[k], g.positions[(k + 1) % 3]] == [ends[1], ends[0]]);
            if !same && bad.len() < 10 { bad.push(json!({"face":id,"edge":e,"neighbour":f.neighbours[e]})); }
            checked += 1;
        }
    }
    Ok(json!({"faces":n,"edges_checked":checked,"bad":bad}))
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let root = Path::new(a.first().ok_or("usage: pgs2_surface PGS2_FIXTURE_DIR [PACKAGE_DIR...]")?);
    let queries: Value = serde_json::from_slice(&std::fs::read(root.join("queries.json"))?)?;
    let index: Value = serde_json::from_slice(&std::fs::read(root.join("fixtures.json"))?)?;
    let (s, _) = Surface::open_package(&root.join(queries["package"].as_str().unwrap()))?;
    let all = queries["queries"].as_array().unwrap();
    let dir = |q: &Value| DVec3::from_slice(&q["direction"].as_array().unwrap().iter().map(|v| v.as_f64().unwrap()).collect::<Vec<_>>());
    let mut pass = 0;
    let mut failures = vec![];
    for q in all {
        let r = s.query(dir(q))?;
        let ok = q["composed_face"].as_u64() == Some(r.face as u64)
            && (r.height_m - q["height_m"].as_f64().unwrap()).abs() <= queries["height_tolerance_m"].as_f64().unwrap()
            && r.categories.map(|c| (c.0 as u64, c.1 as u64)) == Some((q["categories"]["rock"].as_u64().unwrap(), q["categories"]["pattern"].as_u64().unwrap()));
        if ok { pass += 1 } else if failures.len() < 5 { failures.push(json!({"q":q,"face":r.face,"height":r.height_m})); }
    }
    let mut refused = vec![];
    for case in index["refused"].as_array().unwrap() {
        let outcome = Surface::open_package(&root.join(case["dir"].as_str().unwrap())).and_then(|(s, _)| {
            for q in all { s.query(dir(q))?; }
            Ok(())
        });
        refused.push(json!([case["dir"], outcome.err()]));
    }
    let mut packages = vec![json!({"package":"goldens","topology":topology(&s, usize::MAX)?})];
    for p in &a[1..] {
        let clock = std::time::Instant::now();
        let (s, sha) = Surface::open_package(Path::new(p))?;
        let open_ms = clock.elapsed().as_secs_f64() * 1e3;
        // The root's faces next to each footprint and their region neighbours: the root's first
        // 50,000 composed faces cover every root face of an S15 root.
        packages.push(json!({"package":p,"manifest_sha256":sha,"open_ms":open_ms,"faces":s.face_count(),"points":s.point_count(),"max_height_m":s.max_height(),
            "topology_first_50000":topology(&s, 50_000)?,"reads":s.package_report()}));
    }
    println!("{}", serde_json::to_string_pretty(&json!({"queries":all.len(),"passed":pass,"failures":failures,"refused":refused,"packages":packages}))?);
    Ok(())
}
