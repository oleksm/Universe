// Shared by the kernel's and the world's build scripts: a sheet (RON: sections
// of named constants with their units, kinds and reasons) into Rust: one
// `pub const` each, documented, and the whole as a table.

use serde::Deserialize;

#[derive(Deserialize)]
struct Section {
    name: String,
    note: String,
    entries: Vec<SheetEntry>,
}

#[derive(Deserialize)]
struct SheetEntry {
    name: String,
    value: f64,
    unit: String,
    kind: SheetKind,
    note: String,
}

#[derive(Deserialize, Debug)]
enum SheetKind {
    Real,
    Grounded,
    Simplified,
    Invented,
    Tuning,
    Planned,
}

/// Generates `out` (in OUT_DIR) from the sheet at `path`; `types` is the path
/// the generated code names `Entry` and `Kind` by.
pub fn generate(path: &str, out: &str, types: &str) {
    println!("cargo:rerun-if-changed={path}");
    let text = std::fs::read_to_string(path).unwrap_or_else(|e| panic!("{path}: {e}"));
    let sections: Vec<Section> = ron::from_str(&text).unwrap_or_else(|e| panic!("{path}: {e}"));
    let mut code = format!("// Generated from {path} by build.rs: edit the sheet, not this.\n\nuse {types}::{{Entry, Kind}};\n\n");
    let mut table = String::from("/// The whole sheet, in order.\npub const SHEET: &[Entry] = &[\n");
    let mut names = std::collections::HashSet::new();
    for s in &sections {
        code.push_str(&format!("// {}: {}\n", s.name, s.note));
        for e in &s.entries {
            assert!(names.insert(e.name.clone()), "{path} names {} twice", e.name);
            assert!(e.value.is_finite(), "{path}: {} isn't a number", e.name);
            assert!(!e.note.is_empty(), "{path}: {} has no reason", e.name);
            let unit = if e.unit.is_empty() { String::new() } else { format!(" ({})", e.unit) };
            code.push_str(&format!("/// {}{} [{:?}]\npub const {}: f64 = {:?};\n", e.note, unit, e.kind, e.name, e.value));
            table.push_str(&format!(
                "    Entry {{ section: {:?}, name: {:?}, value: {:?}, unit: {:?}, kind: Kind::{:?}, note: {:?} }},\n",
                s.name, e.name, e.value, e.unit, e.kind, e.note
            ));
        }
        code.push('\n');
    }
    table.push_str("];\n");
    code.push_str(&table);
    let notes: String = sections.iter().map(|s| format!("    ({:?}, {:?}),\n", s.name, s.note)).collect();
    code.push_str(&format!("\n/// Each section's name and note.\npub const SECTIONS: &[(&str, &str)] = &[\n{notes}];\n"));
    let dest = std::path::Path::new(&std::env::var("OUT_DIR").unwrap()).join(out);
    std::fs::write(dest, code).unwrap();
}
