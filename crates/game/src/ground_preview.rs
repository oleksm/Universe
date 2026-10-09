//! A grown planet's ground (planet-unfold's tiles) drawn on a body, as a preview for the lab:
//! `UNIVERSE_GROUND_<WORLD ID>=<folder>` (as `UNIVERSE_BAKE_<WORLD ID>` for a bake), the folder holding the
//! tiles, the level-0 lines and their `manifest.json` (`planet-unfold-tiles/1`). Every file is checked against
//! the manifest (size and SHA-256) before anything is drawn, and a half-copied folder stops the game with
//! the file it lacks. The ground is drawn near the body in place of its own patches, scaled to the body's
//! radius (the manifest's over the record's); the physics stands on the old bake meanwhile.

use std::path::PathBuf;
use std::sync::OnceLock;

/// The body the ground is drawn on, and the ground's own radius (m).
pub struct Preview {
    pub body: String,
    pub radius_m: f64,
}

static PREVIEW: OnceLock<Preview> = OnceLock::new();

/// The preview on `body`, if there is one.
pub fn on(body: &str) -> Option<&'static Preview> {
    PREVIEW.get().filter(|p| p.body == body)
}

/// Is a preview running at all (for the HUD's notice)?
pub fn running() -> bool {
    PREVIEW.get().is_some()
}

/// The ground pass for the preview named in the environment (None: none asked for). Stops the game if the
/// folder isn't what its manifest says.
pub fn open() -> Option<Box<dyn universe_engine::GroundPass>> {
    let (var, folder) = std::env::vars_os().find_map(|(k, v)| {
        let k = k.into_string().ok()?;
        k.strip_prefix("UNIVERSE_GROUND_").map(|id| (id.to_string(), PathBuf::from(v)))
    })?;
    let fail = |e: String| -> ! { panic!("UNIVERSE_GROUND_{var}: {e}") };
    let bytes = std::fs::read(folder.join("manifest.json")).unwrap_or_else(|e| fail(format!("{}: {e}", folder.join("manifest.json").display())));
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
    eprintln!("PREVIEW: {body}'s ground from {} (UNIVERSE_GROUND_{var}, {} files checked), physics on its old bake", folder.display(), files.len());
    let pass = unfold_view::bench::ground_pass(&folder, &l0).unwrap_or_else(|e| fail(format!("{e:#}")));
    let _ = PREVIEW.set(Preview { body, radius_m });
    Some(pass)
}

/// The ground's own frame (north +Z, longitude 0 on +X, east +Y) in a body's (north +Y, longitude
/// `atan2(−z, x)`: `worlds::direction`).
pub fn frame() -> glam::DQuat {
    glam::DQuat::from_mat3(&glam::DMat3::from_cols(glam::DVec3::X, glam::DVec3::NEG_Z, glam::DVec3::Y))
}
