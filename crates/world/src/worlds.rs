//! Worlds grown by the planet simulation (`docs/survey-contract.md`): a body's
//! record names its **survey** (rock map, deposits, districts, what each rock
//! yields) and its **energy** package (petroleum basins, oil and gas fields,
//! coalfields), both kept in the registry, and its **surface bake** (heights,
//! rivers, textures), kept in a worlds store outside it. All by reference:
//! each package is checked against the hash its record gives, and the bake's
//! files each against its manifest, and a mismatch is refused.
//!
//! The worlds store is a folder (`UNIVERSE_WORLDS`, else `~/git/planet-sim/out`
//! where it exists) holding `worlds/<world_id>/surface/v<N>/`; any copy will
//! do, since files are fetched by their hashes.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::Deserialize;
use sha2::{Digest, Sha256};

use crate::registry::{registry, Body};

/// A barrel (m³).
const BARREL: f64 = 0.158_987_294_928;
/// A billion cubic feet (m³).
const BCF: f64 = 2.831_684_659_2e7;

/// Where the registry's files lie: `UNIVERSE_ROOT`, else the source tree this was built from.
pub fn root() -> PathBuf {
    std::env::var_os("UNIVERSE_ROOT").map(PathBuf::from).unwrap_or_else(|| Path::new(env!("CARGO_MANIFEST_DIR")).join("../.."))
}

/// The worlds store: `UNIVERSE_WORLDS`, else the planet simulation's output beside this tree
/// (`~/git/planet-sim/out`), if either exists.
pub fn store() -> Option<PathBuf> {
    if let Some(p) = std::env::var_os("UNIVERSE_WORLDS") {
        return Some(PathBuf::from(p));
    }
    let home = std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE"))?;
    Some(PathBuf::from(home).join("git/planet-sim/out")).filter(|p| p.is_dir())
}

/// The SHA-256 of `bytes`, in hex.
pub fn sha256(bytes: &[u8]) -> String {
    Sha256::digest(bytes).iter().map(|b| format!("{b:02x}")).collect()
}

/// A package's manifest: its files and their hashes.
#[derive(Deserialize)]
struct Manifest {
    files: HashMap<String, FileEntry>,
}

#[derive(Deserialize)]
struct FileEntry {
    sha256: String,
}

/// A package folder, its manifest checked against `want` (the record's hash).
struct Package {
    folder: PathBuf,
    files: HashMap<String, String>,
}

impl Package {
    fn open(folder: PathBuf, want: &str) -> Result<Self, String> {
        let bytes = std::fs::read(folder.join("manifest.json")).map_err(|e| format!("{}: {e}", folder.display()))?;
        let got = sha256(&bytes);
        if got != want {
            return Err(format!("{}: manifest is {got}, the record says {want}", folder.display()));
        }
        let m: Manifest = serde_json::from_slice(&bytes).map_err(|e| format!("{}: {e}", folder.display()))?;
        Ok(Package { folder, files: m.files.into_iter().map(|(k, v)| (k, v.sha256)).collect() })
    }

    /// File `name`'s bytes, checked against the manifest.
    fn read(&self, name: &str) -> Result<Vec<u8>, String> {
        let want = self.files.get(name).ok_or_else(|| format!("{}: {name} is not in its manifest", self.folder.display()))?;
        let bytes = std::fs::read(self.folder.join(name)).map_err(|e| format!("{}/{name}: {e}", self.folder.display()))?;
        if &sha256(&bytes) != want {
            return Err(format!("{}/{name}: not the file its manifest names", self.folder.display()));
        }
        Ok(bytes)
    }

    fn json<T: for<'a> Deserialize<'a>>(&self, name: &str) -> Result<T, String> {
        serde_json::from_slice(&self.read(name)?).map_err(|e| format!("{}/{name}: {e}", self.folder.display()))
    }
}

/// A place on a body's sphere: longitude and latitude (degrees).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LonLat {
    pub lon: f64,
    pub lat: f64,
}

