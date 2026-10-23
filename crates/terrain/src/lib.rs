//! The terrain subsystem: a world's ground as the planet LOD renderer draws it, for the game's planet
//! view and the planet LOD app alike (`freefall --app planetlod` is a thin viewer on it). `Ground::open`
//! reads a planet LOD view or a land package (`planet-land-pyramid/1`): the anchor planet in tile groups,
//! the pattern's progressive mesh by chunks (each decided again only when the eye could cross one of its
//! vertices' appearing distances; changed chunks made again off the frame and shown together once on the
//! GPU) or by clusters, the land pyramid streamed by screen error with branch-4 morphs and hysteresis,
//! windowed levels, an LRU cache and the GPU pool's eviction, and the landform layers. `update` steps it
//! for an eye, `draw` draws it placed by a body's transform, `ground_at` and `drawn_ground_at_world` give
//! the finest and the drawn ground, and `flight` the smooth ground reference a camera flies over.

pub mod flight;
mod pyramid;
mod tri_index;
use tri_index::TriIndex;
pub use pyramid::Pyramid;

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use universe_engine::glam::{DVec3, Vec3};
use universe_engine::{Camera, Color, Frame, Mesh, Morph, PoolModel, PooledMesh, Transform, WireModel};

/// Chunk edge (m): a chunk is one mesh.
/// Chunks hold about this many finest triangles (their edge from the pattern's density: about 15 km on the 3x3
/// grid, 2 km on Yosemite's 50 m sampling, where 8 km chunks were 30k triangles and a flip re-sent MBs).
pub const CHUNK_TRIS: f64 = 3000.0;

/// Decision cell edge (m): the vertices decided together (each cell keeps its own margin).
pub const CELL: f64 = 1000.0;

/// Mesh rebuilds a frame stop once this much time is spent (ms); the rest keep their old mesh.
/// Chunk meshes are built off the frame by a worker from a snapshot of what is drawn; a job's meshes are
/// handed to the engine to upload (UPLOAD_TRIS triangles a frame) and all go on screen together once all are
/// on the GPU, so a mixed state (some chunks new, their neighbours old: holes along long triangles, the
/// owner's black triangles when scrolling) is never drawn.
pub const UPLOAD_TRIS: usize = 80_000;

/// Tiles of the planet outside the pattern grouped this many to a side per mesh.
pub const TILE_GROUP: u32 = 4;

pub const ALT_MIN: f64 = 2.0;

/// Each layer's own lift over the ground (m), the k-th at (k + 1) x this: shells, not one surface.
pub const LAYER_LIFT: f64 = 1500.0;

/// A layer's faces are this opaque (75 % transparent, the owner's).
pub const LAYER_ALPHA: f32 = 0.25;

#[derive(Clone, Copy, PartialEq)]
pub enum View {
    Mesh,
    Real,
}

pub struct Data {
    pub r: f64,
    /// Every vertex's direction, and its place (m, planet frame) at its own height times the height x.
    pub dirs: Vec<DVec3>,
    pub pos: Vec<DVec3>,
    pub z: Vec<f32>,
    /// sRGB per vertex (planet source: rock on land, depth at sea).
    pub col: Vec<[u8; 3]>,
    /// Collapsed vertices coarse first: the vertex, its stand-in, its ring (dptr/dep), appearing distance.
    pub mv: Vec<u32>,
    pub ma: Vec<u32>,
    pub dptr: Vec<u32>,
    pub dep: Vec<u32>,
    pub ao: Vec<f32>,
    /// Where it must be fully in (m), and its level (0: the coarsest's).
    pub am: Vec<f32>,
    pub ml: Vec<u8>,
    /// Per vertex: its stand-in (itself if never collapsed).
    pub stand_in: Vec<u32>,
    /// The finest pattern triangles and edges.
    pub ft: Vec<[u32; 3]>,
    pub fe: Vec<[u32; 2]>,
}

pub struct Chunk {
    pub origin: DVec3,
    pub radius: f64,
    pub tris: Vec<u32>,
    pub edges: Vec<u32>,
    pub mesh: Option<Mesh>,
    pub mesh_view: Option<View>,
    pub surface: Option<(Arc<Vec<u8>>, Arc<BuildData>)>,
    pub stale: bool,
    pub tri_count: usize,
    /// Its finest triangles' index (`ground_at`), and its drawn surface's (`drawn_ground_at_world`: its
    /// triangles as drawn, made again with each new surface), each made when first asked.
    pub fine_hits: std::sync::OnceLock<TriIndex>,
    pub drawn_hits: std::sync::OnceLock<(TriIndex, Vec<[DVec3; 3]>)>,
}

pub struct Cell {
    /// Its collapsed vertices (indices into `mv`, ascending: coarse first).
    verts: Vec<u32>,
    /// Where the eye was when it was decided, and how far it may move before deciding again.
    eye_at: DVec3,
    margin: f64,
}

/// What the files say about the pattern and the planet, for the panel.
pub struct Meta {
    pub level_d: Vec<f64>,
    pub level_tris: Vec<usize>,
    pub ahead: f64,
    pub margin: f64,
    pub px_per_m: f64,
    pub peak: DVec3,
    pub radius_m: f64,
    pub top_m: f64,
    pub land_tris: usize,
    pub sea_tris: usize,
    pub kept: usize,
    pub segments: usize,
    pub h_lo: f64,
    pub h_hi: f64,
    pub features: Vec<(String, Vec<[f64; 3]>)>,
}

pub struct TileGroup {
    pub origin: DVec3,
    pub dir: DVec3,
    /// cos of its angular radius, highest ground (m).
    pub cos_a: f64,
    pub zmax: f64,
    pub tris: Vec<[u32; 3]>,
    pub edges: Vec<[u32; 2]>,
    pub sea: bool,
    pub mesh: Option<(View, Mesh)>,
    /// Its triangles' index, made when first asked (by direction alone: right for any height x).
    pub hits: std::sync::OnceLock<TriIndex>,
}

#[derive(Default)]
pub struct Stats {
    pub decided: usize,
    pub changed: usize,
    pub rebuilt: usize,
    pub lod_ms: f64,
    pub mesh_ms: f64,
    /// Collapsed vertices drawn now.
    pub drawn: usize,
}

/// The decisions, in a thread of their own (the owner, 14 Oct: 7.7 ms a frame, 25 at worst, on Yosemite's 903k
/// vertices): it owns what is drawn, takes the eye (the latest, older ones skipped), and sends back a snapshot of
/// what is drawn, the chunks that went stale (boxes of chunks) and its counts.
pub struct Decider {
    mv: Vec<u32>,
    ma: Vec<u32>,
    dptr: Vec<u32>,
    dep: Vec<u32>,
    ao: Vec<f32>,
    am: Vec<f32>,
    ml: Vec<u8>,
    pos: Vec<DVec3>,
    cells: Vec<Cell>,
    cell_of: Vec<u32>,
    rdep_ptr: Vec<u32>,
    rdep: Vec<u32>,
    subtree: Vec<[i32; 4]>,
    queued: Vec<bool>,
    on: Vec<u8>,
    drawn: usize,
    lvl_count: Vec<usize>,
    runs: u64,
}

pub enum ToDecider {
    Eye(DVec3),
    /// New places (the height x changed): every cell decided again.
    Positions(Vec<DVec3>),
}

pub struct Decided {
    /// What is drawn now, when anything changed.
    on: Option<Arc<Vec<u8>>>,
    stale: Vec<[i32; 4]>,
    drawn: usize,
    lvl_count: Vec<usize>,
    decided: usize,
    changed: usize,
    ms: f64,
    /// Every 15th run: the drawn vertices fully in and rising.
    risen: Option<(usize, usize)>,
}

impl Decider {
    fn update(&mut self, eye: DVec3) -> Decided {
        let t0 = Instant::now();
        let mut queue: Vec<u32> = Vec::new();
        for (c, ch) in self.cells.iter().enumerate() {
            if !ch.verts.is_empty() && ch.eye_at.distance(eye) > ch.margin {
                queue.push(c as u32);
                self.queued[c] = true;
            }
        }
        let (mut decided, mut changed) = (0, 0);
        let mut stale: Vec<[i32; 4]> = Vec::new();
        let mut k = 0;
        while k < queue.len() {
            let c = queue[k] as usize;
            k += 1;
            self.queued[c] = false;
            decided += 1;
            let mut margin = f64::INFINITY;
            let verts = std::mem::take(&mut self.cells[c].verts);
            for &i in &verts {
                let i = i as usize;
                let v = self.mv[i] as usize;
                let dist = self.pos[v].distance(eye);
                let ao = self.ao[i] as f64;
                margin = margin.min((dist - ao).abs());
                let mut o = dist < ao && self.on[self.ma[i] as usize] == 1;
                if o {
                    o = (self.dptr[i]..self.dptr[i + 1]).all(|b| self.on[self.dep[b as usize] as usize] == 1);
                }
                if o as u8 != self.on[v] {
                    self.on[v] = o as u8;
                    changed += 1;
                    if o {
                        self.drawn += 1;
                        self.lvl_count[self.ml[i] as usize] += 1;
                    } else {
                        self.drawn -= 1;
                        self.lvl_count[self.ml[i] as usize] -= 1;
                    }
                    stale.push(self.subtree[v]);
                    // Those whose decision reads it are decided again.
                    for &j in &self.rdep[self.rdep_ptr[v] as usize..self.rdep_ptr[v + 1] as usize] {
                        let cj = self.cell_of[self.mv[j as usize] as usize] as usize;
                        if !self.queued[cj] {
                            self.queued[cj] = true;
                            queue.push(cj as u32);
                        }
                    }
                }
            }
            let ch = &mut self.cells[c];
            ch.verts = verts;
            ch.eye_at = eye;
            ch.margin = margin;
        }
        self.runs += 1;
        let risen = (self.runs % 15 == 0).then(|| {
            let (mut full, mut rising) = (0, 0);
            for (i, &v) in self.mv.iter().enumerate() {
                if self.on[v as usize] == 1 {
                    if self.pos[v as usize].distance(eye) <= self.am[i] as f64 { full += 1 } else { rising += 1 }
                }
            }
            (full, rising)
        });
        Decided {
            on: (changed > 0 || self.runs == 1).then(|| Arc::new(self.on.clone())),
            stale,
            drawn: self.drawn,
            lvl_count: self.lvl_count.clone(),
            decided,
            changed,
            ms: t0.elapsed().as_secs_f64() * 1e3,
            risen,
        }
    }
}

