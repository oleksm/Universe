//! The rule set in stone (docs/rearchitecture.md §0): the engine knows no
//! intentions and makes no client decisions. Its source (the world's tick,
//! traffic, combat, the market service, records, the input log, and the
//! contract) must not reach into clients: no client modules or types (the
//! pool, pilots, the cockpit, the operator, avionics), and no intentions
//! (roles, hunts, flight, orders). Test modules and comments aside.

const ENGINE: [&str; 9] = ["universe", "traffic", "combat", "commerce", "recorder", "vessel", "audit", "contract", "contacts"];

/// Paths (anywhere in a line), and names (whole words: types, fields,
/// functions) the engine mustn't use.
const PATHS: [(&str, &str); 5] = [
    (".trader", "a role"),
    ("crate::pilots", "the NPC pool's code"),
    ("crate::cockpit", "the player's cockpit"),
    ("crate::operator", "the NPC operator"),
    ("crate::setup", "the composition of world and clients"),
];
const NAMES: [(&str, &str); 8] = [
    ("Cockpit", "the player's cockpit"),
    ("Pool", "the NPC pool"),
    ("Pilot", "a pilot itself"),
    ("Avionics", "a pilot's avionics"),
    ("avionics", "a pilot's avionics"),
    ("pirate", "a role"),
    ("hunting", "a hunt"),
    ("flee", "fight or flight"),
];

/// The identifiers in `line` (`Party::Pilot`, the ledger's account holder, aside).
fn words(line: &str) -> Vec<String> {
    let line = line.replace("Party::Pilot", "Party::");
    line.split(|c: char| !(c.is_alphanumeric() || c == '_')).filter(|w| !w.is_empty()).map(str::to_string).collect()
}

#[test]
fn the_engine_knows_no_intentions() {
    let mut found = Vec::new();
    // (A module is a file, or a directory of them.)
    let files = ENGINE.iter().flat_map(|name| {
        let src = format!("{}/src", env!("CARGO_MANIFEST_DIR"));
        let dir = std::path::Path::new(&src).join(name);
        let mut files: Vec<(String, std::path::PathBuf)> = match std::fs::read_dir(&dir) {
            Ok(d) => d.flatten().map(|e| e.path()).filter(|p| p.extension().is_some_and(|x| x == "rs")).map(|p| (format!("{name}/{}", p.file_stem().unwrap().to_string_lossy()), p)).collect(),
            Err(_) => vec![(name.to_string(), dir.with_extension("rs"))],
        };
        files.sort();
        files
    });
    for (name, path) in files {
        let source = std::fs::read_to_string(&path).expect("engine source");
        let code = source.split("#[cfg(test)]").next().unwrap_or("");
        for (n, line) in code.lines().enumerate() {
            let line = line.split("//").next().unwrap_or("");
            for (pattern, what) in PATHS {
                if line.contains(pattern) {
                    found.push(format!("{name}.rs:{}: {what} (`{pattern}`): {}", n + 1, line.trim()));
                }
            }
            let words = words(line);
            for (name_, what) in NAMES {
                if words.iter().any(|w| w == name_) {
                    found.push(format!("{name}.rs:{}: {what} (`{name_}`): {}", n + 1, line.trim()));
                }
            }
        }
    }
    assert!(found.is_empty(), "the engine reaches into clients:\n{}", found.join("\n"));
}
