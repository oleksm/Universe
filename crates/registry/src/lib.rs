//! Freefall Facts, the registry the world is made from (`standards/`), as
//! typed records. It is the only copy: the game holds no table of its own of
//! anything the registry describes.
//!
//! How it reaches the game: when the game is built, [`Registry::read`] reads
//! every record under `standards/` (each file's `identity.key` says its kind,
//! the first part of the key), parses the kinds the game uses into their types
//! (unknown fields refused), checks the references between them, and
//! [`Registry::encode`]s the result. The binary carries that encoding;
//! [`Registry::decode`] turns it back into records once at start. Nothing is
//! read from disk while the game runs, and nothing is looked up by key in play:
//! the game resolves keys to its own handles when it loads.
//!
//! Units are SI, as the records hold them (angles in degrees, marked so in the
//! schema). A record's figures each carry a [`Basis`]: where they come from,
//! and whether they're a guess to review.

use std::collections::{BTreeMap, HashMap};
use std::fmt;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

mod celestial;

pub use celestial::{ClassMix, Composition, Found, Galaxy, GalaxySeeding, Mining, NamedIdentity, RockClass, RockClassIdentity, RockPhysical, Seeding, System, SystemIdentity, SystemPosition};

/// Where a record's figures come from (the common schema's `basis`).
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Basis {
    /// The groups, or `group.property`, it covers.
    pub of: Vec<String>,
    pub tier: Tier,
    #[serde(default)]
    pub source: Option<String>,
    #[serde(default)]
    pub note: Option<String>,
    /// A guess put in so the figure is there, to be reviewed.
    #[serde(default)]
    pub review: bool,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Tier {
    Sourced,
    Derived,
    Invented,
}

/// Is any of `field`'s figures (`group` or `group.property`) a guess to review?
pub fn under_review(basis: &[Basis], field: &str) -> bool {
    basis.iter().any(|b| b.review && b.of.iter().any(|o| o == field || field.starts_with(&format!("{o}.")) || o.starts_with(&format!("{field}."))))
}

/// Whether the seed's record of a celestial thing is the truth (`curated`,
/// `frozen`) or only what the seed makes, written down (`seeded`).
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Provenance {
    Seeded,
    Curated,
    Frozen,
}

/// The registry, as far as the game reads it.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Registry {
    pub seeding: Seeding,
    pub rock_classes: Vec<RockClass>,
    pub systems: Vec<System>,
    /// How many records of each kind there are, the ones the game doesn't
    /// read yet included (by kind: `hull`, `part`, ...).
    pub counts: BTreeMap<String, usize>,
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
        let mut galaxy = None;
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
            let Some(key) = head.identity.and_then(|i| i.key) else { continue };
            if let Some(first) = keys.insert(key.clone(), file.clone()) {
                problems.push(Problem { file: file.clone(), what: format!("{key} is also {}", first.display()) });
            }
            let kind = key.split('.').next().unwrap_or_default().to_string();
            *reg.counts.entry(kind.clone()).or_default() += 1;
            let mut parse = |what: &mut dyn FnMut(&str) -> Result<(), serde_norway::Error>| {
                if let Err(e) = what(&text) {
                    problems.push(Problem { file: file.clone(), what: format!("{key}: {e}") });
                }
            };
            match kind.as_str() {
                "rock-class" => parse(&mut |t| Ok(reg.rock_classes.push(serde_norway::from_str(t)?))),
                "system" => parse(&mut |t| Ok(reg.systems.push(serde_norway::from_str(t)?))),
                "seeding" if key == "seeding.galaxy" => parse(&mut |t| Ok(galaxy = Some(serde_norway::from_str::<GalaxySeeding>(t)?))),
                _ => {}
            }
        }
        match galaxy {
            Some(g) => reg.seeding.galaxy = g,
            None => problems.push(Problem { file: root.to_path_buf(), what: "no seeding.galaxy".into() }),
        }
        // References the game follows: each must name a record of its kind.
        let has = |k: &str| keys.contains_key(k);
        if !has(&reg.seeding.galaxy.galaxy.home) || !reg.seeding.galaxy.galaxy.home.starts_with("system.") {
            problems.push(Problem { file: keys.get("seeding.galaxy").cloned().unwrap_or_default(), what: format!("home {} is no system", reg.seeding.galaxy.galaxy.home) });
        }
        for r in &reg.rock_classes {
            for y in [r.mining.as_ref().and_then(|m| m.yields.as_ref()), r.mining.as_ref().and_then(|m| m.rich_yields.as_ref())].into_iter().flatten() {
                if !has(y) || !y.starts_with("good.") {
                    problems.push(Problem { file: keys[&r.identity.key].clone(), what: format!("yields {y}, which is no good") });
                }
            }
        }
        if problems.is_empty() { Ok(reg) } else { Err(problems) }
    }

    /// The compact encoding the binary carries.
    pub fn encode(&self) -> Vec<u8> {
        postcard::to_stdvec(self).expect("the registry encodes")
    }

    /// The registry from [`Registry::encode`]'s bytes.
    pub fn decode(bytes: &[u8]) -> Registry {
        postcard::from_bytes(bytes).expect("the registry built into the game decodes")
    }

    /// The system with this key.
    pub fn system(&self, key: &str) -> Option<&System> {
        self.systems.iter().find(|s| s.identity.key == key)
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

    /// The registry in the repository reads without a problem, and comes back
    /// the same through the binary's encoding.
    #[test]
    fn the_registry_reads_and_round_trips() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../standards");
        let reg = Registry::read(&root).unwrap_or_else(|p| panic!("{}", p.iter().map(|p| p.to_string()).collect::<Vec<_>>().join("\n")));
        assert!(reg.rock_classes.len() >= 4 && !reg.systems.is_empty());
        let back = Registry::decode(&reg.encode());
        assert_eq!(back.seeding.galaxy.galaxy.seed, reg.seeding.galaxy.galaxy.seed);
        assert_eq!(back.rock_classes.len(), reg.rock_classes.len());
    }
}
