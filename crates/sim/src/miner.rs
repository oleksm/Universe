//! The miner's business (the NPC operator's, a client's): what a player
//! could do, done the same way. Its route is a work site, an asteroid field
//! in its home system, then a market there to sell at. The route autopilot
//! takes it out to the field; there it picks a rock worth digging (ore price
//! × dig rate, a little of its own choice among the best), closes on the
//! surface and matches its drift (the follow program), fires the anchor,
//! digs until the hold is full, lets go, and moves its route on; at the
//! market it asks for quotes and sells its ore. Worked out rocks and anchors
//! that didn't hold: it picks again. The world sees only its commands and
//! requests.

use universe_avionics::follow::{Anchor, Manoeuvre};
use universe_avionics::route::{Route, Stop, WORKING};
use universe_avionics::{Avionics, Bus, Event, NavTarget};
use universe_services::market::Side;
use universe_world::charts::Charts;
use universe_world::goods::TONNE;
use universe_world::mining::{self, ANCHOR_REACH, ANCHOR_SPEED};
use universe_world::ship::{ShipCommands, SHIP_RADIUS};
use universe_world::{Facility, ShipEvent, ShipState, StarSystem};

use crate::contract::MarketAnswer;
use crate::rng::{mix, Rng};
use crate::vessel::Request;

/// It fires the anchor drifting slower than this against the surface (m/s).
const FIRE_DRIFT: f64 = 0.6 * ANCHOR_SPEED;

/// The rock a miner is working (field, body among the field's bodies), and
/// how many it has tried.
#[derive(Clone, Copy, Debug, Default)]
pub struct Dig {
    pub rock: Option<(usize, usize)>,
    pub tries: u64,
}

/// The value of digging body `i` of field `f` (credits per second).
fn worth(charts: &Charts, sys: &StarSystem, f: usize, i: usize) -> f64 {
    let bodies = sys.field_bodies(f);
    bodies[i].rock.as_ref().map_or(0.0, |r| charts.goods[mining::ore(r).item()].price / TONNE * mining::dig_rate(r))
}

/// A miner's route in `system` (seeded): the field best worth working (its
/// remnant), then the market it sells at. None where there are no fields.
pub fn route(charts: &Charts, system: usize, seed: u64) -> Option<Vec<Stop>> {
    let sys = charts.system(system);
    let market = sys.station().map(Facility::Station).or_else(|| (!sys.spaceports.is_empty()).then_some(Facility::Spaceport(0)))?;
    let value: Vec<f64> = (0..sys.fields.len()).map(|f| worth(charts, &sys, f, sys.fields[f].body)).collect();
    let best = value.iter().copied().fold(0.0, f64::max);
    let good: Vec<usize> = (0..value.len()).filter(|&f| value[f] >= 0.5 * best && best > 0.0).collect();
    let f = *good.get((mix(seed, 0x6d69_6e65) % good.len().max(1) as u64) as usize)?;
    Some(vec![Stop { system, target: NavTarget::Asteroid(sys.fields[f].body) }, Stop { system, target: market }])
}

/// A miner parked with its route done: the next trip.
pub(crate) fn new_route(a: &mut Avionics, dig: &mut Dig, charts: &Charts, system: usize, seed: u64) -> bool {
    let Some(stops) = route(charts, system, seed) else { return false };
    a.route = Route { stops, next: 0, active: true, dwell_until: None, departing: false, stay: None };
    *dig = Dig::default();
    true
}

/// A rock among field `f` to dig: one of the best few by worth (the
/// `tries`-th pick from `seed`).
fn choose(charts: &Charts, sys: &StarSystem, f: usize, seed: u64, tries: u64) -> Option<usize> {
    let bodies = sys.field_bodies(f);
    let mut rocks: Vec<(usize, f64)> = sys.field_rocks(f).filter(|&i| bodies[i].rail.radius > 10.0).map(|i| (i, worth(charts, sys, f, i))).collect();
    rocks.sort_by(|a, b| b.1.total_cmp(&a.1).then(a.0.cmp(&b.0)));
    rocks.truncate(8);
    let mut rng = Rng::new(mix(seed, tries));
    rocks.get((rng.next_u64() % rocks.len().max(1) as u64) as usize).map(|r| r.0)
}

