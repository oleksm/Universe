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
use universe_avionics::hunter::{may_defend, wants_sightings, Hunt, HuntEnd, Sighting, DEFEND_RANGE};
use universe_avionics::route::{Route, Stop};
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
/// The longest a pilot goes without thinking (s).
const THINK_AT_LEAST: f64 = 5.0;
/// Coasting with nothing to do, a pilot thinks this often (s).
const COAST_THINK: f64 = 0.5;

/// What a pilot shows of itself: its transponder and flight plan, and what
/// its operator and the services know of it (published with each posting;
/// the world reads this, never the pilot itself).
#[derive(Clone, Debug, Default)]
pub struct Status {
    /// Flies with the pirates (they see each other's transponders).
    pub pirate: bool,
    pub hunting: Option<Hunt>,
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
            pirate: a.pirate,
            hunting: a.hunting,
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

/// What a pilot's operator tells it (it acts on it when it next thinks).
#[derive(Clone, Debug)]
pub enum Order {
    /// Fly this route (dispatch: a new one when the last is done).
    Route(Route),
    /// After the stop it's bound for, on to this one (a trader's next market).
    Then(Stop),
    /// Run for this haven now, giving up any clearance.
    Flee(Stop),
    /// Lock a nav target, ask for clearance, or switch the autopilot (dev
    /// tools and tests, as a pilot at the controls would).
    NavTarget(Option<NavTarget>),
    RequestClearance,
    ToggleAutopilot,
}

/// A message for a pilot.
#[derive(Clone, Debug)]
pub(crate) enum Msg {
    /// What happened to its ship (its sensors and the devices report it).
    Feed(Vec<ShipEvent>),
    Order(Order),
}

/// A pilot: its programs, what's waiting for it, and when it'll next think.
pub struct Pilot {
    pub avionics: Avionics,
    /// Fault injection: it thinks, but posts nothing (a client gone quiet).
    pub silent: bool,
    pub(crate) feed: Vec<ShipEvent>,
    orders: Vec<Order>,
    /// The tick it next wants to think at.
    next_think: u64,
    /// Its commands not yet due, and when they will be: it sees its ship as
    /// they'll leave it (as a ship's controls show what was set, not yet
    /// what the devices do), so it doesn't order again what's on its way.
    pub(crate) pending: Vec<(u64, ShipCommands)>,
}

impl Pilot {
    pub fn new(avionics: Avionics) -> Self {
        Pilot { avionics, silent: false, feed: Vec::new(), orders: Vec::new(), next_think: 0, pending: Vec::new() }
    }
}

/// What pilots read: the world as it stood at the end of a tick.
pub struct PilotView {
    pub tick: u64,
    pub time: f64,
    /// Game seconds a tick spans.
    pub dt: f64,
    pub charts: Arc<Charts>,
    /// Every ship and the system it's in, by combat id (ours 0, craft i: i + 1).
    pub ships: Vec<(usize, Ship)>,
    /// Every ship as others see it (by combat id), and who's aggressed and flying.
    pub(crate) snaps: Vec<Snap>,
    pub aggressors: Vec<(usize, DVec3)>,
    pub board: Board,
    /// Bodies' positions, and the defence turrets' (where and how they move),
    /// per system with crafts in it.
    pub rails: HashMap<usize, Arc<Vec<DVec3>>>,
    pub turrets: HashMap<usize, Arc<Vec<(DVec3, DVec3)>>>,
}

/// What a pilot posts after thinking.
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
    /// A hunt begun, or ended, this think.
    pub hunt_begun: bool,
    pub hunt_end: Option<HuntEnd>,
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