/// A mineral deposit, as the survey has it.
#[derive(Clone, Debug)]
pub struct Deposit {
    pub id: String,
    pub at: LonLat,
    pub district: String,
    /// Its kind's key in the registry (`deposit-type.<kind>`).
    pub kind: String,
    /// Ore (kg).
    pub ore: f64,
    /// Grades, per commodity, in the survey's units ("Cu %", "Au g/t").
    pub grades: Vec<(String, f64)>,
    pub depth: f64,
    /// Not seen at the surface.
    pub blind: bool,
    /// The rock unit it lies in.
    pub host: String,
    /// Which survey methods find it from above.
    pub seen_by: Vec<(String, bool)>,
    /// Water over it (m; 0 on land).
    pub water_depth: f64,
}

/// A district: deposits worked together, the unit of a mining licence.
#[derive(Clone, Debug, Deserialize)]
pub struct District {
    pub id: String,
    pub kind: String,
    pub lat: f64,
    pub lon: f64,
    pub extent_km: f64,
    pub deposits: Vec<String>,
}

/// What a quarry on a rock unit yields.
#[derive(Clone, Debug, Deserialize)]
pub struct BulkRock {
    pub unit: String,
    pub name: String,
    pub land_km2: f64,
    #[serde(default)]
    pub yields: Vec<serde_json::Value>,
}

/// An oil or gas field.
#[derive(Clone, Debug)]
pub struct Field {
    pub id: String,
    pub at: LonLat,
    pub basin: String,
    /// "oil_field" or "gas_field".
    pub kind: String,
    /// In place (m³): oil, and gas.
    pub oil: f64,
    pub gas: f64,
    pub depth: f64,
    pub water_depth: f64,
}

/// A petroleum basin.
#[derive(Clone, Debug)]
pub struct Basin {
    pub id: String,
    pub at: LonLat,
    pub fields: u32,
    pub area_km2: f64,
}

/// A coalfield.
#[derive(Clone, Debug)]
pub struct Coalfield {
    pub id: String,
    pub at: LonLat,
    /// In place (kg).
    pub coal: f64,
    pub rank: String,
    pub area_km2: f64,
}

/// The rock map: equirectangular, row 0 at the north pole, column 0 at −180°; each pixel the
/// rock unit's number in `units`, or none (sea).
pub struct RockMap {
    pub width: usize,
    pub height: usize,
    pixels: Vec<u8>,
    pub units: Vec<String>,
    sea: u8,
}

impl RockMap {
    /// The rock unit at `p`, or None at sea.
    pub fn unit(&self, p: LonLat) -> Option<&str> {
        let x = (((p.lon + 180.0) / 360.0 * self.width as f64).floor() as isize).rem_euclid(self.width as isize) as usize;
        let y = (((90.0 - p.lat) / 180.0 * self.height as f64).floor() as usize).min(self.height - 1);
        let v = self.pixels[y * self.width + x];
        (v != self.sea).then(|| self.units.get(v as usize).map(String::as_str)).flatten()
    }
}

/// A world's survey and energy packages, read.
pub struct World {
    pub id: String,
    /// Its body's key.
    pub body: String,
    pub radius: f64,
    pub rocks: RockMap,
    pub deposits: Vec<Deposit>,
    pub districts: Vec<District>,
    pub bulk_rock: Vec<BulkRock>,
    pub fields: Vec<Field>,
    pub basins: Vec<Basin>,
    pub coalfields: Vec<Coalfield>,
}

#[derive(Deserialize)]
struct Summary {
    radius_m: f64,
    rock_units: SummaryUnits,
}

#[derive(Deserialize)]
struct SummaryUnits {
    file: String,
    index: Vec<String>,
    sea: u8,
}

#[derive(Deserialize)]
struct Collection {
    features: Vec<Feature>,
}

#[derive(Deserialize)]
struct Feature {
    geometry: Point,
    properties: serde_json::Value,
}

#[derive(Deserialize)]
struct Point {
    coordinates: [f64; 2],
}

