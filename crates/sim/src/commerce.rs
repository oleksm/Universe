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

/// A module taken out at a refit fetches this share of its price.
pub const BUYBACK: f64 = 0.6;

impl crate::universe::Universe {
    /// Pilot `id` refits slot `slot` with `module` (None: empties it) where
    /// it's docked or landed. At a station, the module is brought in from
    /// outside the economy at its shipyard's price, and the one taken out
    /// fetches `BUYBACK` of its price. At a port with a market, the module is
    /// one lying in its warehouse, at the exchange's ask, and the one taken
    /// out goes into the warehouse at its bid. Refused, with the reason, if
    /// there's neither here, none in stock, the fit won't do (see
    /// `ClassSpec::assemble`), or it can't pay. The credits it cost.
    pub fn refit_as(&mut self, id: ShipId, slot: &str, module: Option<universe_world::content::Handle<universe_world::modules::Module>>) -> Result<f64, String> {
        use universe_services::{Asset, Party};
        let Some((_, system, ship)) = self.ship_by_id(id) else { return Err("NO SHIP".into()) };
        self.barred(id, system)?;
        let sys = self.world.system(system);
        let here = match universe_world::traffic::docked_at(&sys, ship) {
            Some(f @ Facility::Station(_)) => f,
            Some(f) if self.markets.has_market(system, f) => f,
            _ => return Err("REFIT DOCKED AT A STATION, OR LANDED AT A PORT'S MARKET".into()),
        };
        let c = universe_world::content::content();
        let mut fit = ship.fit.as_deref().cloned().unwrap_or_else(|| c.get(ship.class).fit.clone());
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
        use universe_services::outfitter;
        let stocked = matches!(here, Facility::Spaceport(_));
        let item = |m: universe_world::content::Handle<universe_world::modules::Module>| universe_world::goods::item(&c.get(m).key);
        let (price, back) = if stocked {
            // (From the warehouse: what lies there, at the exchange's prices.)
            let price = match module {
                Some(m) => {
                    let q = item(m).and_then(|i| self.quote_for(here, i)).filter(|q| q.buy.is_some() && q.level >= 1.0).ok_or("NONE IN STOCK HERE")?;
                    q.buy.unwrap_or(0.0)
                }
                None => 0.0,
            };
            (price, taken.and_then(item).and_then(|i| self.quote_for(here, i)).map_or(0.0, |q| q.sell))
        } else {
            // (This station's shipyard's, if it carries it: brought in.)
            let price = match module {
                Some(m) => {
                    let o = outfitter::offer(self.world.galaxy.seed, &self.world.gate_links, system, here, c.get(m));
                    if !o.carried {
                        return Err("NOT CARRIED HERE".into());
                    }
                    o.price
                }
                None => 0.0,
            };
            (price, taken.map_or(0.0, |m| c.get(m).price * BUYBACK))
        };
        let cost = price - back;
        let (me, market) = (Party::Pilot(id), Party::Market(system, here));
        let cause = universe_protocol::Cause::Rules;
        self.ledger.transfer(me, market, Asset::Credits, cost, self.tick, cause)?;
        // (The module fitted comes with its mark, onto the ship; one taken out goes back with its own.)
        let mut marks: Vec<universe_world::registry::Mark> = refitted.marks.as_deref().cloned().unwrap_or_default();
        if stocked {
            if let Some(i) = module.and_then(item) {
                let (_, m, _) = self.markets.economy.take_marked(system, here, i, self.world.goods[i].mass);
                marks.extend(m);
            }
            if let Some(i) = taken.and_then(item) {
                let key = &self.world.goods[i].key;
                let back: Vec<_> = marks.iter().position(|m| &m.design == key).map(|k| marks.remove(k)).into_iter().collect();
                self.markets.economy.put_marked(system, here, i, self.world.goods[i].mass, back, Vec::new());
            }
        }
        let mut refitted = refitted;
        refitted.marks = (!marks.is_empty()).then(|| std::sync::Arc::new(marks));
        self.vessels[id].ship = refitted;
        Ok(cost)
    }

