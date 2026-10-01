//! Markets: every station and spaceport trades a few dozen of the goods.
//! What it trades is its own, generated from the seed and what the place is
//! (a station makes machines and wants food; a farm world the reverse; a
//! mining world makes ores and metals):
//!
//! - goods it **produces** it sells to you from stock, cheaply while stocked
//!   up and dearer as the stock runs down; it buys them back for less;
//! - goods it **wants** it buys from you, dearly while demand lasts and for
//!   less as it's met;
//! - a few categories it **bans**: it won't trade them at all.
//!
//! Stock and demand recover toward their usual levels over `RECOVERY`. A
//! market's state is kept only once it has been traded with. One currency,
//! credits, booked in the ledger. You trade only while docked or landed at
//! the market (a physical fact the core reports).
//!
//! In the settled systems a market is its place's economy (see `economy`):
//! it sells what the place makes, from the place's real stock, and buys
//! what it uses, as far as it has room; its prices follow that stock; and
//! every trade puts tonnes in or takes them out. Nothing comes or goes
//! but by ship.

use std::collections::HashMap;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use universe_protocol::{BodyId, Cause, Tick};
use universe_world::goods::{Category, Item};
use universe_world::rng::{mix, Rng};
use universe_world::ship::HOLD_CAPACITY;
use universe_world::system::StarSystem;
use universe_world::terrain::TerrainKind;
use universe_world::traffic::Facility;

use crate::ledger::{Asset, Ledger, Party};
/// Time for stock and demand to get most of the way back (game s).
pub const RECOVERY: f64 = 6.0 * 3600.0;
/// A market buys back what it produces at this share of its selling price.
pub const BUY_BACK: f64 = 0.7;

/// Which way a market trades an item.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Side {
    /// It produces it: sells to you (and buys back for less).
    Sells,
    /// It wants it: buys from you.
    Buys,
}

/// An item a market trades, and how.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Offer {
    pub item: usize,
    pub side: Side,
    /// Its reference price here (credits per unit), at the usual level.
    pub base: f64,
    /// The usual stock (selling) or demand (buying), in units.
    pub usual: f64,
}

/// A market's standing make-up.
#[derive(Clone, Debug, PartialEq)]
pub struct Market {
    pub offers: Vec<Offer>,
    /// Categories it won't trade.
    pub banned: Vec<Category>,
    /// Categories it wants in general: anything of these it buys (listed
    /// items at their own prices, the rest at `GENERAL_PREMIUM` over worth).
    pub wants: Vec<Category>,
}

/// What a market pays for unlisted goods of a category it wants, over their worth.
pub const GENERAL_PREMIUM: f64 = 1.05;
/// Worth of a market's usual stock or demand of one line (credits).
pub const LOT_VALUE: f64 = 8_000.0;

/// Its usual demand for unlisted goods of a kind it wants (units): half a lot.
fn general_demand(item: &Item) -> f64 {
    (LOT_VALUE * 0.5 / item.price).clamp(2.0, 200.0).round()
}

/// An offer as it stands now.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Quote {
    pub offer: Offer,
    /// Stock left (selling) or demand left (buying), units.
    pub level: f64,
    /// Price per unit if you buy from it (selling side), and what it pays you per unit.
    pub buy: Option<f64>,
    pub sell: f64,
}

/// A market's changing state: current levels by item, as of `updated`.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct MarketState {
    pub levels: HashMap<usize, f64>,
    pub updated: f64,
    /// Usual levels of unlisted lines traded (general demand).
    #[serde(default)]
    pub usual: HashMap<usize, f64>,
}

