//! The hypernet in a star system (`docs/hypernet.md`, step 2): its nodes (the
//! structures' relays), which of them link (in each other's reach, with no
//! world between), and how long a message takes from the backbone to each.
//! A ship is on the net when its comm links to a node that is.
//!
//! Physical facts only: where the relays are, what they reach, the light
//! time. What's said over it comes later (capture and delivery).

use glam::DVec3;

use crate::content::content;
use crate::galaxy::Galaxy;
use crate::modules::{Comm, Does};
use crate::structures_catalogue::{Structure, StructureKind};
use crate::system::{BodyKind, StarSystem};
use universe_physics::laws::SPEED_OF_LIGHT;

/// Where a node is: a body (a station, a gate), or a spaceport on its world.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NodeAt {
    Body(usize),
    Port(usize),
    /// A beacon someone planted (a claim): beside `body`, at the node's `offset`.
    Beacon { body: usize, claim: usize },
}

/// A relay in the system.
#[derive(Clone, Debug)]
pub struct Node {
    pub at: NodeAt,
    pub name: String,
    pub comm: Comm,
    /// The system's backbone: its station's relay (with no station, its ports').
    pub backbone: bool,
    /// A gate relay: the system its twin is in, and its handling lag (s).
    pub gate_relay: Option<(usize, f64)>,
    /// A beacon's place from its body (m; zero for the rest).
    pub offset: DVec3,
    /// An orbital site: the world it's round (a few satellites, one always
    /// in sight: it sees past its own world on every side).
    pub around: Option<usize>,
    /// Its hyper relay's handling lag (s); None: on the ground, no relay (a
    /// port talks up to the transceivers over its world).
    pub relay: Option<f64>,
    /// Its hyper relay's cadence (s: it throws a batch this often).
    pub cadence: f64,
}

/// The structure that stands for a body or port of the seeded world: the
/// platform for a station, the spaceport for a port, and for a gate the
/// smallest ring class spanning its lane (the largest if none does).
fn structure(kind: &StructureKind, lane_ly: f64) -> &'static Structure {
    let c = content();
    let all = || c.structures.iter().map(|(_, s)| s);
    match kind {
        StructureKind::GateRing { .. } => {
            let mut rings: Vec<&Structure> = all().filter(|s| matches!(s.kind, StructureKind::GateRing { .. })).collect();
            rings.sort_by(|a, b| span(a).total_cmp(&span(b)));
            rings.iter().find(|s| span(s) >= lane_ly).or(rings.last()).copied().expect("the world builds gate rings")
        }
        k => all().find(|s| std::mem::discriminant(&s.kind) == std::mem::discriminant(k)).expect("the world builds it"),
    }
}

fn span(s: &Structure) -> f64 {
    if let StructureKind::GateRing { span_ly, .. } = s.kind { span_ly } else { 0.0 }
}

/// The ring a gate of the seeded world is: the smallest class spanning its lane.
pub fn gate_ring(galaxy: &Galaxy, sys: &StarSystem, gate: usize) -> &'static Structure {
    let to = sys.bodies[gate].link.unwrap_or(sys.index);
    structure(&StructureKind::GateRing { class: 0, span_ly: 0.0, capture: 0.0 }, galaxy.stars[sys.index].position.distance(galaxy.stars[to].position))
}

/// The fastest a gate catches a ship entering it (m/s): its ring's.
pub fn capture_speed(galaxy: &Galaxy, sys: &StarSystem, gate: usize) -> f64 {
    if let StructureKind::GateRing { capture, .. } = gate_ring(galaxy, sys, gate).kind { capture } else { 0.0 }
}

/// What a structure has fitted: its comm (a transceiver, or a ground
/// terminal), and its hyper relay's and gate relay's (lag, cadence), if any.
/// A relay's (handling lag, cadence) (s).
type RelaySpec = (f64, f64);

