//! The economy, on stock (`docs/economy-stock.md`): only what the registry
//! describes runs. Each facility a settlement has is one pool of stock: its
//! modules, each set to one of its recipes (its setup), take their inputs
//! from the pool and put what they make back, as far as the pool holds what
//! they take, the settlement's power stations supply them, and its storing
//! modules have room. A full store stops what fills it. Between facilities,
//! the market: the settlement's warehouse, which the exchange has approved.
//! Each facility's owner sells it what it makes and doesn't use, and buys
//! from it what it needs and doesn't make. The exchange makes a market in any
//! stock while its warehouse has room. Nothing moves between settlements but
//! by ship.
//!
//! Every `STEP` the facilities run. Prices are the game's own: an item's
//! reference price (see `goods`), moved by how the warehouse's stock of it
//! stands against what the settlement's works take of it over `COVER_DAYS`.
//!
//! A settlement's people: as many as the registry says live there, living by
//! their needs (`need.*`): what each person takes a second, from its
//! warehouse (a whole market category, such as food, as any stock of it), and
//! gives back (waste water, breath). Where a world's air is breathable they
//! breathe it free. A need unmet long enough (its record's `lasts`) kills: past
//! it, people die at the rate it goes unmet. Hungry, thirsty or short of
//! air, some queue to leave. Needs that take no stock (a home, safety, news)
//! aren't run here.

use std::collections::{BTreeMap, HashMap};

use universe_world::goods::{Item, POWER_PRICE};
use universe_world::recipes::Recipe;
use universe_world::registry::Module;
use universe_world::traffic::Facility;
use universe_world::units::DAY;

use crate::land::{LandOffice, Run};
use crate::ledger::{Asset, Ledger, Party};

/// The economy steps this often (game s).
pub const STEP: f64 = 600.0;
/// A works keeps this many days of what it takes; a market prices against as much.
pub const COVER_DAYS: f64 = 10.0;
/// A settlement welcomes newcomers up to this many times its founding size.
pub const ROOM: f64 = 3.0;
/// A maker of a market sells at this over its price, and buys at this under it.
const ASK: f64 = 1.05;
const BID: f64 = 0.85;
/// What the exchange pays for what no one here takes, against its
/// reference price, with its warehouse empty (less as it fills). Invented.
const SPECULATE: f64 = 0.5;
/// A works with no store keeps this long of what it takes (s).
const UNSTORED: f64 = DAY;

/// Stock lying in one place (kg, by item).
#[derive(Clone, Debug, Default)]
pub struct Pool {
    pub stock: BTreeMap<usize, f64>,
    /// What it can hold (kg).
    pub room: f64,
}

impl Pool {
    pub fn of(&self, item: usize) -> f64 {
        self.stock.get(&item).copied().unwrap_or(0.0)
    }

    pub fn total(&self) -> f64 {
        self.stock.values().sum()
    }

    /// Room left (kg).
    pub fn free(&self) -> f64 {
        (self.room - self.total()).max(0.0)
    }

    pub fn put(&mut self, item: usize, kg: f64) {
        if kg > 0.0 {
            *self.stock.entry(item).or_default() += kg;
        }
    }

    /// Take up to `kg`; what was taken.
    pub fn take(&mut self, item: usize, kg: f64) -> f64 {
        let Some(have) = self.stock.get_mut(&item) else {
            return 0.0;
        };
        let t = kg.min(*have).max(0.0);
        *have -= t;
        if *have <= 1e-9 {
            self.stock.remove(&item);
        }
        t
    }
}

/// A module (`count` of them) and what it's set to: one of its recipes (by
/// its place in `recipes::of` the module), or none.
#[derive(Clone, Debug)]
pub struct Setup {
    pub module: &'static Module,
    pub count: u32,
    pub recipe: Option<usize>,
}

impl Setup {
    pub fn recipe(&self) -> Option<&'static Recipe> {
        self.recipe.and_then(|r| universe_world::recipes::of(&self.module.identity.key).get(r))
    }
}

/// A facility as the economy runs it.
#[derive(Clone, Debug)]
pub struct Works {
    /// Where it stands, and which works there (a ground's: its land office works).
    pub site: Site,
    pub works: usize,
    pub name: String,
    pub setups: Vec<Setup>,
    pub pool: Pool,
    /// Does the exchange keep its market here?
    pub exchange: bool,
    /// A rig's owner (its record's): a ground's works are their parcel's.
    pub owner: Option<String>,
    /// How it ran over the last step.
    pub last: Option<Run>,
}

/// Where works stand: a land office's ground, or a rig (its system and its
/// body there, see `world::rigs`), which has no ground, parcels or zoning.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Site {
    Ground(usize),
    Rig(usize, usize),
}

impl Works {
    /// Built as the registry's facility `key`, each line's modules set to
    /// the recipes that lead to what it makes.
    fn new(ground: usize, works: usize, key: &str) -> Option<Self> {
        let reg = universe_world::registry::registry();
        let f = reg.facilities.iter().find(|f| f.identity.key == key)?;
        Self::built(Site::Ground(ground), works, &f.identity.name, &f.lines, &f.modules, &f.stock, f.exchange.is_some())
    }

    /// Built as the registry's rig `key`, body `body` of `system`: its own warehouse, its owner's.
    fn rig(system: usize, body: usize, key: &str) -> Option<Self> {
        let reg = universe_world::registry::registry();
        let r = reg.settlements.iter().find(|s| s.identity.key == key)?;
        let mut w = Self::built(Site::Rig(system, body), 0, &r.identity.name, &r.lines, &r.modules, &[], true)?;
        w.owner = r.owner.clone();
        Some(w)
    }

