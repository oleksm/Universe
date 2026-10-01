//! Traders (a slice of the settlers) go where the profit is. At each stop a
//! trader sells what pays here (goods the market wants, or anything that beats
//! what it paid), then looks over every market in the system, as anyone in the
//! system can: what its cargo would fetch at each, and what it could buy here
//! to sell there (margin × units, within its credits, hold, the stock here and
//! the demand there). It buys for the best of them and heads there; if none
//! is worth `MIN_PROFIT`, it moves on through a gate to another system and
//! looks again. Every trade and every choice is recorded.

use universe_avionics::route::Stop;
use universe_services::market::{cargo_mass, Order, Side};
use universe_services::{Asset, Party};
use universe_world::ship::HOLD_CAPACITY;
use universe_world::traffic::{docked_at, facilities};
use universe_world::Facility;

use crate::universe::Universe;
use universe_services::records::{Deal, TradeRecord};

/// What a new settler starts with (credits).
pub const SETTLER_CREDITS: f64 = 3000.0;
/// A trip must promise at least this (credits), or the trader moves on to another system.
pub const MIN_PROFIT: f64 = 300.0;
/// Goods lines it buys for one trip.
const LINES: usize = 3;
/// Sell without demand only for at least this margin over what was paid.
const MARGIN: f64 = 1.05;
/// Past this share of the hold, sell what the market takes back, loss or not.
const STUCK: f64 = 0.8;

/// A planned trip: where to, what to buy for it (item, units), the profit expected.
type Trip = (Facility, Vec<(usize, u32)>, f64);



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

    /// Craft `i` trades at the market it has just arrived at.
    pub(crate) fn craft_trades(&mut self, i: usize) {
        let system = self.crafts[i].system;
        let sys = self.system(system);
        let Some(f) = docked_at(&sys, &self.crafts[i].ship) else { return };
        let market = f.name(&sys);
        let mut records = Vec::new();

        // Sell.
        let me = crate::combat::craft_id(i);
        let now = self.world.time;
        let held: Vec<(usize, u32)> = self.ledger.hold(me);
        for (item, have) in held {
            let Some(q) = self.markets.quote_for(system, &sys, f, item, now) else { continue };
            let paid = self.crafts[i].paid.get(&item).copied().unwrap_or(0.0);
            // A hold nearly full with nothing selling: take the buy-back, at a loss if need be.
            let stuck = self.crafts[i].ship.cargo > HOLD_CAPACITY * STUCK;
            let units = match q.offer.side {
                Side::Buys => have.min(q.level.floor().max(0.0) as u32),
                Side::Sells if q.sell >= paid * MARGIN || stuck => have,
                Side::Sells => 0,
            };
            if units == 0 {
                continue;
            }
            if let Ok(moved) = self.pilot_trade(me, f, item, -(units as i64)) {
                if self.ledger.balance(Party::Pilot(me), Asset::Goods(item)) < 0.5 {
                    self.crafts[i].paid.remove(&item);
                }
                records.push((false, item, units, -moved));
            }
        }

        // Look over the system's markets, buy for the best, and go.
        let plan = self.plan_trip(i, system, f);
        let mut decision = None;
        match plan {
            Some((to, buys, expect)) => {
                for (item, units) in buys {
                    let before = self.ledger.balance(Party::Pilot(me), Asset::Goods(item));
                    if let Ok(cost) = self.pilot_trade(me, f, item, units as i64) {
                        let c = &mut self.crafts[i];
                        let avg = c.paid.get(&item).copied().unwrap_or(0.0);
                        c.paid.insert(item, (avg * before + cost) / (before + units as f64));
                        records.push((true, item, units, cost));
                    }
                }
                decision = Some((Stop { system, target: to }, Deal::Heading { to: to.name(&sys).to_uppercase(), expect }));
            }
            None => {
                // Nothing worth it here: a neighbouring system (seeded, so reproducible), at one of its markets.
                let links = self.world.gate_links_of(system);
                let c = &self.crafts[i];
                let pick = crate::rng::mix(c.route_seed, self.records.stats.stops);
                if let Some((next, name)) = links.get(pick as usize % links.len().max(1)).cloned() {
                    let there = self.system(next);
                    let markets = facilities(&there);
                    if let Some(&target) = markets.get((pick >> 16) as usize % markets.len().max(1)) {
                        decision = Some((Stop { system: next, target }, Deal::MovingOn { to: name.to_uppercase() }));
                    }
                }
            }
        }
        if let Some((stop, _)) = &decision {
            // After the stop, on to there.
            let r = &mut self.crafts[i].avionics.route;
            r.stops.truncate(r.next + 1);
            r.stops.push(*stop);
        }

        for (bought, item, units, amount) in records {
            let c = &self.crafts[i];
            self.records.stats.trades += 1;
            self.records.stats.turnover += amount;
            let record = TradeRecord {
                time: self.world.time,
                system,
                market: market.clone(),
                trader: c.name.to_uppercase(),
                deal: if bought { Deal::Bought } else { Deal::Sold },
                bought,
                item: self.world.goods[item].name.to_uppercase(),
                units,
                amount,
                cargo: c.ship.cargo,
                credits: self.craft_credits(i),
            };
            self.log_trade(record);
        }
        if let Some((_, deal)) = decision {
            let c = &self.crafts[i];
            let record = TradeRecord {
                time: self.world.time,
                system,
                market: market.clone(),
                trader: c.name.to_uppercase(),
                deal,
                bought: false,
                item: String::new(),
                units: 0,
                amount: 0.0,
                cargo: c.ship.cargo,
                credits: self.craft_credits(i),
            };
            self.log_trade(record);
        }
    }

    /// The best trip from market `here` in `system` for trader `i`: where to,
    /// what to buy here for it, and the profit expected (cargo sold there over
    /// what it cost, plus the margin on what's bought for it). None if
    /// nothing reaches `MIN_PROFIT`.
    fn plan_trip(&mut self, i: usize, system: usize, here: Facility) -> Option<Trip> {
        let sys = self.system(system);
        let now = self.world.time;
        let here_quotes: Vec<_> = self.markets.quotes(system, &sys, here, now).into_iter().filter(|q| q.buy.is_some() && q.level >= 1.0).collect();
        let held: Vec<(usize, u32)> = self.ledger.hold(crate::combat::craft_id(i));
        let (credits, cargo) = (self.craft_credits(i), self.crafts[i].ship.cargo);
        let mut best: Option<Trip> = None;
        for there in facilities(&sys).into_iter().filter(|&m| m != here) {
            // What the cargo would fetch there, over what it cost.
            let held_ids: Vec<usize> = held.iter().map(|h| h.0).collect();
            let sells = self.markets.quotes_for(system, &sys, there, &held_ids, now);
            let mut value = 0.0;
            for ((item, n), q) in held.iter().zip(&sells) {
                if let Some(q) = q {
                    let units = if q.offer.side == Side::Buys { (*n as f64).min(q.level) } else { *n as f64 };
                    let paid = self.crafts[i].paid.get(item).copied().unwrap_or(0.0);
                    value += (q.sell - paid).max(0.0) * units;
                }
            }
            // What to buy here for there: the best margins per kilo first.
            let ids: Vec<usize> = here_quotes.iter().map(|q| q.offer.item).collect();
            let there_q = self.markets.quotes_for(system, &sys, there, &ids, now);
            let mut margins: Vec<(f64, usize, f64, f64, f64)> = here_quotes
                .iter()
                .zip(&there_q)
                .filter_map(|(h, t)| {
                    let t = t.as_ref()?;
                    let buy = h.buy?;
                    let margin = t.sell - buy;
                    let demand = if t.offer.side == Side::Buys { t.level } else { f64::INFINITY };
                    let mass = self.world.goods[h.offer.item].mass;
                    (margin > 0.0).then_some((margin / mass, h.offer.item, buy, margin, (h.level * 0.5).min(demand)))
                })
                .collect();
            margins.sort_by(|a, b| b.0.total_cmp(&a.0));
            let (mut room, mut money, mut gain, mut buys) = (HOLD_CAPACITY - cargo, credits, 0.0, Vec::new());
            for (_, item, buy, margin, most) in margins.into_iter().take(LINES) {
                let mass = self.world.goods[item].mass;
                let units = (room / mass).min(money / buy).min(most).floor();
                if units < 1.0 {
                    continue;
                }
                room -= units * mass;
                money -= units * buy;
                gain += units * margin;
                buys.push((item, units as u32));
            }
            let total = value + gain;
            if total >= MIN_PROFIT && best.as_ref().is_none_or(|b| total > b.2) {
                best = Some((there, buys, total));
            }
        }
        best
    }

    /// Keep a trade in the log (the last `TRADE_LOG`).
    pub(crate) fn log_trade(&mut self, record: TradeRecord) {
        self.records.trade(record);
    }
}

