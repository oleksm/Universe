//! The ground near a world: its terrain as a cube-sphere quadtree of
//! patches, each a grid of the very height the physics lands on, finer the
//! nearer the eye (down to tens of metres underfoot). Patches are made on a
//! pool of their own, nearest first, and taken in a few a frame as they're
//! done: no frame waits on one. Until a patch is ready the nearest made above
//! it stands in, and they're kept a while. Each has its own origin, so nothing shakes up
//! close; skirts hang from their edges to hide the seams between sizes.

use std::collections::HashMap;
use std::sync::Arc;

use universe_engine::glam::{DQuat, DVec3};
use universe_engine::{Color, Frame, GlobeMap, Mesh, Transform, WireModel};
use universe_sim::Body;

/// Cells a side of a patch.
const GRID: u32 = 16;
/// The finest patches (about 1/2^MAX_LEVEL of a cube face across).
const MAX_LEVEL: u8 = 15;
/// A patch splits when the eye is nearer than this many times its size.
const SPLIT: f64 = 2.4;
/// Patches wanted a frame (the next finer), at most.
const BUDGET: usize = 12;
/// Patches being made at once, at most.
const IN_FLIGHT: usize = 48;
/// Patches taken in a frame, at most (each an upload).
const TAKE: usize = 24;
/// Frames an unused patch is kept.
const KEEP: u64 = 600;

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
struct Key {
    system: usize,
    body: usize,
    face: u8,
    level: u8,
    x: u32,
    y: u32,
}

impl Key {
    fn children(self) -> [Key; 4] {
        let (x, y, level) = (self.x * 2, self.y * 2, self.level + 1);
        [(0, 0), (1, 0), (0, 1), (1, 1)].map(|(dx, dy)| Key { level, x: x + dx, y: y + dy, ..self })
    }

    /// Its middle's direction, and its size (radians across).
    fn shape(self) -> (DVec3, f64) {
        let n = (1u32 << self.level) as f64;
        let (u, v) = ((self.x as f64 + 0.5) / n * 2.0 - 1.0, (self.y as f64 + 0.5) / n * 2.0 - 1.0);
        (face_dir(self.face, u, v), std::f64::consts::FRAC_PI_2 / n)
    }
}

struct Patch {
    mesh: Mesh,
    /// Made before all its ground was read: made again once it is.
    partial: bool,
    /// Its origin (body frame, m).
    origin: DVec3,
    used: u64,
}

/// The patches made so far, for every world in view.
/// A patch made: its key, mesh, origin, and whether its ground was all read.
type Made = (Key, Mesh, DVec3, bool);

#[derive(Default)]
pub struct Lod {
    patches: HashMap<Key, Patch>,
    frame: u64,
    /// Made on the pool (two cores left to the rest), and those being made.
    pool: Option<rayon::ThreadPool>,
    pending: std::collections::HashSet<Key>,
    done: Arc<std::sync::Mutex<Vec<Made>>>,
}

impl Key {
    fn parent(self) -> Option<Key> {
        (self.level > 0).then(|| Key { level: self.level - 1, x: self.x / 2, y: self.y / 2, ..self })
    }
}

/// A direction on the world from a cube face and a place on it (-1..1 each
/// way), warped so the cells are about the same size everywhere.
fn face_dir(face: u8, u: f64, v: f64) -> DVec3 {
    let (u, v) = ((u * std::f64::consts::FRAC_PI_4).tan(), (v * std::f64::consts::FRAC_PI_4).tan());
    let d = match face {
        0 => DVec3::new(1.0, -v, -u),
        1 => DVec3::new(-1.0, -v, u),
        2 => DVec3::new(u, 1.0, v),
        3 => DVec3::new(u, -1.0, -v),
        4 => DVec3::new(u, -v, 1.0),
        _ => DVec3::new(-u, -v, -1.0),
    };
    d.normalize()
}

/// The ground's height at `dir` (m; the sea's surface over an Earth-like world's deeps), as far
/// as it's read (a world's bake reads in the background), and whether that's all of it.
fn ground(body: &Body, dir: DVec3) -> (f64, bool) {
    body.terrain.as_ref().map_or((0.0, true), |t| t.surface_view(dir))
}

