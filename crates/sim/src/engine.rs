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
// (A command is a message passed once: its size doesn't matter.)
#[allow(clippy::large_enum_variant)]
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
    /// Keep at a range from, or orbit, the locked contact or nav target:
    /// at `range` (m) if given, else the preset nearest how far it is (again
    /// for the same: the next one out).
    Follow(FollowKind, Option<f64>),
    /// Close on a rock (body `body` among field `field`'s), to anchor.
    CloseOn { field: usize, body: usize },
    /// Lock on a rock (field, body among its bodies), or let the lock go.
    LockRock(Option<(usize, usize)>),
    /// Lock on the radar contact with this id (a pick from the list).
    LockContact(usize),
    /// Fill the tank where docked or landed.
    Refuel,
    /// Have the hull mended (docked at a station).
    Repair,
    /// Buy item `0` of the vending machine within reach (on foot at a spaceport).
    Vend(usize),
    /// Trim the ship (docked at a station's shipyard).
    Trim(universe_world::trim::Trim),
    /// Board the passengers booked for there (docked), or (None) land
    /// those aboard where they're bound.
    Passengers(Option<(usize, universe_world::Facility)>),
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
    /// Refit slot `slot` with module `module` (content key; None: empty it), docked at a station.
    Refit { slot: String, module: Option<String> },
    /// Buy hull `hull` (content key), trading in the ship, docked at a station.
    BuyHull { hull: String },
    /// Anything else, run on the universe (dev scenarios, tools).
    Run(Box<dyn FnOnce(&mut Universe) + Send>),
    /// The client's cockpit's postings (see `cockpit`).
    Post(Vec<crate::pilots::Posting>),
}

/// Another ship, as the client draws it.
#[derive(Clone, Debug)]
pub struct CraftView {
    pub name: String,
    pub system: usize,
    pub ship: Ship,
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
    /// How old the quotes are (s): 0 where we're docked; elsewhere, its price
    /// board as it reached us over the hypernet (infinite: long known);
    /// None: no word of it reaches us.
    pub age: Option<f64>,
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
    /// Missiles in flight: (system, position, velocity, motor burning, after us).
    pub missiles: Vec<(usize, DVec3, DVec3, bool, bool)>,
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
    /// What our orders on their way will do to our ship (the cockpit's
    /// prediction): add to where it is, and turn it by, to draw it.
    pub prediction: Option<(DVec3, glam::DQuat)>,
    /// What the target marker points at (the nav target, else the nearest station).
    pub nav_marker: Option<(String, DVec3)>,
    /// On foot: what's in reach.
    pub reach: Option<Reach>,
    /// Anchored: what's been dug out of our rock so far (kg).
    pub dug: f64,
    /// The settled economy's places, as of the last change.
    pub economy: Arc<Vec<universe_services::economy::Place>>,
    /// What's been dug out of the rocks of our system: ((field, body), kg).
    pub mined: Vec<((usize, usize), f64)>,
    /// The market we're docked at; the markets of the system; the one watched.
    pub docked_market: Option<Facility>,
    /// Passage booked from the market docked at (see `Universe::bookings`).
    pub bookings: Vec<crate::commerce::Booking>,
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
    watched: Option<Facility>,
    events: Vec<Event>,
    last_step: StepResult,
    sim_ms: f32,
    serial: u64,
    /// The pilot's stick and the warp, as last set.
    stick: Controls,
    warp: f64,
}

/// World ticks per second when the engine runs on its own.
pub const TICK_HZ: f64 = 60.0;

