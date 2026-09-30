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
/// Pitch and yaw rate at full stick (rad/s).
pub const TURN_RATE: f64 = 1.0;
pub const ROLL_RATE: f64 = 1.8;
/// Touching a surface slower than this lands instead of crashing (m/s).
pub const LAND_SPEED: f64 = 30.0;
/// Collision radius (m).
pub const SHIP_RADIUS: f64 = 12.0;
/// Hyperdrive speed = this * distance to the nearest obstacle, per second.
pub const HYPER_RATE: f64 = 2.0;
pub const LIFT_THRUST: f64 = 2.25e6;

fn full_tank() -> f64 {
    FUEL_CAPACITY
}

/// Something in the current system you can dock or land at.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum NavTarget {
    /// Index into the system's bodies.
    Station(usize),
    /// Index into the system's spaceports.
    Spaceport(usize),
    /// Index into the system's bodies (a ring gate).
    Gate(usize),
}

/// Autopilot phases for docking and landing.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Phase {
    /// Heading for the corridor entry / the point above the pad.
    #[default]
    Approach,
    /// Docking: holding at the corridor entry, matching the station's roll.
    Align,
    /// Docking: down the corridor into the slot.
    Final,
    /// Landing: belly down, descending vertically onto the pad.
    Descent,
}

impl Phase {
    pub fn label(self) -> &'static str {
        match self {
            Phase::Approach => "APPROACH",
            Phase::Align => "ALIGN",
            Phase::Final => "FINAL",
            Phase::Descent => "DESCENT",
        }
    }
}

/// Permission to dock or land at a target, and the autopilot's state.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct Clearance {
    pub target: NavTarget,
    pub autopilot: bool,
    pub phase: Phase,
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
    /// 0..1.
    pub throttle: f64,
    pub hyperdrive: bool,
    pub state: ShipState,
    /// Translation thrusters, body frame, each axis -1..1 (x right, y up, z back).
    #[serde(default)]
    pub rcs: DVec3,
    /// Clearance to dock or land (and autopilot state), if granted.
    #[serde(default)]
    pub clearance: Option<Clearance>,
    /// Target locked on the navigation map.
    #[serde(default)]
    pub nav_target: Option<NavTarget>,
    /// Hyperdrive autopilot: steer to the nav target (only when the pilot turns it on).
    #[serde(default)]
    pub hyper_autopilot: bool,
    /// Fuel on board (kg).
    #[serde(default = "full_tank")]
    pub fuel: f64,
    /// Cargo on board (kg).
    #[serde(default)]
    pub cargo: f64,
}

/// Pilot stick input, each axis -1..1.
/// Positive pitch raises the nose, positive yaw turns left, positive roll banks left.
#[derive(Clone, Copy, Debug, Default)]
pub struct Controls {
    pub pitch: f64,
    pub yaw: f64,
    pub roll: f64,
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
            clearance: None,
            nav_target: None,
            hyper_autopilot: false,
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

    /// Rotate toward the commanded rates. Uses real time, never warped time.
    pub(crate) fn steer(&mut self, c: &Controls, real_dt: f64) {
        let target = DVec3::new(c.pitch * TURN_RATE, c.yaw * TURN_RATE, c.roll * ROLL_RATE);
        let k = 1.0 - (-6.0 * real_dt).exp();
        self.angular_velocity += (target - self.angular_velocity) * k;
        self.orientation = (self.orientation * DQuat::from_scaled_axis(self.angular_velocity * real_dt)).normalize();
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
