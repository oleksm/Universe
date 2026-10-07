//! Freefall Facts, the registry the world is made from (`standards/`), as
//! typed records. It is the only copy: the game holds no table of its own of
//! anything the registry describes.
//!
//! **The types are generated from the registry's schemas** (`build.rs`, into
//! [`generated`]): a record's struct is its schema, field for field, unknown
//! fields refused. A schema changed is a rebuild; nothing here is kept by hand.
//!
//! How it reaches the game: when the game is built, [`Registry::read`] reads
//! every record under `standards/` (each file's `identity.key` says its kind,
//! the first part of the key), parses it into its kind's type, checks every
//! reference (each property marked `x-ref` must name a record of a kind it
//! allows), and [`Registry::encode`]s the result. The binary carries that
//! encoding; [`Registry::decode`] turns it back into records once at start.
//! Nothing is read from disk while the game runs.
//!
//! Units are SI, as the records hold them; an angle is [`Degrees`] (the one
//! exception), with `.rad()` for the engine.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::fmt;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

pub mod generated;

pub use generated::*;

/// The registry: every record, by kind ([`Records`], generated), and every
/// record's name by key.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Registry {
    pub records: Records,
    /// Every record's name, by key.
    pub names: BTreeMap<String, String>,
    /// How many records of each kind there are (by kind: `hull`, `part`, ...).
    pub counts: BTreeMap<String, usize>,
    /// The folder each part lies in (`parts/<folder>/`), by its key: the
    /// product it is a part of names that folder.
    #[serde(default)]
    pub folders: BTreeMap<String, String>,
    /// Every key the registry has renamed, old to new (`renames.yaml`): for
    /// anything that saved a key by itself.
    #[serde(default)]
    pub renames: BTreeMap<String, String>,
    /// Each kind's records by key (built when read or decoded: see `index`); the lookups
    /// (`reg.hull(key)`, one a kind, generated) go through it.
    #[serde(skip)]
    pub by_key: KeyIndex,
}

impl std::ops::Deref for Registry {
    type Target = Records;
    fn deref(&self) -> &Records {
        &self.records
    }
}

/// Something wrong with the records: the game isn't built from them until
/// it's put right.
#[derive(Clone, Debug)]
pub struct Problem {
    pub file: PathBuf,
    pub what: String,
}

impl fmt::Display for Problem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.file.display(), self.what)
    }
}

/// Just enough of any record to know what it is.
#[derive(Deserialize)]
struct Head {
    #[serde(default)]
    identity: Option<HeadIdentity>,
}

#[derive(Deserialize)]
struct HeadIdentity {
    #[serde(default)]
    key: Option<String>,
    #[serde(default)]
    name: Option<String>,
}

