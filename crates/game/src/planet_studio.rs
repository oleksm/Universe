//! The planet studio (WORLDS): every world the planet simulation has released (the worlds store's
//! index), the game's or not, and one looked at in the observer's orbit with its ground, air and
//! clouds as the game draws them (`docs/planet-studio-plan.md`). Up and down pick a world, ENTER
//! goes to it, L changes the look (its true colour, its rock, its oil and gas), ESC closes. The
//! wheel and a drag turn and zoom as in the observer.

use universe_engine::glam::Vec2;
use universe_engine::{Color, Context, Frame, KeyCode};
use universe_sim::world::worlds::Release;

use crate::App;

const TEXT: Color = Color::hex(0xdcebf2);
const DIM: Color = Color::hex(0x7d93a0);
const PANEL: Color = Color([0.012, 0.018, 0.026, 0.85]);

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
}

impl PlanetStudio {
    pub fn open() -> Self {
        PlanetStudio { list: universe_sim::world::worlds::releases(), selected: 0, note: String::new() }
    }
}

/// Go to world `r`: the observer round its body (the game's), at a few of its radii.
pub fn go_to(app: &mut App, r: &Release) -> Result<(), String> {
    let key = r.body.as_deref().ok_or("NOT ONE OF THE GAME'S WORLDS (YET)")?;
    let body = app.view.system.bodies.iter().position(|b| b.key == key).ok_or("NOT IN THE SYSTEM IN VIEW")?;
    app.mode = crate::Mode::Observer;
    app.observer.focus = crate::observer::Focus::Body { system: app.view.origin, body };
    app.observer.distance = app.view.system.bodies[body].rail.radius * 3.0;
    Ok(())
}

/// Keys while open. False when it should close.
pub fn input(app: &mut App, ctx: &Context) -> bool {
    let input = &ctx.input;
    if input.pressed(KeyCode::Escape) || crate::keys::pressed(input, crate::keys::Act::Worlds) {
        return false;
    }
    let Some(studio) = app.planet_studio.as_mut() else { return false };
    let n = studio.list.len().max(1);
    if input.pressed(KeyCode::ArrowDown) {
        studio.selected = (studio.selected + 1) % n;
    }
    if input.pressed(KeyCode::ArrowUp) {
        studio.selected = (studio.selected + n - 1) % n;
    }
    if input.pressed(KeyCode::KeyL) {
        app.world_look = app.world_look.next();
        // (Its maps made again in the new look.)
        app.world_maps.clear();
    }
    if input.pressed(KeyCode::Enter) {
        let r = app.planet_studio.as_ref().and_then(|s| s.list.get(s.selected).cloned());
        let note = match r {
            Some(r) => go_to(app, &r).err().unwrap_or_default(),
            None => "NO WORLDS: IS THE WORLDS STORE HERE? (UNIVERSE_WORLDS)".into(),
        };
        if let Some(s) = app.planet_studio.as_mut() {
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
    frame.hud_rect(Vec2::new(x - 6.0, y - 6.0), Vec2::new(w, line * (rows + 12.0)), PANEL);
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
    frame.text(Vec2::new(x, y), "UP/DOWN PICK  ENTER GO  L LOOK  ESC CLOSE", DIM);
}
