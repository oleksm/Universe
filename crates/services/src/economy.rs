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
//! A settlement's people: the registry gives no population yet, so there are
//! none, and nothing is eaten (their needs are `need.*`, for when there are).

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
    /// Where it stands: the land office's ground and its works there.
    pub ground: usize,
    pub works: usize,
    pub name: String,
    pub setups: Vec<Setup>,
    pub pool: Pool,
    /// Does the exchange keep its market here?
    pub exchange: bool,
}

impl Works {
    /// Built as the registry's facility `key`, each line's modules set to
    /// the recipes that lead to what it makes.
    fn new(ground: usize, works: usize, key: &str) -> Option<Self> {
        let reg = universe_world::registry::registry();
        let f = reg.facilities.iter().find(|f| f.identity.key == key)?;
        let module = |k: &str| reg.module(k);
        let mut setups = Vec::new();
        for line in &f.lines {
            let steps: Vec<&str> = line.modules.iter().map(|m| m.module.as_str()).collect();
            let chosen = line.makes.as_deref().and_then(|t| universe_world::settlements::route(reg, &steps, t).ok()).unwrap_or_else(|| vec![None; steps.len()]);
            for (m, r) in line.modules.iter().zip(chosen) {
                // (The registry's recipe, as the engine has it.)
                let r = r.and_then(|k| universe_world::recipes::of(&m.module).iter().position(|x| x.index == Some(k)));
                setups.push(Setup { module: module(&m.module)?, count: m.count, recipe: r });
            }
        }
        for m in &f.modules {
            setups.push(Setup { module: module(&m.module)?, count: m.count, recipe: None });
        }
        let holds: f64 = setups.iter().map(|s| s.module.capacity.holds.unwrap_or(0.0) * s.count as f64).sum();
        let mut w = Works { ground, works, name: f.identity.name.clone(), setups, pool: Pool::default(), exchange: f.exchange.is_some() };
        w.pool.room = if holds > 0.0 { holds } else { w.takes().iter().map(|(_, r)| r * UNSTORED).sum() };
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

    /// The power it draws at full rate (W).
    fn draws(&self) -> f64 {
        self.setups.iter().map(|s| s.recipe().map(|r| r.power).or(s.module.needs.power).unwrap_or(0.0) * s.count as f64).sum()
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
    /// Its land office's ground.
    pub ground: usize,
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
        let have = self.stock.of(item.id);
        let want = self.need(item.id) * COVER_DAYS;
        let (stock, room) = (have / item.mass, self.stock.free() / item.mass);
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
            e.places.push(Place {
                system: g.system,
                facility,
                name: g.recorded.name.clone(),
                ground: k,
                warehouse: None,
                stock: Pool::default(),
                wants: Vec::new(),
                population: 0.0,
                founded: 0.0,
                fed: 1.0,
                waiting: 0.0,
                made: BTreeMap::new(),
                used: BTreeMap::new(),
            });
        }
        e.sync(land);
        e
    }

