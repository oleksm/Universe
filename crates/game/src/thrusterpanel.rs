//! The thrusters panel (F7): the ship's plan, from above and from the side,
//! with every thruster where it sits and its plume as long as it's firing;
//! each one's level, thrust and fuel flow; the fuel burning now and how
//! long the tank lasts at that; where the centre of mass is, and what the
//! thrusters give without turning the ship (its balance).

use universe_engine::glam::{DVec3, Vec2};
use universe_engine::{Color, Frame};
use universe_sim::world::ship::{ThrusterRole, EXHAUST_VELOCITY, HYPER_FUEL_FLOW};

use crate::fmt;
use crate::App;

const TEXT: Color = Color::hex(0x40ff70);
const DIM: Color = Color::hex(0x208838);
const HULL: Color = Color::hex(0x2a6a40);
const MAIN: Color = Color::hex(0xffb050);
const LIFT: Color = Color::hex(0x60d0ff);
const RCS: Color = Color::hex(0xf0f0f0);
const COM: Color = Color::hex(0xff60ff);
const LINE: f32 = 12.0;

fn color(role: ThrusterRole) -> Color {
    match role {
        ThrusterRole::Main => MAIN,
        ThrusterRole::Lift => LIFT,
        ThrusterRole::Rcs => RCS,
    }
}

/// A nozzle's name as read: `nozzle_nose_left_up` → NOSE LEFT UP.
fn label(nozzle: &str) -> String {
    nozzle.trim_start_matches("nozzle_").replace('_', " ").to_uppercase()
}

/// The ship drawn in a box at `at`, `size` across, looking along `view`
/// (`across` to the right, `up` up the box), every thruster on it.
#[allow(clippy::too_many_arguments)]
fn view(frame: &mut Frame, app: &App, at: Vec2, size: Vec2, across: DVec3, up: DVec3, title: &str) {
    let ship = &app.ship;
    let s = ship.spec();
    let shape = s.shape();
    let (lo, hi) = shape.mesh.extent();
    let reach = (hi - lo).length() * 0.5;
    let scale = (size.x.min(size.y) as f64 * 0.42) / reach;
    let center = at + size * 0.5;
    let to = |p: DVec3| center + Vec2::new((p.dot(across) * scale) as f32, -(p.dot(up) * scale) as f32);
    frame.hud_box(at, size, DIM.scale(0.6));
    frame.text(at + Vec2::new(6.0, 4.0), title, DIM);
    for e in &shape.mesh.edges {
        frame.hud_line(to(shape.mesh.points[e[0] as usize]), to(shape.mesh.points[e[1] as usize]), HULL);
    }
    // Each thruster: a mark where it sits, its plume out along its exhaust.
    for (t, &u) in s.thrusters.iter().zip(&ship.jets) {
        let p = to(t.at);
        let c = color(t.role);
        frame.hud_rect(p - Vec2::splat(1.5), Vec2::splat(3.0), c.scale(if u > 0.02 { 1.0 } else { 0.45 }));
        if u > 0.02 {
            let out = -t.push;
            let dir = Vec2::new(out.dot(across) as f32, -out.dot(up) as f32);
            let len = (8.0 + 40.0 * u) as f32 * if t.role == ThrusterRole::Rcs { 0.5 } else { 1.0 };
            // (Along the line of sight, a plume is a dot: drawn as a ring.)
            if dir.length() < 0.2 {
                frame.hud_ellipse(p, Vec2::splat(2.0 + 5.0 * u as f32), 12, c);
            } else {
                frame.hud_line(p, p + dir.normalize() * len, c);
            }
        }
    }
    // The centre of mass, now.
    let com = to(ship.centre_of_mass());
    frame.hud_line(com - Vec2::new(5.0, 0.0), com + Vec2::new(5.0, 0.0), COM);
    frame.hud_line(com - Vec2::new(0.0, 5.0), com + Vec2::new(0.0, 5.0), COM);
}