impl Engine {
    pub fn new(mut universe: Universe) -> Self {
        let charts = Arc::new(universe.world.charts());
        // The HUD shows the flight plan; the cockpit has a first look.
        if let Some(c) = universe.player.as_mut().and_then(|p| p.as_any_mut().downcast_mut::<crate::cockpit::Cockpit>()) {
            c.plan_wanted = true;
        }
        universe.cockpit_now();
        Engine {
            universe,
            charts,
            watched: None,
            events: Vec::new(),
            last_step: StepResult::default(),
            sim_ms: 0.0,
            serial: 0,
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
            Command::Throttle { delta, set } => u.throttle(delta, set),
            Command::Thrusters(rcs) => u.thrusters(rcs),
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
            Command::Follow(kind, range) => u.follow(kind, range),
            Command::CloseOn { field, body } => u.close_on(field, body),
            Command::LockRock(rock) => u.cockpit().lock_rock(rock),
            Command::LockContact(id) => u.cockpit().lock_contact(id),
            Command::StopFollowing => u.stop_following(),
            Command::LockInBeam => {
                u.lock_in_beam();
            }
            Command::CollisionWarning(on) => u.cockpit().collision_warning(on),
            Command::Respawn => u.respawn(),
            Command::Refuel => u.refuel_player(),
            Command::Repair => u.repair_player(),
            Command::Vend(item) => u.vend(item),
            Command::Trim(t) => {
                let _ = u.set_trim(t);
            }
            Command::Passengers(to) => u.passengers(to),
            Command::RoutePush(stop) => u.cockpit().route_push(stop),
            Command::RoutePop => u.cockpit().route_pop(),
            Command::RouteClear => u.cockpit().route_set(Vec::new()),
            Command::RouteRandom { seed, stops } => {
                let stops = u.settler_route(seed, stops);
                u.cockpit().route_set(stops);
            }
            Command::WatchMarket(m) => self.watched = m,
            Command::BuyHull { hull } => {
                if let Some(h) = universe_world::content::content().handle(&hull) {
                    let _ = u.buy_hull(h);
                }
            }
            Command::Refit { slot, module } => {
                let m = module.and_then(|k| universe_world::content::content().handle(&k));
                let _ = u.refit(&slot, m);
            }
            Command::Trade { market, item, units } => {
                let name = u.world.goods[item].name.to_uppercase();
                let e = match u.trade(market, item, units) {
                    Ok(credits) => Event::Traded { item: name, units, credits },
                    Err(reason) => Event::Refused { reason },
                };
                u.events.push(e);
            }
            Command::Run(f) => f(&mut self.universe),
            Command::Post(postings) => self.universe.accept(postings),
        }
    }

