mod dev;
mod fmt;
mod hud;
mod market;
mod models;
mod navmap;
mod observer;
mod onfoot;
mod save;
mod scene;
mod sound;
mod terrain_view;

use std::rc::Rc;

use serde::{Deserialize, Serialize};
use universe_engine::glam::DVec3;
use universe_engine::{run, Camera, Config, Context, Frame, Game, KeyCode, MouseButton};
use universe_sim::world::crew::Reach;
use universe_sim::world::{CrewEvent, Triggers, WalkCommands};
use universe_sim::{Approach, ClearanceKind, Controls, Event, ShipCommands, ShipEvent, ShipState, StarSystem, StepResult, TrafficEvent, Universe};

use models::Models;
use observer::{Focus, Observer};

const SEED: u64 = 1984;
const WARPS: [f64; 8] = [1.0, 10.0, 100.0, 1e3, 1e4, 1e5, 1e6, 1e7];
/// Where a hit landed, shown as a spark for a moment.
pub struct Spark {
    pub system: usize,
    pub point: DVec3,
    /// Seconds since.
    pub age: f32,
    pub laser: bool,
    /// Our round or beam (and whose ship it struck).
    pub ours: bool,
    pub target: usize,
}

/// Game seconds per real second, by default.
const DEFAULT_TIME_SCALE: f64 = 1.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Mode {
    Observer,
    Pilot,
}

/// What is being rendered this frame: one star system, in its star's frame.
pub struct View {
    /// Galaxy index of the system at the origin.
    pub origin: usize,
    pub system: Rc<StarSystem>,
    /// Body positions at the current time, relative to the origin star.
    pub positions: Vec<DVec3>,
    /// Ship position in the origin frame.
    pub ship_pos: DVec3,
    /// Dominant body near the ship, if the ship is in this system.
    pub reference: Option<usize>,
}

pub struct Message {
    pub text: String,
    pub ttl: f32,
}

pub struct App {
    pub u: Universe,
    pub mode: Mode,
    pub observer: Observer,
    pub chase_cam: bool,
    pub warp_index: usize,
    /// The world's base time scale (game seconds per real second). The same
    /// for everyone in a shared world; set with UNIVERSE_TIME_SCALE (1-100).
    pub time_scale: f64,
    pub paused: bool,
    pub last_step: StepResult,
    pub show_help: bool,
    pub show_orbits: bool,
    pub show_labels: bool,
    pub muted: bool,
    pub messages: Vec<Message>,
    pub view: View,
    /// Docking or landing guidance for the HUD, when cleared.
    pub approach: Option<Approach>,
    /// The flight plan to the cleared target: the path, attitudes and actions ahead.
    pub plan: Option<universe_sim::Plan>,
    /// When the plan was last rebuilt (real seconds), for which clearance,
    /// and how long building it took (real seconds).
    plan_age: f32,
    plan_for: Option<universe_sim::Clearance>,
    pub plan_cost: f32,
    /// Time the world tick took (ms, smoothed): every ship's turn.
    pub sim_ms: f32,
    /// The ETA shown on the HUD (real seconds): counts down each frame and
    /// eases toward each new plan's prediction instead of jumping.
    pub eta_shown: Option<f64>,
    /// Colored terrain globes, built once per (system, body).
    pub globes: std::collections::HashMap<(usize, usize), universe_engine::WireModel>,
    /// The navigation map, when open.
    pub nav_map: Option<navmap::NavMap>,
    /// The market screen, when open; and whether we're docked at a market.
    pub market: Option<market::MarketView>,
    pub docked_market: bool,
    /// What the target marker points at: the nav target, else the nearest station.
    pub nav_marker: Option<(String, DVec3)>,
    /// Ships on the radar, nearest first (refreshed every frame).
    pub contacts: Vec<universe_sim::Contact>,
    /// Fire control on the locked contact: the track, and the gun's lead once it has one.
    pub fire: Option<(universe_sim::avionics::Track, Option<universe_sim::avionics::Solution>)>,
    /// Seconds since the ship was last hit (for the HUD's flash).
    pub hit_age: f32,
    /// Recent hits, for their sparks.
    pub sparks: Vec<Spark>,
    /// On foot: what's in reach to use.
    pub reach: Option<Reach>,
    /// Seconds left to show the lock beam's ring (after T, outside combat mode).
    pub beam_shown: f32,
    /// The collision warning's prediction, when it's on (made at `collision_at`, world time).
    pub collision: Option<universe_sim::avionics::collision::Prediction>,
    pub collision_at: f64,
    collision_age: f32,
    /// What the last prediction cost (s of real time).
    pub collision_cost: f32,
    /// The weapon keys as last sent to the ship (commands go on a change).
    triggers_held: Triggers,
    /// Display names of the route's stops (refreshed when the route changes).
    pub route_labels: Vec<String>,
    route_labels_for: Vec<universe_sim::Stop>,
    pub camera: Camera,
    pub models: Models,
    prev_focus: Option<(usize, DVec3)>,
    /// Seconds into `UNIVERSE_SOUND_TEST`, if running.
    sound_test: Option<f64>,
    launched: bool,
}

