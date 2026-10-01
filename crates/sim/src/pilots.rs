//! NPC pilots (re-architecture R6): programs that fly crafts the way any
//! client does. A pilot reads the newest snapshot of the world (its ship as
//! its sensors report it, the charts, traffic control's board) and posts its
//! ship's commands, due `COMMAND_DELAY` ticks after the snapshot it read.
//!
//! Pilots run apart from the world, on a pool of their own threads, or in
//! lockstep with it (tests, and `UNIVERSE_LOCKSTEP=1`). Either way the world
//! never waits on them, and they change it only by posting:
//! - **On time** (it came by its due tick), a posting takes effect at exactly
//!   that tick, so the world runs the same as in lockstep.
//! - **Late**, it takes effect at once, counted (`Pool::late`).
//! - **Stale** (more than `LATE_HORIZON` past due), it's dropped, counted
//!   (`Pool::dropped`).
//!
//! A pilot that falls silent leaves its ship holding its controls, until the
//! dead-man rule (a core rule, after `DEAD_MAN` s) cuts its engines and
//! makes its weapons safe.
//!
//! Each pilot says when it next needs to think (`Pilot::next_think`): every
//! tick while flying by hand or by program, every `COAST_THINK` coasting,
//! and when parked, at the end of its stop (at least every `THINK_AT_LEAST`,
//! so its binding stays alive). What happens to its ship, and the orders of
//! its operator (dispatch, the market, a run for the guns), reach it as
//! messages, and wake it.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc, Condvar, Mutex, MutexGuard};

use glam::DVec3;
use universe_avionics::hunter::{may_defend, wants_sightings, Sighting, DEFEND_RANGE};
use universe_avionics::route::Stop;
use universe_avionics::{Avionics, Bus, Event, NavTarget};
use universe_protocol::PadGrant;
use universe_world::radar::RADAR_RANGE;
use universe_world::{Controls, Ship, ShipCommands, ShipEvent, ShipState, StarSystem};

use crate::vessel::Request;
pub use crate::contract::{Gun, Guns, PilotView, Posting, Status, COMMAND_DELAY, DEAD_MAN, LATE_HORIZON};
pub(crate) use crate::contract::{turret_motions, Msg};
use crate::traffic::Snap;

/// With nothing new to say, a pilot still posts this often (s): its binding
/// stays alive (see `DEAD_MAN`).
const KEEP_ALIVE: f64 = 1.0;
/// The longest a pilot goes without thinking (s).
const THINK_AT_LEAST: f64 = 5.0;
/// Coasting with nothing to do, a pilot thinks this often (s).
const COAST_THINK: f64 = 0.5;

impl Status {
    pub fn of(a: &Avionics) -> Self {
        let r = &a.route;
        Status {
            nav_target: a.nav_target,
            clearance: a.clearance,
            corridor_denied: a.corridor_denied,
            route_active: r.active,
            route_next: r.next,
            route_len: r.stops.len(),
            next_stop: r.stops.get(r.next).copied(),
            dwelling: r.dwell_until.is_some(),
            departing: r.departing,
        }
    }
}

/// A pilot: its programs, what's waiting for it, and when it'll next think.
pub struct Pilot {
    pub avionics: Avionics,
    /// Fault injection: it thinks, but posts nothing (a client gone quiet).
    pub silent: bool,
    pub(crate) feed: Vec<ShipEvent>,
    /// The tick it next wants to think at.
    next_think: u64,
    /// Its commands not yet due, and when they will be: it sees its ship as
    /// they'll leave it (as a ship's controls show what was set, not yet
    /// what the devices do), so it doesn't order again what's on its way.
    pub(crate) pending: Vec<(u64, ShipCommands)>,
    /// What it last posted of its turn and status, and when it last posted
    /// (it posts only what's new, and a keep-alive).
    last_turn: Option<Option<Controls>>,
    last_status: Option<Status>,
    last_posted: f64,
    last_sleep: u64,
    /// Its operator's business (see `operator`): a trader or not, what it
    /// paid for what it carries, its route's seed, stops made so far, and
    /// the market service's answer, waiting.
    pub trader: bool,
    /// A miner, and the rock it's working (see `miner`).
    pub miner: bool,
    pub(crate) dig: crate::miner::Dig,
    pub(crate) paid: std::collections::BTreeMap<usize, f64>,
    pub(crate) route_seed: u64,
    pub(crate) stops_made: u64,
    market: Option<crate::contract::MarketAnswer>,
}

