//! Goods: the catalog of everything that's bought and sold, generated from
//! the galaxy's seed (`CATALOG_SIZE` items in `CATEGORIES`, each with a name,
//! a base price and a mass per unit). Same seed, same goods.

use crate::rng::{mix, Rng};

/// How many goods there are.
pub const CATALOG_SIZE: usize = 1000;

/// A kind of goods.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Category {
    Food,
    Water,
    Ores,
    Metals,
    Minerals,
    Chemicals,
    Fuel,
    Textiles,
    Machinery,
    Electronics,
    Computers,
    Medicine,
    Biologics,
    Luxuries,
    Art,
    Weapons,
    Narcotics,
    Artifacts,
    Robots,
    Tools,
}

/// What goes into a category's names, what its goods are worth, and how heavy.
struct Kind {
    category: Category,
    adjectives: &'static [&'static str],
    nouns: &'static [&'static str],
    /// Base price range per unit (credits).
    price: (f64, f64),
    /// Mass per unit (kg).
    mass: f64,
}

const KINDS: [Kind; 20] = [
    Kind { category: Category::Food, adjectives: &["Hydroponic", "Freeze-Dried", "Synthetic", "Orbital", "Tinned", "Smoked", "Organic", "Vat-Grown", "Pickled"], nouns: &["Kelp", "Protein Bars", "Grain", "Fruit", "Fungi", "Algae Cakes", "Tubers", "Beans", "Cheese"], price: (4.0, 30.0), mass: 500.0 },
    Kind { category: Category::Water, adjectives: &["Distilled", "Glacial", "Comet", "Mineral", "Heavy", "Recycled", "Sterile", "Polar", "Spring"], nouns: &["Water", "Ice", "Brine", "Slush", "Condensate", "Snowpack", "Meltwater", "Vapour Cells"], price: (1.0, 12.0), mass: 1000.0 },
    Kind { category: Category::Ores, adjectives: &["Raw", "Crushed", "Magnetic", "Banded", "Rich", "Lean", "Oxidised", "Sulphide", "Placer"], nouns: &["Iron Ore", "Copper Ore", "Bauxite", "Nickel Ore", "Cobalt Ore", "Tin Ore", "Zinc Ore", "Lead Ore", "Chromite"], price: (6.0, 40.0), mass: 1000.0 },
    Kind { category: Category::Metals, adjectives: &["Refined", "Cast", "Rolled", "Forged", "Sintered", "Alloyed", "Vacuum-Cast", "Drawn", "Plated"], nouns: &["Titanium", "Aluminium", "Steel", "Copper", "Nickel", "Tungsten", "Magnesium", "Iridium", "Chromium"], price: (30.0, 200.0), mass: 800.0 },
    Kind { category: Category::Minerals, adjectives: &["Cut", "Rough", "Polished", "Industrial", "Clear", "Flawed", "Star", "Deep", "Asteroid"], nouns: &["Quartz", "Diamonds", "Beryl", "Corundum", "Garnets", "Opal", "Jade", "Spinel", "Topaz"], price: (50.0, 900.0), mass: 20.0 },
    Kind { category: Category::Chemicals, adjectives: &["Industrial", "Reagent", "Bulk", "Stabilised", "Chilled", "Pressurised", "Catalytic", "Pure", "Crude"], nouns: &["Solvents", "Acids", "Polymers", "Resins", "Salts", "Catalysts", "Reagents", "Monomers", "Lubricants"], price: (15.0, 120.0), mass: 600.0 },
    Kind { category: Category::Fuel, adjectives: &["Liquid", "Slush", "Enriched", "Cracked", "Compressed", "Cryo", "Refined", "Blended", "Metallic"], nouns: &["Hydrogen", "Methane", "Deuterium", "Helium-3", "Oxygen", "Hydrazine", "Kerosene", "Ammonia", "Xenon"], price: (10.0, 150.0), mass: 700.0 },
    Kind { category: Category::Textiles, adjectives: &["Woven", "Spun", "Synthetic", "Fine", "Bulk", "Insulated", "Dyed", "Printed", "Smart"], nouns: &["Cotton", "Silk", "Canvas", "Fibre", "Nanoweave", "Wool", "Linen", "Mesh", "Felt"], price: (10.0, 90.0), mass: 200.0 },
    Kind { category: Category::Machinery, adjectives: &["Heavy", "Mining", "Farm", "Precision", "Surplus", "Rebuilt", "Modular", "Hydraulic", "Autonomous"], nouns: &["Drills", "Pumps", "Presses", "Turbines", "Excavators", "Harvesters", "Compressors", "Lathes", "Cranes"], price: (80.0, 600.0), mass: 900.0 },
    Kind { category: Category::Electronics, adjectives: &["Consumer", "Hardened", "Surplus", "Salvaged", "Precision", "Optical", "Quantum", "Flexible", "Military-Spec"], nouns: &["Circuits", "Sensors", "Displays", "Batteries", "Transceivers", "Capacitors", "Emitters", "Relays", "Lasers"], price: (60.0, 500.0), mass: 50.0 },
    Kind { category: Category::Computers, adjectives: &["Personal", "Navigation", "Neural", "Rack", "Embedded", "Rugged", "Archive", "Tactical", "Cold"], nouns: &["Terminals", "Processors", "Cores", "Memory Cubes", "Nav Units", "Servers", "Tablets", "Controllers", "Logic Arrays"], price: (150.0, 1200.0), mass: 30.0 },
    Kind { category: Category::Medicine, adjectives: &["Generic", "Sterile", "Antiviral", "Trauma", "Radiation", "Gene", "Field", "Cryo", "Pediatric"], nouns: &["Antibiotics", "Vaccines", "Kits", "Serum", "Bandages", "Stims", "Therapies", "Plasma", "Implants"], price: (40.0, 400.0), mass: 20.0 },
    Kind { category: Category::Biologics, adjectives: &["Live", "Frozen", "Seed", "Engineered", "Wild", "Cloned", "Dormant", "Heritage", "Marine"], nouns: &["Embryos", "Cultures", "Seeds", "Spores", "Cattle", "Bees", "Coral", "Enzymes", "Yeasts"], price: (30.0, 350.0), mass: 100.0 },
    Kind { category: Category::Luxuries, adjectives: &["Vintage", "Imported", "Handmade", "Gilded", "Rare", "Designer", "Aged", "Perfumed", "Exotic"], nouns: &["Wines", "Spirits", "Furs", "Perfume", "Chocolate", "Coffee", "Tea", "Tobacco", "Jewellery"], price: (150.0, 2000.0), mass: 10.0 },
    Kind { category: Category::Art, adjectives: &["Old-Earth", "Holographic", "Carved", "Painted", "Sculpted", "Colonial", "Abstract", "Antique", "Signed"], nouns: &["Canvases", "Statues", "Tapestries", "Prints", "Ceramics", "Masks", "Mosaics", "Scrolls", "Figurines"], price: (300.0, 5000.0), mass: 15.0 },
    Kind { category: Category::Weapons, adjectives: &["Small", "Heavy", "Surplus", "Hand", "Automatic", "Pulse", "Ceremonial", "Military", "Hunting"], nouns: &["Arms", "Rifles", "Pistols", "Charges", "Grenades", "Blades", "Ammunition", "Launchers", "Mines"], price: (100.0, 900.0), mass: 60.0 },
    Kind { category: Category::Narcotics, adjectives: &["Refined", "Raw", "Synthetic", "Street", "Pure", "Cut", "Designer", "Liquid", "Pressed"], nouns: &["Dust", "Spice", "Resin", "Crystals", "Tabs", "Leaf", "Powder", "Drops", "Smoke"], price: (200.0, 3000.0), mass: 5.0 },
    Kind { category: Category::Artifacts, adjectives: &["Alien", "Ancient", "Precursor", "Relic", "Unknown", "Fossil", "Buried", "Glowing", "Broken"], nouns: &["Shards", "Tablets", "Idols", "Devices", "Bones", "Spheres", "Keys", "Glyphs", "Engines"], price: (500.0, 9000.0), mass: 25.0 },
    Kind { category: Category::Robots, adjectives: &["Service", "Mining", "Cargo", "Medical", "Farm", "Security", "Repair", "Survey", "Companion"], nouns: &["Drones", "Walkers", "Arms", "Units", "Crawlers", "Androids", "Swarms", "Rovers", "Frames"], price: (300.0, 2500.0), mass: 250.0 },
    Kind { category: Category::Tools, adjectives: &["Hand", "Power", "Welding", "Cutting", "Survey", "Vacuum", "Diagnostic", "Precision", "Utility"], nouns: &["Tools", "Torches", "Saws", "Kits", "Gauges", "Scanners", "Wrenches", "Meters", "Rigs"], price: (20.0, 250.0), mass: 40.0 },
];