/// The cluster DAG (planet-trees-pgs2 docs/cluster-dag.md, the owner's step 4): fixed clusters of about 128
/// triangles, chosen each frame by their error on screen (own error under a pixel at its bounds, its parent's not:
/// a group switches whole, watertight), each built and uploaded once and kept. Read from `<view>-clusters` beside
/// the view folder; in place of the per-vertex decisions and the chunks.
pub struct Clusters {
    /// err, cx, cy, cz, r, perr, pcx, pcy, pcz, pr (planet metres; perr < 0: nothing above).
    pub row: Vec<[f32; 10]>,
    pub ptr: Arc<Vec<[u32; 2]>>,
    pub tris: Arc<Vec<[u32; 3]>>,
    /// Per corner (as `tris`): metres its parent's surface stands above it (none: no morph).
    pub morph: Vec<f32>,
    pub level: Vec<u8>,
    pub meshes: Vec<Option<(View, PooledMesh, bool)>>,
    pub shown: Vec<u32>,
    /// The clusters whose meshes go to the GPU before any other's.
    pub upload_first: Vec<u32>,
    /// The last frame each was wanted (shown, chosen or coming), for evicting the longest unwanted past RESIDENT.
    pub used: Vec<u64>,
    pub frame: u64,
    /// metres of error a pixel covers per metre away (1080 lines, 50 degrees)
    pub px: f64,
    pub stats: [usize; 5],
}

impl Clusters {
    fn open(dir: &Path) -> Result<Self, String> {
        let f: Vec<f32> = read(dir, "cluster.f32")?;
        let pt: Vec<u32> = read(dir, "cluster_tri.u32")?;
        let t: Vec<u32> = read(dir, "clusters.u32")?;
        let level: Vec<u8> = read(dir, "cluster_level.u8")?;
        let n = level.len();
        Ok(Clusters {
            row: f.chunks_exact(10).map(|c| std::array::from_fn(|i| c[i])).collect(),
            ptr: Arc::new(pt.chunks_exact(2).map(|c| [c[0], c[1]]).collect()),
            tris: Arc::new(t.chunks_exact(3).map(|c| [c[0], c[1], c[2]]).collect()),
            morph: read(dir, "cluster_morph.f32").unwrap_or_default(),
            level,
            meshes: (0..n).map(|_| None).collect(),
            shown: vec![],
            upload_first: vec![],
            used: vec![0; n],
            frame: 0,
            px: 2.0 * 25f64.to_radians().tan() / 1080.0,
            stats: [0; 5],
        })
    }

    /// The clusters chosen for an eye at `eye`.
    fn select(&self, eye: DVec3) -> Vec<u32> {
        let mut out = Vec::new();
        for (i, r) in self.row.iter().enumerate() {
            let dist = |c: [f32; 3], rad: f32| (DVec3::new(c[0] as f64, c[1] as f64, c[2] as f64).distance(eye) - rad as f64).max(1e-3);
            let own = r[0] as f64 <= self.px * dist([r[1], r[2], r[3]], r[4]);
            let parent_coarse_enough = r[5] >= 0.0 && r[5] as f64 <= self.px * dist([r[6], r[7], r[8]], r[9]);
            if own && !parent_coarse_enough {
                out.push(i as u32);
            }
        }
        out
    }

    fn origin(&self, i: usize) -> DVec3 {
        let r = &self.row[i];
        DVec3::new(r[1] as f64, r[2] as f64, r[3] as f64)
    }
}

/// A cluster's mesh in the engine's pool (ask 5: indexed, 12-20 B a vertex, every pooled mesh one draw): corners
/// shared; Mesh view every edge once, Real view each face its last corner's colour.
pub fn cluster_mesh(cl: &Clusters, i: usize, pos: &[DVec3], col: &[[u8; 3]], view: View) -> PooledMesh {
    let [a, n] = cl.ptr[i];
    let tris = &cl.tris[a as usize..(a + n) as usize];
    let origin = cl.origin(i);
    let mut m = PoolModel::default();
    // Morph (the smooth swap): each corner slides up or down to its parent's surface, all the way at the distance
    // its parent takes over (its error a pixel there), none at half of it; so the swap finds the same shape.
    let r = cl.row[i];
    let far = if r[5] >= 0.0 { (r[5] as f64 / cl.px) as f32 } else { 0.0 };
    let morph_on = far > 0.0 && cl.morph.len() == cl.tris.len() * 3;
    let mut local: std::collections::HashMap<u32, u32> = std::collections::HashMap::with_capacity(tris.len() * 2);
    let mut at = |v: u32, corner: usize, m: &mut PoolModel| -> u32 {
        *local.entry(v).or_insert_with(|| {
            m.positions.push((pos[v as usize] - origin).as_vec3());
            if morph_on {
                let dz = cl.morph[corner] as f64;
                m.morph.push(Morph { offset: (pos[v as usize].normalize() * dz).as_vec3(), near: 0.5 * far, far });
            }
            m.positions.len() as u32 - 1
        })
    };
    let mut seen = std::collections::HashSet::with_capacity(tris.len() * 3);
    for (k, t) in tris.iter().enumerate() {
        let c0 = (a as usize + k) * 3;
        let f = [at(t[0], c0, &mut m), at(t[1], c0 + 1, &mut m), at(t[2], c0 + 2, &mut m)];
        m.faces.push(f);
        if view == View::Real {
            let c = col[t[2] as usize];
            m.face_colors.push([c[0], c[1], c[2], 255]);
        } else {
            for (x, y) in [(t[0], t[1]), (t[1], t[2]), (t[2], t[0])] {
                if seen.insert((x.min(y), x.max(y))) {
                    // (both ends are corners of this triangle: already placed)
                    let e = [at(x, 0, &mut m), at(y, 0, &mut m)];
                    m.edges.push(e);
                }
            }
        }
    }
    PooledMesh::new(&m)
}

/// Cluster meshes kept at most (about 2 GB of GPU at 128 triangles each).
pub const RESIDENT: usize = 16_000;

/// What a chunk mesh is built from, shared with the build worker (made again when the height x changes).
pub struct BuildData {
    pub pos: Vec<DVec3>,
    pub col: Vec<[u8; 3]>,
    pub ft: Vec<[u32; 3]>,
    pub fe: Vec<[u32; 2]>,
    pub stand_in: Vec<u32>,
    pub chunk_tris: Vec<Vec<u32>>,
    pub chunk_edges: Vec<Vec<u32>>,
    pub origin: Vec<DVec3>,
    /// The package's light exaggeration (`info.json` `light_exaggeration`): the radius, and how much steeper land
    /// (over the radius) and sea floor (under) are lit than they stand. None: lit as they stand.
    pub light: Option<(f64, f64, f64)>,
}

/// A triangle's normal as the package lights it: its corners lifted by `light` (its land or sea factor on their
/// height over `light`'s radius); None without one (the faces' own).
fn lit_normal(t: [DVec3; 3], light: Option<(f64, f64, f64)>) -> Option<Vec3> {
    let (r, land, sea) = light?;
    let [a, b, c] = t.map(|p| {
        let h = p.length() - r;
        p.normalize() * (r + h * if h >= 0.0 { land } else { sea })
    });
    Some((b - a).cross(c - a).normalize_or_zero().as_vec3())
}

/// A package colour (sRGB bytes) as the renderer takes it: linear, as the scene is lit (the screen's encoding is
/// the film curve's, at the end: `blit.wgsl`), as the pool's tiles are decoded in their shader.
pub fn package_colour(c: [u8; 3], alpha: f32) -> [f32; 4] {
    [srgb_to_linear(c[0]), srgb_to_linear(c[1]), srgb_to_linear(c[2]), alpha]
}

pub fn srgb_to_linear(c: u8) -> f32 {
    let c = c as f32 / 255.0;
    if c <= 0.04045 { c / 12.92 } else { ((c + 0.055) / 1.055).powf(2.4) }
}

pub struct Job {
    generation: u64,
    view: View,
    on: Arc<Vec<u8>>,
    bd: Arc<BuildData>,
    chunks: Vec<usize>,
}

/// A job's meshes, built: waiting to be uploaded, then put on screen together.
pub struct Built {
    generation: u64,
    view: View,
    meshes: Vec<(usize, Mesh, usize)>,
    /// How many of them have been handed to the engine to upload.
    prepared: usize,
    surface: (Arc<Vec<u8>>, Arc<BuildData>),
}

