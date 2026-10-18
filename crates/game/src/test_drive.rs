//! Studio isolated World flight and a separate static allocator analysis stand.
//! Pilot input, manual keys, rate control and actuator allocation are shared with
//! normal flight. The saved design and live flight both use -Z forward and +X right.

use universe_engine::glam::{DMat3, DQuat, DVec3, Vec2, Vec3};
use universe_engine::{Color, Context, Frame, KeyCode, MouseButton};
use universe_sim::{
    ShipEvent, ShipState,
    world::{
        flight_control,
        ship::{Thruster, ThrusterRole},
    },
};

mod local_flight;
mod visuals;
use local_flight::LocalFlight;

/// What pushes: where it is and the way it pushes (the craft's frame, from its
/// middle of mass), its most thrust (N), and what it is.
#[derive(Clone)]
pub struct Actuator {
    pub at: DVec3,
    pub dir: DVec3,
    pub thrust: f64,
    pub kind: Kind,
}

#[derive(Clone, Copy, PartialEq)]
pub enum Kind {
    /// A ducted fan: its draw at full thrust (W), the least air it works in (kg/m3).
    Fan { full_power: f64, min_density: f64 },
    /// A rocket: its exhaust speed (m/s).
    Rocket { exhaust: f64 },
}

/// A leg: its foot (the craft's frame), what it holds (N) over its stroke (m),
/// and the sink speed it's rated for (m/s).
#[derive(Clone)]
pub struct Leg {
    pub foot: DVec3,
    pub sink: f64,
}

/// The design as a craft to fly (the craft's frame: its middle of mass at the
/// origin, +y up, -z forward).
#[derive(Clone, Default)]
pub struct Craft {
    /// Faithfully compiled runtime hardware; absent means DRIVE is unavailable.
    pub spec: Option<std::sync::Arc<universe_sim::ship::ClassSpec>>,
    pub runtime_errors: Vec<String>,
    /// Exact per-record shared devices compiled with the same thruster ordering.
    pub devices: Option<universe_sim::world::ship_devices::Fit>,
    pub name: String,
    pub native: Option<(String,DVec3)>,
    pub equipment: Vec<crate::equipment_visual::Placed>,
    pub structure: Vec<crate::equipment_visual::Placed>,
    /// Design coordinates to centre on the physical COM exactly once.
    pub visual_origin: Vec3,
    /// Its mass, full (kg), and its inertia about its middle (kg m2).
    pub mass: f64,
    pub inertia: DMat3,
    pub actuators: Vec<Actuator>,
    pub legs: Vec<Leg>,
    /// Registry power plant output and steady equipment demand (W).
    pub plants: f64,
    pub steady: f64,
    /// Presentation envelopes; collision uses spec.shape().
    pub boxes: Vec<(Vec3, Vec3)>,
    /// Fitted flight computer pitch/yaw/roll limits (rad/s); absent means manual only.
    pub rates: Option<DVec3>,
    /// The gravity it's flown in (m/s2); the area it drags by (m2).
    pub gravity: f64,
    pub area: f64,
}

/// Presentation markers in the laboratory's tangent frame (metres).
const PAD_AHEAD: f64 = 150.0;
const PAD_ACROSS: f64 = 20.0;

/// The flight owns its World, Ship, clock and scene. Remaining values are HUD
/// snapshots read from that Ship, never a second simulation state.
pub struct Drive {
    pub craft: Craft,
    flight: Option<LocalFlight>,
    scene: std::cell::RefCell<universe_engine::scene::Scene>,
    pos: DVec3,
    rot: DQuat,
    energy: f64,
    skin_temp: f64,
    propellant: f64,
    mass: f64,
    throttle: f64,
    /// The pilot's helm: the game's flying rules (see `universe_avionics::helm`).
    helm: universe_sim::avionics::helm::Helm,
    air: usize,
    time: f64,
    throttles: Vec<f64>,
    grabbed: bool,
    mouse_pending: Vec2,
    hardest: f64,
    pub events: Vec<String>,
    stopped: bool,
    cam_yaw: f32,
    cam_pitch: f32,
    cam_dist: f32,
}

impl Drive {
    pub fn new(craft: Craft) -> Self {
        Self::in_air(craft, 0)
    }

    fn in_air(craft: Craft, air: usize) -> Self {
        let size = craft
            .boxes
            .iter()
            .map(|b| b.1.distance(b.0))
            .fold(4.0f32, f32::max);
        let mut events = craft.runtime_errors.clone();
        let flight = if events.is_empty() {
            match craft
                .spec
                .clone()
                .ok_or_else(|| "NO COMPILED LIVE SHIP: RETURN TO DESIGN".to_string())
                .and_then(|s| {
                    LocalFlight::new(
                        s,
                        craft.devices.clone(),
                        craft.gravity,
                        air,
                        craft.rates.is_none(),
                    )
                }) {
                Ok(f) => Some(f),
                Err(e) => {
                    events.push(e);
                    None
                }
            }
        } else {
            None
        };
        // (Dev: UNIVERSE_DRIVE_DROP=<m> starts it that high, falling, to watch it land.)
        let mut flight = flight;
        if let (Some(f), Some(h)) = (&mut flight, crate::devenv::num("UNIVERSE_DRIVE_DROP")) {
            f.ship.state = universe_sim::ShipState::Flying;
            f.ship.position += f.ship.position.normalize() * h;
            if let Some(tilt) = crate::devenv::num("UNIVERSE_DRIVE_TILT") {
                f.ship.orientation = universe_engine::glam::DQuat::from_rotation_z(tilt.to_radians());
            }
        }
        if let Some(f) = &flight {
            if !craft.legs.is_empty()
                && f.ship
                    .devices
                    .as_ref()
                    .is_none_or(|d| d.fit.legs.is_empty())
                && universe_sim::world::legs::of_spec(f.ship.spec()).is_none()
            {
                events.push("NO RECORD OF WHAT THE LEGS HOLD: EACH TAKEN AS 3 G OF ITS SHARE".into());
            } else if craft.legs.is_empty() {
                events.push("NO LANDING GEAR PLACED: IT RESTS ON ITS HULL".into());
            }
        }
        let mut d = Self {
            pos: DVec3::ZERO,
            rot: DQuat::IDENTITY,
            energy: 0.0,
            skin_temp: 0.0,
            propellant: 0.0,
            mass: craft.mass,
            throttle: 0.0,
            helm: Default::default(),
            air,
            time: 0.0,
            throttles: Vec::new(),
            grabbed: false,
            mouse_pending: Vec2::ZERO,
            hardest: 0.0,
            events,
            stopped: false,
            cam_yaw: 0.0,
            cam_pitch: 0.25,
            cam_dist: size * 2.2,
            scene: Default::default(),
            craft,
            flight,
        };
        d.observe();
        d
    }

