//! Standing: each settled system's local authority's view of every pilot,
//! from the deeds done in its system as they reach it over the hypernet. It
//! hears at its station (as anyone there would: `news::Knowledge`), so a deed
//! far out counts late, and one nobody saw never does. Every pilot alike: the
//! player, settlers, pirates.
//!
//! What counts, in its own system (Tuning):
//! - opening fire on a ship that wasn't fair game: `AGGRESSION`;
//! - destroying a ship that wasn't fair game: `MURDER`; one that was: `BOUNTY` (a wreck in a
//!   collision is neither: an accident);
//! - a trade at one of its markets: `TRADE`.
//!
//! The authority's own bookkeeping (a client of the world, like an outlet):
//! what it makes of a deed is its call. Too low, and its turrets fire (`HOSTILE`).

use universe_protocol::ShipId;
use std::collections::{HashMap, HashSet};

use universe_world::charts::Charts;
use universe_world::hypernet::NodeAt;

use crate::news::{Key, Knowledge, Listener, Sighting};
use crate::universe::Universe;

/// Standing's bounds.
pub const MOST: f64 = 100.0;
/// Opening fire on the innocent in its system.
pub const AGGRESSION: f64 = -10.0;
/// Destroying the innocent in its system.
pub const MURDER: f64 = -30.0;
/// Destroying the fair game in its system.
pub const BOUNTY: f64 = 5.0;
/// A trade at its market.
pub const TRADE: f64 = 0.2;
/// At or below this its turrets treat the pilot as fair game.
pub const HOSTILE: f64 = -50.0;
/// The law, in every settled system (one with a station or a port): one who
/// opens fire on the innocent there stays fair game this long (s).
pub const FAIR_GAME: f64 = 600.0;

/// What a standing means, in words.
pub fn label(s: f64) -> &'static str {
    if s <= HOSTILE {
        "HOSTILE"
    } else if s <= -10.0 {
        "UNWELCOME"
    } else if s < 10.0 {
        "NEUTRAL"
    } else if s < 50.0 {
        "FRIENDLY"
    } else {
        "HONOURED"
    }
}

/// A system's authority's desk: where it listens (its station), and what it's heard.
struct Desk {
    system: usize,
    station: usize,
    knows: Knowledge,
}

#[derive(Default)]
pub struct Standings {
    desks: Vec<Desk>,
    /// Each pilot's standing with each settled system; absent: 0.
    table: HashMap<(ShipId, usize), f64>,
    /// Deeds already counted, by system.
    counted: HashSet<(Key, usize)>,
    next: f64,
}

impl Standings {
    /// Pilot `pilot`'s standing with system `system`'s authority.
    pub fn of(&self, pilot: ShipId, system: usize) -> f64 {
        self.table.get(&(pilot, system)).copied().unwrap_or(0.0)
    }

    /// A pilot's standings as saved: (system, standing).
    pub fn restore(&mut self, pilot: ShipId, standing: impl Iterator<Item = (usize, f64)>) {
        self.table.retain(|(p, _), _| *p != pilot);
        for (system, s) in standing {
            self.set(pilot, system, s);
        }
    }

    /// A pilot's standings: (system, standing).
    pub fn all(&self, pilot: ShipId) -> Vec<(usize, f64)> {
        let mut out: Vec<(usize, f64)> = self.table.iter().filter(|((p, _), _)| *p == pilot).map(|(&(_, s), &v)| (s, v)).collect();
        out.sort_by_key(|o| o.0);
        out
    }

    /// Set it outright (a scenario, a test, a court).
    pub fn set(&mut self, pilot: ShipId, system: usize, s: f64) {
        self.table.insert((pilot, system), s.clamp(-MOST, MOST));
    }

    fn add(&mut self, pilot: ShipId, system: usize, by: f64) {
        let s = self.table.entry((pilot, system)).or_insert(0.0);
        *s = (*s + by).clamp(-MOST, MOST);
    }

    /// A desk at each settled system's station.
    fn open(charts: &Charts) -> Vec<Desk> {
        let settled = charts.settled();
        settled.into_iter().filter_map(|s| charts.system(s).station().map(|station| Desk { system: s, station, knows: Knowledge::default() })).collect()
    }
}

impl Universe {
    /// The authorities take in the deeds that have reached them, when due.
    pub(crate) fn update_standings(&mut self) {
        let now = self.world.time;
        if now < self.standings.next {
            return;
        }
        self.standings.next = now + crate::clocks::period(universe_world::registry::ClockKey::Market);
        let charts = self.charts();
        if self.standings.desks.is_empty() {
            self.standings.desks = Standings::open(&charts);
        }
        // Who opened fire on whom, seen where the shooter was.
        let sightings: Vec<Sighting> = self
            .law
            .rulings
            .iter()
            .filter(|r| r.new)
            .filter_map(|r| {
                let (_, system, ship) = self.ship_by_id(r.ship)?;
                Some((Key::Aggression { time: r.evidence.time.to_bits(), ship: r.ship }, r.evidence.time, system, ship.position))
            })
            .collect();
        let (kills, trades) = (self.records.kills.clone(), self.records.trades.clone());
        let mut deeds: Vec<(ShipId, usize, f64, Key)> = Vec::new();
        for d in &mut self.standings.desks {
            let system = d.system;
            let sys = charts.system(system);
            let mut positions = Vec::new();
            sys.positions(now, &mut positions);
            let Some(comm) = universe_world::hypernet::nodes(&charts.galaxy, &sys).into_iter().find(|n| n.at == NodeAt::Body(d.station)).map(|n| n.comm) else { continue };
            d.knows.update(&charts, now, &Listener { system, at: positions[d.station], comm, player: false, in_tube: false }, &crate::news::Happenings { kills: &kills, trades: &trades, broadcasts: &[], sightings: &sightings });
            // (A wreck in a collision is an accident, not a deed: nobody fired.)
            for k in kills.iter().filter(|k| k.system == system && universe_world::turrets::turret_of(k.killer).is_none() && k.weapon != "COLLISION") {
                let key = Key::kill(k);
                if d.knows.heard(&key).is_some() {
                    let fair = self.law.until(k.victim, k.time).is_some();
                    deeds.push((k.killer, system, if fair { BOUNTY } else { MURDER }, key));
                }
            }
            for r in trades.iter().filter(|r| r.system == system && matches!(r.deal, universe_services::records::Deal::Bought | universe_services::records::Deal::Sold)) {
                let key = Key::trade(r);
                if d.knows.heard(&key).is_some() {
                    deeds.push((r.pilot, system, TRADE, key));
                }
            }
            for (key, ..) in sightings.iter().filter(|s| s.2 == system) {
                if let (Some(_), Key::Aggression { ship, .. }) = (d.knows.heard(key), key) {
                    deeds.push((*ship, system, AGGRESSION, key.clone()));
                }
            }
        }
        for (pilot, system, by, key) in deeds {
            if self.standings.counted.insert((key, system)) {
                self.standings.add(pilot, system, by);
            }
        }
        // (Forget what's off the record: it can't come round again.)
        let live: HashSet<Key> = kills.iter().map(Key::kill).chain(trades.iter().map(Key::trade)).chain(sightings.into_iter().map(|s| s.0)).collect();
        self.standings.counted.retain(|(k, _)| live.contains(k));
    }
}
