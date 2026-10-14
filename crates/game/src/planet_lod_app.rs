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
use std::sync::Mutex;
use std::time::Instant;

use crate::hud_kit::{self, Hit, Panel};

use universe_engine::apps::{App, Next};
use universe_engine::glam::{DVec3, Quat, Vec2, Vec3};
use universe_engine::{Camera, Color, Context, Frame, KeyCode, Light, Mesh, MouseButton, Transform, WireModel};

/// Chunk edge (m): a chunk is one mesh.
const CHUNK: f64 = 8000.0;
/// Decision cell edge (m): the vertices decided together (each cell keeps its own margin).
const CELL: f64 = 1000.0;
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
const ID_EX: u32 = 30;
const ID_KEYS: u32 = 40;
const ID_VIS: u32 = 50;
const PANEL_W: f32 = 372.0;

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
    in_view: usize,
    sent: usize,
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

pub struct PlanetLodApp {
    dir: PathBuf,
    d: Data,
    chunks: Vec<Chunk>,
    cells: Vec<Cell>,
    /// Per vertex: its decision cell (u32::MAX outside the pattern).
    cell_of: Vec<u32>,
    /// Per vertex: the collapsed vertices whose decision reads it (csr).
    rdep_ptr: Vec<u32>,
    rdep: Vec<u32>,
    /// Per vertex: the chunks (x0, y0, x1, y1) its collapse subtree's triangles lie in.
    subtree: Vec<[i32; 4]>,
    grid: Grid,
    on: Vec<u8>,
    tiles: Vec<TileGroup>,
    view: View,
    hud: bool,
    lon: f64,
    lat: f64,
    alt: f64,
    heading: f64,
    stats: Stats,
    queued: Vec<bool>,
    /// UNIVERSE_PLANETLOD_FLY: the camera descends and drifts by itself (captures ignore the keys).
    fly: bool,
    meta: Meta,
    ex: f64,
    show_edges: bool,
    show_faces: bool,
    show_feat: bool,
    keys_open: bool,
    /// The slider being dragged, a drag over the ground (its button), the ground under the eye (m, drawn).
    sliding: bool,
    drag: Option<MouseButton>,
    ground_h: Option<f64>,
    /// Drawn collapsed vertices per level; the last full count of those fully in and rising.
    lvl_count: Vec<usize>,
    scan: [usize; 3],
    risen: (usize, usize),
    /// (altitude, drawn share, fully-in share) as flown, for the chart.
    history: Vec<[f32; 3]>,
    last_hist: f64,
    feat: Option<Mesh>,
    pins: Vec<(DVec3, f64, String, bool)>,
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
        let gn = ((hi - lo) / CHUNK).ceil() as i32 + 1;
        let grid = Grid { p0, east, north, min: lo, n: gn, size: CHUNK };
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
        let queued = vec![false; cells.len()];
        let mut app = PlanetLodApp {
            dir,
            d,
            chunks,
            cells,
            cell_of,
            rdep_ptr: cnt,
            rdep,
            subtree,
            grid,
            on,
            tiles,
            view: View::Mesh,
            hud: true,
            lon: peak["lon"].as_f64().unwrap_or(0.0),
            lat: peak["lat"].as_f64().unwrap_or(0.0),
            alt: 20_000.0,
            heading: 0.0,
            stats: Stats::default(),
            queued,
            fly: std::env::var_os("UNIVERSE_PLANETLOD_FLY").is_some(),
            meta,
            ex: 1.0,
            show_edges: true,
            show_faces: true,
            show_feat: false,
            keys_open: false,
            sliding: false,
            drag: None,
            ground_h: None,
            lvl_count: vec![0; 256],
            scan: [0; 3],
            risen: (0, 0),
            history: vec![],
            last_hist: 0.0,
            feat: None,
            pins,
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
                    ("view", _) if kv.ends_with("real") => app.view = View::Real,
                    _ => {}
                }
            }
        }
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
                    let r = d.ft[t as usize].map(|v| self.resolve(v));
                    if r[0] != r[1] && r[1] != r[2] && r[0] != r[2] && let Some(h) = hit(r) {
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
        for c in &mut self.cells {
            c.eye_at = DVec3::splat(f64::INFINITY);
        }
        for c in &mut self.chunks {
            c.stale = true;
        }
        for g in &mut self.tiles {
            g.mesh = None;
        }
        self.feat = None;
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
    fn lod_update(&mut self, eye: DVec3) {
        let t0 = Instant::now();
        let d = &self.d;
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
                let v = d.mv[i] as usize;
                let dist = d.pos[v].distance(eye);
                let ao = d.ao[i] as f64;
                margin = margin.min((dist - ao).abs());
                let mut o = dist < ao && self.on[d.ma[i] as usize] == 1;
                if o {
                    o = (d.dptr[i]..d.dptr[i + 1]).all(|b| self.on[d.dep[b as usize] as usize] == 1);
                }
                if o as u8 != self.on[v] {
                    self.on[v] = o as u8;
                    changed += 1;
                    if o {
                        self.stats.drawn += 1;
                        self.lvl_count[d.ml[i] as usize] += 1;
                    } else {
                        self.stats.drawn -= 1;
                        self.lvl_count[d.ml[i] as usize] -= 1;
                    }
                    stale.push(self.subtree[v]);
                    // Those whose decision reads it are decided again.
                    for &j in &self.rdep[self.rdep_ptr[v] as usize..self.rdep_ptr[v + 1] as usize] {
                        let cj = self.cell_of[d.mv[j as usize] as usize] as usize;
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
        let gn = self.grid.n;
        for b in stale {
            if b[0] > b[2] {
                continue;
            }
            for u in b[0]..=b[2] {
                for v in b[1]..=b[3] {
                    self.chunks[(u * gn + v) as usize].stale = true;
                }
            }
        }
        self.stats.decided = decided;
        self.stats.changed = changed;
        self.stats.lod_ms = t0.elapsed().as_secs_f64() * 1e3;
    }

    fn resolve(&self, mut v: u32) -> u32 {
        while self.on[v as usize] == 0 {
            v = self.d.stand_in[v as usize];
        }
        v
    }

    /// The chunk's mesh again: its finest triangles through their drawn stand-ins.
    fn build_chunk(&self, c: usize) -> (Mesh, usize) {
        let ch = &self.chunks[c];
        let d = &self.d;
        let mut m = WireModel::default();
        let o = ch.origin;
        let real = self.view == View::Real;
        for &t in &ch.tris {
            let [a, b, cc] = d.ft[t as usize].map(|v| self.resolve(v));
            if a == b || b == cc || a == cc {
                continue;
            }
            let k = m.positions.len() as u32;
            for v in [a, b, cc] {
                m.positions.push((d.pos[v as usize] - o).as_vec3());
            }
            m.faces.push([k, k + 1, k + 2]);
            let rgb = if real { d.col[cc as usize].map(srgb_to_linear) } else { [1.0; 3] };
            m.colors.extend([[rgb[0], rgb[1], rgb[2], 1.0]; 3]);
        }
        let tris = m.faces.len();
        if !real {
            for &e in &ch.edges {
                let [a, b] = d.fe[e as usize].map(|v| self.resolve(v));
                if a == b {
                    continue;
                }
                let k = m.positions.len() as u32;
                m.positions.push((d.pos[a as usize] - o).as_vec3());
                m.positions.push((d.pos[b as usize] - o).as_vec3());
                m.colors.extend([[1.0; 4]; 2]);
                m.edges.push([k, k + 1]);
            }
        }
        (Mesh::new(m), tris)
    }

    fn build_tiles(&self, g: &TileGroup) -> Mesh {
        let d = &self.d;
        let mut m = WireModel::default();
        let real = self.view == View::Real;
        for t in &g.tris {
            let k = m.positions.len() as u32;
            for &v in t {
                m.positions.push((d.pos[v as usize] - g.origin).as_vec3());
            }
            m.faces.push([k, k + 1, k + 2]);
            let rgb = if real { d.col[t[2] as usize].map(srgb_to_linear) } else { [1.0; 3] };
            m.colors.extend([[rgb[0], rgb[1], rgb[2], 1.0]; 3]);
        }
        if !real {
            for e in &g.edges {
                let k = m.positions.len() as u32;
                m.positions.push((d.pos[e[0] as usize] - g.origin).as_vec3());
                m.positions.push((d.pos[e[1] as usize] - g.origin).as_vec3());
                m.colors.extend([[1.0; 4]; 2]);
                m.edges.push([k, k + 1]);
            }
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
                Some(ID_KEYS) => self.keys_open = !self.keys_open,
                Some(ID_VIS) => self.ui.lock().unwrap().vis_note = true,
                Some(ID_EX) => self.sliding = true,
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
        if input.scroll != 0.0 && !over_panel {
            self.alt = (self.alt * (-0.15 * input.scroll as f64).exp()).clamp(ALT_MIN, 6.0 * r);
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
            self.alt = (self.alt * (1.2 * boost * dt * climb).exp()).clamp(ALT_MIN, 6.0 * r);
        }
        self.heading += input.axis(KeyCode::ArrowLeft, KeyCode::ArrowRight) as f64 * (1.2 * dt).to_degrees();
        if self.fly {
            self.alt = (self.alt * 0.995).max(300.0);
            self.lon += 2e-4;
        }
        let eye = self.eye();
        self.lod_update(eye);
        let t0 = Instant::now();
        let mut rebuilt = 0;
        let stale: Vec<usize> = (0..self.chunks.len()).filter(|&c| !self.chunks[c].tris.is_empty() && (self.chunks[c].stale || self.chunks[c].mesh_view != Some(self.view))).collect();
        for c in stale {
            let (mesh, tris) = self.build_chunk(c);
            let ch = &mut self.chunks[c];
            ch.mesh = Some(mesh);
            ch.mesh_view = Some(self.view);
            ch.stale = false;
            ch.tri_count = tris;
            rebuilt += 1;
        }
        let view = self.view;
        let need: Vec<usize> = (0..self.tiles.len()).filter(|&g| self.tiles[g].mesh.as_ref().is_none_or(|(v, _)| *v != view)).collect();
        for g in need {
            let mesh = self.build_tiles(&self.tiles[g]);
            self.tiles[g].mesh = Some((view, mesh));
        }
        if self.show_feat && self.feat.is_none() && !self.meta.features.is_empty() {
            self.feat = Some(self.build_features());
        }
        self.stats.rebuilt = rebuilt;
        self.stats.mesh_ms = t0.elapsed().as_secs_f64() * 1e3;
        self.ground_h = self.ground_at(unit(self.lon, self.lat));
        // Fully in and rising: counted over a fifteenth of the collapsed vertices a frame.
        let n = self.d.mv.len();
        let step = n.div_ceil(15);
        let (a, b) = (self.scan[0], (self.scan[0] + step).min(n));
        for i in a..b {
            let v = self.d.mv[i] as usize;
            if self.on[v] == 1 {
                if self.d.pos[v].distance(eye) <= self.d.am[i] as f64 { self.scan[1] += 1 } else { self.scan[2] += 1 }
            }
        }
        self.scan[0] = b;
        if b >= n {
            self.risen = (self.scan[1], self.scan[2]);
            self.scan = [0; 3];
        }
        if ctx.time - self.last_hist > 0.15 {
            self.last_hist = ctx.time;
            let nm = n.max(1) as f32;
            self.history.push([self.alt as f32, self.stats.drawn as f32 / nm, self.risen.0 as f32 / nm]);
            if self.history.len() > 5000 {
                self.history.remove(0);
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
        // (UNIVERSE_PLANETLOD_MAXDRAW: at most this many chunks, to tell a draw's cost from a triangle's.)
        let max_draw: usize = std::env::var("UNIVERSE_PLANETLOD_MAXDRAW").ok().and_then(|v| v.parse().ok()).unwrap_or(usize::MAX);
        for ch in &self.chunks {
            if chunks_drawn >= max_draw {
                break;
            }
            let Some(mesh) = &ch.mesh else { continue };
            if ch.tri_count == 0 {
                continue;
            }
            let dir = ch.origin.normalize();
            let cos_a = 1.0 - 0.5 * (ch.radius / self.d.r).powi(2);
            let top = 6000.0 * self.ex;
            if !self.above_horizon(dir, cos_a, top, eye) || !Self::in_view(&cam, ch.origin, ch.radius + top, aspect) {
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
        // The counts the panel shows that need the drawn meshes: the view box every 0.3 s.
        let mut ui = self.ui.lock().unwrap();
        ui.sent = sent;
        ui.chunks_drawn = chunks_drawn;
        ui.tiles_drawn = tiles_drawn;
        ui.pattern_tris = pattern_tris;
        if ctx.time - ui.last_count > 0.3 {
            ui.last_count = ctx.time;
            ui.in_view = view_box_count(&cam, aspect as f32, &drawn);
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
        let mut p = Panel::new(10.0, 10.0, PANEL_W, 0.7);
        let fps = ctx.fps;
        let fc = if fps >= 50.0 { GOOD } else if fps >= 25.0 { WARN } else { BAD };
        p.title("PLANET LOD", (&format!("{fps:.0} FPS"), fc), ("-", ID_HIDE));
        let alt = if self.alt >= 10_000.0 { format!("{} KM", thousands((self.alt / 1000.0).round())) } else { format!("{} M", thousands(self.alt.round())) };
        p.big(&alt, "over the surface");
        match self.ground_h {
            Some(h) => p.row("ground under you", &format!("{} m", thousands(h.round())), ""),
            None => p.row("ground under you", "no triangle under the eye", ""),
        }
        let lo = (self.lon + 540.0).rem_euclid(360.0) - 180.0;
        p.row("position", &format!("{:.2} {}  {:.2} {}", self.lat.abs(), if self.lat < 0.0 { "S" } else { "N" }, lo.abs(), if lo < 0.0 { "W" } else { "E" }), "");
        p.row("heading - pitch", &format!("{:.0} - {:.0} down", self.heading.rem_euclid(360.0), self.pitch_down()), "");
        // Levels of detail.
        let n = self.d.mv.len().max(1);
        let lv = (0..self.lvl_count.len()).rev().find(|&k| self.lvl_count[k] > 0).map_or(0, |k| k + 1);
        let (full, rising) = self.risen;
        let rising_badge = if rising > 0 { (format!("{} RISING", thousands(rising as f64)), WARN) } else { ("ALL IN".to_string(), GOOD) };
        let lvb = format!("L{lv} / {}", m.level_d.len());
        p.section("LEVEL OF DETAIL", &[(&lvb, ACCENT), (&rising_badge.0, rising_badge.1)]);
        let eye = self.eye();
        let cr = eye.length();
        let ang = (eye.dot(m.peak) / cr).clamp(-1.0, 1.0).acos();
        let d_reg = ((ang * self.d.r - m.radius_m).max(0.0)).hypot((cr - self.d.r - self.ex * m.top_m).max(0.0));
        let dist = if d_reg >= 1000.0 { format!("{:.1} km", d_reg / 1000.0) } else { format!("{d_reg:.0} m") };
        p.row("distance to the pattern", &dist, "");
        p.row("pattern vertices drawn", &thousands(self.stats.drawn as f64), &format!("of {} - {:.1}%", thousands(n as f64), 100.0 * self.stats.drawn as f64 / n as f64));
        p.bar(self.stats.drawn as f32 / n as f32, full as f32 / n as f32);
        p.row("fully risen - rising", &format!("{} - {}", thousands(full as f64), thousands(rising as f64)), "");
        p.row("pattern triangles drawn", &thousands(ui.pattern_tris as f64), "");
        let a = m.level_d.iter().rposition(|&dd| dd > d_reg).map_or(0, |j| j + 1);
        let one = m.level_tris.get(a).copied().unwrap_or(0);
        let more = if one > ui.pattern_tris && ui.pattern_tris > 0 { format!("{:.1}x more", one as f64 / ui.pattern_tris as f64) } else { String::new() };
        p.row("with one level for all", &thousands(one as f64), &more);
        p.row("LOD update", &format!("{:.2} ms", self.stats.lod_ms), &format!("meshes {:.2} ms - {} made", self.stats.mesh_ms, self.stats.rebuilt));
        p.row("planet tiles drawn", &thousands(ui.tiles_drawn as f64), &format!("of {} groups", self.tiles.len()));
        p.row("pattern chunks drawn", &thousands(ui.chunks_drawn as f64), &format!("of {}", self.chunks.iter().filter(|c| !c.tris.is_empty()).count()));
        p.row("settings", &format!("{}x ahead - {}x margin - {:.1} m/km", m.ahead, m.margin, m.px_per_m * 1000.0), "");
        // The chart: drawn (accent) and fully in (white) against the altitude flown, level distances.
        let (o, s) = p.area(84.0);
        let x_of = |alt: f32| o.x + 30.0 + (7.0 - alt.clamp(10.0, 1e7).log10()) / 6.0 * (s.x - 40.0);
        let y_of = |f: f32| o.y + s.y - 16.0 - f * (s.y - 28.0);
        for (k, dd) in m.level_d.iter().enumerate() {
            let x = x_of(*dd as f32).round();
            p.line(Vec2::new(x, o.y + 12.0), Vec2::new(x, o.y + s.y - 16.0), Color::hex(0x262a32));
            p.text(Vec2::new(x - 8.0, o.y + 2.0), &format!("L{}", k + 1), INK3);
        }
        p.line(Vec2::new(o.x + 30.0, y_of(0.0)), Vec2::new(o.x + s.x - 8.0, y_of(0.0)), Color::hex(0x3a3f4a));
        p.text(Vec2::new(o.x + 2.0, y_of(1.0) - 4.0), "100%", INK3);
        p.text(Vec2::new(o.x + 20.0, y_of(0.0) - 4.0), "0", INK3);
        p.text(Vec2::new(o.x + 30.0, o.y + s.y - 11.0), "10,000 km", INK3);
        p.text(Vec2::new(o.x + s.x - 48.0, o.y + s.y - 11.0), "10 m", INK3);
        for h in &self.history {
            p.rect(Vec2::new(x_of(h[0]) - 1.5, y_of(h[1]) - 1.5), Vec2::splat(3.0), ACCENT);
            p.rect(Vec2::new(x_of(h[0]) - 1.0, y_of(h[2]) - 1.0), Vec2::splat(2.0), INK);
        }
        p.note("# drawn   # fully risen    per altitude flown", INK3);
        // The mesh.
        p.section("MESH", &[]);
        p.row("vertices", &thousands(self.d.pos.len() as f64), &format!("{} in the pattern", kilo(n)));
        let off = self.tiles.iter().map(|g| g.tris.len()).sum::<usize>();
        p.row("triangles off the pattern", &thousands(off as f64), &format!("{} land - {} sea", kilo(m.land_tris), kilo(m.sea_tris)));
        p.row("sent to the GPU", &thousands(ui.sent as f64), "");
        p.row("in the view box", &thousands(ui.in_view as f64), "");
        if ui.vis_note {
            p.row("visible (own a pixel)", "needs the engine's ID render", "");
        } else {
            p.row_button("visible (own a pixel)", "measure", ID_VIS);
        }
        p.row("edges kept from lines", &thousands(m.kept as f64), &format!("of {} - {:.1}%", thousands(m.segments as f64), 100.0 * m.kept as f64 / m.segments.max(1) as f64));
        p.row("heights", &format!("{} ... {} m", thousands(m.h_lo.round()), thousands(m.h_hi.round())), "");
        // The view.
        p.section("VIEW", &[]);
        p.seg(&["Mesh", "Real"], if self.view == View::Mesh { 0 } else { 1 }, ID_MESH);
        let real = self.view == View::Real;
        p.chips(&[("edges", self.show_edges && !real, !real, ID_EDGES), ("faces", self.show_faces, true, ID_FACES), ("crest & valleys", self.show_feat, !m.features.is_empty(), ID_FEAT)]);
        p.slider("height x", self.ex as f32, 1.0, 50.0, &format!("{:.0}", self.ex), ID_EX);
        p.chips(&[(if self.keys_open { "controls (hide)" } else { "controls" }, self.keys_open, true, ID_KEYS)]);
        if self.keys_open {
            for (k, what) in [("W A S D", "fly"), ("Q E - wheel", "down / up"), ("<- -> - right-drag", "turn"), ("Shift", "x5 speed"), ("drag", "slide"), ("V", "mesh or real"), ("H", "hide this panel"), ("Esc", "close")] {
                p.note(&format!("{k:<20}{what}"), hud_kit::INK2);
            }
            p.note("Straight down in orbit, tilting to the horizon at the ground.", INK3);
        }
        p.finish(frame)
    }
}
