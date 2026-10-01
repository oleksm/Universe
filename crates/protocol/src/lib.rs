//! The contracts between the core, the services and the clients (see
//! `docs/rearchitecture.md`): what crosses between them, and nothing else.
//!
//! - **Actuation** (client → core): a ship's device settings — engine,
//!   thrusters, turn rates, weapons, hyperdrive. The core clamps them to the
//!   hardware; settings hold until changed.
//! - **Stamps and causes**: everything that crosses carries the tick it's
//!   for (or was made at), and authoritative changes carry what caused them,
//!   so any outcome can be traced back.

use glam::DVec3;
use serde::{Deserialize, Serialize};

pub mod actuation;
pub mod traffic;

pub use actuation::{Controls, Destination, HyperdriveCommand, ShipCommands, Triggers};
pub use traffic::PadGrant;

/// A core tick's number. The core advances world time only in whole ticks.
pub type Tick = u64;

/// A body in the core (a ship, a projectile, a crate…).
pub type BodyId = usize;

/// A client (the player's, an NPC pilot, a turret gunner).
pub type ClientId = u64;

/// A message, unique per sender (with the sender: unique).
pub type MessageId = u64;

/// Something meant for (or made at) a tick.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Stamped<T> {
    pub tick: Tick,
    pub value: T,
}

/// Why an authoritative change happened: the core event or the message that
/// led to it (see accountability, `docs/rearchitecture.md` §5).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Cause {
    /// The world's rules on their own (seeded generation, a rail body's motion).
    Rules,
    /// A core event, by its tick and number in that tick's log.
    Event { tick: Tick, index: u32 },
    /// A message, by its sender and id.
    Message { sender: ClientId, id: MessageId },
}

/// A point and a direction in a star system's frame (for contracts that
/// talk about places without the core's types).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Ray {
    pub system: usize,
    pub origin: DVec3,
    pub direction: DVec3,
}
