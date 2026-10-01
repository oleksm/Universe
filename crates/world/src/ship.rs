//! Ships: a rigid body with devices (main engine, thrusters and lift,
//! attitude control, hyperdrive, gun and laser), a hull, fuel, ammunition and
//! cargo.
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
/// Drag coefficient × frontal area (m²): a blunt 90 t ship falls at about
/// 150 m/s through sea-level air.
pub const DRAG_AREA: f64 = 60.0;

fn full_tank() -> f64 {
    FUEL_CAPACITY
}

fn intact() -> f64 {
    1.0
}

fn full_magazine() -> u32 {
    crate::weapons::GUN_AMMO
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
    /// Cargo on board (kg): the mass of what's in the hold.
    #[serde(default)]
    pub cargo: f64,
    /// The hold: units of each item (by catalog id; see `market`).
    #[serde(default)]
    pub hold: std::collections::BTreeMap<usize, u32>,
    /// Hull integrity, 1 (intact) .. 0 (destroyed): see `damage`.
    #[serde(default = "intact")]
    pub hull: f64,
    /// Gun rounds left.
    #[serde(default = "full_magazine")]
    pub ammo: u32,
    /// Laser heat, 0 (cold) .. 1 (too hot to fire).
    #[serde(default)]
    pub laser_heat: f64,
    /// The laser overheated and is locked out until it has cooled (see `weapons::LASER_RESET`).
    #[serde(default)]
    pub laser_overheated: bool,
    /// Combat mode: the master arm is on (see `weapons`). The weapons are
    /// hot once `arming` has run down.
    #[serde(default)]
    pub armed: bool,
    /// Seconds until the weapons are primed.
    #[serde(default)]
    pub arming: f64,
    /// Weapon triggers, as last commanded.
    #[serde(skip)]
    pub triggers: Triggers,
    /// Seconds until the gun can fire again.
    #[serde(skip)]
    pub gun_cooldown: f64,
    /// Where the gun points, in the ship's frame (unit; -Z is the nose). Its
    /// gimbal swings it a few degrees off the nose (see `weapons`).
    #[serde(skip, default = "boresight")]
    pub gun_dir: DVec3,
    /// Where the gun is to be laid, in the world (unit), as last commanded.
    /// None: along the nose.
    #[serde(skip)]
    pub gun_target: Option<DVec3>,
    /// How the hyperdrive flies while engaged, as last ordered (held like any
    /// device setting).
    #[serde(skip, default = "cruise")]
    pub hyper_orders: HyperdriveCommand,
    /// The hull's skin temperature (K): see `heat`.
    #[serde(default = "skin_ambient")]
    pub skin_temp: f64,
    /// Seconds the hyperdrive stays jammed (hits disrupt it: see `damage::HYPER_JAM`).
    #[serde(skip)]
    pub hyper_jam: f64,
}

fn boresight() -> DVec3 {
    DVec3::NEG_Z
}

pub use universe_protocol::{Controls, Destination, HyperdriveCommand, ShipCommands, Triggers};

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
            hold: Default::default(),
            hull: 1.0,
            ammo: crate::weapons::GUN_AMMO,
            laser_heat: 0.0,
            laser_overheated: false,
            armed: false,
            arming: 0.0,
            triggers: Triggers::default(),
            gun_cooldown: 0.0,
            gun_dir: DVec3::NEG_Z,
            gun_target: None,
            hyper_jam: 0.0,
            skin_temp: crate::heat::AMBIENT,
            hyper_orders: HyperdriveCommand::CRUISE,
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
        // Drag in air (none in the hyperdrive's field).
        let ballistic = if self.hyperdrive { 0.0 } else { self.mass() / DRAG_AREA };
        RigidBody { position: self.position, velocity: self.velocity, orientation: self.orientation, angular_velocity: self.angular_velocity, radius: SHIP_RADIUS, ballistic }
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
        ShipCommands { throttle: self.throttle, rcs: self.rcs, turn: None, hyperdrive: None, weapons: None, arm: None, gun_target: None }
    }

    /// The main engine and thrusters take their new settings.
    pub(crate) fn set_controls(&mut self, c: &ShipCommands) {
        self.throttle = c.throttle;
        self.rcs = c.rcs;
        if let Some(t) = c.weapons {
            self.triggers = t;
        }
        if let Some(g) = c.gun_target {
            self.gun_target = g;
        }
    }

    /// Where the gun points, in the world.
    pub fn gun_forward(&self) -> DVec3 {
        self.orientation * self.gun_dir
    }

    /// Weapons can fire: combat mode on, primed.
    pub fn weapons_hot(&self) -> bool {
        self.armed && self.arming <= 0.0
    }

    /// Acceleration from the main engine and thrusters as set (m/s^2): thrust / current mass.
    pub fn thrust(&self) -> DVec3 {
        self.forward() * (self.main_accel() * self.throttle) + self.orientation * self.thruster_accel(self.rcs)
    }

    /// Attitude control: rotate toward the commanded rates over `dt` seconds.
    pub fn steer(&mut self, c: &Controls, dt: f64) {
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

fn skin_ambient() -> f64 {
    crate::heat::AMBIENT
}

fn cruise() -> HyperdriveCommand {
    HyperdriveCommand::CRUISE
}
