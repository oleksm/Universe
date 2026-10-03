//! Standing (`docs/factions.md`, step 2): each faction's view of every pilot,
//! from the deeds done in its space as they reach it over the hypernet. A
//! faction hears at its seat's station (as anyone there would:
//! `news::Knowledge`), so a deed far off counts late, and one nobody saw
//! never does. Every pilot alike: the player, settlers, pirates.
//!
//! What counts, in a faction's own space (Tuning):
//! - opening fire on a ship that wasn't fair game: `AGGRESSION`;
//! - destroying a ship that wasn't fair game: `MURDER`; one that was: `BOUNTY` (a wreck in a
//!   collision is neither: an accident);
//! - a trade at one of its markets: `TRADE`.
//!
//! The faction's own bookkeeping (a client of the world, like an outlet):
//! what it makes of a deed is its call.

use std::collections::{HashMap, HashSet};

use universe_world::charts::Charts;
use universe_world::hypernet::NodeAt;

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
/// Swearing in.
pub const SWORN: f64 = 10.0;
/// Leaving.
pub const LEFT: f64 = -5.0;
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
    /// Who's sworn to which faction (by content order).
    members: HashMap<usize, usize>,
    next: f64,
}

impl Standings {
    /// Pilot `pilot`'s standing with faction `faction` (by content order).
    pub fn of(&self, pilot: usize, faction: usize) -> f64 {
        self.table.get(&(pilot, faction)).copied().unwrap_or(0.0)
    }

    /// The faction pilot `pilot` is sworn to, if any.
    pub fn member_of(&self, pilot: usize) -> Option<usize> {
        self.members.get(&pilot).copied()
    }

    /// Pilot `pilot` swears to faction `faction`: +`SWORN` with it.
    pub(crate) fn swear(&mut self, pilot: usize, faction: usize) {
        self.members.insert(pilot, faction);
        self.add(pilot, faction, SWORN);
    }

    /// Everyone sworn, and to whom.
    pub fn members(&self) -> impl Iterator<Item = (usize, usize)> + '_ {
        self.members.iter().map(|(&p, &k)| (p, k))
    }

    /// A pilot's allegiance and standings as saved.
    pub fn restore(&mut self, pilot: usize, sworn: Option<usize>, standing: impl Iterator<Item = (usize, f64)>) {
        self.table.retain(|(p, _), _| *p != pilot);
        for (k, s) in standing {
            self.set(pilot, k, s);
        }
        match sworn {
            Some(k) => self.members.insert(pilot, k),
            None => self.members.remove(&pilot),
        };
    }

    /// Set it outright (a scenario, a test, a court).
    pub fn set(&mut self, pilot: usize, faction: usize, s: f64) {
        self.table.insert((pilot, faction), s.clamp(-MOST, MOST));
    }

    fn add(&mut self, pilot: usize, faction: usize, by: f64) {
        let s = self.table.entry((pilot, faction)).or_insert(0.0);
        *s = (*s + by).clamp(-MOST, MOST);
    }

    /// A desk for each faction holding territory: at its seat (the first of
    /// its systems with a station).
    fn open(charts: &Charts, realm: &crate::realm::Realm) -> Vec<Desk> {
        (0..realm.factions.len())
            .filter_map(|k| {
                let mut held: Vec<usize> = realm.territory().filter(|(s, _)| realm.holder_index(*s) == Some(k)).map(|(s, _)| s).collect();
                // (The home system first, for the faction that holds it.)
                held.sort_by_key(|&s| (s != charts.home_system, s));
                held.into_iter().find_map(|s| charts.system(s).station().map(|station| Desk { system: s, station, knows: Knowledge::default() }))
            })
            .collect()
    }
}

