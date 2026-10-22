//! A world whose ground is a land package (`planet-land-pyramid/1`, the folder its record's `ground`
//! names in the worlds store): the planet lab's anchor mesh (`dir.f32`, `z.f32`, `tri.u32`, `tiles.*`),
//! its pattern's finest triangles, and its land pyramid (`pyramid/`: height, colour and mask rasters per
//! level, read a rectangle at a time). The game draws it with the planet LOD renderer; `LandHeights` is
//! the same ground for landing and contact: the pyramid where it covers, else the pattern, else the
//! anchor mesh, as the renderer's own `ground_at` finds it.
//!
//! The package keeps its own frame and radius: directions as the planet lab writes them (a quarter turn
//! about the pole from the body's, `to_body`), heights in metres over `radius`. The body scales it to its
//! own radius (heights with it).

use super::{store, Package};
use glam::{DQuat, DVec3};
use serde::Deserialize;
use std::collections::HashMap;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

pub const FORMAT: &str = "planet-land-pyramid/1";

/// Tile rasters of the pyramid kept for height queries (each about 190 KB at 193 samples a side), in SHARDS
/// caches locked apart so parallel ships rarely wait on each other.
const RASTERS: usize = 512;
const SHARDS: usize = 16;

/// From the package's frame to the body's (`super::direction`): the lab's (x, y, z) is the body's (-z, y, x).
pub fn to_body() -> DQuat {
    DQuat::from_rotation_y(std::f64::consts::FRAC_PI_2)
}

/// The planet lab's direction for a longitude and latitude (degrees): its packages' and the planet LOD
/// app's frame, (cos lat sin lon, sin lat, cos lat cos lon). The game's own is `super::direction`;
/// `to_body` turns the one into the other.
pub fn lab_direction(lon: f64, lat: f64) -> DVec3 {
    let (lo, la) = (lon.to_radians(), lat.to_radians());
    DVec3::new(la.cos() * lo.sin(), la.sin(), la.cos() * lo.cos())
}

/// Latitude and longitude (degrees) of a direction in the lab's frame (as `lab_direction`).
pub fn lab_lat_lon(d: DVec3) -> (f64, f64) {
    (d.y.clamp(-1.0, 1.0).asin().to_degrees(), d.x.atan2(d.z).to_degrees())
}

/// Body `key`'s land package: its folder (manifest hash and every file's size checked) and its radius
/// (m). None: its ground is not a land package.
pub fn ground(key: &str) -> Option<Result<(PathBuf, f64), String>> {
    let g = crate::registry::registry().body(key)?.ground.as_ref()?;
    if g.format != FORMAT {
        return None;
    }
    Some((|| {
        let folder = store().ok_or("no worlds store (set UNIVERSE_WORLDS)")?.join(&g.path);
        let package = Package::open(folder.clone(), &g.manifest_sha256)?;
        let manifest: serde_json::Value = serde_json::from_slice(package.manifest()).map_err(|e| e.to_string())?;
        let radius = manifest.get("radius_m").and_then(|v| v.as_f64()).ok_or("manifest: radius_m")?;
        Ok((folder, radius))
    })())
}

#[derive(Clone, Deserialize)]
pub struct Projection {
    pub kind: String,
    pub radius_m: f64,
    pub centre: [f64; 3],
    pub east: [f64; 3],
    pub north: [f64; 3],
}

impl Projection {
    /// The direction at (x, y) m on the azimuthal equidistant plane.
    pub fn inverse(&self, x: f64, y: f64) -> DVec3 {
        let r = x.hypot(y);
        let c = r / self.radius_m;
        let centre = DVec3::from_array(self.centre);
        if r < 1e-12 {
            centre
        } else {
            centre * c.cos() + (DVec3::from_array(self.east) * x + DVec3::from_array(self.north) * y) * (c.sin() / r)
        }
    }

    /// Where `dir` lies on the plane. None: the far pole.
    pub fn project(&self, dir: DVec3) -> Option<(f64, f64)> {
        let c = dir.dot(DVec3::from_array(self.centre)).clamp(-1.0, 1.0).acos();
        if c >= std::f64::consts::PI - 1e-8 {
            return None;
        }
        let f = if c < 1e-10 { self.radius_m } else { self.radius_m * c / c.sin() };
        Some((dir.dot(DVec3::from_array(self.east)) * f, dir.dot(DVec3::from_array(self.north)) * f))
    }
}

#[derive(Clone, Copy, Deserialize)]
pub struct Window {
    pub row0: usize,
    pub col0: usize,
    pub rows: usize,
    pub cols: usize,
}

#[derive(Clone, Deserialize)]
pub struct Level {
    #[serde(default)]
    pub window: Option<Window>,
    pub k: usize,
    pub n: usize,
    pub cell_m: f64,
    pub z: String,
    pub rgb: String,
    pub mask: String,
    /// Its tiles' feature-aware meshes (`TileMesh`), if it has them.
    #[serde(default)]
    pub mesh: Option<String>,
}

impl Level {
    /// The rows and columns the files hold (all of the level unless it is windowed).
    pub fn storage(&self) -> Window {
        self.window.unwrap_or(Window { row0: 0, col0: 0, rows: self.n, cols: self.n })
    }
}