/// A greyscale (or indexed) 8-bit PNG: its width, height and pixels.
fn png8(bytes: &[u8]) -> Result<(usize, usize, Vec<u8>), String> {
    let mut d = png::Decoder::new(std::io::Cursor::new(bytes));
    d.set_transformations(png::Transformations::IDENTITY);
    let mut r = d.read_info().map_err(|e| e.to_string())?;
    let mut buf = vec![0; r.output_buffer_size().ok_or("too big")?];
    let info = r.next_frame(&mut buf).map_err(|e| e.to_string())?;
    let (w, h) = (info.width as usize, info.height as usize);
    let per = info.line_size / w.max(1);
    // (One byte a pixel, or the first channel of each.)
    let px = (0..h).flat_map(|y| (0..w).map(move |x| (y, x))).map(|(y, x)| buf[y * info.line_size + x * per]).collect();
    Ok((w, h, px))
}

fn num(v: &serde_json::Value, k: &str) -> f64 {
    v.get(k).and_then(serde_json::Value::as_f64).unwrap_or(0.0)
}

fn text(v: &serde_json::Value, k: &str) -> String {
    v.get(k).and_then(serde_json::Value::as_str).unwrap_or_default().to_string()
}

impl World {
    /// The body `key`'s survey and energy packages, checked and read (None: it has no survey).
    pub fn load(key: &str) -> Option<Result<World, String>> {
        let body = registry().bodies.iter().find(|b| b.identity.key == key)?;
        Some(Self::read(body))
    }

    fn read(body: &Body) -> Result<World, String> {
        let s = body.survey.as_ref().ok_or("no survey")?;
        let survey = Package::open(root().join(&s.folder), &s.manifest_sha256)?;
        let summary: Summary = survey.json("summary.json")?;
        let (width, height, pixels) = png8(&survey.read(&summary.rock_units.file)?)?;
        let rocks = RockMap { width, height, pixels, units: summary.rock_units.index, sea: summary.rock_units.sea };
        let deposits = survey
            .json::<Collection>("deposits.geojson")?
            .features
            .into_iter()
            .map(|f| {
                let p = &f.properties;
                let pairs = |k: &str| -> Vec<(String, serde_json::Value)> { p.get(k).and_then(|g| g.as_object()).map(|g| g.iter().map(|(k, v)| (k.clone(), v.clone())).collect()).unwrap_or_default() };
                Deposit {
                    id: text(p, "id"),
                    at: LonLat { lon: f.geometry.coordinates[0], lat: f.geometry.coordinates[1] },
                    district: text(p, "district_id"),
                    kind: text(p, "kind"),
                    ore: num(p, "tonnage_mt") * 1e9,
                    grades: pairs("grades").into_iter().map(|(k, v)| (k, v.as_f64().unwrap_or(0.0))).collect(),
                    depth: num(p, "depth_m"),
                    blind: p.get("blind").and_then(serde_json::Value::as_bool).unwrap_or(false),
                    host: text(p, "host"),
                    seen_by: pairs("seen_by").into_iter().map(|(k, v)| (k, v.as_bool().unwrap_or(false))).collect(),
                    water_depth: num(p, "water_depth_m"),
                }
            })
            .collect();
        #[derive(Deserialize)]
        struct Districts {
            districts: Vec<District>,
        }
        #[derive(Deserialize)]
        struct Units {
            units: Vec<BulkRock>,
        }
        let districts = survey.json::<Districts>("districts.json")?.districts;
        let bulk_rock = survey.json::<Units>("bulk_rock.json")?.units;
        let (mut fields, mut basins, mut coalfields) = (Vec::new(), Vec::new(), Vec::new());
        if let Some(e) = &body.energy {
            let energy = Package::open(root().join(&e.folder), &e.manifest_sha256)?;
            fields = energy
                .json::<Collection>("fields.geojson")?
                .features
                .into_iter()
                .map(|f| {
                    let p = &f.properties;
                    Field {
                        id: text(p, "id"),
                        at: LonLat { lon: f.geometry.coordinates[0], lat: f.geometry.coordinates[1] },
                        basin: text(p, "basin_id"),
                        kind: text(p, "kind"),
                        oil: num(p, "oil_mmbbl") * 1e6 * BARREL,
                        gas: num(p, "gas_bcf") * BCF,
                        depth: num(p, "depth_m"),
                        water_depth: num(p, "water_depth_m"),
                    }
                })
                .collect();
            basins = energy
                .json::<Vec<serde_json::Value>>("basins.json")?
                .iter()
                .map(|b| Basin { id: text(b, "id"), at: LonLat { lon: num(b, "lon"), lat: num(b, "lat") }, fields: num(b, "fields") as u32, area_km2: num(b, "area_km2") })
                .collect();
            coalfields = energy
                .json::<Vec<serde_json::Value>>("coalfields.json")?
                .iter()
                .map(|c| Coalfield { id: text(c, "id"), at: LonLat { lon: num(c, "lon"), lat: num(c, "lat") }, coal: num(c, "coal_mt") * 1e9, rank: text(c, "rank"), area_km2: num(c, "area_km2") })
                .collect();
        }
        Ok(World { id: s.world_id.clone(), body: body.identity.key.clone(), radius: summary.radius_m, rocks, deposits, districts, bulk_rock, fields, basins, coalfields })
    }
}

