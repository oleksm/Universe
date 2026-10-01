//! The services' boundary (`docs/rearchitecture.md`): institutions read what
//! the core reports (a ship's state, its events), the charts and published
//! specs, and the contracts; they decide their own domain. They never reach
//! into the core's machinery.

const FORBIDDEN: &[&str] = &[
    "universe_world::World",
    "World::",
    "universe_world::damage",
    "universe_world::weapons",
    "universe_world::collisions",
    "rules::apply",
    "rules::release",
    "turrets_fire",
    "step_ship",
];

#[test]
fn services_stay_out_of_the_cores_machinery() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut found = Vec::new();
    for entry in std::fs::read_dir(&dir).expect("services sources") {
        let path = entry.expect("entry").path();
        if path.extension().is_none_or(|e| e != "rs") {
            continue;
        }
        let text = std::fs::read_to_string(&path).expect("readable");
        // Tests may build a world to check a service against.
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
    assert!(found.is_empty(), "services reaching into the core:\n{}", found.join("\n"));
}
