//! The economy, coarse-grained: every settled market is a place with people
//! and works, and a stock of each kind of goods (tonnes, by category; the
//! named goods are varieties within their kind). Every `STEP` the works turn
//! inputs into outputs — as far as their inputs and their storage allow —
//! and the people use goods up. Nothing moves between places by itself:
//! ships carry it, by trading (see `market`). A place's prices follow its
//! stock against what it needs or makes over `COVER_DAYS`: short is dear, a
//! glut cheap.
//!
//! What a place is follows from where it is: a station runs factories; an
//! Earth-like world farms; a dry world mines and refines; a cratered moon
//! mines and works ice. Unsettled systems (beyond the gate network) have no
//! economy: their markets stay as generated.

use std::collections::HashMap;

use universe_world::goods::Category;
use universe_world::system::{BodyKind, StarSystem};
use universe_world::terrain::TerrainKind;
use universe_world::traffic::{facilities, Facility};

/// Kinds of goods (the categories), in order.
pub const LINES: usize = 20;
/// The economy steps this often (game s).
pub const STEP: f64 = 600.0;
/// Stock a place aims to hold: this many days of what it uses or makes.
pub const COVER_DAYS: f64 = 10.0;
/// Its storage holds this many times that; full, its works stop.
const STORAGE: f64 = 3.0;
const DAY: f64 = 86_400.0;

fn line(c: Category) -> usize {
    c as usize
}

/// What a place is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlaceKind {
    /// An orbital station: factories, a fab, a fuel plant, a smelter.
    Station,
    /// A spaceport on an Earth-like world: farms.
    Farm,
    /// A spaceport on a dry world: mines and a refinery.
    Mine,
    /// A spaceport on a cratered moon: a mine and ice works.
    Outpost,
}

impl PlaceKind {
    pub fn label(self) -> &'static str {
        match self {
            PlaceKind::Station => "STATION",
            PlaceKind::Farm => "FARM WORLD",
            PlaceKind::Mine => "MINING WORLD",
            PlaceKind::Outpost => "OUTPOST",
        }
    }

    /// People (thousands).
    fn population(self) -> f64 {
        match self {
            PlaceKind::Station => 25.0,
            PlaceKind::Farm => 60.0,
            PlaceKind::Mine => 20.0,
            PlaceKind::Outpost => 3.0,
        }
    }

    /// Fuel it keeps for the ships that call (t/day): sold to them (see
    /// `Markets::refuel`), not used by the place itself.
    pub fn ship_fuel(self) -> f64 {
        match self {
            PlaceKind::Station => 200.0,
            PlaceKind::Farm => 60.0,
            PlaceKind::Mine => 60.0,
            PlaceKind::Outpost => 20.0,
        }
    }

    /// Its works: (recipe, how many).
    fn works(self) -> &'static [(&'static Recipe, f64)] {
        match self {
            PlaceKind::Station => &STATION_WORKS,
            PlaceKind::Farm => &FARM_WORKS,
            PlaceKind::Mine => &MINE_WORKS,
            PlaceKind::Outpost => &OUTPOST_WORKS,
        }
    }
}

/// A works: what it takes and what it makes, in tonnes a day.
pub struct Recipe {
    pub name: &'static str,
    pub takes: &'static [(Category, f64)],
    pub makes: &'static [(Category, f64)],
}

use Category::*;
static MINE: Recipe = Recipe { name: "MINE", takes: &[], makes: &[(Ores, 40.0), (Minerals, 4.0)] };
static ICE: Recipe = Recipe { name: "ICE WORKS", takes: &[], makes: &[(Water, 300.0)] };
// (An Earth-like world's own rain waters its fields.)
static FARM: Recipe = Recipe { name: "FARMS", takes: &[(Chemicals, 2.0)], makes: &[(Food, 30.0), (Biologics, 0.6), (Textiles, 1.2)] };
static ARTISANS: Recipe = Recipe { name: "ARTISANS", takes: &[(Textiles, 1.0), (Minerals, 0.5)], makes: &[(Luxuries, 1.5), (Art, 0.3)] };
static REFINERY: Recipe = Recipe { name: "REFINERY", takes: &[(Ores, 30.0)], makes: &[(Metals, 20.0), (Minerals, 2.0)] };
static SMELTER: Recipe = Recipe { name: "SMELTER", takes: &[(Ores, 10.0)], makes: &[(Metals, 6.0)] };
static CHEMICALS: Recipe = Recipe { name: "CHEMICAL PLANT", takes: &[(Ores, 8.0), (Water, 10.0)], makes: &[(Chemicals, 24.0)] };
static FUEL_PLANT: Recipe = Recipe { name: "FUEL PLANT", takes: &[(Water, 20.0)], makes: &[(Fuel, 18.0)] };
static FACTORY: Recipe = Recipe {
    name: "FACTORY",
    takes: &[(Metals, 10.0), (Chemicals, 3.0), (Electronics, 1.0)],
    makes: &[(Machinery, 6.0), (Tools, 4.0), (Robots, 1.0), (Weapons, 1.0)],
};
static FAB: Recipe = Recipe { name: "ELECTRONICS FAB", takes: &[(Metals, 1.0), (Minerals, 2.0), (Chemicals, 2.0)], makes: &[(Electronics, 3.0), (Computers, 1.0)] };
static PHARMA: Recipe = Recipe { name: "PHARMA PLANT", takes: &[(Chemicals, 2.0), (Biologics, 2.0)], makes: &[(Medicine, 4.0)] };