fn fitted(s: &Structure) -> (Comm, Option<RelaySpec>, Option<RelaySpec>) {
    let c = content();
    let modules = s.fit.iter().filter_map(|k| c.handle::<crate::modules::Module>(k)).map(|h| &c.get(h).does);
    let (mut comm, mut hyper, mut gate) = (None, None, None);
    for d in modules {
        match d {
            Does::GateRelay { lag, cadence, .. } => gate = Some((*lag, *cadence)),
            Does::HyperRelay { lag, cadence, .. } => hyper = Some((*lag, *cadence)),
            d => comm = comm.or(d.comm()),
        }
    }
    (comm.expect("every structure has a comm (content checks)"), hyper, gate)
}

/// A hyper relay's (handling lag, cadence) (s), by its module's key.
pub fn relay_lag(module: &str) -> Option<(f64, f64)> {
    let c = content();
    c.handle::<crate::modules::Module>(module).and_then(|h| match c.get(h).does {
        Does::HyperRelay { lag, cadence, .. } => Some((lag, cadence)),
        _ => None,
    })
}

/// The system's net: its sites in space (the station, its gates, and in a
/// settled system an orbital site round every planet and moon), each a
/// transceiver and a hyper relay; and its ports on the ground, each a
/// terminal up to the transceivers over its world.
pub fn nodes(galaxy: &Galaxy, sys: &StarSystem) -> Vec<Node> {
    let mut out = Vec::new();
    for (i, b) in sys.bodies.iter().enumerate() {
        let (kind, lane) = match b.kind {
            BodyKind::Station => (StructureKind::Station, 0.0),
            BodyKind::Gate => {
                let to = b.link.unwrap_or(sys.index);
                (StructureKind::GateRing { class: 0, span_ly: 0.0, capture: 0.0 }, galaxy.stars[sys.index].position.distance(galaxy.stars[to].position))
            }
            _ => continue,
        };
        let (comm, relay, gate) = fitted(structure(&kind, lane));
        let backbone = kind == StructureKind::Station;
        // (A gate relay's capsules cross the lane's tube, thrown at its cadence.)
        let gate_relay = gate.zip(b.link).map(|((lag, cadence), to)| (to, lag + capsule_time(crate::sheet::GATE_CAPSULE, lane * crate::units::LIGHT_YEAR, cadence)));
        out.push(Node { at: NodeAt::Body(i), name: b.name.clone(), comm, backbone, gate_relay, offset: DVec3::ZERO, around: None, relay: relay.map(|r| r.0), cadence: relay.map_or(0.0, |r| r.1) });
    }
    let (comm, _, _) = fitted(structure(&StructureKind::Spaceport, 0.0));
    for (k, sp) in sys.spaceports.iter().enumerate() {
        out.push(Node { at: NodeAt::Port(k), name: sp.name.clone(), comm, backbone: false, gate_relay: None, offset: DVec3::ZERO, around: None, relay: None, cadence: 0.0 });
    }
    if !out.is_empty() {
        let (comm, relay, _) = fitted(structure(&StructureKind::Orbital, 0.0));
        for (i, b) in sys.bodies.iter().enumerate().filter(|(_, b)| matches!(b.kind, BodyKind::Rocky | BodyKind::GasGiant | BodyKind::IceGiant | BodyKind::Moon)) {
            out.push(Node { at: NodeAt::Body(i), name: format!("{} orbital", b.name), comm, backbone: false, gate_relay: None, offset: DVec3::ZERO, around: Some(i), relay: relay.map(|r| r.0), cadence: relay.map_or(0.0, |r| r.1) });
        }
    }
    out
}

/// Data across a tube `span` m long in capsules of `capsule` kg thrown every
/// `cadence` s (s): half the cadence (on average, the wait for the next throw),
/// and the capsule's natural crossing.
pub fn capsule_time(capsule: f64, span: f64, cadence: f64) -> f64 {
    cadence / 2.0 + universe_physics::hyper::tube_natural_time(capsule, span)
}
/// Where a node is at `t` (`positions` at `t`).
pub fn position(sys: &StarSystem, node: &Node, t: f64, positions: &[DVec3]) -> DVec3 {
    match node.at {
        NodeAt::Body(i) => positions[i],
        NodeAt::Port(k) => crate::spaceport::pad_position(sys, k, t, positions),
        NodeAt::Beacon { body, .. } => positions[body] + node.offset,
    }
}