    fn built(site: Site, works: usize, name: &str, lines: &[universe_world::registry::Line], modules: &[universe_world::registry::ModuleEntry], stock: &[universe_world::registry::StockItem], exchange: bool) -> Option<Self> {
        let reg = universe_world::registry::registry();
        let module = |k: &str| reg.module(k);
        let mut setups = Vec::new();
        for line in lines {
            let steps: Vec<&str> = line.modules.iter().map(|m| m.module.as_str()).collect();
            let chosen = line.makes.as_deref().and_then(|t| universe_world::settlements::route(reg, &steps, t).ok()).unwrap_or_else(|| vec![None; steps.len()]);
            for (m, r) in line.modules.iter().zip(chosen) {
                // (The registry's recipe, as the engine has it.)
                let r = r.and_then(|k| universe_world::recipes::of(&m.module).iter().position(|x| x.index == Some(k)));
                setups.push(Setup { module: module(&m.module)?, count: m.count, recipe: r });
            }
        }
        for m in modules {
            setups.push(Setup { module: module(&m.module)?, count: m.count, recipe: None });
        }
        let holds: f64 = setups.iter().map(|s| s.module.capacity.holds.unwrap_or(0.0) * s.count as f64).sum();
        let mut w = Works { site, works, name: name.to_string(), setups, pool: Pool::default(), exchange, owner: None, last: None };
        w.pool.room = if holds > 0.0 { holds } else { w.takes().iter().map(|(_, r)| r * UNSTORED).sum() };
        // (What lies in it at day 0: the registry's seed state.)
        let goods = &universe_world::content::content().stock;
        for s in stock {
            if let Some(i) = universe_world::goods::item(&s.item) {
                w.pool.put(i, s.quantity.unwrap_or(0.0) + s.pieces.map_or(0.0, |n| n as f64 * goods[i].mass));
            }
        }
        Some(w)
    }

    /// What it takes at full rate (item, kg/s): its recipes' inputs, and
    /// what its power stations burn (as any stock of the material).
    pub fn takes(&self) -> Vec<(usize, f64)> {
        let mut out: Vec<(usize, f64)> = Vec::new();
        let mut add = |item: usize, r: f64| match out.iter_mut().find(|(i, _)| *i == item) {
            Some(e) => e.1 += r,
            None => out.push((item, r)),
        };
        for s in &self.setups {
            if let Some(r) = s.recipe() {
                let rate = r.rate * s.count as f64;
                for &(i, q) in &r.inputs {
                    add(i, rate * q);
                }
            }
            for b in s.module.generation.iter().flat_map(|g| &g.burns) {
                if let Some(i) = burnable(&b.item).first() {
                    add(*i, b.rate * s.count as f64);
                }
            }
        }
        out
    }

    /// Does it take `item` (as an input or a fuel)?
    #[cfg(test)]
    fn uses(&self, item: usize) -> bool {
        self.takes().iter().any(|(i, _)| *i == item)
    }

    /// Is `item` for sale from here: something its modules make (as they're
    /// set) that none of them could take in, set to anything? (What it was
    /// given or bought to make things with, and what one of its modules
    /// could build on, it keeps.)
    fn sells(&self, item: usize) -> bool {
        let makes = self.setups.iter().filter_map(Setup::recipe).any(|r| r.makes == item || r.outputs.iter().any(|o| o.0 == item));
        makes && !self.setups.iter().any(|s| universe_world::recipes::of(&s.module.identity.key).iter().any(|r| r.inputs.iter().any(|x| x.0 == item)))
    }

    /// The power it supplies at full output (W).
    fn supplies(&self) -> f64 {
        self.setups.iter().map(|s| s.module.generation.as_ref().map_or(0.0, |g| g.supplies) * s.count as f64).sum()
    }

    /// Of that, the share its fuel in store lets it supply for `dt`.
    fn fuelled(&self, dt: f64) -> f64 {
        let mut k: f64 = 1.0;
        for s in &self.setups {
            for b in s.module.generation.iter().flat_map(|g| &g.burns) {
                let have: f64 = burnable(&b.item).iter().map(|&i| self.pool.of(i)).sum();
                k = k.min(have / (b.rate * s.count as f64 * dt).max(1e-12));
            }
        }
        k
    }

    /// The power it would draw this step at full rate (W): a module set to a
    /// recipe, if its store holds something of each of its inputs (an idle one
    /// asks for none); one with no recipe, what it draws all the time.
    fn draws(&self) -> f64 {
        let draw = |s: &Setup| match s.recipe() {
            Some(r) if r.inputs.iter().all(|&(i, _)| self.pool.of(i) > 0.0) => r.power,
            Some(_) => 0.0,
            None => s.module.needs.power.unwrap_or(0.0),
        };
        self.setups.iter().map(|s| draw(s) * s.count as f64).sum()
    }
}

/// The stock items of `key`: an item itself, or a material as any stock made from it.
fn burnable(key: &str) -> Vec<usize> {
    if let Some(i) = universe_world::goods::item(key)
        && !key.starts_with("material.")
    {
        return vec![i];
    }
    let reg = universe_world::registry::registry();
    let mut v: Vec<usize> = reg.stock.iter().filter(|s| s.made_from.iter().any(|m| m.item == key)).filter_map(|s| universe_world::goods::item(&s.identity.key)).collect();
    v.extend(universe_world::goods::item(key));
    v
}

