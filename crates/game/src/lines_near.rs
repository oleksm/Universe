//! A world drawn from its vector lines, near: the lines within the horizon laid on the ground the
//! patches show (each edge cut finer toward the eye, each point at the ground's height there), so the
//! planet keeps its lines down to the ground. Laid again on a thread of its own when the eye has moved
//! on, the last laid drawn till then.

use std::sync::{Arc, Mutex};

use universe_engine::glam::{DQuat, DVec3, Vec3};
use universe_engine::{Frame, Mesh, Transform, WireModel};
use universe_sim::world::worlds::LinesDraw;
use universe_sim::StarSystem;

/// The lines near the eye: those laid, and those being laid.
#[derive(Default)]
pub struct Near {
    shown: Option<Laid>,
    /// Being laid: what for, and where they'll be put when done.
    working: Option<(Asked, Arc<Mutex<Option<Laid>>>)>,
    /// Frames since the last laying began (to try again while the ground is still being read).
    since: u32,
}

/// What lines were laid for: which world (system, body), the eye's direction and its height over the
/// datum (m).
#[derive(Clone, Copy)]
struct Asked {
    key: (usize, usize),
    eye_dir: DVec3,
    eye_alt: f64,
}

/// Lines laid round one place.
struct Laid {
    asked: Asked,
    /// Their origin (body frame, m): the ground under the eye then; the mesh is round it.
    origin: DVec3,
    mesh: Mesh,
    /// All the ground under them was read (else they're laid again).
    whole: bool,
}

/// The finest and coarsest cut of an edge (m), and the cut a metre of distance from the eye.
const STEP_MIN: f64 = 20.0;
const STEP_MAX: f64 = 20_000.0;
const STEP_PER_M: f64 = 0.01;
/// How far over the ground a line is drawn: at least this (m), more with distance from the eye.
const OVER_MIN: f64 = 1.5;
const OVER_PER_M: f64 = 0.002;

/// The lines of world `key` near the eye, laid again (on a thread) if the eye has moved on since.
#[allow(clippy::too_many_arguments)]
pub fn draw(near: &mut Near, frame: &mut Frame, key: (usize, usize), sys: &Arc<StarSystem>, lines: &Arc<LinesDraw>, center: DVec3, rotation: DQuat, eye: DVec3) {
    let r = sys.bodies[key.1].rail.radius;
    let eye_local = rotation.inverse() * (eye - center);
    let asked = Asked { key, eye_dir: eye_local.normalize(), eye_alt: (eye_local.length() - r).max(1.0) };
    near.since += 1;
    if let Some((_, slot)) = &near.working
        && let Some(laid) = slot.lock().ok().and_then(|mut s| s.take())
    {
        near.shown = Some(laid);
        near.working = None;
    }
    if near.working.is_none() {
        let stale = match &near.shown {
            None => true,
            Some(n) => {
                let a = n.asked;
                let moved = a.eye_dir.angle_between(asked.eye_dir) * r;
                a.key != key || moved > 0.15 * a.eye_alt.max(300.0) || asked.eye_alt > a.eye_alt * 1.4 || asked.eye_alt < a.eye_alt / 1.4 || (!n.whole && near.since > 30)
            }
        };
        if stale {
            let slot = Arc::new(Mutex::new(None));
            let (out, sys, lines) = (slot.clone(), sys.clone(), lines.clone());
            std::thread::spawn(move || {
                if let Some(laid) = lay(asked, &sys, &lines, eye_local)
                    && let Ok(mut s) = out.lock()
                {
                    *s = Some(laid);
                }
            });
            near.working = Some((asked, slot));
            near.since = 0;
        }
    }
    if let Some(n) = near.shown.as_ref().filter(|n| n.asked.key == key) {
        let at = Transform { position: center + rotation * n.origin, rotation: rotation.as_quat(), scale: 1.0 };
        frame.no_shadow(|frame| frame.model_colored(&n.mesh, &at, 1.0, 0.0));
    }
}

/// The lines within the horizon of an eye at `eye_local` (body frame), laid on the ground.
fn lay(asked: Asked, sys: &StarSystem, lines: &LinesDraw, eye_local: DVec3) -> Option<Laid> {
    let (eye_dir, eye_alt) = (asked.eye_dir, asked.eye_alt);
    let body = &sys.bodies[asked.key.1];
    let terrain = body.terrain.as_ref()?;
    let r = body.rail.radius;
    // (The horizon from the eye over the highest ground, and a margin.)
    let reach = ((2.0 * r * (eye_alt + terrain.max_height())).sqrt() * 1.2 + 20_000.0).min(r * 1.2);
    let reach_angle = reach / r;
    let origin = eye_dir * (r + terrain.surface_view(eye_dir).0);
    let dirs: Vec<DVec3> = lines.positions.iter().map(|p| Vec3::from(*p).as_dvec3().normalize()).collect();
    let mut m = WireModel::default();
    let mut whole = true;
    for &[a, b] in &lines.edges {
        let (da, db) = (dirs[a as usize], dirs[b as usize]);
        let (aa, ab) = (da.angle_between(eye_dir), db.angle_between(eye_dir));
        if aa > reach_angle && ab > reach_angle {
            continue;
        }
        // (Cut by the nearer end's distance from the eye.)
        let near_end = if aa < ab { da } else { db };
        let dist = (near_end * r).distance(eye_local);
        let step = (dist * STEP_PER_M).clamp(STEP_MIN, STEP_MAX);
        let n = ((da.angle_between(db) * r / step).ceil() as u32).clamp(1, 400);
        let colour = lines.colors[a as usize];
        let first = m.positions.len() as u32;
        for k in 0..=n {
            let d = da.lerp(db, k as f64 / n as f64).normalize();
            let (h, w) = terrain.surface_view_to(d, step);
            whole &= w;
            let over = OVER_MIN.max((d * r).distance(eye_local) * OVER_PER_M);
            m.positions.push((d * (r + h + over) - origin).as_vec3());
            m.colors.push(colour);
            if k > 0 {
                m.edges.push([first + k - 1, first + k]);
            }
        }
    }
    Some(Laid { asked, origin, mesh: m.into(), whole })
}
