//! Station docking: geometry, clearance, guidance numbers and the docking computer.
//!
//! A Coriolis station is a cuboctahedron spinning about its local +Y axis. The
//! docking slot sits in the +Y face, so the approach corridor runs along the
//! spin axis: it never moves sideways, but the slot rolls with the station and
//! the ship has to roll with it.

use glam::{DMat3, DQuat, DVec3};
use universe_physics::{segment_distance, Contact, CutOut, Feature, Frame, Polytope};

use crate::ship::{facing, Clearance, Controls, Phase, Ship, ROLL_RATE, TURN_RATE};
use crate::system::StarSystem;

/// Distance from the station center to its square faces (m). Also the render scale.
pub const STATION_SIZE: f64 = 500.0;
/// Slot half-extents in station units: long axis (local X) and short axis (local Z).
pub const SLOT_HALF: (f64, f64) = (0.3, 0.08);
/// Depth into the slot (station units from center) at which a ship counts as docked.
const DOCKED_DEPTH: f64 = 0.9;
/// Where the final approach starts, measured along the axis from the station center (m).
pub const APPROACH_HEIGHT: f64 = 4000.0;
/// Lowest point at which guidance joins the corridor (m from the station center).
pub const ENTRY_MIN: f64 = 1200.0;
/// Guidance paths keep at least this far from the station center (m).
const HULL_CLEARANCE: f64 = 1000.0;
/// Maximum range for granting clearance (m).
pub const CLEARANCE_RANGE: f64 = 50_000.0;
/// Fastest safe speed through the slot (m/s).
pub const MAX_DOCK_SPEED: f64 = 25.0;
/// Largest roll mismatch with the slot that still fits (radians).
pub const MAX_ROLL_ERROR: f64 = 0.52; // 30 degrees
/// Hull contact slower than this bounces instead of destroying the ship (m/s).
pub const BUMP_SPEED: f64 = 15.0;

/// A station's pose and motion at one instant, in the system frame.
#[derive(Clone, Copy, Debug)]
pub struct StationFrame {
    pub center: DVec3,
    pub velocity: DVec3,
    pub rotation: DQuat,
    pub angular_velocity: DVec3,
}

impl StationFrame {
    pub fn new(sys: &StarSystem, station: usize, t: f64, positions: &[DVec3]) -> Self {
        let f = Frame::of(&sys.bodies, station, t, positions);
        Self { center: f.center, velocity: f.velocity, rotation: f.rotation, angular_velocity: f.angular_velocity }
    }

    /// Docking axis: out of the slot, along the spin axis.
    pub fn axis(&self) -> DVec3 {
        self.rotation * DVec3::Y
    }

    /// The slot's long direction.
    pub fn slot_long(&self) -> DVec3 {
        self.rotation * DVec3::X
    }

    pub fn slot_short(&self) -> DVec3 {
        self.rotation * DVec3::Z
    }

    /// Point `height` meters out along the axis.
    pub fn on_axis(&self, height: f64) -> DVec3 {
        self.center + self.axis() * height
    }

    /// Velocity of the station's material at world point `p` (includes spin).
    pub fn velocity_at(&self, p: DVec3) -> DVec3 {
        self.velocity + self.angular_velocity.cross(p - self.center)
    }

    /// Ship orientation that points into the slot with wings along the slot,
    /// choosing whichever of the two fitting rolls is closer to `current`.
    pub fn docking_orientation(&self, current: DQuat) -> DQuat {
        let forward = -self.axis();
        let mut right = self.slot_long();
        if (current * DVec3::X).dot(right) < 0.0 {
            right = -right;
        }
        let up = right.cross(forward);
        DQuat::from_mat3(&DMat3::from_cols(right, up, -forward))
    }

    /// Angle between the ship's wings and the slot's long axis, 0..90 degrees (radians).
    pub fn roll_error(&self, orientation: DQuat) -> f64 {
        let axis = self.axis();
        let right = orientation * DVec3::X;
        let flat = right - axis * right.dot(axis);
        match flat.try_normalize() {
            Some(flat) => flat.dot(self.slot_long()).abs().clamp(0.0, 1.0).acos(),
            None => std::f64::consts::FRAC_PI_2,
        }
    }
}

/// The station's real shape, for the physics kernel: a cuboctahedron (the
/// intersection of a cube, |x|,|y|,|z| <= 1, and an octahedron,
/// |x|+|y|+|z| <= 2), with the slot cut into its +Y face. Reaching the
/// slot's floor is the docking port's contact; the mouth above it is open.
pub fn hull() -> Polytope {
    Polytope::cuboctahedron(STATION_SIZE).with_cut_out(CutOut { half_x: SLOT_HALF.0, half_z: SLOT_HALF.1, floor: DOCKED_DEPTH })
}

/// Hull contact gentle enough to bounce off instead of being destroyed.
pub fn bounces(contact: &Contact) -> bool {
    contact.feature == Feature::Hull && contact.relative_velocity.length() < BUMP_SPEED
}

/// The docking port: reaching the slot's floor slowly and lined up with the
/// slot docks; anything else there is a crash. `roll_error` is the ship's
/// (see `StationFrame::roll_error`).
pub fn docks(contact: &Contact, roll_error: f64) -> bool {
    matches!(contact.feature, Feature::CutOut(_)) && contact.relative_velocity.length() < MAX_DOCK_SPEED && roll_error < MAX_ROLL_ERROR
}

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

/// Stick commands that turn the ship toward `target`, also matching an angular
/// velocity `spin` (world frame), e.g. a station's rotation.
pub(crate) fn attitude(ship: &Ship, target: DQuat, spin: DVec3) -> Controls {
    let mut err = (ship.orientation.inverse() * target).normalize();
    if err.w < 0.0 {
        err = -err;
    }
    let rate = err.to_scaled_axis() * 1.5 + ship.orientation.inverse() * spin;
    Controls {
        pitch: (rate.x / TURN_RATE).clamp(-1.0, 1.0),
        yaw: (rate.y / TURN_RATE).clamp(-1.0, 1.0),
        roll: (rate.z / ROLL_RATE).clamp(-1.0, 1.0),
    }
}

/// The docking computer. Translates with RCS only (no flip-and-burn needed) and
/// steers toward either the station or the docking orientation.
pub fn autopilot(frame: &StationFrame, ship: &Ship, phase: Phase) -> Command {
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
    let accel = ((v_des - v) * 1.2).clamp_length_max(ship.side_accel());
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
    let controls = attitude(ship, target, spin);
    Command { controls, throttle: 0.0, rcs, phase, attitude: target }
}
