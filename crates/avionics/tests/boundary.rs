//! The pilot boundary (`docs/rearchitecture.md`, R3): avionics are pilots.
//! They see the world only through their bus (sensors, the feed, traffic
//! control's replies) and act only through it (actuation, requests). They
//! may use what's shared or published — the charts (star systems and their
//! geometry), hardware specs, the contracts, the physics kernel (to predict),
//! published contact rules — but never the world's internals.

/// World internals a pilot must not reach (paths as they'd be written).
const FORBIDDEN: &[&str] = &[
    "universe_world::World",
    "World::",
    "TrafficControl",
    "pads::",
    "traffic::request",
    "traffic::lapsed",
    "traffic::nearest_station",
    "universe_world::damage",
    "universe_world::market",
    "universe_world::collisions",
    "universe_world::turrets::turrets",
];

#[test]
fn avionics_reach_the_world_only_through_their_bus() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut found = Vec::new();
    for entry in std::fs::read_dir(&dir).expect("avionics sources") {
        let path = entry.expect("entry").path();
        if path.extension().is_none_or(|e| e != "rs") {
            continue;
        }
        let text = std::fs::read_to_string(&path).expect("readable");
        // Tests may build a world to check a pilot against.
        let code = text.split("#[cfg(test)]").next().unwrap_or("");
        for (n, line) in code.lines().enumerate() {
            if line.trim_start().starts_with("//") {
                continue;
            }
            for f in FORBIDDEN {
                if line.contains(f) {
                    found.push(format!("{}:{}: {f}", path.file_name().unwrap().to_string_lossy(), n + 1));
                }
            }
        }
    }
    assert!(found.is_empty(), "pilots reaching into the world:\n{}", found.join("\n"));
}
