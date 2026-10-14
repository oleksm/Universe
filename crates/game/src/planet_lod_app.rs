//! Planet LOD (`docs/planet-lod-app.md`): the planet-trees pattern viewer as an app, `freefall --app
//! planetlod [VIEW_DIR]` (default `~/planet-data/views/earth_s15-pattern`). The pattern's progressive
//! mesh is drawn by chunks, never by vertices: each chunk keeps how far the eye may move before one of
//! its vertices could cross its appearing distance, and is decided again only then; a vertex that
//! changes marks the chunks its collapse subtree covers, whose meshes are made again. The planet
//! outside the pattern is static, one mesh per group of tiles, drawn when above the horizon.
//! Stage A: a vertex appears at its own height (the rise on the GPU waits for the engine's
//! per-vertex morph band). Keys: W/S forward and back, A/D aside, Q/E or the wheel down and up, left
//! and right (or a drag) turn, V mesh or real view, H the panel, ESC closes.
//! `UNIVERSE_PLANETLOD_AT="alt=20000&lat=-50.99&lon=71.44&heading=0"` sets the start (captures).

use std::path::{Path, PathBuf};
use std::time::Instant;

use universe_engine::apps::{App, Next};
use universe_engine::glam::{DQuat, DVec3, Quat, Vec2, Vec3};
use universe_engine::{Camera, Color, Context, Frame, KeyCode, Light, Mesh, MouseButton, Transform, WireModel};

/// Chunk edge (m): a chunk is one mesh.
const CHUNK: f64 = 8000.0;
/// Decision cell edge (m): the vertices decided together (each cell keeps its own margin).
const CELL: f64 = 1000.0;
/// Tiles of the planet outside the pattern grouped this many to a side per mesh.
const TILE_GROUP: u32 = 4;
/// Straight down at and above this altitude, level with the horizon at the ground (as the page).
const TILT_HI: f64 = 100_000.0;

#[derive(Clone, Copy, PartialEq)]
enum View {
    Mesh,
    Real,
}