    pub fn set_throttle(&mut self, c: f64) {
        if let Some(f) = &mut self.flight {
            f.commands.throttle = c.clamp(0.0, 1.0);
            // Developer startup's explicit request to fly with throttle.
            f.commands.power = Some(true);
        }
    }
    pub fn grabbed(&self) -> bool {
        self.grabbed
    }
    pub fn release(&mut self) {
        self.grabbed = false;
        self.mouse_pending = Vec2::ZERO;
    }
    fn say(&mut self, what: String) {
        if !self.events.contains(&what) {
            self.events.push(what);
        }
    }
    fn density(&self) -> f64 {
        self.flight.as_ref().map_or(0.0, LocalFlight::density)
    }

    pub fn input(&mut self, ctx: &Context) {
        let input = &ctx.input;
        if input.pressed(KeyCode::KeyR) || input.pressed(KeyCode::KeyZ) {
            let air = if input.pressed(KeyCode::KeyZ) {
                (self.air + 1) % 4
            } else {
                self.air
            };
            let grabbed = self.grabbed;
            *self = Self::in_air(self.craft.clone(), air);
            self.grabbed = grabbed;
            return;
        }
        let clicked = input.button_pressed(MouseButton::Left) && !self.grabbed;
        if clicked {
            self.grabbed = true;
        }
        if self.grabbed && !clicked {
            self.mouse_pending += input.mouse_delta;
        } else {
            self.mouse_pending = Vec2::ZERO;
        }
        let mouse = (self.grabbed && !clicked).then_some(self.mouse_pending);
        let dt = f64::from(ctx.dt);
        let pilot = crate::flight_input::sample(input, mouse, dt.min(0.05));
        if !self.grabbed && input.button_down(MouseButton::Right) {
            self.cam_yaw -= input.mouse_delta.x * 0.008;
            self.cam_pitch = (self.cam_pitch + input.mouse_delta.y * 0.008).clamp(-0.2, 1.4);
        }
        if input.scroll != 0.0 {
            self.cam_dist = (self.cam_dist * 0.88f32.powf(input.scroll)).clamp(3.0, 400.0);
        }
        if let Some(f) = &mut self.flight {
            // The game's helm, for the pilot's hands; the orders go straight to
            // this sandbox's ship (no autopilot here).
            let hands = universe_sim::avionics::helm::HelmInput {
                turn: pilot.turn,
                translation: pilot.translation,
                throttle_delta: pilot.throttle_delta,
                held: crate::manual::held_for(input, &f.ship.spec().thrusters),
                systems: crate::keys::pressed(input, crate::keys::Act::Systems),
                manual: crate::keys::pressed(input, crate::keys::Act::Manual),
            };
            let out = self.helm.fly(&f.ship, false, &hands);
            for order in &out.orders {
                f.order(order);
            }
            f.commands.turn = Some(out.turn);
            if let Some(line) = out.say
                && !self.events.iter().any(|e| e == line)
            {
                self.events.push(line.into());
            }
            let before = f.world.time;
            let result = f.advance(dt);
            if f.world.time > before {
                self.mouse_pending = Vec2::ZERO;
            }
            if let Err(e) = result {
                self.say(e);
            }
        }
        self.observe();
    }

    fn observe(&mut self) {
        let Some(f) = &mut self.flight else {
            return;
        };
        let s = &f.ship;
        self.pos = s.position - f.origin;
        self.rot = s.orientation;
        self.energy = s.energy;
        self.skin_temp = s.skin_temp;
        self.propellant = s.fuel;
        self.mass = s.mass();
        self.throttle = s.throttle;
        self.time = f.world.time;
        self.throttles.clone_from(&s.jets);
        self.stopped = matches!(s.state, ShipState::Destroyed { .. });
        let events = std::mem::take(&mut f.events);
        for e in events {
            if let ShipEvent::HardLanding { sink, .. } = &e {
                self.hardest = self.hardest.max(*sink);
            }
            self.say(format!("{e:?}"));
        }
    }
}

impl Craft {
    fn thrusters(&self) -> Vec<Thruster> {
        if let Some(spec) = &self.spec {
            return spec.thrusters.clone();
        }
        self.actuators
            .iter()
            .enumerate()
            .map(|(k, a)| {
                let push = a.dir;
                Thruster {
                    nozzle: format!("studio_{k}"),
                    at: a.at,
                    push,
                    thrust: a.thrust,
                    role: if push.z < -0.5 {
                        ThrusterRole::Main
                    } else if push.y > 0.5 {
                        ThrusterRole::Lift
                    } else {
                        ThrusterRole::Rcs
                    },
                    exhaust: match a.kind {
                        Kind::Rocket { exhaust } => exhaust,
                        _ => 0.,
                    },
                    efficiency: 1.,
                }
            })
            .collect()
    }
    /// BALANCE uses the same full-tank, empty-cargo state as a fresh DRIVE.
    /// Uncompiled drafts retain an explicitly labelled estimate for analysis.
    fn balance_body(&self) -> flight_control::Body {
        if let Some(spec) = &self.spec {
            let fuel = spec.fuel_capacity;
            flight_control::Body {
                mass: spec.dry_mass + fuel,
                com: spec.centre_of_mass(fuel, 0.0),
                inertia: spec.inertia(fuel, 0.0),
                spin: DVec3::ZERO,
            }
        } else {
            flight_control::Body {
                mass: self.mass,
                com: DVec3::ZERO,
                inertia: self.inertia,
                spin: DVec3::ZERO,
            }
        }
    }
}

