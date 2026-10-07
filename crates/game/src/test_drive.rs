//! TEST DRIVE: a studio design flown as it is, over flat ground at its design
//! gravity: one rigid body (its parts' masses where they sit, its inertia from
//! them), each engine and fan pushing from where it is the way it points, at its
//! record's thrust. Fans take their thrust from the air (thinner with height) and
//! their power from the batteries (thrust to the 1.5), which run down and warm by
//! their losses; rockets burn their propellant; legs are springs, rated by what
//! they hold over their stroke. Flown by hand: a collective and differential
//! thrust about each axis; ASSIST (optional) damps the turning and levels it.

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
    air: usize,
    time: f64,
    /// Each actuator's throttle now (0..1), the power drawn now (W).
    throttles: Vec<f64>,
    power: f64,
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
            assist: false,
            air: 0,
            time: 0.0,
            throttles: vec![0.0; n],
            power: 0.0,
            grounded: true,
            hardest: 0.0,
            leg_most: (0.0, 0),
            events: Vec::new(),
            stopped: false,
            cam_yaw: 0.6,
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
        if input.pressed(KeyCode::KeyH) {
            self.assist = !self.assist;
        }
        if input.pressed(KeyCode::KeyZ) {
            self.air = (self.air + 1) % AIRS.len();
        }
        if input.pressed(KeyCode::KeyR) {
            *self = Drive::new(self.craft.clone());
            return;
        }
        // (W and S move the collective; X cuts it.)
        self.collective = (self.collective + (key(KeyCode::KeyW) - key(KeyCode::KeyS)) * 0.4 * dt).clamp(0.0, 1.0);
        if input.pressed(KeyCode::KeyX) {
            self.collective = 0.0;
        }
        let command = DVec3::new(key(KeyCode::ArrowUp) - key(KeyCode::ArrowDown), key(KeyCode::KeyQ) - key(KeyCode::KeyE), key(KeyCode::ArrowRight) - key(KeyCode::ArrowLeft));
        if input.button_down(MouseButton::Left) || input.button_down(MouseButton::Right) {
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
        let up = self.rot.inverse() * DVec3::Y;
        // (Turning about the craft's axes, by the right hand: about x, + takes the
        // nose (+z) down; about z, + takes the right side (+x) up. The arrows: up
        // lifts the nose, right drops the right side.)
        let mut want = DVec3::new(-command.x * 0.5, command.y * 0.5, -command.z * 0.5);
        if self.assist {
            // (Damp the turning; lean back toward level.)
            want -= body_spin * 1.5;
            want += DVec3::new(up.z, 0.0, -up.x) * 2.0;
            want = want.clamp(DVec3::splat(-1.0), DVec3::splat(1.0));
        }
        let torques: Vec<DVec3> = c.actuators.iter().map(|a| a.at.cross(a.dir * a.thrust)).collect();
        let most = |axis: DVec3| torques.iter().map(|t| t.dot(axis).abs()).fold(1e-9, f64::max);
        let (mx, my, mz) = (most(DVec3::X), most(DVec3::Y), most(DVec3::Z));
        c.actuators.iter().zip(&torques).map(|(a, t)| {
            let lifts = a.dir.y > 0.5;
            let base = if lifts { self.collective } else { 0.0 };
            // (A lifting one turns by a share of its collective's room; a side one by
            // all of its own.)
            let gain = if lifts { 0.35 } else { 1.0 };
            (base + gain * (want.x * t.x / mx + want.y * t.y / my + want.z * t.z / mz)).clamp(0.0, 1.0)
        }).collect()
    }

    fn step(&mut self, dt: f64, command: DVec3) {
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
        for (a, &t) in self.craft.actuators.iter().zip(&throttles) {
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
    }
}

