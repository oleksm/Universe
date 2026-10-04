// Dogma's laws, read from the registry (`standards/Dogma/metadata`: a section
// record `<section>.yaml` and its laws in the folder `<section>/`), into Rust:
// one `pub const` per law under its label, in SI as the record holds it,
// documented, and the whole as a table. The registry is the only copy; the
// build reads it each time it changes.

use serde::Deserialize;
use std::path::Path;

#[derive(Deserialize)]
struct Section {
    identity: SectionIdentity,
}

#[derive(Deserialize)]
struct SectionIdentity {
    key: String,
    name: String,
    #[serde(default)]
    order: Option<u32>,
    about: String,
}

#[derive(Deserialize)]
struct Law {
    identity: LawIdentity,
    value: f64,
    #[serde(default)]
    unit: String,
    kind: LawKind,
    #[serde(default)]
    note: String,
}

#[derive(Deserialize)]
struct LawIdentity {
    key: String,
    name: String,
    label: String,
    section: String,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "lowercase")]
enum LawKind {
    Real,
    Simplified,
    Invented,
}

fn read<T: for<'de> Deserialize<'de>>(path: &Path) -> T {
    let text = std::fs::read_to_string(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    serde_norway::from_str(&text).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

fn yamls(dir: &Path) -> Vec<std::path::PathBuf> {
    let mut files: Vec<_> = std::fs::read_dir(dir)
        .unwrap_or_else(|e| panic!("{}: {e}", dir.display()))
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|x| x == "yaml"))
        .collect();
    files.sort();
    files
}

/// Generates `out` (in OUT_DIR) from the Dogma registry at `dir`; `types` is
/// the path the generated code names `Entry` and `Kind` by.
pub fn generate_dogma(dir: &str, out: &str, types: &str) {
    println!("cargo:rerun-if-changed={dir}");
    let root = Path::new(dir);
    let mut sections: Vec<(Section, Vec<Law>)> = yamls(root)
        .into_iter()
        .map(|file| {
            println!("cargo:rerun-if-changed={}", file.display());
            let section: Section = read(&file);
            let folder = root.join(file.file_stem().unwrap());
            println!("cargo:rerun-if-changed={}", folder.display());
            let laws: Vec<Law> = if folder.is_dir() { yamls(&folder).iter().map(|f| { println!("cargo:rerun-if-changed={}", f.display()); read(f) }).collect() } else { Vec::new() };
            for l in &laws {
                assert_eq!(l.identity.section, section.identity.key, "{} is filed under {} but names section {}", l.identity.key, section.identity.key, l.identity.section);
            }
            (section, laws)
        })
        .collect();
    sections.sort_by_key(|(s, _)| (s.identity.order.unwrap_or(99), s.identity.key.clone()));

    let mut code = format!("// Generated from the Dogma registry ({dir}) by build.rs: edit the records, not this.\n\nuse {types}::{{Entry, Kind}};\n\n");
    let mut table = String::from("/// Every law, in order.\npub const SHEET: &[Entry] = &[\n");
    let mut labels = std::collections::HashSet::new();
    for (s, laws) in &sections {
        code.push_str(&format!("// {}: {}\n", s.identity.name, s.identity.about));
        for l in laws {
            let label = &l.identity.label;
            assert!(labels.insert(label.clone()), "Dogma labels {label} twice");
            assert!(l.value.is_finite(), "{}: {label} isn't a number", l.identity.key);
            let note = if l.note.is_empty() { l.identity.name.clone() } else { format!("{}: {}", l.identity.name, l.note) };
            let unit = if l.unit.is_empty() { String::new() } else { format!(" ({})", l.unit) };
            code.push_str(&format!("/// {note}{unit} [{:?}; `{}`]\npub const {label}: f64 = {:?};\n", l.kind, l.identity.key, l.value));
            table.push_str(&format!(
                "    Entry {{ section: {:?}, name: {:?}, value: {:?}, unit: {:?}, kind: Kind::{:?}, note: {:?} }},\n",
                s.identity.name, label, l.value, l.unit, l.kind, note
            ));
        }
        code.push('\n');
    }
    table.push_str("];\n");
    code.push_str(&table);
    let notes: String = sections.iter().map(|(s, _)| format!("    ({:?}, {:?}),\n", s.identity.name, s.identity.about)).collect();
    code.push_str(&format!("\n/// Each section's name and what its laws are.\npub const SECTIONS: &[(&str, &str)] = &[\n{notes}];\n"));
    let dest = Path::new(&std::env::var("OUT_DIR").unwrap()).join(out);
    std::fs::write(dest, code).unwrap();
}