/// What the market trades, from what the place is.
fn leanings(sys: &StarSystem, f: Facility) -> (&'static [Category], &'static [Category]) {
    use Category::*;
    const STATION: (&[Category], &[Category]) = (
        &[Machinery, Electronics, Computers, Robots, Tools, Medicine, Fuel, Chemicals],
        &[Food, Water, Ores, Metals, Luxuries, Textiles, Biologics, Art],
    );
    const FARM: (&[Category], &[Category]) = (
        &[Food, Water, Textiles, Biologics, Luxuries, Art],
        &[Machinery, Electronics, Computers, Medicine, Tools, Robots, Fuel],
    );
    const MINE: (&[Category], &[Category]) = (
        &[Ores, Metals, Minerals, Chemicals, Fuel, Artifacts],
        &[Food, Water, Machinery, Tools, Medicine, Robots, Luxuries],
    );
    match f {
        Facility::Spaceport(p) => {
            let body = &sys.bodies[sys.spaceports[p].body];
            match body.terrain.as_ref().map(|t| t.kind) {
                Some(TerrainKind::Terran) => FARM,
                _ => MINE,
            }
        }
        _ => STATION,
    }
}

/// Its index for seeding.
fn key(f: Facility) -> u64 {
    match f {
        Facility::Station(i) => i as u64,
        Facility::Spaceport(i) => 1000 + i as u64,
        Facility::Gate(i) => 2000 + i as u64,
        Facility::Asteroid(i) => 3000 + i as u64,
    }
}

/// Ship fuel's usual price (credits a tonne), and what the frontier asks over it.
pub const FUEL_PRICE: f64 = 60.0;
const FRONTIER: f64 = 2.0;

/// Varieties of each kind of goods a settled market lists.
const VARIETIES: usize = 3;

/// The market of a settled place: for each kind of goods it makes or uses,
/// a few varieties (the raw ores always, where it trades ore or water),
/// sold where it makes more than it uses, else bought. It wants any variety
/// of what it uses.
fn place_market(seed: u64, place: &crate::economy::Place, catalog: &[Item]) -> Market {
    let mut rng = Rng::new(mix(mix(seed, 0x3a4c_e701 + place.system as u64), key(place.facility)));
    let mut offers = Vec::new();
    for c in Category::all().filter(|&c| place.trades(c)) {
        let side = if place.sells(c) { Side::Sells } else { Side::Buys };
        let mut pool: Vec<usize> = catalog.iter().filter(|i| i.category == c && i.id < universe_world::goods::CATALOG_SIZE).map(|i| i.id).collect();
        let mut picked: Vec<usize> = catalog.iter().filter(|i| i.category == c && i.id >= universe_world::goods::CATALOG_SIZE).map(|i| i.id).collect();
        for _ in 0..VARIETIES {
            if pool.is_empty() {
                break;
            }
            let j = (rng.range(0.0, pool.len() as f64) as usize).min(pool.len() - 1);
            picked.push(pool.swap_remove(j));
        }
        for id in picked {
            let usual = (place.target(c) * 1000.0 / catalog[id].mass).round().max(1.0);
            offers.push(Offer { item: id, side, base: catalog[id].price, usual });
        }
    }
    offers.sort_by_key(|o| (o.side == Side::Buys, catalog[o.item].category, o.item));
    let wants = Category::all().filter(|&c| place.trades(c) && !place.sells(c)).collect();
    Market { offers, banned: Vec::new(), wants }
}

/// A settled market's quote for `o`, from its place's stock now: selling,
/// what's in stock; buying, the room it has. Prices follow the stock (see
/// `Place::factor`).
fn place_quote(place: &crate::economy::Place, o: &Offer, item: &Item) -> Quote {
    let c = item.category;
    let p = item.price * place.factor(c).unwrap_or(1.0);
    match o.side {
        Side::Sells => Quote { offer: *o, level: (place.stock_of(c) * 1000.0 / item.mass).floor(), buy: Some(p * 1.1), sell: p * 0.9 },
        Side::Buys => Quote { offer: *o, level: (place.room(c) * 1000.0 / item.mass).floor(), buy: None, sell: p },
    }
}

