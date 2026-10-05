//! The shipyard: the interior studio (see `interior`), on the hull we fly. Open
//! anywhere with the shipyard key; it or ESC closes. (The deck layout studio,
//! `studio`, is kept for its walk-through and dev scenarios.)

use universe_engine::glam::Vec2;
use universe_engine::{Color, Context, Frame};

use crate::keys::Act;
use crate::App;

/// The panel: the interior studio, or the deck layout studio.
pub struct Shipyard {
    page: Page,
    studio: crate::studio::Studio,
    interior: crate::interior::Interior,
}

/// Which studio is open.
#[derive(Clone, Copy, PartialEq)]
enum Page {
    Interior,
    Layout,
}

impl Shipyard {
    /// The interior studio.
    pub fn interior(_app: &App) -> Self {
        Shipyard { page: Page::Interior, studio: Default::default(), interior: crate::interior::Interior::new() }
    }

    /// The walls being walked through: the interior studio's walled tubes (none from
    /// the deck studio).
    pub fn walls(&self) -> Option<Vec<[universe_engine::glam::DVec3; 3]>> {
        (self.page == Page::Interior).then(|| self.interior.walls())
    }

    /// Those walls to draw: each triangle with its colour.
    pub fn wall_faces(&self) -> Option<Vec<crate::interior::WallFace>> {
        (self.page == Page::Interior).then(|| self.interior.wall_faces())
    }

    /// The interior studio turned to look from `yaw`, `pitch` (dev scenarios).
    pub fn interior_turned(yaw: f32, pitch: f32) -> Self {
        Shipyard { page: Page::Interior, studio: Default::default(), interior: crate::interior::Interior::turned(yaw, pitch) }
    }

    /// Its interior studio (dev scenarios).
    pub fn interior_mut(&mut self) -> &mut crate::interior::Interior {
        &mut self.interior
    }

    /// The deck layout studio.
    pub fn laying_out(_app: &App) -> Self {
        Shipyard { page: Page::Layout, studio: Default::default(), interior: crate::interior::Interior::new() }
    }

    /// Its studio (for dev scenarios: a tool picked).
    pub fn studio_mut(&mut self) -> &mut crate::studio::Studio {
        &mut self.studio
    }

    /// Back from a walk-through: the studio as it was left.
    pub fn back_to(studio: crate::studio::Studio) -> Self {
        Shipyard { page: Page::Layout, studio, interior: crate::interior::Interior::new() }
    }
}

pub fn open(app: &mut App) -> Option<Shipyard> {
    Some(Shipyard::interior(app))
}

/// This frame's input. False: close it.
pub fn input(app: &mut App, ctx: &Context) -> bool {
    if crate::keys::pressed(&ctx.input, Act::Shipyard) {
        return false;
    }
    let spec = app.ship.spec();
    let Some(y) = app.shipyard.as_mut() else { return false };
    if y.page == Page::Interior {
        let mut interior = std::mem::take(&mut y.interior);
        let stay = crate::interior::input(app, ctx, &mut interior);
        // A walk-through: its walled tubes the hull's walls, the shipyard put by, the
        // pilot on foot there, first person.
        if let Some(at) = interior.walk.take() {
            app.engine.send(universe_sim::Command::Walls { hull: spec.key.clone(), walls: interior.walls() });
            app.engine.send(universe_sim::Command::Preview(Some(at)));
            app.preview = app.shipyard.take().map(|mut y| {
                y.interior = interior;
                y
            });
            app.mode = crate::Mode::Pilot;
            app.chase_cam = false;
            return false;
        }
        if let Some(y) = app.shipyard.as_mut() {
            y.interior = interior;
        }
        return stay;
    }
    let mut studio = std::mem::take(&mut y.studio);
    let stay = crate::studio::input(app, ctx, &spec.key, spec.shape(), &mut studio);
    // A walk-through: the studio put by, the pilot on foot there, first person.
    if let Some(at) = studio.walk.take() {
        app.engine.send(universe_sim::Command::Preview(Some(at)));
        app.preview = Some(Shipyard::back_to(studio));
        app.mode = crate::Mode::Pilot;
        app.chase_cam = false;
        return false;
    }
    if let Some(y) = app.shipyard.as_mut() {
        y.studio = studio;
    }
    stay
}

pub fn draw(frame: &mut Frame, app: &App, y: &Shipyard) {
    let size = frame.size();
    frame.hud_rect(Vec2::ZERO, size, Color([0.012, 0.018, 0.026, 1.0]));
    let spec = app.ship.spec();
    let place = station(app).map_or_else(|| "SHIPYARD".to_string(), |s| format!("SHIPYARD - {s}"));
    match y.page {
        Page::Interior => crate::interior::draw(frame, app, &place, &y.interior),
        Page::Layout => crate::studio::draw(frame, app, &place, &spec.key, &spec.name, &y.studio),
    }
}

/// The station we're docked at, if any (its name).
fn station(app: &App) -> Option<String> {
    let universe_sim::ShipState::Landed { body, .. } = app.ship.state else { return None };
    let b = &app.view.system.bodies[body];
    (b.kind == universe_sim::world::system::BodyKind::Station).then(|| b.name.to_uppercase())
}
