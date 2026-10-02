//! The radar: a ship's sensor for other ships. It sees ships within range in
//! the same star system that are physically there, flying or resting on a
//! body. Ships in gate transit (between systems) or wrecked return nothing.
//! It measures position and velocity; who a contact is comes from its
//! transponder, which is not the radar's business.

use glam::DVec3;

use crate::ship::{Ship, ShipState};

/// How far the radar sees (m).
pub const RADAR_RANGE: f64 = 500_000.0;

/// One ship the radar sees.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Blip {
    /// The caller's id for the ship.
    pub id: usize,
    pub position: DVec3,
    pub velocity: DVec3,
    /// Distance from the radar (m).
    pub distance: f64,
}

impl Blip {
    /// Speed toward the radar at `own_position` moving at `own_velocity`
    /// (m/s; negative when opening).
    pub fn closing_speed(&self, own_position: DVec3, own_velocity: DVec3) -> f64 {
        let toward = (own_position - self.position).normalize_or_zero();
        (self.velocity - own_velocity).dot(toward)
    }
}

/// Whether a ship shows on radar at all (not inside a hangar).
fn visible(ship: &Ship) -> bool {
    matches!(ship.state, ShipState::Flying | ShipState::Landed { .. }) && ship.hangar.is_none()
}

/// Sweep for `others` (id, star system, ship) from `own` in `system`: every
/// ship in range, nearest first. Sees nothing while `own` itself isn't
/// in normal space.
pub fn sweep<'a>(own: &Ship, system: usize, others: impl IntoIterator<Item = (usize, usize, &'a Ship)>) -> Vec<Blip> {
    if !visible(own) {
        return Vec::new();
    }
    let mut blips: Vec<Blip> = others
        .into_iter()
        .filter(|(_, s, ship)| *s == system && visible(ship))
        .map(|(id, _, ship)| Blip { id, position: ship.position, velocity: ship.velocity, distance: ship.position.distance(own.position) })
        .filter(|b| b.distance <= RADAR_RANGE)
        .collect();
    blips.sort_by(|a, b| a.distance.total_cmp(&b.distance).then(a.id.cmp(&b.id)));
    blips
}