/// The market at facility `f` in system `system` (`sys`), for the galaxy
/// `seed`'s `catalog`. Gates have none.
pub fn market(seed: u64, system: usize, sys: &StarSystem, f: Facility, catalog: &[Item]) -> Option<Market> {
    if matches!(f, Facility::Gate(_) | Facility::Asteroid(_)) {
        return None;
    }
    let mut rng = Rng::new(mix(mix(seed, 0x3a4c_e700 + system as u64), key(f)));
    // Bans: vice first.
    let banned: Vec<Category> = [(Category::Narcotics, 0.7), (Category::Weapons, 0.4), (Category::Artifacts, 0.25), (Category::Biologics, 0.1), (Category::Robots, 0.1)]
        .into_iter()
        .filter_map(|(c, p)| (rng.range(0.0, 1.0) < p).then_some(c))
        .collect();
    let (makes, wants) = leanings(sys, f);
    let pick = |from: &[Category], count: usize, rng: &mut Rng| -> Vec<usize> {
        let pool: Vec<usize> = catalog.iter().filter(|i| from.contains(&i.category) && !banned.contains(&i.category)).map(|i| i.id).collect();
        let mut chosen = Vec::new();
        let mut pool = pool;
        while chosen.len() < count && !pool.is_empty() {
            let j = (rng.range(0.0, pool.len() as f64) as usize).min(pool.len() - 1);
            chosen.push(pool.swap_remove(j));
        }
        chosen
    };
    let mut made = pick(makes, 24 + rng.range(0.0, 16.0) as usize, &mut rng);
    let mut wanted = pick(wants, 24 + rng.range(0.0, 16.0) as usize, &mut rng);
    // Now and then something off the usual list, the illicit included (where it's allowed).
    let all: Vec<Category> = Category::all().collect();
    made.extend(pick(&all, 3, &mut rng));
    wanted.extend(pick(&all, 3, &mut rng));
    let mut offers = Vec::new();
    for (ids, side) in [(made, Side::Sells), (wanted, Side::Buys)] {
        for id in ids {
            if offers.iter().any(|o: &Offer| o.item == id) {
                continue;
            }
            let item = &catalog[id];
            let (lo, hi) = if side == Side::Sells { (0.75, 0.95) } else { (1.05, 1.35) };
            let base = item.price * rng.range(lo, hi);
            // A market's appetite is about `LOT_VALUE` of each line: many
            // units of cheap bulk, a few of luxuries.
            let usual = (LOT_VALUE / item.price).clamp(3.0, 400.0) * rng.range(0.5, 1.5);
            offers.push(Offer { item: id, side, base, usual: usual.round() });
        }
    }
    offers.sort_by_key(|o| (o.side == Side::Buys, catalog[o.item].category, o.item));
    let wants = wants.iter().copied().filter(|c| !banned.contains(c)).collect();
    Some(Market { offers, banned, wants })
}

/// How a settled market trades `item`: its listed offer, else as it trades
/// that kind (any variety of what it makes or uses).
fn place_offer(m: &Market, place: &crate::economy::Place, item: &Item) -> Option<Offer> {
    m.offers.iter().find(|o| o.item == item.id).copied().or_else(|| {
        let c = item.category;
        place.trades(c).then(|| Offer { item: item.id, side: if place.sells(c) { Side::Sells } else { Side::Buys }, base: item.price, usual: (place.target(c) * 1000.0 / item.mass).round().max(1.0) })
    })
}

/// The price of an offer at `level` (stock or demand left).
fn prices(o: &Offer, level: f64) -> (Option<f64>, f64) {
    let fill = (level / o.usual).clamp(0.0, 2.0);
    match o.side {
        // Scarce stock is dear; it buys back below its selling price.
        Side::Sells => {
            let p = o.base * (1.0 + 0.6 * (1.0 - fill));
            (Some(p), p * BUY_BACK)
        }
        // Keen while demand lasts, less as it's met.
        Side::Buys => (None, o.base * (0.4 + 0.6 * fill)),
    }
}

impl Market {
    /// The market's quotes, its `state` brought forward to `now`.
    pub fn quotes(&self, state: &mut MarketState, now: f64) -> Vec<Quote> {
        self.recover(state, now);
        self.offers
            .iter()
            .map(|o| {
                let level = state.levels.get(&o.item).copied().unwrap_or(o.usual);
                let (buy, sell) = prices(o, level);
                Quote { offer: *o, level, buy, sell }
            })
            .collect()
    }