/// Balance uses the same bounded flight allocator, with an explicit stand lift demand.
fn throttles(c: &Craft, collective: f64, want: DVec3) -> Vec<f64> {
    let ts = c.thrusters();
    let body = c.balance_body();
    let mut settings = vec![0.; ts.len()];
    let lift = ts.iter().map(|t| t.push.y.max(0.) * t.thrust).sum::<f64>();
    flight_control::step(
        &ts,
        body,
        flight_control::Demand {
            force: DVec3::Y * (collective * lift),
            want_rates: Some(want),
            drive: Some(0.),
            enabled: true,
        },
        1. / 60.,
        &mut settings,
    );
    settings
}

/// The push and turning of throttles (the craft's frame, about its middle of mass).
fn push_and_turn(c: &Craft, throttles: &[f64]) -> (DVec3, DVec3) {
    let com = c.balance_body().com;
    c.thrusters()
        .iter()
        .zip(throttles)
        .fold((DVec3::ZERO, DVec3::ZERO), |(f, t), (a, &th)| {
            let push = a.push * a.thrust * th;
            (f + push, t + (a.at - com).cross(push))
        })
}

/// Rated static lift balance, without atmospheric or electrical derating.
fn hover_collective(c: &Craft) -> Option<f64> {
    let lift: f64 = c
        .thrusters()
        .iter()
        .map(|a| a.thrust * a.push.y.max(0.0))
        .sum();
    (lift > 0.0).then(|| c.balance_body().mass * c.gravity / lift)
}

/// The balance room: the craft on a stand, its middle of mass and its pushes shown,
/// thrust tried all together, one at a time, or turned about each axis; the trim
/// that holds it level, and how much of its turning that takes.
pub struct Balance {
    scene: std::cell::RefCell<universe_engine::scene::Scene>,
    pub craft: Craft,
    collective: f64,
    command: DVec3,
    single: Option<usize>,
    trimmed: bool,
    cam_yaw: f32,
    cam_pitch: f32,
    cam_dist: f32,
}

/// What the balance room's keys ask: stay, fly it, or go back to the studio.
pub enum Asked {
    Stay,
    Fly,
    Back,
}

impl Balance {
    pub fn new(craft: Craft) -> Self {
        let size = craft
            .boxes
            .iter()
            .map(|b| b.1.distance(b.0))
            .fold(4.0f32, f32::max);
        Balance {
            scene: Default::default(),
            collective: hover_collective(&craft).unwrap_or(0.0).min(1.0),
            command: DVec3::ZERO,
            single: None,
            trimmed: false,
            cam_yaw: 0.7,
            cam_pitch: 0.3,
            cam_dist: size * 1.8,
            craft,
        }
    }

    /// Trimmed or not (dev).
    pub fn set_trim(&mut self, on: bool) {
        self.trimmed = on;
    }

    pub fn input(&mut self, ctx: &Context) -> Asked {
        let input = &ctx.input;
        let key = |k: KeyCode| if input.down(k) { 1.0 } else { 0.0 };
        if input.pressed(KeyCode::Escape) {
            return Asked::Back;
        }
        if input.pressed(KeyCode::Enter) || input.pressed(KeyCode::NumpadEnter) {
            return Asked::Fly;
        }
        let dt = f64::from(ctx.dt).min(0.05);
        self.collective = (self.collective + (key(KeyCode::KeyW) - key(KeyCode::KeyS)) * 0.3 * dt)
            .clamp(0.0, 1.0);
        if input.pressed(KeyCode::KeyH) {
            self.collective = hover_collective(&self.craft).unwrap_or(0.0).min(1.0);
        }
        let turn = crate::flight_input::sample(input, None, dt).turn;
        self.command = flight_control::rates(
            DVec3::new(turn.pitch, turn.yaw, turn.roll),
            self.craft.spec.as_ref().map_or_else(
                || self.craft.rates.unwrap_or(DVec3::ZERO),
                |s| DVec3::new(s.turn_rate, s.turn_rate, s.roll_rate),
            ),
        );
        if input.pressed(KeyCode::Tab) {
            let n = self.craft.thrusters().len();
            self.single = match self.single {
                None if n > 0 => Some(0),
                Some(k) if k + 1 < n => Some(k + 1),
                _ => None,
            };
        }
        if input.pressed(KeyCode::KeyT) {
            self.trimmed = !self.trimmed;
        }
        if input.button_down(MouseButton::Left) || input.button_down(MouseButton::Right) {
            self.cam_yaw -= input.mouse_delta.x * 0.008;
            self.cam_pitch = (self.cam_pitch + input.mouse_delta.y * 0.008).clamp(-1.4, 1.4);
        }
        if input.scroll != 0.0 {
            self.cam_dist = (self.cam_dist * 0.88f32.powf(input.scroll)).clamp(2.0, 400.0);
        }
        Asked::Stay
    }

    /// The throttles now: one actuator alone at the collective, or all mixed from the
    /// collective, the keys and (T) the trim.
    fn now(&self) -> Vec<f64> {
        match self.single {
            Some(k) => (0..self.craft.thrusters().len())
                .map(|i| {
                    if i == k {
                        self.collective.max(0.5)
                    } else {
                        0.0
                    }
                })
                .collect(),
            None if self.trimmed || self.command != DVec3::ZERO => {
                throttles(&self.craft, self.collective, self.command)
            }
            None => self
                .craft
                .thrusters()
                .iter()
                .map(|a| if a.push.y > 0.0 { self.collective } else { 0. })
                .collect(),
        }
    }
}

