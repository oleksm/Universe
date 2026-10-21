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
//! Every `step()` the facilities run. Prices are the game's own: an item's
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

use std::collections::{BTreeMap, HashMap, VecDeque};

use universe_world::goods::{Item, POWER_PRICE};
use universe_world::recipes::Recipe;
use universe_world::registry::{Batch, Mark, Module};
use universe_world::traffic::Facility;
use universe_world::units::DAY;

use crate::land::{LandOffice, Run};
use crate::ledger::{Asset, Ledger, Party};

/// The economy's step (s): its clock's period (`clock.economy`).
pub fn step() -> f64 {
    static STEP: std::sync::OnceLock<f64> = std::sync::OnceLock::new();
    *STEP.get_or_init(|| universe_world::registry::clock_every(universe_world::registry::ClockKey::Economy).expect("clock.economy has a period"))
}
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
/// A lot of bulk stock (SFO 21) is what a works makes of an item over this long (s).
const LOT_SPAN: f64 = DAY;
/// A works with no store keeps this long of what it takes (s).
const UNSTORED: f64 = DAY;

/// Stock lying in one place (kg, by item), and what's marked of it (SFO 21): a product's units
/// each by its mark, oldest first; bulk stock by its lots (and the kg of each still here).
#[derive(Clone, Debug, Default)]
pub struct Pool {
    pub stock: BTreeMap<usize, f64>,
    /// What it can hold (kg).
    pub room: f64,
    pub marks: BTreeMap<usize, VecDeque<Mark>>,
    pub lots: BTreeMap<usize, crate::lots::Lots>,
    pub packages: BTreeMap<usize, crate::packages::Units>,
}

/// Is stock item `item` a made product (one serial a unit: a hull, equipment, an industrial module,
/// a structure, a part), not bulk stock (by its lots)?
pub fn is_product(item: usize) -> bool {
    let key = &universe_world::content::content().stock[item].key;
    ["hull.", "equipment.", "module.", "structure.", "part."].iter().any(|p| key.starts_with(p))
}

impl Pool {
    pub fn of(&self, item: usize) -> f64 {
        self.stock.get(&item).copied().unwrap_or(0.0)
    }

