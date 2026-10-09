//! The planet studio (WORLDS): every world the planet simulation has released (the worlds store's
//! index), the game's or not, and one looked at in the observer's orbit with its ground, air and
//! clouds as the game draws them (`docs/planet-studio-plan.md`). A world that isn't one of the
//! game's is shown alone: a system of its own for the view (the sim never sees it), the system's
//! star and the world, its radius, day and tilt as the store gives them, its ground from its
//! release, the rest (its air) as the world it is shaped from. Its timeline: the store's history
//! of it, globes in time order, PAGE UP back and PAGE DOWN on, its colour then on today's relief (its
//! sea and clouds today's, so not drawn). Up and down pick a world, ENTER goes to it, L changes
//! the look (its true colour, its rock, its oil and gas), ESC closes. The wheel and a drag turn
//! and zoom as in the observer.

use universe_engine::glam::Vec2;
use universe_engine::{Color, Context, Frame, KeyCode};
use universe_sim::world::worlds::{History, Release};

use crate::App;
use crate::palette::{DIM, PANEL, TEXT};


/// How a baked world's ground is coloured close up: which of its bake's globe maps.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Look {
    #[default]
    Colour,
    Geology,
    Energy,
}

impl Look {
    /// The bake's map for it.
    pub fn file(self) -> &'static str {
        match self {
            Look::Colour => "globe_color.jpg",
            Look::Geology => "globe_geology.jpg",
            Look::Energy => "globe_energy.jpg",
        }
    }

    fn name(self) -> &'static str {
        match self {
            Look::Colour => "TRUE COLOUR",
            Look::Geology => "ROCK",
            Look::Energy => "OIL AND GAS",
        }
    }

    fn next(self) -> Look {
        match self {
            Look::Colour => Look::Geology,
            Look::Geology => Look::Energy,
            Look::Energy => Look::Colour,
        }
    }
}

/// The studio's list and cursor.
pub struct PlanetStudio {
    pub list: Vec<Release>,
    pub selected: usize,
    /// What the last ENTER said, if it couldn't go there.
    note: String,
    /// The world gone to: its body's key (the game's, or the studio's own) and its history, if
    /// it has one (read on going).
    pub shown: Option<(String, Option<std::sync::Arc<History>>)>,
}

/// A frame of a world's history drawn in place of today's colour: the world's body key, its
/// history, the frame.
#[derive(Clone)]
pub struct WorldFrame {
    pub key: String,
    pub history: std::sync::Arc<History>,
    pub frame: usize,
}

impl PlanetStudio {
    pub fn open() -> Self {
        PlanetStudio { list: universe_sim::world::worlds::releases(), selected: 0, note: String::new(), shown: None }
    }
}

/// Go to world `r`: the observer round its body (the game's, or one of its own: see `alone`),
/// at a few of its radii.
pub fn go_to(app: &mut App, r: &Release) -> Result<(), String> {
    app.studio_world = None;
    if app.world_frame.take().is_some() {
        app.caches.world_maps.clear();
    }
    let history = match History::release(r) {
        Some(Ok(h)) => Some(std::sync::Arc::new(h)),
        Some(Err(e)) => {
            eprintln!("{}: its history can't be read ({e})", r.world_id);
            None
        }
        None => None,
    };
    let key = r.body.clone().unwrap_or_else(|| format!("studio.{}", r.world_id));
    if let Some(s) = app.panels.planet_studio.as_mut() {
        s.shown = Some((key, history));
    }
    let Some(key) = r.body.as_deref() else {
        let sys = alone(app, r)?;
        let radius = sys.bodies[1].rail.radius;
        app.studio_world = Some(std::sync::Arc::new(sys));
        app.mode = crate::Mode::Observer;
        app.observer.focus = crate::observer::Focus::Body { system: app.view.origin, body: 1 };
        app.observer.distance = radius * 3.0;
        return Ok(());
    };
    let body = app.view.system.bodies.iter().position(|b| b.key == key).ok_or("NOT IN THE SYSTEM IN VIEW")?;
    app.mode = crate::Mode::Observer;
    app.observer.focus = crate::observer::Focus::Body { system: app.view.origin, body };
    app.observer.distance = app.view.system.bodies[body].rail.radius * 3.0;
    Ok(())
}

