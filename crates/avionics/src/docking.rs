//! Station docking guidance: the approach to a pad on the station's deck,
//! guidance numbers and the docking computer. (The station itself, its
//! frame and its deck are the world's: `universe_world::station`.)
//!
//! The deck faces the station's +Y; a ship cleared to dock has a pad of
//! its own. It comes in to a point straight above its pad (around the
//! station if need be), turns belly to the deck with its nose toward the
//! main structure, and sets down on the pad, slowly: there's no gravity to
//! fight here, only the station's slow turn to match.

use glam::{DQuat, DVec3};
use universe_world::ship::facing;
use universe_world::spaceport::{CENTER_PAD, GRID};
use universe_world::station::{DECK_HALF, DECK_SPEED, STATION_SIZE};
use universe_world::{Controls, Ship, ShipCommands, StationFrame};

use crate::nav::{Clearance, PadSlot, Phase};

/// Where the final descent starts, at most, above the pad (m).
pub const APPROACH_HEIGHT: f64 = 2000.0;
/// Lowest point above the pad at which guidance joins its vertical (m):
/// well clear of the main structure's top.
pub const ENTRY_MIN: f64 = 500.0;
/// Paths keep at least this far off the station's solid (m).
const CLEARANCE: f64 = 150.0;
/// The station's solid, in its own frame, for path checks (m).
const SOLID: (DVec3, DVec3) = (DVec3::new(-DECK_HALF, -150.0, -375.0), DVec3::new(DECK_HALF, 150.0, 375.0));

/// The pad a clearance is for (the middle one if it has none yet).
pub fn pad_of(slot: PadSlot) -> usize {
    match slot {
        PadSlot::Pad(k) => k,
        _ => CENTER_PAD,
    }
}

/// Numbers for the docking HUD.
#[derive(Clone, Copy, Debug)]
pub struct DockingStatus {
    pub phase: Phase,
    pub autopilot: bool,
    pub pad: usize,
    /// Straight-line distance to the pad (m).
    pub range: f64,
    /// Height above the pad (m): where a ship resting on it is; negative = below.
    pub height: f64,
    /// Distance from the pad's vertical (m).
    pub offset: f64,
    /// Speed down toward the deck (m/s); positive = closing.
    pub closing: f64,
    /// Total speed relative to the station (m/s).
    pub speed: f64,
    /// Recommended top speed here (m/s).
    pub speed_limit: f64,
    /// Above its pad, belly to the deck: set to come straight down.
    pub lined_up: bool,
    /// Ship velocity relative to the station (m/s).
    pub relative_velocity: DVec3,
    pub guidance: Guidance,
}

/// Recommended speed down toward the pad at a height above it: slow at the
/// deck (well under what it takes).
pub fn speed_limit(height: f64) -> f64 {
    (height / 15.0).clamp(DECK_SPEED * 0.2, 60.0)
}

/// Where the ship is relative to pad `pad`: (height above it, offset from its vertical, as a vector).
fn over(frame: &StationFrame, pad: usize, pos: DVec3) -> (f64, DVec3) {
    let up = frame.up();
    let r = pos - frame.pad(pad);
    let height = r.dot(up);
    (height, r - up * height)
}

/// Belly to the deck, the nose within ~10° of the landing heading, steady.
fn upright_on(frame: &StationFrame, ship: &Ship, tolerance: f64) -> bool {
    let landing = frame.landing_orientation();
    (ship.orientation * DVec3::Y).dot(frame.up()) > tolerance && (ship.orientation * DVec3::NEG_Z).dot(landing * DVec3::NEG_Z) > tolerance
}

