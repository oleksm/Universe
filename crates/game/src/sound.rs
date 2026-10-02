//! What the pilot hears, from inside the ship: what carries through the
//! hull and the cabin air (there's no sound in space). The thrusters hiss
//! where they sit, the drive rumbles through the frame, struck plating
//! rings, the life support hums while the ship's powered; the computer
//! chirps. Driven by what the simulation does (see `audio`).

use std::sync::Mutex;

use universe_engine::glam::DVec3;
use universe_engine::{Context, Jet};
use universe_sim::world::ship::ThrusterRole;
use universe_sim::world::Place;
use universe_sim::{Event, ShipEvent, ShipState, TrafficEvent};

use crate::{App, Mode};

/// Master volume (0..1).
pub const VOLUME: f32 = 0.25;

pub fn click(ctx: &Context, freq: f32) {
    if let Some(a) = ctx.audio() {
        a.tone(freq, freq, 0.04, 0.15);
    }
}

/// The game starts: the ship's systems hum into life (and the sound is working).
pub fn launch(ctx: &Context) {
    if let Some(a) = ctx.audio() {
        a.set_master(VOLUME);
        a.tone(440.0, 660.0, 0.3, 0.18);
    }
}

pub fn chime(ctx: &Context) {
    if let Some(a) = ctx.audio() {
        a.tone(800.0, 1200.0, 0.15, 0.22);
    }
}

/// Which side of the ship (-1 left .. 1 right) a ship seen from `us` is on.
fn side_of(app: Option<&App>, by: usize) -> f32 {
    let Some(app) = app else { return 0.0 };
    let Some(c) = by.checked_sub(1).and_then(|i| app.v.crafts.get(i)) else { return 0.0 };
    let ship = &app.v.ship;
    let local = ship.orientation.inverse() * (c.ship.position - ship.position);
    (local.normalize_or_zero().x as f32).clamp(-1.0, 1.0)
}

pub fn event(ctx: &Context, app: Option<&App>, event: &Event) {
    let Some(a) = ctx.audio() else { return };
    play(a, app, event);
}