/// The miner at work, once a think: what its ship's `feed` brought, then
/// what to do next. Its commands go through its avionics, like anyone's.
pub(crate) fn work(a: &mut Avionics, dig: &mut Dig, charts: &Charts, seed: u64, feed: &[ShipEvent], bus: &mut impl Bus, events: &mut Vec<Event>) {
    let ship = bus.ship().clone();
    let at_site = a.route.active && a.route.dwell_until == Some(WORKING) || dig.rock.is_some();
    if !at_site {
        return;
    }
    let stopped = |why: &str| feed.iter().any(|e| matches!(e, ShipEvent::ExcavatorStopped { why: w } if w == why));
    let full = stopped("HOLD FULL") || ship.hold_room() < 1.0;
    match ship.state {
        ShipState::Anchored { .. } if full => {
            // Done here: let go, and on to the market.
            order(a, bus, events, |c| c.anchor = Some(false));
            *dig = Dig::default();
            a.route.active = true;
            a.route.dwell_until = None;
            a.route.next += 1;
        }
        ShipState::Anchored { .. } if stopped("ROCK WORKED OUT") => {
            order(a, bus, events, |c| c.anchor = Some(false));
            dig.rock = None;
            dig.tries += 1;
        }
        ShipState::Anchored { .. } if !ship.excavator => order(a, bus, events, |c| c.excavate = Some(true)),
        ShipState::Anchored { .. } => {}
        ShipState::Flying if !ship.hyperdrive => {
            let sys = bus.star_system();
            if dig.rock.is_none() {
                let Some(NavTarget::Asteroid(remnant)) = a.route.current().map(|s| s.target) else { return };
                let Some(f) = sys.fields.iter().position(|f| f.body == remnant) else { return };
                let Some(i) = choose(charts, &sys, f, seed, dig.tries) else { return };
                dig.rock = Some((f, i));
            }
            let Some((f, i)) = dig.rock else { return };
            if feed.iter().any(|e| matches!(e, ShipEvent::AnchorFailed { .. })) {
                dig.tries += 1;
            }
            // Close on it; there, with the surface, fire the anchor.
            let (center, velocity) = sys.field_body_state(f, i, bus.time());
            let b = &sys.field_bodies(f)[i];
            let gap = ship.position.distance(center) - b.surface_radius_at(center, ship.position, bus.time()) - SHIP_RADIUS;
            let drift = (ship.velocity - velocity - b.angular_velocity().cross(ship.position - center)).length();
            if gap < ANCHOR_REACH * 0.8 && drift < FIRE_DRIFT {
                a.stop_following(bus, events);
                order(a, bus, events, |c| {
                    c.anchor = Some(true);
                    c.throttle = 0.0;
                    c.rcs = glam::DVec3::ZERO;
                });
            } else if a.following.is_none_or(|fo| fo.anchor != (Anchor::Rock { field: f, body: i })) {
                let route = a.route.clone();
                a.follow(bus, Anchor::Rock { field: f, body: i }, Manoeuvre::Surface(mining::CLOSE_STANDOFF), events);
                // (The follow program takes the route off; the work site is still the stop.)
                a.route = route;
            }
        }
        _ => {}
    }
}

/// Its ship's devices told: what they hold, changed by `change`.
fn order(a: &mut Avionics, bus: &mut impl Bus, events: &mut Vec<Event>, change: impl FnOnce(&mut ShipCommands)) {
    let mut c = bus.ship().holding();
    change(&mut c);
    a.command(bus, &c, events);
}

/// A miner at a market, from `ans`: it sells its ore (whatever the market
/// will take of it).
pub(crate) fn sell(ans: &MarketAnswer, requests: &mut Vec<Request>) {
    for (&(item, have), q) in ans.hold.iter().zip(&ans.here_held) {
        let ore = item >= universe_world::goods::catalog_size();
        let Some(q) = q.filter(|_| ore && have > 0) else { continue };
        let units = match q.offer.side {
            Side::Buys => have.min(q.level.floor().max(0.0) as u32),
            Side::Sells => have,
        };
        if units > 0 {
            requests.push(Request::Trade { market: ans.at, item, units: -(units as i64) });
        }
    }
}