/// A body's surface bake, in the worlds store: its files fetched by name, each checked
/// against the bake's manifest (itself checked against the record).
pub struct Bake {
    package: Package,
}

impl Bake {
    /// The body `key`'s bake (None: it has none; an error: the store lacks it, or it's not the
    /// bake the record names).
    pub fn open(key: &str) -> Option<Result<Bake, String>> {
        let body = registry().bodies.iter().find(|b| b.identity.key == key)?;
        let b = body.bake.as_ref()?;
        Some(match store() {
            None => Err("no worlds store (set UNIVERSE_WORLDS)".into()),
            Some(store) => Package::open(store.join(&b.path), &b.manifest_sha256).map(|package| Bake { package }),
        })
    }

    /// File `name` of the bake, checked.
    pub fn read(&self, name: &str) -> Result<Vec<u8>, String> {
        self.package.read(name)
    }

    /// Does the bake hold file `name`?
    pub fn has(&self, name: &str) -> bool {
        self.package.files.contains_key(name)
    }
}

/// A body direction (its own frame) as the registry's latitude and longitude (degrees): north
/// is +Y, longitude `atan2(−z, x)`, east as the body turns. A settlement's `position` and a
/// survey's coordinates are both in it.
pub fn lon_lat(dir: glam::DVec3) -> LonLat {
    let d = dir.normalize();
    LonLat { lon: (-d.z).atan2(d.x).to_degrees(), lat: d.y.clamp(-1.0, 1.0).asin().to_degrees() }
}

/// The body direction at `p` (the inverse of `lon_lat`).
pub fn direction(p: LonLat) -> glam::DVec3 {
    let (lat, lon) = (p.lat.to_radians(), p.lon.to_radians());
    glam::DVec3::new(lat.cos() * lon.cos(), lat.sin(), -lat.cos() * lon.sin())
}

/// The air's tables (see `Heights::air_luts`): (width, height, RGBA floats) each.
pub struct AirLuts {
    pub transmittance: (u32, u32, Vec<f32>),
    pub multiscatter: (u32, u32, Vec<f32>),
}

/// An equirectangular RGB image (row 0 north, column 0 at −180°), sRGB.
pub struct Equirect {
    pub width: usize,
    pub height: usize,
    rgb: Vec<u8>,
}