/// A chunk's mesh from a snapshot `on` of what is drawn: its finest triangles and edges through their drawn
/// stand-ins, each kept once (many resolve onto the same one). Mesh view: corners shared, edges indexing them;
/// Real: flat faces in their last corner's colour, no edges.
pub fn build_mesh(bd: &BuildData, on: &[u8], c: usize, view: View) -> (Mesh, usize) {
    let resolve = |mut v: u32| {
        while on[v as usize] == 0 {
            v = bd.stand_in[v as usize];
        }
        v
    };
    let origin = bd.origin[c];
    let mut seen_t = std::collections::HashSet::with_capacity(bd.chunk_tris[c].len());
    let tris: Vec<[u32; 3]> = bd.chunk_tris[c].iter().map(|&t| bd.ft[t as usize].map(resolve)).filter(|[a, b, c]| a != b && b != c && a != c).filter(|t| {
        let mut k = *t;
        k.sort_unstable();
        seen_t.insert(k)
    }).collect();
    let mut m = WireModel::default();
    if view == View::Real {
        for t in &tris {
            let k = m.positions.len() as u32;
            for &v in t {
                m.positions.push((bd.pos[v as usize] - origin).as_vec3());
            }
            m.faces.push([k, k + 1, k + 2]);
            m.colors.extend([package_colour(bd.col[t[2] as usize], 1.0); 3]);
            if let Some(n) = lit_normal(t.map(|v| bd.pos[v as usize]), bd.light) {
                m.normals.extend([n; 3]);
            }
        }
        let n = tris.len();
        return (Mesh::new(m), n);
    }
    let mut local: std::collections::HashMap<u32, u32> = std::collections::HashMap::with_capacity(tris.len());
    let mut at = |v: u32, m: &mut WireModel| -> u32 {
        *local.entry(v).or_insert_with(|| {
            m.positions.push((bd.pos[v as usize] - origin).as_vec3());
            m.positions.len() as u32 - 1
        })
    };
    for t in &tris {
        let f = [at(t[0], &mut m), at(t[1], &mut m), at(t[2], &mut m)];
        m.faces.push(f);
    }
    let mut seen_e = std::collections::HashSet::with_capacity(bd.chunk_edges[c].len());
    for &e in &bd.chunk_edges[c] {
        let [a, b] = bd.fe[e as usize].map(resolve);
        if a != b && seen_e.insert((a.min(b), a.max(b))) {
            let pair = [at(a, &mut m), at(b, &mut m)];
            m.edges.push(pair);
        }
    }
    let n = tris.len();
    (Mesh::new(m), n)
}

/// The chunk grid on the plane tangent at the peak.
pub struct Grid {
    p0: DVec3,
    east: DVec3,
    north: DVec3,
    min: f64,
    n: i32,
    size: f64,
}

impl Grid {
    fn uv(&self, p: DVec3) -> (i32, i32) {
        let d = p.normalize();
        let k = d.dot(self.p0);
        let (x, y) = (d.dot(self.east) / k * 6_371_000.0, d.dot(self.north) / k * 6_371_000.0);
        let f = |v: f64| (((v - self.min) / self.size).floor() as i32).clamp(0, self.n - 1);
        (f(x), f(y))
    }
    fn id(&self, p: DVec3) -> u32 {
        let (u, v) = self.uv(p);
        (u * self.n + v) as u32
    }
}

pub fn read<T: Copy>(dir: &Path, name: &str) -> Result<Vec<T>, String> {
    let bytes = std::fs::read(dir.join(name)).map_err(|e| format!("{}: {e}", dir.join(name).display()))?;
    let n = bytes.len() / std::mem::size_of::<T>();
    let mut out = Vec::<T>::with_capacity(n);
    // SAFETY: plain little-endian numbers (f32, u32, u8) written by numpy, read into their own type.
    unsafe {
        std::ptr::copy_nonoverlapping(bytes.as_ptr(), out.as_mut_ptr() as *mut u8, n * std::mem::size_of::<T>());
        out.set_len(n);
    }
    Ok(out)
}

pub fn json(dir: &Path, name: &str) -> Result<serde_json::Value, String> {
    serde_json::from_slice(&std::fs::read(dir.join(name)).map_err(|e| format!("{name}: {e}"))?).map_err(|e| format!("{name}: {e}"))
}

/// The lab's direction for a longitude and latitude (see `land::lab_direction`).
pub use universe_world::worlds::land::lab_direction as unit;

/// One landform layer over the land (views' `layers/`, tools/layer_meshes.py): a class for every land triangle.
pub struct Layer {
    pub title: String,
    pub names: Vec<String>,
    pub rgb: Vec<[u8; 3]>,
    pub share: Vec<f32>,
    pub cls: Vec<u16>,
}

/// The layer meshes (owner 16 Oct): each layer its own mesh over the land, in its classes' colours, 75 %
/// transparent, lifted a little more than the one under it; switched on and off in the panel. The land triangles
/// index the view's vertices (the coast carved in the source), so a layer stands where the ground is drawn.
pub struct Layers {
    pub tri: Vec<[u32; 3]>,
    /// Land triangles grouped by where they are (cube-face cells): each group's centre direction, cos of its
    /// angular radius, its triangles.
    pub groups: Vec<(DVec3, f64, Vec<u32>)>,
    pub list: Vec<Layer>,
    pub on: Vec<bool>,
    /// Built meshes per layer (group's origin), and the (view, height x) they were built for.
    pub meshes: Vec<Vec<(Mesh, DVec3, f64)>>,
    pub built: Vec<Option<u64>>,
}

impl Layers {
    fn open(dir: &Path) -> Result<Option<Layers>, String> {
        let ld = dir.join("layers");
        if !ld.join("layers.json").exists() {
            return Ok(None);
        }
        let meta = json(&ld, "layers.json")?;
        let t: Vec<u32> = read(&ld, "land_tri.u32")?;
        let tri: Vec<[u32; 3]> = t.chunks_exact(3).map(|c| [c[0], c[1], c[2]]).collect();
        let mut list = vec![];
        for l in meta["layers"].as_array().ok_or("layers.json: layers")? {
            let key = l["key"].as_str().unwrap_or("");
            let strs = |v: &serde_json::Value| v.as_array().map(|a| a.iter().map(|x| x.as_str().unwrap_or("").to_string()).collect::<Vec<_>>()).unwrap_or_default();
            let rgb = l["rgb"].as_array().map(|a| a.iter().map(|c| [0, 1, 2].map(|j| c[j].as_u64().unwrap_or(0) as u8)).collect()).unwrap_or_default();
            let share = l["share"].as_array().map(|a| a.iter().map(|x| x.as_f64().unwrap_or(0.0) as f32).collect()).unwrap_or_default();
            let cls: Vec<u16> = read(&ld, &format!("{key}.u16"))?;
            if cls.len() != tri.len() {
                return Err(format!("layers/{key}.u16: {} classes for {} triangles", cls.len(), tri.len()));
            }
            list.push(Layer { title: l["title"].as_str().unwrap_or(key).to_string(), names: strs(&l["names"]), rgb, share, cls });
        }
        let n = list.len();
        Ok(Some(Layers { tri, groups: vec![], list, on: (0..n).map(|k| k == 0).collect(), meshes: (0..n).map(|_| vec![]).collect(), built: vec![None; n] }))
    }

    /// The triangles into cube-face cells (6 x G x G), once the vertices are known.
    fn group(&mut self, dirs: &[DVec3]) {
        const G: usize = 8;
        let mut cells: std::collections::HashMap<usize, Vec<u32>> = Default::default();
        for (i, t) in self.tri.iter().enumerate() {
            let c = (dirs[t[0] as usize] + dirs[t[1] as usize] + dirs[t[2] as usize]).normalize();
            let a = c.abs();
            let (ax, u, v) = if a.x >= a.y && a.x >= a.z { (0, c.y / a.x, c.z / a.x) } else if a.y >= a.z { (1, c.z / a.y, c.x / a.y) } else { (2, c.x / a.z, c.y / a.z) };
            let face = 2 * ax + (c[ax] < 0.0) as usize;
            let cu = (((u + 1.0) / 2.0 * G as f64) as usize).min(G - 1);
            let cv = (((v + 1.0) / 2.0 * G as f64) as usize).min(G - 1);
            cells.entry((face * G + cu) * G + cv).or_default().push(i as u32);
        }
        self.groups = cells.into_values().map(|ts| {
            let mut m = DVec3::ZERO;
            for &i in &ts {
                for v in self.tri[i as usize] {
                    m += dirs[v as usize];
                }
            }
            let m = m.normalize();
            let cos_a = ts.iter().flat_map(|&i| self.tri[i as usize]).map(|v| dirs[v as usize].dot(m)).fold(1.0, f64::min);
            (m, cos_a, ts)
        }).collect();
    }

    /// Layer k's meshes on the ground as drawn now (`pos`), each corner lifted along its own up.
    fn build(&self, k: usize, pos: &[DVec3], dirs: &[DVec3]) -> Vec<(Mesh, DVec3, f64)> {
        let l = &self.list[k];
        let lift = LAYER_LIFT * (k + 1) as f64;
        let col: Vec<[f32; 4]> = l.rgb.iter().map(|c| package_colour(*c, LAYER_ALPHA)).collect();
        self.groups.iter().filter_map(|(m, cos_a, ts)| {
            let origin = *m * pos.iter().take(1).map(|p| p.length()).next().unwrap_or(6.371e6);
            let mut w = WireModel::default();
            let mut top: f64 = 0.0;
            for &i in ts {
                let c = l.cls[i as usize];
                if c == u16::MAX || c as usize >= col.len() {
                    continue;
                }
                let base = w.positions.len() as u32;
                for v in self.tri[i as usize] {
                    let p = pos[v as usize] + dirs[v as usize] * lift;
                    top = top.max(p.length());
                    w.positions.push((p - origin).as_vec3());
                }
                w.faces.push([base, base + 1, base + 2]);
                w.colors.extend([col[c as usize]; 3]);
            }
            let _ = cos_a;
            (!w.faces.is_empty()).then(|| (Mesh::new(w), origin, top))
        }).collect()
    }
}


/// What `land_draw` drew, for the app's panel and its view-box count.
pub struct LandDrawn<'a> {
    pub drawn: Vec<(&'a Mesh, DVec3)>,
    pub sent: usize,
    pub chunks_drawn: usize,
    pub tiles_drawn: usize,
    pub pattern_tris: usize,
    pub drawn_clusters: Vec<u32>,
    pub cl_data: Option<(Arc<Vec<[u32; 2]>>, Arc<Vec<[u32; 3]>>)>,
    pub aspect: f64,
}

