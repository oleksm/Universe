//! Collision warning: fly a copy of the ship ahead as it is set now (engine
//! and thrusters holding), through Dogma, and report the first
//! thing it would hit: a body's surface, a station, a gate's ring (flying
//! through the opening is fine), or another ship (radar contacts, assumed to
//! hold their course). A ship's system that can be switched on or off.
//!
//! The prediction is measured against a reference: the nearest station or
//! gate within range, else the body whose gravity dominates. It stops
//! `RANGE` from where the ship is now, in that reference's frame, or after
//! `HORIZON`.

use glam::DVec3;
use universe_physics::{simulate, Driver, Fact, Feature, Response, RigidBody, Span};
use universe_world::ship::{Ship, SHIP_RADIUS};
use universe_world::StarSystem;

/// How far ahead to look (m), in the reference's frame.
pub const RANGE: f64 = 100_000.0;
/// And for how long at most (s).
pub const HORIZON: f64 = 1800.0;
/// How far the ship may move between contact checks near stations and gates (m).
const CONTACT_RESOLUTION: f64 = 20.0;
/// Most Dogma substeps per prediction (keeps it cheap near stations, where
/// Dogma steps finely).
const BUDGET: u32 = 8000;

/// What the ship would hit.
#[derive(Clone, Debug, PartialEq)]
pub struct Collision {
    /// Seconds from now.
    pub time: f64,
    /// Where, as an offset from the reference's position now (drawn with it).
    pub offset: DVec3,
    /// What: a body's name, or a ship's.
    pub what: String,
    /// Speed of the impact, relative to what's hit (m/s).
    pub speed: f64,
}

/// The ship's path ahead, and the first collision on it.
#[derive(Clone, Debug, Default)]
pub struct Prediction {
    /// The body the path is drawn against (it moves with it).
    pub reference: usize,
    /// Path points as offsets from the reference's position now, with their
    /// times (s from now).
    pub path: Vec<(f64, DVec3)>,
    pub collision: Option<Collision>,
    /// The prediction reached `RANGE` or `HORIZON` without a collision.
    pub clear: bool,
    /// How far ahead it looked (m, in the reference's frame).
    pub reach: f64,
}

/// Another ship, as the radar sees it now.
#[derive(Clone, Debug)]
pub struct Traffic {
    pub name: String,
    pub position: DVec3,
    pub velocity: DVec3,
}

/// The copy's driver: thrust as set, stop on any contact, fly on through
/// gate openings.
struct Coast(DVec3);

impl Driver for Coast {
    fn applied(&mut self, _: &mut RigidBody, _: f64, _: f64, _: &[DVec3]) -> DVec3 {
        self.0
    }

    fn respond(&mut self, fact: &Fact, _: &RigidBody) -> Response {
        match fact {
            Fact::Trigger { .. } => Response::Continue,
            Fact::Contact(_) => Response::Stop,
        }
    }
}

/// The reference for a ship at `p`: the nearest station or gate within
/// `RANGE`, else the dominant body.
fn reference(sys: &StarSystem, p: DVec3, positions: &[DVec3]) -> usize {
    sys.bodies
        .iter()
        .enumerate()
        .filter(|(i, b)| b.rail.collider.is_small(b.rail.radius) && positions[*i].distance(p) < RANGE)
        .min_by(|(i, _), (j, _)| positions[*i].distance(p).total_cmp(&positions[*j].distance(p)))
        .map(|(i, _)| i)
        .unwrap_or_else(|| sys.dominant(p, positions))
}

/// Closest approach between the ship moving from `a` to `b` over `dt` and a
/// ship at `q` moving at `v` (from the same moment): the fraction of the way
/// and the distance.
fn closest(a: DVec3, b: DVec3, dt: f64, q: DVec3, v: DVec3) -> (f64, f64) {
    let r0 = a - q;
    let dv = (b - a) / dt - v;
    let s = if dv.length_squared() < 1e-12 { 0.0 } else { (-r0.dot(dv) / dv.length_squared()).clamp(0.0, dt) };
    (s / dt, (r0 + dv * s).length())
}

