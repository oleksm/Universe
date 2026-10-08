//! A world's surface bake as heights: its tiles by level on the cube's faces, read where
//! they're wanted (the coarse pyramid loaded whole, the fine tiles on demand).

use std::collections::HashMap;
use std::path::Path;

use serde::Deserialize;

use super::*;
use super::releases::decode;

/// How fine a height is wanted (see `Heights::at_detail`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Detail {
    /// The 600 m tiles, waited for if they aren't read yet: what the physics stands on.
    Full,
    /// The 600 m tiles read so far; any missing is read in the background meanwhile (for
    /// drawing, which can't wait).
    Loaded,
    /// The 5 km heights alone (a whole globe's map).
    Coarse,
}

/// A world's ground from its bake (`planet-sim-surface/1`): the 5 km heights over the whole
/// sphere, read the first time a height is wanted, and the 600 m tiles (where there is land) as
/// the difference from them, read as they're wanted and kept while there's room (`TILE_BUDGET`):
/// the least lately used go first, and are read again if wanted again (the same files: the same
/// heights). Metres from the sea.
pub struct Heights {
    bake: Bake,
    /// The 5 km heights' layout (rows, columns of tiles; a tile's size) and the heights, raw
    /// (R·256 + G): equirectangular, row 0 north, column 0 at −180°.
    index: (usize, usize, usize, usize),
    coarse: std::sync::OnceLock<Vec<u16>>,
    width: usize,
    height: usize,
    /// The 600 m tiles: (face, x, y) at `level`, each `n + 1` samples a side, raw (R·256 + G);
    /// those read, by slot, with when each was last used.
    level: u32,
    n: usize,
    fine: std::sync::RwLock<HashMap<usize, (std::sync::Arc<Tile>, std::sync::atomic::AtomicU64)>>,
    clock: std::sync::atomic::AtomicU64,
    /// The ~150 m tiles over mountains (layer B: `fz150.json`, `fz150_<face>_<x>_<y>.png`), where
    /// the bake has them: their level and samples a side (`n + 1`); their slots follow the 600 m
    /// tiles' and the river tiles'.
    level150: Option<(u32, usize)>,
    /// The 5 km rock map (`rockid.png`: a rock unit's number a texel, equirectangular), read the
    /// first time the runtime detail wants it.
    rocks: std::sync::OnceLock<Option<(usize, usize, Vec<u8>)>>,
    /// Tiles asked for in the background.
    asked: Vec<std::sync::atomic::AtomicBool>,
    /// The highest the ground stands (m).
    pub max: f64,
}

/// A fine tile kept: a 600 m tile's heights (raw, `n + 1` a side), or a river tile's surface
/// fields (wetness, scree, bare rock; `n / 2 + 1` a side, the tile's lower block).
enum Tile {
    Heights(Vec<u16>),
    Surface(Vec<[u8; 3]>),
    /// A ~150 m tile's fields (`fd150`: flow, drainage, threshold slope, ice), RGBA.
    Fields(Vec<[u8; 4]>),
}

impl Tile {
    fn bytes(&self) -> usize {
        match self {
            Tile::Heights(h) => h.len() * 2,
            Tile::Surface(s) => s.len() * 3,
            Tile::Fields(f) => f.len() * 4,
        }
    }
}

/// The fine tiles kept, at most (bytes; `UNIVERSE_TILE_MB` to set it): a tile is about 2.1 MB.
fn tile_budget() -> usize {
    static BUDGET: std::sync::OnceLock<usize> = std::sync::OnceLock::new();
    *BUDGET.get_or_init(|| std::env::var("UNIVERSE_TILE_MB").ok().and_then(|v| v.parse::<usize>().ok()).unwrap_or(384) << 20)
}

impl std::fmt::Debug for Heights {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Heights({}×{}, max {:.0} m)", self.width, self.height, self.max)
    }
}

