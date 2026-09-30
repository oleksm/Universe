//! Ships: a rigid body with devices (main engine, thrusters and lift,
//! attitude control, hyperdrive), fuel and cargo.
//!
//! Whatever flies a ship — a pilot's hands or a flight computer — does it
//! through `ShipCommands`, and the devices turn those into forces within their
//! limits. The ship carries no navigation state: no target, no clearance.

use glam::{DMat3, DQuat, DVec3};
use serde::{Deserialize, Serialize};
use universe_physics::RigidBody;

/// Ship mass without fuel or cargo (kg).
pub const DRY_MASS: f64 = 60_000.0;
/// Fuel tank capacity (kg).
pub const FUEL_CAPACITY: f64 = 30_000.0;
/// Main engine thrust (N): 30 m/s^2 (about 3 g) at full load.
pub const MAIN_THRUST: f64 = 2.7e6;
/// Translation thrusters (RCS), per axis (N): 6 m/s^2 at full load.
pub const RCS_THRUST: f64 = 5.4e5;
/// Belly lift thrusters, pushing along the ship's +Y (N): 25 m/s^2 at full load, enough to hover.
pub const LIFT_THRUST: f64 = 2.25e6;
/// Pitch and yaw rate at full stick (rad/s).
pub const TURN_RATE: f64 = 1.0;
pub const ROLL_RATE: f64 = 1.8;
/// Collision radius (m).
pub const SHIP_RADIUS: f64 = 12.0;

fn full_tank() -> f64 {
    FUEL_CAPACITY
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum ShipState {
    Flying,
    /// Resting on a body, stored in that body's rotating frame.
    Landed { body: usize, local_position: DVec3, local_orientation: DQuat },
    Destroyed { respawn_in: f64 },
    /// Between gates. The ship's motion relative to the entry gate is kept and
    /// reapplied at the paired gate on arrival.
    Transit {
        /// Galaxy index of the system we're going to, and the one we came from.
        to: usize,
        from: usize,
        remaining: f64,
        local_velocity: DVec3,
        local_offset: DVec3,
        local_orientation: DQuat,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Ship {
    /// Relative to the star of the ship's current system (m).
    pub position: DVec3,
    pub velocity: DVec3,
    /// Local -Z is forward, +Y is up.
    pub orientation: DQuat,
    /// Body-frame angular velocity (x = pitch, y = yaw, z = roll), rad/s.
    pub angular_velocity: DVec3,
    /// Main engine setting, 0..1, as last commanded.
    pub throttle: f64,
    /// The hyperdrive is engaged.
    pub hyperdrive: bool,
    pub state: ShipState,
    /// Translation thruster setting, body frame, each axis -1..1 (x right, y up, z back), as last commanded.
    #[serde(default)]
    pub rcs: DVec3,
    /// Fuel on board (kg).
    #[serde(default = "full_tank")]
    pub fuel: f64,
    /// Cargo on board (kg).
    #[serde(default)]
    pub cargo: f64,
}

/// Stick input for the attitude control, each axis -1..1 of the turn rate.
/// Positive pitch raises the nose, positive yaw turns left, positive roll banks left.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Controls {
    pub pitch: f64,
    pub yaw: f64,
    pub roll: f64,
}

/// What a ship's devices are told to do: the only way anything flies a ship.
/// Engine and thruster settings hold until commanded otherwise (see
/// `Ship::holding` for commands that change nothing).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ShipCommands {
    /// Main engine, 0..1.
    pub throttle: f64,
    /// Translation thrusters (+Y: the lift thrusters), body frame, each axis -1..1.
    pub rcs: DVec3,
    /// Attitude control: turn at these rates over the span the commands cover.
    /// None: no turn is commanded (the attitude isn't worked this span).
    pub turn: Option<Controls>,
    /// Orders for the hyperdrive (engage, disengage, or how to fly while
    /// engaged). None: leave it as it is.
    pub hyperdrive: Option<HyperdriveCommand>,
}

/// Orders for the hyperdrive (see `hyperdrive`).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct HyperdriveCommand {
    /// On (engage, or stay engaged) or off (disengage).
    pub engage: bool,
    /// Direction of travel. None: wherever the nose points.
    pub heading: Option<DVec3>,
    /// The heading is being steered around obstacles (by whoever commands
    /// it), so the drive doesn't drop out on its own short of one dead ahead.
    pub steering: bool,
    /// Velocity of the reference frame the drive moves relative to (m/s).
    /// None: the frame of the body whose gravity dominates.
    pub frame_velocity: Option<DVec3>,
    /// On dropping out, the ship is left moving with this velocity (m/s).
    /// None: co-moving with the body whose gravity dominates.
    pub exit_velocity: Option<DVec3>,
    /// Where the drive is headed: it slows down so as to stop there (the stop
    /// distance is the distance to it), and the body it is on or near doesn't
    /// count as an obstacle while the approach to it is clear.
    pub destination: Option<Destination>,
}

/// A commanded hyperdrive destination.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Destination {
    pub point: DVec3,
    /// The body it is on or orbits.
    pub body: usize,
}

