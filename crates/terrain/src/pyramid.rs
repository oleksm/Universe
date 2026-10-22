//! Streamed vertex-aligned land pyramid. Immutable tiles; only selection and morph weights change per frame.
use super::*;
use std::collections::{HashMap, HashSet};
use std::sync::mpsc::{self, Receiver};

const PX: f64 = 0.0007;
/// UNIVERSE_TERRAIN_CELL_PX=n: a tile is refined only while its children's cells stay at least n pixels across
/// (the error it answers to capped at its children's cell over n). Cells finer than a pixel alias into sparkle as
/// the eye moves, but 1 also softens ridges seen from afar; off (0) by default.
fn min_cell_px() -> f64 {
    static V: std::sync::OnceLock<f64> = std::sync::OnceLock::new();
    *V.get_or_init(|| std::env::var("UNIVERSE_TERRAIN_CELL_PX").ok().and_then(|v| v.parse().ok()).unwrap_or(0.0))
}
const CACHE_BYTES: usize = 8 * 1024 * 1024 * 1024;
const UPLOAD_BYTES: usize = 32 * 1024 * 1024;
use universe_world::worlds::land::{tile_height, Meta, Raster, Tile, TileMesh};
struct Source {
    dir: PathBuf,
    m: Meta,
    index: HashMap<(usize, usize, usize), usize>,
    bounds: Vec<(DVec3, f64)>,
    holes: Vec<(f64, f64, f64)>,
}
impl Source {
    fn inverse(&self, x: f64, y: f64) -> DVec3 {
        self.m.projection.inverse(x, y)
    }
    fn project(&self, dir: DVec3) -> Option<(f64, f64)> {
        self.m.projection.project(dir)
    }
    fn xy(&self, t: Tile, i: f64, j: f64) -> (f64, f64) {
        let step = self.m.cell0_m * self.m.scale(t.k) as f64;
        (
            self.m.x0 + 0.5 * self.m.cell0_m + (t.col * self.m.tile) as f64 * step + i * step,
            self.m.y0 + 0.5 * self.m.cell0_m + (t.row * self.m.tile) as f64 * step + j * step,
        )
    }
    fn intersects_hole(&self, t: Tile) -> bool {
        let (x0, y0) = self.xy(t, 0.0, 0.0);
        let (x1, y1) = self.xy(t, self.m.tile as f64, self.m.tile as f64);
        self.hole_rect(x0, y0, x1, y1)
    }
    fn hole_rect(&self, x0: f64, y0: f64, x1: f64, y1: f64) -> bool {
        self.holes
            .iter()
            .any(|&(x, y, r)| (x - x.clamp(x0, x1)).hypot(y - y.clamp(y0, y1)) <= r)
    }
    fn children(&self, id: usize) -> Vec<usize> {
        let t = self.m.tiles[id];
        if t.k == 0 {
            return vec![];
        }
        (0..self.m.branch)
            .flat_map(|j| {
                (0..self.m.branch).filter_map(move |i| {
                    self.index
                        .get(&(
                            t.k - 1,
                            t.row * self.m.branch + j,
                            t.col * self.m.branch + i,
                        ))
                        .copied()
                })
            })
            .collect()
    }
    fn can_split(&self, id: usize) -> bool {
        let t = self.m.tiles[id];
        if t.k == 0 || self.children(id).is_empty() {
            return false;
        }
        let level = &self.m.levels[t.k - 1];
        let w = level.storage();
        let span = self.m.tile * self.m.branch;
        let (i0, j0) = (t.col * span, t.row * span);
        let (i1, j1) = ((i0 + span).min(level.n - 1), (j0 + span).min(level.n - 1));
        // A missing child outside a local window can still contain coarse ground.
        // Retain the complete parent at that boundary rather than punch out its remainder.
        i0 >= w.col0 && j0 >= w.row0 && i1 < w.col0 + w.cols && j1 < w.row0 + w.rows
    }
    fn parent(&self, id: usize) -> Option<usize> {
        let t = self.m.tiles[id];
        self.index
            .get(&(t.k + 1, t.row / self.m.branch, t.col / self.m.branch))
            .copied()
    }
    fn bounds(&self, t: Tile, ex: f64) -> (DVec3, f64) {
        let (x, y) = self.xy(t, self.m.tile as f64 * 0.5, self.m.tile as f64 * 0.5);
        let dir = self.inverse(x, y);
        let mid = (t.zmin + t.zmax) * 0.5 * ex;
        let centre = dir * (self.m.projection.radius_m + mid);
        // Exp-map is non-expanding within the supported hemisphere. Chord/sagitta and vertical relief
        // are both bounded by this enclosing sphere, including the clamped last row/column.
        let half = self.m.tile as f64
            * self.m.cell0_m
            * self.m.scale(t.k) as f64
            * std::f64::consts::FRAC_1_SQRT_2;
        let radius = half * (1.0 + mid.abs() / self.m.projection.radius_m)
            + (t.zmax - t.zmin).abs() * 0.5 * ex
            + 1.0;
        (centre, radius)
    }
    fn distance(&self, id: usize, eye: DVec3, ex: f64) -> f64 {
        let (c, r) = if ex == 1.0 {
            self.bounds[id]
        } else {
            self.bounds(self.m.tiles[id], ex)
        };
        ((eye - c).length() - r).max(1.0)
    }
    fn visible(&self, id: usize, cam: &Camera, aspect: f64, ex: f64) -> bool {
        let (c, r) = if ex == 1.0 {
            self.bounds[id]
        } else {
            self.bounds(self.m.tiles[id], ex)
        };
        let to = c - cam.position;
        let depth = to.dot(cam.forward().as_dvec3());
        let ty = (cam.fov_y as f64 * 0.5).tan();
        let tx = ty * aspect;
        if depth < -r
            || to.dot(cam.right().as_dvec3()).abs() > depth * tx + r * (1.0 + tx * tx).sqrt()
            || to.dot(cam.up().as_dvec3()).abs() > depth * ty + r * (1.0 + ty * ty).sqrt()
        {
            return false;
        }
        let planet = self.m.projection.radius_m;
        let occ = (planet - 12000.0 * ex).max(1.0);
        let dc = cam.position.length();
        let cap = (r / c.length()).min(1.0).asin();
        let h = (occ / dc).clamp(0.0, 1.0).acos()
            + (occ / (planet + self.m.tiles[id].zmax.max(0.0) * ex))
                .clamp(0.0, 1.0)
                .acos();
        c.normalize().dot(cam.position / dc).clamp(-1.0, 1.0).acos() <= h + cap
    }
    fn select(
        &self,
        cam: &Camera,
        aspect: f64,
        ex: f64,
        history: &mut HashSet<usize>,
    ) -> Selection {
        let mut out = Selection::default();
        fn walk(
            s: &Source,
            id: usize,
            cam: &Camera,
            aspect: f64,
            ex: f64,
            history: &mut HashSet<usize>,
            out: &mut Selection,
        ) {
            if !s.visible(id, cam, aspect, ex) {
                return;
            }
            out.visited.push(id);
            let t = s.m.tiles[id];
            let distance = s.distance(id, cam.position, ex);
            let error = if s.intersects_hole(t) {
                s.refine_error(t, ex).max(s.m.cell0_m * s.m.scale(t.k) as f64)
            } else {
                s.refine_error(t, ex)
            };
            // Retain invisible tiles' state; changing view direction must not reset hysteresis.
            let ratio = error / distance / PX;
            let split = s.can_split(id)
                && if history.contains(&id) {
                    ratio >= 0.7
                } else {
                    ratio > 1.0
                };
            if split {
                history.insert(id);
            } else {
                history.remove(&id);
            }
            if split {
                out.split.insert(id);
                for c in s.children(id) {
                    walk(s, c, cam, aspect, ex, history, out);
                }
            } else {
                out.leaves.push(id);
            }
        }
        if let Some(&root) = self.index.get(&(self.m.top_level, 0, 0)) {
            walk(self, root, cam, aspect, ex, history, &mut out);
        }
        out
    }
    /// The error refinement answers to: the tile's own, no more than its children's cell at min_cell_px.
    fn refine_error(&self, t: Tile, ex: f64) -> f64 {
        let e = t.error_m * ex;
        if t.k == 0 || min_cell_px() <= 0.0 {
            return e;
        }
        e.min(self.m.cell0_m * self.m.scale(t.k - 1) as f64 / min_cell_px())
    }
    fn weight(&self, id: usize, eye: DVec3, ex: f64) -> f32 {
        let Some(parent) = self.parent(id) else {
            return 0.0;
        };
        // Hole boundaries deliberately stay fine; never morph back across their missing coarse quads.
        if self.intersects_hole(self.m.tiles[parent]) {
            return 0.0;
        }
        let p = self.m.tiles[parent];
        let smooth = |x: f64| {
            let t = x.clamp(0.0, 1.0);
            t * t * (3.0 - 2.0 * t)
        };
        let m =
            smooth((self.distance(parent, eye, ex) * PX / self.refine_error(p, ex).max(1e-9) - 0.5) / 0.5);
        let t = self.m.tiles[id];
        // Keep the parent unmorphed through both the split and hysteretic merge boundary.
        // Children then meet the same surface on either side of a hierarchy change.
        let own = if t.k == 0 || t.error_m == 0.0 {
            1.0
        } else {
            smooth((self.distance(id, eye, ex) * PX / self.refine_error(t, ex) - 1.0 / 0.7) / (0.5 / 0.7))
        };
        (m * own) as f32
    }
}
#[derive(Default)]
struct Selection {
    visited: Vec<usize>,
    leaves: Vec<usize>,
    split: HashSet<usize>,
}
fn raster(
    source: &Source,
    k: usize,
    i0: usize,
    j0: usize,
    width: usize,
    height: usize,
) -> Result<Raster, String> {
    universe_world::worlds::land::raster(&source.dir, &source.m, k, i0, j0, width, height, true)
}
struct Built {
    mesh: PooledMesh,
    origin: DVec3,
    ground: Raster,
    /// The tile's feature-aware mesh, if it has one (its vertices are the model's first).
    tri: Option<TileMesh>,
}
impl Built {
    fn bytes(&self) -> usize {
        self.mesh.bytes()
            + self.ground.z.len() * 4
            + self.ground.rgb.len() * 3
            + self.ground.mask.len()
            + self.tri.as_ref().map_or(0, |m| m.verts.len() * 4 + m.tris.len() * 10)
    }
}
fn build_model(
    source: &Source,
    id: usize,
    ex: f64,
    real: bool,
) -> Result<(PoolModel, DVec3, Raster, Option<TileMesh>), String> {
    let t = source.m.tiles[id];
    let mesh = TileMesh::read(&source.dir, &source.m, &t)?;
    let tile = source.m.tile;
    let side = tile + 1;
    let i0 = t.col * tile;
    let j0 = t.row * tile;
    // Central differences use the same global neighbours on both sides of a tile boundary.
    let hi0 = i0.saturating_sub(1);
    let hj0 = j0.saturating_sub(1);
    let n = source.m.levels[t.k].n;
    let data = raster(
        source,
        t.k,
        hi0,
        hj0,
        (i0 + side + 1).min(n) - hi0,
        (j0 + side + 1).min(n) - hj0,
    )?;
    let parent = if t.k < source.m.top_level {
        Some(raster(
            source,
            t.k + 1,
            i0 / source.m.branch,
            j0 / source.m.branch,
            tile / source.m.branch + 2,
            tile / source.m.branch + 2,
        )?)
    } else {
        None
    };
    let r = source.m.projection.radius_m;
    let point = |i: usize, j: usize, k: usize, z: f64| {
        let s = source.m.scale(k) as f64 * source.m.cell0_m;
        source.inverse(
            source.m.x0 + 0.5 * source.m.cell0_m + i as f64 * s,
            source.m.y0 + 0.5 * source.m.cell0_m + j as f64 * s,
        ) * (r + z * ex)
    };
    let (cx, cy) = source.xy(t, tile as f64 * 0.5, tile as f64 * 0.5);
    let origin = source.inverse(cx, cy) * r;
    let mut model = PoolModel {
        smooth: true,
        ..Default::default()
    };
    // The vertices: the tile's mesh's, else every sample of the tile (row by row).
    let verts: Vec<(usize, usize)> = match &mesh {
        Some(mesh) => mesh.verts.iter().map(|v| (v[0] as usize, v[1] as usize)).collect(),
        None => (0..side).flat_map(|j| (0..side).map(move |i| (i, j))).collect(),
    };
    for &(i, j) in &verts {
        {
            let gi = (i0 + i).min(data.n - 1);
            let gj = (j0 + j).min(data.n - 1);
            let at = data.at(gi, gj);
            let p = point(gi, gj, t.k, data.z[at] as f64);
            model.positions.push((p - origin).as_vec3());
            let (left, right) = (gi.saturating_sub(1), (gi + 1).min(data.n - 1));
            let (down, up) = (gj.saturating_sub(1), (gj + 1).min(data.n - 1));
            let dx =
                point(right, gj, t.k, data.z(right, gj)) - point(left, gj, t.k, data.z(left, gj));
            let dy = point(gi, up, t.k, data.z(gi, up)) - point(gi, down, t.k, data.z(gi, down));
            model
                .normals
                .push(dx.cross(dy).normalize_or(p.normalize()).as_vec3());
            model.colors.push(if real {
                let c = data.rgb[at];
                [c[0], c[1], c[2], 255]
            } else {
                [255; 4]
            });
            let target = if let Some(parent) = &parent {
                let pi = gi / source.m.branch;
                let pj = gj / source.m.branch;
                let u = (gi % source.m.branch) as f64 / source.m.branch as f64;
                let v = (gj % source.m.branch) as f64 / source.m.branch as f64;
                let p00 = point(pi, pj, t.k + 1, parent.z(pi, pj));
                let p10 = point(
                    (pi + 1).min(parent.n - 1),
                    pj,
                    t.k + 1,
                    parent.z(pi + 1, pj),
                );
                let p01 = point(
                    pi,
                    (pj + 1).min(parent.n - 1),
                    t.k + 1,
                    parent.z(pi, pj + 1),
                );
                let p11 = point(
                    (pi + 1).min(parent.n - 1),
                    (pj + 1).min(parent.n - 1),
                    t.k + 1,
                    parent.z(pi + 1, pj + 1),
                );
                p00 * ((1.0 - u) * (1.0 - v))
                    + p10 * (u * (1.0 - v))
                    + p01 * ((1.0 - u) * v)
                    + p11 * (u * v)
            } else {
                p
            };
            model.morph.push(Morph {
                offset: (target - p).as_vec3(),
                near: 0.0,
                far: 1.0,
            });
        }
    }
    let hole = |(x0, y0): (f64, f64), (x1, y1): (f64, f64)| t.k > 0 && source.hole_rect(x0, y0, x1, y1);
    if let Some(mesh) = &mesh {
        // (Stored only where all three corners are ground; a coarse level's guard against a hole smaller
        // than its cells, as the grid's.)
        let mut seen = HashSet::new();
        for tri in &mesh.tris {
            let p = tri.map(|k| mesh.verts[k as usize].map(|x| x as f64));
            let lo = (p.iter().map(|v| v[0]).fold(f64::MAX, f64::min), p.iter().map(|v| v[1]).fold(f64::MAX, f64::min));
            let hi = (p.iter().map(|v| v[0]).fold(f64::MIN, f64::max), p.iter().map(|v| v[1]).fold(f64::MIN, f64::max));
            if hole(source.xy(t, lo.0, lo.1), source.xy(t, hi.0, hi.1)) {
                continue;
            }
            let [a, b, c] = tri.map(|k| k as u32);
            model.faces.push([a, b, c]);
            for (u, v) in [(a, b), (b, c), (c, a)] {
                if seen.insert((u.min(v), u.max(v))) {
                    model.edges.push([u, v]);
                }
            }
        }
    } else {
        for j in 0..tile {
            for i in 0..tile {
                let a = (j * side + i) as u32;
                let b = a + 1;
                let c = a + side as u32;
                let d = c + 1;
                let masks = [
                    data.mask[data.at(i0 + i, j0 + j)],
                    data.mask[data.at(i0 + i + 1, j0 + j)],
                    data.mask[data.at(i0 + i, j0 + j + 1)],
                    data.mask[data.at(i0 + i + 1, j0 + j + 1)],
                ];
                // Guard a hole smaller than a coarse cell even if none of its coarse samples falls in it.
                if !masks.contains(&1)
                    || masks.contains(&2)
                    || hole(source.xy(t, i as f64, j as f64), source.xy(t, (i + 1) as f64, (j + 1) as f64))
                {
                    continue;
                }
                model.faces.extend([[a, b, d], [a, d, c]]);
                model.edges.extend([[a, b], [a, c]]);
            }
        }
    }
    // Skirts on every edge only one triangle has (the tile's sides, the ground's, a hole's): they also cover
    // unequal morphs on same-level neighbours and the step to another level.
    let parent_error = source
        .parent(id)
        .map_or(t.error_m, |p| source.m.tiles[p].error_m);
    let depth = 2.0 * parent_error.max(t.error_m).max(1.0) * ex;
    let mut used: HashMap<(u32, u32), u32> = HashMap::new();
    for f in &model.faces {
        for (u, v) in [(f[0], f[1]), (f[1], f[2]), (f[2], f[0])] {
            *used.entry((u.min(v), u.max(v))).or_default() += 1;
        }
    }
    let faces = model.faces.len();
    for n in 0..faces {
        let f = model.faces[n];
        for (u, v) in [(f[0], f[1]), (f[1], f[2]), (f[2], f[0])] {
            if used[&(u.min(v), u.max(v))] != 1 {
                continue;
            }
            let k = model.positions.len() as u32;
            for w in [u, v] {
                let p = model.positions[w as usize];
                let down = (origin + p.as_dvec3()).normalize() * depth;
                model.positions.push(p - down.as_vec3());
                model.colors.push(model.colors[w as usize]);
                model.normals.push(model.normals[w as usize]);
                model.morph.push(model.morph[w as usize]);
            }
            model.faces.extend([[u, k, k + 1], [u, k + 1, v]]);
        }
    }
    Ok((model, origin, data, mesh))
}
fn build(source: &Source, id: usize, ex: f64, real: bool) -> Result<Built, String> {
    let (model, origin, ground, tri) = build_model(source, id, ex, real)?;
    Ok(Built {
        mesh: PooledMesh::new(&model),
        origin,
        ground,
        tri,
    })
}

