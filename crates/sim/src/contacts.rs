//! Radar contacts for the player's ship: what its radar sees (a world
//! device), who each is (the crafts' transponders: name, what it's doing,
//! where it's bound), and the lock its avionics hold on one of them.

use universe_avionics::NavTarget;
use universe_world::radar::Blip;
use universe_world::ShipState;

use crate::traffic::Craft;

/// A ship on the radar, with its transponder's answer.
#[derive(Clone, Debug)]
pub struct Contact {
    /// Position, velocity and distance, as the radar measures them; `blip.id`
    /// is the craft's index.
    pub blip: Blip,
    pub name: String,
    /// What it's doing ("DOCKING", "HYPERDRIVE", ...).
    pub activity: &'static str,
    /// Where it's bound, if it says.
    pub destination: Option<String>,
    /// Hull integrity 0..1, as a combat scan reads it (shown in combat mode).
    pub hull: f64,
    /// Aggressed: fair game (it opened fire on someone).
    pub aggressed: bool,
}

/// Half-angle of the lock beam around the nose (rad): 6°.
pub const LOCK_BEAM: f64 = 6.0 * std::f64::consts::PI / 180.0;

/// What a craft's transponder says it's doing.
pub(crate) fn activity(craft: &Craft) -> &'static str {
    let a = &craft.status;
    // Weapons hot is plain to see, whatever the transponder says.
    if craft.ship.weapons_hot() {
        return "WEAPONS HOT";
    }
    match craft.ship.state {
        ShipState::Landed { .. } if a.dwelling => "AT STOP",
        ShipState::Landed { .. } => "PARKED",
        ShipState::Anchored { .. } => "MINING",
        _ if a.departing => "DEPARTING",
        _ if craft.ship.hyperdrive => "HYPERDRIVE",
        _ => match a.clearance.map(|c| c.target) {
            Some(NavTarget::Station(_)) => "DOCKING",
            Some(NavTarget::Spaceport(_)) => "LANDING",
            Some(NavTarget::Gate(_)) => "GATE RUN",
            Some(NavTarget::Asteroid(_)) => "UNDERWAY",
            None => "UNDERWAY",
        },
    }
}

