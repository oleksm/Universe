//! On foot: out of the pilot's seat, walking through the ship or on the
//! ground outside. Input (WASD, mouse or arrows to look, SPACE jump, SHIFT
//! run, F to use the seat / hatch / ramp), the ship's interior, the ramp, and
//! the HUD while walking.

use universe_engine::glam::{DVec3, Vec2};
use universe_engine::{text_size, Color, Context, Frame, KeyCode};
use universe_sim::world::crew::{Reach, Room, BESIDE_SEAT, DECK, HATCH, HEADROOM, RAMP_FOOT, ROOMS, SEAT};
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

const WALL: Color = Color::hex(0x2a3038);
const FLOOR: Color = Color::hex(0x1c2026);
const CEILING: Color = Color::hex(0x23282f);
const EDGE: Color = Color::hex(0x7fa0b0);
const HATCH_C: Color = Color::hex(0xffc040);
const SEAT_C: Color = Color::hex(0x60ff90);

/// The ship's interior, seen from inside: shaded walls, deck and ceiling with
/// their edges, doorways between the rooms, the cockpit's windows (open to
/// space), the pilot's seat and console, and the hatch.
pub fn interior(frame: &mut Frame, app: &App) {
    let ship = &app.v.ship;
    let at = |p: DVec3| app.view.ship_pos + ship.orientation * p;
    let top = DECK + HEADROOM;
    let quad = |frame: &mut Frame, a: DVec3, b: DVec3, c: DVec3, d: DVec3, fill: Option<Color>| {
        let [a, b, c, d] = [a, b, c, d].map(at);
        if let Some(f) = fill {
            frame.triangle(a, b, c, f);
            frame.triangle(a, c, d, f);
        }
        for (p, q) in [(a, b), (b, c), (c, d), (d, a)] {
            frame.line(p, q, EDGE.scale(0.6));
        }
    };
    // A wall across x (at z) from x0 to x1, leaving a doorway x2..x3 open.
    let wall_x = |frame: &mut Frame, z: f64, x0: f64, x1: f64, door: Option<(f64, f64)>, fill: Option<Color>| {
        let spans = match door {
            Some((d0, d1)) => vec![(x0, d0), (d1, x1)],
            None => vec![(x0, x1)],
        };
        for (a, b) in spans {
            if b - a > 0.01 {
                quad(frame, DVec3::new(a, DECK, z), DVec3::new(b, DECK, z), DVec3::new(b, top, z), DVec3::new(a, top, z), fill);
            }
        }
    };
    let doorway = |a: &Room, b: &Room| (a.x0.max(b.x0), a.x1.min(b.x1));
    for (i, r) in ROOMS.iter().enumerate() {
        let cockpit = i == 0;
        // Deck and ceiling (the cockpit's ceiling is canopy glass).
        quad(frame, DVec3::new(r.x0, DECK, r.z0), DVec3::new(r.x1, DECK, r.z0), DVec3::new(r.x1, DECK, r.z1), DVec3::new(r.x0, DECK, r.z1), Some(FLOOR));
        let ceiling = if cockpit { None } else { Some(CEILING) };
        quad(frame, DVec3::new(r.x0, top, r.z0), DVec3::new(r.x1, top, r.z0), DVec3::new(r.x1, top, r.z1), DVec3::new(r.x0, top, r.z1), ceiling);
        // Side walls (the cockpit's are windows; the cabin's port wall has the hatch).
        let side = if cockpit { None } else { Some(WALL) };
        for x in [r.x0, r.x1] {
            quad(frame, DVec3::new(x, DECK, r.z0), DVec3::new(x, DECK, r.z1), DVec3::new(x, top, r.z1), DVec3::new(x, top, r.z0), side);
        }
        // Front and back walls, with doorways to the neighbours (the cockpit's front is the windscreen).
        let front_door = (i > 0).then(|| doorway(&ROOMS[i - 1], r));
        let back_door = ROOMS.get(i + 1).map(|n| doorway(r, n));
        let front = if cockpit { None } else { Some(WALL) };
        wall_x(frame, r.z0, r.x0, r.x1, front_door, front);
        wall_x(frame, r.z1, r.x0, r.x1, back_door, Some(WALL));
    }
    // The windscreen's frame.
    let c = &ROOMS[0];
    for x in [-1.0, 1.0] {
        frame.line(at(DVec3::new(x, DECK + 1.0, c.z0)), at(DVec3::new(x, top, c.z0)), EDGE);
    }
    frame.line(at(DVec3::new(c.x0, DECK + 1.0, c.z0)), at(DVec3::new(c.x1, DECK + 1.0, c.z0)), EDGE);
    // The seat and the console before it.
    let s = SEAT;
    for (a, b) in [
        (DVec3::new(-0.4, 0.0, -0.4), DVec3::new(0.4, 0.0, -0.4)),
        (DVec3::new(0.4, 0.0, -0.4), DVec3::new(0.4, 0.0, 0.4)),
        (DVec3::new(0.4, 0.0, 0.4), DVec3::new(-0.4, 0.0, 0.4)),
        (DVec3::new(-0.4, 0.0, 0.4), DVec3::new(-0.4, 0.0, -0.4)),
    ] {
        let lift = DVec3::Y * 0.5;
        frame.line(at(s + a + lift), at(s + b + lift), SEAT_C);
    }
    frame.line(at(s + DVec3::new(-0.4, 0.5, 0.4)), at(s + DVec3::new(-0.4, 1.4, 0.45)), SEAT_C);
    frame.line(at(s + DVec3::new(0.4, 0.5, 0.4)), at(s + DVec3::new(0.4, 1.4, 0.45)), SEAT_C);
    frame.line(at(s + DVec3::new(-0.4, 1.4, 0.45)), at(s + DVec3::new(0.4, 1.4, 0.45)), SEAT_C);
    let console = |x: f64, y: f64| at(DVec3::new(x, DECK + y, c.z0 + 0.6));
    frame.line(console(-2.0, 1.0), console(2.0, 1.0), EDGE);
    frame.line(console(-2.0, 0.8), console(2.0, 0.8), SEAT_C.scale(0.6));
    // The hatch in the cabin's port wall.
    let h = HATCH;
    let x = ROOMS[2].x0 + 0.01;
    let corners = [(h.z - 1.0, DECK), (h.z + 1.0, DECK), (h.z + 1.0, DECK + 2.2), (h.z - 1.0, DECK + 2.2)];
    for i in 0..4 {
        let (z0, y0) = corners[i];
        let (z1, y1) = corners[(i + 1) % 4];
        frame.line(at(DVec3::new(x, y0, z0)), at(DVec3::new(x, y1, z1)), HATCH_C);
    }
}