/// An 8-bit RGB PNG's pixels, and its size.
fn png_rgb(bytes: &[u8]) -> Option<(usize, usize, Vec<[u8; 3]>)> {
    let mut d = png::Decoder::new(std::io::Cursor::new(bytes));
    d.set_transformations(png::Transformations::IDENTITY);
    let mut r = d.read_info().ok()?;
    let mut buf = vec![0; r.output_buffer_size()?];
    let info = r.next_frame(&mut buf).ok()?;
    let (w, h) = (info.width as usize, info.height as usize);
    let per = info.line_size / w.max(1);
    (per >= 3).then(|| (w, h, (0..h * w).map(|i| { let o = (i / w) * info.line_size + (i % w) * per; [buf[o], buf[o + 1], buf[o + 2]] }).collect()))
}

/// An 8-bit RGB PNG's (R·256 + G) per pixel, and its size.
fn png_rg(bytes: &[u8]) -> Result<(usize, usize, Vec<u16>), String> {
    let mut d = png::Decoder::new(std::io::Cursor::new(bytes));
    d.set_transformations(png::Transformations::IDENTITY);
    let mut r = d.read_info().map_err(|e| e.to_string())?;
    let mut buf = vec![0; r.output_buffer_size().ok_or("too big")?];
    let info = r.next_frame(&mut buf).map_err(|e| e.to_string())?;
    let (w, h) = (info.width as usize, info.height as usize);
    let per = info.line_size / w.max(1);
    Ok((w, h, (0..h * w).map(|i| (i / w, i % w)).map(|(y, x)| buf[y * info.line_size + x * per] as u16 * 256 + buf[y * info.line_size + x * per + 1] as u16).collect()))
}

impl Heights {
    /// Body `key`'s heights, read once per run and shared (None: no bake, or none to be had:
    /// no store here, or not the bake the record names; said once on stderr).
    pub fn of(key: &str) -> Option<std::sync::Arc<Heights>> {
        Self::shared(key, || Bake::open(key))
    }

    /// A released world's heights from its surface package (see `Bake::release`), read once per
    /// run and shared.
    pub fn of_release(r: &Release) -> Option<std::sync::Arc<Heights>> {
        Self::shared(&format!("release {} v{}", r.world_id, r.surface.unwrap_or(0)), || Bake::release(r))
    }

    fn shared(key: &str, open: impl FnOnce() -> Option<Result<Bake, String>>) -> Option<std::sync::Arc<Heights>> {
        static LOADED: std::sync::OnceLock<std::sync::Mutex<HashMap<String, Option<std::sync::Arc<Heights>>>>> = std::sync::OnceLock::new();
        let mut loaded = LOADED.get_or_init(Default::default).lock().unwrap_or_else(|e| e.into_inner());
        loaded
            .entry(key.to_string())
            .or_insert_with(|| match open()?.and_then(Heights::read) {
                Ok(h) => Some(std::sync::Arc::new(h)),
                Err(e) => {
                    eprintln!("{key}: its bake can't be read ({e}); its ground is the seed's");
                    None
                }
            })
            .clone()
    }

    fn read(bake: Bake) -> Result<Heights, String> {
        #[derive(Deserialize)]
        struct Index {
            rows: usize,
            cols: usize,
            tile: [usize; 2],
        }
        #[derive(Deserialize)]
        struct Fine {
            level: u32,
            n: usize,
        }
        let ix: Index = serde_json::from_slice(&bake.read("hz.json")?).map_err(|e| e.to_string())?;
        let (tw, th) = (ix.tile[0], ix.tile[1]);
        let fz: serde_json::Value = serde_json::from_slice(&bake.read("fz.json")?).map_err(|e| e.to_string())?;
        let fine: Fine = serde_json::from_value(fz.clone()).map_err(|e| e.to_string())?;
        // (The highest ground from the tiles' bounds, where the bake has them.)
        let bound = fz.get("bounds").and_then(|b| b.as_object()).and_then(|b| b.values().filter_map(|v| v.get(1)?.as_f64()).reduce(f64::max));
        let tiles = 6 * (1usize << fine.level).pow(2);
        // (The ~150 m level, if baked: as fz.json, its own level and size.)
        let level150 = bake.has("fz150.json").then(|| bake.read("fz150.json")).transpose()?.map(|b| serde_json::from_slice::<Fine>(&b)).transpose().map_err(|e| e.to_string())?.map(|f| (f.level, f.n));
        let tiles150 = level150.map_or(0, |(l, _)| 6 * (1usize << l).pow(2));
        // (The highest ground from the bake's peaks, with room: the heights themselves aren't read
        // till one's wanted.)
        #[derive(Deserialize)]
        struct Peak {
            h: f64,
        }
        let top = bound.or_else(|| bake.read("peaks.json").ok().and_then(|b| serde_json::from_slice::<Vec<Peak>>(&b).ok()).and_then(|p| p.iter().map(|p| p.h).reduce(f64::max))).unwrap_or(9_000.0);
        Ok(Heights {
            bake,
            index: (ix.rows, ix.cols, tw, th),
            coarse: std::sync::OnceLock::new(),
            width: ix.cols * tw,
            height: ix.rows * th,
            level: fine.level,
            n: fine.n,
            fine: Default::default(),
            clock: Default::default(),
            level150,
            rocks: std::sync::OnceLock::new(),
            asked: (0..2 * tiles + 2 * tiles150).map(|_| Default::default()).collect(),
            max: top + 500.0,
        })
    }

