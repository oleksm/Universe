//! Guidance for the follow programs (keep at range, orbit, close on a rock),
//! like the landing and docking guides: a phase banner with what's to go,
//! the numbers, and in the view the path to where the program is taking us
//! — the hold point and the range shell, the orbit's circle, or the spot on
//! the rock where the anchor will reach.

use universe_engine::glam::{DVec3, Vec2};
use universe_engine::{text_size, Color, Frame, GLYPH};
use universe_sim::avionics::follow::{Anchor, Follow, Manoeuvre};
use universe_sim::world::mining::{ANCHOR_REACH, ANCHOR_SPEED};
use universe_sim::world::ship::SHIP_RADIUS;

use crate::{fmt, App};

const PATH: Color = Color::hex(0xff60ff);
const OK: Color = Color::hex(0x30ff60);
const AMBER: Color = Color::hex(0xffb030);
const DIM: Color = Color::hex(0x208040);
const PANEL: Color = Color([0.0, 0.05, 0.0, 0.85]);

/// Where a follow program stands: what it follows, where it's taking us,
/// and how we're doing.
struct Standing {
    follow: Follow,
    name: String,
    /// The anchor: where it is and how it moves.
    at: DVec3,
    /// Where the program is taking us now.
    goal: DVec3,
    /// To go (m), and our closing speed toward the goal (m/s).
    to_go: f64,
    closing: f64,
    /// A rock: our gap to its surface (m), our drift against it (m/s), and
    /// the spot on its surface under us.
    rock: Option<(f64, f64, DVec3, DVec3)>,
    /// Which step of the banner we're on.
    step: usize,
}

fn standing(app: &App) -> Option<Standing> {
    let follow = app.v.avionics.following?;
    if app.view.origin != app.v.ship_system {
        return None;
    }
    let sys = &app.view.system;
    let t = app.now();
    let ship = app.view.ship_pos;
    let (at, vel, name) = match follow.anchor {
        Anchor::Ship(id) => {
            let c = app.contacts.iter().find(|c| c.blip.id + 1 == id)?;
            (c.blip.position, c.blip.velocity, c.name.to_uppercase())
        }
        Anchor::Place(target) => {
            let at = target.position(sys, t, &app.view.positions)?;
            let body = match target {
                universe_sim::NavTarget::Station(b) | universe_sim::NavTarget::Gate(b) | universe_sim::NavTarget::Asteroid(b) => b,
                universe_sim::NavTarget::Spaceport(p) => sys.spaceports.get(p)?.body,
            };
            (at, sys.velocity(body, t), target.name(sys).to_uppercase())
        }
        Anchor::Rock { field, body } => {
            let (at, vel) = sys.field_body_state(field, body, t);
            (at, vel, sys.field_bodies(field).get(body)?.name.to_uppercase())
        }
    };
    let out = (ship - at).normalize_or(DVec3::Y);
    let range = follow.manoeuvre.range();
    let mut rock = None;
    let goal = match follow.manoeuvre {
        Manoeuvre::KeepAt(r) => at + out * r,
        Manoeuvre::Orbit(r) => {
            // The nearest point of the circle (in its plane, round the anchor).
            let axis = follow.axis.unwrap_or(DVec3::Y);
            let flat = (out - axis * out.dot(axis)).normalize_or(axis.any_orthonormal_vector());
            at + flat * r
        }
        Manoeuvre::Surface(gap) => {
            let Anchor::Rock { field, body } = follow.anchor else { return None };
            let b = &sys.field_bodies(field)[body];
            let ground = b.surface_radius_at(at, ship, t);
            let spot = at + out * ground;
            let w = b.angular_velocity();
            let surface_vel = vel + w.cross(ship - at);
            let gap_now = ship.distance(at) - ground - SHIP_RADIUS;
            let drift = (app.ship.velocity - surface_vel).length();
            rock = Some((gap_now, drift, spot, out));
            at + out * (ground + SHIP_RADIUS + gap)
        }
    };
    let to_go = ship.distance(goal);
    let rel = app.ship.velocity - vel;
    let closing = rel.dot((goal - ship).normalize_or(DVec3::ZERO));
    let step = match (follow.manoeuvre, rock) {
        (Manoeuvre::Surface(_), Some((gap, drift, _, _))) => {
            if gap > ANCHOR_REACH * 2.0 {
                0
            } else if drift > ANCHOR_SPEED * 0.9 || gap > ANCHOR_REACH {
                1
            } else {
                2
            }
        }
        (Manoeuvre::Orbit(_), _) => usize::from(to_go < range * 0.15),
        _ => usize::from(to_go < range * 0.1 && rel.length() < 2.0),
    };
    Some(Standing { follow, name, at, goal, to_go, closing, rock, step })
}

fn steps(m: Manoeuvre) -> &'static [&'static str] {
    match m {
        Manoeuvre::Surface(_) => &["CLOSE IN", "MATCH DRIFT", "ANCHOR (Y)"],
        Manoeuvre::Orbit(_) => &["JOIN ORBIT", "ORBIT"],
        Manoeuvre::KeepAt(_) => &["CLOSE TO RANGE", "HOLD"],
    }
}