impl Pilot {
    pub fn new(avionics: Avionics) -> Self {
        Pilot { avionics, silent: false, feed: Vec::new(), next_think: 0, pending: Vec::new(), last_turn: None, last_status: None, last_posted: f64::NEG_INFINITY, last_sleep: 0, trader: false, miner: false, dig: Default::default(), paid: Default::default(), route_seed: 0, stops_made: 0, market: None }
    }
}

/// A pilot's bus while it thinks: its sensors read the view, its commands
/// and requests are kept for its posting.
pub struct PoolLink<'a> {
    view: &'a PilotView,
    sys: Arc<StarSystem>,
    /// Its ship as it sees it (its own pending settings showing).
    ship: Ship,
    system: usize,
    id: usize,
    devices: Vec<ShipCommands>,
    requests: Vec<Request>,
    pending: &'a mut Vec<(u64, ShipCommands)>,
}

impl PoolLink<'_> {
    fn rails(&self) -> Vec<DVec3> {
        match self.view.rails.get(&self.system) {
            Some(r) => (**r).clone(),
            None => {
                let mut out = Vec::new();
                self.sys.positions(self.view.time, &mut out);
                out
            }
        }
    }
}

impl Bus for PoolLink<'_> {
    fn ship(&self) -> &Ship {
        &self.ship
    }

    fn system(&self) -> usize {
        self.system
    }

    fn star_system(&mut self) -> Arc<StarSystem> {
        self.sys.clone()
    }

    fn time(&self) -> f64 {
        self.view.time
    }

    fn gate_links(&self) -> &[(usize, usize)] {
        &self.view.charts.gate_links
    }

    fn id(&self) -> usize {
        self.id
    }

    fn request_pad(&mut self, port: usize) -> PadGrant {
        self.requests.push(Request::Pad { system: self.system, port, ship: self.id, now: self.view.time });
        self.view.board.peek_pad(self.system, port, self.id)
    }

    fn request_corridor(&mut self, body: usize) -> Option<usize> {
        self.requests.push(Request::Corridor { system: self.system, body, ship: self.id, now: self.view.time });
        self.view.board.peek_corridor(self.system, body, self.id)
    }

    fn turrets(&mut self) -> Vec<(DVec3, DVec3, f64)> {
        match self.view.turrets.get(&self.system) {
            Some(t) => t.iter().map(|g| (g.at, g.velocity, g.reach)).collect(),
            None => turret_motions(&self.view.charts, self.system, &self.sys, self.view.time, &self.rails()).iter().map(|g| (g.at, g.velocity, g.reach)).collect(),
        }
    }

    fn positions(&mut self) -> (Arc<StarSystem>, Vec<DVec3>) {
        (self.sys.clone(), self.rails())
    }

    fn actuate(&mut self, c: &ShipCommands) {
        self.devices.push(*c);
        self.pending.push((self.view.tick + COMMAND_DELAY, *c));
        expect(&mut self.ship, c);
    }

    fn feed(&mut self) -> Vec<ShipEvent> {
        // (Its commands take effect later: what they do comes in its feed then.)
        Vec::new()
    }

    fn request_clearance(&mut self, target: Option<NavTarget>) -> Result<NavTarget, String> {
        let positions = self.rails();
        let target = target.or_else(|| universe_services::atc::nearest_station(&self.sys, self.ship.position, &positions));
        universe_services::atc::request(&self.sys, &self.ship, target, self.view.time, &positions)
    }

    fn clearance_holds(&mut self, target: NavTarget) -> bool {
        let positions = self.rails();
        !universe_services::atc::lapsed(&self.sys, &self.ship, target, self.view.time, &positions)
    }
}

/// What the pilot expects of its ship once command `c` is carried out:
/// engine and thrusters set, the hyperdrive and master arm switched.
fn expect(ship: &mut Ship, c: &ShipCommands) {
    ship.throttle = c.throttle;
    ship.rcs = c.rcs;
    if let Some(h) = c.hyperdrive
        && h.engage != ship.hyperdrive
        && (h.start || !h.engage)
    {
        // (Switching, the drive sets the engine back to zero.)
        ship.hyperdrive = h.engage;
        ship.throttle = 0.0;
    }
    if let Some(on) = c.arm {
        ship.armed = on;
    }
}