    /// The 5 km heights, read the first time they're wanted.
    fn coarse(&self) -> &[u16] {
        self.coarse.get_or_init(|| {
            let (rows, cols, tw, th) = self.index;
            let mut coarse = vec![0u16; self.width * self.height];
            for r in 0..rows {
                for c in 0..cols {
                    let Ok((w, h, px)) = self.bake.read(&format!("hz_{r}_{c}.png")).and_then(|b| png_rg(&b)) else { continue };
                    for y in 0..h.min(th) {
                        let at = (r * th + y) * self.width + c * tw;
                        coarse[at..at + w.min(tw)].copy_from_slice(&px[y * w..y * w + w.min(tw)]);
                    }
                }
            }
            coarse
        })
    }

    /// Fine tile `slot` if it's kept (marked used now). (A river tile's slot is past the height
    /// tiles': `slot + asked.len() / 2`.)
    fn kept(&self, slot: usize) -> Option<std::sync::Arc<Tile>> {
        let fine = self.fine.read().unwrap_or_else(|e| e.into_inner());
        let (t, used) = fine.get(&slot)?;
        used.store(self.clock.fetch_add(1, std::sync::atomic::Ordering::Relaxed), std::sync::atomic::Ordering::Relaxed);
        Some(t.clone())
    }

    /// Keep fine tile `slot`, the least lately used let go past the budget.
    fn keep(&self, slot: usize, tile: std::sync::Arc<Tile>) {
        let mut fine = self.fine.write().unwrap_or_else(|e| e.into_inner());
        let now = self.clock.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        fine.insert(slot, (tile, std::sync::atomic::AtomicU64::new(now)));
        let bytes = |f: &HashMap<usize, (std::sync::Arc<Tile>, std::sync::atomic::AtomicU64)>| f.values().map(|(t, _)| t.bytes()).sum::<usize>();
        while bytes(&fine) > tile_budget() && fine.len() > 1 {
            let Some(oldest) = fine.iter().filter(|(k, _)| **k != slot).min_by_key(|(_, (_, u))| u.load(std::sync::atomic::Ordering::Relaxed)).map(|(k, _)| *k) else { break };
            fine.remove(&oldest);
            self.asked[oldest].store(false, std::sync::atomic::Ordering::Relaxed);
        }
    }

    /// How many fine tiles are kept, and their bytes.
    pub fn kept_tiles(&self) -> (usize, usize) {
        let fine = self.fine.read().unwrap_or_else(|e| e.into_inner());
        (fine.len(), fine.values().map(|(t, _)| t.bytes()).sum())
    }

    /// The 5 km height at `p` (m), bilinear.
    fn coarse_at(&self, p: LonLat) -> f64 {
        let (w, h) = (self.width as f64, self.height as f64);
        let row = ((90.0 - p.lat) / 180.0 * h - 0.5).clamp(0.0, h - 1.0);
        let col = (p.lon + 180.0) / 360.0 * w - 0.5;
        let (r0, c0) = (row.floor(), col.floor());
        let (fr, fc) = (row - r0, col - c0);
        let coarse = self.coarse();
        let at = |r: f64, c: f64| {
            let r = (r as usize).min(self.height - 1);
            let c = (c as isize).rem_euclid(self.width as isize) as usize;
            coarse[r * self.width + c] as f64
        };
        let v = at(r0, c0) * (1.0 - fr) * (1.0 - fc) + at(r0, c0 + 1.0) * (1.0 - fr) * fc + at(r0 + 1.0, c0) * fr * (1.0 - fc) + at(r0 + 1.0, c0 + 1.0) * fr * fc;
        v / 2.0 - 12_000.0
    }

