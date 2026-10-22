//! Scenes: an app that runs a scenario file (`content/scenes/*.scene.yaml`) in a world instance of
//! its own (`docs/miniapps-plan.md` step 8): a lab copy of the charted home system, the crafts the
//! file places, and only the clocks the file names. Realtime steps the crafts through the world's
//! own ship step (no copies of its laws); Celestial and Planetary move its time (both are functions
//! of time today). A clock with no work in a scene yet is refused when the file is read.
//!
//! ```yaml
//! name: Two orbits round Heath
//! clocks: [clock.realtime, clock.celestial]
//! scale: 20                      # game seconds a real second
//! look_at: body.treistun.treistun-d   # a body's key, or a craft's name
//! distance_radii: 4
//! crafts:
//!   - { name: LOW, around: body.treistun.treistun-d, altitude_m: 400000 }
//!   - { name: HIGH, around: body.treistun.treistun-d, altitude_m: 2000000, inclination_deg: 40 }
//! ```
//!
//! SPACE pauses and runs it, R starts it over, a drag or left and right turn, the wheel zooms, ESC
//! closes.

use std::sync::Arc;

use serde::Deserialize;
use universe_engine::apps::{App, Next};
use universe_engine::glam::{DQuat, DVec3, Vec2};
use universe_engine::{Camera, Color, Context, Frame, KeyCode, Light, MouseButton};
use universe_sim::clocks::{Runtime, Subscriptions};
use universe_sim::world::registry::ClockKey;
use universe_sim::world::World;
use universe_sim::{BodyKind, Galaxy, GalaxyStar, Ship, ShipCommands, ShipEvent, StarSystem};

use crate::planet_view::{PlanetView, Sight};

/// A scene as its file gives it.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SceneFile {
    pub name: String,
    /// Clock records' keys (`clock.realtime`...).
    pub clocks: Vec<String>,
    #[serde(default = "one")]
    pub scale: f64,
    pub look_at: String,
    #[serde(default = "four")]
    pub distance_radii: f64,
    #[serde(default)]
    pub crafts: Vec<CraftFile>,
}

/// A craft placed in a scene: on a circular orbit round a body, the starter hull.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CraftFile {
    pub name: String,
    pub around: String,
    pub altitude_m: f64,
    #[serde(default)]
    pub inclination_deg: f64,
}

fn one() -> f64 {
    1.0
}
fn four() -> f64 {
    4.0
}

/// The clocks a scene can run, and what each does in one.
const RUNNABLE: &[ClockKey] = &[ClockKey::Realtime, ClockKey::Celestial, ClockKey::Planetary];

/// A trail point is kept every this many game seconds, this many of them.
const TRAIL_EVERY: f64 = 10.0;
const TRAIL: usize = 720;

struct Craft {
    name: String,
    ship: Ship,
    commands: ShipCommands,
    events: Vec<ShipEvent>,
    /// Where it has been, relative to the body it was placed round.
    trail: Vec<DVec3>,
    around: usize,
}

pub struct SceneApp {
    file: SceneFile,
    path: String,
    world: World,
    crafts: Vec<Craft>,
    clocks: Runtime,
    running: bool,
    view: PlanetView,
    positions: Vec<DVec3>,
    trail_at: f64,
    orbit: crate::orbit_cam::OrbitCam,
}

impl SceneApp {
    /// Scene file `path`, read and set up (not yet run).
    pub fn open(path: &str) -> Result<Self, String> {
        let text = std::fs::read_to_string(path).map_err(|e| format!("{path}: {e}"))?;
        let file: SceneFile = serde_norway::from_str(&text).map_err(|e| format!("{path}: {e}"))?;
        Self::new(file, path)
    }