/// A settlement's market: what lies in its warehouse, and its people.
#[derive(Clone, Debug)]
pub struct Place {
    pub system: usize,
    pub facility: Facility,
    pub name: String,
    /// Where it is: its land office's ground, or a rig.
    pub site: Site,
    /// Who trades at its market: the exchange (`Party::Market`), or a rig's owner at its dock.
    pub trader: Party,
    /// The share of every sale there owed to the administration as duty (its system's law; 0: none).
    pub duty: f64,
    /// The warehouse the exchange keeps its market in (its works), and that
    /// warehouse's stock as of the last step.
    pub warehouse: Option<usize>,
    pub stock: Pool,
    /// What its works take at full rate (item, kg/s): what the market is priced against.
    pub wants: Vec<(usize, f64)>,
    /// People (thousands); as founded.
    pub population: f64,
    pub founded: f64,
    /// How well fed (0..1).
    pub fed: f64,
    /// Those waiting for passage away (thousands).
    pub waiting: f64,
    /// Over the last step, per day (kg): made and used by its works.
    pub made: BTreeMap<usize, f64>,
    pub used: BTreeMap<usize, f64>,
    /// Its world's air is breathable: it breathes it free.
    pub breathes: bool,
    /// Each need it lives by, by key: how far it was met over the last step
    /// (0..1), and how long it has gone short (s, as if wholly unmet).
    pub needs: BTreeMap<String, (f64, f64)>,
    /// Over the last step, per day: people died (thousands).
    pub deaths: f64,
    /// What its market aims to hold for its people, by the administration's
    /// stocking law: for each need's take, a stock item or a market category
    /// (None: an item, `.1`), and the kg it aims at: its warehouse's cover of
    /// resupply cycles, and its share of the law's reserve.
    pub people_wants: Vec<(Option<universe_world::goods::Category>, usize, f64)>,
}

/// One item as a market stands on it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Price {
    /// What you pay a unit (None: none in stock), and what it pays you.
    pub ask: Option<f64>,
    pub bid: f64,
    /// Units in stock, and units it would take (its room): tonnes of bulk
    /// stock, pieces of parts, products and hulls.
    pub stock: f64,
    pub room: f64,
    /// Do its works take it?
    pub wanted: bool,
}

impl Place {
    /// What its works take of `item` a day at full rate (kg).
    pub fn need(&self, item: usize) -> f64 {
        self.wants.iter().find(|(i, _)| *i == item).map_or(0.0, |(_, r)| r * DAY)
    }

    /// How the market stands on `item` (of the catalogue `goods`).
    pub fn price(&self, item: &Item) -> Price {
        // (What its people's stocking law aims at for it, against what it holds of that: a whole
        // category's stock where the aim is a category's.)
        let people = self.people_wants.iter().find(|(c, i, _)| match c {
            Some(c) => item.category == Some(*c),
            None => *i == item.id,
        });
        let (have, want) = match people {
            Some((Some(c), _, aim)) => (self.stock.stock.iter().filter(|(i, _)| universe_world::content::content().stock[**i].category == Some(*c)).map(|(_, kg)| kg).sum::<f64>(), *aim + self.need(item.id) * COVER_DAYS),
            Some((None, _, aim)) => (self.stock.of(item.id), *aim + self.need(item.id) * COVER_DAYS),
            None => (self.stock.of(item.id), self.need(item.id) * COVER_DAYS),
        };
        let have_it = self.stock.of(item.id);
        let (stock, room) = (have_it / item.mass, self.stock.free() / item.mass);
        if want > 0.0 {
            let factor = (want / have.max(want * 0.05)).powf(0.6).clamp(0.4, 3.0);
            let p = item.price * factor;
            Price { ask: (stock >= 1.0 - 1e-9).then_some(p * ASK), bid: p * BID, stock, room, wanted: true }
        } else {
            let fill = if self.stock.room > 0.0 { self.stock.free() / self.stock.room } else { 0.0 };
            Price { ask: (stock >= 1.0 - 1e-9).then_some(item.price * ASK), bid: item.price * SPECULATE * fill, stock, room, wanted: false }
        }
    }

    pub fn depart(&mut self, people: f64) -> f64 {
        let n = people.min(self.waiting).max(0.0);
        self.waiting -= n;
        self.population -= n;
        n
    }

    pub fn arrive(&mut self, people: f64) {
        self.population += people.max(0.0);
    }

    pub fn welcomes(&self) -> bool {
        self.fed > 0.9 && self.population < self.founded * ROOM
    }
}

/// The duty on sales in the system named `system`: its law's (0: no law, or none levied).
fn duty_in(system: &str) -> f64 {
    universe_world::order::law(system).and_then(|l| l.policies.duty).unwrap_or(0.0)
}

#[derive(Clone, Debug, Default)]
pub struct Economy {
    pub places: Vec<Place>,
    pub works: Vec<Works>,
    index: HashMap<(usize, Facility), usize>,
    pub stepped_to: f64,
    snapshot: Option<std::sync::Arc<Vec<Place>>>,
    works_snap: Option<std::sync::Arc<Vec<Works>>>,
}

impl Economy {
    /// The settlements the land office has ground for, and their facilities.
    pub fn new(land: &LandOffice, now: f64) -> Self {
        let mut e = Economy { stepped_to: now, ..Default::default() };
        for (k, g) in land.grounds.iter().enumerate() {
            let facility = Facility::Spaceport(g.port);
            e.index.insert((g.system, facility), e.places.len());
            // (Its people, as the registry has them: thousands.)
            let reg = universe_world::registry::registry();
            let record = reg.settlements.iter().find(|s| s.identity.name.eq_ignore_ascii_case(&g.recorded.name));
            let people = record.and_then(|s| s.population).map_or(0.0, |n| n as f64 / 1000.0);
            let breathes = record.and_then(|s| reg.bodies.iter().find(|b| Some(&b.identity.key) == s.at.as_ref())).is_some_and(|b| b.atmosphere.breathable == Some(true));
            e.places.push(Place {
                system: g.system,
                facility,
                name: g.recorded.name.clone(),
                site: Site::Ground(k),
                trader: Party::Market(g.system, facility),
                duty: duty_in(&g.recorded.system),
                warehouse: None,
                stock: Pool::default(),
                wants: Vec::new(),
                population: people,
                founded: people,
                fed: 1.0,
                waiting: 0.0,
                made: BTreeMap::new(),
                used: BTreeMap::new(),
                breathes,
                needs: BTreeMap::new(),
                deaths: 0.0,
                people_wants: Vec::new(),
            });
        }
        e.sync(land);
        e
    }

