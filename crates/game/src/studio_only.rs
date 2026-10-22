//! Client-owned Studio host. Opening an empty design requires no commissioned
//! ship; its document and preview sessions own their state.

use universe_engine::glam::Vec2;
use universe_engine::apps::{App, Next};
use universe_engine::{Camera, Color, Context, Frame};

pub struct StudioOnly {
    interior: crate::interior::Interior,
    deckplans: Vec<universe_sim::world::deckplan::DeckPlan>,
    /// Dev (UNIVERSE_ISSUE=n): the issue to go to, once the checks have it.
    issue: Option<usize>,
    debug: u8,
    #[cfg(feature = "dev")]
    diagnostics: Diagnostics,
}

impl StudioOnly {
    /// The studio with `design` open (none: a new design with no hull).
    pub fn new(design: Option<&str>) -> Self {
        // Validation can load registry data; keep it inside the preparation worker.
        if let Some(path) = design.filter(|p| p.ends_with(".ship.yaml") || p.ends_with(".ship.yml")) {
            crate::interior::ship_code::check_path(std::path::Path::new(path))
                .unwrap_or_else(|e| panic!("ship code: {e}"));
        }
        let hull_error = design.and_then(|id| prepare_legacy_hull(id).err());
        let mut interior = crate::interior::Interior::new();
        match design {
            Some(name) => interior.open_design(name),
            None => interior.new_design(None),
        }
        // (Its modules matched to their records, as the shipyard does on opening it.)
        interior.refit();
        if let Some(error) = hull_error {
            log::warn!("Studio hull: {error}");
            interior.note(&format!("HULL: {error}"));
        }
        // Saved decks must be present before a startup WALK or test drive is built.
        let mut deckplans = Vec::new();
        interior.sync_decks(&mut deckplans, 0.0);
        if let Ok(t) = crate::devenv::var("UNIVERSE_TOOL") {
            interior.use_tool(&t);
        }
        if let Ok(at) = crate::devenv::var("UNIVERSE_STUDIO_WALK") {
            let values: Vec<f64> = at.split(',').filter_map(|v|v.parse().ok()).collect();
            let at = <[f64;4]>::try_from(values).ok();
            interior.stand_test(at);
        }
        if let Ok(how) = crate::devenv::var("UNIVERSE_BALANCE") {
            interior.balance_room(how == "trim");
        }
        if crate::devenv::var_os("UNIVERSE_DRIVE").is_some() {
            interior.test_drive(crate::devenv::num("UNIVERSE_DRIVE"));
        }
        if let Ok(other) = crate::devenv::var("UNIVERSE_COMPARE") {
            interior.compare_with(&other);
        }
        if let Some(k) = crate::devenv::num("UNIVERSE_VIEW").map(|n| n as u8) {
            interior.flat_view(k);
        }
        if crate::devenv::var_os("UNIVERSE_SHIP_CODE").is_some(){interior.open_code();}
        let issue = crate::devenv::num("UNIVERSE_ISSUE").map(|n| n as usize);
        StudioOnly { interior, deckplans, issue, #[cfg(feature = "dev")] diagnostics: Diagnostics::default(), debug: crate::devenv::var("UNIVERSE_DEBUG").ok().and_then(|v|v.parse().ok()).unwrap_or(0) }
    }
}

/// Legacy JSON stores an imported hull key, not a persistent content handle.
/// Resolve only that model before opening; YAML resolves its own dependencies.
fn prepare_legacy_hull(id: &str) -> Result<(), String> {
    let folder = crate::save::data_dir().join("freefall").join("interiors");
    if id.ends_with(".ship.yaml") || id.ends_with(".ship.yml") || folder.join(format!("{id}.ship.yaml")).exists() {
        return Ok(());
    }
    #[derive(serde::Deserialize)]
    struct HullRef {
        #[serde(default)]
        on: Option<String>,
    }
    // Match Interior::open_design: absent `on` means the design ID is the hull
    // key; an explicitly empty `on` means a design with no hull.
    let saved = std::fs::read_to_string(folder.join(format!("{id}.json")))
        .ok().and_then(|text| serde_json::from_str::<HullRef>(&text).ok());
    let key = saved.as_ref().and_then(|p| p.on.as_deref()).unwrap_or(id);
    if !key.starts_with("hull.import.")
        || universe_sim::world::content::content().handle::<universe_sim::world::ship::ClassSpec>(key).is_some()
    {
        return Ok(());
    }
    let model = universe_sim::world::registry::registry().hulls.iter()
        .filter_map(|record| record.model.as_deref())
        .find(|model| {
            // This is the stable path-derived key used by import::hull_from_gltf.
            let stem = std::path::Path::new(model).file_stem().and_then(|s| s.to_str()).unwrap_or("imported");
            format!("hull.import.{}", stem.to_lowercase().replace(|c: char| !c.is_ascii_alphanumeric(), "_")) == key
        })
        .ok_or_else(|| format!("no installed model for {key}"))?;
    let bytes = std::fs::read(model).map_err(|e| format!("{model}: {e}"))?;
    universe_sim::world::import::commission(&bytes, model).map(|_| ())
}

impl App for StudioOnly {
    fn name(&self) -> &str {
        "studio"
    }