    /// Advance the world by `real_dt` real seconds at `warp`, the pilot's
    /// stick at `controls`; then the player's ship's computers have their say.
    pub fn tick(&mut self, real_dt: f64, warp: f64, controls: &Controls) {
        let start = std::time::Instant::now();
        self.last_step = self.universe.step_world(real_dt, warp, controls);
        let ms = start.elapsed().as_secs_f32() * 1000.0;
        self.sim_ms = if self.sim_ms == 0.0 { ms } else { self.sim_ms + (ms - self.sim_ms) * 0.05 };
        let u = &mut self.universe;
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
                route_next: c.status.route_next,
                route_stops: c.status.route_len,
                stage: crate::contacts::activity(c),
                aggressed: u.law.aggressed(crate::combat::craft_id(i), now),
            })
            .collect();
        let markets = universe_world::traffic::facilities(&sys).into_iter().map(|f| (f, f.name(&sys))).collect();
        let market = self.watched.map(|f| u.market_view(f));
        let pads = (0..sys.spaceports.len()).map(|p| u.atc.owners(system, universe_world::Facility::Spaceport(p))).collect();
        self.serial += 1;
        View {
            time: now,
            ship: u.ship.clone(),
            ship_system: system,
            aggressed_until: u.law.until(crate::combat::PLAYER, now),
            avionics: Default::default(),
            crew: u.crew,
            credits: u.credits(),
            hold: u.hold(),
            crafts: Arc::new(crafts),
            // (Hunts and posses: the NPC operator's own tally.)
            traffic: universe_services::records::TrafficStats {
                hunts: u.npcs.tally().0,
                defences: u.npcs.tally().1,
                ..u.records.stats
            },
            kills: u.records.kills.clone(),
            trade_log: u.records.trades.clone(),
            events: std::mem::take(&mut self.events),
            slugs: u.world.slugs.iter().map(|s| (s.system, s.projectile.position, s.projectile.velocity)).collect(),
            missiles: u.world.missiles.iter().map(|m| (m.system, m.position, m.velocity, m.age < universe_world::missiles::MISSILE_BURN, m.target == crate::combat::PLAYER)).collect(),
            beams: u.world.beams.clone(),
            impacts: u.world.impacts.clone(),
            contacts: Vec::new(),
            fire: None,
            approach: None,
            plan: None,
            plan_serial: 0,
            plan_cost: 0.0,
            plan_every: 0.0,
            collision: None,
            collision_at: 0.0,
            collision_cost: 0.0,
            following: None,
            prediction: None,
            nav_marker: None,
            reach: u.pilot_reach(),
            economy: u.markets.economy.snapshot(),
            mined: u.world.mined.iter().filter(|((s, _, _), _)| *s == system).map(|(&(_, f, b), &kg)| ((f, b), kg)).collect(),
            dug: match u.ship.state {
                universe_world::ShipState::Anchored { field, body, .. } => u.world.dug(system, field, body),
                _ => 0.0,
            },
            docked_market: u.docked_market(),
            bookings: u.docked_market().map(|m| u.bookings(u.ship_system, m)).unwrap_or_default(),
            markets,
            market,
            turrets: u.world.turret_motions(system).into_iter().map(|(t, p, _)| (t, p)).collect(),
            pads,
            last_step: self.last_step,
            sim_ms: self.sim_ms,
            pilots: (u.npcs.apart(), u.late, u.dropped),
            serial: self.serial,
            made: std::time::Instant::now(),
        }
        .with_cockpit(self.universe.player.as_ref().and_then(|p| p.as_any().downcast_ref::<crate::cockpit::Cockpit>()))
    }
}

impl View {
    /// The view with the cockpit's displays filled in (the cockpit is the
    /// client's, wherever it runs).
    pub fn with_cockpit(mut self, c: Option<&crate::cockpit::Cockpit>) -> View {
        let Some(c) = c else { return self };
        self.avionics = c.avionics().clone();
        self.contacts = c.contacts.clone();
        self.fire = c.fire;
        self.approach = c.approach();
        self.plan = c.plan.clone();
        self.plan_serial = c.plan_serial;
        self.plan_cost = c.plan_cost;
        self.plan_every = crate::cockpit::plan_every(c.plan_cost);
        self.collision = c.collision.clone();
        self.collision_at = c.collision_at;
        self.collision_cost = c.collision_cost;
        self.following = c.following_status();
        self.prediction = c.prediction;
        self.nav_marker = c.nav_marker();
        self
    }

    /// The cockpit's part as `before` had it (the cockpit busy just now).
    pub fn with_cockpit_of(mut self, before: &View) -> View {
        self.avionics = before.avionics.clone();
        self.contacts = before.contacts.clone();
        self.fire = before.fire;
        self.approach = before.approach.clone();
        self.plan = before.plan.clone();
        self.plan_serial = before.plan_serial;
        self.plan_cost = before.plan_cost;
        self.plan_every = before.plan_every;
        self.collision = before.collision.clone();
        self.collision_at = before.collision_at;
        self.collision_cost = before.collision_cost;
        self.following = before.following.clone();
        self.prediction = before.prediction;
        self.nav_marker = before.nav_marker.clone();
        self
    }
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
    /// The player's cockpit, on the client's side once the engine runs on
    /// its own thread: it thinks on its own thread (the world link) on every
    /// tick's view, and posts back.
    cockpit: Option<Arc<std::sync::Mutex<crate::cockpit::Cockpit>>>,
    link: Option<std::thread::JoinHandle<()>>,
    charts: Arc<Charts>,
    view: Arc<View>,
    /// When the current view arrived (real time).
    pub view_at: std::time::Instant,
    tx: Option<std::sync::mpsc::Sender<Msg>>,
    mailbox: Arc<std::sync::Mutex<Mailbox>>,
    thread: Option<std::thread::JoinHandle<()>>,
    /// Commands waiting for the cockpit (busy thinking on its own thread):
    /// the client never waits on it — they go, in order, once it's free.
    held: std::collections::VecDeque<Command>,
}