impl App {
    fn new() -> Self {
        let mut u = Universe::new(SEED);
        // Traffic: reproducible settlers (UNIVERSE_SETTLERS, default 1,000).
        let settlers = std::env::var("UNIVERSE_SETTLERS").ok().and_then(|v| v.parse().ok()).unwrap_or(1_000);
        u.spawn_settlers(settlers, SEED);
        let system = u.ship_system();
        let origin = u.ship_system;
        let mut app = Self {
            u,
            mode: Mode::Pilot,
            observer: Observer::new(),
            chase_cam: true,
            warp_index: 0,
            time_scale: std::env::var("UNIVERSE_TIME_SCALE")
                .ok()
                .and_then(|v| v.parse::<f64>().ok())
                .map_or(DEFAULT_TIME_SCALE, |v| v.clamp(1.0, 100.0)),
            paused: false,
            last_step: StepResult::default(),
            show_help: false,
            show_orbits: true,
            show_labels: true,
            muted: false,
            messages: Vec::new(),
            view: View { origin, system, positions: Vec::new(), ship_pos: DVec3::ZERO, reference: None },
            approach: None,
            plan: None,
            plan_age: 0.0,
            plan_for: None,
            plan_cost: 0.0,
            sim_ms: 0.0,
            eta_shown: None,
            globes: std::collections::HashMap::new(),
            nav_map: None,
            market: None,
            docked_market: false,
            nav_marker: None,
            contacts: Vec::new(),
            fire: None,
            hit_age: 99.0,
            sparks: Vec::new(),
            reach: None,
            beam_shown: 0.0,
            collision: None,
            collision_at: 0.0,
            collision_age: 99.0,
            collision_cost: 0.0,
            triggers_held: Triggers::default(),
            route_labels: Vec::new(),
            route_labels_for: Vec::new(),
            camera: Camera::default(),
            models: Models::new(),
            prev_focus: None,
            sound_test: std::env::var_os("UNIVERSE_SOUND_TEST").map(|_| 0.0),
            launched: false,
        };
        let name = app.view.system.bodies[app.view.system.station().unwrap_or(0)].name.clone();
        app.say(format!("LAUNCHED FROM {}", name.to_uppercase()));
        app.say("PRESS F1 FOR CONTROLS".into());
        if let Ok(name) = std::env::var("UNIVERSE_SCENARIO") {
            dev::apply(&mut app, &name);
        }
        app
    }

    pub fn say(&mut self, text: String) {
        self.messages.push(Message { text, ttl: 4.0 });
        if self.messages.len() > 4 {
            self.messages.remove(0);
        }
    }

    /// How fast game time runs relative to real time: the world's time scale,
    /// times any single-player warp on top (no warp in hyperdrive).
    pub fn warp(&self) -> f64 {
        if self.paused {
            0.0
        } else if self.u.ship.hyperdrive {
            self.time_scale
        } else {
            self.time_scale * WARPS[self.warp_index]
        }
    }

