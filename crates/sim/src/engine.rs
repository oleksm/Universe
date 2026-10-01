//! The world engine's boundary: the client sends `Command`s and reads `View`s.
//!
//! The engine owns the universe. Everything the client does to it is a
//! command (applied in order, before the next tick); everything it shows comes
//! from the latest view, a snapshot the engine publishes after each tick —
//! the player's ship and what its computers make of things (radar contacts,
//! fire control, the flight plan, the collision warning…), the other ships as
//! the client draws them, and what happened since the last view. Requests
//! with an answer (a lock, a trade) answer with events in the view.
//!
//! What's fixed about the galaxy (stars, gates, goods, the star systems) the
//! client reads from the shared `Charts`.

use std::sync::Arc;

use glam::DVec3;
use universe_avionics::avionics::Approach;
use universe_avionics::collision::Prediction;
use universe_avionics::follow::Manoeuvre;
use universe_avionics::route::Stop;
use universe_avionics::{Avionics, Event, NavTarget, Plan, Solution, Track};
use universe_world::charts::Charts;
use universe_world::crew::Reach;
use universe_world::goods::Category;
use universe_services::market::Quote;
use universe_world::turrets::Turret;
use universe_world::weapons::{Beam, Impact};
use universe_world::{Controls, Facility, Person, Ship, ShipCommands, StepResult, WalkCommands};

use universe_services::records::Kill;
use universe_services::records::TradeRecord;
use crate::contacts::Contact;
use crate::follow::FollowKind;
use universe_services::records::TrafficStats;
use crate::universe::Universe;

/// What the client asks of the world.
pub enum Command {
    /// New settings for the ship's devices (engine, thrusters, weapons…).
    Ship(ShipCommands),
    /// The throttle: changed by `delta`, or set to a value; and the
    /// thrusters (from the ship's settings as they are, not a stale view).
    Throttle { delta: f64, set: Option<f64> },
    Thrusters(DVec3),
    /// The pilot's stick, held until changed; the time warp.
    Stick(Controls),
    Warp(f64),
    /// On foot, for `dt` real seconds.
    Walk(WalkCommands, f64),
    ToggleHyperdrive,
    SetNavTarget(Option<NavTarget>),
    RequestClearance,
    CancelClearance,
    ToggleAutopilot,
    ToggleRoute,
    Follow(FollowKind),
    StopFollowing,
    /// Lock what's in the beam around the nose (again: the next).
    LockInBeam,
    CollisionWarning(bool),
    Respawn,
    /// Route editing: add a stop, drop the last, clear it, or a settler's random route.
    RoutePush(Stop),
    RoutePop,
    RouteClear,
    RouteRandom { seed: u64, stops: usize },
    /// Show this market's quotes in the view (None: none).
    WatchMarket(Option<Facility>),
    Trade { market: Facility, item: usize, units: i64 },
    /// Anything else, run on the universe (dev scenarios, tools).
    Run(Box<dyn FnOnce(&mut Universe) + Send>),
}

/// Another ship, as the client draws it.
#[derive(Clone, Debug)]
pub struct CraftView {
    pub name: String,
    pub system: usize,
    pub ship: Ship,
    pub pirate: bool,
    pub trader: bool,
    /// Its route: the stop it's on, how many, and what it's doing.
    pub route_next: usize,
    pub route_stops: usize,
    pub stage: &'static str,
    /// Fair game, as the law has it.
    pub aggressed: bool,
}

/// A market's quotes, for the market screen.
#[derive(Clone, Debug)]
pub struct MarketView {
    pub market: Facility,
    pub quotes: Vec<Quote>,
    pub banned: Vec<Category>,
    /// Quotes for what's in the hold and not listed (it may still be taken).
    pub held: Vec<(usize, Option<Quote>)>,
}

