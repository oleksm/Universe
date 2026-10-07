//! The news panel: the digests that have reached us over the hypernet,
//! newest first, each with its outlet, where it's from, and how long it was
//! on its way.

use universe_engine::glam::Vec2;
use universe_engine::{Color, Context, Frame, KeyCode, GLYPH};

use crate::{fmt, App};
use crate::palette::{AMBER, DIM, RED, TEXT};

const HEAD: Color = Color::hex(0x60ffb0);

/// Keys while open. False when it should close.
pub fn input(_app: &mut App, ctx: &Context) -> bool {
    !(ctx.input.pressed(KeyCode::F11) || ctx.input.pressed(KeyCode::Escape))
}

pub fn draw(frame: &mut Frame, app: &App) {
    use crate::hud::{news_items, News};
    let size = frame.size();
    frame.hud_rect(Vec2::ZERO, size, crate::palette::panel(1.0));
    let line = GLYPH + 4.0;
    let (x, mut y) = (16.0, 16.0);
    let now = app.v.time;
    let items = news_items(app);
    let status = match &app.net {
        Some((lag, _)) => format!("ON THE HYPERNET ({} FROM THE BACKBONE)", fmt::lag(*lag)),
        None => "OFF THE HYPERNET: NOTHING NEW REACHES US".to_string(),
    };
    frame.text(Vec2::new(x, y), &format!("NEWS - {} ITEMS HEARD   {status}   (F11 CLOSES)", items.len()), TEXT);
    y += line * 2.0;
    if items.is_empty() {
        frame.text(Vec2::new(x, y), "NO NEWS HAS REACHED US YET (OUTLETS PUT OUT A DIGEST EVERY 10 MIN)", DIM);
    }
    for item in &items {
        if y > size.y - line * 3.0 {
            frame.text(Vec2::new(x, y), "...", DIM);
            return;
        }
        let sys = universe_sim::names::star_name(app.charts.galaxy.stars[item.system].seed).to_uppercase();
        let late = item.heard - item.time;
        let way = if late > 2.0 { format!(", {} ON ITS WAY", fmt::lag(late)) } else { String::new() };
        let when = format!("{} AGO{way}", fmt::lag(now - item.time));
        match &item.what {
            News::Kill(text, ours) => {
                frame.text(Vec2::new(x, y), &format!("REPORT - {sys}   {when}"), DIM);
                y += line;
                frame.text(Vec2::new(x + 24.0, y), text, if *ours { RED } else { AMBER });
                y += line * 1.5;
            }
            News::Digest(d) => {
                frame.text(Vec2::new(x, y), &format!("{} - {sys}   {when}", d.outlet), HEAD);
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
    }
}
