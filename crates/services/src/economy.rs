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

use universe_world::content::content;
use universe_world::goods::{Category, PlaceDef, Recipe};
use universe_world::system::{BodyKind, StarSystem};
use universe_world::terrain::TerrainKind;
use universe_world::traffic::{facilities, Facility};

/// Kinds of goods there are (the lines a place keeps stock in).
pub fn lines() -> usize {
    universe_world::goods::kinds()
}
/// The economy steps this often (game s).
pub const STEP: f64 = 600.0;
/// Stock a place aims to hold: this many days of what it uses or makes.
pub const COVER_DAYS: f64 = 10.0;
/// Its storage holds this many times that; full, its works stop.
const STORAGE: f64 = 3.0;
/// Fed (its food and water met, over a few days): it grows this much a
/// day, to `ROOM` times its founding size.
pub const GROWTH: f64 = 0.002;
pub const ROOM: f64 = 3.0;
/// Hungry, its people want to leave: at worst this share a day joins those
/// waiting for passage (no more than `WAITING_MOST` of them).
pub const EMIGRATE: f64 = 0.05;
pub const WAITING_MOST: f64 = 0.33;
/// Starving (under half fed), at worst this share of its people die a day.
pub const DEATH: f64 = 0.01;
/// How long being fed or hungry takes to tell (days).
const FED_DAYS: f64 = 3.0;
const DAY: f64 = 86_400.0;

fn line(c: Category) -> usize {
    c.index()
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
        &self.def().label
    }

    /// People (thousands).
    /// Its content key.
    pub fn key(self) -> &'static str {
        match self {
            PlaceKind::Station => "place.station",
            PlaceKind::Farm => "place.farm",
            PlaceKind::Mine => "place.mine",
            PlaceKind::Outpost => "place.outpost",
        }
    }

    /// What the content says it is.
    pub fn def(self) -> &'static PlaceDef {
        let c = content();
        c.get(c.handle::<PlaceDef>(self.key()).expect("every kind of place is in the content (checked at load)"))
    }

    /// People (thousands).
    fn population(self) -> f64 {
        self.def().population
    }

    /// Fuel it keeps for the ships that call (t/day): sold to them (see
    /// `Markets::refuel`), not used by the place itself.
    pub fn ship_fuel(self) -> f64 {
        self.def().ship_fuel
    }

    /// Its works: (recipe, how many).
    fn works(self) -> impl Iterator<Item = (&'static Recipe, f64)> {
        self.def().works.iter().map(|&(r, n)| (content().get(r), n))
    }
}

/// A settled market's economy.
#[derive(Clone, Debug)]
pub struct Place {
    pub system: usize,
    pub facility: Facility,
    pub kind: PlaceKind,
    /// People (thousands); as founded.
    pub population: f64,
    pub founded: f64,
    /// How well fed (its food and water met, over the last few days, 0..1).
    pub fed: f64,
    /// Those waiting for passage away (thousands, of its people).
    pub waiting: f64,
    /// Over the last step, per day: how many more of its people (thousands,
    /// births less deaths), and how many died.
    pub growth: f64,
    pub deaths: f64,
    /// Stock by kind of goods (tonnes).
    pub stock: Vec<f64>,
    /// Over the last step, per day (tonnes): made, used (by works and
    /// people), and wanted but not there.
    pub made: Vec<f64>,
    pub used: Vec<f64>,
    pub short: Vec<f64>,
}

impl Place {
    fn new(system: usize, facility: Facility, kind: PlaceKind) -> Self {
        let mut p = Place {
            system,
            facility,
            kind,
            population: kind.population(),
            founded: kind.population(),
            fed: 1.0,
            waiting: 0.0,
            growth: 0.0,
            deaths: 0.0,
            stock: vec![0.0; lines()],
            made: vec![0.0; lines()],
            used: vec![0.0; lines()],
            short: vec![0.0; lines()],
        };
        // Starting at the stock it aims for, working at full: what it makes
        // and uses a day so (until its first step says otherwise).
        for c in Category::all() {
            p.stock[line(c)] = p.target(c);
            p.made[line(c)] = p.makes(c);
            p.used[line(c)] = p.needs(c) - if c == Category::fuel() { kind.ship_fuel() } else { 0.0 };
        }
        p
    }