#[derive(Clone, Copy, Deserialize)]
pub struct Tile {
    pub k: usize,
    pub row: usize,
    pub col: usize,
    pub error_m: f64,
    pub zmin: f64,
    pub zmax: f64,
    /// Its mesh in the level's mesh file: where (u16s in), vertices, triangles (none: the full grid).
    #[serde(default)]
    pub mesh_off: u64,
    #[serde(default)]
    pub mesh_nv: usize,
    #[serde(default)]
    pub mesh_nt: usize,
    /// Its coarser meshes (`tile_mesh 2`): nested, at `factor` x the allowed error, coarsest last.
    #[serde(default, deserialize_with = "mesh_lod")]
    pub mesh_lod: MeshLod,
}

/// A tile's coarser meshes: the first `n` of `v`, each (factor, offset in u16s, vertices, triangles).
#[derive(Clone, Copy, Default, Debug)]
pub struct MeshLod {
    pub n: usize,
    pub v: [(f64, u64, usize, usize); MESH_LODS],
}

/// The most coarser meshes a tile carries.
pub const MESH_LODS: usize = 8;

impl MeshLod {
    pub fn get(&self) -> &[(f64, u64, usize, usize)] {
        &self.v[..self.n]
    }
}

fn mesh_lod<'de, D: serde::Deserializer<'de>>(d: D) -> Result<MeshLod, D::Error> {
    let list: Vec<(f64, u64, usize, usize)> = Deserialize::deserialize(d)?;
    if list.len() > MESH_LODS {
        return Err(serde::de::Error::custom(format!("mesh_lod: {} versions, at most {MESH_LODS}", list.len())));
    }
    let mut lod = MeshLod { n: list.len(), ..Default::default() };
    lod.v[..list.len()].copy_from_slice(&list);
    Ok(lod)
}

impl Tile {
    /// Its meshes, finest first: the full one (offset, vertices, triangles), then its coarser ones; none: a grid.
    pub fn meshes(&self) -> Vec<(u64, usize, usize)> {
        if self.mesh_nv == 0 {
            return vec![];
        }
        std::iter::once((self.mesh_off, self.mesh_nv, self.mesh_nt)).chain(self.mesh_lod.get().iter().map(|&(_, o, v, t)| (o, v, t))).collect()
    }
}

#[derive(Clone, Deserialize)]
pub struct Hole {
    pub lat: f64,
    pub lon: f64,
    pub radius_km: f64,
}

/// `pyramid.json`.
#[derive(Clone, Deserialize)]
pub struct Meta {
    #[serde(default = "default_branch")]
    pub branch: usize,
    pub format: String,
    pub projection: Projection,
    pub x0: f64,
    pub y0: f64,
    pub cell0_m: f64,
    pub side: usize,
    pub tile: usize,
    pub top_level: usize,
    pub levels: Vec<Level>,
    pub tiles: Vec<Tile>,
    #[serde(default)]
    pub holes: Vec<Hole>,
}

fn default_branch() -> usize {
    2
}

impl Meta {
    /// Level `k`'s cell in level-0 cells.
    pub fn scale(&self, k: usize) -> usize {
        self.branch.pow(k as u32)
    }
}

/// A rectangle of one level: heights, colours and mask (0 absent, 1 ground, 2 a hole) by the level's
/// own sample indices.
pub struct Raster {
    pub n: usize,
    pub i0: usize,
    pub j0: usize,
    pub width: usize,
    pub z: Vec<f32>,
    pub rgb: Vec<[u8; 3]>,
    pub mask: Vec<u8>,
}

impl Raster {
    pub fn at(&self, i: usize, j: usize) -> usize {
        (j.min(self.n - 1) - self.j0) * self.width + i.min(self.n - 1) - self.i0
    }
    pub fn z(&self, i: usize, j: usize) -> f64 {
        self.z[self.at(i, j)] as f64
    }
}

/// `width` x `height` samples of `level`'s file at `path` from (i0, j0), `stride` bytes each.
pub fn read_rect(path: &Path, level: &Level, i0: usize, j0: usize, width: usize, height: usize, stride: usize) -> Result<Vec<u8>, String> {
    let w = level.storage();
    let mut file = File::open(path).map_err(|e| format!("{}: {e}", path.display()))?;
    if file.metadata().map_err(|e| e.to_string())?.len() != (w.rows as u64 * w.cols as u64 * stride as u64) {
        return Err(format!("{}: wrong raster length", path.display()));
    }
    // Read only the stored row interval. Extend edge heights/colours for a tile's halo;
    // raster() marks samples outside the actual window as absent in its mask.
    let first = i0.clamp(w.col0, w.col0 + w.cols - 1);
    let last = (i0 + width - 1).clamp(w.col0, w.col0 + w.cols - 1);
    let mut row = vec![0; (last - first + 1) * stride];
    let mut bytes = vec![0; width * height * stride];
    for j in 0..height {
        let r = (j0 + j).clamp(w.row0, w.row0 + w.rows - 1);
        file.seek(SeekFrom::Start(((r - w.row0) * w.cols + first - w.col0) as u64 * stride as u64)).map_err(|e| e.to_string())?;
        file.read_exact(&mut row).map_err(|e| e.to_string())?;
        for i in 0..width {
            let src = ((i0 + i).clamp(first, last) - first) * stride;
            let dst = (j * width + i) * stride;
            bytes[dst..dst + stride].copy_from_slice(&row[src..src + stride]);
        }
    }
    Ok(bytes)
}

