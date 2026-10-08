//! A world's survey and energy packages as the planet simulation releases them: its rock map,
//! deposits, districts, bulk rock, petroleum basins, oil and gas fields, coalfields.


use serde::Deserialize;

use super::*;
use crate::registry::{registry, Body};

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
pub struct Survey {
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

impl Survey {
    /// The body `key`'s survey and energy packages, checked and read (None: it has no survey).
    pub fn load(key: &str) -> Option<Result<Survey, String>> {
        let body = registry().body(key)?;
        Some(Self::read(body))
    }

    fn read(body: &Body) -> Result<Survey, String> {
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
        Ok(Survey { id: s.world_id.clone(), body: body.identity.key.clone(), radius: summary.radius_m, rocks, deposits, districts, bulk_rock, fields, basins, coalfields })
    }
}
