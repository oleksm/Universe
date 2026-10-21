//! A design's harness: a run from the power hub to every module that draws
//! power, and from every other source to the hub, laid along the frame's
//! members. Each run is on the bus its power calls for, in the lightest cable
//! of the registry's that carries its current (derated for the runs bundled
//! with it) and loses no more than `MAX_DROP` of its voltage.
use super::*;
use universe_sim::world::registry::{registry, EquipmentFunction, RunKind};

/// The ship's DC buses (V): electronics, the ship's systems, and the power
/// bus that the sources feed and the big loads draw from.
pub const BUSES: [f64; 3] = [28.0, 270.0, 3000.0];
/// The most each lower bus is given (W); above the last, the power bus.
const BUS_UP_TO: [f64; 2] = [1_000.0, 30_000.0];
/// The most of its voltage a run may lose (out and back).
pub(super) const MAX_DROP: f64 = 0.05;
/// Copper's resistivity at 20 C (ohm m).
pub(super) const COPPER: f64 = 1.72e-8;
/// The most parallel pairs a run is given before it's said not to fit (a
/// pole of a few big cores, not a sheaf of thin ones).
const MOST_PAIRS: usize = 4;

/// The bus for a load of `watts`: its voltage.
pub fn bus_for(watts: f64) -> f64 {
    BUS_UP_TO.iter().position(|&w| watts <= w).map_or(BUSES[2], |k| BUSES[k])
}

/// What a bundle of `n` runs leaves of each one's rating alone: they warm each
/// other, and in vacuum only radiation takes the heat. Invented, to review
/// (the order of aerospace bundle derating: about half for a dozen or more).
fn bundled(n: usize) -> f64 {
    (1.0 / (1.0 + 0.08 * (n.max(1) - 1) as f64)).max(0.5)
}

#[derive(Clone, Debug)]
pub struct WireRun {
    /// The module it feeds (or the source it brings in), by block id.
    pub module: String,
    /// The bus (V).
    pub volts: f64,
    /// What it carries (W).
    pub watts: f64,
    /// Its cable's record, if one carries it.
    pub cable: Option<String>,
    /// Pairs in parallel (each a plus and a minus).
    pub pairs: usize,
    /// Its way, from the hub out.
    pub path: Vec<Vec3>,
    /// m, one way.
    pub length: f64,
    /// kg: its cable, both ways, and its connectors at both ends.
    pub mass: f64,
    /// The share of its voltage it loses at full load.
    pub drop: f64,
}

#[derive(Clone, Debug, Default)]
pub struct Harness {
    /// The module the runs meet at.
    pub hub: Option<String>,
    pub runs: Vec<WireRun>,
    /// What can't be wired as asked.
    pub faults: Vec<String>,
}

impl Harness {
    pub fn mass(&self) -> f64 {
        self.runs.iter().map(|r| r.mass).sum()
    }
}

/// A placed module's part in the harness, from its record: what it draws and
/// gives (W), whether it works only in air (a fan: its runs are cooled by air
/// whenever they carry), and, a converter, the bus it gives (V), how much
/// (W) and how efficiently.
struct Power {
    draws: f64,
    gives: f64,
    air_only: bool,
    converts: Option<(f64, f64, f64)>,
}

fn power(f: &Fitted) -> Power {
    let none = Power { draws: 0.0, gives: 0.0, air_only: false, converts: None };
    let Some(e) = registry().equipment(&f.key) else { return none };
    let mut p = Power { draws: e.needs.power.unwrap_or(0.0), ..none };
    match &e.function {
        EquipmentFunction::PowerPlant(x) => p.gives = x.output,
        EquipmentFunction::Battery(x) => p.gives = x.rate.unwrap_or(0.0),
        EquipmentFunction::SolarArray(x) => p.gives = x.output.unwrap_or(0.0),
        EquipmentFunction::Engine(x) => p.air_only = x.min_density.is_some_and(|d| d > 0.0),
        EquipmentFunction::Converter(x) => p.converts = Some((x.to_voltage, x.carries, x.efficiency)),
        _ => {}
    }
    p
}

/// The harness cables the registry has for `volts`: hook-up wire on the low
/// buses, the power bus's feeders (copper or aluminium) on it; not a
/// building's cable or a grid's.
fn cables(volts: f64) -> Vec<&'static universe_sim::world::registry::Run> {
    let family = if volts < BUSES[2] { "template.wire" } else { "template.feeder" };
    registry().runs.iter().filter(|r| r.kind == RunKind::Cable && r.template.as_deref() == Some(family) && r.rating.voltage.is_some_and(|v| v >= volts) && r.rating.current.is_some()).collect()
}

