//! A grown planet's ground (planet-unfold's tiles) drawn on a body, as a preview for the lab:
//! `UNIVERSE_GROUND_<WORLD ID>=<folder>` (as `UNIVERSE_BAKE_<WORLD ID>` for a bake), the folder holding the
//! tiles, the level-0 lines and their `manifest.json` (`planet-unfold-tiles/1`). Every file is checked against
//! the manifest (size and SHA-256) before anything is drawn, and a half-copied folder stops the game with
//! the file it lacks. The ground is the body's at every range, its own globe and patches never drawn, scaled
//! to the body's radius (the manifest's over the record's); the physics stands on it everywhere (the ground's
//! own query round the rings it last showed near the eye, six sets round the body beyond;
//! `UNIVERSE_GROUND_PHYSICS=0`: on the old bake).

use std::path::PathBuf;
use std::sync::OnceLock;

/// The body the ground is drawn on, the ground's own radius (m), and whether the physics stands on it.
pub struct Preview {
    pub body: String,
    pub radius_m: f64,
    pub physics: bool,
    /// The planet graph's pins: its top peaks and dips (direction in the ground's frame, height m, a peak).
    pub pins: Vec<(glam::DVec3, f64, bool)>,
}

/// The top five peaks and dips of a planet graph's root, as its page picks them: anchors flagged a peak by
/// height down, a low by height up, each at least 1,500 km from those picked before.
fn pins(l0: &std::path::Path) -> Vec<(glam::DVec3, f64, bool)> {
    use unfold_view::unfold_core::{direction, flags, Kind, LineSet};
    let Ok(set) = LineSet::load(l0) else { return Vec::new() };
    let mut out = Vec::new();
    for peak in [true, false] {
        let bit = if peak { flags::PEAK } else { flags::LOW };
        let mut a: Vec<(glam::DVec3, f64)> = set.lines.iter().filter(|l| l.kind == Kind::Anchor && l.flags & bit != 0).map(|l| (direction(l.vertices[0].lon, l.vertices[0].lat), l.vertices[0].z as f64)).collect();
        a.sort_by(|x, y| if peak { y.1.total_cmp(&x.1) } else { x.1.total_cmp(&y.1) });
        let mut picked: Vec<glam::DVec3> = Vec::new();
        for (d, z) in a {
            if picked.len() == 5 {
                break;
            }
            if picked.iter().all(|p| p.angle_between(d) * set.header.radius_m >= 1.5e6) {
                picked.push(d);
                out.push((d, z, peak));
            }
        }
    }
    out
}

/// Whether the planet graph's overlay is on (SHIFT+F7; `UNIVERSE_GROUND_GRAPH=1` at launch): its lines and anchors
/// over the ground, its top peaks and dips pinned.
pub fn graph_on() -> bool {
    unfold_view::bench::graph_overlay()
}

pub fn toggle_graph() {
    unfold_view::bench::set_graph_overlay(!graph_on());
}

static PREVIEW: OnceLock<Preview> = OnceLock::new();

/// The preview on `body`, if there is one.
pub fn on(body: &str) -> Option<&'static Preview> {
    PREVIEW.get().filter(|p| p.body == body)
}

/// The preview running, if one is (for the HUD's notice).
pub fn running() -> Option<&'static Preview> {
    PREVIEW.get()
}

