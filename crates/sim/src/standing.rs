//! Standing (`docs/factions.md`, step 2): each faction's view of every pilot,
//! from the deeds done in its space as they reach it over the hypernet. A
//! faction hears at its seat's station (as anyone there would:
//! `news::Knowledge`), so a deed far off counts late, and one nobody saw
//! never does. Every pilot alike: the player, settlers, pirates.
//!
//! What counts, in a faction's own space (Tuning):
//! - opening fire on a ship that wasn't fair game: `AGGRESSION`;
//! - destroying a ship that wasn't fair game: `MURDER`; one that was: `BOUNTY`;
//! - a trade at one of its markets: `TRADE`.
//!
//! The faction's own bookkeeping (a client of the world, like an outlet):
//! what it makes of a deed is its call.

use std::collections::{HashMap, HashSet};

use universe_world::charts::Charts;
use universe_world::hypernet::{nodes, NodeAt};

use crate::news::{Key, Knowledge, Listener, Sighting};
use crate::universe::Universe;

/// Standing's bounds.
pub const MOST: f64 = 100.0;
/// Opening fire on the innocent in its space.
pub const AGGRESSION: f64 = -10.0;
/// Destroying the innocent in its space.
pub const MURDER: f64 = -30.0;
/// Destroying the fair game in its space.
pub const BOUNTY: f64 = 5.0;
/// A trade at its market.
pub const TRADE: f64 = 0.2;
/// How often the factions take in what they've heard (s).
const EVERY: f64 = 5.0;

/// What a standing means, in words.
pub fn label(s: f64) -> &'static str {
    if s <= -50.0 {
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

/// A faction's desk: where it listens (its seat's station), and what it's heard.
struct Desk {
    system: usize,
    station: usize,
    knows: Knowledge,
}

#[derive(Default)]
pub struct Standings {
    desks: Vec<Desk>,
    /// Each pilot's standing with each faction (by content order); absent: 0.
    table: HashMap<(usize, usize), f64>,
    /// Deeds already counted, by faction.
    counted: HashSet<(Key, usize)>,
    next: f64,
}

impl Standings {
    /// Pilot `pilot`'s standing with faction `faction` (by content order).
    pub fn of(&self, pilot: usize, faction: usize) -> f64 {
        self.table.get(&(pilot, faction)).copied().unwrap_or(0.0)
    }

    fn add(&mut self, pilot: usize, faction: usize, by: f64) {
        let s = self.table.entry((pilot, faction)).or_insert(0.0);
        *s = (*s + by).clamp(-MOST, MOST);
    }

    /// A desk for each faction holding territory: at its seat (the first of
    /// its systems with a station).
    fn open(charts: &Charts) -> Vec<Desk> {
        let n = universe_world::content::content().factions.iter().count();
        (0..n)
            .filter_map(|k| {
                let mut held: Vec<usize> = charts.territory().filter(|(s, _)| charts.holder_index(*s) == Some(k)).map(|(s, _)| s).collect();
                // (The home system first, for the faction that holds it.)
                held.sort_by_key(|&s| (s != charts.home_system, s));
                held.into_iter().find_map(|s| charts.system(s).station().map(|station| Desk { system: s, station, knows: Knowledge::default() }))
            })
            .collect()
    }
}

impl Universe {
    /// The factions take in the deeds that have reached them, when due.
    pub(crate) fn update_standings(&mut self) {
        let now = self.world.time;
        if now < self.standings.next {
            return;
        }
        self.standings.next = now + EVERY;
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
        let mut deeds: Vec<(usize, usize, f64, Key)> = Vec::new();
        for d in &mut self.standings.desks {
            let faction = charts.holder_index(d.system).unwrap_or(0);
            let sys = charts.system(d.system);
            let mut positions = Vec::new();
            sys.positions(now, &mut positions);
            let Some(comm) = nodes(&charts.galaxy, &sys).into_iter().find(|n| n.at == NodeAt::Body(d.station)).map(|n| n.comm) else { continue };
            d.knows.update(&charts, now, &Listener { system: d.system, at: positions[d.station], comm, player: false }, &crate::news::Happenings { kills: &kills, trades: &trades, broadcasts: &[], sightings: &sightings });
            let ours = |system: usize| charts.holder_index(system) == Some(faction);
            for k in kills.iter().filter(|k| ours(k.system) && universe_world::turrets::turret_of(k.killer).is_none()) {
                let key = Key::kill(k);
                if d.knows.heard(&key).is_some() {
                    let fair = self.law.until(k.victim, k.time).is_some();
                    deeds.push((k.killer, faction, if fair { BOUNTY } else { MURDER }, key));
                }
            }
            for r in trades.iter().filter(|r| ours(r.system) && matches!(r.deal, universe_services::records::Deal::Bought | universe_services::records::Deal::Sold)) {
                let key = Key::trade(r);
                if d.knows.heard(&key).is_some() {
                    deeds.push((r.pilot, faction, TRADE, key));
                }
            }
            for (key, ..) in sightings.iter().filter(|s| ours(s.2)) {
                if let (Some(_), Key::Aggression { ship, .. }) = (d.knows.heard(key), key) {
                    deeds.push((*ship, faction, AGGRESSION, key.clone()));
                }
            }
        }
        for (pilot, faction, by, key) in deeds {
            if self.standings.counted.insert((key, faction)) {
                self.standings.add(pilot, faction, by);
            }
        }
        // (Forget what's off the record: it can't come round again.)
        let live: HashSet<Key> = kills.iter().map(Key::kill).chain(trades.iter().map(Key::trade)).chain(sightings.into_iter().map(|s| s.0)).collect();
        self.standings.counted.retain(|(k, _)| live.contains(k));
    }
}