    /// The registry's rig `key`, body `body` of `system` (see `world::rigs`):
    /// a place with no people whose market is its own works, where its owner
    /// trades at its dock (its owner a company of the land office's from now).
    pub fn add_rig(&mut self, land: &mut LandOffice, system: usize, system_name: &str, body: usize, key: &str) {
        let facility = Facility::Rig(body);
        if self.index.contains_key(&(system, facility)) {
            return;
        }
        let Some(w) = Works::rig(system, body, key) else { return };
        let trader = w.owner.as_deref().map_or(Party::Market(system, facility), |o| land.enlist(o));
        self.index.insert((system, facility), self.places.len());
        self.places.push(Place {
            system,
            facility,
            name: w.name.clone(),
            site: Site::Rig(system, body),
            trader,
            duty: duty_in(system_name),
            warehouse: None,
            stock: Pool::default(),
            wants: Vec::new(),
            population: 0.0,
            founded: 0.0,
            fed: 1.0,
            waiting: 0.0,
            made: BTreeMap::new(),
            used: BTreeMap::new(),
            breathes: false,
            needs: BTreeMap::new(),
            deaths: 0.0,
            people_wants: Vec::new(),
        });
        self.works.push(w);
        self.sync_wants();
    }

    /// Facilities built since: into the economy, each as its blueprint.
    fn sync(&mut self, land: &LandOffice) {
        for (k, g) in land.grounds.iter().enumerate() {
            for (j, w) in g.works.iter().enumerate() {
                if self.works.iter().any(|x| x.site == Site::Ground(k) && x.works == j) {
                    continue;
                }
                let key = g.recorded.facilities.iter().find(|f| f.name.eq_ignore_ascii_case(&w.blueprint)).map(|f| f.key.clone()).or_else(|| crate::land::blueprint_key(&w.blueprint));
                if let Some(x) = key.and_then(|key| Works::new(k, j, &key)) {
                    self.works.push(x);
                }
            }
        }
        self.sync_wants();
    }

    /// Each market's warehouse, and what its works take.
    fn sync_wants(&mut self) {
        for p in &mut self.places {
            p.warehouse = self.works.iter().position(|w| w.site == p.site && w.exchange);
            let mut wants: Vec<(usize, f64)> = Vec::new();
            for w in self.works.iter().filter(|w| w.site == p.site) {
                for (i, r) in w.takes() {
                    match wants.iter_mut().find(|(x, _)| *x == i) {
                        Some(e) => e.1 += r,
                        None => wants.push((i, r)),
                    }
                }
            }
            p.wants = wants;
            if let Some(h) = p.warehouse {
                p.stock = self.works[h].pool.clone();
            }
        }
        self.snapshot = None;
        self.works_snap = None;
    }

    /// What each market aims to hold for its people (`Place::people_wants`), by
    /// its administration's stocking law (`compulsory_stock`): for each need
    /// people take stock for, `warehouse_cover` of its resupply interval
    /// (`settlement.resupply`; `COVER_DAYS` where none is said) of what its
    /// people take, and, where the law holds a reserve of that need, its share
    /// of what all the system's people would take over the reserve's time.
    fn people_aims(&mut self) {
        let reg = universe_world::registry::registry();
        let needs: Vec<&universe_world::registry::Need> = reg.needs.iter().filter(|n| !n.takes.is_empty()).collect();
        // (Each system's people, for its reserves.)
        let mut people: HashMap<usize, f64> = HashMap::new();
        for p in &self.places {
            *people.entry(p.system).or_default() += p.population * 1000.0;
        }
        for k in 0..self.places.len() {
            let p = &self.places[k];
            let record = reg.settlements.iter().find(|s| s.identity.name.eq_ignore_ascii_case(&p.name));
            let law = record.and_then(|s| s.identity.key.split('.').nth(1)).and_then(|system| reg.orgs.iter().find(|o| o.compulsory_stock.is_some() && o.identity.key.split('.').nth(1) == Some(system))).and_then(|o| o.compulsory_stock.as_ref());
            let cover = law.and_then(|l| l.warehouse_cover).unwrap_or(1.5) * record.and_then(|s| s.resupply.as_ref()).map_or(COVER_DAYS * DAY, |r| r.interval);
            let mut wants = Vec::new();
            for need in &needs {
                if p.breathes && need.identity.key == "need.air" {
                    continue;
                }
                let reserve = law.and_then(|l| l.reserves.iter().find(|r| r.need == need.identity.key)).and_then(|r| Some((r.lasts, r.held_at.iter().find(|h| Some(&h.settlement) == record.map(|s| &s.identity.key))?.share)));
                for t in &need.takes {
                    let mine = t.rate * p.population * 1000.0 * cover;
                    let held = reserve.map_or(0.0, |(lasts, share)| t.rate * people.get(&p.system).copied().unwrap_or(0.0) * lasts * share);
                    let aim = mine + held;
                    match t.item.strip_prefix("market.").and_then(|_| universe_world::goods::Category::of(&t.item)) {
                        Some(c) => wants.push((Some(c), usize::MAX, aim)),
                        None => {
                            if let Some(i) = universe_world::goods::item(&t.item) {
                                wants.push((None, i, aim));
                            }
                        }
                    }
                }
            }
            self.places[k].people_wants = wants;
        }
    }

