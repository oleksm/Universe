//! The shipyard: the interior studio (see `interior`), on the hull we fly, an app over the game
//! (`ShipyardApp`). Open anywhere with the shipyard key; it or ESC closes. Both editors share a
//! private deck document, copied from the game's once on opening and never handed back. Walking
//! tests that document locally, never aboard the live ship.

use universe_engine::glam::Vec2;
use universe_engine::{Color, Context, Frame};

use crate::keys::Act;
use crate::App;

/// The panel: the interior studio, or the deck layout studio.
pub struct Shipyard {
    page: Page,
    /// Initial live decks are copied once; all edits and saved-design loads stay here.
    deckplans: Vec<universe_sim::world::deckplan::DeckPlan>,
    seed: Option<std::sync::Arc<universe_sim::world::ship::ClassSpec>>,
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
    /// Does the studio want the mouse taken (a test drive steering with it)?
    pub fn wants_mouse(&self) -> bool {
        self.interior.wants_mouse()
    }

    /// The interior studio.
    pub fn interior(app: &App) -> Self {
        let mut y = Shipyard {
            page: Page::Interior,
            deckplans: Vec::new(),
            seed: None,
            studio: Default::default(),
            interior: crate::interior::Interior::new(),
        };
        y.initialize(app);
        y
    }

    fn initialize(&mut self, app: &App) {
        if self.seed.is_none() {
            self.deckplans = app.deckplans.clone();
            self.seed = Some(std::sync::Arc::new(app.ship.spec().clone()));
            self.sync(0.0);
        }
    }

    fn sync(&mut self, dt: f32) {
        if let Some(spec) = &self.seed {
            self.interior.sync(&spec.key, spec.shape(), &mut self.deckplans, dt);
        }
    }

    /// The editor's document, for dev fixtures prepared after opening the panel.
    pub fn deckplans_mut(&mut self) -> &mut Vec<universe_sim::world::deckplan::DeckPlan> {
        &mut self.deckplans
    }

