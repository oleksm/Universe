//! The market service's side of trading: pilots' trades (decided by the
//! market, booked in the ledger), quotes on request, and the trade log. What
//! a trader buys and sells, and where it goes, is its own (see `operator`).

use universe_services::market::{cargo_mass, Order};
use universe_services::Party;
use universe_world::traffic::{docked_at, facilities};
use universe_world::Facility;

use crate::universe::Universe;
use universe_services::records::{Deal, TradeRecord};

/// What a new settler starts with (credits).
pub const SETTLER_CREDITS: f64 = 3000.0;

impl Universe {
    /// Pilot `pilot` (the player's 0, craft i: i + 1) asks the market at `f`
    /// to trade `units` of `item`: the market service decides, the ledger
    /// books it (the request's cause on every entry), and the ship's cargo
    /// mass follows what's in its hold. Credits paid (negative: received),
    /// or why not.
    pub(crate) fn pilot_trade(&mut self, pilot: usize, f: Facility, item: usize, units: i64) -> Result<f64, String> {
        let (system, ship) = if pilot == crate::combat::PLAYER { (self.ship_system, &self.ship) } else { (self.crafts[pilot - 1].system, &self.crafts[pilot - 1].ship) };
        let sys = self.world.system(system);
        let order = Order { pilot, system, market: f, docked_at: docked_at(&sys, ship), room: ship.hold_room(), item, units };
        self.messages += 1;
        let cause = universe_protocol::Cause::Message { sender: pilot as u64, id: self.messages };
        let r = self.markets.trade(&mut self.ledger, &sys, order, self.world.time, self.tick, cause);
        if r.is_ok() {
            // The core: the hold weighs what's in it.
            let mass = cargo_mass(&self.world.goods, &self.ledger.hold(pilot));
            match pilot {
                crate::combat::PLAYER => self.ship.cargo = mass,
                id => self.crafts[id - 1].ship.cargo = mass,
            }
        }
        r
    }

    /// Ore ship `id` dug this tick (as its step logged it): the world
    /// remembers it's gone from the rock, the ledger puts it in the hold
    /// (from the world's account, because of the dig).
    pub(crate) fn book_mined(&mut self, id: usize, events: &[crate::Event]) {
        for (k, e) in events.iter().enumerate() {
            let crate::Event::Ship(universe_world::ShipEvent::Mined { field, rock, item }) = e else { continue };
            let Some((_, system, _)) = self.ship_by_id(id) else { return };
            self.world.dig(system, *field, *rock);
            // (The k-th of its mined tonnes this tick, as logged.)
            let n = events[..=k].iter().filter(|e| matches!(e, crate::Event::Ship(universe_world::ShipEvent::Mined { .. }))).count();
            let cause = self.mined_cause(id, n).unwrap_or(universe_protocol::Cause::Rules);
            let _ = self.ledger.transfer(Party::World, Party::Pilot(id), universe_services::ledger::Asset::Goods(*item), 1.0, self.tick, cause);
        }
    }

    /// The `n`-th of ship `id`'s `Mined` events in this tick's log, as a cause.
    fn mined_cause(&self, id: usize, n: usize) -> Option<universe_protocol::Cause> {
        let index = self.log.iter().enumerate().filter(|(_, (s, e))| *s == id && matches!(e, universe_world::ShipEvent::Mined { .. })).nth(n - 1)?.0;
        Some(universe_protocol::Cause::Event { tick: self.tick, index: index as u32 })
    }

