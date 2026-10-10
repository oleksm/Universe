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

pub use actuation::{Controls, Destination, HangarCommand, HyperdriveCommand, ShipCommands, Triggers, TurretCommand};
pub use traffic::PadGrant;

/// A core tick's number. The core advances world time only in whole ticks.
pub type Tick = u64;

/// A ship, or anything that shoots and is shot: the player's is 0, NPC craft `i` is `i + 1`, a
/// port's turret from `TURRETS` up. Written as its number (logs, saves).
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Debug, Default, serde::Serialize, serde::Deserialize)]
#[serde(transparent)]
pub struct ShipId(pub usize);

impl ShipId {
    /// The player's ship.
    pub const PLAYER: ShipId = ShipId(0);

    /// NPC craft `i`'s.
    pub const fn craft(i: usize) -> ShipId {
        ShipId(i + 1)
    }

    /// The player's.
    pub const fn is_player(self) -> bool {
        self.0 == 0
    }

    /// Where turrets' ids start.
    pub const TURRETS: usize = 1 << 40;

    /// Which NPC craft it is (None: the player's, or a turret).
    pub const fn craft_index(self) -> Option<usize> {
        if self.0 >= Self::TURRETS { None } else { self.0.checked_sub(1) }
    }

    /// Turret `k` of `system`'s.
    pub const fn turret(system: usize, k: usize) -> ShipId {
        ShipId(Self::TURRETS + system * 256 + k)
    }

    /// The system and index of the turret it is, if it is one.
    pub const fn turret_of(self) -> Option<(usize, usize)> {
        if self.0 >= Self::TURRETS { Some(((self.0 - Self::TURRETS) / 256, (self.0 - Self::TURRETS) % 256)) } else { None }
    }
}

impl std::fmt::Display for ShipId {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

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