/// A tile's feature-aware mesh (the planet lab's `tile_mesh`): its vertices, samples of the level within the
/// tile ((i, j): column and row, 0 to `tile`), and its triangles. Heights and the rest are the rasters' at the
/// vertices; drawn and touched alike (`height`).
pub struct TileMesh {
    pub verts: Vec<[u16; 2]>,
    pub tris: Vec<[u16; 3]>,
    /// The triangles over each BUCKET-cell square of the tile, row by row (`find`).
    buckets: Vec<Vec<u32>>,
    side: usize,
}

/// A bucket's side (cells).
const BUCKET: usize = 8;

impl TileMesh {
    /// Tile `t`'s mesh from the pyramid in `dir`, if its level has meshes and it has one.
    pub fn read(dir: &Path, m: &Meta, t: &Tile) -> Result<Option<Self>, String> {
        if t.mesh_nv == 0 {
            return Ok(None);
        }
        Self::read_at(dir, m, t, (t.mesh_off, t.mesh_nv, t.mesh_nt))
    }

    /// One of tile `t`'s meshes, at (offset in u16s, vertices, triangles) in its level's mesh file.
    pub fn read_at(dir: &Path, m: &Meta, t: &Tile, (off, nv, nt): (u64, usize, usize)) -> Result<Option<Self>, String> {
        let Some(name) = &m.levels[t.k].mesh else { return Ok(None) };
        let path = dir.join(name);
        let mut file = File::open(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        let mut bytes = vec![0u8; (2 * nv + 3 * nt) * 2];
        file.seek(SeekFrom::Start(off * 2)).map_err(|e| e.to_string())?;
        file.read_exact(&mut bytes).map_err(|e| format!("{}: {e}", path.display()))?;
        let w: Vec<u16> = bytes.chunks_exact(2).map(|c| u16::from_le_bytes([c[0], c[1]])).collect();
        // (Stored row, column: kept column, row, as the rasters' (i, j).)
        let verts: Vec<[u16; 2]> = w[..2 * nv].chunks_exact(2).map(|p| [p[1], p[0]]).collect();
        let tris: Vec<[u16; 3]> = w[2 * nv..].chunks_exact(3).map(|p| [p[0], p[1], p[2]]).collect();
        if verts.iter().any(|v| v[0] as usize > m.tile || v[1] as usize > m.tile) || tris.iter().flatten().any(|&i| i as usize >= nv) {
            return Err(format!("{}: tile ({}, {}, {}): a vertex outside the tile or a triangle's past them", path.display(), t.k, t.row, t.col));
        }
        Ok(Some(Self::new(verts, tris, m.tile)))
    }

    pub fn new(verts: Vec<[u16; 2]>, tris: Vec<[u16; 3]>, tile: usize) -> Self {
        let side = tile.div_ceil(BUCKET);
        let mut buckets = vec![Vec::new(); side * side];
        for (n, t) in tris.iter().enumerate() {
            let p = t.map(|i| verts[i as usize]);
            let lo = |c: usize| (p.iter().map(|v| v[c] as usize).min().unwrap() / BUCKET).min(side - 1);
            let hi = |c: usize| (p.iter().map(|v| v[c] as usize).max().unwrap() / BUCKET).min(side - 1);
            for bj in lo(1)..=hi(1) {
                for bi in lo(0)..=hi(0) {
                    buckets[bj * side + bi].push(n as u32);
                }
            }
        }
        Self { verts, tris, buckets, side }
    }

    /// The triangles that may hold (u, v) (cells within the tile, fractional).
    pub fn near(&self, u: f64, v: f64) -> &[u32] {
        let b = |x: f64| ((x.max(0.0) as usize) / BUCKET).min(self.side - 1);
        &self.buckets[b(v) * self.side + b(u)]
    }

    /// The triangle holding (u, v) (cells within the tile) and its weights at its corners, if any.
    pub fn find(&self, u: f64, v: f64) -> Option<(usize, [f64; 3])> {
        const EDGE: f64 = -1e-9;
        self.near(u, v).iter().find_map(|&n| {
            let [a, b, c] = self.tris[n as usize].map(|i| self.verts[i as usize].map(|x| x as f64));
            let d = (b[1] - c[1]) * (a[0] - c[0]) + (c[0] - b[0]) * (a[1] - c[1]);
            if d == 0.0 {
                return None;
            }
            let wa = ((b[1] - c[1]) * (u - c[0]) + (c[0] - b[0]) * (v - c[1])) / d;
            let wb = ((c[1] - a[1]) * (u - c[0]) + (a[0] - c[0]) * (v - c[1])) / d;
            let wc = 1.0 - wa - wb;
            (wa >= EDGE && wb >= EDGE && wc >= EDGE).then_some((n as usize, [wa, wb, wc]))
        })
    }
}

/// The ground's height in tile `t` of level `t.k` at (u, v) (that level's samples, fractional), as the
/// renderer draws it: on the tile's mesh if it has one (None off its triangles), else on its grid (cells cut
/// corner to opposite corner; None in a cell with no ground or with a hole's). `d` holds the tile's samples.
pub fn tile_height(m: &Meta, t: &Tile, d: &Raster, mesh: Option<&TileMesh>, u: f64, v: f64) -> Option<f64> {
    let (i0, j0) = (t.col * m.tile, t.row * m.tile);
    if let Some(mesh) = mesh {
        let (n, w) = mesh.find(u - i0 as f64, v - j0 as f64)?;
        let z = mesh.tris[n].map(|k| {
            let [i, j] = mesh.verts[k as usize];
            d.z(i0 + i as usize, j0 + j as usize)
        });
        return Some(z[0] * w[0] + z[1] * w[1] + z[2] * w[2]);
    }
    let (i, j) = (u.floor() as usize, v.floor() as usize);
    let indices = [d.at(i, j), d.at(i + 1, j), d.at(i, j + 1), d.at(i + 1, j + 1)];
    let masks = indices.map(|i| d.mask[i]);
    if masks.contains(&2) || !masks.contains(&1) {
        return None;
    }
    let [a, b, c, d] = indices.map(|i| d.z[i] as f64);
    let (u, v) = (u - i as f64, v - j as f64);
    Some(if u >= v { a * (1.0 - u) + b * (u - v) + d * v } else { a * (1.0 - v) + d * u + c * (v - u) })
}

/// Level `k`'s rectangle from the pyramid in `dir`; its colours too if `rgb`.
pub fn raster(dir: &Path, m: &Meta, k: usize, i0: usize, j0: usize, width: usize, height: usize, rgb: bool) -> Result<Raster, String> {
    let l = &m.levels[k];
    let width = width.min(l.n - i0);
    let height = height.min(l.n - j0);
    let bytes = read_rect(&dir.join(&l.z), l, i0, j0, width, height, 4)?;
    let z: Vec<_> = bytes.chunks_exact(4).map(|c| f32::from_le_bytes(c.try_into().unwrap())).collect();
    if z.iter().any(|v| !v.is_finite()) {
        return Err("non-finite pyramid height".into());
    }
    let rgb = if rgb { read_rect(&dir.join(&l.rgb), l, i0, j0, width, height, 3)?.chunks_exact(3).map(|c| [c[0], c[1], c[2]]).collect() } else { vec![] };
    let mut mask = read_rect(&dir.join(&l.mask), l, i0, j0, width, height, 1)?;
    let w = l.storage();
    for j in 0..height {
        for i in 0..width {
            if j0 + j < w.row0 || j0 + j >= w.row0 + w.rows || i0 + i < w.col0 || i0 + i >= w.col0 + w.cols {
                mask[j * width + i] = 0;
            }
        }
    }
    if mask.iter().any(|&v| v > 2) {
        return Err("pyramid mask must be 0, 1 or 2".into());
    }
    Ok(Raster { n: l.n, i0, j0, width, z, rgb, mask })
}

/// The pyramid for heights: its tiles' rasters read as asked, the latest RASTERS kept.
struct Pyramid {
    dir: PathBuf,
    m: Meta,
    index: HashMap<(usize, usize, usize), usize>,
    cache: [Mutex<(u64, HashMap<usize, (u64, Arc<(Raster, Option<TileMesh>)>)>)>; SHARDS],
}

impl Pyramid {
    fn open(dir: PathBuf) -> Result<Option<Self>, String> {
        let path = dir.join("pyramid.json");
        let text = match std::fs::read_to_string(&path) {
            Ok(t) => t,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(format!("{}: {e}", path.display())),
        };
        let m: Meta = serde_json::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))?;
        let index = m.tiles.iter().enumerate().map(|(i, t)| ((t.k, t.row, t.col), i)).collect();
        Ok(Some(Self { dir, m, index, cache: std::array::from_fn(|_| Mutex::new((0, HashMap::new()))) }))
    }

    fn tile(&self, id: usize) -> Option<Arc<(Raster, Option<TileMesh>)>> {
        let shard = &self.cache[id % SHARDS];
        {
            let mut c = shard.lock().unwrap_or_else(|e| e.into_inner());
            c.0 += 1;
            let tick = c.0;
            if let Some(e) = c.1.get_mut(&id) {
                e.0 = tick;
                return Some(e.1.clone());
            }
        }
        // (Read with the cache let go: other threads' queries don't wait on the file.)
        let t = self.m.tiles[id];
        let r = raster(&self.dir, &self.m, t.k, t.col * self.m.tile, t.row * self.m.tile, self.m.tile + 1, self.m.tile + 1, false)
            .map_err(|e| log::warn!("land pyramid tile {id}: {e}"))
            .ok()?;
        let mesh = TileMesh::read(&self.dir, &self.m, &t).map_err(|e| log::warn!("land pyramid tile {id}: {e}")).ok()?;
        let r = Arc::new((r, mesh));
        let mut c = shard.lock().unwrap_or_else(|e| e.into_inner());
        c.0 += 1;
        let tick = c.0;
        if c.1.len() >= RASTERS / SHARDS
            && let Some(old) = c.1.iter().min_by_key(|e| e.1 .0).map(|e| *e.0)
        {
            c.1.remove(&old);
        }
        c.1.insert(id, (tick, r.clone()));
        Some(r)
    }

    /// The finest level's height at `dir` (package frame), as the renderer's tiles split their cells.
    /// None: the pyramid has no ground there (outside it, or a hole the pattern fills).
    fn at(&self, dir: DVec3) -> Option<f64> {
        let (x, y) = self.m.projection.project(dir)?;
        let m = &self.m;
        for k in 0..=m.top_level {
            let cell = m.cell0_m * m.scale(k) as f64;
            let u = (x - m.x0 - 0.5 * m.cell0_m) / cell;
            let v = (y - m.y0 - 0.5 * m.cell0_m) / cell;
            let n = m.levels[k].n;
            if u < 0.0 || v < 0.0 || u >= n as f64 || v >= n as f64 {
                continue;
            }
            let (i, j) = (u.floor() as usize, v.floor() as usize);
            let Some(&id) = self.index.get(&(k, j / m.tile, i / m.tile)) else { continue };
            let d = self.tile(id)?;
            return tile_height(m, &m.tiles[id], &d.0, d.1.as_ref(), u, v);
        }
        None
    }
}

