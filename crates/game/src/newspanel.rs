//! The news panel: the digests that have reached us over the hypernet,
//! newest first, each with its outlet, where it's from, and how long it was
//! on its way.

use universe_engine::glam::Vec2;
use universe_engine::{Color, Context, Frame, KeyCode, GLYPH};

use crate::{fmt, App};

const TEXT: Color = Color::hex(0xdcebf2);
const DIM: Color = Color::hex(0x7d93a0);
const HEAD: Color = Color::hex(0x60ffb0);

/// Keys while open. False when it should close.
pub fn input(_app: &mut App, ctx: &Context) -> bool {
    !(ctx.input.pressed(KeyCode::F11) || ctx.input.pressed(KeyCode::Escape))
}

pub fn draw(frame: &mut Frame, app: &App) {
    let size = frame.size();
    frame.hud_rect(Vec2::ZERO, size, Color([0.012, 0.018, 0.026, 1.0]));
    let line = GLYPH + 4.0;
    let (x, mut y) = (16.0, 16.0);
    let now = app.v.time;
    let mut heard: Vec<_> = app.newsroom.iter().flat_map(|r| r.digests.iter()).filter_map(|d| Some((d, app.news.heard(&d.key())?))).collect();
    heard.sort_by(|a, b| b.1.total_cmp(&a.1));
    let status = match &app.net {
        Some((lag, _)) => format!("ON THE HYPERNET ({} FROM THE BACKBONE)", fmt::lag(*lag)),
        None => "OFF THE HYPERNET: NOTHING NEW REACHES US".to_string(),
    };
    frame.text(Vec2::new(x, y), &format!("NEWS - {} DIGESTS HEARD   {status}   ({} CLOSES)", heard.len(), "F11"), TEXT);
    y += line * 2.0;
    if heard.is_empty() {
        frame.text(Vec2::new(x, y), "NO NEWS HAS REACHED US YET (OUTLETS PUT OUT A DIGEST EVERY 10 MIN)", DIM);
    }
    for (d, at) in heard {
        if y > size.y - line * 4.0 {
            break;
        }
        let sys = universe_sim::names::star_name(app.charts.galaxy.stars[d.system].seed).to_uppercase();
        let late = at - d.time;
        let way = if late > 2.0 { format!(", {} ON ITS WAY", fmt::lag(late)) } else { String::new() };
        frame.text(Vec2::new(x, y), &format!("{} - {sys}   PUT OUT {} AGO{way}", d.outlet, fmt::lag(now - d.time)), HEAD);
        y += line;
        for h in &d.headlines {
            if y > size.y - line * 2.0 {
                frame.text(Vec2::new(x + 24.0, y), "...", DIM);
                return;
            }
            frame.text(Vec2::new(x + 24.0, y), h, TEXT);
            y += line;
        }
        y += line * 0.5;
    }
}