    fn global_keys(&mut self, ctx: &mut Context) {
        let input = &ctx.input;
        if input.pressed(KeyCode::Tab) {
            self.mode = match self.mode {
                Mode::Observer => Mode::Pilot,
                Mode::Pilot => {
                    ctx.grab_cursor(false);
                    self.observer.focus = Focus::Ship;
                    self.observer.distance = 180.0;
                    Mode::Observer
                }
            };
            sound::click(ctx, 500.0);
        }
        let input = &ctx.input;
        if input.pressed(KeyCode::Period) || input.pressed(KeyCode::Equal) {
            self.warp_index = (self.warp_index + 1).min(WARPS.len() - 1);
            sound::click(ctx, 600.0 + 150.0 * self.warp_index as f32);
        }
        if input.pressed(KeyCode::Comma) || input.pressed(KeyCode::Minus) {
            self.warp_index = self.warp_index.saturating_sub(1);
            sound::click(ctx, 600.0 + 150.0 * self.warp_index as f32);
        }
        if input.pressed(KeyCode::KeyP) {
            self.paused = !self.paused;
            sound::click(ctx, 400.0);
        }
        if input.pressed(KeyCode::F1) {
            self.show_help = !self.show_help;
        }
        if input.pressed(KeyCode::KeyO) {
            self.show_orbits = !self.show_orbits;
        }
        if input.pressed(KeyCode::KeyL) {
            self.show_labels = !self.show_labels;
        }
        if input.pressed(KeyCode::F8) {
            self.muted = !self.muted;
            if let Some(a) = ctx.audio() {
                a.set_master(if self.muted { 0.0 } else { sound::VOLUME });
            }
        }
        if input.pressed(KeyCode::F5) {
            match save::save(self) {
                Ok(path) => self.say(format!("SAVED TO {}", path.display())),
                Err(e) => self.say(format!("SAVE FAILED: {e}")),
            }
            sound::chime(ctx);
        }
        if ctx.input.pressed(KeyCode::F9) {
            match save::load(self) {
                Ok(()) => self.say("GAME LOADED".into()),
                Err(e) => self.say(format!("LOAD FAILED: {e}")),
            }
            sound::chime(ctx);
        }
        if ctx.input.pressed(KeyCode::F12) {
            let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default();
            ctx.screenshot(format!("screenshots/{}.png", stamp.as_millis()));
        }
    }

