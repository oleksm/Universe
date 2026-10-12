//! The interior studio alone (`freefall --studio [design]`): no universe, no
//! galaxy, no world running, only the studio in a window, on the MC-07's hull spec
//! (imported from its model, as a hull the studio can design in) for any design
//! that has one.

use universe_engine::glam::Vec2;
use universe_engine::{Camera, Color, Context, Frame, Game};

pub struct StudioOnly {
    interior: crate::interior::Interior,
    spec: &'static universe_sim::world::ship::ClassSpec,
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
        crate::mounted::prepare();
        let started = std::time::Instant::now();
        let content = universe_sim::world::content::content();
        let path = "assets/models/mc07.glb";
        let class = std::fs::read(path).map_err(|e| e.to_string()).and_then(|b| universe_sim::world::import::commission(&b, path));
        let spec = content.get(class.unwrap_or_else(|e| panic!("the studio needs the MC-07's model, {path}: {e}")));
        log::info!("startup Studio hull physics: {:.3} ms", started.elapsed().as_secs_f64()*1000.);
        let mut interior = crate::interior::Interior::new();
        match design {
            Some(name) => interior.open_design(name),
            None => interior.new_design(None),
        }
        // (Its modules matched to their records, as the shipyard does on opening it.)
        interior.refit();
        if let Ok(t) = std::env::var("UNIVERSE_TOOL") {
            interior.use_tool(&t);
        }
        if let Ok(at) = std::env::var("UNIVERSE_STUDIO_WALK") {
            let values: Vec<f64> = at.split(',').filter_map(|v|v.parse().ok()).collect();
            let at = <[f64;4]>::try_from(values).ok();
            interior.stand_test(at);
        }
        if let Ok(how) = std::env::var("UNIVERSE_BALANCE") {
            interior.balance_room(how == "trim");
        }
        if std::env::var_os("UNIVERSE_DRIVE").is_some() {
            interior.test_drive(crate::devenv::num("UNIVERSE_DRIVE"));
        }
        if let Ok(other) = std::env::var("UNIVERSE_COMPARE") {
            interior.compare_with(&other);
        }
        if let Some(k) = crate::devenv::num("UNIVERSE_VIEW").map(|n| n as u8) {
            interior.flat_view(k);
        }
        if std::env::var_os("UNIVERSE_SHIP_CODE").is_some(){interior.open_code();}
        let issue = crate::devenv::num("UNIVERSE_ISSUE").map(|n| n as usize);
        StudioOnly { interior, spec, deckplans: Vec::new(), issue, #[cfg(feature = "dev")] diagnostics: Diagnostics::default(), debug: std::env::var("UNIVERSE_DEBUG").ok().and_then(|v|v.parse().ok()).unwrap_or(0) }
    }
}

impl Game for StudioOnly {
    fn update(&mut self, ctx: &mut Context) {
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
        let stay = crate::interior::input_with(self.spec, &mut self.deckplans, ctx, &mut self.interior);
        // (A test drive steering with the mouse takes it, as the game's flight does.)
        let want = self.interior.wants_mouse();
        if ctx.cursor_grabbed() != want {
            ctx.grab_cursor(want);
        }
        // (A walk through the ship flown: there's none here; the test stand walks.)
        self.interior.walk = None;
        if let Some(n) = self.issue
            && self.interior.go_to_issue(n)
        {
            self.issue = None;
        }
        if !stay {
            #[cfg(feature = "dev")]
            crate::devkit::observe::stop(self, ctx);
            ctx.exit();
        }
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
        Self { on: std::env::var("UNIVERSE_OBSERVING").as_deref() == Ok("1"), tried: false, port: None, recording: None }
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
