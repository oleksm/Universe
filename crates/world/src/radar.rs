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

/// Whether a ship shows on radar at all.
fn visible(ship: &Ship) -> bool {
    matches!(ship.state, ShipState::Flying | ShipState::Landed { .. })
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

#[cfg(test)]
mod tests {
    use glam::DQuat;

    use super::*;

    fn ship_at(x: f64) -> Ship {
        Ship::new(DVec3::new(x, 0.0, 0.0), DVec3::ZERO, DQuat::IDENTITY)
    }

    #[test]
    fn sees_ships_in_range_in_the_same_system_nearest_first() {
        let own = ship_at(0.0);
        let far = ship_at(RADAR_RANGE * 1.5);
        let near = ship_at(2_000.0);
        let mid = ship_at(50_000.0);
        let elsewhere = ship_at(1_000.0);
        let mut wreck = ship_at(500.0);
        wreck.state = ShipState::Destroyed { respawn_in: 5.0 };
        let blips = sweep(&own, 7, [(0, 7, &far), (1, 7, &mid), (2, 7, &near), (3, 8, &elsewhere), (4, 7, &wreck)]);
        let ids: Vec<usize> = blips.iter().map(|b| b.id).collect();
        assert_eq!(ids, [2, 1]);
        assert_eq!(blips[0].distance, 2_000.0);
    }

    #[test]
    fn closing_speed_is_positive_when_approaching() {
        let own = ship_at(0.0);
        let mut other = ship_at(10_000.0);
        other.velocity = DVec3::new(-30.0, 5.0, 0.0);
        let b = sweep(&own, 0, [(0, 0, &other)])[0];
        assert!((b.closing_speed(own.position, own.velocity) - 30.0).abs() < 1e-9);
    }
}