/// The balance room drawn: the craft as it stands; its middle of mass (and its
/// weight down from it); each push as an arrow its length; the pushes' sum along the
/// line it acts on; and what the turning is and the trim takes.
pub fn draw_balance(frame: &mut Frame, b: &Balance) {
    let _scope = universe_prof::scope("draw/studio/balance");
    let size = frame.size();
    frame.hud_rect(Vec2::ZERO, size, Color([0.03, 0.04, 0.08, 1.0]));
    let c = &b.craft;
    let body = c.balance_body();
    let thrusters = c.thrusters();
    let com = body.com.as_vec3();
    let (sy, cy) = b.cam_yaw.sin_cos();
    let (sp, cp) = b.cam_pitch.sin_cos();
    let eye = Vec3::new(sy * cp, sp, cy * cp) * b.cam_dist;
    let forward = (-eye).normalize();
    let right = forward.cross(Vec3::Y).normalize_or_zero();
    let up = right.cross(forward);
    let focal = size.y * 0.5 / (0.85f32 * 0.5).tan();
    let project = |p: Vec3| {
        let v = p - eye;
        let z = v.dot(forward);
        (z > 0.2).then(|| size * 0.5 + Vec2::new(v.dot(right), -v.dot(up)) * (focal / z))
    };
    let seg = |frame: &mut Frame, a: Vec3, b: Vec3, col: Color| {
        if let (Some(p), Some(q)) = (project(a), project(b)) {
            frame.hud_line(p, q, col);
        }
    };
    // (Arrows: a shaft and a head, `len` metres along `dir` from `at`.)
    let arrow = |frame: &mut Frame, at: Vec3, dir: Vec3, len: f32, col: Color| {
        let tip = at + dir * len;
        seg(frame, at, tip, col);
        let side = dir
            .cross(if dir.y.abs() < 0.9 { Vec3::Y } else { Vec3::X })
            .normalize_or_zero()
            * len.min(1.5)
            * 0.15;
        seg(frame, tip, tip - dir * len.min(1.5) * 0.25 + side, col);
        seg(frame, tip, tip - dir * len.min(1.5) * 0.25 - side, col);
    };
    // (The stand's floor under its lowest point.)
    let floor = c
        .legs
        .iter()
        .map(|l| l.foot.y as f32)
        .chain(c.boxes.iter().map(|b| b.0.y))
        .fold(f32::MAX, f32::min);
    let camera = crate::equipment_visual::Camera {
        eye,
        right,
        up,
        forward,
        focal,
        centre: size * 0.5,
        near: 0.2,
    };
    frame.canvas_scene();
    visuals::draw(
        &b.scene,
        c,
        DVec3::ZERO,
        DQuat::IDENTITY,
        &camera,
        frame,
        visuals::Environment::Balance {
            floor: if floor.is_finite() && floor != f32::MAX {
                f64::from(floor)
            } else {
                0.0
            },
        },
    );
    // (The scale: the strongest push 4 m long.)
    let strongest = thrusters
        .iter()
        .map(|a| a.thrust)
        .fold(body.mass * c.gravity, f64::max)
        .max(f64::MIN_POSITIVE);
    let metres = |n: f64| (n / strongest * 4.0) as f32;
    let ts = b.now();
    let (force, turn) = push_and_turn(c, &ts);
    for (k, (a, &t)) in thrusters.iter().zip(&ts).enumerate() {
        let at = a.at.as_vec3();
        let col = if b.single == Some(k) {
            Color([1.0, 1.0, 0.4, 1.0])
        } else {
            Color([0.4, 0.85, 1.0, 0.95])
        };
        if t > 0.001 {
            arrow(
                frame,
                at,
                a.push.as_vec3(),
                metres(a.thrust * t).max(0.3),
                col,
            );
        }
        if let Some(q) = project(at) {
            frame.text_scaled(
                q + Vec2::new(6.0, 4.0),
                &format!("{:.0}%", t * 100.0),
                col,
                0.55,
            );
        }
    }
    // (The middle of mass: a cross and a ring; its weight straight down.)
    if let Some(q) = project(com) {
        frame.hud_line(
            q - Vec2::new(10.0, 0.0),
            q + Vec2::new(10.0, 0.0),
            Color([1.0, 0.85, 0.3, 1.0]),
        );
        frame.hud_line(
            q - Vec2::new(0.0, 10.0),
            q + Vec2::new(0.0, 10.0),
            Color([1.0, 0.85, 0.3, 1.0]),
        );
        frame.hud_box(
            q - Vec2::splat(6.0),
            Vec2::splat(12.0),
            Color([1.0, 0.85, 0.3, 1.0]),
        );
        frame.text_scaled(
            q + Vec2::new(12.0, -14.0),
            "MIDDLE OF MASS",
            Color([1.0, 0.85, 0.3, 1.0]),
            0.6,
        );
    }
    arrow(
        frame,
        com,
        Vec3::NEG_Y,
        metres(body.mass * c.gravity),
        Color([1.0, 0.85, 0.3, 0.8]),
    );
    // (The pushes' sum, along the line it acts on: through the point of that line
    // nearest the middle (F x M / F^2).)
    let f2 = force.length_squared();
    let through = if f2 > 1e-6 {
        force.cross(turn) / f2
    } else {
        DVec3::ZERO
    };
    if f2 > 1e-6 {
        let dir = force.normalize().as_vec3();
        let p = com + through.as_vec3();
        seg(frame, p - dir * 3.0, p, Color([1.0, 0.4, 0.9, 0.6]));
        arrow(
            frame,
            p,
            dir,
            metres(force.length()),
            Color([1.0, 0.4, 0.9, 1.0]),
        );
        if through.length() > 0.02 {
            seg(frame, com, p, Color([1.0, 0.4, 0.9, 0.8]));
        }
    }
    // (What it reads.)
    let weight = body.mass * c.gravity;
    let hover = hover_collective(c);
    let verdict = match hover {
        None => ("NOTHING LIFTS IT: NO HOVER TO BALANCE".to_string(), false),
        Some(h) if h > 1.0 => (
            format!("TOO HEAVY TO HOVER: IT NEEDS {:.0}% OF ITS LIFT", h * 100.0),
            false,
        ),
        Some(h) => {
            let (f, t) = push_and_turn(c, &throttles(c, h, DVec3::ZERO));
            (
                format!(
                    "SHARED HOVER ALLOCATION: LIFT {:.1} KN / WEIGHT {:.1} KN; RESIDUAL TURN {:.2} KN M",
                    f.y / 1000.0,
                    weight / 1000.0,
                    t.length() / 1000.0
                ),
                false,
            )
        }
    };
    let lines = [
        format!(
            "BALANCE ROOM - {} - {}",
            c.name.to_uppercase(),
            if c.spec.is_some() {
                "COMPILED / FULL TANK"
            } else {
                "UNCOMPILED ESTIMATE"
            }
        ),
        format!(
            "MASS {:.2} T   WEIGHT {:.1} KN AT {:.2} G",
            body.mass / 1000.0,
            weight / 1000.0,
            c.gravity / universe_sim::world::units::STANDARD_GRAVITY
        ),
        format!(
            "INERTIA XX/YY/ZZ {:.3}/{:.3}/{:.3} T M2   RATED STATIC ANALYSIS (NO AIR/POWER LIMITS)",
            body.inertia.x_axis.x / 1000.0,
            body.inertia.y_axis.y / 1000.0,
            body.inertia.z_axis.z / 1000.0
        ),
        format!(
            "COLLECTIVE {:.0}%{}   RATED HOVER {}",
            b.collective * 100.0,
            b.single
                .map_or(String::new(), |k| format!("   ONE ALONE: NUMBER {}", k + 1)),
            hover.map_or("-".into(), |h| format!("{:.0}%", h * 100.0))
        ),
        format!(
            "PUSH {:.1} KN UP, {:.1} KN ACROSS   ITS LINE PASSES {:.2} M FROM THE MIDDLE ({:+.2} ACROSS, {:+.2} ALONG)",
            force.y / 1000.0,
            (force.x.powi(2) + force.z.powi(2)).sqrt() / 1000.0,
            DVec3::new(through.x, 0.0, through.z).length(),
            through.x,
            through.z
        ),
        format!(
            "TURNING: PITCH {:+.1} KN M (NOSE {}), ROLL {:+.1} KN M, YAW {:+.1} KN M",
            -turn.x / 1000.0,
            if turn.x > 0.0 { "DOWN" } else { "UP" },
            turn.z / 1000.0,
            turn.y / 1000.0
        ),
        format!(
            "SHARED ALLOCATION {} (T): RATE DEMAND PITCH {:+.2}, ROLL {:+.2}, YAW {:+.2} RAD/S",
            if b.trimmed { "ON" } else { "OFF" },
            b.command.x,
            b.command.z,
            b.command.y
        ),
    ];
    frame.hud_rect(
        Vec2::new(10.0, 10.0),
        Vec2::new(620.0, 30.0 + lines.len() as f32 * 12.0),
        Color([0.02, 0.06, 0.13, 0.88]),
    );
    for (k, l) in lines.iter().enumerate() {
        frame.text_scaled(
            Vec2::new(16.0, 14.0 + k as f32 * 12.0),
            l,
            if k == 0 {
                Color([1.0, 0.85, 0.4, 1.0])
            } else {
                Color([0.5, 1.0, 0.6, 0.95])
            },
            0.62,
        );
    }
    frame.text_scaled(
        Vec2::new(16.0, 16.0 + lines.len() as f32 * 12.0),
        &verdict.0,
        if verdict.1 {
            Color([0.4, 1.0, 0.5, 1.0])
        } else {
            Color([1.0, 0.6, 0.3, 1.0])
        },
        0.66,
    );
    frame.text_scaled(Vec2::new(12.0, size.y - 18.0), "W / S COLLECTIVE - H TO HOVER - ARROWS PITCH AND ROLL, Q / E YAW - TAB ONE AT A TIME - T TRIM - DRAG TO LOOK - ENTER FLY IT - ESC BACK", Color([0.75, 0.88, 1.0, 0.7]), 0.6);
}