    fn new(file: SceneFile, path: &str) -> Result<Self, String> {
        let mut keys = vec![];
        for k in &file.clocks {
            let key = ClockKey::of(k).ok_or_else(|| format!("no clock {k} in the registry"))?;
            if !RUNNABLE.contains(&key) {
                return Err(format!("{k} has no work in a scene yet (a scene runs {})", RUNNABLE.iter().map(|k| k.key()).collect::<Vec<_>>().join(", ")));
            }
            keys.push(key);
        }
        if !(file.scale.is_finite() && file.scale > 0.0) {
            return Err("scale: a positive number of game seconds a real second".into());
        }
        let clocks = Runtime::new(&Subscriptions::of(&keys), file.scale, 100_000)?;
        // (A lab copy of the charted home system, a world of its own: one star, system 0.)
        let charted = World::new(crate::seed());
        let home = charted.system(charted.home_system);
        // (Its star and bodies; no ports, fields or belts, and its gates lead nowhere: a lab of one star.)
        let mut bodies = home.bodies.clone();
        for b in bodies.iter_mut().filter(|b| b.kind == BodyKind::Gate) {
            b.link = None;
        }
        let system = StarSystem {
            index: 0,
            name: home.name.clone(),
            class: home.class,
            luminosity: home.luminosity,
            bodies,
            spaceports: Vec::new(),
            fields: Vec::new(),
            small: home.small.clone(),
            belts: Vec::new(),
            belt_seed: 0,
            patches: Default::default(),
        };
        let galaxy = Galaxy { seed: charted.galaxy.seed, stars: vec![GalaxyStar { position: DVec3::ZERO, ..charted.galaxy.stars[charted.home_system].clone() }] };
        let world = World::isolated(galaxy, system)?;
        let sys = world.system(0);
        let body = |key: &str| sys.bodies.iter().position(|b| b.key == key).ok_or_else(|| format!("no body {key} in the home system"));
        let mut positions = vec![];
        sys.positions(0.0, &mut positions);
        let mut crafts = vec![];
        for c in &file.crafts {
            let i = body(&c.around)?;
            let b = &sys.bodies[i];
            let r = b.rail.radius + c.altitude_m;
            // (A circular orbit: over the body's equator, turned up by its inclination, at the speed
            // its pull gives at that height.)
            let tilt = DQuat::from_rotation_x(c.inclination_deg.to_radians());
            let out = DVec3::X * r;
            let along = tilt * DVec3::NEG_Z * (b.rail.mu / r).sqrt();
            let position = positions[i] + out;
            let velocity = sys.velocity(i, 0.0) + along;
            let spec = Arc::new(universe_sim::ship::starter().clone());
            let ship = Ship::with_spec(spec, position, velocity, DQuat::IDENTITY);
            let commands = ship.holding();
            crafts.push(Craft { name: c.name.clone(), ship, commands, events: vec![], trail: vec![out], around: i });
        }
        let target_radius = match body(&file.look_at) {
            Ok(i) => sys.bodies[i].rail.radius,
            Err(_) if crafts.iter().any(|c| c.name == file.look_at) => 100.0,
            Err(e) => return Err(format!("look_at: {e}, nor a craft's name")),
        };
        let distance = target_radius * file.distance_radii;
        let mut app = SceneApp { file, path: path.into(), world, crafts, clocks, running: true, view: PlanetView::default(), positions, trail_at: 0.0, orbit: crate::orbit_cam::OrbitCam::new(0.6, 0.35, distance) };
        app.update_positions();
        Ok(app)
    }

    fn update_positions(&mut self) {
        self.world.system(0).positions(self.clocks.time(), &mut self.positions);
    }

    /// Where the camera looks: the body or craft named, now.
    fn target(&self) -> DVec3 {
        let sys = self.world.system(0);
        if let Some(i) = sys.bodies.iter().position(|b| b.key == self.file.look_at) {
            return self.positions[i];
        }
        self.crafts.iter().find(|c| c.name == self.file.look_at).map_or(DVec3::ZERO, |c| c.ship.position)
    }

    /// The crafts stepped one realtime tick of `dt` (game seconds), side by side from the same moment.
    fn step_crafts(&mut self, t: f64, dt: f64) {
        let start = t - dt;
        for c in &mut self.crafts {
            let mut clock = start;
            let mut system = 0;
            self.world.step_ship_at(&mut clock, &mut c.ship, &mut system, &c.commands, dt, 1.0, &mut c.events);
            c.events.clear();
        }
        self.world.time = t;
    }
}

impl App for SceneApp {
    fn name(&self) -> &str {
        "scene"
    }

    fn status(&self) -> String {
        let crafts: Vec<_> = self.crafts.iter().map(|c| serde_json::json!({"name": c.name, "position": c.ship.position.to_array(), "velocity": c.ship.velocity.to_array()})).collect();
        serde_json::json!({"scene": self.file.name, "file": self.path, "running": self.running, "time_s": self.clocks.time(), "scale": self.file.scale,
            "clocks": self.file.clocks, "realtime_pending_s": self.clocks.pending(ClockKey::Realtime), "crafts": crafts}).to_string()
    }