    /// How it trades `item`: its listed offer, or a general one if it wants the kind.
    pub fn offer_for(&self, item: &Item) -> Option<Offer> {
        if self.banned.contains(&item.category) {
            return None;
        }
        self.offers.iter().find(|o| o.item == item.id).copied().or_else(|| {
            self.wants.contains(&item.category).then(|| Offer { item: item.id, side: Side::Buys, base: item.price * GENERAL_PREMIUM, usual: general_demand(item) })
        })
    }

    /// The quote for `item` now, if it trades it at all.
    pub fn quote_for(&self, state: &mut MarketState, now: f64, item: &Item) -> Option<Quote> {
        self.recover(state, now);
        let o = self.offer_for(item)?;
        let level = state.levels.get(&o.item).copied().unwrap_or(o.usual);
        let (buy, sell) = prices(&o, level);
        Some(Quote { offer: o, level, buy, sell })
    }

    /// Stock and demand drift back toward the usual since `state.updated`.
    fn recover(&self, state: &mut MarketState, now: f64) {
        let dt = (now - state.updated).max(0.0);
        let k = 1.0 - (-dt / RECOVERY).exp();
        for (item, l) in state.levels.iter_mut() {
            // Unlisted lines (general demand) recover toward their own usual level, kept alongside.
            let usual = self.offers.iter().find(|o| o.item == *item).map_or_else(|| state.usual.get(item).copied().unwrap_or(*l), |o| o.usual);
            *l += (usual - *l) * k;
        }
        state.updated = now;
    }

    /// Trade `units` of `item` (positive: buy from the market, negative: sell
    /// to it) for `pilot`, whose cargo weighs `cargo` (kg): the credits moved
    /// (positive: paid by the pilot), or why not. Credits and goods move in
    /// the ledger (this market's account: `me`), for `cause`.
    #[allow(clippy::too_many_arguments)]
    pub fn trade(&self, state: &mut MarketState, now: f64, item: &Item, units: i64, pilot: BodyId, cargo: f64, ledger: &mut Ledger, me: Party, tick: Tick, cause: Cause) -> Result<f64, String> {
        self.recover(state, now);
        if self.banned.contains(&item.category) {
            return Err(format!("{} IS ILLEGAL HERE", item.category.name()));
        }
        let Some(o) = self.offer_for(item) else { return Err("NOT TRADED HERE".into()) };
        state.usual.entry(item.id).or_insert(o.usual);
        let level = state.levels.get(&item.id).copied().unwrap_or(o.usual);
        let (buy, sell) = prices(&o, level);
        let who = Party::Pilot(pilot);
        if units > 0 {
            let Some(price) = buy else { return Err("NOT SOLD HERE - ONLY BOUGHT".into()) };
            let n = units as f64;
            if level < n {
                return Err(format!("ONLY {level:.0} IN STOCK"));
            }
            let cost = price * n;
            if ledger.credits(who) < cost {
                return Err("NOT ENOUGH CREDITS".into());
            }
            if cargo + item.mass * n > HOLD_CAPACITY {
                return Err("HOLD FULL".into());
            }
            ledger.transfer(who, me, Asset::Credits, cost, tick, cause)?;
            ledger.transfer(me, who, Asset::Goods(item.id), n, tick, cause)?;
            state.levels.insert(item.id, level - n);
            Ok(cost)
        } else {
            let n = (-units) as f64;
            if ledger.balance(who, Asset::Goods(item.id)) + 1e-6 < n {
                return Err("NOT IN THE HOLD".into());
            }
            if o.side == Side::Buys && level < n {
                return Err(format!("THEY ONLY WANT {level:.0} MORE"));
            }
            let paid = sell * n;
            ledger.transfer(who, me, Asset::Goods(item.id), n, tick, cause)?;
            ledger.transfer(me, who, Asset::Credits, paid, tick, cause)?;
            // Selling back raises their stock; meeting demand lowers what's left of it.
            let next = if o.side == Side::Sells { level + n } else { level - n };
            state.levels.insert(item.id, next);
            Ok(-paid)
        }
    }
}

