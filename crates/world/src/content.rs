//! Content: what the world is made of — hulls, kinds of goods, ores,
//! recipes, kinds of place, how markets are made up; modules, brands and
//! shapes as they move in — as data, not code (see `docs/content.md`).
//!
//! Content comes in **packs**: folders of RON files. The base pack
//! (`content/base/`) is built into the binary; override packs, the folders
//! named in `UNIVERSE_CONTENT` (`:`-separated, in order), add entries or
//! replace them by key. It's loaded once, validated, and shared read-only:
//! [`content()`]. Entries are reached by typed [`Handle`]s; what's stored and
//! sent is their **key** (`hull.drover`), never a position in a list, and
//! renamed keys resolve through the packs' aliases. The loaded content has
//! one [`Content::hash`], for saves and peers to compare.

use std::collections::HashMap;
use std::fmt;
use std::marker::PhantomData;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use serde::de::DeserializeOwned;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::goods::{Category, GoodsKind, MarketRules, OreEntry, PlaceDef, Recipe};
use crate::shape::Shape;
use crate::ship::ClassSpec;

/// The base pack, built in: (file, source).
const BASE: &[(&str, &str)] = &[
    ("shapes.ron", include_str!("../../../content/base/shapes.ron")),
    ("materials.ron", include_str!("../../../content/base/materials.ron")),
    ("brands.ron", include_str!("../../../content/base/brands.ron")),
    ("structures.ron", include_str!("../../../content/base/structures.ron")),
    ("modules.ron", include_str!("../../../content/base/modules.ron")),
    ("hulls.ron", include_str!("../../../content/base/hulls.ron")),
    ("goods.ron", include_str!("../../../content/base/goods.ron")),
    ("ores.ron", include_str!("../../../content/base/ores.ron")),
    ("recipes.ron", include_str!("../../../content/base/recipes.ron")),
    ("places.ron", include_str!("../../../content/base/places.ron")),
    ("markets.ron", include_str!("../../../content/base/markets.ron")),
    ("aliases.ron", include_str!("../../../content/base/aliases.ron")),
];

/// A kind of content entry: what file of a pack it's in, its key, whether
/// it makes sense, and where the registry keeps it.
pub trait Entry: Sized + 'static {
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

    /// Its place among its kind's entries (for tables sized to them; not
    /// to be stored: keys are).
    pub fn index(self) -> usize {
        self.index as usize
    }
}

impl<T> PartialOrd for Handle<T> {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}
impl<T> Ord for Handle<T> {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.index.cmp(&other.index)
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
pub struct Registry<T: 'static> {
    entries: Vec<T>,
    index: HashMap<String, u32>,
    /// Entries added as the game runs (designed hulls): after the loaded
    /// ones, in the order added, each fixed once set.
    extra: Box<[OnceLock<&'static T>]>,
    extra_index: std::sync::RwLock<HashMap<String, u32>>,
    extra_len: std::sync::atomic::AtomicUsize,
}

/// How many entries of a kind can be added as the game runs.
pub const EXTRA: usize = 256;

impl<T: Entry> Registry<T> {
    /// From the packs' entries in order: a key seen again replaces the
    /// entry where it stands.
    fn build(defs: Vec<T>) -> Result<Self, String> {
        let mut r = Registry { entries: Vec::new(), index: HashMap::new(), extra: (0..EXTRA).map(|_| OnceLock::new()).collect(), extra_index: Default::default(), extra_len: Default::default() };
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
        let i = h.index as usize;
        match self.entries.get(i) {
            Some(e) => e,
            None => self.extra[i - self.entries.len()].get().expect("a handle to an entry added"),
        }
    }

