//! The developer settings scenarios read from the environment (`UNIVERSE_*`): one way to read
//! each kind, so every scenario parses alike.

/// A number (None: unset or not a number).
pub fn num(name: &str) -> Option<f64> {
    std::env::var(name).ok().and_then(|v| v.trim().parse().ok())
}

/// Set at all.
pub fn flag(name: &str) -> bool {
    std::env::var_os(name).is_some()
}

/// A comma-separated list (empty: unset; what doesn't parse is skipped).
pub fn list<T: std::str::FromStr>(name: &str) -> Vec<T> {
    std::env::var(name).ok().map(|v| v.split(',').filter_map(|n| n.trim().parse().ok()).collect()).unwrap_or_default()
}