/// The market service: every market met so far (generated from the seed on
/// first look), the state of those traded with, and the trades — booked in
/// the ledger.
#[derive(Clone, Debug)]
pub struct Markets {
    seed: u64,
    goods: Arc<Vec<Item>>,
    markets: HashMap<(usize, Facility), Arc<Market>>,
    states: HashMap<(usize, Facility), MarketState>,
    /// The settled systems' economy: their markets trade from it.
    pub economy: crate::economy::Economy,
}

/// A request to trade: who, where (and where it physically is), what.
#[derive(Clone, Copy, Debug)]
pub struct Order {
    pub pilot: BodyId,
    pub system: usize,
    pub market: Facility,
    /// Where the pilot's ship is docked or landed, as the core reports.
    pub docked_at: Option<Facility>,
    /// What its cargo weighs now (kg).
    pub cargo: f64,
    pub item: usize,
    pub units: i64,
}

impl Markets {
    pub fn new(seed: u64, goods: Arc<Vec<Item>>) -> Self {
        Markets { seed, goods, markets: HashMap::new(), states: HashMap::new(), economy: Default::default() }
    }

    pub fn goods(&self) -> &[Item] {
        &self.goods
    }

    /// The market at `f` in `system` (`sys`), if there is one.
    pub fn market(&mut self, system: usize, sys: &StarSystem, f: Facility) -> Option<Arc<Market>> {
        if let Some(m) = self.markets.get(&(system, f)) {
            return Some(m.clone());
        }
        let m = Arc::new(match self.economy.place(system, f) {
            Some(place) => place_market(self.seed, place, &self.goods),
            None => market(self.seed, system, sys, f, &self.goods)?,
        });
        self.markets.insert((system, f), m.clone());
        Some(m)
    }

    fn state(&self, system: usize, f: Facility, now: f64) -> MarketState {
        self.states.get(&(system, f)).cloned().unwrap_or(MarketState { updated: now, ..Default::default() })
    }

    /// The market's quotes now.
    pub fn quotes(&mut self, system: usize, sys: &StarSystem, f: Facility, now: f64) -> Vec<Quote> {
        let Some(m) = self.market(system, sys, f) else { return Vec::new() };
        if let Some(place) = self.economy.place(system, f) {
            return m.offers.iter().map(|o| place_quote(place, o, &self.goods[o.item])).collect();
        }
        let mut state = self.state(system, f, now);
        m.quotes(&mut state, now)
    }

    /// Its quote for one item, if it trades it (listed, or of a kind it wants).
    pub fn quote_for(&mut self, system: usize, sys: &StarSystem, f: Facility, item: usize, now: f64) -> Option<Quote> {
        let m = self.market(system, sys, f)?;
        if let Some(place) = self.economy.place(system, f) {
            return place_offer(&m, place, &self.goods[item]).map(|o| place_quote(place, &o, &self.goods[item]));
        }
        let mut state = self.state(system, f, now);
        m.quote_for(&mut state, now, &self.goods[item])
    }

    /// Its quotes for several items at once (None: not traded there).
    pub fn quotes_for(&mut self, system: usize, sys: &StarSystem, f: Facility, items: &[usize], now: f64) -> Vec<Option<Quote>> {
        let Some(m) = self.market(system, sys, f) else { return vec![None; items.len()] };
        if let Some(place) = self.economy.place(system, f) {
            return items.iter().map(|&i| place_offer(&m, place, &self.goods[i]).map(|o| place_quote(place, &o, &self.goods[i]))).collect();
        }
        let mut state = self.state(system, f, now);
        items.iter().map(|&i| m.quote_for(&mut state, now, &self.goods[i])).collect()
    }

