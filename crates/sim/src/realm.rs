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
pub const BEACON: &str = "transceiver.beacon";
/// The hyper relay planted with it.
pub const BEACON_RELAY: &str = "relay.hyper";

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
        let bare = out.iter().all(|n| !n.backbone);
        let (Some(comm), relay) = (comm_of(BEACON), universe_world::hypernet::relay_lag(BEACON_RELAY)) else { return out };
        for (k, c) in self.claims.iter().enumerate().filter(|(_, c)| c.system == sys.index) {
            out.push(Node { at: NodeAt::Beacon { body: c.body, claim: k }, name: c.name.clone(), comm, backbone: bare && k == self.claims.iter().position(|x| x.system == sys.index).unwrap_or(k), gate_relay: None, offset: c.offset, around: None, relay: relay.or(Some(0.0)) });
        }
        out
    }
}

/// A faction's charter (credits): paid to the market of the station it's founded at.
pub const CHARTER: f64 = 250_000.0;
/// A claim's fee over its beacon's price (credits).
pub const CLAIM_FEE: f64 = 50_000.0;

/// A founded faction's three-letter tag: its words' initials, made up from
/// its letters, and different from the others'.
fn tag_for(name: &str, taken: &[String]) -> String {
    let words: Vec<&str> = name.split_whitespace().collect();
    let letters: Vec<char> = name.chars().filter(|c| c.is_ascii_alphabetic()).collect();
    let mut tag: String = words.iter().filter_map(|w| w.chars().next()).take(3).collect();
    for &c in letters.iter().skip(1) {
        if tag.len() >= 3 {
            break;
        }
        tag.push(c);
    }
    while tag.len() < 3 {
        tag.push('X');
    }
    let mut out = tag.clone();
    let mut n = 0;
    while taken.contains(&out) {
        n += 1;
        out = format!("{}{}", &tag[..2], char::from(b'A' + (n % 26) as u8));
    }
    out
}

impl crate::universe::Universe {
    /// Pilot `id` founds a faction named `name`, docked at a station, paying
    /// its charter there: sworn to it, it holds nothing until it claims.
    /// What's said, or why not.
    pub fn found(&mut self, id: usize, name: &str) -> Result<String, String> {
        use universe_services::{Asset, Party};
        let name = name.trim().to_uppercase();
        if !(3..=24).contains(&name.len()) || !name.chars().all(|c| c.is_ascii_alphanumeric() || c == ' ' || c == '-') {
            return Err("A NAME OF 3 TO 24 LETTERS".into());
        }
        if self.realm.factions.iter().any(|f| f.name == name) {
            return Err(format!("THERE'S A {name} ALREADY"));
        }
        let (system, ship) = self.ship_by_id(id).map(|(_, s, ship)| (s, ship.clone())).ok_or("NO SHIP")?;
        let sys = self.system(system);
        let Some(market @ universe_world::Facility::Station(_)) = universe_world::traffic::docked_at(&sys, &ship) else {
            return Err("DOCK AT A STATION TO FOUND A FACTION".into());
        };
        if let Some(k) = self.standings.member_of(id) {
            return Err(format!("SWORN TO THE {} - LEAVE IT FIRST", self.realm.faction(k).map_or("", |f| f.name.as_str())));
        }
        self.ledger.transfer(Party::Pilot(id), Party::Market(system, market), Asset::Credits, CHARTER, self.tick, universe_protocol::Cause::Rules).map_err(|_| format!("THE CHARTER IS {CHARTER:.0} CR"))?;
        let taken: Vec<String> = self.realm.factions.iter().map(|f| f.tag.clone()).collect();
        let hue = (name.bytes().fold(7u64, |h, b| h.wrapping_mul(31).wrapping_add(b as u64)) % 360) as f32 / 360.0;
        let rgb = |o: f32| 0.55 + 0.45 * ((hue + o) * std::f32::consts::TAU).cos();
        let founder = if id == crate::combat::PLAYER { "YOU".to_string() } else { self.ship_name(id) };
        let f = universe_world::factions::Faction {
            key: format!("faction.founded.{}", self.realm.factions.len()),
            tag: tag_for(&name, &taken),
            name: name.clone(),
            color: [rgb(0.0), rgb(1.0 / 3.0), rgb(2.0 / 3.0)],
            aggression: 600.0,
            hostile: -50.0,
            note: format!("FOUNDED BY {founder}"),
            founder: Some(id),
        };
        let k = std::sync::Arc::make_mut(&mut self.realm).found(f);
        self.standings.swear(id, k);
        Ok(format!("THE {name} IS FOUNDED - PLANT A CLAIM IN AN UNCLAIMED SYSTEM TO HOLD IT"))
    }

    /// Pilot `id` plants a claim beacon where it is for the faction it's
    /// sworn to, in an unclaimed system, paying for the beacon and the claim:
    /// the system is its faction's. What's said, or why not.
    pub fn plant_claim(&mut self, id: usize) -> Result<String, String> {
        use universe_services::{Asset, Party};
        let k = self.standings.member_of(id).ok_or("SWEAR TO A FACTION (OR FOUND ONE) TO CLAIM FOR IT")?;
        let (system, ship) = self.ship_by_id(id).map(|(_, s, ship)| (s, ship.clone())).ok_or("NO SHIP")?;
        if !ship.is_flying() || ship.hyperdrive {
            return Err("IN FLIGHT, OUT OF HYPERDRIVE".into());
        }
        let sys = self.system(system);
        if let Some(f) = self.realm.holder(system) {
            return Err(format!("{} HOLDS THIS SYSTEM", f.name));
        }
        let c = universe_world::content::content();
        let price = [BEACON, BEACON_RELAY].iter().map(|k| c.handle::<universe_world::modules::Module>(k).map_or(0.0, |h| c.get(h).price)).sum::<f64>() + CLAIM_FEE;
        self.ledger.transfer(Party::Pilot(id), Party::World, Asset::Credits, price, self.tick, universe_protocol::Cause::Rules).map_err(|_| format!("A BEACON AND ITS CLAIM: {price:.0} CR"))?;
        let mut positions = Vec::new();
        sys.positions(self.world.time, &mut positions);
        let body = sys.dominant(ship.position, &positions);
        let faction = self.realm.faction(k).cloned().ok_or("NO SUCH FACTION")?;
        let claim = Claim { system, faction: k, body, offset: ship.position - positions[body], name: format!("{} BEACON", faction.tag) };
        std::sync::Arc::make_mut(&mut self.realm).claim(claim);
        Ok(format!("{} CLAIMED FOR THE {} - ITS BEACON RELAYS HERE", sys.name.to_uppercase(), faction.name))
    }
}
