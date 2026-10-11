//! Models for the things the registry describes (docs/asset-contract.md): a record of a kind that takes
//! a model (equipment, industrial modules, buildings, structures, gate rings, hulls) may carry `visual`,
//! a version folder in the assets store (`UNIVERSE_ASSETS`, else `~/git/freefall-assets`) holding
//! `model.glb` and `manifest.json` (`freefall-model/1`). The manifest is checked against the record's
//! hash and the model against the manifest, as a world's ground is; anything that fails is said once and
//! the thing is drawn as its box.

use std::path::PathBuf;

use crate::registry::{registry, Visual};
use crate::worlds::Package;

/// The assets store: `UNIVERSE_ASSETS`, else `~/git/freefall-assets`, if either exists.
pub fn store() -> Option<PathBuf> {
    if let Some(p) = std::env::var_os("UNIVERSE_ASSETS") {
        return Some(PathBuf::from(p));
    }
    let home = std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE"))?;
    Some(PathBuf::from(home).join("git/freefall-assets")).filter(|p| p.is_dir())
}

/// The `visual` of the record `key`, of whichever kind takes a model. None: no such record, or no model.
pub fn visual(key: &str) -> Option<&'static Visual> {
    let r = registry();
    r.equipment(key)
        .and_then(|x| x.visual.as_ref())
        .or_else(|| r.module(key).and_then(|x| x.visual.as_ref()))
        .or_else(|| r.building(key).and_then(|x| x.visual.as_ref()))
        .or_else(|| r.structure(key).and_then(|x| x.visual.as_ref()))
        .or_else(|| r.gate(key).and_then(|x| x.visual.as_ref()))
        .or_else(|| r.hull(key).and_then(|x| x.visual.as_ref()))
}

/// A model, checked: its glTF bytes, and its bounds in its own frame (m; zero when the manifest lacks
/// them), to stand it at its thing's place.
pub struct Model {
    pub glb: Vec<u8>,
    pub lo: [f64; 3],
    pub hi: [f64; 3],
    /// A declared motion contract requires an articulated consumer.
    pub has_motion: bool,
}

/// The model `visual` names, from the assets store, checked (the manifest against the record's hash,
/// `model.glb` against the manifest).
pub fn model(visual: &Visual) -> Result<Model, String> {
    if visual.format != "freefall-model/1" {
        return Err(format!("{}: format {:?}, this reads freefall-model/1", visual.path, visual.format));
    }
    let folder = store().ok_or("no assets store (set UNIVERSE_ASSETS)")?.join(&visual.path);
    let package = Package::open(folder.clone(), &visual.manifest_sha256)?;
    let glb = package.read("model.glb")?;
    // (The bounds' centre, from the manifest: `bounds_m` as [[min x, y, z], [max x, y, z]].)
    let manifest: serde_json::Value = serde_json::from_slice(&std::fs::read(folder.join("manifest.json")).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
    let corner = |k: usize| -> Option<[f64; 3]> {
        let c = manifest.get("bounds_m")?.get(k)?;
        Some([c.get(0)?.as_f64()?, c.get(1)?.as_f64()?, c.get(2)?.as_f64()?])
    };
    let (lo, hi) = corner(0).zip(corner(1)).unwrap_or(([0.0; 3], [0.0; 3]));
    let has_motion = manifest.get("motion").is_some_and(|v| !v.is_null());
    Ok(Model { glb, lo, hi, has_motion })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::worlds::sha256;

    /// A package made on the spot reads back checked; a changed model or manifest is refused.
    #[test]
    fn packages_are_checked() {
        let dir = std::env::temp_dir().join(format!("freefall-assets-test-{}", std::process::id()));
        let folder = dir.join("models/test-box/v1");
        std::fs::create_dir_all(&folder).unwrap();
        let glb = b"glTF fake bytes".to_vec();
        std::fs::write(folder.join("model.glb"), &glb).unwrap();
        let manifest = serde_json::json!({"format": "freefall-model/1", "key": "equipment.test.box", "bounds_m": [[-1.0, 0.0, -2.0], [1.0, 4.0, 2.0]],
            "files": {"model.glb": {"sha256": sha256(&glb), "size": glb.len()}}});
        let bytes = serde_json::to_vec(&manifest).unwrap();
        std::fs::write(folder.join("manifest.json"), &bytes).unwrap();
        let visual = |sha: String| Visual { format: "freefall-model/1".into(), path: "models/test-box/v1".into(), manifest_sha256: sha, version: 1, triangles: None, note: None };
        // SAFETY of the env change: this test alone reads UNIVERSE_ASSETS.
        unsafe { std::env::set_var("UNIVERSE_ASSETS", &dir) };
        let m = model(&visual(sha256(&bytes))).unwrap();
        assert_eq!(m.glb, glb);
        assert_eq!((m.lo, m.hi), ([-1.0, 0.0, -2.0], [1.0, 4.0, 2.0]));
        assert!(model(&visual("0".repeat(64))).is_err(), "a manifest not the record's");
        std::fs::write(folder.join("model.glb"), b"changed").unwrap();
        assert!(model(&visual(sha256(&bytes))).is_err(), "a model not the manifest's");
        let _ = std::fs::remove_dir_all(&dir);
    }
}

#[path = "assets_mounts.rs"]
pub mod mounts;