/// The flight drawn: the ground's grid round it, the craft's boxes and members as
/// it lies, each actuator's push as a line the length of its throttle; and what it
/// reads.
pub fn draw(frame: &mut Frame, d: &Drive) {
    let _scope = universe_prof::scope("draw/studio/drive");
    let size = frame.size();
    let sky = if d.density() > 0.05 {
        Color([0.05, 0.09, 0.16, 1.0])
    } else {
        Color([0.01, 0.01, 0.03, 1.0])
    };
    frame.hud_rect(Vec2::ZERO, size, sky);
    let target = d.pos.as_vec3();
    // (Behind it, as the game's chase view: the camera round to the back of the
    // craft's heading, plus what the drag has turned it.)
    let nose = d.rot * DVec3::NEG_Z;
    let behind = (-nose.x).atan2(-nose.z) as f32;
    let (sy, cy) = (behind + d.cam_yaw).sin_cos();
    let (sp, cp) = d.cam_pitch.sin_cos();
    let eye = target + Vec3::new(sy * cp, sp, cy * cp) * d.cam_dist;
    let forward = (target - eye).normalize();
    let right = forward.cross(Vec3::Y).normalize_or_zero();
    let up = right.cross(forward);
    let focal = size.y * 0.5 / (0.85f32 * 0.5).tan();
    let project = |p: Vec3| {
        let v = p - eye;
        let z = v.dot(forward);
        (z > 0.2).then(|| size * 0.5 + Vec2::new(v.dot(right), -v.dot(up)) * (focal / z))
    };
    let seg = |frame: &mut Frame, a: Vec3, b: Vec3, c: Color| {
        if let (Some(p), Some(q)) = (project(a), project(b)) {
            frame.hud_line(p, q, c);
        }
    };
    let place = |p: Vec3| (d.pos + d.rot * p.as_dvec3()).as_vec3();
    let camera = crate::equipment_visual::Camera {
        eye,
        right,
        up,
        forward,
        focal,
        centre: size * 0.5,
        near: 0.2,
    };
    frame.canvas_scene();
    visuals::draw(
        &d.scene,
        &d.craft,
        d.pos,
        d.rot,
        &camera,
        frame,
        visuals::Environment::Flight {
            position: d.pos,
            time: d.time,
            sky,
        },
    );
    // (Each push: a line the other way from where it pushes, its throttle long.)
    // Display the same manual key assignment as normal flight.
    let live_thrusters = d.flight.as_ref().map(|f| &f.ship.spec().thrusters);
    let manual_labels = crate::manual::labels_for(live_thrusters.map_or(&[][..], |t| t.as_slice()));
    for (k, (a, &t)) in live_thrusters
        .into_iter()
        .flatten()
        .zip(&d.throttles)
        .enumerate()
    {
        let from = place(a.at.as_vec3());
        if t > 0.01 {
            let to = place((a.at - a.push * (0.5 + 3.0 * t)).as_vec3());
            seg(frame, from, to, Color([0.4, 0.8, 1.0, 0.9]));
        }
        if let Some(q) = project(from) {
            let label = manual_labels.get(k).map_or("", String::as_str);
            let text = format!("{} [{}] OUTPUT {:.0}%", k + 1, label, t * 100.0);
            let col = if t > 0.01 {
                Color([0.6, 0.95, 1.0, 1.0])
            } else {
                Color([0.6, 0.7, 0.8, 0.6])
            };
            frame.hud_rect(
                q + Vec2::new(6.0, -2.0),
                Vec2::new(text.len() as f32 * 5.6 + 6.0, 11.0),
                Color([0.0, 0.02, 0.06, 0.75]),
            );
            frame.text_scaled(q + Vec2::new(9.0, 0.0), &text, col, 0.55);
        }
    }
    // (What it reads.)
    let up_now = d.rot * DVec3::Y;
    let nose = d.rot * DVec3::NEG_Z;
    let pitch = nose.y.asin().to_degrees();
    let roll = (d.rot * DVec3::X).y.asin().to_degrees();
    let heading = nose.x.atan2(-nose.z).to_degrees();
    let c = &d.craft;
    let devices = d.flight.as_ref().and_then(|f| f.ship.devices.as_ref());
    let bus = devices.map_or_else(
        || "NO ADDITIONAL ELECTRICAL DEVICES".to_string(),
        |s| {
            let energy: f64 = s.packs.iter().map(|p| p.energy).sum();
            let hottest = s.packs.iter().map(|p| p.temperature).reduce(f64::max);
            let temperature =
                hottest.map_or_else(|| "NO PACKS".into(), |t| format!("HOTTEST {t:.1} K"));
            format!(
                "BUS {:.2}/{:.2} MW DELIVERED/ASKED   PACKS {:.2} MJ   {temperature}",
                s.delivered_w / 1e6,
                s.demand_w / 1e6,
                energy / 1e6
            )
        },
    );
    let height = d.flight.as_ref().map_or(0.0, |f| {
        d.craft
            .legs
            .iter()
            .map(|leg| f.altitude(leg.foot))
            .fold(f.altitude(DVec3::ZERO), f64::min)
            .max(0.0)
    });
    let climb = d.flight.as_ref().map_or(0.0, LocalFlight::climb);
    let ground_speed = d.flight.as_ref().map_or(0.0, LocalFlight::ground_speed);
    let lines = [
        format!("TEST DRIVE - {}   {:.1} S", c.name.to_uppercase(), d.time),
        format!(
            "ALTITUDE {height:.1} M OVER THE GROUND   CLIMB {climb:+.1} M/S   SPEED {ground_speed:.1} M/S"
        ),
        format!(
            "PITCH {pitch:+.0}   ROLL {roll:+.0}   HEADING {heading:.0}   {}",
            if up_now.y < 0.0 { "UPSIDE DOWN" } else { "" }
        ),
        control_status(d),
        flight_keys(d),
        format!(
            "LIVE CAPACITOR {:.3} MJ   SKIN {:.1} K   FUEL {:.0} KG   MASS {:.2} T",
            d.energy / 1e6,
            d.skin_temp,
            d.propellant,
            d.mass / 1000.0
        ),
        format!(
            "AIR {:.3} KG/M3   GRAVITY {:.2} M/S2   HARD LANDING PEAK {:.1} M/S",
            d.density(),
            c.gravity,
            d.hardest
        ),
        format!(
            "SHARED WORLD / UNLIT SMOOTH SPHERE   CLOCK BACKLOG {:.3} S",
            d.flight.as_ref().map_or(0.0, LocalFlight::pending)
        ),
        bus,
        ground_line(d),
        profile_line(d),
    ];
    let panel = Color([0.02, 0.06, 0.13, 0.85]);
    frame.hud_rect(
        Vec2::new(10.0, 10.0),
        Vec2::new(560.0, 14.0 + lines.len() as f32 * 12.0),
        panel,
    );
    for (k, l) in lines.iter().enumerate() {
        frame.text_scaled(
            Vec2::new(16.0, 14.0 + k as f32 * 12.0),
            l,
            if k == 0 {
                Color([1.0, 0.85, 0.4, 1.0])
            } else {
                Color([0.5, 1.0, 0.6, 0.95])
            },
            0.62,
        );
    }
    for (k, e) in d.events.iter().enumerate() {
        frame.text_scaled(
            Vec2::new(16.0, 30.0 + (lines.len() + k) as f32 * 12.0),
            e,
            Color([1.0, 0.45, 0.35, 1.0]),
            0.62,
        );
    }
    // (The tapes: altitude over the ground (the lowest foot's height) on the right,
    // with the climb beside it; speed over the ground on the left; each reading
    // large in its box at the middle.)
    let tape = |frame: &mut Frame,
                x: f32,
                value: f64,
                per: f64,
                step: f64,
                label: &str,
                unit: &str,
                left: bool| {
        let (h, w) = (300.0f32, 64.0f32);
        let top = size.y * 0.5 - h * 0.5;
        frame.hud_rect(
            Vec2::new(x, top),
            Vec2::new(w, h),
            Color([0.02, 0.05, 0.1, 0.55]),
        );
        frame.hud_box(
            Vec2::new(x, top),
            Vec2::new(w, h),
            Color([0.5, 0.8, 1.0, 0.5]),
        );
        // (Ticks every step, labelled every other; the value scrolls under the middle.)
        let px = f64::from(h) / (per * 2.0);
        let first = ((value - per) / step).ceil() as i64;
        let last = ((value + per) / step).floor() as i64;
        for n in first..=last {
            let v = n as f64 * step;
            if v < 0.0 {
                continue;
            }
            let y = size.y * 0.5 - ((v - value) * px) as f32;
            let major = n % 2 == 0;
            let len = if major { 12.0 } else { 6.0 };
            let (a, b) = if left {
                (x + w - len, x + w)
            } else {
                (x, x + len)
            };
            frame.hud_line(
                Vec2::new(a, y),
                Vec2::new(b, y),
                Color([0.7, 0.9, 1.0, 0.8]),
            );
            if major {
                let tx = if left { x + 6.0 } else { x + 16.0 };
                frame.text_scaled(
                    Vec2::new(tx, y - 4.0),
                    &format!("{v:.0}"),
                    Color([0.7, 0.9, 1.0, 0.8]),
                    0.55,
                );
            }
        }
        let bx = Vec2::new(x - 6.0, size.y * 0.5 - 11.0);
        frame.hud_rect(bx, Vec2::new(w + 12.0, 22.0), Color([0.0, 0.0, 0.0, 0.9]));
        frame.hud_box(bx, Vec2::new(w + 12.0, 22.0), Color([1.0, 0.85, 0.4, 1.0]));
        frame.text_scaled(
            bx + Vec2::new(4.0, 4.0),
            &format!("{value:.1}"),
            Color([1.0, 0.85, 0.4, 1.0]),
            0.95,
        );
        frame.text_scaled(
            Vec2::new(x, top - 14.0),
            label,
            Color([0.75, 0.88, 1.0, 0.85]),
            0.6,
        );
        frame.text_scaled(
            Vec2::new(x, top + h + 4.0),
            unit,
            Color([0.75, 0.88, 1.0, 0.7]),
            0.55,
        );
    };
    let alt_x = size.x - 120.0;
    tape(
        frame,
        alt_x,
        height,
        25.0,
        5.0,
        "ALTITUDE",
        "M OVER THE GROUND",
        false,
    );
    tape(
        frame,
        40.0,
        ground_speed,
        25.0,
        5.0,
        "SPEED",
        "M/S OVER THE GROUND",
        true,
    );
    // (The climb: a bar from the middle, up green, down amber (red past the legs'
    // rated sink), to 10 m/s each way.)
    {
        let (h, x) = (300.0f32, alt_x + 76.0);
        let mid = size.y * 0.5;
        frame.hud_rect(
            Vec2::new(x, mid - h * 0.5),
            Vec2::new(14.0, h),
            Color([0.02, 0.05, 0.1, 0.55]),
        );
        frame.hud_box(
            Vec2::new(x, mid - h * 0.5),
            Vec2::new(14.0, h),
            Color([0.5, 0.8, 1.0, 0.5]),
        );
        let climb = climb.clamp(-10.0, 10.0);
        let len = (climb / 10.0) as f32 * h * 0.5;
        let rated = d
            .craft
            .legs
            .iter()
            .map(|l| l.sink)
            .reduce(f64::min)
            .unwrap_or(3.0);
        let c = if climb >= 0.0 {
            Color([0.4, 1.0, 0.5, 0.9])
        } else if -climb > rated {
            Color([1.0, 0.35, 0.3, 0.95])
        } else {
            Color([1.0, 0.75, 0.3, 0.9])
        };
        let (top, hgt) = if len >= 0.0 {
            (mid - len, len)
        } else {
            (mid, -len)
        };
        frame.hud_rect(Vec2::new(x + 2.0, top), Vec2::new(10.0, hgt.max(1.0)), c);
        // (The legs' rated sink marked.)
        let mark = mid + (rated / 10.0) as f32 * h * 0.5;
        frame.hud_line(
            Vec2::new(x - 3.0, mark),
            Vec2::new(x + 17.0, mark),
            Color([1.0, 0.35, 0.3, 0.8]),
        );
        frame.hud_line(
            Vec2::new(x - 3.0, mid),
            Vec2::new(x + 17.0, mid),
            Color([0.7, 0.9, 1.0, 0.8]),
        );
        frame.text_scaled(
            Vec2::new(x - 12.0, mid - h * 0.5 - 14.0),
            "CLIMB",
            Color([0.75, 0.88, 1.0, 0.85]),
            0.6,
        );
        frame.text_scaled(
            Vec2::new(x - 24.0, mid + h * 0.5 + 18.0),
            &format!("{climb:+.1} M/S"),
            c,
            0.7,
        );
    }
    // (The pad: how far, which way (from the nose, right +), how high over it.)
    let to_pad = DVec3::new(-d.pos.x, 0.0, -PAD_AHEAD - d.pos.z);
    let bearing = {
        let n = DVec3::new(nose.x, 0.0, nose.z);
        let a = to_pad.x.atan2(-to_pad.z) - n.x.atan2(-n.z);
        (a.to_degrees() + 540.0) % 360.0 - 180.0
    };
    frame.text_scaled(
        Vec2::new(
            16.0,
            18.0 + (lines.len() + d.events.len() + 1) as f32 * 12.0,
        ),
        &format!(
            "PAD {:.0} M, {:+.0} DEG FROM THE NOSE, {:.1} M OVER IT   BEACON {:.0} M",
            to_pad.length(),
            bearing,
            height,
            DVec3::new(d.pos.x, 0.0, d.pos.z).length()
        ),
        Color([1.0, 0.85, 0.35, 0.95]),
        0.62,
    );
    // (The game's own radar, on a landing's scale: 10 m in the middle to 10 km at
    // the rim; both use ahead -Z, right +X.)
    let scope = crate::hud::Scope::draw(
        frame,
        Vec2::new((size.x / 2.0).floor(), size.y - 74.0),
        10.0,
        3.0,
    );
    let to_scanner = |world: DVec3| {
        let v = d.rot.inverse() * (world - d.pos);
        v
    };
    scope.body(
        frame,
        to_scanner(DVec3::new(0.0, 0.0, -PAD_AHEAD)),
        Color([1.0, 0.85, 0.35, 1.0]),
        2.5,
    );
    scope.craft(
        frame,
        to_scanner(DVec3::new(0.0, 6.0, 0.0)),
        Color([1.0, 0.35, 0.3, 1.0]),
        false,
    );
    frame.text_scaled(
        Vec2::new(12.0, size.y - 18.0),
        &control_help(d),
        Color([0.75, 0.88, 1.0, 0.7]),
        0.6,
    );
}