/// Tiles built at once, each by a loader of its own.
const LOADERS: usize = 4;
/// Tiles wanted where the eye will be this far ahead (s), at its speed and heading, after those wanted now.
const LOOKAHEAD_S: f64 = 2.0;
/// Kept by where tiles are from the eye, not only when the budget is full: a tile not used for PURGE_UNUSED
/// updates is let go once it is behind the eye (behind where it looks and where it heads) and past
/// PURGE_BEHIND times the distance it refines at, or anywhere past PURGE_FAR times it.
const PURGE_UNUSED: u64 = 60;
const PURGE_BEHIND: f64 = 2.0;
const PURGE_FAR: f64 = 6.0;

/// The loaders' work: what to build next, best first (rewritten each update), and what they're building.
struct Plan {
    generation: u64,
    ex: f64,
    real: bool,
    queue: std::collections::VecDeque<usize>,
    building: HashSet<usize>,
    stop: bool,
}
type Shared = Arc<(Mutex<Plan>, std::sync::Condvar)>;
struct ResultTile {
    generation: u64,
    id: usize,
    result: Result<Built, String>,
}
struct Cached {
    tile: Built,
    last: u64,
}
pub struct Pyramid {
    source: Arc<Source>,
    pub mask: Vec<u8>,
    mask_level: usize,
    cache: HashMap<usize, Cached>,
    failed: HashSet<usize>,
    plan: Shared,
    /// The eye's last place and when, and its velocity (m/s, smoothed): for what's ahead.
    last_eye: Option<(DVec3, std::time::Instant)>,
    velocity: DVec3,
    results: Receiver<ResultTile>,
    selection: Selection,
    split_history: HashSet<usize>,
    drawn: HashMap<usize, f32>,
    generation: u64,
    key: (u64, bool),
    tick: u64,
    releases: Vec<u64>,
    pub stats: PyramidStats,
}
#[derive(Default, Clone, Copy)]
pub struct PyramidStats {
    pub selected: usize,
    pub drawn: usize,
    pub triangles: usize,
    pub pending: usize,
    pub bytes: usize,
    pub cached: usize,
    pub fallback: usize,
    pub worst: f64,
    pub resident: usize,
    pub largest_tile: usize,
}
fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