/// Its ship as pilot `pending` sees it at `tick`: as the view reports it,
/// with its commands not yet due carried out (the rest forgotten).
fn seen(ship: &Ship, pending: &mut Vec<(u64, ShipCommands)>, tick: u64) -> Ship {
    pending.retain(|(due, _)| *due > tick);
    let mut seen = ship.clone();
    for (_, c) in pending.iter() {
        expect(&mut seen, c);
    }
    seen
}

/// The guns of `system` as a pilot reads them from `view`.
fn guns_of(view: &PilotView, system: usize, sys: &StarSystem) -> Guns {
    view.turrets.get(&system).cloned().unwrap_or_else(|| {
        let mut rails = Vec::new();
        sys.positions(view.time, &mut rails);
        Arc::new(turret_motions(&view.charts, system, sys, view.time, &rails))
    })
}

/// Fight or flight, the flight: run for the nearest defended place (not a
/// gate), giving up any clearance (traffic control hears of it).
fn flee(a: &mut Avionics, ship: &Ship, system: usize, guns: &[Gun], events: &mut Vec<Event>) {
    if !ship.is_flying() || ship.hyperdrive {
        return;
    }
    let Some(haven) = guns.iter().filter(|g| !matches!(g.guards, universe_world::Facility::Gate(_))).min_by(|x, y| x.at.distance(ship.position).total_cmp(&y.at.distance(ship.position))) else { return };
    let stop = Stop { system, target: haven.guards };
    let r = &mut a.route;
    if r.stops.get(r.next) == Some(&stop) && r.active {
        return;
    }
    r.stops.insert(r.next.min(r.stops.len()), stop);
    r.active = true;
    r.dwell_until = None;
    r.departing = false;
    if a.clearance.take().is_some() {
        events.push(Event::Traffic(universe_world::TrafficEvent::ClearanceCancelled));
    }
}

/// What pilot `i` makes of the ships around it (radar, and the pirates'
/// transponders): shelter is real (docked or landed, or under a turret's guns).
fn sightings(view: &PilotView, me: usize, system: usize, pos: DVec3, guns: &[Gun], crew: &Crew) -> Vec<Sighting> {
    let sheltered = |s: &Snap| s.landed || guns.iter().any(|g| g.at.distance(s.position) < g.reach + universe_avionics::hunter::SHELTER_MARGIN);
    view.snaps
        .iter()
        .enumerate()
        .filter(|&(id, s)| id != me && s.system == system && !s.transit && s.position.distance(pos) < RADAR_RANGE)
        .map(|(id, s)| Sighting {
            id,
            position: s.position,
            velocity: s.velocity,
            pirate: crew.pirates.get(id.wrapping_sub(1)).copied().unwrap_or(false),
            docked: sheltered(s),
            destroyed: s.destroyed,
            hyperdrive: s.hyperdrive,
            landed: s.landed,
            aggressed: s.aggressed,
            hull: s.hull,
        })
        .collect()
}

