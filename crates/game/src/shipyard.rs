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

    /// The walls being walked through: the interior studio's walled tubes (from
    /// either studio).
    pub fn walls(&self) -> Option<Vec<[universe_engine::glam::DVec3; 3]>> {
        Some(self.interior.walls())
    }

    /// The interior studio's hatches' leaves (closed).
    pub fn leaves(&self) -> Vec<crate::interior::Leaf> {
        self.interior.leaves()
    }

    /// Those walls to draw: each triangle with its colour.
    pub fn wall_faces(&self) -> Option<Vec<crate::interior::WallFace>> {
        Some(self.interior.wall_faces())
    }

    /// The interior studio turned to look from `yaw`, `pitch` (dev scenarios).
    pub fn interior_turned(yaw: f32, pitch: f32) -> Self {
        Shipyard { page: Page::Interior, studio: Default::default(), interior: crate::interior::Interior::turned(yaw, pitch) }
    }

    /// Its interior studio (dev scenarios).
    /// The deck studio open (dev scenarios).
    pub fn open_decks(&mut self) {
        self.page = Page::Layout;
    }

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

impl Shipyard {
    /// Asked to close: true if it can; the interior studio's unsaved plan asked
    /// about first (that studio shown, its question up).
    fn may_close(&mut self) -> bool {
        if self.interior.close() {
            return true;
        }
        self.page = Page::Interior;
        false
    }
}

pub fn open(app: &mut App) -> Option<Shipyard> {
    Some(Shipyard::interior(app))
}

