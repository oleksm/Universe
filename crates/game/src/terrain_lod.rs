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
/// The finest patches (about 1/2^MAX_LEVEL of a cube face across: some 320 m on Heath, 20 m
/// cells), and four levels finer (some 1.2 m cells) once the ground's runtime detail is in
/// (`world::detail::ACTIVE`): within a few kilometres of the eye, as `SPLIT` has it.
const MAX_LEVEL: u8 = if universe_sim::world::detail::ACTIVE { 19 } else { 15 };
/// Canonical data already contains its relief. Finer samples resolve that field; no
/// procedural detail is added. Levels16..19 are requested only close to the eye.
const CANONICAL_MAX_LEVEL: u8 = 19;
/// A patch splits when the eye is nearer than this many times its size (the engine's: its mesh
/// shader geomorphs by the same).
const SPLIT: f64 = universe_engine::shaders::GEOMORPH_SPLIT;
/// Patches wanted a frame (the next finer), at most.
const BUDGET: usize = 12;
/// Patches being made at once, at most.
const IN_FLIGHT: usize = 48;
/// Patches taken in a frame, at most (each an upload).
const TAKE: usize = 24;
/// Frames an unused patch is kept.
const KEEP: u64 = 600;
/// Patches kept at most (each about 40 kB, here and on the GPU): past it the least lately used go.
const MAX_PATCHES: usize = 3000;

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
    sources: HashMap<(usize, usize), u64>,
    /// Made on the pool (two cores left to the rest), and those being made.
    pool: Option<rayon::ThreadPool>,
    pending: std::collections::HashSet<Key>,
    done: Arc<std::sync::Mutex<Vec<Made>>>,
    /// Where the eye was last frame (the world's frame) and when: its way ahead, for the look-ahead.
    last_eye: Option<(DVec3, std::time::Instant)>,
}

/// How far ahead the look-ahead looks (s): the patches the eye will want then are made after those
/// it wants now, so they're ready a second or more before they're first drawn (the lab's R3).
const AHEAD: [f64; 3] = [1.0, 2.0, 4.0];
/// Patches the look-ahead asks for in a frame, at most.
const AHEAD_BUDGET: usize = 8;

/// The patches an eye at `eye` (the world's frame, m) would draw, made or not: the leaves of the
/// split as far as it goes, behind the horizon left out (the lowest ground `low` from the centre).
fn ideal(system: usize, body: usize, r: f64, low: f64, eye: DVec3, patches: &HashMap<Key, Patch>, canonical: bool) -> Vec<Key> {
    let eye_dist = eye.length();
    let eye_dir = eye / eye_dist.max(1.0);
    let horizon = (low / eye_dist).clamp(-1.0, 1.0).acos();
    let mut out = Vec::new();
    let mut stack: Vec<Key> = (0..6).map(|face| Key { system, body, face, level: 0, x: 0, y: 0 }).collect();
    while let Some(key) = stack.pop() {
        let (mid, size) = key.shape();
        if mid.angle_between(eye_dir) > horizon + size * 0.75 + 0.02 {
            continue;
        }
        if key.level < (if canonical { CANONICAL_MAX_LEVEL } else { MAX_LEVEL }) && key.centre(patches, r, canonical).distance(eye) < SPLIT * size * r {
            stack.extend(key.children());
        } else {
            out.push(key);
        }
    }
    out
}

