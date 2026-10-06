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
    }
}