pub fn status(frame: &StationFrame, ship: &Ship, docking: &Clearance) -> DockingStatus {
    let pad = pad_of(docking.pad);
    let v = ship.velocity - frame.velocity_at(ship.position);
    let (height, lateral) = over(frame, pad, ship.position);
    let lined_up = height > 0.0 && lateral.length() < 40.0 + height * 0.05 && upright_on(frame, ship, 0.97);
    DockingStatus {
        phase: docking.phase,
        autopilot: docking.autopilot,
        pad,
        range: ship.position.distance(frame.pad(pad)),
        height,
        offset: lateral.length(),
        closing: -v.dot(frame.up()),
        speed: v.length(),
        speed_limit: speed_limit(height),
        lined_up,
        relative_velocity: v,
        guidance: guidance(
            frame,
            pad,
            ship.position,
            if docking.autopilot { docking.phase == Phase::Final } else { in_final_zone(frame, pad, ship.position) },
            ship.side_accel(),
        ),
    }
}

/// Where to go next and how fast: the same answer for the HUD and the docking computer.
#[derive(Clone, Copy, Debug)]
pub struct Guidance {
    /// Velocity the ship should have, relative to the station (m/s).
    pub desired_velocity: DVec3,
    /// Next point to fly through (system frame).
    pub waypoint: DVec3,
    /// Direction to be travelling when passing the waypoint.
    pub waypoint_dir: DVec3,
    /// On the final descent onto the pad.
    pub final_run: bool,
}

/// Does the segment `a`–`b` (station frame) pass within `CLEARANCE` of the station's solid?
fn blocked(a: DVec3, b: DVec3) -> bool {
    let (lo, hi) = (SOLID.0 - DVec3::splat(CLEARANCE), SOLID.1 + DVec3::splat(CLEARANCE));
    let d = b - a;
    let (mut t0, mut t1) = (0.0f64, 1.0f64);
    for i in 0..3 {
        if d[i].abs() < 1e-9 {
            if a[i] < lo[i] || a[i] > hi[i] {
                return false;
            }
        } else {
            let (u, w) = ((lo[i] - a[i]) / d[i], (hi[i] - a[i]) / d[i]);
            t0 = t0.max(u.min(w));
            t1 = t1.min(u.max(w));
            if t0 > t1 {
                return false;
            }
        }
    }
    true
}

/// Guidance from the ship's position, for pad `pad`. `final_run` selects
/// the descent; otherwise it routes to the point above the pad, around
/// the station if needed. `accel` is the thruster acceleration available
/// (m/s²); it sets how fast it's safe to go.
pub fn guidance(frame: &StationFrame, pad: usize, pos: DVec3, final_run: bool, accel: f64) -> Guidance {
    let up = frame.up();
    let (height, lateral) = over(frame, pad, pos);
    let above = |h: f64| frame.pad(pad) + up * h;
    if final_run {
        let down = -speed_limit(height) * 0.8;
        let correct = (-lateral * 0.25).clamp_length_max(10.0);
        return Guidance { desired_velocity: up * down + correct, waypoint: above((height - 200.0).max(0.0)), waypoint_dir: -up, final_run: true };
    }
    // Join the pad's vertical at our own height (between ENTRY_MIN and
    // APPROACH_HEIGHT), so the path leads mostly toward the station.
    let entry = above(height.clamp(ENTRY_MIN, APPROACH_HEIGHT));
    let to_local = |p: DVec3| frame.rotation.inverse() * (p - frame.center);
    let here = to_local(pos);
    let (waypoint, waypoint_dir) = if !blocked(here, to_local(entry)) {
        (entry, -up)
    } else {
        // Below or beside the station: out sideways first, then up past it.
        let flat = DVec3::new(here.x, 0.0, here.z);
        let out = flat.try_normalize().unwrap_or(DVec3::Z) * (STATION_SIZE + 600.0);
        if flat.length() < STATION_SIZE + 300.0 {
            (frame.world(DVec3::new(out.x, here.y, out.z)), frame.rotation * flat.normalize_or(DVec3::Z))
        } else {
            (frame.world(DVec3::new(here.x, SOLID.1.y + ENTRY_MIN, here.z)), up)
        }
    };
    let d = waypoint - pos;
    let dist = d.length();
    let top = (2.0 * accel * 0.25 * dist).sqrt().min(250.0);
    let desired_velocity = if dist > 1.0 { d / dist * top.min(dist * 0.5) } else { DVec3::ZERO };
    Guidance { desired_velocity, waypoint, waypoint_dir, final_run: false }
}

