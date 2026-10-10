//! The workspace's layers, held: the core (crates/core) depends on nothing outside it; the engine on the
//! profiler alone (the world only for its agreement test); nothing depends on the game. Read from each
//! crate's Cargo.toml, its `[dependencies]` only (tests may reach further).

use std::path::Path;

/// The workspace's own crates crate `dir` depends on (its `[dependencies]` section).
fn deps(dir: &Path) -> Vec<String> {
    let text = std::fs::read_to_string(dir.join("Cargo.toml")).unwrap();
    let mut out = Vec::new();
    let mut section = String::new();
    for line in text.lines().map(str::trim) {
        if line.starts_with('[') {
            section = line.to_string();
        } else if section == "[dependencies]"
            && let Some((name, rest)) = line.split_once('=')
            && rest.contains("path")
        {
            out.push(name.trim().to_string());
        }
    }
    out
}

#[test]
fn layers_hold() {
    let crates = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    let core: Vec<String> = std::fs::read_dir(crates.join("core")).unwrap().flatten().map(|e| format!("universe-{}", e.file_name().to_string_lossy())).collect();
    assert!(core.len() >= 8, "the core's crates: {core:?}");
    for e in std::fs::read_dir(crates.join("core")).unwrap().flatten() {
        for d in deps(&e.path()) {
            assert!(core.contains(&d), "core crate {:?} depends on {d}, outside the core", e.file_name());
        }
    }
    assert_eq!(deps(&crates.join("engine")), vec!["universe-prof".to_string()], "the engine depends on the profiler alone");
}