/// The pilot of ship `id` thinks on `view`, if it's time to or something's
/// waiting for it: what it posts. With a human at the stick (`human`), it
/// thinks every time, flies by the stick (unless a program has it), and
/// hunts no one.
pub(crate) fn think(pilot: &mut Pilot, id: usize, view: &PilotView, human: Option<Controls>, crew: &Crew, tally: &Tally) -> Option<Posting> {
    let (system, ref ship) = *view.ships.get(&id)?;
    if human.is_none() && view.tick < pilot.next_think && pilot.feed.is_empty() && pilot.market.is_none() {
        return None;
    }
    // Its operator's business: a route done (parked), the next; the market's
    // answer in, the trades.
    let mut business = Vec::new();
    if human.is_none() {
        if !pilot.avionics.route.active && matches!(ship.state, ShipState::Landed { .. }) {
            let seed = crate::rng::mix(pilot.route_seed, pilot.stops_made);
            if !(pilot.miner && crate::miner::new_route(&mut pilot.avionics, &mut pilot.dig, &view.charts, system, seed)) {
                crate::operator::new_route(pilot, &view.charts, system);
            }
        }
        if let Some(answer) = pilot.market.take() {
            if pilot.miner {
                crate::miner::sell(&answer, &mut business);
            } else {
                crate::operator::trade(pilot, &view.charts, &answer, &mut business);
            }
        }
    }
    let a = &mut pilot.avionics;
    let was_hunting = a.hunting.is_some();
    let mut events = Vec::new();
    // What happened to the ship since it last thought.
    let feed = std::mem::take(&mut pilot.feed);
    let hit = feed.iter().any(|e| matches!(e, ShipEvent::Hit { .. }));
    let mined_feed = if pilot.miner { feed.clone() } else { Vec::new() };
    a.record(feed, &mut events);
    // Its ship, with its own commands on their way.
    let seen = seen(ship, &mut pilot.pending, view.tick);
    let sys = view.charts.system(system);
    let mut link = PoolLink { view, sys, ship: seen, system, id, devices: Vec::new(), requests: Vec::new(), pending: &mut pilot.pending };
    // The last step's outcome (an arrival, a lapsed clearance), then this one.
    a.conclude(&mut link, &mut events);
    let pos = link.ship.position;
    let threat = human.is_none() && may_defend(a, &link.ship) && view.aggressors.iter().any(|&(s, p)| s == system && p.distance(pos) < DEFEND_RANGE);
    let guns = guns_of(view, system, &link.sys);
    let sightings = if threat || (human.is_none() && wants_sightings(a, &link.ship, view.time)) { sightings(view, id, system, pos, &guns, crew) } else { Vec::new() };
    // Fired on (and not a hunter itself, nor standing to fight with hull to
    // spare): run for the guns. (A human decides that for themselves.)
    use universe_avionics::hunter::FLEE_HULL;
    let fighting = a.hunting.is_some_and(|h| h.lawful) && link.ship.hull >= FLEE_HULL;
    if human.is_none() && hit && !a.pirate && !fighting {
        flee(a, &link.ship, system, &guns, &mut events);
    }
    let mark = match a.following.map(|f| f.anchor) {
        Some(universe_avionics::follow::Anchor::Ship(id)) => crate::follow::mark_in(&view.snaps, system, pos, id),
        _ => None,
    };
    if human.is_none() && pilot.miner && a.hunting.is_none() {
        crate::miner::work(a, &mut pilot.dig, &view.charts, pilot.route_seed, &mined_feed, &mut link, &mut events);
    }
    let (stick, hunt_end) = if human.is_none() { a.hunt(&mut link, &sightings, &mut events) } else { (None, None) };
    // A defender that broke off hurt runs for the guns.
    if hunt_end.is_some() && !a.pirate && link.ship.hull < FLEE_HULL {
        flee(a, &link.ship, system, &guns, &mut events);
    }
    let stick = stick.or_else(|| a.follow_step(&mut link, mark, &mut events)).or(human);
    let program = a.prepare(&mut link, view.dt, &mut events);

    let flown = a.flies(&link.ship);
    let turn = if flown { program } else { Some(stick.unwrap_or_default()) };
    // When it needs to think again.
    let s = &link.ship;
    let r = &a.route;
    let busy = flown
        || turn.is_some_and(|c| c != Controls::default())
        || s.throttle > 0.0
        || s.rcs != DVec3::ZERO
        || a.hunting.is_some()
        || a.following.is_some()
        || a.clearance.is_some()
        || a.corridor_denied
        || r.departing
        // Parked with its time up: trying to leave (asking for its corridor or pad).
        || (matches!(s.state, ShipState::Landed { .. }) && r.active && r.dwell_until.is_none_or(|t| t <= view.time));
    let wait = match s.state {
        _ if busy => 0.0,
        ShipState::Landed { .. } => r.dwell_until.map_or(THINK_AT_LEAST, |t| t - view.time).clamp(0.0, THINK_AT_LEAST),
        ShipState::Flying => COAST_THINK,
        _ => THINK_AT_LEAST,
    };
    pilot.next_think = view.tick + ((wait / view.dt).ceil() as u64).max(1);
    let PoolLink { devices, mut requests, .. } = link;
    requests.extend(business);
    let stopped = events.iter().any(|e| matches!(e, Event::RouteStop { .. }));
    if stopped {
        pilot.stops_made += 1;
        // Every pilot fills its tank at a stop with a market.
        if let Some(market) = universe_world::traffic::docked_at(&view.charts.system(system), ship) {
            requests.push(Request::Refuel { market });
            // And mends its hull, at a station.
            if ship.hull < 1.0 && matches!(market, universe_world::Facility::Station(_)) {
                requests.push(Request::Repair { market });
            }
        }
        if (pilot.trader || pilot.miner)
            && let Some(market) = universe_world::traffic::docked_at(&view.charts.system(system), ship)
        {
            requests.push(Request::Quotes { system, market });
        }
    }
    // Its ship's events went to the services when they happened: the rest.
    events.retain(|e| !matches!(e, Event::Ship(_)));
    // Only what's new (the ship holds its turn, the world its status), and a keep-alive.
    let status = Status::of(&pilot.avionics);
    // (A hunt or a posse begun: the operator's own tally.)
    if !was_hunting && let Some(h) = pilot.avionics.hunting {
        let n = if h.lawful { &tally.defences } else { &tally.hunts };
        n.fetch_add(1, Ordering::Relaxed);
    }
    let new_turn = pilot.last_turn != Some(turn);
    let new_status = pilot.last_status.as_ref() != Some(&status);
    // Going to sleep (past the next tick), it says so: the world leaves its
    // ship out of the views till then.
    let sleep = (pilot.next_think > view.tick + 1 && pilot.next_think != pilot.last_sleep).then_some(pilot.next_think);
    if devices.is_empty() && requests.is_empty() && events.is_empty() && !new_turn && !new_status && sleep.is_none() && view.time - pilot.last_posted < KEEP_ALIVE {
        return None;
    }
    pilot.last_turn = Some(turn);
    pilot.last_status = Some(status.clone());
    pilot.last_posted = view.time;
    if let Some(t) = sleep {
        pilot.last_sleep = t;
    }
    Some(Posting { id, thought: view.tick, seen: view.time, devices, turn: new_turn.then_some(turn), requests, events, status, gun: None, sleep_until: sleep })
}

