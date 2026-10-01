//! The NPC operator (a client, with the pilots it employs; see `pilots`):
//! who each settler is (its name, and whether it flies with the pirates or
//! trades), its routes (a new one when it's done), and a trader's choices at
//! each stop. None of it is the world's: the world gets registrations (a
//! name and a pad to start on), and requests (quotes, trades), and is told
//! of the trader's plans only as it declares them.
//!
//! A trader at a stop asks the market service for quotes (every market in
//! the system, as anyone there could see them) and its own account; with
//! the answer it sells what pays here (goods the market wants, or anything
//! that beats what it paid), and buys for the best trip, by margin × units
//! within its credits and hold, the stock here and the demand there. If no
//! trip is worth `MIN_PROFIT`, it moves on through a gate to another system.

use universe_avionics::route::{Route, Stop};
use universe_avionics::{Avionics, NavTarget};
use universe_services::market::{Quote, Side};
use universe_services::records::Deal;
use universe_world::charts::Charts;
use universe_world::ship::HOLD_CAPACITY;
use universe_world::traffic::facilities;
use universe_world::{BodyKind, Facility};

use crate::pilots::Pilot;
use crate::rng::{mix, Rng};
use crate::vessel::Request;

/// Stops on a settler's route.
pub const ROUTE_STOPS: usize = 10;
/// About one settler in `PIRATE_ONE_IN` is a pirate, and as many again traders.
pub const PIRATE_ONE_IN: u64 = 10;
/// A trip must promise at least this (credits), or the trader moves on to another system.
pub const MIN_PROFIT: f64 = 300.0;
/// Goods lines it buys for one trip.
const LINES: usize = 3;
/// Sell without demand only for at least this margin over what was paid.
const MARGIN: f64 = 1.05;
/// Past this share of the hold, sell what the market takes back, loss or not.
const STUCK: f64 = 0.8;

/// A ship the operator puts in the world: its name, and where it starts.
#[derive(Clone, Debug)]
pub struct Registration {
    pub name: String,
    pub at: Stop,
    pub pad: usize,
}

/// A reproducible route of `count` stops (stations and spaceports) across
/// the gate network, from a seed: same seed, same route.
pub fn route(charts: &Charts, seed: u64, count: usize) -> Vec<Stop> {
    let mut systems: Vec<usize> = charts.gate_links.iter().flat_map(|&(a, b)| [a, b]).collect();
    systems.sort();
    systems.dedup();
    let mut candidates = Vec::new();
    for s in systems {
        let sys = charts.system(s);
        for (i, b) in sys.bodies.iter().enumerate() {
            if b.kind == BodyKind::Station {
                candidates.push(Stop { system: s, target: NavTarget::Station(i) });
            }
        }
        for i in 0..sys.spaceports.len() {
            candidates.push(Stop { system: s, target: NavTarget::Spaceport(i) });
        }
    }
    // Shuffle (Fisher-Yates) and take the first `count`: no repeats while
    // there are enough destinations, then cycle through again.
    let mut rng = Rng::new(mix(charts.seed, seed));
    for i in (1..candidates.len()).rev() {
        let j = (rng.next_u64() % (i as u64 + 1)) as usize;
        candidates.swap(i, j);
    }
    let mut stops: Vec<Stop> = Vec::new();
    for pick in candidates.iter().cycle().take(count * 2) {
        if stops.len() == count {
            break;
        }
        if stops.last() != Some(pick) {
            stops.push(*pick);
        }
    }
    stops
}

/// `count` settlers (numbered from `first`), each on its own reproducible
/// route from `seed`, starting at their first stop with staggered departures
/// from `now`: what the world is told of each, and its pilot.
pub fn settlers(charts: &Charts, seed: u64, count: usize, first: usize, now: f64) -> Vec<(Registration, Pilot)> {
    let mut rng = Rng::new(seed);
    let mut out = Vec::new();
    for i in 0..count {
        let route_seed = mix(seed, i as u64);
        let stops = route(charts, route_seed, ROUTE_STOPS);
        let Some(&at) = stops.first() else { continue };
        let route = Route { stops, next: 0, active: true, dwell_until: Some(now + rng.range(0.0, 600.0)), departing: false };
        // Roles, from the seed (the same settlers every time): one slice
        // pirates, another traders, the rest just travel.
        let role = mix(route_seed, 0x0917_27e5) % PIRATE_ONE_IN;
        let (pirate, trader) = (role == 0, role == 1);
        // Named for what they do (the number stays each craft's own).
        let name = format!("{} {}", if pirate { "Pirate" } else if trader { "Trader" } else { "Settler" }, first + out.len() + 1);
        let mut pilot = Pilot::new(Avionics { route, pirate, ..Avionics::default() });
        pilot.trader = trader;
        pilot.route_seed = route_seed;
        out.push((Registration { name, at, pad: (route_seed % 9) as usize }, pilot));
    }
    out
}

/// A pilot whose route is done, parked: a new one (starting from where it is).
pub(crate) fn new_route(pilot: &mut Pilot, charts: &Charts, system: usize) {
    let seed = mix(pilot.route_seed, 1);
    let mut stops = route(charts, seed, ROUTE_STOPS);
    if stops.first().is_some_and(|s| s.system == system) {
        stops.rotate_left(1);
    }
    pilot.route_seed = seed;
    pilot.avionics.route = Route { stops, next: 0, active: true, dwell_until: None, departing: false };
}