    /// The 600 m surface fields at body direction `d` (wetness, scree, bare rock, 0..1 each;
    /// bilinear), from the river tiles' lower block, to `detail`; and whether they're the whole
    /// of it. None where the bake has no tile (the sea's).
    pub fn surface_at(self: &std::sync::Arc<Self>, d: glam::DVec3, detail: Detail) -> (Option<[f32; 3]>, bool) {
        if detail == Detail::Coarse {
            return (None, true);
        }
        let d = d.normalize();
        let (face, x, y, slot, fu, fv) = self.locate(d);
        let (t, whole) = self.tile(slot + self.asked.len() / 2, format!("rv_{face}_{x}_{y}.png"), detail, |b, n| {
            let (w, h, rgb) = png_rgb(b)?;
            let m = n / 2 + 1;
            (w > m && h >= n + 1 + m).then(|| Tile::Surface((0..m * m).map(|i| rgb[(n + 1 + i / m) * w + i % m]).collect()))
        });
        let Some(t) = t else { return (None, whole) };
        let Tile::Surface(s) = &*t else { return (None, true) };
        let m = self.n / 2 + 1;
        let (su, sv) = (fu * (m - 1) as f64, fv * (m - 1) as f64);
        let (i0, j0) = ((su.floor() as usize).min(m - 2), (sv.floor() as usize).min(m - 2));
        let (fi, fj) = ((su - i0 as f64) as f32, (sv - j0 as f64) as f32);
        let at = |i: usize, j: usize, c: usize| s[j * m + i][c] as f32 / 255.0;
        (Some(std::array::from_fn(|c| at(i0, j0, c) * (1.0 - fi) * (1.0 - fj) + at(i0 + 1, j0, c) * fi * (1.0 - fj) + at(i0, j0 + 1, c) * (1.0 - fi) * fj + at(i0 + 1, j0 + 1, c) * fi * fj)), true)
    }

    /// Where body direction `d` falls on the fine tiles: (face, x, y, its slot, and the place
    /// within the tile, 0..1 each way).
    fn locate(&self, d: glam::DVec3) -> (usize, usize, usize, usize, f64, f64) {
        locate(d, self.level)
    }

    /// The ~150 m difference at body direction `d` (m: from the 600 m heights, read bilinearly),
    /// bilinear; 0 where there's no tile (all but mountains). And whether it's the whole of it.
    fn fine150_at(self: &std::sync::Arc<Self>, d: glam::DVec3, detail: Detail) -> (f64, bool) {
        let Some((level, n)) = self.level150.filter(|_| detail != Detail::Coarse) else { return (0.0, true) };
        let (face, x, y, slot, fu, fv) = locate(d, level);
        // (Its slots after the 600 m tiles' and the river tiles'.)
        let base = 2 * 6 * (1usize << self.level).pow(2);
        let (t, whole) = self.tile(base + slot, format!("fz150_{face}_{x}_{y}.png"), detail, |b, _| png_rg(b).ok().map(|t| Tile::Heights(t.2)));
        let Some(t) = t else { return (0.0, whole) };
        let Tile::Heights(t) = &*t else { return (0.0, true) };
        if t.len() < (n + 1) * (n + 1) {
            return (0.0, true);
        }
        let (su, sv) = (fu * n as f64, fv * n as f64);
        let (i0, j0) = ((su.floor() as usize).min(n - 1), (sv.floor() as usize).min(n - 1));
        let (fi, fj) = (su - i0 as f64, sv - j0 as f64);
        let at = |i: usize, j: usize| t[j * (n + 1) + i] as f64;
        let raw = at(i0, j0) * (1.0 - fi) * (1.0 - fj) + at(i0 + 1, j0) * fi * (1.0 - fj) + at(i0, j0 + 1) * (1.0 - fi) * fj + at(i0 + 1, j0 + 1) * fi * fj;
        // (Quarter-metres: the differences are small.)
        ((raw - 32_768.0) / 4.0, whole)
    }
}