// The instructions must describe the fitted controller, including ships without one.
/// What touches the ground: asleep or awake, its feet and hull points, each
/// leg pushed in (m) and the ones that gave way.
fn ground_line(d: &Drive) -> String {
    let Some(f) = &d.flight else { return String::new() };
    let ship = &f.ship;
    let asleep = matches!(ship.state, universe_sim::ShipState::Landed { .. });
    let Some(g) = &ship.ground else {
        return (if asleep { "ON THE GROUND, AT REST" } else { "OFF THE GROUND" }).into();
    };
    let c = &g.last;
    let legs: Vec<String> = c
        .compression
        .iter()
        .enumerate()
        .map(|(k, x)| if k < 64 && ship.broken_legs & (1 << k) != 0 { "GONE".into() } else { format!("{x:.2}") })
        .collect();
    format!(
        "{}   FEET {}   HULL POINTS {}   LEGS IN {} M",
        if asleep { "ON THE GROUND, AT REST" } else if c.touching > 0 { "ON THE GROUND" } else { "OFF THE GROUND" },
        c.feet,
        c.touching - c.feet,
        if legs.is_empty() { "-".into() } else { legs.join("/") }
    )
}

/// What its ground profile says before it lands (worked out once for the
/// design): the tilt it rests at, the slopes it stands and slides on, the
/// hardest landing its legs take here.
fn profile_line(d: &Drive) -> String {
    let Some(f) = &d.flight else { return String::new() };
    let ship = &f.ship;
    let p = universe_sim::world::ground::profile_of(ship);
    let com = ship.centre_of_mass();
    format!(
        "RESTS {:.0} DEG OFF LEVEL   STANDS TO {:.0} DEG   SLIDES AT {:.0} DEG   LEGS TAKE {:.1} M/S",
        (p.rest(com).tilt * universe_engine::glam::DVec3::Y).angle_between(universe_engine::glam::DVec3::Y).to_degrees(),
        p.tip_slope(com).min(p.slide_slope()).to_degrees(),
        p.slide_slope().to_degrees(),
        p.max_sink(ship.mass(), com, d.craft.gravity, ship.broken_legs)
    )
}