/// The comm a structure product (an equipment key) is: a
/// beacon's, say.
pub fn comm_of(module: &str) -> Option<Comm> {
    let c = content();
    c.handle::<crate::modules::Module>(module).and_then(|h| c.get(h).does.comm())
}

/// Whether a world (a star, planet or moon) stands between `a` and `b`.
pub fn blocked(sys: &StarSystem, positions: &[DVec3], a: DVec3, b: DVec3) -> bool {
    blocked_but(sys, positions, a, b, [None, None])
}

/// `blocked`, but for the worlds in `skip` (a world's relay sees past its own).
pub fn blocked_but(sys: &StarSystem, positions: &[DVec3], a: DVec3, b: DVec3, skip: [Option<usize>; 2]) -> bool {
    // (A hair under the radius: a port on the ground sees its own sky.)
    sys.bodies.iter().zip(positions).enumerate().any(|(i, (body, p))| !skip.contains(&Some(i)) && matches!(body.kind, BodyKind::Star | BodyKind::Rocky | BodyKind::GasGiant | BodyKind::IceGiant | BodyKind::Moon) && universe_physics::segment_distance(a, b, *p) < body.rail.radius * 0.999)
}

/// The net at a moment. Each day (as the sites stand at its start) the
/// system's hyper relays link its sites the shortest way all told (a
/// minimum spanning tree: each link to a near neighbour, no more links than
/// it takes); messages cross their tubes in capsules, at the flow's settle
/// cadence (`capsule_time`): a second or two a hop. Ports on
/// the ground talk up to a transceiver in sight, at light speed; so do ships
/// within a transceiver's radius.
#[derive(Clone, Debug, Default)]
pub struct Net {
    pub nodes: Vec<Node>,
    pub at: Vec<DVec3>,
    /// Each node's uplink (None: the backbone itself, or cut off).
    pub uplink: Vec<Option<usize>>,
    /// Each node's lag from the backbone along its route (None: cut off).
    pub lag: Vec<Option<f64>>,
    /// On the net (its relay linked in, or a port with a transceiver in sight).
    pub used: Vec<bool>,
}

/// A ship's (or anything's) place on the net.
#[derive(Clone, Debug, PartialEq)]
pub struct Status {
    /// From the backbone to here (s): light time and each node's handling.
    pub lag: f64,
    /// The node it links through.
    pub via: usize,
}