/// The land package's ground for landing and contact (see the module).
pub struct LandHeights {
    /// The package's radius (m).
    pub radius: f64,
    dirs: Vec<[f32; 3]>,
    z: Vec<f32>,
    tri: Vec<u32>,
    /// The anchor mesh's tiles ([first triangle, count, first edge, edges, sea]) by cube-face cell.
    tiles: Vec<[u32; 5]>,
    cells: HashMap<(u8, u8, u8), Vec<u32>>,
    /// Each tile's triangles by SUB x SUB parts of its face cell, made the first time it is asked.
    sub: Vec<std::sync::OnceLock<SubGrid>>,
    /// The pattern's finest triangles, by PATTERN_CELL degrees of latitude and longitude.
    pattern: Vec<[u32; 3]>,
    pattern_cells: HashMap<(i32, i32), Vec<u32>>,
    pyramid: Option<Pyramid>,
    max: f64,
}

impl std::fmt::Debug for LandHeights {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "LandHeights({} vertices, {} triangles)", self.z.len(), self.tri.len() / 3)
    }
}

/// About 450 m: a few dozen of the pattern's 60 m triangles to a cell.
const PATTERN_CELL: f64 = 0.004;
/// A tile's sub-grid, SUB parts to a side.
const SUB: usize = 8;

