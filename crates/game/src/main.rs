mod palette;
mod devenv;
mod dev;
mod economy;
mod zoning;
mod newspanel;
mod fmt;
mod followguide;
mod galaxymap;
mod graphics;
mod hud;
mod market;
mod models;
mod navmap;
mod observer;
mod keys;
mod lock;
mod mining;
mod observe;
mod onfoot;
mod orbitpick;
mod rig;
mod rocks;
mod save;
mod scene;
mod interior;
mod shipyard;
mod studio;
mod studio_only;
mod test_drive;
mod planet_studio;
mod standards;
mod sound;
mod terrain_lod;
mod terrain_view;
mod thrusterpanel;
mod passengers;
mod manual;

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

/// The world's seed: the registry's (`seeding.galaxy`, which the registry requires).
fn seed() -> u64 {
    universe_sim::world::registry::registry().galaxy().expect("seeding.galaxy").seed as u64
}
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
    pub target: universe_sim::ShipId,
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
    /// What the system's globes, maps and ground patches are kept under: `origin`, or
    /// `STUDIO_CACHE` for the planet studio's world alone.
    pub cache: usize,
}

/// The planet studio's own system's cache key (see `View::cache`).
pub const STUDIO_CACHE: usize = usize::MAX;

/// Whose motion to draw: ours, or craft `i`'s.
#[derive(Clone, Copy, Debug)]
pub enum Who {
    Me,
    Craft(usize),
}

impl Who {
    /// The ship with id `id` (a turret's: none).
    pub fn of(id: universe_sim::ShipId) -> Option<Who> {
        match id.craft_index() {
            Some(i) => Some(Who::Craft(i)),
            None if id.is_player() => Some(Who::Me),
            None => None,
        }
    }
}

pub struct Message {
    pub text: String,
    pub ttl: f32,
}

/// Where keys go: the world (the root), or a panel open over it. Only the
/// top one open takes them (see `App::top_layer`); the system's keys (the
/// F-keys) work in any.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Layer {
    World,
    NavMap,
    GalaxyMap,
    Market,
    Economy,
    Worlds,
    News,
    Shipyard,
    Graphics,
    Standards,
    Passengers,
}