/// `event`'s sound on `a` (`app`, if there is one, for where things are).
pub fn play(a: &universe_engine::Audio, app: Option<&App>, event: &Event) {
    match event {
        // Setting down: the gear takes the weight (a thud, its hydraulics).
        Event::Ship(ShipEvent::Landed { .. }) => {
            a.thud(70.0, 0.6, 0.0);
            a.hiss(0.7, 0.12, 0.3, 0.0);
            a.tone(660.0, 660.0, 0.12, 0.2);
        }
        Event::Ship(ShipEvent::LandedAtPort { .. }) => {
            a.tone(660.0, 660.0, 0.12, 0.2);
            a.tone(880.0, 880.0, 0.15, 0.16);
        }
        Event::Ship(ShipEvent::TookOff) => a.hiss(0.6, 0.12, 0.4, 0.0),
        // The flight systems: relays, the plant spooling up (or down).
        Event::Ship(ShipEvent::SystemsOn) => a.spool(true),
        Event::Ship(ShipEvent::SystemsOff) => a.spool(false),
        Event::Ship(ShipEvent::Crashed { .. }) => {
            a.impact(1.0, 0.5, 0.0);
            a.noise(2.5, 0.9);
            a.rattle(1.6, 0.5, 0.0);
        }
        Event::Ship(ShipEvent::WeaponsArming) => a.tone(150.0, 700.0, 2.0, 0.12),
        Event::Ship(ShipEvent::WeaponsHot) => {
            a.thud(140.0, 0.3, 0.0);
            a.tone(1320.0, 1320.0, 0.15, 0.15);
        }
        Event::Ship(ShipEvent::WeaponsSafe) => a.tone(600.0, 200.0, 0.4, 0.12),
        Event::Crew(universe_sim::world::CrewEvent::HatchRefused { .. }) => a.tone(200.0, 150.0, 0.2, 0.2),
        // The hatch: its seals let go with a hiss, the door thuds home.
        Event::Crew(universe_sim::world::CrewEvent::SteppedOutside { .. } | universe_sim::world::CrewEvent::CameAboard) => {
            a.hiss(1.1, 0.35, 0.55, 0.0);
            a.thud(85.0, 0.45, 0.0);
        }
        Event::Crew(_) => a.thud(220.0, 0.2, 0.0),
        Event::Traffic(TrafficEvent::PadAssigned { .. }) => a.tone(1100.0, 1100.0, 0.12, 0.2),
        Event::Traffic(TrafficEvent::Holding { .. }) => a.tone(500.0, 400.0, 0.3, 0.2),
        // Struck: the plating rings and the frame takes it; harder, bits rattle off.
        Event::Ship(ShipEvent::Collided { with, speed }) => {
            let s = (*speed as f32 / 25.0).min(1.0);
            a.impact(0.2 + 0.8 * s, 0.7, side_of(app, *with));
            if s > 0.4 {
                a.rattle(0.4 + s, 0.4 * s, 0.0);
            }
        }
        Event::Ship(ShipEvent::StruckRock { speed, .. }) => {
            let s = (*speed as f32 / 25.0).min(1.0);
            a.impact(0.3 + 0.7 * s, 0.6, 0.0);
            a.rattle(0.5 + s, 0.3 + 0.3 * s, 0.0);
        }
        Event::Ship(ShipEvent::Bumped) => a.impact(0.2, 0.9, 0.0),
        // A round through the plating: the crack and ring of it, from where
        // it came; a breach, the air going out.
        Event::Ship(ShipEvent::Hit { by, damage, hull, .. }) => {
            let pan = side_of(app, *by);
            let s = (*damage as f32 * 6.0).clamp(0.2, 1.0);
            a.impact(s, 1.4, pan);
            if *damage > 0.04 || *hull < 0.5 {
                a.hiss(1.5 + 2.0 * s, 0.2 + 0.25 * s, 0.9, pan);
                a.rattle(0.5, 0.25, pan);
            }
        }
        Event::Ship(ShipEvent::Respawned) => a.tone(440.0, 880.0, 0.3, 0.2),
        Event::Ship(ShipEvent::EnteredSystem { .. }) => a.tone(880.0, 880.0, 0.2, 0.2),
        Event::Ship(ShipEvent::HyperdriveEngaged) => {
            a.tone(150.0, 1400.0, 0.7, 0.2);
            a.thud(50.0, 0.6, 0.0);
        }
        Event::Ship(ShipEvent::HyperdriveDisengaged) => {
            a.tone(1400.0, 150.0, 0.6, 0.2);
            a.thud(50.0, 0.5, 0.0);
        }
        Event::HyperdriveArrived { .. } => a.tone(880.0, 1320.0, 0.25, 0.18),
        Event::Traffic(TrafficEvent::ClearanceGranted { .. }) => {
            a.tone(880.0, 880.0, 0.1, 0.2);
            a.tone(1320.0, 1320.0, 0.25, 0.14);
        }
        Event::Traffic(TrafficEvent::ClearanceDenied { .. }) | Event::Refused { .. } => a.tone(180.0, 160.0, 0.35, 0.22),
        Event::Traffic(TrafficEvent::ClearanceCancelled) => a.tone(600.0, 300.0, 0.3, 0.18),
        Event::Autopilot { on: true } | Event::Following { what: Some(_) } => a.tone(500.0, 900.0, 0.2, 0.18),
        Event::Following { what: None } | Event::Autopilot { on: false } => a.tone(900.0, 500.0, 0.2, 0.18),
        Event::NavTargetSet { .. } => a.tone(1000.0, 1300.0, 0.08, 0.14),
        // Released by the deck's clamps.
        Event::Ship(ShipEvent::Launched { .. }) => {
            a.thud(110.0, 0.45, 0.0);
            a.hiss(0.4, 0.15, 0.5, 0.0);
        }
        // Into the gate: a deep swell and the rush of it; out, it fades.
        Event::Ship(ShipEvent::GateEntered { .. }) => {
            a.tone(120.0, 1800.0, 1.5, 0.16);
            a.hiss(2.5, 0.35, 0.15, 0.0);
            a.thud(40.0, 0.8, 0.0);
        }
        Event::Ship(ShipEvent::GateArrived { .. }) => {
            a.tone(1800.0, 300.0, 0.8, 0.14);
            a.thud(45.0, 0.6, 0.0);
        }
        Event::RouteStop { .. } => a.tone(700.0, 1050.0, 0.25, 0.16),
        Event::RouteComplete => {
            a.tone(523.0, 523.0, 0.2, 0.18);
            a.tone(659.0, 659.0, 0.4, 0.14);
            a.tone(784.0, 784.0, 0.7, 0.1);
        }
        Event::RouteBlocked { .. } => a.tone(300.0, 150.0, 0.5, 0.22),
        // The station's pumps: the hose's valve, the fuel going in.
        Event::Refuelled { .. } => {
            a.thud(160.0, 0.3, 0.0);
            a.hiss(1.4, 0.15, 0.1, 0.0);
        }
        Event::Repaired { .. } => {
            a.impact(0.15, 1.6, -0.4);
            a.impact(0.15, 1.5, 0.4);
        }
        // The machine: its works clunk, the can drops into the tray.
        Event::Vended { .. } => {
            a.thud(140.0, 0.35, 0.0);
            a.impact(0.12, 2.2, 0.1);
            a.rattle(0.25, 0.15, 0.1);
        }
        // Anything else is silent (add a sound here for a new event that should have one).
        _ => {}
    }
}

