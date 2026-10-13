//! Models for the things the registry describes (docs/asset-contract.md): a record of a kind that takes
//! a model (equipment, industrial modules, buildings, structures, gate rings, hulls) may carry `visual`,
//! a version folder in the assets store (`UNIVERSE_ASSETS`, else `~/git/freefall-assets`) holding
//! `model.glb` and `manifest.json` (`freefall-model/1`). The manifest is checked against the record's
//! hash and the model against the manifest, as a world's ground is; anything that fails is said once and
//! the thing is drawn as its box.

use std::path::PathBuf;

use crate::registry::{registry, Visual};
pub use crate::worlds::Limits;
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
/// `model.glb` against the manifest) within `Limits::visual()`.
pub fn model(visual: &Visual) -> Result<Model, String> {
    model_with(visual, Limits::visual())
}

/// As `model`, within `limits`: the manifest, and every file it lists, are refused by size before any
/// is allocated or read.
pub fn model_with(visual: &Visual, limits: Limits) -> Result<Model, String> {
    let _profile=universe_prof::scope("assets/read and verify");
    if visual.format != "freefall-model/1" {
        return Err(format!("{}: format {:?}, this reads freefall-model/1", visual.path, visual.format));
    }
    load(store().ok_or("no assets store (set UNIVERSE_ASSETS)")?.join(&visual.path), visual, limits)
}

fn load(folder: PathBuf, visual: &Visual, limits: Limits) -> Result<Model, String> {
    let package = Package::open_limited(folder, &visual.manifest_sha256, limits)?;
    package.preflight()?;
    let glb = package.read("model.glb")?;
    // (The bounds' centre, from the manifest: `bounds_m` as [[min x, y, z], [max x, y, z]].)
    let manifest: serde_json::Value = serde_json::from_slice(package.manifest()).map_err(|e| e.to_string())?;
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

    /// A package in a fresh folder: its files, and a manifest listing them with `sizes` (None: the
    /// true sizes). Returns the folder and the manifest's hash.
    fn package(name: &str, files: &[(&str, &[u8])], sizes: Option<&[u64]>) -> (PathBuf, String) {
        let folder = std::env::temp_dir().join(format!("freefall-assets-bounds-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&folder);
        std::fs::create_dir_all(&folder).unwrap();
        let mut listed = serde_json::Map::new();
        for (i, (n, bytes)) in files.iter().enumerate() {
            std::fs::write(folder.join(n), bytes).unwrap();
            let size = sizes.map_or(bytes.len() as u64, |s| s[i]);
            listed.insert(n.to_string(), serde_json::json!({"sha256": sha256(bytes), "size": size}));
        }
        let bytes = serde_json::to_vec(&serde_json::json!({"format": "freefall-model/1", "files": listed})).unwrap();
        std::fs::write(folder.join("manifest.json"), &bytes).unwrap();
        (folder, sha256(&bytes))
    }

    fn try_load(folder: &std::path::Path, sha: &str, limits: Limits) -> Result<Model, String> {
        let visual = Visual { format: "freefall-model/1".into(), path: String::new(), manifest_sha256: sha.into(), version: 1, triangles: None, note: None };
        let r = load(folder.to_path_buf(), &visual, limits);
        let _ = std::fs::remove_dir_all(folder);
        r
    }

    /// Oversized manifests, files and packages, wrong declared sizes, missing files and paths out of
    /// the package are all refused by size or name; a package within its limits reads.
    #[test]
    fn packages_are_bounded() {
        let big = Limits { manifest: 4096, file: 64, package: 100 };
        let (f, sha) = package("ok", &[("model.glb", &[1; 60]), ("thumb.png", &[2; 30])], None);
        assert_eq!(try_load(&f, &sha, big).unwrap().glb.len(), 60);
        let (f, sha) = package("manifest", &[("model.glb", &[1; 10])], None);
        assert!(try_load(&f, &sha, Limits { manifest: 16, ..big }).err().unwrap().contains("manifest.json"), "manifest over its limit");
        let (f, sha) = package("file", &[("model.glb", &[1; 65])], None);
        assert!(try_load(&f, &sha, big).err().unwrap().contains("over the limit of 64"), "model over the file limit");
        let (f, sha) = package("dependency", &[("model.glb", &[1; 10]), ("thumb.png", &[2; 65])], None);
        assert!(try_load(&f, &sha, big).err().unwrap().contains("thumb.png"), "a dependency over the file limit");
        let (f, sha) = package("total", &[("model.glb", &[1; 60]), ("thumb.png", &[2; 60])], None);
        assert!(try_load(&f, &sha, big).err().unwrap().contains("lists 120 bytes"), "the package over its limit");
        let (f, sha) = package("declared", &[("model.glb", &[1; 60])], Some(&[10]));
        assert!(try_load(&f, &sha, big).err().unwrap().contains("its manifest says 10"), "a file not its declared size");
        let (f, sha) = package("missing", &[("model.glb", &[1; 10]), ("thumb.png", &[2; 10])], None);
        std::fs::remove_file(f.join("thumb.png")).unwrap();
        assert!(try_load(&f, &sha, big).err().unwrap().contains("thumb.png"), "a listed file missing");
        let (f, sha) = package("escape", &[("model.glb", &[1; 10]), ("../outside.glb", &[2; 10])], None);
        let _ = std::fs::remove_file(f.join("../outside.glb"));
        assert!(try_load(&f, &sha, big).err().unwrap().contains("not a file inside"), "a path out of the package");
        let (f, sha) = package("hash", &[("model.glb", &[1; 10])], None);
        std::fs::write(f.join("model.glb"), [9; 10]).unwrap();
        assert!(try_load(&f, &sha, big).err().unwrap().contains("not the file its manifest names"), "same size, other bytes");
    }
}

#[path = "assets_mounts.rs"]
pub mod mounts;
