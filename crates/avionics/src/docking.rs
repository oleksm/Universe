//! Station docking guidance: the approach corridor, guidance numbers and the
//! docking computer. (The station itself, its frame and its docking port are
//! the world's: `universe_world::station`.)
//!
//! The docking slot sits in the station's +Y face, so the approach corridor
//! runs along the spin axis: it never moves sideways, but the slot rolls with
//! the station and the ship has to roll with it.

use glam::{DQuat, DVec3};
use universe_physics::segment_distance;
use universe_world::ship::{facing, ROLL_RATE, TURN_RATE};
use universe_world::station::{MAX_ROLL_ERROR, SLOT_HALF, STATION_SIZE};
use universe_world::{Controls, Ship, ShipCommands, StationFrame};

use crate::nav::{Clearance, Phase};

/// Where the final approach starts, measured along the axis from the station center (m).
pub const APPROACH_HEIGHT: f64 = 4000.0;
/// Lowest point at which guidance joins the corridor (m from the station center).
pub const ENTRY_MIN: f64 = 1200.0;
/// Guidance paths keep at least this far from the station center (m).
const HULL_CLEARANCE: f64 = 1000.0;

/// Numbers for the docking HUD.
#[derive(Clone, Copy, Debug)]
pub struct DockingStatus {
    pub phase: Phase,
    pub autopilot: bool,
    /// Straight-line distance to the slot entrance (m).
    pub range: f64,
    /// Distance out along the docking axis from the station center (m); negative = behind.
    pub height: f64,
    /// Distance from the docking axis (m).
    pub offset: f64,
    /// Speed toward the station along the axis (m/s); positive = closing.
    pub closing: f64,
    /// Total speed relative to the station (m/s).
    pub speed: f64,
    /// Recommended top speed here (m/s).
    pub speed_limit: f64,
    pub roll_error: f64,
    /// Inside the approach corridor and lined up for the slot.
    pub in_corridor: bool,
    /// Ship velocity relative to the station (m/s).
    pub relative_velocity: DVec3,
    pub guidance: Guidance,
}

/// Recommended speed toward the slot at a given height along the axis.
pub fn speed_limit(height: f64) -> f64 {
    ((height - STATION_SIZE) / 20.0).clamp(8.0, 150.0)
}

/// Half-width of the approach corridor at a given height (m): the slot, flaring outward.
pub fn corridor_half(height: f64) -> (f64, f64) {
    let flare = (height - STATION_SIZE).max(0.0) * 0.12;
    (SLOT_HALF.0 * STATION_SIZE + flare, SLOT_HALF.1 * STATION_SIZE + flare)
}

