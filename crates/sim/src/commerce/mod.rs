//! The market service's side of trading: pilots' trades (decided by the
//! market, booked in the ledger), quotes on request, and the trade log. What
//! a trader buys and sells, and where it goes, is its own (see `operator`).

use universe_protocol::ShipId;
use std::collections::{HashMap, VecDeque};

use universe_services::market::{cargo_mass, Order, Quote};
use universe_services::Party;
use universe_world::traffic::{docked_at, facilities};
use universe_world::Facility;

use crate::universe::Universe;
use universe_services::records::{Deal, TradeRecord};

/// What a new settler starts with (credits).
pub const SETTLER_CREDITS: f64 = 3000.0;

/// How often a market publishes its board (s): its clock's period (`clock.market`).
pub fn board_every() -> f64 {
    crate::clocks::period(universe_world::registry::ClockKey::Market)
}
/// Boards kept this long (s): past the slowest way round a system's net.
const BOARD_KEPT: f64 = 3.0 * 3600.0;

/// A price board: when it was put out, and the market's quote for every good
/// (by the goods' order; None: not traded there).
pub type Board = (f64, Vec<Option<Quote>>);

/// What markets know of each other (`docs/hypernet.md`, step 5): each puts
/// out its board every `board_every()`; another market has it once it's come
/// over the net (the board's market's lag from the backbone, and its own).
/// A market off the net puts out nothing that's heard, and hears nothing.
/// The world's first boards are long known everywhere.
#[derive(Default)]
pub struct Boards {
    next: f64,
    boards: HashMap<(usize, Facility), VecDeque<Board>>,
    /// Each market's lag from its system's backbone at the last boards (None: dark).
    lags: HashMap<(usize, Facility), Option<f64>>,
    /// Each system's net at the last boards: the system, its bodies then, the net.
    nets: HashMap<usize, (std::sync::Arc<universe_world::StarSystem>, Vec<glam::DVec3>, universe_world::hypernet::Net)>,
    /// The settled places' reports (their stocks, people, what they make and
    /// lack), put out with the boards: when, and all of them then.
    places: VecDeque<(f64, std::sync::Arc<Vec<universe_services::economy::Place>>)>,
}

/// What we know of the economy: for each place (as the economy has them), the
/// report of it that's reached us (in a snapshot of them all: its own entry),
/// and how old it is (None: no word reaches us; what's long known shown).
pub type Heard = Vec<(std::sync::Arc<Vec<universe_services::economy::Place>>, Option<f64>)>;

impl Boards {
    /// The newest board of `f` that `here` (in `system`) has by `now`, and
    /// its age (infinite: long known). None: nothing of it reaches here.
    pub fn known(&self, system: usize, here: Facility, f: Facility, now: f64) -> Option<(f64, &Vec<Option<Quote>>)> {
        let lag = |m: Facility| self.lags.get(&(system, m)).copied().flatten();
        let delay = lag(f)? + lag(here)?;
        self.newest(system, f, now - delay).map(|(t, b)| (now - t, b))
    }

    /// The newest board of `f` put out by `by`: when, and its quotes.
    fn newest(&self, system: usize, f: Facility, by: f64) -> Option<(f64, &Vec<Option<Quote>>)> {
        self.boards.get(&(system, f))?.iter().rev().find(|b| b.0 <= by).map(|b| (b.0, &b.1))
    }

    /// The newest board of `f` (in `system`) that's reached a comm at `p` by
    /// `now`, over the net, and its age. None: nothing of it reaches there.
    pub fn known_at(&self, system: usize, f: Facility, p: glam::DVec3, comm: &universe_world::modules::Comm, now: f64) -> Option<(f64, &Vec<Option<Quote>>)> {
        let delay = self.delay(system, f, &self.to_us(system, p, comm)?)?;
        self.newest(system, f, now - delay).map(|(t, b)| (now - t, b))
    }

    /// From each system to us (in `us`, a comm at `p`), over the net (s): its
    /// backbone through the gate relays to ours, and out to us. None: we're off the net.
    fn to_us(&self, us: usize, p: glam::DVec3, comm: &universe_world::modules::Comm) -> Option<HashMap<usize, f64>> {
        let (sys, positions, net) = self.nets.get(&us)?;
        let ours = net.status(sys, positions, p, comm)?.lag;
        // (Out of each system through its gates, and in at the far end.)
        let mut hops = Vec::new();
        for (&a, (_, _, na)) in &self.nets {
            for (b, out) in na.gates() {
                if let Some(back) = self.nets.get(&b).and_then(|(_, _, nb)| nb.gate_in(a)) {
                    hops.push((a, b, out + back));
                }
            }
        }
        Some(universe_world::hypernet::delays_to(us, ours, &hops))
    }