impl Net {
    /// The net of `nodes` at `t` (`positions` at `t`).
    pub fn at(sys: &StarSystem, nodes: Vec<Node>, t: f64, positions: &[DVec3]) -> Net {
        let n = nodes.len();
        let sites: Vec<usize> = (0..n).filter(|&k| nodes[k].relay.is_some()).collect();
        let mut uplink: Vec<Option<usize>> = vec![None; n];
        let mut lag: Vec<Option<f64>> = vec![None; n];
        let at: Vec<DVec3> = nodes.iter().map(|x| position(sys, x, t, positions)).collect();
        // The day's links: a minimum spanning tree over the sites as they stood
        // at its start, grown from the backbone (Prim's; a few dozen sites).
        if let Some(&root) = sites.iter().find(|&&k| nodes[k].backbone).or(sites.first()) {
            let day = (t / crate::units::DAY).floor() * crate::units::DAY;
            let mut then = Vec::new();
            sys.positions(day, &mut then);
            let at_then: Vec<DVec3> = nodes.iter().map(|x| position(sys, x, day, &then)).collect();
            let mut joined = vec![false; n];
            joined[root] = true;
            let mut best: Vec<(f64, usize)> = (0..n).map(|k| (at_then[k].distance(at_then[root]), root)).collect();
            for _ in 1..sites.len() {
                let Some(&k) = sites.iter().filter(|&&k| !joined[k]).min_by(|&&x, &&y| best[x].0.total_cmp(&best[y].0)) else { break };
                joined[k] = true;
                uplink[k] = Some(best[k].1);
                for &m in sites.iter().filter(|&&m| !joined[m]) {
                    let d = at_then[m].distance(at_then[k]);
                    if d < best[m].0 {
                        best[m] = (d, k);
                    }
                }
            }
            // Now: the lag along the tree, through hyperspace.
            lag[root] = Some(0.0);
            for _ in 0..sites.len() {
                for &k in &sites {
                    if let (None, Some(u)) = (lag[k], uplink[k])
                        && let Some(lu) = lag[u]
                    {
                        lag[k] = Some(lu + capsule_time(crate::sheet::RELAY_CAPSULE, at[u].distance(at[k]), nodes[k].cadence) + nodes[k].relay.unwrap_or(0.0));
                    }
                }
            }
        }
        // The ports: up to the transceiver in sight that gets them in soonest
        // (their own world's orbital site is always in sight).
        for k in (0..n).filter(|&k| nodes[k].relay.is_none()) {
            let ground = match nodes[k].at {
                NodeAt::Port(p) => Some(sys.spaceports[p].body),
                _ => None,
            };
            let up = sites
                .iter()
                .filter_map(|&s| {
                    let d = at[s].distance(at[k]);
                    let sight = nodes[s].around.is_some() && nodes[s].around == ground || !blocked_but(sys, positions, at[s], at[k], [nodes[s].around, None]);
                    (d <= nodes[s].comm.link && sight).then(|| Some((s, lag[s]? + d / SPEED_OF_LIGHT + nodes[k].comm.lag))).flatten()
                })
                .min_by(|a, b| a.1.total_cmp(&b.1));
            if let Some((s, l)) = up {
                uplink[k] = Some(s);
                lag[k] = Some(l);
            }
        }
        let used = lag.iter().map(|l| l.is_some()).collect();
        Net { nodes, at, uplink, lag, used }
    }

    /// Where a comm at `p` stands on the net: through the node that gets a
    /// message to it soonest (None: no node on the net in reach).
    pub fn status(&self, sys: &StarSystem, positions: &[DVec3], p: DVec3, comm: &Comm) -> Option<Status> {
        let mut best: Option<Status> = None;
        for (k, n) in self.nodes.iter().enumerate() {
            // (Transceivers in space only: within one's radius, in sight.)
            let Some(l) = self.lag[k].filter(|_| n.relay.is_some()) else { continue };
            let d = self.at[k].distance(p);
            if d > n.comm.link || blocked_but(sys, positions, self.at[k], p, [n.around, None]) {
                continue;
            }
            let lag = l + d / SPEED_OF_LIGHT + n.comm.lag + comm.lag;
            if best.as_ref().is_none_or(|b| lag < b.lag) {
                best = Some(Status { lag, via: k });
            }
        }
        best
    }

    /// How long after something happens at `p` it's on the backbone: heard
    /// by the soonest relay on the net in its capture range with the line
    /// clear (light time), handled, and passed in (None: no relay on the net
    /// hears it).
    pub fn heard(&self, sys: &StarSystem, positions: &[DVec3], p: DVec3) -> Option<f64> {
        let mut best: Option<f64> = None;
        for (k, n) in self.nodes.iter().enumerate() {
            let Some(l) = self.lag[k] else { continue };
            let d = self.at[k].distance(p);
            if d > n.comm.capture || blocked_but(sys, positions, self.at[k], p, [n.around, None]) {
                continue;
            }
            let t = d / SPEED_OF_LIGHT + n.comm.lag + l;
            best = Some(best.map_or(t, |b: f64| b.min(t)));
        }
        best
    }