/// A point of the package's frame placed by `body` (centre, spin, scale), as a model's transform there.
pub fn placed(body: &Transform, at: DVec3) -> Transform {
    // (Normalised in f64: an f32 rotation's length is off by about 1e-7, a metre at a planet's radius.)
    Transform { position: body.position + body.rotation.as_dquat().normalize() * (at * body.scale), rotation: body.rotation, scale: body.scale }
}

/// A world's ground drawn as the planet LOD app draws it: its anchor mesh in tile groups, its pattern by
/// chunks or clusters, its land pyramid, its layers (see the crate).
pub struct Ground {
    pub dir: PathBuf,
    pub d: Data,
    pub chunks: Vec<Chunk>,
    pub grid: Grid,
    pub tiles: Vec<TileGroup>,
    pub view: View,
    /// The render surface's height (px, f64 bits; 0: not drawn yet), from the last `draw`: the density cap's.
    screen_px: std::sync::atomic::AtomicU64,
    pub stats: Stats,
    /// The cluster DAG, when the view has one (in place of the decisions and the chunks).
    pub clusters: Option<Mutex<Clusters>>,
    /// The eye's smoothed velocity (m/s), for the clusters' preload a second ahead, and where it was.
    pub vel: DVec3,
    pub last_eye: Option<DVec3>,
    /// The decider: its inbox and what it sends back; the latest snapshot of what is drawn.
    pub to_decider: std::sync::mpsc::Sender<ToDecider>,
    pub from_decider: Mutex<std::sync::mpsc::Receiver<Decided>>,
    pub on_snap: Arc<Vec<u8>>,
    /// The build worker: its inbox, what it sends back, whether a job is out, what jobs are built from, and the
    /// generation (height x and view: an older one's results are dropped); the job built and going up.
    pub jobs: std::sync::mpsc::Sender<Job>,
    pub done: Mutex<std::sync::mpsc::Receiver<Built>>,
    pub busy: bool,
    pub bd: Arc<BuildData>,
    pub generation: u64,
    pub last_view: View,
    pub built: Mutex<Option<Built>>,
    /// UNIVERSE_PLANETLOD_MAXDRAW: at most this many chunks drawn (to tell a draw's cost from a triangle's).
    pub max_draw: usize,
    pub meta: Meta,
    pub ex: f64,
    pub show_edges: bool,
    pub show_faces: bool,
    /// Drawn collapsed vertices per level; the last full count of those fully in and rising.
    pub lvl_count: Vec<usize>,
    pub risen: (usize, usize),
    /// The landform layers over the land, when the view has them.
    pub layers: Option<Layers>,
    pub pyramid: Option<Mutex<Pyramid>>,
    /// The layers keep their meshes (a slider is being dragged).
    pub hold_layers: bool,
}

impl Ground {
    /// The ground in `dir` (a planet LOD view or a land package): read, its workers started.
    pub fn open(dir: &Path) -> Result<Self, String> {
        let t0 = Instant::now();
        let dir = dir.to_path_buf();
        let info = json(&dir, "info.json")?;
        let lod = json(&dir, "lod.json")?;
        let r = info["radius_m"].as_f64().ok_or("info.json: radius_m")?;
        let levels = lod["levels"].as_array().ok_or("lod.json: levels")?.len();
        let dirs: Vec<f32> = read(&dir, "dir.f32")?;
        let z: Vec<f32> = read(&dir, "z.f32")?;
        let n = z.len();
        let unit_dirs: Vec<DVec3> = (0..n).map(|i| DVec3::new(dirs[3 * i] as f64, dirs[3 * i + 1] as f64, dirs[3 * i + 2] as f64).normalize()).collect();
        let pos: Vec<DVec3> = (0..n).map(|i| unit_dirs[i] * (r + z[i] as f64)).collect();
        let col8: Vec<u8> = read(&dir, "color.u8")?;
        let col = (0..n).map(|i| [col8[3 * i], col8[3 * i + 1], col8[3 * i + 2]]).collect();
        let mv: Vec<u32> = read(&dir, "morph_vertex.u32")?;
        let ma: Vec<u32> = read(&dir, "morph_anc.u32")?;
        let dptr: Vec<u32> = read(&dir, "morph_dptr.u32")?;
        let dep: Vec<u32> = read(&dir, "morph_dep.u32")?;
        let md: Vec<f32> = read(&dir, "morph_dist.f32")?;
        let ao = (0..mv.len()).map(|i| md[2 * i]).collect();
        let am = (0..mv.len()).map(|i| md[2 * i + 1]).collect();
        let ml: Vec<u8> = read(&dir, "morph_level.u8")?;
        let mut stand_in: Vec<u32> = (0..n as u32).collect();
        for (i, &v) in mv.iter().enumerate() {
            stand_in[v as usize] = ma[i];
        }
        let ft32: Vec<u32> = read(&dir, &format!("lvl_{levels}.u32"))?;
        let fe32: Vec<u32> = read(&dir, &format!("lvl_{levels}.e.u32"))?;
        let ft: Vec<[u32; 3]> = ft32.chunks_exact(3).map(|c| [c[0], c[1], c[2]]).collect();
        let fe: Vec<[u32; 2]> = fe32.chunks_exact(2).map(|c| [c[0], c[1]]).collect();
        let mut d = Data { r, dirs: unit_dirs, pos, z, col, mv, ma, dptr, dep, ao, am, ml, stand_in, ft, fe };
        drop(dirs);
        drop(col8);
        let mut pyramid = Pyramid::open(&dir, r)?;
        let mut tiles = Self::tile_groups(&dir, &d, pyramid.as_ref())?;
        if let Some(p) = &mut pyramid {
            // The full fine mask is only needed for this one-time static-package exclusion.
            p.mask = Vec::new();
            // Optional legacy overlays and cluster indices use the package's original vertex ids.
            // Keep those ids when present; otherwise release covered vertices before worker snapshots.
            let clusters = PathBuf::from(format!("{}-clusters", dir.display()));
            if !dir.join("layers/layers.json").exists() && !clusters.join("cluster.f32").exists() {
                Self::compact_package(&mut d, &mut tiles);
            }
        }
        let n = d.pos.len();
        let pyramid = pyramid.map(Mutex::new);
        let mut layers = Layers::open(&dir)?;
        if let Some(l) = &mut layers {
            l.group(&d.dirs);
        }

        // The chunk grid: over every finest triangle's centre, in the plane tangent at the peak.
        let peak = &lod["peak"];
        let p0 = unit(peak["lon"].as_f64().unwrap_or(0.0), peak["lat"].as_f64().unwrap_or(0.0));
        let east = DVec3::new(p0.z, 0.0, -p0.x).normalize();
        let north = p0.cross(east);
        let (mut lo, mut hi) = (f64::INFINITY, f64::NEG_INFINITY);
        for t in &d.ft {
            let c = (d.pos[t[0] as usize] + d.pos[t[1] as usize] + d.pos[t[2] as usize]).normalize();
            let k = c.dot(p0);
            for v in [c.dot(east) / k * 6_371_000.0, c.dot(north) / k * 6_371_000.0] {
                lo = lo.min(v);
                hi = hi.max(v);
            }
        }
        // (The triangles' centres fill a square of (hi - lo) on a side, not evenly: the chunk edge from the mean.)
        let chunk = ((hi - lo).powi(2) * CHUNK_TRIS / d.ft.len().max(1) as f64).sqrt().clamp(1000.0, 16_000.0);
        let gn = ((hi - lo) / chunk).ceil() as i32 + 1;
        let grid = Grid { p0, east, north, min: lo, n: gn, size: chunk };
        let cn = ((hi - lo) / CELL).ceil() as i32 + 1;
        let cell_grid = Grid { p0, east, north, min: lo, n: cn, size: CELL };
        let mut chunks: Vec<Chunk> = (0..gn * gn).map(|_| Chunk { origin: DVec3::ZERO, radius: 0.0, tris: vec![], edges: vec![], mesh: None, mesh_view: None, surface: None, stale: true, tri_count: 0, fine_hits: Default::default(), drawn_hits: Default::default() }).collect();
        let mut cells: Vec<Cell> = (0..cn * cn).map(|_| Cell { verts: vec![], eye_at: DVec3::splat(f64::INFINITY), margin: 0.0 }).collect();
        for (k, t) in d.ft.iter().enumerate() {
            chunks[grid.id(d.pos[t[0] as usize] + d.pos[t[1] as usize] + d.pos[t[2] as usize]) as usize].tris.push(k as u32);
        }
        for (k, e) in d.fe.iter().enumerate() {
            chunks[grid.id(d.pos[e[0] as usize] + d.pos[e[1] as usize]) as usize].edges.push(k as u32);
        }
        let mut cell_of = vec![u32::MAX; n];
        for (i, &v) in d.mv.iter().enumerate() {
            let c = cell_grid.id(d.pos[v as usize]);
            cell_of[v as usize] = c;
            cells[c as usize].verts.push(i as u32);
        }
                for c in chunks.iter_mut() {
            let mut sum = DVec3::ZERO;
            let mut k = 0.0;
            for &t in &c.tris {
                for &v in &d.ft[t as usize] {
                    sum += d.pos[v as usize];
                    k += 1.0;
                }
            }
            if k > 0.0 {
                c.origin = sum / k;
            }
        }
        // Who reads whom: a collapsed vertex's decision reads its stand-in and its ring.
        let mut cnt = vec![0u32; n + 1];
        for i in 0..d.mv.len() {
            cnt[d.ma[i] as usize + 1] += 1;
            for b in d.dptr[i]..d.dptr[i + 1] {
                cnt[d.dep[b as usize] as usize + 1] += 1;
            }
        }
        for k in 0..n {
            cnt[k + 1] += cnt[k];
        }
        let mut rdep = vec![0u32; cnt[n] as usize];
        let mut at = cnt.clone();
        for i in 0..d.mv.len() {
            let mut put = |u: u32| {
                rdep[at[u as usize] as usize] = i as u32;
                at[u as usize] += 1;
            };
            put(d.ma[i]);
            for b in d.dptr[i]..d.dptr[i + 1] {
                put(d.dep[b as usize]);
            }
        }
        // A vertex's collapse subtree: the chunks of the triangles of every vertex that resolves through it.
        let mut subtree = vec![[i32::MAX, i32::MAX, i32::MIN, i32::MIN]; n];
        let grow = |b: &mut [i32; 4], u: i32, v: i32| {
            b[0] = b[0].min(u);
            b[1] = b[1].min(v);
            b[2] = b[2].max(u);
            b[3] = b[3].max(v);
        };
        for (c, ch) in chunks.iter().enumerate() {
            let (u, v) = (c as i32 / gn, c as i32 % gn);
            for &t in &ch.tris {
                for &w in &d.ft[t as usize] {
                    grow(&mut subtree[w as usize], u, v);
                }
            }
        }
        for i in (0..d.mv.len()).rev() {
            let (v, s) = (d.mv[i] as usize, d.ma[i] as usize);
            let b = subtree[v];
            if b[0] <= b[2] {
                grow(&mut subtree[s], b[0], b[1]);
                grow(&mut subtree[s], b[2], b[3]);
            }
        }
        for c in chunks.iter_mut() {
            c.radius = c.tris.iter().flat_map(|&t| d.ft[t as usize]).map(|v| d.pos[v as usize].distance(c.origin)).fold(0.0, f64::max);
        }
        let mut on = vec![1u8; n];
        for &v in &d.mv {
            on[v as usize] = 0;
        }

        let info = &info;
        let num = |v: &serde_json::Value| v.as_f64().unwrap_or(0.0);
        let features = json(&dir, "features.json").ok().and_then(|f| f.as_array().cloned()).unwrap_or_default().iter()
            .map(|f| (f["role"].as_str().unwrap_or("").to_string(), f["lonlatz"].as_array().map(|a| a.iter().map(|q| [num(&q[0]), num(&q[1]), num(&q[2])]).collect()).unwrap_or_default()))
            .collect();
        let meta = Meta {
            level_d: lod["levels"].as_array().map(|a| a.iter().map(|l| num(&l["D_m"])).collect()).unwrap_or_default(),
            level_tris: lod["table"].as_array().map(|a| a.iter().map(|l| num(&l["triangles"]) as usize).collect()).unwrap_or_default(),
            ahead: num(&lod["ahead"]),
            margin: num(&lod["margin"]),
            px_per_m: num(&lod["px_per_m"]),
            peak: p0,
            radius_m: num(&lod["radius_m"]),
            top_m: num(&lod["top_m"]),
            land_tris: num(&info["land_triangles"]) as usize,
            sea_tris: num(&info["water_triangles"]) as usize,
            kept: num(&info["segments_kept_as_edges"]) as usize,
            segments: num(&info["line_segments"]) as usize,
            h_lo: num(&info["height_min_m"]),
            h_hi: num(&info["height_max_m"]),
            features,
        };
        let queued = vec![false; cells.len()];
        let bd = Arc::new(BuildData {
            pos: d.pos.clone(),
            col: d.col.clone(),
            ft: d.ft.clone(),
            fe: d.fe.clone(),
            stand_in: d.stand_in.clone(),
            chunk_tris: chunks.iter().map(|c| c.tris.clone()).collect(),
            chunk_edges: chunks.iter().map(|c| c.edges.clone()).collect(),
            origin: chunks.iter().map(|c| c.origin).collect(),
            light: info.get("light_exaggeration").and_then(|l| Some((r, l["land"].as_f64()?, l["sea"].as_f64()?))),
        });
        let (jobs, inbox) = std::sync::mpsc::channel::<Job>();
        let (outbox, done) = std::sync::mpsc::channel::<Built>();
        std::thread::Builder::new().name("planetlod build".into()).spawn(move || {
            use rayon::prelude::*;
            while let Ok(job) = inbox.recv() {
                let meshes: Vec<(usize, Mesh, usize)> = job.chunks.par_iter().map(|&c| {
                    let (m, n) = build_mesh(&job.bd, &job.on, c, job.view);
                    // (Packed on the worker, off the render thread.)
                    m.pack();
                    (c, m, n)
                }).collect();
                if outbox.send(Built { generation: job.generation, view: job.view, meshes, prepared: 0, surface: (job.on.clone(), job.bd.clone()) }).is_err() {
                    return;
                }
            }
        }).map_err(|e| format!("build worker: {e}"))?;
        let on_snap = Arc::new(on.clone());
        let cdir = PathBuf::from(format!("{}-clusters", dir.display()));
        let clusters = if cdir.join("cluster.f32").exists() { Some(Mutex::new(Clusters::open(&cdir)?)) } else { None };
        let mut decider = Decider {
            mv: d.mv.clone(), ma: d.ma.clone(), dptr: d.dptr.clone(), dep: d.dep.clone(), ao: d.ao.clone(), am: d.am.clone(), ml: d.ml.clone(),
            pos: d.pos.clone(), cells, cell_of, rdep_ptr: cnt, rdep, subtree, queued, on, drawn: 0, lvl_count: vec![0; 256], runs: 0,
        };
        let (to_decider, decider_inbox) = std::sync::mpsc::channel::<ToDecider>();
        let (decider_out, from_decider) = std::sync::mpsc::channel::<Decided>();
        std::thread::Builder::new().name("planetlod decide".into()).spawn(move || {
            while let Ok(first) = decider_inbox.recv() {
                // (The latest eye; new places applied in order.)
                let mut eye = None;
                for msg in std::iter::once(first).chain(decider_inbox.try_iter()) {
                    match msg {
                        ToDecider::Eye(e) => eye = Some(e),
                        ToDecider::Positions(p) => {
                            decider.pos = p;
                            for c in &mut decider.cells {
                                c.eye_at = DVec3::splat(f64::INFINITY);
                            }
                        }
                    }
                }
                if let Some(e) = eye
                    && decider_out.send(decider.update(e)).is_err()
                {
                    return;
                }
            }
        }).map_err(|e| format!("decider: {e}"))?;
        let g = Ground {
            dir, d, chunks, grid, clusters, vel: DVec3::ZERO, last_eye: None, to_decider, from_decider: Mutex::new(from_decider), on_snap, tiles,
            view: View::Real, screen_px: Default::default(), stats: Stats::default(), jobs, done: Mutex::new(done), busy: false, bd, generation: 0, last_view: View::Real, built: Mutex::new(None),
            max_draw: std::env::var("UNIVERSE_PLANETLOD_MAXDRAW").ok().and_then(|v| v.parse().ok()).unwrap_or(usize::MAX),
            meta, ex: 1.0, show_edges: true, show_faces: true, lvl_count: vec![0; 256], risen: (0, 0), layers, pyramid, hold_layers: false,
        };
        log::info!(
            "planet LOD: {} vertices ({} collapsed), {} finest triangles in {} chunks, {} tile groups, read in {:.1} s from {}",
            g.d.pos.len(), g.d.mv.len(), g.d.ft.len(), g.chunks.iter().filter(|c| !c.tris.is_empty()).count(), g.tiles.len(), t0.elapsed().as_secs_f64(), g.dir.display()
        );
        Ok(g)
    }

