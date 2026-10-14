//! The planet viewer as an app of its own (`docs/miniapps-plan.md` step 5): every world the planet
//! simulation has released, one looked at from an orbit round it, drawn by the shared planet view.
//! It opens without the universe: its own world instance (the charted home system, or a world shown
//! alone round its star), its own clocks (Celestial and Planetary, both functions of time today, so
//! they only move its time), its own caches. The keys are the WORLDS panel's: up and down pick,
//! ENTER goes, PAGE UP and DOWN move through the world's history, L changes the look, ESC closes;
//! a drag or left and right turn, the wheel or W and S zoom.

use std::f64::consts::FRAC_PI_2;
use std::sync::Arc;

use universe_engine::apps::{App, Next};
use universe_engine::glam::{DVec3, Vec2};
use universe_engine::{Camera, Color, Context, Frame, KeyCode, Light, MouseButton};
use universe_sim::clocks::{Runtime, Subscriptions};
use universe_sim::world::registry::ClockKey;
use universe_sim::{BodyKind, StarSystem};

use crate::planet_studio::{self, Look, PlanetStudio, WorldFrame};
use crate::planet_view::{PlanetView, Sight};

/// What its globes are kept under (it has one system in view at a time).
const CACHE: usize = 0;

pub struct PlanetApp {
    studio: PlanetStudio,
    look: Look,
    then: Option<WorldFrame>,
    /// The charted home system: the game's worlds, and the star a world not in the game is shown round.
    home: Arc<StarSystem>,
    /// The system shown, and the world looked at in it.
    system: Arc<StarSystem>,
    body: usize,
    view: PlanetView,
    clocks: Runtime,
    positions: Vec<DVec3>,
    yaw: f64,
    pitch: f64,
    distance: f64,
}

impl PlanetApp {
    /// The viewer at the home system's first baked world, or at world `id` (a release's world id).
    pub fn new(id: Option<&str>) -> Result<Self, String> {
        let world = universe_sim::world::World::new(crate::seed());
        Self::with_home(world.charts().system(world.home_system), id)
    }

    /// The viewer over a charted system already at hand (the game's home system, from its charts).
    pub fn with_home(home: Arc<StarSystem>, id: Option<&str>) -> Result<Self, String> {
        let clocks = Runtime::new(&Subscriptions::of(&[ClockKey::Celestial, ClockKey::Planetary]), 1.0, 1000)?;
        let body = home.bodies.iter().position(|b| b.terrain.as_ref().is_some_and(|t| t.baked())).unwrap_or(0);
        let mut app = PlanetApp {
            studio: PlanetStudio::open(),
            look: Look::default(),
            then: None,
            system: home.clone(),
            home,
            body,
            view: PlanetView::default(),
            clocks,
            positions: vec![],
            yaw: 0.6,
            pitch: 0.25,
            distance: 0.0,
        };
        app.distance = app.system.bodies[body].rail.radius * 3.0;
        // (At world `id`, else the home world: the first of the home system's worlds with a bake and
        // air, as the game's start is; through the list, so its cursor and timeline are that world's.)
        let home_world = app.home.bodies.iter().find(|b| b.rail.atmosphere.is_some() && b.terrain.as_ref().is_some_and(|t| t.baked())).map(|b| b.key.clone());
        let k = match id {
            Some(id) => Some(app.studio.list.iter().position(|r| r.world_id == id).ok_or_else(|| format!("no world {id} in the worlds store"))?),
            None => app.studio.list.iter().position(|r| r.body.is_some() && r.body == home_world),
        };
        if let Some(k) = k {
            app.studio.selected = k;
            app.go(k)?;
        }
        app.update_positions();
        Ok(app)
    }

    /// Go to the studio's world `k`: the game's body in the home system, or the world alone.
    fn go(&mut self, k: usize) -> Result<(), String> {
        let r = self.studio.list.get(k).cloned().ok_or("NO WORLDS: IS THE WORLDS STORE HERE? (UNIVERSE_WORLDS)")?;
        let history = match universe_sim::world::worlds::History::release(&r) {
            Some(Ok(h)) => Some(Arc::new(h)),
            Some(Err(e)) => {
                log::warn!("{}: its history can't be read ({e})", r.world_id);
                None
            }
            None => None,
        };
        let in_home = r.body.as_deref().and_then(|key| self.home.bodies.iter().position(|b| b.key == key));
        let (system, body) = match in_home {
            Some(i) => (self.home.clone(), i),
            None => (Arc::new(planet_studio::alone(&self.home, &r)?), 1),
        };
        let key = r.body.clone().unwrap_or_else(|| format!("studio.{}", r.world_id));
        self.studio.shown = Some((key, history));
        self.then = None;
        self.view = PlanetView::default();
        self.distance = system.bodies[body].rail.radius * 3.0;
        (self.system, self.body) = (system, body);
        // (Looked at from the sun's side, a little round towards evening.)
        self.update_positions();
        if let Some(star) = self.system.bodies.iter().position(|b| b.kind == BodyKind::Star) {
            let sun = (self.positions[star] - self.positions[body]).normalize_or_zero();
            (self.yaw, self.pitch) = (sun.x.atan2(sun.z) + 0.6, 0.25);
        }
        Ok(())
    }

    /// Shown in look `look` (its maps made again).
    pub fn set_look(&mut self, look: Look) {
        self.look = look;
        self.view.world_maps.clear();
    }

