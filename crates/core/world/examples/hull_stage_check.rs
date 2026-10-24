//! A candidate hull export checked as an uncommissioned stage before the registry has its record: the real importer
//! on its exact bytes, its mechanisms and exits as given (JSON: model_sha256, mechanisms, exits, in the hull schema's
//! form), an empty fit. Reports whether it binds, each marker's place in each state, ground gaps against the plane of
//! its gear feet, which exits open, a walk down the walk exit on its posed floors and the climb exit's ladder.
//!
//! `cargo run --release -p universe-world --example hull_stage_check -- <model.glb> <mechanisms.json>`
//!
//! A check, not a record: its mass and identity are the MC-07 record's template (the stage's own mass isn't read).
use glam::{DQuat, DVec3};
use universe_world::{crew, walk, Ship};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let (model, json) = (&args[1], &args[2]);
    let bytes = std::fs::read(model).expect("model");
    let given: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(json).expect("json")).expect("json");
    let mut rec = universe_world::registry::registry().hull("hull.mc-07").unwrap().clone();
    rec.model = Some(model.clone());
    rec.slots.clear();
    rec.fit.clear();
    rec.thrusters.clear();
    rec.model_sha256 = given["model_sha256"].as_str().map(str::to_string);
    rec.mechanisms = serde_json::from_value(given["mechanisms"].clone()).expect("mechanisms");
    rec.exits = serde_json::from_value(given["exits"].clone()).expect("exits");
    let spec = match universe_world::import::hull_from_record(&bytes, model, Some(&rec)) {
        Ok(s) => s,
        Err(e) => return println!("REFUSED: {e}"),
    };
    println!("imported: commissioned {} ({})", spec.commissioned, spec.limits.first().map_or("", String::as_str));
    let h = spec.mechanisms.clone().expect("no mechanisms bound");
    let b = &h.bound;
    println!("bound: {} mechanisms, {} exits, {} moving floors", b.mechanisms.len(), b.exits.len(), h.floors.len());
    let shape = spec.shape();
    let feet: Vec<DVec3> = shape.nodes(universe_world::shape::Role::Gear).map(|n| n.at).collect();
    let ground = feet.iter().map(|f| f.y).fold(f64::INFINITY, f64::min);
    println!("gear feet: {} (ground plane y {ground:.3} m, spread {:.3} m)", feet.len(), feet.iter().map(|f| f.y).fold(f64::NEG_INFINITY, f64::max) - ground);
    let closed = b.defaults();
    let deployed: Vec<usize> = (0..b.mechanisms.len()).map(|i| b.state(i, "deployed").unwrap_or(b.mechanisms[i].default)).collect();
    for (i, m) in b.mechanisms.iter().enumerate() {
        if let Some(c) = m.contact {
            let (p0, p1) = (h.point(c, &closed), h.point(c, &deployed));
            println!("{}: contact {} closed y {:.3}, deployed y {:.3}: gap over the gear plane {:+.3} m", m.key, b.name(c), p0.y, p1.y, p1.y - ground);
        }
        let _ = i;
    }
    for e in &b.exits {
        let top = h.point(e.top, &deployed);
        let foot = e.foot.map(|f| h.point(f, &deployed));
        println!("exit {} ({:?}): open closed {} / deployed {}; top {:?}, foot {:?} (foot over gear plane {:+.3} m)", e.key, e.kind, b.open(e, &closed), b.open(e, &deployed), top, foot, foot.map_or(f64::NAN, |f| f.y - ground));
    }
    let mut ship = Ship::with_spec(std::sync::Arc::new(spec.clone()), DVec3::ZERO, DVec3::ZERO, DQuat::IDENTITY);
    ship.mechanisms = deployed.clone();
    let mut colliders = Vec::new();
    crew::ship_colliders(&ship, DVec3::ZERO, DQuat::IDENTITY, 0.0, &mut colliders);
    for e in &b.exits {
        let (top, foot) = (h.point(e.top, &deployed), e.foot.map(|f| h.point(f, &deployed)));
        match (e.kind, foot) {
            (universe_world::registry::ExitKind::Walk, Some(foot)) => {
                // Down the walk exit: the floor under each step from its top to its foot, posed.
                let mut last: Option<f64> = None;
                let (mut worst_step, mut steep, mut gaps) = (0.0f64, 0, 0);
                for k in 0..=40 {
                    let p = top.lerp(foot, k as f64 / 40.0);
                    let from = DVec3::new(p.x, p.y.max(foot.y) + 1.0, p.z);
                    match walk::ray(&colliders, from, DVec3::NEG_Y, 4.0) {
                        Some((d, n)) => {
                            let y = from.y - d;
                            if n.y <= walk::SLOPE { steep += 1; }
                            if let Some(l) = last { worst_step = worst_step.max((y - l).abs()); }
                            last = Some(y);
                        }
                        None => gaps += 1,
                    }
                }
                let off = last.map_or(f64::NAN, |l| l - ground);
                println!("walk {}: 41 samples, {gaps} without floor, {steep} too steep, worst step {worst_step:.3} m (walker {:.2}), last floor {off:+.3} m over the gear plane", e.key, walk::STEP);
            }
            (universe_world::registry::ExitKind::Climb, Some(foot)) => {
                let held = (0..=20).filter(|k| crew::on_ladder(&ship, top.lerp(foot, *k as f64 / 20.0))).count();
                println!("climb {}: length {:.3} m, held at {held}/21 points along it; foot {:+.3} m over the gear plane", e.key, top.distance(foot), foot.y - ground);
            }
            _ => {}
        }
    }
}