/// The world as the client sees it after a tick.
#[derive(Clone, Debug)]
pub struct View {
    pub time: f64,
    pub ship: Ship,
    pub ship_system: usize,
    /// Until when we're fair game, as the law has it (None: we're not).
    pub aggressed_until: Option<f64>,
    pub avionics: Avionics,
    pub crew: Person,
    /// Our credits, and what's in our hold (good, units), as the ledger has them.
    pub credits: f64,
    pub hold: Vec<(usize, u32)>,
    pub crafts: Arc<Vec<CraftView>>,
    pub traffic: TrafficStats,
    pub kills: Vec<Kill>,
    pub trade_log: Vec<TradeRecord>,
    /// What happened since the last view.
    pub events: Vec<Event>,
    /// Weapons fire: slugs in flight (system, where, how fast), beams, hits.
    pub slugs: Vec<(usize, DVec3, DVec3)>,
    pub beams: Vec<Beam>,
    pub impacts: Vec<Impact>,
    /// The ship's computers: radar contacts (nearest first), fire control on
    /// the locked one, approach guidance, the flight plan (and its serial,
    /// a new one each rebuild, and what it cost), the collision warning (made
    /// at `collision_at`, world time, costing `collision_cost`), the follow
    /// program's status.
    pub contacts: Vec<Contact>,
    pub fire: Option<(Track, Option<Solution>)>,
    pub approach: Option<Approach>,
    pub plan: Option<Arc<Plan>>,
    pub plan_serial: u64,
    pub plan_cost: f32,
    pub plan_every: f32,
    pub collision: Option<Prediction>,
    pub collision_at: f64,
    pub collision_cost: f32,
    pub following: Option<(Manoeuvre, String, f64)>,
    /// What the target marker points at (the nav target, else the nearest station).
    pub nav_marker: Option<(String, DVec3)>,
    /// On foot: what's in reach.
    pub reach: Option<Reach>,
    /// The market we're docked at; the markets of the system; the one watched.
    pub docked_market: Option<Facility>,
    pub markets: Vec<(Facility, String)>,
    pub market: Option<MarketView>,
    /// The ship's system's defence turrets, where they are now; who's on
    /// each pad of each of its ports.
    pub turrets: Vec<(Turret, DVec3)>,
    pub pads: Vec<[Option<usize>; universe_world::spaceport::PADS]>,
    /// The last tick: what it did, and what it took (ms).
    pub last_step: StepResult,
    pub sim_ms: f32,
    /// The NPC pilots: apart from the world (or in lockstep), and how many
    /// of their postings came late, and too late (dropped), so far.
    pub pilots: (bool, u64, u64),
    pub serial: u64,
    /// When the engine made it (real time): clients draw between views by it.
    pub made: std::time::Instant,
}

/// The world engine: the universe, and the player's ship's computers that
/// run with it.
pub struct Engine {
    pub universe: Universe,
    charts: Arc<Charts>,
    plan: Option<Arc<Plan>>,
    plan_serial: u64,
    plan_age: f64,
    plan_cost: f32,
    plan_for: Option<universe_avionics::Clearance>,
    collision: Option<Prediction>,
    collision_age: f64,
    collision_at: f64,
    collision_cost: f32,
    watched: Option<Facility>,
    events: Vec<Event>,
    last_step: StepResult,
    sim_ms: f32,
    serial: u64,
    contacts: Vec<Contact>,
    fire: Option<(Track, Option<Solution>)>,
    /// The pilot's stick and the warp, as last set.
    stick: Controls,
    warp: f64,
}

/// World ticks per second when the engine runs on its own.
pub const TICK_HZ: f64 = 60.0;

impl Engine {
    pub fn new(mut universe: Universe) -> Self {
        let charts = Arc::new(universe.world.charts());
        // The NPC pilots think apart from the world (UNIVERSE_LOCKSTEP=1: in
        // step with it), on half the cores.
        if std::env::var_os("UNIVERSE_LOCKSTEP").is_none() {
            universe.run_pilots_apart(std::thread::available_parallelism().map_or(2, |n| (n.get() / 2).max(1)));
        }
        Engine {
            universe,
            charts,
            plan: None,
            plan_serial: 0,
            plan_age: f64::INFINITY,
            plan_cost: 0.0,
            plan_for: None,
            collision: None,
            collision_age: f64::INFINITY,
            collision_at: 0.0,
            collision_cost: 0.0,
            watched: None,
            events: Vec::new(),
            last_step: StepResult::default(),
            sim_ms: 0.0,
            serial: 0,
            contacts: Vec::new(),
            fire: None,
            stick: Controls::default(),
            warp: 1.0,
        }
    }

    pub fn charts(&self) -> Arc<Charts> {
        self.charts.clone()
    }