    /// Pilot `id` fills its tank at `market`: the market service sells what
    /// it can (the ledger books it), and the core's tank takes it.
    pub(crate) fn refuel(&mut self, id: usize, market: Facility) -> Result<(f64, f64), String> {
        let Some((_, system, ship)) = self.ship_by_id(id) else { return Err("NO SHIP".into()) };
        let want = ship.spec().fuel_capacity - ship.fuel;
        if want < 0.01 {
            return Err("TANK FULL".into());
        }
        let docked = docked_at(&self.world.system(system), ship);
        self.messages += 1;
        let cause = universe_protocol::Cause::Message { sender: id as u64, id: self.messages };
        let (tonnes, cost) = self.markets.refuel(&mut self.ledger, system, market, docked, id, want / 1000.0, self.tick, cause)?;
        match id {
            crate::combat::PLAYER => self.ship.fuel += tonnes * 1000.0,
            i => self.crafts[i - 1].ship.fuel += tonnes * 1000.0,
        }
        Ok((tonnes, cost))
    }

    /// The player fills the tank where docked or landed (as its pilot would on arrival).
    pub fn refuel_player(&mut self) {
        let sys = self.world.system(self.ship_system);
        let Some(market) = docked_at(&sys, &self.ship) else { return };
        let e = match self.refuel(crate::combat::PLAYER, market) {
            Ok((tonnes, credits)) => universe_avionics::Event::Refuelled { tonnes, credits },
            Err(reason) if reason == "TANK FULL" => return,
            Err(reason) => universe_avionics::Event::Refused { reason: format!("REFUEL: {reason}") },
        };
        self.events.push(e);
    }

    /// Craft `i`'s credits, as the ledger has them.
    pub fn craft_credits(&self, i: usize) -> f64 {
        self.ledger.credits(Party::Pilot(crate::combat::craft_id(i)))
    }

    /// A pilot's request to the market service (see `vessel::Request`):
    /// the quotes in its system (answered by message), a trade, or a plan
    /// declared, recorded in the trade log.
    pub(crate) fn market_request(&mut self, id: usize, r: crate::vessel::Request) {
        use crate::vessel::Request;
        match r {
            Request::Quotes { system, market } => {
                if id == crate::combat::PLAYER {
                    return;
                }
                let answer = self.market_answer(id, system, market);
                self.tell(id - 1, crate::contract::Msg::Market(answer));
            }
            Request::Trade { market, item, units } => {
                if let Ok(amount) = self.pilot_trade(id, market, item, units) {
                    let deal = if units > 0 { Deal::Bought } else { Deal::Sold };
                    self.record_trade(id, market, deal, Some(item), units.unsigned_abs() as u32, amount.abs());
                }
            }
            Request::Declare { market, deal } => self.record_trade(id, market, deal, None, 0, 0.0),
            Request::Refuel { market } => {
                let _ = self.refuel(id, market);
            }
            _ => {}
        }
    }

    /// What the market service tells pilot `id` at `at`: every market's
    /// quotes in the system (as anyone there could see them), and its account.
    fn market_answer(&mut self, id: usize, system: usize, at: Facility) -> crate::contract::MarketAnswer {
        let sys = self.system(system);
        let now = self.world.time;
        let hold = self.ledger.hold(id);
        let held: Vec<usize> = hold.iter().map(|h| h.0).collect();
        let here = self.markets.quotes(system, &sys, at, now);
        let here_held = self.markets.quotes_for(system, &sys, at, &held, now);
        let mut items = held.clone();
        items.extend(here.iter().filter(|q| q.buy.is_some()).map(|q| q.offer.item).filter(|i| !held.contains(i)));
        let there = facilities(&sys).into_iter().filter(|&f| f != at).map(|f| (f, self.markets.quotes_for(system, &sys, f, &items, now))).collect();
        let (cargo, capacity) = self.ship_by_id(id).map_or((0.0, 0.0), |(_, _, s)| (s.cargo, s.spec().hold_capacity));
        crate::contract::MarketAnswer { system, at, here, here_held, items, there, credits: self.ledger.credits(Party::Pilot(id)), hold, cargo, capacity }
    }