/// Every pilot due thinks on `view`, side by side: their postings, in craft order.
/// The defence service's gunners (clients, in the pool): in every system
/// with someone fair game in it, each turret's gunner orders its gun (see
/// `gunner`); where no one is any more, they stand down.
fn aim_guns(gunners: &mut HashMap<usize, universe_avionics::gunner::Gunner>, view: &PilotView) -> Vec<Posting> {
    use universe_avionics::gunner::Quarry;
    use universe_protocol::TurretCommand;
    let mut systems: Vec<usize> = view.snaps.iter().filter(|s| s.aggressed && (s.flying || s.landed)).map(|s| s.system).collect();
    systems.sort_unstable();
    systems.dedup();
    let latency = view.dt * (COMMAND_DELAY + 1) as f64;
    let order = |id: usize, c: TurretCommand| Posting { id, thought: view.tick, seen: view.time, devices: Vec::new(), turn: None, requests: Vec::new(), events: Vec::new(), status: Status::default(), gun: Some(c), sleep_until: None };
    let mut out = Vec::new();
    for &system in &systems {
        let sys = view.charts.system(system);
        let Some(positions) = view.rails.get(&system) else { continue };
        let guns = guns_of(view, system, &sys);
        let quarry: Vec<Quarry> = view.snaps.iter().enumerate().filter(|(_, s)| s.system == system && s.aggressed && (s.flying || s.landed)).map(|(id, s)| Quarry { id, position: s.position, velocity: s.velocity }).collect();
        for g in guns.iter() {
            let gun = g.aim.unwrap_or_else(|| (quarry.first().map_or(g.at, |q| q.position) - g.at).normalize_or(DVec3::Y));
            let clear = |p: DVec3| {
                let d = p - g.at;
                universe_physics::ray(&sys.bodies, positions, g.at + d.normalize() * 10.0, d.normalize(), d.length() - 30.0, view.time, &[]).is_none()
            };
            let gravity = |p: DVec3| sys.gravity(p, positions);
            let mut c = gunners.entry(g.id).or_default().orders(view.time, g.at, g.velocity, gun, g.reach, &quarry, clear, gravity, latency);
            // Missiles at the nearest fair game within the launcher's reach,
            // on a clear line: the guns reach a few kilometres, these a hundred.
            c.launch = quarry
                .iter()
                .filter(|q| q.position.distance(g.at) < universe_world::missiles::MISSILE_RANGE && clear(q.position))
                .min_by(|a, b| a.position.distance(g.at).total_cmp(&b.position.distance(g.at)))
                .map(|q| q.id);
            out.push(order(g.id, c));
        }
    }
    let idle: Vec<usize> = gunners.keys().copied().filter(|id| universe_world::turrets::turret_of(*id).is_some_and(|(s, _)| !systems.contains(&s))).collect();
    for id in idle {
        gunners.remove(&id);
        out.push(order(id, TurretCommand { aim: None, fire: false, launch: None }));
    }
    out.sort_by_key(|p| p.id);
    out
}