static STATION_WORKS: [(&Recipe, f64); 5] = [(&FACTORY, 2.0), (&FAB, 2.0), (&FUEL_PLANT, 32.0), (&PHARMA, 1.0), (&SMELTER, 1.0)];
static FARM_WORKS: [(&Recipe, f64); 2] = [(&FARM, 8.0), (&ARTISANS, 2.0)];
static MINE_WORKS: [(&Recipe, f64); 3] = [(&MINE, 2.0), (&REFINERY, 1.0), (&CHEMICALS, 1.0)];
static OUTPOST_WORKS: [(&Recipe, f64); 2] = [(&MINE, 0.5), (&ICE, 1.0)];

/// What a thousand people use in a day (tonnes).
const BASKET: &[(Category, f64)] = &[
    (Food, 1.5),
    (Water, 0.2),
    (Fuel, 0.1),
    (Medicine, 0.02),
    (Textiles, 0.05),
    (Luxuries, 0.02),
    (Electronics, 0.02),
    (Computers, 0.005),
    (Tools, 0.02),
    (Machinery, 0.05),
    (Robots, 0.003),
    (Weapons, 0.005),
    (Biologics, 0.01),
    (Chemicals, 0.1),
    (Art, 0.002),
];

/// A settled market's economy.
#[derive(Clone, Debug)]
pub struct Place {
    pub system: usize,
    pub facility: Facility,
    pub kind: PlaceKind,
    /// People (thousands).
    pub population: f64,
    /// Stock by kind of goods (tonnes).
    pub stock: [f64; LINES],
    /// Over the last step, per day (tonnes): made, used (by works and
    /// people), and wanted but not there.
    pub made: [f64; LINES],
    pub used: [f64; LINES],
    pub short: [f64; LINES],
}

impl Place {
    fn new(system: usize, facility: Facility, kind: PlaceKind) -> Self {
        let mut p = Place { system, facility, kind, population: kind.population(), stock: [0.0; LINES], made: [0.0; LINES], used: [0.0; LINES], short: [0.0; LINES] };
        // Starting at the stock it aims for.
        for c in Category::all() {
            p.stock[line(c)] = p.target(c);
        }
        p
    }

    /// What it uses of kind `c` in a day, at full work (tonnes).
    pub fn needs(&self, c: Category) -> f64 {
        let works: f64 = self.kind.works().iter().flat_map(|(r, n)| r.takes.iter().filter(|t| t.0 == c).map(move |t| t.1 * n)).sum();
        let ships = if c == Fuel { self.kind.ship_fuel() } else { 0.0 };
        works + ships + BASKET.iter().filter(|b| b.0 == c).map(|b| b.1 * self.population).sum::<f64>()
    }

    /// What it makes of kind `c` in a day, at full work (tonnes).
    pub fn makes(&self, c: Category) -> f64 {
        self.kind.works().iter().flat_map(|(r, n)| r.makes.iter().filter(|m| m.0 == c).map(move |m| m.1 * n)).sum()
    }

    /// The stock it aims to hold of kind `c` (tonnes); zero if it neither uses nor makes it.
    pub fn target(&self, c: Category) -> f64 {
        COVER_DAYS * self.needs(c).max(self.makes(c))
    }

    /// Does it trade kind `c`? As a seller (it makes more than it uses) or a buyer.
    pub fn sells(&self, c: Category) -> bool {
        self.makes(c) > self.needs(c)
    }

    pub fn trades(&self, c: Category) -> bool {
        self.target(c) > 0.0
    }

