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
}

impl Host {
    pub fn new(first: Box<dyn App>) -> Self {
        Host { apps: vec![first] }
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
        let plan = plan(&self.covers());
        let top = self.apps.len() - 1;
        let mut next = Next::Stay;
        for k in plan.updated {
            let _p = universe_prof::scope("app");
            if k == top {
                next = self.apps[k].update(ctx);
            } else {
                // (Under the top one: its clocks run, but the keys aren't its.)
                let input = std::mem::take(&mut ctx.input);
                let _ = self.apps[k].update(ctx);
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
    crate::app::run_prepared(config, move || Host::new(Box::new(factory())));
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
    }
}