    /// Carry out a command now.
    pub fn apply(&mut self, c: Command) {
        let u = &mut self.universe;
        match c {
            Command::Ship(c) => u.command(&c),
            Command::Throttle { delta, set } => {
                let throttle = set.unwrap_or(u.ship.throttle + delta).clamp(0.0, 1.0);
                if throttle != u.ship.throttle {
                    u.command(&ShipCommands { throttle, ..u.ship.holding() });
                }
            }
            Command::Thrusters(rcs) => {
                if rcs != u.ship.rcs {
                    u.command(&ShipCommands { rcs, ..u.ship.holding() });
                }
            }
            Command::Stick(c) => self.stick = c,
            Command::Warp(w) => self.warp = w,
            Command::Walk(c, dt) => u.walk(&c, dt),
            Command::ToggleHyperdrive => u.toggle_hyperdrive(),
            Command::SetNavTarget(t) => u.set_nav_target(t),
            Command::RequestClearance => {
                u.request_clearance();
            }
            Command::CancelClearance => u.cancel_clearance(),
            Command::ToggleAutopilot => u.toggle_autopilot(),
            Command::ToggleRoute => u.toggle_route(),
            Command::Follow(kind) => u.follow(kind),
            Command::StopFollowing => u.stop_following(),
            Command::LockInBeam => {
                let had = u.avionics.contact.is_some();
                let e = match u.lock_in_beam() {
                    Some(c) => Event::Lock { name: Some(c.name) },
                    None if had => Event::Lock { name: None },
                    None => Event::NothingInBeam,
                };
                u.events.push(e);
            }
            Command::CollisionWarning(on) => {
                u.avionics.collision_warning = on;
                self.collision_age = f64::INFINITY;
            }
            Command::Respawn => u.respawn(),
            Command::RoutePush(stop) => u.avionics.route.stops.push(stop),
            Command::RoutePop => {
                u.avionics.route.pop();
            }
            Command::RouteClear => u.avionics.route.clear(),
            Command::RouteRandom { seed, stops } => {
                u.avionics.route.clear();
                u.avionics.route.stops = u.settler_route(seed, stops);
            }
            Command::WatchMarket(m) => self.watched = m,
            Command::Trade { market, item, units } => {
                let name = u.world.goods[item].name.to_uppercase();
                let e = match u.trade(market, item, units) {
                    Ok(credits) => Event::Traded { item: name, units, credits },
                    Err(reason) => Event::Refused { reason },
                };
                u.events.push(e);
            }
            Command::Run(f) => f(&mut self.universe),
        }
    }

    /// Advance the world by `real_dt` real seconds at `warp`, the pilot's
    /// stick at `controls`; then the player's ship's computers have their say.
    pub fn tick(&mut self, real_dt: f64, warp: f64, controls: &Controls) {
        let start = std::time::Instant::now();
        self.last_step = self.universe.step_world(real_dt, warp, controls);
        let ms = start.elapsed().as_secs_f32() * 1000.0;
        self.sim_ms = if self.sim_ms == 0.0 { ms } else { self.sim_ms + (ms - self.sim_ms) * 0.05 };
        let _p = universe_prof::scope("sim/ship computers");
        let u = &mut self.universe;
        // Radar and fire control, every tick.
        self.contacts = universe_prof::time("sim/ship computers/radar", || u.contacts());
        if u.avionics.contact.is_some() && u.locked_contact_in(&self.contacts).is_none() {
            u.avionics.contact = None;
            u.events.push(Event::ContactLost);
        }
        self.fire = u.fire_control(&self.contacts);
        // The flight plan flies the autopilot ahead through the physics (1-10
        // ms): rebuilt ten times a second, less often when it's dearer (about
        // 5% of the time), or at once when the clearance or phase changes.
        self.plan_age += real_dt;
        let key = u.avionics.clearance;
        let changed = key.map(|c| (c.target, c.autopilot, c.phase)) != self.plan_for.map(|c| (c.target, c.autopilot, c.phase));
        if changed || self.plan_age >= plan_every(self.plan_cost) as f64 || !u.ship.is_flying() {
            let start = std::time::Instant::now();
            let plan = universe_prof::time("sim/ship computers/flight plan", || u.plan());
            self.plan_cost = start.elapsed().as_secs_f32();
            self.plan = plan.map(Arc::new);
            self.plan_serial += 1;
            self.plan_age = 0.0;
            self.plan_for = key;
        }
        // The collision warning, five times a second.
        self.collision_age += real_dt;
        if !u.avionics.collision_warning {
            self.collision = None;
        } else if self.collision_age >= 0.2 {
            self.collision_age = 0.0;
            let start = std::time::Instant::now();
            self.collision = universe_prof::time("sim/ship computers/collision warning", || u.collision_warning(&self.contacts));
            self.collision_cost = start.elapsed().as_secs_f32();
            self.collision_at = u.world.time;
        }
        self.events.append(&mut u.events);
    }