    /// The package's radius (m).
    pub fn package_radius(&self) -> f64 {
        self.d.r
    }

    /// The planet outside the pattern: its tiles (land and sea apart) grouped TILE_GROUP to a side.
    pub fn tile_groups(dir: &Path, d: &Data, pyramid: Option<&Pyramid>) -> Result<Vec<TileGroup>, String> {
        let tri: Vec<u32> = read(dir, "tri.u32")?;
        let tf: Vec<f32> = read(dir, "tiles.f32")?;
        let tu: Vec<u32> = read(dir, "tiles.u32")?;
        let te: Vec<u32> = read(dir, "tiles.e.u32")?;
        let nt = tu.len() / 5;
        let mut groups: std::collections::BTreeMap<(u32, u32, u32, u32), Vec<usize>> = Default::default();
        for j in 0..nt {
            // (Tiles in key order: sea * 6 T^2 + (face * T + u) * T + v, T = 32; the key is not stored, so it
            // is found again from the tile's centre.)
            let c = DVec3::new(tf[6 * j] as f64, tf[6 * j + 1] as f64, tf[6 * j + 2] as f64);
            let a = c.abs();
            let (ax, m) = if a.x >= a.y && a.x >= a.z { (0, c.x) } else if a.y >= a.z { (1, c.y) } else { (2, c.z) };
            let (o1, o2) = ((ax + 1) % 3, (ax + 2) % 3);
            let face = 2 * ax + (m < 0.0) as usize;
            let cell = |o: usize| ((((c[o] / m.abs()) + 1.0) / 2.0 * 32.0).floor().clamp(0.0, 31.0) as u32) / TILE_GROUP;
            groups.entry((tu[5 * j + 4], face as u32, cell(o1), cell(o2))).or_default().push(j);
        }
        let mut out = Vec::with_capacity(groups.len());
        for ((sea, ..), js) in groups {
            let mut tris = vec![];
            let mut edges = vec![];
            let mut dir_sum = DVec3::ZERO;
            for &j in &js {
                let (s0, n, e0, en) = (tu[5 * j] as usize, tu[5 * j + 1] as usize, tu[5 * j + 2] as usize, tu[5 * j + 3] as usize);
                tris.extend((s0..s0 + n).map(|t| [tri[3 * t], tri[3 * t + 1], tri[3 * t + 2]]).filter(|t| {
                    !pyramid.is_some_and(|p| p.covered((d.pos[t[0] as usize] + d.pos[t[1] as usize] + d.pos[t[2] as usize]).normalize()))
                }));
                edges.extend((e0..e0 + en).map(|e| [te[2 * e], te[2 * e + 1]]).filter(|e| {
                    !pyramid.is_some_and(|p| p.covered((d.pos[e[0] as usize] + d.pos[e[1] as usize]).normalize()))
                }));
                dir_sum += DVec3::new(tf[6 * j] as f64, tf[6 * j + 1] as f64, tf[6 * j + 2] as f64);
            }
            if tris.is_empty() && edges.is_empty() { continue; }
            let dir = dir_sum.normalize();
            let (mut cos_a, mut zmax) = (1.0f64, f64::NEG_INFINITY);
            for &v in tris.iter().flatten().chain(edges.iter().flatten()) {
                cos_a = cos_a.min(d.pos[v as usize].normalize().dot(dir));
                zmax = zmax.max(d.z[v as usize] as f64);
            }
            out.push(TileGroup { origin: dir * d.r, dir, cos_a: cos_a - 1e-6, zmax, tris, edges, sea: sea == 1, mesh: None, hits: Default::default() });
        }
        Ok(out)
    }