fn think_all(pilots: &mut [Pilot], view: &PilotView, tally: &Tally) -> Vec<Posting> {
    use rayon::prelude::*;
    // (The pirates' own network: who flies with them, as they know it.)
    let crew = Crew { pirates: pilots.iter().map(|p| p.avionics.pirate).collect() };
    pilots.par_iter_mut().enumerate().filter_map(|(i, p)| think(p, crate::combat::craft_id(i), view, None, &crew, tally).filter(|_| !p.silent)).collect()
}

/// What the operator's pilots know of each other: who flies with the
/// pirates (they see each other's transponders), by craft. (Not the world's.)
#[derive(Default)]
pub struct Crew {
    pub pirates: Vec<bool>,
}

/// The operator's own tally: hunts begun, and posses formed against aggressors.
#[derive(Default, Debug)]
pub struct Tally {
    pub hunts: std::sync::atomic::AtomicU64,
    pub defences: std::sync::atomic::AtomicU64,
}

/// Messages to pilots, delivered before they next think.
fn deliver(pilots: &mut [Pilot], mail: &mut Vec<(usize, Msg)>) {
    for (i, m) in mail.drain(..) {
        let Some(p) = pilots.get_mut(i) else { continue };
        match m {
            Msg::Feed(events) => p.feed.extend(events),
            Msg::Market(a) => p.market = Some(a),
        }
    }
}

/// The NPC pilots, and where they think: in lockstep with the world, or
/// apart from it on threads of their own.
#[derive(Default)]
pub struct Pool {
    pilots: Arc<Mutex<Vec<Pilot>>>,
    mail: Arc<Mutex<Vec<(usize, Msg)>>>,
    worker: Option<Worker>,
    /// Fault injection: apart, the pool takes this much longer over each view (µs).
    slow: Arc<std::sync::atomic::AtomicU64>,
    /// The operator's own tally (see `Tally`).
    pub tally: Arc<Tally>,
    /// The defence service's gunners (see `aim_guns`).
    gunners: Arc<Mutex<HashMap<usize, universe_avionics::gunner::Gunner>>>,
}

/// The pool's own thread: thinks on the newest view whenever there's one,
/// skipping any it couldn't get to.
struct Worker {
    slot: Arc<(Mutex<Option<Arc<PilotView>>>, Condvar)>,
    /// The tick of the last view thought on (its postings sent).
    done: Arc<std::sync::atomic::AtomicU64>,
    postings: mpsc::Receiver<Vec<Posting>>,
    stop: Arc<AtomicBool>,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl Pool {
    fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
        m.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// The pilots (waits while they're thinking, apart).
    pub fn pilots(&self) -> MutexGuard<'_, Vec<Pilot>> {
        Self::lock(&self.pilots)
    }

    pub(crate) fn add(&self, pilot: Pilot) {
        self.pilots().push(pilot);
    }

    pub(crate) fn send(&self, craft: usize, msg: Msg) {
        Self::lock(&self.mail).push((craft, msg));
    }

    pub fn apart(&self) -> bool {
        self.worker.is_some()
    }

    /// Fault injection: apart, the pool takes `extra` longer over each view.
    pub fn slow_down(&self, extra: std::time::Duration) {
        self.slow.store(extra.as_micros() as u64, Ordering::Relaxed);
    }

