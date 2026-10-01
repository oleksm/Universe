//! Content: what the world is made of — hulls now; recipes, kinds of
//! place, kinds of goods, modules, brands and shapes as they move in — as
//! data, not code (see `docs/content.md`).
//!
//! Content comes in **packs**: folders of RON files. The base pack
//! (`content/base/`) is built into the binary; override packs, the folders
//! named in `UNIVERSE_CONTENT` (`:`-separated, in order), add entries or
//! replace them by key. It's loaded once, validated, and shared read-only:
//! [`content()`]. Entries are reached by typed [`Handle`]s; what's stored and
//! sent is their **key** (`hull.cobra`), never a position in a list, and
//! renamed keys resolve through the packs' aliases. The loaded content has
//! one [`Content::hash`], for saves and peers to compare.

use std::collections::HashMap;
use std::fmt;
use std::marker::PhantomData;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use serde::de::DeserializeOwned;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::ship::ClassSpec;

/// The base pack, built in: (file, source).
const BASE: &[(&str, &str)] = &[
    ("hulls.ron", include_str!("../../../content/base/hulls.ron")),
    ("aliases.ron", include_str!("../../../content/base/aliases.ron")),
];

/// A kind of content entry: what file of a pack it's in, its key, whether
/// it makes sense, and where the registry keeps it.
pub trait Entry: DeserializeOwned + Sized + 'static {
    /// The pack file entries of this kind are in.
    const FILE: &'static str;
    fn key(&self) -> &str;
    /// Is it physically and logically sound? (The reason if not.)
    fn validate(&self) -> Result<(), String>;
    /// Its registry in `c`.
    fn registry(c: &Content) -> &Registry<Self>;
}

/// A typed reference to an entry of the loaded content.
pub struct Handle<T> {
    index: u32,
    _kind: PhantomData<fn() -> T>,
}

impl<T> Handle<T> {
    fn new(index: usize) -> Self {
        Handle { index: index as u32, _kind: PhantomData }
    }
}

impl<T> Clone for Handle<T> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<T> Copy for Handle<T> {}
impl<T> PartialEq for Handle<T> {
    fn eq(&self, other: &Self) -> bool {
        self.index == other.index
    }
}
impl<T> Eq for Handle<T> {}
impl<T> std::hash::Hash for Handle<T> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.index.hash(state);
    }
}
impl<T: Entry> fmt::Debug for Handle<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", content().key_of(*self))
    }
}

/// Stored and sent as its key.
impl<T: Entry> Serialize for Handle<T> {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(content().key_of(*self))
    }
}

impl<'de, T: Entry> Deserialize<'de> for Handle<T> {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let key = String::deserialize(d)?;
        content().handle(&key).ok_or_else(|| serde::de::Error::custom(format!("no {} entry '{key}' in the loaded content", T::FILE)))
    }
}

/// The entries of one kind, in load order.
pub struct Registry<T> {
    entries: Vec<T>,
    index: HashMap<String, u32>,
}

impl<T: Entry> Registry<T> {
    /// From the packs' entries in order: a key seen again replaces the
    /// entry where it stands.
    fn build(defs: Vec<T>) -> Result<Self, String> {
        let mut r = Registry { entries: Vec::new(), index: HashMap::new() };
        for d in defs {
            d.validate().map_err(|e| format!("{} '{}': {e}", T::FILE, d.key()))?;
            match r.index.get(d.key()) {
                Some(&i) => r.entries[i as usize] = d,
                None => {
                    r.index.insert(d.key().to_string(), r.entries.len() as u32);
                    r.entries.push(d);
                }
            }
        }
        Ok(r)
    }

    pub fn get(&self, h: Handle<T>) -> &T {
        &self.entries[h.index as usize]
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn iter(&self) -> impl Iterator<Item = (Handle<T>, &T)> {
        self.entries.iter().enumerate().map(|(i, e)| (Handle::new(i), e))
    }

    fn find(&self, key: &str) -> Option<Handle<T>> {
        self.index.get(key).map(|&i| Handle::new(i as usize))
    }
}

/// The loaded content.
pub struct Content {
    pub hulls: Registry<ClassSpec>,
    aliases: HashMap<String, String>,
    hash: u64,
    packs: Vec<String>,
}

/// One pack's files: (file, source); None for a file it hasn't.
struct Pack {
    name: String,
    files: Vec<(&'static str, Option<String>)>,
}

impl Pack {
    fn base() -> Self {
        Pack { name: "base".into(), files: BASE.iter().map(|(f, s)| (*f, Some(s.to_string()))).collect() }
    }

