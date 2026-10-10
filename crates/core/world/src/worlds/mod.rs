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

use crate::registry::registry;

mod heights;
mod lines;
mod releases;
mod survey;
pub use heights::*;
pub use lines::*;
pub use releases::*;
pub use survey::*;

/// A barrel (m³).
const BARREL: f64 = 0.158_987_294_928;
/// A billion cubic feet (m³).
const BCF: f64 = 2.831_684_659_2e7;

/// Where the registry's files lie: `UNIVERSE_ROOT`, else the source tree this was built from.
pub fn root() -> PathBuf {
    std::env::var_os("UNIVERSE_ROOT").map(PathBuf::from).unwrap_or_else(|| Path::new(env!("CARGO_MANIFEST_DIR")).join("../../.."))
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
pub(crate) struct Package {
    folder: PathBuf,
    files: HashMap<String, String>,
}

impl Package {
    /// The package in `folder`, its manifest's hash `want` (None: not checked against a record:
    /// a preview, see `Bake::open`).
    pub(crate) fn open(folder: PathBuf, want: &str) -> Result<Self, String> {
        Self::open_checked(folder, Some(want))
    }

    fn open_checked(folder: PathBuf, want: Option<&str>) -> Result<Self, String> {
        let bytes = std::fs::read(folder.join("manifest.json")).map_err(|e| format!("{}: {e}", folder.display()))?;
        let got = sha256(&bytes);
        if let Some(want) = want
            && got != want
        {
            return Err(format!("{}: manifest is {got}, the record says {want}", folder.display()));
        }
        let m: Manifest = serde_json::from_slice(&bytes).map_err(|e| format!("{}: {e}", folder.display()))?;
        Ok(Package { folder, files: m.files.into_iter().map(|(k, v)| (k, v.sha256)).collect() })
    }

    /// File `name`'s bytes, checked against the manifest.
    pub(crate) fn read(&self, name: &str) -> Result<Vec<u8>, String> {
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

/// A body's surface bake, in the worlds store: its files fetched by name, each checked
/// against the bake's manifest (itself checked against the record).
pub struct Bake {
    package: Package,
    /// Its world's id (`TRD1`): the runtime detail's seed.
    world: String,
}

impl Bake {
    /// The body `key`'s bake (None: it has none; an error: the store lacks it, or it's not the
    /// bake the record names).
    pub fn open(key: &str) -> Option<Result<Bake, String>> {
        let body = registry().body(key)?;
        // (A preview, for the lab: `UNIVERSE_BAKE_<WORLD ID>=<folder>`, a surface package not yet
        // released, read as the bake without the record's hash: each file still checked against
        // its own manifest.)
        if let Some(id) = body.survey.as_ref().map(|s| s.world_id.clone())
            && let Some(folder) = std::env::var_os(format!("UNIVERSE_BAKE_{id}"))
        {
            eprintln!("PREVIEW: {key}'s ground from {} (UNIVERSE_BAKE_{id}), not the bake its record names", PathBuf::from(&folder).display());
            return Some(Package::open_checked(PathBuf::from(folder), None).map(|package| Bake { package, world: id }));
        }
        let b = body.bake.as_ref()?;
        let world = body.survey.as_ref().map_or_else(|| key.to_string(), |s| s.world_id.clone());
        Some(match store() {
            None => Err("no worlds store (set UNIVERSE_WORLDS)".into()),
            Some(store) => Package::open(store.join(&b.path), &b.manifest_sha256).map(|package| Bake { package, world }),
        })
    }

    /// A released world's surface package, as the store's index names it and checked against
    /// the index's hash: for a world that isn't one of the game's (the planet studio). None: it
    /// has no surface.
    pub fn release(r: &Release) -> Option<Result<Bake, String>> {
        let (folder, sha) = r.surface_package.as_ref()?;
        Some(match store() {
            None => Err("no worlds store (set UNIVERSE_WORLDS)".into()),
            Some(store) => Package::open(store.join("worlds").join(folder), sha).map(|package| Bake { package, world: r.world_id.clone() }),
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

/// A world's clouds as baked (see `Heights::clouds`): its three maps (width, height, RGBA8), its
/// year (days) and its El Niño's series (days a world month, the index month by month), if any.
pub struct CloudsBake {
    pub maps: [(usize, usize, Vec<u8>); 3],
    pub year_days: f64,
    pub enso: Option<(f64, Vec<f32>)>,
    /// Its format's version (`planet-sim-clouds/N`; 1 when unsaid): 2's air map holds the water
    /// the air can carry in its A channel, where 1's held the jet's wind.
    pub format: u32,
}

/// A world's clouds at world time `t` (s, one epoch for every player), its year `year_days` and
/// its El Niño's series (days a month, the index a month): the year's phase in its months
/// (0..12), and El Niño's index then.
pub fn clouds_at(t: f64, year_days: f64, enso: Option<&(f64, Vec<f32>)>) -> (f32, f32) {
    let days = t / 86_400.0;
    let month = ((days / year_days).rem_euclid(1.0) * 12.0) as f32;
    let index = enso.filter(|(_, s)| !s.is_empty()).map_or(0.0, |(m, s)| s[((days / m).floor().max(0.0) as usize) % s.len()]);
    (month, index)
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

#[cfg(test)]
mod tests {
    use super::*;
    use super::heights::locate;

    /// A cube face's place and its body direction go there and back (`cube_dir`, `locate`).
    #[test]
    fn cube_faces_there_and_back() {
        for face in 0..6 {
            for (u, v) in [(-0.9, 0.3), (0.0, 0.0), (0.7, -0.6), (0.99, 0.99)] {
                let (f, x, y, _, fu, fv) = locate(cube_dir(face, u, v), 10);
                let back = |c: usize, fr: f64| (c as f64 + fr) / 1024.0 * 2.0 - 1.0;
                assert_eq!(f, face);
                assert!((back(x, fu) - u).abs() < 1e-9 && (back(y, fv) - v).abs() < 1e-9, "face {face} at {u}, {v}");
            }
        }
    }

    /// Heath's survey and energy packages read, checked against their records; a few
    /// figures held to the record's; its bake found in the store and a file read, if a store is
    /// here (the store is outside the tree).
    #[test]
    fn harvest_reads_from_its_packages() {
        let key = "body.treistun.treistun-d";
        let w = Survey::load(key).expect("a survey").expect("read");
        let record = registry().body(&key).unwrap();
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
