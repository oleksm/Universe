//! Mini apps on one engine (`docs/miniapps-plan.md`): the planet viewer, the studio, the game, each an
//! `App`, stacked in one window by the `Host`. The host owns the window, renderer, input and the
//! player's frame clock (`Context::dt`, real time, never a world clock); an app owns its own state and
//! whatever world clocks it runs. Only the top app sees input; `Cover` says what the apps under it do.
//!
//! The host is a `Game`, so `run`/`run_prepared` start it like any other.

use crate::app::{Context, Game};
use crate::camera::Camera;
use crate::frame::Frame;

/// What an app over others does to them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cover {
    /// They are neither updated nor drawn (a full app: its own view of its own world).
    Hides,
    /// They are updated (without input), not drawn: their clocks run on, their view is covered.
    Covers,
    /// They are drawn, not updated: their clocks stand still (a panel over a paused scene).
    Pauses,
    /// They are drawn and updated, without input: their clocks run on under it.
    Runs,
}

/// What the host does after an app's update.
pub enum Next {
    Stay,
    /// This app closes; the one under it is on top again (the last one closing exits).
    Close,
    /// Another app opens over this one.
    Open(Box<dyn App>),
    /// This app closes and another takes its place.
    Swap(Box<dyn App>),
}

/// (`Send`: an app is prepared off the window's thread, as `run_prepared` does.)
pub trait App: Send {
    /// Its name (the launcher's, the profiler's).
    fn name(&self) -> &str;
    /// Advance it: input (when on top) and the player's frame clock in `ctx`.
    fn update(&mut self, ctx: &mut Context) -> Next;
    fn camera(&self) -> Camera;
    fn draw(&self, frame: &mut Frame, ctx: &Context);
    /// What it does to the apps under it.
    fn cover(&self) -> Cover {
        Cover::Hides
    }
    /// Whether it handles F3 (the debug view and profiler) itself; if not, the host does for it.
    fn debug_keys(&self) -> bool {
        false
    }
}

/// The host's debug view over the apps (F3: 0 off, 1 frame figures, 2 and the profiler): drawn
/// after them. A client gives its own (the game's frame graph and asset figures); without one,
/// `plain_overlay`.
pub type Overlay = fn(&mut Frame, &Context, u8);

/// The frame's figures and the profiler's scopes as plain text, top right.
pub fn plain_overlay(frame: &mut Frame, ctx: &Context, debug: u8) {
    if debug == 0 {
        return;
    }
    let p = &ctx.perf;
    let mut lines = vec![
        format!("{:.0} FPS  FRAME {:.1} MS", ctx.fps, p.frame_ms),
        format!("UPDATE {:.1}  DRAW {:.1}  RENDER {:.1}  WAIT {:.1} MS", p.update_ms, p.draw_ms, p.render_ms, p.wait_ms),
        format!("{} TRIANGLES  {} LINES  {} MESHES", p.triangles, p.lines, p.resources.meshes),
    ];
    if debug > 1 {
        for (name, ms, calls) in universe_prof::last_frame().into_iter().filter(|x| x.1 >= 0.05).take(24) {
            lines.push(format!("{ms:>7.2} MS  {name}  ({calls}X)"));
        }
    }
    let size = frame.size();
    let line = crate::frame::GLYPH + 2.0;
    let w = lines.iter().map(|l| l.len()).max().unwrap_or(0) as f32 * crate::frame::GLYPH + 12.0;
    let x = size.x - w - 6.0;
    frame.hud_rect(glam::Vec2::new(x, 6.0), glam::Vec2::new(w, line * lines.len() as f32 + 8.0), crate::Color([0.0, 0.0, 0.0, 0.7]));
    for (k, l) in lines.iter().enumerate() {
        frame.text(glam::Vec2::new(x + 6.0, 10.0 + k as f32 * line), l, crate::Color::hex(0x91adbb));
    }
}

/// Which of a stack's apps (bottom first) are updated and drawn, given each one's cover.
#[derive(Debug, PartialEq, Eq)]
pub struct Plan {
    /// Updated this frame, bottom first; the last is the top (the one with input).
    pub updated: Vec<usize>,
    /// Drawn this frame, bottom first; the first one's camera is the frame's.
    pub drawn: Vec<usize>,
}

/// The plan for a stack whose apps cover as `covers` (bottom first).
pub fn plan(covers: &[Cover]) -> Plan {
    let mut updated = vec![];
    let mut drawn = vec![];
    // From the top down: each app is drawn while nothing above it hides, updated while
    // everything above it runs.
    let (mut draws, mut runs) = (true, true);
    for k in (0..covers.len()).rev() {
        if draws {
            drawn.push(k);
        }
        if runs {
            updated.push(k);
        }
        match covers[k] {
            Cover::Hides => (draws, runs) = (false, false),
            Cover::Covers => draws = false,
            Cover::Pauses => runs = false,
            Cover::Runs => {}
        }
    }
    updated.reverse();
    drawn.reverse();
    Plan { updated, drawn }
}

/// The apps in one window, bottom first.
pub struct Host {
    apps: Vec<Box<dyn App>>,
    /// The host's debug view (F3), for apps that don't handle it, and how it's drawn.
    debug: u8,
    overlay: Overlay,
}

impl Host {
    pub fn new(first: Box<dyn App>) -> Self {
        Host { apps: vec![first], debug: 0, overlay: plain_overlay }
    }