/// A patch's mesh: its grid on the ground (round its origin), and skirts.
fn make(body: &Body, key: Key) -> (WireModel, DVec3, bool) {
    let r = body.rail.radius;
    let n = (1u32 << key.level) as f64;
    let (mid, size) = key.shape();
    let whole = std::cell::Cell::new(true);
    let at = |d: DVec3| {
        let (h, w) = ground(body, d);
        whole.set(whole.get() && w);
        h
    };
    let origin = mid * (r + at(mid));
    let g = GRID as usize + 1;
    let place = |i: usize, j: usize, drop: f64| {
        let (u, v) = ((key.x as f64 + i as f64 / GRID as f64) / n * 2.0 - 1.0, (key.y as f64 + j as f64 / GRID as f64) / n * 2.0 - 1.0);
        let d = face_dir(key.face, u, v);
        ((d * (r + at(d) - drop) - origin).as_vec3(), d)
    };
    let mut m = WireModel { smooth: true, ..WireModel::default() };
    let mut dirs = Vec::with_capacity(g * g);
    for j in 0..g {
        for i in 0..g {
            let (p, d) = place(i, j, 0.0);
            m.positions.push(p);
            dirs.push(d);
        }
    }
    // (Wound to face out: which way round depends on the face.)
    let out = {
        let (a, b, c) = (m.positions[0], m.positions[1], m.positions[g]);
        (b - a).cross(c - a).dot(dirs[0].as_vec3()) > 0.0
    };
    let tri = |m: &mut WireModel, a: u32, b: u32, c: u32| m.faces.push(if out { [a, b, c] } else { [a, c, b] });
    for j in 0..GRID as usize {
        for i in 0..GRID as usize {
            let k = |i: usize, j: usize| (j * g + i) as u32;
            tri(&mut m, k(i, j), k(i + 1, j), k(i + 1, j + 1));
            tri(&mut m, k(i, j), k(i + 1, j + 1), k(i, j + 1));
        }
    }
    // Skirts: from each edge down a way (their own vertices, so they don't
    // bend the ground's shading), both sides drawn.
    let drop = size * r * 0.02 + 20.0;
    let edge: Vec<(usize, usize)> = (0..g)
        .map(|i| (i, 0))
        .chain((0..g).map(|j| (g - 1, j)))
        .chain((0..g).rev().map(|i| (i, g - 1)))
        .chain((0..g).rev().map(|j| (0, j)))
        .collect();
    let base = m.positions.len() as u32;
    for &(i, j) in &edge {
        m.positions.push(place(i, j, 0.0).0);
        m.positions.push(place(i, j, drop).0);
    }
    for w in 0..edge.len() as u32 - 1 {
        let (a, b, c, d) = (base + w * 2, base + w * 2 + 2, base + w * 2 + 1, base + w * 2 + 3);
        m.faces.extend([[a, b, d], [a, d, c], [a, d, b], [a, c, d]]);
    }
    m.colors = vec![[1.0; 4]; m.positions.len()];
    (m, origin, !whole.get())
}

impl Lod {
    /// Patches made and kept.
    pub fn patch_count(&self) -> usize {
        self.patches.len()
    }