    fn pilot_input(&mut self, ctx: &mut Context) -> Controls {
        if ctx.input.button_pressed(MouseButton::Left) {
            ctx.grab_cursor(true);
        }
        if ctx.input.pressed(KeyCode::Escape) {
            ctx.grab_cursor(false);
        }
        // On foot: walking, not flying (the ship flies on as last set).
        if !self.u.crew.seated() {
            let c = onfoot::commands(ctx);
            self.u.walk(&c, ctx.dt as f64);
            return Controls::default();
        }
        // F: out of the seat. Hands off the stick and the triggers.
        if ctx.input.pressed(KeyCode::KeyF) {
            if self.triggers_held != Triggers::default() {
                self.triggers_held = Triggers::default();
                self.u.command(&ShipCommands { weapons: Some(Triggers::default()), ..self.u.ship.holding() });
            }
            self.u.walk(&WalkCommands { interact: true, ..Default::default() }, ctx.dt as f64);
            return Controls::default();
        }
        let input = &ctx.input;
        let dt = ctx.dt as f64;

        if input.pressed(KeyCode::KeyC) {
            self.chase_cam = !self.chase_cam;
        }
        if input.pressed(KeyCode::KeyJ) {
            self.u.toggle_hyperdrive();
        }
        if input.pressed(KeyCode::KeyR) {
            // R again gives the clearance up.
            if self.u.avionics.clearance.is_some() {
                self.u.cancel_clearance();
            } else {
                self.u.request_clearance();
            }
        }
        if input.pressed(KeyCode::KeyK) {
            // With a route set, K flies the whole route; otherwise the current clearance.
            if self.u.avionics.route.stops.is_empty() {
                self.u.toggle_autopilot();
            } else {
                self.u.toggle_route();
            }
        }
        if input.pressed(KeyCode::Backspace) {
            self.u.respawn();
        }
        // Weapons: SPACE the gun, V the laser, while held (the autopilot
        // doesn't hold them back).
        if input.pressed(KeyCode::KeyB) {
            self.u.command(&ShipCommands { arm: Some(!self.u.ship.armed), ..self.u.ship.holding() });
        }
        if !self.u.ship.armed && (input.pressed(KeyCode::Space) || input.pressed(KeyCode::KeyV)) {
            self.say("WEAPONS SAFE - B FOR COMBAT MODE".into());
        }
        let triggers = Triggers { gun: input.down(KeyCode::Space), laser: input.down(KeyCode::KeyV) };
        if triggers != self.triggers_held {
            self.triggers_held = triggers;
            self.u.command(&ShipCommands { weapons: Some(triggers), ..self.u.ship.holding() });
        }
        if input.pressed(KeyCode::KeyI) {
            let on = !self.u.avionics.collision_warning;
            self.u.avionics.collision_warning = on;
            self.collision_age = 99.0;
            self.say(if on { "COLLISION WARNING ON" } else { "COLLISION WARNING OFF" }.into());
        }
        if input.pressed(KeyCode::KeyT) {
            // Lock what's in the beam around the crosshair (the ring shows it for a moment).
            let had = self.u.avionics.contact.is_some();
            self.beam_shown = 1.5;
            match self.u.lock_in_beam() {
                Some(c) => {
                    sound::click(ctx, 1200.0);
                    self.say(format!("LOCKED: {}", c.name));
                }
                None if had => self.say("LOCK RELEASED".into()),
                None => self.say("NO TARGET IN THE BEAM - PUT IT IN THE RING".into()),
            }
        }
        // The autopilot has the stick.
        if self.u.avionics.route.active || self.u.avionics.clearance.is_some_and(|c| c.autopilot) {
            return Controls::default();
        }

        // Shift turns W/S/A/D/Q/E into translation thrusters (RCS).
        let shift = input.down(KeyCode::ShiftLeft) || input.down(KeyCode::ShiftRight);
        let mut command = self.u.ship.holding();
        if shift {
            command.rcs = DVec3::new(
                input.axis(KeyCode::KeyA, KeyCode::KeyD) as f64,
                input.axis(KeyCode::KeyQ, KeyCode::KeyE) as f64,
                input.axis(KeyCode::KeyW, KeyCode::KeyS) as f64,
            );
        } else {
            command.rcs = DVec3::ZERO;
            command.throttle += input.axis(KeyCode::KeyS, KeyCode::KeyW) as f64 * 0.6 * dt;
        }
        if input.pressed(KeyCode::KeyZ) {
            command.throttle = 1.0;
        }
        if input.pressed(KeyCode::KeyX) {
            command.throttle = 0.0;
        }
        command.throttle = command.throttle.clamp(0.0, 1.0);
        self.u.command(&command);

        let keys: f32 = if shift { 0.0 } else { 1.0 };
        let mut c = Controls {
            pitch: input.axis(KeyCode::ArrowUp, KeyCode::ArrowDown) as f64,
            yaw: (input.axis(KeyCode::KeyE, KeyCode::KeyQ) * keys) as f64,
            roll: (input.axis(KeyCode::KeyD, KeyCode::KeyA) * keys + input.axis(KeyCode::ArrowRight, KeyCode::ArrowLeft))
                .clamp(-1.0, 1.0) as f64,
        };
        if ctx.cursor_grabbed() {
            c.pitch = (c.pitch - input.mouse_delta.y as f64 * 0.08).clamp(-1.0, 1.0);
            c.yaw = (c.yaw - input.mouse_delta.x as f64 * 0.08).clamp(-1.0, 1.0);
        }
        c
    }

