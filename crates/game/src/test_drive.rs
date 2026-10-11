//! TEST DRIVE: a studio design flown as it is, over flat ground at its design
//! gravity: one rigid body (its parts' masses where they sit, its inertia from
//! them), each engine and fan pushing from where it is the way it points, at its
//! record's thrust. Fans take their thrust from the air (thinner with height) and
//! their power from the batteries (thrust to the 1.5), which run down and warm by
//! their losses; rockets burn their propellant; legs are springs, rated by what
//! they hold over their stroke. Flown as the game flies a ship: the flight computer
//! on (the steering assist: the arrows and Q/E say which way to turn and it shares
//! the thrust out among the engines, damping the turning and levelling the craft),
//! or manual thrusters (G, the game's key): no sharing, no steadying, each engine
//! answering its own key (1-9) over the collective.

use universe_engine::glam::{DMat3, DQuat, DVec3, Vec2, Vec3};
use universe_engine::{Color, Context, Frame, KeyCode, MouseButton};

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
    pub holds: f64,
    pub stroke: f64,
    pub sink: f64,
}

/// The design as a craft to fly (the craft's frame: its middle of mass at the
/// origin, +y up, +z forward).
#[derive(Clone, Default)]
pub struct Craft {
    pub name: String,
    pub equipment: Vec<crate::equipment_visual::Placed>,
    /// Design coordinates to centre on the physical COM exactly once.
    pub visual_origin: Vec3,
    /// Its mass, full (kg), and its inertia about its middle (kg m2).
    pub mass: f64,
    pub inertia: DMat3,
    pub actuators: Vec<Actuator>,
    pub legs: Vec<Leg>,
    /// The batteries: what they store (J), the most they give (W), how efficiently
    /// (0..1), their heat capacity (J/K) and how far they may warm (K); what the
    /// power plants give (W); what the rest draws all the while (W).
    pub stored: f64,
    pub rate: f64,
    pub efficiency: f64,
    pub heat_capacity: f64,
    pub warming: f64,
    pub plants: f64,
    pub steady: f64,
    /// The rockets' propellant (kg).
    pub propellant: f64,
    /// Its boxes and members, to draw and to touch the ground with (each box: is
    /// it a leg's, which may).
    pub boxes: Vec<(Vec3, Vec3)>,
    pub leg_boxes: Vec<bool>,
    pub members: Vec<[Vec3; 2]>,
    /// The gravity it's flown in (m/s2); the area it drags by (m2).
    pub gravity: f64,
    pub area: f64,
}

/// Air density at the ground (kg/m3) for each setting Z cycles through: sea level,
/// thin, very thin, none.
const AIRS: [f64; 4] = [1.225, 0.6, 0.3, 0.0];
/// How fast air thins with height: its scale height (m).
const SCALE_HEIGHT: f64 = 8500.0;
/// The practice field: a beacon where it starts, a landing pad PAD_AHEAD metres
/// ahead of it (the way its nose points at the start), PAD_ACROSS across.
const PAD_AHEAD: f64 = 150.0;
const PAD_ACROSS: f64 = 20.0;

/// Drag (the craft's blunt body), friction on the ground.
const DRAG: f64 = 1.0;
const FRICTION: f64 = 0.6;

/// A flight under way.
pub struct Drive {
    pub craft: Craft,
    pos: DVec3,
    vel: DVec3,
    rot: DQuat,
    /// Turning, the world's frame (rad/s).
    spin: DVec3,
    energy: f64,
    warm: f64,
    propellant: f64,
    mass: f64,
    collective: f64,
    assist: bool,
    /// Manual thrusters: the engines' keys held (1-9: the first nine actuators).
    held: Vec<bool>,
    air: usize,
    time: f64,
    /// Each actuator's throttle now (0..1), the power drawn now (W).
    throttles: Vec<f64>,
    power: f64,
    /// Each actuator's thrust now (N: a fan's as the air allows).
    pushes: Vec<f64>,
    /// The mouse taken to steer (a click in the view, as the game's flight: up and
    /// down pitch, aside yaws); ESC gives it back to look round with.
    grabbed: bool,
    /// The attitude the flight computer holds: the pilot turns it (the mouse, the
    /// keys), it flies the craft there; L levels it.
    target: DQuat,
    /// On the ground (any foot touching) last step; the hardest touchdown so far
    /// (m/s) and the most any leg took (N, which).
    grounded: bool,
    hardest: f64,
    leg_most: (f64, usize),
    /// What's happened: each said once.
    pub events: Vec<String>,
    stopped: bool,
    /// The camera: round the craft and how far.
    cam_yaw: f32,
    cam_pitch: f32,
    cam_dist: f32,
}

impl Drive {
    /// The craft stood on its legs (or its lowest point) on the ground.
    pub fn new(craft: Craft) -> Self {
        let lowest = craft.legs.iter().map(|l| l.foot.y).chain(craft.boxes.iter().map(|b| f64::from(b.0.y))).fold(f64::MAX, f64::min);
        let size = craft.boxes.iter().map(|b| b.1.distance(b.0)).fold(4.0f32, f32::max);
        let n = craft.actuators.len();
        Drive {
            pos: DVec3::new(0.0, -lowest + 0.01, 0.0),
            vel: DVec3::ZERO,
            rot: DQuat::IDENTITY,
            spin: DVec3::ZERO,
            energy: craft.stored,
            warm: 0.0,
            propellant: craft.propellant,
            mass: craft.mass,
            collective: 0.0,
            assist: true,
            held: vec![false; n],
            air: 0,
            time: 0.0,
            throttles: vec![0.0; n],
            power: 0.0,
            pushes: vec![0.0; n],
            grabbed: false,
            target: DQuat::IDENTITY,
            grounded: true,
            hardest: 0.0,
            leg_most: (0.0, 0),
            events: Vec::new(),
            stopped: false,
            cam_yaw: 0.0,
            cam_pitch: 0.25,
            cam_dist: size * 2.2,
            craft,
        }
    }

    /// Its collective set (dev: a test drive started part way up), and the craft
    /// flown on with no hands for `dt`.
    pub fn set_collective(&mut self, c: f64) {
        self.collective = c.clamp(0.0, 1.0);
    }

    /// Steering with the mouse (it's taken: hidden, held in the window)?
    pub fn grabbed(&self) -> bool {
        self.grabbed
    }

    /// The mouse given back (ESC): to look round with.
    pub fn release(&mut self) {
        self.grabbed = false;
    }

    pub fn set_assist(&mut self, on: bool) {
        self.assist = on;
    }

    #[cfg(test)]
    pub fn advance(&mut self, dt: f64) {
        for _ in 0..4 {
            self.step(dt / 4.0, DVec3::ZERO);
        }
        self.time += dt;
    }

    /// Pitch and roll (degrees), and the speed (m/s).
    #[cfg(test)]
    pub fn attitude(&self) -> (f64, f64, f64) {
        let nose = self.rot * DVec3::Z;
        (nose.y.asin().to_degrees(), (self.rot * DVec3::X).y.asin().to_degrees(), self.vel.length())
    }