    /// Draw world `body` (of system `system`, its middle at `center`,
    /// turned `rotation`) as patches for an eye at `eye`, surfaced from `map`.
    #[allow(clippy::too_many_arguments)]
    pub fn draw(&mut self, frame: &mut Frame, system: usize, sys: &Arc<universe_sim::StarSystem>, body_index: usize, map: &Arc<GlobeMap>, center: DVec3, rotation: DQuat, eye: DVec3, tint: Color) {
        let body = &sys.bodies[body_index];
        self.frame += 1;
        let now = self.frame;
        // Take in what the pool has made (a few a frame: each is an upload).
        let made: Vec<Made> = self.done.lock().map(|mut d| {
            let n = d.len().min(TAKE);
            d.drain(..n).collect()
        }).unwrap_or_default();
        for (k, mesh, origin, partial) in made {
            self.pending.remove(&k);
            self.patches.insert(k, Patch { mesh, origin, used: now, partial });
        }
        let r = body.rail.radius;
        let eye_local = rotation.inverse() * (eye - center);
        let eye_dist = eye_local.length();
        let eye_dir = eye_local / eye_dist.max(1.0);
        // (The lowest ground there could be: what the horizon may hide behind.)
        let low = r - body.terrain.as_ref().map_or(0.0, |t| t.max_height());
        let horizon = (low / eye_dist).clamp(-1.0, 1.0).acos();
        let mut draw = Vec::new();
        let mut want = Vec::new();
        let mut stack: Vec<Key> = (0..6).map(|face| Key { system, body: body_index, face, level: 0, x: 0, y: 0 }).collect();
        while let Some(key) = stack.pop() {
            let (mid, size) = key.shape();
            // Behind the horizon: none of it in sight.
            if mid.angle_between(eye_dir) > horizon + size * 0.75 + 0.02 {
                continue;
            }
            let dist = (mid * r).distance(eye_local);
            if key.level < MAX_LEVEL && dist < SPLIT * size * r {
                let children = key.children();
                if children.iter().all(|c| self.patches.contains_key(c)) {
                    // (All four kept while their parent splits, those out of
                    // sight too: dropped, the parent stood in for a frame —
                    // a coarse patch metres off the ground, at your feet.)
                    for c in &children {
                        if let Some(p) = self.patches.get_mut(c) {
                            p.used = now;
                        }
                    }
                    stack.extend(children);
                    continue;
                }
                for c in children {
                    if !self.patches.contains_key(&c) && want.len() < BUDGET && !want.contains(&c) {
                        want.push(c);
                    }
                }
            }
            draw.push(key);
        }
        // The whole faces made now if they're missing (nothing stands in for them).
        for key in draw.iter().copied().filter(|k| k.level == 0 && !self.patches.contains_key(k)).collect::<Vec<_>>() {
            let (m, origin, partial) = make(body, key);
            self.pending.remove(&key);
            self.patches.insert(key, Patch { mesh: m.into(), origin, used: now, partial });
        }
        // The rest made on the pool, nearest first (in its own sizes): what must be drawn now,
        // the next finer, and made again what was made before all its ground was read.
        let mut todo: Vec<Key> = draw.iter().copied().filter(|k| !self.patches.contains_key(k)).collect();
        todo.extend(want);
        todo.extend(draw.iter().copied().filter(|k| self.patches.get(k).is_some_and(|p| p.partial)));
        todo.retain(|k| !self.pending.contains(k));
        let near = |k: &Key| {
            let (mid, size) = k.shape();
            (mid * r).distance(eye_local) / (size * r)
        };
        todo.sort_by(|a, b| near(a).total_cmp(&near(b)));
        todo.dedup();
        let pool = self.pool.get_or_insert_with(|| {
            let cores = std::thread::available_parallelism().map_or(4, |c| c.get());
            rayon::ThreadPoolBuilder::new().num_threads(cores.saturating_sub(2).max(1)).thread_name(|i| format!("ground {i}")).build().expect("the ground's pool")
        });
        for k in todo.into_iter().take(IN_FLIGHT.saturating_sub(self.pending.len())) {
            self.pending.insert(k);
            let (sys, done) = (sys.clone(), self.done.clone());
            pool.spawn(move || {
                let (m, origin, partial) = make(&sys.bodies[body_index], k);
                if let Ok(mut d) = done.lock() {
                    d.push((k, m.into(), origin, partial));
                }
            });
        }
        // What's not made yet: the nearest made above it stands in (each once).
        let mut shown: Vec<Key> = Vec::with_capacity(draw.len());
        for mut key in draw {
            while !self.patches.contains_key(&key) {
                match key.parent() {
                    Some(p) => key = p,
                    None => break,
                }
            }
            if !shown.contains(&key) {
                shown.push(key);
            }
        }
        // (A stand-in and a patch inside it both drawn: the finer wins in depth, hardly seen.)
        let draw = shown;
        let kind = crate::terrain_view::globe_kind(body);
        let relief = body.terrain.as_ref().map_or(0.0, |t| t.amplitude) as f32;
        let turn = rotation.as_quat();
        let (depth, shell) = crate::terrain_view::air(body).unwrap_or_default();
        for key in draw {
            let Some(p) = self.patches.get_mut(&key) else { continue };
            p.used = now;
            let at = [(p.origin.x / r) as f32, (p.origin.y / r) as f32, (p.origin.z / r) as f32, (1.0 / r) as f32];
            let t = Transform { position: center + rotation * p.origin, rotation: turn, scale: 1.0 };
            frame.no_shadow(|frame| {
                frame.with_air(depth, shell, |frame| {
                    frame.with_globe(map, kind, relief, crate::terrain_view::FILL * 2.5, at, p.origin, |frame| {
                        frame.model_shaded_faded(&p.mesh, &t, tint, tint, 0.0);
                    })
                })
            });
        }
        self.patches.retain(|_, p| now - p.used < KEEP);
    }
}