    /// Carry out an order at `now` (tick `tick`, because of `cause`): the
    /// credits moved (positive: paid by the pilot), or why not. Only while
    /// docked or landed at that market.
    pub fn trade(&mut self, ledger: &mut Ledger, sys: &StarSystem, o: Order, now: f64, tick: Tick, cause: Cause) -> Result<f64, String> {
        if o.docked_at != Some(o.market) {
            return Err("DOCK OR LAND THERE TO TRADE".into());
        }
        let m = self.market(o.system, sys, o.market).ok_or("NO MARKET")?;
        if self.economy.place(o.system, o.market).is_some() {
            return self.trade_at_place(&m, ledger, o, tick, cause);
        }
        let state = self.states.entry((o.system, o.market)).or_insert_with(|| MarketState { updated: now, ..Default::default() });
        m.trade(state, now, &self.goods[o.item], o.units, o.pilot, o.cargo, ledger, Party::Market(o.system, o.market), tick, cause)
    }

    /// A trade at a settled market: from (or into) its place's stock.
    fn trade_at_place(&mut self, m: &Market, ledger: &mut Ledger, o: Order, tick: Tick, cause: Cause) -> Result<f64, String> {
        let item = &self.goods[o.item];
        let place = self.economy.place_mut(o.system, o.market).ok_or("NO MARKET")?;
        let offer = place_offer(m, place, item).ok_or("NOT TRADED HERE")?;
        let q = place_quote(place, &offer, item);
        let (who, me) = (Party::Pilot(o.pilot), Party::Market(o.system, o.market));
        let t = item.mass * o.units.unsigned_abs() as f64 / 1000.0;
        if o.units > 0 {
            let n = o.units as f64;
            let Some(price) = q.buy else { return Err("NOT SOLD HERE - ONLY BOUGHT".into()) };
            if q.level < n {
                return Err(format!("ONLY {:.0} IN STOCK", q.level));
            }
            let cost = price * n;
            if ledger.credits(who) < cost {
                return Err("NOT ENOUGH CREDITS".into());
            }
            if o.cargo + item.mass * n > HOLD_CAPACITY {
                return Err("HOLD FULL".into());
            }
            ledger.transfer(who, me, Asset::Credits, cost, tick, cause)?;
            ledger.transfer(me, who, Asset::Goods(item.id), n, tick, cause)?;
            place.take(item.category, t);
            Ok(cost)
        } else {
            let n = (-o.units) as f64;
            if ledger.balance(who, Asset::Goods(item.id)) + 1e-6 < n {
                return Err("NOT IN THE HOLD".into());
            }
            if q.offer.side == Side::Buys && q.level < n {
                return Err(format!("THEY ONLY HAVE ROOM FOR {:.0} MORE", q.level));
            }
            let paid = q.sell * n;
            ledger.transfer(who, me, Asset::Goods(item.id), n, tick, cause)?;
            ledger.transfer(me, who, Asset::Credits, paid, tick, cause)?;
            place.put(item.category, t);
            Ok(-paid)
        }
    }

    /// Fill a tank at `market` (where the pilot is docked or landed): up to
    /// `want` tonnes of fuel, as far as the place has it (a settled place,
    /// from its stock; out on the frontier, at a frontier price) and the
    /// pilot's credits go. The tonnes and the credits paid, or why not.
    #[allow(clippy::too_many_arguments)]
    pub fn refuel(&mut self, ledger: &mut Ledger, system: usize, market: Facility, docked_at: Option<Facility>, pilot: BodyId, want: f64, tick: Tick, cause: Cause) -> Result<(f64, f64), String> {
        if docked_at != Some(market) {
            return Err("DOCK OR LAND THERE TO REFUEL".into());
        }
        if matches!(market, Facility::Gate(_) | Facility::Asteroid(_)) {
            return Err("NO FUEL HERE".into());
        }
        let who = Party::Pilot(pilot);
        let (price, stock) = match self.economy.place(system, market) {
            Some(place) => (FUEL_PRICE * place.factor(Category::Fuel).unwrap_or(1.0), place.stock_of(Category::Fuel)),
            None => (FUEL_PRICE * FRONTIER, f64::INFINITY),
        };
        let t = want.min(stock).min(ledger.credits(who) / price).max(0.0);
        if t < 0.01 {
            return Err(if stock < 0.01 { "NO FUEL IN STOCK".into() } else { "NOT ENOUGH CREDITS".into() });
        }
        let cost = t * price;
        ledger.transfer(who, Party::Market(system, market), Asset::Credits, cost, tick, cause)?;
        if let Some(place) = self.economy.place_mut(system, market) {
            place.take(Category::Fuel, t);
        }
        Ok((t, cost))
    }