    /// The player refits slot `slot` (see `refit_as`); the cockpit is told.
    pub fn refit(&mut self, slot: &str, module: Option<universe_world::content::Handle<universe_world::modules::Module>>) -> Result<f64, String> {
        self.note(|| crate::audit::Input::Op(crate::audit::Op::Refit { slot: slot.to_string(), module }));
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

impl crate::universe::Universe {
    /// What hull `hull` costs pilot `id` where it's docked or landed, and what
    /// its ship fetches in trade: at a station, the frame and its stock fit
    /// at its prices (brought in); at a port with a market, a frame lying in
    /// its warehouse at the exchange's ask, its stock fit brought in. The old
    /// ship's frame and modules at `BUYBACK`.
    pub fn hull_offer(&self, id: ShipId, hull: universe_world::ship::Hull) -> Result<(f64, f64), String> {
        use universe_services::outfitter;
        let Some((_, system, ship)) = self.ship_by_id(id) else { return Err("NO SHIP".into()) };
        let sys = self.world.system(system);
        let here = match universe_world::traffic::docked_at(&sys, ship) {
            Some(f @ Facility::Station(_)) => f,
            Some(f) if self.markets.has_market(system, f) => f,
            _ => return Err("BUY A SHIP DOCKED AT A STATION, OR LANDED AT A PORT'S MARKET".into()),
        };
        let c = universe_world::content::content();
        let spec = c.get(hull);
        let frame = match here {
            Facility::Spaceport(_) => self.frame_in_stock(system, here, hull).ok_or("NO SUCH HULL IN STOCK HERE")?.1,
            _ => spec.frame.price,
        };
        let price = frame + spec.fit.iter().map(|(_, m)| outfitter::offer(self.world.galaxy.seed, &self.world.gate_links, system, here, c.get(*m)).price).sum::<f64>();
        let old = ship.spec();
        let trade_in = BUYBACK * (old.frame.price + old.fit.iter().map(|(_, m)| c.get(*m).price).sum::<f64>());
        Ok((price, trade_in))
    }

    /// Pilot `id` buys hull `hull` (with its stock fit), trading in its ship,
    /// at the station it's docked at: built from the place's metals (the
    /// frame) and its modules' own materials; its cargo moves over (if the
    /// new hold takes it) and its fuel (up to the new tank). The credits it cost.
    pub fn buy_hull_as(&mut self, id: ShipId, hull: universe_world::ship::Hull) -> Result<f64, String> {
        use universe_services::{Asset, Party};
        let (price, trade_in) = self.hull_offer(id, hull)?;
        let Some((_, system, ship)) = self.ship_by_id(id) else { return Err("NO SHIP".into()) };
        self.barred(id, system)?;
        if ship.class == hull && ship.fit.is_none() {
            return Err("THAT'S THE SHIP YOU HAVE".into());
        }
        let sys = self.world.system(system);
        let here = universe_world::traffic::docked_at(&sys, ship).expect("docked (checked)");
        let c = universe_world::content::content();
        let spec = c.get(hull);
        if ship.cargo + ship.hopper > spec.hold_capacity + 1e-6 {
            return Err(format!("ITS HOLD TAKES {:.0} T, THERE'S {:.1} T IN YOURS", spec.hold_capacity / 1000.0, (ship.cargo + ship.hopper) / 1000.0));
        }
        // (At a station, what building it takes is brought in from outside the economy, for now.)
        let cost = price - trade_in;
        let (me, market) = (Party::Pilot(id), Party::Market(system, here));
        self.ledger.transfer(me, market, Asset::Credits, cost, self.tick, universe_protocol::Cause::Rules)?;
        // (The hull bought comes with its mark.)
        let mut hull_marks = Vec::new();
        if let Some((item, _)) = self.frame_in_stock(system, here, hull) {
            hull_marks = self.markets.economy.take_marked(system, here, item, self.world.goods[item].mass).1;
        }
        // The new ship where the old one stood, with its cargo and fuel.
        let old = self.ship_by_id(id).expect("there").2.clone();
        let mut new = old.clone();
        new.class = hull;
        new.fit = None;
        new.marks = (!hull_marks.is_empty()).then(|| std::sync::Arc::new(hull_marks));
        // (A new hull: untrimmed.)
        new.trim = Default::default();
        new.refresh();
        new.fuel = old.fuel.min(spec.fuel_capacity);
        new.hull = 1.0;
        new.jets.clear();
        // (On its own feet: the new hull may stand taller or lower.)
        self.world.resettle(system, &mut new);
        self.vessels[id].ship = new;
        Ok(cost)
    }

    /// A frame of `hull` lying in the warehouse at `here` (a port's market):
    /// its stock item and the exchange's ask. (A hull the game imported is the
    /// registry's that names its model.)
    fn frame_in_stock(&self, system: usize, here: Facility, hull: universe_world::ship::Hull) -> Option<(usize, f64)> {
        let spec = universe_world::content::content().get(hull);
        let key = universe_world::ship::hull_record(spec)?.identity.key.clone();
        let item = universe_world::goods::item(&key)?;
        let place = self.markets.economy.place(system, here)?;
        let ask = place.price(&self.world.goods[item]).ask?;
        Some((item, ask))
    }

    /// The player buys a hull (see `buy_hull_as`); the cockpit is told.
    pub fn buy_hull(&mut self, hull: universe_world::ship::Hull) -> Result<f64, String> {
        self.note(|| crate::audit::Input::Op(crate::audit::Op::BuyHull(hull)));
        let r = self.buy_hull_as(crate::combat::PLAYER, hull);
        let name = universe_world::content::content().get(hull).name.clone();
        self.events.push(match &r {
            Ok(credits) => universe_avionics::Event::BoughtShip { name, credits: *credits },
            Err(reason) => universe_avionics::Event::Refused { reason: format!("SHIPYARD: {reason}") },
        });
        r
    }
}

/// A whole hull's repair costs this share of its frame's price.
pub const REPAIR_PRICE: f64 = 0.3;

impl crate::universe::Universe {
    /// Pilot `id`'s hull mended at the station it's docked at, as far as its
    /// credits go: `REPAIR_PRICE` of the frame's price for a whole hull.
    /// What it cost, and how far it's mended now.
    pub fn repair(&mut self, id: ShipId) -> Result<(f64, f64), String> {
        use universe_services::{Asset, Party};
        let Some((_, system, ship)) = self.ship_by_id(id) else { return Err("NO SHIP".into()) };
        self.barred(id, system)?;
        let sys = self.world.system(system);
        let Some(here @ Facility::Station(_)) = universe_world::traffic::docked_at(&sys, ship) else { return Err("REPAIRS DOCKED AT A STATION".into()) };
        let missing = 1.0 - ship.hull;
        if missing <= 1e-9 {
            return Err("THE HULL IS SOUND".into());
        }
        let spec = ship.spec();
        let full_price = spec.frame.price * REPAIR_PRICE;
        // As much as the credits allow (the metals brought in from outside the economy, for now).
        let credits = self.ledger.credits(Party::Pilot(id)).max(0.0);
        let part = missing.min(credits / full_price);
        if part <= 1e-6 {
            return Err("NO CREDITS FOR REPAIRS".into());
        }
        let cost = part * full_price;
        self.ledger.transfer(Party::Pilot(id), Party::Market(system, here), Asset::Credits, cost, self.tick, universe_protocol::Cause::Rules)?;
        let hull = &mut self.vessels[id].ship;
        hull.hull = (hull.hull + part).min(1.0);
        Ok((cost, hull.hull))
    }

    /// The player's hull mended (see `repair`); the cockpit is told.
    pub fn repair_player(&mut self) {
        self.note(|| crate::audit::Input::Op(crate::audit::Op::Repair));
        let e = match self.repair(crate::combat::PLAYER) {
            Ok((credits, hull)) => universe_avionics::Event::Repaired { credits, hull },
            Err(reason) => universe_avionics::Event::Refused { reason: format!("REPAIR: {reason}") },
        };
        self.events.push(e);
    }

    /// A ship's value: its frame and what's fitted, at list prices.
    pub fn ship_value(ship: &universe_world::Ship) -> f64 {
        let c = universe_world::content::content();
        let s = ship.spec();
        s.frame.price + s.fit.iter().map(|(_, m)| c.get(*m).price).sum::<f64>()
    }
}

impl crate::universe::Universe {
    /// The player, on foot by a spaceport's vending machine, buys item
    /// `item` of it (`spaceport::VENDING`): paid to the port's market.
    pub fn vend(&mut self, item: usize) {
        self.note(|| crate::audit::Input::Op(crate::audit::Op::Vend(item)));
        use universe_services::{Asset, Party};
        let e = match (self.pilot_reach(), universe_world::spaceport::VENDING.get(item)) {
            (Some(universe_world::crew::Reach::Vending(port)), Some(&(what, price, note))) => {
                let market = Party::Market(self.vessels[crate::combat::PLAYER].system, Facility::Spaceport(port));
                match self.ledger.transfer(Party::Pilot(crate::combat::PLAYER), market, Asset::Credits, price, self.tick, universe_protocol::Cause::Rules) {
                    Ok(_) => universe_avionics::Event::Vended { what: what.into(), credits: price, note: note.into() },
                    Err(_) => universe_avionics::Event::Refused { reason: "THE MACHINE WANTS CREDITS YOU HAVEN'T GOT".into() },
                }
            }
            _ => universe_avionics::Event::Refused { reason: "NO VENDING MACHINE WITHIN REACH".into() },
        };
        self.events.push(e);
    }
}

impl crate::universe::Universe {
    /// The player's ship trimmed (see `world::trim`): at a station's
    /// shipyard, where its fuel can be pumped and its drive set.
    pub fn set_trim(&mut self, trim: universe_world::trim::Trim) -> Result<(), String> {
        self.note(|| crate::audit::Input::Op(crate::audit::Op::Trim(trim.clone())));
        let sys = self.ship_system();
        let r = match universe_world::traffic::docked_at(&sys, &self.vessels[crate::combat::PLAYER].ship) {
            Some(Facility::Station(_)) => {
                self.vessels[crate::combat::PLAYER].ship.trim = trim.clamped();
                Ok(())
            }
            _ => Err("TRIM DOCKED AT A STATION".to_string()),
        };
        self.events.push(match &r {
            Ok(()) => universe_avionics::Event::Trimmed,
            Err(reason) => universe_avionics::Event::Refused { reason: reason.clone() },
        });
        r
    }
}

/// A passenger's fare (credits): this much, and this much more for each
/// gate on the way (paid on arrival by the settlement fund of the place
/// they settle at).
pub const FARE: f64 = 40.0;
pub const FARE_PER_GATE: f64 = 120.0;

/// Passage booked by people waiting to leave a place: where to, how many,
/// what each pays.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Booking {
    pub system: usize,
    pub to: Facility,
    pub people: u32,
    pub fare: f64,
}

impl crate::universe::Universe {
    fn ship_mut_by_id(&mut self, id: ShipId) -> Option<&mut universe_world::Ship> {
        self.vessels.get_mut(id).map(|v| &mut v.ship)
    }

