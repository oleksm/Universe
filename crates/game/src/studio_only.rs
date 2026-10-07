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
}

impl StudioOnly {
    /// The studio with `design` open (none: a new design with no hull).
    pub fn new(design: Option<&str>) -> Self {
        let content = universe_sim::world::content::content();
        let path = "assets/models/mc07.glb";
        let class = std::fs::read(path).map_err(|e| e.to_string()).and_then(|b| universe_sim::world::import::commission(&b, path));
        let spec = content.get(class.unwrap_or_else(|e| panic!("the studio needs the MC-07's model, {path}: {e}")));
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
        let issue = std::env::var("UNIVERSE_ISSUE").ok().and_then(|v| v.parse().ok());
        StudioOnly { interior, spec, deckplans: Vec::new(), issue }
    }
}

impl Game for StudioOnly {
    fn update(&mut self, ctx: &mut Context) {
        let stay = crate::interior::input_with(self.spec, &mut self.deckplans, ctx, &mut self.interior);
        // (A walk through the ship flown: there's none here; the test stand walks.)
        self.interior.walk = None;
        if let Some(n) = self.issue
            && self.interior.go_to_issue(n)
        {
            self.issue = None;
        }
        if !stay {
            ctx.exit();
        }
    }

    fn camera(&self) -> Camera {
        Camera::default()
    }

    fn draw(&self, frame: &mut Frame, _ctx: &Context) {
        let size = frame.size();
        frame.hud_rect(Vec2::ZERO, size, Color([0.012, 0.018, 0.026, 1.0]));
        crate::interior::draw(frame, "STUDIO", &self.interior);
    }
}