    /// How its price for kind `c` stands against usual: up when short,
    /// down in a glut (None: not traded here).
    pub fn factor(&self, c: Category) -> Option<f64> {
        let target = self.target(c);
        (target > 0.0).then(|| (target / self.stock[line(c)].max(target * 0.05)).powf(0.6).clamp(0.4, 3.0))
    }

    /// Room left for kind `c` (tonnes): what it would take in.
    pub fn room(&self, c: Category) -> f64 {
        (self.target(c) * STORAGE - self.stock[line(c)]).max(0.0)
    }

    pub fn stock_of(&self, c: Category) -> f64 {
        self.stock[line(c)]
    }

    /// Tonnes of kind `c` taken out (bought from it) or put in (sold to it).
    pub fn take(&mut self, c: Category, t: f64) {
        self.stock[line(c)] = (self.stock[line(c)] - t).max(0.0);
    }

    pub fn put(&mut self, c: Category, t: f64) {
        self.stock[line(c)] += t;
    }

    /// `days` of work and life.
    fn step(&mut self, days: f64) {
        let (mut made, mut used, mut short) = ([0.0; LINES], [0.0; LINES], [0.0; LINES]);
        let full: Vec<bool> = Category::all().map(|c| self.stock[line(c)] >= self.target(c) * STORAGE).collect();
        for (r, n) in self.kind.works() {
            // As much as its inputs allow; none while its outputs have nowhere to go.
            let mut k: f64 = if r.makes.iter().all(|m| full[line(m.0)]) { 0.0 } else { 1.0 };
            for &(c, rate) in r.takes {
                k = k.min(self.stock[line(c)] / (rate * n * days));
            }
            for &(c, rate) in r.takes {
                let t = rate * n * days * k;
                self.stock[line(c)] -= t;
                used[line(c)] += t;
                short[line(c)] += rate * n * days * (1.0 - k);
            }
            for &(c, rate) in r.makes {
                let t = rate * n * days * k;
                // (What there's no room for is dumped.)
                let room = (self.target(c) * STORAGE - self.stock[line(c)]).max(0.0);
                self.stock[line(c)] += t.min(room);
                made[line(c)] += t;
            }
        }
        for &(c, rate) in BASKET {
            let want = rate * self.population * days;
            let got = want.min(self.stock[line(c)]);
            self.stock[line(c)] -= got;
            used[line(c)] += got;
            short[line(c)] += want - got;
        }
        for i in 0..LINES {
            self.made[i] = made[i] / days;
            self.used[i] = used[i] / days;
            self.short[i] = short[i] / days;
        }
    }
}

/// What a facility is, in the economy: from what it's on.
pub fn kind_of(sys: &StarSystem, f: Facility) -> Option<PlaceKind> {
    match f {
        Facility::Station(_) => Some(PlaceKind::Station),
        Facility::Spaceport(p) => {
            let body = &sys.bodies[sys.spaceports.get(p)?.body];
            Some(match (body.kind, body.terrain.as_ref().map(|t| t.kind)) {
                (_, Some(TerrainKind::Terran)) => PlaceKind::Farm,
                (BodyKind::Moon, _) => PlaceKind::Outpost,
                _ => PlaceKind::Mine,
            })
        }
        _ => None,
    }
}

/// The economy of the settled systems.
#[derive(Clone, Debug, Default)]
pub struct Economy {
    pub places: Vec<Place>,
    index: HashMap<(usize, Facility), usize>,
    /// Stepped up to this world time.
    pub stepped_to: f64,
    /// For the instruments: the places as of the last change, shared.
    snapshot: Option<std::sync::Arc<Vec<Place>>>,
}