/// The body direction at `u`, `v` (−1..1 each, equal-angle) on cube face `face`: `locate`'s
/// inverse.
pub fn cube_dir(face: usize, u: f64, v: f64) -> glam::DVec3 {
    let (tu, tv) = ((u * std::f64::consts::FRAC_PI_4).tan(), (v * std::f64::consts::FRAC_PI_4).tan());
    match face {
        0 => glam::DVec3::new(1.0, -tv, -tu),
        1 => glam::DVec3::new(-1.0, -tv, tu),
        2 => glam::DVec3::new(tu, 1.0, tv),
        3 => glam::DVec3::new(tu, -1.0, -tv),
        4 => glam::DVec3::new(tu, -tv, 1.0),
        _ => glam::DVec3::new(-tu, -tv, -1.0),
    }
    .normalize()
}

/// Where body direction `d` falls on cube tiles at `level`: (face, x, y, its slot, and the place
/// within the tile, 0..1 each way).
pub(super) fn locate(d: glam::DVec3, level: u32) -> (usize, usize, usize, usize, f64, f64) {
    // (The tiles' cube is in the body's own frame: face by the largest axis, equal-angle u, v.)
    let a = d.abs();
    let (face, tu, tv) = if a.x >= a.y && a.x >= a.z {
        if d.x > 0.0 { (0, -d.z / d.x, -d.y / d.x) } else { (1, d.z / -d.x, -d.y / -d.x) }
    } else if a.y >= a.z {
        if d.y > 0.0 { (2, d.x / d.y, d.z / d.y) } else { (3, d.x / -d.y, -d.z / -d.y) }
    } else if d.z > 0.0 {
        (4, d.x / d.z, -d.y / d.z)
    } else {
        (5, -d.x / -d.z, -d.y / -d.z)
    };
    let k = 1usize << level;
    let to = |t: f64| ((t.atan() * 4.0 / std::f64::consts::PI + 1.0) / 2.0 * k as f64).clamp(0.0, k as f64 - 1e-9);
    let (u, v) = (to(tu), to(tv));
    let (x, y) = (u.floor() as usize, v.floor() as usize);
    (face, x, y, (face * k + x) * k + y, u - x as f64, v - y as f64)
}

impl Heights {
    /// Tile `slot` (its file `name`) to `detail`: kept, read now (`Full`), or asked for in the
    /// background (`Loaded`: None and false meanwhile). Some(None): the bake has no such tile.
    fn tile(self: &std::sync::Arc<Self>, slot: usize, name: String, detail: Detail, read: fn(&[u8], usize) -> Option<Tile>) -> (Option<std::sync::Arc<Tile>>, bool) {
        if let Some(t) = self.kept(slot) {
            return (Some(t), true);
        }
        if !self.bake.has(&name) {
            return (None, true);
        }
        let n = self.n;
        match detail {
            Detail::Full => match self.bake.read(&name).ok().and_then(|b| read(&b, n)) {
                Some(t) => {
                    let t = std::sync::Arc::new(t);
                    self.keep(slot, t.clone());
                    (Some(t), true)
                }
                None => (None, true),
            },
            _ => {
                if !self.asked[slot].swap(true, std::sync::atomic::Ordering::Relaxed) {
                    let me = self.clone();
                    rayon::spawn(move || {
                        if let Some(t) = me.bake.read(&name).ok().and_then(|b| read(&b, n)) {
                            me.keep(slot, std::sync::Arc::new(t));
                        }
                    });
                }
                (None, false)
            }
        }
    }

