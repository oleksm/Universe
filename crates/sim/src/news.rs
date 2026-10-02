//! What we've heard (`docs/hypernet.md`, step 4: capture and delivery). A
//! kill or a trade is known to us when its news has come to us at the speed
//! it can travel: seen directly (our own comm hears it), or captured by a
//! relay on the net in its system, passed to the backbone, through gate
//! relays to our system's, and out to us while we're on the net. Off the net
//! we hear nothing more until we're back on it; then everything that's come
//! in meanwhile at once.
//!
//! The client's: it decides nothing in the world. (Ships carrying news in
//! their memories come later.)

use std::collections::HashMap;
use std::sync::Arc;

use glam::DVec3;
use universe_services::records::{Kill, TradeRecord};
use universe_world::charts::Charts;
use universe_world::hypernet::{blocked, nodes, Net, NodeAt};
use universe_world::modules::Comm;
use universe_world::physics::laws::SPEED_OF_LIGHT;
use universe_world::{Facility, StarSystem};

/// A kill or trade, by what tells it apart.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Key {
    Kill { time: u64, killer: usize, victim: usize },
    Trade { time: u64, system: usize, trader: String, market: String, item: String, units: u32 },
    /// A broadcast: an outlet's digest, by its system and when it went out.
    Digest { system: usize, time: u64 },
}

impl Key {
    pub fn kill(k: &Kill) -> Key {
        Key::Kill { time: k.time.to_bits(), killer: k.killer, victim: k.victim }
    }

    pub fn trade(r: &TradeRecord) -> Key {
        Key::Trade { time: r.time.to_bits(), system: r.system, trader: r.trader.clone(), market: r.market.clone(), item: r.item.clone(), units: r.units }
    }
}

/// A system's net as last worked out: when, the system, its bodies' positions then.
struct SystemNet {
    at: f64,
    sys: Arc<StarSystem>,
    positions: Vec<DVec3>,
    net: Net,
}

/// Where we are, to hear from.
pub struct Listener {
    pub system: usize,
    pub at: DVec3,
    pub comm: Comm,
    /// The player's own: what it does itself it knows at once.
    pub player: bool,
}

/// A broadcast put out at a place: (what it is, when, its system, where).
pub type Broadcast = (Key, f64, usize, Facility);

/// A net is worked out again after this long (s): relays move slowly.
const NET_EVERY: f64 = 5.0;
/// News is checked this often (s).
const EVERY: f64 = 0.5;

#[derive(Default)]
pub struct Knowledge {
    /// When each happening was on its own system's backbone (infinite: never: nothing heard it).
    entered: HashMap<Key, f64>,
    /// When we heard each.
    heard: HashMap<Key, f64>,
    nets: HashMap<usize, SystemNet>,
    last: f64,
}

impl Knowledge {
    /// When we heard it (None: not yet).
    pub fn heard(&self, key: &Key) -> Option<f64> {
        self.heard.get(key).copied()
    }

    /// Whether we heard it after `from`, by `to`.
    pub fn heard_in(&self, key: &Key, from: f64, to: f64) -> bool {
        self.heard(key).is_some_and(|t| t > from && t <= to)
    }

    fn net(&mut self, charts: &Charts, system: usize, now: f64) -> &SystemNet {
        let stale = self.nets.get(&system).is_none_or(|n| (now - n.at).abs() > NET_EVERY);
        if stale {
            let sys = charts.system(system);
            let mut positions = Vec::new();
            sys.positions(now, &mut positions);
            let net = Net::at(&sys, nodes(&charts.galaxy, &sys), now, &positions);
            self.nets.insert(system, SystemNet { at: now, sys, positions, net });
        }
        &self.nets[&system]
    }

    /// From each system in the gate network to ours (s): through gate relays
    /// on both ends of each lane, the quickest way.
    fn delays_to_us(&mut self, charts: &Charts, us: usize, now: f64) -> HashMap<usize, f64> {
        let mut systems: Vec<usize> = charts.gate_links.iter().flat_map(|&(a, b)| [a, b]).chain([us]).collect();
        systems.sort();
        systems.dedup();
        // (Out of each system through its gates, and in at the far end.)
        let mut hops: Vec<(usize, usize, f64)> = Vec::new();
        for &s in &systems {
            let out = self.net(charts, s, now).net.gates();
            for (to, d) in out {
                if let Some(back) = self.net(charts, to, now).net.gate_in(s) {
                    hops.push((s, to, d + back));
                }
            }
        }
        let mut dist = HashMap::from([(us, 0.0)]);
        loop {
            let mut changed = false;
            for &(a, b, d) in &hops {
                if let Some(&rest) = dist.get(&b) {
                    let via = d + rest;
                    if dist.get(&a).is_none_or(|&old| via < old - 1e-9) {
                        dist.insert(a, via);
                        changed = true;
                    }
                }
            }
            if !changed {
                break;
            }
        }
        dist
    }