    /// How long word from `f` (in `system`) takes to reach us, given `to_us`.
    fn delay(&self, system: usize, f: Facility, to_us: &HashMap<usize, f64>) -> Option<f64> {
        Some(self.lags.get(&(system, f)).copied().flatten()? + to_us.get(&system)?)
    }

    /// The economy as it's reached a comm at `p` in `us` by `now` (see `Heard`).
    pub fn heard_economy(&self, us: usize, p: glam::DVec3, comm: &universe_world::modules::Comm, now: f64) -> Heard {
        let Some((_, latest)) = self.places.back() else { return Vec::new() };
        let to_us = self.to_us(us, p, comm);
        let first = self.places.front().map(|s| s.1.clone()).unwrap_or_else(|| latest.clone());
        latest
            .iter()
            .map(|place| {
                let delay = to_us.as_ref().and_then(|t| self.delay(place.system, place.facility, t));
                match delay.and_then(|d| self.places.iter().rev().find(|s| s.0 <= now - d)) {
                    Some((t, snap)) => (snap.clone(), Some(now - t)),
                    None => (first.clone(), None),
                }
            })
            .collect()
    }
}

impl Universe {
    /// Pilot `pilot` (the player's 0, craft i: i + 1) asks the market at `f`
    /// to trade `units` of `item`: the market service decides, the ledger
    /// books it (the request's cause on every entry), and the ship's cargo
    /// mass follows what's in its hold. Credits paid (negative: received),
    /// or why not.
    pub(crate) fn pilot_trade(&mut self, pilot: ShipId, f: Facility, item: usize, units: i64) -> Result<f64, String> {
        let (system, ship) = (self.vessels[pilot].system, &self.vessels[pilot].ship);
        self.barred(pilot, system)?;
        let sys = self.world.system(system);
        let order = Order { pilot, system, market: f, docked_at: docked_at(&sys, ship), room: ship.hold_room(), space: ship.hold_space(), item, units };
        self.messages += 1;
        let cause = universe_protocol::Cause::Message { sender: pilot.0 as u64, id: self.messages };
        let r = self.markets.trade(&mut self.ledger, &sys, order, self.world.time, self.tick, cause);
        if r.is_ok() {
            // The core: the hold weighs what's in it.
            let hold = self.ledger.hold(pilot);
            let (mass, volume) = (cargo_mass(&self.world.goods, &hold), universe_services::market::cargo_volume(&self.world.goods, &hold));
            let ship = &mut self.vessels[pilot].ship;
            ship.cargo = mass;
            ship.cargo_volume = volume;
        }
        r
    }