    fn handle_events(&mut self, ctx: &Context) {
        for event in std::mem::take(&mut self.u.events) {
            sound::event(ctx, &event);
            // Hits show on the HUD (hull, flash), not as messages: a burst would flood them.
            if let Event::Ship(ShipEvent::Hit { .. }) = event {
                self.hit_age = 0.0;
                continue;
            }
            let text = match event {
                Event::Ship(ShipEvent::Landed { body, station: true }) => format!("DOCKED AT {body}"),
                Event::Ship(ShipEvent::Landed { body, station: false }) => format!("LANDED ON {body}"),
                Event::Ship(ShipEvent::TookOff) => "LIFT OFF".into(),
                Event::Ship(ShipEvent::Crashed { body }) => format!("SHIP DESTROYED - {body}"),
                Event::Ship(ShipEvent::Respawned) => "NEW SHIP DELIVERED TO HOME STATION".into(),
                Event::Ship(ShipEvent::EnteredSystem { name }) => format!("ENTERING {name} SYSTEM"),
                Event::Ship(ShipEvent::HyperdriveEngaged) => "HYPERDRIVE ENGAGED".into(),
                Event::Ship(ShipEvent::HyperdriveDisengaged) => "HYPERDRIVE OFF".into(),
                Event::HyperdriveArrived { target } => format!("ARRIVED AT {target}\nR TO REQUEST CLEARANCE"),
                Event::Traffic(TrafficEvent::ClearanceGranted { target, kind: ClearanceKind::Dock }) => {
                    format!("DOCKING GRANTED - {target}\nFOLLOW THE GATES, OR K FOR AUTO")
                }
                Event::Traffic(TrafficEvent::ClearanceGranted { target, kind: ClearanceKind::Land }) => {
                    format!("LANDING GRANTED - {target}\nFOLLOW THE PATH, OR K FOR AUTO")
                }
                Event::Traffic(TrafficEvent::ClearanceGranted { target, kind: ClearanceKind::Transit }) => {
                    format!("TRANSIT GRANTED - {target}\nFLY THROUGH THE RING UNDER 300 M/S, OR K FOR AUTO")
                }
                Event::Ship(ShipEvent::GateEntered { to }) => format!("GATE TRANSIT TO {to}"),
                Event::Ship(ShipEvent::GateArrived { system }) => format!("WELCOME TO THE {system} SYSTEM"),
                Event::Ship(ShipEvent::GateTooFast { speed }) => format!("TOO FAST FOR THE GATE ({:.0} M/S)", speed),
                Event::Traffic(TrafficEvent::ClearanceDenied { reason }) | Event::Refused { reason } => format!("CLEARANCE DENIED - {reason}"),
                Event::Traffic(TrafficEvent::ClearanceCancelled) => "CLEARANCE CANCELLED".into(),
                Event::Traffic(TrafficEvent::PadAssigned { pad }) => format!("LAND ON PAD {}", pad + 1),
                Event::Traffic(TrafficEvent::Holding { ahead }) => format!("ALL PADS TAKEN - HOLD OVER THE PORT ({ahead} AHEAD)"),
                Event::Autopilot { on: true } => "AUTOPILOT ON".into(),
                Event::Autopilot { on: false } => "AUTOPILOT OFF".into(),
                Event::NavTargetSet { name: Some(name) } => format!("NAV TARGET - {name}\nR TO REQUEST CLEARANCE"),
                Event::NavTargetSet { name: None } => "NAV TARGET CLEARED".into(),
                Event::Ship(ShipEvent::LandedAtPort { port }) => format!("TOUCHDOWN - WELCOME TO {port}"),
                Event::RouteStop { number, name } => format!("ROUTE STOP {number} - {name}"),
                Event::RouteComplete => "ROUTE COMPLETE".into(),
                Event::RouteBlocked { reason } => format!("ROUTE STOPPED - {reason}"),
                Event::Ship(ShipEvent::Bumped) => "HULL CONTACT!".into(),
                Event::Ship(ShipEvent::Launched { station }) => format!("LAUNCHED FROM {station}"),
                Event::Ship(ShipEvent::Hit { .. }) => continue,
                Event::Ship(ShipEvent::Collided { with, speed }) => format!("COLLISION WITH {} AT {speed:.1} M/S", self.u.ship_name(with)),
                Event::Ship(ShipEvent::WeaponsArming) => "COMBAT MODE - WEAPONS ARMING".into(),
                Event::Ship(ShipEvent::WeaponsHot) => "WEAPONS HOT".into(),
                Event::Ship(ShipEvent::WeaponsSafe) => "WEAPONS SAFE".into(),
                Event::Crew(CrewEvent::StoodUp) => "OUT OF THE SEAT - THE SHIP FLIES ON AS SET".into(),
                Event::Crew(CrewEvent::SatDown) => "IN THE PILOT'S SEAT".into(),
                Event::Crew(CrewEvent::SteppedOutside { body }) => format!("STEPPED OUT ONTO {body}"),
                Event::Crew(CrewEvent::CameAboard) => "BACK ABOARD".into(),
                Event::Crew(CrewEvent::HatchRefused { reason }) => format!("HATCH LOCKED - {reason}"),
            };
            self.say(text.to_uppercase());
        }
    }