    /// The links in use: each relay to its uplink.
    pub fn routes(&self) -> impl Iterator<Item = (usize, usize)> + '_ {
        self.uplink.iter().enumerate().filter_map(|(k, u)| u.map(|u| (k, u)))
    }

    /// The node at `at`, if it's one of the net's.
    pub fn node(&self, at: NodeAt) -> Option<usize> {
        self.nodes.iter().position(|n| n.at == at)
    }

    /// From the backbone out through each gate relay on the net to the far
    /// ring: (the system it leads to, the delay (s): the lag to the gate, and
    /// its relay's handling and capsules' crossing of the lane's tube).
    pub fn gates(&self) -> Vec<(usize, f64)> {
        self.nodes
            .iter()
            .zip(&self.lag)
            .filter_map(|(n, l)| {
                let (to, handling) = n.gate_relay?;
                Some((to, (*l)? + handling))
            })
            .collect()
    }

    /// The lag from the far ring of the gate from `from` in to the backbone
    /// (None: no relay there, or it's dark).
    pub fn gate_in(&self, from: usize) -> Option<f64> {
        self.nodes.iter().zip(&self.lag).find(|(n, _)| n.gate_relay.is_some_and(|g| g.0 == from)).and_then(|(_, l)| *l)
    }

    /// The systems its gate relays reach (from nodes on the net).
    pub fn gates_out(&self) -> impl Iterator<Item = usize> + '_ {
        self.nodes.iter().zip(&self.lag).filter_map(|(n, l)| l.and(n.gate_relay.map(|g| g.0)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_home_net_links_its_relays_and_a_ship_near_is_on_it_a_ship_far_is_not() {
        let w = crate::World::new(1984);
        let sys = w.system(w.home_system);
        let mut positions = Vec::new();
        sys.positions(0.0, &mut positions);
        let net = Net::at(&sys, nodes(&w.galaxy, &sys), 0.0, &positions);
        assert!(net.nodes.iter().any(|n| n.backbone), "a backbone");
        // Its gates are on the net (a port may not be: its world turns it away
        // from every relay, and back).
        assert!(net.nodes.iter().zip(&net.lag).filter(|(n, _)| n.gate_relay.is_some()).all(|(_, l)| l.is_some()));
        assert!(net.gates_out().count() > 0);
        // A tree: one uplink for each relay on the net but the backbone's.
        let on = net.lag.iter().zip(&net.nodes).filter(|(l, n)| l.is_some() && !n.backbone).count();
        assert_eq!(net.routes().count(), on);
        let comm = crate::ship::starter().comm;
        // By the station: on the net, a whisker of lag.
        let station = sys.station().unwrap();
        let near = positions[station] + DVec3::new(5_000.0, 0.0, 0.0);
        let s = net.status(&sys, &positions, near, &comm).expect("on the net by the station");
        assert!(s.lag < 0.2, "{}", s.lag);
        // Far out past the last world and gate: nobody hears it.
        let far = DVec3::new(1.0e14, 0.0, 0.0);
        assert!(net.status(&sys, &positions, far, &comm).is_none());
        // A fight by the station is on the backbone within a second; one far out isn't heard.
        assert!(net.heard(&sys, &positions, near).is_some_and(|t| t < 1.0));
        assert!(net.heard(&sys, &positions, far).is_none());
        // Out through a gate relay: its handling, the flow's cadence, and the capsules'
        // 200 ms a light year along the lane.
        let lane = |to: usize| w.galaxy.stars[w.home_system].position.distance(w.galaxy.stars[to].position);
        let site = |to: usize| net.nodes.iter().zip(&net.lag).find(|(n, _)| n.gate_relay.is_some_and(|g| g.0 == to)).and_then(|(_, l)| *l).unwrap_or(0.0);
        assert!(net.gates().iter().all(|&(to, d)| (d - site(to) - (0.2 * lane(to) + 1.5 + 1.0)).abs() < 0.1), "{:?}", net.gates());
        // Behind a world, from its only relay: blocked.
        let p = positions[sys.bodies[station].rail.parent.unwrap()];
        assert!(blocked(&sys, &positions, p + DVec3::X * 1.0e9, p - DVec3::X * 1.0e9));
    }
}