impl Equirect {
    /// Its colour at body direction `dir` (sRGB bytes), bilinear.
    pub fn at(&self, dir: glam::DVec3) -> [u8; 3] {
        let p = lon_lat(dir);
        let (w, h) = (self.width as f64, self.height as f64);
        let row = ((90.0 - p.lat) / 180.0 * h - 0.5).clamp(0.0, h - 1.0);
        let col = (p.lon + 180.0) / 360.0 * w - 0.5;
        let (r0, c0) = (row.floor(), col.floor());
        let (fr, fc) = (row - r0, col - c0);
        let px = |r: f64, c: f64, k: usize| {
            let r = (r as usize).min(self.height - 1);
            let c = (c as isize).rem_euclid(self.width as isize) as usize;
            self.rgb[(r * self.width + c) * 3 + k] as f64
        };
        std::array::from_fn(|k| (px(r0, c0, k) * (1.0 - fr) * (1.0 - fc) + px(r0, c0 + 1.0, k) * (1.0 - fr) * fc + px(r0 + 1.0, c0, k) * fr * (1.0 - fc) + px(r0 + 1.0, c0 + 1.0, k) * fr * fc).round() as u8)
    }
}

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
    fine: std::sync::RwLock<HashMap<usize, (std::sync::Arc<Vec<u16>>, std::sync::atomic::AtomicU64)>>,
    clock: std::sync::atomic::AtomicU64,
    /// Tiles asked for in the background.
    asked: Vec<std::sync::atomic::AtomicBool>,
    /// The highest the ground stands (m).
    pub max: f64,
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
        static LOADED: std::sync::OnceLock<std::sync::Mutex<HashMap<String, Option<std::sync::Arc<Heights>>>>> = std::sync::OnceLock::new();
        let mut loaded = LOADED.get_or_init(Default::default).lock().unwrap_or_else(|e| e.into_inner());
        loaded
            .entry(key.to_string())
            .or_insert_with(|| match Bake::open(key)?.and_then(Heights::read) {
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
        let fine: Fine = serde_json::from_slice(&bake.read("fz.json")?).map_err(|e| e.to_string())?;
        let tiles = 6 * (1usize << fine.level).pow(2);
        // (The highest ground from the bake's peaks, with room: the heights themselves aren't read
        // till one's wanted.)
        #[derive(Deserialize)]
        struct Peak {
            h: f64,
        }
        let top = bake.read("peaks.json").ok().and_then(|b| serde_json::from_slice::<Vec<Peak>>(&b).ok()).and_then(|p| p.iter().map(|p| p.h).reduce(f64::max)).unwrap_or(9_000.0);
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
            asked: (0..tiles).map(|_| Default::default()).collect(),
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

    /// Fine tile `slot` if it's kept (marked used now).
    fn kept(&self, slot: usize) -> Option<std::sync::Arc<Vec<u16>>> {
        let fine = self.fine.read().unwrap_or_else(|e| e.into_inner());
        let (t, used) = fine.get(&slot)?;
        used.store(self.clock.fetch_add(1, std::sync::atomic::Ordering::Relaxed), std::sync::atomic::Ordering::Relaxed);
        Some(t.clone())
    }

    /// Keep fine tile `slot`, the least lately used let go past the budget.
    fn keep(&self, slot: usize, tile: std::sync::Arc<Vec<u16>>) {
        let mut fine = self.fine.write().unwrap_or_else(|e| e.into_inner());
        let now = self.clock.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        fine.insert(slot, (tile, std::sync::atomic::AtomicU64::new(now)));
        let bytes = |f: &HashMap<usize, (std::sync::Arc<Vec<u16>>, std::sync::atomic::AtomicU64)>| f.values().map(|(t, _)| t.len() * 2).sum::<usize>();
        while bytes(&fine) > tile_budget() && fine.len() > 1 {
            let Some(oldest) = fine.iter().filter(|(k, _)| **k != slot).min_by_key(|(_, (_, u))| u.load(std::sync::atomic::Ordering::Relaxed)).map(|(k, _)| *k) else { break };
            fine.remove(&oldest);
            self.asked[oldest].store(false, std::sync::atomic::Ordering::Relaxed);
        }
    }

    /// How many fine tiles are kept, and their bytes.
    pub fn kept_tiles(&self) -> (usize, usize) {
        let fine = self.fine.read().unwrap_or_else(|e| e.into_inner());
        (fine.len(), fine.values().map(|(t, _)| t.len() * 2).sum())
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

    /// Tile `name` (in `slot`), read.
    fn load(&self, name: &str) -> Option<Vec<u16>> {
        self.bake.has(name).then(|| self.bake.read(name).and_then(|b| png_rg(&b)).map(|t| t.2).ok()).flatten()
    }

    /// The 600 m difference at body direction `d` (m), bilinear; 0 where there's no tile. And
    /// whether it's the whole of it (false: a tile not read yet, at `Detail::Loaded`).
    fn fine_at(self: &std::sync::Arc<Self>, d: glam::DVec3, detail: Detail) -> (f64, bool) {
        if detail == Detail::Coarse {
            return (0.0, true);
        }
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
        let k = 1usize << self.level;
        let to = |t: f64| ((t.atan() * 4.0 / std::f64::consts::PI + 1.0) / 2.0 * k as f64).clamp(0.0, k as f64 - 1e-9);
        let (u, v) = (to(tu), to(tv));
        let (x, y) = (u.floor() as usize, v.floor() as usize);
        let slot = (face * k + x) * k + y;
        let name = || format!("fz_{face}_{x}_{y}.png");
        // (Where the bake has no tile (the sea's), the 5 km heights alone.)
        let t = match self.kept(slot) {
            Some(t) => t,
            None if !self.bake.has(&name()) => return (0.0, true),
            None => match detail {
                Detail::Full => match self.load(&name()) {
                    Some(t) => {
                        let t = std::sync::Arc::new(t);
                        self.keep(slot, t.clone());
                        t
                    }
                    None => return (0.0, true),
                },
                _ => {
                    if !self.asked[slot].swap(true, std::sync::atomic::Ordering::Relaxed) {
                        let (me, name) = (self.clone(), name());
                        rayon::spawn(move || {
                            if let Some(t) = me.load(&name) {
                                me.keep(slot, std::sync::Arc::new(t));
                            }
                        });
                    }
                    return (0.0, false);
                }
            },
        };
        let n = self.n;
        let (su, sv) = ((u - x as f64) * n as f64, (v - y as f64) * n as f64);
        let (i0, j0) = ((su.floor() as usize).min(n - 1), (sv.floor() as usize).min(n - 1));
        let (fi, fj) = (su - i0 as f64, sv - j0 as f64);
        let at = |i: usize, j: usize| t[j * (n + 1) + i] as f64;
        let raw = at(i0, j0) * (1.0 - fi) * (1.0 - fj) + at(i0 + 1, j0) * fi * (1.0 - fj) + at(i0, j0 + 1) * (1.0 - fi) * fj + at(i0 + 1, j0 + 1) * fi * fj;
        ((raw - 32_768.0) / 2.0, true)
    }

    /// Image `name` of the bake (a JPEG or PNG), as RGBA8: its width, height and pixels. None:
    /// not in the bake, or unreadable.
    pub fn image(&self, name: &str) -> Option<(usize, usize, Vec<u8>)> {
        let bytes = self.bake.read(name).ok()?;
        if name.ends_with(".jpg") {
            let mut d = zune_jpeg::JpegDecoder::new(&bytes);
            let rgb = d.decode().ok()?;
            let (w, h) = d.dimensions()?;
            return (rgb.len() == w * h * 3).then(|| (w, h, rgb.chunks(3).flat_map(|c| [c[0], c[1], c[2], 255]).collect()));
        }
        let mut d = png::Decoder::new(std::io::Cursor::new(&bytes));
        d.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
        let mut r = d.read_info().ok()?;
        let mut buf = vec![0; r.output_buffer_size()?];
        let info = r.next_frame(&mut buf).ok()?;
        let (w, h) = (info.width as usize, info.height as usize);
        let per = info.line_size / w.max(1);
        let px = (0..h * w)
            .flat_map(|i| {
                let o = (i / w) * info.line_size + (i % w) * per;
                match per {
                    1 => [buf[o], buf[o], buf[o], 255],
                    2 => [buf[o], buf[o], buf[o], buf[o + 1]],
                    3 => [buf[o], buf[o + 1], buf[o + 2], 255],
                    _ => [buf[o], buf[o + 1], buf[o + 2], buf[o + 3]],
                }
            })
            .collect();
        Some((w, h, px))
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
    /// transmittance (256 × 64) and the light of scattering's higher orders (32 × 32, scaled to
    /// its true size here), RGBA floats row by row. None: none baked. (`UNIVERSE_AIR_LUTS`: a
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
        let dims = |k: &str, rows: &str, cols: &str| Some((info.get(k)?.get(cols)?.as_u64()? as u32, info.get(k)?.get(rows)?.as_u64()? as u32));
        let (tw, th) = dims("transmittance", "rows_height", "cols_mu")?;
        let (mw, mh) = dims("multiscatter", "rows_height", "cols_mu_sun")?;
        let scale = info.get("multiscatter")?.get("scale").and_then(serde_json::Value::as_f64).unwrap_or(1.0) as f32;
        let transmittance = floats(read("air_transmittance.rgba32f")?);
        let multiscatter: Vec<f32> = floats(read("air_multiscatter.rgba32f")?).into_iter().map(|v| v * scale).collect();
        (transmittance.len() == (tw * th * 4) as usize && multiscatter.len() == (mw * mh * 4) as usize).then_some(AirLuts { transmittance: (tw, th, transmittance), multiscatter: (mw, mh, multiscatter) })
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

    /// The ground's height at `dir` to `detail`, and whether it's the whole of it.
    pub fn at_detail(self: &std::sync::Arc<Self>, dir: glam::DVec3, detail: Detail) -> (f64, bool) {
        let d = dir.normalize();
        let (fine, whole) = self.fine_at(d, detail);
        (self.coarse_at(lon_lat(d)) + fine, whole)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Harvest's survey and energy packages read, checked against their records; a few
    /// figures held to the record's; its bake found in the store and a file read, if a store is
    /// here (the store is outside the tree).
    #[test]
    fn harvest_reads_from_its_packages() {
        let key = "body.treistun.treistun-d";
        let w = World::load(key).expect("a survey").expect("read");
        let record = registry().bodies.iter().find(|b| b.identity.key == key).unwrap();
        assert_eq!(w.deposits.len() as i64, 3756);
        assert!(w.districts.len() > 900 && !w.bulk_rock.is_empty());
        let e = record.energy.as_ref().unwrap();
        assert_eq!(w.fields.iter().filter(|f| f.kind == "oil_field").count() as i64, e.oil_fields.unwrap());
        assert_eq!(w.coalfields.len() as i64, e.coalfields.unwrap());
        // (The record's oil in place is the basins' total, in m³; the fields add up to 1.7% more.)
        let oil: f64 = w.fields.iter().map(|f| f.oil).sum();
        assert!((oil / e.oil_in_place.unwrap() - 1.0).abs() < 0.02, "{oil:e} m3");
        // Land where the survey says: about a quarter of the map.
        let land = (0..1000).filter(|i| w.rocks.unit(LonLat { lon: (*i as f64 * 137.5) % 360.0 - 180.0, lat: (*i as f64 * 0.179) - 89.5 }).is_some()).count();
        assert!((150..400).contains(&land), "{land} of 1000 on land");
        // Each deposit lies on land or under the sea as the map has it.
        let d = w.deposits.iter().find(|d| d.water_depth == 0.0).unwrap();
        assert!(w.rocks.unit(d.at).is_some(), "{} on land", d.id);
        if store().is_some() {
            let bake = Bake::open(key).unwrap().expect("its bake in the store");
            assert!(bake.read("world.json").is_ok());
        }
        // A body direction and the registry's latitude and longitude, both ways.
        let p = LonLat { lon: 165.925, lat: 45.649 };
        let q = lon_lat(direction(p));
        assert!((q.lon - p.lon).abs() < 1e-9 && (q.lat - p.lat).abs() < 1e-9);
    }
}