impl Category {
    pub fn name(self) -> &'static str {
        match self {
            Category::Food => "FOOD",
            Category::Water => "WATER",
            Category::Ores => "ORES",
            Category::Metals => "METALS",
            Category::Minerals => "MINERALS",
            Category::Chemicals => "CHEMICALS",
            Category::Fuel => "FUEL",
            Category::Textiles => "TEXTILES",
            Category::Machinery => "MACHINERY",
            Category::Electronics => "ELECTRONICS",
            Category::Computers => "COMPUTERS",
            Category::Medicine => "MEDICINE",
            Category::Biologics => "BIOLOGICS",
            Category::Luxuries => "LUXURIES",
            Category::Art => "ART",
            Category::Weapons => "WEAPONS",
            Category::Narcotics => "NARCOTICS",
            Category::Artifacts => "ARTIFACTS",
            Category::Robots => "ROBOTS",
            Category::Tools => "TOOLS",
        }
    }

    pub fn all() -> impl Iterator<Item = Category> {
        KINDS.iter().map(|k| k.category)
    }
}

/// One kind of goods.
#[derive(Clone, Debug, PartialEq)]
pub struct Item {
    pub id: usize,
    pub name: String,
    pub category: Category,
    /// Base price per unit (credits): what it's worth where it's neither
    /// made nor wanted.
    pub price: f64,
    /// Mass per unit (kg).
    pub mass: f64,
}