impl Drop for Pyramid {
    fn drop(&mut self) {
        let (m, ready) = &*self.plan;
        lock(m).stop = true;
        ready.notify_all();
    }
}

impl Pyramid {
    pub fn open(view: &Path, radius: f64) -> Result<Option<Self>, String> {
        let dir = if view.join("pyramid_deep/pyramid.json").exists() {
            view.join("pyramid_deep")
        } else if view.join("pyramid4/pyramid.json").exists() {
            view.join("pyramid4")
        } else {
            view.join("pyramid")
        };
        let path = dir.join("pyramid.json");
        let text = match std::fs::read_to_string(&path) {
            Ok(t) => t,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(format!("{}: {e}", path.display())),
        };
        let m: Meta =
            serde_json::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))?;
        validate(&m, radius)?;
        for l in &m.levels {
            for (name, stride) in [(&l.z, 4u64), (&l.rgb, 3), (&l.mask, 1)] {
                let p = dir.join(name);
                let size = std::fs::metadata(&p)
                    .map_err(|e| format!("{}: {e}", p.display()))?
                    .len();
                let w = l.storage();
                if size != w.rows as u64 * w.cols as u64 * stride {
                    return Err(format!("{}: wrong raster length", p.display()));
                }
            }
        }
        let mut source = Source {
            dir,
            m,
            index: HashMap::new(),
            bounds: Vec::new(),
            holes: Vec::new(),
        };
        for (i, t) in source.m.tiles.iter().enumerate() {
            if source.index.insert((t.k, t.row, t.col), i).is_some() {
                return Err("duplicate pyramid tile".into());
            }
        }
        for t in &source.m.tiles {
            if t.k < source.m.top_level
                && !source.index.contains_key(&(
                    t.k + 1,
                    t.row / source.m.branch,
                    t.col / source.m.branch,
                ))
            {
                return Err("pyramid tile without parent".into());
            }
        }
        if !source.index.contains_key(&(source.m.top_level, 0, 0)) {
            return Err("pyramid root missing".into());
        }
        source.holes = source
            .m
            .holes
            .iter()
            .filter_map(|h| {
                source
                    .project(unit(h.lon, h.lat))
                    .map(|(x, y)| (x, y, h.radius_km * 1000.0))
            })
            .collect();
        source.bounds = source
            .m
            .tiles
            .iter()
            .map(|&t| source.bounds(t, 1.0))
            .collect();
        // Static-package coverage needs the whole continent, including outside local fine windows.
        let mask_level = source
            .m
            .levels
            .iter()
            .position(|l| {
                let w = l.storage();
                w.row0 == 0 && w.col0 == 0 && w.rows == l.n && w.cols == l.n
            })
            .ok_or("pyramid requires a full coverage level")?;
        let mask = std::fs::read(source.dir.join(&source.m.levels[mask_level].mask))
            .map_err(|e| e.to_string())?;
        if mask.iter().any(|&v| v > 2) {
            return Err("invalid coverage pyramid mask".into());
        }
        let root = source.index[&(source.m.top_level, 0, 0)];
        let root_tile = build(&source, root, 1.0, false)?;
        let source = Arc::new(source);
        let plan: Shared = Arc::new((Mutex::new(Plan { generation: 0, ex: 1.0, real: false, queue: Default::default(), building: HashSet::new(), stop: false }), std::sync::Condvar::new()));
        let (outbox, results) = mpsc::channel::<ResultTile>();
        for k in 0..LOADERS {
            let (source, plan, outbox) = (source.clone(), plan.clone(), outbox.clone());
            std::thread::Builder::new()
                .name(format!("planetlod pyramid {k}"))
                .spawn(move || loop {
                    // (The best next tile of the latest plan; built with the plan let go.)
                    let (id, generation, ex, real) = {
                        let (m, ready) = &*plan;
                        let mut p = m.lock().unwrap_or_else(|e| e.into_inner());
                        loop {
                            if p.stop {
                                return;
                            }
                            if let Some(id) = p.queue.pop_front() {
                                p.building.insert(id);
                                break (id, p.generation, p.ex, p.real);
                            }
                            p = ready.wait(p).unwrap_or_else(|e| e.into_inner());
                        }
                    };
                    let result = build(&source, id, ex, real);
                    if outbox.send(ResultTile { generation, id, result }).is_err() {
                        return;
                    }
                })
                .map_err(|e| e.to_string())?;
        }
        log::info!(
            "planet LOD pyramid: branch {}, {} tiles, {} levels at {}; root ready",
            source.m.branch,
            source.m.tiles.len(),
            source.m.levels.len(),
            source.dir.display()
        );
        Ok(Some(Self {
            source,
            mask,
            mask_level,
            cache: HashMap::from([(
                root,
                Cached {
                    tile: root_tile,
                    last: 0,
                },
            )]),
            failed: HashSet::new(),
            plan,
            last_eye: None,
            velocity: DVec3::ZERO,
            results,
            selection: Selection::default(),
            split_history: HashSet::new(),
            drawn: HashMap::new(),
            generation: 0,
            key: (1.0f64.to_bits(), false),
            tick: 0,
            releases: vec![],
            stats: PyramidStats::default(),
        }))
    }
    pub fn covered(&self, dir: DVec3) -> bool {
        let Some((x, y)) = self.source.project(dir) else {
            return false;
        };
        let m = &self.source.m;
        let n = m.levels[self.mask_level].n;
        let cell = m.cell0_m * m.scale(self.mask_level) as f64;
        let x0 = m.x0 + 0.5 * m.cell0_m - 0.5 * cell;
        let y0 = m.y0 + 0.5 * m.cell0_m - 0.5 * cell;
        let i = ((x - x0) / cell).floor() as i64;
        let j = ((y - y0) / cell).floor() as i64;
        i >= 0
            && j >= 0
            && i < n as i64
            && j < n as i64
            && self.mask[j as usize * n + i as usize] == 1
    }
    /// World height from the last submitted resident cover, including its actual morph weight.
    /// Prefetched fine tiles never substitute for a coarse fallback drawn on screen.
    pub fn drawn_ground_at_world(&self, dir: DVec3) -> Option<f64> {
        let (x, y) = self.source.project(dir)?;
        let m = &self.source.m;
        for k in 0..=m.top_level {
            let cell = m.cell0_m * m.scale(k) as f64;
            let (u, v) = (
                (x - m.x0 - 0.5 * m.cell0_m) / cell,
                (y - m.y0 - 0.5 * m.cell0_m) / cell,
            );
            if u < 0. || v < 0. || u >= m.levels[k].n as f64 || v >= m.levels[k].n as f64 {
                continue;
            }
            let (i, j) = (u.floor() as usize, v.floor() as usize);
            let id = match self.source.index.get(&(k, j / m.tile, i / m.tile)) {
                Some(&id) => id,
                None => continue,
            };
            let Some(&weight) = self.drawn.get(&id) else {
                continue;
            };
            let Some(c) = self.cache.get(&id) else {
                continue;
            };
            let t = m.tiles[id];
            let data = &c.tile.ground;
            if let Some(mesh) = &c.tile.tri {
                // (The triangles drawn near: the morph can shift a point a cell or so.)
                let (lu, lv) = (u - (t.col * m.tile) as f64, v - (t.row * m.tile) as f64);
                let mut near: Vec<u32> = [(-1.0, -1.0), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)]
                    .iter()
                    .flat_map(|(du, dv)| mesh.near(lu + du, lv + dv).iter().copied())
                    .collect();
                near.sort_unstable();
                near.dedup();
                for n in near {
                    let points = mesh.tris[n as usize].map(|vi| c.tile.origin + c.tile.mesh.morphed_position(vi as usize, weight).unwrap().as_dvec3());
                    if let Some(h) = super::flight::triangle_height(dir, points, m.projection.radius_m) {
                        return Some(h);
                    }
                }
                continue;
            }
            // The morph can shift the projected cell slightly; test its immediate neighbours too.
            for dj in -1isize..=1 {
                for di in -1isize..=1 {
                    let (li, lj) = (
                        i as isize - (t.col * m.tile) as isize + di,
                        j as isize - (t.row * m.tile) as isize + dj,
                    );
                    if li < 0 || lj < 0 || li >= m.tile as isize || lj >= m.tile as isize {
                        continue;
                    }
                    let (li, lj) = (li as usize, lj as usize);
                    let (gi, gj) = (t.col * m.tile + li, t.row * m.tile + lj);
                    let masks = [
                        data.mask[data.at(gi, gj)],
                        data.mask[data.at(gi + 1, gj)],
                        data.mask[data.at(gi, gj + 1)],
                        data.mask[data.at(gi + 1, gj + 1)],
                    ];
                    let (x0, y0) = self.source.xy(t, li as f64, lj as f64);
                    let (x1, y1) = self.source.xy(t, (li + 1) as f64, (lj + 1) as f64);
                    if !masks.contains(&1)
                        || masks.contains(&2)
                        || (k > 0 && self.source.hole_rect(x0, y0, x1, y1))
                    {
                        continue;
                    }
                    let a = lj * (m.tile + 1) + li;
                    for tri in [
                        [a, a + 1, a + m.tile + 2],
                        [a, a + m.tile + 2, a + m.tile + 1],
                    ] {
                        let points = tri.map(|vi| {
                            c.tile.origin
                                + c.tile.mesh.morphed_position(vi, weight).unwrap().as_dvec3()
                        });
                        if let Some(h) =
                            super::flight::triangle_height(dir, points, m.projection.radius_m)
                        {
                            return Some(h);
                        }
                    }
                }
            }
        }
        None
    }
    pub fn ground_at(&self, dir: DVec3) -> Option<f64> {
        let (x, y) = self.source.project(dir)?;
        let m = &self.source.m;
        for k in 0..=m.top_level {
            let cell = m.cell0_m * m.scale(k) as f64;
            let u = (x - m.x0 - 0.5 * m.cell0_m) / cell;
            let v = (y - m.y0 - 0.5 * m.cell0_m) / cell;
            let n = m.levels[k].n;
            if u < 0.0 || v < 0.0 || u >= n as f64 || v >= n as f64 {
                continue;
            }
            let i = u.floor() as usize;
            let j = v.floor() as usize;
            let Some(&id) = self.source.index.get(&(k, j / m.tile, i / m.tile)) else {
                continue;
            };
            let Some(c) = self.cache.get(&id) else {
                continue;
            };
            return tile_height(m, &m.tiles[id], &c.tile.ground, c.tile.tri.as_ref(), u, v);
        }
        None
    }
    /// Keep recently used tiles until the byte budget needs their space. Selected work is pinned.
    /// How readily tile `id` goes from the eye at `eye` looking along `look` and heading along `heading`
    /// (unit or zero): its distance over the distance it refines at, doubled when it is behind both.
    fn remoteness(&self, id: usize, eye: DVec3, look: DVec3, heading: DVec3, ex: f64) -> (f64, bool) {
        let s = &self.source;
        let far = s.distance(id, eye, ex) / (s.refine_error(s.m.tiles[id], ex) / PX).max(1.0);
        let to = (s.bounds[id].0 - eye).normalize_or_zero();
        let behind = to.dot(look) < 0.0 && to.dot(heading) <= 0.0;
        (if behind { far * 2.0 } else { far }, behind)
    }

    fn trim_cache(&mut self, protected: &HashSet<usize>, budget: usize, eye: DVec3, look: DVec3, heading: DVec3, ex: f64) -> usize {
        // By where tiles are, whatever the budget: unused a while and behind the eye past PURGE_BEHIND times
        // where they refine, or anywhere past PURGE_FAR times it.
        let tick = self.tick;
        let gone: Vec<usize> = self
            .cache
            .iter()
            .filter(|(id, c)| !protected.contains(id) && tick.saturating_sub(c.last) > PURGE_UNUSED)
            .map(|(&id, _)| id)
            .filter(|&id| {
                let (r, behind) = self.remoteness(id, eye, look, heading, ex);
                (behind && r > 2.0 * PURGE_BEHIND) || r > PURGE_FAR
            })
            .collect();
        for id in gone {
            let c = self.cache.remove(&id).unwrap();
            self.releases.push(c.tile.mesh.id());
        }
        let mut bytes: usize = self.cache.values().map(|c| c.tile.bytes()).sum();
        if bytes <= budget {
            return bytes;
        }
        // Over budget: the most remote first (behind and far before ahead and near), then the longest unused.
        let mut evict: Vec<_> = self
            .cache
            .iter()
            .filter(|(id, _)| !protected.contains(id))
            .map(|(&id, c)| (-self.remoteness(id, eye, look, heading, ex).0, c.last, id))
            .collect();
        evict.sort_unstable_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
        for (_, _, id) in evict {
            if bytes <= budget {
                break;
            }
            let c = self.cache.remove(&id).unwrap();
            bytes -= c.tile.bytes();
            self.releases.push(c.tile.mesh.id());
        }
        bytes
    }
    pub fn update(&mut self, cam: &Camera, aspect: f64, ex: f64, real: bool) {
        let _scope = universe_prof::scope("update/planetlod/pyramid");
        self.tick += 1;
        let key = (ex.to_bits(), real);
        if key != self.key {
            self.key = key;
            self.generation += 1;
            lock(&self.plan.0).queue.clear();
            self.failed.clear();
            self.drawn.clear();
            for (_, c) in self.cache.drain() {
                self.releases.push(c.tile.mesh.id());
            }
        }
        let arrived: Vec<ResultTile> = self.results.try_iter().collect();
        if !arrived.is_empty() {
            let mut p = lock(&self.plan.0);
            for r in &arrived {
                p.building.remove(&r.id);
            }
        }
        for result in arrived {
            if result.generation != self.generation {
                continue;
            }
            match result.result {
                Ok(tile) => {
                    if let Some(old) = self.cache.insert(
                        result.id,
                        Cached {
                            tile,
                            last: self.tick,
                        },
                    ) {
                        self.releases.push(old.tile.mesh.id());
                    }
                }
                Err(e) => {
                    log::error!("planet LOD pyramid tile {}: {e}", result.id);
                    self.failed.insert(result.id);
                }
            }
        }
        self.selection = self.source.select(cam, aspect, ex, &mut self.split_history);
        let underfoot = self
            .source
            .project(cam.position.normalize())
            .and_then(|(x, y)| {
                let m = &self.source.m;
                let i = ((x - m.x0 - 0.5 * m.cell0_m) / m.cell0_m).floor();
                let j = ((y - m.y0 - 0.5 * m.cell0_m) / m.cell0_m).floor();
                if i < 0.0 || j < 0.0 || i >= m.side as f64 || j >= m.side as f64 {
                    None
                } else {
                    self.source
                        .index
                        .get(&(0, j as usize / m.tile, i as usize / m.tile))
                        .copied()
                }
            });
        let mut want = self.selection.visited.clone();
        for &id in &self.selection.leaves {
            let t = self.source.m.tiles[id];
            if self.source.can_split(id)
                && t.error_m * ex / self.source.distance(id, cam.position, ex) > PX * 0.6
            {
                let children = self.source.children(id);
                // Limit speculative geometry so it cannot churn the cache around the selected cover.
                if want.len() + children.len() <= CACHE_BYTES / (2 * 1024 * 1024) {
                    want.extend(children);
                }
            }
        }
        want.sort_unstable_by_key(|&i| (std::cmp::Reverse(self.source.m.tiles[i].k), i));
        want.dedup();
        if let Some(id) = underfoot {
            want.retain(|&i| i != id);
            want.insert(0, id);
        }
        let mut protected: HashSet<_> = want.iter().copied().collect();
        if let Some(id) = underfoot {
            protected.insert(id);
        }
        for id in &protected {
            if let Some(c) = self.cache.get_mut(id) {
                c.last = self.tick;
            }
        }
        // Keep the coarse cover warm even while the camera looks away from this pyramid.
        protected.insert(self.source.index[&(self.source.m.top_level, 0, 0)]);
        // Where the eye will be LOOKAHEAD_S on (its speed and heading): what it would select there, after
        // what's wanted now (not protected: still let go under pressure).
        let now = std::time::Instant::now();
        if let Some((at, then)) = self.last_eye {
            let dt = (now - then).as_secs_f64();
            if dt > 0.0 {
                let k = (dt / 0.25).min(1.0);
                self.velocity = self.velocity * (1.0 - k) + (cam.position - at) / dt * k;
            }
        }
        self.last_eye = Some((cam.position, now));
        let ahead: Vec<usize> = if self.velocity.length() * LOOKAHEAD_S > 0.05 * (cam.position.length() - self.source.m.projection.radius_m).abs().max(100.0) {
            let mut seen = Camera { position: cam.position + self.velocity * LOOKAHEAD_S, ..*cam };
            // (Never under the ground the camera is over.)
            seen.position = seen.position.normalize() * seen.position.length().max(cam.position.length());
            let mut history = self.split_history.clone();
            self.source.select(&seen, aspect, ex, &mut history).visited
        } else {
            Vec::new()
        };
        let look = cam.forward().as_dvec3();
        let heading = self.velocity.normalize_or_zero();
        let bytes = self.trim_cache(&protected, CACHE_BYTES, cam.position, look, heading, ex);
        // The loaders' plan, rewritten each update: missing tiles underfoot first, then in view coarse before
        // fine and each level by where the eye looks and heads (nearest, toward the middle of the view and
        // the way it moves, first), then those ahead; what's no longer wanted is never started.
        let order = |id: usize| {
            let to = (self.source.bounds[id].0 - cam.position).normalize_or_zero();
            let toward = 0.5 + 0.5 * to.dot(look).max(to.dot(heading));
            self.source.distance(id, cam.position, ex) / toward.max(0.1)
        };
        let first = want.first().copied().filter(|&i| Some(i) == underfoot);
        let mut rest: Vec<usize> = want.into_iter().filter(|&i| Some(i) != first).collect();
        rest.sort_by(|&a, &b| self.source.m.tiles[b].k.cmp(&self.source.m.tiles[a].k).then(order(a).total_cmp(&order(b))));
        let mut queued = HashSet::new();
        let missing: Vec<usize> = first
            .into_iter()
            .chain(rest)
            .chain(ahead)
            .filter(|i| !self.cache.contains_key(i) && !self.failed.contains(i) && queued.insert(*i))
            .collect();
        let pending = {
            let (m, ready) = &*self.plan;
            let mut p = lock(m);
            (p.generation, p.ex, p.real) = (self.generation, ex, real);
            let building = p.building.clone();
            p.queue = missing.into_iter().filter(|i| !building.contains(i)).collect();
            if !p.queue.is_empty() {
                ready.notify_all();
            }
            p.queue.len() + p.building.len()
        };
        self.stats.resident = self
            .selection
            .visited
            .iter()
            .filter(|i| {
                self.cache
                    .get(i)
                    .is_some_and(|c| c.tile.mesh.gpu_resident())
            })
            .count();
        self.stats.largest_tile = self
            .cache
            .values()
            .map(|c| c.tile.mesh.bytes())
            .max()
            .unwrap_or(0);
        self.stats.selected = self.selection.leaves.len();
        self.stats.cached = self.cache.len();
        self.stats.bytes = bytes;
        self.stats.pending = pending;
        self.stats.worst = self
            .selection
            .leaves
            .iter()
            .map(|&id| {
                self.source.m.tiles[id].error_m * ex
                    / self.source.distance(id, cam.position, ex)
                    / PX
            })
            .fold(0.0, f64::max);
    }
    pub fn draw(&mut self, frame: &mut Frame, eye: DVec3, body: &Transform, ex: f64, fill: Color, line: Color, edges: bool) {
        let _scope = universe_prof::scope("draw/planetlod/pyramid");
        for id in self.releases.drain(..) {
            frame.release_pooled(id);
        }
        let mut budget = UPLOAD_BYTES;
        // Coarse ancestors first, so incomplete refinement always has a resident cover.
        let mut ids = self.selection.visited.clone();
        ids.sort_unstable_by_key(|&i| std::cmp::Reverse(self.source.m.tiles[i].k));
        for id in ids {
            if let Some(c) = self.cache.get(&id) {
                frame.retain_pooled(c.tile.mesh.id());
                if !c.tile.mesh.gpu_resident()
                    && (c.tile.mesh.bytes() <= budget || budget == UPLOAD_BYTES)
                {
                    frame.prepare_pooled(&c.tile.mesh);
                    budget = budget.saturating_sub(c.tile.mesh.bytes());
                }
            }
        }
        let visible: HashSet<_> = self.selection.visited.iter().copied().collect();
        let root = self.source.index[&(self.source.m.top_level, 0, 0)];
        let mut drawn = Vec::new();
        if !ready_cover(
            &self.source,
            &self.selection,
            root,
            &visible,
            &|id| {
                self.cache
                    .get(&id)
                    .is_some_and(|c| c.tile.mesh.gpu_resident())
            },
            &mut drawn,
        ) && visible.contains(&root)
            && self.cache.contains_key(&root)
        {
            drawn.push(root);
        }
        let group = frame.pool_group(
            &placed(body, DVec3::ZERO),
            line,
            fill,
            if edges { 1.0 } else { 0.0 },
            true,
        );
        self.drawn.clear();
        self.stats.drawn = drawn.len();
        self.stats.triangles = 0;
        self.stats.fallback = 0;
        for id in drawn {
            let tile = &self.cache[&id].tile;
            let fallback = self.selection.split.contains(&id);
            let weight = if fallback {
                self.stats.fallback += 1;
                0.0
            } else {
                self.source.weight(id, eye, ex)
            };
            self.drawn.insert(id, weight);
            frame.pooled_morphed(group, &tile.mesh, placed(body, tile.origin).position, weight);
            self.stats.triangles += tile.mesh.faces() as usize;
        }
    }
}
// Publish a complete resident subtree, or one parent; never publish a partial replacement.
fn ready_cover(
    source: &Source,
    selection: &Selection,
    id: usize,
    visible: &HashSet<usize>,
    ready: &impl Fn(usize) -> bool,
    out: &mut Vec<usize>,
) -> bool {
    if !visible.contains(&id) {
        return true;
    }
    let start = out.len();
    if selection.split.contains(&id) {
        if source
            .children(id)
            .into_iter()
            .all(|c| ready_cover(source, selection, c, visible, ready, out))
        {
            return true;
        }
        out.truncate(start);
    }
    if ready(id) {
        out.push(id);
        true
    } else {
        false
    }
}