    fn update(&mut self, ctx: &mut Context) -> Next {
        let _scope = universe_prof::scope("update/scene");
        crate::visual_assets::maintain();
        let input = &ctx.input;
        if input.pressed(KeyCode::Escape) {
            return Next::Close;
        }
        if input.pressed(KeyCode::KeyR) {
            if let Ok(fresh) = Self::new(self.file.clone(), &self.path) {
                let (yaw, pitch, distance) = (self.orbit.yaw, self.orbit.pitch, self.orbit.distance);
                *self = fresh;
                (self.orbit.yaw, self.orbit.pitch, self.orbit.distance) = (yaw, pitch, distance);
            }
            return Next::Stay;
        }
        if input.pressed(KeyCode::Space) {
            self.running = !self.running;
        }
        self.orbit.drag(input, &[MouseButton::Left, MouseButton::Right], (-crate::orbit_cam::PITCH_LIMIT, crate::orbit_cam::PITCH_LIMIT));
        self.orbit.keys(input, ctx.dt as f64, None);
        self.orbit.zoom(input, ctx.dt as f64, false, (10.0, f64::INFINITY));
        // Its clocks, and only its: realtime steps the crafts; the others move its time.
        if self.running {
            let mut ticks = vec![];
            self.clocks.advance(ctx.dt as f64, &mut |k, t, dt| {
                if k == ClockKey::Realtime {
                    ticks.push((t, dt));
                }
            });
            for (t, dt) in ticks {
                self.step_crafts(t, dt);
                if t >= self.trail_at + TRAIL_EVERY {
                    self.trail_at = t;
                    self.update_positions();
                    for c in &mut self.crafts {
                        let p = c.ship.position - self.positions[c.around];
                        c.trail.push(p);
                        if c.trail.len() > TRAIL {
                            c.trail.remove(0);
                        }
                    }
                }
            }
        }
        self.update_positions();
        let camera = self.camera();
        self.view.prepare(&self.world.system(0), 0, &self.positions, &camera, Default::default(), None);
        Next::Stay
    }

    fn camera(&self) -> Camera {
        let target = self.target();
        self.orbit.camera(target, universe_engine::glam::DQuat::IDENTITY)
    }

    fn draw(&self, frame: &mut Frame, _: &Context) {
        crate::visuals::begin_view(&frame.camera, DVec3::ZERO, "scene".into());
        let sys = self.world.system(0);
        let star = sys.bodies.iter().position(|b| b.kind == BodyKind::Star);
        let tint = sys.class.color().map(|c| 0.2 * c + 0.8);
        let top = tint.iter().copied().fold(0.0, f32::max);
        frame.light = star.map(|i| Light { position: self.positions[i], color: tint.map(|c| c / top), luminosity: sys.luminosity, reference: universe_sim::units::AU, radius: sys.bodies[i].rail.radius });
        frame.eclipsers = sys.bodies.iter().enumerate().filter(|(_, b)| !b.kind.artificial() && b.kind != BodyKind::Star).map(|(i, b)| (self.positions[i], b.rail.radius)).collect();
        let sight = Sight { system: &sys, cache: 0, positions: &self.positions, t: self.clocks.time(), show_grid: false, outside: false, underfoot: None };
        self.view.draw_maps(frame, &sight);
        for (i, b) in sys.bodies.iter().enumerate() {
            if b.kind.artificial() || b.kind.is_rock() {
                continue;
            }
            let center = self.positions[i];
            let c = crate::scene::color(b.color);
            let px = frame.projected_radius(center, b.rail.radius);
            if px < 1.5 || !self.view.draw_world(frame, &sight, i, center, px, c) {
                frame.point(center, c);
                continue;
            }
            if b.rail.atmosphere.is_some() {
                self.view.atmosphere(frame, &sight, i, center);
            }
        }
        // The crafts: each a mark and its name, its trail behind it.
        let mark = Color::hex(0xe5c778);
        for c in &self.crafts {
            let base = self.positions[c.around];
            for w in c.trail.windows(2) {
                frame.line(base + w[0], base + w[1], mark.scale(0.5));
            }
            if let Some(last) = c.trail.last() {
                frame.line(base + *last, c.ship.position, mark.scale(0.5));
            }
            frame.point(c.ship.position, mark);
            frame.world_text(c.ship.position, &c.name, mark, 1.0, false);
        }
        let line = universe_engine::GLYPH + 4.0;
        let dim = Color::hex(0x91adbb);
        let t = self.clocks.time();
        frame.text(Vec2::new(12.0, 12.0), &format!("SCENE  {}", self.file.name.to_uppercase()), Color::hex(0xe5c778));
        frame.text(Vec2::new(12.0, 12.0 + line), &format!("{}  {:.0}:{:02.0}:{:02.0}  X{}  CLOCKS {}", if self.running { "RUNNING" } else { "PAUSED" }, (t / 3600.0).floor(), (t / 60.0).floor() % 60.0, t.floor() % 60.0, self.file.scale, self.file.clocks.iter().map(|c| c.trim_start_matches("clock.").to_uppercase()).collect::<Vec<_>>().join(" ")), dim);
        for (k, c) in self.crafts.iter().enumerate() {
            let b = &sys.bodies[c.around];
            let alt = (c.ship.position - self.positions[c.around]).length() - b.rail.radius;
            let speed = (c.ship.velocity - sys.velocity(c.around, t)).length();
            frame.text(Vec2::new(12.0, 12.0 + line * (2.5 + k as f32)), &format!("{:<8} {:>9.1} KM OVER {}  {:>7.0} M/S", c.name, alt / 1000.0, b.name.to_uppercase(), speed), dim);
        }
        frame.text(Vec2::new(12.0, frame.size().y - 24.0), "SPACE RUN/PAUSE  R START OVER  DRAG OR LEFT/RIGHT TURN  WHEEL ZOOM  ESC CLOSE", dim);
    }
}