    /// Think apart from the world from now on, on `threads` threads.
    pub fn run_apart(&mut self, threads: usize) {
        if self.worker.is_some() {
            return;
        }
        let slot: Arc<(Mutex<Option<Arc<PilotView>>>, Condvar)> = Default::default();
        let stop = Arc::new(AtomicBool::new(false));
        let (tx, rx) = mpsc::channel();
        let done = Arc::new(std::sync::atomic::AtomicU64::new(0));
        let (pilots, mail, s, st, slow, d, tally, gunners) = (self.pilots.clone(), self.mail.clone(), slot.clone(), stop.clone(), self.slow.clone(), done.clone(), self.tally.clone(), self.gunners.clone());
        let thread = std::thread::Builder::new()
            .name("pilots".into())
            .spawn(move || {
                let workers = rayon::ThreadPoolBuilder::new().num_threads(threads.max(1)).thread_name(|i| format!("pilot {i}")).build().expect("pilot threads");
                loop {
                    let view = {
                        let (lock, ready) = &*s;
                        let mut slot = Self::lock(lock);
                        while slot.is_none() && !st.load(Ordering::Relaxed) {
                            slot = ready.wait(slot).unwrap_or_else(|e| e.into_inner());
                        }
                        if st.load(Ordering::Relaxed) {
                            return;
                        }
                        slot.take().expect("a view")
                    };
                    let _p = universe_prof::scope("pilots/think");
                    let mut guard = Self::lock(&pilots);
                    deliver(&mut guard, &mut Self::lock(&mail));
                    let list: &mut [Pilot] = &mut guard;
                    let mut postings = workers.install(|| think_all(list, &view, &tally));
                    postings.extend(aim_guns(&mut Self::lock(&gunners), &view));
                    drop(guard);
                    let extra = slow.load(Ordering::Relaxed);
                    if extra > 0 {
                        std::thread::sleep(std::time::Duration::from_micros(extra));
                    }
                    if tx.send(postings).is_err() {
                        return;
                    }
                    d.store(view.tick, Ordering::Release);
                }
            })
            .expect("pilot thread");
        self.worker = Some(Worker { slot, done, postings: rx, stop, thread: Some(thread) });
    }

    /// The world has a new view: in lockstep, the pilots think on it now
    /// (their postings returned); apart, it's theirs to take when they can.
    pub(crate) fn view(&mut self, view: Arc<PilotView>) -> Vec<Posting> {
        match &self.worker {
            None => {
                let mut pilots = Self::lock(&self.pilots);
                deliver(&mut pilots, &mut Self::lock(&self.mail));
                let mut postings = think_all(&mut pilots, &view, &self.tally);
                postings.extend(aim_guns(&mut Self::lock(&self.gunners), &view));
                postings
            }
            Some(w) => {
                let (lock, ready) = &*w.slot;
                *Self::lock(lock) = Some(view);
                ready.notify_one();
                Vec::new()
            }
        }
    }

    /// Test harness: wait until the pool (apart) has thought on the view of
    /// `tick`, as an on-time pool would have.
    pub fn wait_for(&self, tick: u64) {
        if let Some(w) = &self.worker {
            while w.done.load(Ordering::Acquire) < tick {
                std::thread::yield_now();
            }
        }
    }

    /// Postings that have come in from apart (none in lockstep).
    pub(crate) fn collect(&mut self) -> Vec<Posting> {
        match &self.worker {
            None => Vec::new(),
            Some(w) => w.postings.try_iter().flatten().collect(),
        }
    }

    /// Run `f` on pilot `i` at once (dev tools and tests), against `view`:
    /// what it posts.
    pub(crate) fn run<R>(&self, i: usize, view: &PilotView, f: impl FnOnce(&mut Avionics, &mut PoolLink, &mut Vec<Event>) -> R) -> (R, Posting) {
        let mut pilots = self.pilots();
        run(&mut pilots[i], crate::combat::craft_id(i), view, f)
    }
}