fn validate(m: &Meta, radius: f64) -> Result<(), String> {
    let p = &m.projection;
    let err = || "invalid or unsupported pyramid metadata".to_string();
    if m.format != "planet-forge land pyramid 1"
        || p.kind != "azimuthal equidistant"
        || !radius.is_finite()
        || radius <= 0.0
        || (p.radius_m - radius).abs() > 1.0
        || !p.radius_m.is_finite()
        || !m.cell0_m.is_finite()
        || m.cell0_m <= 0.0
        || !m.x0.is_finite()
        || !m.y0.is_finite()
        || m.top_level > 16
        || m.tile < 2
        || m.tile > 256
        || !matches!(m.branch, 2 | 4)
        || m.tile % m.branch != 0
        || m.side
            != m.branch
                .checked_pow(m.top_level as u32)
                .and_then(|n| n.checked_mul(m.tile))
                .ok_or_else(err)?
        || m.levels.len() != m.top_level + 1
    {
        return Err(err());
    }
    let [c, e, n] = [p.centre, p.east, p.north].map(DVec3::from_array);
    if [c, e, n]
        .iter()
        .any(|v| !v.is_finite() || (v.length() - 1.0).abs() > 1e-5)
        || c.dot(e).abs() > 1e-5
        || c.dot(n).abs() > 1e-5
        || e.dot(n).abs() > 1e-5
        || e.cross(n).dot(c) < 0.999
    {
        return Err(err());
    }
    for (k, l) in m.levels.iter().enumerate() {
        if l.k != k
            || l.n != m.side / m.scale(k)
            || !l.cell_m.is_finite()
            || (l.cell_m - m.cell0_m * m.scale(k) as f64).abs() > 1e-5
        {
            return Err(err());
        }
        let w = l.storage();
        if w.rows == 0
            || w.cols == 0
            || w.row0.checked_add(w.rows).is_none_or(|end| end > l.n)
            || w.col0.checked_add(w.cols).is_none_or(|end| end > l.n)
        {
            return Err(err());
        }
        for name in [&l.z, &l.rgb, &l.mask] {
            if Path::new(name).components().count() != 1
                || !matches!(
                    Path::new(name).components().next(),
                    Some(std::path::Component::Normal(_))
                )
            {
                return Err(err());
            }
        }
    }
    for t in &m.tiles {
        if t.k > m.top_level
            || t.row >= m.scale(m.top_level - t.k)
            || t.col >= m.scale(m.top_level - t.k)
            || !t.error_m.is_finite()
            || t.error_m < 0.0
            || !t.zmin.is_finite()
            || !t.zmax.is_finite()
            || t.zmax < t.zmin
        {
            return Err(err());
        }
    }
    for h in &m.holes {
        if !h.lat.is_finite()
            || h.lat.abs() > 90.0
            || !h.lon.is_finite()
            || !h.radius_km.is_finite()
            || h.radius_km <= 0.0
        {
            return Err(err());
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::flight::pan_longitude;
    use universe_engine::glam::Quat;
    struct Fixture(PathBuf);
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    fn fixture() -> Fixture {
        fixture_branch(2, 2)
    }
    fn fixture_branch(branch: usize, tile: usize) -> Fixture {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let dir = std::env::temp_dir().join(format!(
            "universe-pyramid-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        let pyramid = dir.join("pyramid");
        std::fs::create_dir_all(&pyramid).unwrap();
        let mut levels = Vec::new();
        let mut tiles = Vec::new();
        for k in 0..2 {
            let n = tile * branch / branch.pow(k as u32);
            let z: Vec<u8> = (0..n * n)
                .flat_map(|i| ((i * 3 + k * 7) as f32).to_le_bytes())
                .collect();
            let rgb = vec![128u8; n * n * 3];
            let mask = vec![1u8; n * n];
            for (name, bytes) in [
                (format!("z_{k}.f32"), z),
                (format!("rgb_{k}.u8"), rgb),
                (format!("mask_{k}.u8"), mask),
            ] {
                std::fs::write(pyramid.join(name), bytes).unwrap();
            }
            levels.push(serde_json::json!({"k":k,"n":n,"cell_m":10*branch.pow(k as u32),"z":format!("z_{k}.f32"),"rgb":format!("rgb_{k}.u8"),"mask":format!("mask_{k}.u8")}));
            for row in 0..branch.pow(1 - k as u32) {
                for col in 0..branch.pow(1 - k as u32) {
                    tiles.push(serde_json::json!({"k":k,"row":row,"col":col,"error_m":if k==0 {0} else {100},"zmin":0,"zmax":100}));
                }
            }
        }
        let meta = serde_json::json!({"format":"planet-forge land pyramid 1","projection":{"kind":"azimuthal equidistant","radius_m":1000,"centre":[0,0,1],"east":[1,0,0],"north":[0,1,0]},"x0":-20,"y0":-20,"cell0_m":10,"branch":branch,"side":tile*branch,"tile":tile,"top_level":1,"levels":levels,"tiles":tiles,"holes":[]});
        let mut meta = meta;
        if branch == 2 {
            meta.as_object_mut().unwrap().remove("branch");
        }
        std::fs::write(pyramid.join("pyramid.json"), meta.to_string()).unwrap();
        Fixture(dir)
    }
    #[test]
    fn pyramid_flight_samples_submitted_cover_and_morph_not_prefetched_detail() {
        let f = fixture();
        let mut p = Pyramid::open(&f.0, 1000.).unwrap().unwrap();
        let root = p.source.index[&(1, 0, 0)];
        let fine = p.source.index[&(0, 0, 0)];
        p.cache.insert(
            fine,
            Cached {
                tile: build(&p.source, fine, 1., true).unwrap(),
                last: 0,
            },
        );
        let dir = p.source.inverse(-5., -5.);
        assert!(p.drawn_ground_at_world(dir).is_none());
        p.drawn.insert(root, 0.);
        let coarse = p.drawn_ground_at_world(dir).unwrap();
        p.drawn.clear();
        p.drawn.insert(fine, 0.);
        let detailed = p.drawn_ground_at_world(dir).unwrap();
        assert!((coarse - detailed).abs() > 1.);
        p.drawn.insert(fine, 1.);
        let morphed = p.drawn_ground_at_world(dir).unwrap();
        assert!(
            (morphed - coarse).abs() < 0.02,
            "fully morphed fine surface meets the parent"
        );
    }
    /// A tile with a feature-aware mesh is drawn and touched on its triangles (skirts on its outer edges);
    /// its neighbours without one keep the grid. (Heights here are a plane: the mesh and the grid agree.)
    #[test]
    fn pyramid_tile_mesh_drawn_and_touched_on_its_triangles() {
        let f = fixture_branch(2, 4);
        let path = f.0.join("pyramid");
        // Tile (0, 0, 0): its four corners and its middle, four triangles round the middle.
        let words: [u16; 22] = [0, 0, 0, 4, 4, 0, 4, 4, 2, 2, 0, 1, 4, 1, 3, 4, 3, 2, 4, 2, 0, 4];
        std::fs::write(path.join("mesh_0.u16"), words.iter().flat_map(|w| w.to_le_bytes()).collect::<Vec<u8>>()).unwrap();
        let mut meta: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(path.join("pyramid.json")).unwrap()).unwrap();
        meta["levels"][0]["mesh"] = "mesh_0.u16".into();
        let t0 = meta["tiles"].as_array_mut().unwrap().iter_mut().find(|t| t["k"] == 0 && t["row"] == 0 && t["col"] == 0).unwrap();
        (t0["mesh_off"], t0["mesh_nv"], t0["mesh_nt"]) = (0.into(), 5.into(), 4.into());
        std::fs::write(path.join("pyramid.json"), meta.to_string()).unwrap();
        let mut p = Pyramid::open(&f.0, 1000.).unwrap().unwrap();
        let (meshed, plain) = (p.source.index[&(0, 0, 0)], p.source.index[&(0, 0, 1)]);
        let (model, ..) = build_model(&p.source, meshed, 1., true).unwrap();
        assert_eq!(model.faces.len(), 4 + 4 * 2, "its triangles and a skirt under each outer edge");
        assert_eq!(model.positions.len(), 5 + 4 * 2);
        let (grid, ..) = build_model(&p.source, plain, 1., true).unwrap();
        assert_eq!(grid.faces.len(), 4 * 4 * 2 + 4 * 4 * 2, "the grid's cells, and skirts round it");
        for id in [meshed, plain] {
            p.cache.insert(id, Cached { tile: build(&p.source, id, 1., true).unwrap(), last: 0 });
            p.drawn.insert(id, 0.);
        }
        // Inside the meshed tile, off its vertices: the plane's height, the same drawn and touched.
        let dir = p.source.inverse(-20. + 0.5 * 10. + 1.3 * 10., -20. + 0.5 * 10. + 2.6 * 10.);
        let (touched, drawn) = (p.ground_at(dir).unwrap(), p.drawn_ground_at_world(dir).unwrap());
        let plane = (2.6 * 8. + 1.3) * 3.;
        assert!((touched - plane).abs() < 1e-6, "{touched} vs {plane}");
        // (Drawn, a flat triangle's chord under this 1 km sphere: a 40 m tile's sags 40² / 8 R = 0.2 m, its
        // corners to its middle more; on a world's radius, millimetres.)
        assert!(drawn < plane && plane - drawn < 1.0, "{drawn} vs {plane}");
    }
    #[test]
    fn pyramid_windowed_rows_mask_and_boundary_cover() {
        let f = fixture_branch(4, 12);
        let dir = f.0.join("pyramid");
        let path = dir.join("pyramid.json");
        let mut m: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        let (row0, col0, rows, cols) = (7, 5, 34, 38);
        m["levels"][0]["window"] =
            serde_json::json!({"row0":row0,"col0":col0,"rows":rows,"cols":cols});
        for (name, stride) in [("z_0.f32", 4), ("rgb_0.u8", 3), ("mask_0.u8", 1)] {
            let bytes = std::fs::read(dir.join(name)).unwrap();
            let cropped: Vec<u8> = (row0..row0 + rows)
                .flat_map(|r| {
                    bytes[(r * 48 + col0) * stride..(r * 48 + col0 + cols) * stride]
                        .iter()
                        .copied()
                })
                .collect();
            std::fs::write(dir.join(name), cropped).unwrap();
        }
        std::fs::write(&path, m.to_string()).unwrap();
        let p = Pyramid::open(&f.0, 1000.).unwrap().unwrap();
        let s = &p.source;
        assert_eq!(p.mask_level, 1);
        assert_eq!(p.mask.len(), 12 * 12); // Never allocate the implied full fine raster.
        let data = raster(s, 0, 3, 5, 8, 8).unwrap();
        assert_eq!(data.z(5, 7), (7 * 48 + 5) as f64 * 3.);
        assert_eq!(data.mask[data.at(4, 7)], 0);
        assert_eq!(data.mask[data.at(5, 6)], 0);
        assert_eq!(data.mask[data.at(5, 7)], 1);
        let root = s.index[&(1, 0, 0)];
        assert!(
            !s.can_split(root),
            "partial fine window retains coarse ground outside it"
        );
        let (tile, _, _, _) = build_model(s, s.index[&(0, 0, 0)], 1., true).unwrap();
        assert!(!tile.faces.is_empty());
        assert!(tile.normals.iter().all(|n| n.is_finite()));
        assert!(
            p.covered(s.inverse(0., 0.)),
            "static package uses whole-continent coverage"
        );
        let mut invalid: Meta = serde_json::from_value(m.clone()).unwrap();
        invalid.levels[0].window.as_mut().unwrap().rows = 0;
        assert!(validate(&invalid, 1000.).is_err());
        invalid.levels[0].window.as_mut().unwrap().rows = rows;
        invalid.levels[0].window.as_mut().unwrap().row0 = usize::MAX;
        assert!(validate(&invalid, 1000.).is_err());
        // A listed parent with no listed children is terminal even with nonzero error.
        m["tiles"].as_array_mut().unwrap().retain(|t| t["k"] == 1);
        // Read metadata directly: the terminal decision does not depend on has_fine hints.
        let terminal: Meta = serde_json::from_value(m).unwrap();
        let source = Source {
            dir,
            m: terminal,
            index: HashMap::from([((1, 0, 0), 0)]),
            bounds: vec![],
            holes: vec![],
        };
        assert!(!source.can_split(0));
    }
    #[test]
    fn pyramid_branch_four_bilinear_morph_and_hysteresis() {
        let f = fixture_branch(4, 12); // Real data likewise has a non-power-of-two tile size.
        std::fs::rename(f.0.join("pyramid"), f.0.join("pyramid4")).unwrap();
        let p = Pyramid::open(&f.0, 1000.).unwrap().unwrap();
        let s = &p.source;
        let root = s.index[&(1, 0, 0)];
        let fine = s.index[&(0, 0, 0)];
        assert_eq!(s.children(root).len(), 16);
        for id in s.children(root) {
            assert_eq!(s.parent(id), Some(root));
        }
        let (model, origin, _, _) = build_model(s, fine, 1., true).unwrap();
        let (parent, po, _, _) = build_model(s, root, 1., true).unwrap();
        // A quarter across and three quarters up: all four corners contribute.
        let (u, v) = (0.25, 0.75);
        let target = po
            + (parent.positions[0] * ((1. - u) * (1. - v))
                + parent.positions[1] * (u * (1. - v))
                + parent.positions[13] * ((1. - u) * v)
                + parent.positions[14] * (u * v))
                .as_dvec3();
        let index = 3 * 13 + 1;
        let actual = origin + (model.positions[index] + model.morph[index].offset).as_dvec3();
        assert!(actual.distance(target) < 1e-3);
        let (centre, radius) = s.bounds[root];
        let direction = centre.normalize();
        let camera = |ratio: f64| {
            let mut cam = Camera {
                position: centre + direction * (radius + 100. / (PX * ratio)),
                ..Default::default()
            };
            cam.look_at(centre, Vec3::Y);
            cam
        };
        let mut history = HashSet::new();
        for (ratio, split) in [
            (0.99, false),
            (1.01, true),
            (0.99, true),
            (0.71, true),
            (0.69, false),
            (0.99, false),
        ] {
            let selection = s.select(&camera(ratio), 1., 1., &mut history);
            assert_eq!(selection.split.contains(&root), split, "ratio={ratio}");
        }
        // The last half of the geometric range morphs smoothly; the hysteresis tail is fully coarse.
        for (distance_ratio, weight) in [(0.5, 0.), (0.75, 0.5), (1., 1.), (1. / 0.7, 1.)] {
            let cam = camera(1. / distance_ratio);
            assert!((s.weight(fine, cam.position, 1.) - weight).abs() < 1e-5);
        }
        let mut invalid: Meta =
            serde_json::from_slice(&std::fs::read(f.0.join("pyramid4/pyramid.json")).unwrap())
                .unwrap();
        invalid.branch = 0;
        assert!(validate(&invalid, 1000.).is_err());
        invalid.branch = 4;
        invalid.tile = 10;
        assert!(validate(&invalid, 1000.).is_err());
    }
    #[test]
    fn pyramid_projection_masks_shared_edges_and_exact_parent_targets() {
        let f = fixture();
        let p = Pyramid::open(&f.0, 1000.0).unwrap().unwrap();
        let s = &p.source;
        let (x, y) = (4.0, -13.0);
        let q = s.project(s.inverse(x, y)).unwrap();
        assert!((q.0 - x).abs() < 1e-8 && (q.1 - y).abs() < 1e-8);
        assert!(p.covered(s.inverse(0.0, 0.0)));
        assert!(!p.covered(s.inverse(100.0, 0.0)));
        let a = s.index[&(0, 0, 0)];
        let b = s.index[&(0, 0, 1)];
        let root = s.index[&(1, 0, 0)];
        let (fine, origin, _, _) = build_model(s, a, 1.0, true).unwrap();
        let (neighbour, no, _, _) = build_model(s, b, 1.0, true).unwrap();
        let (coarse, co, _, _) = build_model(s, root, 1.0, true).unwrap();
        for j in 0..3 {
            let a = origin + fine.positions[j * 3 + 2].as_dvec3();
            let b = no + neighbour.positions[j * 3].as_dvec3();
            assert!(a.distance(b) < 1e-4);
        }
        // Fine corner and centre land on the bilinear parent, including filtered lattice heights.
        for (fi, target) in [
            (0, coarse.positions[0]),
            (
                4,
                (coarse.positions[0]
                    + coarse.positions[1]
                    + coarse.positions[3]
                    + coarse.positions[4])
                    * 0.25,
            ),
        ] {
            let parent = co + target.as_dvec3();
            let morphed = origin + (fine.positions[fi] + fine.morph[fi].offset).as_dvec3();
            assert!(parent.distance(morphed) < 1e-4);
        }
        assert_eq!(fine.positions.len(), fine.morph.len());
        assert!(fine.faces.len() > 8, "boundary skirts added");
        // A hole at a corner rejects that quad even when other corners are ground.
        let mut mask = vec![1u8; 16];
        mask[0] = 2;
        std::fs::write(f.0.join("pyramid/mask_0.u8"), mask).unwrap();
        let (hole, _, _, _) = build_model(s, a, 1.0, true).unwrap();
        assert!(!hole.faces.contains(&[0, 1, 4]));
    }
    #[test]
    fn pyramid_smooth_grid_normals_and_vertex_colours_match_shared_edges() {
        let f = fixture();
        let height = |i: usize, j: usize| (i * i * 3 + j * j * 7) as f32;
        let z: Vec<u8> = (0..16)
            .flat_map(|v| height(v % 4, v / 4).to_le_bytes())
            .collect();
        let rgb: Vec<u8> = (0..16)
            .flat_map(|v| [(v % 4 * 31) as u8, (v / 4 * 41) as u8, 100])
            .collect();
        std::fs::write(f.0.join("pyramid/z_0.f32"), z).unwrap();
        std::fs::write(f.0.join("pyramid/rgb_0.u8"), rgb).unwrap();
        let p = Pyramid::open(&f.0, 1000.).unwrap().unwrap();
        let s = &p.source;
        let (a, _, _, _) = build_model(s, s.index[&(0, 0, 0)], 1.0, true).unwrap();
        let (b, _, _, _) = build_model(s, s.index[&(0, 0, 1)], 1.0, true).unwrap();
        assert!(a.smooth && b.smooth);
        assert!(a.face_colors.is_empty() && b.face_colors.is_empty());
        assert_eq!(a.normals.len(), a.positions.len());
        assert!(
            a.normals
                .iter()
                .all(|n| n.is_finite() && (n.length() - 1.0).abs() < 1e-6)
        );
        assert_ne!(a.colors[0], a.colors[1]);
        for j in 0..3 {
            assert_eq!(a.normals[j * 3 + 2], b.normals[j * 3]);
            assert_eq!(a.colors[j * 3 + 2], b.colors[j * 3]);
        }
        let point = |i: usize, j: usize| {
            s.inverse(-15. + i as f64 * 10., -15. + j as f64 * 10.) * (1000. + height(i, j) as f64)
        };
        let expected = (point(3, 1) - point(1, 1))
            .cross(point(2, 2) - point(2, 0))
            .normalize()
            .as_vec3();
        assert!((a.normals[5] - expected).length() < 1e-6);
    }
    #[test]
    fn pyramid_selection_refines_and_rejects_bad_metadata() {
        let f = fixture();
        let p = Pyramid::open(&f.0, 1000.0).unwrap().unwrap();
        let mut cam = Camera {
            position: DVec3::new(0., 0., 1e6),
            ..Default::default()
        };
        cam.look_at(DVec3::ZERO, Vec3::Y);
        let far = p.source.select(&cam, 1.0, 1.0, &mut HashSet::new());
        assert_eq!(far.leaves.len(), 1);
        cam.position.z = 1100.;
        let close = p.source.select(&cam, 1.0, 1.0, &mut HashSet::new());
        assert!(close.leaves.len() > 1);
        for &id in &close.leaves {
            assert_eq!(p.source.m.tiles[id].k, 0);
            assert_eq!(p.source.weight(id, cam.position, 1.0), 0.0);
        }
        let path = f.0.join("pyramid/pyramid.json");
        let mut m: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        m["levels"][0]["z"] = "../escape".into();
        std::fs::write(path, m.to_string()).unwrap();
        assert!(Pyramid::open(&f.0, 1000.).is_err());
    }
    #[test]
    fn pyramid_refinement_publishes_whole_subtrees() {
        for branch in [2, 4] {
            let f = fixture_branch(branch, branch);
            let p = Pyramid::open(&f.0, 1000.).unwrap().unwrap();
            let s = &p.source;
            let root = s.index[&(1, 0, 0)];
            let children = s.children(root);
            let mut selection = Selection::default();
            selection.split.insert(root);
            let visible: HashSet<_> = std::iter::once(root)
                .chain(children.iter().copied())
                .collect();
            let mut out = vec![];
            assert!(ready_cover(
                s,
                &selection,
                root,
                &visible,
                &|id| id != children[3],
                &mut out
            ));
            assert_eq!(out, vec![root], "one missing child retains only the parent");
            out.clear();
            assert!(ready_cover(
                s,
                &selection,
                root,
                &visible,
                &|_| true,
                &mut out
            ));
            assert_eq!(out, children);
            out.clear();
            assert!(!ready_cover(
                s,
                &selection,
                root,
                &visible,
                &|_| false,
                &mut out
            ));
            assert!(out.is_empty());
        }
    }
    #[test]
    fn pyramid_cache_keeps_tiles_by_place_not_only_by_budget() {
        let f = fixture();
        let mut p = Pyramid::open(&f.0, 1000.).unwrap().unwrap();
        let root = p.source.index[&(1, 0, 0)];
        let a = p.source.index[&(0, 0, 0)];
        let b = p.source.index[&(0, 1, 1)];
        for (id, last) in [(a, 10), (b, 20)] {
            p.cache.insert(id, Cached { tile: build(&p.source, id, 1., false).unwrap(), last });
        }
        p.tick = 10000;
        let full: usize = p.cache.values().map(|c| c.tile.bytes()).sum();
        let protected = HashSet::from([root]);
        let (ca, cb) = (p.source.bounds[a].0, p.source.bounds[b].0);
        let mid = (ca + cb) * 0.5;
        let eye = mid.normalize() * (mid.length() + 1.0);
        // Near and in view: cold tiles stay (age alone expires nothing).
        assert_eq!(p.trim_cache(&protected, full, eye, (mid - eye).normalize(), DVec3::ZERO, 1.), full);
        assert_eq!(p.cache.len(), 3);
        assert!(p.releases.is_empty());
        // Under pressure the one behind where the eye looks goes first, though it was used last.
        p.cache.get_mut(&b).unwrap().last = p.tick;
        let b_bytes = p.cache[&b].tile.bytes();
        let b_mesh = p.cache[&b].tile.mesh.id();
        assert_eq!(p.trim_cache(&protected, full - b_bytes, eye, (ca - eye).normalize(), DVec3::ZERO, 1.), full - b_bytes);
        assert!(p.cache.contains_key(&a) && p.cache.contains_key(&root) && !p.cache.contains_key(&b));
        assert_eq!(p.releases, vec![b_mesh]);
        // Left far behind: gone whatever the budget; the root stays.
        let far = mid.normalize() * 1e9;
        p.trim_cache(&protected, usize::MAX, far, -mid.normalize(), DVec3::ZERO, 1.);
        assert!(!p.cache.contains_key(&a) && p.cache.contains_key(&root));
    }
    #[test]
    fn pyramid_looking_away_preserves_budgeted_cache_and_root() {
        let f = fixture();
        let mut p = Pyramid::open(&f.0, 1000.).unwrap().unwrap();
        let root = p.source.index[&(1, 0, 0)];
        let id = p.source.index[&(0, 1, 1)];
        let tile = build(&p.source, id, 1., false).unwrap();
        p.cache.insert(id, Cached { tile, last: 0 });
        p.tick = 10000;
        // (Turning the head close by drops nothing; tiles far behind do go: see the test above.)
        let cam = Camera {
            position: DVec3::new(0., 0., 1100.),
            orientation: Quat::from_rotation_y(std::f32::consts::PI),
            ..Default::default()
        };
        p.update(&cam, 1., 1., false);
        assert!(p.cache.contains_key(&id) && p.cache.contains_key(&root));
        assert!(p.releases.is_empty());
        assert!(p.stats.bytes <= CACHE_BYTES);
    }
    #[test]
    fn pyramid_diagnostic_pan_stays_local_at_every_frame_rate() {
        for fps in [30, 60, 120, 240] {
            let mut lon = 108.;
            for frame in 0..fps * 45 {
                lon = pan_longitude(108., frame as f64 / fps as f64);
                assert!((lon - 108.).abs() <= 0.25);
            }
            assert!((lon - pan_longitude(108., 45. - 1. / fps as f64)).abs() < 1e-12);
        }
    }
    #[test]
    #[ignore = "requires full land pyramid fixture"]
    fn pyramid_pan_fixture_keeps_land_selected() {
        let dir = std::env::var("UNIVERSE_PYRAMID_TEST_VIEW").unwrap();
        let p = Pyramid::open(Path::new(&dir), 6371000.).unwrap().unwrap();
        let mut min = usize::MAX;
        let mut max = 0;
        let mut history = HashSet::new();
        for step in 0..=450 {
            let time = step as f64 * 0.1;
            let lon = pan_longitude(108., time);
            let up = unit(lon, -25.);
            assert!(p.covered(up), "bounded pan stays over the supplied land");
            let east = DVec3::Y.cross(up).normalize();
            let north = up.cross(east);
            let pitch = 54f64.to_radians();
            let forward = north * pitch.cos() - up * pitch.sin();
            let cam_up = up * pitch.cos() + north * pitch.sin();
            let mut cam = Camera {
                position: up * (6371000. + 60000.),
                fov_y: 50f32.to_radians(),
                ..Default::default()
            };
            cam.look_at(cam.position + forward, cam_up.as_vec3());
            let selection = p.source.select(&cam, 16. / 9., 1., &mut history);
            min = min.min(selection.leaves.len());
            max = max.max(selection.leaves.len());
            assert!(
                !selection.leaves.is_empty(),
                "land selection must not disappear at time={time},lon={lon}"
            );
        }
        println!("PYRAMID bounded pan selection min={min} max={max}, samples=451");
    }
    #[test]
    #[ignore = "requires windowed deep belt fixture"]
    fn pyramid_deep_fixture_window_edges_and_belt() {
        let dir = std::env::var("UNIVERSE_PYRAMID_TEST_VIEW").unwrap();
        let p = Pyramid::open(Path::new(&dir), 6371000.).unwrap().unwrap();
        let s = &p.source;
        assert_eq!(s.m.levels.len(), 6);
        assert_eq!(p.mask_level, 2);
        assert_eq!(p.mask.len(), s.m.levels[2].n * s.m.levels[2].n);
        for k in [0, 1] {
            let mut ids: Vec<_> =
                s.m.tiles
                    .iter()
                    .enumerate()
                    .filter(|(_, t)| t.k == k)
                    .map(|(id, _)| id)
                    .collect();
            ids.sort_unstable_by_key(|&id| (s.m.tiles[id].row, s.m.tiles[id].col));
            let samples = [ids[0], ids[ids.len() / 2], *ids.last().unwrap()];
            for id in samples {
                let built = build(s, id, 1., true).unwrap();
                assert!(built.mesh.bytes() <= UPLOAD_BYTES);
                let w = s.m.levels[k].storage();
                for j in 0..built.ground.mask.len() / built.ground.width {
                    for i in 0..built.ground.width {
                        let (gi, gj) = (built.ground.i0 + i, built.ground.j0 + j);
                        if gi < w.col0
                            || gi >= w.col0 + w.cols
                            || gj < w.row0
                            || gj >= w.row0 + w.rows
                        {
                            assert_eq!(built.ground.mask[j * built.ground.width + i], 0);
                        }
                    }
                }
            }
        }
        let up = unit(97.4, -33.6);
        let mut history = HashSet::new();
        for alt in [100000., 30000., 10000., 3000.] {
            let mut cam = Camera {
                position: up * (6371000. + alt),
                fov_y: 50f32.to_radians(),
                ..Default::default()
            };
            cam.look_at(DVec3::ZERO, Vec3::Y);
            let selection = s.select(&cam, 16. / 9., 1., &mut history);
            let mut counts = [0usize; 6];
            for &id in &selection.leaves {
                counts[s.m.tiles[id].k] += 1;
            }
            println!(
                "DEEP belt alt={alt} leaves={} levels={counts:?}",
                selection.leaves.len()
            );
            if alt == 100000. {
                assert!(
                    counts[0] + counts[1] > 0,
                    "the 100km band reaches at least150m detail"
                );
            } else {
                assert!(counts[0] > 0, "near belt views reach37.5m detail");
            }
            assert!(selection.split.iter().all(|&id| s.can_split(id)));
        }
    }
    #[test]
    #[ignore = "requires full land pyramid fixture"]
    fn pyramid_full_fixture_selection_and_streaming() {
        let dir = std::env::var("UNIVERSE_PYRAMID_TEST_VIEW").unwrap();
        let mut p = Pyramid::open(Path::new(&dir), 6371000.).unwrap().unwrap();
        let u = unit(100.4723, -37.1172);
        for alt in [
            10000000., 3000000., 1000000., 300000., 100000., 30000., 10000.,
        ] {
            let mut cam = Camera {
                position: u * (6371000. + alt),
                fov_y: 50f32.to_radians(),
                ..Default::default()
            };
            cam.look_at(DVec3::ZERO, Vec3::Y);
            let sel = p.source.select(&cam, 16.0 / 9.0, 1.0, &mut HashSet::new());
            let tris = sel.leaves.len() * 2 * p.source.m.tile * p.source.m.tile;
            println!(
                "PYRAMID selection altitude={alt} tiles={} upper_triangles={tris}",
                sel.leaves.len()
            );
            assert!(
                tris < if p.source.m.branch == 4 {
                    20_000_000
                } else {
                    5_000_000
                },
                "bounded selection, not finest full raster"
            );
            assert!(sel.leaves.iter().all(|&i| !p.source.can_split(i)
                || p.source.m.tiles[i].error_m / p.source.distance(i, cam.position, 1.0) <= PX));
        }
        let mut cam = Camera {
            position: u * (6371000. + 3000000.),
            fov_y: 50f32.to_radians(),
            ..Default::default()
        };
        cam.look_at(DVec3::ZERO, Vec3::Y);
        let selection = p.source.select(&cam, 16.0 / 9.0, 1.0, &mut HashSet::new());
        let mut largest = 0;
        let mut oversized = 0;
        for id in selection.visited {
            let tile = build(&p.source, id, 1., true).unwrap();
            let size = tile.mesh.bytes();
            largest = largest.max(size);
            oversized += (size > UPLOAD_BYTES) as usize;
        }
        println!("PYRAMID tile bytes largest={largest} over_upload_target={oversized}");
        let hole = u;
        assert!(!p.covered(hole), "Shasta remains the pattern");
        p.mask.clear();
        // Read a finest tile and its parent via row ranges; never materialise a full height level.
        let fine = p
            .source
            .m
            .tiles
            .iter()
            .position(|t| t.k == 0 && p.source.intersects_hole(*t))
            .or_else(|| p.source.m.tiles.iter().position(|t| t.k == 0))
            .unwrap();
        let built = build(&p.source, fine, 1., true).unwrap();
        assert!(built.mesh.bytes() <= UPLOAD_BYTES);
    }
}