    /// The 600 m difference at body direction `d` (m), bilinear; 0 where there's no tile. And
    /// whether it's the whole of it (false: a tile not read yet, at `Detail::Loaded`).
    fn fine_at(self: &std::sync::Arc<Self>, d: glam::DVec3, detail: Detail) -> (f64, bool) {
        if detail == Detail::Coarse {
            return (0.0, true);
        }
        let (face, x, y, slot, fu, fv) = self.locate(d);
        let name = || format!("fz_{face}_{x}_{y}.png");
        // (Where the bake has no tile (the sea's), the 5 km heights alone.)
        let (t, whole) = self.tile(slot, name(), detail, |b, _| png_rg(b).ok().map(|t| Tile::Heights(t.2)));
        let Some(t) = t else { return (0.0, whole) };
        let Tile::Heights(t) = &*t else { return (0.0, true) };
        let n = self.n;
        let (su, sv) = (fu * n as f64, fv * n as f64);
        let (i0, j0) = ((su.floor() as usize).min(n - 1), (sv.floor() as usize).min(n - 1));
        let (fi, fj) = (su - i0 as f64, sv - j0 as f64);
        let at = |i: usize, j: usize| t[j * (n + 1) + i] as f64;
        let raw = at(i0, j0) * (1.0 - fi) * (1.0 - fj) + at(i0 + 1, j0) * fi * (1.0 - fj) + at(i0, j0 + 1) * (1.0 - fi) * fj + at(i0 + 1, j0 + 1) * fi * fj;
        ((raw - 32_768.0) / 2.0, true)
    }

    /// Image `name` of the bake (a JPEG or PNG), as RGBA8: its width, height and pixels. None:
    /// not in the bake, or unreadable.
    pub fn image(&self, name: &str) -> Option<(usize, usize, Vec<u8>)> {
        decode(name, &self.bake.read(name).ok()?)
    }

    /// The world's air from its bake (`atmosphere.json`, SI), packed as the engine's `Air` takes
    /// it (16 floats): Rayleigh's β (rgb, per m) and scale height (m); Mie's scattering and
    /// extinction (per m), scale height (m) and g; ozone's absorption at its peak (rgb, per m), its
    /// peak's height and half-width (m); the air's top (m) and the world's radius (m); 1 (it has
    /// air). None: none baked.
    pub fn air(&self) -> Option<[f32; 16]> {
        let v: serde_json::Value = serde_json::from_slice(&self.bake.read("atmosphere.json").ok()?).ok()?;
        let f = |p: &[&str]| p.iter().try_fold(&v, |v, k| v.get(k)).and_then(serde_json::Value::as_f64).map(|x| x as f32);
        let rgb = |p: &[&str]| -> Option<[f32; 3]> {
            let a = p.iter().try_fold(&v, |v, k| v.get(k))?.as_array()?;
            Some([a.first()?.as_f64()? as f32, a.get(1)?.as_f64()? as f32, a.get(2)?.as_f64()? as f32])
        };
        let (br, oz) = (rgb(&["rayleigh", "beta_per_m"])?, rgb(&["ozone", "absorb_per_m"]).unwrap_or([0.0; 3]));
        Some([
            br[0], br[1], br[2], f(&["rayleigh", "scale_height_m"])?,
            f(&["mie", "scatter_per_m"]).unwrap_or(0.0), f(&["mie", "extinct_per_m"]).unwrap_or(0.0), f(&["mie", "scale_height_m"]).unwrap_or(1.0), f(&["mie", "g"]).unwrap_or(0.0),
            oz[0], oz[1], oz[2], f(&["ozone", "peak_m"]).unwrap_or(0.0),
            f(&["ozone", "half_width_m"]).unwrap_or(1.0), f(&["top_m"])?, f(&["planet_radius_m"])?, 1.0,
        ])
    }