/// Walking: where the last step fell, and how far since.
static STEPS: Mutex<Option<(DVec3, f64)>> = Mutex::new(None);
/// A step every this many metres.
const STRIDE: f64 = 0.75;

/// Continuous layers follow the ship state every frame.
pub fn update(ctx: &Context, app: &App) {
    let Some(a) = ctx.audio() else { return };
    let ship = &app.v.ship;
    let piloting = app.mode == Mode::Pilot && !app.paused;
    let aboard = piloting && !matches!(app.v.crew.place, Place::Outside { .. });
    let flying = matches!(ship.state, ShipState::Flying) && !app.paused;
    // The thrusters, each where it sits; the main drive's nozzles together.
    let s = ship.spec();
    let (mut lo, mut hi) = (DVec3::splat(f64::INFINITY), DVec3::splat(f64::NEG_INFINITY));
    for t in &s.thrusters {
        lo = lo.min(t.at);
        hi = hi.max(t.at);
    }
    let half = ((hi.x - lo.x) * 0.5).max(1.0);
    let length = (hi.z - lo.z).max(1.0);
    let mut jets = Vec::with_capacity(s.thrusters.len());
    let (mut main, mut mains) = (0.0f32, 0);
    for (k, t) in s.thrusters.iter().enumerate() {
        let level = if aboard && ship.powered { ship.jets.get(k).copied().unwrap_or(0.0) as f32 } else { 0.0 };
        if t.role == ThrusterRole::Main {
            main += level;
            mains += 1;
            continue;
        }
        jets.push(Jet { level, pan: (t.at.x / half) as f32, near: ((hi.z - t.at.z) / length) as f32, lift: t.role == ThrusterRole::Lift });
    }
    a.set_jets(&jets);
    let engine = if flying && aboard && !ship.hyperdrive { main / mains.max(1) as f32 } else { 0.0 };
    a.set_engine(engine);
    // Aboard a powered ship, the life support.
    a.set_ambience(if aboard && ship.powered { 1.0 } else { 0.0 });
    let lasing = app.v.beams.iter().any(|b| b.owner == universe_sim::PLAYER);
    if flying && ship.hyperdrive {
        let pitch = 40.0 + 9.0 * (ship.velocity.length().max(1.0).log10() as f32);
        a.set_drone(0.6, pitch);
    } else if lasing {
        a.set_drone(0.45, 180.0 + 120.0 * ship.laser_heat as f32);
    } else {
        a.set_drone(0.0, 60.0);
    }
    // On foot: a step every stride; aboard, boots on the deck plating;
    // outside, softer on the ground.
    let at = match app.v.crew.place {
        Place::Aboard { position, .. } => Some((position, true)),
        Place::Outside { position, .. } => Some((position, false)),
        Place::Seat => None,
    };
    let Ok(mut steps) = STEPS.lock() else { return };
    match at {
        Some((p, inside)) if piloting => {
            let (last, walked) = steps.get_or_insert((p, 0.0));
            let moved = p.distance(*last);
            // (A jump in place, a respawn: start over.)
            *walked = if moved > 5.0 { 0.0 } else { *walked + moved };
            *last = p;
            if *walked >= STRIDE {
                *walked -= STRIDE;
                if inside {
                    a.thud(150.0, 0.22, 0.0);
                    a.impact(0.05, 2.5, 0.0);
                } else {
                    a.thud(90.0, 0.16, 0.0);
                    a.hiss(0.08, 0.05, 0.2, 0.0);
                }
            }
        }
        _ => *steps = None,
    }
}

/// The gun fired (once or more this frame): the mass driver's kick through
/// the frame, the round's snap leaving, the breech.
pub fn gunshot(ctx: &Context) {
    if let Some(a) = ctx.audio() {
        a.thud(60.0, 0.6, 0.0);
        a.impact(0.12, 2.0, 0.0);
        a.hiss(0.12, 0.12, 0.8, 0.0);
    }
}

