//! Markets: where a settlement has a warehouse the exchange has approved,
//! what lies in it is on the market (see `economy`), as stock (a good in bulk
//! is its stock) and what's made of it (parts, equipment, hulls): no bare
//! good, element or material is sold. It sells what it holds
//! and buys any stock while it has room: what the settlement's works take,
//! at prices that follow its stock of it; anything else, for less as the
//! warehouse fills. Every trade moves the stock in or out of the warehouse,
//! and books it and the credits in the ledger. You trade only while docked
//! or landed there (a physical fact the core reports).
//!
//! Places the registry doesn't describe have no market. Fuel is sold
//! anywhere a ship can dock: from the warehouse where there is one with fuel
//! in it, else brought in from outside the economy at `FUEL_PRICE`.

use std::sync::Arc;

use universe_protocol::{BodyId, Cause, Tick};
use universe_world::goods::Item;
use universe_world::system::StarSystem;
use universe_world::traffic::Facility;

use crate::ledger::{Asset, Ledger, Party};

/// Which way a market trades an item.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Side {
    /// It has some: sells to you (and buys more).
    Sells,
    /// It has none: only buys from you.
    Buys,
}

/// An item a market trades, and how.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Offer {
    pub item: usize,
    pub side: Side,
    /// Its reference price (credits a tonne).
    pub base: f64,
    /// The stock it aims to hold (t): what its works take over the cover; else what it has.
    pub usual: f64,
}

/// An offer as it stands now.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Quote {
    pub offer: Offer,
    /// In stock (selling) or room for (buying), tonnes.
    pub level: f64,
    /// Price a tonne if you buy from it, and what it pays you a tonne.
    pub buy: Option<f64>,
    pub sell: f64,
}

/// Ship fuel's price where it's brought in from outside the economy (credits a tonne).
pub const FUEL_PRICE: f64 = 60.0;

#[derive(Clone, Debug)]
pub struct Markets {
    goods: Arc<Vec<Item>>,
    /// The settlements' economy: their markets trade from it.
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
    /// Room left in its hold (kg) and space (m³), as the core reports.
    pub room: f64,
    pub space: f64,
    pub item: usize,
    pub units: i64,
}

impl Markets {
    pub fn new(goods: Arc<Vec<Item>>) -> Self {
        Markets { goods, economy: Default::default() }
    }

    pub fn goods(&self) -> &[Item] {
        &self.goods
    }

    /// Run the settlements' facilities to `now` (see `Economy::step_to`).
    pub fn step(&mut self, now: f64, land: &mut crate::land::LandOffice, ledger: &mut Ledger, tick: Tick) {
        self.economy.step_to(now, land, ledger, &self.goods, tick);
    }

    /// Is there a market at `f`?
    pub fn has_market(&self, system: usize, f: Facility) -> bool {
        self.economy.place(system, f).is_some_and(|p| p.warehouse.is_some())
    }

    fn quote(&self, system: usize, f: Facility, item: usize) -> Option<Quote> {
        let place = self.economy.place(system, f).filter(|p| p.warehouse.is_some())?;
        let it = &self.goods[item];
        // (What is sold is stock, and what's made of it: no bare good, element or material.)
        if !["stock.", "part.", "equipment.", "hull."].iter().any(|k| it.key.starts_with(k)) {
            return None;
        }
        let p = place.price(it);
        let side = if p.ask.is_some() { Side::Sells } else { Side::Buys };
        let usual = if p.wanted { place.need(item) * crate::economy::COVER_DAYS / it.mass } else { p.stock };
        Some(Quote { offer: Offer { item, side, base: it.price, usual }, level: if side == Side::Sells { p.stock.floor() } else { p.room.floor() }, buy: p.ask, sell: p.bid })
    }

    /// The market's quotes now: what it holds, and what its works take.
    pub fn quotes(&mut self, system: usize, _sys: &StarSystem, f: Facility, _now: f64) -> Vec<Quote> {
        let Some(place) = self.economy.place(system, f).filter(|p| p.warehouse.is_some()) else { return Vec::new() };
        let mut items: Vec<usize> = place.stock.stock.keys().copied().chain(place.wants.iter().map(|(i, _)| *i)).collect();
        items.sort_unstable();
        items.dedup();
        let mut q: Vec<Quote> = items.into_iter().filter_map(|i| self.quote(system, f, i)).collect();
        q.sort_by_key(|q| (q.offer.side == Side::Buys, self.goods[q.offer.item].category.map_or(usize::MAX, |c| c.index()), q.offer.item));
        q
    }

    pub fn quote_for(&mut self, system: usize, _sys: &StarSystem, f: Facility, item: usize, _now: f64) -> Option<Quote> {
        self.quote(system, f, item)
    }

    pub fn quotes_for(&mut self, system: usize, _sys: &StarSystem, f: Facility, items: &[usize], _now: f64) -> Vec<Option<Quote>> {
        items.iter().map(|&i| self.quote(system, f, i)).collect()
    }