    /// Height (m), climb (m/s), batteries left (0..1), packs' warming (K).
    #[cfg(test)]
    pub fn reading(&self) -> (f64, f64, f64, f64) {
        (self.pos.y, self.vel.y, if self.craft.stored > 0.0 { self.energy / self.craft.stored } else { 0.0 }, self.warm)
    }

    fn say(&mut self, what: String) {
        if !self.events.contains(&what) {
            self.events.push(what);
        }
    }

    /// The air's density at height `h` (kg/m3).
    fn density(&self, h: f64) -> f64 {
        AIRS[self.air] * (-h.max(0.0) / SCALE_HEIGHT).exp()
    }

    /// One frame's input and flight.
    pub fn input(&mut self, ctx: &Context) {
        let input = &ctx.input;
        let key = |k: KeyCode| if input.down(k) { 1.0 } else { 0.0 };
        let dt = f64::from(ctx.dt).min(0.05);
        // (G: manual thrusters, the flight computer off; again, back on: the game's key.)
        if input.pressed(KeyCode::KeyG) {
            self.assist = !self.assist;
            self.target = within_tilt(self.rot);
            self.events.retain(|e| !e.contains("FLIGHT COMPUTER"));
            self.say(if self.assist { "FLIGHT COMPUTER ON".into() } else { "MANUAL THRUSTERS: FLIGHT COMPUTER OFF; 1-9 FIRE EACH ENGINE".into() });
        }
        let digits = [KeyCode::Digit1, KeyCode::Digit2, KeyCode::Digit3, KeyCode::Digit4, KeyCode::Digit5, KeyCode::Digit6, KeyCode::Digit7, KeyCode::Digit8, KeyCode::Digit9];
        for (k, h) in self.held.iter_mut().enumerate() {
            *h = digits.get(k).is_some_and(|&d| input.down(d));
        }
        if input.pressed(KeyCode::KeyZ) {
            self.air = (self.air + 1) % AIRS.len();
        }
        if input.pressed(KeyCode::KeyR) {
            let grabbed = self.grabbed;
            *self = Drive::new(self.craft.clone());
            self.grabbed = grabbed;
            return;
        }
        // (W and S move the collective; X cuts it.)
        self.collective = (self.collective + (key(KeyCode::KeyW) - key(KeyCode::KeyS)) * 0.4 * dt).clamp(0.0, 1.0);
        if input.pressed(KeyCode::KeyX) {
            self.collective = 0.0;
        }
        // (The mouse, as in the game's flight: a click takes it to steer, ESC gives it
        // back. Taken, its moves pitch (up: nose up) and yaw (aside), as the game's
        // do; given back, a drag turns the view and the wheel brings it nearer.)
        let clicked = input.button_pressed(MouseButton::Left) && !self.grabbed;
        if clicked {
            self.grabbed = true;
        }
        let command = DVec3::new(
            key(KeyCode::ArrowUp) - key(KeyCode::ArrowDown),
            key(KeyCode::KeyQ) - key(KeyCode::KeyE),
            key(KeyCode::ArrowRight) - key(KeyCode::ArrowLeft) + key(KeyCode::KeyD) - key(KeyCode::KeyA),
        );
        if self.grabbed && !clicked {
            // (The mouse turns the attitude held, a degree for every 4 px: up and down
            // pitch (up: nose up), aside yaws; no limit.)
            let m = input.mouse_delta;
            let k = 0.25f64.to_radians();
            self.target = within_tilt((self.target * DQuat::from_scaled_axis(DVec3::new(f64::from(m.y) * k, -f64::from(m.x) * k, 0.0))).normalize());
        }
        if input.pressed(KeyCode::KeyL) {
            // (Level: upright, its heading kept.)
            let nose = self.rot * DVec3::Z;
            self.target = DQuat::from_rotation_y(nose.x.atan2(nose.z));
        }
        let command = command.clamp(DVec3::splat(-1.0), DVec3::splat(1.0));
        if !self.grabbed && (input.button_down(MouseButton::Left) || input.button_down(MouseButton::Right)) {
            self.cam_yaw -= input.mouse_delta.x * 0.008;
            self.cam_pitch = (self.cam_pitch + input.mouse_delta.y * 0.008).clamp(-0.2, 1.4);
        }
        if input.scroll != 0.0 {
            self.cam_dist = (self.cam_dist * 0.88f32.powf(input.scroll)).clamp(3.0, 400.0);
        }
        if self.stopped {
            return;
        }
        // (Four steps a frame: the legs are stiff springs.)
        for _ in 0..4 {
            self.step(dt / 4.0, command);
        }
        self.time += dt;
    }

    /// The throttles from the pilot's (and ASSIST's) wishes: the collective on what
    /// pushes up, and about each axis each actuator by its share of the most torque
    /// any gives about it.
    fn mix(&self, command: DVec3) -> Vec<f64> {
        let c = &self.craft;
        let body_spin = self.rot.inverse() * self.spin;
        // (Manual thrusters: the collective on what lifts, each engine whose key is
        // held a third more (a side one full); nothing shared out, nothing steadied.)
        if !self.assist {
            return c.actuators.iter().zip(&self.held).map(|(a, &h)| {
                let base = if a.dir.y > 0.5 { self.collective } else { 0.0 };
                (base + if h { if a.dir.y > 0.5 { 0.33 } else { 1.0 } } else { 0.0 }).clamp(0.0, 1.0)
            }).collect();
        }
        // (The flight computer, holding an attitude: it turns the craft toward the
        // attitude held (the turn between them, about the craft's own axes, by the
        // right hand), damping the turning, and shares that out among the engines;
        // leaning, it adds collective to hold the height (to half as much again,
        // none past sideways).)
        let off = self.target * self.rot.inverse();
        let (axis, angle) = off.to_axis_angle();
        let angle = if angle > std::f64::consts::PI { angle - std::f64::consts::TAU } else { angle };
        let turn = self.rot.inverse() * (axis * angle);
        let want = (turn * 1.6 - body_spin * 1.2).clamp(DVec3::splat(-1.0), DVec3::splat(1.0));
        let _ = command;
        let up = (self.rot * DVec3::Y).y;
        let tilt = if up > 0.0 { up.max(1.0 / 1.6) } else { 1.0 };
        throttles(c, (self.collective / tilt).min(1.0), want)
    }

