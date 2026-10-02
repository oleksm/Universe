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
    /// A world's relay: the world it's round (a constellation in orbit, it
    /// sees past its own world on every side).
    pub around: Option<usize>,
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

/// What a structure has fitted: its comm, and a gate relay's lag if any.
fn fitted(s: &Structure) -> (Comm, Option<f64>) {
    let c = content();
    let modules = s.fit.iter().filter_map(|k| c.handle::<crate::modules::Module>(k)).map(|h| &c.get(h).does);
    let (mut comm, mut relay) = (None, None);
    for d in modules {
        match d {
            Does::GateRelay { lag, .. } => relay = Some(*lag),
            d => comm = comm.or(d.comm()),
        }
    }
    (comm.expect("every structure has a comm (content checks)"), relay)
}

/// The system's relays: every station, spaceport and gate, and in a settled
/// system (a station or ports in it) a relay round every planet and moon.
pub fn nodes(galaxy: &Galaxy, sys: &StarSystem) -> Vec<Node> {
    let mut out = Vec::new();
    for (i, b) in sys.bodies.iter().enumerate() {
        let (kind, lane) = match b.kind {
            BodyKind::Station => (StructureKind::Station, 0.0),
            BodyKind::Gate => {
                let to = b.link.unwrap_or(sys.index);
                (StructureKind::GateRing { class: 0, span_ly: 0.0 }, galaxy.stars[sys.index].position.distance(galaxy.stars[to].position))
            }
            _ => continue,
        };
        let (comm, relay) = fitted(structure(&kind, lane));
        let backbone = kind == StructureKind::Station;
        out.push(Node { at: NodeAt::Body(i), name: b.name.clone(), comm, backbone, gate_relay: relay.zip(b.link).map(|(lag, to)| (to, lag)), offset: DVec3::ZERO, around: None });
    }
    // (The station is the system's hub; with none, its ports are.)
    let hub = !out.iter().any(|n| n.backbone);
    let (comm, _) = fitted(structure(&StructureKind::Spaceport, 0.0));
    for (k, sp) in sys.spaceports.iter().enumerate() {
        out.push(Node { at: NodeAt::Port(k), name: sp.name.clone(), comm, backbone: hub, gate_relay: None, offset: DVec3::ZERO, around: None });
    }
    if !out.is_empty() {
        let (comm, _) = fitted(structure(&StructureKind::Relay, 0.0));
        for (i, b) in sys.bodies.iter().enumerate().filter(|(_, b)| matches!(b.kind, BodyKind::Rocky | BodyKind::GasGiant | BodyKind::IceGiant | BodyKind::Moon)) {
            out.push(Node { at: NodeAt::Body(i), name: format!("{} relay", b.name), comm, backbone: false, gate_relay: None, offset: DVec3::ZERO, around: Some(i) });
        }
    }
    out
}

/// Where a node is at `t` (`positions` at `t`).
pub fn position(sys: &StarSystem, node: &Node, t: f64, positions: &[DVec3]) -> DVec3 {
    match node.at {
        NodeAt::Body(i) => positions[i],
        NodeAt::Port(k) => crate::spaceport::pad_position(sys, k, t, positions),
        NodeAt::Beacon { body, .. } => positions[body] + node.offset,
    }
}

/// The comm a structure product (`structures.ron`-style module key) is: a
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

/// The net at a moment. Each day (at its start, as the worlds stand then)
/// every place that needs the net — the station, ports, gates, beacons —
/// gets its quickest way to the backbone, hopping relay to relay (a tree,
/// not a mesh), and only the relays on those ways are switched on; the rest
/// stand by. As the worlds go round, a line on the day's routes may close:
/// that branch finds another way then.
#[derive(Clone, Debug, Default)]
pub struct Net {
    pub nodes: Vec<Node>,
    pub at: Vec<DVec3>,
    /// Each node's uplink (None: the backbone itself, or cut off).
    pub uplink: Vec<Option<usize>>,
    /// Each node's lag from the backbone along its route (None: cut off).
    pub lag: Vec<Option<f64>>,
    /// Switched on: a place that needs the net, or a relay on one's way in.
    pub used: Vec<bool>,
}