pub fn status(frame: &StationFrame, ship: &Ship, docking: &Clearance) -> DockingStatus {
    let axis = frame.axis();
    let r = ship.position - frame.center;
    let v = ship.velocity - frame.velocity;
    let height = r.dot(axis);
    let lateral = r - axis * height;
    let roll_error = frame.roll_error(ship.orientation);
    let (half_long, half_short) = corridor_half(height);
    let in_corridor = height > 0.0
        && lateral.dot(frame.slot_long()).abs() < half_long
        && lateral.dot(frame.slot_short()).abs() < half_short
        && roll_error < MAX_ROLL_ERROR;
    DockingStatus {
        phase: docking.phase,
        autopilot: docking.autopilot,
        range: (ship.position - frame.on_axis(STATION_SIZE)).length(),
        height,
        offset: lateral.length(),
        closing: -v.dot(axis),
        speed: v.length(),
        speed_limit: speed_limit(height),
        roll_error,
        in_corridor,
        relative_velocity: v,
        guidance: guidance(
            frame,
            ship.position,
            if docking.autopilot { docking.phase == Phase::Final } else { in_final_zone(frame, ship.position) },
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
    /// On the final run down the corridor.
    pub final_run: bool,
}

/// Guidance from the ship's position. `final_run` selects the corridor run;
/// otherwise we route to the approach point, going around the station if needed.
/// `accel` is the thruster acceleration available (m/s^2); it sets how fast it's safe to go.
pub fn guidance(frame: &StationFrame, pos: DVec3, final_run: bool, accel: f64) -> Guidance {
    let axis = frame.axis();
    let r = pos - frame.center;
    let height = r.dot(axis);
    let lateral = r - axis * height;
    let lat_dist = lateral.length();
    let lat_dir = lateral.try_normalize().unwrap_or_else(|| frame.slot_long());

    if final_run {
        let along = -speed_limit(height) * 0.8;
        let correct = (-lateral * 0.25).clamp_length_max(10.0);
        return Guidance {
            desired_velocity: axis * along + correct,
            waypoint: frame.on_axis((height - 400.0).max(STATION_SIZE)),
            waypoint_dir: -axis,
            final_run: true,
        };
    }
    // Join the corridor at our own height (between ENTRY_MIN and the approach
    // height), so the path leads mostly toward the station rather than away.
    let entry = frame.on_axis(height.clamp(ENTRY_MIN, APPROACH_HEIGHT));
    let clear_path = segment_distance(pos, entry, frame.center) > HULL_CLEARANCE;
    let (waypoint, waypoint_dir) = if clear_path {
        (entry, -axis)
    } else if lat_dist < 2500.0 {
        // Behind or beside the hull: first move out sideways.
        (frame.center + lat_dir * 3000.0 + axis * height.max(0.0), axis)
    } else {
        // Out to the side but the straight line would clip the hull: climb first.
        let above = frame.center + lat_dir * lat_dist + axis * ENTRY_MIN.max(height);
        (above, (entry - above).normalize_or(-axis))
    };
    let d = waypoint - pos;
    let dist = d.length();
    let top = (2.0 * accel * 0.25 * dist).sqrt().min(250.0);
    let desired_velocity = if dist > 1.0 { d / dist * top.min(dist * 0.5) } else { DVec3::ZERO };
    Guidance { desired_velocity, waypoint, waypoint_dir, final_run: false }
}

/// For a pilot flying by hand: are we already lined up in the corridor?
pub fn in_final_zone(frame: &StationFrame, pos: DVec3) -> bool {
    let axis = frame.axis();
    let r = pos - frame.center;
    let height = r.dot(axis);
    let lateral = r - axis * height;
    let (half_long, _) = corridor_half(height);
    height > STATION_SIZE && height < APPROACH_HEIGHT + 300.0 && lateral.length() < half_long
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
        ShipCommands { throttle: self.throttle, rcs: self.rcs, turn: Some(self.controls), hyperdrive: None, weapons: None }
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
pub(crate) fn attitude(ship: &Ship, target: DQuat, spin: DVec3, h: f64) -> Controls {
    let mut err = (ship.orientation.inverse() * target).normalize();
    if err.w < 0.0 {
        err = -err;
    }
    let rate = err.to_scaled_axis() * gain(1.5, h) + ship.orientation.inverse() * spin;
    Controls {
        pitch: (rate.x / TURN_RATE).clamp(-1.0, 1.0),
        yaw: (rate.y / TURN_RATE).clamp(-1.0, 1.0),
        roll: (rate.z / ROLL_RATE).clamp(-1.0, 1.0),
    }
}

/// The docking computer. Translates with RCS only (no flip-and-burn needed) and
/// steers toward either the station or the docking orientation. Its command
/// holds for `h` seconds, until it next reacts.
pub fn autopilot(frame: &StationFrame, ship: &Ship, phase: Phase, h: f64) -> Command {
    let axis = frame.axis();
    let r = ship.position - frame.center;
    let v = ship.velocity - frame.velocity;
    let height = r.dot(axis);
    let lateral = r - axis * height;
    let lat_dist = lateral.length();

    // Phase transitions. The entry point is where the corridor is joined.
    let entry = guidance(frame, ship.position, false, ship.side_accel());
    let on_axis = lat_dist < 60.0 && height >= ENTRY_MIN - 60.0;
    let to_entry = if on_axis { 0.0 } else { entry.waypoint.distance(ship.position) };
    let aligned = frame.roll_error(ship.orientation) < 0.05
        && (ship.orientation * DVec3::NEG_Z).dot(-axis) > 0.998
        && ship.angular_velocity.length() < 0.15;
    let phase = match phase {
        Phase::Approach if to_entry < 60.0 && v.length() < 5.0 => Phase::Align,
        Phase::Align if to_entry < 60.0 && v.length() < 3.0 && aligned => Phase::Final,
        Phase::Final if height < 0.0 || lat_dist > 250.0 => Phase::Approach,
        p => p,
    };

    let v_des = guidance(frame, ship.position, phase == Phase::Final, ship.side_accel()).desired_velocity;
    let accel = ((v_des - v) * gain(1.2, h)).clamp_length_max(ship.side_accel());
    let rcs = ship.thruster_command(ship.orientation.inverse() * accel);

    // Attitude: look at the station while travelling, then line up with the slot.
    let (target, spin) = match phase {
        Phase::Approach => {
            // Roll: keep the ship's top along the station's axis (out of the slot).
            let look = (frame.center - ship.position).normalize();
            (facing(look, frame.axis()), DVec3::ZERO)
        }
        _ => (frame.docking_orientation(ship.orientation), frame.angular_velocity),
    };
    let controls = attitude(ship, target, spin, h);
    Command { controls, throttle: 0.0, rcs, phase, attitude: target }
}
