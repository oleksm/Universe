//! Guidance for the follow programs (keep at range, orbit, close on a rock),
//! drawn the way the landing and docking guides are: the path the program
//! takes as a plan, with the same guide frames along it (see
//! `scene::Guide`, `scene::guided_path`); a phase banner with what's to go;
//! the numbers; and what's particular to each — the range shell, the
//! orbit's circle, the anchor's reach on the rock.

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

/// An orbit's frames: one every this much of the way round.
const ORBIT_STEP: f64 = std::f64::consts::TAU / 36.0;
/// How far round the orbit the way ahead is shown.
const ORBIT_AHEAD: f64 = std::f64::consts::TAU * 5.0 / 12.0;

/// The way the program takes us, as a plan (from now, relative to the
/// anchor as it is now), and how far apart its frames go (None: closer
/// together near the goal, as a landing's). Straight to the goal while
/// there's a way to go; an orbit's way round is on a fixed grid of angles
/// round the anchor, so its frames hold still and we fly through them. Over
/// a rock it turns with the rock, so its frames stay over the same ground.
pub fn plan(app: &App) -> Option<(universe_sim::Plan, Option<f64>)> {
    use universe_engine::glam::DQuat;
    use universe_sim::avionics::plan::{Action, PlanPoint};
    let s = standing(app)?;
    let ship = app.view.ship_pos;
    let range = s.follow.manoeuvre.range();
    // The way in, while there's a way to go: on station, the program holds
    // within a metre or so either side of its goal, and a join that short
    // would turn the frames about every frame.
    let joining = s.to_go > (0.02 * range).max(50.0);
    let mut path = Vec::new();
    let mut even = None;
    match s.follow.manoeuvre {
        Manoeuvre::Orbit(r) => {
            let axis = s.follow.axis.unwrap_or(DVec3::Y);
            // Angles round the axis, from a fixed reference across it.
            let e1 = axis.any_orthonormal_vector();
            let e2 = axis.cross(e1);
            let angle = |p: DVec3| {
                let o = p - s.at;
                o.dot(e2).atan2(o.dot(e1))
            };
            let point = |a: f64| s.at + (e1 * a.cos() + e2 * a.sin()) * r;
            // From the mark on the grid just behind us (the path's start has
            // no frame of its own): the next one ahead is ours to fly
            // through, and it goes once we're through it.
            let first = (angle(ship) / ORBIT_STEP).floor() * ORBIT_STEP;
            if joining {
                path.push(ship);
            }
            let marks = (ORBIT_AHEAD / ORBIT_STEP).round() as usize + 1;
            for k in 0..=marks {
                // (Between the marks, enough points for the curve.)
                for j in 0..4 {
                    if k == marks && j > 0 {
                        break;
                    }
                    path.push(point(first + (k as f64 + j as f64 / 4.0) * ORBIT_STEP));
                }
            }
            even = Some(r * ORBIT_STEP);
        }
        _ => {
            if joining {
                let n = 24;
                for k in 0..=n {
                    path.push(ship.lerp(s.goal, k as f64 / n as f64));
                }
            }
            if let Manoeuvre::KeepAt(r) = s.follow.manoeuvre {
                even = Some((r / 6.0).max(100.0));
            }
        }
    }
    // Only a rock's surface turns under us; an orbit round it doesn't.
    let spin = match (s.follow.anchor, s.follow.manoeuvre) {
        (Anchor::Rock { field, body }, Manoeuvre::Surface(_)) => app.view.system.field_bodies(field).get(body).map_or(DVec3::ZERO, |b| b.angular_velocity()),
        _ => DVec3::ZERO,
    };
    let speed = s.closing.abs().max(5.0);
    let mut along = 0.0;
    let mut points = Vec::with_capacity(path.len());
    for (i, &p) in path.iter().enumerate() {
        if i > 0 {
            along += path[i - 1].distance(p);
        }
        let next = path.get(i + 1).copied().unwrap_or(p + (p - path[i.saturating_sub(1)]));
        let dir = (next - p).normalize_or(DVec3::NEG_Z);
        let facing = DQuat::from_rotation_arc(DVec3::NEG_Z, dir);
        points.push(PlanPoint { time: along / speed, position: p, orientation: facing, aim: facing, action: Action::Thrusters, phase: universe_sim::Phase::Approach });
    }
    (points.len() >= 2).then(|| (universe_sim::Plan { start: app.now(), center: s.at, spin, points, arrives: true, holds: false }, even))
}

fn steps(m: Manoeuvre) -> &'static [&'static str] {
    match m {
        Manoeuvre::Surface(_) => &["CLOSE IN", "MATCH DRIFT", "ANCHOR"],
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
    let y = crate::hud::banner_y(app);
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
    lines.push((format!("{label} {} - {} TO GO, CLOSING {}  {} TO CANCEL", s.name, fmt::distance(s.to_go), fmt::speed(s.closing), crate::keys::key(crate::keys::Act::Cancel)), AMBER));
    if let Some((gap, drift, _, _)) = s.rock {
        let ready = gap < ANCHOR_REACH && drift < ANCHOR_SPEED;
        let c = if ready { OK } else { AMBER };
        lines.push((
            format!("SURFACE {}  DRIFT {drift:.2} M/S (ANCHOR: WITHIN {:.0} M, UNDER {:.1} M/S){}", fmt::distance(gap.max(0.0)), ANCHOR_REACH, ANCHOR_SPEED, if ready { format!("  {} TO ANCHOR", crate::keys::key(crate::keys::Act::Anchor)) } else { String::new() }),
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
    // The path and its frames, as every guide draws them.
    if let Some((plan, _)) = &app.follow_plan {
        crate::scene::guided_path(frame, app, plan, s.at, None, app.now(), ship);
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