impl Registry {
    /// Reads every record under `root` (the `standards/` folder). All the
    /// problems found, if any.
    pub fn read(root: &Path) -> Result<Registry, Vec<Problem>> {
        let mut files = Vec::new();
        yaml_files(root, &mut files);
        files.sort();
        let mut problems = Vec::new();
        let mut reg = Registry::default();
        let mut keys: HashMap<String, PathBuf> = HashMap::new();
        for file in &files {
            let text = match std::fs::read_to_string(file) {
                Ok(t) => t,
                Err(e) => {
                    problems.push(Problem { file: file.clone(), what: e.to_string() });
                    continue;
                }
            };
            // Schemas and the registry's own files have no key: they aren't records.
            let Ok(head) = serde_norway::from_str::<Head>(&text) else { continue };
            let Some(identity) = head.identity else { continue };
            let Some(key) = identity.key else { continue };
            if let Some(first) = keys.insert(key.clone(), file.clone()) {
                problems.push(Problem { file: file.clone(), what: format!("{key} is also {}", first.display()) });
            }
            if let Some(name) = identity.name {
                reg.names.insert(key.clone(), name);
            }
            let kind = key.split('.').next().unwrap_or_default().to_string();
            if kind == "part"
                && let Some(folder) = file.parent().and_then(|d| d.file_name()).and_then(|d| d.to_str())
            {
                reg.folders.insert(key.clone(), folder.to_string());
            }
            *reg.counts.entry(kind.clone()).or_default() += 1;
            match reg.records.parse(&kind, &text) {
                Ok(true) => {}
                Ok(false) => problems.push(Problem { file: file.clone(), what: format!("{key}: no schema has the kind {kind}") }),
                Err(e) => problems.push(Problem { file: file.clone(), what: format!("{key}: {e}") }),
            }
        }
        // Parts as a rule (SFO 12): an equipment record's `built_of.list` derives its part records through the
        // same text the build writes (tools/standards/lib.py, part_yaml), parsed with the generated Part type,
        // so the game sees what it saw when each part was a file.
        // (The modules that cut parts from stock say so in their records: `cuts: true`.)
        let cuts: HashSet<String> = reg.records.modules.iter().filter(|m| m.cuts == Some(true)).map(|m| m.identity.key.clone()).collect();
        let derived: Vec<(PathBuf, String, String, String, String)> = reg
            .records
            .equipment
            .iter()
            .flat_map(|e| {
                let folder = e.built_of.parts.clone().unwrap_or_else(|| e.identity.key.trim_start_matches("equipment.").replace('.', "-"));
                let file = keys.get(&e.identity.key).cloned().unwrap_or_default();
                let cuts = &cuts;
                e.built_of.list.iter().map(move |it| (file.clone(), folder.clone(), it.code.clone(), it.name.clone(), derived_part_yaml(e, it, cuts))).collect::<Vec<_>>()
            })
            .collect();
        for (file, folder, code, name, text) in derived {
            let key = format!("part.{}", code.to_lowercase());
            if let Some(first) = keys.insert(key.clone(), file.clone()) {
                problems.push(Problem { file: file.clone(), what: format!("{key} (derived) is also {}", first.display()) });
                continue;
            }
            reg.names.insert(key.clone(), name);
            reg.folders.insert(key.clone(), folder);
            *reg.counts.entry("part".to_string()).or_default() += 1;
            match reg.records.parse("part", &text) {
                Ok(true) => {}
                Ok(false) => problems.push(Problem { file: file.clone(), what: format!("{key}: no schema has the kind part") }),
                Err(e) => problems.push(Problem { file: file.clone(), what: format!("{key} (derived): {e}") }),
            }
        }
        // (Every record read, the derived parts with them: the lookups by key from here on.)
        reg.by_key = reg.records.key_index();
        // Every reference names a record of a kind it may.
        reg.records.refs(&mut |from, to, kinds| {
            let kind = to.split('.').next().unwrap_or_default();
            if !keys.contains_key(to) || !kinds.contains(&kind) {
                problems.push(Problem { file: keys.get(from).cloned().unwrap_or_default(), what: format!("{from} names {to}, which is no {}", kinds.join(" or ")) });
            }
        });
        if reg.galaxy().is_none() {
            problems.push(Problem { file: root.to_path_buf(), what: "no seeding.galaxy".into() });
        }
        // (The renames: `renames.yaml` at the root, `was` to `is`.)
        #[derive(Deserialize)]
        struct Rename {
            was: String,
            is: String,
        }
        #[derive(Deserialize)]
        struct Renames {
            renames: Vec<Rename>,
        }
        let file = root.join("renames.yaml");
        if let Ok(text) = std::fs::read_to_string(&file) {
            match serde_norway::from_str::<Renames>(&text) {
                Ok(r) => reg.renames.extend(r.renames.into_iter().map(|r| (r.was, r.is))),
                Err(e) => problems.push(Problem { file, what: e.to_string() }),
            }
        }
        if problems.is_empty() { Ok(reg) } else { Err(problems) }
    }

    /// The compact encoding the binary carries.
    pub fn encode(&self) -> Vec<u8> {
        rmp_serde::to_vec_named(self).expect("the registry encodes")
    }

    /// The registry from [`Registry::encode`]'s bytes.
    pub fn decode(bytes: &[u8]) -> Registry {
        let mut reg: Registry = rmp_serde::from_slice(bytes).expect("the registry built into the game decodes");
        reg.by_key = reg.records.key_index();
        reg
    }