/// A system for the view alone: the star of the system in view and world `r` round it, shaped
/// from the system's first baked world with air (its orbit, its air), its ground `r`'s release.
fn alone(app: &App, r: &Release) -> Result<universe_sim::world::system::StarSystem, String> {
    use universe_sim::world::system::StarSystem;
    use universe_sim::world::terrain::{Terrain, TerrainKind};
    let home = app.charts.system(app.view.origin);
    let like = home.bodies.iter().find(|b| b.terrain.as_ref().is_some_and(|t| t.baked()) && b.rail.atmosphere.is_some()).ok_or("NO BAKED WORLD HERE TO SHAPE IT FROM")?;
    let heights = universe_sim::world::worlds::Heights::of_release(r).ok_or("ITS SURFACE CAN'T BE READ")?;
    let mut world = like.clone();
    world.key = format!("studio.{}", r.world_id);
    world.name = r.name.clone();
    if r.radius > 0.0 {
        // (Its mass as the shaping world's density gives it: for the view, nothing pulls.)
        let k = (r.radius / like.rail.radius).powi(3);
        world.rail.radius = r.radius;
        world.rail.mu *= k;
        world.mass *= k;
    }
    world.rail.parent = Some(0);
    world.rail.pulled_by.clear();
    if let Some(day) = r.day {
        world.rail.day = day;
    }
    if let Some(tilt) = r.tilt {
        world.rail.tilt = universe_engine::glam::DQuat::from_rotation_z(tilt.to_radians());
    }
    let mut ground = Terrain::new(TerrainKind::Terran, world.rail.radius, 0);
    ground.bake(heights);
    world.terrain = Some(ground);
    // (The star without its own planets' pull on it.)
    let mut star = home.bodies[0].clone();
    star.rail.pulled_by.clear();
    Ok(StarSystem {
        index: home.index,
        name: home.name.clone(),
        class: home.class,
        luminosity: home.luminosity,
        bodies: vec![star, world],
        spaceports: Vec::new(),
        fields: Vec::new(),
        small: 0..0,
        belts: Vec::new(),
        belt_seed: 0,
        patches: Default::default(),
    })
}

/// Keys while open. False when it should close.
pub fn input(app: &mut App, ctx: &Context) -> bool {
    let input = &ctx.input;
    if input.pressed(KeyCode::Escape) || crate::keys::pressed(input, crate::keys::Act::Worlds) {
        return false;
    }
    let Some(studio) = app.panels.planet_studio.as_mut() else { return false };
    let n = studio.list.len().max(1);
    if input.pressed(KeyCode::ArrowDown) {
        studio.selected = (studio.selected + 1) % n;
    }
    if input.pressed(KeyCode::ArrowUp) {
        studio.selected = (studio.selected + n - 1) % n;
    }
    // (The timeline: PAGE UP back in time from today, PAGE DOWN on to today. The arrows turn the
    // observer.)
    let step = match (input.pressed(KeyCode::PageUp), input.pressed(KeyCode::PageDown)) {
        (true, false) => -1,
        (false, true) => 1,
        _ => 0,
    };
    if step != 0
        && let Some((key, Some(history))) = app.panels.planet_studio.as_ref().and_then(|s| s.shown.clone())
    {
        let last = history.frames.len().saturating_sub(1);
        let now = app.world_frame.as_ref().map(|f| f.frame);
        let next = match (now, step) {
            (None, -1) => Some(last),
            (None, _) => None,
            (Some(i), -1) => Some(i.saturating_sub(1)),
            (Some(i), _) => (i < last).then_some(i + 1),
        };
        if next != now {
            app.world_frame = next.map(|frame| WorldFrame { key, history, frame });
            app.caches.world_maps.clear();
        }
    }
    if input.pressed(KeyCode::KeyL) {
        app.world_look = app.world_look.next();
        // (Its maps made again in the new look.)
        app.caches.world_maps.clear();
    }
    if input.pressed(KeyCode::Enter) {
        let r = app.panels.planet_studio.as_ref().and_then(|s| s.list.get(s.selected).cloned());
        let note = match r {
            Some(r) => go_to(app, &r).err().unwrap_or_default(),
            None => "NO WORLDS: IS THE WORLDS STORE HERE? (UNIVERSE_WORLDS)".into(),
        };
        if let Some(s) = app.panels.planet_studio.as_mut() {
            s.note = note;
        }
    }
    true
}