/// For a pilot flying by hand: already above the pad, near its vertical?
pub fn in_final_zone(frame: &StationFrame, pad: usize, pos: DVec3) -> bool {
    let (height, lateral) = over(frame, pad, pos);
    height > 0.0 && height < APPROACH_HEIGHT + 300.0 && lateral.length() < 60.0 + height * 0.05
}

/// What an autopilot wants this instant.
pub struct Command {
    pub controls: Controls,
    /// Main engine, 0..1.
    pub throttle: f64,
    /// Translation thruster command in the ship's body frame, each axis -1..1.
    pub rcs: DVec3,
    pub phase: Phase,
    /// The orientation the autopilot is turning toward (where the pilot should face).
    pub attitude: DQuat,
}

impl Command {
    /// As commands for the ship's devices.
    pub fn commands(&self) -> ShipCommands {
        ShipCommands { throttle: self.throttle, rcs: self.rcs, turn: Some(self.controls), hyperdrive: None, weapons: None, arm: None, gun_target: None, anchor: None, excavate: None, hangar: None, power: None }
    }
}

/// A feedback gain `k` (1/s) for a controller whose command holds for `h`
/// seconds: never more than closes the whole error within that time, or it
/// overshoots and swings. (Flying, the autopilots react every 0.05 s and
/// this is just `k`; a flight planner looking far ahead takes longer steps.)
pub(crate) fn gain(k: f64, h: f64) -> f64 {
    k.min(1.0 / h)
}

/// Stick commands that turn the ship toward `target`, also matching an angular
/// velocity `spin` (world frame), e.g. a station's rotation, held for `h` seconds.
pub fn attitude(ship: &Ship, target: DQuat, spin: DVec3, h: f64) -> Controls {
    let mut err = (ship.orientation.inverse() * target).normalize();
    if err.w < 0.0 {
        err = -err;
    }
    // Toward the target at a rate the thrusters can still stop from in the
    // angle left (√(2αθ), with a margin), each axis by its own envelope.
    let e = err.to_scaled_axis();
    let alpha = ship.turn_accel() * 0.6;
    let k = gain(1.5, h);
    let toward = |e: f64, a: f64| e.signum() * (e.abs() * k).min((2.0 * a * e.abs()).sqrt());
    let rate = DVec3::new(toward(e.x, alpha.x), toward(e.y, alpha.y), toward(e.z, alpha.z)) + ship.orientation.inverse() * spin;
    let s = ship.spec();
    Controls {
        pitch: (rate.x / s.turn_rate).clamp(-1.0, 1.0),
        yaw: (rate.y / s.turn_rate).clamp(-1.0, 1.0),
        roll: (rate.z / s.roll_rate).clamp(-1.0, 1.0),
    }
}

/// The docking computer, for pad `pad`. Translates with the thrusters only
/// (no flip-and-burn needed) and steers toward the station, then upright
/// over the pad. Its command holds for `h` seconds, until it next reacts.
pub fn autopilot(frame: &StationFrame, ship: &Ship, phase: Phase, pad: usize, h: f64) -> Command {
    let v = ship.velocity - frame.velocity_at(ship.position);
    let (height, lateral) = over(frame, pad, ship.position);

    // Phase transitions. The entry point is where the pad's vertical is joined.
    let entry = guidance(frame, pad, ship.position, false, ship.side_accel());
    let on_line = lateral.length() < 30.0 && height >= ENTRY_MIN - 60.0;
    let to_entry = if on_line { 0.0 } else { entry.waypoint.distance(ship.position) };
    let steady = ship.angular_velocity.length() < 0.1;
    let phase = match phase {
        Phase::Approach | Phase::Hold if to_entry < 60.0 && v.length() < 5.0 => Phase::Align,
        Phase::Align if to_entry < 60.0 && v.length() < 3.0 && upright_on(frame, ship, 0.995) && steady => Phase::Final,
        Phase::Final if height < -SHIP_CLEAR || lateral.length() > 150.0 => Phase::Approach,
        Phase::Hold => Phase::Approach,
        p => p,
    };

    let v_des = guidance(frame, pad, ship.position, phase == Phase::Final, ship.side_accel()).desired_velocity;
    let accel = ((v_des - v) * gain(1.2, h)).clamp_length_max(ship.side_accel());
    let rcs = ship.thruster_command(ship.orientation.inverse() * accel);

    // Attitude: look at the station while travelling, then upright over the pad.
    let target = match phase {
        Phase::Approach => facing((frame.center - ship.position).normalize(), frame.up()),
        _ => frame.landing_orientation(),
    };
    let controls = attitude(ship, target, frame.angular_velocity, h);
    Command { controls, throttle: 0.0, rcs, phase, attitude: target }
}