    /// Frame `n` of the shown world's history in place of today, if it has one.
    pub fn show_frame(&mut self, n: usize) {
        if let Some((key, Some(history))) = self.studio.shown.clone()
            && n < history.frames.len()
        {
            self.then = Some(WorldFrame { key, history, frame: n });
            self.view.world_maps.clear();
        }
    }

    fn update_positions(&mut self) {
        self.system.positions(self.clocks.time(), &mut self.positions);
    }

    fn sight(&self) -> Sight<'_> {
        Sight { system: &self.system, cache: CACHE, positions: &self.positions, t: self.clocks.time(), show_grid: false, outside: false, underfoot: None }
    }
}

impl App for PlanetApp {
    fn name(&self) -> &str {
        "planet"
    }

    /// Over the game, the game runs on under it (its world, its sounds), not drawn.
    fn cover(&self) -> universe_engine::apps::Cover {
        universe_engine::apps::Cover::Covers
    }

    fn update(&mut self, ctx: &mut Context) -> Next {
        let _scope = universe_prof::scope("update/planet");
        crate::visual_assets::maintain();
        let input = &ctx.input;
        if input.pressed(KeyCode::Escape) {
            return Next::Close;
        }
        let n = self.studio.list.len().max(1);
        if input.pressed(KeyCode::ArrowDown) {
            self.studio.selected = (self.studio.selected + 1) % n;
        }
        if input.pressed(KeyCode::ArrowUp) {
            self.studio.selected = (self.studio.selected + n - 1) % n;
        }
        if input.pressed(KeyCode::Enter) {
            self.studio.note = self.go(self.studio.selected).err().unwrap_or_default();
        }
        let step = match (input.pressed(KeyCode::PageUp), input.pressed(KeyCode::PageDown)) {
            (true, false) => -1,
            (false, true) => 1,
            _ => 0,
        };
        if let Some(next) = planet_studio::timeline(&self.studio, self.then.as_ref(), step) {
            self.then = next;
            self.view.world_maps.clear();
        }
        if input.pressed(KeyCode::KeyL) {
            self.look = self.look.next();
            self.view.world_maps.clear();
        }
        // (Turned by a drag or left and right, zoomed by the wheel or W and S, as the observer is; up
        // and down pick a world, as in the WORLDS panel.)
        if input.button_down(MouseButton::Left) || input.button_down(MouseButton::Right) {
            self.yaw -= input.mouse_delta.x as f64 * 0.006;
            self.pitch = (self.pitch + input.mouse_delta.y as f64 * 0.006).clamp(-FRAC_PI_2 + 0.01, FRAC_PI_2 - 0.01);
        }
        self.yaw += input.axis(KeyCode::ArrowRight, KeyCode::ArrowLeft) as f64 * 1.2 * ctx.dt as f64;
        let zoom = input.scroll as f64 + input.axis(KeyCode::KeyS, KeyCode::KeyW) as f64 * ctx.dt as f64 * 4.0;
        let radius = self.system.bodies[self.body].rail.radius;
        self.distance = (self.distance * 0.85f64.powf(zoom)).clamp(radius * 1.0005, radius * 200.0);
        // Its own clocks: the world's time moves on (nothing of theirs is stepped yet).
        self.clocks.advance(ctx.dt as f64, &mut |_, _, _| {});
        self.update_positions();
        let camera = self.camera();
        self.view.prepare(&self.system, CACHE, &self.positions, &camera, self.look, self.then.clone());
        Next::Stay
    }

    fn camera(&self) -> Camera {
        let target = self.positions.get(self.body).copied().unwrap_or_default();
        let dir = DVec3::new(self.pitch.cos() * self.yaw.sin(), self.pitch.sin(), self.pitch.cos() * self.yaw.cos());
        let mut camera = Camera { position: target + dir * self.distance, ..Default::default() };
        camera.look_at(target, universe_engine::glam::Vec3::Y);
        camera
    }

    fn draw(&self, frame: &mut Frame, _: &Context) {
        crate::visuals::begin_view(&frame.camera, DVec3::ZERO, "planet".into());
        let sys = &self.system;
        let star = sys.bodies.iter().position(|b| b.kind == BodyKind::Star);
        let tint = sys.class.color().map(|c| 0.2 * c + 0.8);
        let top = tint.iter().copied().fold(0.0, f32::max);
        frame.light = star.map(|i| Light {
            position: self.positions[i],
            color: tint.map(|c| c / top),
            luminosity: sys.luminosity,
            reference: universe_sim::units::AU,
            radius: sys.bodies[i].rail.radius,
        });
        frame.eclipsers = sys.bodies.iter().enumerate().filter(|(_, b)| !b.kind.artificial() && b.kind != BodyKind::Star).map(|(i, b)| (self.positions[i], b.rail.radius)).collect();
        let sight = self.sight();
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
        planet_studio::panel(frame, &self.studio, self.look, self.then.as_ref());
        let b = &sys.bodies[self.body];
        let line = format!("{}  RADIUS {:.0} KM  {:.0} KM OFF   DRAG OR LEFT/RIGHT TURN  WHEEL OR W/S ZOOM", b.name.to_uppercase(), b.rail.radius / 1000.0, (self.distance - b.rail.radius) / 1000.0);
        frame.text(Vec2::new(12.0, frame.size().y - 24.0), &line, Color::hex(0x91adbb));
    }
}