/// A tile's triangles by where they lie on its cube face (in face cells, 32 a side).
struct SubGrid {
    face: u8,
    min: [f64; 2],
    size: [f64; 2],
    parts: Vec<Vec<u32>>,
}

/// Where `c` lies on cube face `face` (in face cells, 32 a side). None: not that side of the planet.
fn on_face(c: DVec3, face: u8) -> Option<[f64; 2]> {
    let ax = (face / 2) as usize;
    let m = if face % 2 == 1 { -c[ax] } else { c[ax] };
    (m > 1e-9).then(|| {
        let (o1, o2) = ((ax + 1) % 3, (ax + 2) % 3);
        [(c[o1] / m + 1.0) / 2.0 * 32.0, (c[o2] / m + 1.0) / 2.0 * 32.0]
    })
}

/// A cube face and its cell (32 a side) for a direction, as the lab keys the anchor mesh's tiles.
fn face_cell(c: DVec3) -> (u8, u8, u8) {
    let a = c.abs();
    let (ax, m) = if a.x >= a.y && a.x >= a.z { (0, c.x) } else if a.y >= a.z { (1, c.y) } else { (2, c.z) };
    let (o1, o2) = ((ax + 1) % 3, (ax + 2) % 3);
    let cell = |o: usize| ((((c[o] / m.abs()) + 1.0) / 2.0 * 32.0).floor().clamp(0.0, 31.0)) as u8;
    ((2 * ax + (m < 0.0) as usize) as u8, cell(o1), cell(o2))
}

fn read<T: bytemuck::Pod + bytemuck::Zeroable>(dir: &Path, name: &str) -> Result<Vec<T>, String> {
    let bytes = std::fs::read(dir.join(name)).map_err(|e| format!("{name}: {e}"))?;
    if bytes.len() % std::mem::size_of::<T>() != 0 {
        return Err(format!("{name}: not whole values"));
    }
    let mut out = vec![T::zeroed(); bytes.len() / std::mem::size_of::<T>()];
    bytemuck::cast_slice_mut::<T, u8>(&mut out).copy_from_slice(&bytes);
    Ok(out)
}

impl LandHeights {
    /// Body `key`'s land heights, made once. None: its ground is not a land package (or it could not
    /// be read: said).
    pub fn of(key: &str) -> Option<Arc<LandHeights>> {
        use std::sync::OnceLock;
        static MADE: OnceLock<Mutex<HashMap<String, Option<Arc<LandHeights>>>>> = OnceLock::new();
        let made = MADE.get_or_init(Default::default);
        if let Some(l) = made.lock().ok()?.get(key) {
            return l.clone();
        }
        let l = ground(key)?.and_then(|(folder, radius)| Self::open(&folder, radius)).map_err(|e| log::warn!("{key}: no land heights: {e}")).ok().map(Arc::new);
        made.lock().ok()?.insert(key.to_string(), l.clone());
        l
    }