    /// Its works' workforce against what they were founded with: fewer
    /// people, less work (more, up to twice as much).
    pub fn labour(&self) -> f64 {
        (self.population / self.founded.max(1e-9)).min(2.0)
    }

    /// What it uses of kind `c` in a day, at full work (tonnes).
    pub fn needs(&self, c: Category) -> f64 {
        let works: f64 = self.labour() * self.kind.works().flat_map(|(r, n)| r.takes.iter().filter(|t| t.0 == c).map(move |t| t.1 * n)).sum::<f64>();
        let ships = if c == Category::fuel() { self.kind.ship_fuel() } else { 0.0 };
        works + ships + c.basket() * self.population
    }

    /// What it makes of kind `c` in a day, at full work (tonnes).
    pub fn makes(&self, c: Category) -> f64 {
        self.labour() * self.kind.works().flat_map(|(r, n)| r.makes.iter().filter(|m| m.0 == c).map(move |m| m.1 * n)).sum::<f64>()
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

    /// What it can store of kind `c` (tonnes): `STORAGE` times its target,
    /// as built for its founding numbers (its warehouses don't shrink when
    /// its people do).
    pub fn storage(&self, c: Category) -> f64 {
        self.target(c) * STORAGE * (self.founded / self.population.max(1e-9)).max(1.0)
    }

    /// Room left for kind `c` (tonnes): what it would take in.
    pub fn room(&self, c: Category) -> f64 {
        (self.storage(c) - self.stock[line(c)]).max(0.0)
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
        let (mut made, mut used, mut short) = (vec![0.0; lines()], vec![0.0; lines()], vec![0.0; lines()]);
        let full: Vec<bool> = Category::all().map(|c| self.stock[line(c)] >= self.storage(c)).collect();
        let labour = self.labour();
        for (r, n) in self.kind.works() {
            let n = n * labour;
            // As much as its inputs allow; none while its outputs have nowhere to go.
            let mut k: f64 = if r.makes.iter().all(|m| full[line(m.0)]) { 0.0 } else { 1.0 };
            for &(c, rate) in &r.takes {
                k = k.min(self.stock[line(c)] / (rate * n * days));
            }
            for &(c, rate) in &r.takes {
                let t = rate * n * days * k;
                self.stock[line(c)] -= t;
                used[line(c)] += t;
                short[line(c)] += rate * n * days * (1.0 - k);
            }
            for &(c, rate) in &r.makes {
                let t = rate * n * days * k;
                // (What there's no room for is dumped.)
                let room = (self.storage(c) - self.stock[line(c)]).max(0.0);
                self.stock[line(c)] += t.min(room);
                made[line(c)] += t;
            }
        }
        // Its people's needs; the food and water of them, how well met.
        let (mut wanted, mut had) = (0.0, 0.0);
        let essential = [Category::of("goods.food"), Category::of("goods.water")];
        for c in Category::all() {
            let want = c.basket() * self.population * days;
            if want <= 0.0 {
                continue;
            }
            let got = want.min(self.stock[line(c)]);
            self.stock[line(c)] -= got;
            used[line(c)] += got;
            short[line(c)] += want - got;
            if essential.contains(&Some(c)) {
                wanted += want;
                had += got;
            }
        }
        self.live(if wanted > 0.0 { had / wanted } else { 1.0 }, days);
        for i in 0..lines() {
            self.made[i] = made[i] / days;
            self.used[i] = used[i] / days;
            self.short[i] = short[i] / days;
        }
    }
}

impl Place {
    /// `days` of its people's lives, their food and water met this well
    /// (0..1): fed, they grow; hungry, they want to leave (and wait for
    /// passage); starving, they die.
    fn live(&mut self, met: f64, days: f64) {
        self.fed += (met - self.fed) * (days / FED_DAYS).min(1.0);
        let before = self.population;
        if self.fed > 0.95 {
            self.population = (self.population * (1.0 + GROWTH * days)).min(self.founded * ROOM).max(self.population);
            // (Fed again: those waiting stay.)
            self.waiting *= (1.0 - days).max(0.0);
        } else if self.fed < 0.9 {
            let leaving = self.population * EMIGRATE * (0.9 - self.fed) / 0.9 * days;
            self.waiting = (self.waiting + leaving).min(self.population * WAITING_MOST);
        }
        let died = if self.fed < 0.5 { self.population * DEATH * (0.5 - self.fed) / 0.5 * days } else { 0.0 };
        if died > 0.0 {
            let left = (self.population - died).max(0.0);
            self.waiting *= left / self.population.max(1e-12);
            self.population = left;
        }
        if self.population < 0.001 {
            self.population = 0.0;
            self.waiting = 0.0;
        }
        self.growth = (self.population - before) / days;
        self.deaths = died / days;
    }