    fn folder(dir: &Path) -> Result<Self, String> {
        if !dir.is_dir() {
            return Err(format!("content pack {} is not a folder", dir.display()));
        }
        let files = BASE
            .iter()
            .map(|(f, _)| {
                let path = dir.join(f);
                let source = path.is_file().then(|| std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))).transpose()?;
                Ok((*f, source))
            })
            .collect::<Result<_, String>>()?;
        Ok(Pack { name: dir.display().to_string(), files })
    }

    fn source(&self, file: &str) -> Option<&str> {
        self.files.iter().find(|(f, _)| *f == file).and_then(|(_, s)| s.as_deref())
    }
}

/// FNV-1a, 64-bit: the same everywhere and in every build (unlike std's hasher).
fn fnv(hash: &mut u64, bytes: &[u8]) {
    for &b in bytes {
        *hash ^= b as u64;
        *hash = hash.wrapping_mul(0x100_0000_01b3);
    }
}

impl Content {
    /// The base pack, then `overrides` in order.
    pub fn load(overrides: &[PathBuf]) -> Result<Content, String> {
        let mut packs = vec![Pack::base()];
        for dir in overrides {
            packs.push(Pack::folder(dir)?);
        }
        let mut hash = 0xcbf2_9ce4_8422_2325;
        for p in &packs {
            for (f, s) in &p.files {
                if let Some(s) = s {
                    fnv(&mut hash, f.as_bytes());
                    fnv(&mut hash, s.as_bytes());
                }
            }
        }
        let mut aliases = HashMap::new();
        for p in &packs {
            if let Some(s) = p.source("aliases.ron") {
                let more: HashMap<String, String> = ron::from_str(s).map_err(|e| format!("{} aliases.ron: {e}", p.name))?;
                aliases.extend(more);
            }
        }
        let hulls = Registry::build(Self::entries::<ClassSpec>(&packs)?)?;
        let c = Content { hulls, aliases, hash, packs: packs.into_iter().map(|p| p.name).collect() };
        c.check()?;
        Ok(c)
    }

    /// The entries of one kind across the packs, in order.
    fn entries<T: Entry>(packs: &[Pack]) -> Result<Vec<T>, String> {
        let mut all = Vec::new();
        for p in packs {
            if let Some(s) = p.source(T::FILE) {
                let mut defs: Vec<T> = ron::from_str(s).map_err(|e| format!("{} {}: {e}", p.name, T::FILE))?;
                all.append(&mut defs);
            }
        }
        Ok(all)
    }

    /// What must hold across the content as a whole.
    fn check(&self) -> Result<(), String> {
        for (old, new) in &self.aliases {
            if self.hulls.find(new).is_none() {
                return Err(format!("alias '{old}' -> '{new}': no such entry"));
            }
        }
        if self.hulls.find(crate::ship::STARTING_HULL).is_none() {
            return Err(format!("no starting hull '{}'", crate::ship::STARTING_HULL));
        }
        Ok(())
    }

    /// The entry `key` names (through the aliases).
    pub fn handle<T: Entry>(&self, key: &str) -> Option<Handle<T>> {
        let key = self.aliases.get(key).map_or(key, String::as_str);
        T::registry(self).find(key)
    }

    pub fn get<T: Entry>(&self, h: Handle<T>) -> &T {
        T::registry(self).get(h)
    }

    pub fn key_of<T: Entry>(&self, h: Handle<T>) -> &str {
        self.get(h).key()
    }

    /// One number for the content as loaded: the same packs, the same hash.
    pub fn hash(&self) -> u64 {
        self.hash
    }

    /// The packs loaded, in order.
    pub fn packs(&self) -> &[String] {
        &self.packs
    }
}

static CONTENT: OnceLock<Content> = OnceLock::new();

