//! Actuation: what a pilot (any client) tells a ship's devices. The only
//! way anything flies a ship; the core clamps it to the hardware.

use serde::{Deserialize, Serialize};
use glam::DVec3;
use crate::ShipId;

/// Weapon triggers: held (true) or released.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Triggers {
    pub gun: bool,
    pub laser: bool,
}

/// Stick input for the attitude control, each axis -1..1 of the turn rate.
/// Positive pitch raises the nose, positive yaw turns left, positive roll banks left.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Controls {
    pub pitch: f64,
    pub yaw: f64,
    pub roll: f64,
}

/// What a ship's devices are told to do: the only way anything flies a ship.
/// Engine and thruster settings hold until commanded otherwise (see
/// `Ship::holding` for commands that change nothing).
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
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
    /// Weapon triggers. None: leave them as they are.
    pub weapons: Option<Triggers>,
    /// Master arm: combat mode on (weapons prime, then go hot) or off (safe).
    /// None: leave it as it is.
    pub arm: Option<bool>,
    /// Lay the gun: Some(Some(direction)) toward a world direction (as far
    /// as its gimbal reaches), Some(None) back along the nose. None: leave it.
    pub gun_target: Option<Option<DVec3>>,
    /// The anchor: fire it (hold to the asteroid within its reach) or let
    /// go. None: leave it as it is.
    #[serde(default)]
    pub anchor: Option<bool>,
    /// The excavator: on (digging, while anchored) or off. None: leave it.
    #[serde(default)]
    pub excavate: Option<bool>,
    /// A spaceport's hangar: in off the pad, or out onto pad `pad`. None: stay.
    #[serde(default)]
    pub hangar: Option<HangarCommand>,
    /// Flight systems on (drives, thrusters and the turn answer) or off
    /// (powered down, parked: only while landed). None: leave them.
    #[serde(default)]
    pub power: Option<bool>,
    /// The flight computer off (manual: each thruster fires only as the
    /// pilot holds it, nothing steadies the ship) or back on. None: leave it.
    #[serde(default)]
    pub manual: Option<bool>,
    /// In manual: the thrusters held firing (bit `k`: the hull's thruster
    /// `k`, at full). None: leave them as they are.
    #[serde(default)]
    pub jets: Option<u64>,
}

/// Moving between a spaceport's pads and its hangar (see `ShipCommands::hangar`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum HangarCommand {
    /// Off the pad it's landed on, into the port's hangar.
    Enter,
    /// Out of the hangar onto pad `pad` (one traffic control has given it).
    Leave { pad: usize },
}

/// Orders for the hyperdrive (see `hyperdrive`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct HyperdriveCommand {
    /// On (engage, or stay engaged) or off (disengage).
    pub engage: bool,
    /// Switch the drive on if it's off: the pilot's engage control. Orders
    /// without it only fly a drive that's engaged, so one arriving late,
    /// after the drive has dropped out, doesn't start it again.
    pub start: bool,
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
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Destination {
    pub point: DVec3,
    /// The body it is on or orbits.
    pub body: usize,
}

impl HyperdriveCommand {
    /// Stay engaged, flying along the nose, with nothing commanded.
    pub const CRUISE: Self = Self { engage: true, start: false, heading: None, steering: false, frame_velocity: None, exit_velocity: None, destination: None };
}

/// What a gunner (a client) tells a turret's gun: where to lay it (a world
/// direction; None: hold where it is) and whether the trigger is held. The
/// gun slews toward its aim at its hardware's rate and fires along wherever
/// it actually points.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct TurretCommand {
    pub aim: Option<DVec3>,
    pub fire: bool,
    /// Launch missiles at this ship (by id) as the launcher reloads; None: hold fire.
    #[serde(default)]
    pub launch: Option<ShipId>,
}