    fn step(&mut self, dt: f64, command: DVec3) {
        // (Held keys turn the attitude held at KEY_TURN: up lifts the nose, right
        // drops the pilot's right side (facing the nose, +z, with +y up, that's -x:
        // a turn about +z), Q yaws left.)
        if self.assist && command != DVec3::ZERO {
            self.target = within_tilt((self.target * DQuat::from_scaled_axis(DVec3::new(-command.x, command.y, command.z) * KEY_TURN * dt)).normalize());
        }
        let g = self.craft.gravity;
        let h = self.pos.y;
        let rho = self.density(h);
        let mut throttles = self.mix(command);
        // (Fans: their thrust as the air allows; their power as thrust to the 1.5,
        // the most the batteries and plants give sharing it out.)
        let mut fan_power = 0.0;
        for (a, t) in self.craft.actuators.iter().zip(throttles.iter_mut()) {
            if let Kind::Fan { full_power, min_density } = a.kind {
                if rho < min_density {
                    *t = 0.0;
                }
                fan_power += full_power * t.powf(1.5);
            }
        }
        let give = self.craft.rate + self.craft.plants;
        let want = fan_power + self.craft.steady;
        if want > give && fan_power > 0.0 {
            let scale = ((give - self.craft.steady).max(0.0) / fan_power).powf(2.0 / 3.0);
            for (a, t) in self.craft.actuators.iter().zip(throttles.iter_mut()) {
                if matches!(a.kind, Kind::Fan { .. }) {
                    *t *= scale;
                }
            }
            fan_power = (give - self.craft.steady).max(0.0);
            self.say("THE PACKS CAN'T GIVE ALL THE FANS ASK: THROTTLED".into());
        }
        let empty = self.energy <= 0.0;
        let hot = self.warm >= self.craft.warming;
        let fanless = self.craft.actuators.iter().any(|a| matches!(a.kind, Kind::Fan { min_density, .. } if rho < min_density));
        if fanless && self.collective > 0.0 && self.craft.actuators.iter().any(|a| matches!(a.kind, Kind::Fan { .. })) {
            self.say(format!("THE AIR IS TOO THIN FOR THE FANS ({rho:.2} KG/M3)"));
        }
        if empty || hot {
            for (a, t) in self.craft.actuators.iter().zip(throttles.iter_mut()) {
                if matches!(a.kind, Kind::Fan { .. }) {
                    *t = 0.0;
                }
            }
            fan_power = 0.0;
            self.say(if empty { "THE BATTERIES ARE EMPTY: THE FANS STOP".into() } else { "THE PACKS ARE TOO HOT: THEY CUT OUT".into() });
        }
        // (The batteries give what's drawn less what the plants give; their losses
        // warm them.)
        let from_packs = (fan_power + self.craft.steady - self.craft.plants).max(0.0);
        let eff = self.craft.efficiency.max(0.01);
        self.energy -= from_packs / eff * dt;
        self.warm += from_packs * (1.0 / eff - 1.0) * dt / self.craft.heat_capacity.max(1.0);
        self.power = fan_power + self.craft.steady;
        // (Rockets burn propellant while there's any.)
        if self.propellant <= 0.0 {
            for (a, t) in self.craft.actuators.iter().zip(throttles.iter_mut()) {
                if matches!(a.kind, Kind::Rocket { .. }) {
                    *t = 0.0;
                }
            }
        }
        // (Forces and turning, the world's frame.)
        let mut force = DVec3::new(0.0, -g * self.mass, 0.0);
        let mut torque = DVec3::ZERO;
        let mut pushes = vec![0.0; throttles.len()];
        for (k, (a, &t)) in self.craft.actuators.iter().zip(&throttles).enumerate() {
            let thrust = match a.kind {
                Kind::Fan { .. } => a.thrust * t * (rho / 1.225).cbrt().min(1.0),
                Kind::Rocket { exhaust } => {
                    let f = a.thrust * t;
                    let burned = (f / exhaust.max(1.0) * dt).min(self.propellant);
                    self.propellant -= burned;
                    self.mass -= burned;
                    f
                }
            };
            pushes[k] = thrust;
            let f = self.rot * (a.dir * thrust);
            force += f;
            torque += (self.rot * a.at).cross(f);
        }
        let speed = self.vel.length();
        force -= self.vel * (0.5 * rho * DRAG * self.craft.area * speed);
        torque -= self.spin * self.mass * 0.5;
        // (The legs: springs where a foot is below the ground; friction along it.)
        let mut touching = false;
        let mut contact = vec![0.0; self.craft.legs.len()];
        for (k, leg) in self.craft.legs.iter().enumerate() {
            let r = self.rot * leg.foot;
            let p = self.pos + r;
            if p.y >= 0.0 {
                continue;
            }
            touching = true;
            let v = self.vel + self.spin.cross(r);
            let stiff = leg.holds / leg.stroke.max(0.05);
            let damp = 2.0 * 0.7 * (stiff * self.mass / self.craft.legs.len().max(1) as f64).sqrt();
            let up = (stiff * -p.y - damp * v.y).max(0.0);
            contact[k] = up;
            let slide = DVec3::new(v.x, 0.0, v.z);
            let rub = if slide.length() > 1e-3 { -slide.normalize() * (FRICTION * up).min(slide.length() * self.mass * 4.0) } else { DVec3::ZERO };
            let f = DVec3::new(0.0, up, 0.0) + rub;
            force += f;
            torque += r.cross(f);
        }
        if touching && !self.grounded {
            let sink = -self.vel.y;
            self.hardest = self.hardest.max(sink);
            // (Down on the pad, or off it: where, how hard.)
            let off = DVec3::new(self.pos.x, 0.0, self.pos.z - PAD_AHEAD).length();
            if self.time > 1.0 {
                self.events.retain(|e| !e.starts_with("TOUCHED DOWN"));
                self.say(if off <= PAD_ACROSS * 0.5 { format!("TOUCHED DOWN ON THE PAD, {off:.1} M FROM ITS MIDDLE, AT {sink:.1} M/S") } else { format!("TOUCHED DOWN {:.0} M FROM THE PAD, AT {sink:.1} M/S", off - PAD_ACROSS * 0.5) });
            }
            if let Some(rated) = self.craft.legs.iter().map(|l| l.sink).reduce(f64::min)
                && sink > rated
            {
                self.say(format!("HARD LANDING: {sink:.1} M/S, THE LEGS ARE RATED {rated:.1}"));
            }
        }
        self.grounded = touching;
        for (k, &f) in contact.iter().enumerate() {
            if f > self.leg_most.0 {
                self.leg_most = (f, k);
            }
            if f > self.craft.legs[k].holds {
                self.say(format!("LEG {} OVERLOADED: {:.0} OF {:.0} KN", k + 1, f / 1000.0, self.craft.legs[k].holds / 1000.0));
            }
        }
        // (Anything but a leg on the ground: a crash if it hits it moving, else a
        // scrape (said).)
        let (pos, rot) = (self.pos, self.rot);
        let scraped = self.craft.boxes.iter().enumerate().filter(|(k, _)| !self.craft.leg_boxes.get(*k).copied().unwrap_or(false)).any(|(_, (lo, hi))| {
            (0..8).map(|n| Vec3::new(if n & 1 == 0 { lo.x } else { hi.x }, if n & 2 == 0 { lo.y } else { hi.y }, if n & 4 == 0 { lo.z } else { hi.z })).any(|c| (pos + rot * c.as_dvec3()).y < 0.0)
        });
        if scraped {
            if speed > 2.0 {
                self.say(format!("CRASHED AT {speed:.1} M/S"));
                self.stopped = true;
                return;
            }
            self.say("SOMETHING BUT A LEG TOUCHED THE GROUND".into());
        }
        // (Moving on: the pace, then the place; the turning, by the inertia as it now lies.)
        self.vel += force / self.mass * dt;
        self.pos += self.vel * dt;
        let r = DMat3::from_quat(self.rot);
        let world_inertia = r * self.craft.inertia * r.transpose();
        let spin_change = world_inertia.inverse() * (torque - self.spin.cross(world_inertia * self.spin));
        self.spin += spin_change * dt;
        let turn = DQuat::from_scaled_axis(self.spin * dt);
        self.rot = (turn * self.rot).normalize();
        self.throttles = throttles;
        self.pushes = pushes;
    }
}