    /// The passage booked from `market` in `system`: its people waiting to
    /// leave, each bound for a place that would have them (fed, with room)
    /// in this system or one a gate away, shared out by the room there.
    pub fn bookings(&self, system: usize, market: Facility) -> Vec<Booking> {
        let Some(here) = self.markets.economy.place(system, market) else { return Vec::new() };
        let waiting = (here.waiting * 1000.0).floor();
        if waiting < 1.0 {
            return Vec::new();
        }
        let near = |s: usize| s == system || self.world.gate_links.iter().any(|&(a, b)| (a == system && b == s) || (b == system && a == s));
        let homes: Vec<(&universe_services::economy::Place, f64)> = self
            .markets
            .economy
            .places
            .iter()
            .filter(|p| near(p.system) && !(p.system == system && p.facility == market) && p.welcomes())
            .map(|p| (p, (p.founded * universe_services::economy::ROOM - p.population).max(0.0)))
            .filter(|(_, room)| *room > 0.0)
            .collect();
        let rooms: f64 = homes.iter().map(|h| h.1).sum();
        homes
            .iter()
            .filter_map(|(p, room)| {
                let people = (waiting * room / rooms).floor() as u32;
                (people > 0).then_some(Booking { system: p.system, to: p.facility, people, fare: FARE + if p.system == system { 0.0 } else { FARE_PER_GATE } })
            })
            .collect()
    }

