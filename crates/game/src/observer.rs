
use serde::{Deserialize, Serialize};
use universe_engine::glam::DVec3;
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
    /// Its orbit round the focus (saved as its own yaw, pitch and distance, as before).
    #[serde(flatten)]
    pub orbit: crate::orbit_cam::OrbitCam,
    /// Remaining offset of an in-flight focus change; decays to zero.
    #[serde(skip)]
    pub transition: DVec3,
    #[serde(skip)]
    star_cycle: Option<(usize, usize)>, // (base system, index into its neighbour list)
    /// The focused ship in the studio: alone, on a neutral backdrop, under
    /// studio lights (to judge a design as in a modelling tool), not as space lights it.
    #[serde(skip)]
    pub studio: bool,
    /// Development inspection offset in the focused ship frame (COM-relative).
    #[serde(skip)]
    pub inspection_offset: DVec3,
}

pub const MAX_DISTANCE: f64 = 6.0e21;
const SHIP_SIZE: f64 = 30.0;

impl Observer {
    pub fn new() -> Self {
        Self { focus: Focus::Ship, orbit: crate::orbit_cam::OrbitCam::new(0.6, 0.25, 180.0), transition: DVec3::ZERO, star_cycle: None, studio: false, inspection_offset: DVec3::ZERO }
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
        self.orbit.drag(input, &[MouseButton::Left, MouseButton::Right], (-crate::orbit_cam::PITCH_LIMIT, crate::orbit_cam::PITCH_LIMIT));
        self.orbit.keys(input, ctx.dt as f64, Some((-crate::orbit_cam::PITCH_LIMIT, crate::orbit_cam::PITCH_LIMIT)));
        self.orbit.zoom(input, ctx.dt as f64, true, (0.0, f64::INFINITY));

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
            self.orbit.distance = sys.bodies[body].rail.radius * 5.0;
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
            self.orbit.distance = 250.0;
            self.star_cycle = None;
        }
        // The studio: only for a ship.
        if crate::keys::pressed(input, crate::keys::Act::Studio) {
            self.studio = !self.studio && matches!(self.focus, Focus::Ship | Focus::Craft(_));
        }
        if !matches!(self.focus, Focus::Ship | Focus::Craft(_)) {
            self.studio = false;
        }
        if crate::keys::pressed(input, crate::keys::Act::Home) {
            self.focus = Focus::Ship;
            self.inspection_offset = DVec3::ZERO;
            self.orbit.distance = 180.0;
            self.star_cycle = None;
        }

        let min = if matches!(self.focus, Focus::Ship) && self.inspection_offset != DVec3::ZERO {
            0.1 // Explicit equipment inspection may enter the hull's bounding sphere.
        } else {
            self.focus_radius(charts) * 1.2
        };
        self.orbit.distance = self.orbit.distance.clamp(min, MAX_DISTANCE);
        self.focus != old
    }

    pub fn camera(&self, target: DVec3) -> Camera {
        self.camera_in(target, glam::DQuat::IDENTITY)
    }

    /// Orbiting `target` in `frame` (a ship's turn: yaw about its up, pitch over its top).
    pub fn camera_in(&self, target: DVec3, frame: glam::DQuat) -> Camera {
        let target = target + self.transition;
        let near = if matches!(self.focus, Focus::Ship) && self.inspection_offset != DVec3::ZERO { 0.02 } else { 1.0 };
        Camera { near, ..self.orbit.camera(target, frame) }
    }
}