/// The flight drawn: the ground's grid round it, the craft's boxes and members as
/// it lies, each actuator's push as a line the length of its throttle; and what it
/// reads.
pub fn draw(frame: &mut Frame, d: &Drive) {
    let size = frame.size();
    let sky = if d.density(d.pos.y) > 0.05 { Color([0.05, 0.09, 0.16, 1.0]) } else { Color([0.01, 0.01, 0.03, 1.0]) };
    frame.hud_rect(Vec2::ZERO, size, sky);
    let target = d.pos.as_vec3();
    let (sy, cy) = d.cam_yaw.sin_cos();
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
    // (The ground: a grid of 5 m round under the craft, to read speed and height by.)
    let ground = Color([0.25, 0.4, 0.3, 0.6]);
    let (cx, cz) = ((target.x / 5.0).round() * 5.0, (target.z / 5.0).round() * 5.0);
    for k in -12..=12 {
        let o = k as f32 * 5.0;
        seg(frame, Vec3::new(cx + o, 0.0, cz - 60.0), Vec3::new(cx + o, 0.0, cz + 60.0), ground);
        seg(frame, Vec3::new(cx - 60.0, 0.0, cz + o), Vec3::new(cx + 60.0, 0.0, cz + o), ground);
    }
    let place = |p: Vec3| (d.pos + d.rot * p.as_dvec3()).as_vec3();
    // (Its shadow: straight down.)
    seg(frame, target, Vec3::new(target.x, 0.0, target.z), Color([0.6, 0.6, 0.6, 0.4]));
    let part = Color([0.75, 0.6, 1.0, 0.85]);
    for (lo, hi) in &d.craft.boxes {
        let c = |x: bool, y: bool, z: bool| place(Vec3::new(if x { hi.x } else { lo.x }, if y { hi.y } else { lo.y }, if z { hi.z } else { lo.z }));
        for (a, b) in [((0, 0, 0), (1, 0, 0)), ((0, 0, 0), (0, 1, 0)), ((0, 0, 0), (0, 0, 1)), ((1, 1, 1), (0, 1, 1)), ((1, 1, 1), (1, 0, 1)), ((1, 1, 1), (1, 1, 0)), ((1, 0, 0), (1, 1, 0)), ((1, 0, 0), (1, 0, 1)), ((0, 1, 0), (1, 1, 0)), ((0, 1, 0), (0, 1, 1)), ((0, 0, 1), (1, 0, 1)), ((0, 0, 1), (0, 1, 1))] {
            seg(frame, c(a.0 == 1, a.1 == 1, a.2 == 1), c(b.0 == 1, b.1 == 1, b.2 == 1), part);
        }
    }
    for [a, b] in &d.craft.members {
        seg(frame, place(*a), place(*b), Color([1.0, 0.7, 0.3, 0.8]));
    }
    // (Each push: a line the other way from where it pushes, its throttle long.)
    for (a, &t) in d.craft.actuators.iter().zip(&d.throttles) {
        if t > 0.01 {
            let from = place(a.at.as_vec3());
            let to = place((a.at - a.dir * (0.5 + 3.0 * t)).as_vec3());
            seg(frame, from, to, Color([0.4, 0.8, 1.0, 0.9]));
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
        format!("ALTITUDE {:.1} M   CLIMB {:+.1} M/S   SPEED {:.1} M/S", d.pos.y, d.vel.y, DVec3::new(d.vel.x, 0.0, d.vel.z).length()),
        format!("PITCH {pitch:+.0}   ROLL {roll:+.0}   HEADING {heading:.0}   {}", if up_now.y < 0.0 { "UPSIDE DOWN" } else { "" }),
        format!("COLLECTIVE {:.0}%   ASSIST {}", d.collective * 100.0, if d.assist { "ON (H)" } else { "OFF (H)" }),
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
    frame.text_scaled(Vec2::new(12.0, size.y - 18.0), "W / S COLLECTIVE (X CUTS IT) - ARROWS PITCH AND ROLL - Q / E YAW - H ASSIST - Z AIR - R RESTART - DRAG TO LOOK, WHEEL NEARER - ESC BACK TO THE STUDIO", Color([0.75, 0.88, 1.0, 0.7]), 0.6);
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