    /// Terrain globes for the system in view (built once, on first sight).
    fn build_globes(&mut self) {
        let origin = self.view.origin;
        for (i, b) in self.view.system.bodies.iter().enumerate() {
            if !self.globes.contains_key(&(origin, i))
                && let Some(g) = terrain_view::globe(b)
            {
                self.globes.insert((origin, i), g);
            }
        }
    }

    fn find_nav_marker(&mut self) -> Option<(String, DVec3)> {
        if let Some(t) = self.u.avionics.nav_target {
            let pos = self.u.target_position(t)?;
            let name = match t {
                universe_sim::NavTarget::Station(_) | universe_sim::NavTarget::Gate(_) => self.u.target_name(t),
                universe_sim::NavTarget::Spaceport(p) => self.u.ship_system().spaceports[p].name.clone(),
            };
            return Some((name.to_uppercase(), pos));
        }
        let sys = self.u.ship_system();
        let station = sys.station()?;
        let mut positions = Vec::new();
        sys.positions(self.u.world.time, &mut positions);
        Some((String::new(), positions[station]))
    }

    fn build_view(&mut self) {
        let origin = match self.mode {
            Mode::Pilot => self.u.ship_system,
            Mode::Observer => self.observer.origin(&self.u),
        };
        let system = self.u.system(origin);
        let mut positions = std::mem::take(&mut self.view.positions);
        system.positions(self.u.world.time, &mut positions);
        let ship_pos = self.u.ship.position + self.u.world.galaxy.offset(origin, self.u.ship_system);
        let reference = (origin == self.u.ship_system).then(|| system.dominant(ship_pos, &positions));
        self.view = View { origin, system, positions, ship_pos, reference };
    }

    fn focus_position(&self) -> DVec3 {
        match self.observer.focus {
            Focus::Body { body, .. } => self.view.positions[body],
            Focus::Ship => self.view.ship_pos,
            Focus::Craft(i) => self.u.crafts.get(i).map_or(DVec3::ZERO, |c| c.ship.position),
        }
    }