impl Universe {
    /// Pilot `id` swears to the faction holding the station `market` it's
    /// docked at: what's said, or why not. Not while that faction finds it
    /// unwelcome, nor while sworn to another.
    pub fn enlist(&mut self, id: usize, market: universe_world::Facility) -> Result<String, String> {
        let (system, ship) = self.ship_by_id(id).map(|(_, s, ship)| (s, ship.clone())).ok_or("NO SHIP")?;
        let sys = self.system(system);
        if universe_world::traffic::docked_at(&sys, &ship) != Some(market) || !matches!(market, universe_world::Facility::Station(_)) {
            return Err("DOCK AT A STATION TO ENLIST".into());
        }
        let realm = self.realm.clone();
        let (k, f) = realm.holder_index(system).zip(realm.holder(system)).ok_or("NOBODY HOLDS THIS STATION")?;
        match self.standings.member_of(id) {
            Some(m) if m == k => return Err(format!("ALREADY SWORN TO THE {}", f.name)),
            Some(m) => {
                let other = realm.faction(m).map_or(String::new(), |g| g.name.clone());
                return Err(format!("SWORN TO THE {other} - RESIGN AT ONE OF ITS STATIONS FIRST"));
            }
            None => {}
        }
        if self.standings.of(id, k) <= -10.0 {
            return Err(format!("THE {} WON'T HAVE YOU ({})", f.name, label(self.standings.of(id, k))));
        }
        self.standings.swear(id, k);
        Ok(format!("SWORN TO THE {}", f.name))
    }

    /// Pilot `id` leaves its faction, at one of its stations: what's said, or why not.
    pub fn resign(&mut self, id: usize, market: universe_world::Facility) -> Result<String, String> {
        let k = self.standings.member_of(id).ok_or("SWORN TO NO ONE")?;
        let (system, ship) = self.ship_by_id(id).map(|(_, s, ship)| (s, ship.clone())).ok_or("NO SHIP")?;
        let sys = self.system(system);
        let realm = self.realm.clone();
        let name = realm.faction(k).map_or(String::new(), |f| f.name.clone());
        if universe_world::traffic::docked_at(&sys, &ship) != Some(market) || realm.holder_index(system) != Some(k) {
            return Err(format!("RESIGN AT A STATION OF THE {name}"));
        }
        self.standings.members.remove(&id);
        self.standings.add(id, k, LEFT);
        Ok(format!("LEFT THE {name}"))
    }

    /// The factions take in the deeds that have reached them, when due.
    pub(crate) fn update_standings(&mut self) {
        let now = self.world.time;
        if now < self.standings.next {
            return;
        }
        self.standings.next = now + EVERY;
        let charts = self.charts();
        let realm = self.realm.clone();
        // (A desk for each faction holding something; opened again as that changes.)
        if self.standings.desks.len() != realm.factions.iter().enumerate().filter(|(k, _)| realm.territory().any(|(s, _)| realm.holder_index(s) == Some(*k))).count() {
            self.standings.desks = Standings::open(&charts, &realm);
        }
        // Who opened fire on whom, seen where the shooter was.
        let targets: HashMap<Key, usize> = self.law.rulings.iter().filter(|r| r.new).map(|r| (Key::Aggression { time: r.evidence.time.to_bits(), ship: r.ship }, r.evidence.target)).collect();
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
            let faction = realm.holder_index(d.system).unwrap_or(0);
            let sys = charts.system(d.system);
            let mut positions = Vec::new();
            sys.positions(now, &mut positions);
            let Some(comm) = realm.nodes(&charts.galaxy, &sys).into_iter().find(|n| n.at == NodeAt::Body(d.station)).map(|n| n.comm) else { continue };
            d.knows.update(&charts, &realm, now, &Listener { system: d.system, at: positions[d.station], comm, player: false, in_tube: false }, &crate::news::Happenings { kills: &kills, trades: &trades, broadcasts: &[], sightings: &sightings });
            let ours = |system: usize| realm.holder_index(system) == Some(faction);
            // (Its space, or one of its own: a deed against a member counts wherever it's heard.)
            let members = &self.standings.members;
            let sworn = |id: usize| members.get(&id) == Some(&faction);
            // (A wreck in a collision is an accident, not a deed: nobody fired.)
            for k in kills.iter().filter(|k| (ours(k.system) || sworn(k.victim)) && universe_world::turrets::turret_of(k.killer).is_none() && k.weapon != "COLLISION") {
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
            for (key, ..) in sightings.iter().filter(|s| ours(s.2) || targets.get(&s.0).is_some_and(|&t| sworn(t))) {
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