    /// Pilot `id`, docked at `market`, takes aboard people who've booked
    /// passage to `to` (system, market): as many as there are and its
    /// cabins seat. How many.
    pub fn board_passengers(&mut self, id: ShipId, market: Facility, to: (usize, Facility)) -> Result<u32, String> {
        let Some((_, system, _)) = self.ship_by_id(id) else { return Err("NO SHIP".into()) };
        let sys = self.system(system);
        let ship = self.ship_by_id(id).expect("there").2;
        if universe_world::traffic::docked_at(&sys, ship) != Some(market) {
            return Err("BOARD PASSENGERS DOCKED OR LANDED".into());
        }
        if ship.spec().seats == 0 {
            return Err("NO PASSENGER CABIN FITTED".into());
        }
        if ship.passengers > 0 && ship.bound_for != Some(to) {
            return Err("THE PASSENGERS ABOARD ARE BOUND ELSEWHERE".into());
        }
        let seats = ship.passenger_room();
        let Some(b) = self.bookings(system, market).into_iter().find(|b| (b.system, b.to) == to) else { return Err("NOBODY BOOKED FOR THERE".into()) };
        let n = b.people.min(seats);
        if n == 0 {
            return Err("EVERY SEAT TAKEN".into());
        }
        if let Some(place) = self.markets.economy.place_mut(system, market) {
            place.depart(n as f64 / 1000.0);
        }
        let ship = self.ship_mut_by_id(id).expect("there");
        ship.passengers += n;
        ship.bound_for = Some(to);
        ship.fare = b.fare;
        Ok(n)
    }

