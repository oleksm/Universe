//! The developer settings scenarios read from the environment (`UNIVERSE_*`): one way to read
//! each kind, so every scenario parses alike. The environment is read once, at the first ask, and
//! kept: the game reads its settings from here (`var`, `var_os`, as `std::env`'s), never the process's
//! environment in a frame (that takes a lock and a copy every time).

use std::collections::HashMap;
use std::ffi::{OsStr, OsString};

fn snapshot() -> &'static HashMap<OsString, OsString> {
    static S: std::sync::OnceLock<HashMap<OsString, OsString>> = std::sync::OnceLock::new();
    S.get_or_init(|| std::env::vars_os().collect())
}

/// As `std::env::var`, from the environment as it was at the start.
pub fn var(name: &str) -> Result<String, std::env::VarError> {
    match snapshot().get(OsStr::new(name)) {
        Some(v) => v.clone().into_string().map_err(std::env::VarError::NotUnicode),
        None => Err(std::env::VarError::NotPresent),
    }
}

/// As `std::env::var_os`, from the environment as it was at the start.
pub fn var_os(name: &str) -> Option<OsString> {
    snapshot().get(OsStr::new(name)).cloned()
}

/// `UNIVERSE_OBSERVING`: the observer's port and profiler on (any value but empty or 0).
pub fn observing() -> bool {
    var("UNIVERSE_OBSERVING").is_ok_and(|v| !v.is_empty() && v != "0")
}

/// A number (None: unset or not a number).
pub fn num(name: &str) -> Option<f64> {
    var(name).ok().and_then(|v| v.trim().parse().ok())
}

/// Set at all.
pub fn flag(name: &str) -> bool {
    var_os(name).is_some()
}

#[cfg(feature = "dev")]
/// A comma-separated list (empty: unset; what doesn't parse is skipped).
pub fn list<T: std::str::FromStr>(name: &str) -> Vec<T> {
    var(name).ok().map(|v| v.split(',').filter_map(|n| n.trim().parse().ok()).collect()).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    /// The game reads its settings here only: a frame never asks the process's environment.
    #[test]
    fn settings_are_read_through_devenv_only() {
        fn walk(dir: &std::path::Path, out: &mut Vec<String>) {
            for e in std::fs::read_dir(dir).unwrap().flatten() {
                let p = e.path();
                if p.is_dir() {
                    walk(&p, out);
                } else if p.extension().is_some_and(|x| x == "rs") && !p.ends_with("devenv.rs") && !p.components().any(|c| c.as_os_str() == "bin") {
                    let text = std::fs::read_to_string(&p).unwrap();
                    if text.contains(concat!("std::env::", "var")) {
                        out.push(p.display().to_string());
                    }
                }
            }
        }
        let mut found = Vec::new();
        walk(&std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src"), &mut found);
        assert!(found.is_empty(), "read the environment through crate::devenv: {found:?}");
    }
}