/// The banner across the top (when no clearance has one there already).
pub fn banner(frame: &mut Frame, app: &App) {
    if app.approach.is_some() {
        return;
    }
    let Some(s) = standing(app) else { return };
    let parts: Vec<String> = steps(s.follow.manoeuvre).iter().enumerate().map(|(i, p)| format!("{} {p}", i + 1)).collect();
    let eta = if s.closing > 0.05 && s.step == 0 { format!("ETA {}", fmt::countdown(s.to_go / s.closing)) } else { String::new() };
    let full = format!("{}: {}      {} TO GO  {eta}", s.name, parts.join("  >  "), fmt::distance(s.to_go));
    let size = frame.size();
    let mut x = ((size.x - text_size(&full).x) / 2.0).floor();
    let y = 22.0;
    frame.hud_rect(Vec2::new(x - 6.0, y - 3.0), Vec2::new(text_size(&full).x + 12.0, GLYPH + 6.0), PANEL);
    x = frame.text(Vec2::new(x, y), &format!("{}: ", s.name), DIM.scale(1.6)).x;
    for (i, part) in parts.iter().enumerate() {
        let c = if i == s.step { AMBER } else { DIM };
        if i == s.step {
            frame.hud_box(Vec2::new(x - 3.0, y - 2.0), Vec2::new(text_size(part).x + 6.0, GLYPH + 4.0), AMBER);
        }
        x = frame.text(Vec2::new(x, y), part, c).x;
        if i + 1 < parts.len() {
            x = frame.text(Vec2::new(x, y), "  >  ", DIM).x;
        }
    }
    frame.text(Vec2::new(x, y), &format!("      {} TO GO  {eta}", fmt::distance(s.to_go)), OK);
}

/// The numbers, for the HUD's lines.
pub fn lines(app: &App, lines: &mut Vec<(String, Color)>) {
    let Some(s) = standing(app) else { return };
    let label = s.follow.manoeuvre.label();
    lines.push((format!("{label} {} - {} TO GO, CLOSING {}  X TO LET GO", s.name, fmt::distance(s.to_go), fmt::speed(s.closing)), AMBER));
    if let Some((gap, drift, _, _)) = s.rock {
        let ready = gap < ANCHOR_REACH && drift < ANCHOR_SPEED;
        let c = if ready { OK } else { AMBER };
        lines.push((
            format!("SURFACE {}  DRIFT {drift:.2} M/S (ANCHOR: WITHIN {:.0} M, UNDER {:.1} M/S){}", fmt::distance(gap.max(0.0)), ANCHOR_REACH, ANCHOR_SPEED, if ready { "  Y TO ANCHOR" } else { "" }),
            c,
        ));
    }
}

/// In the view: the path to the goal and the goal itself; the range shell,
/// the orbit's circle, or the anchor's reach on the rock.
pub fn draw(frame: &mut Frame, app: &App) {
    let Some(s) = standing(app) else { return };
    let ship = app.view.ship_pos;
    let cam = frame.camera.position;
    // The path: dashes from us to the goal.
    let n = 24;
    for k in (0..n).step_by(2) {
        let (a, b) = (ship.lerp(s.goal, k as f64 / n as f64), ship.lerp(s.goal, (k + 1) as f64 / n as f64));
        frame.line(a, b, PATH.scale(0.8));
    }
    // The goal: a diamond, a few pixels whatever the distance.
    let size = s.goal.distance(cam) * 0.012;
    let to = (s.goal - cam).normalize_or(DVec3::Y);
    let (u, v) = (to.any_orthonormal_vector(), to.cross(to.any_orthonormal_vector()));
    let goal_c = if s.step + 1 == steps(s.follow.manoeuvre).len() { OK } else { PATH };
    let d = [s.goal + u * size, s.goal + v * size, s.goal - u * size, s.goal - v * size];
    for i in 0..4 {
        frame.line(d[i], d[(i + 1) % 4], goal_c);
    }
    match s.follow.manoeuvre {
        Manoeuvre::Orbit(r) => {
            let axis = s.follow.axis.unwrap_or(DVec3::Y);
            frame.circle(s.at, axis, r, 128, PATH.scale(0.45));
        }
        Manoeuvre::KeepAt(r) => {
            frame.circle(s.at, (cam - s.at).normalize_or(DVec3::Y), r, 96, PATH.scale(0.3));
        }
        Manoeuvre::Surface(_) => {
            // The anchor's reach, ringed on the surface under us.
            if let Some((gap, drift, spot, up)) = s.rock {
                let ready = gap < ANCHOR_REACH && drift < ANCHOR_SPEED;
                frame.circle(spot, up, ANCHOR_REACH, 48, if ready { OK } else { AMBER.scale(0.8) });
                frame.circle(spot, up, 3.0, 12, if ready { OK } else { AMBER });
            }
        }
    }
}