/// Run `f` on the pilot of ship `id` at once (dev tools, tests, and the
/// player's requests), against `view`: what it posts.
pub(crate) fn run<R>(pilot: &mut Pilot, id: usize, view: &PilotView, f: impl FnOnce(&mut Avionics, &mut PoolLink, &mut Vec<Event>) -> R) -> (R, Posting) {
    {
        let (system, ref ship) = view.ships[&id];
        let sys = view.charts.system(system);
        let seen = seen(ship, &mut pilot.pending, view.tick);
        let mut link = PoolLink { view, sys, ship: seen, system, id, devices: Vec::new(), requests: Vec::new(), pending: &mut pilot.pending };
        let mut events = Vec::new();
        let r = f(&mut pilot.avionics, &mut link, &mut events);
        let PoolLink { devices, requests, .. } = link;
        events.retain(|e| !matches!(e, Event::Ship(_)));
        pilot.next_think = view.tick;
        let posting = Posting { id, thought: view.tick, seen: view.time, devices, turn: None, requests, events, status: Status::of(&pilot.avionics), gun: None, sleep_until: None };
        (r, posting)
    }
}

impl Drop for Pool {
    fn drop(&mut self) {
        if let Some(mut w) = self.worker.take() {
            w.stop.store(true, Ordering::Relaxed);
            w.slot.1.notify_all();
            if let Some(t) = w.thread.take() {
                let _ = t.join();
            }
        }
    }
}

/// The NPC pilots, in step with the world (tests, dev tools): reached
/// through the world's link to them.
impl crate::universe::Universe {
    /// Craft `i`'s pilot does `f` now (dev tools and tests, as a pilot at
    /// the controls would): what it posts goes in at once.
    pub(crate) fn craft_run<R>(&mut self, i: usize, f: impl FnOnce(&mut Avionics, &mut crate::pilots::PoolLink, &mut Vec<Event>) -> R) -> R {
        self.crafts[i].asleep_until = 0;
        let view = self.pilot_view(crate::universe::TICK);
        let (r, posting) = self.pool().run(i, &view, f);
        self.post_now(vec![posting]);
        r
    }

    /// The crafts' pilots (waits while they're thinking, apart).
    pub fn pilots(&self) -> std::sync::MutexGuard<'_, Vec<crate::pilots::Pilot>> {
        self.pool().pilots()
    }

    /// From now on the pilots think apart from the world, on `threads` threads.
    pub fn run_pilots_apart(&mut self, threads: usize) {
        self.npcs.run_apart(threads);
    }

    /// Lock craft `i`'s nav target.
    pub fn craft_set_nav_target(&mut self, i: usize, target: Option<NavTarget>) {
        self.craft_run(i, |a, link, events| a.set_nav_target(link, target, events));
    }

    /// Craft `i` asks traffic control for clearance (to its nav target, else the nearest station).
    pub fn craft_request_clearance(&mut self, i: usize) -> bool {
        self.craft_run(i, |a, link, events| a.request_clearance(link, events))
    }

    /// Craft `i` engages (or releases) its autopilot.
    pub fn craft_toggle_autopilot(&mut self, i: usize) {
        self.craft_run(i, |a, link, events| a.toggle_autopilot(link, events));
    }

    /// Craft `i` follows `anchor`.
    pub fn craft_follow(&mut self, i: usize, anchor: universe_avionics::follow::Anchor, manoeuvre: universe_avionics::follow::Manoeuvre) {
        self.craft_run(i, |a, link, events| a.follow(link, anchor, manoeuvre, events));
    }
}

impl crate::contract::Pilots for Pool {
    fn view(&mut self, view: Arc<PilotView>) -> Vec<Posting> {
        Pool::view(self, view)
    }
    fn collect(&mut self) -> Vec<Posting> {
        Pool::collect(self)
    }
    fn tell(&mut self, craft: usize, msg: Msg) {
        self.send(craft, msg);
    }
    fn run_apart(&mut self, threads: usize) {
        Pool::run_apart(self, threads);
    }
    fn apart(&self) -> bool {
        Pool::apart(self)
    }
    fn tally(&self) -> (u64, u64) {
        (self.tally.hunts.load(Ordering::Relaxed), self.tally.defences.load(Ordering::Relaxed))
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

impl crate::universe::Universe {
    /// The NPC pilots' pool (when they're this world's, in this process).
    pub fn pool(&self) -> &Pool {
        self.npcs.as_any().downcast_ref::<Pool>().expect("no NPC pilots here")
    }

    pub fn pool_mut(&mut self) -> &mut Pool {
        if self.npcs.as_any().downcast_ref::<Pool>().is_none() {
            self.npcs = Box::new(Pool::default());
        }
        self.npcs.as_any_mut().downcast_mut::<Pool>().expect("a pool")
    }
}