    /// Facilities built since: into the economy, each as its blueprint.
    fn sync(&mut self, land: &LandOffice) {
        for (k, g) in land.grounds.iter().enumerate() {
            for (j, w) in g.works.iter().enumerate() {
                if self.works.iter().any(|x| x.ground == k && x.works == j) {
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
            p.warehouse = self.works.iter().position(|w| w.ground == p.ground && w.exchange);
            let mut wants: Vec<(usize, f64)> = Vec::new();
            for w in self.works.iter().filter(|w| w.ground == p.ground) {
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

    /// Set module `setup` of works `works` to `recipe` (its place in
    /// `recipes::of` the module; None: to nothing). Its owner's to choose, a
    /// player's or a company's alike: anyone else is refused. (What a
    /// changeover costs isn't described yet: it is at once.)
    pub fn set_up(&mut self, land: &LandOffice, works: usize, setup: usize, recipe: Option<usize>, by: Party) -> Result<(), String> {
        let w = self.works.get(works).ok_or("NO SUCH WORKS")?;
        let g = &land.grounds[w.ground];
        let owner = g.works.get(w.works).and_then(|x| g.lots.iter().find(|l| l.number == x.parcel)).and_then(|l| land.party(&l.owner));
        if owner != Some(by) {
            return Err("NOT YOURS TO SET".into());
        }
        let s = w.setups.get(setup).ok_or("NO SUCH MODULE")?;
        if recipe.is_some_and(|r| r >= universe_world::recipes::of(&s.module.identity.key).len()) {
            return Err(format!("{} CAN'T BE SET TO THAT", s.module.identity.name.to_uppercase()));
        }
        self.works[works].setups[setup].recipe = recipe;
        self.sync_wants();
        Ok(())
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
            for p in 0..self.places.len() {
                self.run_place(p, at, land, ledger, goods, tick);
            }
        }
        self.snapshot = None;
        self.works_snap = None;
    }

    fn run_place(&mut self, p: usize, at: f64, land: &mut LandOffice, ledger: &mut Ledger, goods: &[Item], tick: u64) {
        let dt = STEP;
        let (system, facility, ground) = (self.places[p].system, self.places[p].facility, self.places[p].ground);
        let market = Party::Market(system, facility);
        let cause = universe_protocol::Cause::Rules;
        let g = &land.grounds[ground];
        let here: Vec<usize> = (0..self.works.len()).filter(|&k| self.works[k].ground == ground && g.works.get(self.works[k].works).is_some_and(|w| w.built(at))).collect();
        let owner = |k: usize| g.works.get(self.works[k].works).and_then(|w| g.lots.iter().find(|l| l.number == w.parcel)).and_then(|l| land.party(&l.owner));
        let owners: Vec<Option<Party>> = here.iter().map(|&k| owner(k)).collect();
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
                    runs[n].earned -= cost;
                    self.places[p].stock = self.works[h].pool.clone();
                }
            }
        }
        let days = STEP / DAY;
        let place = &mut self.places[p];
        place.made = made.into_iter().map(|(i, kg)| (i, kg / days)).collect();
        place.used = used.into_iter().map(|(i, kg)| (i, kg / days)).collect();
        let g = std::sync::Arc::make_mut(&mut land.grounds[ground]);
        for (n, &k) in here.iter().enumerate() {
            if let Some(w) = g.works.get_mut(self.works[k].works) {
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
    #[test]
    fn a_works_runs_its_setups_from_its_pool_and_trades_through_the_warehouse() {
        let w = universe_world::World::new(1984);
        let content = universe_world::content::content();
        let sys = w.system(w.home_system);
        let mut land = LandOffice::seed(sys.spaceports.iter().enumerate().filter_map(|(p, sp)| content.settlement(&sys.name, &sys.bodies[sp.body].name, &sp.name).map(|s| (w.home_system, p, s))));
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
        let g = &land.grounds[e.places[trethi].ground];
        assert!(g.works.iter().any(|w| w.last.as_ref().is_some_and(|r| r.rate < 1.0 && r.held_by.is_some())), "held back by what it lacks");

        // Given a day of everything its works take, and fuel for the power station.
        let takes: Vec<(usize, f64)> = e.works.iter().filter(|x| x.ground == e.places[trethi].ground).flat_map(|x| x.takes()).collect();
        for x in e.works.iter_mut().filter(|x| x.ground == e.places[trethi].ground && !x.exchange) {
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

        // Its owner sets the yard's welding bays to the MC-07's nose cap (a recipe from
        // the bill: the part, from the sheet it's cut from); given sheet, it welds them.
        let yard = e.works.iter().position(|x| x.name == "Trethi Yard").expect("its yard");
        let bay = e.works[yard].setups.iter().position(|s| s.module.identity.key == "module.welding-bay").unwrap();
        let cap = universe_world::goods::item("part.mc07-01").unwrap();
        let recipe = universe_world::recipes::of("module.welding-bay").iter().position(|r| r.makes == cap).expect("a recipe for it");
        let g = &land.grounds[e.works[yard].ground];
        let lot = g.lots.iter().find(|l| l.number == g.works[e.works[yard].works].parcel).unwrap();
        let owner = land.party(&lot.owner).expect("an owner");
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
