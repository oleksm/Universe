//! The shipyard: the layout studio (see `studio`), on the hull we fly. Open
//! anywhere with the shipyard key; it or ESC (when nothing's being drawn) closes.

use universe_engine::glam::Vec2;
use universe_engine::{Color, Context, Frame};

use crate::keys::Act;
use crate::App;

/// The panel: the studio.
pub struct Shipyard {
    studio: crate::studio::Studio,
}

impl Shipyard {
    pub fn laying_out(_app: &App) -> Self {
        Shipyard { studio: Default::default() }
    }

    /// Its studio (for dev scenarios: a tool picked).
    pub fn studio_mut(&mut self) -> &mut crate::studio::Studio {
        &mut self.studio
    }

    /// Back from a walk-through: the studio as it was left.
    pub fn back_to(studio: crate::studio::Studio) -> Self {
        Shipyard { studio }
    }
}

pub fn open(app: &mut App) -> Option<Shipyard> {
    Some(Shipyard::laying_out(app))
}

/// This frame's input. False: close it.
pub fn input(app: &mut App, ctx: &Context) -> bool {
    if crate::keys::pressed(&ctx.input, Act::Shipyard) {
        return false;
    }
    let spec = app.ship.spec();
    let Some(y) = app.shipyard.as_mut() else { return false };
    let mut studio = std::mem::take(&mut y.studio);
    let stay = crate::studio::input(app, ctx, &spec.key, spec.shape(), &mut studio);
    // A walk-through: the studio put by, the pilot on foot there, first person.
    if let Some(at) = studio.walk.take() {
        app.engine.send(universe_sim::Command::Preview(Some(at)));
        app.preview = Some(studio);
        app.mode = crate::Mode::Pilot;
        app.chase_cam = false;
        return false;
    }
    if let Some(y) = app.shipyard.as_mut() {
        y.studio = studio;
    }
    stay
}

pub fn draw(frame: &mut Frame, app: &App, y: &Shipyard) {
    let size = frame.size();
    frame.hud_rect(Vec2::ZERO, size, Color([0.012, 0.018, 0.026, 1.0]));
    let spec = app.ship.spec();
    let place = station(app).map_or_else(|| "SHIPYARD".to_string(), |s| format!("SHIPYARD - {s}"));
    crate::studio::draw(frame, app, &place, &spec.key, &spec.name, &y.studio);
}

/// The station we're docked at, if any (its name).
fn station(app: &App) -> Option<String> {
    let universe_sim::ShipState::Landed { body, .. } = app.ship.state else { return None };
    let b = &app.view.system.bodies[body];
    (b.kind == universe_sim::world::system::BodyKind::Station).then(|| b.name.to_uppercase())
}