    /// A step of place `p`'s people: each need's takes from its warehouse (into
    /// `used`), its gives back into it, how far each was met; then deaths
    /// where a need has gone short past what it lasts, and the queue to leave.
    fn live(&mut self, p: usize, used: &mut BTreeMap<usize, f64>, goods: &[Item]) {
        let dt = STEP;
        let people = self.places[p].population * 1000.0;
        if people <= 0.0 {
            return;
        }
        let Some(h) = self.places[p].warehouse else { return };
        let site = self.places[p].site;
        // (Its stores: its works', the life support and the water works first, the warehouse last.)
        let mut stores: Vec<usize> = (0..self.works.len()).filter(|&k| self.works[k].site == site && k != h).collect();
        stores.push(h);
        let reg = universe_world::registry::registry();
        let mut dying: f64 = 0.0;
        let mut worst: f64 = 1.0;
        for need in reg.needs.iter().filter(|n| !n.takes.is_empty()) {
            let key = &need.identity.key;
            let mut met: f64 = 1.0;
            for t in &need.takes {
                // (Air, where the world's is breathable: breathed free.)
                if self.places[p].breathes && key == "need.air" {
                    continue;
                }
                let want = t.rate * people * dt;
                let mut got = 0.0;
                let category = t.item.strip_prefix("market.").and_then(|_| universe_world::goods::Category::of(&t.item));
                let one = universe_world::goods::item(&t.item);
                for &k in &stores {
                    if got >= want {
                        break;
                    }
                    let pool = &mut self.works[k].pool;
                    // (A market category: any stock of it, what there's most of first.)
                    let mut of: Vec<(usize, f64)> = match category {
                        Some(c) => pool.stock.iter().filter(|(i, _)| goods[**i].category == Some(c)).map(|(i, kg)| (*i, *kg)).collect(),
                        None => one.map(|i| (i, pool.of(i))).into_iter().collect(),
                    };
                    of.sort_by(|a, b| b.1.total_cmp(&a.1));
                    for (i, _) in of {
                        if got >= want {
                            break;
                        }
                        let took = pool.take(i, want - got);
                        *used.entry(i).or_default() += took;
                        got += took;
                    }
                }
                met = met.min(if want > 0.0 { got / want } else { 1.0 });
            }
            // What it gives back, as far as it was met: to a works that takes it (the water
            // works, used water), else the warehouse, as far as there's room.
            for g in &need.gives {
                if let Some(i) = universe_world::goods::item(&g.item) {
                    let k = stores.iter().copied().find(|&k| self.works[k].takes().iter().any(|t| t.0 == i)).unwrap_or(h);
                    let pool = &mut self.works[k].pool;
                    let kg = (g.rate * people * dt * met).min(pool.free());
                    pool.put(i, kg);
                }
            }
            let lasts = need.lasts.unwrap_or(f64::INFINITY);
            let short = self.places[p].needs.get(key).map_or(0.0, |s| s.1);
            // (Short: the time it went unmet adds up; met, it heals as fast.)
            let short = (short + dt * (1.0 - met) - dt * met).clamp(0.0, lasts.min(1e12));
            self.places[p].needs.insert(key.clone(), (met, short));
            let alive = need.identity.rung.as_str() == "alive";
            if alive {
                worst = worst.min(met);
                if short >= lasts {
                    // Past what it lasts: they die as fast as it goes unmet.
                    dying = dying.max((1.0 - met) * dt / lasts);
                }
            }
        }
        let place = &mut self.places[p];
        let died = place.population * dying.min(1.0);
        place.population -= died;
        place.deaths = died / (dt / DAY);
        place.fed = worst;
        // (Short of what keeps them alive, some want to leave: up to a third.)
        place.waiting = (place.population * (1.0 - worst) / 3.0).max(place.waiting.min(place.population));
        place.stock = self.works[h].pool.clone();
    }

    /// Set module `setup` of works `works` to `recipe` (its place in
    /// `recipes::of` the module; None: to nothing). Its owner's to choose, a
    /// player's or a company's alike: anyone else is refused. (What a
    /// changeover costs isn't described yet: it is at once.)
    pub fn set_up(&mut self, land: &LandOffice, works: usize, setup: usize, recipe: Option<usize>, by: Party) -> Result<(), String> {
        if self.owner(land, works) != Some(by) {
            return Err("NOT YOURS TO SET".into());
        }
        let w = self.works.get(works).ok_or("NO SUCH WORKS")?;
        let s = w.setups.get(setup).ok_or("NO SUCH MODULE")?;
        if recipe.is_some_and(|r| r >= universe_world::recipes::of(&s.module.identity.key).len()) {
            return Err(format!("{} CAN'T BE SET TO THAT", s.module.identity.name.to_uppercase()));
        }
        self.works[works].setups[setup].recipe = recipe;
        self.sync_wants();
        Ok(())
    }

    /// Who owns works `works`: its parcel's owner, or a rig's.
    pub fn owner(&self, land: &LandOffice, works: usize) -> Option<Party> {
        let w = self.works.get(works)?;
        match w.site {
            Site::Ground(k) => {
                let g = &land.grounds[k];
                g.works.get(w.works).and_then(|x| g.lots.iter().find(|l| l.number == x.parcel)).and_then(|l| land.party(&l.owner))
            }
            Site::Rig(..) => land.companies.iter().position(|c| Some(&c.0) == w.owner.as_ref()).map(|i| Party::Company(i as u32)),
        }
    }

    /// Is works `works` standing at `at`? (A rig is.)
    fn built(&self, land: &LandOffice, works: usize, at: f64) -> bool {
        let w = &self.works[works];
        match w.site {
            Site::Ground(k) => land.grounds[k].works.get(w.works).is_some_and(|x| x.built(at)),
            Site::Rig(..) => true,
        }
    }

    pub fn place(&self, system: usize, f: Facility) -> Option<&Place> {
        self.index.get(&(system, f)).map(|&i| &self.places[i])
    }

    pub fn place_mut(&mut self, system: usize, f: Facility) -> Option<&mut Place> {
        self.snapshot = None;
        self.works_snap = None;
        self.index.get(&(system, f)).map(|&i| &mut self.places[i])
    }

    /// The works as they stand (shared: copied when they've changed).
    pub fn works_snapshot(&mut self) -> std::sync::Arc<Vec<Works>> {
        self.works_snap.get_or_insert_with(|| std::sync::Arc::new(self.works.clone())).clone()
    }

    pub fn snapshot(&mut self) -> std::sync::Arc<Vec<Place>> {
        self.snapshot.get_or_insert_with(|| std::sync::Arc::new(self.places.clone())).clone()
    }

