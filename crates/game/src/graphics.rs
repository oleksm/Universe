//! Graphics settings: each thing the renderer draws, on or off, to see what
//! it brings and trace a difference in looks to it. A panel at the side (`),
//! the scene live behind it; number keys switch. Kept between sessions.
//! `UNIVERSE_GRAPHICS=-shadows,-occlusion`: off from the start (dev, captures).

use universe_engine::glam::Vec2;
use universe_engine::{Color, Context, Frame, Graphics, KeyCode, GLYPH};

use crate::App;

const TEXT: Color = Color::hex(0xdcebf2);
const DIM: Color = Color::hex(0x7d93a0);
const ON: Color = Color::hex(0x60ffb0);
const OFF: Color = Color::hex(0xff8040);

/// The settings, by name (as `UNIVERSE_GRAPHICS` has them) and as shown: what each does.
const ROWS: &[(&str, &str)] = &[
    ("shadows", "SHADOWS"),
    ("textures", "MODEL TEXTURES"),
    ("normal_maps", "NORMAL MAPS (FINE RELIEF)"),
    ("occlusion", "AMBIENT OCCLUSION"),
    ("emission", "LAMPS AND GLOWS"),
    ("specular", "GLOSSY HIGHLIGHTS"),
    ("planet_light", "PLANET LIGHT"),
    ("tone_map", "FILM CURVE (AGX)"),
    ("clouds", "CLOUDS"),
];

fn flag<'a>(g: &'a mut Graphics, name: &str) -> Option<&'a mut bool> {
    Some(match name {
        "shadows" => &mut g.shadows,
        "textures" => &mut g.textures,
        "normal_maps" => &mut g.normal_maps,
        "occlusion" => &mut g.occlusion,
        "emission" => &mut g.emission,
        "specular" => &mut g.specular,
        "planet_light" => &mut g.planet_light,
        "tone_map" => &mut g.tone_map,
        "clouds" => &mut g.clouds,
        _ => return None,
    })
}

fn path() -> std::path::PathBuf {
    crate::save::data_dir().join("freefall").join("graphics.json")
}

/// As last set (all on, the first time), then `UNIVERSE_GRAPHICS`'s changes.
pub fn load() -> Graphics {
    let mut g: Graphics = std::fs::read_to_string(path()).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default();
    if let Ok(list) = std::env::var("UNIVERSE_GRAPHICS") {
        for item in list.split(',').map(str::trim).filter(|s| !s.is_empty()) {
            let (on, name) = match item.strip_prefix('-') {
                Some(n) => (false, n),
                None => (true, item.trim_start_matches('+')),
            };
            match flag(&mut g, name) {
                Some(f) => *f = on,
                None => log::warn!("UNIVERSE_GRAPHICS: no setting '{name}'"),
            }
        }
    }
    g
}

fn store(g: &Graphics) {
    let p = path();
    let _ = std::fs::create_dir_all(p.parent().expect("a folder"));
    if let Ok(s) = serde_json::to_string_pretty(g) {
        let _ = std::fs::write(p, s);
    }
}

/// One setting by name (as `UNIVERSE_GRAPHICS` has them) set; false if there's none of that name.
pub fn set(g: &mut Graphics, name: &str, on: bool) -> bool {
    match flag(g, name) {
        Some(f) => {
            *f = on;
            true
        }
        None => false,
    }
}

/// Opens it (`): true if the key was it.
pub fn opens(ctx: &Context) -> bool {
    ctx.input.pressed(KeyCode::Backquote)
}

/// Keys while open: 1-8 switch. False when it should close.
pub fn input(app: &mut App, ctx: &Context) -> bool {
    let digits = [KeyCode::Digit1, KeyCode::Digit2, KeyCode::Digit3, KeyCode::Digit4, KeyCode::Digit5, KeyCode::Digit6, KeyCode::Digit7, KeyCode::Digit8];
    for (k, (name, _)) in digits.iter().zip(ROWS) {
        if ctx.input.pressed(*k)
            && let Some(f) = flag(&mut app.graphics, name)
        {
            *f = !*f;
            store(&app.graphics);
        }
    }
    !(ctx.input.pressed(KeyCode::Backquote) || ctx.input.pressed(KeyCode::Escape))
}

pub fn draw(frame: &mut Frame, app: &App) {
    let line = GLYPH + 4.0;
    let size = frame.size();
    let w = 34.0 * (GLYPH * 0.75) + 24.0;
    let h = line * (ROWS.len() as f32 + 4.0) + 8.0;
    let (x, mut y) = (size.x - w - 16.0, 60.0);
    frame.hud_rect(Vec2::new(x - 8.0, y - 8.0), Vec2::new(w + 16.0, h), Color([0.012, 0.018, 0.026, 0.92]));
    frame.text(Vec2::new(x, y), "GRAPHICS   (` OR ESC CLOSES)", TEXT);
    y += line * 1.5;
    let (kx, sx) = (x + GLYPH * 3.0, x + w - GLYPH * 3.5);
    frame.text(Vec2::new(x, y), "KEY", DIM);
    frame.text(Vec2::new(kx, y), "WHAT", DIM);
    frame.text(Vec2::new(sx, y), "STATE", DIM);
    y += line;
    let mut g = app.graphics;
    for (k, (name, what)) in ROWS.iter().enumerate() {
        let on = flag(&mut g, name).is_some_and(|f| *f);
        frame.text(Vec2::new(x, y), &format!("{}", k + 1), TEXT);
        frame.text(Vec2::new(kx, y), what, TEXT);
        frame.text(Vec2::new(sx, y), if on { "ON" } else { "OFF" }, if on { ON } else { OFF });
        y += line;
    }
}