pub fn draw(frame: &mut Frame, app: &App, studio: &PlanetStudio) {
    let line = universe_engine::GLYPH + 2.0;
    let (x, mut y) = (12.0, 90.0);
    let w = 470.0;
    let rows = studio.list.len() as f32;
    frame.hud_rect(Vec2::new(x - 6.0, y - 6.0), Vec2::new(w, line * (rows + 16.0)), PANEL);
    frame.text(Vec2::new(x, y), &format!("WORLDS   LOOK: {}  (L)", app.world_look.name()), TEXT);
    y += line * 1.5;
    frame.text(Vec2::new(x, y), &format!(" {:<6} {:<14} {:>7} {:>6} {:>8} {:>7}", "WORLD", "NAME", "RADIUS", "LAND", "DEPOSITS", "SURFACE"), DIM);
    y += line;
    for (k, r) in studio.list.iter().enumerate() {
        let mark = if k == studio.selected { ">" } else { " " };
        let surface = r.surface.map_or("-".to_string(), |v| format!("V{v}"));
        let text = format!("{mark}{:<6} {:<14} {:>5.0}KM {:>5.1}% {:>8} {:>7}", r.world_id, r.name.to_uppercase().chars().take(14).collect::<String>(), r.radius / 1000.0, r.land_pct, r.deposits, surface);
        let c = if k == studio.selected { TEXT } else if r.status != "current" { DIM.scale(0.7) } else { DIM };
        frame.text(Vec2::new(x, y), &text, c);
        y += line;
    }
    y += line * 0.5;
    // The timeline of the world gone to: today, or one of its history's globes.
    if let Some((_, history)) = &studio.shown {
        match (history, &app.world_frame) {
            (Some(h), Some(f)) => {
                let fr = &h.frames[f.frame];
                frame.text(Vec2::new(x, y), &format!("TIMELINE {}/{}  {:.2} GYR ({:.2} GYR AGO)", f.frame + 1, h.frames.len(), fr.time_gyr, fr.ago_gyr), TEXT);
                y += line;
                frame.text(Vec2::new(x, y), &format!("LAND {:.1}%  PLATES {}  HIGHEST {:.0} M  DEEPEST {:.0} M", fr.land_pct, fr.plates, fr.highest_m, fr.deepest_m), DIM);
                y += line;
                frame.text(Vec2::new(x, y), "ITS COLOUR THEN ON TODAY'S RELIEF", DIM);
            }
            (Some(h), None) => {
                frame.text(Vec2::new(x, y), &format!("TIMELINE: TODAY  ({} GLOBES BEFORE: PAGE UP)", h.frames.len()), TEXT);
            }
            (None, _) => {
                frame.text(Vec2::new(x, y), "TIMELINE: NONE IN THE STORE", DIM);
            }
        }
        y += line * 1.5;
    }
    if let Some(r) = studio.list.get(studio.selected) {
        frame.text(Vec2::new(x, y), &format!("{}  ({})", r.name.to_uppercase(), r.status.to_uppercase()), TEXT);
        y += line;
        let body = r.body.as_deref().unwrap_or("NOT ONE OF THE GAME'S WORLDS");
        frame.text(Vec2::new(x, y), &format!("BODY {}", body.to_uppercase()), DIM);
        y += line;
        frame.text(Vec2::new(x, y), &format!("DISTRICTS {}", r.districts), DIM);
        y += line;
        // (Its line about itself, wrapped at the panel's width.)
        let mut row = String::new();
        for word in r.subtitle.split_whitespace() {
            if row.len() + word.len() + 1 > 56 {
                frame.text(Vec2::new(x, y), &row.to_uppercase(), DIM);
                y += line;
                row.clear();
            }
            if !row.is_empty() {
                row.push(' ');
            }
            row.push_str(word);
        }
        if !row.is_empty() {
            frame.text(Vec2::new(x, y), &row.to_uppercase(), DIM);
            y += line;
        }
    }
    y += line * 0.5;
    if !studio.note.is_empty() {
        frame.text(Vec2::new(x, y), &studio.note, Color::hex(0xffb030));
        y += line;
    }
    frame.text(Vec2::new(x, y), "UP/DOWN PICK  ENTER GO  PGUP/PGDN TIME  L LOOK  ESC CLOSE", DIM);
}