impl Economy {
    /// Places for every market of the settled `systems`, from world time `now`.
    pub fn new<'a>(systems: impl Iterator<Item = (usize, &'a StarSystem)>, now: f64) -> Self {
        let mut e = Economy { stepped_to: now, ..Default::default() };
        for (system, sys) in systems {
            for f in facilities(sys) {
                if let Some(kind) = kind_of(sys, f) {
                    e.index.insert((system, f), e.places.len());
                    e.places.push(Place::new(system, f, kind));
                }
            }
        }
        e
    }

    pub fn place(&self, system: usize, f: Facility) -> Option<&Place> {
        self.index.get(&(system, f)).map(|&i| &self.places[i])
    }

    pub fn place_mut(&mut self, system: usize, f: Facility) -> Option<&mut Place> {
        self.snapshot = None;
        self.index.get(&(system, f)).map(|&i| &mut self.places[i])
    }

    /// The places as they are, shared (rebuilt only after a change).
    pub fn snapshot(&mut self) -> std::sync::Arc<Vec<Place>> {
        self.snapshot.get_or_insert_with(|| std::sync::Arc::new(self.places.clone())).clone()
    }

    /// Bring every place up to world time `now`, a `STEP` at a time.
    pub fn step_to(&mut self, now: f64) {
        while self.stepped_to + STEP <= now {
            self.snapshot = None;
            for p in &mut self.places {
                p.step(STEP / DAY);
            }
            self.stepped_to += STEP;
        }
    }

    /// Across all places, per kind of goods: stock (t), and made, used and
    /// short per day (t) — for the instruments.
    pub fn totals(&self) -> [(f64, f64, f64, f64); LINES] {
        let mut t = [(0.0, 0.0, 0.0, 0.0); LINES];
        for p in &self.places {
            for (i, t) in t.iter_mut().enumerate() {
                t.0 += p.stock[i];
                t.1 += p.made[i];
                t.2 += p.used[i];
                t.3 += p.short[i];
            }
        }
        t
    }
}

impl Economy {
    /// An ideal hauler (for the instruments): what's short anywhere brought
    /// at once from wherever has spare. The tonnes moved.
    pub fn haul_ideally(&mut self) -> f64 {
        self.snapshot = None;
        let mut hauled = 0.0;
        for c in Category::all() {
            let i = line(c);
            let spare: f64 = self.places.iter().map(|p| (p.stock[i] - p.target(c)).max(0.0)).sum();
            let want: f64 = self.places.iter().map(|p| (p.target(c) - p.stock[i]).max(0.0)).sum();
            let k = if want > 0.0 { (spare / want).min(1.0) } else { 0.0 };
            let give = if spare > 0.0 { (want * k) / spare } else { 0.0 };
            for p in &mut self.places {
                let t = p.target(c);
                if p.stock[i] > t {
                    p.stock[i] -= (p.stock[i] - t) * give;
                } else {
                    let add = (t - p.stock[i]) * k;
                    p.stock[i] += add;
                    hauled += add;
                }
            }
        }
        hauled
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use universe_world::World;

    fn economy() -> (World, Economy) {
        let w = World::new(1984);
        let mut s: Vec<usize> = w.gate_links.iter().flat_map(|&(a, b)| [a, b]).collect();
        s.sort_unstable();
        s.dedup();
        let systems: Vec<_> = s.iter().map(|&i| (i, w.system(i))).collect();
        let e = Economy::new(systems.iter().map(|(i, s)| (*i, &**s)), 0.0);
        (w, e)
    }

    #[test]
    fn a_place_alone_works_while_its_inputs_last_then_goes_short() {
        let (_, mut e) = economy();
        assert!(e.places.len() > 20);
        let station = e.places.iter().position(|p| p.kind == PlaceKind::Station).unwrap();
        // Thirty days with no ship calling: it eats through its food (it grows
        // none), and its factories stop when the metal runs out.
        e.step_to(30.0 * DAY);
        let p = &e.places[station];
        assert_eq!(p.stock_of(Food), 0.0);
        assert!(p.short[line(Food)] > 0.0, "people go hungry");
        assert!(p.factor(Food).unwrap() > 2.0, "and food is dear there");
        assert_eq!(p.made[line(Machinery)], 0.0, "no metal, no machines");
        // Nothing runs away: every stock stays within its storage.
        for p in &e.places {
            for c in Category::all() {
                assert!(p.stock_of(c) <= p.target(c) * STORAGE + 1e-6 && p.stock_of(c) >= 0.0);
            }
        }
    }

    #[test]
    fn hauled_what_they_need_the_places_keep_working() {
        // An ideal hauler: every step, what's short somewhere is brought from
        // wherever has spare. The world sustains itself if it's carried.
        let (_, mut e) = economy();
        let mut hauled = 0.0;
        for _ in 0..(30.0 * DAY / STEP) as usize {
            let now = e.stepped_to + STEP;
            e.step_to(now);
            hauled += e.haul_ideally();
        }
        let t = e.totals();
        let short: f64 = t.iter().map(|l| l.3).sum();
        let made: f64 = t.iter().map(|l| l.1).sum();
        eprintln!("hauled {:.0} t/day; made {made:.0} t/day; short {short:.1} t/day", hauled / 30.0);
        assert!(short < made * 0.05, "hardly anything short: {short:.1} of {made:.0} t/day");
    }
}
