//! The market service's side of trading: pilots' trades (decided by the
//! market, booked in the ledger), quotes on request, and the trade log. What
//! a trader buys and sells, and where it goes, is its own (see `operator`).

use std::collections::{HashMap, VecDeque};

use universe_services::market::{cargo_mass, Order, Quote};
use universe_services::Party;
use universe_world::traffic::{docked_at, facilities};
use universe_world::Facility;

use crate::universe::Universe;
use universe_services::records::{Deal, TradeRecord};

/// What a new settler starts with (credits).
pub const SETTLER_CREDITS: f64 = 3000.0;

/// Markets put out their price boards this often (s), over the hypernet.
pub const BOARD_EVERY: f64 = 120.0;
/// Boards kept this long (s): past the slowest way round a system's net.
const BOARD_KEPT: f64 = 3.0 * 3600.0;

/// A price board: when it was put out, and the market's quote for every good
/// (by the goods' order; None: not traded there).
pub type Board = (f64, Vec<Option<Quote>>);

/// What markets know of each other (`docs/hypernet.md`, step 5): each puts
/// out its board every `BOARD_EVERY`; another market has it once it's come
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
}

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
        let (sys, positions, net) = self.nets.get(&system)?;
        let ours = net.status(sys, positions, p, comm)?.lag;
        let delay = self.lags.get(&(system, f)).copied().flatten()? + ours;
        self.newest(system, f, now - delay).map(|(t, b)| (now - t, b))
    }
}