    pub fn open(dir: &Path, radius: f64) -> Result<Self, String> {
        let t0 = std::time::Instant::now();
        let d: Vec<f32> = read(dir, "dir.f32")?;
        let dirs: Vec<[f32; 3]> = d.chunks_exact(3).map(|c| DVec3::new(c[0] as f64, c[1] as f64, c[2] as f64).normalize().as_vec3().to_array()).collect();
        let z: Vec<f32> = read(dir, "z.f32")?;
        if z.len() != dirs.len() {
            return Err("dir.f32 and z.f32 differ in length".into());
        }
        let tri: Vec<u32> = read(dir, "tri.u32")?;
        let tf: Vec<f32> = read(dir, "tiles.f32")?;
        let tu: Vec<u32> = read(dir, "tiles.u32")?;
        let tiles: Vec<[u32; 5]> = tu.chunks_exact(5).map(|c| [c[0], c[1], c[2], c[3], c[4]]).collect();
        if tf.len() != 6 * tiles.len() {
            return Err("tiles.f32 and tiles.u32 differ".into());
        }
        let mut cells: HashMap<(u8, u8, u8), Vec<u32>> = HashMap::new();
        for (j, t) in tiles.iter().enumerate() {
            if (t[0] as usize + t[1] as usize) * 3 > tri.len() {
                return Err(format!("tile {j} beyond tri.u32"));
            }
            cells.entry(face_cell(DVec3::new(tf[6 * j] as f64, tf[6 * j + 1] as f64, tf[6 * j + 2] as f64))).or_default().push(j as u32);
        }
        if tri.iter().any(|&v| v as usize >= z.len()) {
            return Err("tri.u32 names a vertex beyond dir.f32".into());
        }
        let lod: serde_json::Value = serde_json::from_slice(&std::fs::read(dir.join("lod.json")).map_err(|e| format!("lod.json: {e}"))?).map_err(|e| format!("lod.json: {e}"))?;
        let levels = lod["levels"].as_array().ok_or("lod.json: levels")?.len();
        let ft: Vec<u32> = read(dir, &format!("lvl_{levels}.u32"))?;
        let pattern: Vec<[u32; 3]> = ft.chunks_exact(3).map(|c| [c[0], c[1], c[2]]).filter(|t| t.iter().all(|&v| (v as usize) < z.len())).collect();
        let mut pattern_cells: HashMap<(i32, i32), Vec<u32>> = HashMap::new();
        for (i, t) in pattern.iter().enumerate() {
            let ll = t.map(|v| lab_lat_lon(DVec3::from_array(dirs[v as usize].map(|x| x as f64))));
            let (la0, la1) = (ll.iter().map(|p| p.0).fold(f64::MAX, f64::min), ll.iter().map(|p| p.0).fold(f64::MIN, f64::max));
            let (lo0, lo1) = (ll.iter().map(|p| p.1).fold(f64::MAX, f64::min), ll.iter().map(|p| p.1).fold(f64::MIN, f64::max));
            if lo1 - lo0 > 180.0 {
                continue;
            }
            for a in (la0 / PATTERN_CELL).floor() as i32..=(la1 / PATTERN_CELL).floor() as i32 {
                for b in (lo0 / PATTERN_CELL).floor() as i32..=(lo1 / PATTERN_CELL).floor() as i32 {
                    pattern_cells.entry((a, b)).or_default().push(i as u32);
                }
            }
        }
        let pyramid = Pyramid::open(dir.join("pyramid"))?;
        let max = z.iter().copied().fold(0.0f32, f32::max).max(pyramid.as_ref().map_or(0.0, |p| p.m.tiles.iter().map(|t| t.zmax).fold(0.0, f64::max) as f32)) as f64;
        log::info!("land heights: {} vertices, {} triangles, {} pattern, pyramid {} in {:.1} s from {}", z.len(), tri.len() / 3, pattern.len(), pyramid.is_some(), t0.elapsed().as_secs_f64(), dir.display());
        let sub = (0..tiles.len()).map(|_| std::sync::OnceLock::new()).collect();
        Ok(Self { radius, dirs, z, tri, tiles, cells, sub, pattern, pattern_cells, pyramid, max })
    }

    /// The highest ground (m over `radius`).
    pub fn max(&self) -> f64 {
        self.max
    }

    fn dir(&self, v: u32) -> DVec3 {
        DVec3::from_array(self.dirs[v as usize].map(|x| x as f64))
    }

    /// The height of triangle `t` under `u`, if `u` is in it.
    fn hit(&self, u: DVec3, t: [u32; 3]) -> Option<f64> {
        let [a, b, c] = t.map(|v| self.dir(v));
        let (wa, wb, wc) = (u.dot(b.cross(c)), u.dot(c.cross(a)), u.dot(a.cross(b)));
        let s = wa + wb + wc;
        (s.abs() > 0.0 && wa / s >= -1e-9 && wb / s >= -1e-9 && wc / s >= -1e-9).then(|| (wa * self.z[t[0] as usize] as f64 + wb * self.z[t[1] as usize] as f64 + wc * self.z[t[2] as usize] as f64) / s)
    }