/// The most the flight computer leans the craft from upright (rad): 40 degrees.
const MOST_TILT: f64 = 0.698;

/// An attitude leant no further than MOST_TILT from upright: turned back toward
/// upright, about the level line its lean is across, by what it's over.
fn within_tilt(q: DQuat) -> DQuat {
    let up = q * DVec3::Y;
    let lean = up.y.clamp(-1.0, 1.0).acos();
    if lean <= MOST_TILT {
        return q;
    }
    let axis = up.cross(DVec3::Y).normalize_or_zero();
    if axis == DVec3::ZERO {
        return q;
    }
    (DQuat::from_axis_angle(axis, lean - MOST_TILT) * q).normalize()
}

/// How fast held keys turn the attitude the flight computer holds (rad/s): 90
/// degrees a second.
const KEY_TURN: f64 = std::f64::consts::FRAC_PI_2;

/// The pilot's keys as turning wished for, about the craft's axes (by the right
/// hand: about x, + takes the nose (+z) down; about z, + takes +x, the pilot's left
/// side, up): up lifts the nose, right drops the pilot's right side, Q yaws left.
fn wished(command: DVec3) -> DVec3 {
    DVec3::new(-command.x * 0.5, command.y * 0.5, command.z * 0.5)
}

/// The throttles for a collective and a turning wished for: the collective on what
/// pushes up; about each axis each actuator by its share of the most torque any
/// gives about it (a lifting one by a share of its room, a side one by all its own).
fn throttles(c: &Craft, collective: f64, want: DVec3) -> Vec<f64> {
    let torques: Vec<DVec3> = c.actuators.iter().map(|a| a.at.cross(a.dir * a.thrust)).collect();
    let most = |axis: DVec3| torques.iter().map(|t| t.dot(axis).abs()).fold(1e-9, f64::max);
    let (mx, my, mz) = (most(DVec3::X), most(DVec3::Y), most(DVec3::Z));
    c.actuators.iter().zip(&torques).map(|(a, t)| {
        let lifts = a.dir.y > 0.5;
        let base = if lifts { collective } else { 0.0 };
        let gain = if lifts { 0.35 } else { 1.0 };
        (base + gain * (want.x * t.x / mx + want.y * t.y / my + want.z * t.z / mz)).clamp(0.0, 1.0)
    }).collect()
}

/// The push and turning of throttles (the craft's frame, about its middle of mass).
fn push_and_turn(c: &Craft, throttles: &[f64]) -> (DVec3, DVec3) {
    c.actuators.iter().zip(throttles).fold((DVec3::ZERO, DVec3::ZERO), |(f, t), (a, &th)| {
        let push = a.dir * a.thrust * th;
        (f + push, t + a.at.cross(push))
    })
}

/// The collective that holds its weight up, its lifting thrust all at once (none:
/// nothing lifts it).
fn hover_collective(c: &Craft) -> Option<f64> {
    let lift: f64 = c.actuators.iter().filter(|a| a.dir.y > 0.5).map(|a| a.thrust * a.dir.y).sum();
    (lift > 0.0).then(|| c.mass * c.gravity / lift)
}

/// The trim that holds it level at a collective: the turning wished for about each
/// axis that cancels what the throttles turn it by, solved through how each axis's
/// wish turns it (yaw left out if nothing turns it about that axis).
fn trim(c: &Craft, collective: f64) -> DVec3 {
    let (_, t0) = push_and_turn(c, &throttles(c, collective, DVec3::ZERO));
    let d = 0.05;
    let column = |axis: DVec3| (push_and_turn(c, &throttles(c, collective, axis * d)).1 - t0) / d;
    let mut j = DMat3::from_cols(column(DVec3::X), column(DVec3::Y), column(DVec3::Z));
    let mut keep = DVec3::ONE;
    for k in 0..3 {
        if j.col(k).length() < 1e-6 {
            *j.col_mut(k) = DVec3::ZERO;
            j.col_mut(k)[k] = 1.0;
            keep[k] = 0.0;
        }
    }
    if j.determinant().abs() < 1e-12 {
        return DVec3::ZERO;
    }
    (j.inverse() * -t0) * keep
}

