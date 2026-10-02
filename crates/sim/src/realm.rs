//! The realm (`docs/factions.md`): the factions as they stand — the world's
//! own and any founded since — who holds which system, and the claims
//! planted. World state that changes (founding, claiming), kept by the
//! world's tick and shared with clients through the view.
//!
//! A system is held by the faction that owns its station, ports and gates
//! (at the start: `factions::territory`), or, where there were none, by the
//! one that planted a claim beacon there. A beacon is a hypernet relay: a
//! faction's relays are its borders.

use glam::DVec3;
use universe_world::charts::Charts;
use universe_world::factions::Faction;
use universe_world::hypernet::{comm_of, nodes, Node, NodeAt};
use universe_world::{Galaxy, StarSystem};

/// The relay a claim beacon is (a module of the content).
pub const BEACON: &str = "relay.beacon";

/// A claim beacon: where (beside a body of a system), for whom.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Claim {
    pub system: usize,
    pub faction: usize,
    pub body: usize,
    pub offset: DVec3,
    pub name: String,
}

#[derive(Clone, Debug, Default)]
pub struct Realm {
    /// Every faction, by index: the content's first, then those founded.
    pub factions: Vec<Faction>,
    /// Who holds each held system: (system, faction), by system.
    holders: Vec<(usize, usize)>,
    pub claims: Vec<Claim>,
}

impl Realm {
    /// The world's factions and their territory at the start.
    pub fn new(charts: &Charts) -> Self {
        let factions: Vec<Faction> = universe_world::content::content().factions.iter().map(|(_, f)| f.clone()).collect();
        let holders = universe_world::factions::territory(charts.seed, &charts.gate_links, charts.home_system, factions.len());
        Realm { factions, holders, claims: Vec::new() }
    }

    /// The faction holding system `i` (None: unclaimed), by index.
    pub fn holder_index(&self, i: usize) -> Option<usize> {
        self.holders.iter().find(|h| h.0 == i).map(|h| h.1)
    }

    pub fn holder(&self, i: usize) -> Option<&Faction> {
        self.faction(self.holder_index(i)?)
    }

    pub fn faction(&self, k: usize) -> Option<&Faction> {
        self.factions.get(k)
    }

    /// Every held system, and who holds it.
    pub fn territory(&self) -> impl Iterator<Item = (usize, &Faction)> + '_ {
        self.holders.iter().filter_map(|&(s, k)| Some((s, self.faction(k)?)))
    }

    /// A faction founded: its index.
    pub fn found(&mut self, f: Faction) -> usize {
        self.factions.push(f);
        self.factions.len() - 1
    }

    /// A claim planted: the system is its faction's.
    pub fn claim(&mut self, c: Claim) {
        self.holders.retain(|h| h.0 != c.system);
        self.holders.push((c.system, c.faction));
        self.holders.sort();
        self.claims.push(c);
    }

    /// A system's relays: its structures', and the beacons planted in it. A
    /// beacon is the backbone where there's no station or port.
    pub fn nodes(&self, galaxy: &Galaxy, sys: &StarSystem) -> Vec<Node> {
        let mut out = nodes(galaxy, sys);
        let bare = out.iter().all(|n| !n.backbone) && sys.spaceports.is_empty();
        let Some(comm) = comm_of(BEACON) else { return out };
        for (k, c) in self.claims.iter().enumerate().filter(|(_, c)| c.system == sys.index) {
            out.push(Node { at: NodeAt::Beacon { body: c.body, claim: k }, name: c.name.clone(), comm, backbone: bare, gate_relay: None, offset: c.offset });
        }
        out
    }
}