    /// A trade (or a plan) in the log, as pilot `id` made it at `market`.
    fn record_trade(&mut self, id: usize, market: Facility, deal: Deal, item: Option<usize>, units: u32, amount: f64) {
        let Some((_, system, ship)) = self.ship_by_id(id) else { return };
        let cargo = ship.cargo;
        let trader = if id == crate::combat::PLAYER { "YOU".to_string() } else { self.crafts[id - 1].name.to_uppercase() };
        let sys = self.system(system);
        if item.is_some() {
            self.records.stats.trades += 1;
            self.records.stats.turnover += amount;
        }
        let record = TradeRecord {
            time: self.world.time,
            system,
            market: market.name(&sys),
            trader,
            bought: deal == Deal::Bought,
            deal,
            item: item.map_or(String::new(), |i| self.world.goods[i].name.to_uppercase()),
            units,
            amount,
            cargo,
            credits: self.ledger.credits(Party::Pilot(id)),
        };
        self.log_trade(record);
    }

    /// Keep a trade in the log (the last `TRADE_LOG`).
    pub(crate) fn log_trade(&mut self, record: TradeRecord) {
        self.records.trade(record);
    }
}

/// A module taken out at a refit fetches this share of its price.
pub const BUYBACK: f64 = 0.6;

impl crate::universe::Universe {
    /// Pilot `id` refits slot `slot` with `module` (None: empties it) at the
    /// station it's docked at: the module's price paid to the station's
    /// market, less what the one taken out fetches (`BUYBACK` of its price).
    /// Refused, with the reason, if it isn't docked at a station, the fit
    /// won't do (see `ClassSpec::assemble`), or it can't pay. The credits it cost.
    pub fn refit_as(&mut self, id: usize, slot: &str, module: Option<universe_world::content::Handle<universe_world::modules::Module>>) -> Result<f64, String> {
        use universe_services::{Asset, Party};
        let Some((_, system, ship)) = self.ship_by_id(id) else { return Err("NO SHIP".into()) };
        let sys = self.world.system(system);
        let Some(Facility::Station(station)) = universe_world::traffic::docked_at(&sys, ship) else { return Err("REFIT DOCKED AT A STATION".into()) };
        let c = universe_world::content::content();
        let mut fit = ship.fit.clone().unwrap_or_else(|| c.get(ship.class).fit.clone());
        let old = fit.iter().position(|(s, _)| s == slot);
        let taken = old.map(|i| fit[i].1);
        if taken == module {
            return Err("FITTED ALREADY".into());
        }
        match (old, module) {
            (Some(i), Some(m)) => fit[i].1 = m,
            (Some(i), None) => {
                fit.remove(i);
            }
            (None, Some(m)) => fit.push((slot.to_string(), m)),
            (None, None) => return Err("EMPTY ALREADY".into()),
        }
        let mut refitted = ship.clone();
        refitted.refit(fit)?;
        let cost = module.map_or(0.0, |m| c.get(m).price) - taken.map_or(0.0, |m| c.get(m).price * BUYBACK);
        let (me, market) = (Party::Pilot(id), Party::Market(system, Facility::Station(station)));
        let cause = universe_protocol::Cause::Rules;
        self.ledger.transfer(me, market, Asset::Credits, cost, self.tick, cause)?;
        match id {
            crate::combat::PLAYER => self.ship = refitted,
            _ => self.crafts[id - 1].ship = refitted,
        }
        Ok(cost)
    }

    /// The player refits slot `slot` (see `refit_as`); the cockpit is told.
    pub fn refit(&mut self, slot: &str, module: Option<universe_world::content::Handle<universe_world::modules::Module>>) -> Result<f64, String> {
        let r = self.refit_as(crate::combat::PLAYER, slot, module);
        let c = universe_world::content::content();
        let e = match &r {
            Ok(credits) => universe_avionics::Event::Refitted { slot: slot.to_string(), module: module.map(|m| c.get(m).name.clone()), credits: *credits },
            Err(reason) => universe_avionics::Event::Refused { reason: format!("REFIT: {reason}") },
        };
        self.events.push(e);
        r
    }
}