    fn update_camera(&mut self, dt: f64, focus_changed: bool) {
        match self.mode {
            Mode::Observer => {
                let focus = self.focus_position();
                if let (true, Some((prev_origin, prev_pos))) = (focus_changed, self.prev_focus) {
                    // Glide from the old target: express it in the new frame first.
                    let old = prev_pos + self.observer.transition + self.u.world.galaxy.offset(self.view.origin, prev_origin);
                    self.observer.transition = old - focus;
                }
                self.observer.transition *= (-4.0 * dt).exp();
                self.prev_focus = Some((self.view.origin, focus));
                self.camera = self.observer.camera(focus);
            }
            Mode::Pilot if !self.u.crew.seated() => {
                let (position, orientation) = self.u.pilot_eye(self.view.ship_pos);
                self.camera = Camera { position, orientation: orientation.as_quat(), near: 0.05, ..Default::default() };
                self.prev_focus = None;
            }
            Mode::Pilot => {
                let ship = &self.u.ship;
                let orientation = ship.orientation.as_quat();
                let docked = matches!(ship.state, ShipState::Landed { body, .. } if self.view.system.bodies[body].kind == universe_sim::BodyKind::Station);
                // Docked: the ship is inside the slot, so back off far enough to see the station.
                let chase = if docked { DVec3::new(0.0, 150.0, 1800.0) } else { DVec3::new(0.0, 20.0, 115.0) };
                let offset = if self.chase_cam || docked { ship.orientation * chase } else { DVec3::ZERO };
                self.camera = Camera { position: self.view.ship_pos + offset, orientation, near: 0.5, ..Default::default() };
                self.prev_focus = None;
            }
        }
    }
}

