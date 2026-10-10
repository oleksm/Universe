//! The store's index of released worlds, and a world's history (its frames through time).


use serde::Deserialize;

use super::*;

/// A world in the store's index (`worlds/releases.json`: every world the planet simulation has
/// released, a game body's or not).
#[derive(Clone, Debug)]
pub struct Release {
    pub world_id: String,
    /// Its body in the game (`body.<system>.<name>`), if it's one of the game's.
    pub body: Option<String>,
    /// "current" or "superseded".
    pub status: String,
    /// Its name and a line about it (the bake's `world.json`), where it has a surface.
    pub name: String,
    pub subtitle: String,
    /// Its figures (the survey's summary): radius (m), land (%), deposits, districts.
    pub radius: f64,
    pub land_pct: f64,
    pub deposits: u64,
    pub districts: u64,
    /// Its surface version, if it has one.
    pub surface: Option<u32>,
    /// Its surface package: the folder under the store's `worlds/` and its manifest's hash.
    pub surface_package: Option<(String, String)>,
    /// Its history package: the folder under the store's `worlds/` and its manifest's hash.
    pub history_package: Option<(String, String)>,
    /// Its day (s) and its axis's tilt (degrees), where its bake's `world.json` gives them.
    pub day: Option<f64>,
    pub tilt: Option<f64>,
}


/// A world's growth as its run saw it (the store's history package, `planet-sim-history/1`):
/// globes in time order, each with its figures then.
pub struct History {
    package: Package,
    pub frames: Vec<HistoryFrame>,
}

/// One of a history's globes: its file (equirectangular, as `globe_color`), when (Gyr since the
/// world began, and before today), and the world then.
#[derive(Clone, Debug, Deserialize)]
pub struct HistoryFrame {
    pub file: String,
    pub time_gyr: f64,
    pub ago_gyr: f64,
    pub land_pct: f64,
    pub plates: u32,
    pub highest_m: f64,
    pub deepest_m: f64,
}

impl History {
    /// Released world `r`'s history, checked against the store's index (None: it has none).
    pub fn release(r: &Release) -> Option<Result<History, String>> {
        let (folder, sha) = r.history_package.as_ref()?;
        let store = store()?;
        Some(Package::open(store.join("worlds").join(folder), sha).and_then(|package| {
            let frames = serde_json::from_slice(&package.read("frames.json")?).map_err(|e| e.to_string())?;
            Ok(History { package, frames })
        }))
    }

    /// Frame `i`'s globe, as RGBA8 (see `Heights::image`).
    pub fn image(&self, i: usize) -> Option<(usize, usize, Vec<u8>)> {
        let name = &self.frames.get(i)?.file;
        decode(name, &self.package.read(name).ok()?)
    }
}

/// Image file `name`'s bytes (a JPEG or PNG) as RGBA8: its width, height and pixels.
pub(super) fn decode(name: &str, bytes: &[u8]) -> Option<(usize, usize, Vec<u8>)> {
    if name.ends_with(".jpg") {
        let mut d = zune_jpeg::JpegDecoder::new(bytes);
        let rgb = d.decode().ok()?;
        let (w, h) = d.dimensions()?;
        return (rgb.len() == w * h * 3).then(|| (w, h, rgb.chunks(3).flat_map(|c| [c[0], c[1], c[2], 255]).collect()));
    }
    let mut d = png::Decoder::new(std::io::Cursor::new(bytes));
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

/// The worlds in the store (none without a store), current first.
pub fn releases() -> Vec<Release> {
    let Some(store) = store() else { return Vec::new() };
    let dir = store.join("worlds");
    let Ok(bytes) = std::fs::read(dir.join("releases.json")) else { return Vec::new() };
    let Ok(index) = serde_json::from_slice::<serde_json::Value>(&bytes) else { return Vec::new() };
    let read = |rel: &str| -> Option<serde_json::Value> { serde_json::from_slice(&std::fs::read(dir.join(rel)).ok()?).ok() };
    let mut out: Vec<Release> = index
        .get("worlds")
        .and_then(|w| w.as_array())
        .map(|ws| {
            ws.iter()
                .filter_map(|w| {
                    let id = w.get("world_id")?.as_str()?.to_string();
                    let pkg = |k: &str| w.get("packages")?.get(k).filter(|p| !p.is_null()).cloned();
                    let folder = |p: &serde_json::Value| p.get("manifest").and_then(|m| m.as_str()).and_then(|m| m.rsplit_once('/')).map(|(f, _)| f.to_string());
                    let summary = pkg("survey").and_then(|p| folder(&p)).and_then(|f| read(&format!("{f}/summary.json")));
                    let surface = pkg("surface");
                    let world = surface.as_ref().and_then(folder).and_then(|f| read(&format!("{f}/world.json")));
                    let num = |v: &Option<serde_json::Value>, k: &str| v.as_ref().and_then(|v| v.get(k)).and_then(|x| x.as_f64()).unwrap_or(0.0);
                    let txt = |v: &Option<serde_json::Value>, k: &str| v.as_ref().and_then(|v| v.get(k)).and_then(|x| x.as_str()).unwrap_or_default().to_string();
                    Some(Release {
                        body: w.get("body").and_then(|b| b.as_str()).map(str::to_string),
                        status: w.get("status").and_then(|s| s.as_str()).unwrap_or("current").to_string(),
                        name: Some(txt(&world, "name")).filter(|n| !n.is_empty()).unwrap_or_else(|| id.clone()),
                        subtitle: txt(&world, "subtitle"),
                        radius: num(&summary, "radius_m"),
                        land_pct: num(&summary, "land_pct"),
                        deposits: num(&summary, "deposits") as u64,
                        districts: num(&summary, "districts") as u64,
                        surface: surface.as_ref().and_then(|s| s.get("version")).and_then(|v| v.as_u64()).map(|v| v as u32),
                        surface_package: surface.as_ref().and_then(|s| Some((folder(s)?, s.get("manifest_sha256")?.as_str()?.to_string()))),
                        history_package: pkg("history").and_then(|s| Some((folder(&s)?, s.get("manifest_sha256")?.as_str()?.to_string()))),
                        day: world.as_ref().and_then(|w| w.get("day_hours")).and_then(|d| d.as_f64()).map(|h| h * 3600.0),
                        tilt: world.as_ref().and_then(|w| w.get("tilt_deg")).and_then(|d| d.as_f64()),
                        world_id: id,
                    })
                })
                .collect()
        })
        .unwrap_or_default();
    out.sort_by_key(|r| (r.status != "current", r.world_id.clone()));
    out
}
