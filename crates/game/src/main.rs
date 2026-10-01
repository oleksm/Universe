mod dev;
mod economy;
mod fmt;
mod followguide;
mod galaxymap;
mod hud;
mod market;
mod models;
mod navmap;
mod observer;
mod keys;
mod lock;
mod mining;
mod onfoot;
mod rig;
mod rocks;
mod save;
mod scene;
mod sound;
mod terrain_view;

use std::sync::Arc;

use serde::{Deserialize, Serialize};
use universe_engine::glam::DVec3;
use universe_engine::{run, Camera, Config, Context, Frame, Game, KeyCode, MouseButton};
use universe_sim::world::crew::Reach;
use universe_sim::world::{CrewEvent, Triggers, WalkCommands};
use universe_sim::world::charts::Charts;
use universe_sim::{Approach, ClearanceKind, Command, Controls, EngineHandle, Event, FollowKind, ShipCommands, ShipEvent, ShipState, StarSystem, StepResult, TrafficEvent, Universe};

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
    pub system: Arc<StarSystem>,
    /// Body positions at the current time, relative to the origin star.
    pub positions: Vec<DVec3>,
    /// Ship position in the origin frame.
    pub ship_pos: DVec3,
    /// Dominant body near the ship, if the ship is in this system.
    pub reference: Option<usize>,
}

/// Whose motion to draw: ours, or craft `i`'s.
#[derive(Clone, Copy, Debug)]
pub enum Who {
    Me,
    Craft(usize),
}

pub struct Message {
    pub text: String,
    pub ttl: f32,
}