/// A cable's conductor's conductivity (S/m), from its material's record.
fn conductivity(c: &universe_sim::world::registry::Run) -> f64 {
    c.per_m.built_of.first().and_then(|b| registry().material(&b.item)).and_then(|m| m.electrical_magnetic.electrical_conductivity).unwrap_or(1.0 / COPPER)
}

/// Shortest ways from one node: the node, each node's last step, its distance.
type Tree = (usize, Vec<usize>, Vec<f32>);

/// The frame as a graph: its members' ends, joined where they meet.
struct Graph {
    at: Vec<Vec3>,
    edges: Vec<Vec<(usize, f32)>>,
}

impl Graph {
    fn new(beams: &[Beam]) -> Self {
        let mut g = Graph { at: Vec::new(), edges: Vec::new() };
        let mut index: std::collections::HashMap<glam::IVec3, usize> = Default::default();
        for b in beams {
            let mut node = |p: Vec3| *index.entry(key(p)).or_insert_with(|| { g.at.push(p); g.edges.push(Vec::new()); g.at.len() - 1 });
            let (x, y) = (node(b.a), node(b.b));
            let d = b.a.distance(b.b);
            g.edges[x].push((y, d));
            g.edges[y].push((x, d));
        }
        g
    }
    fn nearest(&self, p: Vec3) -> Option<usize> {
        (0..self.at.len()).min_by(|&a, &b| self.at[a].distance_squared(p).total_cmp(&self.at[b].distance_squared(p)))
    }
    /// From `from`'s nearest node, the shortest way along the members to every
    /// other: each node's last step. A member already carrying `laid` runs
    /// counts as longer, so runs that would crowd it go round.
    fn tree(&self, from: Vec3, laid: &std::collections::HashMap<Seg, usize>) -> Option<Tree> {
        let s = self.nearest(from)?;
        let mut dist = vec![f32::INFINITY; self.at.len()];
        let mut prev = vec![usize::MAX; self.at.len()];
        dist[s] = 0.0;
        let mut open: std::collections::BinaryHeap<std::cmp::Reverse<(u32, usize)>> = Default::default();
        open.push(std::cmp::Reverse((0, s)));
        while let Some(std::cmp::Reverse((d, n))) = open.pop() {
            if d > (dist[n] * 1000.0) as u32 {
                continue;
            }
            for &(m, w) in &self.edges[n] {
                let nd = dist[n] + w * (1.0 + SPREAD * laid.get(&seg(self.at[n], self.at[m])).copied().unwrap_or(0) as f32);
                if nd < dist[m] {
                    dist[m] = nd;
                    prev[m] = n;
                    open.push(std::cmp::Reverse(((nd * 1000.0) as u32, m)));
                }
            }
        }
        Some((s, prev, dist))
    }
    /// A run's way from `from` to `to`: along the members as far as they go,
    /// straight where the frame doesn't reach.
    fn way(&self, from: Vec3, tree: &Option<Tree>, to: Vec3) -> Vec<Vec3> {
        let mut path = vec![from];
        if let (Some((s, prev, dist)), Some(e)) = (tree, self.nearest(to)) && dist[e].is_finite() {
            let mut back = vec![e];
            while back.last() != Some(s) {
                back.push(prev[*back.last().unwrap()]);
            }
            path.extend(back.iter().rev().map(|&n| self.at[n]));
        }
        path.push(to);
        path.dedup_by(|a, b| a.distance(*b) < 1e-3);
        path
    }
}

/// A point to the millimetre, to know where members meet and runs share.
fn key(p: Vec3) -> glam::IVec3 {
    (p * 1000.0).round().as_ivec3()
}

/// A stretch between two points, either way round.
type Seg = (glam::IVec3, glam::IVec3);

fn seg(a: Vec3, b: Vec3) -> Seg {
    let (x, y) = (key(a), key(b));
    if x.to_array() < y.to_array() { (x, y) } else { (y, x) }
}

/// A run's stretches along the frame: not its tails to the modules at its
/// ends, where the runs fan out to their terminals.
fn along(p: &[Vec3]) -> Vec<Seg> {
    p.windows(2).skip(1).take(p.len().saturating_sub(3)).map(|w| seg(w[0], w[1])).collect()
}

/// How much longer a member counts for each power-bus run already on it.
const SPREAD: f32 = 0.5;