fn control_status(d: &Drive) -> String {
    let Some(f) = &d.flight else {
        return "DRIVE UNAVAILABLE - COMPILER ERRORS BELOW - ESC BACK".into();
    };
    if d.stopped {
        return "CRASHED - R RESTARTS THE TEST DRIVE".into();
    }
    let ship = &f.ship;
    if !ship.powered {
        "FLIGHT SYSTEMS DOWN".into()
    } else if ship.manual && !universe_sim::avionics::helm::has_computer(ship) {
        "NO FLIGHT COMPUTER - MANUAL THRUSTERS ONLY, NOTHING KEEPS IT LEVEL (FIT ONE)".into()
    } else if ship.manual {
        "MANUAL THRUSTERS - FLIGHT COMPUTER OFF".into()
    } else {
        format!("FLIGHT COMPUTER ON - THROTTLE {:.0}%", ship.throttle * 100.0)
    }
}

/// The keys that fly it now: the game's own (`flight_input::keys`).
fn flight_keys(d: &Drive) -> String {
    let Some(f) = &d.flight else {
        return "BALANCE REMAINS AVAILABLE FOR DESIGN ANALYSIS".into();
    };
    crate::flight_input::keys(&f.ship).iter().map(|(k, what)| format!("{k} {what}")).collect::<Vec<_>>().join("   ")
}

/// The test drive's own keys (the sandbox, not the ship).
fn control_help(d: &Drive) -> String {
    if d.stopped {
        return "R RESTART - ESC RELEASE MOUSE / BACK".into();
    }
    "Z AIR/RESET - R RESET - ESC RELEASE MOUSE / BACK".into()
}
