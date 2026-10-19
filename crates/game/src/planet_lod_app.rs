//! Planet LOD (`docs/planet-lod-app.md`): the planet-trees pattern viewer as an app, `freefall --app
//! planetlod [VIEW_DIR]` (default `~/planet-data/views/earth_s15-pattern`). The pattern's progressive
//! mesh is drawn by chunks, never by vertices: each chunk keeps how far the eye may move before one of
//! its vertices could cross its appearing distance, and is decided again only then; a vertex that
//! changes marks the chunks its collapse subtree covers, whose meshes are made again. The planet
//! outside the pattern is static, one mesh per group of tiles, drawn when above the horizon.
//! The page's panel (`hud_kit`): altitude, ground, position, the levels of detail (badges, bar, chart),
//! the mesh counts, the view (Mesh or Real, edges, faces, crest and valleys, height x); its pins (the
//! five highest and lowest anchors) and the pattern's crest and valley lines. Stage A: a vertex appears
//! at its own height (the rise on the GPU waits for the engine's per-vertex morph band).
//! Keys as the page: W A S D fly, Q E or the wheel down and up, left and right or a right-drag turn,
//! Shift x5, a drag slides over the ground, V mesh or real, H the panel, ESC closes.
//! `UNIVERSE_PLANETLOD_AT="alt=20000&lat=-50.99&lon=71.44&heading=0&ex=1&view=real"` sets the start.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use crate::hud_kit::{self, Hit, Panel};

use universe_engine::apps::{App, Next};
use universe_engine::glam::{DVec3, Quat, Vec2, Vec3};
use universe_engine::{Camera, Color, Context, Frame, KeyCode, Light, Mesh, Morph, MouseButton, PoolModel, PooledMesh, Transform, WireModel};

/// Chunk edge (m): a chunk is one mesh.
/// Chunks hold about this many finest triangles (their edge from the pattern's density: about 15 km on the 3x3
/// grid, 2 km on Yosemite's 50 m sampling, where 8 km chunks were 30k triangles and a flip re-sent MBs).
const CHUNK_TRIS: f64 = 3000.0;
/// Decision cell edge (m): the vertices decided together (each cell keeps its own margin).
const CELL: f64 = 1000.0;
/// Mesh rebuilds a frame stop once this much time is spent (ms); the rest keep their old mesh.
/// Chunk meshes are built off the frame by a worker from a snapshot of what is drawn; a job's meshes are
/// handed to the engine to upload (UPLOAD_TRIS triangles a frame) and all go on screen together once all are
/// on the GPU, so a mixed state (some chunks new, their neighbours old: holes along long triangles, the
/// owner's black triangles when scrolling) is never drawn.
const UPLOAD_TRIS: usize = 80_000;
/// Tiles of the planet outside the pattern grouped this many to a side per mesh.
const TILE_GROUP: u32 = 4;
/// Straight down at and above this altitude, level with the horizon at the ground (as the page).
const TILT_HI: f64 = 100_000.0;
const ALT_MIN: f64 = 2.0;
/// The panel's widgets.
const ID_HIDE: u32 = 1;
const ID_MESH: u32 = 10;
const ID_EDGES: u32 = 20;
const ID_FACES: u32 = 21;
const ID_FEAT: u32 = 22;
const ID_PEAKS: u32 = 23;
/// Peak and dip pins: labels on at most this many, the nearest to the eye (posts on all).
const PEAK_LABELS: usize = 300;
const ID_EX: u32 = 30;
const ID_KEYS: u32 = 40;
const ID_VIS: u32 = 50;
/// The layer chips: ID_LAYER + the layer's index.
const ID_LAYER: u32 = 100;
/// Each layer's own lift over the ground (m), the k-th at (k + 1) x this: shells, not one surface.
const LAYER_LIFT: f64 = 1500.0;
/// A layer's faces are this opaque (75 % transparent, the owner's).
const LAYER_ALPHA: f32 = 0.25;
const PANEL_W: f32 = 284.0;

#[derive(Clone, Copy, PartialEq)]
enum View {
    Mesh,
    Real,
}

struct Data {
    r: f64,
    /// Every vertex's direction, and its place (m, planet frame) at its own height times the height x.
    dirs: Vec<DVec3>,
    pos: Vec<DVec3>,
    z: Vec<f32>,
    /// sRGB per vertex (planet source: rock on land, depth at sea).
    col: Vec<[u8; 3]>,
    /// Collapsed vertices coarse first: the vertex, its stand-in, its ring (dptr/dep), appearing distance.
    mv: Vec<u32>,
    ma: Vec<u32>,
    dptr: Vec<u32>,
    dep: Vec<u32>,
    ao: Vec<f32>,
    /// Where it must be fully in (m), and its level (0: the coarsest's).
    am: Vec<f32>,
    ml: Vec<u8>,
    /// Per vertex: its stand-in (itself if never collapsed).
    stand_in: Vec<u32>,
    /// The finest pattern triangles and edges.
    ft: Vec<[u32; 3]>,
    fe: Vec<[u32; 2]>,
}

struct Chunk {
    origin: DVec3,
    radius: f64,
    tris: Vec<u32>,
    edges: Vec<u32>,
    mesh: Option<Mesh>,
    mesh_view: Option<View>,
    stale: bool,
    tri_count: usize,
}

struct Cell {
    /// Its collapsed vertices (indices into `mv`, ascending: coarse first).
    verts: Vec<u32>,
    /// Where the eye was when it was decided, and how far it may move before deciding again.
    eye_at: DVec3,
    margin: f64,
}

/// What the files say about the pattern and the planet, for the panel.
struct Meta {
    level_d: Vec<f64>,
    level_tris: Vec<usize>,
    ahead: f64,
    margin: f64,
    px_per_m: f64,
    peak: DVec3,
    radius_m: f64,
    top_m: f64,
    land_tris: usize,
    sea_tris: usize,
    kept: usize,
    segments: usize,
    h_lo: f64,
    h_hi: f64,
    features: Vec<(String, Vec<[f64; 3]>)>,
}

/// What the last draw left for the next update (the panel's boxes) and its own counts.
#[derive(Default)]
struct Ui {
    panel: Option<Hit>,
    hits: Vec<Hit>,
    /// The last view-box count, from a worker (one at a time, every 0.3 s).
    in_view: std::sync::Arc<std::sync::atomic::AtomicUsize>,
    counting: std::sync::Arc<std::sync::atomic::AtomicBool>,
    sent: usize,
    /// The window's width over its height (the last draw's).
    aspect: f64,
    chunks_drawn: usize,
    tiles_drawn: usize,
    pattern_tris: usize,
    last_count: f64,
    vis_note: bool,
}

struct TileGroup {
    origin: DVec3,
    dir: DVec3,
    /// cos of its angular radius, highest ground (m).
    cos_a: f64,
    zmax: f64,
    tris: Vec<[u32; 3]>,
    edges: Vec<[u32; 2]>,
    sea: bool,
    mesh: Option<(View, Mesh)>,
}

#[derive(Default)]
struct Stats {
    decided: usize,
    changed: usize,
    rebuilt: usize,
    lod_ms: f64,
    mesh_ms: f64,
    /// Collapsed vertices drawn now.
    drawn: usize,
}