/// The ramp down from the hatch to the ground, while the pilot is outside.
pub fn ramp(frame: &mut Frame, app: &App) {
    let Place::Outside { body, .. } = app.v.crew.place else { return };
    let ship = &app.v.ship;
    let b = &app.view.system.bodies[body];
    let center = app.view.positions[body];
    let at = |p: DVec3| app.view.ship_pos + ship.orientation * p;
    let t = app.v.time;
    let ground = |p: DVec3| {
        let w = at(p);
        let dir = (w - center).normalize();
        center + dir * b.surface_radius_at(center, w, t)
    };
    let x = ROOMS[2].x0;
    for dz in [-1.0, 1.0] {
        let top = at(DVec3::new(x, DECK, HATCH.z + dz));
        let foot = ground(RAMP_FOOT + DVec3::Z * dz);
        frame.line(top, foot, HATCH_C);
    }
    frame.line(ground(RAMP_FOOT - DVec3::Z), ground(RAMP_FOOT + DVec3::Z), HATCH_C);
    frame.line(at(DVec3::new(x, DECK, HATCH.z - 1.0)), at(DVec3::new(x, DECK, HATCH.z + 1.0)), HATCH_C);
}

/// Status lines on foot, and the prompt for what's in reach.
pub fn hud(frame: &mut Frame, app: &App, lines: &mut Vec<(String, Color)>, reach: Option<Reach>) {
    const HUD: Color = Color::hex(0x30ff60);
    const DIM: Color = Color::hex(0x178a38);
    match app.v.crew.place {
        Place::Seat => {}
        Place::Aboard { position, .. } => {
            let room = ["COCKPIT", "CORRIDOR", "CABIN"]
                .iter()
                .zip(ROOMS)
                .find(|(_, r)| position.x >= r.x0 && position.x <= r.x1 && position.z >= r.z0 && position.z <= r.z1)
                .map_or("ABOARD", |(n, _)| n);
            lines.push((format!("ON FOOT - {room}"), HUD));
            let ship = &app.v.ship;
            let state = if ship.is_flying() { format!("SHIP FLYING  {}", fmt::speed(ship.velocity.length())) } else { "SHIP RESTING".into() };
            lines.push((state, DIM));
            lines.push(("WASD WALK  MOUSE/ARROWS LOOK  F USE".into(), DIM));
            let _ = BESIDE_SEAT;
        }
        Place::Outside { body, position, .. } => {
            let b = &app.view.system.bodies[body];
            let r = position.length();
            let g = b.rail.mu / (r * r);
            lines.push((format!("ON FOOT - {} SURFACE", b.name.to_uppercase()), HUD));
            lines.push((format!("GRAVITY {g:.2} M/S2 ({:.2} G)", g / 9.81), DIM));
            let ship_at = app.view.ship_pos;
            let me = app.view.positions[body] + b.rotation(app.v.time) * position;
            lines.push((format!("SHIP {}", fmt::distance(me.distance(ship_at))), DIM));
            lines.push(("WASD WALK  SHIFT RUN  SPACE JUMP  F USE".into(), DIM));
        }
    }
    let prompt = match reach {
        Some(Reach::Seat) => "F  SIT DOWN",
        Some(Reach::Hatch) => "F  OPEN HATCH",
        Some(Reach::Ramp) => "F  BOARD SHIP",
        None => return,
    };
    let size = frame.size();
    let p = Vec2::new(((size.x - text_size(prompt).x) / 2.0).floor(), (size.y * 0.62).floor());
    frame.text_boxed(p, prompt, Color::hex(0xffc040), Color([0.0, 0.03, 0.01, 0.85]));
}