    /// The air's tables from the bake (`air_luts.json`: the lab's, Hillaire 2020): the sun's
    /// transmittance (256 × 64) and the light of scattering's higher orders (32 × 32), RGBA
    /// floats row by row, as `air_luts.json` names their files and sizes. None: none baked. (`UNIVERSE_AIR_LUTS`: a
    /// folder of them to use instead, to try a world's before its bake has them.)
    pub fn air_luts(&self) -> Option<AirLuts> {
        let read = |name: &str| -> Option<Vec<u8>> {
            match std::env::var_os("UNIVERSE_AIR_LUTS") {
                Some(dir) => std::fs::read(Path::new(&dir).join(name)).ok(),
                None => self.bake.read(name).ok(),
            }
        };
        let info: serde_json::Value = serde_json::from_slice(&read("air_luts.json")?).ok()?;
        let floats = |b: Vec<u8>| -> Vec<f32> { b.chunks_exact(4).map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]])).collect() };
        // (Each table: its file, width and height, as the contract names them.)
        let table = |k: &str| -> Option<(u32, u32, Vec<f32>)> {
            let t = info.get(k)?;
            let (w, h) = (t.get("width")?.as_u64()? as u32, t.get("height")?.as_u64()? as u32);
            Some((w, h, floats(read(t.get("file")?.as_str()?)?)))
        };
        let (tw, th, transmittance) = table("transmittance")?;
        let (mw, mh, multiscatter) = table("multiscatter")?;
        (transmittance.len() == (tw * th * 4) as usize && multiscatter.len() == (mw * mh * 4) as usize).then_some(AirLuts { transmittance: (tw, th, transmittance), multiscatter: (mw, mh, multiscatter) })
    }

    /// The world's clouds from its bake (`clouds.json` and its three maps: by month, El Niño's
    /// change, the air they sit in; each RGBA8), with its year (days) and its El Niño's series
    /// (days a world month, the index a month). None: none baked.
    pub fn clouds(&self) -> Option<CloudsBake> {
        let info: serde_json::Value = serde_json::from_slice(&self.bake.read("clouds.json").ok()?).ok()?;
        let map = |name: &str| self.image(name);
        let maps = [map("clouds_month.png")?, map("clouds_enso.png")?, map("clouds_air.png")?];
        let year_days = info.get("year_days")?.as_f64()?;
        let enso = info.get("enso").filter(|e| !e.is_null()).and_then(|e| Some((e.get("month_days")?.as_f64()?, e.get("index")?.as_array()?.iter().filter_map(|v| v.as_f64().map(|x| x as f32)).collect::<Vec<f32>>())));
        let format = info.get("format").and_then(|f| f.as_str()).and_then(|f| f.strip_prefix("planet-sim-clouds/")).and_then(|v| v.parse().ok()).unwrap_or(1);
        Some(CloudsBake { maps, year_days, enso, format })
    }

    /// The world's true colour from its bake (`globe_color.jpg`: equirectangular, as the 5 km
    /// heights), read now (not kept: a globe's map is made from it once). None: none baked.
    pub fn colour(&self) -> Option<Equirect> {
        let bytes = self.bake.read("globe_color.jpg").ok()?;
        let mut d = zune_jpeg::JpegDecoder::new(&bytes);
        let rgb = d.decode().ok()?;
        let (width, height) = d.dimensions()?;
        (rgb.len() == width * height * 3).then_some(Equirect { width, height, rgb })
    }

    /// The ground's height (m from the sea; below 0, the sea floor) at body direction `dir`,
    /// in full.
    pub fn at(self: &std::sync::Arc<Self>, dir: glam::DVec3) -> f64 {
        self.at_detail(dir, Detail::Full).0
    }

    /// The runtime detail at body direction `dir` (m, over the ~150 m ground; see `detail`), to the
    /// band `cell` (m), on a world of `radius` (m); and whether it's the whole of it. 0 where
    /// there's no ~150 m tile, or the generator isn't in yet (`detail::ACTIVE`).
    pub fn detail_at(self: &std::sync::Arc<Self>, dir: glam::DVec3, detail: Detail, cell: f64, radius: f64) -> (f64, bool) {
        let Some((level, n)) = self.level150.filter(|_| crate::detail::ACTIVE && detail != Detail::Coarse) else { return (0.0, true) };
        let (face, x, y, _, fu, fv) = locate(dir.normalize(), level);
        let k = 1usize << level;
        let tiles150 = 6 * k * k;
        let base = 2 * 6 * (1usize << self.level).pow(2);
        let whole = std::cell::Cell::new(true);
        // (Its tile and the neighbours on its face, read to `detail`, for the halo; past a face's
        // edge the nearest sample of its own.)
        let neighbour = |dx: i64, dy: i64, fields: bool| -> Option<std::sync::Arc<Tile>> {
            let (tx, ty) = (x as i64 + dx, y as i64 + dy);
            if tx < 0 || ty < 0 || tx >= k as i64 || ty >= k as i64 {
                return None;
            }
            let slot = (face * k + tx as usize) * k + ty as usize;
            let (t, w) = if fields {
                self.tile(base + tiles150 + slot, format!("fd150_{face}_{tx}_{ty}.png"), detail, |b, _| decode("fd150.png", b).map(|(_, _, px)| Tile::Fields(px.chunks(4).map(|c| [c[0], c[1], c[2], c[3]]).collect())))
            } else {
                self.tile(base + slot, format!("fz150_{face}_{tx}_{ty}.png"), detail, |b, _| png_rg(b).ok().map(|t| Tile::Heights(t.2)))
            };
            whole.set(whole.get() && w);
            t
        };
        if neighbour(0, 0, false).is_none() {
            return (0.0, whole.get());
        }
        let sample = |i: i64, j: i64, m: usize, fields: bool| -> Option<(std::sync::Arc<Tile>, usize)> {
            let (dx, dy) = (i.div_euclid(m as i64), j.div_euclid(m as i64));
            let (t, (i, j)) = match neighbour(dx.clamp(-1, 1), dy.clamp(-1, 1), fields) {
                Some(t) => (t, (i - dx * m as i64, j - dy * m as i64)),
                None => (neighbour(0, 0, fields)?, (i.clamp(0, m as i64), j.clamp(0, m as i64))),
            };
            Some((t, j as usize * (m + 1) + i as usize))
        };
        // (The ground's whole height at a sample of the tile: the 5 km, 600 m and ~150 m levels,
        // as the ground reads them there.)
        let height = |i: i64, j: i64| {
            let to = |c: usize, s: i64| (c as f64 + s as f64 / n as f64) / k as f64 * 2.0 - 1.0;
            self.at_detail(cube_dir(face, to(x, i), to(y, j)), detail).0
        };
        let fields = |i: i64, j: i64| match sample(i, j, n / 2, true) {
            Some((t, at)) => match &*t {
                Tile::Fields(f) => f.get(at).map_or([0.0; 4], |c| c.map(|v| v as f64)),
                _ => [0.0; 4],
            },
            None => [0.0; 4],
        };
        // (On the face in metres: its width a quarter turn of the world; the tile's corner floored
        // to a whole metre, the place from there.)
        let face_m = std::f64::consts::FRAC_PI_2 * radius;
        let tile_m = face_m / k as f64;
        let spacing = tile_m / n as f64;
        let corner = [x as f64 * tile_m, y as f64 * tile_m];
        let origin = corner.map(|c| c.floor() as i64);
        let site = crate::detail::Site { origin, at: [corner[0] - origin[0] as f64 + fu * tile_m, corner[1] - origin[1] as f64 + fv * tile_m], spacing, height: &height, fields: &fields, rock: self.rock_at(lon_lat(dir)), seed: crate::detail::seed(&self.bake.world), cell };
        (crate::detail::offset(&site), whole.get())
    }

    /// The rock unit under `p` (the 5 km rock map's number, nearest texel; 0 without one). The
    /// map holds a unit's number times 8.
    fn rock_at(&self, p: LonLat) -> u32 {
        let Some((w, h, px)) = self.rocks.get_or_init(|| self.image("rockid.png")) else { return 0 };
        let row = (((90.0 - p.lat) / 180.0 * *h as f64) as usize).min(h - 1);
        let col = (((p.lon + 180.0) / 360.0 * *w as f64) as usize).min(w - 1);
        (px[(row * w + col) * 4] as u32 + 4) / 8
    }

    /// The ground's height at `dir` to `detail`, and whether it's the whole of it.
    pub fn at_detail(self: &std::sync::Arc<Self>, dir: glam::DVec3, detail: Detail) -> (f64, bool) {
        let d = dir.normalize();
        let (fine, whole) = self.fine_at(d, detail);
        let (fine150, whole150) = self.fine150_at(d, detail);
        (self.coarse_at(lon_lat(d)) + fine + fine150, whole && whole150)
    }
}
