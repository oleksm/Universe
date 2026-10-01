//! Asteroids on screen. Each rock gets its own faceted mesh, built from its
//! shape (the same height function it collides with: what's seen is what's
//! hit), the first time it's near. Remnants show from afar; a field's swarm
//! when the camera is among it or close.

use std::collections::HashMap;

use universe_engine::glam::{DVec3, Vec3};
use universe_engine::{Frame, Transform, WireModel};
use universe_sim::world::{Body, StarSystem};

use crate::scene::color;
use crate::App;

/// A swarm is drawn (and its meshes built) within this of its remnant, beyond its reach (m).
const SWARM_SIGHT: f64 = 300_000.0;
/// Fragments show as dots out to this far (m).
const DOT_RANGE: f64 = 60_000.0;

/// A unit icosphere, its faces split `levels` times.
fn icosphere(levels: u32) -> (Vec<Vec3>, Vec<[u32; 3]>) {
    let t = (1.0 + 5f32.sqrt()) / 2.0;
    let mut v: Vec<Vec3> = [(-1.0, t, 0.0), (1.0, t, 0.0), (-1.0, -t, 0.0), (1.0, -t, 0.0), (0.0, -1.0, t), (0.0, 1.0, t), (0.0, -1.0, -t), (0.0, 1.0, -t), (t, 0.0, -1.0), (t, 0.0, 1.0), (-t, 0.0, -1.0), (-t, 0.0, 1.0)]
        .iter()
        .map(|&(x, y, z)| Vec3::new(x, y, z).normalize())
        .collect();
    let mut f: Vec<[u32; 3]> = vec![
        [0, 11, 5], [0, 5, 1], [0, 1, 7], [0, 7, 10], [0, 10, 11], [1, 5, 9], [5, 11, 4], [11, 10, 2], [10, 7, 6], [7, 1, 8],
        [3, 9, 4], [3, 4, 2], [3, 2, 6], [3, 6, 8], [3, 8, 9], [4, 9, 5], [2, 4, 11], [6, 2, 10], [8, 6, 7], [9, 8, 1],
    ];
    for _ in 0..levels {
        let mut mid: HashMap<(u32, u32), u32> = HashMap::new();
        let mut split = |a: u32, b: u32, v: &mut Vec<Vec3>| -> u32 {
            *mid.entry((a.min(b), a.max(b))).or_insert_with(|| {
                v.push(((v[a as usize] + v[b as usize]) * 0.5).normalize());
                v.len() as u32 - 1
            })
        };
        f = f
            .iter()
            .flat_map(|&[a, b, c]| {
                let (ab, bc, ca) = (split(a, b, &mut v), split(b, c, &mut v), split(c, a, &mut v));
                [[a, ab, ca], [b, bc, ab], [c, ca, bc], [ab, bc, ca]]
            })
            .collect();
    }
    (v, f)
}

/// Rock `b`'s mesh, in units of its radius, in its own frame.
pub fn mesh(b: &Body) -> Option<WireModel> {
    b.rock.as_ref()?;
    let (dirs, faces) = icosphere(2);
    let r = b.rail.radius;
    let positions = dirs.iter().map(|&d| d * (b.surface_radius(d.as_dvec3()) / r) as f32).collect();
    let mut edges: Vec<[u32; 2]> = faces.iter().flat_map(|&[a, b, c]| [[a, b], [b, c], [c, a]]).map(|[a, b]| [a.min(b), a.max(b)]).collect();
    edges.sort_unstable();
    edges.dedup();
    Some(WireModel { positions, edges, faces, colors: Vec::new() })
}

/// The field whose swarm is in sight from `p` (bodies at `positions`).
fn swarm_in_sight(sys: &StarSystem, p: DVec3, positions: &[DVec3]) -> Option<usize> {
    sys.fields.iter().position(|f| positions.get(f.body).is_some_and(|q| q.distance(p) < f.extent + SWARM_SIGHT))
}

impl App {
    /// Meshes for the remnants in view, and for the swarm in sight (built once, on first sight).
    pub fn build_rocks(&mut self) {
        let origin = self.view.origin;
        let sys = self.view.system.clone();
        if self.view.positions.len() < sys.bodies.len() {
            return;
        }
        for (f, field) in sys.fields.iter().enumerate() {
            if !self.rocks.contains_key(&(origin, f, field.body))
                && let Some(m) = mesh(&sys.bodies[field.body])
            {
                self.rocks.insert((origin, f, field.body), m.into());
            }
        }
        let Some(f) = swarm_in_sight(&sys, self.camera.position, &self.view.positions) else { return };
        let bodies = sys.field_bodies(f);
        for i in sys.bodies.len()..bodies.len() {
            if !self.rocks.contains_key(&(origin, f, i))
                && let Some(m) = mesh(&bodies[i])
            {
                self.rocks.insert((origin, f, i), m.into());
            }
        }
    }
}

/// One rock, body `i` of field `f`'s bodies, at `center`.
fn rock(frame: &mut Frame, app: &App, f: usize, i: usize, b: &Body, center: DVec3, t: f64) {
    let c = color(b.color);
    let px = frame.projected_radius(center, b.rail.radius);
    if px < 3.0 {
        // Too small to make out: the sensors mark it (a diamond a few
        // pixels across, fading with distance), else a point of light.
        let d = center.distance(frame.camera.position);
        if d < DOT_RANGE {
            let to = (center - frame.camera.position) / d;
            let (u, v) = (to.any_orthonormal_vector(), to.cross(to.any_orthonormal_vector()));
            let s = d * 0.004;
            let k = c.scale((1.0 - d / DOT_RANGE) as f32 * 0.8 + 0.2);
            let p = [center + u * s, center + v * s, center - u * s, center - v * s];
            for j in 0..4 {
                frame.line(p[j], p[(j + 1) % 4], k);
            }
        } else {
            frame.point(center, c.scale(0.7));
        }
        if px < 0.8 {
            return;
        }
    }
    let Some(mesh) = app.rocks.get(&(app.view.origin, f, i)) else {
        frame.point(center, c);
        return;
    };
    let edges = ((px - 2.0) / 30.0).clamp(0.0, 1.0);
    let at = Transform { position: center, rotation: b.rotation(t).as_quat(), scale: b.rail.radius };
    frame.model_shaded_faded(mesh, &at, c.scale(0.8), c.scale(0.3), edges);
}

/// The remnants of the system in view, and the swarm in sight.
pub fn draw(frame: &mut Frame, app: &App) {
    let sys = &app.view.system;
    let t = app.now();
    if app.view.positions.len() < sys.bodies.len() {
        return;
    }
    for (f, field) in sys.fields.iter().enumerate() {
        rock(frame, app, f, field.body, &sys.bodies[field.body], app.view.positions[field.body], t);
    }
    let Some(f) = swarm_in_sight(sys, frame.camera.position, &app.view.positions) else { return };
    let bodies = sys.field_bodies(f);
    let n = sys.bodies.len();
    let mut positions = Vec::with_capacity(bodies.len());
    universe_sim::world::physics::positions(&bodies[..], t, &mut positions);
    // (The remnant's place as the view has it, so the swarm sits round it.)
    let shift = app.view.positions[sys.fields[f].body] - positions[sys.fields[f].body];
    for i in n..bodies.len() {
        rock(frame, app, f, i, &bodies[i], positions[i] + shift, t);
    }
}