/// This frame's input. False: close it.
pub fn input(app: &mut App, ctx: &Context) -> bool {
    // The shipyard key closes it (the interior studio asks first if its plan is unsaved).
    if crate::keys::pressed(&ctx.input, Act::Shipyard) {
        return match app.shipyard.as_mut() {
            Some(y) => y.may_close(),
            None => false,
        };
    }
    let spec = app.ship.spec();
    let Some(y) = app.shipyard.as_mut() else { return false };
    // (The interior studio kept in step either way: its plan, the decks, saving.)
    y.interior.sync(&spec.key, spec.shape(), &mut app.deckplans, ctx.dt);
    y.interior.refit(spec);
    // CTRL+S in the deck studio: saved with the interior's plan.
    let ctrl = ctx.input.down(universe_engine::KeyCode::ControlLeft) || ctx.input.down(universe_engine::KeyCode::ControlRight);
    if y.page == Page::Layout && ctrl && ctx.input.pressed(universe_engine::KeyCode::KeyS) {
        y.interior.save();
        return true;
    }
    // The two studios: the 3D interior and the 2D deck layout, switched any time
    // (their tabs at the top right, or TAB), each as it was left.
    let input = &ctx.input;
    let size = ctx.hud_size.as_vec2();
    let tab = (input.button_pressed(universe_engine::MouseButton::Left)).then(|| (0..2).find(|&k| inside(tab_rect(size, k), input.cursor))).flatten();
    if input.pressed(universe_engine::KeyCode::Tab) || tab.is_some() {
        y.page = match (tab, y.page) {
            (Some(0), _) => Page::Interior,
            (Some(_), _) => Page::Layout,
            (None, Page::Interior) => Page::Layout,
            (None, Page::Layout) => Page::Interior,
        };
        return true;
    }
    if y.page == Page::Interior {
        let mut interior = std::mem::take(&mut y.interior);
        let stay = crate::interior::input(app, ctx, &mut interior);
        // A walk-through: its walled tubes the hull's walls, the shipyard put by, the
        // pilot on foot there, first person.
        if let Some(at) = interior.walk.take() {
            app.send_layout();
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
    // The layers panel (as the 3D studio's, the same layers): a click there is its own.
    if input.button_pressed(universe_engine::MouseButton::Left)
        && crate::interior::layers_click(&mut y.interior, crate::studio::layers_corner(size, &y.studio), input.cursor)
    {
        return true;
    }
    let mut studio = std::mem::take(&mut y.studio);
    let stay = crate::studio::input(app, ctx, &spec.key, spec.shape(), &mut studio);
    // A walk-through: the studio put by, the pilot on foot there, first person.
    if let Some(at) = studio.walk.take() {
        // (The interior studio's walled tubes walked in too.)
        let walls = app.shipyard.as_ref().map(|y| y.interior.walls()).unwrap_or_default();
        app.send_layout();
        app.engine.send(universe_sim::Command::Walls { hull: spec.key.clone(), walls });
        app.engine.send(universe_sim::Command::Preview(Some(at)));
        app.preview = app.shipyard.take().map(|mut y| {
            y.studio = studio;
            y
        });
        app.mode = crate::Mode::Pilot;
        app.chase_cam = false;
        return false;
    }
    let Some(y) = app.shipyard.as_mut() else { return false };
    y.studio = studio;
    // Closed from the deck studio: the interior's plan asked about first (shown there).
    stay || y.may_close()
}

/// The studios' tabs at the top right: the 3D interior (0), the 2D decks (1).
fn tab_rect(size: Vec2, k: usize) -> (Vec2, Vec2) {
    let w = 120.0;
    (Vec2::new(size.x - 12.0 - (2 - k) as f32 * (w + 4.0) + 4.0, 6.0), Vec2::new(w, 16.0))
}

fn inside((p, c): (Vec2, Vec2), q: Vec2) -> bool {
    q.x >= p.x && q.x <= p.x + c.x && q.y >= p.y && q.y <= p.y + c.y
}

pub fn draw(frame: &mut Frame, app: &App, y: &Shipyard) {
    let size = frame.size();
    frame.hud_rect(Vec2::ZERO, size, Color([0.012, 0.018, 0.026, 1.0]));
    let spec = app.ship.spec();
    let place = station(app).map_or_else(|| "SHIPYARD".to_string(), |s| format!("SHIPYARD - {s}"));
    match y.page {
        Page::Interior => crate::interior::draw(frame, app, &place, &y.interior),
        Page::Layout => crate::studio::draw(frame, app, &place, &spec.key, &spec.name, &y.studio, &y.interior.access()),
    }
    // In the deck studio: how it's saved (with the 3D plan), what was said; the
    // layers (the 3D studio's).
    if y.page == Page::Layout {
        crate::interior::draw_layers(frame, &y.interior, crate::studio::layers_corner(size, &y.studio), y.studio.cursor);
        let note = y.interior.message().map_or_else(|| format!("CTRL+S SAVES{}", if y.interior.unsaved() { " *" } else { "" }), str::to_string);
        let w = note.chars().count() as f32 * universe_engine::frame::GLYPH * 0.7;
        frame.text_scaled(Vec2::new(tab_rect(size, 0).0.x - w - 12.0, 10.0), &note, Color([1.0, 0.85, 0.35, 1.0]), 0.7);
    }
    // The studios' tabs (the one open lit).
    use crate::hud::{draw_cell, Lamp};
    let cursor = if y.page == Page::Interior { y.interior.cursor() } else { y.studio.cursor };
    for (k, (page, name)) in [(Page::Interior, "3D INTERIOR"), (Page::Layout, "2D DECKS")].into_iter().enumerate() {
        let (p, c) = tab_rect(size, k);
        let lamp = if y.page == page || inside((p, c), cursor) { Lamp::On } else { Lamp::Off };
        draw_cell(frame, p, c, "TAB", name, lamp);
    }
}

/// The station we're docked at, if any (its name).
fn station(app: &App) -> Option<String> {
    let universe_sim::ShipState::Landed { body, .. } = app.ship.state else { return None };
    let b = &app.view.system.bodies[body];
    (b.kind == universe_sim::world::system::BodyKind::Station).then(|| b.name.to_uppercase())
}