    /// How many: the loaded ones and those added since.
    pub fn len(&self) -> usize {
        self.entries.len() + self.extra_len.load(std::sync::atomic::Ordering::Acquire)
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn iter(&self) -> impl Iterator<Item = (Handle<T>, &T)> {
        let base = self.entries.len();
        let added = self.extra_len.load(std::sync::atomic::Ordering::Acquire);
        self.entries.iter().enumerate().map(|(i, e)| (Handle::new(i), e)).chain((0..added).filter_map(move |k| self.extra[k].get().map(|e| (Handle::new(base + k), *e))))
    }

    fn find(&self, key: &str) -> Option<Handle<T>> {
        self.index.get(key).map(|&i| Handle::new(i as usize)).or_else(|| self.extra_index.read().unwrap_or_else(|e| e.into_inner()).get(key).map(|&i| Handle::new(i as usize)))
    }

    /// Add an entry as the game runs (it stays for good). One with the
    /// same key already there is kept, and its handle given.
    pub fn add(&self, entry: T) -> Result<Handle<T>, String> {
        if let Some(h) = self.find(entry.key()) {
            return Ok(h);
        }
        entry.validate()?;
        let mut index = self.extra_index.write().unwrap_or_else(|e| e.into_inner());
        if let Some(&i) = index.get(entry.key()) {
            return Ok(Handle::new(i as usize));
        }
        let k = self.extra_len.load(std::sync::atomic::Ordering::Acquire);
        if k >= EXTRA {
            return Err(format!("no room for more than {EXTRA} added {}", T::FILE));
        }
        let key = entry.key().to_string();
        let leaked: &'static T = Box::leak(Box::new(entry));
        let _ = self.extra[k].set(leaked);
        let i = self.entries.len() + k;
        index.insert(key, i as u32);
        self.extra_len.store(k + 1, std::sync::atomic::Ordering::Release);
        Ok(Handle::new(i))
    }
}

/// The loaded content.
pub struct Content {
    pub shapes: Registry<Shape>,
    pub materials: Registry<crate::materials::Material>,
    pub brands: Registry<crate::modules::Brand>,
    pub structures: Registry<crate::structures_catalogue::Structure>,
    pub modules: Registry<crate::modules::Module>,
    pub hulls: Registry<ClassSpec>,
    pub goods: Registry<GoodsKind>,
    pub ores: Registry<OreEntry>,
    pub recipes: Registry<Recipe>,
    pub places: Registry<PlaceDef>,
    pub markets: MarketRules,
    /// Ship fuel: what tanks are filled with (the code's one kind of goods by name).
    pub fuel: Category,
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
        // Each kind in turn, resolving references to the kinds before it.
        let shapes = Registry::build(Self::defs::<crate::shape::ShapeDef>(&packs, "shapes.ron")?.into_iter().map(|d| {
            let key = d.key.clone();
            d.build().map_err(|e| format!("shapes.ron '{key}': {e}"))
        }).collect::<Result<_, String>>()?)?;
        let materials: Registry<crate::materials::Material> = Registry::build(Self::defs(&packs, "materials.ron")?)?;
        let brands: Registry<crate::modules::Brand> = Registry::build(Self::defs(&packs, "brands.ron")?)?;
        let modules: Registry<crate::modules::Module> = Registry::build(Self::defs(&packs, "modules.ron")?)?;
        let structures: Registry<crate::structures_catalogue::Structure> = Registry::build(Self::defs(&packs, "structures.ron")?)?;
        for (_, s) in structures.iter() {
            if resolve(&brands, &aliases, &s.brand).is_none() {
                return Err(format!("structures.ron '{}': no brand '{}' (every product has a maker)", s.key, s.brand));
            }
            // What's installed: a comm on each, a gate relay only on a ring.
            let fitted = s.fit.iter().map(|k| resolve(&modules, &aliases, k).map(|h| modules.get(h)).ok_or_else(|| format!("structures.ron '{}': no module '{k}'", s.key))).collect::<Result<Vec<_>, _>>()?;
            if !fitted.iter().any(|m| m.does.comm().is_some()) {
                return Err(format!("structures.ron '{}': no comm (every structure has one)", s.key));
            }
            // (Relays only in space; a gate relay only on a ring.)
            let ring = matches!(s.kind, crate::structures_catalogue::StructureKind::GateRing { .. });
            let space = !s.kind.grounded();
            let fits = |d: &crate::modules::Does| match d {
                crate::modules::Does::GateRelay { .. } => ring,
                crate::modules::Does::HyperRelay { .. } => space,
                d => d.comm().is_some(),
            };
            if let Some(m) = fitted.iter().find(|m| !fits(&m.does)) {
                return Err(format!("structures.ron '{}': {} doesn't go on it", s.key, m.key));
            }
        }
        for (_, m) in modules.iter() {
            // (Every product has a maker.)
            if resolve(&brands, &aliases, &m.brand).is_none() {
                return Err(format!("modules.ron '{}': no brand '{}' (every product has a maker)", m.key, m.brand));
            }
            // What it holds or burns is a material, at its real properties.
            let of = |key: &str| resolve(&materials, &aliases, key).map(|h| materials.get(h)).ok_or_else(|| format!("modules.ron '{}': no material '{key}'", m.key));
            match &m.does {
                crate::modules::Does::Tank { capacity, holds } => {
                    let mat = of(holds)?;
                    if *capacity > mat.density * m.volume * 1.001 {
                        return Err(format!("modules.ron '{}': holds {:.0} kg of {} in {:.0} m³: denser than it is ({:.0} kg/m³)", m.key, capacity, mat.name, m.volume, mat.density));
                    }
                }
                crate::modules::Does::PowerPlant { burns, .. } => {
                    let mat = of(burns)?;
                    if mat.process == crate::materials::Process::None {
                        return Err(format!("modules.ron '{}': {} doesn't burn", m.key, mat.name));
                    }
                }
                // An engine's jet can't carry more energy per kg than its fuel gives at its efficiency.
                d => {
                    if let Some((_, exhaust, efficiency, burns)) = d.engine() {
                        let mat = of(burns)?;
                        let jet = 0.5 * exhaust * exhaust;
                        if jet > efficiency * mat.energy * 1.001 {
                            return Err(format!("modules.ron '{}': an exhaust of {:.0} m/s carries {:.1e} J/kg; {} at {:.0}% gives {:.1e}", m.key, exhaust, jet, mat.name, efficiency * 100.0, efficiency * mat.energy));
                        }
                    }
                }
            }
        }
        let module = |key: &str| resolve(&modules, &aliases, key).map(|h| (h, modules.get(h)));
        let hulls: Registry<ClassSpec> = Registry::build(
            Self::defs::<crate::ship::HullDef>(&packs, "hulls.ron")?
                .into_iter()
                .map(|d| {
                    let key = d.key().to_string();
                    if resolve(&brands, &aliases, d.brand()).is_none() {
                        return Err(format!("hulls.ron '{key}': no brand '{}' (every product has a maker)", d.brand()));
                    }
                    let shape = resolve(&shapes, &aliases, &d_shape(&d)).ok_or_else(|| format!("hulls.ron '{key}': no shape '{}'", d_shape(&d)))?;
                    d.build(shape, shapes.get(shape), module).map_err(|e| format!("hulls.ron '{key}': {e}"))
                })
                .collect::<Result<_, String>>()?,
        )?;
        let goods: Registry<GoodsKind> = Registry::build(Self::defs(&packs, "goods.ron")?)?;
        let kind = |key: &str, whose: &str| resolve(&goods, &aliases, key).ok_or_else(|| format!("{whose}: no kind of goods '{key}'"));
        let ores = Registry::build(
            Self::defs::<crate::goods::OreDef>(&packs, "ores.ron")?
                .into_iter()
                .map(|d| {
                    let k = kind(&d.kind, &d.key)?;
                    Ok(OreEntry { bulk_density: if d.bulk_density > 0.0 { d.bulk_density } else { k.bulk_density() }, kind: k, key: d.key, name: d.name, price: d.price })
                })
                .collect::<Result<_, String>>()?,
        )?;
        let pairs = |list: &[(String, f64)], whose: &str| list.iter().map(|(k, t)| Ok((kind(k, whose)?, *t))).collect::<Result<Vec<_>, String>>();
        let recipes: Registry<Recipe> = Registry::build(
            Self::defs::<crate::goods::RecipeDef>(&packs, "recipes.ron")?
                .into_iter()
                .map(|d| Ok(Recipe { takes: pairs(&d.takes, &d.key)?, makes: pairs(&d.makes, &d.key)?, key: d.key, name: d.name }))
                .collect::<Result<_, String>>()?,
        )?;
        let kinds = |list: &[String], whose: &str| list.iter().map(|k| kind(k, whose)).collect::<Result<Vec<_>, String>>();
        let places = Registry::build(
            Self::defs::<crate::goods::PlaceDefSource>(&packs, "places.ron")?
                .into_iter()
                .map(|d| {
                    let works = d.works.iter().map(|(r, n)| resolve(&recipes, &aliases, r).map(|h| (h, *n)).ok_or_else(|| format!("{}: no recipe '{r}'", d.key))).collect::<Result<_, String>>()?;
                    Ok(PlaceDef { works, sells: kinds(&d.sells, &d.key)?, wants: kinds(&d.wants, &d.key)?, key: d.key, label: d.label, population: d.population, ship_fuel: d.ship_fuel })
                })
                .collect::<Result<_, String>>()?,
        )?;
        let rules = Self::single::<crate::goods::MarketRulesDef>(&packs, "markets.ron")?;
        let markets = MarketRules { bans: rules.bans.iter().map(|(k, p)| Ok((kind(k, "markets.ron")?, *p))).collect::<Result<_, String>>()? };
        // Ships' fuel, as traded: what the starting hull's tanks hold.
        let starter = resolve(&hulls, &aliases, crate::ship::STARTING_HULL).ok_or("no starting hull")?;
        let tank_fuel = &hulls.get(starter).fuel;
        let fuel_goods = resolve(&materials, &aliases, tank_fuel).map(|h| materials.get(h).goods.clone()).unwrap_or_default();
        let fuel = kind(&fuel_goods, &format!("the starting hull's fuel '{tank_fuel}'"))?;
        let c = Content { shapes, materials, brands, structures, modules, hulls, goods, ores, recipes, places, markets, fuel, aliases, hash, packs: packs.into_iter().map(|p| p.name).collect() };
        c.check()?;
        Ok(c)
    }