    pub fn compact_package(d: &mut Data, tiles: &mut [TileGroup]) {
        let before = d.pos.len();
        let mut used = vec![false; before];
        for &v in d.ft.iter().flatten().chain(d.fe.iter().flatten()).chain(&d.mv).chain(&d.ma).chain(&d.dep)
            .chain(tiles.iter().flat_map(|g| g.tris.iter().flatten().chain(g.edges.iter().flatten()))) { used[v as usize] = true; }
        // Collapse ancestors are needed even if no retained triangle directly references them.
        for &v in &d.mv { if used[v as usize] { used[d.stand_in[v as usize] as usize] = true; } }
        let ids: Vec<_> = used.iter().enumerate().filter_map(|(i,&on)| on.then_some(i)).collect();
        let mut remap = vec![u32::MAX; before];
        for (new,&old) in ids.iter().enumerate() { remap[old] = new as u32; }
        d.pos = ids.iter().map(|&i| d.pos[i]).collect();
        d.dirs = ids.iter().map(|&i| d.dirs[i]).collect();
        d.z = ids.iter().map(|&i| d.z[i]).collect();
        d.col = ids.iter().map(|&i| d.col[i]).collect();
        d.stand_in = ids.iter().map(|&i| remap[d.stand_in[i] as usize]).collect();
        for v in d.ft.iter_mut().flatten().chain(d.fe.iter_mut().flatten()).chain(&mut d.mv).chain(&mut d.ma).chain(&mut d.dep)
            .chain(tiles.iter_mut().flat_map(|g| g.tris.iter_mut().flatten().chain(g.edges.iter_mut().flatten()))) { *v = remap[*v as usize]; }
        log::info!("planet LOD: compacted pyramid-covered package: {} -> {} vertices", before, d.pos.len());
    }

    /// Finest available ground, before exaggeration, used only for peak clearance and the HUD.
    pub fn ground_at(&self, u: DVec3) -> Option<f64> {
        if let Some(p) = &self.pyramid {
            if let Some(h) = p.lock().unwrap().ground_at(u) { return Some(h); }
        }
        let d = &self.d;
        let hit = |t: [u32; 3]| -> Option<f64> {
            let [a, b, c] = t.map(|v| d.dirs[v as usize]);
            let (wa, wb, wc) = (u.dot(b.cross(c)), u.dot(c.cross(a)), u.dot(a.cross(b)));
            let s = wa + wb + wc;
            (s.abs() > 0.0 && wa / s >= -1e-9 && wb / s >= -1e-9 && wc / s >= -1e-9).then(|| (wa * d.z[t[0] as usize] as f64 + wb * d.z[t[1] as usize] as f64 + wc * d.z[t[2] as usize] as f64) / s)
        };
        let (cu, cv) = self.grid.uv(u);
        for du in -1..=1 {
            for dv in -1..=1 {
                let (x, y) = (cu + du, cv + dv);
                if x < 0 || y < 0 || x >= self.grid.n || y >= self.grid.n {
                    continue;
                }
                let ch = &self.chunks[(x * self.grid.n + y) as usize];
                if ch.tris.is_empty() {
                    continue;
                }
                let index = ch.fine_hits.get_or_init(|| {
                    let tris = ch.tris.iter().map(|&t| d.ft[t as usize].map(|v| d.dirs[v as usize]));
                    TriIndex::new(ch.origin.try_normalize().unwrap_or(u), tris)
                });
                for &k in index.candidates(u) {
                    // (The finest surface, not the drawn: a change of detail must not move the eye.)
                    if let Some(h) = hit(d.ft[ch.tris[k as usize] as usize]) {
                        return Some(h);
                    }
                }
            }
        }
        for g in &self.tiles {
            if u.dot(g.dir) >= g.cos_a {
                for &k in self.group_hits(g).candidates(u) {
                    if let Some(h) = hit(g.tris[k as usize]) {
                        return Some(h);
                    }
                }
            }
        }
        None
    }