    /// The view of the world now (what's happened since the last one goes with it).
    pub fn view(&mut self) -> View {
        let _p = universe_prof::scope("sim/view");
        let u = &mut self.universe;
        let now = u.world.time;
        let system = u.ship_system;
        let sys = u.ship_system();
        let crafts = u
            .crafts
            .iter()
            .enumerate()
            .map(|(i, c)| CraftView {
                name: c.name.clone(),
                system: c.system,
                ship: c.ship.clone(),
                pirate: c.status.pirate,
                trader: c.trader,
                route_next: c.status.route_next,
                route_stops: c.status.route_len,
                stage: crate::contacts::activity(c),
                aggressed: u.law.aggressed(crate::combat::craft_id(i), now),
            })
            .collect();
        let markets = universe_world::traffic::facilities(&sys).into_iter().map(|f| (f, f.name(&sys))).collect();
        let market = self.watched.map(|f| {
            let (quotes, banned) = u.market_quotes(f);
            let held = u.hold().into_iter().map(|(i, _)| i).filter(|i| !quotes.iter().any(|q| q.offer.item == *i)).collect::<Vec<_>>();
            let held = held.into_iter().map(|i| (i, u.quote_for(f, i))).collect();
            MarketView { market: f, quotes, banned, held }
        });
        let pads = (0..sys.spaceports.len()).map(|p| u.atc.owners(system, p)).collect();
        self.serial += 1;
        View {
            time: now,
            ship: u.ship.clone(),
            ship_system: system,
            aggressed_until: u.law.until(crate::combat::PLAYER, now),
            avionics: u.avionics.clone(),
            crew: u.crew,
            credits: u.credits(),
            hold: u.hold(),
            crafts: Arc::new(crafts),
            traffic: u.records.stats,
            kills: u.records.kills.clone(),
            trade_log: u.records.trades.clone(),
            events: std::mem::take(&mut self.events),
            slugs: u.world.slugs.iter().map(|s| (s.system, s.projectile.position, s.projectile.velocity)).collect(),
            beams: u.world.beams.clone(),
            impacts: u.world.impacts.clone(),
            contacts: self.contacts.clone(),
            fire: self.fire,
            approach: u.approach(),
            plan: self.plan.clone(),
            plan_serial: self.plan_serial,
            plan_cost: self.plan_cost,
            plan_every: plan_every(self.plan_cost),
            collision: self.collision.clone(),
            collision_at: self.collision_at,
            collision_cost: self.collision_cost,
            following: u.following_status(),
            nav_marker: nav_marker(u),
            reach: u.pilot_reach(),
            docked_market: u.docked_market(),
            markets,
            market,
            turrets: u.world.turret_motions(system).into_iter().map(|(t, p, _)| (t, p)).collect(),
            pads,
            last_step: self.last_step,
            sim_ms: self.sim_ms,
            pilots: (u.pool.apart(), u.pool.late, u.pool.dropped),
            serial: self.serial,
            made: std::time::Instant::now(),
        }
    }
}

/// How often the flight plan is rebuilt, for what it costs (s).
fn plan_every(cost: f32) -> f32 {
    (cost * 20.0).clamp(0.1, 1.0)
}

/// The nav target (its name and where it is), else the nearest station.
fn nav_marker(u: &mut Universe) -> Option<(String, DVec3)> {
    if let Some(t) = u.avionics.nav_target {
        let pos = u.target_position(t)?;
        let name = match t {
            NavTarget::Station(_) | NavTarget::Gate(_) => u.target_name(t),
            NavTarget::Spaceport(p) => u.ship_system().spaceports[p].name.clone(),
        };
        return Some((name.to_uppercase(), pos));
    }
    let sys = u.ship_system();
    let station = sys.station()?;
    let positions = u.world.rails_now(u.ship_system);
    Some((String::new(), positions[station]))
}

/// What goes to the engine's thread.
enum Msg {
    Command(Box<Command>),
    /// Run this on the engine, then reply.
    Call(Box<dyn FnOnce(&mut Engine) + Send>),
    Stop,
}

/// The latest view, waiting for the client. A view not taken before the
/// next is published passes on what happened (events, hits) to the next.
#[derive(Default)]
struct Mailbox {
    view: Option<View>,
}

fn post(mailbox: &std::sync::Mutex<Mailbox>, mut view: View) {
    let mut m = mailbox.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(old) = m.view.take() {
        let mut events = old.events;
        events.append(&mut view.events);
        view.events = events;
        let mut impacts = old.impacts;
        impacts.append(&mut view.impacts);
        view.impacts = impacts;
    }
    m.view = Some(view);
}

