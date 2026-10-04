//! On foot: out of the pilot's seat, walking through the ship or on the
//! ground outside. Input (WASD, mouse or arrows to look, SPACE jump, SHIFT
//! run, F to use the seat / hatch / ramp), the ramp, and the HUD while
//! walking. (The ship's spaces are its own model's: walked on as they are.)

use universe_engine::glam::{DVec3, Vec2};
use universe_engine::{text_size, Color, Context, Frame, KeyCode};
use universe_sim::world::crew::Reach;
use universe_sim::world::{Place, WalkCommands};

use crate::{fmt, App};

/// Turn rate of the arrow keys (rad/s), and of the mouse (rad per pixel).
const LOOK_KEYS: f64 = 1.6;
const LOOK_MOUSE: f64 = 0.0025;

/// The walking commands from the keyboard and mouse this frame.
pub fn commands(ctx: &Context) -> WalkCommands {
    let input = &ctx.input;
    let dt = ctx.dt as f64;
    let mut c = WalkCommands {
        forward: input.axis(KeyCode::KeyS, KeyCode::KeyW) as f64,
        right: input.axis(KeyCode::KeyA, KeyCode::KeyD) as f64,
        run: input.down(KeyCode::ShiftLeft) || input.down(KeyCode::ShiftRight),
        jump: input.pressed(KeyCode::Space),
        yaw: input.axis(KeyCode::ArrowRight, KeyCode::ArrowLeft) as f64 * LOOK_KEYS * dt,
        pitch: input.axis(KeyCode::ArrowDown, KeyCode::ArrowUp) as f64 * LOOK_KEYS * dt,
        interact: input.pressed(KeyCode::KeyF),
    };
    if ctx.cursor_grabbed() {
        c.yaw -= input.mouse_delta.x as f64 * LOOK_MOUSE;
        c.pitch -= input.mouse_delta.y as f64 * LOOK_MOUSE;
    }
    c
}

const HATCH_C: Color = Color::hex(0xffc040);

/// The ramp down from the hatch to the ground, while the pilot is outside.
pub fn ramp(frame: &mut Frame, app: &App) {
    let Place::Outside { body, .. } = app.v.crew.place else { return };
    let ship = &app.ship;
    // (A hull with its own ramp: that, swung down, is the way.)
    if universe_sim::world::crew::walks_out(ship) {
        return;
    }
    let b = &app.view.system.bodies[body];
    let center = app.view.positions[body];
    let at = |p: DVec3| app.view.ship_pos + ship.orientation * p;
    let t = app.now();
    let ground = |p: DVec3| {
        let w = at(p);
        let dir = (w - center).normalize();
        center + dir * b.surface_radius_at(center, w, t)
    };
    // (Its two rails, a metre either side; the top and the foot across.)
    let (top, foot) = universe_sim::world::crew::stair(ship);
    let run = foot - top;
    let side = DVec3::Y.cross(DVec3::new(run.x, 0.0, run.z)).normalize_or(DVec3::Z);
    for d in [-1.0, 1.0] {
        frame.line(at(top + side * d), ground(foot + side * d), HATCH_C);
    }
    frame.line(ground(foot - side), ground(foot + side), HATCH_C);
    frame.line(at(top - side), at(top + side), HATCH_C);
}

/// Status lines on foot, and the prompt for what's in reach.
pub fn hud(frame: &mut Frame, app: &App, lines: &mut Vec<(String, Color)>, reach: Option<Reach>) {
    const HUD: Color = Color::hex(0xdcebf2);
    const DIM: Color = Color::hex(0x7d93a0);
    match app.v.crew.place {
        Place::Seat => {}
        Place::Aboard { .. } => {
            lines.push(("ON FOOT - ABOARD".into(), HUD));
            if let Some((name, address)) = app.v.crew.room(&app.view.system, &app.ship, app.now(), &app.view.positions) {
                lines.push((name.to_uppercase(), HUD));
                lines.push((address, DIM));
            }
            let ship = &app.ship;
            let state = if ship.is_flying() { format!("SHIP FLYING  {}", fmt::speed(ship.velocity.length())) } else { "SHIP RESTING".into() };
            lines.push((state, DIM));
            lines.push(("WASD WALK  MOUSE/ARROWS LOOK  F USE  (LADDERS: W CLIMBS)".into(), DIM));
        }
        Place::Outside { body, position, .. } => {
            let b = &app.view.system.bodies[body];
            let r = position.length();
            let g = b.rail.mu / (r * r);
            match app.v.crew.room(&app.view.system, &app.ship, app.now(), &app.view.positions) {
                Some((name, address)) => {
                    lines.push((format!("ON FOOT - {}", name.to_uppercase()), HUD));
                    lines.push((address, DIM));
                }
                None => lines.push((format!("ON FOOT - {} SURFACE", b.name.to_uppercase()), HUD)),
            }
            lines.push((format!("GRAVITY {g:.2} M/S2 ({:.2} G)", g / 9.81), DIM));
            let ship_at = app.view.ship_pos;
            let me = app.view.positions[body] + b.rotation(app.now()) * position;
            // The ground's (or the air's) temperature here.
            let temp = universe_sim::world::climate::air_temperature(&app.view.system, body, &app.view.positions, app.now(), me);
            lines.push((format!("{} {}", if b.rail.atmosphere.is_some() { "AIR" } else { "GROUND" }, crate::fmt::temperature(temp)), DIM));
            lines.push((format!("SHIP {}", fmt::distance(me.distance(ship_at))), DIM));
            lines.push(("WASD WALK  SHIFT RUN  SPACE JUMP  F USE".into(), DIM));
        }
    }
    let prompt = match reach {
        Some(Reach::Seat) => "F  SIT DOWN",
        Some(Reach::Hatch) => "F  OPEN HATCH",
        Some(Reach::Ramp) => "F  BOARD SHIP",
        _ if app.vending.is_some() => return vending_panel(frame, app),
        Some(Reach::Vending(_)) => "F  USE VENDING MACHINE",
        None => return,
    };
    let size = frame.size();
    let p = Vec2::new(((size.x - text_size(prompt).x) / 2.0).floor(), (size.y * 0.62).floor());
    frame.text_boxed(p, prompt, Color::hex(0xffc040), Color([0.012, 0.018, 0.026, 0.85]));
}

