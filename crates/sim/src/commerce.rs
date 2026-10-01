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
        let order = Order { pilot, system, market: f, docked_at: docked_at(&sys, ship), cargo: ship.cargo, item, units };
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
                self.tell(id - 1, crate::pilots::Msg::Market(answer));
            }
            Request::Trade { market, item, units } => {
                if let Ok(amount) = self.pilot_trade(id, market, item, units) {
                    let deal = if units > 0 { Deal::Bought } else { Deal::Sold };
                    self.record_trade(id, market, deal, Some(item), units.unsigned_abs() as u32, amount.abs());
                }
            }
            Request::Declare { market, deal } => self.record_trade(id, market, deal, None, 0, 0.0),
            _ => {}
        }
    }

    /// What the market service tells pilot `id` at `at`: every market's
    /// quotes in the system (as anyone there could see them), and its account.
    fn market_answer(&mut self, id: usize, system: usize, at: Facility) -> crate::operator::MarketAnswer {
        let sys = self.system(system);
        let now = self.world.time;
        let hold = self.ledger.hold(id);
        let held: Vec<usize> = hold.iter().map(|h| h.0).collect();
        let here = self.markets.quotes(system, &sys, at, now);
        let here_held = self.markets.quotes_for(system, &sys, at, &held, now);
        let mut items = held.clone();
        items.extend(here.iter().filter(|q| q.buy.is_some()).map(|q| q.offer.item).filter(|i| !held.contains(i)));
        let there = facilities(&sys).into_iter().filter(|&f| f != at).map(|f| (f, self.markets.quotes_for(system, &sys, f, &items, now))).collect();
        let cargo = self.ship_by_id(id).map_or(0.0, |(_, _, s)| s.cargo);
        crate::operator::MarketAnswer { system, at, here, here_held, items, there, credits: self.ledger.credits(Party::Pilot(id)), hold, cargo }
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