    /// Pilot `id`, docked at `market`, lands its passengers where they
    /// booked passage to (if it still has room for them): their fares paid
    /// by its settlement fund. The fares.
    pub fn land_passengers(&mut self, id: ShipId, market: Facility) -> Result<f64, String> {
        use universe_services::{Asset, Party};
        let Some((_, system, _)) = self.ship_by_id(id) else { return Err("NO SHIP".into()) };
        let sys = self.system(system);
        let ship = self.ship_by_id(id).expect("there").2;
        if universe_world::traffic::docked_at(&sys, ship) != Some(market) {
            return Err("LAND PASSENGERS DOCKED OR LANDED".into());
        }
        let (n, fare) = (ship.passengers, ship.fare);
        if n == 0 {
            return Err("NO PASSENGERS ABOARD".into());
        }
        if ship.bound_for != Some((system, market)) {
            return Err("THEY BOOKED PASSAGE ELSEWHERE".into());
        }
        let Some(place) = self.markets.economy.place_mut(system, market) else { return Err("NOBODY LIVES HERE".into()) };
        place.arrive(n as f64 / 1000.0);
        let paid = fare * n as f64;
        self.ledger.transfer(Party::World, Party::Pilot(id), Asset::Credits, paid, self.tick, universe_protocol::Cause::Rules).map_err(|e| format!("{e:?}"))?;
        let ship = self.ship_mut_by_id(id).expect("there");
        ship.passengers = 0;
        ship.bound_for = None;
        ship.fare = 0.0;
        Ok(paid)
    }

    /// The player, docked: lands the passengers aboard where they're bound
    /// (if this is it), or boards those booked for `to`.
    pub fn passengers(&mut self, to: Option<(usize, Facility)>) {
        self.note(|| crate::audit::Input::Op(crate::audit::Op::Passengers(to)));
        let sys = self.ship_system();
        let Some(market) = universe_world::traffic::docked_at(&sys, &self.vessels[crate::combat::PLAYER].ship) else {
            self.events.push(universe_avionics::Event::Refused { reason: "PASSENGERS DOCKED OR LANDED".into() });
            return;
        };
        let e = match to {
            None => {
                let n = self.vessels[crate::combat::PLAYER].ship.passengers;
                match self.land_passengers(crate::combat::PLAYER, market) {
                    Ok(credits) => universe_avionics::Event::PassengersLanded { count: n, credits },
                    Err(reason) => universe_avionics::Event::Refused { reason },
                }
            }
            Some(to) => match self.board_passengers(crate::combat::PLAYER, market, to) {
                Ok(count) => universe_avionics::Event::PassengersBoarded { count },
                Err(reason) => universe_avionics::Event::Refused { reason },
            },
        };
        self.events.push(e);
    }
}