    /// The states of the markets traded with (to save).
    pub fn states(&self) -> &HashMap<(usize, Facility), MarketState> {
        &self.states
    }
}

/// What the goods in a hold weigh (kg).
pub fn cargo_mass(goods: &[Item], hold: &[(usize, u32)]) -> f64 {
    hold.iter().map(|&(g, n)| goods[g].mass * n as f64).sum()
}

#[cfg(test)]
mod tests {
    use super::*;
    use universe_world::goods::catalog;
    use universe_world::traffic::facilities;
    use universe_world::World;

    #[test]
    fn markets_are_local_varied_and_prices_move_with_trade() {
        let w = World::new(1984);
        let sys = w.system(w.home_system);
        let goods = catalog(1984);
        let fs = facilities(&sys);
        assert!(fs.len() >= 2);
        let a = market(1984, w.home_system, &sys, fs[0], &goods).unwrap();
        let b = market(1984, w.home_system, &sys, fs[1], &goods).unwrap();
        assert_ne!(a.offers, b.offers, "each market trades its own goods");
        assert!(a.offers.len() >= 40 && a.offers.len() < 100, "{} offers", a.offers.len());
        assert!(a.offers.iter().all(|o| !a.banned.contains(&goods[o.item].category)));
        assert_eq!(a, market(1984, w.home_system, &sys, fs[0], &goods).unwrap(), "the same every time");

        // Buy the first thing it sells: its price rises; sell it back for less.
        let mut state = MarketState::default();
        let mut ledger = Ledger::default();
        let (me, shop) = (Party::Pilot(1), Party::Market(w.home_system, fs[0]));
        ledger.settle(me, Asset::Credits, 1.0e6, 0, Cause::Rules);
        let o = *a.offers.iter().find(|o| o.side == Side::Sells).unwrap();
        let item = &goods[o.item];
        let before = a.quotes(&mut state, 0.0).into_iter().find(|q| q.offer.item == o.item).unwrap();
        let n = (before.level * 0.5).floor().min(5_000.0 / item.mass).floor().max(1.0) as i64;
        let paid = a.trade(&mut state, 0.0, item, n, 1, 0.0, &mut ledger, shop, 1, Cause::Rules).unwrap();
        assert!((paid - before.buy.unwrap() * n as f64).abs() < 1e-6);
        assert_eq!(ledger.hold(1), vec![(o.item, n as u32)]);
        assert!((cargo_mass(&goods, &ledger.hold(1)) - item.mass * n as f64).abs() < 1e-6, "the hold weighs what's in it");
        let after = a.quotes(&mut state, 0.0).into_iter().find(|q| q.offer.item == o.item).unwrap();
        assert!(after.buy.unwrap() > before.buy.unwrap(), "scarcer, dearer");
        let got = -a.trade(&mut state, 0.0, item, -n, 1, item.mass * n as f64, &mut ledger, shop, 2, Cause::Rules).unwrap();
        assert!(got < paid, "bought back for less");
        assert!(ledger.hold(1).is_empty());
        assert!(ledger.balanced(), "nothing made or lost");
        // A day later the stock is back.
        let later = a.quotes(&mut state, 24.0 * 3600.0).into_iter().find(|q| q.offer.item == o.item).unwrap();
        assert!((later.level - o.usual).abs() < o.usual * 0.05);
        // Banned goods don't trade.
        if let Some(c) = a.banned.first() {
            let banned = goods.iter().find(|i| i.category == *c).unwrap();
            assert!(a.trade(&mut state, 0.0, banned, 1, 1, 0.0, &mut ledger, shop, 3, Cause::Rules).is_err());
        }
    }
}