impl EngineHandle {
    pub fn new(universe: Universe) -> Self {
        let mut engine = Engine::new(universe);
        let view = Arc::new(engine.view());
        let charts = engine.charts();
        EngineHandle { local: Some(engine), cockpit: None, link: None, charts, view, view_at: std::time::Instant::now(), tx: None, mailbox: Default::default(), thread: None, held: Default::default() }
    }

    pub fn charts(&self) -> Arc<Charts> {
        self.charts.clone()
    }

    /// Run the engine on its own thread from now on.
    pub fn start(&mut self) {
        let Some(mut engine) = self.local.take() else { return };
        // The NPC pilots think apart from the world from now on, as it runs in
        // real time (before, setting up, they keep in step with it, however
        // fast it's run), on half the cores. UNIVERSE_LOCKSTEP=1: in step always.
        if std::env::var_os("UNIVERSE_LOCKSTEP").is_none() {
            engine.universe.run_pilots_apart(std::thread::available_parallelism().map_or(2, |n| (n.get() / 2).max(1)));
        }
        let (tx, rx) = std::sync::mpsc::channel::<Msg>();
        let mailbox = self.mailbox.clone();
        // The cockpit comes to the client (UNIVERSE_COCKPIT_IN_ENGINE=1: it stays).
        let (to_link, from_engine) = std::sync::mpsc::channel::<(Arc<crate::cockpit::CockpitView>, Vec<universe_world::ShipEvent>)>();
        if std::env::var_os("UNIVERSE_COCKPIT_IN_ENGINE").is_none()
            && let Some(c) = engine.universe.player.take().and_then(|p| p.into_any().downcast::<crate::cockpit::Cockpit>().ok())
        {
            let cockpit = Arc::new(std::sync::Mutex::new(*c));
            let (k, back) = (cockpit.clone(), tx.clone());
            let link = std::thread::Builder::new()
                .name("world link".into())
                .spawn(move || {
                    while let Ok((view, feed)) = from_engine.recv() {
                        let postings = {
                            let mut c = k.lock().unwrap_or_else(|e| e.into_inner());
                            c.feed(feed);
                            c.view(view);
                            c.take_postings()
                        };
                        if back.send(Msg::Command(Box::new(Command::Post(postings)))).is_err() {
                            return;
                        }
                    }
                })
                .expect("world link thread");
            self.cockpit = Some(cockpit);
            self.link = Some(link);
        }
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
                            Msg::Stop => {
                                // A recorded session is saved on the way out.
                                if let Some(path) = std::env::var_os("UNIVERSE_RECORD")
                                    && let Some(save) = engine.universe.world_save()
                                    && let Ok(json) = serde_json::to_string(&save)
                                {
                                    let _ = std::fs::write(&path, json);
                                    eprintln!("session recorded to {}: {} ticks, state hash {:x}", path.to_string_lossy(), save.log.ticks.len(), engine.universe.state_hash());
                                }
                                return;
                            }
                        }
                    }
                    let (warp, stick) = (engine.warp, engine.stick);
                    engine.tick(1.0 / TICK_HZ, warp, &stick);
                    if let Some(out) = engine.universe.cockpit_out.take() {
                        let _ = to_link.send(out);
                    }
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
        self.held.push_back(c);
        self.flush();
    }

    /// The commands held, in order, as far as the cockpit is free to take
    /// them (never waiting on it: it may be busy thinking).
    fn flush(&mut self) {
        while let Some(c) = self.held.pop_front() {
            // The cockpit's commands go to it, here; the rest on to the world.
            let c = match self.cockpit.clone() {
                Some(k) => {
                    let mut cockpit = match k.try_lock() {
                        Ok(g) => g,
                        Err(std::sync::TryLockError::Poisoned(p)) => p.into_inner(),
                        Err(std::sync::TryLockError::WouldBlock) => {
                            self.held.push_front(c);
                            return;
                        }
                    };
                    match self.cockpit_command(&mut cockpit, c) {
                        Some(c) => c,
                        None => continue,
                    }
                }
                None => c,
            };
            self.deliver(c);
        }
    }

    fn deliver(&mut self, c: Command) {
        match (&mut self.local, &self.tx) {
            (Some(engine), _) => engine.apply(c),
            (None, Some(tx)) => {
                let _ = tx.send(Msg::Command(Box::new(c)));
            }
            (None, None) => {}
        }
    }

    /// Carry out a command meant for the client's cockpit (None), or hand it
    /// back for the engine.
    fn cockpit_command(&mut self, k: &mut crate::cockpit::Cockpit, c: Command) -> Option<Command> {
        if let Command::RouteRandom { seed, stops } = c {
            let stops = self.call(move |u| u.settler_route(seed, stops))?;
            k.route_set(stops);
            return None;
        }
        match c {
            Command::Ship(c) => k.command(&c),
            Command::Throttle { delta, set } => k.throttle(delta, set),
            Command::Thrusters(rcs) => k.thrusters(rcs),
            Command::Stick(s) => k.stick(s),
            Command::ToggleHyperdrive => k.toggle_hyperdrive(),
            Command::SetNavTarget(t) => k.set_nav_target(t),
            Command::RequestClearance => {
                k.request_clearance();
            }
            Command::CancelClearance => k.cancel_clearance(),
            Command::ToggleAutopilot => k.toggle_autopilot(),
            Command::ToggleRoute => k.toggle_route(),
            Command::Follow(kind, range) => k.follow(kind, range),
            Command::CloseOn { field, body } => k.close_on(field, body),
            Command::LockRock(rock) => k.lock_rock(rock),
            Command::LockContact(id) => k.lock_contact(id),
            Command::StopFollowing => k.stop_following(),
            Command::LockInBeam => {
                k.lock_in_beam();
            }
            Command::CollisionWarning(on) => k.collision_warning(on),
            Command::RoutePush(stop) => k.route_push(stop),
            Command::RoutePop => k.route_pop(),
            Command::RouteClear => k.route_set(Vec::new()),
            other => return Some(other),
        }
        None
    }

    /// A save of the game (the world's, with the cockpit's avionics).
    pub fn save(&mut self) -> Option<crate::save::UniverseSave> {
        match &self.cockpit {
            Some(k) => {
                let avionics = k.lock().unwrap_or_else(|e| e.into_inner()).avionics().clone();
                self.call(move |u| u.save_with(avionics))
            }
            None => self.call(|u| u.save()),
        }
    }

    /// Load a save (the world's part there, the avionics into the cockpit).
    pub fn load(&mut self, save: crate::save::UniverseSave) -> Option<()> {
        if let Some(k) = &self.cockpit {
            let mut k = k.lock().unwrap_or_else(|e| e.into_inner());
            *k.avionics_mut() = universe_avionics::Avionics { route: save.route.clone(), ..save.avionics.clone() };
        }
        self.call(move |u| u.load(save))
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
        self.flush();
        let fresh = self.mailbox.lock().unwrap_or_else(|e| e.into_inner()).view.take();
        match fresh {
            Some(v) => {
                // (The cockpit busy thinking: its displays as they last were.)
                let v = match &self.cockpit {
                    Some(k) => match k.try_lock() {
                        Ok(c) => v.with_cockpit(Some(&c)),
                        Err(std::sync::TryLockError::Poisoned(p)) => v.with_cockpit(Some(&p.into_inner())),
                        Err(std::sync::TryLockError::WouldBlock) => v.with_cockpit_of(&self.view),
                    },
                    None => v,
                };
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
        if let Some(t) = self.link.take() {
            let _ = t.join();
        }
    }
}