    /// Ore ship `id` dug this tick (as its step logged it): the world
    /// remembers it's gone from the rock, the ledger puts it in the hold
    /// (from the world's account, because of the dig).
    pub(crate) fn book_mined(&mut self, id: ShipId, events: &[crate::Event]) {
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

    /// Ship `id`'s tube crossings this tick, paid to the system's
    /// administration (cleared only if it could pay; what it has, if less now).
    pub(crate) fn book_tolls(&mut self, id: ShipId, events: &[crate::Event]) {
        for e in events {
            let crate::Event::Ship(universe_world::ShipEvent::TubeToll { credits }) = e else { continue };
            let Some((_, system, _)) = self.ship_by_id(id) else { return };
            let pay = credits.min(self.ledger.credits(Party::Pilot(id)).max(0.0));
            let _ = self.ledger.transfer(Party::Pilot(id), Party::Administration(system), universe_services::ledger::Asset::Credits, pay, self.tick, universe_protocol::Cause::Rules);
        }
    }

    /// Ship `id`'s hold after its jolts this tick (a hard landing's): what takes
    /// less than the jolt breaks, and is written off (SFO 15).
    pub fn book_jolts(&mut self, id: ShipId, events: &[crate::Event]) -> Vec<universe_world::ShipEvent> {
        let jolts: Vec<f64> = events.iter().filter_map(|e| if let crate::Event::Ship(universe_world::ShipEvent::HardLanding { jolt, .. }) = e { Some(*jolt) } else { None }).collect();
        let Some(jolt) = jolts.into_iter().reduce(f64::max) else { return Vec::new() };
        let g = jolt * universe_physics::laws::STANDARD_GRAVITY;
        let goods = self.world.goods.clone();
        let broken: Vec<(usize, u32)> = self.ledger.hold(id).into_iter().filter(|(i, _)| goods[*i].shock_limit.is_some_and(|l| g > l)).collect();
        for &(item, _) in &broken {
            self.ledger.settle(Party::Pilot(id), universe_services::ledger::Asset::Goods(item), 0.0, self.tick, universe_protocol::Cause::Rules);
        }
        if !broken.is_empty() {
            let hold = self.ledger.hold(id);
            if let Some(ship) = self.ship_mut_by_id(id) {
                ship.cargo = universe_services::market::cargo_mass(&goods, &hold);
                ship.cargo_volume = universe_services::market::cargo_volume(&goods, &hold);
            }
        }
        broken.into_iter().map(|(item, units)| universe_world::ShipEvent::CargoBroken { item, units }).collect()
    }

    /// The `n`-th of ship `id`'s `Mined` events in this tick's log, as a cause.
    fn mined_cause(&self, id: ShipId, n: usize) -> Option<universe_protocol::Cause> {
        let index = self.log.iter().enumerate().filter(|(_, (s, e))| *s == id && matches!(e, universe_world::ShipEvent::Mined { .. })).nth(n - 1)?.0;
        Some(universe_protocol::Cause::Event { tick: self.tick, index: index as u32 })
    }

    /// Pilot `id` fills its tank at `market`: the market service sells what
    /// it can (the ledger books it), and the core's tank takes it.
    pub(crate) fn refuel(&mut self, id: ShipId, market: Facility) -> Result<(f64, f64), String> {
        let Some((_, system, ship)) = self.ship_by_id(id) else { return Err("NO SHIP".into()) };
        self.barred(id, system)?;
        let want = ship.spec().fuel_capacity - ship.fuel;
        if want < 0.01 {
            return Err("TANK FULL".into());
        }
        let docked = docked_at(&self.world.system(system), ship);
        self.messages += 1;
        let cause = universe_protocol::Cause::Message { sender: id.0 as u64, id: self.messages };
        let (tonnes, cost) = self.markets.refuel(&mut self.ledger, system, market, docked, id, want / 1000.0, self.tick, cause)?;
        self.vessels[id].ship.fuel += tonnes * 1000.0;
        Ok((tonnes, cost))
    }

    /// The player fills the tank where docked or landed (as its pilot would on arrival).
    pub fn refuel_player(&mut self) {
        self.note(|| crate::audit::Input::Op(crate::audit::Op::Refuel));
        let sys = self.world.system(self.vessels[crate::combat::PLAYER].system);
        let Some(market) = docked_at(&sys, &self.vessels[crate::combat::PLAYER].ship) else {
            return self.events.push(universe_avionics::Event::Refused { reason: "REFUEL: NOT AT A PORT OR STATION".into() });
        };
        let e = match self.refuel(crate::combat::PLAYER, market) {
            Ok((tonnes, credits)) => universe_avionics::Event::Refuelled { tonnes, credits },
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
    pub(crate) fn market_request(&mut self, id: ShipId, r: crate::vessel::Request) {
        use crate::vessel::Request;
        match r {
            Request::Quotes { system, market } => {
                // (A craft's pilot hears back; the player's client reads the boards itself.)
                let Some(i) = id.craft_index() else { return };
                let answer = self.market_answer(id, system, market);
                self.tell(i, crate::contract::Msg::Market(Box::new(answer)));
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
            Request::Repair { .. } => {
                let _ = self.repair(id);
            }
            Request::Board { market, to } => {
                let _ = self.board_passengers(id, market, to);
            }
            Request::Land { market } => {
                let _ = self.land_passengers(id, market);
            }
            _ => {}
        }
    }

    /// The market screen's view of `f` in our system: live where we're docked;
    /// elsewhere, its newest board that's reached us over the net (`age`:
    /// how old; None: nothing of it reaches us, and no quotes).
    pub fn market_view(&mut self, f: Facility) -> crate::engine::MarketView {
        let live = self.market_quotes(f);
        let held: Vec<usize> = self.hold().into_iter().map(|(i, _)| i).collect();
        if self.docked_market() == Some(f) {
            let held = held.into_iter().filter(|i| !live.iter().any(|q| q.offer.item == *i)).map(|i| (i, self.quote_for(f, i))).collect();
            return crate::engine::MarketView { market: f, quotes: live, held, age: Some(0.0) };
        }
        // (In a gate's tube we're off the net: what we knew going in.)
        let when = match self.vessels[crate::combat::PLAYER].ship.state {
            universe_world::ShipState::Transit { remaining, duration, .. } => self.world.time - (duration - remaining),
            _ => self.world.time,
        };
        let known = self.boards.known_at(self.vessels[crate::combat::PLAYER].system, f, self.vessels[crate::combat::PLAYER].ship.position, &self.vessels[crate::combat::PLAYER].ship.spec().comm, when).map(|(age, b)| (age + self.world.time - when, b));
        let Some((age, board)) = known else { return crate::engine::MarketView { market: f, quotes: Vec::new(), held: Vec::new(), age: None } };
        // (What it listed then, in its listing's order; and what we hold besides.)
        let quotes: Vec<Quote> = live.iter().filter_map(|q| board.get(q.offer.item).copied().flatten()).collect();
        let held = held.into_iter().filter(|i| !quotes.iter().any(|q| q.offer.item == *i)).map(|i| (i, board.get(i).copied().flatten())).collect();
        crate::engine::MarketView { market: f, quotes, held, age: Some(age) }
    }

    /// Every market in the gate network puts out its board, when due.
    pub(crate) fn publish_boards(&mut self) {
        use universe_world::hypernet::Net;
        let now = self.world.time;
        if now < self.boards.next {
            return;
        }
        let first = self.boards.boards.is_empty();
        self.boards.next = now + board_every();
        let mut systems: Vec<usize> = self.world.gate_links.iter().flat_map(|&(a, b)| [a, b]).chain(self.markets.economy.places.iter().map(|p| p.system)).collect();
        systems.sort();
        systems.dedup();
        let all: Vec<usize> = (0..self.world.goods.len()).collect();
        for system in systems {
            let sys = self.system(system);
            let mut positions = Vec::new();
            sys.positions(now, &mut positions);
            let net = Net::at(&sys, universe_world::hypernet::nodes(&self.world.galaxy, &sys), now, &positions);
            let net = &self.boards.nets.entry(system).insert_entry((sys.clone(), positions, net)).into_mut().2;
            for f in facilities(&sys) {
                let lag = universe_world::hypernet::relay_of(f).and_then(|a| net.node(a)).and_then(|k| net.lag[k]);
                self.boards.lags.insert((system, f), lag);
                let quotes = self.markets.quotes_for(system, &sys, f, &all, now);
                let list = self.boards.boards.entry((system, f)).or_default();
                // (The world's first boards: long known.)
                list.push_back((if first { f64::NEG_INFINITY } else { now }, quotes));
                // (Older ones go, but for the newest from before then: still the latest someone far may have.)
                while list.len() >= 2 && list[1].0 <= now - BOARD_KEPT {
                    list.pop_front();
                }
            }
        }
        // And the places' reports, all at once.
        let snap = self.markets.economy.snapshot();
        self.boards.places.push_back((if first { f64::NEG_INFINITY } else { now }, snap));
        while self.boards.places.len() >= 2 && self.boards.places[1].0 <= now - BOARD_KEPT {
            self.boards.places.pop_front();
        }
    }

    /// What the market service tells pilot `id` at `at`: its own quotes, the
    /// other markets' as their boards have reached this one over the
    /// hypernet (with their age), and its account.
    fn market_answer(&mut self, id: ShipId, system: usize, at: Facility) -> crate::contract::MarketAnswer {
        let sys = self.system(system);
        let now = self.world.time;
        let hold = self.ledger.hold(id);
        let held: Vec<usize> = hold.iter().map(|h| h.0).collect();
        let here = self.markets.quotes(system, &sys, at, now);
        let here_held = self.markets.quotes_for(system, &sys, at, &held, now);
        let mut items = held.clone();
        items.extend(here.iter().filter(|q| q.buy.is_some()).map(|q| q.offer.item).filter(|i| !held.contains(i)));
        let there = facilities(&sys)
            .into_iter()
            .filter(|&f| f != at)
            .filter_map(|f| {
                let (age, board) = self.boards.known(system, at, f, now)?;
                Some((f, age, items.iter().map(|&i| board.get(i).copied().flatten()).collect()))
            })
            .collect();
        let (cargo, capacity, space) = self.ship_by_id(id).map_or((0.0, 0.0, 0.0), |(_, _, s)| (s.cargo, s.spec().hold_capacity, s.hold_space()));
        let (passengers, bound_for, seats) = self.ship_by_id(id).map_or((0, None, 0), |(_, _, s)| (s.passengers, s.bound_for, s.passenger_room()));
        let bookings = self.bookings(system, at);
        let waiting = self.markets.economy.places.iter().filter(|p| p.system == system).map(|p| (p.facility, p.waiting)).collect();
        crate::contract::MarketAnswer { system, at, here, here_held, items, there, credits: self.ledger.credits(Party::Pilot(id)), hold, cargo, capacity, space, bookings, waiting, passengers, bound_for, seats }
    }

    /// A trade (or a plan) in the log, as pilot `id` made it at `market`.
    pub(crate) fn record_trade(&mut self, id: ShipId, market: Facility, deal: Deal, item: Option<usize>, units: u32, amount: f64) {
        let Some((_, system, ship)) = self.ship_by_id(id) else { return };
        let cargo = ship.cargo;
        let trader = self.vessels[id].name.to_uppercase();
        let sys = self.system(system);
        if item.is_some() {
            self.records.stats.trades += 1;
            self.records.stats.turnover += amount;
        }
        let record = TradeRecord {
            time: self.world.time,
            system,
            market: market.name(&sys),
            pilot: id,
            place: Some(market),
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

mod passengers;
mod yard;
pub use passengers::Booking;
pub use yard::BUYBACK;