    /// Take in what's come to us by `now`, from the kills and trades on record.
    pub fn update(&mut self, charts: &Charts, now: f64, us: &Listener, kills: &[Kill], trades: &[TradeRecord], broadcasts: &[Broadcast]) {
        if (now - self.last).abs() < EVERY {
            return;
        }
        self.last = now;
        // Our lag from the backbone, if we're on the net.
        let ours = {
            let n = self.net(charts, us.system, now);
            n.net.status(&n.sys, &n.positions, us.at, &us.comm).map(|s| s.lag)
        };
        let to_us = self.delays_to_us(charts, us.system, now);
        let happenings = kills
            .iter()
            .map(|k| (Key::kill(k), k.time, k.system, Some(k.at), None, us.player && (k.killer == crate::combat::PLAYER || k.victim == crate::combat::PLAYER)))
            .chain(trades.iter().map(|r| (Key::trade(r), r.time, r.system, None, r.place, us.player && r.trader == "YOU")))
            .chain(broadcasts.iter().map(|(key, time, system, at)| (key.clone(), *time, *system, None, Some(*at), false)));
        let mut live = std::collections::HashSet::new();
        for (key, time, system, at, place, ours_too) in happenings {
            live.insert(key.clone());
            if self.heard.contains_key(&key) {
                continue;
            }
            // Our own doings we know.
            if ours_too {
                self.heard.insert(key, time);
                continue;
            }
            // Seen with our own comm.
            if let Some(p) = at.filter(|_| system == us.system) {
                let n = self.net(charts, system, now);
                let d = p.distance(us.at);
                if d <= us.comm.capture && !blocked(&n.sys, &n.positions, p, us.at) {
                    self.heard.insert(key, time + d / SPEED_OF_LIGHT);
                    continue;
                }
            }
            // On its system's backbone: heard by a relay as it happened (a
            // fight's light passes once), or from the market's own relay
            // whenever that's on the net.
            if !self.entered.contains_key(&key) {
                let n = self.net(charts, system, now);
                let entered = match (at, place) {
                    (Some(p), _) => Some(n.net.heard(&n.sys, &n.positions, p).map_or(f64::INFINITY, |d| time + d)),
                    (None, Some(f)) => match f {
                        Facility::Station(b) | Facility::Gate(b) => Some(NodeAt::Body(b)),
                        Facility::Spaceport(k) => Some(NodeAt::Port(k)),
                        // (Out at a rock: no relay there.)
                        _ => None,
                    }
                    .map_or(Some(f64::INFINITY), |at| n.net.node(at).and_then(|k| n.net.lag[k]).map(|l| time.max(now - EVERY) + l)),
                    (None, None) => Some(f64::INFINITY),
                };
                if let Some(e) = entered {
                    self.entered.insert(key.clone(), e);
                }
            }
            // Through the gates to our backbone, and out to us.
            let (Some(&entered), Some(&across), Some(lag)) = (self.entered.get(&key), to_us.get(&system), ours) else { continue };
            let arrives = entered + across + lag;
            if arrives <= now {
                // (Back on the net after a while: it all comes in now.)
                self.heard.insert(key, arrives.max(now - EVERY));
            }
        }
        self.heard.retain(|k, _| live.contains(k));
        self.entered.retain(|k, _| live.contains(k));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_kill_by_a_relay_comes_at_once_one_unseen_never_one_next_door_after_the_gates() {
        let w = universe_world::World::new(1984);
        let charts = w.charts();
        let home = w.home_system;
        let sys = charts.system(home);
        let mut positions = Vec::new();
        sys.positions(0.0, &mut positions);
        let station = positions[sys.station().unwrap()];
        let kill = |system: usize, at: DVec3, victim: usize| Kill { time: 0.0, system, killer: 7, victim, killer_name: "A".into(), victim_name: "B".into(), weapon: "GUNFIRE".into(), at, cause: universe_protocol::Cause::Rules };
        // A fight 200,000 km from the station (past our own comm's hearing); one far out in the dark;
        // one by the station of a system next door through a gate.
        let next = charts.gate_links.iter().find_map(|&(a, b)| if a == home { Some(b) } else if b == home { Some(a) } else { None }).unwrap();
        let there = charts.system(next);
        let mut their = Vec::new();
        there.positions(0.0, &mut their);
        let kills = [kill(home, station + DVec3::new(2.0e8, 0.0, 0.0), 1), kill(home, DVec3::new(1.0e14, 0.0, 0.0), 2), kill(next, their[there.station().unwrap()], 3)];
        let us = Listener { system: home, at: station + DVec3::new(5_000.0, 0.0, 0.0), comm: universe_world::ship::starter().comm, player: true };
        let mut news = Knowledge::default();
        news.update(&charts, 2.0, &us, &kills, &[], &[]);
        let heard = |n: &Knowledge, k: &Kill| n.heard(&Key::kill(k));
        assert!(heard(&news, &kills[0]).is_some_and(|t| t < 2.0), "by the station: at once");
        assert!(heard(&news, &kills[2]).is_none(), "next door: not yet");
        news.update(&charts, 60.0, &us, &kills, &[], &[]);
        assert!(heard(&news, &kills[1]).is_none(), "nobody saw it");
        let t = heard(&news, &kills[2]).expect("through the gates by now");
        assert!(t >= universe_world::gate::TRANSIT_TIME, "a crossing at least: {t}");
    }
}
