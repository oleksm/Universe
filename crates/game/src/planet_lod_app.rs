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

mod objects;
use objects::Objects;
use universe_terrain::{flight, flight::pan_longitude, json, unit, Ground, LandDrawn, View, ALT_MIN};

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use crate::hud_kit::{self, Hit, Panel};

use universe_engine::apps::{App, Next};
use universe_engine::glam::{DVec3, Quat, Vec2, Vec3};
use universe_engine::{Camera, Color, Context, Frame, KeyCode, Light, Mesh, MouseButton, Transform, WireModel};

/// Straight down at and above this altitude, level with the horizon at the ground (as the page).
const TILT_HI: f64 = 100_000.0;
/// The light every face gets, whatever it faces: the planet lab's views' (`features.rs` `draw`).
const VIEW_AMBIENT: f32 = 0.3;
/// The panel's widgets.
const ID_HIDE: u32 = 1;
const ID_MESH: u32 = 10;
const ID_EDGES: u32 = 20;
const ID_FACES: u32 = 21;
const ID_FEAT: u32 = 22;
const ID_PEAKS: u32 = 23;
const ID_MARKS: u32 = 24;
/// Landmark markers (markers.json beside the view): labels on at most this many, the nearest to the eye; size in px.
const MARK_LABELS: usize = 60;
const MARK_PX: f32 = 5.0;
/// Peak and dip pins: labels on at most this many, the nearest to the eye (posts on all).
const PEAK_LABELS: usize = 300;
const ID_EX: u32 = 30;
const ID_KEYS: u32 = 40;
const ID_VIS: u32 = 50;
/// The layer chips: ID_LAYER + the layer's index.
const ID_LAYER: u32 = 100;
const ID_BOX_FAMILY: u32 = 200;
const PANEL_W: f32 = 284.0;