/// The ground pass for the body whose record has a `ground`, or the preview named in the environment over it
/// (None: neither). Stops the game if the folder isn't what its manifest says, or the manifest isn't the record's.
pub fn open() -> Option<Box<dyn universe_engine::GroundPass>> {
    // (The record's ground (a body's `ground`: its folder in the worlds store, its manifest's hash); over it, a
    // preview's folder, `UNIVERSE_GROUND_<WORLD ID>`, its manifest not checked against a record.)
    let preview = std::env::vars_os().find_map(|(k, v)| {
        let k = k.into_string().ok()?;
        k.strip_prefix("UNIVERSE_GROUND_").filter(|id| !matches!(*id, "STATS" | "PHYSICS" | "PROBE" | "GRAPH")).map(|id| (id.to_string(), PathBuf::from(v)))
    });
    let (var, folder, want) = match preview {
        Some((id, folder)) => (id, folder, None),
        None => {
            let r = universe_sim::world::registry::registry();
            let (b, g) = r.records.bodies.iter().find_map(|b| b.ground.as_ref().map(|g| (b, g)))?;
            let id = b.survey.as_ref().map_or_else(|| b.identity.key.clone(), |s| s.world_id.clone());
            let Some(store) = universe_sim::world::worlds::store() else {
                eprintln!("{}: its ground is in the worlds store, and there is none here (set UNIVERSE_WORLDS): drawn without it", b.identity.key);
                return None;
            };
            (id, store.join(&g.path), Some(g.manifest_sha256.clone()))
        }
    };
    let fail = |e: String| -> ! { panic!("ground of {var}: {e}") };
    let bytes = std::fs::read(folder.join("manifest.json")).unwrap_or_else(|e| fail(format!("{}: {e}", folder.join("manifest.json").display())));
    if let Some(want) = &want {
        let got = universe_sim::world::worlds::sha256(&bytes);
        if &got != want {
            fail(format!("{}/manifest.json is {got}, the record says {want}", folder.display()));
        }
    }
    let m: serde_json::Value = serde_json::from_slice(&bytes).unwrap_or_else(|e| fail(format!("manifest.json: {e}")));
    if m["format"] != "planet-unfold-tiles/1" {
        fail(format!("manifest.json: format {}, not planet-unfold-tiles/1", m["format"]));
    }
    if m["world_id"].as_str() != Some(var.as_str()) {
        fail(format!("manifest.json: world {}, not {var}", m["world_id"]));
    }
    let files = m["files"].as_object().unwrap_or_else(|| fail("manifest.json: no files".into()));
    for (name, f) in files {
        let b = std::fs::read(folder.join(name)).unwrap_or_else(|e| fail(format!("{name}: {e}")));
        if f["size"].as_u64() != Some(b.len() as u64) || f["sha256"].as_str() != Some(universe_sim::world::worlds::sha256(&b).as_str()) {
            fail(format!("{name}: not the file its manifest names"));
        }
    }
    let body = m["body"].as_str().unwrap_or_else(|| fail("manifest.json: no body".into())).to_string();
    let radius_m = m["radius_m"].as_f64().unwrap_or_else(|| fail("manifest.json: no radius_m".into()));
    let l0 = folder.join(m["l0"].as_str().unwrap_or("L0_0_0.lines"));
    let physics = std::env::var("UNIVERSE_GROUND_PHYSICS").map_or(true, |v| v != "0");
    if want.is_some() {
        eprintln!("GROUND: {body}'s ground from {} (its record's, {} files checked)", folder.display(), files.len());
    } else {
        eprintln!("PREVIEW: {body}'s ground from {} (UNIVERSE_GROUND_{var}, {} files checked)", folder.display(), files.len());
    }
    let (pass, query) = unfold_view::bench::ground_pass(&folder, &l0).unwrap_or_else(|e| fail(format!("{e:#}")));
    if physics {
        // (The body's frame into the ground's, and the ground's query there: where its rings reach.)
        // (Through the query's node cache: a thousand heights a frame at ~15 µs each were 20 ms.)
        let to_ground = frame().inverse();
        let cached = unfold_view::bench::CachedQuery::new(query);
        // (The ground over the whole body, for physics far from the eye: built in the background.)
        let slot = cached.far_slot();
        let dir = folder.clone();
        std::thread::Builder::new()
            .name("far ground".into())
            .spawn(move || match (std::time::Instant::now(), unfold_view::bench::far_grounds(&dir)) {
                (t0, Ok(g)) => {
                    eprintln!("far ground: the whole body in {:.1} s", t0.elapsed().as_secs_f64());
                    // (UNIVERSE_GROUND_PROBE="lon lat; lon lat": the physics query's height there, on stderr.)
                    if let Ok(v) = std::env::var("UNIVERSE_GROUND_PROBE") {
                        for pair in v.split(';') {
                            let n: Vec<f64> = pair.split_whitespace().filter_map(|x| x.parse().ok()).collect();
                            if n.len() == 2 {
                                let d = unfold_view::unfold_core::direction(n[0], n[1]);
                                let h = g.iter().max_by(|a, b| a.rings[0].0.frame.origin().dot(d).total_cmp(&b.rings[0].0.frame.origin().dot(d))).and_then(|f| f.height(d));
                                eprintln!("ground probe ({}, {}): {}", n[0], n[1], h.map_or("none".into(), |h| format!("{h:.1} m")));
                            }
                        }
                    }
                    let _ = slot.set(g);
                }
                (_, Err(e)) => eprintln!("far ground: {e:#}"),
            })
            .expect("far ground thread");
        universe_sim::world::terrain::set_outside(&body, universe_sim::world::terrain::Outside {
            query: std::sync::Arc::new(move |dir: glam::DVec3| {
                // (UNIVERSE_GROUND_STATS=1: the query's calls and time every 5 s, on stderr.)
                if !stats() {
                    return cached.height(to_ground * dir);
                }
                let t0 = std::time::Instant::now();
                let h = cached.height(to_ground * dir);
                let mut s = STATS.lock().unwrap();
                s.0 += 1;
                s.1 += t0.elapsed().as_secs_f64();
                if s.2.elapsed().as_secs_f64() >= 5.0 {
                    eprintln!("ground query: {} calls in {:.1} s, {:.2} µs a call, {:.2} ms a second", s.0, s.2.elapsed().as_secs_f64(), s.1 / s.0.max(1) as f64 * 1e6, s.1 * 1e3 / s.2.elapsed().as_secs_f64());
                    *s = (0, 0.0, std::time::Instant::now());
                }
                h
            }),
            max: 9_000.0,
        });
    }
    if std::env::var("UNIVERSE_GROUND_GRAPH").is_ok_and(|v| v == "1") {
        unfold_view::bench::set_graph_overlay(true);
    }
    let pins = pins(&l0);
    let _ = PREVIEW.set(Preview { body, radius_m, physics, pins });
    Some(pass)
}

static STATS: std::sync::LazyLock<std::sync::Mutex<(u64, f64, std::time::Instant)>> = std::sync::LazyLock::new(|| std::sync::Mutex::new((0, 0.0, std::time::Instant::now())));

fn stats() -> bool {
    static ON: OnceLock<bool> = OnceLock::new();
    *ON.get_or_init(|| std::env::var("UNIVERSE_GROUND_STATS").is_ok_and(|v| v == "1"))
}

/// The ground's own frame (north +Z, longitude 0 on +X, east +Y) in a body's (north +Y, longitude
/// `atan2(−z, x)`: `worlds::direction`).
pub fn frame() -> glam::DQuat {
    glam::DQuat::from_mat3(&glam::DMat3::from_cols(glam::DVec3::X, glam::DVec3::NEG_Z, glam::DVec3::Y))
}