/// What vending item `k` looks like: a few coloured boxes in a unit square
/// (x across, y up, 0..1): (x, y, w, h, colour). Drawn on the machine's
/// shelves (`scene`) and beside its name in the panel.
pub(crate) fn vending_icon(k: usize) -> Vec<([f32; 4], Color)> {
    let (red, white, blue, cap, yellow, brown, green) =
        (Color::hex(0xd82020), Color::hex(0xf4f4f4), Color::hex(0x60b0ff), Color::hex(0x2050a0), Color::hex(0xffd030), Color::hex(0x6a3a1a), Color::hex(0x40c060));
    match k {
        // A can, its white band.
        0 => vec![([0.3, 0.05, 0.4, 0.8], red), ([0.3, 0.45, 0.4, 0.12], white)],
        // A bottle and its cap.
        1 => vec![([0.36, 0.05, 0.28, 0.65], blue), ([0.42, 0.7, 0.16, 0.12], blue), ([0.42, 0.82, 0.16, 0.08], cap)],
        // A crisp bag, puffed, its label.
        2 => vec![([0.2, 0.08, 0.6, 0.78], yellow), ([0.3, 0.35, 0.4, 0.2], red)],
        // A chocolate bar, its wrapper half open.
        3 => vec![([0.12, 0.3, 0.76, 0.3], brown), ([0.12, 0.3, 0.3, 0.3], white)],
        // A protein bar.
        _ => vec![([0.12, 0.32, 0.76, 0.26], green), ([0.42, 0.32, 0.16, 0.26], white)],
    }
}

/// The vending machine's panel: what it sells, the pick, and how to buy.
fn vending_panel(frame: &mut Frame, app: &App) {
    use universe_sim::world::spaceport::VENDING;
    let pick = app.vending.unwrap_or(0);
    const ROW: f32 = 18.0;
    const ICON: f32 = 14.0;
    let size = frame.size();
    let rows: Vec<String> = VENDING.iter().enumerate().map(|(k, (what, price, _))| format!("{}{:<18} {:>3.0} CR", if k == pick { ">" } else { " " }, what, price)).collect();
    let help = "UP/DOWN PICK  ENTER BUY  F DONE";
    let w = rows.iter().map(|r| text_size(r).x + ICON + 8.0).fold(text_size(help).x, f32::max);
    let h = 14.0 + rows.len() as f32 * ROW + 20.0;
    // (To the right of the machine, so it's in sight, and clear of the messages.)
    let at = Vec2::new((size.x * 0.68).floor().min(size.x - w - 16.0), (size.y * 0.42).floor());
    frame.hud_rect(at - 8.0, Vec2::new(w, h) + 16.0, Color([0.05, 0.0, 0.0, 0.92]));
    frame.hud_box(at - 8.0, Vec2::new(w, h) + 16.0, Color::hex(0xff5050));
    frame.text(at, "VENDING MACHINE", Color::hex(0xff5050));
    for (k, row) in rows.iter().enumerate() {
        let y = at.y + 14.0 + k as f32 * ROW;
        let here = k == pick;
        // Its icon, then its name and price.
        let box_at = Vec2::new(at.x, y - 2.0);
        frame.hud_rect(box_at, Vec2::splat(ICON), Color([0.0, 0.0, 0.0, 0.6]));
        if here {
            frame.hud_box(box_at - 1.0, Vec2::splat(ICON + 2.0), Color::hex(0xffc040));
        }
        for ([x, yy, bw, bh], c) in vending_icon(k) {
            frame.hud_rect(box_at + Vec2::new(x * ICON, (1.0 - yy - bh) * ICON), Vec2::new(bw * ICON, bh * ICON), c);
        }
        frame.text(Vec2::new(at.x + ICON + 8.0, y), row, if here { Color::hex(0xffc040) } else { Color::hex(0xdcebf2) });
    }
    frame.text(Vec2::new(at.x, at.y + 14.0 + rows.len() as f32 * ROW + 6.0), help, Color::hex(0x7d93a0));
}