/// The balance room: the craft on a stand, its middle of mass and its pushes shown,
/// thrust tried all together, one at a time, or turned about each axis; the trim
/// that holds it level, and how much of its turning that takes.
pub struct Balance {
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
        let size = craft.boxes.iter().map(|b| b.1.distance(b.0)).fold(4.0f32, f32::max);
        Balance { collective: hover_collective(&craft).unwrap_or(0.0).min(1.0), command: DVec3::ZERO, single: None, trimmed: false, cam_yaw: 0.7, cam_pitch: 0.3, cam_dist: size * 1.8, craft }
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
        self.collective = (self.collective + (key(KeyCode::KeyW) - key(KeyCode::KeyS)) * 0.3 * dt).clamp(0.0, 1.0);
        if input.pressed(KeyCode::KeyH) {
            self.collective = hover_collective(&self.craft).unwrap_or(0.0).min(1.0);
        }
        self.command = DVec3::new(key(KeyCode::ArrowUp) - key(KeyCode::ArrowDown), key(KeyCode::KeyQ) - key(KeyCode::KeyE), key(KeyCode::ArrowRight) - key(KeyCode::ArrowLeft));
        if input.pressed(KeyCode::Tab) {
            let n = self.craft.actuators.len();
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
    fn now(&self) -> (Vec<f64>, DVec3) {
        let t = if self.trimmed { trim(&self.craft, self.collective) } else { DVec3::ZERO };
        match self.single {
            Some(k) => ((0..self.craft.actuators.len()).map(|i| if i == k { self.collective.max(0.5) } else { 0.0 }).collect(), DVec3::ZERO),
            None => (throttles(&self.craft, self.collective, wished(self.command) + t), t),
        }
    }
}

/// The balance room drawn: the craft as it stands; its middle of mass (and its
/// weight down from it); each push as an arrow its length; the pushes' sum along the
/// line it acts on; and what the turning is and the trim takes.
pub fn draw_balance(frame: &mut Frame, b: &Balance) {
    let size = frame.size();
    frame.hud_rect(Vec2::ZERO, size, Color([0.03, 0.04, 0.08, 1.0]));
    let c = &b.craft;
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
        let side = dir.cross(if dir.y.abs() < 0.9 { Vec3::Y } else { Vec3::X }).normalize_or_zero() * len.min(1.5) * 0.15;
        seg(frame, tip, tip - dir * len.min(1.5) * 0.25 + side, col);
        seg(frame, tip, tip - dir * len.min(1.5) * 0.25 - side, col);
    };
    // (The stand's floor under its lowest point.)
    let floor = c.legs.iter().map(|l| l.foot.y as f32).chain(c.boxes.iter().map(|b| b.0.y)).fold(f32::MAX, f32::min);
    for k in -8..=8 {
        let o = k as f32 * 2.0;
        seg(frame, Vec3::new(o, floor, -16.0), Vec3::new(o, floor, 16.0), Color([0.3, 0.5, 0.4, 0.25]));
        seg(frame, Vec3::new(-16.0, floor, o), Vec3::new(16.0, floor, o), Color([0.3, 0.5, 0.4, 0.25]));
    }
    let camera = crate::equipment_visual::Camera {eye,right,up,forward,focal,centre:size*0.5,near:0.2};
    frame.canvas_scene();
    for t in crate::equipment_visual::project(&c.equipment, universe_engine::glam::Mat4::from_translation(-c.visual_origin), &camera) {t.draw(frame);}
    for [a, bb] in &c.members {
        seg(frame, *a, *bb, Color([1.0, 0.7, 0.3, 0.35]));
    }
    // (The scale: the strongest push 4 m long.)
    let strongest = c.actuators.iter().map(|a| a.thrust).fold(c.mass * c.gravity, f64::max);
    let metres = |n: f64| (n / strongest * 4.0) as f32;
    let (ts, trim_now) = b.now();
    let (force, turn) = push_and_turn(c, &ts);
    for (k, (a, &t)) in c.actuators.iter().zip(&ts).enumerate() {
        let at = a.at.as_vec3();
        let col = if b.single == Some(k) { Color([1.0, 1.0, 0.4, 1.0]) } else { Color([0.4, 0.85, 1.0, 0.95]) };
        if t > 0.001 {
            arrow(frame, at, a.dir.as_vec3(), metres(a.thrust * t).max(0.3), col);
        }
        if let Some(q) = project(at) {
            frame.text_scaled(q + Vec2::new(6.0, 4.0), &format!("{:.0}%", t * 100.0), col, 0.55);
        }
    }
    // (The middle of mass: a cross and a ring; its weight straight down.)
    if let Some(q) = project(Vec3::ZERO) {
        frame.hud_line(q - Vec2::new(10.0, 0.0), q + Vec2::new(10.0, 0.0), Color([1.0, 0.85, 0.3, 1.0]));
        frame.hud_line(q - Vec2::new(0.0, 10.0), q + Vec2::new(0.0, 10.0), Color([1.0, 0.85, 0.3, 1.0]));
        frame.hud_box(q - Vec2::splat(6.0), Vec2::splat(12.0), Color([1.0, 0.85, 0.3, 1.0]));
        frame.text_scaled(q + Vec2::new(12.0, -14.0), "MIDDLE OF MASS", Color([1.0, 0.85, 0.3, 1.0]), 0.6);
    }
    arrow(frame, Vec3::ZERO, Vec3::NEG_Y, metres(c.mass * c.gravity), Color([1.0, 0.85, 0.3, 0.8]));
    // (The pushes' sum, along the line it acts on: through the point of that line
    // nearest the middle (F x M / F^2).)
    let f2 = force.length_squared();
    let through = if f2 > 1e-6 { force.cross(turn) / f2 } else { DVec3::ZERO };
    if f2 > 1e-6 {
        let dir = force.normalize().as_vec3();
        let p = through.as_vec3();
        seg(frame, p - dir * 3.0, p, Color([1.0, 0.4, 0.9, 0.6]));
        arrow(frame, p, dir, metres(force.length()), Color([1.0, 0.4, 0.9, 1.0]));
        if through.length() > 0.02 {
            seg(frame, Vec3::ZERO, p, Color([1.0, 0.4, 0.9, 0.8]));
        }
    }
    // (What it reads.)
    let weight = c.mass * c.gravity;
    let hover = hover_collective(c);
    let trim_needed = hover.map(|h| trim(c, h.min(1.0)));
    let use_of = |u: f64| u.abs() / 0.5 * 100.0;
    let verdict = match (hover, trim_needed) {
        (None, _) => ("NOTHING LIFTS IT: NO HOVER TO BALANCE".to_string(), false),
        (Some(h), _) if h > 1.0 => (format!("TOO HEAVY TO HOVER: IT NEEDS {:.0}% OF ITS LIFT", h * 100.0), false),
        (Some(_), Some(t)) => {
            let most = use_of(t.x).max(use_of(t.z));
            (format!("TO HOVER LEVEL IT TRIMS {:.0}% OF ITS PITCH AND {:.0}% OF ITS ROLL{}", use_of(t.x), use_of(t.z), if most > 100.0 { ": MORE THAN IT HAS, IT CAN'T HOLD LEVEL" } else if most > 50.0 { ": LITTLE LEFT TO FLY WITH" } else { ": WELL IN HAND" }), most <= 50.0)
        }
        _ => (String::new(), false),
    };
    let lines = [
        format!("BALANCE ROOM - {}", c.name.to_uppercase()),
        format!("MASS {:.2} T   WEIGHT {:.1} KN AT {:.2} G", c.mass / 1000.0, weight / 1000.0, c.gravity / 9.80665),
        format!("COLLECTIVE {:.0}%{}   HOVERS AT {}", b.collective * 100.0, b.single.map_or(String::new(), |k| format!("   ONE ALONE: NUMBER {}", k + 1)), hover.map_or("-".into(), |h| format!("{:.0}%", h * 100.0))),
        format!("PUSH {:.1} KN UP, {:.1} KN ACROSS   ITS LINE PASSES {:.2} M FROM THE MIDDLE ({:+.2} ACROSS, {:+.2} ALONG)", force.y / 1000.0, (force.x.powi(2) + force.z.powi(2)).sqrt() / 1000.0, DVec3::new(through.x, 0.0, through.z).length(), through.x, through.z),
        format!("TURNING: PITCH {:+.1} KN M (NOSE {}), ROLL {:+.1} KN M, YAW {:+.1} KN M", -turn.x / 1000.0, if turn.x > 0.0 { "DOWN" } else { "UP" }, turn.z / 1000.0, turn.y / 1000.0),
        format!("TRIM {}: PITCH {:+.0}%, ROLL {:+.0}%, YAW {:+.0}% OF WHAT IT HAS", if b.trimmed { "ON (T)" } else { "OFF (T)" }, use_of(trim_now.x) * trim_now.x.signum(), use_of(trim_now.z) * trim_now.z.signum(), use_of(trim_now.y) * trim_now.y.signum()),
    ];
    frame.hud_rect(Vec2::new(10.0, 10.0), Vec2::new(620.0, 30.0 + lines.len() as f32 * 12.0), Color([0.02, 0.06, 0.13, 0.88]));
    for (k, l) in lines.iter().enumerate() {
        frame.text_scaled(Vec2::new(16.0, 14.0 + k as f32 * 12.0), l, if k == 0 { Color([1.0, 0.85, 0.4, 1.0]) } else { Color([0.5, 1.0, 0.6, 0.95]) }, 0.62);
    }
    frame.text_scaled(Vec2::new(16.0, 16.0 + lines.len() as f32 * 12.0), &verdict.0, if verdict.1 { Color([0.4, 1.0, 0.5, 1.0]) } else { Color([1.0, 0.6, 0.3, 1.0]) }, 0.66);
    frame.text_scaled(Vec2::new(12.0, size.y - 18.0), "W / S COLLECTIVE - H TO HOVER - ARROWS PITCH AND ROLL, Q / E YAW - TAB ONE AT A TIME - T TRIM - DRAG TO LOOK - ENTER FLY IT - ESC BACK", Color([0.75, 0.88, 1.0, 0.7]), 0.6);
}