    /// The entries in one file across the packs, in order.
    fn defs<D: DeserializeOwned>(packs: &[Pack], file: &str) -> Result<Vec<D>, String> {
        let mut all = Vec::new();
        for p in packs {
            if let Some(s) = p.source(file) {
                let mut defs: Vec<D> = ron::from_str(s).map_err(|e| format!("{} {file}: {e}", p.name))?;
                all.append(&mut defs);
            }
        }
        Ok(all)
    }

    /// A file that's one record: the last pack's that has it.
    fn single<D: DeserializeOwned>(packs: &[Pack], file: &str) -> Result<D, String> {
        let (p, s) = packs.iter().rev().find_map(|p| p.source(file).map(|s| (p, s))).ok_or_else(|| format!("no pack has {file}"))?;
        ron::from_str(s).map_err(|e| format!("{} {file}: {e}", p.name))
    }

    /// What must hold across the content as a whole.
    fn check(&self) -> Result<(), String> {
        for (old, new) in &self.aliases {
            let found = self.shapes.find(new).is_some() || self.brands.find(new).is_some() || self.modules.find(new).is_some() || self.hulls.find(new).is_some() || self.goods.find(new).is_some() || self.ores.find(new).is_some() || self.recipes.find(new).is_some() || self.places.find(new).is_some();
            if !found {
                return Err(format!("alias '{old}' -> '{new}': no such entry"));
            }
        }
        for ore in crate::goods::Ore::ALL {
            if self.ores.find(ore.key()).is_none() {
                return Err(format!("no ore '{}' (asteroids are made of it)", ore.key()));
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

/// A hull definition's shape key.
fn d_shape(d: &crate::ship::HullDef) -> String {
    d.shape_key().to_string()
}

/// `key` in `r`, through the aliases (while loading).
fn resolve<T: Entry>(r: &Registry<T>, aliases: &HashMap<String, String>, key: &str) -> Option<Handle<T>> {
    r.find(aliases.get(key).map_or(key, String::as_str))
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

/// The content entries kept by key alone (they reference others only as
/// resolved handles, so need no checks of their own beyond their numbers).
macro_rules! entry {
    ($t:ty, $file:literal, $field:ident, |$e:ident| $check:expr) => {
        impl Entry for $t {
            const FILE: &'static str = $file;
            fn key(&self) -> &str {
                &self.key
            }
            fn validate(&self) -> Result<(), String> {
                let $e = self;
                $check
            }
            fn registry(c: &Content) -> &Registry<Self> {
                &c.$field
            }
        }
    };
}

fn positive(what: &str, v: f64) -> Result<(), String> {
    if v.is_finite() && v > 0.0 { Ok(()) } else { Err(format!("{what} must be positive ({v})")) }
}

entry!(GoodsKind, "goods.ron", goods, |k| {
    positive("mass", k.mass)?;
    positive("bulk_density", k.bulk_density)?;
    positive("the lower price", k.price.0)?;
    if k.price.1 < k.price.0 {
        return Err("price range upside down".into());
    }
    if k.basket < 0.0 {
        return Err("basket can't be negative".into());
    }
    if k.adjectives.is_empty() || k.nouns.is_empty() || k.adjectives.len() * k.nouns.len() < crate::goods::PER_KIND {
        return Err(format!("too few names: {} adjectives × {} nouns for {} goods", k.adjectives.len(), k.nouns.len(), crate::goods::PER_KIND));
    }
    Ok(())
});
entry!(OreEntry, "ores.ron", ores, |o| positive("price", o.price));
entry!(Shape, "shapes.ron", shapes, |_s| Ok(()));
entry!(crate::modules::Module, "modules.ron", modules, |m| m.check());
entry!(crate::modules::Brand, "brands.ron", brands, |_b| Ok(()));
entry!(crate::materials::Material, "materials.ron", materials, |m| m.check());
entry!(crate::structures_catalogue::Structure, "structures.ron", structures, |s| s.check());
entry!(Recipe, "recipes.ron", recipes, |r| {
    for (_, t) in r.takes.iter().chain(&r.makes) {
        positive("a rate", *t)?;
    }
    if r.makes.is_empty() {
        return Err("makes nothing".into());
    }
    Ok(())
});
entry!(PlaceDef, "places.ron", places, |p| {
    positive("population", p.population)?;
    if p.ship_fuel < 0.0 {
        return Err("ship_fuel can't be negative".into());
    }
    for (_, n) in &p.works {
        positive("a works count", *n)?;
    }
    Ok(())
});

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
        // (Until the thrust's handled nozzle by nozzle, every direction needs its thrusters.)
        if self.rcs_thrust <= 0.0 || self.lift_thrust <= 0.0 {
            return Err("translation thrusters missing in some direction".into());
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
        let hull = c.handle::<ClassSpec>("hull.drover").expect("the Drover");
        assert_eq!(c.get(hull).name, "DROVER");
        assert_eq!(c.key_of(hull), "hull.drover");
        assert_eq!(c.hash(), Content::load(&[]).unwrap().hash(), "same packs, same hash");
    }
}
