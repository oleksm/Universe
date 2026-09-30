//! Traders (a slice of the settlers): at each stop of its route, a trader sells
//! what it carries where it pays — goods the market wants, or anything that
//! makes a profit on what it paid — then buys what's going cheap here (well
//! under its usual worth) to sell further on. Like the pilot, it sees only
//! the market it's standing in. Every trade is recorded.

use universe_world::market::{docked_at, Side, HOLD_CAPACITY};

use crate::universe::Universe;

/// What a new settler starts with (credits).
pub const SETTLER_CREDITS: f64 = 3000.0;
/// Trades kept in `Universe::trade_log`.
const TRADE_LOG: usize = 100;
/// Buy only what's selling at under this share of its usual worth.
const BARGAIN: f64 = 0.85;
/// Sell without demand only for at least this margin over what was paid.
const MARGIN: f64 = 1.05;
/// Past this share of the hold, sell what the market takes back, loss or not.
const STUCK: f64 = 0.8;

/// One trade: who bought or sold what, where, for how much, and how they
/// stood after it.
#[derive(Clone, Debug)]
pub struct TradeRecord {
    pub time: f64,
    pub system: usize,
    pub market: String,
    pub trader: String,
    pub bought: bool,
    pub item: String,
    pub units: u32,
    /// Credits paid (bought) or received (sold).
    pub amount: f64,
    /// Cargo aboard after it (kg), and credits.
    pub cargo: f64,
    pub credits: f64,
}

impl Universe {
    /// Craft `i` trades at the market it has just arrived at.
    pub(crate) fn craft_trades(&mut self, i: usize) {
        let system = self.crafts[i].system;
        let sys = self.system(system);
        let Some(f) = docked_at(&sys, &self.crafts[i].ship) else { return };
        let market = f.name(&sys);
        let quotes = self.world.quotes(system, f);
        let mut records = Vec::new();

        // Sell.
        let held: Vec<(usize, u32)> = self.crafts[i].ship.hold.iter().map(|(k, v)| (*k, *v)).collect();
        for (item, have) in held {
            let Some(q) = self.world.quote_for(system, f, item) else { continue };
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
            let c = &mut self.crafts[i];
            if let Ok(moved) = self.world.trade(system, f, item, -(units as i64), &mut c.ship, &mut c.credits) {
                if !c.ship.hold.contains_key(&item) {
                    c.paid.remove(&item);
                }
                records.push((false, item, units, -moved));
            }
        }

        // Buy what's cheap here, a few lines of it.
        // Goods flow from where they're made to where they're wanted: skip the
        // kinds this market wants itself (they'd sell badly nearby).
        let wants = self.world.market(system, f).map(|m| m.wants.clone()).unwrap_or_default();
        let mut bargains: Vec<_> = quotes
            .iter()
            .filter(|q| !wants.contains(&self.world.goods[q.offer.item].category))
            .filter_map(|q| {
                let price = q.buy?;
                let ratio = price / self.world.goods[q.offer.item].price;
                (ratio < BARGAIN && q.level >= 1.0).then_some((ratio, q.offer.item, price, q.level))
            })
            .collect();
        bargains.sort_by(|a, b| a.0.total_cmp(&b.0));
        for (_, item, price, level) in bargains.into_iter().take(3) {
            let c = &self.crafts[i];
            let mass = self.world.goods[item].mass;
            let room = ((HOLD_CAPACITY - c.ship.cargo) / mass).floor();
            let afford = (c.credits * 0.3 / price).floor();
            let units = room.min(afford).min((level * 0.5).floor());
            if units < 1.0 {
                continue;
            }
            let c = &mut self.crafts[i];
            let before = c.ship.hold.get(&item).copied().unwrap_or(0) as f64;
            if let Ok(cost) = self.world.trade(system, f, item, units as i64, &mut c.ship, &mut c.credits) {
                let avg = c.paid.get(&item).copied().unwrap_or(0.0);
                c.paid.insert(item, (avg * before + cost) / (before + units));
                records.push((true, item, units as u32, cost));
            }
        }

        for (bought, item, units, amount) in records {
            let c = &self.crafts[i];
            self.traffic.trades += 1;
            self.traffic.turnover += amount;
            let record = TradeRecord {
                time: self.world.time,
                system,
                market: market.clone(),
                trader: c.name.to_uppercase(),
                bought,
                item: self.world.goods[item].name.to_uppercase(),
                units,
                amount,
                cargo: c.ship.cargo,
                credits: c.credits,
            };
            self.log_trade(record);
        }
    }

    /// Keep a trade in the log (the last `TRADE_LOG`).
    pub(crate) fn log_trade(&mut self, record: TradeRecord) {
        self.trade_log.push(record);
        let excess = self.trade_log.len().saturating_sub(TRADE_LOG);
        self.trade_log.drain(..excess);
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
        for r in u.trade_log.iter().rev().take(12) {
            eprintln!(
                "{} {} {} {} AT {} FOR {:.0} CR - CARGO {:.1} T, {:.0} CR",
                r.trader,
                if r.bought { "BOUGHT" } else { "SOLD" },
                r.units,
                r.item,
                r.market,
                r.amount,
                r.cargo / 1000.0,
                r.credits
            );
        }
        let traders: Vec<f64> = u.crafts.iter().filter(|c| c.trader).map(|c| c.credits).collect();
        eprintln!("{} traders, {} pirates of {}", traders.len(), u.crafts.iter().filter(|c| c.avionics.pirate).count(), u.crafts.len());
        let mean = traders.iter().sum::<f64>() / traders.len() as f64;
        let (lo, hi) = traders.iter().fold((f64::MAX, f64::MIN), |(a, b), &x| (a.min(x), b.max(x)));
        let cargo: f64 = u.crafts.iter().filter(|c| c.trader).map(|c| c.ship.cargo).sum::<f64>() / traders.len() as f64;
        eprintln!(
            "{:.1} game h: stops {}, trades {}, turnover {:.0} CR; trader credits mean {mean:.0} (min {lo:.0}, max {hi:.0}), cargo mean {:.1} T",
            (u.world.time) / 3600.0,
            u.traffic.stops,
            u.traffic.trades,
            u.traffic.turnover,
            cargo / 1000.0
        );
        assert!(u.traffic.trades > 0);
    }
}