    /// (F3 and SHIFT+F3 are its own: the debug view, the observer.)
    fn debug_keys(&self) -> bool {
        true
    }

    fn update(&mut self, ctx: &mut Context) -> Next {
        let _scope = universe_prof::scope("update/studio");
        crate::visual_assets::maintain();
        #[cfg(feature = "dev")]
        self.observe(ctx);
        let shift = ctx.input.down(universe_engine::KeyCode::ShiftLeft) || ctx.input.down(universe_engine::KeyCode::ShiftRight);
        if ctx.input.pressed(universe_engine::KeyCode::F3) && !shift {
            self.debug = (self.debug+1)%3;
            #[cfg(feature = "dev")]
            let observing = self.diagnostics.on;
            #[cfg(not(feature = "dev"))]
            let observing = false;
            universe_prof::enable(self.debug == 2 || observing);
        }
        let stay = crate::interior::input_with(&mut self.deckplans, ctx, &mut self.interior);
        // Native-hull WALK requests use the same local stand as the embedded host.
        if let Some((at, yaw)) = self.interior.walk.take() {
            self.interior.stand_test(Some([at.x, at.y, at.z, yaw]));
        }
        // (A test drive steering with the mouse takes it, as the game's flight does.)
        let want = self.interior.wants_mouse();
        if ctx.cursor_grabbed() != want {
            ctx.grab_cursor(want);
        }
        if let Some(n) = self.issue
            && self.interior.go_to_issue(n)
        {
            self.issue = None;
        }
        if !stay {
            #[cfg(feature = "dev")]
            crate::devkit::observe::stop(self, ctx);
            return Next::Close;
        }
        Next::Stay
    }

    fn camera(&self) -> Camera {
        Camera::default()
    }

    fn draw(&self, frame: &mut Frame, ctx: &Context) {
        let size = frame.size();
        frame.hud_rect(Vec2::ZERO, size, Color([0.012, 0.018, 0.026, 1.0]));
        crate::interior::draw(frame, "STUDIO", &self.interior);
        crate::hud::studio_assets(frame, ctx, self.debug);
        #[cfg(feature = "dev")]
        if self.diagnostics.on {
            frame.text(Vec2::new(12., 55.), "OBSERVING  SHIFT+F3 TO STOP", Color::hex(0xe5c778));
        }
    }
}

#[cfg(feature = "dev")]
struct Diagnostics {
    on: bool,
    tried: bool,
    port: Option<std::sync::mpsc::Receiver<crate::devkit::observe::Request>>,
    recording: Option<crate::devkit::observe::Recording>,
}
#[cfg(feature = "dev")]
impl Default for Diagnostics {
    fn default() -> Self {
        Self { on: crate::devenv::observing(), tried: false, port: None, recording: None }
    }
}
#[cfg(feature = "dev")]
impl StudioOnly {
    fn observe(&mut self, ctx: &mut Context) {
        use crate::devkit::observe;
        let shift = ctx.input.down(universe_engine::KeyCode::ShiftLeft) || ctx.input.down(universe_engine::KeyCode::ShiftRight);
        if ctx.input.pressed(universe_engine::KeyCode::F3) && shift {
            if self.diagnostics.on {
                observe::stop(self, ctx);
                self.diagnostics.on = false;
                universe_prof::enable(self.debug == 2);
            } else {
                self.diagnostics.on = true;
                observe::start(self, ctx);
            }
        }
        if !self.diagnostics.tried {
            if self.diagnostics.on {
                self.diagnostics.tried = true;
                self.diagnostics.port = observe::listen();
                universe_prof::enable(true);
            }
        }
        ctx.watch_hitches = self.diagnostics.on;
        observe::answer(self, ctx);
        observe::frame(self, ctx);
        // Flush recordings before the app exits through Escape/window navigation.
    }
}
#[cfg(feature = "dev")]
impl crate::devkit::observe::Host for StudioOnly {
    fn status(&self, ctx: &Context) -> serde_json::Value {
        serde_json::json!({"mode":"studio", "view":self.interior.diagnostic_mode(), "fps":ctx.fps,
            "debug":self.debug, "cpu":crate::devkit::observe::cpu()})
    }
    fn resources(&self, ctx: &Context) -> serde_json::Value {
        serde_json::json!({"renderer":ctx.perf.resources,"assets":crate::visual_assets::snapshot(),"world_visuals":crate::globe_assets::snapshot(),"rock_visuals":crate::rocks::snapshot()})
    }
    fn recording(&mut self) -> &mut Option<crate::devkit::observe::Recording> { &mut self.diagnostics.recording }
    fn requests(&self) -> Option<&std::sync::mpsc::Receiver<crate::devkit::observe::Request>> { self.diagnostics.port.as_ref() }
    fn observing(&self) -> bool { self.diagnostics.on }
    fn profiling(&self) -> bool { self.debug == 2 || self.diagnostics.on }
    fn notify(&mut self, text: String) { log::info!("{text}"); }
    fn graphics(&mut self, _query: &str) -> serde_json::Value {
        serde_json::json!({"supported":false,"reason":"Studio uses the shared canvas; world PBR graphics switches do not apply"})
    }
}
