//! Heat: the hull's skin in air. Moving fast through an atmosphere, its
//! leading surfaces are heated by the compressed air ahead of them
//! (Sutton–Graves stagnation heating, from the air's density and the speed
//! through it); the skin radiates heat away over all of its area. Its
//! temperature follows the balance, slowed by its heat capacity. A shallow
//! entry from orbit stays within the hull's limit; a steep dive, or flying
//! low at several times the speed of sound, doesn't: beyond `SKIN_LIMIT` the
//! heat it can't shed eats into the hull, and can burn the ship up.

use glam::DVec3;

use crate::damage::destroy;
use crate::events::ShipEvent;
use crate::ship::Ship;

/// Share of the heat beyond the limit that goes into damaging the hull (the
/// rest is carried off by what burns away).
const ABLATION: f64 = 0.05;
use crate::sheet::AIR_CP;
use universe_physics::laws::STEFAN_BOLTZMANN as SIGMA;
// (Its constants are the physics sheet's: config/dogma.ron.)
pub use crate::sheet::{AMBIENT, EMISSIVITY, HEATED_AREA, NOSE_RADIUS, RADIATING_AREA, SKIN_CAPACITY, SKIN_LIMIT};

/// The skin over `dt` seconds, flying through air of `density` (kg/m³)
/// moving at `air` (none: in vacuum). Burns the hull past the limit.
pub fn heat(ship: &mut Ship, air: Option<(f64, DVec3)>, dt: f64, events: &mut Vec<ShipEvent>) {
    if ship.skin_temp <= AMBIENT && air.is_none() {
        ship.skin_temp = AMBIENT;
        return;
    }
    let t = ship.skin_temp;
    // The flow can't heat the skin past its recovery temperature (the air's
    // own, plus what stopping it adds, v²/2cp): the Sutton–Graves rate is for
    // a cold wall, and scales with how far below that the skin is. (A skin
    // hotter than that is cooled by the flow instead.)
    let heating = air.map_or(0.0, |(density, v)| {
        let speed = (ship.velocity - v).length();
        let recovery = AMBIENT + speed * speed / (2.0 * AIR_CP);
        let cold = universe_physics::heat_flux(density, speed, NOSE_RADIUS) * HEATED_AREA;
        cold * (recovery - t) / (recovery - AMBIENT).max(1.0)
    });
    let cooling = EMISSIVITY * SIGMA * RADIATING_AREA * (t.powi(4) - AMBIENT.powi(4));
    let mut temp = t + (heating - cooling) / SKIN_CAPACITY * dt;
    if temp < AMBIENT && heating <= 0.0 {
        temp = AMBIENT;
    }
    if temp > SKIN_LIMIT {
        // What it can't hold at the limit burns into the hull.
        let excess = (temp - SKIN_LIMIT) * SKIN_CAPACITY;
        temp = SKIN_LIMIT;
        ship.hull = (ship.hull - excess * ABLATION / ship.spec().hull_strength).max(0.0);
        if ship.hull <= 0.0 {
            destroy(ship, "RE-ENTRY HEAT", events);
        }
    }
    ship.skin_temp = temp;
}

#[cfg(test)]
mod tests {
    use glam::DQuat;

    use super::*;

    fn ship_at(speed: f64) -> Ship {
        Ship::new(DVec3::ZERO, DVec3::X * speed, DQuat::IDENTITY)
    }

    /// The skin's temperature after `seconds` at `speed` through air of `density`.
    fn soak(speed: f64, density: f64, seconds: f64) -> Ship {
        let mut s = ship_at(speed);
        let mut events = Vec::new();
        for _ in 0..(seconds * 60.0) as usize {
            heat(&mut s, Some((density, DVec3::ZERO)), 1.0 / 60.0, &mut events);
        }
        s
    }

    #[test]
    fn a_shallow_entry_stays_cool_enough_a_steep_one_burns() {
        let air = universe_physics::Atmosphere::earthlike(9.81);
        // Orbital speed high up (~80 km): hot, but within the limit.
        let high = soak(7_800.0, air.density(80_000.0), 120.0);
        assert!(high.skin_temp > 600.0 && high.skin_temp < SKIN_LIMIT && high.hull == 1.0, "{} K", high.skin_temp);
        // The same speed down at 40 km: burnt up.
        let low = soak(7_800.0, air.density(40_000.0), 60.0);
        assert!(low.hull < 0.5, "hull {}", low.hull);
    }

}