/// Predict `ship`'s path in `sys` from time `t` (bodies at `positions`), with
/// the `traffic` around it.
pub fn predict(sys: &StarSystem, ship: &Ship, t: f64, positions: &[DVec3], traffic: &[Traffic]) -> Prediction {
    predict_within(sys, ship, t, positions, traffic, RANGE, HORIZON)
}

/// `predict`, looking at most `range` metres or `horizon` seconds ahead.
pub fn predict_within(sys: &StarSystem, ship: &Ship, t: f64, positions: &[DVec3], traffic: &[Traffic], range: f64, horizon: f64) -> Prediction {
    let reference = reference(sys, ship.position, positions);
    let mut out = Prediction { reference, ..Default::default() };
    let mut scratch = Vec::new();
    let mut reference_at = |time: f64| {
        sys.positions(time, &mut scratch);
        scratch[reference]
    };
    let origin = positions[reference];
    let start = ship.position - origin;
    out.path.push((0.0, start));

    // Other ships fall under the same gravity (in orbit, straight lines would
    // part in seconds).
    let mut others: Vec<(DVec3, DVec3)> = traffic.iter().map(|o| (o.position, o.velocity)).collect();
    let mut rails = Vec::new();
    let mut body = ship.rigid();
    let mut driver = Coast(ship.thrust());
    let mut time = t;
    let mut spent = 0;
    let mut prev = (0.0, ship.position);
    while spent < BUDGET {
        // Points close together where things are close, sparse far out.
        let here = body.position - reference_at(time);
        let near = others.iter().map(|o| o.0.distance(body.position)).fold(here.length(), f64::min);
        let rel_speed = (body.velocity - sys.velocity(reference, time)).length().max(1.0);
        sys.positions(time, &mut rails);
        let ground = sys.dominant(body.position, &rails);
        let altitude = (body.position.distance(rails[ground]) - sys.bodies[ground].rail.radius).max(20.0);
        let piece = (near / rel_speed * 0.05).min(altitude / (rel_speed + 50.0) * 0.5).clamp(0.05, 30.0);
        // Short substeps near the ground, so an impact is timed to within a fraction of a second.
        let max_h = (altitude / (rel_speed + 50.0) * 0.05).clamp(0.05, 30.0);
        // Near stations and gates, substeps of about `CONTACT_RESOLUTION` of
        // relative motion: enough to catch a hull or a ring.
        let contact_step = (CONTACT_RESOLUTION / rel_speed).clamp(universe_physics::integrate::FINE_STEP, 2.0);
        let (next, outcome) = simulate(&sys.bodies, None, &body, Span { t: time, dt: piece, max_h, contact_step }, &mut driver);
        spent += 1 + (outcome.simulated / contact_step).min(2000.0) as u32;
        let now = outcome.time;
        let ahead = now - t;
        // Other ships along this stretch.
        let span = (ahead - prev.0).max(1e-6);
        for (k, o) in traffic.iter().enumerate() {
            let (q, v0) = others[k];
            let g = universe_physics::gravity(&sys.bodies, q, &rails);
            let v1 = v0 + g * span;
            let q1 = q + (v0 + v1) * 0.5 * span;
            others[k] = (q1, v1);
            let v = (q1 - q) / span;
            let (f, d) = closest(prev.1, next.position, span, q, v);
            if d < 2.0 * SHIP_RADIUS {
                let at = prev.1 + (next.position - prev.1) * f;
                let when = prev.0 + span * f;
                let rel = (next.position - prev.1) / span - v;
                let offset = at - reference_at(t + when);
                out.path.push((when, offset));
                out.collision = Some(Collision { time: when, offset, what: o.name.clone(), speed: rel.length() });
                return out;
            }
        }
        let offset = next.position - reference_at(now);
        out.path.push((ahead, offset));
        out.reach = out.reach.max(offset.distance(start));
        if let Some(Fact::Contact(c)) = outcome.fact {
            let b = &sys.bodies[c.body];
            let what = match c.feature {
                Feature::Surface { liquid: true } => format!("{} (SEA)", b.name),
                _ => b.name.clone(),
            };
            out.collision = Some(Collision { time: ahead, offset, what, speed: c.relative_velocity.length() });
            return out;
        }
        if offset.distance(start) > range || ahead > horizon {
            out.clear = true;
            return out;
        }
        body = next;
        time = now;
        prev = (ahead, next.position);
    }
    out
}