/// How far below the pad's rest height a ship can be on its final run
/// before it's clearly not landing there (m).
const SHIP_CLEAR: f64 = 30.0;

/// Waiting for a pad: holding at place `n` of a ring above the deck.
pub fn hold(frame: &StationFrame, n: usize, ship: &Ship, h: f64) -> Command {
    let (ring, k) = (n / 12, n % 12);
    let a = (k as f64 + 0.5 * ring as f64) * std::f64::consts::TAU / 12.0;
    let r = 900.0 + 300.0 * ring as f64;
    let place = frame.world(DVec3::new(a.cos() * r, SOLID.1.y + ENTRY_MIN + 200.0 * ring as f64, a.sin() * r));
    let v = ship.velocity - frame.velocity_at(ship.position);
    let d = place - ship.position;
    let desired = d.clamp_length_max((d.length() * 0.05).min(120.0));
    let accel = ((desired - v) * gain(1.0, h)).clamp_length_max(ship.side_accel());
    let rcs = ship.thruster_command(ship.orientation.inverse() * accel);
    let target = facing((frame.center - ship.position).normalize(), frame.up());
    let controls = attitude(ship, target, DVec3::ZERO, h);
    Command { controls, throttle: 0.0, rcs, phase: Phase::Hold, attitude: target }
}

/// The pads, row by row: how many to a row (for the HUD's pad map).
pub const PAD_ROW: usize = GRID;

#[cfg(test)]
mod tests {
    use super::*;
    use universe_world::Ship;

    /// A 180° turn about `axis`, at `throttle`: seconds until within 5°, and
    /// the worst it strays after.
    fn flip(throttle: f64, axis: DVec3) -> (f64, f64) {
        let mut s = Ship::new(DVec3::ZERO, DVec3::ZERO, DQuat::IDENTITY);
        s.throttle = throttle;
        let target = DQuat::from_axis_angle(axis, std::f64::consts::PI * 0.999);
        let (h, mut t, mut worst, mut settled) = (1.0 / 60.0, 0.0, 0.0f64, None);
        while t < 15.0 {
            let c = attitude(&s, target, DVec3::ZERO, h);
            s.drive(Some(&c), h, true);
            t += h;
            let off = (s.orientation * DVec3::NEG_Z).angle_between(target * DVec3::NEG_Z).to_degrees();
            if settled.is_none() && off < 5.0 {
                settled = Some(t);
            }
            if settled.is_some() {
                worst = worst.max(off);
            }
        }
        (settled.unwrap_or(f64::INFINITY), worst)
    }

    #[test]
    fn a_flip_turns_round_in_seconds_on_the_thrusters_and_settles() {
        for axis in [DVec3::Y, DVec3::X] {
            for throttle in [0.0, 1.0] {
                let (t, worst) = flip(throttle, axis);
                assert!(t < 5.0 && worst < 6.0, "{axis} at throttle {throttle}: {t:.1} s, strays {worst:.1}°");
            }
        }
    }
}