impl HyperdriveCommand {
    /// Stay engaged, flying along the nose, with nothing commanded.
    pub const CRUISE: Self = Self { engage: true, heading: None, steering: false, frame_velocity: None, exit_velocity: None, destination: None };
}

impl Ship {
    pub fn new(position: DVec3, velocity: DVec3, orientation: DQuat) -> Self {
        Self {
            position,
            velocity,
            orientation,
            angular_velocity: DVec3::ZERO,
            throttle: 0.0,
            hyperdrive: false,
            state: ShipState::Flying,
            rcs: DVec3::ZERO,
            fuel: FUEL_CAPACITY,
            cargo: 0.0,
        }
    }

    /// Total mass right now (kg).
    pub fn mass(&self) -> f64 {
        DRY_MASS + self.fuel + self.cargo
    }

    /// Main engine acceleration at full throttle (m/s^2): thrust / mass.
    pub fn main_accel(&self) -> f64 {
        MAIN_THRUST / self.mass()
    }

    /// Translation thruster acceleration, per axis (m/s^2).
    pub fn side_accel(&self) -> f64 {
        RCS_THRUST / self.mass()
    }

    /// Lift thruster acceleration (m/s^2).
    pub fn lift_accel(&self) -> f64 {
        LIFT_THRUST / self.mass()
    }

    /// Body-frame acceleration from a thruster command (each axis -1..1).
    pub fn thruster_accel(&self, rcs: DVec3) -> DVec3 {
        let c = rcs.clamp(DVec3::splat(-1.0), DVec3::ONE);
        let (side, lift) = (self.side_accel(), self.lift_accel());
        DVec3::new(c.x * side, c.y * if c.y > 0.0 { lift } else { side }, c.z * side)
    }

    /// The thruster command that best produces a body-frame acceleration.
    pub fn thruster_command(&self, accel: DVec3) -> DVec3 {
        let (side, lift) = (self.side_accel(), self.lift_accel());
        let up = if accel.y > 0.0 { lift } else { side };
        DVec3::new(accel.x / side, accel.y / up, accel.z / side).clamp(DVec3::splat(-1.0), DVec3::ONE)
    }

    pub fn forward(&self) -> DVec3 {
        self.orientation * DVec3::NEG_Z
    }

    pub fn up(&self) -> DVec3 {
        self.orientation * DVec3::Y
    }

    pub fn is_flying(&self) -> bool {
        matches!(self.state, ShipState::Flying)
    }

    /// The ship as the physics kernel sees it.
    pub fn rigid(&self) -> RigidBody {
        RigidBody { position: self.position, velocity: self.velocity, orientation: self.orientation, angular_velocity: self.angular_velocity, radius: SHIP_RADIUS }
    }

    /// Take the kernel's word for where the ship is and how it moves.
    pub fn set_rigid(&mut self, body: &RigidBody) {
        self.position = body.position;
        self.velocity = body.velocity;
        self.orientation = body.orientation;
        self.angular_velocity = body.angular_velocity;
    }

    /// Commands that keep the engine and thrusters as they are, turn nothing
    /// and leave the hyperdrive alone: a starting point for new commands.
    pub fn holding(&self) -> ShipCommands {
        ShipCommands { throttle: self.throttle, rcs: self.rcs, turn: None, hyperdrive: None }
    }

    /// The main engine and thrusters take their new settings.
    pub(crate) fn set_controls(&mut self, c: &ShipCommands) {
        self.throttle = c.throttle;
        self.rcs = c.rcs;
    }

    /// Acceleration from the main engine and thrusters as set (m/s^2): thrust / current mass.
    pub fn thrust(&self) -> DVec3 {
        self.forward() * (self.main_accel() * self.throttle) + self.orientation * self.thruster_accel(self.rcs)
    }

    /// Attitude control: rotate toward the commanded rates over `dt` seconds.
    pub(crate) fn steer(&mut self, c: &Controls, dt: f64) {
        let target = DVec3::new(c.pitch * TURN_RATE, c.yaw * TURN_RATE, c.roll * ROLL_RATE);
        let k = 1.0 - (-6.0 * dt).exp();
        self.angular_velocity += (target - self.angular_velocity) * k;
        self.orientation = (self.orientation * DQuat::from_scaled_axis(self.angular_velocity * dt)).normalize();
    }
}

/// Orientation with the nose exactly along `forward` and the ship's top as close
/// as possible to `up_hint`. A fixed reference keeps the roll steady instead of
/// drifting with whatever the last turn left behind.
pub fn facing(forward: DVec3, up_hint: DVec3) -> DQuat {
    let f = forward.normalize();
    let right = f.cross(up_hint).try_normalize().unwrap_or_else(|| f.any_orthonormal_vector());
    let up = right.cross(f);
    DQuat::from_mat3(&DMat3::from_cols(right, up, -f))
}

/// Orientation with `up` along `normal` and forward as close as possible to `forward`.
pub fn upright(normal: DVec3, forward: DVec3) -> DQuat {
    let tangent = (forward - normal * forward.dot(normal)).try_normalize().unwrap_or_else(|| normal.any_orthonormal_vector());
    let right = tangent.cross(normal);
    DQuat::from_mat3(&DMat3::from_cols(right, normal, -tangent))
}