/// The quickest ways to the backbone over the links standing at `at` (the
/// worlds at `positions`): each node's uplink and lag. `fixed`: nodes whose
/// way is settled already (their lag), the rest finding theirs through them.
fn quickest(sys: &StarSystem, nodes: &[Node], at: &[DVec3], positions: &[DVec3], fixed: &[Option<f64>]) -> (Vec<Option<usize>>, Vec<Option<f64>>) {
    let n = nodes.len();
    let mut lag = fixed.to_vec();
    let mut uplink = vec![None; n];
    let mut done = vec![false; n];
    // (Dijkstra; a few dozen nodes.)
    while let Some(a) = (0..n).filter(|&k| !done[k] && lag[k].is_some()).min_by(|&x, &y| lag[x].unwrap_or(f64::MAX).total_cmp(&lag[y].unwrap_or(f64::MAX))) {
        done[a] = true;
        let la = lag[a].unwrap_or(0.0);
        for b in 0..n {
            if done[b] || fixed[b].is_some() {
                continue;
            }
            let d = at[a].distance(at[b]);
            if d > nodes[a].comm.link_with(&nodes[b].comm) || blocked_but(sys, positions, at[a], at[b], [nodes[a].around, nodes[b].around]) {
                continue;
            }
            let via = la + d / SPEED_OF_LIGHT + nodes[b].comm.lag;
            if lag[b].is_none_or(|old| via < old - 1e-12) {
                lag[b] = Some(via);
                uplink[b] = Some(a);
            }
        }
    }
    (uplink, lag)
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
        let seeds: Vec<Option<f64>> = nodes.iter().map(|x| x.backbone.then_some(0.0)).collect();
        // The day's routes, as the worlds stood when it began.
        let day = (t / crate::units::DAY).floor() * crate::units::DAY;
        let mut then = Vec::new();
        sys.positions(day, &mut then);
        let at_then: Vec<DVec3> = nodes.iter().map(|x| position(sys, x, day, &then)).collect();
        let (plan, _) = quickest(sys, &nodes, &at_then, &then, &seeds);
        // Only the relays on a place's way in.
        let needs = |x: &Node| x.around.is_none();
        let mut used = vec![false; n];
        for k in (0..n).filter(|&k| needs(&nodes[k])) {
            let mut c = Some(k);
            while let Some(i) = c.filter(|&i| !used[i]) {
                used[i] = true;
                c = plan[i];
            }
        }
        // Now: the lag along the day's routes where every line still holds.
        let at: Vec<DVec3> = nodes.iter().map(|x| position(sys, x, t, positions)).collect();
        let holds = |a: usize, b: usize| at[a].distance(at[b]) <= nodes[a].comm.link_with(&nodes[b].comm) && !blocked_but(sys, positions, at[a], at[b], [nodes[a].around, nodes[b].around]);
        let mut lag: Vec<Option<f64>> = seeds.clone();
        let mut uplink: Vec<Option<usize>> = vec![None; n];
        for _ in 0..n {
            for b in 0..n {
                if let (None, Some(a)) = (lag[b], plan[b])
                    && let Some(la) = lag[a]
                    && holds(a, b)
                {
                    lag[b] = Some(la + at[a].distance(at[b]) / SPEED_OF_LIGHT + nodes[b].comm.lag);
                    uplink[b] = Some(a);
                }
            }
        }
        // A broken branch finds another way, through what's standing.
        if (0..n).any(|k| used[k] && lag[k].is_none() && plan[k].is_some()) {
            let (up, l) = quickest(sys, &nodes, &at, positions, &lag);
            for k in 0..n {
                if lag[k].is_none() && l[k].is_some() && needs(&nodes[k]) {
                    let mut c = Some(k);
                    while let Some(i) = c.filter(|&i| lag[i].is_none()) {
                        lag[i] = l[i];
                        uplink[i] = up[i];
                        used[i] = true;
                        c = up[i];
                    }
                }
            }
        }
        for k in 0..n {
            if !used[k] {
                lag[k] = None;
                uplink[k] = None;
            }
        }
        Net { nodes, at, uplink, lag, used }
    }

    /// Where a comm at `p` stands on the net: through the node that gets a
    /// message to it soonest (None: no node on the net in reach).
    pub fn status(&self, sys: &StarSystem, positions: &[DVec3], p: DVec3, comm: &Comm) -> Option<Status> {
        let mut best: Option<Status> = None;
        for (k, n) in self.nodes.iter().enumerate() {
            let Some(l) = self.lag[k] else { continue };
            let d = self.at[k].distance(p);
            if d > n.comm.link_with(comm) || blocked_but(sys, positions, self.at[k], p, [n.around, None]) {
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

    /// From the backbone out through each gate relay on the net to the throat:
    /// (the system it leads to, the delay to the far ring (s): the lag to the
    /// gate, its handling, and the crossing).
    pub fn gates(&self) -> Vec<(usize, f64)> {
        self.nodes
            .iter()
            .zip(&self.lag)
            .filter_map(|(n, l)| {
                let (to, handling) = n.gate_relay?;
                Some((to, (*l)? + handling + crate::gate::TRANSIT_TIME))
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
        // Out through a gate relay: the crossing and a little.
        assert!(net.gates().iter().all(|(_, d)| *d >= crate::gate::TRANSIT_TIME && *d < crate::gate::TRANSIT_TIME + 5.0));
        // Behind a world, from its only relay: blocked.
        let p = positions[sys.bodies[station].rail.parent.unwrap()];
        assert!(blocked(&sys, &positions, p + DVec3::X * 1.0e9, p - DVec3::X * 1.0e9));
    }
}
