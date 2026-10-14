//! The ground near a world: its terrain as a cube-sphere quadtree of
//! patches, each a grid of the very height the physics lands on, finer the
//! nearer the eye (down to tens of metres underfoot). Patches are made on a
//! pool of their own, nearest first, and taken in a few a frame as they're
//! done: no frame waits on one. Until a patch is ready the nearest made above
//! it stands in, and they're kept a while. Each has its own origin, so nothing shakes up
//! close; skirts hang from their edges to hide the seams between sizes.

pub(crate) mod contact;
pub(crate) mod group;

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
/// Half a pixel at 1280 pixels across 60° (rad): the drawn field may stand this far (as an
/// angle) from level 0 at the distance a patch is drawn from (revision-5 levels).
pub(crate) const DRAWN_TOLERANCE_RAD: f64 = 4.1e-4;
/// The tolerance (m) a patch with cells of `cell` m is drawn to: half a pixel at the nearest
/// distance of its middle where it is drawn (SPLIT times its size; nearer, it splits).
pub(crate) fn patch_tolerance(cell: f64) -> f64 {
    drawn_scale() * DRAWN_TOLERANCE_RAD * SPLIT * GRID as f64 * cell
}
/// 0 under `UNIVERSE_PGS1_LEVEL0_ONLY` (ground patches and the globe map drawn from level 0, for
/// matched captures), else 1.
pub(crate) fn drawn_scale() -> f64 {
    static SCALE: std::sync::OnceLock<f64> = std::sync::OnceLock::new();
    *SCALE.get_or_init(|| if std::env::var_os("UNIVERSE_PGS1_LEVEL0_ONLY").is_some() { 0.0 } else { 1.0 })
}
/// A package's region files resident at most (bytes, page cache), and how long one stays after
/// its last read (ms): past either, its pages are released (`Surface::trim_package`).
const PACKAGE_BUDGET: u64 = 256 << 20;
const PACKAGE_IDLE_MS: f64 = 20_000.0;
/// Frames between residency passes.
const PACKAGE_TRIM_EVERY: u64 = 120;
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
pub(crate) struct Key {
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

pub(crate) struct Patch {
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
    sources: HashMap<(usize, usize), (u64, usize)>,
    /// Made on the pool (two cores left to the rest), and those being made.
    pool: Option<rayon::ThreadPool>,
    pending: std::collections::HashSet<Key>,
    done: Arc<std::sync::Mutex<Vec<Made>>>,
    /// Where the eye was last frame (the world's frame) and when: its way ahead, for the look-ahead.
    last_eye: Option<(DVec3, std::time::Instant)>,
    contact_inputs: contact::ResidentInputs,
    contact_group: group::Runtime,
    readiness: Option<group::Trace>,
}

impl Drop for Lod {
    fn drop(&mut self) {
        if let Some(trace) = self.readiness.as_mut() {
            trace.write(&self.contact_group);
        }
    }
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
/// A canonical surface is read to the patch's tolerance (`patch_tolerance`).
fn ground(body: &Body, dir: DVec3, cell: f64) -> (f64, bool) {
    let Some(t) = body.terrain.as_ref() else { return (0.0, true) };
    if let Some(h) = t.canonical_view_to(dir, patch_tolerance(cell)) { return (h, true); }
    t.surface_view_to(dir, cell)
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
    // Each vertex's surface fields and slope, for the ground's material (see `fields_at`).
    let fields = |d: DVec3| -> [f32; 4] {
        let (f, w) = fields_at(body, r, d, patch_tolerance(cell));
        whole.set(whole.get() && w);
        f
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

/// A ground vertex's surface fields at `dir` from a baked world's river tiles (wetness, scree, bare
/// rock), and its slope (rise over run) from the heights 600 m either side of it each way (central
/// differences: smooth from vertex to vertex, where a triangle's own normal steps at its edges), for
/// the ground's material: w = ±(1 + slope), + where the fields are read. And whether all was read.
/// A canonical surface is read to `tolerance` (m; 0 is level 0).
pub(crate) fn fields_at(body: &Body, r: f64, d: DVec3, tolerance: f64) -> ([f32; 4], bool) {
    let Some(t) = body.terrain.as_ref() else { return ([0.0; 4], true) };
    // Palette 4 reads the local authoritative wet mask, independent of the orbit map.
    if let Some(sample) = crate::terrain_view::canonical_texel(t, d, tolerance) {
        let slope = if crate::pgs_preview::palette(body).is_some_and(|p| p.materials()) {
            t.surface_differential_to(d, r, tolerance).map_or(0.0, |(_, derivative)| derivative.slope as f32)
        } else { 0.0 };
        return ([sample[1], 0.0, 0.0, -(1.0+slope)], true);
    }
    let step = 600.0 / r;
    let (f, w) = t.surface_fields_view(d);
    let e1 = d.any_orthonormal_vector();
    let e2 = d.cross(e1);
    let h = |v: DVec3| ground(body, (d + v * step).normalize(), 600.0).0;
    let (gx, gy) = ((h(e1) - h(-e1)) / 1200.0, (h(e2) - h(-e2)) / 1200.0);
    let slope = (gx * gx + gy * gy).sqrt() as f32;
    (match f {
        Some(f) => [f[0], f[1], f[2], 1.0 + slope],
        None => [0.0, 0.0, 0.0, -(1.0 + slope)],
    }, w)
}

impl Lod {
    /// Immutable actual-selected inputs; no requested or ideal neighbours.
    pub(crate) fn contact_snapshot(&self) -> Option<Arc<contact::Snapshot>> { self.contact_inputs.snapshot() }

    /// Patches made and kept.
    pub fn patch_count(&self) -> usize {
        self.patches.len()
    }

    /// The actual worker/fallback selection shared by drawing and bounded CPU export.
    /// This does not invent missing children or reconstruct resident geometry.
    fn select_resident(&mut self, system:usize, sys:&Arc<universe_sim::StarSystem>, body_index:usize, map_id:u64, center:DVec3, rotation:DQuat, eye:DVec3)->Option<Vec<Key>> {
        let body = &sys.bodies[body_index];
        let source = (system, body_index);
        let surface = body.terrain.as_ref().and_then(|t| t.surface_source());
        let source_identity = (map_id, surface.as_ref().map_or(0, |s| Arc::as_ptr(s) as usize));
        if self.sources.get(&source).is_some_and(|id| *id != source_identity) {
            // A new canonical source (including a Studio edit) cannot reuse the
            // previous world's patches or accept its in-flight completions.
            self.patches.clear();
            self.pending.clear();
            self.done = Default::default();
            self.last_eye = None;
            self.contact_inputs.invalidate();
            self.contact_group.invalidate(self.frame);
        }
        self.sources.insert(source, source_identity);
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
        // The selection as the eye would have it: what is made and wanted follows this one.
        let (draw, want) = self.traverse(system, body_index, r, canonical, eye_local, eye_dir, horizon, now, &Default::default());
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
            self.contact_inputs.invalidate();
            #[cfg(feature = "dev")]
            if canonical && (now <= 10 || now % 30 == 0) && std::env::var_os("UNIVERSE_PGS1_CONTACT_AUDIT").is_some() {
                log::info!("PGS_CONTACT {}", serde_json::json!({"frame":now,"state":"base_globe_pending_roots","resident":self.patches.len(),"pending":self.pending.len(),"hits":[]}));
            }
            return None;
        }
        // Settled round the eye: nothing it wants within 300 m is standing in for.
        let settled = draw.iter().all(|k| self.patches.contains_key(k) || k.centre(&self.patches, r, canonical).distance(eye_local) > 300.0 + k.shape().1 * r);
        let natural = self.stand_in(draw);
        if !(canonical && surface.is_some() && std::env::var_os("UNIVERSE_PGS1_CONTACT_GROUP").is_some()) {
            self.contact_inputs.invalidate();
            return Some(natural);
        }
        // A contact group built and on the GPU takes over before this frame's selection, so its
        // patches are pinned in it; standing groups' patches stay as they are (see `traverse`).
        let natural_set: std::collections::HashSet<Key> = natural.iter().copied().collect();
        self.contact_group.promote(now, &self.patches);
        let pinned = self.contact_group.pinned();
        let shown = if pinned.is_empty() {
            natural.clone()
        } else {
            let (draw, _) = self.traverse(system, body_index, r, canonical, eye_local, eye_dir, horizon, now, &pinned);
            self.stand_in(draw)
        };
        self.contact_inputs.capture((system, body_index, map_id), r, eye_local, surface, &natural, &self.patches);
        if let Some(pool) = self.pool.as_ref() {
            self.contact_group.update(now, &shown, &natural_set, settled, &self.patches, self.contact_inputs.snapshot(), eye_local, sys, body_index, pool);
        }
        Some(shown)
    }

    /// The patches to draw for an eye at `eye` (body frame), and the next finer wanted. Pinned
    /// patches are kept as they are: never split, and what holds them always split down to them
    /// (while all four of each split's children are resident).
    #[allow(clippy::too_many_arguments)]
    fn traverse(&mut self, system: usize, body_index: usize, r: f64, canonical: bool, eye_local: DVec3, eye_dir: DVec3, horizon: f64, now: u64, pinned: &std::collections::HashSet<Key>) -> (Vec<Key>, Vec<Key>) {
        let above: std::collections::HashSet<Key> = pinned.iter().flat_map(|k| std::iter::successors(k.parent(), |p| p.parent())).collect();
        let mut draw = Vec::new();
        let mut want = Vec::new();
        let mut stack: Vec<Key> = (0..6).map(|face| Key { system, body: body_index, face, level: 0, x: 0, y: 0 }).collect();
        while let Some(key) = stack.pop() {
            let (mid, size) = key.shape();
            let held = pinned.contains(&key) || above.contains(&key);
            // Behind the horizon: none of it in sight.
            if !held && mid.angle_between(eye_dir) > horizon + size * 0.75 + 0.02 {
                continue;
            }
            let split = if pinned.contains(&key) {
                false
            } else if above.contains(&key) {
                true
            } else {
                let dist = key.centre(&self.patches, r, canonical).distance(eye_local);
                key.level < (if canonical { CANONICAL_MAX_LEVEL } else { MAX_LEVEL }) && dist < SPLIT * size * r
            };
            if split {
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
        (draw, want)
    }

    /// What's not made yet: the nearest made above it stands in (each once).
    fn stand_in(&self, draw: Vec<Key>) -> Vec<Key> {
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
        shown
    }

    fn evict_unused(&mut self, now:u64) {
        self.patches.retain(|_, p| now - p.used < KEEP);
        if self.patches.len() > MAX_PATCHES {
            let mut by_use: Vec<(u64, Key)> = self.patches.iter().map(|(k, p)| (p.used, *k)).collect();
            by_use.sort_unstable_by_key(|(u, _)| *u);
            for (_, k) in by_use.into_iter().take(self.patches.len() - MAX_PATCHES) {
                self.patches.remove(&k);
            }
        }
    }

    /// Draw world `body` (of system `system`, its middle at `center`,
    /// turned `rotation`) as patches for an eye at `eye`, surfaced from `map`.
    #[allow(clippy::too_many_arguments)]
    pub fn draw(&mut self, frame: &mut Frame, system: usize, sys: &Arc<universe_sim::StarSystem>, body_index: usize, map: &Arc<GlobeMap>, center: DVec3, rotation: DQuat, eye: DVec3, tint: Color, time_s: f64, on_foot: bool) -> bool {
        let draw = match self.select_resident(system, sys, body_index, map.id(), center, rotation, eye) {
            Some(draw) => draw, None => return false,
        };
        let body = &sys.bodies[body_index];
        let now = self.frame;
        let r = body.rail.radius;
        let canonical = body.terrain.as_ref().is_some_and(|t| t.canonical_surface());
        let eye_local = rotation.inverse() * (eye - center);
        let eye_dist = eye_local.length();
        let eye_dir = eye_local / eye_dist.max(1.0);
        if canonical && now % PACKAGE_TRIM_EVERY == 0 {
            if let Some(trim) = body.terrain.as_ref().and_then(|t| t.surface_source()).and_then(|s| s.trim_package(PACKAGE_IDLE_MS, PACKAGE_BUDGET)) {
                if trim["released"].as_array().is_some_and(|r| !r.is_empty()) { log::info!("PGS_RESIDENCY {}", serde_json::json!({"frame":now,"trim":trim})); }
            }
        }
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
        self.contact_group.prepare(frame, now);
        let group = self.contact_group.active.as_ref();
        // Each piece drawn: its mesh, origin and how many of its triangles are ground (not skirt).
        let mut pieces: Vec<(&Mesh, DVec3, usize)> = Vec::with_capacity(draw.len() + 2);
        for key in &draw {
            if let Some(p) = self.patches.get_mut(key) {
                p.used = now;
            }
        }
        for key in &draw {
            let Some(p) = self.patches.get(key) else { continue };
            // A published contact group draws its own in place of the patches it replaces.
            match group.and_then(|g| g.patches.get(key)) {
                Some(Some((m, ground))) => pieces.push((m, p.origin, *ground)),
                Some(None) => {}
                None => pieces.push((&p.mesh, p.origin, (GRID * GRID * 2) as usize)),
            }
        }
        if let Some(g) = group {
            pieces.push((&g.band, g.meta.band_origin, g.band.faces.len()));
            pieces.extend(g.rim.iter().map(|m| (m, g.meta.band_origin, m.faces.len())));
        }
        for &(mesh, origin, _) in &pieces {
            let at = [(origin.x / r) as f32, (origin.y / r) as f32, (origin.z / r) as f32, (1.0 / r) as f32];
            let t = Transform { position: center + rotation * origin, rotation: turn, scale: 1.0 };
            let draw_patch = |frame: &mut Frame| {
                frame.with_air(depth, shell, |frame| {
                    frame.with_globe(map, kind, relief, crate::terrain_view::globe_bright(body), at, origin, |frame| {
                        frame.model_shaded_faded(mesh, &t, tint, tint, 0.0);
                    })
                })
            };
            #[cfg(feature = "dev")]
            crate::pgs_debug::patch(frame, body, mesh, origin, center, rotation);
            if crate::pgs_preview::terrain_shadows(body) { frame.ground_shadow(draw_patch); }
            else { frame.no_shadow(draw_patch); }
        }
        if canonical && std::env::var_os("UNIVERSE_PGS1_CONTACT_GROUP").is_some() {
            let surface = body.terrain.as_ref().and_then(|t| t.surface_source());
            if self.readiness.is_none() {
                self.readiness = surface.as_ref().and_then(|s| group::Trace::from_env(r, s));
            }
            if let (Some(trace), Some(surface)) = (self.readiness.as_mut(), surface) {
                // Diagnostic only: the ground under the eye, from the pieces actually submitted.
                let submitted = pieces
                    .iter()
                    .filter(|(m, o, _)| (*o - eye_dir * o.dot(eye_dir)).length() <= m.radius() as f64 + 1.)
                    .filter_map(|&(m, o, ground)| group::ray_hit(m, ground, o, eye_local))
                    .reduce(f64::max);
                trace.record(now, time_s, on_foot, eye_local, &surface, submitted, &self.contact_group, self.pending.len(), self.patches.len());
            }
        }
        #[cfg(feature = "dev")]
        crate::pgs_debug::draw(frame, body, center, rotation);
        self.evict_unused(now);
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

#[cfg(test)]
mod pinned {
    use super::*;
    #[test]
    fn a_pinned_patch_is_drawn_as_it_is_and_its_holders_split_down_to_it() {
        let r = 6_281_370.0;
        let root = Key { system: 0, body: 1, face: 0, level: 0, x: 0, y: 0 };
        let mut lod = Lod::default();
        let patch = |k: Key| Patch { mesh: WireModel::default().into(), origin: k.shape().0 * r, used: 0, partial: false };
        let mut keys = vec![root];
        keys.extend(root.children());
        keys.extend(root.children()[0].children());
        for k in &keys {
            lod.patches.insert(*k, patch(*k));
        }
        let target = root.children()[0].children()[3];
        // Far off, the eye would have the root alone; right over the target, finer than it has.
        let (far, near) = (DVec3::X * r * 10.0, target.shape().0 * (r + 2.0));
        let run = |lod: &mut Lod, eye: DVec3, pins: &std::collections::HashSet<Key>| {
            let mut d = lod.traverse(0, 1, r, false, eye, eye.normalize(), 3.0, 1, pins).0;
            d.retain(|k| k.face == 0);
            d.sort_by_key(|k| (k.level, k.x, k.y));
            d
        };
        let none = Default::default();
        assert_eq!(run(&mut lod, far, &none), vec![root]);
        assert!(!run(&mut lod, near, &none).contains(&root));
        let pins = std::collections::HashSet::from([target]);
        for eye in [far, near] {
            let drawn = run(&mut lod, eye, &pins);
            assert!(drawn.contains(&target), "{drawn:?}");
            assert!(!drawn.contains(&root) && !drawn.contains(&root.children()[0]));
        }
    }
}

#[cfg(test)]
mod contact_export {
    use super::*;
    use std::io::Write;
    /// External lab data and scheduler fixture: explicit invocation only, no GPU/window.
    #[test]
    #[ignore = "requires frozen PGS and feature witnesses; bounded CPU scheduler export"]
    fn actual_selected_contact_inputs() {
        use universe_sim::world::{World, worlds::{pgs::{Surface,Water},sha256}};
        let folder=std::path::PathBuf::from(std::env::var("UNIVERSE_CONTACT_EXPORT").expect("output folder"));
        std::fs::create_dir_all(&folder).unwrap();
        let bytes=std::fs::read(std::env::var("UNIVERSE_CONTACT_PGS").expect("frozen PGS")).unwrap();
        let hash=sha256(&bytes);
        assert_eq!(hash,"4b30e7eda8e707dff50498f0be80476bf82d5dfd4b8160c662b1a2113dadfe80");
        let source=Arc::new(Surface::read(&bytes).unwrap());drop(bytes);
        let world=World::with_surfaces(universe_sim::world::registry::registry().galaxy().unwrap().seed as u64,Arc::new([("body.treistun.treistun-e".to_string(),source.clone())].into_iter().collect()));
        let system=world.home_system;let sys=world.system(system);
        let body=sys.bodies.iter().position(|b|b.key=="body.treistun.treistun-e").unwrap();
        let radius=sys.bodies[body].rail.radius;assert_eq!(radius,6281370.);
        let fs:serde_json::Value=serde_json::from_slice(&std::fs::read(std::env::var("UNIVERSE_CONTACT_FEATURES").expect("frozen features")).unwrap()).unwrap();
        struct Limit<W>{writer:W,bytes:usize}
        impl<W:Write> Write for Limit<W>{
            fn write(&mut self,b:&[u8])->std::io::Result<usize>{if b.len()>128*1024*1024-self.bytes{return Err(std::io::Error::other("128MiB snapshot disk cap"));}let n=self.writer.write(b)?;self.bytes+=n;Ok(n)}
            fn flush(&mut self)->std::io::Result<()>{self.writer.flush()}
        }
        let mut results=Vec::new();
        for feature in fs["features"].as_array().unwrap().iter().filter(|f|["canonical_crest_1","canonical_crest_2","parent_face_join"].contains(&f["id"].as_str().unwrap())) {
            let id=feature["id"].as_str().unwrap();
            let direction=DVec3::from_array(serde_json::from_value(feature["queries"][0]["unit_direction"].clone()).unwrap());
            let sample=source.query(direction).unwrap();let visible=match sample.water{Water::Wet{level_m,..}=>level_m,_=>sample.height_m};
            let eye=direction*(radius+visible+1.7);
            // Real map identity, no GPU upload or material claim; selection geometry comes from source.
            let map=GlobeMap::new(1,vec![[0.;2];6]);
            let mut lod=Lod::default();let began=std::time::Instant::now();let mut exported_parent=false;let mut stable=0;
            let mut trace=std::io::BufWriter::new(std::fs::OpenOptions::new().write(true).create_new(true).open(folder.join(format!("{id}-states.jsonl"))).unwrap());
            let mut export=|stage:&str,snapshot:&contact::Snapshot,pending_count:usize|{
                let path=folder.join(format!("{id}-{stage}.json"));
                let input=Arc::new(snapshot.clone()).worker_input().unwrap();
                let chart=universe_terrain_contact::Chart::new(snapshot.eye.normalize(),snapshot.radius);
                let (converted,visited)=input.triangles(&chart).unwrap();
                assert!(!converted.is_empty());
                assert_eq!(visited,input.patches.len());
                assert_eq!(input.selected.len(),snapshot.patches.len());
                let file=std::fs::OpenOptions::new().write(true).create_new(true).open(&path).unwrap();
                let mut writer=Limit{writer:std::io::BufWriter::new(file),bytes:0};snapshot.write_json(&mut writer,&hash).unwrap();writer.flush().unwrap();
                results.push(serde_json::json!({"feature":id,"stage":stage,"path":path,"bytes":writer.bytes,"selected":snapshot.patches.len(),"generation":snapshot.generation,"pending":pending_count,"shared_worker_triangles":converted.len(),"borrowed_patches":visited,"gpu":false}));
            };
            let mut settled=false;
            for _ in 0..1500 {
                assert!(began.elapsed().as_secs()<150,"site scheduler timeout");
                let selected=lod.select_resident(system,&sys,body,map.id(),DVec3::ZERO,DQuat::IDENTITY,eye);
                if let Some(keys)=&selected {for key in keys {lod.patches.get_mut(key).unwrap().used=lod.frame;}}
                let snapshot=lod.contact_snapshot();
                let state=if selected.is_none(){"roots_pending"}else if lod.pending.is_empty(){"selected_settled"}else{"selected_loading"};
                let ids=snapshot.as_ref().map(|s|s.stamp.selected.iter().map(|(k,id)|([k.face as u32,k.level as u32,k.x,k.y],*id)).collect::<Vec<_>>()).unwrap_or_default();
                writeln!(trace,"{}",serde_json::json!({"frame":lod.frame,"simulation_time_s":0.,"state":state,"ready":false,"source_sha256":hash,"source_generation":1,"selection_generation":snapshot.as_ref().map(|s|s.generation),"map_id":map.id(),"eye":eye.to_array(),"radius_m":radius,"selected":ids,"resident":lod.patches.len(),"pending":lod.pending.len(),"workers":2,"subject":"camera","measurement":"cpu_actual_selected_geometry","active_group":null,"gpu_bytes":{"active":0,"uploading":0,"retired":0}})).unwrap();
                if let Some(s)=snapshot.as_ref().filter(|s|s.complete()) {
                    assert!(Arc::ptr_eq(s.surface.as_ref().unwrap(), &source));
                    let foot_level=s.patches.iter().filter(|p|p.key.shape().0.angle_between(direction)<p.key.shape().1*1.5).filter_map(|p|rendered_height(&Patch{mesh:p.mesh.clone(),origin:p.origin,partial:p.partial,used:0},eye,radius).map(|_|p.key.level)).max();
                    if !exported_parent && foot_level.is_some_and(|l|(13..19).contains(&l)) && !lod.pending.is_empty(){export("parent",s,lod.pending.len());exported_parent=true;}
                    if lod.pending.is_empty(){stable+=1;}else{stable=0;}
                    if stable>=3 {export("settled",s,lod.pending.len());settled=true;break;}
                }
                lod.evict_unused(lod.frame);
                std::thread::sleep(std::time::Duration::from_millis(10));
            }
            trace.flush().unwrap();assert!(settled,"no settled actual selection");
        }
        assert_eq!(results.iter().filter(|r|r["stage"]=="settled").count(),3);
        std::fs::write(folder.join("exports.json"),serde_json::to_vec_pretty(&results).unwrap()).unwrap();
    }
}