/// The content: the base pack and the override packs `UNIVERSE_CONTENT`
/// names, loaded on first use. Content that doesn't load is fatal: better
/// at the start than as nonsense in play.
pub fn content() -> &'static Content {
    CONTENT.get_or_init(|| {
        let overrides: Vec<PathBuf> = std::env::var("UNIVERSE_CONTENT").map(|v| v.split(':').filter(|s| !s.is_empty()).map(PathBuf::from).collect()).unwrap_or_default();
        Content::load(&overrides).unwrap_or_else(|e| panic!("content: {e}"))
    })
}

impl Entry for ClassSpec {
    const FILE: &'static str = "hulls.ron";

    fn key(&self) -> &str {
        &self.key
    }

    fn validate(&self) -> Result<(), String> {
        let positive = [("dry_mass", self.dry_mass), ("radius", self.radius), ("drag_area", self.drag_area), ("hull_strength", self.hull_strength), ("turn_rate", self.turn_rate), ("roll_rate", self.roll_rate)];
        let not_negative = [("fuel_capacity", self.fuel_capacity), ("hold_capacity", self.hold_capacity), ("main_thrust", self.main_thrust), ("rcs_thrust", self.rcs_thrust), ("lift_thrust", self.lift_thrust)];
        for (what, v) in positive {
            if !(v.is_finite() && v > 0.0) {
                return Err(format!("{what} must be positive ({v})"));
            }
        }
        for (what, v) in not_negative {
            if !(v.is_finite() && v >= 0.0) {
                return Err(format!("{what} can't be negative ({v})"));
            }
        }
        if self.main_thrust <= 0.0 {
            return Err("no main drive: every ship needs one".into());
        }
        Ok(())
    }

    fn registry(c: &Content) -> &Registry<Self> {
        &c.hulls
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_base_pack_loads_and_its_hulls_are_sound() {
        let c = Content::load(&[]).expect("the base pack loads");
        let cobra = c.handle::<ClassSpec>("hull.cobra").expect("the Cobra");
        assert_eq!(c.get(cobra).name, "COBRA MK III");
        assert_eq!(c.key_of(cobra), "hull.cobra");
        assert_eq!(c.hash(), Content::load(&[]).unwrap().hash(), "same packs, same hash");
    }

    #[test]
    fn an_override_pack_replaces_by_key_adds_and_renames() {
        let dir = std::env::temp_dir().join(format!("universe-pack-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let base = BASE[0].1;
        // (The entry: from the parenthesis that opens it, before its key.)
        let open = base[..base.find("key:").unwrap()].rfind('(').unwrap();
        let entry = &base[open..=base.rfind(')').unwrap()];
        // The Cobra replaced (heavier), and a Mk IV added.
        let heavier = entry.replacen("dry_mass: 60000.0", "dry_mass: 70000.0", 1);
        let another = entry.replacen("hull.cobra", "hull.cobra_mk4", 1);
        std::fs::write(dir.join("hulls.ron"), format!("[{heavier}, {another}]")).unwrap();
        std::fs::write(dir.join("aliases.ron"), r#"{"hull.cobra3": "hull.cobra"}"#).unwrap();
        let c = Content::load(std::slice::from_ref(&dir)).expect("the override loads");
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(c.hulls.len(), 2);
        let cobra = c.handle::<ClassSpec>("hull.cobra").unwrap();
        assert_eq!(c.get(cobra).dry_mass, 70_000.0, "replaced where it stands");
        assert!(c.handle::<ClassSpec>("hull.cobra_mk4").is_some(), "added");
        assert_eq!(c.handle::<ClassSpec>("hull.cobra3"), Some(cobra), "the old name resolves");
        assert_ne!(c.hash(), Content::load(&[]).unwrap().hash(), "other packs, another hash");
    }

    #[test]
    fn unsound_content_is_refused_with_the_reason() {
        let dir = std::env::temp_dir().join(format!("universe-bad-pack-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("hulls.ron"), BASE[0].1.replacen("main_thrust: 2.7e6", "main_thrust: 0.0", 1)).unwrap();
        let err = Content::load(std::slice::from_ref(&dir)).err().expect("refused");
        let _ = std::fs::remove_dir_all(&dir);
        assert!(err.contains("hull.cobra") && err.contains("main drive"), "{err}");
    }
}
