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
use universe_avionics::{Avionics, Bus, Clearance, Event, NavTarget};
use universe_protocol::PadGrant;
use universe_services::Board;
use universe_world::charts::Charts;
use universe_world::radar::RADAR_RANGE;
use universe_world::{Controls, Ship, ShipCommands, ShipEvent, ShipState, StarSystem};

use crate::traffic::Snap;
use crate::vessel::Request;

/// Ticks from the snapshot a pilot read to its commands taking effect (k).
pub const COMMAND_DELAY: u64 = 2;
/// A posting this many ticks past due is dropped: too stale to act on.
pub const LATE_HORIZON: u64 = 30;
/// After this long with no posting from its pilot, a ship's engines are cut
/// and its weapons made safe (s).
pub const DEAD_MAN: f64 = 30.0;
/// With nothing new to say, a pilot still posts this often (s): its binding
/// stays alive (see `DEAD_MAN`).
const KEEP_ALIVE: f64 = 1.0;
/// The longest a pilot goes without thinking (s).
const THINK_AT_LEAST: f64 = 5.0;
/// Coasting with nothing to do, a pilot thinks this often (s).
const COAST_THINK: f64 = 0.5;

/// What a pilot shows of itself: its transponder and flight plan, and what
/// its operator and the services know of it (published with each posting;
/// the world reads this, never the pilot itself).
#[derive(Clone, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Status {
    pub nav_target: Option<NavTarget>,
    pub clearance: Option<Clearance>,
    pub corridor_denied: bool,
    pub route_active: bool,
    pub route_next: usize,
    pub route_len: usize,
    /// The stop it's bound for.
    pub next_stop: Option<Stop>,
    /// Parked at a stop until its time is up.
    pub dwelling: bool,
    pub departing: bool,
}

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

/// A message for a pilot.
#[derive(Clone, Debug)]
pub(crate) enum Msg {
    /// What happened to its ship (its sensors and the devices report it).
    Feed(Vec<ShipEvent>),
    /// The market service's answer to its request for quotes.
    Market(crate::operator::MarketAnswer),
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
    pub(crate) paid: std::collections::BTreeMap<usize, f64>,
    pub(crate) route_seed: u64,
    pub(crate) stops_made: u64,
    market: Option<crate::operator::MarketAnswer>,
}

impl Pilot {
    pub fn new(avionics: Avionics) -> Self {
        Pilot { avionics, silent: false, feed: Vec::new(), next_think: 0, pending: Vec::new(), last_turn: None, last_status: None, last_posted: f64::NEG_INFINITY, last_sleep: 0, trader: false, paid: Default::default(), route_seed: 0, stops_made: 0, market: None }
    }
}

/// A defence turret, as the charts have it: where it is, how it moves, how
/// far it reaches, and what it guards.
#[derive(Clone, Copy, Debug)]
pub struct Gun {
    pub at: DVec3,
    pub velocity: DVec3,
    pub reach: f64,
    pub guards: universe_world::Facility,
}
pub type Guns = Arc<Vec<Gun>>;

/// What pilots read: the world as it stood at the end of a tick.
pub struct PilotView {
    pub tick: u64,
    pub time: f64,
    /// Game seconds a tick spans.
    pub dt: f64,
    pub charts: Arc<Charts>,
    /// The ships of the pilots awake this tick (and ours), with the system
    /// each is in, by combat id (ours 0, craft i: i + 1). The rest are in
    /// `snaps`: what anyone sees of anyone.
    pub ships: HashMap<usize, (usize, Ship), universe_physics::pairs::CellHash>,
    /// Every ship as others see it (by combat id), and who's aggressed and flying.
    pub(crate) snaps: Arc<Vec<Snap>>,
    pub aggressors: Vec<(usize, DVec3)>,
    pub board: Board,
    /// Bodies' positions, and the defence turrets' (where and how they move),
    /// per system with crafts in it.
    pub rails: HashMap<usize, Arc<Vec<DVec3>>>,
    pub turrets: HashMap<usize, Guns>,
}

/// What a pilot posts after thinking.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct Posting {
    /// Whose: the ship's combat id (the player's 0, craft i: i + 1).
    pub id: usize,
    /// The tick of the snapshot it read (due `COMMAND_DELAY` after), and its world time.
    pub thought: u64,
    pub seen: f64,
    /// For its devices, in order, and its turn at the due tick (None: it
    /// says nothing of the turn; Some(None): none commanded).
    pub devices: Vec<ShipCommands>,
    pub turn: Option<Option<Controls>>,
    /// To traffic control.
    pub(crate) requests: Vec<Request>,
    /// What its programs report (route stops, traffic): for the services.
    pub events: Vec<Event>,
    pub status: Status,
    /// Going to sleep: it needn't be in a view before this tick (unless a
    /// message wakes it). None: it says nothing of it.
    pub sleep_until: Option<u64>,
}

impl Posting {
    pub fn due(&self) -> u64 {
        self.thought + COMMAND_DELAY
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

/// Where system `system`'s defence turrets are at `t`, and how they move.
pub(crate) fn turret_motions(charts: &Charts, system: usize, sys: &StarSystem, t: f64, positions: &[DVec3]) -> Vec<Gun> {
    universe_world::turrets::turrets(charts.seed, system, sys)
        .iter()
        .map(|tu| {
            let (at, velocity) = tu.motion(sys, t, positions);
            Gun { at, velocity, reach: tu.range(), guards: tu.facility }
        })
        .collect()
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
            crate::operator::new_route(pilot, &view.charts, system);
        }
        if let Some(answer) = pilot.market.take() {
            crate::operator::trade(pilot, &view.charts, &answer, &mut business);
        }
    }
    let a = &mut pilot.avionics;
    let was_hunting = a.hunting.is_some();
    let mut events = Vec::new();
    // What happened to the ship since it last thought.
    let feed = std::mem::take(&mut pilot.feed);
    let hit = feed.iter().any(|e| matches!(e, ShipEvent::Hit { .. }));
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
    let (stick, hunt_end) = if human.is_some() { (None, None) } else { a.hunt(&mut link, &sightings, &mut events) };
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
        if pilot.trader
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
    Some(Posting { id, thought: view.tick, seen: view.time, devices, turn: new_turn.then_some(turn), requests, events, status, sleep_until: sleep })
}

/// Every pilot due thinks on `view`, side by side: their postings, in craft order.
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
    /// Postings that came past due (applied at once), and too stale (dropped).
    pub late: u64,
    pub dropped: u64,
    /// Fault injection: apart, the pool takes this much longer over each view (µs).
    slow: Arc<std::sync::atomic::AtomicU64>,
    /// The operator's own tally (see `Tally`).
    pub tally: Arc<Tally>,
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
        let (pilots, mail, s, st, slow, d, tally) = (self.pilots.clone(), self.mail.clone(), slot.clone(), stop.clone(), self.slow.clone(), done.clone(), self.tally.clone());
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
                    let postings = workers.install(|| think_all(list, &view, &tally));
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
                think_all(&mut pilots, &view, &self.tally)
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
        let posting = Posting { id, thought: view.tick, seen: view.time, devices, turn: None, requests, events, status: Status::of(&pilot.avionics), sleep_until: None };
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