    /// Trade `o.units` tonnes (bought if positive, sold if negative).
    pub fn trade(&mut self, ledger: &mut Ledger, _sys: &StarSystem, o: Order, _now: f64, tick: Tick, cause: Cause) -> Result<f64, String> {
        if o.docked_at != Some(o.market) {
            return Err("DOCK OR LAND THERE TO TRADE".into());
        }
        let q = self.quote(o.system, o.market, o.item).ok_or("NO MARKET HERE")?;
        let item = self.goods[o.item].clone();
        // (The exchange's market, or at a rig's dock its owner.)
        let me = self.economy.place(o.system, o.market).map_or(Party::Market(o.system, o.market), |p| p.trader);
        let duty = self.economy.place(o.system, o.market).map_or(0.0, |p| p.duty);
        let who = Party::Pilot(o.pilot);
        let n = o.units.unsigned_abs() as f64;
        let kg = item.mass * n;
        if o.units > 0 {
            let Some(price) = q.buy else { return Err("NONE IN STOCK".into()) };
            if q.level < n {
                return Err(format!("ONLY {:.0} IN STOCK", q.level));
            }
            let cost = price * n;
            if ledger.credits(who) < cost {
                return Err("NOT ENOUGH CREDITS".into());
            }
            if kg > o.room {
                return Err("HOLD FULL".into());
            }
            if kg / 1000.0 / item.bulk_density > o.space + 1e-9 {
                return Err("NO SPACE IN THE HOLD".into());
            }
            ledger.transfer(who, me, Asset::Credits, cost, tick, cause)?;
            ledger.transfer(me, who, Asset::Goods(item.id), n, tick, cause)?;
            // (The seller owes the administration its duty on the sale.)
            ledger.transfer(me, Party::Administration(o.system), Asset::Credits, cost * duty, tick, cause)?;
            self.economy.take(o.system, o.market, item.id, kg);
            Ok(cost)
        } else {
            if ledger.balance(who, Asset::Goods(item.id)) + 1e-6 < n {
                return Err("NOT IN THE HOLD".into());
            }
            let room = self.economy.place(o.system, o.market).map_or(0.0, |p| p.stock.free());
            if kg > room + 1e-6 {
                return Err(format!("THEY ONLY HAVE ROOM FOR {:.0} T", (room / 1000.0).floor()));
            }
            if q.sell <= 0.0 {
                return Err("THEY WON'T BUY IT".into());
            }
            let paid = q.sell * n;
            ledger.transfer(who, me, Asset::Goods(item.id), n, tick, cause)?;
            ledger.transfer(me, who, Asset::Credits, paid, tick, cause)?;
            ledger.transfer(who, Party::Administration(o.system), Asset::Credits, paid * duty, tick, cause)?;
            self.economy.put(o.system, o.market, item.id, kg);
            Ok(-paid * (1.0 - duty))
        }
    }

    /// Fill a tank at `market` (where the pilot is docked or landed): up to
    /// `want` tonnes of fuel, as far as the pilot's credits go: from the
    /// warehouse's fuel where it has some, else brought in. The tonnes and
    /// the credits paid, or why not.
    #[allow(clippy::too_many_arguments)]
    pub fn refuel(&mut self, ledger: &mut Ledger, system: usize, market: Facility, docked_at: Option<Facility>, pilot: BodyId, want: f64, tick: Tick, cause: Cause) -> Result<(f64, f64), String> {
        if docked_at != Some(market) {
            return Err("DOCK OR LAND THERE TO REFUEL".into());
        }
        if matches!(market, Facility::Gate(_) | Facility::Asteroid(_)) {
            return Err("NO FUEL HERE".into());
        }
        let who = Party::Pilot(pilot);
        let fuel = universe_world::goods::Category::fuel();
        let stocked = self.goods.iter().filter(|i| i.category == Some(fuel)).find_map(|i| Some((i.id, self.quote(system, market, i.id)?)).filter(|(_, q)| q.buy.is_some()));
        let (price, stock) = match stocked {
            Some((_, q)) => (q.buy.unwrap_or(FUEL_PRICE), q.level),
            None => (FUEL_PRICE, f64::INFINITY),
        };
        let t = want.min(stock).min(ledger.credits(who) / price).max(0.0);
        if t < 0.01 {
            return Err("NOT ENOUGH CREDITS".into());
        }
        let cost = t * price;
        let seller = self.economy.place(system, market).map_or(Party::Market(system, market), |p| p.trader);
        ledger.transfer(who, seller, Asset::Credits, cost, tick, cause)?;
        let duty = self.economy.place(system, market).map_or(0.0, |p| p.duty);
        ledger.transfer(seller, Party::Administration(system), Asset::Credits, cost * duty, tick, cause)?;
        if let Some((item, _)) = stocked {
            self.economy.take(system, market, item, t * 1000.0);
        }
        Ok((t, cost))
    }
}

/// What the goods in a hold weigh (kg).
pub fn cargo_mass(goods: &[Item], hold: &[(usize, u32)]) -> f64 {
    hold.iter().map(|&(g, n)| goods[g].mass * n as f64).sum()
}

/// The room what's in the hold takes (m³).
pub fn cargo_volume(goods: &[Item], hold: &[(usize, u32)]) -> f64 {
    hold.iter().map(|&(g, n)| goods[g].mass * n as f64 / 1000.0 / goods[g].bulk_density).sum()
}
