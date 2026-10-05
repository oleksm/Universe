//! Stock: everything bought, sold, carried or stored, as the registry has it
//! (goods, mill stock, and the elements and materials a recipe names), in a
//! fixed order by key. A ledger line and a hold name one by its place in
//! that order. Each is counted by the tonne. Its price here is the game's
//! reference: what nothing makes is priced in `prices.ron`; what is made, by
//! what goes into making it. Markets move off it with their stock.

use std::collections::HashMap;

use serde::Deserialize;

use crate::content::{content, Handle};

/// A kind of goods: one of the registry's market categories.
#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GoodsKind {
    pub key: String,
    pub name: String,
    /// Base price range per unit (credits): its goods spread log-uniformly across it.
    pub price: (f64, f64),
    /// Mass per unit (kg), about.
    pub mass: f64,
    /// Bulk density as stowed in a hold (t/m³): crated, sacked or loose.
    pub bulk_density: f64,
}

/// A kind of goods of the loaded content.
pub type Category = Handle<GoodsKind>;

impl Handle<GoodsKind> {
    fn kind(self) -> &'static GoodsKind {
        content().get(self)
    }

    pub fn name(self) -> &'static str {
        &self.kind().name
    }

    /// Bulk density as stowed in a hold (t/m³).
    pub fn bulk_density(self) -> f64 {
        self.kind().bulk_density
    }

    /// Every kind, in order.
    pub fn all() -> impl Iterator<Item = Category> {
        content().goods.iter().map(|(h, _)| h)
    }

    /// Ship fuel.
    pub fn fuel() -> Category {
        content().fuel
    }

    /// The kind `key` names.
    pub fn of(key: &str) -> Option<Category> {
        content().handle(key)
    }
}

/// How many kinds of goods there are.
pub fn kinds() -> usize {
    content().goods.len()
}

/// One stock item.
#[derive(Clone, Debug, PartialEq)]
pub struct Item {
    pub id: usize,
    /// Its registry key (`good.bread`, `stock.al6061-pl-5`).
    pub key: String,
    pub name: String,
    /// The market category it is traded as (None: the registry doesn't say yet).
    pub category: Option<Category>,
    /// Reference price (credits a unit: a tonne, or one piece): what it's
    /// worth where it's neither short nor in glut.
    pub price: f64,
    /// Mass per unit (kg): a tonne of bulk stock; a part, a product or a hull, one.
    pub mass: f64,
    /// As stowed in a hold (t/m³).
    pub bulk_density: f64,
}

impl Item {
    /// Its category's name, or OTHER where the registry doesn't say yet.
    pub fn kind_name(&self) -> &'static str {
        self.category.map_or("OTHER", |c| c.name())
    }
}

/// What a megawatt-hour of power costs (credits). Invented, the game's own.
pub const POWER_PRICE: f64 = 40.0;
/// What a maker asks over what went into a thing (its works, its people). Invented.
const MARGIN: f64 = 1.25;
/// What a tonne of anything is worth that nothing makes and `prices.ron`
/// doesn't price (credits). Invented.
const RAW_PRICE: f64 = 100.0;
/// As stowed, where nothing says (t/m³).
const STOWED: f64 = 1.0;