/// The client's handle on the world engine: send commands, read the latest
/// view. It starts out holding the engine (to set the world up: scenarios,
/// tests); `start` moves it to its own thread, ticking at `TICK_HZ`.
pub struct EngineHandle {
    local: Option<Engine>,
    charts: Arc<Charts>,
    view: Arc<View>,
    /// When the current view arrived (real time).
    pub view_at: std::time::Instant,
    tx: Option<std::sync::mpsc::Sender<Msg>>,
    mailbox: Arc<std::sync::Mutex<Mailbox>>,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl EngineHandle {
    pub fn new(universe: Universe) -> Self {
        let mut engine = Engine::new(universe);
        let view = Arc::new(engine.view());
        let charts = engine.charts();
        EngineHandle { local: Some(engine), charts, view, view_at: std::time::Instant::now(), tx: None, mailbox: Default::default(), thread: None }
    }

    pub fn charts(&self) -> Arc<Charts> {
        self.charts.clone()
    }

    /// Run the engine on its own thread from now on.
    pub fn start(&mut self) {
        let Some(mut engine) = self.local.take() else { return };
        let (tx, rx) = std::sync::mpsc::channel::<Msg>();
        let mailbox = self.mailbox.clone();
        let thread = std::thread::Builder::new()
            .name("world engine".into())
            .spawn(move || {
                let step = std::time::Duration::from_secs_f64(1.0 / TICK_HZ);
                let mut next = std::time::Instant::now();
                loop {
                    while let Ok(m) = rx.try_recv() {
                        match m {
                            Msg::Command(c) => engine.apply(*c),
                            Msg::Call(f) => f(&mut engine),
                            Msg::Stop => return,
                        }
                    }
                    let (warp, stick) = (engine.warp, engine.stick);
                    engine.tick(1.0 / TICK_HZ, warp, &stick);
                    post(&mailbox, engine.view());
                    // Keep time; fallen far behind, start afresh rather than race.
                    next += step;
                    let now = std::time::Instant::now();
                    if next > now {
                        std::thread::sleep(next - now);
                    } else if now - next > step * 10 {
                        next = now;
                    }
                }
            })
            .expect("world engine thread");
        self.tx = Some(tx);
        self.thread = Some(thread);
    }

    pub fn send(&mut self, c: Command) {
        match (&mut self.local, &self.tx) {
            (Some(engine), _) => engine.apply(c),
            (None, Some(tx)) => {
                let _ = tx.send(Msg::Command(Box::new(c)));
            }
            (None, None) => {}
        }
    }

    /// Run `f` on the engine and wait for its answer (save, load, tools).
    pub fn call<R: Send + 'static>(&mut self, f: impl FnOnce(&mut Universe) -> R + Send + 'static) -> Option<R> {
        if let Some(engine) = &mut self.local {
            return Some(f(&mut engine.universe));
        }
        let (reply, answer) = std::sync::mpsc::channel();
        self.tx.as_ref()?.send(Msg::Call(Box::new(move |e: &mut Engine| {
            let _ = reply.send(f(&mut e.universe));
        }))).ok()?;
        answer.recv().ok()
    }

    /// Before `start`: run the world for a frame here.
    pub fn tick(&mut self, real_dt: f64, warp: f64, controls: &Controls) {
        if let Some(engine) = &mut self.local {
            engine.tick(real_dt, warp, controls);
            self.view = Arc::new(engine.view());
            self.view_at = std::time::Instant::now();
        }
    }

    /// Take the newest view, if there's one since the last; true if so.
    pub fn poll(&mut self) -> bool {
        let fresh = self.mailbox.lock().unwrap_or_else(|e| e.into_inner()).view.take();
        match fresh {
            Some(v) => {
                self.view = Arc::new(v);
                self.view_at = std::time::Instant::now();
                true
            }
            None => false,
        }
    }

    /// Before `start`: refresh the view without running the world.
    pub fn refresh(&mut self) {
        if let Some(engine) = &mut self.local {
            self.view = Arc::new(engine.view());
        }
    }

    pub fn view(&self) -> Arc<View> {
        self.view.clone()
    }

    /// The universe itself, before `start` (setting up: scenarios). Panics after.
    pub fn universe(&mut self) -> &mut Universe {
        &mut self.local.as_mut().expect("the engine runs on its own thread now: use `call`").universe
    }

    pub fn running(&self) -> bool {
        self.thread.is_some()
    }
}

impl Drop for EngineHandle {
    fn drop(&mut self) {
        if let Some(tx) = &self.tx {
            let _ = tx.send(Msg::Stop);
        }
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}
