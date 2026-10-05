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

use std::collections::{BTreeMap, HashMap};
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
        if problems.is_empty() { Ok(reg) } else { Err(problems) }
    }

    /// The compact encoding the binary carries.
    pub fn encode(&self) -> Vec<u8> {
        rmp_serde::to_vec_named(self).expect("the registry encodes")
    }

    /// The registry from [`Registry::encode`]'s bytes.
    pub fn decode(bytes: &[u8]) -> Registry {
        rmp_serde::from_slice(bytes).expect("the registry built into the game decodes")
    }

    /// A record's name, by its key.
    pub fn name(&self, key: &str) -> Option<&str> {
        self.names.get(key).map(String::as_str)
    }

    /// The world as a whole: `seeding.galaxy`'s settings.
    pub fn galaxy(&self) -> Option<&SeedingGalaxy> {
        self.seeding.iter().find(|s| s.identity.key == "seeding.galaxy")?.galaxy.as_ref()
    }

    /// The good with this key.
    pub fn good(&self, key: &str) -> Option<&Good> {
        self.goods.iter().find(|g| g.identity.key == key)
    }

    /// What `product` (a piece of equipment, a hull) is built of: its parts,
    /// each with how many it takes. Equipment names its folder of parts
    /// (`built_of.parts`); a hull's is the folder of its own name, until hulls
    /// name theirs.
    pub fn built_of(&self, product: &str) -> Vec<(&Part, u32)> {
        let folder = match product.split_once('.') {
            Some(("equipment", _)) => self.equipment.iter().find(|e| e.identity.key == product).and_then(|e| e.built_of.parts.clone()),
            Some(("hull", name)) => Some(name.to_string()),
            _ => None,
        };
        let Some(folder) = folder else { return Vec::new() };
        self.parts.iter().filter(|p| self.folders.get(&p.identity.key) == Some(&folder)).map(|p| (p, p.fit.count.unwrap_or(1).max(1) as u32)).collect()
    }

    /// The module with this key.
    pub fn module(&self, key: &str) -> Option<&Module> {
        self.modules.iter().find(|m| m.identity.key == key)
    }

    /// The system with this key.
    pub fn system(&self, key: &str) -> Option<&System> {
        self.systems.iter().find(|s| s.identity.key == key)
    }

    /// What `item` (a good, a stock item or a material) is traded as: a
    /// market category, by key (`market.fuel`). A material is traded as the
    /// stock made from it is: what burns or holds a material takes any stock
    /// of it.
    pub fn traded_as(&self, item: &str) -> Option<String> {
        match item.split('.').next() {
            Some("good") => self.good(item)?.identity.traded_as.clone(),
            Some("stock") => self.stock.iter().find(|s| s.identity.key == item)?.identity.traded_as.clone(),
            Some("material") => self.stock.iter().find(|s| s.made_from.iter().any(|m| m.item == item))?.identity.traded_as.clone(),
            _ => None,
        }
    }
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
    }
}