/// The stock catalogue, from the registry and the game's prices (by key, a
/// unit's), and every module's recipes over it (see `recipes`). Bulk stock
/// is counted by the tonne; parts, equipment and hulls by the piece.
pub(crate) fn build_catalog(reg: &crate::registry::Registry, priced: &HashMap<String, f64>, goods: &crate::content::Registry<GoodsKind>) -> (Vec<Item>, HashMap<String, Vec<crate::recipes::Recipe>>) {
    let mut keys: Vec<String> = reg.goods.iter().map(|g| g.identity.key.clone()).chain(reg.stock.iter().map(|s| s.identity.key.clone())).collect();
    let named = reg.modules.iter().flat_map(|m| m.recipes.iter().flat_map(|r| r.inputs.iter().chain(&r.outputs).map(|a| a.item.clone()).chain([r.makes.clone()])).chain(m.generation.iter().flat_map(|g| g.burns.iter().map(|b| b.item.clone()))));
    keys.extend(named.filter(|k| k.starts_with("element.") || k.starts_with("material.") || k.starts_with("good.") || k.starts_with("stock.")));
    keys.extend(reg.parts.iter().map(|p| p.identity.key.clone()));
    keys.extend(reg.equipment.iter().map(|e| e.identity.key.clone()));
    keys.extend(reg.hulls.iter().map(|h| h.identity.key.clone()));
    keys.sort();
    keys.dedup();
    let index: HashMap<String, usize> = keys.iter().enumerate().map(|(i, k)| (k.clone(), i)).collect();
    // A unit's mass (kg): a tonne of bulk stock; one part, product or hull (a hull: its parts').
    let piece = |key: &str| -> Option<f64> {
        let p = reg.parts.iter().find(|p| p.identity.key == key).map(|p| p.physical.mass);
        let e = || reg.equipment.iter().find(|e| e.identity.key == key).map(|e| e.physical.mass);
        let h = || {
            let h = reg.hulls.iter().find(|h| h.identity.key == key)?;
            let parts: f64 = reg.built_of(key).iter().filter_map(|(p, n)| Some(p.physical.mass? * *n as f64)).sum();
            Some(h.physical.mass.or((parts > 0.0).then_some(parts)))
        };
        p.or_else(e).or_else(h).flatten()
    };
    let masses: Vec<f64> = keys.iter().map(|k| piece(k).filter(|m| *m > 0.0).unwrap_or(TONNE)).collect();
    let recipes = crate::recipes::build(reg, &index, &|i| masses[i]);
    // Every recipe, by what it makes.
    let mut making: HashMap<usize, Vec<&crate::recipes::Recipe>> = HashMap::new();
    for r in recipes.values().flatten() {
        making.entry(r.makes).or_default().push(r);
    }
    let category = |key: &str| reg.traded_as(key).and_then(|k| goods.find(&k));
    // Its price a kg: priced, or what goes into making it, or raw.
    fn price(i: usize, keys: &[String], masses: &[f64], making: &HashMap<usize, Vec<&crate::recipes::Recipe>>, priced: &HashMap<String, f64>, done: &mut HashMap<usize, Option<f64>>) -> Option<f64> {
        if let Some(p) = priced.get(&keys[i]) {
            return Some(*p / masses[i]);
        }
        if let Some(p) = done.get(&i) {
            return *p;
        }
        done.insert(i, None);
        let made = making.get(&i).into_iter().flatten().filter_map(|r| {
            let mut cost = 0.0;
            for &(x, q) in &r.inputs {
                cost += q * price(x, keys, masses, making, priced, done)?;
            }
            // (Power: J a kg, as MWh.)
            let power = if r.rate > 0.0 { r.power / r.rate / 3.6e9 * POWER_PRICE } else { 0.0 };
            Some((cost + power) * MARGIN)
        });
        let p = made.fold(None, |a: Option<f64>, b| Some(a.map_or(b, |a| a.min(b)))).unwrap_or(RAW_PRICE / TONNE);
        done.insert(i, Some(p));
        Some(p)
    }
    let mut done = HashMap::new();
    let items = keys
        .iter()
        .enumerate()
        .map(|(id, key)| {
            let physical = reg.goods.iter().find(|g| &g.identity.key == key).map(|g| &g.physical).or_else(|| reg.stock.iter().find(|s| &s.identity.key == key).map(|s| &s.physical));
            let c = category(key);
            let bulk = physical
                .and_then(|p| p.bulk_density.or_else(|| Some(p.mass? / p.volume?)))
                .map(|d| d / 1000.0)
                .or_else(|| c.map(|c| goods.get(c).bulk_density))
                .filter(|d| *d > 0.0)
                .unwrap_or(STOWED);
            let p = price(id, &keys, &masses, &making, priced, &mut done).unwrap_or(RAW_PRICE / TONNE) * masses[id];
            // (A part by its code too: every product has a first wall.)
            let name = reg.name(key).unwrap_or(key).to_string();
            let name = reg.parts.iter().find(|p| &p.identity.key == key).map_or(name.clone(), |p| format!("{} {name}", p.identity.code));
            Item { id, key: key.clone(), name, category: c, price: (p * 10.0).round() / 10.0, mass: masses[id], bulk_density: bulk }
        })
        .collect();
    (items, recipes)
}

/// Is `a` drawn where the module stands (the world's air, rain or ground
/// water), not taken from stock?
pub fn from_place(a: &crate::registry::Amount) -> bool {
    a.from == Some(crate::registry::AmountFrom::Place)
}

/// The stock catalogue.
pub fn catalog() -> Vec<Item> {
    content().stock.clone()
}

/// The stock item with this key.
pub fn item(key: &str) -> Option<usize> {
    content().stock_index.get(key).copied()
}

/// Raw materials dug out of asteroids (see `mining`): what an excavator
/// fills a hold with, by the tonne. In the catalogue after its generated
/// goods, the same in every galaxy; their names and prices are content
/// (the registry's rock goods, by key).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ore {
    /// Icy bodies: water ice with frozen volatiles.
    WaterIce,
    /// C-types: clays with water bound in, carbon, organics.
    Carbonaceous,
    /// S-types: silicates with nickel-iron grains.
    Stony,
    /// M-types: nickel-iron.
    NickelIron,
    /// M-types rich in platinum-group metals.
    Pgm,
}

/// One unit of ore (kg).
pub const TONNE: f64 = 1000.0;

impl Ore {
    pub const ALL: [Ore; 5] = [Ore::WaterIce, Ore::Carbonaceous, Ore::Stony, Ore::NickelIron, Ore::Pgm];

    /// The ore with this content key.
    pub fn from_key(key: &str) -> Option<Ore> {
        Ore::ALL.into_iter().find(|o| o.key() == key)
    }

    /// Its content key.
    pub fn key(self) -> &'static str {
        match self {
            Ore::WaterIce => "good.asteroid-water-ice",
            Ore::Carbonaceous => "good.carbonaceous-ore",
            Ore::Stony => "good.stony-ore",
            Ore::NickelIron => "good.nickel-iron-ore",
            Ore::Pgm => "good.pgm-rich-ore",
        }
    }

    /// As stowed, broken, in a hold (t/m³).
    pub fn bulk_density(self) -> f64 {
        content().stock[self.item()].bulk_density
    }

    /// Its stock item.
    pub fn item(self) -> usize {
        item(self.key()).expect("every ore is in the registry (checked at load)")
    }

    /// The ore stock item `id` is, if one.
    pub fn of_item(id: usize) -> Option<Ore> {
        Ore::ALL.into_iter().find(|o| o.item() == id)
    }
}
