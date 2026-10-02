//! Goods: the catalogue of everything that's bought and sold, generated from
//! the galaxy's seed from the kinds of goods in the content (`goods.ron`):
//! `PER_KIND` of each, with a name, a base price and a mass per unit; then
//! the ores dug out of asteroids (`ores.ron`). Same seed and content, same
//! goods. Also the economy's other content: recipes, kinds of place, and
//! how markets are made up.

use serde::Deserialize;

use crate::content::{content, Handle};
use crate::rng::{mix, Rng};

/// Goods of each kind in the catalogue.
pub const PER_KIND: usize = 50;

/// A kind of goods (content: `goods.ron`).
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
    /// What a thousand people use in a day (t).
    #[serde(default)]
    pub basket: f64,
    /// What goes into its goods' names.
    pub adjectives: Vec<String>,
    pub nouns: Vec<String>,
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

    /// What a thousand people use in a day (t).
    pub fn basket(self) -> f64 {
        self.kind().basket
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

/// How many goods are generated (the ores come after).
pub fn catalog_size() -> usize {
    kinds() * PER_KIND
}

/// An ore as `ores.ron` has it: its kind of goods by key.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct OreDef {
    pub key: String,
    pub name: String,
    pub kind: String,
    pub price: f64,
    /// As stowed, broken, in a hold (t/m³); missing (0): its kind's.
    #[serde(default)]
    pub bulk_density: f64,
}

/// An ore of the loaded content: the goods an excavator fills a hold
/// with, by the tonne (price per tonne).
#[derive(Clone, Debug, PartialEq)]
pub struct OreEntry {
    pub key: String,
    pub name: String,
    pub kind: Category,
    pub price: f64,
    /// As stowed, broken, in a hold (t/m³).
    pub bulk_density: f64,
}

/// A recipe as `recipes.ron` has it.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RecipeDef {
    pub key: String,
    pub name: String,
    pub takes: Vec<(String, f64)>,
    pub makes: Vec<(String, f64)>,
}

/// A works: what it takes and what it makes, in tonnes a day.
#[derive(Clone, Debug, PartialEq)]
pub struct Recipe {
    pub key: String,
    pub name: String,
    pub takes: Vec<(Category, f64)>,
    pub makes: Vec<(Category, f64)>,
}

/// A kind of place as `places.ron` has it.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct PlaceDefSource {
    pub key: String,
    pub label: String,
    pub population: f64,
    pub ship_fuel: f64,
    pub works: Vec<(String, f64)>,
    pub sells: Vec<String>,
    pub wants: Vec<String>,
}

/// A kind of place in the economy.
#[derive(Clone, Debug, PartialEq)]
pub struct PlaceDef {
    pub key: String,
    pub label: String,
    /// People (thousands).
    pub population: f64,
    /// Fuel it keeps for the ships that call (t/day).
    pub ship_fuel: f64,
    /// Its works: (recipe, how many).
    pub works: Vec<(Handle<Recipe>, f64)>,
    /// What its market leans to selling and wanting where there's no
    /// economy behind it (beyond the gate network).
    pub sells: Vec<Category>,
    pub wants: Vec<Category>,
}

/// How markets are made up, as `markets.ron` has it.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct MarketRulesDef {
    pub bans: Vec<(String, f64)>,
}

/// How markets are made up.
#[derive(Clone, Debug, PartialEq)]
pub struct MarketRules {
    /// What a market may refuse to trade, and how likely it is to (rolled in this order).
    pub bans: Vec<(Category, f64)>,
}

/// One kind of goods.
#[derive(Clone, Debug, PartialEq)]
pub struct Item {
    pub id: usize,
    /// Its stable key: a generated good's is its kind's and its number among
    /// them (`goods.food/17`), an ore's its own (`ore.stony`).
    pub key: String,
    pub name: String,
    pub category: Category,
    /// Base price per unit (credits): what it's worth where it's neither
    /// made nor wanted.
    pub price: f64,
    /// Mass per unit (kg).
    pub mass: f64,
    /// As stowed in a hold (t/m³): its kind's, or (an ore) its own.
    pub bulk_density: f64,
}

/// Raw materials dug out of asteroids (see `mining`): what an excavator
/// fills a hold with, by the tonne. In the catalogue after its generated
/// goods, the same in every galaxy; their names and prices are content
/// (`ores.ron`, by key).
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

    /// Its content key.
    pub fn key(self) -> &'static str {
        match self {
            Ore::WaterIce => "ore.water_ice",
            Ore::Carbonaceous => "ore.carbonaceous",
            Ore::Stony => "ore.stony",
            Ore::NickelIron => "ore.nickel_iron",
            Ore::Pgm => "ore.pgm",
        }
    }

    /// As stowed, broken, in a hold (t/m³).
    pub fn bulk_density(self) -> f64 {
        let c = content();
        c.get(c.handle::<OreEntry>(self.key()).expect("every ore is in the content (checked at load)")).bulk_density
    }

    /// Its goods item: after the generated goods, in `ores.ron`'s order.
    pub fn item(self) -> usize {
        let h: Handle<OreEntry> = content().handle(self.key()).expect("every ore is in the content (checked at load)");
        catalog_size() + h.index()
    }
}

/// The catalog of goods for a galaxy `seed`: `PER_KIND` of each kind of
/// goods, with distinct names; then the ores. (A kind's goods are drawn by
/// its place in the content: new kinds go after the existing ones.)
pub fn catalog(seed: u64) -> Vec<Item> {
    let per = PER_KIND;
    let mut items = Vec::with_capacity(catalog_size() + content().ores.len());
    for (k, (category, kind)) in content().goods.iter().enumerate() {
        let mut rng = Rng::new(mix(seed, 0x6000_d500 + k as u64));
        // Every adjective–noun pair, shuffled; the first `per` of them.
        let mut names: Vec<(usize, usize)> = (0..kind.adjectives.len()).flat_map(|a| (0..kind.nouns.len()).map(move |n| (a, n))).collect();
        for i in (1..names.len()).rev() {
            let j = rng.range(0.0, (i + 1) as f64) as usize;
            names.swap(i, j.min(i));
        }
        for (number, (a, n)) in names.into_iter().take(per).enumerate() {
            let (lo, hi) = kind.price;
            // Prices spread log-uniformly across the category's range.
            let price = (lo.ln() + rng.range(0.0, 1.0) * (hi.ln() - lo.ln())).exp();
            let mass = kind.mass * rng.range(0.6, 1.4);
            items.push(Item {
                id: items.len(),
                key: format!("{}/{number}", kind.key),
                name: format!("{} {}", kind.adjectives[a], kind.nouns[n]),
                category,
                price: (price * 10.0).round() / 10.0,
                mass: mass.round().max(1.0),
                bulk_density: kind.bulk_density,
            });
        }
    }
    for (_, o) in content().ores.iter() {
        items.push(Item { id: items.len(), key: o.key.clone(), name: o.name.clone(), category: o.kind, price: o.price, mass: TONNE, bulk_density: o.bulk_density });
    }
    items
}