pub struct App {
    /// The world engine (the client sends it commands), its latest view of
    /// the world, and the galaxy's charts.
    pub engine: EngineHandle,
    pub v: Arc<universe_sim::View>,
    /// The view before it: the client draws between the two (see `alpha`),
    /// and how far between, for this frame.
    pub prev: Arc<universe_sim::View>,
    alpha: f64,
    /// Our ship as drawn this frame: the view's, at the moment drawn (see
    /// `alpha`). Everything drawn of or from our ship (hull, camera, HUD)
    /// reads this, never `v.ship`, so none of it slides against the rest.
    pub ship: universe_sim::Ship,
    pub charts: Arc<Charts>,
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
    /// Grid lines: orbit trajectories, planets' latitude/longitude grids, the ground grid's lines (O).
    pub show_grid: bool,
    pub show_labels: bool,
    pub muted: bool,
    pub messages: Vec<Message>,
    pub view: View,
    /// Docking or landing guidance for the HUD, when cleared.
    pub approach: Option<Approach>,
    /// The flight plan to the cleared target: the path, attitudes and actions ahead.
    pub plan: Option<Arc<universe_sim::Plan>>,
    /// The plan before it, and how far (0..1) the display has eased from it
    /// to `plan`: each rebuild starts from where the ship is, so the guide
    /// would otherwise jump a little at every one.
    pub plan_prev: Option<Arc<universe_sim::Plan>>,
    /// The guide frames, set in space along the plan (see `scene::Guide`).
    pub guide: crate::scene::Guide,
    pub plan_blend: f32,
    /// Real seconds since the plan was rebuilt, its serial, and how long
    /// building it took (real seconds) and how often it's rebuilt.
    plan_age: f32,
    plan_serial: u64,
    pub plan_cost: f32,
    pub plan_every: f32,
    /// Time the world tick took (ms, smoothed): every ship's turn.
    pub sim_ms: f32,
    /// The ETA shown on the HUD (real seconds): counts down each frame and
    /// eases toward each new plan's prediction instead of jumping.
    pub eta_shown: Option<f64>,
    /// Colored terrain globes, built once per (system, body): the full mesh,
    /// and a coarse one for when it's small on screen.
    pub globes: std::collections::HashMap<(usize, usize), (universe_engine::Mesh, universe_engine::Mesh)>,
    /// Asteroid meshes, built once per (system, field, body among the field's bodies).
    pub rocks: std::collections::HashMap<(usize, usize, usize), universe_engine::Mesh>,
    /// Mining rigs: how far each ship's gear is out (see `rig`).
    pub rigs: rig::Rigs,
    /// The galaxy's stars as seen from a system: (system, direction and colour of each).
    pub sky_cache: std::cell::RefCell<Option<SkyCache>>,
    /// The navigation map, when open.
    pub nav_map: Option<navmap::NavMap>,
    /// The economy panel, when open (5).
    pub economy_panel: Option<economy::EconomyPanel>,
    /// The galaxy map, when open (U from the navigation map).
    pub galaxy_map: Option<galaxymap::GalaxyMap>,
    /// The star systems we've been to (kept in the save).
    pub explored: std::collections::BTreeSet<usize>,
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
    /// Defence turrets of the system in view, where they are now.
    pub turrets: Vec<(universe_sim::world::turrets::Turret, DVec3)>,
    /// The follow program: how, what, and how far it is now (m).
    pub following: Option<(universe_sim::avionics::follow::Manoeuvre, String, f64)>,
    /// Seconds left to show the lock beam's ring (after T, outside combat mode).
    pub beam_shown: f32,
    /// The cargo panel is open (4).
    pub show_cargo: bool,
    /// Mining mode and the prospector's pulse; T's lock picker.
    pub mining: mining::Mining,
    pub picker: lock::Picker,
    /// The collision warning's prediction, when it's on (made at `collision_at`, world time).
    pub collision: Option<universe_sim::avionics::collision::Prediction>,
    pub collision_at: f64,
    /// What the last prediction cost (s of real time).
    pub collision_cost: f32,
    /// The weapon keys as last sent to the ship (commands go on a change).
    triggers_held: Triggers,
    /// Mouse movement since the world's last tick (the stick it gives holds
    /// for the whole tick, so the frames in between add up).
    mouse_since_tick: universe_engine::glam::Vec2,
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
        // UNIVERSE_RECORD=path: record the session from its start, saved there
        // on exit (replay it: `cargo run -p universe-sim --release --example replay -- path`).
        if std::env::var_os("UNIVERSE_RECORD").is_some() {
            u.record_inputs();
        }
        // Traffic: reproducible settlers (UNIVERSE_SETTLERS, default 1,000).
        let settlers = std::env::var("UNIVERSE_SETTLERS").ok().and_then(|v| v.parse().ok()).unwrap_or(1_000);
        u.spawn_settlers(settlers, SEED);
        let engine = EngineHandle::new(u);
        let (v, charts) = (engine.view(), engine.charts());
        let origin = v.ship_system;
        let system = charts.system(origin);
        let mut app = Self {
            engine,
            prev: v.clone(),
            alpha: 1.0,
            ship: v.ship.clone(),
            v,
            charts,
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
            show_grid: false,
            show_labels: true,
            muted: false,
            messages: Vec::new(),
            view: View { origin, system, positions: Vec::new(), ship_pos: DVec3::ZERO, reference: None },
            approach: None,
            plan: None,
            plan_age: 0.0,
            plan_serial: 0,
            plan_every: 0.1,
            plan_prev: None,
            guide: Default::default(),
            plan_blend: 1.0,
            plan_cost: 0.0,
            sim_ms: 0.0,
            eta_shown: None,
            globes: std::collections::HashMap::new(),
            rocks: std::collections::HashMap::new(),
            rigs: Default::default(),
            mining: Default::default(),
            picker: Default::default(),
            show_cargo: false,
            sky_cache: std::cell::RefCell::new(None),
            nav_map: None,
            galaxy_map: None,
            economy_panel: None,
            explored: Default::default(),
            market: None,
            docked_market: false,
            nav_marker: None,
            contacts: Vec::new(),
            fire: None,
            hit_age: 99.0,
            sparks: Vec::new(),
            reach: None,
            turrets: Vec::new(),
            following: None,
            beam_shown: 0.0,
            collision: None,
            collision_at: 0.0,
            collision_cost: 0.0,
            triggers_held: Triggers::default(),
            mouse_since_tick: universe_engine::glam::Vec2::ZERO,
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
            app.engine.refresh();
            app.v = app.engine.view();
        }
        // The world engine runs on its own thread (UNIVERSE_ENGINE_THREAD=0: here, for debugging).
        if std::env::var("UNIVERSE_ENGINE_THREAD").as_deref() != Ok("0") {
            app.engine.start();
        }
        app
    }

    pub fn say(&mut self, text: String) {
        self.messages.push(Message { text, ttl: 4.0 });
        if self.messages.len() > 4 {
            self.messages.remove(0);
        }
    }

    /// How far between the previous view and the latest to draw (0..1). The
    /// client draws one tick behind real time, between the two views made
    /// around then (by when the engine made them), so motion is smooth
    /// whatever the display's rate and however late a view is picked up. A
    /// view late in coming holds at the latest rather than guess ahead.
    pub fn alpha(&self) -> f64 {
        self.alpha
    }

    /// `alpha` as of now (taken once a frame: everything drawn that frame is
    /// drawn at the same moment).
    fn alpha_now(&self) -> f64 {
        if !self.engine.running() {
            return 1.0;
        }
        let span = self.v.made.saturating_duration_since(self.prev.made).as_secs_f64();
        if span <= 0.0 {
            return 1.0;
        }
        let behind = std::time::Instant::now() - std::time::Duration::from_secs_f64(1.0 / universe_sim::engine::TICK_HZ);
        let into = behind.saturating_duration_since(self.prev.made).as_secs_f64();
        (into / span).clamp(0.0, 1.0)
    }

    /// Our ship at the moment drawn: position and turn as `place`, velocity
    /// between the two views likewise.
    fn drawn_ship(&self) -> universe_sim::Ship {
        let (position, orientation) = self.place(Who::Me);
        let same = self.prev.ship_system == self.v.ship_system && self.prev.ship.position.distance(self.v.ship.position) < 20_000.0;
        let velocity = if same { self.prev.ship.velocity.lerp(self.v.ship.velocity, self.alpha) } else { self.v.ship.velocity };
        universe_sim::Ship { position, orientation, velocity, ..self.v.ship.clone() }
    }

    /// World time to draw at.
    pub fn now(&self) -> f64 {
        self.prev.time + (self.v.time - self.prev.time) * self.alpha()
    }

    /// Where a ship is to be drawn, and how it's turned: between the two
    /// views (unless it jumped between them: a new ship, a gate, another star).
    pub fn place(&self, who: Who) -> (DVec3, universe_engine::glam::DQuat) {
        let (now, before) = match who {
            Who::Me => (&self.v.ship, (self.prev.ship_system == self.v.ship_system).then_some(&self.prev.ship)),
            Who::Craft(i) => {
                let Some(c) = self.v.crafts.get(i) else { return (DVec3::ZERO, universe_engine::glam::DQuat::IDENTITY) };
                (&c.ship, self.prev.crafts.get(i).filter(|p| p.system == c.system).map(|p| &p.ship))
            }
        };
        let (position, orientation) = match before.filter(|b| b.position.distance(now.position) < 20_000.0) {
            Some(b) => {
                let a = self.alpha();
                (b.position.lerp(now.position, a), b.orientation.slerp(now.orientation, a))
            }
            None => (now.position, now.orientation),
        };
        // Our own ship: with what our orders on their way will do (the
        // cockpit's prediction), so the stick shows at once.
        match (who, self.v.prediction) {
            (Who::Me, Some((moved, turned))) => (position + moved, (turned * orientation).normalize()),
            _ => (position, orientation),
        }
    }

    /// How fast game time runs relative to real time: the world's time scale,
    /// times any single-player warp on top (no warp in hyperdrive).
    pub fn warp(&self) -> f64 {
        if self.paused {
            0.0
        } else if self.v.ship.hyperdrive {
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
        if input.pressed(KeyCode::F6) {
            self.paused = !self.paused;
            sound::click(ctx, 400.0);
        }
        if input.pressed(KeyCode::F1) {
            self.show_help = !self.show_help;
        }
        // F3: the profiler, and its panel.
        if input.pressed(KeyCode::F3) {
            universe_prof::enable(!universe_prof::enabled());
        }
        if input.pressed(KeyCode::F4) {
            self.show_grid = !self.show_grid;
        }
        if input.pressed(KeyCode::F2) {
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
        if !self.v.crew.seated() {
            let c = onfoot::commands(ctx);
            self.engine.send(Command::Walk(c, ctx.dt as f64));
            return Controls::default();
        }
        // Out of the seat. Hands off the stick and the triggers.
        if keys::pressed(&ctx.input, keys::Act::Foot) {
            if self.triggers_held != Triggers::default() {
                self.triggers_held = Triggers::default();
                self.engine.send(Command::Ship(ShipCommands { weapons: Some(Triggers::default()), ..self.v.ship.holding() }));
            }
            self.engine.send(Command::Walk(WalkCommands { interact: true, ..Default::default() }, ctx.dt as f64));
            return Controls::default();
        }
        let input = &ctx.input;
        let dt = ctx.dt as f64;

        use keys::{pressed, Act};
        use hud::ShipMode;
        let mode = hud::active_mode(self);
        if pressed(input, Act::View) {
            self.chase_cam = !self.chase_cam;
        }
        if pressed(input, Act::Hyperdrive) {
            self.engine.send(Command::ToggleHyperdrive);
        }
        if mode == ShipMode::Nav && pressed(input, Act::Clearance) {
            // Again gives the clearance up.
            self.engine.send(if self.v.avionics.clearance.is_some() { Command::CancelClearance } else { Command::RequestClearance });
        }
        if pressed(input, Act::Autopilot) {
            // With a route set, it flies the whole route; otherwise the current clearance.
            self.engine.send(if self.v.avionics.route.stops.is_empty() { Command::ToggleAutopilot } else { Command::ToggleRoute });
        }
        if input.pressed(KeyCode::Backspace) {
            self.engine.send(Command::Respawn);
        }
        // Weapons: SPACE the gun, V the laser, while held (the autopilot
        // doesn't hold them back).
        // B: combat mode (the master arm), or back to navigation. (One mode at a time.)
        if pressed(input, Act::Combat) {
            self.mining.on = false;
            self.engine.send(Command::Ship(ShipCommands { arm: Some(!self.v.ship.armed), ..self.v.ship.holding() }));
        }
        let laser = mode == ShipMode::Combat && keys::down(input, Act::Laser);
        if !self.v.ship.armed && input.pressed(KeyCode::Space) {
            self.say(format!("WEAPONS SAFE - {} FOR COMBAT MODE", keys::key(Act::Combat)));
        }
        let triggers = Triggers { gun: input.down(KeyCode::Space), laser };
        if triggers != self.triggers_held {
            self.triggers_held = triggers;
            self.engine.send(Command::Ship(ShipCommands { weapons: Some(triggers), ..self.v.ship.holding() }));
        }
        if mode == ShipMode::Nav && pressed(input, Act::Proximity) {
            let on = !self.v.avionics.collision_warning;
            self.engine.send(Command::CollisionWarning(on));
            self.say(if on { "COLLISION WARNING ON" } else { "COLLISION WARNING OFF" }.into());
        }
        // T: lock on (tap: what's ahead; hold: choose from the list).
        let listing = lock::input(self, ctx);
        mining::input(self, ctx);
        let input = &ctx.input;
        // N keeps at a range from the locked ship (or the nav target's
        // station or gate), U orbits it; again for the next range out. X lets go.
        if pressed(input, Act::Keep) {
            self.engine.send(Command::Follow(FollowKind::KeepAt));
        }
        if pressed(input, Act::Orbit) {
            self.engine.send(Command::Follow(FollowKind::Orbit));
        }
        // The cargo hold's contents.
        if pressed(input, Act::Cargo) {
            self.show_cargo = !self.show_cargo;
        }
        // Mining: the anchor (fire, or let go), the excavator (anchored).
        if mode == ShipMode::Mining && pressed(input, Act::Anchor) {
            let anchored = matches!(self.v.ship.state, ShipState::Anchored { .. });
            self.engine.send(Command::Ship(ShipCommands { anchor: Some(!anchored), ..self.v.ship.holding() }));
        }
        if mode == ShipMode::Mining && pressed(input, Act::Excavate) {
            self.engine.send(Command::Ship(ShipCommands { excavate: Some(!self.v.ship.excavator), ..self.v.ship.holding() }));
        }
        if pressed(input, Act::LetGo) && self.v.avionics.following.is_some() {
            self.engine.send(Command::StopFollowing);
        }
        // The autopilot has the stick.
        if self.v.avionics.route.active || self.v.avionics.clearance.is_some_and(|c| c.autopilot) || self.v.avionics.following.is_some() {
            return Controls::default();
        }

        // Shift turns W/S/A/D/Q/E into translation thrusters (RCS).
        let shift = input.down(KeyCode::ShiftLeft) || input.down(KeyCode::ShiftRight);
        let mut command = self.v.ship.holding();
        if shift {
            command.rcs = DVec3::new(
                input.axis(KeyCode::KeyA, KeyCode::KeyD) as f64,
                input.axis(KeyCode::KeyQ, KeyCode::KeyE) as f64,
                input.axis(KeyCode::KeyW, KeyCode::KeyS) as f64,
            );
        } else {
            command.rcs = DVec3::ZERO;
        }
        // The throttle as a change (the engine has the ship as it is now).
        let delta = if shift { 0.0 } else { input.axis(KeyCode::KeyS, KeyCode::KeyW) as f64 * 0.6 * dt };
        let set = None;
        if delta != 0.0 || set.is_some() {
            self.engine.send(Command::Throttle { delta, set });
        }
        self.engine.send(Command::Thrusters(command.rcs));

        let keys: f32 = if shift { 0.0 } else { 1.0 };
        let mut c = Controls {
            pitch: input.axis(KeyCode::ArrowUp, KeyCode::ArrowDown) as f64,
            yaw: (input.axis(KeyCode::KeyE, KeyCode::KeyQ) * keys) as f64,
            roll: (input.axis(KeyCode::KeyD, KeyCode::KeyA) * keys + input.axis(KeyCode::ArrowRight, KeyCode::ArrowLeft))
                .clamp(-1.0, 1.0) as f64,
        };
        // (Not while T's list has the mouse.)
        if ctx.cursor_grabbed() && !listing {
            let m = if self.engine.running() {
                self.mouse_since_tick += input.mouse_delta;
                self.mouse_since_tick
            } else {
                input.mouse_delta
            };
            c.pitch = (c.pitch - m.y as f64 * 0.08).clamp(-1.0, 1.0);
            c.yaw = (c.yaw - m.x as f64 * 0.08).clamp(-1.0, 1.0);
        }
        c
    }

    fn handle_events(&mut self, ctx: &Context) {
        let view = self.v.clone();
        for event in view.events.iter().cloned() {
            sound::event(ctx, &event);
            // Hits show on the HUD (hull, flash), not as messages: a burst would flood them.
            if let Event::Ship(ShipEvent::Hit { .. }) = event {
                self.hit_age = 0.0;
                continue;
            }
            // Docked or landed at a market: fill the tank (as any pilot does at a stop).
            if matches!(event, Event::Ship(ShipEvent::Landed { station: true, .. } | ShipEvent::LandedAtPort { .. })) {
                self.engine.send(Command::Refuel);
            }
            let text = match event {
                Event::Ship(ShipEvent::Landed { body, station: true }) => format!("DOCKED AT {body}"),
                Event::Refuelled { tonnes, credits } => format!("REFUELLED {tonnes:.1} T FOR {credits:.0} CR"),
                Event::Ship(ShipEvent::OutOfFuel) => "OUT OF FUEL - NO THRUST, NO HYPERDRIVE".into(),
                Event::Ship(ShipEvent::Landed { body, station: false }) => format!("LANDED ON {body}"),
                Event::Ship(ShipEvent::TookOff) => "LIFT OFF".into(),
                Event::Ship(ShipEvent::Crashed { body }) => format!("SHIP DESTROYED - {body}"),
                Event::Ship(ShipEvent::Respawned) => "NEW SHIP DELIVERED TO HOME STATION".into(),
                Event::Ship(ShipEvent::EnteredSystem { name }) => format!("ENTERING {name} SYSTEM"),
                Event::Ship(ShipEvent::HyperdriveEngaged) => "HYPERDRIVE ENGAGED".into(),
                Event::Ship(ShipEvent::HyperdriveDisengaged) => "HYPERDRIVE OFF".into(),
                Event::Ship(ShipEvent::HyperdriveJammed { seconds }) => format!("HYPERDRIVE JAMMED BY HITS - {seconds:.0} S"),
                Event::HyperdriveArrived { target } => format!("ARRIVED AT {target}\n{}", self.target_hint()),
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
                Event::Traffic(TrafficEvent::ClearanceDenied { reason }) => format!("CLEARANCE DENIED - {reason}"),
                Event::Refused { reason } => reason,
                Event::Following { what: Some((how, range)) } if how == "CLOSE ON" => format!("CLOSING ON THE ROCK, {range:.0} M OFF ITS SURFACE\nY TO ANCHOR WHEN IN REACH, X TO LET GO"),
                Event::Following { what: Some((how, range)) } => format!("{how} {:.0} KM - N/U AGAIN: NEXT RANGE, X: RELEASE", range / 1000.0),
                Event::Following { what: None } => "FOLLOW OFF".into(),
                Event::Traffic(TrafficEvent::ClearanceCancelled) => "CLEARANCE CANCELLED".into(),
                Event::Traffic(TrafficEvent::PadAssigned { pad }) => format!("LAND ON PAD {}", pad + 1),
                Event::Traffic(TrafficEvent::Holding { ahead }) => format!("ALL PADS TAKEN - HOLD OVER THE PORT ({ahead} AHEAD)"),
                Event::Autopilot { on: true } => "AUTOPILOT ON".into(),
                Event::Autopilot { on: false } => "AUTOPILOT OFF".into(),
                Event::NavTargetSet { name: Some(name) } => format!("NAV TARGET - {name}\n{}", self.target_hint()),
                Event::NavTargetSet { name: None } => "NAV TARGET CLEARED".into(),
                Event::Ship(ShipEvent::LandedAtPort { port }) => format!("TOUCHDOWN - WELCOME TO {port}"),
                Event::RouteStop { number, name } => format!("ROUTE STOP {number} - {name}"),
                Event::RouteComplete => "ROUTE COMPLETE".into(),
                Event::RouteBlocked { reason } => format!("ROUTE STOPPED - {reason}"),
                Event::Ship(ShipEvent::Bumped) => "HULL CONTACT!".into(),
                Event::Ship(ShipEvent::Launched { station }) => format!("LAUNCHED FROM {station}"),
                Event::Ship(ShipEvent::Collided { with, speed }) => format!("COLLISION WITH {} AT {speed:.1} M/S", self.ship_name(with)),
                Event::Lock { name: Some(name) } => {
                    sound::click(ctx, 1200.0);
                    format!("LOCKED: {name}")
                }
                Event::Lock { name: None } => "LOCK RELEASED".into(),
                Event::NothingInBeam => "NO TARGET IN THE BEAM - PUT IT IN THE RING".into(),
                Event::ContactLost => "RADAR CONTACT LOST".into(),
                Event::Traded { item, units, credits } if units > 0 => {
                    sound::click(ctx, 1500.0);
                    format!("BOUGHT {units} {item} FOR {credits:.0} CR")
                }
                Event::Traded { item, units, credits } => {
                    sound::click(ctx, 1500.0);
                    format!("SOLD {} {item} FOR {:.0} CR", -units, -credits)
                }
                Event::Ship(ShipEvent::Aggressed { .. }) => "AGGRESSION - YOU FIRED ON AN INNOCENT SHIP\nYOU ARE FAIR GAME FOR 10 MINUTES".into(),
                Event::Ship(ShipEvent::WeaponsArming) => "COMBAT MODE - WEAPONS ARMING".into(),
                Event::Ship(ShipEvent::WeaponsHot) => "WEAPONS HOT".into(),
                Event::Ship(ShipEvent::WeaponsSafe) => "WEAPONS SAFE".into(),
                Event::Crew(CrewEvent::StoodUp) => "OUT OF THE SEAT - THE SHIP FLIES ON AS SET".into(),
                Event::Crew(CrewEvent::SatDown) => "IN THE PILOT'S SEAT".into(),
                Event::Crew(CrewEvent::SteppedOutside { body }) => format!("STEPPED OUT ONTO {body}"),
                Event::Crew(CrewEvent::CameAboard) => "BACK ABOARD".into(),
                Event::Crew(CrewEvent::HatchRefused { reason }) => format!("HATCH LOCKED - {reason}"),
                Event::Ship(ShipEvent::Anchored { body }) => format!("ANCHORED TO {body}\nH TO DIG, Y TO LET GO"),
                Event::Ship(ShipEvent::AnchorFailed { why }) => format!("ANCHOR - {why}"),
                Event::Ship(ShipEvent::AnchorReleased) => "ANCHOR RELEASED".into(),
                Event::Ship(ShipEvent::ExcavatorStopped { why }) => format!("EXCAVATOR STOPPED - {why}"),
                Event::Ship(ShipEvent::Mined { item, .. }) => {
                    let name = self.charts.goods.get(item).map_or(String::new(), |g| g.name.clone());
                    let units = self.v.hold.iter().find(|h| h.0 == item).map_or(0, |h| h.1);
                    format!("+1 T {name} TO THE HOLD ({units} T IN ALL, HOLD {:.0}/{:.0} T)", self.v.ship.cargo / 1000.0, universe_sim::world::ship::HOLD_CAPACITY / 1000.0)
                }
                Event::Ship(ShipEvent::StruckRock { speed, .. }) => format!("ROCK STRIKE AT {speed:.1} M/S"),
                // Anything else says nothing (add a line here for a new event that should).
                _ => continue,
            };
            self.say(text.to_uppercase());
        }
    }

    /// What to do about the nav target.
    fn target_hint(&self) -> &'static str {
        match self.v.avionics.nav_target {
            Some(universe_sim::NavTarget::Asteroid(_)) => "N TO KEEP STATION, U TO ORBIT",
            _ => "R TO REQUEST CLEARANCE",
        }
    }

    /// Terrain globes for the system in view (built once, on first sight).
    fn build_globes(&mut self) {
        let origin = self.view.origin;
        for (i, b) in self.view.system.bodies.iter().enumerate() {
            if !self.globes.contains_key(&(origin, i))
                && let (Some(full), Some(coarse)) = (terrain_view::globe(b, 4), terrain_view::globe(b, 1))
            {
                self.globes.insert((origin, i), (full.into(), coarse.into()));
            }
        }
    }

    /// The nav target (or else the nearest station), where it is at the moment drawn.
    fn nav_marker_now(&self) -> Option<(String, DVec3)> {
        let (name, _) = self.v.nav_marker.clone()?;
        if self.view.origin != self.v.ship_system {
            return None;
        }
        let (sys, t) = (&self.view.system, self.now());
        let at = match self.v.avionics.nav_target {
            Some(target) => target.position(sys, t, &self.view.positions)?,
            None => self.view.positions[sys.station()?],
        };
        Some((name, at))
    }

    /// The approach guidance (docking, landing, gate run) for our ship as
    /// drawn, at the moment drawn.
    fn approach_now(&self) -> Option<Approach> {
        if self.view.origin != self.v.ship_system {
            return None;
        }
        self.v.avionics.approach(&self.view.system, &self.ship, self.now(), &self.view.positions)
    }

    /// A ship's name by its combat id.
    fn ship_name(&self, id: usize) -> String {
        match id {
            universe_sim::PLAYER => "YOU".into(),
            _ if universe_sim::world::turrets::turret_of(id).is_some() => "SAM TURRET".into(),
            _ => self.v.crafts.get(id - 1).map_or_else(|| "UNKNOWN".into(), |c| c.name.to_uppercase()),
        }
    }

    fn build_view(&mut self) {
        let origin = match self.mode {
            Mode::Pilot => self.v.ship_system,
            Mode::Observer => self.observer.origin(&self.v),
        };
        let system = self.charts.system(origin);
        let mut positions = std::mem::take(&mut self.view.positions);
        system.positions(self.now(), &mut positions);
        let ship_pos = self.place(Who::Me).0 + self.charts.galaxy.offset(origin, self.v.ship_system);
        // Speeds and prograde relative to what dominates gravity, or among
        // an asteroid field, to its remnant (the star dominates out there).
        let field = system.fields.iter().find(|f| positions[f.body].distance(ship_pos) < f.extent + rocks::SCAN_RANGE).map(|f| f.body);
        let reference = (origin == self.v.ship_system).then(|| field.unwrap_or_else(|| system.dominant(ship_pos, &positions)));
        self.view = View { origin, system, positions, ship_pos, reference };
    }

    fn focus_position(&self) -> DVec3 {
        match self.observer.focus {
            Focus::Body { body, .. } => self.view.positions[body],
            Focus::Ship => self.view.ship_pos,
            Focus::Craft(i) => self.place(Who::Craft(i)).0,
        }
    }

    fn update_camera(&mut self, dt: f64, focus_changed: bool) {
        match self.mode {
            Mode::Observer => {
                let focus = self.focus_position();
                if let (true, Some((prev_origin, prev_pos))) = (focus_changed, self.prev_focus) {
                    // Glide from the old target: express it in the new frame first.
                    let old = prev_pos + self.observer.transition + self.charts.galaxy.offset(self.view.origin, prev_origin);
                    self.observer.transition = old - focus;
                }
                self.observer.transition *= (-4.0 * dt).exp();
                self.prev_focus = Some((self.view.origin, focus));
                self.camera = self.observer.camera(focus);
            }
            Mode::Pilot if !self.v.crew.seated() => {
                let sys = self.charts.system(self.v.ship_system);
                let (position, orientation) = self.v.crew.eye(&sys, &self.ship, self.now(), &self.view.positions, self.view.ship_pos);
                self.camera = Camera { position, orientation: orientation.as_quat(), near: 0.05, ..Default::default() };
                self.prev_focus = None;
            }
            Mode::Pilot => {
                let ship = &self.v.ship;
                // Turned on as it's turning since the view.
                let turned = self.place(Who::Me).1;
                let orientation = turned.as_quat();
                let docked = matches!(ship.state, ShipState::Landed { body, .. } if self.view.system.bodies[body].kind == universe_sim::BodyKind::Station);
                // Docked: the ship is inside the slot, so back off far enough to see the station.
                // Spine to a rock (closing on it, or anchored): the chase view
                // rolls over, so the rock is below and the ship over it.
                let over_rock = matches!(ship.state, ShipState::Anchored { .. })
                    || self.v.avionics.following.is_some_and(|f| matches!(f.manoeuvre, universe_sim::avionics::follow::Manoeuvre::Surface(_)));
                let (chase, orientation) = if docked {
                    (DVec3::new(0.0, 150.0, 1800.0), orientation)
                } else if over_rock && self.chase_cam {
                    // Off to one side and a little behind, level with the gap
                    // between ship and rock, the rock below: the gear at work.
                    let at = DVec3::new(75.0, -12.0, 60.0);
                    let look = universe_sim::ship::facing(-(turned * at).normalize(), turned * DVec3::NEG_Y);
                    (at, look.as_quat())
                } else {
                    (DVec3::new(0.0, 20.0, 115.0), orientation)
                };
                let offset = if self.chase_cam || docked { turned * chase } else { DVec3::ZERO };
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

        // The economy panel (5) takes the keyboard while open.
        let economy_was_open = self.economy_panel.is_some();
        if economy_was_open {
            if !economy::input(self, ctx) {
                self.economy_panel = None;
            }
        } else if self.nav_map.is_none() && self.galaxy_map.is_none() && self.market.is_none() && keys::pressed(&ctx.input, keys::Act::Economy) {
            self.economy_panel = Some(Default::default());
        }
        // Where we are is explored.
        self.explored.insert(self.v.ship_system);
        // The galaxy map, then the navigation map, take the keyboard while open.
        let galaxy_was_open = self.galaxy_map.is_some();
        if galaxy_was_open && !galaxymap::input(self, ctx) {
            self.galaxy_map = None;
        }
        let map_was_open = self.nav_map.is_some() || galaxy_was_open;
        if self.nav_map.is_some() && !galaxy_was_open {
            navmap::input(self, ctx);
        } else if keys::pressed(&ctx.input, keys::Act::Map) {
            self.nav_map = Some(navmap::NavMap::open(self));
            sound::click(ctx, 900.0);
        }
        // So does the market (G, from the pilot's seat).
        let market_was_open = self.market.is_some();
        if market_was_open {
            market::input(self, ctx);
        } else if !map_was_open && self.nav_map.is_none() && self.mode == Mode::Pilot && self.v.crew.seated() && keys::pressed(&ctx.input, keys::Act::Market) {
            self.market = Some(market::MarketView::open(self));
            sound::click(ctx, 900.0);
        }
        let (controls, focus_changed) = match self.mode {
            _ if map_was_open || self.nav_map.is_some() || self.galaxy_map.is_some() || market_was_open || self.market.is_some() || economy_was_open || self.economy_panel.is_some() => (Controls::default(), false),
            Mode::Pilot => (self.pilot_input(ctx), false),
            Mode::Observer => (Controls::default(), self.observer.input(ctx, &self.v, &self.charts)),
        };
        if focus_changed {
            sound::click(ctx, 1400.0);
        }
        // The world: our commands are in. Running here, it runs a frame;
        // on its own thread, it's ticking away: take its newest view.
        let ammo = self.v.ship.ammo;
        let fresh = if self.engine.running() {
            self.engine.send(Command::Stick(controls));
            self.engine.send(Command::Warp(self.warp()));
            self.engine.poll()
        } else {
            universe_prof::time("update/sim (engine tick)", || self.engine.tick(dt, self.warp(), &controls));
            true
        };
        if fresh {
            self.mouse_since_tick = universe_engine::glam::Vec2::ZERO;
            self.prev = self.v.clone();
        }
        self.v = self.engine.view();
        self.alpha = self.alpha_now();
        self.ship = self.drawn_ship();
        let v = self.v.clone();
        if fresh && v.ship.ammo < ammo {
            sound::gunshot(ctx);
        }
        // Sparks where hits landed; a tick when ours do.
        for s in &mut self.sparks {
            s.age += ctx.dt;
        }
        self.sparks.retain(|s| s.age < hud::SPARK_TIME);
        let mut ours = false;
        for i in v.impacts.iter().filter(|_| fresh) {
            let mine = i.by == universe_sim::PLAYER;
            ours |= mine && !i.laser;
            if self.sparks.len() < 200 {
                self.sparks.push(Spark { system: i.system, point: i.point, age: 0.0, laser: i.laser, ours: mine, target: i.target });
            }
        }
        if ours {
            sound::hit_confirmed(ctx);
        }
        self.sim_ms = v.sim_ms;
        self.last_step = v.last_step;
        if fresh {
            universe_prof::time("update/events", || self.handle_events(ctx));
        }
        universe_prof::time("update/globes", || self.build_globes());
        universe_prof::time("update/rocks", || self.build_rocks());
        self.update_rigs(ctx.dt);
        if self.route_labels_for != v.avionics.route.stops {
            self.route_labels_for = v.avionics.route.stops.clone();
            self.route_labels = self.route_labels_for.iter().map(|&s| universe_sim::route::stop_name(&self.charts.system(s.system), s).to_uppercase()).collect();
        }
        // What the ship's computers make of things (they run in the engine).
        self.following = v.following.clone();
        self.plan_age += ctx.dt;
        if v.plan_serial != self.plan_serial {
            // A new plan: ease from what's on screen now (itself maybe part-way
            // from the one before), if it's for the same target.
            let same_target = v.plan.as_ref().zip(self.plan.as_ref()).is_some_and(|(a, b)| a.center.distance(b.center) < 1.0);
            self.plan_prev = if same_target { self.plan.take() } else { None };
            self.plan = v.plan.clone();
            self.plan_serial = v.plan_serial;
            self.plan_age = 0.0;
            match (&self.plan, v.avionics.clearance) {
                (Some(plan), Some(c)) => self.guide.update(plan, c.target),
                _ => self.guide.clear(),
            }
        }
        self.plan_cost = v.plan_cost;
        self.plan_every = v.plan_every;
        self.plan_blend = (self.plan_age / self.plan_every).min(1.0);
        let raw = self.plan.as_ref().filter(|p| p.arrives).map(|p| {
            let left = p.points.last().map_or(0.0, |x| x.time) - (v.time - p.start);
            left / self.warp().max(1.0)
        });
        self.eta_shown = match (raw, self.eta_shown) {
            (Some(raw), Some(shown)) if (raw - shown).abs() < 10.0 => {
                let ticked = shown - ctx.dt as f64;
                Some(ticked + (raw - ticked) * (ctx.dt as f64 * 2.0).min(1.0))
            }
            (raw, _) => raw,
        };
        self.reach = v.reach;
        self.docked_market = v.docked_market.is_some();
        self.contacts = v.contacts.clone();
        self.fire = v.fire;
        self.collision = v.collision.clone();
        self.collision_cost = v.collision_cost;
        self.collision_at = v.collision_at;
        self.hit_age += ctx.dt;
        self.beam_shown = (self.beam_shown - ctx.dt).max(0.0);
        universe_prof::time("update/build view", || self.build_view());
        // Where things are drawn is the moment drawn: the nav target and the
        // approach guidance are worked out here, at it, from the charts (the
        // view's are a tick off it).
        self.nav_marker = self.nav_marker_now();
        self.approach = self.approach_now();
        // The turrets in view, where they are at the moment drawn (from the charts).
        let (origin, t) = (self.view.origin, self.now());
        let sys = self.view.system.clone();
        self.turrets = universe_sim::world::turrets::turrets(self.charts.seed, origin, &sys)
            .into_iter()
            .map(|tu| {
                let (p, _) = tu.motion(&sys, t, &self.view.positions);
                (tu, p)
            })
            .collect();
        self.update_camera(dt, focus_changed);
        if std::env::var_os("UNIVERSE_DEBUG_MOTION").is_some() {
            // Where the nearest craft (chosen once) is drawn relative to the camera, and the world time drawn.
            static PICK: std::sync::OnceLock<usize> = std::sync::OnceLock::new();
            let cam = self.camera.position;
            let i = *PICK.get_or_init(|| {
                (0..self.v.crafts.len())
                    .filter(|&i| self.v.crafts[i].system == self.view.origin && self.v.crafts[i].ship.is_flying() && !self.v.crafts[i].ship.hyperdrive)
                    .min_by(|&a, &b| self.v.crafts[a].ship.position.distance(cam).total_cmp(&self.v.crafts[b].ship.position.distance(cam)))
                    .unwrap_or(0)
            });
            let rel = self.place(Who::Craft(i)).0 - cam;
            eprintln!("MOTION t {:.5} rel {:.3} {:.3} {:.3} alpha {:.2}", self.now(), rel.x, rel.y, rel.z, self.alpha());
        }
        if let Some(t) = self.sound_test {
            let next = t + dt;
            self.sound_test = dev::sound_test(ctx, next, t).then_some(next);
        } else {
            universe_prof::time("update/sound", || sound::update(ctx, self));
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
        universe_prof::time("draw/scene", || scene::draw(frame, self));
        universe_prof::time("draw/hud", || hud::draw(frame, self, ctx));
    }
}

/// True when the ship is visible as a model rather than being the camera.
pub fn ship_visible(app: &App) -> bool {
    let view = match app.v.crew.place {
        _ if app.mode == Mode::Observer => true,
        universe_sim::world::Place::Seat => app.chase_cam,
        universe_sim::world::Place::Aboard { .. } => false, // the interior instead
        universe_sim::world::Place::Outside { .. } => true,
    };
    !matches!(app.v.ship.state, ShipState::Destroyed { .. } | ShipState::Transit { .. }) && view
}

/// The galaxy's stars seen from a system: the system, and each star's direction and colour.
pub type SkyCache = (usize, Vec<(universe_engine::glam::Vec3, universe_engine::Color)>);

fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info,wgpu_core=warn,wgpu_hal=warn"))
        .init();
    run(Config { title: "universe".into(), ..Default::default() }, App::new());
}
