//! The navigation computer's vocabulary: what the ship is heading for (the
//! nav target), the clearance traffic control granted for it, and the
//! autopilot's phase.

use serde::{Deserialize, Serialize};

/// Something in the current system you can dock or land at, or fly through:
/// the world's `Facility`, as a navigation target.
pub use universe_world::traffic::Facility as NavTarget;

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
    /// Landing: all pads taken; waiting in the holding ring above the port.
    Hold,
}

impl Phase {
    pub fn label(self) -> &'static str {
        match self {
            Phase::Approach => "APPROACH",
            Phase::Align => "ALIGN",
            Phase::Final => "FINAL",
            Phase::Descent => "DESCENT",
            Phase::Hold => "HOLDING",
        }
    }
}

/// Permission to dock or land at a target, and the autopilot's state.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Clearance {
    pub target: NavTarget,
    pub autopilot: bool,
    pub phase: Phase,
    /// For a spaceport: the pad traffic control gave (or the place in the
    /// holding ring, waiting for one).
    #[serde(default)]
    pub pad: PadSlot,
}

/// Where at a spaceport a clearance is for.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum PadSlot {
    /// The port's middle pad (no pad booked).
    #[default]
    Center,
    /// This pad.
    Pad(usize),
    /// Waiting in the holding ring, this many ahead.
    Hold(usize),
}