pub fn draw(frame: &mut Frame, app: &App) {
    if !app.show_thrusters {
        return;
    }
    let ship = &app.ship;
    let s = ship.spec();
    let size = frame.size();
    frame.hud_rect(Vec2::ZERO, size, Color([0.0, 0.015, 0.01, 1.0]));
    frame.text(Vec2::new(12.0, 12.0), &format!("THRUSTERS - {}   (F7 CLOSES)", s.name), TEXT);

    // Left: every thruster, its level, thrust and fuel flow.
    let mut y = 12.0 + LINE * 2.0;
    frame.text(Vec2::new(12.0, y), &format!("{:<18} {:<12} {:>7} {:>9}", "THRUSTER", "LEVEL", "KN", "KG/H"), DIM);
    y += LINE;
    for (t, &u) in s.thrusters.iter().zip(&ship.jets) {
        let bar: String = (0..10).map(|i| if (i as f64) + 0.5 < u * 10.0 { '#' } else { '.' }).collect();
        let flow = t.thrust * u / EXHAUST_VELOCITY;
        let c = if u > 0.02 { color(t.role) } else { DIM };
        frame.text(Vec2::new(12.0, y), &format!("{:<18} [{bar}] {:>6.0} {:>9.1}", label(&t.nozzle).chars().take(18).collect::<String>(), t.thrust * u / 1000.0, flow * 3600.0), c);
        y += LINE;
    }

    // Right: the plan, from above and from the side.
    let left = 12.0 + universe_engine::text_size(&format!("{:<18} [{}] {:>6} {:>9}", "", "..........", "", "")).x + 24.0;
    let box_w = ((size.x - left - 24.0) * 0.5).floor();
    let box_h = (size.y * 0.55).floor();
    let top = 12.0 + LINE * 2.0;
    view(frame, app, Vec2::new(left, top), Vec2::new(box_w, box_h), DVec3::X, DVec3::NEG_Z, "FROM ABOVE (NOSE UP)");
    view(frame, app, Vec2::new(left + box_w + 12.0, top), Vec2::new(box_w, box_h), DVec3::Y, DVec3::NEG_Z, "FROM THE SIDE (TOP RIGHT)");

    // Under them: the fuel, the burn, the balance.
    let flow = ship.fuel_flow() + if ship.hyperdrive { HYPER_FUEL_FLOW * ship.throttle.clamp(0.0, 1.0) } else { 0.0 };
    let mut y = top + box_h + LINE;
    let mut put = |text: String, c: Color| {
        frame.text(Vec2::new(left, y), &text, c);
        y += LINE;
    };
    put(format!("FUEL {} OF {}", fmt::tonnes(ship.fuel), fmt::tonnes(s.fuel_capacity)), TEXT);
    if flow > 1e-9 {
        put(format!("BURNING {:.1} KG/H ({:.3} KG/S)", flow * 3600.0, flow), MAIN);
        let left_s = ship.fuel / flow;
        put(format!("EMPTY IN {} AT THIS BURN", fmt::duration(left_s)), if left_s < 600.0 { Color::hex(0xff4040) } else { TEXT });
    } else {
        put("NOT BURNING".into(), DIM);
    }
    let a = ship.authority();
    let com = ship.centre_of_mass();
    put(format!("CENTRE OF MASS {:+.1} M UP, {:+.1} M AFT  (+)", com.y, com.z), COM);
    put(
        format!(
            "WITHOUT TURNING IT: DRIVE {:.0}%  LIFT {:.0}%  THRUSTERS {:.0}%",
            100.0 * a.main / s.main_thrust.max(1.0),
            100.0 * a.lift / s.lift_thrust.max(1.0),
            100.0 * a.side / s.rcs_thrust.max(1.0)
        ),
        TEXT,
    );
    put(format!("DRIVE {}  LIFT {}  THRUSTERS {}  (RATED)", kn(s.main_thrust), kn(s.lift_thrust), kn(s.rcs_thrust)), DIM);
}

fn kn(n: f64) -> String {
    if n >= 1.0e6 { format!("{:.2} MN", n / 1.0e6) } else { format!("{:.0} KN", n / 1000.0) }
}