/// The flight drawn: the ground's grid round it, the craft's boxes and members as
/// it lies, each actuator's push as a line the length of its throttle; and what it
/// reads.
pub fn draw(frame: &mut Frame, d: &Drive) {
    let size = frame.size();
    let sky = if d.density(d.pos.y) > 0.05 { Color([0.05, 0.09, 0.16, 1.0]) } else { Color([0.01, 0.01, 0.03, 1.0]) };
    frame.hud_rect(Vec2::ZERO, size, sky);
    let target = d.pos.as_vec3();
    // (Behind it, as the game's chase view: the camera round to the back of the
    // craft's heading, plus what the drag has turned it.)
    let nose = d.rot * DVec3::Z;
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
    // (The floor: a sheet of see-through tiles round under the craft, cut where it
    // passes behind the eye; over it a grid of 5 m lines, every 25 m brighter; it
    // fades out at its edge.)
    let view = |p: Vec3| {
        let v = p - eye;
        Vec3::new(v.dot(right), v.dot(up), v.dot(forward))
    };
    let near = 0.3;
    let screen = |v: Vec3| size * 0.5 + Vec2::new(v.x, -v.y) * (focal / v.z);
    let reach = 150.0f32;
    let (cx, cz) = ((target.x / 25.0).round() * 25.0, (target.z / 25.0).round() * 25.0);
    let tile = 25.0f32;
    let tiles = (reach / tile) as i32;
    for i in -tiles..tiles {
        for j in -tiles..tiles {
            let (x0, z0) = (cx + i as f32 * tile, cz + j as f32 * tile);
            let quad = [Vec3::new(x0, 0.0, z0), Vec3::new(x0 + tile, 0.0, z0), Vec3::new(x0 + tile, 0.0, z0 + tile), Vec3::new(x0, 0.0, z0 + tile)].map(view);
            let mut poly = Vec::new();
            for k in 0..4 {
                let (a, b) = (quad[k], quad[(k + 1) % 4]);
                if a.z >= near {
                    poly.push(a);
                }
                if (a.z >= near) != (b.z >= near) {
                    poly.push(a.lerp(b, (near - a.z) / (b.z - a.z)));
                }
            }
            if poly.len() < 3 {
                continue;
            }
            let far = Vec2::new(x0 + tile * 0.5 - target.x, z0 + tile * 0.5 - target.z).length() / reach;
            let shade = if (i + j) % 2 == 0 { 0.16 } else { 0.12 };
            let c = Color([0.2, 0.42, 0.32, shade * (1.0 - far).max(0.0)]);
            let q: Vec<Vec2> = poly.iter().map(|p| screen(*p)).collect();
            for k in 1..q.len() - 1 {
                frame.hud_triangle_colored([q[0], q[k], q[k + 1]], [c; 3]);
            }
        }
    }
    let line = |frame: &mut Frame, a: Vec3, b: Vec3, c: Color| {
        let (mut va, mut vb) = (view(a), view(b));
        if va.z < near && vb.z < near {
            return;
        }
        if va.z < near {
            va = va.lerp(vb, (near - va.z) / (vb.z - va.z));
        } else if vb.z < near {
            vb = vb.lerp(va, (near - vb.z) / (va.z - vb.z));
        }
        frame.hud_line(screen(va), screen(vb), c);
    };
    let (gx, gz) = ((target.x / 5.0).round() * 5.0, (target.z / 5.0).round() * 5.0);
    let lines_out = (reach / 5.0) as i32;
    for k in -lines_out..=lines_out {
        let o = k as f32 * 5.0;
        let major = ((gx + o) / 25.0).fract().abs() < 1e-3;
        let c = if major { Color([0.45, 0.75, 0.6, 0.55]) } else { Color([0.35, 0.6, 0.5, 0.25]) };
        line(frame, Vec3::new(gx + o, 0.0, gz - reach), Vec3::new(gx + o, 0.0, gz + reach), c);
        let major = ((gz + o) / 25.0).fract().abs() < 1e-3;
        let c = if major { Color([0.45, 0.75, 0.6, 0.55]) } else { Color([0.35, 0.6, 0.5, 0.25]) };
        line(frame, Vec3::new(gx - reach, 0.0, gz + o), Vec3::new(gx + reach, 0.0, gz + o), c);
    }
    // (The pad: its ring, its middle's cross, lights round its edge; the beacon:
    // a mast at the start, its light on top.)
    let pad = Vec3::new(0.0, 0.0, PAD_AHEAD as f32);
    let half = PAD_ACROSS as f32 * 0.5;
    let pad_col = Color([1.0, 0.85, 0.35, 0.9]);
    for k in 0..48 {
        let (a0, a1) = (k as f32 / 48.0 * std::f32::consts::TAU, (k + 1) as f32 / 48.0 * std::f32::consts::TAU);
        line(frame, pad + Vec3::new(a0.cos(), 0.0, a0.sin()) * half, pad + Vec3::new(a1.cos(), 0.0, a1.sin()) * half, pad_col);
        line(frame, pad + Vec3::new(a0.cos(), 0.0, a0.sin()) * half * 0.6, pad + Vec3::new(a1.cos(), 0.0, a1.sin()) * half * 0.6, Color([1.0, 0.85, 0.35, 0.5]));
    }
    line(frame, pad - Vec3::X * half * 0.4, pad + Vec3::X * half * 0.4, pad_col);
    line(frame, pad - Vec3::Z * half * 0.4, pad + Vec3::Z * half * 0.4, pad_col);
    for k in 0..8 {
        let a = k as f32 / 8.0 * std::f32::consts::TAU;
        let p = pad + Vec3::new(a.cos(), 0.0, a.sin()) * half;
        if let Some(q) = project(p) {
            frame.hud_box(q - Vec2::splat(2.0), Vec2::splat(4.0), Color([1.0, 0.4, 0.3, 1.0]));
        }
    }
    if let Some(q) = project(pad + Vec3::Y * 0.5) {
        frame.text_scaled(q + Vec2::new(8.0, -10.0), "PAD", pad_col, 0.6);
    }
    let blink = if (d.time * 2.0).fract() < 0.5 { 1.0 } else { 0.35 };
    line(frame, Vec3::ZERO, Vec3::Y * 6.0, Color([0.8, 0.85, 0.9, 0.8]));
    if let Some(q) = project(Vec3::Y * 6.0) {
        frame.hud_box(q - Vec2::splat(3.0), Vec2::splat(6.0), Color([1.0, 0.3, 0.3, blink]));
        frame.text_scaled(q + Vec2::new(8.0, -4.0), "BEACON", Color([1.0, 0.5, 0.45, 0.9]), 0.6);
    }
    let place = |p: Vec3| (d.pos + d.rot * p.as_dvec3()).as_vec3();
    // (Its shadow: straight down.)
    seg(frame, target, Vec3::new(target.x, 0.0, target.z), Color([0.6, 0.6, 0.6, 0.4]));
    let camera = crate::equipment_visual::Camera {eye,right,up,forward,focal,centre:size*0.5,near:0.2};
    let pose = universe_engine::glam::Mat4::from_rotation_translation(d.rot.as_quat(), d.pos.as_vec3()) * universe_engine::glam::Mat4::from_translation(-d.craft.visual_origin);
    frame.canvas_scene();
    for t in crate::equipment_visual::project(&d.craft.equipment, pose, &camera) {t.draw(frame);}
    for [a, b] in &d.craft.members {
        seg(frame, place(*a), place(*b), Color([1.0, 0.7, 0.3, 0.8]));
    }
    // (Each push: a line the other way from where it pushes, its throttle long.)
    // (And on each its number (1-9 are its manual keys), its thrust now and its
    // throttle.)
    for (k, (a, &t)) in d.craft.actuators.iter().zip(&d.throttles).enumerate() {
        let from = place(a.at.as_vec3());
        if t > 0.01 {
            let to = place((a.at - a.dir * (0.5 + 3.0 * t)).as_vec3());
            seg(frame, from, to, Color([0.4, 0.8, 1.0, 0.9]));
        }
        if let Some(q) = project(from) {
            let push = d.pushes.get(k).copied().unwrap_or(0.0);
            let text = if push >= 1000.0 { format!("{} {:.1} KN {:.0}%", k + 1, push / 1000.0, t * 100.0) } else { format!("{} {:.0} N {:.0}%", k + 1, push, t * 100.0) };
            let col = if t > 0.01 { Color([0.6, 0.95, 1.0, 1.0]) } else { Color([0.6, 0.7, 0.8, 0.6]) };
            frame.hud_rect(q + Vec2::new(6.0, -2.0), Vec2::new(text.len() as f32 * 5.6 + 6.0, 11.0), Color([0.0, 0.02, 0.06, 0.75]));
            frame.text_scaled(q + Vec2::new(9.0, 0.0), &text, col, 0.55);
        }
    }
    // (What it reads.)
    let up_now = d.rot * DVec3::Y;
    let nose = d.rot * DVec3::Z;
    let pitch = nose.y.asin().to_degrees();
    let roll = (d.rot * DVec3::X).y.asin().to_degrees();
    let heading = (-nose.x).atan2(nose.z).to_degrees();
    let c = &d.craft;
    let left = if d.power > c.plants && d.energy > 0.0 { format!(", {:.0} S AT THIS DRAW", d.energy * c.efficiency / (d.power - c.plants)) } else { String::new() };
    let lines = [
        format!("TEST DRIVE - {}   {:.1} S", c.name.to_uppercase(), d.time),
        format!("ALTITUDE {:.1} M OVER THE GROUND   CLIMB {:+.1} M/S   SPEED {:.1} M/S", d.craft.legs.iter().map(|l| (d.pos + d.rot * l.foot).y).fold(d.pos.y, f64::min).max(0.0), d.vel.y, DVec3::new(d.vel.x, 0.0, d.vel.z).length()),
        format!("PITCH {pitch:+.0}   ROLL {roll:+.0}   HEADING {heading:.0}   {}", if up_now.y < 0.0 { "UPSIDE DOWN" } else { "" }),
        format!("COLLECTIVE {:.0}%   {}   {}", d.collective * 100.0, if d.grabbed { "THE MOUSE STEERS (ESC FREES IT)" } else { "CLICK TO STEER WITH THE MOUSE" }, if d.assist { "FLIGHT COMPUTER ON: IT STEERS (G FOR MANUAL)".to_string() } else { format!("MANUAL THRUSTERS (G FOR THE FLIGHT COMPUTER): HELD {}", d.held.iter().enumerate().filter(|(_, h)| **h).map(|(k, _)| (k + 1).to_string()).collect::<Vec<_>>().join(" ")) }),
        format!("POWER {:.2} MW OF {:.2} MW   BATTERIES {:.0}%{left}", d.power / 1e6, (c.rate + c.plants) / 1e6, if c.stored > 0.0 { d.energy.max(0.0) / c.stored * 100.0 } else { 0.0 }),
        format!("PACKS +{:.1} K OF {:.0} K   PROPELLANT {:.0} KG   MASS {:.2} T", d.warm, c.warming, d.propellant, d.mass / 1000.0),
        format!("AIR {:.3} KG/M3 (Z)   GRAVITY {:.2} G   HARDEST TOUCHDOWN {:.1} M/S   MOST ON A LEG {:.0} KN", d.density(d.pos.y), c.gravity / 9.80665, d.hardest, d.leg_most.0 / 1000.0),
    ];
    let panel = Color([0.02, 0.06, 0.13, 0.85]);
    frame.hud_rect(Vec2::new(10.0, 10.0), Vec2::new(560.0, 14.0 + lines.len() as f32 * 12.0), panel);
    for (k, l) in lines.iter().enumerate() {
        frame.text_scaled(Vec2::new(16.0, 14.0 + k as f32 * 12.0), l, if k == 0 { Color([1.0, 0.85, 0.4, 1.0]) } else { Color([0.5, 1.0, 0.6, 0.95]) }, 0.62);
    }
    for (k, e) in d.events.iter().enumerate() {
        frame.text_scaled(Vec2::new(16.0, 30.0 + (lines.len() + k) as f32 * 12.0), e, Color([1.0, 0.45, 0.35, 1.0]), 0.62);
    }
    // (The tapes: altitude over the ground (the lowest foot's height) on the right,
    // with the climb beside it; speed over the ground on the left; each reading
    // large in its box at the middle.)
    let feet = d.craft.legs.iter().map(|l| (d.pos + d.rot * l.foot).y).fold(f64::MAX, f64::min);
    let height = if feet == f64::MAX { d.pos.y } else { feet.max(0.0) };
    let ground_speed = DVec3::new(d.vel.x, 0.0, d.vel.z).length();
    let tape = |frame: &mut Frame, x: f32, value: f64, per: f64, step: f64, label: &str, unit: &str, left: bool| {
        let (h, w) = (300.0f32, 64.0f32);
        let top = size.y * 0.5 - h * 0.5;
        frame.hud_rect(Vec2::new(x, top), Vec2::new(w, h), Color([0.02, 0.05, 0.1, 0.55]));
        frame.hud_box(Vec2::new(x, top), Vec2::new(w, h), Color([0.5, 0.8, 1.0, 0.5]));
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
            let (a, b) = if left { (x + w - len, x + w) } else { (x, x + len) };
            frame.hud_line(Vec2::new(a, y), Vec2::new(b, y), Color([0.7, 0.9, 1.0, 0.8]));
            if major {
                let tx = if left { x + 6.0 } else { x + 16.0 };
                frame.text_scaled(Vec2::new(tx, y - 4.0), &format!("{v:.0}"), Color([0.7, 0.9, 1.0, 0.8]), 0.55);
            }
        }
        let bx = Vec2::new(x - 6.0, size.y * 0.5 - 11.0);
        frame.hud_rect(bx, Vec2::new(w + 12.0, 22.0), Color([0.0, 0.0, 0.0, 0.9]));
        frame.hud_box(bx, Vec2::new(w + 12.0, 22.0), Color([1.0, 0.85, 0.4, 1.0]));
        frame.text_scaled(bx + Vec2::new(4.0, 4.0), &format!("{value:.1}"), Color([1.0, 0.85, 0.4, 1.0]), 0.95);
        frame.text_scaled(Vec2::new(x, top - 14.0), label, Color([0.75, 0.88, 1.0, 0.85]), 0.6);
        frame.text_scaled(Vec2::new(x, top + h + 4.0), unit, Color([0.75, 0.88, 1.0, 0.7]), 0.55);
    };
    let alt_x = size.x - 120.0;
    tape(frame, alt_x, height, 25.0, 5.0, "ALTITUDE", "M OVER THE GROUND", false);
    tape(frame, 40.0, ground_speed, 25.0, 5.0, "SPEED", "M/S OVER THE GROUND", true);
    // (The climb: a bar from the middle, up green, down amber (red past the legs'
    // rated sink), to 10 m/s each way.)
    {
        let (h, x) = (300.0f32, alt_x + 76.0);
        let mid = size.y * 0.5;
        frame.hud_rect(Vec2::new(x, mid - h * 0.5), Vec2::new(14.0, h), Color([0.02, 0.05, 0.1, 0.55]));
        frame.hud_box(Vec2::new(x, mid - h * 0.5), Vec2::new(14.0, h), Color([0.5, 0.8, 1.0, 0.5]));
        let climb = d.vel.y.clamp(-10.0, 10.0);
        let len = (climb / 10.0) as f32 * h * 0.5;
        let rated = d.craft.legs.iter().map(|l| l.sink).reduce(f64::min).unwrap_or(3.0);
        let c = if climb >= 0.0 { Color([0.4, 1.0, 0.5, 0.9]) } else if -climb > rated { Color([1.0, 0.35, 0.3, 0.95]) } else { Color([1.0, 0.75, 0.3, 0.9]) };
        let (top, hgt) = if len >= 0.0 { (mid - len, len) } else { (mid, -len) };
        frame.hud_rect(Vec2::new(x + 2.0, top), Vec2::new(10.0, hgt.max(1.0)), c);
        // (The legs' rated sink marked.)
        let mark = mid + (rated / 10.0) as f32 * h * 0.5;
        frame.hud_line(Vec2::new(x - 3.0, mark), Vec2::new(x + 17.0, mark), Color([1.0, 0.35, 0.3, 0.8]));
        frame.hud_line(Vec2::new(x - 3.0, mid), Vec2::new(x + 17.0, mid), Color([0.7, 0.9, 1.0, 0.8]));
        frame.text_scaled(Vec2::new(x - 12.0, mid - h * 0.5 - 14.0), "CLIMB", Color([0.75, 0.88, 1.0, 0.85]), 0.6);
        frame.text_scaled(Vec2::new(x - 24.0, mid + h * 0.5 + 18.0), &format!("{:+.1} M/S", d.vel.y), c, 0.7);
    }
    // (The pad: how far, which way (from the nose, right +), how high over it.)
    let to_pad = DVec3::new(-d.pos.x, 0.0, PAD_AHEAD - d.pos.z);
    let bearing = {
        let n = DVec3::new(nose.x, 0.0, nose.z);
        let a = (-to_pad.x).atan2(to_pad.z) - (-n.x).atan2(n.z);
        (a.to_degrees() + 540.0) % 360.0 - 180.0
    };
    frame.text_scaled(Vec2::new(16.0, 18.0 + (lines.len() + d.events.len() + 1) as f32 * 12.0), &format!("PAD {:.0} M, {:+.0} DEG FROM THE NOSE, {:.1} M OVER IT   BEACON {:.0} M", to_pad.length(), -bearing, height, DVec3::new(d.pos.x, 0.0, d.pos.z).length()), Color([1.0, 0.85, 0.35, 0.95]), 0.62);
    // (The game's own radar, on a landing's scale: 10 m in the middle to 10 km at
    // the rim; the craft's frame turned to the scanner's (ahead -z, right +x).)
    let scope = crate::hud::Scope::draw(frame, Vec2::new((size.x / 2.0).floor(), size.y - 74.0), 10.0, 3.0);
    let to_scanner = |world: DVec3| {
        let v = d.rot.inverse() * (world - d.pos);
        DVec3::new(-v.x, v.y, -v.z)
    };
    scope.body(frame, to_scanner(DVec3::new(0.0, 0.0, PAD_AHEAD)), Color([1.0, 0.85, 0.35, 1.0]), 2.5);
    scope.craft(frame, to_scanner(DVec3::new(0.0, 6.0, 0.0)), Color([1.0, 0.35, 0.3, 1.0]), false);
    frame.text_scaled(Vec2::new(12.0, size.y - 18.0), "CLICK: MOUSE STEERS - W/S COLLECTIVE, X CUT - ARROWS PITCH, A/D ROLL, Q/E YAW - L LEVEL - G MANUAL, 1-9 - Z AIR - R RESTART - ESC: MOUSE, BACK", Color([0.75, 0.88, 1.0, 0.7]), 0.6);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A tonne on four fans a metre out, each 4 kN: past hover thrust it climbs,
    /// level on ASSIST, and the packs run down.
    #[test]
    fn four_fans_lift_a_tonne() {
        let fan = |x: f64, z: f64| Actuator { at: DVec3::new(x, 0.0, z), dir: DVec3::Y, thrust: 4000.0, kind: Kind::Fan { full_power: 200_000.0, min_density: 0.3 } };
        let leg = |x: f64, z: f64| Leg { foot: DVec3::new(x, -1.0, z), holds: 20_000.0, stroke: 0.3, sink: 3.0 };
        let craft = Craft {
            name: "test".into(),
            mass: 1000.0,
            inertia: DMat3::from_diagonal(DVec3::splat(400.0)),
            actuators: vec![fan(-1.0, -1.0), fan(1.0, -1.0), fan(-1.0, 1.0), fan(1.0, 1.0)],
            legs: vec![leg(-1.0, -1.0), leg(1.0, -1.0), leg(-1.0, 1.0), leg(1.0, 1.0)],
            stored: 1.0e8,
            rate: 1.0e6,
            efficiency: 0.95,
            heat_capacity: 1.0e5,
            warming: 40.0,
            gravity: 9.80665,
            area: 4.0,
            ..Default::default()
        };
        let mut d = Drive::new(craft);
        d.set_collective(0.8);
        d.set_assist(true);
        let (h0, ..) = d.reading();
        for _ in 0..300 {
            d.advance(1.0 / 60.0);
        }
        let (h, climb, left, warm) = d.reading();
        let (pitch, roll, _) = d.attitude();
        assert!(h > h0 + 1.0 && climb > 0.0, "{h} {climb}");
        assert!(pitch.abs() < 2.0 && roll.abs() < 2.0, "{pitch} {roll}");
        assert!(left < 1.0 && warm > 0.0, "{left} {warm}");
    }
}