/// One of our rounds struck home.
pub fn hit_confirmed(ctx: &Context) {
    if let Some(a) = ctx.audio() {
        a.tone(1600.0, 1600.0, 0.04, 0.14);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use universe_engine::Audio;

    const RATE: f32 = 44_100.0;

    /// How a sound is started, frame by frame (the time in seconds).
    type Start = Box<dyn Fn(&Audio, f32)>;

    /// A 16-bit stereo WAV of `samples`.
    fn wav(path: &std::path::Path, samples: &[(f32, f32)]) {
        let data: Vec<u8> = samples.iter().flat_map(|&(l, r)| [l, r]).flat_map(|v| ((v.clamp(-1.0, 1.0) * 32767.0) as i16).to_le_bytes()).collect();
        let mut out = Vec::new();
        out.extend(b"RIFF");
        out.extend((36 + data.len() as u32).to_le_bytes());
        out.extend(b"WAVEfmt ");
        out.extend(16u32.to_le_bytes());
        out.extend(1u16.to_le_bytes());
        out.extend(2u16.to_le_bytes());
        out.extend((RATE as u32).to_le_bytes());
        out.extend((RATE as u32 * 4).to_le_bytes());
        out.extend(4u16.to_le_bytes());
        out.extend(16u16.to_le_bytes());
        out.extend(b"data");
        out.extend((data.len() as u32).to_le_bytes());
        out.extend(data);
        std::fs::write(path, out).unwrap();
    }

    /// Each sound, rendered: (name, how it's started, seconds).
    fn sounds() -> Vec<(&'static str, Start, f32)> {
        let ev = |e: Event| -> Start { Box::new(move |a, t| if t == 0.0 { play(a, None, &e) }) };
        // A thruster: fired for 0.3 s twice, on the left, near the cabin.
        let jet = |lift: bool| -> Start {
            Box::new(move |a, t| a.set_jets(&[Jet { level: if (0.1..0.4).contains(&t) || (0.9..1.6).contains(&t) { 1.0 } else { 0.0 }, pan: -0.8, near: 0.8, lift }]))
        };
        vec![
            ("thruster", jet(false), 2.2),
            ("lift", jet(true), 2.2),
            ("drive", Box::new(|a, t| a.set_engine(if t < 2.0 { 1.0 } else { 0.0 })), 2.8),
            ("cabin", Box::new(|a, t| a.set_ambience(if t < 2.0 { 1.0 } else { 0.0 })), 3.0),
            ("hit", ev(Event::Ship(ShipEvent::Hit { by: 0, damage: 0.08, hull: 0.6, weapon: true })), 3.0),
            ("graze", ev(Event::Ship(ShipEvent::Hit { by: 0, damage: 0.01, hull: 0.9, weapon: true })), 2.0),
            ("collision", ev(Event::Ship(ShipEvent::Collided { with: 0, speed: 20.0 })), 2.5),
            ("crash", ev(Event::Ship(ShipEvent::Crashed { body: String::new() })), 3.0),
            ("landed", ev(Event::Ship(ShipEvent::Landed { body: String::new(), station: true })), 1.5),
            ("power up", ev(Event::Ship(ShipEvent::SystemsOn)), 2.0),
            ("power down", ev(Event::Ship(ShipEvent::SystemsOff)), 2.0),
            ("hatch", ev(Event::Crew(universe_sim::world::CrewEvent::CameAboard)), 1.5),
            ("gate", ev(Event::Ship(ShipEvent::GateEntered { to: String::new() })), 3.0),
            ("clearance", ev(Event::Traffic(TrafficEvent::ClearanceGranted { target: String::new(), kind: universe_sim::world::ClearanceKind::Dock })), 1.0),
            ("gun", Box::new(|a, t| if t == 0.0 {
                a.thud(60.0, 0.6, 0.0);
                a.impact(0.12, 2.0, 0.0);
                a.hiss(0.12, 0.12, 0.8, 0.0);
            }), 1.0),
        ]
    }

    #[test]
    fn every_sound_is_heard_and_none_clips() {
        let dir = std::env::var_os("UNIVERSE_SOUND_WAVS").map(std::path::PathBuf::from);
        for (name, start, seconds) in sounds() {
            let a = Audio::offline(RATE);
            a.set_master(VOLUME);
            let mut all = Vec::new();
            // (Driven a frame at a time, as the game does.)
            let frame = (RATE / 60.0) as usize;
            for f in 0..(seconds * 60.0) as usize {
                start(&a, f as f32 / 60.0);
                all.extend(a.render(frame));
            }
            let peak = all.iter().map(|(l, r)| l.abs().max(r.abs())).fold(0.0, f32::max);
            let rms = (all.iter().map(|(l, r)| l * l + r * r).sum::<f32>() / (2 * all.len()) as f32).sqrt();
            eprintln!("{name:<12} peak {peak:.2} rms {rms:.3}");
            assert!(peak > 0.02, "{name}: heard (peak {peak})");
            assert!(peak < 0.9, "{name}: clipping (peak {peak})");
            if let Some(d) = &dir {
                wav(&d.join(format!("{name}.wav")), &all);
            }
        }
    }
}