impl Game for App {
    fn update(&mut self, ctx: &mut Context) {
        let dt = ctx.dt as f64;
        if !self.launched {
            self.launched = true;
            sound::launch(ctx);
        }
        self.global_keys(ctx);

        // The navigation map takes the keyboard while it's open.
        let map_was_open = self.nav_map.is_some();
        if map_was_open {
            navmap::input(self, ctx);
        } else if ctx.input.pressed(KeyCode::KeyM) {
            self.nav_map = Some(navmap::NavMap::open(self));
            sound::click(ctx, 900.0);
        }
        // So does the market (G, from the pilot's seat).
        let market_was_open = self.market.is_some();
        if market_was_open {
            market::input(self, ctx);
        } else if !map_was_open && self.nav_map.is_none() && self.mode == Mode::Pilot && self.u.crew.seated() && ctx.input.pressed(KeyCode::KeyG) {
            self.market = Some(market::MarketView::open(self));
            sound::click(ctx, 900.0);
        }
        let (controls, focus_changed) = match self.mode {
            _ if map_was_open || self.nav_map.is_some() || market_was_open || self.market.is_some() => (Controls::default(), false),
            Mode::Pilot => (self.pilot_input(ctx), false),
            Mode::Observer => (Controls::default(), self.observer.input(ctx, &mut self.u)),
        };
        if focus_changed {
            sound::click(ctx, 1400.0);
        }
        let tick = std::time::Instant::now();
        let ammo = self.u.ship.ammo;
        self.last_step = self.u.step_world(dt, self.warp(), &controls);
        if self.u.ship.ammo < ammo {
            sound::gunshot(ctx);
        }
        // Sparks where hits landed; a tick when ours do.
        for s in &mut self.sparks {
            s.age += ctx.dt;
        }
        self.sparks.retain(|s| s.age < hud::SPARK_TIME);
        let mut ours = false;
        for i in &self.u.world.impacts {
            let mine = i.by == universe_sim::PLAYER;
            ours |= mine && !i.laser;
            if self.sparks.len() < 200 {
                self.sparks.push(Spark { system: i.system, point: i.point, age: 0.0, laser: i.laser, ours: mine, target: i.target });
            }
        }
        if ours {
            sound::hit_confirmed(ctx);
        }
        let ms = tick.elapsed().as_secs_f32() * 1000.0;
        self.sim_ms = if self.sim_ms == 0.0 { ms } else { self.sim_ms + (ms - self.sim_ms) * 0.05 };
        self.handle_events(ctx);
        self.build_globes();
        if self.route_labels_for != self.u.avionics.route.stops {
            self.route_labels_for = self.u.avionics.route.stops.clone();
            self.route_labels = self.route_labels_for.clone().into_iter().map(|s| self.u.stop_name(s).to_uppercase()).collect();
        }
        self.approach = self.u.approach();
        // The planner flies the autopilot ahead through the physics, which
        // takes 1-2 ms near the target and ~10 ms for a landing from orbit:
        // rebuild it 10 times a second, less often when it's dearer (keeping
        // it to ~5% of the time, the countdown easing in between), or at once
        // when the clearance, target or autopilot phase changes.
        self.plan_age += ctx.dt;
        let key = self.u.avionics.clearance;
        let changed = key.map(|c| (c.target, c.autopilot, c.phase)) != self.plan_for.map(|c| (c.target, c.autopilot, c.phase));
        if changed || self.plan_age >= (self.plan_cost * 20.0).clamp(0.1, 1.0) || !self.u.ship.is_flying() {
            let start = std::time::Instant::now();
            self.plan = self.u.plan();
            self.plan_cost = start.elapsed().as_secs_f32();
            self.plan_age = 0.0;
            self.plan_for = key;
        }
        let raw = self.plan.as_ref().filter(|p| p.arrives).map(|p| {
            let left = p.points.last().map_or(0.0, |x| x.time) - (self.u.world.time - p.start);
            left / self.warp().max(1.0)
        });
        self.eta_shown = match (raw, self.eta_shown) {
            (Some(raw), Some(shown)) if (raw - shown).abs() < 10.0 => {
                let ticked = shown - ctx.dt as f64;
                Some(ticked + (raw - ticked) * (ctx.dt as f64 * 2.0).min(1.0))
            }
            (raw, _) => raw,
        };
        self.nav_marker = self.find_nav_marker();
        self.reach = self.u.pilot_reach();
        self.docked_market = self.u.docked_market().is_some();
        self.contacts = self.u.contacts();
        if self.u.avionics.contact.is_some() && self.u.locked_contact_in(&self.contacts).is_none() {
            self.u.avionics.contact = None;
            self.say("RADAR CONTACT LOST".into());
        }
        let contacts = std::mem::take(&mut self.contacts);
        self.fire = self.u.fire_control(&contacts);
        // The collision warning, five times a second.
        self.collision_age += ctx.dt;
        if !self.u.avionics.collision_warning {
            self.collision = None;
        } else if self.collision_age >= 0.2 {
            self.collision_age = 0.0;
            let start = std::time::Instant::now();
            self.collision = self.u.collision_warning(&contacts);
            self.collision_cost = start.elapsed().as_secs_f32();
            self.collision_at = self.u.world.time;
        }
        self.contacts = contacts;
        self.hit_age += ctx.dt;
        self.beam_shown = (self.beam_shown - ctx.dt).max(0.0);
        self.build_view();
        self.update_camera(dt, focus_changed);
        if let Some(t) = self.sound_test {
            let next = t + dt;
            self.sound_test = dev::sound_test(ctx, next, t).then_some(next);
        } else {
            sound::update(ctx, self);
        }

        for m in &mut self.messages {
            m.ttl -= ctx.dt;
        }
        self.messages.retain(|m| m.ttl > 0.0);
    }

    fn camera(&self) -> Camera {
        self.camera
    }

    fn draw(&self, frame: &mut Frame, ctx: &Context) {
        scene::draw(frame, self);
        hud::draw(frame, self, ctx);
    }
}

/// True when the ship is visible as a model rather than being the camera.
pub fn ship_visible(app: &App) -> bool {
    let view = match app.u.crew.place {
        _ if app.mode == Mode::Observer => true,
        universe_sim::world::Place::Seat => app.chase_cam,
        universe_sim::world::Place::Aboard { .. } => false, // the interior instead
        universe_sim::world::Place::Outside { .. } => true,
    };
    !matches!(app.u.ship.state, ShipState::Destroyed { .. } | ShipState::Transit { .. }) && view
}

fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info,wgpu_core=warn,wgpu_hal=warn"))
        .init();
    run(Config { title: "universe".into(), ..Default::default() }, App::new());
}