    /// The interior studio turned to look from `yaw`, `pitch` (dev scenarios).
    pub fn interior_turned(yaw: f32, pitch: f32) -> Self {
        Shipyard { page: Page::Interior, deckplans: Vec::new(), seed: None, studio: Default::default(), interior: crate::interior::Interior::turned(yaw, pitch) }
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
    pub fn laying_out(app: &App) -> Self {
        let mut y = Self::interior(app);
        y.open_decks();
        y
    }

    /// Its studio (for dev scenarios: a tool picked).
    pub fn studio_mut(&mut self) -> &mut crate::studio::Studio {
        &mut self.studio
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

/// The shipyard as an app over the game: the shipyard, where it is (the station docked at) and
/// the credits as they were on opening (nothing is bought in it).
pub struct ShipyardApp {
    y: Shipyard,
    place: String,
    credits: f64,
}

impl ShipyardApp {
    /// Shipyard `y` (seeded from the game, as dev scenarios that make one before it has seen the
    /// game are on handing it over).
    pub fn new(mut y: Shipyard, app: &App) -> Self {
        y.initialize(app);
        let place = station(app).map_or_else(|| "SHIPYARD".to_string(), |s| format!("SHIPYARD - {s}"));
        ShipyardApp { y, place, credits: app.v.credits }
    }
}

impl universe_engine::apps::App for ShipyardApp {
    fn name(&self) -> &str {
        "shipyard"
    }

    fn update(&mut self, ctx: &mut Context) -> universe_engine::apps::Next {
        use universe_engine::apps::Next;
        // The shipyard key closes it (the interior studio asks first if its plan is unsaved).
        let stay = if crate::keys::pressed(&ctx.input, Act::Shipyard) {
            !self.y.may_close()
        } else {
            universe_prof::time("studio", || input_with(&mut self.y, ctx))
        };
        // (Worked with the mouse: the cursor free; but a test drive steering with it takes it,
        // as flight does.)
        let want = stay && self.y.wants_mouse();
        if ctx.cursor_grabbed() != want {
            ctx.grab_cursor(want);
        }
        if stay { Next::Stay } else { Next::Close }
    }

    fn camera(&self) -> universe_engine::Camera {
        universe_engine::Camera::default()
    }

    fn draw(&self, frame: &mut Frame, _: &Context) {
        universe_prof::time("draw/shipyard", || draw(frame, &self.place, self.credits, &self.y));
    }

    /// The game runs on under it (its sounds, its link), not drawn.
    fn cover(&self) -> universe_engine::apps::Cover {
        universe_engine::apps::Cover::Covers
    }
}

/// Both pages edit one session document; no live-world access is needed.
pub fn input_with(y: &mut Shipyard, ctx: &Context) -> bool {
    // (The interior studio kept in step either way: its plan, the decks, saving.)
    y.sync(ctx.dt);
    y.interior.refit();
    // CTRL+S in the deck studio: saved with the interior's plan.
    let ctrl = ctx.input.ctrl();
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
        let stay = crate::interior::input_with(&mut y.deckplans, ctx, &mut y.interior);
        if let Some((at, yaw)) = y.interior.walk.take() {
            // Legacy dev requests also carry their facing; keep it on the local stand.
            y.interior.stand_test(Some([at.x, at.y, at.z, yaw]));
        }
        return stay;
    }
    // The layers panel (as the 3D studio's, the same layers): a click there is its own.
    if input.button_pressed(universe_engine::MouseButton::Left)
        && crate::interior::layers_click(&mut y.interior, crate::studio::layers_corner(size, &y.studio), input.cursor)
    {
        return true;
    }
    // (A design with no hull: no decks to lay out.)
    let Some(hull) = y.interior.spec() else {
        return !input.pressed(universe_engine::KeyCode::Escape) || !y.may_close();
    };
    let id = y.interior.id().to_string();
    let stay = crate::studio::input_with(&mut y.deckplans, ctx, &id, hull, &mut y.studio);
    // Sync edits before building the stand or asking to save on close.
    y.sync(0.0);
    if let Some((at, _yaw)) = y.studio.walk.take() {
        y.interior.walk_at(at);
        y.page = Page::Interior;
        return true;
    }
    // Closed from the deck studio: the interior's plan asked about first (shown there).
    stay || !y.may_close()
}

/// The studios' tabs at the top right: the 3D interior (0), the 2D decks (1).
fn tab_rect(size: Vec2, k: usize) -> (Vec2, Vec2) {
    let w = 120.0;
    (Vec2::new(size.x - 12.0 - (2 - k) as f32 * (w + 4.0) + 4.0, 6.0), Vec2::new(w, 16.0))
}

fn inside((p, c): (Vec2, Vec2), q: Vec2) -> bool {
    q.x >= p.x && q.x <= p.x + c.x && q.y >= p.y && q.y <= p.y + c.y
}

pub fn draw(frame: &mut Frame, place: &str, credits: f64, y: &Shipyard) {
    let size = frame.size();
    frame.hud_rect(Vec2::ZERO, size, crate::palette::panel(1.0));
    match y.page {
        Page::Interior => crate::interior::draw(frame, place, &y.interior),
        Page::Layout => match y.interior.spec() {
            Some(hull) => crate::studio::draw_with(frame, &y.deckplans, &format!("{place}   {credits:.0} CR"), y.interior.id(), &hull.name, &y.studio, &y.interior.access()),
            None => {
                let text = "THE DECK STUDIO LAYS DECKS OUT IN A HULL: THIS DESIGN HAS NONE (TAB: BACK TO 3D)";
                let w = text.chars().count() as f32 * universe_engine::frame::GLYPH;
                frame.text(Vec2::new((size.x - w) * 0.5, size.y * 0.5), text, Color([1.0, 0.85, 0.35, 1.0]));
            }
        },
    }
    // In the deck studio: how it's saved (with the 3D plan), what was said; the
    // layers (the 3D studio's).
    if y.page == Page::Layout {
        if y.interior.spec().is_some() {
            crate::interior::draw_layers(frame, &y.interior, crate::studio::layers_corner(size, &y.studio), y.studio.cursor);
        }
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