    /// The market at (`system`, `f`): its place and its warehouse's pool, if it has one.
    fn market_mut(&mut self, system: usize, f: Facility) -> Option<(&mut Place, &mut Pool)> {
        let i = *self.index.get(&(system, f))?;
        let h = self.places[i].warehouse?;
        self.snapshot = None;
        self.works_snap = None;
        Some((&mut self.places[i], &mut self.works[h].pool))
    }

    /// Put `kg` of `item` into the market at (`system`, `f`) (sold to it), as far as it has room; what went in.
    pub fn put(&mut self, system: usize, f: Facility, item: usize, kg: f64) -> f64 {
        let Some((p, pool)) = self.market_mut(system, f) else {
            return 0.0;
        };
        let t = kg.min(pool.free()).max(0.0);
        pool.put(item, t);
        p.stock = pool.clone();
        t
    }

    /// Take up to `kg` of `item` out of the market at (`system`, `f`) (bought from it); what came out.
    pub fn take(&mut self, system: usize, f: Facility, item: usize, kg: f64) -> f64 {
        let Some((p, pool)) = self.market_mut(system, f) else {
            return 0.0;
        };
        let t = pool.take(item, kg);
        p.stock = pool.clone();
        t
    }

    /// Run every facility up to world time `now`, a `STEP` at a time: each
    /// settlement's power shared out among what draws it, each module as far
    /// as its inputs, power and room let it; then each owner trading with the
    /// settlement's market. How each ran goes to its land office works.
    pub fn step_to(&mut self, now: f64, land: &mut LandOffice, ledger: &mut Ledger, goods: &[Item], tick: u64) {
        if self.stepped_to + STEP > now {
            return;
        }
        self.sync(land);
        while self.stepped_to + STEP <= now {
            self.stepped_to += STEP;
            let at = self.stepped_to;
            self.people_aims();
            for p in 0..self.places.len() {
                self.run_place(p, at, land, ledger, goods, tick);
            }
        }
        self.snapshot = None;
        self.works_snap = None;
    }