    /// A record's name, by its key.
    pub fn name(&self, key: &str) -> Option<&str> {
        self.names.get(key).map(String::as_str)
    }

    /// The world as a whole: `seeding.galaxy`'s settings.
    pub fn galaxy(&self) -> Option<&SeedingGalaxy> {
        self.seeding("seeding.galaxy")?.galaxy.as_ref()
    }

    /// What `product` (a piece of equipment, a hull, a part made of parts) is
    /// built of: its parts, each with how many it takes. Equipment and hulls
    /// name their folder of parts (`built_of.parts`; a hull without, the
    /// folder of its own name); a part made of parts has its own, named by
    /// its code (`parts/mc-07/MC07-23/`).
    pub fn built_of(&self, product: &str) -> Vec<(&Part, u32)> {
        let folder = match product.split_once('.') {
            Some(("equipment", _)) => self.equipment(product).and_then(|e| e.built_of.parts.clone()),
            Some(("hull", name)) => Some(self.hull(product).and_then(|h| h.built_of.parts.clone()).unwrap_or_else(|| name.to_string())),
            Some(("part", _)) => self.part(product).map(|p| p.identity.code.clone()),
            _ => None,
        };
        let Some(folder) = folder else { return Vec::new() };
        self.parts.iter().filter(|p| self.folders.get(&p.identity.key) == Some(&folder)).map(|p| (p, p.fit.count.unwrap_or(1).max(1) as u32)).collect()
    }


    /// What `item` (a good, a stock item or a material) is traded as: a
    /// market category, by key (`market.fuel`). A material is traded as the
    /// stock made from it is: what burns or holds a material takes any stock
    /// of it.
    pub fn traded_as(&self, item: &str) -> Option<String> {
        match item.split('.').next() {
            // (A good is sold as its stock: the stock made from it.)
            Some("good") => self.stock.iter().find(|s| s.made_from.iter().any(|m| m.item == item))?.identity.traded_as.clone(),
            Some("stock") => self.stock(item)?.identity.traded_as.clone(),
            Some("material") => self.stock.iter().find(|s| s.made_from.iter().any(|m| m.item == item))?.identity.traded_as.clone(),
            _ => None,
        }
    }
}

/// A YAML double-quoted scalar.
fn yq(s: &str) -> String {
    format!("\"{}\"", s.replace('"', "\\\""))
}

fn basis_flow(b: &Basis) -> String {
    let mut parts = vec![format!("of: [{}]", b.of.join(", "))];
    if let Some(r) = &b.rule {
        parts.push(format!("rule: {r}"));
    }
    if let Some(t) = &b.tier {
        parts.push(format!("tier: {}", match t { Tier::Sourced => "sourced", Tier::Derived => "derived", Tier::Invented => "invented" }));
    }
    if b.review == Some(true) && b.rule.is_none() {
        parts.push("review: true".to_string());
    }
    if let Some(src) = &b.source {
        parts.push(format!("source: {}", yq(src)));
    }
    if let Some(n) = &b.note {
        parts.push(format!("note: {}", yq(n)));
    }
    format!("{{ {} }}", parts.join(", "))
}