/// The decisions, in a thread of their own (the owner, 14 Oct: 7.7 ms a frame, 25 at worst, on Yosemite's 903k
/// vertices): it owns what is drawn, takes the eye (the latest, older ones skipped), and sends back a snapshot of
/// what is drawn, the chunks that went stale (boxes of chunks) and its counts.
struct Decider {
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

enum ToDecider {
    Eye(DVec3),
    /// New places (the height x changed): every cell decided again.
    Positions(Vec<DVec3>),
}

struct Decided {
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
struct Clusters {
    /// err, cx, cy, cz, r, perr, pcx, pcy, pcz, pr (planet metres; perr < 0: nothing above).
    row: Vec<[f32; 10]>,
    ptr: Arc<Vec<[u32; 2]>>,
    tris: Arc<Vec<[u32; 3]>>,
    /// Per corner (as `tris`): metres its parent's surface stands above it (none: no morph).
    morph: Vec<f32>,
    level: Vec<u8>,
    meshes: Vec<Option<(View, PooledMesh, bool)>>,
    shown: Vec<u32>,
    /// The clusters whose meshes go to the GPU before any other's.
    upload_first: Vec<u32>,
    /// The last frame each was wanted (shown, chosen or coming), for evicting the longest unwanted past RESIDENT.
    used: Vec<u64>,
    frame: u64,
    /// metres of error a pixel covers per metre away (1080 lines, 50 degrees)
    px: f64,
    stats: [usize; 5],
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
fn cluster_mesh(cl: &Clusters, i: usize, pos: &[DVec3], col: &[[u8; 3]], view: View) -> PooledMesh {
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
const RESIDENT: usize = 16_000;

/// What a chunk mesh is built from, shared with the build worker (made again when the height x changes).
struct BuildData {
    pos: Vec<DVec3>,
    col: Vec<[u8; 3]>,
    ft: Vec<[u32; 3]>,
    fe: Vec<[u32; 2]>,
    stand_in: Vec<u32>,
    chunk_tris: Vec<Vec<u32>>,
    chunk_edges: Vec<Vec<u32>>,
    origin: Vec<DVec3>,
}

struct Job {
    generation: u64,
    view: View,
    on: Arc<Vec<u8>>,
    bd: Arc<BuildData>,
    chunks: Vec<usize>,
}

/// A job's meshes, built: waiting to be uploaded, then put on screen together.
struct Built {
    generation: u64,
    view: View,
    meshes: Vec<(usize, Mesh, usize)>,
    /// How many of them have been handed to the engine to upload.
    prepared: usize,
}

/// A chunk's mesh from a snapshot `on` of what is drawn: its finest triangles and edges through their drawn
/// stand-ins, each kept once (many resolve onto the same one). Mesh view: corners shared, edges indexing them;
/// Real: flat faces in their last corner's colour, no edges.
fn build_mesh(bd: &BuildData, on: &[u8], c: usize, view: View) -> (Mesh, usize) {
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
            let rgb = bd.col[t[2] as usize].map(srgb_to_linear);
            m.colors.extend([[rgb[0], rgb[1], rgb[2], 1.0]; 3]);
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

pub struct PlanetLodApp {
    dir: PathBuf,
    d: Data,
    chunks: Vec<Chunk>,
    grid: Grid,
    tiles: Vec<TileGroup>,
    view: View,
    hud: bool,
    lon: f64,
    lat: f64,
    alt: f64,
    heading: f64,
    stats: Stats,
    /// The cluster DAG, when the view has one (in place of the decisions and the chunks).
    clusters: Option<Mutex<Clusters>>,
    /// The eye's smoothed velocity (m/s), for the clusters' preload a second ahead, and where it was.
    vel: DVec3,
    last_eye: Option<DVec3>,
    /// The decider: its inbox and what it sends back; the latest snapshot of what is drawn.
    to_decider: std::sync::mpsc::Sender<ToDecider>,
    from_decider: Mutex<std::sync::mpsc::Receiver<Decided>>,
    on_snap: Arc<Vec<u8>>,
    /// The build worker: its inbox, what it sends back, whether a job is out, what jobs are built from, and the
    /// generation (height x and view: an older one's results are dropped); the job built and going up.
    jobs: std::sync::mpsc::Sender<Job>,
    done: Mutex<std::sync::mpsc::Receiver<Built>>,
    busy: bool,
    bd: Arc<BuildData>,
    generation: u64,
    last_view: View,
    built: Mutex<Option<Built>>,
    /// The wheel's target altitude, eased to.
    alt_target: f64,
    easing: bool,
    /// UNIVERSE_PLANETLOD_FLY: the camera descends and drifts by itself (captures ignore the keys);
    /// =pan: it slides sideways at its altitude instead.
    fly: bool,
    pan: bool,
    /// UNIVERSE_PLANETLOD_MAXDRAW: at most this many chunks drawn (to tell a draw's cost from a triangle's).
    max_draw: usize,
    meta: Meta,
    ex: f64,
    show_edges: bool,
    show_faces: bool,
    show_feat: bool,
    /// Pins on every peak and dip with its height (peaks.json beside the view: [{lon, lat, z, kind}]).
    show_peaks: bool,
    peak_pins: Vec<(DVec3, f64, String, bool)>,
    keys_open: bool,
    /// The slider being dragged, a drag over the ground (its button), the ground under the eye (m, drawn).
    sliding: bool,
    drag: Option<MouseButton>,
    ground_h: Option<f64>,
    /// Drawn collapsed vertices per level; the last full count of those fully in and rising.
    lvl_count: Vec<usize>,
    risen: (usize, usize),
    /// (altitude, drawn share, fully-in share) as flown, for the chart.
    history: std::collections::VecDeque<[f32; 3]>,
    last_hist: f64,
    feat: Option<Mesh>,
    pins: Vec<(DVec3, f64, String, bool)>,
    /// The landform layers over the land, when the view has them.
    layers: Option<Layers>,
    ui: Mutex<Ui>,
    /// Sums for the log line written each second: frames, seconds, LOD ms, mesh ms, max of each, rebuilt.
    log: [f64; 7],
}

/// The chunk grid on the plane tangent at the peak.
struct Grid {
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

fn read<T: Copy>(dir: &Path, name: &str) -> Result<Vec<T>, String> {
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

fn json(dir: &Path, name: &str) -> Result<serde_json::Value, String> {
    serde_json::from_slice(&std::fs::read(dir.join(name)).map_err(|e| format!("{name}: {e}"))?).map_err(|e| format!("{name}: {e}"))
}

fn unit(lon: f64, lat: f64) -> DVec3 {
    let (lo, la) = (lon.to_radians(), lat.to_radians());
    DVec3::new(la.cos() * lo.sin(), la.sin(), la.cos() * lo.cos())
}

/// 1234567 as 1,234,567.
fn thousands(x: f64) -> String {
    let s = format!("{}", x.abs() as u64);
    let mut out = String::new();
    for (k, ch) in s.chars().enumerate() {
        if k > 0 && (s.len() - k) % 3 == 0 {
            out.push(',');
        }
        out.push(ch);
    }
    if x < 0.0 { format!("-{out}") } else { out }
}

fn kilo(x: usize) -> String {
    if x >= 1_000_000 { format!("{:.2}M", x as f64 / 1e6) } else { format!("{}k", (x as f64 / 1e3).round()) }
}

/// One landform layer over the land (views' `layers/`, tools/layer_meshes.py): a class for every land triangle.
struct Layer {
    title: String,
    names: Vec<String>,
    rgb: Vec<[u8; 3]>,
    share: Vec<f32>,
    cls: Vec<u16>,
}

/// The layer meshes (owner 16 Oct): each layer its own mesh over the land, in its classes' colours, 75 %
/// transparent, lifted a little more than the one under it; switched on and off in the panel. The land triangles
/// index the view's vertices (the coast carved in the source), so a layer stands where the ground is drawn.
struct Layers {
    tri: Vec<[u32; 3]>,
    /// Land triangles grouped by where they are (cube-face cells): each group's centre direction, cos of its
    /// angular radius, its triangles.
    groups: Vec<(DVec3, f64, Vec<u32>)>,
    list: Vec<Layer>,
    on: Vec<bool>,
    /// Built meshes per layer (group's origin), and the (view, height x) they were built for.
    meshes: Vec<Vec<(Mesh, DVec3, f64)>>,
    built: Vec<Option<u64>>,
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
        let col: Vec<[f32; 4]> = l.rgb.iter().map(|c| [srgb_to_linear(c[0]), srgb_to_linear(c[1]), srgb_to_linear(c[2]), LAYER_ALPHA]).collect();
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

fn srgb_to_linear(c: u8) -> f32 {
    let c = c as f32 / 255.0;
    if c <= 0.04045 { c / 12.92 } else { ((c + 0.055) / 1.055).powf(2.4) }
}

impl PlanetLodApp {
    pub fn new(dir: Option<&str>) -> Result<Self, String> {
        let t0 = Instant::now();
        let dir = dir.map(PathBuf::from).unwrap_or_else(|| PathBuf::from(std::env::var("HOME").unwrap_or_default()).join("planet-data/views/earth_s15-pattern"));
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
        let d = Data { r, dirs: unit_dirs, pos, z, col, mv, ma, dptr, dep, ao, am, ml, stand_in, ft, fe };
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
        let mut chunks: Vec<Chunk> = (0..gn * gn).map(|_| Chunk { origin: DVec3::ZERO, radius: 0.0, tris: vec![], edges: vec![], mesh: None, mesh_view: None, stale: true, tri_count: 0 }).collect();
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
        let tiles = Self::tile_groups(&dir, &d)?;
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
        // Pins: the five highest and the five lowest anchors of the planet graph.
        let mut anchors: Vec<(f64, f64, f64)> = json(&dir, "anchors.json").ok().and_then(|a| a.as_array().cloned()).unwrap_or_default().iter().map(|a| (num(&a["lon"]), num(&a["lat"]), num(&a["z"]))).collect();
        anchors.sort_by(|a, b| b.2.total_cmp(&a.2));
        let mut pins = vec![];
        for (k, a) in anchors.iter().take(5).enumerate() {
            pins.push((unit(a.0, a.1), a.2, format!("^ {} - {} m", k + 1, thousands(a.2.round())), true));
        }
        for (k, a) in anchors.iter().rev().take(5).enumerate() {
            pins.push((unit(a.0, a.1), a.2, format!("v {} - {} m", k + 1, thousands(a.2.round())), false));
        }
        // Named places (labels.json beside the view, optional: [{lon, lat, z, text}]), e.g. the generated and the
        // real disc of a side-by-side comparison.
        for l in json(&dir, "labels.json").ok().and_then(|l| l.as_array().cloned()).unwrap_or_default() {
            pins.push((unit(num(&l["lon"]), num(&l["lat"])), num(&l["z"]), l["text"].as_str().unwrap_or("").to_string(), true));
        }
        // Every peak and dip of the anchor model with its height (peaks.json beside the view, optional).
        let peak_pins: Vec<(DVec3, f64, String, bool)> = json(&dir, "peaks.json").ok().and_then(|a| a.as_array().cloned()).unwrap_or_default().iter()
            .map(|a| {
                let (z, peak) = (num(&a["z"]), a["kind"].as_str() != Some("dip"));
                (unit(num(&a["lon"]), num(&a["lat"])), z, format!("{} {} m", if peak { "^" } else { "v" }, thousands(z.round())), peak)
            })
            .collect();
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
        });
        let (jobs, inbox) = std::sync::mpsc::channel::<Job>();
        let (outbox, done) = std::sync::mpsc::channel::<Built>();
        std::thread::Builder::new().name("planetlod build".into()).spawn(move || {
            use rayon::prelude::*;
            while let Ok(job) = inbox.recv() {
                let meshes: Vec<(usize, Mesh, usize)> = job.chunks.par_iter().map(|&c| {
                    let (m, n) = build_mesh(&job.bd, &job.on, c, job.view);
                    (c, m, n)
                }).collect();
                if outbox.send(Built { generation: job.generation, view: job.view, meshes, prepared: 0 }).is_err() {
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
        let mut app = PlanetLodApp {
            dir,
            d,
            chunks,
            grid,
            clusters,
            vel: DVec3::ZERO,
            last_eye: None,
            to_decider,
            from_decider: Mutex::new(from_decider),
            on_snap: on_snap.clone(),
            tiles,
            view: View::Mesh,
            hud: true,
            lon: peak["lon"].as_f64().unwrap_or(0.0),
            lat: peak["lat"].as_f64().unwrap_or(0.0),
            alt: 20_000.0,
            heading: 0.0,
            stats: Stats::default(),
            jobs,
            done: Mutex::new(done),
            busy: false,
            bd,
            generation: 0,
            last_view: View::Mesh,
            built: Mutex::new(None),
            alt_target: 20_000.0,
            easing: false,
            fly: std::env::var_os("UNIVERSE_PLANETLOD_FLY").is_some(),
            pan: std::env::var("UNIVERSE_PLANETLOD_FLY").is_ok_and(|v| v == "pan"),
            max_draw: std::env::var("UNIVERSE_PLANETLOD_MAXDRAW").ok().and_then(|v| v.parse().ok()).unwrap_or(usize::MAX),
            meta,
            ex: 1.0,
            show_edges: true,
            show_faces: true,
            show_feat: false,
            show_peaks: false,
            peak_pins,
            keys_open: false,
            sliding: false,
            drag: None,
            ground_h: None,
            lvl_count: vec![0; 256],
            risen: (0, 0),
            history: Default::default(),
            last_hist: 0.0,
            feat: None,
            pins,
            layers,
            ui: Mutex::new(Ui::default()),
            log: [0.0; 7],
        };
        if let Ok(at) = std::env::var("UNIVERSE_PLANETLOD_AT") {
            for kv in at.split('&') {
                let mut p = kv.splitn(2, '=');
                let (k, v) = (p.next().unwrap_or(""), p.next().and_then(|v| v.parse::<f64>().ok()));
                match (k, v) {
                    ("alt", Some(v)) => app.alt = v,
                    ("lat", Some(v)) => app.lat = v,
                    ("lon", Some(v)) => app.lon = v,
                    ("heading", Some(v)) => app.heading = v,
                    ("ex", Some(v)) => app.ex = v.clamp(1.0, 50.0),
                    ("feat", Some(v)) => app.show_feat = v > 0.0,
                    ("peaks", Some(v)) => app.show_peaks = v > 0.0,
                    ("view", _) if kv.ends_with("real") => app.view = View::Real,
                    ("layers", _) => {
                        // (layers=0,2 or layers=none: which layer meshes are on)
                        if let Some(l) = &mut app.layers {
                            let want: Vec<usize> = kv[7..].split(',').filter_map(|x| x.parse().ok()).collect();
                            for k in 0..l.on.len() {
                                l.on[k] = want.contains(&k);
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
        app.alt_target = app.alt;
        if app.ex != 1.0 {
            app.set_ex(app.ex);
        }
        log::info!(
            "planet LOD: {} vertices ({} collapsed), {} finest triangles in {} chunks, {} tile groups, read in {:.1} s from {}",
            app.d.pos.len(),
            app.d.mv.len(),
            app.d.ft.len(),
            app.chunks.iter().filter(|c| !c.tris.is_empty()).count(),
            app.tiles.len(),
            t0.elapsed().as_secs_f64(),
            app.dir.display()
        );
        Ok(app)
    }

    /// The planet outside the pattern: its tiles (land and sea apart) grouped TILE_GROUP to a side.
    fn tile_groups(dir: &Path, d: &Data) -> Result<Vec<TileGroup>, String> {
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
                tris.extend((s0..s0 + n).map(|t| [tri[3 * t], tri[3 * t + 1], tri[3 * t + 2]]));
                edges.extend((e0..e0 + en).map(|e| [te[2 * e], te[2 * e + 1]]));
                dir_sum += DVec3::new(tf[6 * j] as f64, tf[6 * j + 1] as f64, tf[6 * j + 2] as f64);
            }
            let dir = dir_sum.normalize();
            let (mut cos_a, mut zmax) = (1.0f64, f64::NEG_INFINITY);
            for t in &tris {
                for &v in t {
                    cos_a = cos_a.min(d.pos[v as usize].normalize().dot(dir));
                    zmax = zmax.max(d.z[v as usize] as f64);
                }
            }
            out.push(TileGroup { origin: dir * d.r, dir, cos_a: cos_a - 1e-6, zmax, tris, edges, sea: sea == 1, mesh: None });
        }
        Ok(out)
    }

    fn eye(&self) -> DVec3 {
        let up = unit(self.lon, self.lat);
        up * (self.d.r + self.ground_h.unwrap_or(0.0) * self.ex + self.alt)
    }

    /// The drawn ground under direction `u` (m, before the height x): the drawn triangle it is in (the
    /// pattern's through their stand-ins, else the planet's), its corners' heights blended.
    fn ground_at(&self, u: DVec3) -> Option<f64> {
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
                for &t in &self.chunks[(x * self.grid.n + y) as usize].tris {
                    // (The finest surface, not the drawn: a change of detail must not move the eye.)
                    if let Some(h) = hit(d.ft[t as usize]) {
                        return Some(h);
                    }
                }
            }
        }
        for g in &self.tiles {
            if u.dot(g.dir) >= g.cos_a {
                for &t in &g.tris {
                    if let Some(h) = hit(t) {
                        return Some(h);
                    }
                }
            }
        }
        None
    }

    /// A new height x: every place again, every mesh and decision made again.
    fn set_ex(&mut self, ex: f64) {
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
        self.feat = None;
        let old = self.bd.clone();
        self.bd = Arc::new(BuildData { pos: self.d.pos.clone(), col: old.col.clone(), ft: old.ft.clone(), fe: old.fe.clone(), stand_in: old.stand_in.clone(), chunk_tris: old.chunk_tris.clone(), chunk_edges: old.chunk_edges.clone(), origin: old.origin.clone() });
        self.generation += 1;
        *self.built.lock().unwrap() = None;
    }

    /// The crest and valley lines, 3 m over the ground, coloured by their role.
    fn build_features(&self) -> Mesh {
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
    fn update_clusters(&mut self, eye: DVec3, _dt: f64) {
        use rayon::prelude::*;
        let t0 = Instant::now();
        let view = self.view;
        let ahead = {
            let mut step = self.vel * 1.0;
            let most = 0.5 * self.alt.max(ALT_MIN);
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

    fn build_tiles(&self, g: &TileGroup) -> Mesh {
        let edges: &[[u32; 2]] = if self.view == View::Mesh { &g.edges } else { &[] };
        self.model(g.origin, &g.tris, edges)
    }

    /// A mesh of planet vertices about `origin`. Mesh view: the corners shared, the edges indexing
    /// them (the engine finds an edge's normal by its ends); Real view: each face its own corners in
    /// its last corner's colour (the page's flat colour), no edges.
    fn model(&self, origin: DVec3, tris: &[[u32; 3]], edges: &[[u32; 2]]) -> Mesh {
        let d = &self.d;
        let mut m = WireModel::default();
        if self.view == View::Real {
            for t in tris {
                let k = m.positions.len() as u32;
                for &v in t {
                    m.positions.push((d.pos[v as usize] - origin).as_vec3());
                }
                m.faces.push([k, k + 1, k + 2]);
                let rgb = d.col[t[2] as usize].map(srgb_to_linear);
                m.colors.extend([[rgb[0], rgb[1], rgb[2], 1.0]; 3]);
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
    fn above_horizon(&self, dir: DVec3, cos_a: f64, zmax: f64, eye: DVec3) -> bool {
        let r_occ = self.d.r - 11_000.0;
        let dc = eye.length();
        let hz = (r_occ / dc).min(1.0).acos();
        let top = self.d.r + zmax;
        let th = (dir.dot(eye) / dc).clamp(-1.0, 1.0).acos();
        th - cos_a.clamp(-1.0, 1.0).acos() <= hz + (r_occ / top).min(1.0).acos()
    }

    /// Whether chunk `ch` can be seen: some of it above the horizon and in the view.
    fn chunk_visible(&self, ch: &Chunk, cam: &Camera, aspect: f64) -> bool {
        let dir = ch.origin.normalize();
        let cos_a = 1.0 - 0.5 * (ch.radius / self.d.r).powi(2);
        let top = 6000.0 * self.ex;
        self.above_horizon(dir, cos_a, top, cam.position) && Self::in_view(cam, ch.origin, ch.radius + top, aspect)
    }

    fn in_view(cam: &Camera, centre: DVec3, radius: f64, aspect: f64) -> bool {
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
}

impl App for PlanetLodApp {
    fn name(&self) -> &str {
        "planetlod"
    }

    /// For the observer's /status: where the eye is and what the levels of detail are doing.
    fn status(&self) -> String {
        let ui = self.ui.lock().unwrap();
        serde_json::json!({
            "view": if self.view == View::Mesh { "mesh" } else { "real" },
            "altitude_m": self.alt, "lat": self.lat, "lon": self.lon, "heading_deg": self.heading.rem_euclid(360.0),
            "ground_m": self.ground_h, "height_x": self.ex,
            "pattern_vertices_drawn": self.stats.drawn, "pattern_vertices": self.d.mv.len(),
            "fully_risen": self.risen.0, "rising": self.risen.1,
            "lod_ms": self.stats.lod_ms, "cells_decided": self.stats.decided, "vertices_changed": self.stats.changed,
            "mesh_ms": self.stats.mesh_ms, "chunks_rebuilt": self.stats.rebuilt,
            "chunks_drawn": ui.chunks_drawn, "tile_groups_drawn": ui.tiles_drawn,
            "triangles_sent": ui.sent, "pattern_triangles_drawn": ui.pattern_tris,
            "in_view_box": ui.in_view.load(std::sync::atomic::Ordering::Acquire),
        }).to_string()
    }

    fn update(&mut self, ctx: &mut Context) -> Next {
        let _scope = universe_prof::scope("update/planetlod");
        let input = &ctx.input;
        if input.pressed(KeyCode::Escape) {
            return Next::Close;
        }
        if input.pressed(KeyCode::KeyV) {
            self.view = if self.view == View::Mesh { View::Real } else { View::Mesh };
        }
        if input.pressed(KeyCode::KeyH) {
            self.hud = !self.hud;
        }
        let dt = (ctx.dt as f64).min(0.1);
        // The panel: what the last draw laid out, under the cursor.
        let (over_panel, hit) = {
            let ui = self.ui.lock().unwrap();
            let over = self.hud && ui.panel.is_some_and(|p| p.contains(input.cursor));
            let hit = if self.hud { ui.hits.iter().find(|h| h.contains(input.cursor)).copied() } else { None };
            (over, hit)
        };
        if input.button_pressed(MouseButton::Left) {
            match hit.map(|h| h.id) {
                Some(ID_HIDE) => self.hud = false,
                Some(ID_MESH) => self.view = View::Mesh,
                Some(x) if x == ID_MESH + 1 => self.view = View::Real,
                Some(ID_EDGES) => self.show_edges = !self.show_edges,
                Some(ID_FACES) => self.show_faces = !self.show_faces,
                Some(ID_FEAT) => self.show_feat = !self.show_feat,
                Some(ID_PEAKS) => self.show_peaks = !self.show_peaks,
                Some(ID_KEYS) => self.keys_open = !self.keys_open,
                Some(ID_VIS) => self.ui.lock().unwrap().vis_note = true,
                Some(ID_EX) => self.sliding = true,
                Some(x) if (ID_LAYER..ID_LAYER + 64).contains(&x) => {
                    if let Some(l) = &mut self.layers {
                        let k = (x - ID_LAYER) as usize;
                        if k < l.on.len() {
                            l.on[k] = !l.on[k];
                        }
                    }
                }
                _ => {}
            }
            if !over_panel {
                self.drag = Some(MouseButton::Left);
            }
        }
        if input.button_pressed(MouseButton::Right) && !over_panel {
            self.drag = Some(MouseButton::Right);
        }
        if !input.button_down(MouseButton::Left) {
            self.sliding = false;
            if self.drag == Some(MouseButton::Left) {
                self.drag = None;
            }
        }
        if !input.button_down(MouseButton::Right) && self.drag == Some(MouseButton::Right) {
            self.drag = None;
        }
        if self.sliding {
            let hits = self.ui.lock().unwrap().hits.clone();
            if let Some(h) = hits.iter().find(|h| h.id == ID_EX) {
                let k = ((input.cursor.x - h.min.x - 4.0) / (h.size.x - 8.0)).clamp(0.0, 1.0);
                let ex = (1.0 + 49.0 * k as f64).round();
                if ex != self.ex {
                    self.set_ex(ex);
                }
            }
        }
        let shift = input.down(KeyCode::ShiftLeft) || input.down(KeyCode::ShiftRight);
        let r = self.d.r;
        // A drag slides over the ground (the ground follows the pointer); a right-drag or Shift+drag turns.
        let md = input.mouse_delta;
        match self.drag {
            Some(MouseButton::Right) => self.heading += (md.x as f64 * 0.005).to_degrees(),
            Some(MouseButton::Left) if shift => self.heading += (md.x as f64 * 0.005).to_degrees(),
            Some(MouseButton::Left) if !self.sliding => {
                let k = 1.2 * self.alt.max(50.0) / r / 810.0;
                let h = self.heading.to_radians();
                let (dn, de) = (md.y as f64 * k, -md.x as f64 * k);
                self.lat = (self.lat + (dn * h.cos() - de * h.sin()).to_degrees()).clamp(-88.8, 88.8);
                self.lon += ((dn * h.sin() + de * h.cos()) / self.lat.to_radians().cos().max(0.05)).to_degrees();
            }
            _ => {}
        }
        // (The wheel sets where to go and the altitude eases there in about 0.15 s: a step of the wheel was a
        // jump of the whole view in one frame.)
        if input.scroll != 0.0 && !over_panel {
            self.alt_target = ((if self.easing { self.alt_target } else { self.alt }) * (-0.15 * input.scroll as f64).exp()).clamp(ALT_MIN, 6.0 * r);
            self.easing = true;
        }
        if self.easing {
            let k = 1.0 - (-dt / 0.15).exp();
            self.alt = (self.alt.ln() + (self.alt_target.ln() - self.alt.ln()) * k).exp();
            if (self.alt / self.alt_target - 1.0).abs() < 1e-3 {
                self.alt = self.alt_target;
                self.easing = false;
            }
        }
        // W A S D fly at a speed that crosses the view's width in about four seconds; Q E down and up.
        let boost = if shift { 5.0 } else { 1.0 };
        let v = boost * 0.5 * self.alt.max(30.0) / r * dt;
        let (f, s) = (input.axis(KeyCode::KeyS, KeyCode::KeyW) as f64, input.axis(KeyCode::KeyA, KeyCode::KeyD) as f64);
        if f != 0.0 || s != 0.0 {
            let h = self.heading.to_radians();
            let (dn, de) = ((f * h.cos() - s * h.sin()) * v, (f * h.sin() + s * h.cos()) * v);
            self.lat = (self.lat + dn.to_degrees()).clamp(-88.8, 88.8);
            self.lon += (de / self.lat.to_radians().cos().max(0.05)).to_degrees();
        }
        let climb = input.axis(KeyCode::KeyQ, KeyCode::KeyE) as f64;
        if climb != 0.0 {
            self.easing = false;
            self.alt = (self.alt * (1.2 * boost * dt * climb).exp()).clamp(ALT_MIN, 6.0 * r);
        }
        self.heading += input.axis(KeyCode::ArrowLeft, KeyCode::ArrowRight) as f64 * (1.2 * dt).to_degrees();
        if self.pan {
            self.lon += 0.25 * (ctx.time * 0.4).sin();
        } else if self.fly {
            self.alt = (self.alt * 0.995).max(300.0);
            self.lon += 2e-4;
        }
        let eye = self.eye();
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
        let cam = self.camera();
        let aspect = match self.ui.lock().unwrap().aspect { a if a > 0.0 => a, _ => 16.0 / 9.0 };
        let view = self.view;
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
                if l.on[k] && l.built[k] != key && !self.sliding {
                    l.meshes[k] = l.build(k, &self.d.pos, &self.d.dirs);
                    l.built[k] = key;
                }
            }
        }
        if self.show_feat && self.feat.is_none() && !self.meta.features.is_empty() {
            self.feat = Some(self.build_features());
        }
        self.stats.rebuilt = rebuilt;
        self.stats.mesh_ms = t0.elapsed().as_secs_f64() * 1e3;
        self.ground_h = self.ground_at(unit(self.lon, self.lat));
        let n = self.d.mv.len();
        if ctx.time - self.last_hist > 0.15 {
            self.last_hist = ctx.time;
            let nm = n.max(1) as f32;
            self.history.push_back([self.alt as f32, self.stats.drawn as f32 / nm, self.risen.0 as f32 / nm]);
            if self.history.len() > 5000 {
                self.history.pop_front();
            }
        }
        let l = &mut self.log;
        l[0] += 1.0;
        l[1] += dt;
        l[2] += self.stats.lod_ms;
        l[3] += self.stats.mesh_ms;
        l[4] = l[4].max(self.stats.lod_ms);
        l[5] = l[5].max(self.stats.mesh_ms);
        l[6] += rebuilt as f64;
        if l[1] >= 1.0 {
            log::info!("PLANETLOD alt_m={:.0} fps={:.1} lod_ms={:.3} (max {:.2}) mesh_ms={:.3} (max {:.2}) rebuilt_per_s={:.0} drawn={}", self.alt, l[0] / l[1], l[2] / l[0], l[4], l[3] / l[0], l[5], l[6] / l[1], self.stats.drawn);
            *l = [0.0; 7];
        }
        Next::Stay
    }

    fn camera(&self) -> Camera {
        let up = unit(self.lon, self.lat);
        let east = DVec3::Y.cross(up).normalize_or(DVec3::X);
        let north = up.cross(east);
        let h = self.heading.to_radians();
        let ahead = north * h.cos() + east * h.sin();
        let p = self.pitch_down().to_radians();
        let fwd = ahead * p.cos() - up * p.sin();
        let cam_up = up * p.cos() + ahead * p.sin();
        let position = self.eye();
        let mut camera = Camera { position, fov_y: 50f32.to_radians(), near: (0.2 * self.alt).clamp(0.3, 2000.0) as f32, ..Default::default() };
        camera.look_at(position + fwd, cam_up.as_vec3());
        camera
    }

    fn draw(&self, frame: &mut Frame, ctx: &Context) {
        let cam = frame.camera;
        let eye = cam.position;
        let up = eye.normalize();
        // Light for reading shape: the sun from the north-west 35 degrees up, wherever the eye is.
        let east = DVec3::Y.cross(up).normalize_or(DVec3::X);
        let north = up.cross(east);
        let sun = (up * 0.574 + north * 0.579 - east * 0.579).normalize();
        frame.light = Some(Light { position: eye + sun * 1.5e11, color: [1.0, 0.98, 0.94], luminosity: 1.0, reference: 1.5e11, radius: 7e8 });
        let size = frame.size();
        let aspect = (size.x / size.y.max(1.0)) as f64;
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
            let t = Transform { position: g.origin, rotation: Quat::IDENTITY, scale: 1.0 };
            let (fill, line) = if g.sea { (sea, sea_line) } else { (land, land_line) };
            draw(frame, mesh, &t, line, fill_of(fill));
            tiles_drawn += 1;
            sent += g.tris.len();
            drawn.push((mesh, g.origin));
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
                    let t = Transform { position: *origin, rotation: Quat::IDENTITY, scale: 1.0 };
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
            let group = frame.pool_group(&Transform { position: DVec3::ZERO, rotation: Quat::IDENTITY, scale: 1.0 }, pat_line, fill_of(land), if edges { 1.0 } else { 0.0 }, true);
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
                frame.pooled(group, mesh, o);
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
            let t = Transform { position: ch.origin, rotation: Quat::IDENTITY, scale: 1.0 };
            draw(frame, mesh, &t, pat_line, fill_of(land));
            chunks_drawn += 1;
            sent += ch.tri_count;
            pattern_tris += ch.tri_count;
            drawn.push((mesh, ch.origin));
        }
        if self.show_feat && let Some(f) = &self.feat {
            let t = Transform { position: self.meta.peak * self.d.r, rotation: Quat::IDENTITY, scale: 1.0 };
            frame.model_colored(f, &t, 1.0, 0.0);
        }
        // Pins: a post from the ground and its label.
        let alt_r = (eye.length() - self.d.r).max(0.0);
        let post = (0.08 * alt_r).max(0.004 * self.d.r).min(0.08 * alt_r.max(1.0));
        for (dir, z, label, peak) in &self.pins {
            let base = *dir * (self.d.r + self.ex * z);
            let top = base + *dir * post;
            if dir.dot(eye - base) <= 0.0 {
                continue;
            }
            let c = if *peak { Color::rgb(1.0, 0.45, 0.31) } else { Color::rgb(0.45, 0.85, 1.0) };
            frame.line(base, top, c);
            if let Some(p) = frame.project(top)
                && p.x > 0.0 && p.y > 0.0 && p.x < size.x && p.y < size.y
            {
                let w = label.len() as f32 * universe_engine::GLYPH;
                frame.text_boxed(p - Vec2::new(w * 0.5, universe_engine::GLYPH + 6.0), label, Color::hex(0x111111), c);
            }
        }
        // Peak and dip pins: a post on each one facing the eye, its height on the nearest PEAK_LABELS on screen.
        if self.show_peaks {
            let mut shown: Vec<(f64, Vec2, usize)> = vec![];
            for (i, (dir, z, _, peak)) in self.peak_pins.iter().enumerate() {
                let base = *dir * (self.d.r + self.ex * z);
                if dir.dot(eye - base) <= 0.0 {
                    continue;
                }
                let top = base + *dir * post;
                // (only those in view: the post's foot and top both on screen; owner 17 Oct)
                let on = |q: Option<Vec2>| q.filter(|p| p.x > 0.0 && p.y > 0.0 && p.x < size.x && p.y < size.y);
                if let (Some(_), Some(p)) = (on(frame.project(base)), on(frame.project(top))) {
                    let c = if *peak { Color::rgb(1.0, 0.45, 0.31) } else { Color::rgb(0.45, 0.85, 1.0) };
                    frame.line(base, top, c);
                    shown.push(((eye - top).length(), p, i));
                }
            }
            shown.sort_by(|a, b| a.0.total_cmp(&b.0));
            for (_, p, i) in shown.into_iter().take(PEAK_LABELS) {
                let (_, _, label, peak) = &self.peak_pins[i];
                let c = if *peak { Color::rgb(1.0, 0.45, 0.31) } else { Color::rgb(0.45, 0.85, 1.0) };
                let w = label.len() as f32 * universe_engine::GLYPH;
                frame.text_boxed(p - Vec2::new(w * 0.5, universe_engine::GLYPH + 6.0), label, Color::hex(0x111111), c);
            }
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
        // The counts the panel shows that need the drawn meshes: the view box every 0.3 s.
        let mut ui = self.ui.lock().unwrap();
        ui.sent = sent;
        ui.aspect = aspect;
        ui.chunks_drawn = chunks_drawn;
        ui.tiles_drawn = tiles_drawn;
        ui.pattern_tris = pattern_tris;
        use std::sync::atomic::Ordering;
        if ctx.time - ui.last_count > 0.3 && !ui.counting.load(Ordering::Acquire) {
            ui.last_count = ctx.time;
            ui.counting.store(true, Ordering::Release);
            let meshes: Vec<(Mesh, DVec3)> = drawn.iter().map(|(m, o)| ((*m).clone(), *o)).collect();
            let (out, busy) = (ui.in_view.clone(), ui.counting.clone());
            let aspect = aspect as f32;
            // (Pooled clusters keep no triangles the app can read back: counted from the clusters' own lists.)
            let bd = self.bd.clone();
            std::thread::spawn(move || {
                let refs: Vec<(&Mesh, DVec3)> = meshes.iter().map(|(m, o)| (m, *o)).collect();
                let mut k = view_box_count(&cam, aspect, &refs);
                if let Some((ptr, tris)) = cl_data {
                    k += view_box_count_tris(&cam, aspect, &bd.pos, drawn_clusters.iter().flat_map(|&i| { let [a, n] = ptr[i as usize]; tris[a as usize..(a + n) as usize].iter().copied() }));
                }
                out.store(k, Ordering::Release);
                busy.store(false, Ordering::Release);
            });
        }
        if !self.hud {
            ui.panel = None;
            ui.hits.clear();
            return;
        }
        let (panel, hits) = self.panel(frame, ctx, &ui);
        ui.panel = Some(panel);
        ui.hits = hits;
    }
}

/// Triangles of the drawn meshes not wholly outside one clip plane and facing the eye (what the GPU
/// rasterizes), as the page's 'in the view box'.
/// The same count for triangles given by planet vertex ids.
fn view_box_count_tris(cam: &Camera, aspect: f32, pos: &[DVec3], tris: impl Iterator<Item = [u32; 3]>) -> usize {
    let vp = cam.view_proj(aspect);
    let mut k = 0;
    for t in tris {
        let [a, b, c] = t.map(|v| vp * (pos[v as usize] - cam.position).as_vec3().extend(1.0));
        let mut out = false;
        for ax in 0..3 {
            if (a[ax] > a.w && b[ax] > b.w && c[ax] > c.w) || (-a[ax] > a.w && -b[ax] > b.w && -c[ax] > c.w) {
                out = true;
                break;
            }
        }
        if !out && a.w > 0.0 && b.w > 0.0 && c.w > 0.0 {
            let (ax, ay, bx, by, cx, cy) = (a.x / a.w, a.y / a.w, b.x / b.w, b.y / b.w, c.x / c.w, c.y / c.w);
            if (bx - ax) * (cy - ay) - (by - ay) * (cx - ax) <= 0.0 {
                out = true;
            }
        }
        if !out {
            k += 1;
        }
    }
    k
}

fn view_box_count(cam: &Camera, aspect: f32, drawn: &[(&Mesh, DVec3)]) -> usize {
    let vp = cam.view_proj(aspect);
    let mut k = 0;
    for (mesh, origin) in drawn {
        let off = (*origin - cam.position).as_vec3();
        let q: Vec<universe_engine::glam::Vec4> = mesh.positions.iter().map(|p| vp * (*p + off).extend(1.0)).collect();
        for f in &mesh.faces {
            let [a, b, c] = f.map(|i| q[i as usize]);
            let mut out = false;
            for ax in 0..3 {
                if (a[ax] > a.w && b[ax] > b.w && c[ax] > c.w) || (-a[ax] > a.w && -b[ax] > b.w && -c[ax] > c.w) {
                    out = true;
                    break;
                }
            }
            if !out && a.w > 0.0 && b.w > 0.0 && c.w > 0.0 {
                let (ax, ay, bx, by, cx, cy) = (a.x / a.w, a.y / a.w, b.x / b.w, b.y / b.w, c.x / c.w, c.y / c.w);
                if (bx - ax) * (cy - ay) - (by - ay) * (cx - ax) <= 0.0 {
                    out = true;
                }
            }
            if !out {
                k += 1;
            }
        }
    }
    k
}

impl PlanetLodApp {
    fn pitch_down(&self) -> f64 {
        90.0 * (self.alt / TILT_HI).clamp(0.0, 1.0)
    }

    /// The page's panel.
    fn panel(&self, frame: &mut Frame, ctx: &Context, ui: &Ui) -> (Hit, Vec<Hit>) {
        use hud_kit::{ACCENT, BAD, GOOD, INK, INK3, WARN};
        let m = &self.meta;
        let mut p = Panel::new(8.0, 8.0, PANEL_W, 0.62);
        let fps = ctx.fps;
        let fc = if fps >= 50.0 { GOOD } else if fps >= 25.0 { WARN } else { BAD };
        p.title("Planet LOD", (&format!("{fps:.0} fps"), fc), ("–", ID_HIDE));
        let alt = if self.alt >= 10_000.0 { format!("{} km", thousands((self.alt / 1000.0).round())) } else { format!("{} m", thousands(self.alt.round())) };
        p.big(&alt, "over the surface");
        match self.ground_h {
            Some(h) => p.row("ground under you", &format!("{} m", thousands(h.round())), ""),
            None => p.row("ground under you", "no triangle under the eye", ""),
        }
        let lo = (self.lon + 540.0).rem_euclid(360.0) - 180.0;
        p.row("position", &format!("{:.2}° {}  {:.2}° {}", self.lat.abs(), if self.lat < 0.0 { "S" } else { "N" }, lo.abs(), if lo < 0.0 { "W" } else { "E" }), "");
        p.row("heading · pitch", &format!("{:.0}° · {:.0}° down", self.heading.rem_euclid(360.0), self.pitch_down()), "");
        // Levels of detail.
        let n = self.d.mv.len().max(1);
        let lv = (0..self.lvl_count.len()).rev().find(|&k| self.lvl_count[k] > 0).map_or(0, |k| k + 1);
        let (full, rising) = self.risen;
        let rising_badge = if rising > 0 { (format!("{} rising", thousands(rising as f64)), WARN) } else { ("all in".to_string(), GOOD) };
        // (In cluster mode: the finest cluster level drawn, 0 the finest, and the clusters' own counts.)
        let cl_stats = self.clusters.as_ref().map(|c| { let c = c.lock().unwrap(); (c.stats, c.row.len(), c.level.iter().copied().max().unwrap_or(0)) });
        let lvb = match cl_stats { Some((st, _, top)) => format!("cluster L{} / {}", st[4], top), None => format!("L{lv} / {}", m.level_d.len()) };
        p.section("LEVEL OF DETAIL", &[(&lvb, ACCENT), (&rising_badge.0, rising_badge.1)]);
        let eye = self.eye();
        let cr = eye.length();
        let ang = (eye.dot(m.peak) / cr).clamp(-1.0, 1.0).acos();
        let d_reg = ((ang * self.d.r - m.radius_m).max(0.0)).hypot((cr - self.d.r - self.ex * m.top_m).max(0.0));
        let dist = if d_reg >= 1000.0 { format!("{:.1} km", d_reg / 1000.0) } else { format!("{d_reg:.0} m") };
        p.row("distance to the pattern", &dist, "");
        match cl_stats {
            Some((st, total, _)) => p.row("clusters chosen", &thousands(st[0] as f64), &format!("of {} · {} kept", kilo(total), kilo(st[3]))),
            None => p.row("pattern vertices drawn", &thousands(self.stats.drawn as f64), &format!("of {} · {:.1}%", thousands(n as f64), 100.0 * self.stats.drawn as f64 / n as f64)),
        }
        p.bar(self.stats.drawn as f32 / n as f32, full as f32 / n as f32);
        p.row("fully risen · rising", &format!("{} · {}", thousands(full as f64), thousands(rising as f64)), "");
        p.row("pattern triangles drawn", &thousands(ui.pattern_tris as f64), "");
        let a = m.level_d.iter().rposition(|&dd| dd > d_reg).map_or(0, |j| j + 1);
        let one = m.level_tris.get(a).copied().unwrap_or(0);
        let more = if one > ui.pattern_tris && ui.pattern_tris > 0 { format!("{:.1}× more", one as f64 / ui.pattern_tris as f64) } else { String::new() };
        p.row("with one level for all", &thousands(one as f64), &more);
        p.row("LOD update", &format!("{:.2} ms", self.stats.lod_ms), &format!("meshes {:.2} ms · {} made", self.stats.mesh_ms, self.stats.rebuilt));
        p.row("planet tiles drawn", &thousands(ui.tiles_drawn as f64), &format!("of {} groups", self.tiles.len()));
        if cl_stats.is_some() {
            p.row("clusters drawn", &thousands(ui.chunks_drawn as f64), "in view");
        } else {
            p.row("pattern cells drawn", &thousands(ui.chunks_drawn as f64), &format!("of {}", self.chunks.iter().filter(|c| !c.tris.is_empty()).count()));
        }
        p.row("settings", &format!("{}× ahead · {}× margin · {:.1} m/km", m.ahead, m.margin, m.px_per_m * 1000.0), "");
        // The chart: drawn (accent) and fully in (white) against the altitude flown, level distances.
        let (o, s) = p.area(62.0);
        let x_of = |alt: f32| o.x + 22.0 + (7.0 - alt.clamp(10.0, 1e7).log10()) / 6.0 * (s.x - 30.0);
        let y_of = |f: f32| o.y + s.y - 11.0 - f * (s.y - 20.0);
        for (k, dd) in m.level_d.iter().enumerate() {
            let x = x_of(*dd as f32).round();
            p.line(Vec2::new(x, o.y + 9.0), Vec2::new(x, o.y + s.y - 11.0), Color::hex(0x262a32));
            p.text(Vec2::new(x - 4.0, o.y + 2.0), &format!("L{}", k + 1), INK3);
        }
        p.line(Vec2::new(o.x + 22.0, y_of(0.0)), Vec2::new(o.x + s.x - 6.0, y_of(0.0)), Color::hex(0x3a3f4a));
        p.text(Vec2::new(o.x + 2.0, y_of(1.0) - 2.0), "100%", INK3);
        p.text(Vec2::new(o.x + 14.0, y_of(0.0) - 2.0), "0", INK3);
        p.text(Vec2::new(o.x + 22.0, o.y + s.y - 8.0), "10,000 km", INK3);
        p.text(Vec2::new(o.x + s.x - 22.0, o.y + s.y - 8.0), "10 m", INK3);
        for h in &self.history {
            p.rect(Vec2::new(x_of(h[0]) - 1.5, y_of(h[1]) - 1.5), Vec2::splat(3.0), ACCENT);
            p.rect(Vec2::new(x_of(h[0]) - 1.0, y_of(h[2]) - 1.0), Vec2::splat(2.0), INK);
        }
        p.note("drawn (orange) · fully risen (white) · per altitude flown", INK3);
        // The mesh.
        p.section("MESH", &[]);
        p.row("vertices", &thousands(self.d.pos.len() as f64), &format!("{} in the pattern", kilo(n)));
        let off = self.tiles.iter().map(|g| g.tris.len()).sum::<usize>();
        p.row("triangles off the pattern", &thousands(off as f64), &format!("{} land · {} sea", kilo(m.land_tris), kilo(m.sea_tris)));
        p.row("sent to the GPU", &thousands(ui.sent as f64), "");
        p.row("in the view box", &thousands(ui.in_view.load(std::sync::atomic::Ordering::Acquire) as f64), "");
        if ui.vis_note {
            p.row("visible", "needs the engine's ID render", "");
        } else {
            p.row_button("visible (own a pixel)", "measure", ID_VIS);
        }
        p.row("edges kept from lines", &thousands(m.kept as f64), &format!("of {} · {:.1}%", thousands(m.segments as f64), 100.0 * m.kept as f64 / m.segments.max(1) as f64));
        p.row("heights", &format!("{} – {} m", thousands(m.h_lo.round()), thousands(m.h_hi.round())), "");
        // The view.
        p.section("VIEW", &[]);
        p.seg(&["Mesh", "Real"], if self.view == View::Mesh { 0 } else { 1 }, ID_MESH);
        let real = self.view == View::Real;
        p.chips(&[("edges", self.show_edges && !real, !real, ID_EDGES), ("faces", self.show_faces, true, ID_FACES), ("crest & valleys", self.show_feat, !m.features.is_empty(), ID_FEAT), ("peaks", self.show_peaks, !self.peak_pins.is_empty(), ID_PEAKS)]);
        p.slider("height ×", self.ex as f32, 1.0, 50.0, &format!("{:.0}", self.ex), ID_EX);
        if let Some(l) = &self.layers {
            p.section("LAYERS", &[]);
            let short: Vec<&str> = l.list.iter().map(|x| x.title.split(" (").next().unwrap_or(&x.title)).collect();
            for row in (0..l.list.len()).collect::<Vec<_>>().chunks(3) {
                let items: Vec<(&str, bool, bool, u32)> = row.iter().map(|&k| (short[k], l.on[k], true, ID_LAYER + k as u32)).collect();
                p.chips(&items);
            }
            for (k, layer) in l.list.iter().enumerate() {
                if !l.on[k] {
                    continue;
                }
                p.note(&layer.title, hud_kit::INK2);
                // (its classes on the land, largest first, each in its own colour; basins: too many, the closed share)
                let mut rows: Vec<usize> = (0..layer.names.len()).filter(|&i| layer.share.get(i).copied().unwrap_or(0.0) > 0.0).collect();
                rows.sort_by(|a, b| layer.share[*b].total_cmp(&layer.share[*a]));
                if rows.len() > 14 {
                    let last = layer.names.len() - 1;
                    let open: f32 = rows.iter().filter(|&&i| i != last).map(|&i| layer.share[i]).sum();
                    p.note(&format!("  {} basins to the sea · {:.0} % of land", rows.len() - 1, open), hud_kit::INK2);
                    rows = vec![last];
                }
                for i in rows {
                    let c = layer.rgb[i].map(|x| x as f32 / 255.0);
                    // (dark classes lifted so the text reads on the panel)
                    let lift = (0.55 - (c[0] + c[1] + c[2]) / 3.0).max(0.0);
                    p.note(&format!("  - {} · {:.1} %", layer.names[i], layer.share[i]), Color([c[0] + lift, c[1] + lift, c[2] + lift, 1.0]));
                }
            }
        }
        p.chips(&[(if self.keys_open { "controls (hide)" } else { "controls" }, self.keys_open, true, ID_KEYS)]);
        if self.keys_open {
            for (k, what) in [("W A S D", "fly"), ("Q E · wheel", "down / up"), ("<- -> · right-drag", "turn"), ("Shift", "× 5 speed"), ("drag", "slide"), ("V", "mesh or real"), ("H", "hide this panel"), ("Esc", "close")] {
                p.note(&format!("{k:<19}{what}"), hud_kit::INK2);
            }
            p.note("Straight down in orbit, tilting to the horizon at the ground.", INK3);
        }
        p.finish(frame)
    }
}
