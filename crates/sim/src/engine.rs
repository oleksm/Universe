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
use universe_world::market::Quote;
use universe_world::turrets::Turret;
use universe_world::weapons::{Beam, Impact};
use universe_world::{Controls, Facility, Person, Ship, ShipCommands, StepResult, WalkCommands};

use crate::combat::Kill;
use crate::commerce::TradeRecord;
use crate::contacts::Contact;
use crate::follow::FollowKind;
use crate::traffic::TrafficStats;
use crate::universe::Universe;

/// What the client asks of the world.
pub enum Command {
    /// New settings for the ship's devices (engine, thrusters, weapons…).
    Ship(ShipCommands),
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
    pub avionics: Avionics,
    pub crew: Person,
    pub credits: f64,
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
    pub serial: u64,
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
}

impl Engine {
    pub fn new(universe: Universe) -> Self {
        let charts = Arc::new(universe.world.charts());
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
            .map(|c| CraftView {
                name: c.name.clone(),
                system: c.system,
                ship: c.ship.clone(),
                pirate: c.avionics.pirate,
                trader: c.trader,
                route_next: c.avionics.route.next,
                route_stops: c.avionics.route.stops.len(),
                stage: crate::contacts::activity(c),
            })
            .collect();
        let markets = universe_world::market::facilities(&sys).into_iter().map(|f| (f, f.name(&sys))).collect();
        let market = self.watched.map(|f| {
            let (quotes, banned) = u.market_quotes(f);
            let held = u.ship.hold.keys().copied().filter(|i| !quotes.iter().any(|q| q.offer.item == *i)).collect::<Vec<_>>();
            let held = held.into_iter().map(|i| (i, u.quote_for(f, i))).collect();
            MarketView { market: f, quotes, banned, held }
        });
        let pads = (0..sys.spaceports.len()).map(|p| u.world.traffic.owners(system, p)).collect();
        self.serial += 1;
        View {
            time: now,
            ship: u.ship.clone(),
            ship_system: system,
            avionics: u.avionics.clone(),
            crew: u.crew,
            credits: u.credits,
            crafts: Arc::new(crafts),
            traffic: u.traffic,
            kills: u.kills.clone(),
            trade_log: u.trade_log.clone(),
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
            serial: self.serial,
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

/// The client's handle on the world engine: send commands, tick, read the
/// latest view. (For now the engine runs here, in the client's thread.)
pub struct EngineHandle {
    engine: Engine,
    view: Arc<View>,
}

impl EngineHandle {
    pub fn new(universe: Universe) -> Self {
        let mut engine = Engine::new(universe);
        let view = Arc::new(engine.view());
        EngineHandle { engine, view }
    }

    pub fn charts(&self) -> Arc<Charts> {
        self.engine.charts()
    }

    pub fn send(&mut self, c: Command) {
        self.engine.apply(c);
    }

    /// Run the world for a frame and take the new view.
    pub fn tick(&mut self, real_dt: f64, warp: f64, controls: &Controls) {
        self.engine.tick(real_dt, warp, controls);
        self.view = Arc::new(self.engine.view());
    }

    /// Refresh the view without running the world (after setting things up).
    pub fn refresh(&mut self) {
        self.view = Arc::new(self.engine.view());
    }

    pub fn view(&self) -> Arc<View> {
        self.view.clone()
    }

    /// The universe itself, for setting up (dev scenarios, save and load).
    pub fn universe(&mut self) -> &mut Universe {
        &mut self.engine.universe
    }
}