    /// `people` (thousands) of those waiting board a ship: gone from here.
    pub fn depart(&mut self, people: f64) -> f64 {
        let n = people.min(self.waiting).max(0.0);
        self.waiting -= n;
        self.population -= n;
        n
    }

    /// `people` (thousands) arrive to settle.
    pub fn arrive(&mut self, people: f64) {
        self.population += people.max(0.0);
    }

    /// Would people settle here (fed, with room)?
    pub fn welcomes(&self) -> bool {
        self.fed > 0.9 && self.population < self.founded * ROOM
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
    pub fn totals(&self) -> Vec<(f64, f64, f64, f64)> {
        let mut t = vec![(0.0, 0.0, 0.0, 0.0); lines()];
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

    fn kind(key: &str) -> Category {
        Category::of(key).unwrap()
    }

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
        assert_eq!(p.stock_of(kind("goods.food")), 0.0);
        assert!(p.short[line(kind("goods.food"))] > 0.0, "people go hungry");
        assert!(p.factor(kind("goods.food")).unwrap() > 2.0, "and food is dear there");
        assert_eq!(p.made[line(kind("goods.machinery"))], 0.0, "no metal, no machines");
        // Nothing runs away: every stock stays within its storage.
        for p in &e.places {
            for c in Category::all() {
                assert!(p.stock_of(c) <= p.storage(c) + 1e-6 && p.stock_of(c) >= 0.0);
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

#[cfg(test)]
mod people {
    use super::*;

    fn economy() -> Economy {
        let w = universe_world::World::new(1984);
        let sys = w.system(w.home_system);
        Economy::new(std::iter::once((w.home_system, &*sys)), 0.0)
    }

    #[test]
    fn unsupplied_people_go_hungry_queue_to_leave_and_then_die_while_the_fed_grow() {
        let mut e = economy();
        let station = e.places.iter().position(|p| p.kind == PlaceKind::Station).unwrap();
        let farm = e.places.iter().position(|p| p.kind == PlaceKind::Farm).unwrap();
        let (people, farmers) = (e.places[station].population, e.places[farm].population);
        // Ten days: the station's food lasts (it starts with ten days of it).
        e.step_to(8.0 * DAY);
        assert!(e.places[station].fed > 0.95 && e.places[station].waiting == 0.0);
        // A month with nothing delivered: hungry, then starving.
        e.step_to(30.0 * DAY);
        let s = &e.places[station];
        eprintln!("station after a month unsupplied: fed {:.2}, {:.1}k of {people:.1}k left, {:.1}k waiting, {:.2}k dying a day", s.fed, s.population, s.waiting, s.deaths);
        assert!(s.fed < 0.5, "starving: fed {}", s.fed);
        assert!(s.waiting > 0.0, "people want to leave");
        assert!(s.population < people, "people died");
        // The farm feeds itself: it grows.
        assert!(e.places[farm].population > farmers, "the farm world grows: {} -> {}", farmers, e.places[farm].population);
        // Those waiting board a ship and settle at the farm.
        let gone = e.places[station].depart(1.0);
        assert!(gone > 0.0 && e.places[farm].welcomes());
        e.places[farm].arrive(gone);
    }
}