    /// Tile group `g`'s triangles' index (by their corners' directions: the same for its drawn triangles
    /// at any height x).
    fn group_hits<'a>(&self, g: &'a TileGroup) -> &'a TriIndex {
        g.hits.get_or_init(|| TriIndex::new(g.dir, g.tris.iter().map(|t| t.map(|v| self.d.dirs[v as usize]))))
    }

    /// The published pyramid/chunk surface, in world metres, rather than prefetched or desired detail.
    pub fn drawn_ground_at_world(&self, u: DVec3) -> Option<f64> {
        if let Some(p) = &self.pyramid {
            if let Some(h) = p.lock().unwrap().drawn_ground_at_world(u) { return Some(h); }
        }
        let (cu,cv) = self.grid.uv(u);
        for du in -1..=1 { for dv in -1..=1 {
            let (x,y)=(cu+du,cv+dv);
            if x<0 || y<0 || x>=self.grid.n || y>=self.grid.n { continue; }
            let ch=&self.chunks[(x*self.grid.n+y) as usize];
            let Some((on,bd))=&ch.surface else { continue; };
            let (index, points) = ch.drawn_hits.get_or_init(|| {
                let resolve=|mut v:u32| { while on[v as usize]==0 { v=bd.stand_in[v as usize]; } v };
                // (The fine triangles as drawn: many fold into one coarse triangle, or none.)
                let mut seen = std::collections::HashSet::new();
                let tris: Vec<[u32; 3]> = ch.tris.iter().map(|&t| bd.ft[t as usize].map(resolve))
                    .filter(|t| t[0] != t[1] && t[1] != t[2] && t[0] != t[2] && seen.insert(*t)).collect();
                // Meshes retain f32 local positions relative to this origin.
                let points: Vec<[DVec3; 3]> = tris.iter().map(|t| t.map(|v| ch.origin+(bd.pos[v as usize]-ch.origin).as_vec3().as_dvec3())).collect();
                (TriIndex::new(ch.origin.try_normalize().unwrap_or(u), points.iter().copied()), points)
            });
            for &k in index.candidates(u) {
                if let Some(h)=flight::triangle_height(u,points[k as usize],self.d.r) { return Some(h); }
            }
        }}
        for g in &self.tiles {
            if g.mesh.is_none() || u.dot(g.dir)<g.cos_a { continue; }
            for &k in self.group_hits(g).candidates(u) {
                let points=g.tris[k as usize].map(|v|g.origin+(self.d.pos[v as usize]-g.origin).as_vec3().as_dvec3());
                if let Some(h)=flight::triangle_height(u,points,self.d.r) { return Some(h); }
            }
        }
        None
    }

    /// A new height x: every place again, every mesh and decision made again.
    pub fn set_ex(&mut self, ex: f64) {
        self.ex = ex;
        let r = self.d.r;
        for i in 0..self.d.pos.len() {
            self.d.pos[i] = self.d.dirs[i] * (r + ex * self.d.z[i] as f64);
        }
        let _ = self.to_decider.send(ToDecider::Positions(self.d.pos.clone()));
        for c in &mut self.chunks {
            c.stale = true;
        }
        for g in &mut self.tiles {
            g.mesh = None;
        }
        let old = self.bd.clone();
        self.bd = Arc::new(BuildData { pos: self.d.pos.clone(), col: old.col.clone(), ft: old.ft.clone(), fe: old.fe.clone(), stand_in: old.stand_in.clone(), chunk_tris: old.chunk_tris.clone(), chunk_edges: old.chunk_edges.clone(), origin: old.origin.clone(), light: old.light });
        self.generation += 1;
        *self.built.lock().unwrap() = None;
    }

    /// The crest and valley lines, 3 m over the ground, coloured by their role.
    pub fn build_features(&self) -> Mesh {
        let mut m = WireModel::default();
        let o = self.meta.peak * self.d.r;
        for (role, pts) in &self.meta.features {
            let c = match role.as_str() {
                "crest" => [1.0, 0.25, 0.25, 1.0],
                "valley-main" => [1.0, 1.0, 1.0, 1.0],
                "spur" => [1.0, 0.6, 0.38, 1.0],
                _ => [0.25, 0.88, 1.0, 1.0],
            };
            let k0 = m.positions.len() as u32;
            for q in pts {
                m.positions.push((unit(q[0], q[1]) * (self.d.r + self.ex * q[2] + 3.0) - o).as_vec3());
                m.colors.push(c);
            }
            for k in 1..pts.len() as u32 {
                m.edges.push([k0 + k - 1, k0 + k]);
            }
        }
        Mesh::new(m)
    }

    /// Decide again the chunks the eye has moved far enough from, and whatever their changes reach.
    /// The cluster cut: chosen for the eye and for where it will be in a second (built and uploaded, not drawn);
    /// shown once every cluster of it in view is on the GPU, else the cut shown stays.
    pub fn update_clusters(&mut self, eye: DVec3, _dt: f64) {
        use rayon::prelude::*;
        let t0 = Instant::now();
        let view = self.view;
        let ahead = {
            let mut step = self.vel * 1.0;
            let most = 0.5 * (eye.length() - self.d.r).max(ALT_MIN);
            if step.length() > most {
                step = step.normalize() * most;
            }
            eye + step
        };
        let mut cl = self.clusters.as_ref().unwrap().lock().unwrap();
        let want = cl.select(eye);
        let pre = cl.select(ahead);
        // build what is wanted or coming and not built (in this view), in parallel, at most 4,000 a frame
        let mut need: Vec<usize> = want.iter().chain(pre.iter()).map(|&i| i as usize).filter(|&i| cl.meshes[i].as_ref().is_none_or(|(v, _, _)| *v != view)).collect();
        need.sort_unstable();
        need.dedup();
        need.truncate(4000);
        let (pos, col) = (&self.d.pos, &self.d.col);
        let built: Vec<(usize, PooledMesh)> = need.par_iter().map(|&i| (i, cluster_mesh(&cl, i, pos, col, view))).collect();
        for (i, m) in built {
            cl.meshes[i] = Some((view, m, false));
        }
        // shown: the wanted cut once all its clusters are on the GPU. (Only those in view, before: flying or turning
        // brought the rest into view before they were uploaded, and they were skipped: black holes in the ground.)
        let ready = want.iter().all(|&i| cl.meshes[i as usize].as_ref().is_some_and(|(v, m, _)| *v == view && m.gpu_resident()));
        if ready {
            cl.shown = want.clone();
        }
        let finest = cl.shown.iter().map(|&i| cl.level[i as usize] as usize).min().unwrap_or(0);
        // Kept at most RESIDENT cluster meshes: the longest unwanted dropped (their GPU copies go with them).
        cl.frame += 1;
        let now = cl.frame;
        // uploads first for what is shown, wanted, then coming
        cl.upload_first = cl.shown.iter().chain(want.iter()).chain(pre.iter()).copied().collect();
        let marks: Vec<u32> = cl.shown.iter().chain(want.iter()).chain(pre.iter()).copied().collect();
        for i in marks {
            cl.used[i as usize] = now;
        }
        let resident = cl.meshes.iter().filter(|m| m.is_some()).count();
        if resident > RESIDENT {
            let mut old: Vec<(u64, usize)> = (0..cl.meshes.len()).filter(|&i| cl.meshes[i].is_some() && cl.used[i] < now).map(|i| (cl.used[i], i)).collect();
            old.sort_unstable();
            for &(_, i) in old.iter().take(resident - RESIDENT) {
                cl.meshes[i] = None;
            }
        }
        cl.stats = [cl.shown.len(), need.len(), pre.len(), cl.meshes.iter().filter(|m| m.is_some()).count(), finest];
        log::debug!("clusters shown {} built {} ahead {} resident {} finest level {}", cl.stats[0], cl.stats[1], cl.stats[2], cl.stats[3], cl.stats[4]);
        drop(cl);
        self.stats.lod_ms = t0.elapsed().as_secs_f64() * 1e3;
    }

    pub fn build_tiles(&self, g: &TileGroup) -> Mesh {
        let edges: &[[u32; 2]] = if self.view == View::Mesh { &g.edges } else { &[] };
        let m = self.model(g.origin, &g.tris, edges);
        // (Built in parallel off the render thread: packed there too.)
        m.pack();
        m
    }

    /// A mesh of planet vertices about `origin`. Mesh view: the corners shared, the edges indexing
    /// them (the engine finds an edge's normal by its ends); Real view: each face its own corners in
    /// its last corner's colour (the page's flat colour), no edges.
    pub fn model(&self, origin: DVec3, tris: &[[u32; 3]], edges: &[[u32; 2]]) -> Mesh {
        let d = &self.d;
        let mut m = WireModel::default();
        if self.view == View::Real {
            for t in tris {
                let k = m.positions.len() as u32;
                for &v in t {
                    m.positions.push((d.pos[v as usize] - origin).as_vec3());
                }
                m.faces.push([k, k + 1, k + 2]);
                m.colors.extend([package_colour(d.col[t[2] as usize], 1.0); 3]);
                if let Some(n) = lit_normal(t.map(|v| d.pos[v as usize]), self.bd.light) {
                    m.normals.extend([n; 3]);
                }
            }
            return Mesh::new(m);
        }
        let mut local: std::collections::HashMap<u32, u32> = std::collections::HashMap::with_capacity(tris.len());
        let mut at = |v: u32, m: &mut WireModel| -> u32 {
            *local.entry(v).or_insert_with(|| {
                m.positions.push((d.pos[v as usize] - origin).as_vec3());
                m.positions.len() as u32 - 1
            })
        };
        for t in tris {
            let f = [at(t[0], &mut m), at(t[1], &mut m), at(t[2], &mut m)];
            m.faces.push(f);
        }
        for e in edges {
            let pair = [at(e[0], &mut m), at(e[1], &mut m)];
            m.edges.push(pair);
        }
        Mesh::new(m)
    }

    /// Whether a cap (centre direction, cos of its angular radius, highest ground) has some of it above
    /// the horizon of the lowest ground for an eye at `eye`.
    pub fn above_horizon(&self, dir: DVec3, cos_a: f64, zmax: f64, eye: DVec3) -> bool {
        let r_occ = self.d.r - 11_000.0;
        let dc = eye.length();
        let hz = (r_occ / dc).min(1.0).acos();
        let top = self.d.r + zmax;
        let th = (dir.dot(eye) / dc).clamp(-1.0, 1.0).acos();
        th - cos_a.clamp(-1.0, 1.0).acos() <= hz + (r_occ / top).min(1.0).acos()
    }

    /// Whether chunk `ch` can be seen: some of it above the horizon and in the view.
    pub fn chunk_visible(&self, ch: &Chunk, cam: &Camera, aspect: f64) -> bool {
        let dir = ch.origin.normalize();
        let cos_a = 1.0 - 0.5 * (ch.radius / self.d.r).powi(2);
        let top = 6000.0 * self.ex;
        self.above_horizon(dir, cos_a, top, cam.position) && Self::in_view(cam, ch.origin, ch.radius + top, aspect)
    }

    pub fn in_view(cam: &Camera, centre: DVec3, radius: f64, aspect: f64) -> bool {
        let fwd = (cam.orientation * Vec3::NEG_Z).as_dvec3();
        let to = centre - cam.position;
        let dist = to.length();
        if dist <= radius {
            return true;
        }
        let half = ((cam.fov_y as f64 * 0.5).tan() * (1.0 + aspect * aspect).sqrt()).atan();
        let ang = (to.dot(fwd) / dist).clamp(-1.0, 1.0).acos();
        ang - (radius / dist).min(1.0).asin() <= half
    }

    /// The ground's own work for an eye: decisions, chunk and tile meshes, the pyramid's cover; `cam` is in the
    /// package's frame (its centre, unrotated, at its own radius).
    pub fn update(&mut self, cam: &Camera, aspect: f64, dt: f64) {
        let eye = cam.position;
        if self.clusters.is_none() {
            let _ = self.to_decider.send(ToDecider::Eye(eye));
        }
        let decided: Vec<Decided> = self.from_decider.lock().unwrap().try_iter().collect();
        let gn = self.grid.n;
        let (mut n_dec, mut n_chg, mut ms) = (0, 0, 0.0f64);
        for dd in decided {
            if let Some(on) = dd.on {
                self.on_snap = on;
            }
            for b in dd.stale {
                if b[0] > b[2] {
                    continue;
                }
                for u in b[0]..=b[2] {
                    for v in b[1]..=b[3] {
                        self.chunks[(u * gn + v) as usize].stale = true;
                    }
                }
            }
            self.stats.drawn = dd.drawn;
            self.lvl_count = dd.lvl_count;
            if let Some(r) = dd.risen {
                self.risen = r;
            }
            n_dec += dd.decided;
            n_chg += dd.changed;
            ms = ms.max(dd.ms);
        }
        self.stats.decided = n_dec;
        self.stats.changed = n_chg;
        self.stats.lod_ms = ms;
        if let Some(prev) = self.last_eye
            && dt > 0.0
        {
            let k = (dt / 0.25).min(1.0);
            self.vel = self.vel * (1.0 - k) + (eye - prev) / dt * k;
        }
        self.last_eye = Some(eye);
        if self.clusters.is_some() {
            self.update_clusters(eye, dt);
        }
        // Meshes again, built first and drawn after: a job's meshes all go on screen together once all are on
        // the GPU; then the next job: every stale chunk in view and every chunk not built yet, from a snapshot of
        // what is drawn (stale chunks out of view keep their mesh until they come into view).
        use rayon::prelude::*;
        let t0 = Instant::now();
        let mut rebuilt = 0;
        let view = self.view;
        if let Some(p) = &mut self.pyramid {
            let p = p.get_mut().unwrap();
            let h = f64::from_bits(self.screen_px.load(std::sync::atomic::Ordering::Relaxed));
            p.pixel = if h > 0.0 { 2.0 * (cam.fov_y as f64 * 0.5).tan() / h } else { 0.0 };
            p.update(&cam, aspect, self.ex, view == View::Real);
        }
        if view != self.last_view {
            self.last_view = view;
            self.generation += 1;
            *self.built.lock().unwrap() = None;
            self.busy = false;
        }
        if let Some(b) = self.done.lock().unwrap().try_iter().last() {
            if b.generation == self.generation {
                *self.built.lock().unwrap() = Some(b);
            }
            self.busy = false;
        }
        {
            let mut slot = self.built.lock().unwrap();
            let ready = slot.as_ref().is_some_and(|b| b.prepared == b.meshes.len() && b.meshes.iter().all(|(_, m, _)| m.gpu_resident()));
            if ready && let Some(b) = slot.take() {
                for (c, mesh, tris) in b.meshes {
                    let ch = &mut self.chunks[c];
                    ch.mesh = Some(mesh);
                    ch.mesh_view = Some(b.view);
                    ch.tri_count = tris;
                    ch.surface = Some(b.surface.clone());
                    ch.drawn_hits = Default::default();
                    rebuilt += 1;
                }
            }
        }
        if self.clusters.is_none() && !self.busy && self.built.lock().unwrap().is_none() {
            // (In view first; with nothing in view to do, chunks never built, 400 at a time, nearest first.)
            let mut chunks: Vec<(f64, usize)> = (0..self.chunks.len())
                .filter(|&c| {
                    let ch = &self.chunks[c];
                    !ch.tris.is_empty() && (ch.mesh.is_none() || ch.stale || ch.mesh_view != Some(view)) && self.chunk_visible(ch, &cam, aspect)
                })
                .map(|c| (self.chunks[c].origin.distance(cam.position), c))
                .collect();
            if chunks.is_empty() {
                chunks = (0..self.chunks.len()).filter(|&c| !self.chunks[c].tris.is_empty() && self.chunks[c].mesh.is_none()).map(|c| (self.chunks[c].origin.distance(cam.position), c)).collect();
                chunks.sort_by(|a, b| a.0.total_cmp(&b.0));
                chunks.truncate(400);
            }
            chunks.sort_by(|a, b| a.0.total_cmp(&b.0));
            if !chunks.is_empty() {
                for &(_, c) in &chunks {
                    // (Marked built now: a vertex flipping while the job is out marks it stale again.)
                    self.chunks[c].stale = false;
                }
                let job = Job { generation: self.generation, view, on: self.on_snap.clone(), bd: self.bd.clone(), chunks: chunks.into_iter().map(|(_, c)| c).collect() };
                self.busy = self.jobs.send(job).is_ok();
            }
        }
        let need: Vec<usize> = (0..self.tiles.len()).filter(|&g| self.tiles[g].mesh.as_ref().is_none_or(|(v, _)| *v != view)).collect();
        if !need.is_empty() {
            let built: Vec<(usize, Mesh)> = need.par_iter().map(|&g| (g, self.build_tiles(&self.tiles[g]))).collect();
            for (g, mesh) in built {
                self.tiles[g].mesh = Some((view, mesh));
            }
        }
        // (The layers after the tiles: built later, drawn later, so they blend over the ground drawn this frame.)
        if let Some(l) = &mut self.layers {
            let key = Some(self.ex.to_bits() ^ ((view == View::Real) as u64));
            for k in 0..l.list.len() {
                if l.on[k] && l.built[k] != key && !self.hold_layers {
                    l.meshes[k] = l.build(k, &self.d.pos, &self.d.dirs);
                    l.built[k] = key;
                }
            }
        }
        self.stats.rebuilt = rebuilt;
        self.stats.mesh_ms = t0.elapsed().as_secs_f64() * 1e3;
    }

    /// Draw the ground as the app does, the package placed by `body` (its centre, spin and scale) and seen
    /// from `cam`, the eye in the package's frame. Hands the built meshes to the engine to upload.
    pub fn draw<'a>(&'a self, frame: &mut Frame, cam: &Camera, body: &Transform) -> LandDrawn<'a> {
        let cam = *cam;
        let eye = cam.position;
        let size = frame.size();
        let aspect = (size.x / size.y.max(1.0)) as f64;
        self.screen_px.store((size.y as f64).to_bits(), std::sync::atomic::Ordering::Relaxed);
        let mesh_view = self.view == View::Mesh;
        let clear = Color([0.0; 4]);
        let edges = mesh_view && self.show_edges;
        let fill_of = |c: Color| if self.show_faces { c } else { clear };
        let (land, sea) = if mesh_view { (Color::hex(0x8a8f99), Color::hex(0x2f5fa8)) } else { (Color::WHITE, Color::WHITE) };
        let (pat_line, land_line, sea_line) = (Color::hex(0xff6a00), Color::hex(0xf0c060), Color::hex(0x6fc3ff));
        let draw = |frame: &mut Frame, mesh: &Mesh, t: &Transform, line: Color, fill: Color| {
            frame.model_shaded_faded(mesh, t, line, fill, if edges { 1.0 } else { 0.0 });
        };
        let mut drawn: Vec<(&Mesh, DVec3)> = vec![];
        let (mut chunks_drawn, mut tiles_drawn, mut sent, mut pattern_tris) = (0, 0, 0, 0);
        for g in &self.tiles {
            let Some((_, mesh)) = &g.mesh else { continue };
            if !self.above_horizon(g.dir, g.cos_a, g.zmax * self.ex, eye) {
                continue;
            }
            let radius = self.d.r * (2.0 - 2.0 * g.cos_a).max(0.0).sqrt() + (g.zmax * self.ex).max(0.0) + 12_000.0;
            if !Self::in_view(&cam, g.origin, radius, aspect) {
                continue;
            }
            let t = placed(body, g.origin);
            let (fill, line) = if g.sea { (sea, sea_line) } else { (land, land_line) };
            draw(frame, mesh, &t, line, fill_of(fill));
            tiles_drawn += 1;
            sent += g.tris.len();
            drawn.push((mesh, g.origin));
        }
        if let Some(p) = &self.pyramid {
            let mut p = p.lock().unwrap();
            p.draw(frame, eye, body, self.ex, fill_of(land), land_line, edges);
            sent += p.stats.triangles;
        }
        if let Some(l) = &self.layers {
            for k in 0..l.list.len() {
                if !l.on[k] {
                    continue;
                }
                for (mesh, origin, top) in &l.meshes[k] {
                    let r = mesh.radius() as f64;
                    if !Self::in_view(&cam, *origin, r, aspect) {
                        continue;
                    }
                    // (behind the planet: every corner beyond the horizon, as a whole group)
                    let dir = origin.normalize();
                    let cos_a = 1.0 - 0.5 * (r / top.max(1.0)).powi(2);
                    if !self.above_horizon(dir, cos_a.clamp(-1.0, 1.0), top - self.d.r, eye) {
                        continue;
                    }
                    let t = placed(body, *origin);
                    frame.model_colored_shaded(mesh, &t, 0.0, 1.0);
                }
            }
        }
        let mut drawn_clusters: Vec<u32> = vec![];
        let cl_data = self.clusters.as_ref().map(|c| { let c = c.lock().unwrap(); (c.ptr.clone(), c.tris.clone()) });
        if let Some(cl) = &self.clusters {
            let mut cl = cl.lock().unwrap();
            // (Asked again every frame until on the GPU: a frame the render thread skips loses its requests, and
            // asking once left meshes never uploaded. The engine skips those it has.)
            let mut handed = 0;
            let first = std::mem::take(&mut cl.upload_first);
            for i in first.iter().map(|&i| i as usize).chain(0..cl.meshes.len()) {
                if handed >= 3000 {
                    break;
                }
                if let Some(m) = cl.meshes[i].as_mut().filter(|m| !m.1.gpu_resident()) {
                    frame.prepare_pooled(&m.1);
                    m.2 = true;
                    handed += 1;
                }
            }
            cl.upload_first = first;
            let mut tris_shown = 0;
            // (One group for every cluster drawn: the engine draws them all in one call for faces, one for edges.)
            let group = frame.pool_group(&placed(body, DVec3::ZERO), pat_line, fill_of(land), if edges { 1.0 } else { 0.0 }, true);
            for &i in &cl.shown {
                let i = i as usize;
                let Some((_, mesh, _)) = &cl.meshes[i] else { continue };
                let o = cl.origin(i);
                if !Self::in_view(&cam, o, cl.row[i][4] as f64, aspect) {
                    continue;
                }
                if !mesh.gpu_resident() {
                    // (a cluster in view with nothing on the GPU yet: a hole this frame; should never happen)
                    log::warn!("PLANETLOD hole: cluster {i} in view not on the GPU");
                }
                frame.pooled(group, mesh, placed(body, o).position);
                drawn_clusters.push(i as u32);
                chunks_drawn += 1;
                tris_shown += cl.ptr[i][1] as usize;
            }
            sent += tris_shown;
            pattern_tris += tris_shown;
        }
        let max_draw = if self.clusters.is_some() { 0 } else { self.max_draw };
        for ch in &self.chunks {
            if chunks_drawn >= max_draw {
                break;
            }
            let Some(mesh) = &ch.mesh else { continue };
            if ch.tri_count == 0 {
                continue;
            }
            if !self.chunk_visible(ch, &cam, aspect) {
                continue;
            }
            let t = placed(body, ch.origin);
            draw(frame, mesh, &t, pat_line, fill_of(land));
            chunks_drawn += 1;
            sent += ch.tri_count;
            pattern_tris += ch.tri_count;
            drawn.push((mesh, ch.origin));
        }
        // The built job's meshes handed to the engine to upload (not drawn yet), within the frame's budget.
        if let Some(b) = self.built.lock().unwrap().as_mut() {
            let mut tris = 0;
            while b.prepared < b.meshes.len() && tris < UPLOAD_TRIS {
                let (_, mesh, n) = &b.meshes[b.prepared];
                frame.prepare_mesh(mesh);
                tris += n;
                b.prepared += 1;
            }
            // (Those handed over and still not on the GPU asked again: a skipped frame loses its requests.)
            if b.prepared == b.meshes.len() {
                for (_, mesh, _) in b.meshes.iter().filter(|(_, m, _)| !m.gpu_resident()).take(200) {
                    frame.prepare_mesh(mesh);
                }
            }
        }
        LandDrawn { drawn, sent, chunks_drawn, tiles_drawn, pattern_tris, drawn_clusters, cl_data, aspect }
    }

    /// The pyramid's counts as the app logs them (selected, drawn, pending, ...), for the game's log.
    pub fn pyramid_line(&self) -> Option<String> {
        let s = self.pyramid.as_ref()?.lock().unwrap().stats;
        Some(format!("selected={} drawn={} triangles={} pending={} cached={} bytes={} fallback={} error_ratio={:.3} resident={} largest_tile={}", s.selected, s.drawn, s.triangles, s.pending, s.cached, s.bytes, s.fallback, s.worst, s.resident, s.largest_tile))
    }
}
