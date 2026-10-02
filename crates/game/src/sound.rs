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
/// The score's loudness under everything else (0..1).
pub const MUSIC_LEVEL: f32 = 0.6;

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

/// A vending machine's sale, as heard standing at it: the motor, the drop,
/// then (`drink`) a can cracked open, fizzing, a few gulps; or a wrapper
/// torn open and three crunchy bites.
fn vend(a: &universe_engine::Audio, drink: bool) {
    a.motor(0.7, 0.25);
    a.after(0.72, |a| {
        a.thud(if drink { 95.0 } else { 130.0 }, if drink { 0.45 } else { 0.3 }, 0.1);
        a.impact(if drink { 0.12 } else { 0.05 }, 2.2, 0.1);
        a.rattle(0.2, 0.12, 0.1);
    });
    if drink {
        // The ring-pull's click, the gas out, the fizz; a pause, then gulps.
        a.after(1.4, |a| {
            a.thud(1100.0, 0.12, 0.0);
            a.hiss(0.3, 0.3, 0.95, 0.0);
            a.fizz(2.2, 0.5);
        });
        for k in 0..3 {
            a.after(2.4 + 0.45 * k as f32, |a| a.thud(170.0, 0.22, 0.0));
        }
    } else {
        // The wrapper torn (a bright rustle), then bites.
        a.after(1.3, |a| a.hiss(0.45, 0.12, 1.0, 0.0));
        for k in 0..3 {
            a.after(2.0 + 0.6 * k as f32, |a| a.crunch(0.6));
        }
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
        Event::Ship(ShipEvent::Landed { station, .. }) => {
            if *station {
                a.music_swell();
            }
            a.thud(70.0, 0.6, 0.0);
            a.hiss(0.7, 0.12, 0.3, 0.0);
            a.tone(660.0, 660.0, 0.12, 0.2);
        }
        Event::Ship(ShipEvent::LandedAtPort { .. }) => {
            a.music_swell();
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
        Event::Ship(ShipEvent::EnteredSystem { .. }) => {
            a.tone(880.0, 880.0, 0.2, 0.2);
            a.music_swell();
        }
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
            a.music_swell();
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
        // The yard's pumps moving the fuel.
        Event::Trimmed => {
            a.thud(160.0, 0.3, 0.0);
            a.hiss(1.8, 0.12, 0.1, 0.0);
        }
        Event::Repaired { .. } => {
            a.impact(0.15, 1.6, -0.4);
            a.impact(0.15, 1.5, 0.4);
        }
        // The machine: its motor runs, what's bought drops into the tray;
        // then a drink opened and drunk, or a snack unwrapped and eaten.
        Event::Vended { what, .. } => vend(a, what.contains("COLA") || what.contains("WATER")),
        // Anything else is silent (add a sound here for a new event that should have one).
        _ => {}
    }
}

/// The alarms: when each last sounded (wall clock: time warp doesn't hurry
/// them), and what was last seen of the hull.
struct Alarms {
    klaxon: Option<std::time::Instant>,
    fuel: Option<std::time::Instant>,
    proximity: Option<std::time::Instant>,
    missile: Option<std::time::Instant>,
    /// The missile warble's next note high.
    high: bool,
    hull: f64,
}

static ALARMS: Mutex<Alarms> = Mutex::new(Alarms { klaxon: None, fuel: None, proximity: None, missile: None, high: false, hull: 1.0 });

/// The hull below this: the master caution, once.
pub const HULL_CAUTION: f64 = 0.5;
/// Below this: the klaxon, until it's mended.
pub const HULL_CRITICAL: f64 = 0.25;
/// The tank below this share, burning: the low-fuel beeps.
pub const FUEL_LOW: f64 = 0.1;
/// A collision ahead faster than this (m/s) sets the proximity beeps going
/// (slower is a touchdown).
const PROXIMITY_SPEED: f64 = 8.0;
/// They start this far off (s).
const PROXIMITY_TIME: f64 = 30.0;

/// The alarms, as the ship's state calls for them.
fn alarms(a: &universe_engine::Audio, app: &App) {
    use std::time::{Duration, Instant};
    let Ok(mut al) = ALARMS.lock() else { return };
    let ship = &app.v.ship;
    let now = Instant::now();
    let since = |t: Option<Instant>| t.map_or(Duration::MAX, |t| now - t);
    let seated = app.mode == Mode::Pilot && app.v.crew.seated() && !app.paused;
    // The hull: a caution as it drops past half; critical, the klaxon.
    if seated && ship.hull < HULL_CAUTION && al.hull >= HULL_CAUTION {
        a.alarm(880.0, 880.0, 0.18, 0.18);
        a.alarm(660.0, 660.0, 0.3, 0.18);
    }
    al.hull = ship.hull;
    if seated && ship.hull < HULL_CRITICAL && since(al.klaxon) > Duration::from_millis(2500) {
        al.klaxon = Some(now);
        // (Two whoops.)
        a.alarm(500.0, 1000.0, 0.5, 0.2);
        a.alarm_after(0.6, 500.0, 1000.0, 0.5, 0.2);
    }
    // Low fuel, burning: three beeps, again every twenty seconds.
    let burning = ship.jets.iter().any(|&j| j > 0.02);
    let low = ship.fuel < FUEL_LOW * ship.spec().fuel_capacity;
    if seated && low && burning && since(al.fuel) > Duration::from_secs(20) {
        al.fuel = Some(now);
        for k in 0..3 {
            a.alarm_after(0.2 * k as f32, 1200.0, 1200.0, 0.09, 0.14);
        }
    }
    // Impact ahead (the warning on): beeps quicker as it nears, all but one tone at the last.
    if let Some(c) = app.collision.as_ref().and_then(|p| p.collision.as_ref())
        && seated
        && c.speed > PROXIMITY_SPEED
        && c.time < PROXIMITY_TIME
    {
        let every = Duration::from_secs_f64((c.time / 8.0).clamp(0.08, 1.2));
        if since(al.proximity) > every {
            al.proximity = Some(now);
            a.alarm(1500.0, 1500.0, 0.06, 0.14);
        }
    }
    // A missile tracking us: a fast two-tone warble.
    let inbound = app.v.missiles.iter().any(|m| m.4 && m.0 == app.v.ship_system);
    if seated && inbound && since(al.missile) > Duration::from_millis(250) {
        al.missile = Some(now);
        al.high = !al.high;
        let f = if al.high { 1400.0 } else { 1000.0 };
        a.alarm(f, f, 0.12, 0.13);
    }
}

/// Dynamic pressure (Pa) at which the air over the hull roars its loudest.
const AIR_ROAR: f64 = 40_000.0;
/// Thinner than this (kg/m³), there's no air to carry sound: vacuum.
const AIR_THIN: f64 = 0.005;

/// The air density at `p` (world, kg/m³), and the body it's of.
fn air_at(app: &App, p: DVec3) -> (f64, Option<usize>) {
    let sys = &app.view.system;
    sys.bodies
        .iter()
        .enumerate()
        .filter_map(|(i, b)| {
            let atm = b.rail.atmosphere.as_ref()?;
            let altitude = p.distance(app.view.positions[i]) - b.rail.radius;
            Some((atm.density(altitude), Some(i)))
        })
        .fold((0.0, None), |a, b| if b.0 > a.0 { b } else { a })
}

/// Air, or the lack of it: over the hull in flight, the rush of it by
/// its dynamic pressure (½ρv², roaring on the way in); on foot, wind
/// where there's air (and other ships' engines carried through it), and in
/// vacuum nothing from outside, only your breathing in the suit.
fn air(a: &universe_engine::Audio, app: &App) {
    let ship = &app.v.ship;
    let piloting = app.mode == Mode::Pilot && !app.paused;
    let (mut air, mut bright, mut gusty, mut breath, mut distant) = (0.0f32, 0.0f32, false, 0.0f32, 0.0f32);
    match app.v.crew.place {
        Place::Seat | Place::Aboard { .. } if piloting && matches!(ship.state, ShipState::Flying) => {
            let (rho, body) = air_at(app, app.view.ship_pos);
            if let Some(i) = body.filter(|_| rho > 0.0) {
                let b = &app.view.system.bodies[i];
                let ground = app.view.system.velocity(i, app.now()) + b.angular_velocity().cross(app.view.ship_pos - app.view.positions[i]);
                let v = (ship.velocity - ground).length();
                let q = 0.5 * rho * v * v;
                air = (q / AIR_ROAR).sqrt().min(1.0) as f32;
                bright = (v / 1500.0).min(1.0) as f32;
            }
        }
        Place::Outside { body, position, .. } if piloting => {
            let b = &app.view.system.bodies[body];
            let at = app.view.positions[body] + b.rotation(app.now()) * position;
            let (rho, _) = air_at(app, at);
            if rho > AIR_THIN {
                let thick = (rho / 1.2).sqrt().min(1.0);
                air = 0.3 * thick as f32;
                bright = 0.2;
                gusty = true;
                // Ships firing near by: their thrust over the distance squared.
                let loud: f64 = app
                    .v
                    .crafts
                    .iter()
                    .filter(|c| c.system == app.v.ship_system)
                    .map(|c| {
                        let push: f64 = c.ship.spec().thrusters.iter().zip(&c.ship.jets).map(|(t, &u)| t.thrust * u).sum();
                        push / c.ship.position.distance_squared(at).max(100.0)
                    })
                    .sum();
                distant = ((loud / 5.0).sqrt().min(1.0) * thick) as f32;
            } else {
                breath = 0.6;
            }
        }
        _ => {}
    }
    a.set_air(air, bright, gusty);
    a.set_breath(breath);
    a.set_distant(distant, 0.0);
}

/// Walking: where the last step fell, how far since, which foot is next,
/// and whether off the ground (and how fast it was falling).
static STEPS: Mutex<Option<Steps>> = Mutex::new(None);
/// (Where the last step fell, how far since, which foot next, the fastest fall while aloft.)
type Steps = (DVec3, f64, bool, Option<f64>);
/// A step every this many metres, walking; running, a longer stride.
const STRIDE: f64 = 0.9;
const STRIDE_RUNNING: f64 = 2.6;
/// Faster than this (m/s), running.
const RUNNING: f64 = 2.8;

/// A footstep: a soft, muffled thump and a scuff; aboard, a light tap of
/// the deck plating. Each foot a little to its side, a little different.
fn footstep(a: &universe_engine::Audio, inside: bool, left: bool, running: bool) {
    let pan = if left { -0.15 } else { 0.15 };
    // (Running: a lighter step, further apart.)
    let k = if running { 0.75 } else { 1.0 };
    let pitch = if left { 1.0 } else { 1.06 };
    a.thud(85.0 * pitch, 0.2 * k, pan);
    a.hiss(0.07, 0.07 * k, 0.15, pan);
    if inside {
        a.thud(260.0 * pitch, 0.07 * k, pan);
    }
}

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
    alarms(a, app);
    air(a, app);
    // The score: tense in a fight (armed, a missile after us, the hull hurt).
    let inbound = app.v.missiles.iter().any(|m| m.4 && m.0 == app.v.ship_system);
    let tense = ship.armed || inbound || ship.hull < HULL_CAUTION;
    a.set_music(if app.music_off || app.paused { 0.0 } else { MUSIC_LEVEL }, if tense { 1.0 } else { 0.0 });
    // On foot: a step every stride; aboard, boots on the deck plating;
    // outside, softer on the ground.
    // (Outside: how high off the ground, and how fast going up.)
    let (at, air) = match app.v.crew.place {
        Place::Aboard { position, .. } => (Some((position, true)), None),
        Place::Outside { body, position, velocity, .. } => {
            let up = position.normalize_or_zero();
            let height = position.length() - app.view.system.bodies[body].surface_radius(up);
            (Some((position, false)), Some((height, velocity.dot(up))))
        }
        Place::Seat => (None, None),
    };
    let Ok(mut steps) = STEPS.lock() else { return };
    match at {
        Some((p, inside)) if piloting => {
            let (last, walked, left, aloft) = steps.get_or_insert((p, 0.0, false, None));
            let moved = p.distance(*last);
            *last = p;
            // Off the ground: no steps. Pushing off, the jump's; coming down, the landing's.
            let airborne = air.is_some_and(|(height, _)| height > 0.1);
            match (aloft.is_some(), airborne) {
                (false, true) => {
                    a.hiss(0.08, 0.06, 0.2, 0.0);
                    a.thud(110.0, 0.12, 0.0);
                    *aloft = Some(0.0);
                }
                (true, true) => {
                    let falling = air.map_or(0.0, |(_, v)| -v).max(0.0);
                    *aloft = Some(aloft.unwrap_or(0.0).max(falling));
                }
                (true, false) => {
                    // (Harder the faster it came down.)
                    let hit = (aloft.unwrap_or(0.0) as f32 / 4.0).clamp(0.3, 1.0);
                    a.thud(75.0, 0.3 * hit, 0.0);
                    a.hiss(0.1, 0.08 * hit, 0.2, 0.0);
                    *aloft = None;
                    *walked = 0.0;
                }
                (false, false) => {}
            }
            if !airborne {
                // (A jump in place, a respawn: start over.)
                *walked = if moved > 5.0 { 0.0 } else { *walked + moved };
                let running = moved / (ctx.dt as f64).max(1e-3) > RUNNING;
                let stride = if running { STRIDE_RUNNING } else { STRIDE };
                if *walked >= stride {
                    *walked = 0.0;
                    *left = !*left;
                    footstep(a, inside, *left, running);
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
            ("vend drink", Box::new(|a, t| if t == 0.0 { vend(a, true) }), 4.5),
            ("vend snack", Box::new(|a, t| if t == 0.0 { vend(a, false) }), 4.0),
            ("klaxon", Box::new(|a, t| if t == 0.0 {
                a.alarm(500.0, 1000.0, 0.5, 0.2);
                a.alarm_after(0.6, 500.0, 1000.0, 0.5, 0.2);
            }), 1.4),
            ("low fuel", Box::new(|a, t| if t == 0.0 {
                for k in 0..3 {
                    a.alarm_after(0.2 * k as f32, 1200.0, 1200.0, 0.09, 0.14);
                }
            }), 0.8),
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
