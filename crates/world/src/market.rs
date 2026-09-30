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
//! credits. You trade only while docked or landed at the market.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::goods::{Category, Item};
use crate::rng::{mix, Rng};
use crate::ship::{Ship, ShipState};
use crate::system::{BodyKind, StarSystem};
use crate::terrain::TerrainKind;
use crate::traffic::Facility;

/// Most cargo a ship can carry (kg).
pub const HOLD_CAPACITY: f64 = 20_000.0;
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
pub const GENERAL_PREMIUM: f64 = 1.1;
/// Its usual demand for such goods (units).
const GENERAL_DEMAND: f64 = 100.0;

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
    }
}

/// The market at facility `f` in system `system` (`sys`), for the galaxy
/// `seed`'s `catalog`. Gates have none.
pub fn market(seed: u64, system: usize, sys: &StarSystem, f: Facility, catalog: &[Item]) -> Option<Market> {
    if matches!(f, Facility::Gate(_)) {
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
            let (lo, hi) = if side == Side::Sells { (0.55, 0.9) } else { (1.2, 1.9) };
            let base = item.price * rng.range(lo, hi);
            // Heavier, cheaper goods trade in bigger lots.
            let usual = (40_000.0 / item.mass.max(1.0)).clamp(5.0, 2000.0) * rng.range(0.5, 1.5);
            offers.push(Offer { item: id, side, base, usual: usual.round() });
        }
    }
    offers.sort_by_key(|o| (o.side == Side::Buys, catalog[o.item].category, o.item));
    let wants = wants.iter().copied().filter(|c| !banned.contains(c)).collect();
    Some(Market { offers, banned, wants })
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
            self.wants.contains(&item.category).then(|| Offer { item: item.id, side: Side::Buys, base: item.price * GENERAL_PREMIUM, usual: GENERAL_DEMAND })
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
            let usual = self.offers.iter().find(|o| o.item == *item).map_or(GENERAL_DEMAND, |o| o.usual);
            *l += (usual - *l) * k;
        }
        state.updated = now;
    }

    /// Trade `units` of `item` (positive: buy from the market, negative: sell
    /// to it) for a ship with `credits`: the credits moved (positive: paid by
    /// the ship), or why not. The ship's hold and mass change with it.
    pub fn trade(&self, state: &mut MarketState, now: f64, item: &Item, units: i64, ship: &mut Ship, credits: &mut f64) -> Result<f64, String> {
        self.recover(state, now);
        if self.banned.contains(&item.category) {
            return Err(format!("{} IS ILLEGAL HERE", item.category.name()));
        }
        let Some(o) = self.offer_for(item) else { return Err("NOT TRADED HERE".into()) };
        let level = state.levels.get(&item.id).copied().unwrap_or(o.usual);
        let (buy, sell) = prices(&o, level);
        if units > 0 {
            let Some(price) = buy else { return Err("NOT SOLD HERE - ONLY BOUGHT".into()) };
            let n = units as f64;
            if level < n {
                return Err(format!("ONLY {level:.0} IN STOCK"));
            }
            let cost = price * n;
            if *credits < cost {
                return Err("NOT ENOUGH CREDITS".into());
            }
            if ship.cargo + item.mass * n > HOLD_CAPACITY {
                return Err("HOLD FULL".into());
            }
            *credits -= cost;
            state.levels.insert(item.id, level - n);
            *ship.hold.entry(item.id).or_default() += units as u32;
            ship.cargo += item.mass * n;
            Ok(cost)
        } else {
            let n = (-units) as u32;
            let have = ship.hold.get(&item.id).copied().unwrap_or(0);
            if have < n {
                return Err("NOT IN THE HOLD".into());
            }
            if o.side == Side::Buys && level < n as f64 {
                return Err(format!("THEY ONLY WANT {level:.0} MORE"));
            }
            let paid = sell * n as f64;
            *credits += paid;
            // Selling back raises their stock; meeting demand lowers what's left of it.
            let next = if o.side == Side::Sells { level + n as f64 } else { level - n as f64 };
            state.levels.insert(item.id, next);
            if have == n {
                ship.hold.remove(&item.id);
            } else {
                ship.hold.insert(item.id, have - n);
            }
            ship.cargo = (ship.cargo - item.mass * n as f64).max(0.0);
            Ok(-paid)
        }
    }
}

/// The market a ship is docked or landed at, if any.
pub fn docked_at(sys: &StarSystem, ship: &Ship) -> Option<Facility> {
    let ShipState::Landed { body, local_position, .. } = ship.state else { return None };
    if sys.bodies[body].kind == BodyKind::Station {
        return Some(Facility::Station(body));
    }
    sys.port_at(body, local_position.normalize()).map(Facility::Spaceport)
}

/// Every market in a star system: its stations and spaceports.
pub fn facilities(sys: &StarSystem) -> Vec<Facility> {
    let stations = sys.bodies.iter().enumerate().filter(|(_, b)| b.kind == BodyKind::Station).map(|(i, _)| Facility::Station(i));
    stations.chain((0..sys.spaceports.len()).map(Facility::Spaceport)).collect()
}

#[cfg(test)]
mod tests {
    use glam::{DQuat, DVec3};

    use super::*;
    use crate::goods::catalog;
    use crate::World;

    #[test]
    fn markets_are_local_varied_and_prices_move_with_trade() {
        let mut w = World::new(1984);
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
        let mut ship = Ship::new(DVec3::ZERO, DVec3::ZERO, DQuat::IDENTITY);
        let mut credits = 1.0e6;
        let o = *a.offers.iter().find(|o| o.side == Side::Sells).unwrap();
        let item = &goods[o.item];
        let before = a.quotes(&mut state, 0.0).into_iter().find(|q| q.offer.item == o.item).unwrap();
        let n = (5_000.0 / item.mass).floor().max(1.0) as i64;
        let paid = a.trade(&mut state, 0.0, item, n, &mut ship, &mut credits).unwrap();
        assert!((paid - before.buy.unwrap() * n as f64).abs() < 1e-6);
        assert_eq!(ship.hold[&o.item], n as u32);
        assert!((ship.cargo - item.mass * n as f64).abs() < 1e-6, "the hold weighs on the ship");
        let after = a.quotes(&mut state, 0.0).into_iter().find(|q| q.offer.item == o.item).unwrap();
        assert!(after.buy.unwrap() > before.buy.unwrap(), "scarcer, dearer");
        let got = -a.trade(&mut state, 0.0, item, -n, &mut ship, &mut credits).unwrap();
        assert!(got < paid, "bought back for less");
        assert!(ship.hold.is_empty() && ship.cargo.abs() < 1e-6);
        // A day later the stock is back.
        let later = a.quotes(&mut state, 24.0 * 3600.0).into_iter().find(|q| q.offer.item == o.item).unwrap();
        assert!((later.level - o.usual).abs() < o.usual * 0.05);
        // Banned goods don't trade.
        if let Some(c) = a.banned.first() {
            let banned = goods.iter().find(|i| i.category == *c).unwrap();
            assert!(a.trade(&mut state, 0.0, banned, 1, &mut ship, &mut credits).is_err());
        }
    }
}
