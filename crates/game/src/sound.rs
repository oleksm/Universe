use universe_engine::Context;
use universe_sim::{Event, ShipEvent, ShipState, TrafficEvent};

use crate::{App, Mode};

/// Master volume (0..1).
pub const VOLUME: f32 = 0.25;

pub fn click(ctx: &Context, freq: f32) {
    if let Some(a) = ctx.audio() {
        a.tone(freq, freq, 0.04, 0.15);
    }
}

/// Rising two-note fanfare when the game starts: confirms sound is working.
pub fn launch(ctx: &Context) {
    if let Some(a) = ctx.audio() {
        a.set_master(VOLUME);
        a.tone(220.0, 440.0, 0.35, 0.22);
        a.tone(330.0, 660.0, 0.6, 0.15);
    }
}

pub fn chime(ctx: &Context) {
    if let Some(a) = ctx.audio() {
        a.tone(800.0, 1200.0, 0.15, 0.22);
    }
}

pub fn event(ctx: &Context, event: &Event) {
    let Some(a) = ctx.audio() else { return };
    match event {
        Event::Ship(ShipEvent::Landed { .. }) => {
            a.tone(660.0, 660.0, 0.12, 0.25);
            a.tone(990.0, 990.0, 0.3, 0.18);
        }
        Event::Ship(ShipEvent::TookOff) => a.tone(300.0, 500.0, 0.2, 0.25),
        Event::Ship(ShipEvent::Crashed { .. }) => a.noise(1.8, 0.6),
        Event::Ship(ShipEvent::WeaponsArming) => a.tone(150.0, 700.0, 2.0, 0.15),
        Event::Ship(ShipEvent::WeaponsHot) => {
            a.tone(880.0, 880.0, 0.08, 0.2);
            a.tone(1320.0, 1320.0, 0.15, 0.2);
        }
        Event::Ship(ShipEvent::WeaponsSafe) => a.tone(600.0, 200.0, 0.4, 0.15),
        Event::Crew(universe_sim::world::CrewEvent::HatchRefused { .. }) => a.tone(200.0, 150.0, 0.2, 0.2),
        Event::Crew(universe_sim::world::CrewEvent::SteppedOutside { .. } | universe_sim::world::CrewEvent::CameAboard) => {
            a.noise(0.5, 0.15);
            a.tone(300.0, 180.0, 0.5, 0.12);
        }
        Event::Crew(_) => a.tone(500.0, 500.0, 0.05, 0.15),
        Event::Traffic(TrafficEvent::PadAssigned { .. }) => a.tone(1100.0, 1100.0, 0.12, 0.2),
        Event::Traffic(TrafficEvent::Holding { .. }) => a.tone(500.0, 400.0, 0.3, 0.2),
        Event::Ship(ShipEvent::Collided { speed, .. }) => a.noise(0.4, (0.2 + *speed as f32 * 0.02).min(0.7)),
        Event::Ship(ShipEvent::Hit { damage, .. }) => a.noise(0.15, (0.2 + *damage as f32 * 4.0).min(0.6)),
        Event::Ship(ShipEvent::Respawned) => a.tone(440.0, 880.0, 0.3, 0.25),
        Event::Ship(ShipEvent::EnteredSystem { .. }) => a.tone(880.0, 880.0, 0.2, 0.25),
        Event::Ship(ShipEvent::HyperdriveEngaged) => a.tone(150.0, 1400.0, 0.7, 0.3),
        Event::Ship(ShipEvent::HyperdriveDisengaged) => a.tone(1400.0, 150.0, 0.6, 0.3),
        Event::HyperdriveArrived { .. } => a.tone(880.0, 1320.0, 0.25, 0.2),
        Event::Traffic(TrafficEvent::ClearanceGranted { .. }) => {
            a.tone(880.0, 880.0, 0.1, 0.22);
            a.tone(1320.0, 1320.0, 0.25, 0.15);
        }
        Event::Traffic(TrafficEvent::ClearanceDenied { .. }) | Event::Refused { .. } => a.tone(180.0, 160.0, 0.35, 0.25),
        Event::Traffic(TrafficEvent::ClearanceCancelled) => a.tone(600.0, 300.0, 0.3, 0.2),
        Event::Autopilot { on: true } | Event::Following { what: Some(_) } => a.tone(500.0, 900.0, 0.2, 0.2),
        Event::Following { what: None } => a.tone(900.0, 500.0, 0.2, 0.2),
        Event::Autopilot { on: false } => a.tone(900.0, 500.0, 0.2, 0.2),
        Event::NavTargetSet { .. } => a.tone(1000.0, 1300.0, 0.08, 0.15),
        Event::Ship(ShipEvent::LandedAtPort { .. }) => {
            a.tone(660.0, 660.0, 0.12, 0.25);
            a.tone(880.0, 880.0, 0.15, 0.2);
            a.tone(1320.0, 1320.0, 0.4, 0.15);
        }
        Event::Ship(ShipEvent::Bumped) => a.noise(0.3, 0.35),
        Event::Ship(ShipEvent::Launched { .. }) => a.tone(200.0, 800.0, 0.5, 0.25),
        Event::Ship(ShipEvent::GateEntered { .. }) => {
            a.tone(120.0, 2400.0, 1.5, 0.25);
            a.noise(2.0, 0.25);
        }
        Event::Ship(ShipEvent::GateArrived { .. }) => a.tone(2400.0, 300.0, 0.8, 0.22),
        Event::RouteStop { .. } => a.tone(700.0, 1050.0, 0.25, 0.18),
        Event::RouteComplete => {
            a.tone(523.0, 523.0, 0.2, 0.2);
            a.tone(659.0, 659.0, 0.4, 0.15);
            a.tone(784.0, 784.0, 0.7, 0.12);
        }
        Event::RouteBlocked { .. } => a.tone(300.0, 150.0, 0.5, 0.25),
        // Anything else is silent (add a sound here for a new event that should have one).
        _ => {}
    }
}

/// Continuous layers follow the ship state every frame.
pub fn update(ctx: &Context, app: &App) {
    let Some(a) = ctx.audio() else { return };
    let ship = &app.v.ship;
    let flying = matches!(ship.state, ShipState::Flying) && !app.paused;
    let engine = if flying && app.mode == Mode::Pilot && !ship.hyperdrive { ship.throttle as f32 } else { 0.0 };
    a.set_engine(engine);
    let lasing = app.v.beams.iter().any(|b| b.owner == universe_sim::PLAYER);
    if flying && ship.hyperdrive {
        let pitch = 40.0 + 9.0 * (ship.velocity.length().max(1.0).log10() as f32);
        a.set_drone(0.7, pitch);
    } else if lasing {
        a.set_drone(0.5, 180.0 + 120.0 * ship.laser_heat as f32);
    } else {
        a.set_drone(0.0, 60.0);
    }
}

/// The gun fired (once or more this frame).
pub fn gunshot(ctx: &Context) {
    if let Some(a) = ctx.audio() {
        a.tone(220.0, 70.0, 0.07, 0.22);
        a.noise(0.05, 0.15);
    }
}

/// One of our rounds struck home.
pub fn hit_confirmed(ctx: &Context) {
    if let Some(a) = ctx.audio() {
        a.tone(1600.0, 1600.0, 0.04, 0.18);
    }
}
