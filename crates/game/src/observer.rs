use std::f64::consts::FRAC_PI_2;

use serde::{Deserialize, Serialize};
use universe_engine::glam::{DVec3, Vec3};
use universe_engine::{Camera, Context, KeyCode, MouseButton};
use universe_sim::world::charts::Charts;
use universe_sim::View;

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum Focus {
    Body { system: usize, body: usize },
    Ship,
    /// One of the other ships (index into the view's crafts).
    Craft(usize),
}

/// God's-eye camera orbiting a focus target, from a hull plate to galaxy scale.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Observer {
    pub focus: Focus,
    pub yaw: f64,
    pub pitch: f64,
    pub distance: f64,
    /// Remaining offset of an in-flight focus change; decays to zero.
    #[serde(skip)]
    pub transition: DVec3,
    #[serde(skip)]
    star_cycle: Option<(usize, usize)>, // (base system, index into its neighbour list)
}

pub const MAX_DISTANCE: f64 = 6.0e21;
const SHIP_SIZE: f64 = 30.0;

impl Observer {
    pub fn new() -> Self {
        Self { focus: Focus::Ship, yaw: 0.6, pitch: 0.25, distance: 180.0, transition: DVec3::ZERO, star_cycle: None }
    }

    /// The galaxy index of the system this camera is looking at.
    pub fn origin(&self, u: &View) -> usize {
        match self.focus {
            Focus::Body { system, .. } => system,
            Focus::Ship => u.ship_system,
            Focus::Craft(i) => u.crafts.get(i).map_or(u.ship_system, |c| c.system),
        }
    }

    pub fn focus_radius(&self, charts: &Charts) -> f64 {
        match self.focus {
            Focus::Body { system, body } => charts.system(system).bodies[body].rail.radius,
            Focus::Ship | Focus::Craft(_) => SHIP_SIZE,
        }
    }

    /// Handle observer input. Returns true if the focus target changed.
    pub fn input(&mut self, ctx: &Context, u: &View, charts: &Charts) -> bool {
        let input = &ctx.input;
        if input.button_down(MouseButton::Left) || input.button_down(MouseButton::Right) {
            self.yaw -= input.mouse_delta.x as f64 * 0.006;
            self.pitch = (self.pitch + input.mouse_delta.y as f64 * 0.006).clamp(-FRAC_PI_2 + 0.01, FRAC_PI_2 - 0.01);
        }
        let orbit_keys = 1.2 * ctx.dt as f64;
        self.yaw += input.axis(KeyCode::ArrowRight, KeyCode::ArrowLeft) as f64 * orbit_keys;
        self.pitch = (self.pitch + input.axis(KeyCode::ArrowDown, KeyCode::ArrowUp) as f64 * orbit_keys)
            .clamp(-FRAC_PI_2 + 0.01, FRAC_PI_2 - 0.01);
        let zoom = input.scroll as f64 + input.axis(KeyCode::KeyS, KeyCode::KeyW) as f64 * ctx.dt as f64 * 4.0;
        self.distance *= 0.85f64.powf(zoom);

        let old = self.focus;
        let origin = self.origin(u);
        let sys = charts.system(origin);
        let current_body = match self.focus {
            Focus::Body { body, .. } => body,
            Focus::Ship | Focus::Craft(_) => 0,
        };
        let n = sys.bodies.len();
        if input.pressed(KeyCode::BracketRight) || input.pressed(KeyCode::BracketLeft) {
            let step = if input.pressed(KeyCode::BracketRight) { 1 } else { n - 1 };
            let body = (current_body + step) % n;
            self.focus = Focus::Body { system: origin, body };
            self.distance = sys.bodies[body].rail.radius * 5.0;
            self.star_cycle = None;
        }
        if crate::keys::pressed(input, crate::keys::Act::NextStar) || crate::keys::pressed(input, crate::keys::Act::PreviousStar) {
            let (base, index) = self.star_cycle.unwrap_or((origin, usize::MAX));
            let list = charts.galaxy.nearest(base, 12);
            let index = match (crate::keys::pressed(input, crate::keys::Act::NextStar), index) {
                (true, usize::MAX) => 0,
                (true, i) => (i + 1) % list.len(),
                (false, usize::MAX) => list.len() - 1,
                (false, i) => (i + list.len() - 1) % list.len(),
            };
            self.star_cycle = Some((base, index));
            self.focus = Focus::Body { system: list[index], body: 0 };
        }
        if input.pressed(KeyCode::Home) {
            self.focus = Focus::Body { system: charts.home_system, body: 0 };
            self.star_cycle = None;
        }
        if crate::keys::pressed(input, crate::keys::Act::TrackSettler) && !u.crafts.is_empty() {
            // Follow the next settler.
            let next = match self.focus {
                Focus::Craft(i) => (i + 1) % u.crafts.len(),
                _ => 0,
            };
            self.focus = Focus::Craft(next);
            self.distance = 250.0;
            self.star_cycle = None;
        }
        if crate::keys::pressed(input, crate::keys::Act::Home) {
            self.focus = Focus::Ship;
            self.distance = 180.0;
            self.star_cycle = None;
        }

        let min = self.focus_radius(charts) * 1.2;
        self.distance = self.distance.clamp(min, MAX_DISTANCE);
        self.focus != old
    }

    pub fn camera(&self, target: DVec3) -> Camera {
        let dir = DVec3::new(self.pitch.cos() * self.yaw.sin(), self.pitch.sin(), self.pitch.cos() * self.yaw.cos());
        let target = target + self.transition;
        let mut camera = Camera { position: target + dir * self.distance, near: 1.0, ..Default::default() };
        camera.look_at(target, Vec3::Y);
        camera
    }
}
