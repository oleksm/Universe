//! News outlets (`docs/roadmap.md` §2): one at each settled system's station.
//! An outlet hears what reaches its station over the hypernet (as anyone
//! there would: `news::Knowledge`), and every `DIGEST_EVERY` puts out a
//! digest of what it heard since the last: the fighting, the trade. The
//! digest is a broadcast from the station, carried over the net like any
//! news, so a far system's digest comes in late, and off the net none does.
//!
//! The outlets' own (clients of the world, like traders): what's news is
//! their call, not the world's.

use universe_services::records::{Deal, Kill, TradeRecord};
use universe_world::charts::Charts;
use universe_world::hypernet::NodeAt;
use universe_world::Facility;

use crate::news::{Broadcast, Key, Knowledge, Listener};

/// An outlet puts out a digest this often (s of world time).
pub const DIGEST_EVERY: f64 = 600.0;
/// Digests kept.
const KEPT: usize = 40;
/// How often outlets listen (s).
const LISTEN_EVERY: f64 = 5.0;

/// A digest as put out.
#[derive(Clone, Debug)]
pub struct Digest {
    pub outlet: String,
    pub system: usize,
    pub station: usize,
    pub time: f64,
    pub headlines: Vec<String>,
}

impl Digest {
    pub fn key(&self) -> Key {
        Key::Digest { system: self.system, time: self.time.to_bits() }
    }
}

struct Outlet {
    name: String,
    system: usize,
    station: usize,
    knows: Knowledge,
    /// When it last put out a digest (or opened).
    last: f64,
}

#[derive(Default)]
pub struct Newsroom {
    outlets: Vec<Outlet>,
    pub digests: Vec<Digest>,
    listened: f64,
}

/// The outlets' mastheads, picked by the system's seed.
const MASTHEADS: [&str; 5] = ["HERALD", "COURIER", "WIRE", "DISPATCH", "BULLETIN"];

impl Newsroom {
    /// An outlet at the station of each system in the gate network.
    pub fn new(charts: &Charts, now: f64) -> Self {
        let systems = charts.settled();
        let outlets = systems
            .into_iter()
            .filter_map(|system| {
                let sys = charts.system(system);
                let station = sys.station()?;
                let seed = charts.galaxy.stars[system].seed;
                let name = format!("{} {}", universe_world::names::star_name(seed).to_uppercase(), MASTHEADS[(seed % MASTHEADS.len() as u64) as usize]);
                Some(Outlet { name, system, station, knows: Knowledge::default(), last: now })
            })
            .collect();
        Newsroom { outlets, digests: Vec::new(), listened: f64::NEG_INFINITY }
    }

    /// The digests as broadcasts from their stations.
    pub fn broadcasts(&self) -> Vec<Broadcast> {
        self.digests.iter().map(|d| (d.key(), d.time, d.system, Facility::Station(d.station))).collect()
    }

    /// Each outlet hears what's reached its station by `now`, and puts out a digest when due.
    pub fn update(&mut self, charts: &Charts, now: f64, kills: &[Kill], trades: &[TradeRecord]) {
        if now - self.listened < LISTEN_EVERY {
            return;
        }
        self.listened = now;
        for o in &mut self.outlets {
            let sys = charts.system(o.system);
            let mut positions = Vec::new();
            sys.positions(now, &mut positions);
            let relay = universe_world::hypernet::nodes(&charts.galaxy, &sys).into_iter().find(|n| n.at == NodeAt::Body(o.station)).map(|n| n.comm);
            let Some(comm) = relay else { continue };
            o.knows.update(charts, now, &Listener { system: o.system, at: positions[o.station], comm, player: false, in_tube: false }, &crate::news::Happenings { kills, trades, ..Default::default() });
            if (now / DIGEST_EVERY).floor() > (o.last / DIGEST_EVERY).floor() {
                let headlines = compile(charts, o, kills, trades, now);
                if !headlines.is_empty() {
                    self.digests.push(Digest { outlet: o.name.clone(), system: o.system, station: o.station, time: now, headlines });
                }
                o.last = now;
            }
        }
        let excess = self.digests.len().saturating_sub(KEPT);
        self.digests.drain(..excess);
    }
}

/// What an outlet makes of what it heard since its last digest: its own
/// system first (the fighting, the trade), then a line for each other
/// system it heard from.
fn compile(charts: &Charts, o: &Outlet, kills: &[Kill], trades: &[TradeRecord], now: f64) -> Vec<String> {
    let name = |system: usize| universe_world::names::star_name(charts.galaxy.stars[system].seed).to_uppercase();
    let fights: Vec<&Kill> = kills.iter().filter(|k| o.knows.heard_in(&Key::kill(k), o.last, now)).collect();
    let deals: Vec<&TradeRecord> = trades.iter().filter(|r| matches!(r.deal, Deal::Bought | Deal::Sold) && o.knows.heard_in(&Key::trade(r), o.last, now)).collect();
    let mut out = Vec::new();
    // Here.
    let here_fights: Vec<&&Kill> = fights.iter().filter(|k| k.system == o.system).collect();
    if !here_fights.is_empty() {
        out.push(format!("{} DESTROYED HERE", ships(here_fights.len())));
        for k in here_fights.iter().rev().take(2) {
            out.push(format!("{} DESTROYED {} - {}", k.killer_name, k.victim_name, k.weapon));
        }
    }
    let here_deals: Vec<&&TradeRecord> = deals.iter().filter(|r| r.system == o.system).collect();
    if !here_deals.is_empty() {
        let turnover: f64 = here_deals.iter().map(|r| r.amount).sum();
        let mut markets: Vec<(&str, usize)> = Vec::new();
        for r in &here_deals {
            match markets.iter_mut().find(|m| m.0 == r.market) {
                Some(m) => m.1 += 1,
                None => markets.push((&r.market, 1)),
            }
        }
        let busiest = markets.iter().max_by_key(|m| m.1).map_or(String::new(), |m| m.0.to_uppercase());
        out.push(format!("{} TRADES HERE FOR {:.0} CR - BUSIEST: {busiest}", here_deals.len(), turnover));
        if let Some(big) = here_deals.iter().max_by(|a, b| a.amount.total_cmp(&b.amount)) {
            let did = if big.deal == Deal::Bought { "BOUGHT" } else { "SOLD" };
            out.push(format!("BIGGEST: {} {did} {} {} AT {} FOR {:.0} CR", big.trader, big.units, big.item, big.market.to_uppercase(), big.amount));
        }
    }
    let heard_else = fights.iter().any(|k| k.system != o.system) || deals.iter().any(|r| r.system != o.system);
    if out.is_empty() && heard_else {
        out.push("QUIET HERE".into());
    }
    // Elsewhere, a line a system.
    let mut others: Vec<usize> = fights.iter().map(|k| k.system).chain(deals.iter().map(|r| r.system)).filter(|&s| s != o.system).collect();
    others.sort();
    others.dedup();
    for system in others {
        let k = fights.iter().filter(|k| k.system == system).count();
        let d: Vec<&&TradeRecord> = deals.iter().filter(|r| r.system == system).collect();
        let mut parts = Vec::new();
        if k > 0 {
            parts.push(format!("{} DESTROYED", ships(k)));
        }
        if !d.is_empty() {
            parts.push(format!("{} TRADES FOR {:.0} CR", d.len(), d.iter().map(|r| r.amount).sum::<f64>()));
        }
        out.push(format!("FROM {}: {}", name(system), parts.join(", ")));
    }
    out
}

fn ships(n: usize) -> String {
    if n == 1 { "1 SHIP".into() } else { format!("{n} SHIPS") }
}