    fn triangle(&self, k: u32) -> [u32; 3] {
        let k = 3 * k as usize;
        [self.tri[k], self.tri[k + 1], self.tri[k + 2]]
    }

    /// Tile `j`'s sub-grid on the face of its cell `face`: each triangle in every part its corners' box
    /// covers (a corner beyond the face's side: in all of them).
    fn sub_grid(&self, j: u32, face: u8) -> &SubGrid {
        self.sub[j as usize].get_or_init(|| {
            let [s0, n, ..] = self.tiles[j as usize];
            let corners: Vec<Option<[[f64; 2]; 3]>> = (s0..s0 + n)
                .map(|k| {
                    let t = self.triangle(k).map(|v| on_face(self.dir(v), face));
                    Some([t[0]?, t[1]?, t[2]?])
                })
                .collect();
            let (mut lo, mut hi) = ([f64::MAX; 2], [f64::MIN; 2]);
            for c in corners.iter().flatten().flatten() {
                for a in 0..2 {
                    lo[a] = lo[a].min(c[a]);
                    hi[a] = hi[a].max(c[a]);
                }
            }
            let size = [((hi[0] - lo[0]) / SUB as f64).max(1e-9), ((hi[1] - lo[1]) / SUB as f64).max(1e-9)];
            let part = |p: [f64; 2], a: usize| (((p[a] - lo[a]) / size[a]).floor().max(0.0) as usize).min(SUB - 1);
            let mut parts = vec![Vec::new(); SUB * SUB];
            for (i, c) in corners.iter().enumerate() {
                let k = s0 + i as u32;
                match c {
                    Some(c) => {
                        let (u0, u1) = (c.iter().map(|p| part(*p, 0)).min().unwrap(), c.iter().map(|p| part(*p, 0)).max().unwrap());
                        let (v0, v1) = (c.iter().map(|p| part(*p, 1)).min().unwrap(), c.iter().map(|p| part(*p, 1)).max().unwrap());
                        for a in u0..=u1 {
                            for b in v0..=v1 {
                                parts[a * SUB + b].push(k);
                            }
                        }
                    }
                    None => parts.iter_mut().for_each(|p| p.push(k)),
                }
            }
            SubGrid { face, min: lo, size, parts }
        })
    }

    fn in_tiles(&self, u: DVec3, cell: (u8, u8, u8)) -> Option<f64> {
        for &j in self.cells.get(&cell)? {
            let g = self.sub_grid(j, cell.0);
            let Some(p) = on_face(u, g.face) else { continue };
            let (a, b) = (((p[0] - g.min[0]) / g.size[0]).floor(), ((p[1] - g.min[1]) / g.size[1]).floor());
            if a < 0.0 || b < 0.0 || a >= SUB as f64 || b >= SUB as f64 {
                continue;
            }
            for &k in &g.parts[a as usize * SUB + b as usize] {
                if let Some(h) = self.hit(u, self.triangle(k)) {
                    return Some(h);
                }
            }
        }
        None
    }

    /// The ground's height (m over `radius`) at `dir` in the package's frame.
    pub fn at_package(&self, dir: DVec3) -> f64 {
        let u = dir.normalize();
        if let Some(h) = self.pyramid.as_ref().and_then(|p| p.at(u)) {
            return h;
        }
        let (la, lo) = lab_lat_lon(u);
        if let Some(ts) = self.pattern_cells.get(&((la / PATTERN_CELL).floor() as i32, (lo / PATTERN_CELL).floor() as i32)) {
            for &t in ts {
                if let Some(h) = self.hit(u, self.pattern[t as usize]) {
                    return h;
                }
            }
        }
        let home = face_cell(u);
        if let Some(h) = self.in_tiles(u, home) {
            return h;
        }
        // (A triangle across a cell's edge belongs to the tile its centre is in: the cells round about,
        // found by stepping a cell's width each way, over a cube edge too.)
        let (e, n) = {
            let e = DVec3::Y.cross(u).normalize_or(DVec3::X);
            (e, u.cross(e))
        };
        let step = std::f64::consts::FRAC_PI_2 / 32.0;
        let mut seen = [home; 9];
        let mut n_seen = 1;
        for (de, dn) in [(1.0, 0.0), (-1.0, 0.0), (0.0, 1.0), (0.0, -1.0), (1.0, 1.0), (1.0, -1.0), (-1.0, 1.0), (-1.0, -1.0)] {
            let c = face_cell((u + (e * de + n * dn) * step).normalize());
            if seen[..n_seen].contains(&c) {
                continue;
            }
            seen[n_seen] = c;
            n_seen += 1;
            if let Some(h) = self.in_tiles(u, c) {
                return h;
            }
        }
        0.0
    }