impl Universe {
    /// Pilot `pilot` (the player's 0, craft i: i + 1) asks the market at `f`
    /// to trade `units` of `item`: the market service decides, the ledger
    /// books it (the request's cause on every entry), and the ship's cargo
    /// mass follows what's in its hold. Credits paid (negative: received),
    /// or why not.
    pub(crate) fn pilot_trade(&mut self, pilot: usize, f: Facility, item: usize, units: i64) -> Result<f64, String> {
        let (system, ship) = if pilot == crate::combat::PLAYER { (self.ship_system, &self.ship) } else { (self.crafts[pilot - 1].system, &self.crafts[pilot - 1].ship) };
        let sys = self.world.system(system);
        let order = Order { pilot, system, market: f, docked_at: docked_at(&sys, ship), room: ship.hold_room(), space: ship.hold_space(), item, units };
        self.messages += 1;
        let cause = universe_protocol::Cause::Message { sender: pilot as u64, id: self.messages };
        let r = self.markets.trade(&mut self.ledger, &sys, order, self.world.time, self.tick, cause);
        if r.is_ok() {
            // The core: the hold weighs what's in it.
            let hold = self.ledger.hold(pilot);
            let (mass, volume) = (cargo_mass(&self.world.goods, &hold), universe_services::market::cargo_volume(&self.world.goods, &hold));
            let ship = if pilot == crate::combat::PLAYER { &mut self.ship } else { &mut self.crafts[pilot - 1].ship };
            ship.cargo = mass;
            ship.cargo_volume = volume;
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
        let Some(market) = docked_at(&sys, &self.ship) else {
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
    pub(crate) fn market_request(&mut self, id: usize, r: crate::vessel::Request) {
        use crate::vessel::Request;
        match r {
            Request::Quotes { system, market } => {
                if id == crate::combat::PLAYER {
                    return;
                }
                let answer = self.market_answer(id, system, market);
                self.tell(id - 1, crate::contract::Msg::Market(Box::new(answer)));
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
        let (live, banned) = self.market_quotes(f);
        let held: Vec<usize> = self.hold().into_iter().map(|(i, _)| i).collect();
        if self.docked_market() == Some(f) {
            let held = held.into_iter().filter(|i| !live.iter().any(|q| q.offer.item == *i)).map(|i| (i, self.quote_for(f, i))).collect();
            return crate::engine::MarketView { market: f, quotes: live, banned, held, age: Some(0.0) };
        }
        let known = self.boards.known_at(self.ship_system, f, self.ship.position, &self.ship.spec().comm, self.world.time);
        let Some((age, board)) = known else { return crate::engine::MarketView { market: f, quotes: Vec::new(), banned, held: Vec::new(), age: None } };
        // (What it listed then, in its listing's order; and what we hold besides.)
        let quotes: Vec<Quote> = live.iter().filter_map(|q| board.get(q.offer.item).copied().flatten()).collect();
        let held = held.into_iter().filter(|i| !quotes.iter().any(|q| q.offer.item == *i)).map(|i| (i, board.get(i).copied().flatten())).collect();
        crate::engine::MarketView { market: f, quotes, banned, held, age: Some(age) }
    }

    /// Every market in the gate network puts out its board, when due.
    pub(crate) fn publish_boards(&mut self) {
        use universe_world::hypernet::{nodes, Net, NodeAt};
        let now = self.world.time;
        if now < self.boards.next {
            return;
        }
        let first = self.boards.boards.is_empty();
        self.boards.next = now + BOARD_EVERY;
        let mut systems: Vec<usize> = self.world.gate_links.iter().flat_map(|&(a, b)| [a, b]).chain(self.markets.economy.places.iter().map(|p| p.system)).collect();
        systems.sort();
        systems.dedup();
        let all: Vec<usize> = (0..self.world.goods.len()).collect();
        for system in systems {
            let sys = self.system(system);
            let mut positions = Vec::new();
            sys.positions(now, &mut positions);
            let net = Net::at(&sys, nodes(&self.world.galaxy, &sys), now, &positions);
            let net = &self.boards.nets.entry(system).insert_entry((sys.clone(), positions, net)).into_mut().2;
            for f in facilities(&sys) {
                let at = match f {
                    Facility::Station(b) | Facility::Gate(b) => Some(NodeAt::Body(b)),
                    Facility::Spaceport(k) => Some(NodeAt::Port(k)),
                    _ => None,
                };
                let lag = at.and_then(|a| net.node(a)).and_then(|k| net.lag[k]);
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
    }

    /// What the market service tells pilot `id` at `at`: its own quotes, the
    /// other markets' as their boards have reached this one over the
    /// hypernet (with their age), and its account.
    fn market_answer(&mut self, id: usize, system: usize, at: Facility) -> crate::contract::MarketAnswer {
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
        // What this station's shipyard asks for it (if it carries it), and
        // what building it takes from the place's stock.
        use universe_services::outfitter;
        let settled = outfitter::settled(&self.markets.economy.places);
        let here = Facility::Station(station);
        let price = match module {
            Some(m) => {
                let o = outfitter::offer(self.world.galaxy.seed, &self.world.gate_links, &settled, system, here, c.get(m));
                if !o.carried {
                    return Err("NOT CARRIED HERE".into());
                }
                o.price
            }
            None => 0.0,
        };
        if let Some(place) = self.markets.economy.place(system, here)
            && let Some(m) = module
        {
            let (kind, tonnes) = outfitter::materials(c.get(m));
            if place.stock_of(kind) < tonnes {
                return Err(format!("OUT OF {} TO BUILD IT", kind.name()));
            }
        }
        let cost = price - taken.map_or(0.0, |m| c.get(m).price * BUYBACK);
        let (me, market) = (Party::Pilot(id), Party::Market(system, here));
        let cause = universe_protocol::Cause::Rules;
        self.ledger.transfer(me, market, Asset::Credits, cost, self.tick, cause)?;
        if let Some(place) = self.markets.economy.place_mut(system, here) {
            if let Some(m) = module {
                let (kind, tonnes) = outfitter::materials(c.get(m));
                place.take(kind, tonnes);
            }
            if let Some(m) = taken {
                let (kind, tonnes) = outfitter::materials(c.get(m));
                place.put(kind, tonnes * 0.5);
            }
        }
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

impl crate::universe::Universe {
    /// What hull `hull` costs pilot `id` at the station it's docked at, and
    /// what its ship fetches in trade: the frame and its stock fit at this
    /// station's prices; the old ship's frame and modules at `BUYBACK`.
    pub fn hull_offer(&self, id: usize, hull: universe_world::ship::Hull) -> Result<(f64, f64), String> {
        use universe_services::outfitter;
        let Some((_, system, ship)) = self.ship_by_id(id) else { return Err("NO SHIP".into()) };
        let sys = self.world.system(system);
        let Some(here @ Facility::Station(_)) = universe_world::traffic::docked_at(&sys, ship) else { return Err("BUY A SHIP DOCKED AT A STATION".into()) };
        let c = universe_world::content::content();
        let settled = outfitter::settled(&self.markets.economy.places);
        let spec = c.get(hull);
        let price = spec.frame.price + spec.fit.iter().map(|(_, m)| outfitter::offer(self.world.galaxy.seed, &self.world.gate_links, &settled, system, here, c.get(*m)).price).sum::<f64>();
        let old = ship.spec();
        let trade_in = BUYBACK * (old.frame.price + old.fit.iter().map(|(_, m)| c.get(*m).price).sum::<f64>());
        Ok((price, trade_in))
    }

    /// Pilot `id` buys hull `hull` (with its stock fit), trading in its ship,
    /// at the station it's docked at: built from the place's metals (the
    /// frame) and its modules' own materials; its cargo moves over (if the
    /// new hold takes it) and its fuel (up to the new tank). The credits it cost.
    pub fn buy_hull_as(&mut self, id: usize, hull: universe_world::ship::Hull) -> Result<f64, String> {
        use universe_services::{outfitter, Asset, Party};
        let (price, trade_in) = self.hull_offer(id, hull)?;
        let Some((_, system, ship)) = self.ship_by_id(id) else { return Err("NO SHIP".into()) };
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
        // What building it takes from the station's stock.
        let metals = universe_world::goods::Category::of("goods.metals").expect("metals are a kind of goods");
        let mut needs: Vec<(universe_world::goods::Category, f64)> = vec![(metals, spec.frame.frame_mass / 1000.0)];
        needs.extend(spec.fit.iter().map(|(_, m)| outfitter::materials(c.get(*m))));
        if let Some(place) = self.markets.economy.place(system, here) {
            for k in universe_world::goods::Category::all() {
                let want: f64 = needs.iter().filter(|(c, _)| *c == k).map(|(_, t)| t).sum();
                if want > 0.0 && place.stock_of(k) < want {
                    return Err(format!("NOT ENOUGH {} TO BUILD IT ({:.0} OF {:.0} T)", k.name(), place.stock_of(k), want));
                }
            }
        }
        let cost = price - trade_in;
        let (me, market) = (Party::Pilot(id), Party::Market(system, here));
        self.ledger.transfer(me, market, Asset::Credits, cost, self.tick, universe_protocol::Cause::Rules)?;
        if let Some(place) = self.markets.economy.place_mut(system, here) {
            for (k, t) in needs {
                place.take(k, t);
            }
        }
        // The new ship where the old one stood, with its cargo and fuel.
        let old = self.ship_by_id(id).expect("there").2.clone();
        let mut new = old.clone();
        new.class = hull;
        new.fit = None;
        // (A new hull: untrimmed.)
        new.trim = Default::default();
        new.refresh();
        new.fuel = old.fuel.min(spec.fuel_capacity);
        new.hull = 1.0;
        new.jets.clear();
        match id {
            crate::combat::PLAYER => self.ship = new,
            _ => self.crafts[id - 1].ship = new,
        }
        Ok(cost)
    }

    /// The player buys a hull (see `buy_hull_as`); the cockpit is told.
    pub fn buy_hull(&mut self, hull: universe_world::ship::Hull) -> Result<f64, String> {
        let r = self.buy_hull_as(crate::combat::PLAYER, hull);
        let name = universe_world::content::content().get(hull).name.clone();
        self.events.push(match &r {
            Ok(credits) => universe_avionics::Event::BoughtShip { name, credits: *credits },
            Err(reason) => universe_avionics::Event::Refused { reason: format!("SHIPYARD: {reason}") },
        });
        r
    }
}

/// A whole hull's repair costs this share of its frame's price, and takes
/// this share of its frame's mass in metals.
pub const REPAIR_PRICE: f64 = 0.3;
pub const REPAIR_METALS: f64 = 0.05;
/// A ship lost is replaced, the same hull and fit, for this share of its value.
pub const INSURANCE_EXCESS: f64 = 0.1;

impl crate::universe::Universe {
    /// Pilot `id`'s hull mended at the station it's docked at, as far as its
    /// credits (and the station's metals) go: `REPAIR_PRICE` of the frame's
    /// price for a whole hull, from `REPAIR_METALS` of its mass in metals.
    /// What it cost, and how far it's mended now.
    pub fn repair(&mut self, id: usize) -> Result<(f64, f64), String> {
        use universe_services::{Asset, Party};
        let Some((_, system, ship)) = self.ship_by_id(id) else { return Err("NO SHIP".into()) };
        let sys = self.world.system(system);
        let Some(here @ Facility::Station(_)) = universe_world::traffic::docked_at(&sys, ship) else { return Err("REPAIRS DOCKED AT A STATION".into()) };
        let missing = 1.0 - ship.hull;
        if missing <= 1e-9 {
            return Err("THE HULL IS SOUND".into());
        }
        let spec = ship.spec();
        let (full_price, full_metals) = (spec.frame.price * REPAIR_PRICE, spec.frame.frame_mass / 1000.0 * REPAIR_METALS);
        let metals = universe_world::goods::Category::of("goods.metals").expect("metals are a kind of goods");
        // As much as the credits, and the station's metals, allow.
        let credits = self.ledger.credits(Party::Pilot(id)).max(0.0);
        let stock = self.markets.economy.place(system, here).map_or(f64::INFINITY, |p| p.stock_of(metals));
        let part = missing.min(credits / full_price).min(stock / full_metals);
        if part <= 1e-6 {
            return Err(if credits < 1.0 { "NO CREDITS FOR REPAIRS".into() } else { "NO METALS FOR REPAIRS".into() });
        }
        let cost = part * full_price;
        self.ledger.transfer(Party::Pilot(id), Party::Market(system, here), Asset::Credits, cost, self.tick, universe_protocol::Cause::Rules)?;
        if let Some(p) = self.markets.economy.place_mut(system, here) {
            p.take(metals, part * full_metals);
        }
        let hull = match id {
            crate::combat::PLAYER => &mut self.ship,
            _ => &mut self.crafts[id - 1].ship,
        };
        hull.hull = (hull.hull + part).min(1.0);
        Ok((cost, hull.hull))
    }

    /// The player's hull mended (see `repair`); the cockpit is told.
    pub fn repair_player(&mut self) {
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
        use universe_services::{Asset, Party};
        let e = match (self.pilot_reach(), universe_world::spaceport::VENDING.get(item)) {
            (Some(universe_world::crew::Reach::Vending(port)), Some(&(what, price, note))) => {
                let market = Party::Market(self.ship_system, Facility::Spaceport(port));
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
        let sys = self.ship_system();
        let r = match universe_world::traffic::docked_at(&sys, &self.ship) {
            Some(Facility::Station(_)) => {
                self.ship.trim = trim.clamped();
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
    fn ship_mut_by_id(&mut self, id: usize) -> Option<&mut universe_world::Ship> {
        if id == crate::combat::PLAYER { Some(&mut self.ship) } else { self.crafts.get_mut(id - 1).map(|c| &mut c.ship) }
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
    pub fn board_passengers(&mut self, id: usize, market: Facility, to: (usize, Facility)) -> Result<u32, String> {
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
    pub fn land_passengers(&mut self, id: usize, market: Facility) -> Result<f64, String> {
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
        let sys = self.ship_system();
        let Some(market) = universe_world::traffic::docked_at(&sys, &self.ship) else {
            self.events.push(universe_avionics::Event::Refused { reason: "PASSENGERS DOCKED OR LANDED".into() });
            return;
        };
        let e = match to {
            None => {
                let n = self.ship.passengers;
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