    /// Its debug view drawn by `overlay` (the client's own).
    pub fn with_overlay(mut self, overlay: Overlay) -> Self {
        self.overlay = overlay;
        self
    }

    /// The app on top.
    pub fn top(&self) -> &dyn App {
        self.apps.last().expect("a host always has an app").as_ref()
    }

    fn covers(&self) -> Vec<Cover> {
        self.apps.iter().map(|a| a.cover()).collect()
    }
}

impl Game for Host {
    fn update(&mut self, ctx: &mut Context) {
        // F3 for an app that doesn't take it: the debug view, its second press the profiler too.
        if !self.top().debug_keys() && ctx.input.pressed(crate::KeyCode::F3) && !ctx.input.down(crate::KeyCode::ShiftLeft) && !ctx.input.down(crate::KeyCode::ShiftRight) {
            self.debug = (self.debug + 1) % 3;
            universe_prof::enable(self.debug == 2);
        }
        if !self.top().debug_keys() {
            ctx.watch_hitches = self.debug > 0;
        }
        let plan = plan(&self.covers());
        let top = self.apps.len() - 1;
        let mut next = Next::Stay;
        for &k in &plan.updated {
            let _p = universe_prof::scope("app");
            if k == top {
                next = self.apps[k].update(ctx);
            } else {
                // (Under the top one: its clocks run, but the keys aren't its; covered, it
                // needn't prepare what it would draw.)
                let input = std::mem::take(&mut ctx.input);
                ctx.covered = !plan.drawn.contains(&k);
                let _ = self.apps[k].update(ctx);
                ctx.covered = false;
                ctx.input = input;
            }
        }
        match next {
            Next::Stay => {}
            Next::Close => {
                self.apps.pop();
                if self.apps.is_empty() {
                    // (Kept non-empty for `camera` and `draw` until the window closes.)
                    self.apps.push(Box::new(Closed));
                    ctx.exit();
                }
            }
            Next::Open(app) => self.apps.push(app),
            Next::Swap(app) => *self.apps.last_mut().unwrap() = app,
        }
    }

    fn camera(&self) -> Camera {
        let plan = plan(&self.covers());
        self.apps[plan.drawn[0]].camera()
    }

    fn draw(&self, frame: &mut Frame, ctx: &Context) {
        for k in plan(&self.covers()).drawn {
            self.apps[k].draw(frame, ctx);
        }
        if !self.top().debug_keys() {
            (self.overlay)(frame, ctx, self.debug);
        }
    }
}

/// What a host holds after its last app closed, for the frame or two before the window goes.
struct Closed;

impl App for Closed {
    fn name(&self) -> &str {
        "closed"
    }
    fn update(&mut self, _: &mut Context) -> Next {
        Next::Stay
    }
    fn camera(&self) -> Camera {
        Camera::default()
    }
    fn draw(&self, _: &mut Frame, _: &Context) {}
}

/// Start a host whose first app is made by `factory`, off the window's thread (see `run_prepared`).
pub fn run_prepared<A: App + 'static>(config: crate::app::Config, factory: impl FnOnce() -> A + Send + 'static) {
    run_prepared_with(config, plain_overlay, factory);
}

/// `run_prepared` with the client's debug view (`Overlay`).
pub fn run_prepared_with<A: App + 'static>(config: crate::app::Config, overlay: Overlay, factory: impl FnOnce() -> A + Send + 'static) {
    crate::app::run_prepared(config, move || Host::new(Box::new(factory())).with_overlay(overlay));
}

/// Any `Game` as an app that stays until the window closes (the game before it is moved).
pub struct Whole<G: Game + Send>(pub &'static str, pub G);

impl<G: Game + Send> App for Whole<G> {
    fn name(&self) -> &str {
        self.0
    }
    fn update(&mut self, ctx: &mut Context) -> Next {
        self.1.update(ctx);
        Next::Stay
    }
    fn camera(&self) -> Camera {
        self.1.camera()
    }
    fn draw(&self, frame: &mut Frame, ctx: &Context) {
        self.1.draw(frame, ctx);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use Cover::*;

    #[test]
    fn covers_decide_what_is_updated_and_drawn() {
        assert_eq!(plan(&[Hides]), Plan { updated: vec![0], drawn: vec![0] });
        // A full app hides what is under it.
        assert_eq!(plan(&[Runs, Hides]), Plan { updated: vec![1], drawn: vec![1] });
        // A pausing panel shows the scene but stops its clocks.
        assert_eq!(plan(&[Hides, Pauses]), Plan { updated: vec![1], drawn: vec![0, 1] });
        // Running overlays keep what is under them going, down to a pause or a hide.
        assert_eq!(plan(&[Hides, Pauses, Runs, Runs]), Plan { updated: vec![1, 2, 3], drawn: vec![0, 1, 2, 3] });
        assert_eq!(plan(&[Hides, Runs, Runs]), Plan { updated: vec![0, 1, 2], drawn: vec![0, 1, 2] });
        assert_eq!(plan(&[Hides, Hides, Runs]), Plan { updated: vec![1, 2], drawn: vec![1, 2] });
        // A covering app keeps what is under it running, not drawn.
        assert_eq!(plan(&[Hides, Covers]), Plan { updated: vec![0, 1], drawn: vec![1] });
    }
}
