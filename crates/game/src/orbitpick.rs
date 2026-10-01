//! Choosing the orbit's range: the orbit key. A tap orbits the locked
//! contact or nav target at the range chosen last (before any is chosen:
//! the preset nearest how far it is, and again the next one out). Held, it
//! lists the ranges: the mouse (or the wheel) moves the cursor, and letting
//! go orbits at the one under it — which is then the range a tap orbits at.

use universe_engine::glam::Vec2;
use universe_engine::{Color, Context, Frame};
use universe_sim::avionics::follow::RANGES;
use universe_sim::{Command, FollowKind};

use crate::keys::{key, Act};
use crate::App;

/// Held longer than this (s), the key lists the ranges.
const HOLD: f32 = 0.25;
/// Mouse travel per row of the list (device units).
const ROW_TRAVEL: f32 = 40.0;

/// The orbit key's state: how long it's been held, the list's cursor, and
/// the range chosen last.
#[derive(Default)]
pub struct OrbitPick {
    held: f32,
    index: usize,
    travel: f32,
    pub range: Option<f64>,
    /// (Dev scenarios: the list stays up.)
    shown_for_dev: bool,
}

impl OrbitPick {
    /// (Dev scenarios: as if the key had been held a while.)
    pub fn hold_for_show(&mut self) {
        self.held = 1.0;
        self.index = 3;
        self.shown_for_dev = true;
    }

    pub fn listing(&self) -> bool {
        self.held >= HOLD
    }
}

/// The key, while flying. True while the list is up (it has the mouse).
pub fn input(app: &mut App, ctx: &Context) -> bool {
    let input = &ctx.input;
    let pick = &mut app.orbit_pick;
    if pick.shown_for_dev {
        return true;
    }
    if crate::keys::down(input, Act::Orbit) {
        if pick.held == 0.0 {
            // The cursor starts on the range orbited now, else the one chosen last.
            let now = app.v.avionics.following.and_then(|f| match f.manoeuvre {
                universe_sim::avionics::follow::Manoeuvre::Orbit(r) => Some(r),
                _ => None,
            });
            let at = now.or(pick.range).unwrap_or(RANGES[3]);
            pick.index = RANGES.iter().position(|&r| r >= at - 1.0).unwrap_or(RANGES.len() - 1);
            pick.travel = 0.0;
        }
        pick.held += ctx.dt.max(1e-4);
        if !pick.listing() {
            return false;
        }
        pick.travel += input.mouse_delta.y - input.scroll * ROW_TRAVEL;
        let rows = (pick.travel / ROW_TRAVEL).trunc();
        pick.travel -= rows * ROW_TRAVEL;
        pick.index = (pick.index as i64 + rows as i64).clamp(0, RANGES.len() as i64 - 1) as usize;
        return true;
    }
    if pick.held == 0.0 {
        return false;
    }
    let listed = pick.listing();
    pick.held = 0.0;
    if listed {
        pick.range = Some(RANGES[pick.index]);
    }
    let range = pick.range;
    app.engine.send(Command::Follow(FollowKind::Orbit, range));
    false
}

/// A range as the list and the button show it: 500 M, 1 KM, 30 KM.
pub fn label(r: f64) -> String {
    if r < 1000.0 { format!("{r:.0} M") } else { format!("{:.0} KM", r / 1000.0) }
}

const LIST: Color = Color::hex(0x60ffa0);

/// The list, while the key is held: on the right, the cursor's line bright.
pub fn draw(frame: &mut Frame, app: &App) {
    let pick = &app.orbit_pick;
    if !pick.listing() {
        return;
    }
    let size = frame.size();
    let line = 12.0;
    let title = format!("ORBIT AT - MOUSE/WHEEL, LET GO OF {}", key(Act::Orbit));
    let width = universe_engine::text_size(&title).x.max(200.0);
    let pos = Vec2::new(size.x - width - 12.0, 230.0);
    let rows = RANGES.len() as f32;
    let back = Vec2::new(width, (rows + 2.0) * line) + 12.0;
    frame.hud_rect(pos - 6.0, back, Color([0.0, 0.03, 0.0, 0.85]));
    frame.hud_box(pos - 6.0, back, LIST.scale(0.6));
    frame.text(pos, &title, LIST);
    for (k, &r) in RANGES.iter().enumerate() {
        let here = k == pick.index;
        let mark = if here { ">" } else if pick.range == Some(r) { "*" } else { " " };
        let y = pos.y + (2 + k) as f32 * line;
        frame.text(Vec2::new(pos.x, y), &format!("{mark} {:>6}", label(r)), if here { LIST } else { LIST.scale(0.55) });
    }
}