    fn turrets(&mut self) -> Vec<(DVec3, DVec3)> {
        match self.view.turrets.get(&self.system) {
            Some(t) => (**t).clone(),
            None => turret_motions(&self.view.charts, self.system, &self.sys, self.view.time, &self.rails()),
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
pub(crate) fn turret_motions(charts: &Charts, system: usize, sys: &StarSystem, t: f64, positions: &[DVec3]) -> Vec<(DVec3, DVec3)> {
    universe_world::turrets::turrets(charts.seed, system, sys).iter().map(|tu| tu.motion(sys, t, positions)).collect()
}

/// What pilot `i` makes of the ships around it (radar, and the pirates'
/// transponders): shelter is real (docked or landed, or under a turret's guns).
fn sightings(view: &PilotView, me: usize, system: usize, pos: DVec3, guns: &[(DVec3, DVec3)]) -> Vec<Sighting> {
    let sheltered = |s: &Snap| s.landed || guns.iter().any(|(p, _)| p.distance(s.position) < universe_world::turrets::TURRET_RANGE + universe_avionics::hunter::SHELTER_MARGIN);
    view.snaps
        .iter()
        .enumerate()
        .filter(|&(id, s)| id != me && s.system == system && !s.transit && s.position.distance(pos) < RADAR_RANGE)
        .map(|(id, s)| Sighting {
            id,
            position: s.position,
            velocity: s.velocity,
            pirate: s.pirate,
            docked: sheltered(s),
            destroyed: s.destroyed,
            hyperdrive: s.hyperdrive,
            landed: s.landed,
            aggressed: s.aggressed,
            hull: s.hull,
        })
        .collect()
}

/// Pilot `pilot` (flying craft `i`) carries out an order.
fn obey(order: Order, a: &mut Avionics, link: &mut PoolLink, events: &mut Vec<Event>) {
    match order {
        Order::Route(route) => a.route = route,
        Order::Then(stop) => {
            let r = &mut a.route;
            r.stops.truncate(r.next + 1);
            r.stops.push(stop);
        }
        Order::Flee(stop) => {
            let r = &mut a.route;
            if r.stops.get(r.next) != Some(&stop) {
                r.stops.insert(r.next.min(r.stops.len()), stop);
            }
            r.active = true;
            r.dwell_until = None;
            r.departing = false;
            a.clearance = None;
        }
        Order::NavTarget(target) => a.set_nav_target(link, target, events),
        Order::RequestClearance => {
            a.request_clearance(link, events);
        }
        Order::ToggleAutopilot => a.toggle_autopilot(link, events),
    }
}

/// The pilot of ship `id` thinks on `view`, if it's time to or something's
/// waiting for it: what it posts. With a human at the stick (`human`), it
/// thinks every time, flies by the stick (unless a program has it), and
/// hunts no one.
pub(crate) fn think(pilot: &mut Pilot, id: usize, view: &PilotView, human: Option<Controls>) -> Option<Posting> {
    let (system, ref ship) = *view.ships.get(id)?;
    if human.is_none() && view.tick < pilot.next_think && pilot.feed.is_empty() && pilot.orders.is_empty() {
        return None;
    }
    let a = &mut pilot.avionics;
    let was_hunting = a.hunting.is_some();
    let mut events = Vec::new();
    // What happened to the ship since it last thought.
    a.record(std::mem::take(&mut pilot.feed), &mut events);
    // Its ship, with its own commands on their way.
    let seen = seen(ship, &mut pilot.pending, view.tick);
    let sys = view.charts.system(system);
    let mut link = PoolLink { view, sys, ship: seen, system, id, devices: Vec::new(), requests: Vec::new(), pending: &mut pilot.pending };
    for order in std::mem::take(&mut pilot.orders) {
        obey(order, a, &mut link, &mut events);
    }
    // The last step's outcome (an arrival, a lapsed clearance), then this one.
    a.conclude(&mut link, &mut events);
    let pos = link.ship.position;
    let threat = human.is_none() && may_defend(a, &link.ship) && view.aggressors.iter().any(|&(s, p)| s == system && p.distance(pos) < DEFEND_RANGE);
    let sightings = if threat || (human.is_none() && wants_sightings(a, &link.ship, view.time)) {
        let guns = link.turrets();
        sightings(view, id, system, pos, &guns)
    } else {
        Vec::new()
    };
    let mark = match a.following.map(|f| f.anchor) {
        Some(universe_avionics::follow::Anchor::Ship(id)) => crate::follow::mark_in(&view.snaps, system, pos, id),
        _ => None,
    };
    let (stick, hunt_end) = if human.is_some() { (None, None) } else { a.hunt(&mut link, &sightings, &mut events) };
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
    let PoolLink { devices, requests, .. } = link;
    // Its ship's events went to the services when they happened: the rest.
    events.retain(|e| !matches!(e, Event::Ship(_)));
    Some(Posting {
        id,
        thought: view.tick,
        seen: view.time,
        devices,
        turn: Some(turn),
        requests,
        events,
        status: Status::of(&pilot.avionics),
        hunt_begun: !was_hunting && pilot.avionics.hunting.is_some(),
        hunt_end,
    })
}

/// Every pilot due thinks on `view`, side by side: their postings, in craft order.
fn think_all(pilots: &mut [Pilot], view: &PilotView) -> Vec<Posting> {
    use rayon::prelude::*;
    pilots.par_iter_mut().enumerate().filter_map(|(i, p)| think(p, crate::combat::craft_id(i), view, None).filter(|_| !p.silent)).collect()
}

/// Messages to pilots, delivered before they next think.
fn deliver(pilots: &mut [Pilot], mail: &mut Vec<(usize, Msg)>) {
    for (i, m) in mail.drain(..) {
        let Some(p) = pilots.get_mut(i) else { continue };
        match m {
            Msg::Feed(events) => p.feed.extend(events),
            Msg::Order(o) => p.orders.push(o),
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
        let (pilots, mail, s, st, slow, d) = (self.pilots.clone(), self.mail.clone(), slot.clone(), stop.clone(), self.slow.clone(), done.clone());
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
                    let postings = workers.install(|| think_all(list, &view));
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
                think_all(&mut pilots, &view)
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
        let (system, ref ship) = view.ships[id];
        let sys = view.charts.system(system);
        let seen = seen(ship, &mut pilot.pending, view.tick);
        let mut link = PoolLink { view, sys, ship: seen, system, id, devices: Vec::new(), requests: Vec::new(), pending: &mut pilot.pending };
        let mut events = Vec::new();
        let r = f(&mut pilot.avionics, &mut link, &mut events);
        let PoolLink { devices, requests, .. } = link;
        events.retain(|e| !matches!(e, Event::Ship(_)));
        pilot.next_think = view.tick;
        let posting = Posting { id, thought: view.tick, seen: view.time, devices, turn: None, requests, events, status: Status::of(&pilot.avionics), hunt_begun: false, hunt_end: None };
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