/// The basis a derived part gets where its list item says none (lib.py, part_basis): its shares as the
/// equipment's `built_of` says, its stock cut with the loss where a cutting module makes it, else built in
/// whole; the box by rule. An item's own entry replaces the default of the same `of`.
fn derived_basis(e: &Equipment, it: &EquipmentBuiltOfListItem, cuts: &HashSet<String>) -> Vec<Basis> {
    let mass = e.physical.mass.unwrap_or(0.0);
    let share = if mass > 0.0 { it.mass / mass } else { 0.0 };
    let eq_rule = e.built_of.shares.clone().or_else(|| e.basis.iter().find(|b| b.of.iter().any(|o| o == "built_of")).and_then(|b| b.rule.clone()));
    let entry = |of: &[&str], rule: Option<&str>, tier: Option<Tier>, note: Option<String>| Basis { of: of.iter().map(|s| s.to_string()).collect(), tier, rule: rule.map(str::to_string), source: None, note, review: if rule.is_none() { Some(true) } else { None } };
    let first = match eq_rule.as_deref() {
        Some(r) if r.starts_with("rule.") && r != "rule.first-design-parts" => entry(&["physical.mass", "fit"], Some(r), None, None),
        Some("rule.first-design-parts") => entry(&["physical.mass", "fit"], None, Some(Tier::Invented), Some(format!("Its share of the {} ({:.0}%), chosen.", e.identity.name, share * 100.0))),
        _ => entry(&["physical.mass", "fit"], Some("rule.shares-chosen"), None, None),
    };
    let mut out = vec![first];
    if let Some(item) = &it.item {
        let cut = it.module.as_deref().is_some_and(|m| cuts.contains(m)) && item.starts_with("stock.") && e.built_of.whole != Some(true);
        out.push(entry(&["made_from", "making"], Some(if cut { "rule.cut-loss" } else { "rule.built-in-whole" }), None, None));
    }
    out.push(entry(&["physical.length", "physical.width", "physical.height"], Some("rule.fitted-within"), None, None));
    let mut own: Vec<Basis> = it.basis.clone();
    for d in out.iter_mut() {
        if let Some(i) = own.iter().position(|b| b.of == d.of) {
            *d = own.remove(i);
        }
    }
    out.extend(own);
    out
}

/// The YAML text of one derived part (lib.py, part_yaml): the build writes the same.
fn derived_part_yaml(e: &Equipment, it: &EquipmentBuiltOfListItem, cuts: &HashSet<String>) -> String {
    let mut lines = vec![
        "identity:".to_string(),
        format!("  key: part.{}", it.code.to_lowercase()),
        format!("  code: {}", it.code),
        format!("  name: {}", yq(&it.name)),
        "  revision: draft".to_string(),
        format!("  description: {}", yq(it.description.as_deref().unwrap_or(""))),
        "physical:".to_string(),
        format!("  mass: {}", it.mass),
        format!("  length: {}", it.length),
        format!("  width: {}", it.width),
        format!("  height: {}", it.height),
    ];
    if let Some(item) = &it.item {
        lines.push("made_from:".to_string());
        lines.push(format!("  - item: {item}"));
        if let Some(q) = it.quantity {
            lines.push(format!("    quantity: {q}"));
        }
    }
    if let Some(m) = &it.module {
        lines.push("making:".to_string());
        lines.push(format!("  module: {m}"));
    }
    lines.push("fit:".to_string());
    lines.push(format!("  count: {}", it.count.unwrap_or(1)));
    lines.push("basis:".to_string());
    for b in derived_basis(e, it, cuts) {
        lines.push(format!("  - {}", basis_flow(&b)));
    }
    lines.join("\n") + "\n"
}

/// Every `.yaml` under `dir`, depth first.
fn yaml_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for e in entries.flatten() {
        let p = e.path();
        if p.is_dir() {
            yaml_files(&p, out);
        } else if p.extension().is_some_and(|x| x == "yaml") {
            out.push(p);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The registry in the repository reads without a problem (every record
    /// into its generated type, every reference to a record of a kind it may
    /// name), and comes back the same through the binary's encoding.
    #[test]
    fn the_registry_reads_and_round_trips() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../standards");
        let reg = Registry::read(&root).unwrap_or_else(|p| panic!("{}", p.iter().map(|p| p.to_string()).collect::<Vec<_>>().join("\n")));
        let back = Registry::decode(&reg.encode());
        assert_eq!(back.galaxy().map(|g| g.seed), reg.galaxy().map(|g| g.seed));
        assert_eq!((back.equipment.len(), back.bodies.len(), back.hulls.len()), (reg.equipment.len(), reg.bodies.len(), reg.hulls.len()));
        assert_eq!(back.hulls, reg.hulls);
        // (Parts as a rule, SFO 12: the equipment records' lists derive their parts; with the filed ones, the registry holds them all.)
        let derived: usize = reg.equipment.iter().map(|e| e.built_of.list.len()).sum();
        assert!(derived > 600 && reg.parts.len() >= derived + 100, "{} parts, {derived} of them derived", reg.parts.len());
        assert_eq!(back.parts.len(), reg.parts.len());
    }
}