    pub fn total(&self) -> f64 {
        self.stock.values().sum::<f64>() + self.packages.values().flatten().map(|u| u.tare).sum::<f64>()
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

    /// Put `kg` of `item` with the marks and lots that came with it.
    pub fn put_with(&mut self, item: usize, kg: f64, marks: Vec<Mark>, lots: Vec<(Batch, f64)>) {
        self.put(item, kg);
        if !marks.is_empty() {
            self.marks.entry(item).or_default().extend(marks);
        }
        if !lots.is_empty() {
            crate::lots::put(self.lots.entry(item).or_default(), lots);
        }
    }

    /// Take up to `kg`; what was taken.
    pub fn take(&mut self, item: usize, kg: f64) -> f64 {
        self.take_with(item, kg).0
    }

    /// Take up to `kg`, and the marks of the whole units and the lots it was, oldest first.
    pub fn take_with(&mut self, item: usize, kg: f64) -> (f64, Vec<Mark>, Vec<(Batch, f64)>) {
        let (kg, marks, lots, packages) = self.take_cargo(item, kg);
        for unit in packages { self.return_empty(unit); }
        (kg, marks, lots)
    }

    /// A transfer preserves whole package identities; consumption uses take_with instead.
    pub fn take_cargo(&mut self, item: usize, kg: f64) -> (f64, Vec<Mark>, Vec<(Batch, f64)>, Vec<crate::packages::Unit>) {
        let before = self.lots.get(&item).cloned().unwrap_or_default();
        let Some(have) = self.stock.get_mut(&item) else {
            return (0.0, Vec::new(), Vec::new(), Vec::new());
        };
        let t = kg.min(*have).max(0.0);
        *have -= t;
        let left = *have;
        if *have <= 0.0 {
            self.stock.remove(&item);
        }
        // (Marks: no more than the whole units left; those over, gone with what was taken.)
        let mut marks = Vec::new();
        if let Some(m) = self.marks.get_mut(&item) {
            let mass = universe_world::content::content().stock[item].mass.max(1e-9);
            let units = ((left + 1e-6) / mass).floor() as usize;
            while m.len() > units {
                marks.extend(m.pop_front());
            }
            if m.is_empty() {
                self.marks.remove(&item);
            }
        }
        // (Lots: the oldest first, as far as what was taken goes.)
        let mut lots = Vec::new();
        if let Some(l) = self.lots.get_mut(&item) {
            lots = crate::lots::take(l, t);
            if l.is_empty() {
                self.lots.remove(&item);
            }
        }
        let selected = self.packages.get_mut(&item).map(|units| crate::packages::take(units, &before, &lots)).unwrap_or_default();
        for unit in selected.opened { self.return_empty(unit); }
        (t, marks, lots, selected.carried)
    }

    /// Opening returns an intact reusable empty, or retains spent/damaged tare as loose waste.
    fn return_empty(&mut self, unit: crate::packages::Unit) {
        if let Some((key, kg, lots)) = unit.opened_stock(true) {
            let item = universe_world::goods::item(&key).expect("packing disposition stock is in the catalogue");
            self.put_with(item, kg, Vec::new(), lots);
        }
    }

    pub fn preview_packages(&self, item: usize, kg: f64) -> Vec<crate::packages::Unit> {
        match (self.packages.get(&item), self.lots.get(&item)) {
            (Some(units), Some(lots)) => crate::packages::preview(units, lots, kg),
            _ => Vec::new(),
        }
    }

    pub fn put_cargo(&mut self, item: usize, kg: f64, marks: Vec<Mark>, lots: Vec<(Batch, f64)>, packages: Vec<crate::packages::Unit>) {
        self.put_with(item, kg, marks, lots);
        self.packages.entry(item).or_default().extend(packages);
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
    /// The deposit it holds a claim to (its id in the body's survey) and the ore left in it (kg):
    /// what its mine digs, until it's worked out.
    pub deposit: Option<(String, f64)>,
    /// Its record's key (a facility's or a rig's): where what it makes was made.
    pub key: String,
    /// What it has made of a product's next unit so far (kg), and the marks of the marked parts
    /// that went into it (their serials, for the unit's mark).
    pub progress: BTreeMap<usize, f64>,
    pub parts_in: Vec<String>,
    /// Who makes what it makes (SFO 21): the owner of the parcel it stands on (an organisation's
    /// key; the administration where it owns the works), a rig's owner.
    pub maker: Option<String>,
    /// Each bulk item's lot it's making now (a day of it: `LOT_SPAN`), wherever what's made of it lies.
    pub open_lots: BTreeMap<usize, Batch>,
}

/// The ore in deposit `id` (kg), from the survey of the body whose world it names (its first
/// segment: `TRD1-...`). Surveys read once.
fn deposit_ore(id: &str) -> Option<f64> {
    static SURVEYS: std::sync::OnceLock<std::sync::Mutex<HashMap<String, Option<std::sync::Arc<universe_world::worlds::Survey>>>>> = std::sync::OnceLock::new();
    let world_id = id.split('-').next()?;
    let reg = universe_world::registry::registry();
    let body = reg.bodies.iter().find(|b| b.survey.as_ref().is_some_and(|s| s.world_id == world_id))?;
    let mut surveys = SURVEYS.get_or_init(Default::default).lock().unwrap_or_else(|e| e.into_inner());
    let w = surveys.entry(body.identity.key.clone()).or_insert_with(|| universe_world::worlds::Survey::load(&body.identity.key)?.ok().map(std::sync::Arc::new)).clone()?;
    w.deposits.iter().find(|d| d.id == id).map(|d| d.ore)
}

/// Where works stand: a land office's ground, or a rig (its system and its
/// body there, see `world::rigs`), which has no ground, parcels or zoning.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Site {
    Ground(usize),
    Rig(usize, usize),
}

/// Stamp before the next setup can consume an output; package metadata cannot outlive its stock.
fn stamp_output(w: &mut Works, sequences: &mut BTreeMap<String, u32>, package_sequences: &mut crate::packages::Sequences,
    maker: &str, ticker: &str, item: usize, kg: f64, at: f64, goods: &[Item]) {
    if kg <= 0.0 { return; }
    let key = &goods[item].key;
    if is_product(item) {
        let mass = goods[item].mass.max(1e-9);
        let done = w.progress.entry(item).or_default();
        *done += kg;
        let mut units = 0;
        while *done >= mass * (1.0 - 1e-9) { *done -= mass; units += 1; }
        for _ in 0..units {
            let code = key.split_once('.').map_or(key.as_str(), |(_, c)| c).replace(['.', '_'], "-").to_uppercase();
            let series = format!("{ticker}-{code}");
            let n = sequences.entry(series.clone()).or_default(); *n += 1;
            let mark = Mark { serial: format!("{series}-{n:06}"), design: key.clone(), revision: None,
                maker: maker.into(), made_at: w.key.clone(), made_on: at, parts: std::mem::take(&mut w.parts_in) };
            w.pool.marks.entry(item).or_default().push_back(mark);
        }
        return;
    }
    let spec = crate::packages::Spec::for_stock(key, goods[item].bulk_density * 1000.0);
    let mut left = kg;
    while left > 0.0 {
        let batch = w.open_lots.get(&item).filter(|b| at - b.made_on < LOT_SPAN && package_sequences.get(&b.lot).copied().unwrap_or(0) < 9999).cloned().unwrap_or_else(|| {
            let n = sequences.entry(ticker.into()).or_default(); *n += 1;
            let b = Batch { lot: format!("{ticker}-{n:06}"), heat: None, made_at: w.key.clone(), made_on: at };
            w.open_lots.insert(item, b.clone()); b
        });
        if let Some(spec) = &spec {
            // A catalogue/template or ownership change closes the old shell first.
            // Otherwise multiple unsealed units could reserve the same final suffix.
            if let Some(units) = w.pool.packages.get_mut(&item) {
                for u in units.iter_mut().filter(|u| u.batch.lot == batch.lot && u.label.is_none()
                    && (u.template != spec.template || u.brand != maker)) {
                    assert!(u.seal(at, package_sequences), "an open unit reserves a suffix");
                }
            }
            if package_sequences.get(&batch.lot).copied().unwrap_or(0) >= 9999 { continue; }
        }
        let mut units = w.pool.packages.remove(&item).unwrap_or_default();
        let remainder = match &spec {
            Some(spec) => crate::packages::fill(&mut units, package_sequences, spec, &batch, key, maker, left, at, || {
                let empty_item = universe_world::goods::item(&spec.empty_stock)?;
                if w.pool.of(empty_item) + spec.tare * 1e-9 < spec.tare { return None; }
                let (_, _, lots) = w.pool.take_with(empty_item, spec.tare);
                Some(crate::packages::EmptyStock { key: spec.empty_stock.clone(), lots, serviceable: true,
                    returns_empty: spec.returns_empty, waste_stock: spec.waste_stock.clone() })
            }),
            None => 0.0,
        };
        if !units.is_empty() { w.pool.packages.insert(item, units); }
        // Lack of containers leaves output loose. Only serial exhaustion rotates the lot.
        let exhausted = package_sequences.get(&batch.lot).copied().unwrap_or(0) >= 9999;
        let stamped = if exhausted { left - remainder } else { left };
        w.pool.put_with(item, 0.0, Vec::new(), vec![(batch, stamped)]);
        left = if exhausted { remainder } else { 0.0 };
    }
}

impl Works {
    /// Built as the registry's facility `key`, each line's modules set to
    /// the recipes that lead to what it makes.
    fn new(ground: usize, works: usize, key: &str) -> Option<Self> {
        let reg = universe_world::registry::registry();
        let f = reg.facility(key)?;
        let mut w = Self::built(Site::Ground(ground), works, &f.identity.name, &f.lines, &f.modules, &f.stock, f.exchange.is_some())?;
        w.key = key.to_string();
        w.maker = reg.parcel(&f.parcel).and_then(|p| p.owner.clone());
        // (A mine's claim: the deposit it digs, its ore as the survey has it.)
        w.deposit = f.claim.as_ref().and_then(|c| Some((c.deposit.clone(), deposit_ore(&c.deposit)?)));
        Some(w)
    }