#[cfg(test)]
mod tests {
    use universe_world::Controls;

    use crate::universe::Universe;

    #[test]
    #[ignore]
    fn settlers_trade() {
        let mut u = Universe::new(1984);
        u.spawn_settlers(100, 99);
        for _ in 0..60 * 60 * 9 {
            u.step_world(1.0 / 60.0, 20.0, &Controls::default());
        }
        for r in u.records.trades.iter().rev().take(14) {
            eprintln!("{} AT {}: {:?} {} {} FOR {:.0} CR - CARGO {:.1} T, {:.0} CR", r.trader, r.market, r.deal, r.units, r.item, r.amount, r.cargo / 1000.0, r.credits);
        }
        let heading = u.records.trades.iter().filter(|r| matches!(r.deal, super::Deal::Heading { .. })).count();
        let moving = u.records.trades.iter().filter(|r| matches!(r.deal, super::Deal::MovingOn { .. })).count();
        eprintln!("recent decisions: {heading} trips planned, {moving} moves on to another system");
        let traders: Vec<f64> = (0..u.crafts.len()).filter(|&i| u.crafts[i].trader).map(|i| u.craft_credits(i)).collect();
        eprintln!("{} traders, {} pirates of {}", traders.len(), u.crafts.iter().filter(|c| c.avionics.pirate).count(), u.crafts.len());
        let mean = traders.iter().sum::<f64>() / traders.len() as f64;
        let (lo, hi) = traders.iter().fold((f64::MAX, f64::MIN), |(a, b), &x| (a.min(x), b.max(x)));
        let cargo: f64 = u.crafts.iter().filter(|c| c.trader).map(|c| c.ship.cargo).sum::<f64>() / traders.len() as f64;
        eprintln!(
            "{:.1} game h: stops {}, trades {}, turnover {:.0} CR; trader credits mean {mean:.0} (min {lo:.0}, max {hi:.0}), cargo mean {:.1} T",
            (u.world.time) / 3600.0,
            u.records.stats.stops,
            u.records.stats.trades,
            u.records.stats.turnover,
            cargo / 1000.0
        );
        assert!(u.records.stats.trades > 0);
    }
}
