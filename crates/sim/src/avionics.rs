//! A ship's avionics state: what its navigation computer remembers — the nav
//! target, the clearance traffic control granted and the autopilot's phase.
//! None of it is physics or the world's business: the ship carries none of
//! it, and the world never reads it. The avionics learn what happened to the
//! ship from the world's physical events (`Avionics::observe`).

use serde::{Deserialize, Serialize};
use universe_world::ShipEvent;

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

/// One ship's avionics state.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Avionics {
    /// Target locked on the navigation map.
    #[serde(default)]
    pub nav_target: Option<NavTarget>,
    /// Clearance to dock, land or transit (as granted by traffic control), and the autopilot's state.
    #[serde(default)]
    pub clearance: Option<Clearance>,
    /// Hyperdrive autopilot: steer to the nav target (only when the pilot turns it on).
    #[serde(default)]
    pub hyper_autopilot: bool,
}

impl Avionics {
    /// The dock/land/gate autopilot is flying.
    pub fn autopilot_engaged(&self) -> bool {
        self.clearance.is_some_and(|c| c.autopilot)
    }

    /// What a physical event means for the navigation state: arriving,
    /// being destroyed or leaving through a gate ends a clearance; leaving
    /// the system forgets the target (it was in the old one); any change of
    /// the hyperdrive hands its steering back to the pilot; a new ship starts
    /// with nothing set.
    pub fn observe(&mut self, event: &ShipEvent) {
        match event {
            ShipEvent::Landed { .. } | ShipEvent::LandedAtPort { .. } | ShipEvent::Crashed { .. } => self.clearance = None,
            ShipEvent::GateEntered { .. } | ShipEvent::EnteredSystem { .. } => {
                self.clearance = None;
                self.nav_target = None;
            }
            ShipEvent::HyperdriveEngaged | ShipEvent::HyperdriveDisengaged => self.hyper_autopilot = false,
            ShipEvent::Respawned => *self = Avionics::default(),
            ShipEvent::TookOff
            | ShipEvent::Launched { .. }
            | ShipEvent::Bumped
            | ShipEvent::GateArrived { .. }
            | ShipEvent::GateTooFast { .. } => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn physical_events_end_clearances_and_forget_targets() {
        let target = NavTarget::Station(3);
        let set = || Avionics { nav_target: Some(target), clearance: Some(Clearance { target, autopilot: true, phase: Phase::Final }), hyper_autopilot: true };
        let after = |e: ShipEvent| {
            let mut a = set();
            a.observe(&e);
            a
        };
        let docked = after(ShipEvent::Landed { body: "Station".into(), station: true });
        assert!(docked.clearance.is_none() && docked.nav_target == Some(target), "arrived: the clearance is used up");
        let crashed = after(ShipEvent::Crashed { body: "Station".into() });
        assert!(crashed.clearance.is_none());
        let gone = after(ShipEvent::GateEntered { to: "Lave".into() });
        assert!(gone.clearance.is_none() && gone.nav_target.is_none(), "the target was in the old system");
        let dropped = after(ShipEvent::HyperdriveDisengaged);
        assert!(!dropped.hyper_autopilot && dropped.autopilot_engaged());
        assert!(after(ShipEvent::Bumped).autopilot_engaged(), "a bump changes nothing");
        let new = after(ShipEvent::Respawned);
        assert!(new.nav_target.is_none() && new.clearance.is_none() && !new.hyper_autopilot);
    }
}