    fn run_place(&mut self, p: usize, at: f64, land: &mut LandOffice, ledger: &mut Ledger, goods: &[Item], tick: u64) {
        let dt = STEP;
        let (site, system, duty) = (self.places[p].site, self.places[p].system, self.places[p].duty);
        let market = self.places[p].trader;
        let cause = universe_protocol::Cause::Rules;
        let here: Vec<usize> = (0..self.works.len()).filter(|&k| self.works[k].site == site && self.built(land, k, at)).collect();
        let owners: Vec<Option<Party>> = here.iter().map(|&k| self.owner(land, k)).collect();
        // Power: what the stations can supply with the fuel they hold, shared out.
        let supply: f64 = here.iter().map(|&k| self.works[k].supplies() * self.works[k].fuelled(dt).min(1.0)).sum();
        let demand: f64 = here.iter().map(|&k| self.works[k].draws()).sum();
        let share = if demand > 0.0 { (supply / demand).min(1.0) } else { 1.0 };
        let mut runs: Vec<Run> = vec![Run::default(); here.len()];
        let (mut made, mut used): (BTreeMap<usize, f64>, BTreeMap<usize, f64>) = Default::default();
        let mut drawn = vec![0.0; here.len()];
        for (n, &k) in here.iter().enumerate() {
            let w = &mut self.works[k];
            let mut most: f64 = 0.0;
            let mut ran = 0.0;
            let mut held: Option<String> = None;
            for s in 0..w.setups.len() {
                let setup = w.setups[s].clone();
                let Some(r) = setup.recipe() else {
                    drawn[n] += setup.module.needs.power.unwrap_or(0.0) * setup.count as f64 * share;
                    continue;
                };
                let rate = r.rate * setup.count as f64;
                if rate <= 0.0 {
                    continue;
                }
                let full = rate * dt;
                let mut k = share;
                let mut why = (share < 1.0).then(|| "POWER".to_string());
                for &(i, q) in &r.inputs {
                    let can = w.pool.of(i) / (q * full).max(1e-12);
                    if can < k {
                        k = can;
                        why = Some(goods[i].name.to_uppercase());
                    }
                }
                let out: Vec<(usize, f64)> = std::iter::once((r.makes, 1.0)).chain(r.outputs.iter().copied()).collect();
                let grows = out.iter().map(|o| o.1).sum::<f64>() - r.inputs.iter().map(|o| o.1).sum::<f64>();
                if grows > 0.0 {
                    let can = w.pool.free() / (grows * full);
                    if can < k {
                        k = can;
                        why = Some("STORE FULL".into());
                    }
                }
                let k = k.clamp(0.0, 1.0);
                for &(i, q) in &r.inputs {
                    let t = w.pool.take(i, q * full * k);
                    *used.entry(i).or_default() += t;
                }
                for (i, q) in out {
                    w.pool.put(i, q * full * k);
                    *made.entry(i).or_default() += q * full * k;
                }
                drawn[n] += r.power * setup.count as f64 * k;
                most += 1.0;
                ran += k;
                if k < 1.0 && held.is_none() {
                    held = why;
                }
            }
            runs[n] = Run { rate: if most > 0.0 { ran / most } else { 1.0 }, held_by: held, earned: 0.0 };
        }
        // The stations burn for what was drawn, and are paid for it by what drew it.
        let used_w: f64 = drawn.iter().sum();
        for (n, &k) in here.iter().enumerate() {
            let s = self.works[k].supplies() * self.works[k].fuelled(dt).min(1.0);
            if s <= 0.0 {
                continue;
            }
            let part = s / supply.max(1e-9);
            let out = (used_w * part / self.works[k].supplies()).min(1.0);
            let setups = self.works[k].setups.clone();
            for st in &setups {
                for b in st.module.generation.iter().flat_map(|g| &g.burns) {
                    let mut left = b.rate * st.count as f64 * out * dt;
                    for i in burnable(&b.item) {
                        let t = self.works[k].pool.take(i, left);
                        *used.entry(i).or_default() += t;
                        left -= t;
                    }
                }
            }
            runs[n].rate = out;
            if let Some(seller) = owners[n] {
                for (m, w) in drawn.iter().enumerate() {
                    if let Some(payer) = owners[m]
                        && payer != seller
                    {
                        let bill = w * part * dt / 3.6e9 * POWER_PRICE;
                        let _ = ledger.transfer(payer, seller, Asset::Credits, bill, tick, cause);
                        runs[m].earned -= bill;
                        runs[n].earned += bill;
                    }
                }
            }
        }
        // Each owner and the market: what it makes and doesn't use, sold; what it takes, bought to cover.
        if let Some(h) = self.places[p].warehouse {
            self.places[p].stock = self.works[h].pool.clone();
            for (n, &k) in here.iter().enumerate() {
                let Some(owner) = owners[n] else { continue };
                if k == h {
                    continue;
                }
                let spare: Vec<(usize, f64)> = self.works[k].pool.stock.iter().filter(|(i, _)| self.works[k].sells(**i)).map(|(i, kg)| (*i, *kg)).collect();
                for (i, kg) in spare {
                    let price = self.places[p].price(&goods[i]);
                    let t = kg.min(self.works[h].pool.free());
                    if t <= 0.0 || price.bid <= 0.0 {
                        continue;
                    }
                    self.works[k].pool.take(i, t);
                    self.works[h].pool.put(i, t);
                    let paid = t / 1000.0 * price.bid;
                    let _ = ledger.transfer(market, owner, Asset::Credits, paid, tick, cause);
                    let _ = ledger.transfer(owner, Party::Administration(system), Asset::Credits, paid * duty, tick, cause);
                    runs[n].earned += paid;
                    self.places[p].stock = self.works[h].pool.clone();
                }
                for (i, rate) in self.works[k].takes() {
                    let want = rate * COVER_DAYS * DAY - self.works[k].pool.of(i);
                    let t = want.min(self.works[h].pool.of(i)).min(self.works[k].pool.free());
                    if t <= 0.0 {
                        continue;
                    }
                    let Some(ask) = self.places[p].price(&goods[i]).ask else {
                        continue;
                    };
                    self.works[h].pool.take(i, t);
                    self.works[k].pool.put(i, t);
                    let cost = t / 1000.0 * ask;
                    let _ = ledger.transfer(owner, market, Asset::Credits, cost, tick, cause);
                    let _ = ledger.transfer(market, Party::Administration(system), Asset::Credits, cost * duty, tick, cause);
                    runs[n].earned -= cost;
                    self.places[p].stock = self.works[h].pool.clone();
                }
            }
        }
        self.live(p, &mut used, goods);
        let days = STEP / DAY;
        let place = &mut self.places[p];
        place.made = made.into_iter().map(|(i, kg)| (i, kg / days)).collect();
        place.used = used.into_iter().map(|(i, kg)| (i, kg / days)).collect();
        for (n, &k) in here.iter().enumerate() {
            self.works[k].last = Some(runs[n].clone());
            if let Site::Ground(g) = site
                && let Some(w) = std::sync::Arc::make_mut(&mut land.grounds[g]).works.get_mut(self.works[k].works)
            {
                w.last = Some(runs[n].clone());
            }
        }
        let _ = at;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Port Trethi's works, from the registry: the smelter's line set back
    /// from its ingot; given what it takes (bauxite, soda, anodes, oxygen,
    /// power), it makes ingot into its store until the store is full, and its
    /// owner sells the ingot to the warehouse, which then has it to sell.
    /// Hadley Orbital Works, a rig: its own works its market, its owner (a
    /// company) the one who trades there; it bids for its fusion fuel and its
    /// ore, and with nothing brought in it holds nothing and makes nothing.
    #[test]
    fn a_rig_is_its_owners_market() {
        let mut land = LandOffice::seed(std::iter::empty());
        let mut e = Economy::new(&land, 0.0);
        e.add_rig(&mut land, 3, "Treistun", 7, "rig.treistun.hadley-orbital-works");
        let p = e.place(3, Facility::Rig(7)).expect("a place at the rig");
        assert!(p.warehouse.is_some(), "its works are its market");
        let hadley = land.companies.iter().position(|c| c.0 == "org.hadley").expect("its owner a company");
        assert_eq!(p.trader, Party::Company(hadley as u32));
        assert_eq!(e.owner(&land, p.warehouse.unwrap()), Some(p.trader));
        let deuterium = universe_world::goods::item("stock.deuterium-liq").expect("deuterium");
        assert!(p.wants.iter().any(|&(i, _)| i == deuterium), "it takes its fusion fuel");
        let goods = universe_world::goods::catalog();
        assert!(p.price(&goods[deuterium]).bid > 0.0, "and bids for it");
        let mut ledger = Ledger::default();
        e.step_to(STEP, &mut land, &mut ledger, &goods, 0);
        assert!(e.works[0].last.as_ref().is_some_and(|r| r.rate == 0.0), "nothing to run on");
    }

    #[test]
    fn a_works_runs_its_setups_from_its_pool_and_trades_through_the_warehouse() {
        let w = universe_world::World::new(1984);
        let content = universe_world::content::content();
        let sys = w.system(w.home_system);
        let mut land = LandOffice::seed(sys.spaceports.iter().enumerate().filter_map(|(p, sp)| content.settlement(&sys.name, &sys.bodies[sp.body].key, &sp.name).map(|s| (w.home_system, p, s))));
        let mut e = Economy::new(&land, 0.0);
        let trethi = e.places.iter().position(|p| p.name == "Port Trethi").expect("Port Trethi");
        let smelter = e.works.iter().position(|x| x.name == "Trethi Smelter").expect("its smelter");
        let ingot = universe_world::goods::item("stock.al6061-ingot").unwrap();
        let casthouse = e.works[smelter].setups.iter().find(|s| s.module.identity.key == "module.casthouse").unwrap();
        assert_eq!(casthouse.recipe().unwrap().makes, ingot, "set back from what its line makes");
        let mut ledger = Ledger::default();
        let goods = universe_world::goods::catalog();

        // Nothing to take, nothing made.
        e.step_to(STEP, &mut land, &mut ledger, &goods, 0);
        assert_eq!(e.works[smelter].pool.of(ingot), 0.0);
        let site = e.places[trethi].site;
        assert!(e.works.iter().filter(|w| w.site == site).any(|w| w.last.as_ref().is_some_and(|r| r.rate < 1.0 && r.held_by.is_some())), "held back by what it lacks");

        // Given a day of everything its works take, and fuel for the power station.
        let takes: Vec<(usize, f64)> = e.works.iter().filter(|x| x.site == site).flat_map(|x| x.takes()).collect();
        for x in e.works.iter_mut().filter(|x| x.site == site && !x.exchange) {
            x.pool.room = f64::INFINITY;
            for &(i, r) in &takes {
                if x.uses(i) {
                    x.pool.put(i, r * DAY);
                }
            }
        }
        e.step_to(2.0 * STEP, &mut land, &mut ledger, &goods, 1);
        let place = &e.places[trethi];
        assert!(place.made.get(&ingot).copied().unwrap_or(0.0) > 0.0, "it makes ingot");
        assert!(place.stock.of(ingot) > 0.0, "sold into the warehouse");
        let price = place.price(&goods[ingot]);
        assert!(price.ask.is_some() && price.stock > 0.0, "which has it to sell: {price:?}");
        assert!(e.works[smelter].pool.of(ingot) < 1.0, "the smelter doesn't keep what it doesn't use");

        // The exchange buys what no one here takes, for less as its warehouse fills.
        let ore = universe_world::goods::Ore::Stony.item();
        let before = e.places[trethi].price(&goods[ore]);
        assert!(!before.wanted && before.bid > 0.0);
        let room = e.places[trethi].stock.free();
        e.put(w.home_system, Facility::Spaceport(sys.spaceports.iter().position(|s| s.name == "Port Trethi").unwrap()), ore, room * 0.9);
        assert!(e.places[trethi].price(&goods[ore]).bid < before.bid * 0.2, "a full warehouse pays little");

        // Its people live by their needs, from its stores: air from its life support (its world's
        // isn't breathable), water, food from the warehouse.
        let trethi_place = &e.places[trethi];
        assert!(trethi_place.population > 19.0 && !trethi_place.breathes, "{} thousand", trethi_place.population);
        for need in ["need.air", "need.water", "need.food"] {
            assert!(trethi_place.needs.get(need).is_some_and(|(met, _)| *met > 0.99), "{need} met: {:?}", trethi_place.needs.get(need));
        }

        // The stocking law: Port Eikir holds half the system's food reserve beside its own cover;
        // a market below its aim bids more for food than one above it.
        let food = universe_world::goods::Category::of("market.food").unwrap();
        let aim = |name: &str| e.places.iter().find(|p| p.name == name).and_then(|p| p.people_wants.iter().find(|w| w.0 == Some(food)).map(|w| w.2)).unwrap_or(0.0);
        let per_head = |name: &str| aim(name) / e.places.iter().find(|p| p.name == name).map_or(1.0, |p| p.population * 1000.0);
        assert!(per_head("Port Eikir") > 3.0 * per_head("Port Sirnendis"), "the reserve: {} against {} kg a head", per_head("Port Eikir"), per_head("Port Sirnendis"));
        let flour = universe_world::goods::item("stock.flour-bulk").unwrap();
        let bid = |name: &str| e.places.iter().find(|p| p.name == name).unwrap().price(&goods[flour]).bid;
        let (rich, poor) = if e.places.iter().find(|p| p.name == "Port Eikir").unwrap().stock.of(flour) > 0.0 { ("Port Eikir", "Port Sirnendis") } else { ("Port Sirnendis", "Port Eikir") };
        assert!(bid(poor) >= bid(rich), "short, dearer: {poor} {} against {rich} {}", bid(poor), bid(rich));

        // Its owner sets the yard's welding bays to the MC-07's nose cap (a recipe from
        // the bill: the part, from the sheet it's cut from); given sheet, it welds them.
        let yard = e.works.iter().position(|x| x.name == "Trethi Yard").expect("its yard");
        let bay = e.works[yard].setups.iter().position(|s| s.module.identity.key == "module.welding-bay").unwrap();
        let cap = universe_world::goods::item("part.mc07-01").unwrap();
        let recipe = universe_world::recipes::of("module.welding-bay").iter().position(|r| r.makes == cap).expect("a recipe for it");
        let owner = e.owner(&land, yard).expect("an owner");
        assert!(e.set_up(&land, yard, bay, Some(recipe), Party::Pilot(7)).is_err(), "not someone else's to set");
        e.set_up(&land, yard, bay, Some(recipe), owner).unwrap();
        let sheet = universe_world::goods::item("stock.al6061-sh-2").unwrap();
        e.works[yard].pool.put(sheet, 20_000.0);
        e.step_to(4.0 * STEP, &mut land, &mut ledger, &goods, 2);
        let made = e.places[trethi].made.get(&cap).copied().unwrap_or(0.0) * STEP / DAY;
        assert!(made > 0.0 && made <= 20_000.0 / 1710.72 * goods[cap].mass + 1.0, "nose caps welded from the sheet: {made} kg");
        assert!(e.works[yard].pool.of(universe_world::goods::item("stock.al6061-scrap").unwrap()) + e.places[trethi].stock.of(universe_world::goods::item("stock.al6061-scrap").unwrap()) > 0.0, "and the offcuts, scrap");
    }
}
