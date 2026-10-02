//! The physics sheet (`config/physics.ron`) into the code's constants: one
//! `pub const` each, with its unit, kind and reason as its doc, and the whole
//! sheet as a table (`SHEET`) for the report and the dogma's checks.

use serde::Deserialize;

#[derive(Deserialize)]
struct Section {
    name: String,
    note: String,
    entries: Vec<Entry>,
}

#[derive(Deserialize)]
struct Entry {
    name: String,
    value: f64,
    unit: String,
    kind: Kind,
    note: String,
}

#[derive(Deserialize, Debug)]
enum Kind {
    Real,
    Grounded,
    Simplified,
    Invented,
    Tuning,
    Planned,
}

fn main() {
    let path = "../../config/physics.ron";
    println!("cargo:rerun-if-changed={path}");
    let text = std::fs::read_to_string(path).expect("the physics sheet");
    let sections: Vec<Section> = ron::from_str(&text).unwrap_or_else(|e| panic!("the physics sheet: {e}"));
    let mut out = String::from("// Generated from config/physics.ron by build.rs: edit the sheet, not this.\n\n");
    let mut table = String::from("/// The whole sheet, in order: for the report and the dogma's checks.\npub const SHEET: &[Entry] = &[\n");
    let mut names = std::collections::HashSet::new();
    for s in &sections {
        out.push_str(&format!("// {}: {}\n", s.name, s.note));
        for e in &s.entries {
            assert!(names.insert(e.name.clone()), "the physics sheet names {} twice", e.name);
            assert!(e.value.is_finite(), "{} isn't a number", e.name);
            let unit = if e.unit.is_empty() { String::new() } else { format!(" ({})", e.unit) };
            out.push_str(&format!("/// {}{} [{:?}]\npub const {}: f64 = {:?};\n", e.note, unit, e.kind, e.name, e.value));
            table.push_str(&format!(
                "    Entry {{ section: {:?}, name: {:?}, value: {:?}, unit: {:?}, kind: Kind::{:?}, note: {:?} }},\n",
                s.name, e.name, e.value, e.unit, e.kind, e.note
            ));
        }
        out.push('\n');
    }
    table.push_str("];\n");
    let notes: Vec<String> = sections.iter().map(|s| format!("    ({:?}, {:?}),\n", s.name, s.note)).collect();
    out.push_str(&table);
    out.push_str(&format!("\n/// Each section's name and note.\npub const SECTIONS: &[(&str, &str)] = &[\n{}];\n", notes.concat()));
    let dest = std::path::Path::new(&std::env::var("OUT_DIR").unwrap()).join("sheet.rs");
    std::fs::write(dest, out).unwrap();
}