/// The market service's answer to a trader's request for quotes: at its
/// market, at the others in the system, and its own account.
#[derive(Clone, Debug)]
pub struct MarketAnswer {
    pub system: usize,
    pub at: Facility,
    /// Every quote here, and here for each item held.
    pub here: Vec<Quote>,
    pub here_held: Vec<Option<Quote>>,
    /// The other markets, quoting `items` (held, then what's buyable here).
    pub items: Vec<usize>,
    pub there: Vec<(Facility, Vec<Option<Quote>>)>,
    pub credits: f64,
    pub hold: Vec<(usize, u32)>,
    pub cargo: f64,
}

/// A trader decides at its stop, from `ans`: what to sell and buy (its
/// trade requests), where to go next (its route), and what it declares.
pub(crate) fn trade(pilot: &mut Pilot, charts: &Charts, ans: &MarketAnswer, requests: &mut Vec<Request>) {
    let mass = |item: usize| charts.goods[item].mass;
    let (mut credits, mut cargo) = (ans.credits, ans.cargo);
    let mut hold: Vec<(usize, u32)> = Vec::new();
    // Sell.
    for (&(item, have), q) in ans.hold.iter().zip(&ans.here_held) {
        let paid = pilot.paid.get(&item).copied().unwrap_or(0.0);
        let units = match q {
            Some(q) => match q.offer.side {
                Side::Buys => have.min(q.level.floor().max(0.0) as u32),
                // A hold nearly full with nothing selling: take the buy-back, at a loss if need be.
                Side::Sells if q.sell >= paid * MARGIN || ans.cargo > HOLD_CAPACITY * STUCK => have,
                Side::Sells => 0,
            },
            None => 0,
        };
        if units > 0 {
            requests.push(Request::Trade { market: ans.at, item, units: -(units as i64) });
            credits += q.map_or(0.0, |q| q.sell) * units as f64;
            cargo -= mass(item) * units as f64;
            if units == have {
                pilot.paid.remove(&item);
            }
        }
        if units < have {
            hold.push((item, have - units));
        }
    }
    // The best trip from here: what the cargo would fetch there over what it
    // cost, plus the margin on what's bought for it.
    let quote = |list: &Vec<Option<Quote>>, item: usize| ans.items.iter().position(|&i| i == item).and_then(|k| list[k]);
    let buyable: Vec<&Quote> = ans.here.iter().filter(|q| q.buy.is_some() && q.level >= 1.0).collect();
    let mut best: Option<(Facility, Vec<(usize, u32, f64)>, f64)> = None;
    for (there, list) in &ans.there {
        let mut value = 0.0;
        for &(item, n) in &hold {
            if let Some(q) = quote(list, item) {
                let units = if q.offer.side == Side::Buys { (n as f64).min(q.level) } else { n as f64 };
                value += (q.sell - pilot.paid.get(&item).copied().unwrap_or(0.0)).max(0.0) * units;
            }
        }
        let mut margins: Vec<(f64, usize, f64, f64, f64)> = buyable
            .iter()
            .filter_map(|h| {
                let t = quote(list, h.offer.item)?;
                let buy = h.buy?;
                let margin = t.sell - buy;
                let demand = if t.offer.side == Side::Buys { t.level } else { f64::INFINITY };
                (margin > 0.0).then_some((margin / mass(h.offer.item), h.offer.item, buy, margin, (h.level * 0.5).min(demand)))
            })
            .collect();
        margins.sort_by(|a, b| b.0.total_cmp(&a.0));
        let (mut room, mut money, mut gain, mut buys) = (HOLD_CAPACITY - cargo, credits, 0.0, Vec::new());
        for (_, item, buy, margin, most) in margins.into_iter().take(LINES) {
            let units = (room / mass(item)).min(money / buy).min(most).floor();
            if units < 1.0 {
                continue;
            }
            room -= units * mass(item);
            money -= units * buy;
            gain += units * margin;
            buys.push((item, units as u32, buy));
        }
        let total = value + gain;
        if total >= MIN_PROFIT && best.as_ref().is_none_or(|b| total > b.2) {
            best = Some((*there, buys, total));
        }
    }
    let sys = charts.system(ans.system);
    let decision = match best {
        Some((to, buys, expect)) => {
            for (item, units, buy) in buys {
                requests.push(Request::Trade { market: ans.at, item, units: units as i64 });
                let before = hold.iter().find(|h| h.0 == item).map_or(0.0, |h| h.1 as f64);
                let avg = pilot.paid.get(&item).copied().unwrap_or(0.0);
                pilot.paid.insert(item, (avg * before + buy * units as f64) / (before + units as f64));
            }
            Some((Stop { system: ans.system, target: to }, Deal::Heading { to: to.name(&sys).to_uppercase(), expect }))
        }
        None => {
            // Nothing worth it here: a neighbouring system (seeded, so reproducible), at one of its markets.
            let links = charts.gate_links_of(ans.system);
            let pick = mix(pilot.route_seed, pilot.stops_made);
            links.get(pick as usize % links.len().max(1)).cloned().and_then(|(next, name)| {
                let there = charts.system(next);
                let markets = facilities(&there);
                markets.get((pick >> 16) as usize % markets.len().max(1)).map(|&target| (Stop { system: next, target }, Deal::MovingOn { to: name.to_uppercase() }))
            })
        }
    };
    if let Some((stop, deal)) = decision {
        // After this stop, on to there; and say so.
        let r = &mut pilot.avionics.route;
        r.stops.truncate(r.next + 1);
        r.stops.push(stop);
        requests.push(Request::Declare { market: ans.at, deal });
    }
}