/// The catalog of goods for a galaxy `seed`: `CATALOG_SIZE` items, a
/// `CATALOG_SIZE / 20` from each category, with distinct names.
pub fn catalog(seed: u64) -> Vec<Item> {
    let per = CATALOG_SIZE / KINDS.len();
    let mut items = Vec::with_capacity(CATALOG_SIZE);
    for (k, kind) in KINDS.iter().enumerate() {
        let mut rng = Rng::new(mix(seed, 0x6000_d500 + k as u64));
        // Every adjective–noun pair, shuffled; the first `per` of them.
        let mut names: Vec<(usize, usize)> = (0..kind.adjectives.len()).flat_map(|a| (0..kind.nouns.len()).map(move |n| (a, n))).collect();
        for i in (1..names.len()).rev() {
            let j = rng.range(0.0, (i + 1) as f64) as usize;
            names.swap(i, j.min(i));
        }
        for (a, n) in names.into_iter().take(per) {
            let (lo, hi) = kind.price;
            // Prices spread log-uniformly across the category's range.
            let price = (lo.ln() + rng.range(0.0, 1.0) * (hi.ln() - lo.ln())).exp();
            let mass = kind.mass * rng.range(0.6, 1.4);
            items.push(Item {
                id: items.len(),
                name: format!("{} {}", kind.adjectives[a], kind.nouns[n]),
                category: kind.category,
                price: (price * 10.0).round() / 10.0,
                mass: mass.round().max(1.0),
            });
        }
    }
    items
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_thousand_distinct_goods_the_same_every_time() {
        let a = catalog(1984);
        assert_eq!(a.len(), CATALOG_SIZE);
        let mut names: Vec<&str> = a.iter().map(|i| i.name.as_str()).collect();
        names.sort();
        names.dedup();
        assert_eq!(names.len(), CATALOG_SIZE, "names are distinct");
        assert_eq!(a, catalog(1984));
        assert_ne!(a, catalog(7));
        assert!(a.iter().all(|i| i.price > 0.0 && i.mass >= 1.0));
    }
}