struct Data {
    r: f64,
    /// Every vertex's place (m, planet frame) at its own height.
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
    cell_grid: Grid,
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
        let pos: Vec<DVec3> = (0..n).map(|i| DVec3::new(dirs[3 * i] as f64, dirs[3 * i + 1] as f64, dirs[3 * i + 2] as f64).normalize() * (r + z[i] as f64)).collect();
        let col8: Vec<u8> = read(&dir, "color.u8")?;
        let col = (0..n).map(|i| [col8[3 * i], col8[3 * i + 1], col8[3 * i + 2]]).collect();
        let mv: Vec<u32> = read(&dir, "morph_vertex.u32")?;
        let ma: Vec<u32> = read(&dir, "morph_anc.u32")?;
        let dptr: Vec<u32> = read(&dir, "morph_dptr.u32")?;
        let dep: Vec<u32> = read(&dir, "morph_dep.u32")?;
        let md: Vec<f32> = read(&dir, "morph_dist.f32")?;
        let ao = (0..mv.len()).map(|i| md[2 * i]).collect();
        let mut stand_in: Vec<u32> = (0..n as u32).collect();
        for (i, &v) in mv.iter().enumerate() {
            stand_in[v as usize] = ma[i];
        }
        let ft32: Vec<u32> = read(&dir, &format!("lvl_{levels}.u32"))?;
        let fe32: Vec<u32> = read(&dir, &format!("lvl_{levels}.e.u32"))?;
        let ft: Vec<[u32; 3]> = ft32.chunks_exact(3).map(|c| [c[0], c[1], c[2]]).collect();
        let fe: Vec<[u32; 2]> = fe32.chunks_exact(2).map(|c| [c[0], c[1]]).collect();
        let d = Data { r, pos, z, col, mv, ma, dptr, dep, ao, stand_in, ft, fe };

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
            cell_grid,
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
                    ("view", _) if kv.ends_with("real") => app.view = View::Real,
                    _ => {}
                }
            }
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
        up * (self.d.r + self.ground() + self.alt)
    }

    /// The ground under the eye (m): the highest the pattern has in the chunk under it, else sea level.
    fn ground(&self) -> f64 {
        let c = &self.chunks[self.grid.id(unit(self.lon, self.lat)) as usize];
        c.tris.iter().flat_map(|&t| self.d.ft[t as usize]).map(|v| self.d.z[v as usize] as f64).fold(0.0, f64::max)
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
                    if o { self.stats.drawn += 1 } else { self.stats.drawn -= 1 }
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
        let dt = ctx.dt as f64;
        if input.button_down(MouseButton::Left) || input.button_down(MouseButton::Right) {
            self.heading += input.mouse_delta.x as f64 * 0.2;
        }
        self.heading += input.axis(KeyCode::ArrowLeft, KeyCode::ArrowRight) as f64 * 60.0 * dt;
        // Up and down by a share of the altitude a second; forward and aside at the altitude's speed.
        let climb = input.axis(KeyCode::KeyQ, KeyCode::KeyE) as f64 * dt - input.scroll as f64 * 0.15;
        self.alt = (self.alt * (1.0 + climb).max(0.05)).clamp(2.0, 3.0 * self.d.r);
        let (fw, side) = (input.axis(KeyCode::KeyS, KeyCode::KeyW) as f64, input.axis(KeyCode::KeyA, KeyCode::KeyD) as f64);
        if fw != 0.0 || side != 0.0 {
            let speed = self.alt.max(50.0) * 0.6 * dt / self.d.r;   // radians a second
            let h = self.heading.to_radians();
            let (dn, de) = (fw * h.cos() - side * h.sin(), fw * h.sin() + side * h.cos());
            self.lat = (self.lat + (dn * speed).to_degrees()).clamp(-89.9, 89.9);
            self.lon += (de * speed / self.lat.to_radians().cos()).to_degrees();
        }
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
        self.stats.rebuilt = rebuilt;
        self.stats.mesh_ms = t0.elapsed().as_secs_f64() * 1e3;
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
        let p = (90.0 * (self.alt / TILT_HI).min(1.0)).max(2.0).to_radians();
        let fwd = ahead * p.cos() - up * p.sin();
        let cam_up = up * p.cos() + ahead * p.sin();
        let position = self.eye();
        let mut camera = Camera { position, fov_y: 50f32.to_radians(), near: 1.0, ..Default::default() };
        camera.look_at(position + fwd, cam_up.as_vec3());
        camera
    }

    fn draw(&self, frame: &mut Frame, ctx: &Context) {
        let cam = frame.camera.clone();
        let eye = cam.position;
        let up = eye.normalize();
        // The sun high over the eye's shoulder, as the page's light follows the camera.
        let sun = (up * 2.0 + DQuat::from_axis_angle(up, 0.6) * DVec3::Y.cross(up).normalize_or(DVec3::X)).normalize();
        frame.light = Some(Light { position: sun * 1.5e11, color: [1.0, 0.98, 0.94], luminosity: 1.0, reference: 1.5e11, radius: 7e8 });
        let size = frame.size();
        let aspect = (size.x / size.y.max(1.0)) as f64;
        let mesh_view = self.view == View::Mesh;
        let (land, sea, pat_line, land_line, sea_line) = if mesh_view {
            (Color::hex(0x8a8f99), Color::hex(0x2f5fa8), Color::hex(0xff6a00), Color::hex(0xf0c060), Color::hex(0x6fc3ff))
        } else {
            (Color::WHITE, Color::WHITE, Color([0.0; 4]), Color([0.0; 4]), Color([0.0; 4]))
        };
        let mut st = (0usize, 0usize, 0usize);
        for g in &self.tiles {
            let Some((_, mesh)) = &g.mesh else { continue };
            if !self.above_horizon(g.dir, g.cos_a, g.zmax, eye) {
                continue;
            }
            let radius = self.d.r * (2.0 - 2.0 * g.cos_a).max(0.0).sqrt() + g.zmax.max(0.0) + 12_000.0;
            if !Self::in_view(&cam, g.origin, radius, aspect) {
                continue;
            }
            let t = Transform { position: g.origin, rotation: Quat::IDENTITY, scale: 1.0 };
            let (fill, line) = if g.sea { (sea, sea_line) } else { (land, land_line) };
            frame.model_shaded(mesh, &t, line, fill);
            st.1 += 1;
            st.2 += g.tris.len();
        }
        // (UNIVERSE_PLANETLOD_MAXDRAW: at most this many chunks, to tell a draw's cost from a triangle's.)
        let max_draw: usize = std::env::var("UNIVERSE_PLANETLOD_MAXDRAW").ok().and_then(|v| v.parse().ok()).unwrap_or(usize::MAX);
        for ch in &self.chunks {
            if st.0 >= max_draw {
                break;
            }
            let Some(mesh) = &ch.mesh else { continue };
            if ch.tri_count == 0 {
                continue;
            }
            let dir = ch.origin.normalize();
            let cos_a = 1.0 - 0.5 * (ch.radius / self.d.r).powi(2);
            if !self.above_horizon(dir, cos_a, 6000.0, eye) || !Self::in_view(&cam, ch.origin, ch.radius + 6000.0, aspect) {
                continue;
            }
            let t = Transform { position: ch.origin, rotation: Quat::IDENTITY, scale: 1.0 };
            if std::env::var_os("UNIVERSE_PLANETLOD_NOSHADOW").is_some() {
                frame.no_shadow(|f| f.model_shaded(mesh, &t, pat_line, land));
            } else {
                frame.model_shaded(mesh, &t, pat_line, land);
            }
            st.0 += 1;
            st.2 += ch.tri_count;
        }
        if !self.hud {
            return;
        }
        let s = &self.stats;
        let lines = [
            format!("PLANET LOD   {:.0} FPS   {}   ALT {:.1} KM   {:.3} {:.3}   HEADING {:.0}", ctx.fps, if mesh_view { "MESH" } else { "REAL" }, self.alt / 1000.0, self.lat, self.lon, self.heading.rem_euclid(360.0)),
            format!("PATTERN  {} OF {} COLLAPSED VERTICES DRAWN   CHUNKS DRAWN {}   TILE GROUPS DRAWN {}   TRIANGLES SENT {}", s.drawn, self.d.mv.len(), st.0, st.1, st.2),
            format!("LOD  {} CHUNKS DECIDED  {} VERTICES CHANGED  {:.2} MS   MESHES  {} REBUILT  {:.2} MS", s.decided, s.changed, s.lod_ms, s.rebuilt, s.mesh_ms),
            "W/S A/D MOVE  Q/E OR WHEEL DOWN/UP  LEFT/RIGHT OR DRAG TURN  V VIEW  H PANEL  ESC CLOSE".to_string(),
        ];
        for (k, l) in lines.iter().enumerate() {
            frame.text(Vec2::new(12.0, 12.0 + 18.0 * k as f32), l, Color::hex(0xd8dee6));
        }
    }
}