    /// Built as the registry's rig `key`, body `body` of `system`: its own warehouse, its owner's.
    fn rig(system: usize, body: usize, key: &str) -> Option<Self> {
        let reg = universe_world::registry::registry();
        let r = reg.settlement(key)?;
        let mut w = Self::built(Site::Rig(system, body), 0, &r.identity.name, &r.lines, &r.modules, &[], true)?;
        w.owner = r.owner.clone();
        w.key = key.to_string();
        w.maker = r.owner.clone();
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
        let mut w = Works { site, works, name: name.to_string(), setups, pool: Pool::default(), exchange, owner: None, last: None, deposit: None, key: String::new(), progress: BTreeMap::new(), parts_in: Vec::new(), open_lots: BTreeMap::new(), maker: None };
        w.pool.room = if holds > 0.0 { holds } else { w.takes().iter().map(|(_, r)| r * UNSTORED).sum() };
        // (What lies in it at day 0: the registry's seed state.)
        let goods = &universe_world::content::content().stock;
        for s in stock {
            if let Some(i) = universe_world::goods::item(&s.item) {
                let kg = s.quantity.unwrap_or(0.0) + s.pieces.map_or(0.0, |n| n as f64 * goods[i].mass);
                w.pool.put_with(i, kg, s.serials.clone(), s.batch.clone().map(|b| (b, kg)).into_iter().collect());
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
    /// Its settlement record's key (None: a rig, a settlement of its own).
    pub record: Option<String>,
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

/// The duty on sales in system `system` (its galaxy index): its law's (0: no law, or none levied).
fn duty_in(system: usize) -> f64 {
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
    /// The last number given in each series (SFO 21): a serial's maker and product
    /// (`TOL-HAULER`), a lot's maker (`TOL`).
    pub sequences: BTreeMap<String, u32>,
    pub package_sequences: crate::packages::Sequences,
}

/// One works in a place's step: which (`k`), who owns it, who marks what it makes (maker and
/// ticker), how it ran, and the power it drew (W).
struct AtWorks {
    k: usize,
    owner: Option<Party>,
    maker: Option<(String, String)>,
    run: Run,
    drawn: f64,
}

/// A place's step as it goes: what was made and used (kg), and what's to be marked (works, item,
/// kg).
#[derive(Default)]
struct Tally {
    made: BTreeMap<usize, f64>,
    used: BTreeMap<usize, f64>,
}

impl Economy {
    /// Refresh derived market views after restoring authoritative works inventories.
    pub fn invalidate_snapshots(&mut self) {
        for place in &mut self.places {
            if let Some(k) = place.warehouse {
                place.stock = self.works[k].pool.clone();
            }
        }
        self.snapshot = None;
        self.works_snap = None;
    }

    /// The settlements the land office has ground for, and their facilities.
    pub fn new(land: &LandOffice, now: f64) -> Self {
        let mut e = Economy { stepped_to: now, ..Default::default() };
        for (k, g) in land.grounds.iter().enumerate() {
            let facility = Facility::Spaceport(g.port);
            e.index.insert((g.system, facility), e.places.len());
            // (Its people, as the registry has them: thousands.)
            let reg = universe_world::registry::registry();
            let record = reg.settlement(&g.recorded.key);
            let people = record.and_then(|s| s.population).map_or(0.0, |n| n as f64 / 1000.0);
            let breathes = record.and_then(|s| reg.bodies.iter().find(|b| Some(&b.identity.key) == s.at.as_ref())).is_some_and(|b| b.atmosphere.breathable == Some(true));
            e.places.push(Place {
                system: g.system,
                facility,
                name: g.recorded.name.clone(),
                record: Some(g.recorded.key.clone()),
                site: Site::Ground(k),
                trader: Party::Market(g.system, facility),
                duty: duty_in(g.system),
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
        e.count_marks();
        e
    }

    /// Each series' last number, from the marks and lots already lying about (day 0's), so the
    /// next made goes on from them.
    fn count_marks(&mut self) {
        let number = |s: &str| s.rsplit_once('-').and_then(|(series, n)| Some((series.to_string(), n.parse::<u32>().ok()?)));
        let mut seen: Vec<(String, u32)> = Vec::new();
        for w in &self.works {
            seen.extend(w.pool.marks.values().flatten().filter_map(|m| number(&m.serial)));
            seen.extend(w.pool.lots.values().flatten().filter_map(|(b, _)| number(&b.lot)));
        }
        for (series, n) in seen {
            let last = self.sequences.entry(series).or_default();
            *last = (*last).max(n);
        }
    }

    /// Move `kg` of `item` from works `from` to works `to`, with its marks and lots.
    fn shift(&mut self, from: usize, to: usize, item: usize, kg: f64) -> f64 {
        let packages = self.works[from].pool.preview_packages(item, kg);
        if kg + packages.iter().map(|u| u.tare).sum::<f64>() > self.works[to].pool.free() + 1e-6 { return 0.0; }
        let (t, marks, lots, packages) = self.works[from].pool.take_cargo(item, kg);
        self.works[to].pool.put_cargo(item, t, marks, lots, packages);
        t
    }

    /// The registry's rig `key`, body `body` of `system` (see `world::rigs`):
    /// a place with no people whose market is its own works, where its owner
    /// trades at its dock (its owner a company of the land office's from now).
    pub fn add_rig(&mut self, land: &mut LandOffice, system: usize, body: usize, key: &str) {
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
            record: None,
            site: Site::Rig(system, body),
            trader,
            duty: duty_in(system),
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
                if let Some(x) = w.facility.as_deref().and_then(|key| Works::new(k, j, key)) {
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
            let record = p.record.as_deref().and_then(|k| reg.settlement(k));
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
        let dt = step();
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
        self.take_marked(system, f, item, kg).0
    }

    /// As `take`, with the marks and lots of what came out (SFO 21).
    pub fn take_marked(&mut self, system: usize, f: Facility, item: usize, kg: f64) -> (f64, Vec<Mark>, Vec<(Batch, f64)>) {
        let Some((p, pool)) = self.market_mut(system, f) else {
            return (0.0, Vec::new(), Vec::new());
        };
        let out = pool.take_with(item, kg);
        p.stock = pool.clone();
        out
    }

    pub fn take_cargo(&mut self, system: usize, f: Facility, item: usize, kg: f64) -> (f64, Vec<Mark>, Vec<(Batch, f64)>, Vec<crate::packages::Unit>) {
        let Some((p, pool)) = self.market_mut(system, f) else { return (0.0, vec![], vec![], vec![]) };
        let out = pool.take_cargo(item, kg);
        p.stock = pool.clone();
        out
    }

    pub fn put_cargo(&mut self, system: usize, f: Facility, item: usize, kg: f64, marks: Vec<Mark>, lots: Vec<(Batch, f64)>, packages: Vec<crate::packages::Unit>) -> bool {
        let Some((p, pool)) = self.market_mut(system, f) else { return false };
        if kg + packages.iter().map(|u| u.tare).sum::<f64>() > pool.free() + 1e-6 { return false; }
        pool.put_cargo(item, kg, marks, lots, packages);
        p.stock = pool.clone();
        true
    }

    /// As `put`, with the marks and lots of what's put (SFO 21).
    pub fn put_marked(&mut self, system: usize, f: Facility, item: usize, kg: f64, marks: Vec<Mark>, lots: Vec<(Batch, f64)>) -> f64 {
        let Some((p, pool)) = self.market_mut(system, f) else {
            return 0.0;
        };
        let t = kg.min(pool.free()).max(0.0);
        pool.put_with(item, t, marks, lots);
        p.stock = pool.clone();
        t
    }

    /// Run every facility up to world time `now`, a `step()` at a time: each
    /// settlement's power shared out among what draws it, each module as far
    /// as its inputs, power and room let it; then each owner trading with the
    /// settlement's market. How each ran goes to its land office works.
    pub fn step_to(&mut self, now: f64, land: &mut LandOffice, ledger: &mut Ledger, goods: &[Item], tick: u64) {
        if self.stepped_to + step() > now {
            return;
        }
        self.sync(land);
        while self.stepped_to + step() <= now {
            self.stepped_to += step();
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
        let dt = step();
        let site = self.places[p].site;
        // (Who makes what each works makes, for its marks: its maker and that organisation's ticker.)
        let reg = universe_world::registry::registry();
        let mut here: Vec<AtWorks> = (0..self.works.len())
            .filter(|&k| self.works[k].site == site && self.built(land, k, at))
            .map(|k| AtWorks {
                k,
                owner: self.owner(land, k),
                maker: self.works[k].maker.clone().and_then(|m| Some((m.clone(), reg.org(&m)?.ticker.clone()?))),
                run: Run::default(),
                drawn: 0.0,
            })
            .collect();
        let mut tally = Tally::default();
        // Power: what the stations can supply with the fuel they hold, shared out.
        let supply: f64 = here.iter().map(|h| self.works[h.k].supplies() * self.works[h.k].fuelled(dt).min(1.0)).sum();
        let demand: f64 = here.iter().map(|h| self.works[h.k].draws()).sum();
        let share = if demand > 0.0 { (supply / demand).min(1.0) } else { 1.0 };
        for h in &here {
            let w = &mut self.works[h.k];
            let expired: Vec<_> = w.open_lots.iter().filter(|(_, b)| at - b.made_on >= LOT_SPAN).map(|(&i, b)| (i, b.lot.clone())).collect();
            for (i, lot) in expired {
                if let Some(units) = w.pool.packages.get_mut(&i) {
                    assert!(crate::packages::close(units, &lot, at, &mut self.package_sequences), "an open unit reserves a suffix");
                }
                w.open_lots.remove(&i);
            }
        }
        self.produce(&mut here, share, goods, &mut tally, at);
        self.burn_and_bill(&mut here, supply, ledger, tick, &mut tally.used);
        self.trade(p, &mut here, ledger, goods, tick);
        self.live(p, &mut tally.used, goods);
        let days = step() / DAY;
        let place = &mut self.places[p];
        place.made = tally.made.into_iter().map(|(i, kg)| (i, kg / days)).collect();
        place.used = tally.used.into_iter().map(|(i, kg)| (i, kg / days)).collect();
        for h in &here {
            self.works[h.k].last = Some(h.run.clone());
            if let Site::Ground(g) = site
                && let Some(w) = std::sync::Arc::make_mut(&mut land.grounds[g]).works.get_mut(self.works[h.k].works)
            {
                w.last = Some(h.run.clone());
            }
        }
    }

    /// Each works runs its setups for a step, as far as power (`share`), its inputs, its store and
    /// its deposit let it: what it made and used, and what's to be marked, into `tally`.
    fn produce(&mut self, here: &mut [AtWorks], share: f64, goods: &[Item], tally: &mut Tally, at: f64) {
        let dt = step();
        for h in here.iter_mut() {
            let w = &mut self.works[h.k];
            let mut most: f64 = 0.0;
            let mut ran = 0.0;
            let mut held: Option<String> = None;
            for s in 0..w.setups.len() {
                let setup = w.setups[s].clone();
                let Some(r) = setup.recipe() else {
                    h.drawn += setup.module.needs.power.unwrap_or(0.0) * setup.count as f64 * share;
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
                // Packing moves existing empty-stock mass into tare; it adds no pool mass.
                let grows = out.iter().map(|o| o.1).sum::<f64>() - r.inputs.iter().map(|o| o.1).sum::<f64>();
                if grows > 0.0 {
                    let can = w.pool.free() / (grows * full).max(1e-12);
                    if can < k {
                        k = can;
                        why = Some("STORE FULL".into());
                    }
                }
                // (A claimed deposit's ore: dug till it's worked out.)
                if r.from_ground > 0.0
                    && let Some((_, left)) = &w.deposit
                {
                    let can = left / (r.from_ground * full).max(1e-12);
                    if can < k {
                        k = can;
                        why = Some("DEPOSIT WORKED OUT".into());
                    }
                }
                let k = k.clamp(0.0, 1.0);
                if r.from_ground > 0.0
                    && let Some((_, left)) = &mut w.deposit
                {
                    *left = (*left - r.from_ground * full * k).max(0.0);
                }
                for &(i, q) in &r.inputs {
                    let (t, marks, _) = w.pool.take_with(i, q * full * k);
                    *tally.used.entry(i).or_default() += t;
                    // (The marked parts that went in: on the mark of what they went into.)
                    w.parts_in.extend(marks.into_iter().map(|m| m.serial));
                }
                for (i, q) in out {
                    w.pool.put(i, q * full * k);
                    *tally.made.entry(i).or_default() += q * full * k;
                    if let Some((maker,ticker)) = &h.maker {
                        stamp_output(w, &mut self.sequences, &mut self.package_sequences, maker, ticker, i, q * full * k, at, goods);
                    }
                }
                h.drawn += r.power * setup.count as f64 * k;
                most += 1.0;
                ran += k;
                if k < 1.0 && held.is_none() {
                    held = why;
                }
            }
            h.run = Run { rate: if most > 0.0 { ran / most } else { 1.0 }, held_by: held, earned: 0.0 };
        }
    }

    /// The stations burn for what was drawn (of the `supply` they could give), and are paid for it
    /// by what drew it.
    fn burn_and_bill(&mut self, here: &mut [AtWorks], supply: f64, ledger: &mut Ledger, tick: u64, used: &mut BTreeMap<usize, f64>) {
        let (dt, cause) = (step(), universe_protocol::Cause::Rules);
        let used_w: f64 = here.iter().map(|h| h.drawn).sum();
        for n in 0..here.len() {
            let k = here[n].k;
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
            here[n].run.rate = out;
            if let Some(seller) = here[n].owner {
                for m in 0..here.len() {
                    if let Some(payer) = here[m].owner
                        && payer != seller
                    {
                        let bill = here[m].drawn * part * dt / 3.6e9 * POWER_PRICE;
                        let _ = ledger.transfer(payer, seller, Asset::Credits, bill, tick, cause);
                        here[m].run.earned -= bill;
                        here[n].run.earned += bill;
                    }
                }
            }
        }
    }

    /// Each owner and place `p`'s market: what a works makes and doesn't use, sold into the
    /// warehouse; what it takes, bought from there to cover.
    fn trade(&mut self, p: usize, here: &mut [AtWorks], ledger: &mut Ledger, goods: &[Item], tick: u64) {
        let (system, duty, market, cause) = (self.places[p].system, self.places[p].duty, self.places[p].trader, universe_protocol::Cause::Rules);
        let Some(h) = self.places[p].warehouse else { return };
        self.places[p].stock = self.works[h].pool.clone();
        for at in here.iter_mut() {
            let (k, Some(owner)) = (at.k, at.owner) else { continue };
            if k == h {
                continue;
            }
            let spare: Vec<(usize, f64)> = self.works[k].pool.stock.iter().filter(|(i, _)| self.works[k].sells(**i)).map(|(i, kg)| (*i, (*kg - self.works[k].pool.packages.get(i).into_iter().flatten().filter(|u| u.label.is_none()).map(|u| u.net_mass).sum::<f64>()).max(0.0))).collect();
            for (i, kg) in spare {
                let price = self.places[p].price(&goods[i]);
                let t = kg.min(self.works[h].pool.free());
                if t <= 0.0 || price.bid <= 0.0 {
                    continue;
                }
                let t = self.shift(k, h, i, t);
                // (Priced a unit: a tonne of bulk, a piece of a product.)
                let paid = t / goods[i].mass * price.bid;
                let _ = ledger.transfer(market, owner, Asset::Credits, paid, tick, cause);
                let _ = ledger.transfer(owner, Party::Administration(system), Asset::Credits, paid * duty, tick, cause);
                at.run.earned += paid;
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
                let t = self.shift(h, k, i, t);
                let cost = t / goods[i].mass * ask;
                let _ = ledger.transfer(owner, market, Asset::Credits, cost, tick, cause);
                let _ = ledger.transfer(market, Party::Administration(system), Asset::Credits, cost * duty, tick, cause);
                at.run.earned -= cost;
                self.places[p].stock = self.works[h].pool.clone();
            }
        }
    }
}

#[cfg(test)]
mod packing_stock_tests {
    use super::*;
    use universe_protocol::{Cause, ShipId};

    fn fixture() -> (Works, Vec<Item>, usize, usize, crate::packages::Spec) {
        let goods = universe_world::goods::catalog();
        let item = universe_world::goods::item("stock.stony-ore-bulk").unwrap();
        let spec = crate::packages::Spec::for_stock(&goods[item].key, goods[item].bulk_density * 1000.).expect("test with the packaging-stock registry");
        let empty = universe_world::goods::item(&spec.empty_stock).unwrap();
        let w = Works {
            site: Site::Ground(0), works: 0, name: "Test".into(), setups: vec![],
            pool: Pool { room: 1e9, ..Default::default() }, exchange: false,
            owner: None, last: None, deposit: None, key: "facility.test".into(),
            progress: BTreeMap::new(), parts_in: vec![], maker: Some("org.test".into()),
            open_lots: BTreeMap::new(),
        };
        (w, goods, item, empty, spec)
    }

    fn output(w: &mut Works, goods: &[Item], item: usize, kg: f64,
        seq: &mut BTreeMap<String, u32>, pkg: &mut crate::packages::Sequences) {
        w.pool.put(item, kg);
        stamp_output(w, seq, pkg, "org.test", "TEST", item, kg, 10., goods);
    }

    #[test]
    fn packing_stock_reserves_actual_partial_units_and_conserves_tare() {
        let (mut w, goods, item, empty, spec) = fixture();
        let (mut seq, mut pkg) = (BTreeMap::new(), BTreeMap::new());
        w.pool.put(empty, 2. * spec.tare);
        output(&mut w, &goods, item, spec.capacity / 4., &mut seq, &mut pkg);
        assert_eq!(w.pool.of(empty), spec.tare);
        assert_eq!(w.pool.packages[&item].len(), 1);
        assert_eq!(w.pool.total(), 2. * spec.tare + spec.capacity / 4.);
        output(&mut w, &goods, item, spec.capacity / 4., &mut seq, &mut pkg);
        assert_eq!(w.pool.of(empty), spec.tare, "same open unit needs no second empty");
        let lot = w.open_lots[&item].lot.clone();
        assert!(crate::packages::close(w.pool.packages.get_mut(&item).unwrap(), &lot, 11., &mut pkg));
        let label = w.pool.packages[&item][0].label.as_ref().unwrap();
        assert_eq!(label.gross_mass, spec.capacity / 2. + spec.tare);
        let before = w.pool.total();
        w.pool.take(item, spec.capacity / 4.);
        assert_eq!(w.pool.of(empty), 2. * spec.tare, "split returns one empty at the source");
        assert!(w.pool.packages[&item].is_empty());
        assert_eq!(w.pool.total(), before - spec.capacity / 4.);
    }

    #[test]
    fn packing_stock_shortage_stays_loose_and_whole_transfer_keeps_reservation() {
        let (mut w, goods, item, empty, spec) = fixture();
        let (mut seq, mut pkg) = (BTreeMap::new(), BTreeMap::new());
        output(&mut w, &goods, item, 10., &mut seq, &mut pkg);
        assert!(!w.pool.packages.contains_key(&item));
        assert_eq!(w.pool.total(), 10.);
        w.pool.put(empty, spec.tare);
        output(&mut w, &goods, item, spec.capacity, &mut seq, &mut pkg);
        let preview = w.pool.preview_packages(item, spec.capacity + 10.);
        assert_eq!(preview.len(), 1);
        assert_eq!(w.pool.of(empty), 0.);
        let (kg, marks, lots, units) = w.pool.take_cargo(item, spec.capacity + 10.);
        assert_eq!(units.len(), 1);
        assert_eq!(w.pool.of(empty), 0., "whole package travels rather than returning its shell");
        let mut dest = Pool::default();
        dest.put_cargo(item, kg, marks, lots, units);
        assert_eq!(dest.total(), spec.capacity + 10. + spec.tare);
        dest.take(item, spec.capacity + 10.);
        assert_eq!(dest.of(empty), spec.tare);
        assert_eq!(dest.total(), spec.tare);
    }

    #[test]
    fn packing_stock_spent_bindings_become_unpacked_waste() {
        let (mut w, goods, _, _, _) = fixture();
        let item = universe_world::goods::item("stock.wool-bulk").unwrap();
        let spec = crate::packages::Spec::for_stock(&goods[item].key, goods[item].bulk_density * 1000.).unwrap();
        assert!(!spec.returns_empty);
        let empty = universe_world::goods::item(&spec.empty_stock).unwrap();
        let waste = universe_world::goods::item(&spec.waste_stock).unwrap();
        w.pool.put(empty, spec.tare);
        output(&mut w, &goods, item, spec.capacity, &mut BTreeMap::new(), &mut BTreeMap::new());
        let total = w.pool.total();
        w.pool.take(item, spec.capacity);
        assert_eq!(w.pool.of(empty), 0.);
        assert_eq!(w.pool.of(waste), spec.tare);
        assert!(!w.pool.packages.contains_key(&waste), "waste stays unpacked until a real skip is reserved");
        assert_eq!(w.pool.total(), total - spec.capacity);
    }

    #[test]
    fn packing_stock_legacy_shells_cannot_mint_empty_stock() {
        let (mut w, goods, item, empty, spec) = fixture();
        let waste = universe_world::goods::item(&spec.waste_stock).unwrap();
        w.pool.put(empty, spec.tare);
        output(&mut w, &goods, item, spec.capacity, &mut BTreeMap::new(), &mut BTreeMap::new());
        w.pool.packages.get_mut(&item).unwrap()[0].empty = None;
        w.pool.take(item, spec.capacity);
        assert_eq!(w.pool.of(empty), 0.);
        assert_eq!(w.pool.of(waste), spec.tare);
        assert_eq!(w.pool.total(), spec.tare);
    }

    #[test]
    fn packing_stock_damage_writes_tare_to_waste_instead_of_repairing_empty() {
        for damaged_before_opening in [false, true] {
            let (mut w, goods, item, empty, spec) = fixture();
            let waste = universe_world::goods::item(&spec.waste_stock).unwrap();
            w.pool.put(empty, spec.tare);
            output(&mut w, &goods, item, spec.capacity, &mut BTreeMap::new(), &mut BTreeMap::new());
            let (kg, marks, lots, mut packages) = w.pool.take_cargo(item, spec.capacity);
            if damaged_before_opening { packages[0].empty.as_mut().unwrap().serviceable = false; }
            let pilot = ShipId(0);
            let mut ledger = Ledger::default();
            ledger.settle(Party::Pilot(pilot), Asset::Goods(item), kg / goods[item].mass, 0, Cause::Rules);
            ledger.load_cargo(pilot, item, marks, lots, packages);
            if damaged_before_opening {
                ledger.unload_cargo(pilot, item, 1, kg / 2., 42, Cause::Rules);
                assert_eq!(ledger.balance(Party::Pilot(pilot), Asset::Goods(empty)), 0.);
                assert_eq!(ledger.balance(Party::Pilot(pilot), Asset::Goods(waste)) * goods[waste].mass, spec.tare);
            } else {
                ledger.unload_marked(pilot, item, 1, kg, 42, Cause::Rules);
                ledger.settle(Party::Pilot(pilot), Asset::Goods(item), 0., 42, Cause::Rules);
                assert_eq!(ledger.cargo_mass(pilot, &goods), spec.tare);
                assert_eq!(ledger.balance(Party::Pilot(pilot), Asset::Goods(empty)), 0.);
            }
            let returned = ledger.journal.iter().find(|e| e.to == Party::Pilot(pilot) && e.asset == Asset::Goods(waste)).unwrap();
            assert_eq!(returned.tick, 42);
            assert!(ledger.balanced());
        }
    }

    #[test]
    fn packing_stock_ledger_split_returns_at_source_and_conserves_tare() {
        let (mut w, goods, item, empty, spec) = fixture();
        let (mut seq, mut pkg) = (BTreeMap::new(), BTreeMap::new());
        w.pool.put(empty, spec.tare);
        output(&mut w, &goods, item, spec.capacity, &mut seq, &mut pkg);
        let (kg, marks, lots, units) = w.pool.take_cargo(item, spec.capacity);
        let pilot = ShipId(0);
        let mut ledger = Ledger::default();
        ledger.settle(Party::Pilot(pilot), Asset::Goods(item), kg / goods[item].mass, 0, Cause::Rules);
        ledger.load_cargo(pilot, item, marks, lots, units);
        let total = ledger.cargo_mass(pilot, &goods);
        let volume = ledger.cargo_volume(pilot, &goods);
        let change = ledger.withdrawal_volume_change(pilot, item, kg / 2., &goods);
        let (_, _, carried) = ledger.unload_cargo(pilot, item, 1, kg / 2., 1, Cause::Rules);
        ledger.transfer(Party::Pilot(pilot), Party::World, Asset::Goods(item), kg / 2. / goods[item].mass, 1, Cause::Rules).unwrap();
        assert!(carried.is_empty());
        assert!((ledger.cargo_volume(pilot, &goods) - volume - change).abs() < 1e-9);
        assert!(change > 0., "a returned empty and loose remainder need room to open");
        assert_eq!(ledger.balance(Party::Pilot(pilot), Asset::Goods(empty)), 1.);
        assert_eq!(ledger.cargo_mass(pilot, &goods), total - kg / 2.);
        assert!(ledger.balanced());
        assert_eq!(goods[empty].mass, spec.tare);
        assert!((goods[empty].mass / (1000. * goods[empty].bulk_density) - spec.volume).abs() < 1e-9);
    }
}