    /// The ground's height (m over `radius`) at `dir` in the body's frame.
    pub fn at(&self, dir: DVec3) -> f64 {
        self.at_package(to_body().inverse() * dir)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use super::lab_direction as unit;

    /// The installed e104 package (UNIVERSE_LAND_PACKAGE): the belt from the pyramid, Shasta from the
    /// pattern, the rest from the anchor mesh; the package's frame turned to the body's.
    #[test]
    #[ignore]
    fn land_heights_from_the_installed_package() {
        let dir = PathBuf::from(std::env::var("UNIVERSE_LAND_PACKAGE").expect("UNIVERSE_LAND_PACKAGE"));
        let l = LandHeights::open(&dir, 6_371_000.0).unwrap();
        let p = l.pyramid.as_ref().expect("pyramid");
        let belt = unit(97.4, -33.75);
        let h = p.at(belt).expect("the belt is in the pyramid");
        assert!((l.at_package(belt) - h).abs() < 1e-6);
        assert!((-500.0..9000.0).contains(&h), "{h}");
        let t0 = std::time::Instant::now();
        let n = 2000;
        let mut hs = vec![];
        for i in 0..n {
            let (lon, lat) = (-180.0 + 360.0 * (i as f64 * 0.618).fract(), -80.0 + 160.0 * (i as f64 / n as f64));
            let h = l.at_package(unit(lon, lat));
            assert!(h.is_finite() && h.abs() < 15_000.0, "{lon} {lat} {h}");
            hs.push(h);
        }
        let per = t0.elapsed().as_secs_f64() / n as f64 * 1e6;
        println!("belt {h:.1} m; {n} global queries {per:.1} us each; min {:.0} max {:.0}; shasta {:.1}", hs.iter().cloned().fold(f64::MAX, f64::min), hs.iter().cloned().fold(f64::MIN, f64::max), l.at_package(unit(100.47, -37.12)));
        assert!(hs.iter().filter(|h| **h == 0.0).count() < n / 10, "most queries found a triangle");
        assert!((l.at(to_body() * belt) - h).abs() < 1e-9);
        // (The body's frame: lon/lat as `super::direction` gives them.)
        let d = super::super::direction(super::super::LonLat { lon: 97.4, lat: -33.75 });
        assert!((to_body() * belt - d).length() < 1e-12);
    }

    /// Query cost round the places ships fly (UNIVERSE_LAND_PACKAGE): the port and Shasta in the pattern,
    /// the belt in the pyramid, the anchor mesh anywhere; and every answer the same as a full scan's.
    #[test]
    #[ignore]
    fn land_heights_query_cost() {
        let dir = PathBuf::from(std::env::var("UNIVERSE_LAND_PACKAGE").expect("UNIVERSE_LAND_PACKAGE"));
        let l = LandHeights::open(&dir, 6_371_000.0).unwrap();
        let mut seed = 7u64;
        let mut rnd = || { seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407); (seed >> 11) as f64 / (1u64 << 53) as f64 };
        for (name, lon, lat, spread) in [("port", 100.6679, -37.2298, 0.05), ("shasta", 100.47, -37.12, 0.2), ("belt", 97.4, -33.75, 0.3), ("world", 0.0, 0.0, 180.0)] {
            let pts: Vec<DVec3> = (0..2000).map(|_| unit(lon + spread * (2.0 * rnd() - 1.0), (lat + spread.min(80.0) * (2.0 * rnd() - 1.0)).clamp(-80.0, 80.0))).collect();
            let _ = pts.iter().map(|&p| l.at_package(p)).sum::<f64>();
            let t0 = std::time::Instant::now();
            let hs: Vec<f64> = pts.iter().map(|&p| l.at_package(p)).collect();
            println!("{name}: {:.2} us a query", t0.elapsed().as_secs_f64() / pts.len() as f64 * 1e6);
            assert!(hs.iter().all(|h| h.is_finite()));
            // (The same heights as scanning every pattern triangle, then every triangle of the cells round about.)
            for (p, h) in pts.iter().zip(&hs).step_by(20) {
                let u = p.normalize();
                let brute = l.pyramid.as_ref().and_then(|py| py.at(u))
                    .or_else(|| l.pattern.iter().find_map(|&t| l.hit(u, t)))
                    .or_else(|| {
                        let e = DVec3::Y.cross(u).normalize_or(DVec3::X);
                        let n = u.cross(e);
                        let step = std::f64::consts::FRAC_PI_2 / 32.0;
                        let mut cells = vec![face_cell(u)];
                        for (de, dn) in [(1.0, 0.0), (-1.0, 0.0), (0.0, 1.0), (0.0, -1.0), (1.0, 1.0), (1.0, -1.0), (-1.0, 1.0), (-1.0, -1.0)] {
                            let c = face_cell((u + (e * de + n * dn) * step).normalize());
                            if !cells.contains(&c) { cells.push(c); }
                        }
                        cells.iter().find_map(|c| l.cells.get(c)?.iter().find_map(|&j| { let [s0, n, ..] = l.tiles[j as usize]; (s0..s0 + n).find_map(|k| l.hit(u, l.triangle(k))) }))
                    })
                    .unwrap_or(0.0);
                assert!((brute - h).abs() < 1e-6, "{name}: {h} indexed, {brute} scanned at {u:?}");
            }
        }
    }
}