/// A baked world's full maps are read once the eye is within this many of its radii, and let go
/// past `WORLD_MAPS_FAR` (they're drawn within six).
const WORLD_MAPS_NEAR: f64 = 8.0;
const WORLD_MAPS_FAR: f64 = 16.0;

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
    /// The chase camera's own turn: the ship's, eased (it trails into a turn).
    pub chase_turn: Option<universe_engine::glam::DQuat>,
    pub warp_index: usize,
    /// The world's base time scale (game seconds per real second). The same
    /// for everyone in a shared world; set with UNIVERSE_TIME_SCALE (1-100).
    pub time_scale: f64,
    pub paused: bool,
    pub last_step: StepResult,
    pub show_help: bool,
    /// Debug info shown (F3): 0 none, 1 performance and traffic, 2 the profiler as well.
    pub debug: u8,
    /// Grid lines: orbit trajectories, planets' latitude/longitude grids, the ground grid's lines (O).
    pub show_grid: bool,
    pub show_labels: bool,
    pub muted: bool,
    /// The score off (F10), the ship's sounds still on.
    pub music_off: bool,
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
    /// A follow program's way, as a plan (see `followguide`), rebuilt every frame.
    pub follow_plan: Option<(universe_sim::Plan, Option<f64>)>,
    /// How lively the followed target is (a lively one's guide is the line alone).
    pub liveliness: followguide::Liveliness,
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
    /// The ground near worlds, as patches (see `terrain_lod`); made while drawing.
    pub terrain_lod: std::cell::RefCell<terrain_lod::Lod>,
    /// Terrain worlds' globes (full, coarse) and surface maps, by (system, body).
    pub globes: std::collections::HashMap<(usize, usize), (universe_engine::Mesh, universe_engine::Mesh, std::sync::Arc<universe_engine::GlobeMap>)>,
    /// Worlds grown by the planet simulation: their full-resolution maps, made in the
    /// background (None till ready), by (system, body).
    pub world_maps: std::collections::HashMap<(usize, usize), std::sync::Arc<std::sync::Mutex<Option<std::sync::Arc<universe_engine::WorldMaps>>>>>,
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
    /// The planet studio (WORLDS), while open; and how a baked world's ground is coloured.
    pub planet_studio: Option<planet_studio::PlanetStudio>,
    /// The planet studio's world shown alone, in a system of its own (see `planet_studio`).
    pub studio_world: Option<Arc<StarSystem>>,
    /// A frame of the shown world's history drawn in place of its colour today (the studio's
    /// timeline).
    pub world_frame: Option<planet_studio::WorldFrame>,
    pub world_look: planet_studio::Look,
    /// The galaxy map, when open (U from the navigation map).
    pub galaxy_map: Option<galaxymap::GalaxyMap>,
    /// The star systems we've been to (kept in the save).
    pub explored: std::collections::BTreeSet<usize>,
    /// The market screen, when open; and whether we're docked at a market.
    pub market: Option<market::MarketView>,
    /// The ship planner (the shipyard, docked at a station), while open.
    pub shipyard: Option<shipyard::Shipyard>,
    /// The game held to the shipyard studio's budget of cores (see `hold_to_cores`).
    held_to_cores: bool,
    /// At a vending machine, its panel open: the item picked.
    pub vending: Option<usize>,
    /// Ships' insides as laid out in the shipyard's studio, one per hull (kept with the
    /// hull's design: see `studio`).
    pub deckplans: Vec<universe_sim::world::deckplan::DeckPlan>,
    /// The layout last sent to the world engine for our hull (sent again when it changes).
    layout_sent: Option<universe_sim::world::deckplan::DeckPlan>,
    /// Walking through a plan from the shipyard: the shipyard as it was left (ESC goes back to it).
    pub preview: Option<shipyard::Shipyard>,
    /// Docked or landed where there's a market (as the world last said).
    pub docked_market: bool,
    /// What the target marker points at: the nav target, else the nearest station.
    pub nav_marker: Option<(String, DVec3)>,
    /// Ships on the radar, nearest first (refreshed every frame).
    pub contacts: Vec<universe_sim::Contact>,
    /// Fire control on the locked contact: the track, and the gun's lead once it has one.
    pub fire: Option<(universe_sim::avionics::Track, Option<universe_sim::avionics::Solution>)>,
    /// Seconds since the ship was last hit (for the HUD's flash).
    pub hit_age: f32,
    /// On the hypernet: the lag from the backbone (s) and the node it's through.
    pub net: Option<(f64, String)>,
    /// When the status was last worked out (game time).
    pub net_at: f64,
    /// This system's relays (its index, and how many claims there were, with them).
    pub net_nodes: Option<((usize, usize), Vec<universe_sim::world::hypernet::Node>)>,
    /// What news has come to us over the hypernet (or our own comm), and when.
    pub news: universe_sim::news::Knowledge,
    /// The news outlets, and their digests (opened on the first update).
    pub newsroom: Option<universe_sim::newsroom::Newsroom>,
    /// The news panel, when open.
    pub news_panel: bool,
    /// What's drawn of what can be (see `graphics`), and its panel open.
    pub graphics: universe_engine::Graphics,
    pub graphics_panel: bool,
    /// The live observer port's questions (see `observe`), and a recording under way.
    pub observe_port: Option<std::sync::mpsc::Receiver<observe::Request>>,
    pub recording: Option<observe::Recording>,
    /// The port tried (once: it stays open, answering only while observing).
    pub observe_tried: bool,
    /// Observing (SHIFT+F3): the port answering, the profiler and hitch log on.
    pub observing: bool,
    /// Recent hits, for their sparks.
    pub sparks: Vec<Spark>,
    /// A glTF model shown ahead of the eye (dev: `UNIVERSE_MODEL=file.glb`).
    pub showcase: Option<universe_engine::PbrModel>,
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
    /// The thrusters panel (F7).
    pub show_thrusters: bool,
    /// The standards registry open (docked), and where in its tree.
    pub standards: Option<standards::StandardsView>,
    /// The passengers panel open: its row picked.
    pub passengers: Option<usize>,
    /// Manual: the thrusters last held (sent when it changes).
    pub jets_held: u64,
    /// Mining mode and the prospector's pulse; T's lock picker.
    pub mining: mining::Mining,
    pub picker: lock::Picker,
    /// The orbit key's list, and the range chosen last.
    pub orbit_pick: orbitpick::OrbitPick,
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
    /// Our hull's inside as laid out, to the world engine if it's changed since it
    /// was last sent (to walk in).
    pub fn send_layout(&mut self) {
        let key = &self.ship.spec().key;
        let plan = self.deckplans.iter().find(|p| &p.hull == key).cloned().unwrap_or_else(|| universe_sim::world::deckplan::DeckPlan { hull: key.clone(), decks: Vec::new() });
        if self.layout_sent.as_ref() != Some(&plan) {
            self.engine.send(Command::Layout(plan.clone()));
            self.layout_sent = Some(plan);
        }
    }

    fn new() -> Self {
        let mut u = Universe::new(seed());
        // UNIVERSE_RECORD=path: record the session from its start, saved there
        // on exit (replay it: `cargo run -p universe-sim --release --example replay -- path`).
        if std::env::var_os("UNIVERSE_RECORD").is_some() {
            u.record_inputs();
        }
        // Traffic: reproducible settlers (UNIVERSE_SETTLERS, default 1,000).
        let settlers = std::env::var("UNIVERSE_SETTLERS").ok().and_then(|v| v.parse().ok()).unwrap_or(1_000);
        u.spawn_settlers(settlers, seed());
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
            chase_turn: None,
            warp_index: 0,
            time_scale: std::env::var("UNIVERSE_TIME_SCALE")
                .ok()
                .and_then(|v| v.parse::<f64>().ok())
                .map_or(DEFAULT_TIME_SCALE, |v| v.clamp(1.0, 100.0)),
            paused: false,
            last_step: StepResult::default(),
            show_help: false,
            // (UNIVERSE_DEBUG: start with the debug info up.)
            debug: std::env::var("UNIVERSE_DEBUG").ok().and_then(|v| v.parse().ok()).unwrap_or(0),
            show_grid: false,
            show_labels: true,
            muted: false,
            music_off: false,
            messages: Vec::new(),
            view: View { origin, system, positions: Vec::new(), ship_pos: DVec3::ZERO, reference: None, cache: origin },
            approach: None,
            plan: None,
            plan_age: 0.0,
            plan_serial: 0,
            plan_every: 0.1,
            plan_prev: None,
            guide: Default::default(),
            follow_plan: None,
            liveliness: Default::default(),
            plan_blend: 1.0,
            plan_cost: 0.0,
            sim_ms: 0.0,
            eta_shown: None,
            globes: std::collections::HashMap::new(),
            world_maps: std::collections::HashMap::new(),
            terrain_lod: Default::default(),
            rocks: std::collections::HashMap::new(),
            rigs: Default::default(),
            mining: Default::default(),
            picker: Default::default(),
            orbit_pick: Default::default(),
            show_cargo: false,
            show_thrusters: false,
            passengers: None,
            standards: None,
            jets_held: 0,
            sky_cache: std::cell::RefCell::new(None),
            nav_map: None,
            galaxy_map: None,
            economy_panel: None,
            planet_studio: None,
            studio_world: None,
            world_frame: None,
            world_look: Default::default(),
            explored: Default::default(),
            market: None,
            shipyard: None,
            held_to_cores: false,
            deckplans: Vec::new(),
            layout_sent: None,
            preview: None,
            vending: None,
            docked_market: false,
            nav_marker: None,
            contacts: Vec::new(),
            fire: None,
            hit_age: 99.0,
            net: None,
            net_at: f64::NEG_INFINITY,
            net_nodes: None,
            news: Default::default(),
            newsroom: None,
            news_panel: false,
            graphics: graphics::load(),
            graphics_panel: false,
            observe_port: {
                observe::clean_up();
                None
            },
            observe_tried: false,
            observing: false,
            recording: None,
            sparks: Vec::new(),
            showcase: std::env::var("UNIVERSE_MODEL").ok().and_then(|path| {
                let r = std::fs::read(&path).map_err(|e| e.to_string()).and_then(|b| universe_engine::PbrModel::load_gltf(&b));
                r.map_err(|e| log::warn!("showcase {path}: {e}")).ok()
            }),
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
        app.say(format!("WELCOME TO {}", name.to_uppercase()));
        app.say("J POWERS UP, SHIFT+E LIFTS OFF - F1 FOR CONTROLS".into());
        // A hull imported from a glTF file, the ship we fly (`UNIVERSE_HULL=file.glb`;
        // by default, for now, the MC-07, if its file is there).
        const DEFAULT_HULL: &str = "assets/models/mc07.glb";
        let hull = std::env::var("UNIVERSE_HULL").ok().or_else(|| std::path::Path::new(DEFAULT_HULL).exists().then(|| DEFAULT_HULL.to_string()));
        if let Some(path) = hull {
            match std::fs::read(&path).map_err(|e| e.to_string()).and_then(|b| universe_sim::world::import::commission(&b, &path)) {
                Ok(h) => {
                    let u = app.engine.universe();
                    u.ship.class = h;
                    u.ship.refresh();
                    // (Set down at its own height: the new game stood the default hull.)
                    let mut ship = u.ship.clone();
                    u.world.resettle(u.ship_system, &mut ship);
                    u.ship = ship;
                    u.ship.fuel = u.ship.spec().fuel_capacity;
                    u.ship.energy = u.ship.spec().capacitor_capacity;
                    let name = u.ship.spec().name.clone();
                    app.say(format!("FLYING {name}"));
                    app.engine.refresh();
                    app.v = app.engine.view();
                }
                Err(e) => app.say(format!("HULL {path}: {e}").to_uppercase()),
            }
        }
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
        let behind = std::time::Instant::now() - std::time::Duration::from_secs_f64(1.0 / universe_sim::engine::tick_hz());
        let into = behind.saturating_duration_since(self.prev.made).as_secs_f64();
        (into / span).clamp(0.0, 1.0)
    }

    /// Our ship at the moment drawn: position and turn as `place`, velocity
    /// between the two views likewise.
    fn drawn_ship(&self) -> universe_sim::Ship {
        let (position, orientation) = self.place(Who::Me);
        let same = self.prev.ship_system == self.v.ship_system && self.prev.ship.position.distance(self.v.ship.position) < 20_000.0;
        let velocity = if same { self.prev.ship.velocity.lerp(self.v.ship.velocity, self.alpha) } else { self.v.ship.velocity };
        let mut ship = self.v.ship.clone();
        (ship.position, ship.orientation, ship.velocity) = (position, orientation, velocity);
        ship
    }

    /// World time to draw at.
    /// Where the ship stands on its system's hypernet (worked out twice a second).
    fn update_net(&mut self) {
        use universe_sim::world::hypernet::Net;
        let t = self.now();
        if (t - self.net_at).abs() < 0.5 {
            return;
        }
        self.net_at = t;
        let sys = self.view.system.clone();
        // (In a gate's tube: off the net.)
        if sys.index != self.v.ship_system || matches!(self.ship.state, ShipState::Transit { .. }) {
            self.net = None;
            return;
        }
        let key = (sys.index, 0);
        if self.net_nodes.as_ref().is_none_or(|(k, _)| *k != key) {
            self.net_nodes = Some((key, universe_sim::world::hypernet::nodes(&self.charts.galaxy, &sys)));
        }
        let all = self.net_nodes.as_ref().map(|(_, n)| n.clone()).unwrap_or_default();
        let net = Net::at(&sys, all, t, &self.view.positions);
        self.net = net.status(&sys, &self.view.positions, self.view.ship_pos, &self.ship.spec().comm).map(|s| (s.lag, net.nodes[s.via].name.clone()));
    }

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
        // (Not when it jumped: a respawn, a gate. Judged against where its
        // motion would have taken it: around a world it moves tens of km a
        // second, and two views a hitch apart are kilometres apart. Snapping
        // then put the ship at one moment and the world at another: the
        // ground leapt for a frame.)
        let dt = self.v.time - self.prev.time;
        let (position, orientation) = match before.filter(|b| (b.position + b.velocity * dt).distance(now.position) < 20_000.0) {
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

    /// The layer keys go to: the top one open. The world is the root; the
    /// panels open over it (the galaxy map over the navigation map), each
    /// from the layer under it, and close back to it.
    fn top_layer(&self) -> Layer {
        if self.shipyard.is_some() {
            Layer::Shipyard
        } else if self.market.is_some() {
            Layer::Market
        } else if self.economy_panel.is_some() {
            Layer::Economy
        } else if self.planet_studio.is_some() {
            Layer::Worlds
        } else if self.news_panel {
            Layer::News
        } else if self.galaxy_map.is_some() {
            Layer::GalaxyMap
        } else if self.nav_map.is_some() {
            Layer::NavMap
        } else if self.graphics_panel {
            Layer::Graphics
        } else if self.standards.is_some() {
            Layer::Standards
        } else if self.passengers.is_some() {
            Layer::Passengers
        } else {
            Layer::World
        }
    }

    /// The world's own keys, with nothing open over it: what opens a panel,
    /// TAB (watch or fly), the time warp.
    fn world_keys(&mut self, ctx: &mut Context) {
        let input = &ctx.input;
        // Walking through the plan: ESC (or the shipyard key) back to the studio, seated.
        if self.preview.is_some() && (input.pressed(KeyCode::Escape) || keys::pressed(input, keys::Act::Shipyard)) {
            self.engine.send(Command::Preview(None));
            if let Some(s) = self.preview.take() {
                self.shipyard = Some(s);
            }
            ctx.grab_cursor(false);
            return;
        }
        let seated_pilot = self.mode == Mode::Pilot && self.v.crew.seated();
        if keys::pressed(input, keys::Act::Economy) {
            self.economy_panel = Some(Default::default());
            return;
        }
        if self.mode == Mode::Observer && keys::pressed(input, keys::Act::Worlds) {
            self.planet_studio = Some(planet_studio::PlanetStudio::open());
            return;
        }
        if input.pressed(KeyCode::F11) {
            self.news_panel = true;
            return;
        }
        if graphics::opens(ctx) {
            self.graphics_panel = true;
            return;
        }
        if keys::pressed(input, keys::Act::Map) {
            self.nav_map = Some(navmap::NavMap::open(self));
            sound::click(ctx, 900.0);
            return;
        }
        if seated_pilot && keys::pressed(input, keys::Act::Market) {
            self.market = Some(market::MarketView::open(self));
            sound::click(ctx, 900.0);
            return;
        }
        if seated_pilot && keys::pressed(input, keys::Act::Shipyard) {
            self.shipyard = shipyard::open(self);
            sound::click(ctx, 900.0);
            return;
        }
        // TAB: the ship (the chase camera) or watching.
        if input.pressed(KeyCode::Tab) {
            match self.mode {
                Mode::Observer => {
                    self.mode = Mode::Pilot;
                    self.chase_cam = true;
                }
                Mode::Pilot => {
                    ctx.grab_cursor(false);
                    self.observer.focus = Focus::Ship;
                    self.observer.distance = 180.0;
                    self.mode = Mode::Observer;
                }
            }
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
    }

    /// The system's keys, in any layer: the F-keys (help, labels, the debug
    /// overlay and observing, the grid, saving and loading, pause, the
    /// thrusters, sound, music, a screenshot).
    fn system_keys(&mut self, ctx: &mut Context) {
        let input = &ctx.input;
        if input.pressed(KeyCode::F6) {
            self.paused = !self.paused;
            sound::click(ctx, 400.0);
        }
        if input.pressed(KeyCode::F1) {
            self.show_help = !self.show_help;
        }
        // F3: debug info (performance, traffic, trades), then the profiler too, then off.
        // SHIFT+F3: observing, for whoever's helping — the live port, the profiler and
        // the hitch log on, and a recording (profile, trace, every frame); again: all off.
        let shift = input.down(KeyCode::ShiftLeft) || input.down(KeyCode::ShiftRight);
        if input.pressed(KeyCode::F3) && shift {
            if self.observing {
                observe::stop(self, ctx);
                self.observing = false;
                universe_prof::enable(self.debug == 2);
                self.say("OBSERVING OFF".into());
            } else {
                self.observing = true;
                observe::start(self, ctx);
            }
        } else if input.pressed(KeyCode::F3) {
            self.debug = (self.debug + 1) % 3;
            if !self.observing {
                universe_prof::enable(self.debug == 2);
            }
        }
        if input.pressed(KeyCode::F7) {
            self.show_thrusters = !self.show_thrusters;
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
        if input.pressed(KeyCode::F10) {
            self.music_off = !self.music_off;
            self.say(if self.music_off { "MUSIC OFF" } else { "MUSIC ON" }.into());
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
            // At a vending machine: F opens it; its panel takes the keys while open.
            let at_machine = matches!(self.reach, Some(universe_sim::world::crew::Reach::Vending(_)));
            // (Open, it holds you there: it closes with F or ESC. Buying is
            // the machine's to check: within reach.)
            if let Some(pick) = self.vending {
                let input = &ctx.input;
                let n = universe_sim::world::spaceport::VENDING.len();
                if input.pressed(KeyCode::ArrowDown) {
                    self.vending = Some((pick + 1) % n);
                } else if input.pressed(KeyCode::ArrowUp) {
                    self.vending = Some((pick + n - 1) % n);
                } else if input.pressed(KeyCode::Enter) {
                    self.engine.send(Command::Vend(pick));
                } else if input.pressed(KeyCode::KeyF) || input.pressed(KeyCode::Escape) {
                    self.vending = None;
                }
                return Controls::default();
            }
            if at_machine && ctx.input.pressed(KeyCode::KeyF) {
                self.vending = Some(0);
                return Controls::default();
            }
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
        // The hyperdrive is navigation's: nothing fights or digs in hyperspace.
        // Docked or landed: the pilot's own business — flight systems up (to
        // fly) or down (to park), the tank filled, the hull mended.
        if matches!(self.v.ship.state, ShipState::Landed { .. }) {
            if pressed(input, Act::Systems) {
                self.engine.send(Command::Ship(ShipCommands { power: Some(!self.v.ship.powered), ..self.v.ship.holding() }));
            }
            if pressed(input, Act::Refuel) {
                self.engine.send(Command::Refuel);
            }
            if pressed(input, Act::Repair) {
                self.engine.send(Command::Repair);
            }
            if pressed(input, Act::Passengers) && self.v.docked_market.is_some() {
                self.passengers = Some(0);
            }
            if pressed(input, Act::Standards) && universe_sim::world::traffic::docked_at(&self.view.system, &self.ship).is_some() {
                self.standards = Some(standards::StandardsView::new());
            }
        }
        if pressed(input, Act::Hyperdrive) {
            if mode == ShipMode::Nav || self.v.ship.hyperdrive {
                self.engine.send(Command::ToggleHyperdrive);
            } else {
                self.say(format!("HYPERDRIVE IN NAV MODE - {} OR {} TO LEAVE THIS ONE", keys::key(Act::Combat), keys::key(Act::Mining)));
            }
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
        if pressed(input, Act::Combat) && self.v.ship.hyperdrive {
            self.say("NO COMBAT IN HYPERSPACE".into());
        } else if pressed(input, Act::Combat) {
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
            self.say(if on { "IMPACT ON" } else { "IMPACT OFF" }.into());
        }
        // T: lock on (tap: what's ahead; hold: choose from the list).
        let listing = lock::input(self, ctx) | orbitpick::input(self, ctx);
        mining::input(self, ctx);
        let input = &ctx.input;
        // Keep at a range from the locked ship (or the nav target's station
        // or gate), again for the next range out; the orbit key (tap, or
        // hold to choose the range) is `orbitpick`'s. X cancels.
        if pressed(input, Act::Keep) {
            self.engine.send(Command::Follow(FollowKind::KeepAt, None));
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
        if pressed(input, Act::Cancel) && self.v.avionics.following.is_some() {
            self.engine.send(Command::StopFollowing);
        }
        // Manual thrusters (NAV): the flight computer off, every thruster
        // fired by hand; again, back on.
        let flying = self.v.ship.is_flying() && !self.v.ship.hyperdrive;
        if mode == ShipMode::Nav && pressed(input, Act::Manual) {
            if !self.v.ship.manual && !flying {
                self.say("MANUAL THRUSTERS IN FLIGHT ONLY".into());
            } else {
                let on = !self.v.ship.manual;
                // (Going manual, the autopilots let go: nothing flies it but you.)
                if on {
                    if self.v.avionics.following.is_some() {
                        self.engine.send(Command::StopFollowing);
                    }
                    if self.v.avionics.route.active {
                        self.engine.send(Command::ToggleRoute);
                    } else if self.v.avionics.clearance.is_some_and(|c| c.autopilot) {
                        self.engine.send(Command::ToggleAutopilot);
                    }
                }
                self.jets_held = 0;
                self.engine.send(Command::Ship(ShipCommands { manual: Some(on), throttle: 0.0, rcs: DVec3::ZERO, ..self.v.ship.holding() }));
                self.say(if on { "MANUAL THRUSTERS - FLIGHT COMPUTER OFF" } else { "FLIGHT COMPUTER ON" }.into());
            }
        }
        if self.v.ship.manual {
            // An autopilot taken up: the flight computer's back on to fly it.
            if self.v.avionics.route.active || self.v.avionics.clearance.is_some_and(|c| c.autopilot) || self.v.avionics.following.is_some() || !flying {
                self.engine.send(Command::Ship(ShipCommands { manual: Some(false), ..self.v.ship.holding() }));
                return Controls::default();
            }
            let held = manual::held(input, self.v.ship.spec());
            if held != self.jets_held {
                self.jets_held = held;
                self.engine.send(Command::Ship(ShipCommands { jets: Some(held), ..self.v.ship.holding() }));
            }
            return Controls::default();
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
            sound::event(ctx, Some(self), &event);
            // Hits show on the HUD (hull, flash), not as messages: a burst would flood them.
            if let Event::Ship(ShipEvent::Hit { .. }) = event {
                self.hit_age = 0.0;
                continue;
            }
            let text = match event {
                Event::Ship(ShipEvent::Landed { body, station: true }) => format!("DOCKED AT {body}"),
                Event::Refuelled { tonnes, credits } if tonnes < 1.0 => format!("REFUELLED {:.0} KG FOR {credits:.0} CR", tonnes * 1000.0),
                Event::Refuelled { tonnes, credits } => format!("REFUELLED {tonnes:.1} T FOR {credits:.0} CR"),
                Event::Repaired { credits, hull } => format!("HULL REPAIRED TO {:.0}% FOR {credits:.0} CR", hull * 100.0),
                Event::Trimmed => "SHIP TRIMMED".into(),
                Event::PassengersBoarded { count } => format!("{count} PASSENGERS ABOARD"),
                Event::PassengersLanded { count, credits } => format!("{count} PASSENGERS LANDED - {credits:.0} CR IN FARES"),
                Event::Vended { what, credits, note } => format!("{what} - {credits:.0} CR. {note}"),
                Event::Refitted { slot, module, credits } => format!("{} FITTED IN {} - {} {:.0} CR", module.unwrap_or_else(|| "NOTHING".into()), slot.to_uppercase(), if credits >= 0.0 { "COST" } else { "PAID" }, credits.abs()),
                Event::BoughtShip { name, credits } => format!("NEW SHIP: {name} - {} {:.0} CR WITH YOUR OLD ONE TRADED IN", if credits >= 0.0 { "COST" } else { "PAID" }, credits.abs()),
                Event::Charged { offence, system, penalties } => {
                    let then = if penalties.is_empty() { String::new() } else { format!(" - {penalties}") };
                    format!("CHARGED UNDER {}'S LAW: {}{then}", system.to_uppercase(), offence.to_uppercase())
                }
                Event::Bounty { credits, on } => format!("BOUNTY: {credits:.0} CR FOR {on}"),
                Event::Insured { excess, refused, at } => {
                    let at = if at.is_empty() { String::new() } else { format!(", PARKED AT {}", at.to_uppercase()) };
                    match (excess, refused) {
                        (Some(x), _) => format!("INSURED: THE SAME SHIP AGAIN, FOR AN EXCESS OF {x:.0} CR{at}"),
                        (None, Some(o)) => format!("THE INSURER WON'T PAY A LOSS BROUGHT ON BY {} - A BASIC SHIP{at}", o.to_uppercase()),
                        (None, None) => format!("INSURED: COULDN'T PAY THE EXCESS - A BASIC SHIP{at}"),
                    }
                }
                Event::Ship(ShipEvent::OutOfFuel) => "OUT OF FUEL - NO THRUST, NO HYPERDRIVE".into(),
                Event::Ship(ShipEvent::Landed { body, station: false }) => format!("LANDED ON {body}"),
                Event::Ship(ShipEvent::TookOff) => "LIFT OFF".into(),
                Event::Ship(ShipEvent::Crashed { body }) => format!("SHIP DESTROYED - {body}"),
                Event::Ship(ShipEvent::Respawned) => "SHIP LOST".into(),
                Event::Ship(ShipEvent::EnteredSystem { name }) => format!("ENTERING {name} SYSTEM"),
                Event::Ship(ShipEvent::HyperdriveEngaged) => "HYPERDRIVE ENGAGED".into(),
                Event::Ship(ShipEvent::SystemsOn) => "FLIGHT SYSTEMS ON".into(),
                Event::Ship(ShipEvent::SystemsOff) => format!("FLIGHT SYSTEMS DOWN - {} TO POWER UP", keys::key(keys::Act::Systems)),
                Event::Ship(ShipEvent::SystemsRefused { why }) => format!("CAN'T POWER DOWN - {why}"),
                Event::Ship(ShipEvent::HyperdriveDisengaged) => "HYPERDRIVE OFF".into(),
                Event::Ship(ShipEvent::HyperdriveJammed { seconds }) => format!("HYPERDRIVE JAMMED BY HITS - {seconds:.0} S"),
                Event::HyperdriveArrived { target } => format!("ARRIVED AT {target}\n{}", self.target_hint()),
                Event::Traffic(TrafficEvent::ClearanceGranted { target, kind: ClearanceKind::Dock }) => {
                    format!("DOCKING GRANTED - {target}\nCOME DOWN THROUGH THE SQUARES TO YOUR PAD, OR {} FOR AUTOPILOT", crate::keys::key(crate::keys::Act::Autopilot))
                }
                Event::Traffic(TrafficEvent::ClearanceGranted { target, kind: ClearanceKind::Land }) => {
                    format!("LANDING GRANTED - {target}\nFOLLOW THE PATH, OR {} FOR AUTOPILOT", crate::keys::key(crate::keys::Act::Autopilot))
                }
                Event::Traffic(TrafficEvent::ClearanceGranted { target, kind: ClearanceKind::Transit }) => {
                    format!("TRANSIT GRANTED - {target}\nFLY THROUGH THE RING UNDER 300 M/S, OR {} FOR AUTOPILOT", crate::keys::key(crate::keys::Act::Autopilot))
                }
                Event::Ship(ShipEvent::GateEntered { to }) => format!("GATE TRANSIT TO {to}"),
                Event::Ship(ShipEvent::TubeToll { credits }) => format!("THE CROSSING: {credits:.0} CR"),
                Event::Ship(ShipEvent::GateArrived { system }) => format!("WELCOME TO THE {system} SYSTEM"),
                Event::Ship(ShipEvent::GateTooFast { speed }) => format!("TOO FAST FOR THE GATE ({:.0} M/S)", speed),
                Event::Traffic(TrafficEvent::ClearanceDenied { reason }) => format!("CLEARANCE DENIED - {reason}"),
                Event::Refused { reason } => reason,
                Event::Notice { text } => text,
                Event::Following { what: Some((how, range)) } if how == "CLOSE ON" => format!("CLOSING ON THE ROCK, {range:.0} M OFF ITS SURFACE\n{} TO ANCHOR WHEN IN REACH, {} TO CANCEL", crate::keys::key(crate::keys::Act::Anchor), crate::keys::key(crate::keys::Act::Cancel)),
                Event::Following { what: Some((how, range)) } if how == "ORBIT" => format!("ORBIT AT {} - HOLD {} TO CHOOSE THE RANGE, {} TO CANCEL", orbitpick::label(range), crate::keys::key(crate::keys::Act::Orbit), crate::keys::key(crate::keys::Act::Cancel)),
                Event::Following { what: Some((how, range)) } => format!("{how} {} - {} AGAIN: NEXT RANGE, {} TO CANCEL", orbitpick::label(range), crate::keys::key(crate::keys::Act::Keep), crate::keys::key(crate::keys::Act::Cancel)),
                Event::Following { what: None } => "FOLLOW OFF".into(),
                Event::Traffic(TrafficEvent::ClearanceCancelled) => "CLEARANCE CANCELLED".into(),
                Event::Traffic(TrafficEvent::PadAssigned { pad }) => format!("LAND ON PAD {}", pad + 1),
                // (Nobody ahead: the request is on its way; a pad most likely follows.)
                Event::Traffic(TrafficEvent::Holding { ahead: 0 }) => "PAD REQUESTED - STAND BY".into(),
                Event::Traffic(TrafficEvent::Holding { ahead }) => format!("ALL PADS TAKEN - HOLD ({ahead} AHEAD)"),
                Event::Autopilot { on: true } => "AUTOPILOT ON".into(),
                Event::Autopilot { on: false } => "AUTOPILOT OFF".into(),
                Event::NavTargetSet { name: Some(name) } => format!("NAV TARGET - {name}\n{}", self.target_hint()),
                Event::NavTargetSet { name: None } => "NAV TARGET CLEARED".into(),
                Event::Ship(ShipEvent::LandedAtPort { port }) => format!("TOUCHDOWN - WELCOME TO {port}"),
                Event::RouteStop { number, name } => format!("ROUTE STOP {number} - {name}"),
                Event::RouteComplete => "ROUTE COMPLETE".into(),
                Event::RouteBlocked { reason } => format!("ROUTE STOPPED - {reason}"),
                Event::RouteSkipped { name } => format!("ROUTE: {} REFUSED US - ON TO THE NEXT STOP", name.to_uppercase()),
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
                Event::Ship(ShipEvent::Anchored { body }) => format!("ANCHORED TO {body}\n{} TO DIG, {} TO UNANCHOR", crate::keys::key(crate::keys::Act::Excavate), crate::keys::key(crate::keys::Act::Anchor)),
                Event::Ship(ShipEvent::AnchorFailed { why }) => format!("ANCHOR - {why}"),
                Event::Ship(ShipEvent::NotFitted { what }) => format!("NO {what} FITTED"),
                Event::Ship(ShipEvent::AnchorReleased) => "ANCHOR RELEASED".into(),
                Event::Ship(ShipEvent::ExcavatorStopped { why }) => format!("EXCAVATOR STOPPED - {why}"),
                Event::Ship(ShipEvent::Mined { item, .. }) => {
                    let name = self.charts.goods.get(item).map_or(String::new(), |g| g.name.clone());
                    let units = self.v.hold.iter().find(|h| h.0 == item).map_or(0, |h| h.1);
                    format!("+1 T {name} TO THE HOLD ({units} T IN ALL, HOLD {:.0}/{:.0} T)", self.v.ship.cargo / 1000.0, self.v.ship.spec().hold_capacity / 1000.0)
                }
                Event::Ship(ShipEvent::StruckRock { speed, .. }) => format!("ROCK STRIKE AT {speed:.1} M/S"),
                Event::Ship(ShipEvent::HardLanding { sink, jolt }) => format!("HARD LANDING: {sink:.1} M/S DOWN, {jolt:.1} G ABOARD"),
                Event::Ship(ShipEvent::CargoBroken { item, units }) => format!("BROKEN IN THE JOLT: {units} OF {}", self.charts.goods[item].name.to_uppercase()),
                // Anything else says nothing (add a line here for a new event that should).
                _ => continue,
            };
            self.say(text.to_uppercase());
        }
    }

    /// What to do about the nav target.
    fn target_hint(&self) -> String {
        match self.v.avionics.nav_target {
            Some(universe_sim::NavTarget::Asteroid(_)) => format!("{} TO KEEP STATION, {} TO ORBIT", crate::keys::key(crate::keys::Act::Keep), crate::keys::key(crate::keys::Act::Orbit)),
            _ => format!("{} FOR DOCKING", crate::keys::key(crate::keys::Act::Clearance)),
        }
    }

    /// Terrain globes for the system in view (built once, on first sight), and the full maps of
    /// a baked world near the eye.
    fn build_globes(&mut self) {
        let origin = self.view.cache;
        for (i, b) in self.view.system.bodies.iter().enumerate() {
            if !self.globes.contains_key(&(origin, i))
                && let (Some(full), Some(coarse), Some(map)) = (terrain_view::globe(b, 8), terrain_view::globe(b, 2), terrain_view::globe_map(b))
            {
                self.globes.insert((origin, i), (full.into(), coarse.into(), std::sync::Arc::new(map)));
            }
            // A world with a bake: its full-resolution maps, read and encoded on a thread of their
            // own once the eye comes within `WORLD_MAPS_NEAR` of it, let go past `WORLD_MAPS_FAR`
            // (a few hundred megabytes each while held).
            let away = self.view.positions.get(i).map_or(f64::INFINITY, |c| c.distance(self.camera.position)) / b.rail.radius;
            if away > WORLD_MAPS_FAR {
                self.world_maps.remove(&(origin, i));
            }
            if let Some(t) = b.terrain.as_ref().filter(|t| t.baked())
                && away < WORLD_MAPS_NEAR
                && !self.world_maps.contains_key(&(origin, i))
            {
                let slot = std::sync::Arc::new(std::sync::Mutex::new(None));
                self.world_maps.insert((origin, i), slot.clone());
                let (t, look) = (t.clone(), self.world_look);
                // (A frame of its history in place of today's colour: its sea and clouds then
                // aren't today's, so none.)
                let then = self.world_frame.clone().filter(|f| f.key == b.key);
                std::thread::spawn(move || {
                    let started = std::time::Instant::now();
                    // (Each read and encoded before the next is read: one image held at a time.)
                    let names = [look.file(), "globe_ground.jpg", "globe_normal.jpg", "climate.png", "rockid.png", "globe_spec.png"];
                    let mut k = 0;
                    let encoded = names.map(|name| {
                        let image = match &then {
                            Some(f) if k == 0 => f.history.image(f.frame),
                            Some(_) if name == "globe_spec.png" => None,
                            _ => t.bake_image(name),
                        };
                        let e = image.and_then(|(w, h, rgba)| universe_engine::WorldMaps::encode(k, universe_engine::pbr::Image { width: w as u32, height: h as u32, rgba }));
                        k += 1;
                        e
                    });
                    let maps = universe_engine::WorldMaps::encoded(encoded, t.bake_air())
                        .with_air_luts(t.bake_air_luts().map(|l| [l.transmittance, l.multiscatter]));
                    let mut maps = maps;
                    if let Some(c) = t.bake_clouds().filter(|_| then.is_none()) {
                        let img = |(w, h, rgba): (usize, usize, Vec<u8>)| universe_engine::pbr::Image { width: w as u32, height: h as u32, rgba };
                        let [a, b, c3] = c.maps;
                        maps.clouds_year = Some((c.year_days, c.enso));
                        maps.clouds_format = c.format;
                        maps = maps.with_clouds(Some([img(a), img(b), img(c3)]));
                    }
                    log::info!("world maps read and encoded in {:.1} s", started.elapsed().as_secs_f64());
                    if let Ok(mut s) = slot.lock() {
                        *s = Some(std::sync::Arc::new(maps));
                    }
                });
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
    fn ship_name(&self, id: universe_sim::ShipId) -> String {
        match Who::of(id) {
            Some(Who::Me) => "YOU".into(),
            Some(Who::Craft(i)) => self.v.crafts.get(i).map_or_else(|| "UNKNOWN".into(), |c| c.name.to_uppercase()),
            None => "SAM TURRET".into(),
        }
    }

    fn build_view(&mut self) {
        let origin = match self.mode {
            Mode::Pilot => self.v.ship_system,
            Mode::Observer => self.observer.origin(&self.v),
        };
        // (The planet studio's world alone: its own system in the view.)
        let studio = self.studio_world.clone().filter(|_| self.mode == Mode::Observer);
        let cache = if studio.is_some() { STUDIO_CACHE } else { origin };
        let system = studio.unwrap_or_else(|| self.charts.system(origin));
        let mut positions = std::mem::take(&mut self.view.positions);
        system.positions(self.now(), &mut positions);
        let ship_pos = self.place(Who::Me).0 + self.charts.galaxy.offset(origin, self.v.ship_system);
        // Speeds and prograde relative to what dominates gravity, or among
        // an asteroid field, to its remnant (the star dominates out there).
        let field = system.fields.iter().find(|f| positions[f.body].distance(ship_pos) < f.extent + rocks::SCAN_RANGE).map(|f| f.body);
        let reference = (origin == self.v.ship_system).then(|| field.unwrap_or_else(|| system.dominant(ship_pos, &positions)));
        if origin != self.view.origin {
            self.engine.send(universe_sim::Command::LookAt(Some(origin)));
        }
        self.view = View { origin, system, positions, ship_pos, reference, cache };
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
                // (Round a ship in its own frame: about its up, over its top.)
                let frame = match self.observer.focus {
                    Focus::Ship => self.place(Who::Me).1,
                    Focus::Craft(i) => self.place(Who::Craft(i)).1,
                    _ => universe_engine::glam::DQuat::IDENTITY,
                };
                self.camera = self.observer.camera_in(focus, frame);
                self.chase_turn = None;
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
                // Spine to a rock (closing on it, or anchored): the chase view
                // off to the side, upright with the ship: its top up, the rock above it.
                let over_rock = matches!(ship.state, ShipState::Anchored { .. })
                    || self.v.avionics.following.is_some_and(|f| matches!(f.manoeuvre, universe_sim::avionics::follow::Manoeuvre::Surface(_)));
                let (chase, orientation) = if over_rock && self.chase_cam {
                    // Off to one side and a little behind, level with the gap
                    // between ship and rock, the rock above: the gear at work.
                    let at = DVec3::new(75.0, 12.0, 60.0);
                    let look = universe_sim::ship::facing(-(turned * at).normalize(), turned * DVec3::Y);
                    (at, look.as_quat())
                } else if self.chase_cam {
                    // Behind on a soft arm: its turn eased toward the ship's (a third
                    // of a second to catch up), so it trails into a turn and the ship
                    // swings through the frame; a hard turn past a quarter-circle snaps it back.
                    let eased = match self.chase_turn {
                        Some(q) if q.angle_between(turned) < 1.6 => q.slerp(turned, 1.0 - (-dt / 0.33).exp()),
                        _ => turned,
                    };
                    self.chase_turn = Some(eased);
                    (DVec3::new(0.0, 20.0, 115.0), eased.as_quat())
                } else {
                    (DVec3::ZERO, orientation)
                };
                let frame = if over_rock { turned } else { self.chase_turn.unwrap_or(turned) };
                let offset = if self.chase_cam { frame * chase } else { DVec3::ZERO };
                self.camera = Camera { position: self.view.ship_pos + offset, orientation, near: 0.5, ..Default::default() };
                self.prev_focus = None;
            }
        }
    }
}

impl Game for App {
    fn update(&mut self, ctx: &mut Context) {
        let dt = ctx.dt as f64;
        // (In the shipyard studio, the game kept to a few cores; out of it, all of them.)
        if self.shipyard.is_some() != self.held_to_cores {
            self.held_to_cores = self.shipyard.is_some();
            hold_to_cores(self.held_to_cores.then(universe_sim::engine::cores));
        }
        // The observer: the live port's questions answered; a recording's frame kept.
        observe::answer(self, ctx);
        observe::frame(self, ctx);
        // (Slow frames written down only while observing.)
        ctx.watch_hitches = self.observing;
        if self.observing && self.observe_port.is_none() && !self.observe_tried {
            // The live port, opened the first time observing comes on.
            self.observe_tried = true;
            self.observe_port = observe::listen();
        }
        if !self.launched {
            self.launched = true;
            sound::launch(ctx);
        }
        // Keys go to the top layer only (see `Layer`): what's open over the world
        // takes them, and nothing under it sees them; the system's keys work in any.
        self.system_keys(ctx);
        // Our hull's inside as laid out, to the world engine when it changes (to walk
        // in); not while it's being designed in the shipyard (a walk-through sends it).
        if self.shipyard.is_none() {
            self.send_layout();
        }
        // Where we are is explored.
        self.explored.insert(self.v.ship_system);
        let top = self.top_layer();
        match top {
            Layer::Shipyard => {
                // (Worked with the mouse: the cursor free; but a test drive steering
                // with it takes it, as flight does.)
                if !universe_prof::time("studio", || shipyard::input(self, ctx)) {
                    self.shipyard = None;
                }
                let want = self.shipyard.as_ref().is_some_and(|y| y.wants_mouse());
                if ctx.cursor_grabbed() != want {
                    ctx.grab_cursor(want);
                }
            }
            Layer::Market => {
                market::input(self, ctx);
            }
            Layer::Economy => {
                if !economy::input(self, ctx) {
                    self.economy_panel = None;
                }
            }
            Layer::Worlds => {
                if !planet_studio::input(self, ctx) {
                    self.planet_studio = None;
                    // (Its world alone goes with it: back to the system's star.)
                    if self.world_frame.take().is_some() {
                        self.world_maps.clear();
                    }
                    if self.studio_world.take().is_some() {
                        self.observer.focus = observer::Focus::Body { system: self.view.origin, body: 0 };
                    }
                }
            }
            Layer::News => self.news_panel = newspanel::input(self, ctx),
            Layer::GalaxyMap => {
                if !galaxymap::input(self, ctx) {
                    self.galaxy_map = None;
                }
            }
            Layer::NavMap => {
                navmap::input(self, ctx);
            }
            Layer::Graphics => self.graphics_panel = graphics::input(self, ctx),
            Layer::Standards => {
                if !standards::input(self, ctx) || !matches!(self.v.ship.state, ShipState::Landed { .. }) {
                    self.standards = None;
                }
            }
            Layer::Passengers => {
                if !passengers::input(self, ctx) {
                    self.passengers = None;
                }
            }
            Layer::World => self.world_keys(ctx),
        }
        // The world flies (or watches) only with nothing over it, and not the
        // frame a panel opened from it.
        let (controls, focus_changed) = match self.mode {
            _ if top != Layer::World || self.top_layer() != Layer::World => (Controls::default(), false),
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
        }
        // One guide on screen at a time, on the one set of frames: a follow
        // program's while one flies the ship (a clearance may stand meanwhile),
        // else the clearance's — on each new plan, or at once when it takes
        // the guide back.
        if v.avionics.following.is_none() {
            match (&self.plan, v.avionics.clearance) {
                (Some(plan), Some(c)) => {
                    let key = scene::GuideKey::Clearance(c.target);
                    if self.plan_age == 0.0 || self.guide.key() != Some(key) {
                        self.guide.update(plan, key);
                    }
                }
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
        self.update_net();
        // The outlets hear and put out their digests; we hear what reaches us, digests too.
        let room = self.newsroom.get_or_insert_with(|| universe_sim::newsroom::Newsroom::new(&self.charts, self.v.time));
        room.update(&self.charts, self.v.time, &self.v.kills, &self.v.trade_log);
        let casts = room.broadcasts();
        let in_tube = matches!(self.ship.state, ShipState::Transit { .. });
        let us = universe_sim::news::Listener { system: self.v.ship_system, at: self.ship.position, comm: self.ship.spec().comm, player: true, in_tube };
        self.news.update(&self.charts, self.v.time, &us, &universe_sim::news::Happenings { kills: &self.v.kills, trades: &self.v.trade_log, broadcasts: &casts, sightings: &[] });
        // Where things are drawn is the moment drawn: the nav target and the
        // approach guidance are worked out here, at it, from the charts (the
        // view's are a tick off it).
        self.nav_marker = self.nav_marker_now();
        self.approach = self.approach_now();
        // A follow program's way, and its frames on the same guide.
        self.follow_plan = followguide::plan(self);
        followguide::watch(self);
        // (A lively target's guide is the line alone: no frames to lay.)
        if let (Some((p, even)), Some(f), false) = (&self.follow_plan, self.v.avionics.following, self.liveliness.lively) {
            self.guide.update_spaced(p, scene::GuideKey::Follow(f.anchor), *even);
        }
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
        frame.graphics = self.graphics;
        universe_prof::time("draw/scene", || scene::draw(frame, self));
        // (Dev: the showcased model alone, no HUD: `UNIVERSE_MODEL_CLEAN`.)
        if !(self.showcase.is_some() && std::env::var_os("UNIVERSE_MODEL_CLEAN").is_some()) {
            universe_prof::time("draw/hud", || hud::draw(frame, self, ctx));
        }
        if self.graphics_panel {
            graphics::draw(frame, self);
        }
    }
}

/// True when the ship is visible as a model rather than being the camera.
pub fn ship_visible(app: &App) -> bool {
    let view = match app.v.crew.place {
        _ if app.mode == Mode::Observer => true,
        universe_sim::world::Place::Seat => app.chase_cam,
        // (Standing in it: in its own modelled spaces.)
        universe_sim::world::Place::Aboard { .. } => true,
        universe_sim::world::Place::Outside { .. } => true,
    };
    !matches!(app.v.ship.state, ShipState::Destroyed { .. } | ShipState::Transit { .. }) && view
}

/// The galaxy's stars seen from a system: the system, and each star's direction and colour.
pub type SkyCache = (usize, Vec<(universe_engine::glam::Vec3, universe_engine::Color)>);

/// The game kept to `n` cores while the shipyard studio is open (Linux: its
/// threads, now and later, may run on those only), or (None) let go to every
/// core it was allowed at the start. Elsewhere, nothing.
fn hold_to_cores(n: Option<usize>) {
    #[cfg(target_os = "linux")]
    // SAFETY: cpu_set_t values made empty, filled with CPUs we're allowed, and handed
    // to the kernel for this process's threads; nothing else touches them.
    unsafe {
        static ALLOWED: std::sync::OnceLock<libc::cpu_set_t> = std::sync::OnceLock::new();
        let allowed = *ALLOWED.get_or_init(|| {
            let mut allowed: libc::cpu_set_t = std::mem::zeroed();
            libc::sched_getaffinity(0, std::mem::size_of::<libc::cpu_set_t>(), &mut allowed);
            allowed
        });
        let set = match n {
            Some(n) => {
                let mut set: libc::cpu_set_t = std::mem::zeroed();
                let mut taken = 0;
                for cpu in 0..libc::CPU_SETSIZE as usize {
                    if taken < n && libc::CPU_ISSET(cpu, &allowed) {
                        libc::CPU_SET(cpu, &mut set);
                        taken += 1;
                    }
                }
                set
            }
            None => allowed,
        };
        // (Every thread: rayon's and the engine's, one by one.)
        if let Ok(tasks) = std::fs::read_dir("/proc/self/task") {
            for t in tasks.flatten() {
                if let Ok(tid) = t.file_name().to_string_lossy().parse::<libc::pid_t>() {
                    libc::sched_setaffinity(tid, std::mem::size_of::<libc::cpu_set_t>(), &set);
                }
            }
        }
    }
    #[cfg(not(target_os = "linux"))]
    let _ = n;
}

fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info,wgpu_core=warn,wgpu_hal=warn"))
        .init();
    // (The cores shared out before anything starts using them: see `thread_budget`;
    // what the game may use at all, noted before the shipyard holds it to fewer.)
    // (`--report <design>`: the interior studio's report on a design, as text; no
    // window. For checking a change without a game run.)
    let args: Vec<String> = std::env::args().collect();
    // (`--studio [design]`: the interior studio alone, no universe loaded.)
    if let Some(k) = args.iter().position(|a| a == "--studio") {
        let design = args.get(k + 1).filter(|a| !a.starts_with("--")).map(String::as_str);
        run(Config { title: "Freefall studio".into(), ..Default::default() }, studio_only::StudioOnly::new(design));
        return;
    }
    // (`--fit <design>`: its frame fitted (mounted, sized, braced) and saved, the
    // old one kept in backups/.)
    if let Some(k) = args.iter().position(|a| a == "--fit") {
        let name = args.get(k + 1).map_or("design-1", String::as_str);
        print!("{}", interior::fit_design(name));
        return;
    }
    // (`--compare <a> <b>`: two designs' key figures side by side.)
    if let Some(k) = args.iter().position(|a| a == "--compare") {
        let (a, b) = (args.get(k + 1).map_or("design-1", String::as_str), args.get(k + 2).map_or("design-2", String::as_str));
        print!("{}", interior::compare_designs(a, b));
        return;
    }
    if let Some(k) = args.iter().position(|a| a == "--report") {
        let name = args.get(k + 1).map_or("design-1", String::as_str);
        print!("{}", interior::report(name));
        return;
    }
    universe_sim::engine::size_thread_pools();
    hold_to_cores(None);
    // (Slow frames written down beside the quicksave: hitches.log.)
    let hitch_log = Some(save::data_dir().join("freefall").join("hitches.log"));
    if let Some(dir) = hitch_log.as_ref().and_then(|p| p.parent()) {
        let _ = std::fs::create_dir_all(dir);
    }
    run(Config { title: "Freefall".into(), hitch_log, ..Default::default() }, App::new());
}