/// The design's harness as it stands.
pub fn harness(plan: &Plan, fit: &[Fitted]) -> Harness {
    if plan.wiring.is_some() { return super::wire_tool::harness(plan, fit); }
    let mut h = Harness::default();
    let placed: Vec<(&Block, Power)> = plan.blocks.iter().filter_map(|b| {
        let p = power(fit.iter().find(|f| f.id == kind(&b.id))?);
        (p.draws > 0.0 || p.gives > 0.0 || p.converts.is_some()).then_some((b, p))
    }).collect();
    if placed.iter().all(|p| p.1.draws <= 0.0) {
        return h;
    }
    // (The hub: switchgear if there is some, else the biggest source.)
    let hub = placed.iter().find(|p| p.0.id.contains("switchgear")).or_else(|| placed.iter().filter(|p| p.1.gives > 0.0).max_by(|a, b| a.1.gives.total_cmp(&b.1.gives)));
    let Some(hub) = hub.map(|p| p.0) else {
        h.faults.push("NOTHING GIVES POWER: NO BATTERY, PLANT OR ARRAY".into());
        return h;
    };
    h.hub = Some(hub.id.clone());
    let g = Graph::new(&plan.beams);
    let mut trees: std::collections::HashMap<String, Option<Tree>> = Default::default();
    // (What may draw in vacuum: all but what works only in air. A source's run
    // carries no more in vacuum than that.)
    let in_vacuum: f64 = placed.iter().filter(|p| !p.1.air_only && p.1.converts.is_none()).map(|p| p.1.draws).sum();
    let converters: Vec<(&Block, f64, f64, f64)> = placed.iter().filter_map(|p| p.1.converts.map(|(v, w, e)| (p.0, v, w, e))).collect();
    let mut on: std::collections::HashMap<String, f64> = Default::default();
    let mut missing: std::collections::BTreeMap<u32, (usize, f64)> = Default::default();
    // (Each run: from where, to which module, on which bus, carrying how much in
    // air and in vacuum.)
    let mut asked: Vec<(&Block, &Block, f64, f64, f64)> = Vec::new();
    for (b, p) in &placed {
        if b.id == hub.id {
            continue;
        }
        if p.gives > 0.0 {
            asked.push((hub, b, BUSES[2], p.gives, p.gives.min(in_vacuum)));
        } else if let Some((_, w, e)) = p.converts {
            asked.push((hub, b, BUSES[2], w / e, w / e));
        } else if p.draws > 0.0 {
            let volts = bus_for(p.draws);
            let vacuum = if p.air_only { 0.0 } else { p.draws };
            // (A low bus's load draws from the nearest converter to its bus.)
            let from = if volts < BUSES[2] {
                match converters.iter().filter(|c| c.1 == volts).min_by(|x, y| x.0.at.distance_squared(b.at).total_cmp(&y.0.at.distance_squared(b.at))) {
                    Some(c) => {
                        *on.entry(c.0.id.clone()).or_default() += p.draws;
                        c.0
                    }
                    None => {
                        let m = missing.entry(volts as u32).or_default();
                        m.0 += 1;
                        m.1 += p.draws;
                        hub
                    }
                }
            } else {
                hub
            };
            asked.push((from, b, volts, p.draws, vacuum));
        }
    }
    // (The power bus's runs laid biggest first, each round the feeders already
    // laid where it can; the low buses' by the shortest way.)
    asked.sort_by(|a, b| b.3.max(b.4).total_cmp(&a.3.max(a.4)));
    let none = std::collections::HashMap::new();
    let mut laid: std::collections::HashMap<Seg, usize> = Default::default();
    let mut runs: Vec<(WireRun, f64, f64)> = asked.into_iter().map(|(from, to, volts, air, vacuum)| {
        let path = if volts >= BUSES[2] {
            let path = g.way(from.at, &g.tree(from.at, &laid), to.at);
            for s in along(&path) {
                *laid.entry(s).or_default() += 1;
            }
            path
        } else {
            let tree = trees.entry(from.id.clone()).or_insert_with(|| g.tree(from.at, &none));
            g.way(from.at, tree, to.at)
        };
        let length = path.windows(2).map(|w| f64::from(w[0].distance(w[1]))).sum();
        (WireRun { module: to.id.clone(), volts, watts: air.max(vacuum), cable: None, pairs: 1, path, length, mass: 0.0, drop: 0.0 }, air, vacuum)
    }).collect();
    for (c, _, w, _) in &converters {
        let load = on.get(&c.id).copied().unwrap_or(0.0);
        if load > *w {
            h.faults.push(format!("{} OVERLOADED: {:.1} OF {:.1} KW", c.id, load / 1000.0, w / 1000.0));
        }
    }
    for (volts, (n, w)) in &missing {
        // (The smallest converter the registry has that would carry them.)
        // (The smallest converter the registry has that carries them; none
        // alone: as many of the biggest as do.)
        let all: Vec<(f64, &str)> = registry().equipment.iter().filter_map(|e| match &e.function { EquipmentFunction::Converter(x) if x.to_voltage as u32 == *volts => Some((x.carries, e.identity.key.as_str())), _ => None }).collect();
        let one = all.iter().filter(|c| c.0 >= *w).min_by(|a, b| a.0.total_cmp(&b.0)).map(|c| (1, c.1));
        let fits = one.or_else(|| all.iter().max_by(|a, b| a.0.total_cmp(&b.0)).map(|c| ((w / c.0).ceil() as usize, c.1)));
        h.faults.push(format!("NO {volts} V CONVERTER FOR {n} LOADS, {:.1} KW{}", w / 1000.0, fits.map_or(String::new(), |(k, c)| format!(": FIT {}{}", if k > 1 { format!("{k} x ") } else { String::new() }, c.trim_start_matches("equipment.")))));
    }
    // (How many runs of a bus share each stretch along the frame: the bundle
    // each lies in at its fullest. Each bus is laid apart from the others, as
    // high voltage is kept from low; at a module's terminals the runs fan out.)
    let mut shared: std::collections::HashMap<(Seg, u32), usize> = Default::default();
    for (r, _, _) in &runs {
        for s in along(&r.path) {
            *shared.entry((s, r.volts as u32)).or_default() += 1;
        }
    }
    for (r, air, vacuum) in &mut runs {
        let bundle = along(&r.path).into_iter().map(|s| shared[&(s, r.volts as u32)]).max().unwrap_or(1);
        let (air, vacuum) = (*air / r.volts, *vacuum / r.volts);
        // (Each cable and number of pairs that carries it in vacuum and in air,
        // and loses little enough of its voltage; of the fewest pairs, the
        // lightest.)
        let pick = cables(r.volts).into_iter().flat_map(|c| (1..=MOST_PAIRS).map(move |pairs| (c, pairs))).filter_map(|(c, pairs)| {
            let n = pairs as f64;
            let in_vacuum = c.rating.current.unwrap_or(0.0) * bundled(bundle);
            let in_air = if bundle > 1 { c.rating.current_air_bundle.map(|a| a * (bundled(bundle) / bundled(6)).min(1.0)) } else { c.rating.current_air }.unwrap_or(in_vacuum);
            let drop = 2.0 * r.length / (conductivity(c) * c.size) * air.max(vacuum) / n / r.volts;
            if in_vacuum * n < vacuum || in_air * n < air || drop > MAX_DROP {
                return None;
            }
            let ends = registry().connector(&c.connector).and_then(|k| k.physical.mass).unwrap_or(0.0);
            // (A high-voltage pole has a connector of its own; a shell takes both.)
            let per_end = if r.volts > 600.0 { 2 * pairs } else { pairs };
            Some((c, pairs, drop, c.per_m.mass * r.length * 2.0 * n + ends * 2.0 * per_end as f64))
        }).min_by(|a, b| a.1.cmp(&b.1).then(a.3.total_cmp(&b.3)));
        match pick {
            Some((c, pairs, drop, mass)) => {
                r.cable = Some(c.identity.key.clone());
                (r.pairs, r.drop, r.mass) = (pairs, drop, mass);
            }
            None => h.faults.push(format!("{}: {:.0} A ON {:.0} V, NO CABLE CARRIES IT", r.module, air.max(vacuum), r.volts)),
        }
    }
    h.runs = runs.into_iter().map(|r| r.0).collect();
    h
}

impl Harness {
    /// Its mass laid along its runs: each stretch's share of its run's, at the
    /// stretch's middle, with the stretch's size (for the inertia).
    pub fn masses(&self) -> Vec<(Vec3, f64, Vec3)> {
        self.runs.iter().filter(|r| r.length > 0.0).flat_map(|r| r.path.windows(2).map(move |w| {
            let share = f64::from(w[0].distance(w[1])) / r.length;
            ((w[0] + w[1]) * 0.5, r.mass * share, (w[1] - w[0]).abs())
        })).collect()
    }
}