impl Key {
    /// Use already prepared terrain metadata, never a canonical query on the draw thread.
    /// A missing child inherits its nearest resident ancestor's elevation until built.
    fn centre(self, patches: &HashMap<Key, Patch>, r: f64, canonical: bool) -> DVec3 {
        let mid = self.shape().0;
        if canonical {
            let mut ancestor = Some(self);
            while let Some(key) = ancestor {
                if let Some(patch) = patches.get(&key) {
                    return mid * patch.origin.length();
                }
                ancestor = key.parent();
            }
        }
        mid * r
    }

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
/// The runtime detail to the band `cell` (m): the patch's cells.
fn ground(body: &Body, dir: DVec3, cell: f64) -> (f64, bool) {
    body.terrain.as_ref().map_or((0.0, true), |t| t.surface_view_to(dir, cell))
}

/// A patch's mesh: its grid on the ground (round its origin), and skirts.
fn make(body: &Body, key: Key) -> (WireModel, DVec3, bool) {
    let r = body.rail.radius;
    let n = (1u32 << key.level) as f64;
    let (mid, size) = key.shape();
    let cell = size * r / GRID as f64;
    let whole = std::cell::Cell::new(true);
    let at = |d: DVec3| {
        let (h, w) = ground(body, d, cell);
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
    // Each vertex's way to its parent's shape (the vertex colour's rgb, m) and the patch's size
    // (its alpha, m), for the mesh shader to blend toward as the eye nears the distance where the
    // parent takes its place (geomorphing: a swap doesn't pop). A vertex on the parent's grid (even
    // both ways) is where it is; one between, the mean of the parent's two either side of it (the
    // cell's diagonal, as its triangles are cut, where it's between both ways).
    let size_m = (size * r) as f32;
    let grid: Vec<universe_engine::glam::Vec3> = m.positions[..g * g].to_vec();
    let at = |i: usize, j: usize| grid[j * g + i];
    let morph = |i: usize, j: usize| -> [f32; 4] {
        let own = at(i, j);
        let target = match (i % 2, j % 2) {
            (0, 0) => own,
            (1, 0) => (at(i - 1, j) + at(i + 1, j)) * 0.5,
            (0, 1) => (at(i, j - 1) + at(i, j + 1)) * 0.5,
            _ => (at(i - 1, j - 1) + at(i + 1, j + 1)) * 0.5,
        };
        let d = target - own;
        [d.x, d.y, d.z, size_m]
    };
    let mut colors: Vec<[f32; 4]> = (0..g * g).map(|k| morph(k % g, k / g)).collect();
    for &(i, j) in &edge {
        colors.push(morph(i, j));
        colors.push(morph(i, j));
    }
    m.colors = colors;
    // Each vertex's surface fields from a baked world's river tiles (wetness, scree, bare rock),
    // and its slope (rise over run) from the heights 600 m either side of it each way (central
    // differences: smooth from vertex to vertex, where a triangle's own normal steps at its edges),
    // for the ground's material: w = ±(1 + slope), + where the fields are read.
    let step = 600.0 / r;
    let fields = |d: DVec3| -> [f32; 4] {
        let Some(t) = body.terrain.as_ref() else { return [0.0; 4] };
        // Palette 4 reads the local authoritative wet mask, independent of the orbit map.
        if let Some(sample) = crate::terrain_view::canonical_texel(t, d) {
            let slope = if crate::pgs_preview::palette(body).is_some_and(|p| p.materials()) {
                t.surface_differential(d, r).map_or(0.0, |(_, derivative)| derivative.slope as f32)
            } else { 0.0 };
            return [sample[1], 0.0, 0.0, -(1.0+slope)];
        }
        let (f, w) = t.surface_fields_view(d);
        whole.set(whole.get() && w);
        let e1 = d.any_orthonormal_vector();
        let e2 = d.cross(e1);
        let h = |v: DVec3| ground(body, (d + v * step).normalize(), 600.0).0;
        let (gx, gy) = ((h(e1) - h(-e1)) / 1200.0, (h(e2) - h(-e2)) / 1200.0);
        let slope = (gx * gx + gy * gy).sqrt() as f32;
        match f {
            Some(f) => [f[0], f[1], f[2], 1.0 + slope],
            None => [0.0, 0.0, 0.0, -(1.0 + slope)],
        }
    };
    let mut data: Vec<[f32; 4]> = dirs.iter().map(|&d| fields(d)).collect();
    for &(i, j) in &edge {
        let f = data[j * g + i];
        data.push(f);
        data.push(f);
    }
    m.data = data;
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
    pub fn draw(&mut self, frame: &mut Frame, system: usize, sys: &Arc<universe_sim::StarSystem>, body_index: usize, map: &Arc<GlobeMap>, center: DVec3, rotation: DQuat, eye: DVec3, tint: Color) -> bool {
        let body = &sys.bodies[body_index];
        let source = (system, body_index);
        if self.sources.get(&source).is_some_and(|id| *id != map.id()) {
            // A new canonical source (including a Studio edit) cannot reuse the
            // previous world's patches or accept its in-flight completions.
            self.patches.clear();
            self.pending.clear();
            self.done = Default::default();
            self.last_eye = None;
        }
        self.sources.insert(source, map.id());
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
        let canonical = body.terrain.as_ref().is_some_and(|t| t.canonical_surface());
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
            let dist = key.centre(&self.patches, r, canonical).distance(eye_local);
            if key.level < (if canonical { CANONICAL_MAX_LEVEL } else { MAX_LEVEL }) && dist < SPLIT * size * r {
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
        // Root patches are optional assets too. The caller draws the base globe
        // until all visible roots are ready; never sample them on the UI thread.
        let missing_root = draw.iter().any(|k| k.level == 0 && !self.patches.contains_key(k));
        // Where the eye is heading: the patches it will want in a second, two, four.
        let clock = std::time::Instant::now();
        let velocity = self.last_eye.map_or(DVec3::ZERO, |(e, t)| (eye_local - e) / clock.duration_since(t).as_secs_f64().max(1e-3));
        self.last_eye = Some((eye_local, clock));
        let mut ahead: Vec<Key> = Vec::new();
        if velocity.length() > 1.0 {
            for t in AHEAD {
                for k in ideal(system, body_index, r, low, eye_local + velocity * t, &self.patches, canonical) {
                    if !self.patches.contains_key(&k) && !self.pending.contains(&k) && !ahead.contains(&k) {
                        ahead.push(k);
                    }
                }
            }
            // (Coarse first: what stands in for the rest.)
            ahead.sort_by_key(|k| k.level);
            ahead.truncate(AHEAD_BUDGET);
        }
        // The rest made on the pool, nearest first (in its own sizes): what must be drawn now,
        // the next finer, and made again what was made before all its ground was read; then the
        // look-ahead's.
        let mut todo: Vec<Key> = draw.iter().copied().filter(|k| !self.patches.contains_key(k)).collect();
        todo.extend(want);
        todo.extend(draw.iter().copied().filter(|k| self.patches.get(k).is_some_and(|p| p.partial)));
        todo.retain(|k| !self.pending.contains(k));
        let near = |k: &Key| {
            let (_, size) = k.shape();
            k.centre(&self.patches, r, canonical).distance(eye_local) / (size * r)
        };
        todo.sort_by(|a, b| near(a).total_cmp(&near(b)));
        todo.dedup();
        let ahead: Vec<Key> = ahead.into_iter().filter(|k| !todo.contains(k)).collect();
        todo.extend(ahead);
        let pool = self.pool.get_or_insert_with(|| {
            rayon::ThreadPoolBuilder::new().num_threads(2).thread_name(|i| format!("ground {i}")).build().expect("the ground's pool")
        });
        for k in todo.into_iter().take(IN_FLIGHT.saturating_sub(self.pending.len())) {
            self.pending.insert(k);
            let (sys, done) = (sys.clone(), self.done.clone());
            pool.spawn(move || {
                let _scope = universe_prof::scope("assets/terrain/patch");
                let (m, origin, partial) = make(&sys.bodies[body_index], k);
                if let Ok(mut d) = done.lock() {
                    d.push((k, m.into(), origin, partial));
                }
            });
        }
        if missing_root {
            #[cfg(feature = "dev")]
            if canonical && (now <= 10 || now % 30 == 0) && std::env::var_os("UNIVERSE_PGS1_CONTACT_AUDIT").is_some() {
                log::info!("PGS_CONTACT {}", serde_json::json!({"frame":now,"state":"base_globe_pending_roots","resident":self.patches.len(),"pending":self.pending.len(),"hits":[]}));
            }
            return false;
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
        #[cfg(feature = "dev")]
        if crate::pgs_debug::active(body) && now % 120 == 0 {
            let mut levels = std::collections::BTreeMap::new();
            for k in &draw { *levels.entry(k.level).or_insert(0usize) += 1; }
            log::info!("SCENERY DEBUG frame={now} resident_draw_levels={levels:?} pending={} eye_height_m={:.6}", self.pending.len(), eye_dist-r);
        }
        #[cfg(feature = "dev")]
        if canonical && (now <= 10 || now % 30 == 0) && std::env::var_os("UNIVERSE_PGS1_CONTACT_AUDIT").is_some() {
            let canonical_height = body.terrain.as_ref().unwrap().surface(eye_dir);
            let mut hits = Vec::new();
            for key in &draw {
                let Some(patch) = self.patches.get(key) else { continue };
                if let Some(height) = rendered_height(patch, eye_local, r) {
                    hits.push(serde_json::json!({"level":key.level,"height_m":height,"gap_m":height-canonical_height}));
                }
            }
            log::info!("PGS_CONTACT {}", serde_json::json!({"frame":now,"radius_m":r,"eye_height_m":eye_dist-r,"canonical_height_m":canonical_height,"resident":self.patches.len(),"pending":self.pending.len(),"hits":hits}));
        }
        let kind = crate::terrain_view::globe_kind(body);
        let relief = body.terrain.as_ref().map_or(0.0, |t| t.amplitude) as f32;
        let turn = rotation.as_quat();
        let (depth, shell) = crate::terrain_view::air(body).unwrap_or_default();
        for key in draw {
            let Some(p) = self.patches.get_mut(&key) else { continue };
            p.used = now;
            let at = [(p.origin.x / r) as f32, (p.origin.y / r) as f32, (p.origin.z / r) as f32, (1.0 / r) as f32];
            let t = Transform { position: center + rotation * p.origin, rotation: turn, scale: 1.0 };
            let draw_patch = |frame: &mut Frame| {
                frame.with_air(depth, shell, |frame| {
                    frame.with_globe(map, kind, relief, crate::terrain_view::globe_bright(body), at, p.origin, |frame| {
                        frame.model_shaded_faded(&p.mesh, &t, tint, tint, 0.0);
                    })
                })
            };
            #[cfg(feature = "dev")]
            crate::pgs_debug::patch(frame, body, &p.mesh, p.origin, center, rotation);
            if crate::pgs_preview::terrain_shadows(body) { frame.ground_shadow(draw_patch); }
            else { frame.no_shadow(draw_patch); }
        }
        #[cfg(feature = "dev")]
        crate::pgs_debug::draw(frame, body, center, rotation);
        self.patches.retain(|_, p| now - p.used < KEEP);
        if self.patches.len() > MAX_PATCHES {
            let mut by_use: Vec<(u64, Key)> = self.patches.iter().map(|(k, p)| (p.used, *k)).collect();
            by_use.sort_unstable_by_key(|(u, _)| *u);
            for (_, k) in by_use.into_iter().take(self.patches.len() - MAX_PATCHES) {
                self.patches.remove(&k);
            }
        }
        true
    }
}

/// Diagnostic ray intersection against the actual submitted grid vertices, including shader
/// geomorph, excluding seam skirts. This does not alter contact or draw selection.
#[cfg(feature = "dev")]
fn rendered_height(patch: &Patch, eye: DVec3, radius: f64) -> Option<f64> {
    let d = -eye.normalize();
    let start = eye - patch.origin;
    let vertex = |id: usize| {
        let own = patch.mesh.positions[id];
        let color = patch.mesh.colors[id];
        let far = (SPLIT * 2.0 * color[3] as f64 * 0.95) as f32;
        let distance = (patch.origin + own.as_dvec3() - eye).length() as f32;
        let t = ((distance-far*0.6)/(far*0.4)).clamp(0.0,1.0);
        (own + universe_engine::glam::Vec3::new(color[0],color[1],color[2]) * (t*t*(3.0-2.0*t))).as_dvec3()
    };
    for f in patch.mesh.faces.iter().take((GRID*GRID*2) as usize) {
        let [a,b,c] = f.map(|i| vertex(i as usize));
        let e1 = b-a; let e2 = c-a;
        let p = d.cross(e2); let det = e1.dot(p);
        if det.abs() < 1e-12 { continue; }
        let tvec = start-a; let u = tvec.dot(p)/det;
        let q = tvec.cross(e1); let v = d.dot(q)/det;
        if u < -1e-8 || v < -1e-8 || u+v > 1.0+1e-8 { continue; }
        let distance = e2.dot(q)/det;
        return Some(eye.length()-distance-radius);
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn elevated_canonical_patch_refines_at_the_feet() {
        let key = Key { system: 0, body: 1, face: 0, level: 13, x: 4096, y: 4096 };
        let r = 6_281_370.0;
        let (mid, size) = key.shape();
        let origin = mid * (r + 5_000.0);
        let patches = HashMap::from([(key, Patch { mesh: WireModel::default().into(), origin, used: 0, partial: false })]);
        let eye = mid * (r + 5_001.7);
        assert!(key.centre(&patches, r, false).distance(eye) > SPLIT * size * r);
        assert!(key.centre(&patches, r, true).distance(eye) < SPLIT * size * r);
        let child = key.children()[0];
        assert!((child.centre(&patches, r, true).length() - (r + 5_000.0)).abs() < 1e-8);
        assert!((child.centre(&HashMap::new(), r, true).length() - r).abs() < 1e-8);
    }
}