/// What the last draw left for the next update (the panel's boxes) and its own counts.
#[derive(Default)]
struct Ui {
    objects_drawn: usize,
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















pub struct PlanetLodApp {
    /// The ground (see `universe_terrain`).
    g: Ground,
    hud: bool,
    lon: f64,
    lat: f64,
    alt: f64,
    heading: f64,
    flight_reference: flight::Reference,
    /// The wheel's target altitude, eased to.
    alt_target: f64,
    /// `pitch=` (deg down; `nadir`: 90) held at every altitude, else by altitude (`pitch_down`); `fov=` (deg,
    /// vertical); `sun=e,n,u`: the sun fixed to the planet, a direction in the east-north-up frame at the start's
    /// place (else from the north-west, 35° up, wherever the eye is).
    pitch: Option<f64>,
    fov: f64,
    sun: Option<DVec3>,
    /// The screen without the film curve (AgX and its look: its lifted darks read as a blue haze over a far world),
    /// the light gamma-encoded as it is, as the lab's views are: the default (the owner, 2026-10-23); `look=film`
    /// puts the game's curve back.
    raw_look: bool,
    easing: bool,
    /// UNIVERSE_PLANETLOD_FLY: the camera descends and drifts by itself (captures ignore the keys);
    /// =pan: it slides sideways at its altitude instead.
    fly: bool,
    pan: bool,
    pan_start: Option<(f64, f64)>,
    /// =zoom: it zooms out and back in on a loop (zoom_altitude).
    zoom: bool,
    show_feat: bool,
    /// Pins on every peak and dip with its height (peaks.json beside the view: [{lon, lat, z, kind}]).
    show_peaks: bool,
    /// Landmark markers as the slideshow draws them (markers.json: [{lat, lon, z, shape, rgb, label}]; shape tri,
    /// dot, ring, square, diamond or cross), a fixed size on screen, labels on the nearest MARK_LABELS.
    show_marks: bool,
    marks: Vec<(DVec3, f64, u8, Color, String)>,
    peak_pins: Vec<(DVec3, f64, String, bool)>,
    keys_open: bool,
    /// The slider being dragged, a drag over the ground (its button), the ground under the eye (m, drawn).
    sliding: bool,
    drag: Option<MouseButton>,
    ground_h: Option<f64>,
    /// (altitude, drawn share, fully-in share) as flown, for the chart.
    history: std::collections::VecDeque<[f32; 3]>,
    last_hist: f64,
    feat: Option<Mesh>,
    pins: Vec<(DVec3, f64, String, bool)>,
    objects: Option<Objects>,
    ui: Mutex<Ui>,
    /// Sums for the log line written each second: frames, seconds, LOD ms, mesh ms, max of each, rebuilt.
    log: [f64; 7],
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






/// FLY=zoom: the owner's repeated zoom, looping 10 km -> 6.3 Mm -> 350 km -> 10 km; each hold 6 s,
/// each move 3 s even in log altitude.
fn zoom_altitude(elapsed: f64) -> f64 {
    const STOPS: [f64; 3] = [10_000.0, 6_300_000.0, 350_000.0];
    let t = elapsed.rem_euclid(27.0);
    let (i, u) = ((t / 9.0) as usize, (t % 9.0 - 6.0).max(0.0) / 3.0);
    let (a, b) = (STOPS[i].ln(), STOPS[(i + 1) % 3].ln());
    (a + (b - a) * u * u * (3.0 - 2.0 * u)).exp()
}

impl PlanetLodApp {
    pub fn new(dir: Option<&str>) -> Result<Self, String> {
        let g = Ground::open(&dir.map(PathBuf::from).unwrap_or_else(|| PathBuf::from(crate::devenv::var("HOME").unwrap_or_default()).join("planet-data/views/earth_s15-pattern")))?;
        let mut g = g;
        g.view = View::Mesh;
        let dir = g.dir.clone();
        let lod = json(&dir, "lod.json")?;
        let peak = &lod["peak"];
        let num = |v: &serde_json::Value| v.as_f64().unwrap_or(0.0);
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
        let marks: Vec<(DVec3, f64, u8, Color, String)> = json(&dir, "markers.json").ok().and_then(|a| a.as_array().cloned()).unwrap_or_default().iter()
            .map(|a| {
                let shape = match a["shape"].as_str().unwrap_or("dot") { "tri" => 0, "dot" => 1, "ring" => 2, "square" => 3, "diamond" => 4, _ => 5 };
                let c = |k: usize| a["rgb"][k].as_f64().unwrap_or(200.0) as f32 / 255.0;
                (unit(num(&a["lon"]), num(&a["lat"])), num(&a["z"]), shape, Color::rgb(c(0), c(1), c(2)), a["label"].as_str().unwrap_or("").to_string())
            })
            .collect();
        let objects = Objects::open(&dir)?;
        let mut app = PlanetLodApp {
            g,
            hud: true,
            lon: peak["lon"].as_f64().unwrap_or(0.0),
            lat: peak["lat"].as_f64().unwrap_or(0.0),
            alt: 20_000.0,
            heading: 0.0,
            flight_reference: flight::Reference::default(),
            alt_target: 20_000.0,
            pitch: None,
            fov: 50.0,
            sun: None,
            raw_look: true,
            easing: false,
            fly: crate::devenv::var_os("UNIVERSE_PLANETLOD_FLY").is_some(),
            pan: crate::devenv::var("UNIVERSE_PLANETLOD_FLY").is_ok_and(|v| v == "pan"),
            pan_start: None,
            zoom: crate::devenv::var("UNIVERSE_PLANETLOD_FLY").is_ok_and(|v| v == "zoom"),
            show_feat: false,
            show_peaks: false,
            show_marks: !marks.is_empty(),
            marks,
            peak_pins,
            keys_open: false,
            sliding: false,
            drag: None,
            ground_h: None,
            history: Default::default(),
            last_hist: 0.0,
            feat: None,
            pins,
            objects,
            ui: Mutex::new(Ui::default()),
            log: [0.0; 7],

        };
        let mut sun_enu: Option<DVec3> = None;
        if let Ok(at) = crate::devenv::var("UNIVERSE_PLANETLOD_AT") {
            for kv in at.split('&') {
                let mut p = kv.splitn(2, '=');
                let (k, v) = (p.next().unwrap_or(""), p.next().and_then(|v| v.parse::<f64>().ok()));
                match (k, v) {
                    ("alt", Some(v)) => app.alt = v,
                    ("lat", Some(v)) => app.lat = v,
                    ("lon", Some(v)) => app.lon = v,
                    ("heading", Some(v)) => app.heading = v,
                    ("pitch", _) if kv.ends_with("nadir") => app.pitch = Some(90.0),
                    ("pitch", Some(v)) => app.pitch = Some(v.clamp(-90.0, 90.0)),
                    ("fov", Some(v)) => app.fov = v.clamp(1.0, 170.0),
                    ("look", _) if kv.ends_with("raw") => app.raw_look = true,
                    ("look", _) if kv.ends_with("film") => app.raw_look = false,
                    ("sun", _) => {
                        let enu: Vec<f64> = kv[4..].split(',').filter_map(|x| x.trim().parse().ok()).collect();
                        if let [e, n, u] = enu[..] {
                            sun_enu = Some(DVec3::new(e, n, u));
                        }
                    }
                    ("ex", Some(v)) => app.g.ex = v.clamp(1.0, 50.0),
                    ("feat", Some(v)) => app.show_feat = v > 0.0,
                    ("peaks", Some(v)) => app.show_peaks = v > 0.0,
                    ("marks", Some(v)) => app.show_marks = v > 0.0,
                    ("view", _) if kv.ends_with("real") => app.g.view = View::Real,
                    ("boxes", _) => {
                        if let Some(o) = &mut app.objects { o.select(&kv[6..]); }
                    }
                    ("layers", _) => {
                        // (layers=0,2 or layers=none: which layer meshes are on)
                        if let Some(l) = &mut app.g.layers {
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
        // (The sun's east-north-up at the start's place, fixed to the planet from then on.)
        if let Some(enu) = sun_enu.and_then(|v| v.try_normalize()) {
            let up = unit(app.lon, app.lat);
            let east = DVec3::Y.cross(up).normalize_or(DVec3::X);
            let north = up.cross(east);
            app.sun = Some((east * enu.x + north * enu.y + up * enu.z).normalize());
        }
        if app.g.ex != 1.0 {
            app.set_ex(app.g.ex);
        }
        Ok(app)
    }

    /// The height x: the ground's, and the crest and valley lines made again.
    fn set_ex(&mut self, ex: f64) {
        self.g.set_ex(ex);
        self.feat = None;
    }





    fn eye(&self) -> DVec3 {
        let up = unit(self.lon, self.lat);
        up * (self.g.d.r + self.flight_reference.height.unwrap_or(0.0) + self.alt)
    }










}

impl PlanetLodApp {

}

impl PlanetLodApp {
}



impl App for PlanetLodApp {
    fn name(&self) -> &str {
        "planetlod"
    }

    /// For the observer's /status: where the eye is and what the levels of detail are doing.
    fn status(&self) -> String {
        let ui = self.ui.lock().unwrap();
        serde_json::json!({
            "view": if self.g.view == View::Mesh { "mesh" } else { "real" },
            "altitude_m": self.alt, "lat": self.lat, "lon": self.lon, "heading_deg": self.heading.rem_euclid(360.0),
            "ground_m": self.ground_h, "height_x": self.g.ex,
            "terrain_reference_m": self.flight_reference.height, "terrain_target_m": self.flight_reference.target,
            "clearance_m": self.ground_h.map(|h|self.eye().length()-self.g.d.r-h*self.g.ex),
            "pattern_vertices_drawn": self.g.stats.drawn, "pattern_vertices": self.g.d.mv.len(),
            "fully_risen": self.g.risen.0, "rising": self.g.risen.1,
            "lod_ms": self.g.stats.lod_ms, "cells_decided": self.g.stats.decided, "vertices_changed": self.g.stats.changed,
            "mesh_ms": self.g.stats.mesh_ms, "chunks_rebuilt": self.g.stats.rebuilt,
            "chunks_drawn": ui.chunks_drawn, "tile_groups_drawn": ui.tiles_drawn,
            "pyramid": self.g.pyramid.as_ref().map(|p| { let s=p.lock().unwrap().stats; serde_json::json!({"selected":s.selected,"drawn":s.drawn,"triangles":s.triangles,"pending":s.pending,"cached":s.cached,"bytes":s.bytes,"fallback":s.fallback,"worst_error_ratio":s.worst,"cover_resident":s.resident,"largest_tile_bytes":s.largest_tile}) }),
            "objects_drawn": ui.objects_drawn,
            "object_families": self.objects.as_ref().map(|o| o.families.iter().filter(|f| f.on).map(|f| &f.name).collect::<Vec<_>>()),
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
            self.g.view = if self.g.view == View::Mesh { View::Real } else { View::Mesh };
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
                Some(ID_MESH) => self.g.view = View::Mesh,
                Some(x) if x == ID_MESH + 1 => self.g.view = View::Real,
                Some(ID_EDGES) => self.g.show_edges = !self.g.show_edges,
                Some(ID_FACES) => self.g.show_faces = !self.g.show_faces,
                Some(ID_FEAT) => self.show_feat = !self.show_feat,
                Some(ID_PEAKS) => self.show_peaks = !self.show_peaks,
                Some(ID_MARKS) => self.show_marks = !self.show_marks,
                Some(ID_KEYS) => self.keys_open = !self.keys_open,
                Some(ID_VIS) => self.ui.lock().unwrap().vis_note = true,
                Some(ID_EX) => self.sliding = true,
                Some(x) if x >= ID_BOX_FAMILY => {
                    if let Some(o) = &mut self.objects {
                        if let Some(f) = o.families.get_mut((x-ID_BOX_FAMILY) as usize) { f.on = !f.on; }
                    }
                }
                Some(x) if (ID_LAYER..ID_LAYER + 64).contains(&x) => {
                    if let Some(l) = &mut self.g.layers {
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
                if ex != self.g.ex {
                    self.set_ex(ex);
                }
            }
        }
        let shift = input.shift();
        let r = self.g.d.r;
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
            let (origin, started) = *self.pan_start.get_or_insert((self.lon, ctx.time));
            self.lon = pan_longitude(origin, ctx.time - started);
        } else if self.zoom {
            let (_, started) = *self.pan_start.get_or_insert((self.lon, ctx.time));
            self.alt = zoom_altitude(ctx.time - started);
            self.easing = false;
        } else if self.fly {
            self.alt = (self.alt * 0.995).max(300.0);
            self.lon += 2e-4;
        }
        let cam = self.camera();
        let aspect = match self.ui.lock().unwrap().aspect { a if a > 0.0 => a, _ => 16.0 / 9.0 };
        self.g.hold_layers = self.sliding;
        self.g.update(&cam, aspect, dt);
        if self.show_feat && self.feat.is_none() && !self.g.meta.features.is_empty() {
            self.feat = Some(self.g.build_features());
        }
        let rebuilt = self.g.stats.rebuilt;
        let up = unit(self.lon,self.lat);
        self.ground_h = self.g.ground_at(up);
        let mean = flight::average(up,self.alt,self.g.d.r,|u|self.g.drawn_ground_at_world(u));
        self.flight_reference.update(mean,self.ground_h.map(|h|h*self.g.ex),self.alt,dt);
        let n = self.g.d.mv.len();
        if ctx.time - self.last_hist > 0.15 {
            self.last_hist = ctx.time;
            let nm = n.max(1) as f32;
            self.history.push_back([self.alt as f32, self.g.stats.drawn as f32 / nm, self.g.risen.0 as f32 / nm]);
            if self.history.len() > 5000 {
                self.history.pop_front();
            }
        }
        let l = &mut self.log;
        l[0] += 1.0;
        l[1] += dt;
        l[2] += self.g.stats.lod_ms;
        l[3] += self.g.stats.mesh_ms;
        l[4] = l[4].max(self.g.stats.lod_ms);
        l[5] = l[5].max(self.g.stats.mesh_ms);
        l[6] += rebuilt as f64;
        if l[1] >= 1.0 {
            log::info!("PLANETLOD alt_m={:.0} fps={:.1} lod_ms={:.3} (max {:.2}) mesh_ms={:.3} (max {:.2}) rebuilt_per_s={:.0} drawn={} lon={:.6} lat={:.6}", self.alt, l[0] / l[1], l[2] / l[0], l[4], l[3] / l[0], l[5], l[6] / l[1], self.g.stats.drawn, self.lon, self.lat);
            log::info!("FLIGHT ground_m={:.3} reference_m={:.3} target_m={:.3} clearance_m={:.3}",self.ground_h.unwrap_or(0.)*self.g.ex,self.flight_reference.height.unwrap_or(0.),self.flight_reference.target.unwrap_or(0.),self.flight_reference.height.unwrap_or(0.)+self.alt-self.ground_h.unwrap_or(0.)*self.g.ex);
            if let Some(p) = &self.g.pyramid {
                let s=p.lock().unwrap().stats;
                log::info!("PYRAMID selected={} drawn={} triangles={} pending={} cached={} bytes={} fallback={} error_ratio={:.3} resident={} largest_tile={}", s.selected,s.drawn,s.triangles,s.pending,s.cached,s.bytes,s.fallback,s.worst,s.resident,s.largest_tile);
            }
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
        // (The near plane well inside the ground's reach: by the altitude, or half the clearance over the ground
        // below where that is less, so a peak beside the eye isn't cut, looking down most of all.)
        let clearance = self.ground_h.map_or(f64::INFINITY, |h| position.length() - self.g.d.r - h * self.g.ex);
        let mut camera = Camera { position, fov_y: (self.fov as f32).to_radians(), near: (0.2 * self.alt.min(0.5 * clearance)).clamp(0.3, 2000.0) as f32, ..Default::default() };
        camera.look_at(position + fwd, cam_up.as_vec3());
        camera
    }

    fn draw(&self, frame: &mut Frame, ctx: &Context) {
        let cam = frame.camera;
        let eye = cam.position;
        let up = eye.normalize();
        // Light for reading shape: the sun set at the start (`sun=`, fixed to the planet), else from the north-west
        // 35 degrees up, wherever the eye is.
        let east = DVec3::Y.cross(up).normalize_or(DVec3::X);
        let north = up.cross(east);
        let sun = self.sun.unwrap_or_else(|| (up * 0.574 + north * 0.579 - east * 0.579).normalize());
        // (Lit as the planet lab's views are, `0.3 + 0.95 max(0, n·l)`: the frame's ambient 0.3, and a white sun whose
        // adapted brightness, `luminosity^ADAPT`, makes the rest 0.95: 0.95 / 0.7 of full.)
        frame.ambient = VIEW_AMBIENT;
        if self.raw_look {
            frame.graphics.tone_map = false;
            // (And shaded as the lab's views are, on the screen's colours: exact with the tone map off.)
            frame.view_shade = Some(universe_engine::ViewShade { ambient: VIEW_AMBIENT, diffuse: 0.95, sea: 0.6, centre: DVec3::ZERO, radius: self.g.d.r });
        }
        let luminosity = (0.95 / (1.0 - VIEW_AMBIENT as f64)).powf(1.0 / universe_engine::ADAPT as f64);
        frame.light = Some(Light { position: eye + sun * 1.5e11, color: [1.0, 1.0, 1.0], luminosity, reference: 1.5e11, radius: 7e8 });
        let size = frame.size();
        let LandDrawn { drawn, sent, chunks_drawn, tiles_drawn, pattern_tris, drawn_clusters, cl_data, aspect } = self.g.draw(frame, &cam, &Transform::default());
        if self.show_feat && let Some(f) = &self.feat {
            let t = Transform { position: self.g.meta.peak * self.g.d.r, rotation: Quat::IDENTITY, scale: 1.0 };
            frame.model_colored(f, &t, 1.0, 0.0);
        }
        let objects_drawn = self.objects.as_ref().map_or(0, |o| o.draw(self, frame));
        // Pins: a post from the ground and its label.
        let alt_r = (eye.length() - self.g.d.r).max(0.0);
        let post = (0.08 * alt_r).max(0.004 * self.g.d.r).min(0.08 * alt_r.max(1.0));
        for (dir, z, label, peak) in &self.pins {
            let base = *dir * (self.g.d.r + self.g.ex * z);
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
                let base = *dir * (self.g.d.r + self.g.ex * z);
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
        // Landmark markers: the slideshow's shape and colour at a fixed size, those in view; labels on the nearest.
        if self.show_marks {
            let mut shown: Vec<(f64, Vec2, usize)> = vec![];
            let r = MARK_PX;
            for (i, (dir, z, shape, c, _)) in self.marks.iter().enumerate() {
                let base = *dir * (self.g.d.r + self.g.ex * z.max(0.0));
                if dir.dot(eye - base) <= 0.0 {
                    continue;
                }
                let Some(p) = frame.project(base).filter(|p| p.x > 0.0 && p.y > 0.0 && p.x < size.x && p.y < size.y) else { continue };
                let c = *c;
                match shape {
                    0 => frame.hud_triangle_colored([p + Vec2::new(0.0, -r), p + Vec2::new(r, 0.8 * r), p + Vec2::new(-r, 0.8 * r)], [c; 3]),
                    1 => frame.hud_glow(p, 0.75 * r, 12, c, c),
                    2 => frame.hud_ellipse(p, Vec2::splat(1.6 * r), 20, c),
                    3 => frame.hud_rect(p - Vec2::splat(0.8 * r), Vec2::splat(1.6 * r), c),
                    4 => {
                        frame.hud_triangle_colored([p + Vec2::new(0.0, -r), p + Vec2::new(r, 0.0), p + Vec2::new(-r, 0.0)], [c; 3]);
                        frame.hud_triangle_colored([p + Vec2::new(0.0, r), p + Vec2::new(-r, 0.0), p + Vec2::new(r, 0.0)], [c; 3]);
                    }
                    _ => {
                        frame.hud_line(p - Vec2::new(r, 0.0), p + Vec2::new(r, 0.0), c);
                        frame.hud_line(p - Vec2::new(0.0, r), p + Vec2::new(0.0, r), c);
                    }
                }
                shown.push(((eye - base).length(), p, i));
            }
            shown.sort_by(|a, b| a.0.total_cmp(&b.0));
            for (_, p, i) in shown.into_iter().take(MARK_LABELS) {
                let (_, _, _, c, label) = &self.marks[i];
                if label.is_empty() {
                    continue;
                }
                let w = label.len() as f32 * universe_engine::GLYPH;
                frame.text_boxed(p - Vec2::new(w * 0.5, universe_engine::GLYPH + 2.0 * r + 4.0), label, Color::hex(0x111111), *c);
            }
        }
        // The counts the panel shows that need the drawn meshes: the view box every 0.3 s.
        let mut ui = self.ui.lock().unwrap();
        ui.sent = sent;
        ui.objects_drawn = objects_drawn;
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
            let bd = self.g.bd.clone();
            crate::background::spawn(move || {
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
        self.pitch.unwrap_or(90.0 * (self.alt / TILT_HI).clamp(0.0, 1.0))
    }

    /// The page's panel.
    fn panel(&self, frame: &mut Frame, ctx: &Context, ui: &Ui) -> (Hit, Vec<Hit>) {
        use hud_kit::{ACCENT, BAD, GOOD, INK, INK3, WARN};
        let m = &self.g.meta;
        let mut p = Panel::new(8.0, 8.0, PANEL_W, 0.62);
        let fps = ctx.fps;
        let fc = if fps >= 50.0 { GOOD } else if fps >= 25.0 { WARN } else { BAD };
        p.title("Planet LOD", (&format!("{fps:.0} fps"), fc), ("–", ID_HIDE));
        let alt = if self.alt >= 10_000.0 { format!("{} km", thousands((self.alt / 1000.0).round())) } else { format!("{} m", thousands(self.alt.round())) };
        p.big(&alt, "above average ground");
        match self.ground_h {
            Some(h) => {
                p.row("ground under you", &format!("{} m", thousands(h.round())), "");
                p.row("clearance", &format!("{} m",thousands((self.eye().length()-self.g.d.r-h*self.g.ex).round())), "");
            },
            None => p.row("ground under you", "no triangle under the eye", ""),
        }
        let lo = (self.lon + 540.0).rem_euclid(360.0) - 180.0;
        p.row("position", &format!("{:.2}° {}  {:.2}° {}", self.lat.abs(), if self.lat < 0.0 { "S" } else { "N" }, lo.abs(), if lo < 0.0 { "W" } else { "E" }), "");
        p.row("heading · pitch", &format!("{:.0}° · {:.0}° down", self.heading.rem_euclid(360.0), self.pitch_down()), "");
        // Levels of detail.
        let n = self.g.d.mv.len().max(1);
        let lv = (0..self.g.lvl_count.len()).rev().find(|&k| self.g.lvl_count[k] > 0).map_or(0, |k| k + 1);
        let (full, rising) = self.g.risen;
        let rising_badge = if rising > 0 { (format!("{} rising", thousands(rising as f64)), WARN) } else { ("all in".to_string(), GOOD) };
        // (In cluster mode: the finest cluster level drawn, 0 the finest, and the clusters' own counts.)
        let cl_stats = self.g.clusters.as_ref().map(|c| { let c = c.lock().unwrap(); (c.stats, c.row.len(), c.level.iter().copied().max().unwrap_or(0)) });
        let lvb = match cl_stats { Some((st, _, top)) => format!("cluster L{} / {}", st[4], top), None => format!("L{lv} / {}", m.level_d.len()) };
        p.section("LEVEL OF DETAIL", &[(&lvb, ACCENT), (&rising_badge.0, rising_badge.1)]);
        let eye = self.eye();
        let cr = eye.length();
        let ang = (eye.dot(m.peak) / cr).clamp(-1.0, 1.0).acos();
        let d_reg = ((ang * self.g.d.r - m.radius_m).max(0.0)).hypot((cr - self.g.d.r - self.g.ex * m.top_m).max(0.0));
        let dist = if d_reg >= 1000.0 { format!("{:.1} km", d_reg / 1000.0) } else { format!("{d_reg:.0} m") };
        p.row("distance to the pattern", &dist, "");
        match cl_stats {
            Some((st, total, _)) => p.row("clusters chosen", &thousands(st[0] as f64), &format!("of {} · {} kept", kilo(total), kilo(st[3]))),
            None => p.row("pattern vertices drawn", &thousands(self.g.stats.drawn as f64), &format!("of {} · {:.1}%", thousands(n as f64), 100.0 * self.g.stats.drawn as f64 / n as f64)),
        }
        p.bar(self.g.stats.drawn as f32 / n as f32, full as f32 / n as f32);
        p.row("fully risen · rising", &format!("{} · {}", thousands(full as f64), thousands(rising as f64)), "");
        p.row("pattern triangles drawn", &thousands(ui.pattern_tris as f64), "");
        let a = m.level_d.iter().rposition(|&dd| dd > d_reg).map_or(0, |j| j + 1);
        let one = m.level_tris.get(a).copied().unwrap_or(0);
        let more = if one > ui.pattern_tris && ui.pattern_tris > 0 { format!("{:.1}× more", one as f64 / ui.pattern_tris as f64) } else { String::new() };
        p.row("with one level for all", &thousands(one as f64), &more);
        p.row("LOD update", &format!("{:.2} ms", self.g.stats.lod_ms), &format!("meshes {:.2} ms · {} made", self.g.stats.mesh_ms, self.g.stats.rebuilt));
        p.row("planet tiles drawn", &thousands(ui.tiles_drawn as f64), &format!("of {} groups", self.g.tiles.len()));
        if cl_stats.is_some() {
            p.row("clusters drawn", &thousands(ui.chunks_drawn as f64), "in view");
        } else {
            p.row("pattern cells drawn", &thousands(ui.chunks_drawn as f64), &format!("of {}", self.g.chunks.iter().filter(|c| !c.tris.is_empty()).count()));
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
        if let Some(land) = &self.g.pyramid {
            let s = land.lock().unwrap().stats;
            p.section("LAND PYRAMID", &[]);
            p.row("tiles drawn / selected", &format!("{} / {}", s.drawn, s.selected), &format!("{} fallback", s.fallback));
            p.row("triangles", &thousands(s.triangles as f64), "");
            p.row("streaming", &format!("{} pending", s.pending), &format!("{} MB", s.bytes / (1024*1024)));
        }
        if let Some(o) = &self.objects {
            p.section("OBJECTS", &[]);
            p.row("boxes in view", &thousands(ui.objects_drawn as f64), "instanced");
            for (row, families) in o.families.chunks(2).enumerate() {
                let items: Vec<_> = families.iter().enumerate().map(|(i,f)| (f.name.as_str(),f.on,true,ID_BOX_FAMILY+(row*2+i) as u32,f.color)).collect();
                p.colored_chips(&items);
            }
        }
        // The mesh.
        p.section("MESH", &[]);
        p.row("vertices", &thousands(self.g.d.pos.len() as f64), &format!("{} in the pattern", kilo(n)));
        let off = self.g.tiles.iter().map(|g| g.tris.len()).sum::<usize>();
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
        p.seg(&["Mesh", "Real"], if self.g.view == View::Mesh { 0 } else { 1 }, ID_MESH);
        let real = self.g.view == View::Real;
        p.chips(&[("edges", self.g.show_edges && !real, !real, ID_EDGES), ("faces", self.g.show_faces, true, ID_FACES), ("crest & valleys", self.show_feat, !m.features.is_empty(), ID_FEAT)]);
        p.chips(&[("peaks", self.show_peaks, !self.peak_pins.is_empty(), ID_PEAKS), ("landmarks", self.show_marks, !self.marks.is_empty(), ID_MARKS)]);
        p.slider("height ×", self.g.ex as f32, 1.0, 50.0, &format!("{:.0}", self.g.ex), ID_EX);
        if let Some(l) = &self.g.layers {
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
