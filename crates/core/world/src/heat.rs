//! Heat: the hull's skin, an energy balance. In: the sunlight on its lit
//! side (none in a world's shadow), a near world's glow and reflected light,
//! a share of its own power use, and in air the flow's heating
//! (Sutton–Graves stagnation heating, from the air's density and the speed
//! through it) and exchange with the air. Out: radiation over all its area.
//! Its temperature follows the balance, slowed by its heat capacity: warm in
//! the sun, cold in shadow, the air's own when resting in it. A shallow entry
//! from orbit stays within the hull's limit; a steep dive, or flying low at
//! several times the speed of sound, doesn't: beyond `SKIN_LIMIT` the heat it
//! can't shed eats into the hull, and can burn the ship up.

use glam::DVec3;

use crate::damage::destroy;
use crate::events::ShipEvent;
use crate::ship::Ship;
use crate::sheet::{CONVECTION, HULL_ABSORPTIVITY, INTERNAL_LEAK};
use universe_physics::laws::AIR_CP;
use universe_physics::laws::STEFAN_BOLTZMANN as SIGMA;
// (Its constants are the world's sheet's: content/base/sheet.ron; σ is Dogma's.)
pub use crate::sheet::{AMBIENT, EMISSIVITY, HEATED_AREA, NOSE_RADIUS, RADIATING_AREA, SKIN_CAPACITY, SKIN_LIMIT};

/// Share of the heat beyond the limit that goes into damaging the hull (the
/// rest is carried off by what burns away).
const ABLATION: f64 = 0.05;

/// What a ship's skin is exposed to: starlight (W/m², none in shadow), a near
/// world's glow and reflected light (W/m²), and the air it's in, if any
/// (density kg/m³, its velocity, its temperature K).
#[derive(Clone, Copy, Debug, Default)]
pub struct Surroundings {
    pub sun: f64,
    pub world: f64,
    pub air: Option<(f64, DVec3, f64)>,
}

/// The skin over `dt` seconds in `around`. Burns the hull past the limit.
pub fn heat(ship: &mut Ship, around: &Surroundings, dt: f64, events: &mut Vec<ShipEvent>) {
    // (Long steps in short pieces: the balance is stiff in thick air.)
    let pieces = (dt / 2.0).ceil().max(1.0);
    for _ in 0..pieces as usize {
        heat_piece(ship, around, dt / pieces, events);
    }
}

fn heat_piece(ship: &mut Ship, around: &Surroundings, dt: f64, events: &mut Vec<ShipEvent>) {
    let t = ship.skin_temp;
    // About a quarter of a convex hull faces any one way.
    let facing = RADIATING_AREA * 0.25;
    let light = HULL_ABSORPTIVITY * around.sun * facing + EMISSIVITY * around.world * facing;
    let own = if ship.powered { INTERNAL_LEAK * ship.spec().power_draw } else { 0.0 };
    // In air: the flow can't heat the skin past its recovery temperature (the
    // air's own, plus what stopping it adds, v²/2cp): the Sutton–Graves rate
    // is for a cold wall, and scales with how far below that the skin is (a
    // skin hotter than that is cooled by the flow instead). And the air
    // trades heat with it, more the denser.
    let (flow, exchange) = around.air.map_or((0.0, 0.0), |(density, v, air_t)| {
        let speed = (ship.velocity - v).length();
        let recovery = air_t + speed * speed / (2.0 * AIR_CP);
        let cold = universe_physics::heat_flux(density, speed, NOSE_RADIUS) * HEATED_AREA;
        let flow = cold * (recovery - t) / (recovery - air_t).max(1.0);
        (flow, CONVECTION * (density / 1.225).sqrt() * RADIATING_AREA * (air_t - t))
    });
    let radiated = EMISSIVITY * SIGMA * RADIATING_AREA * t.powi(4);
    let mut temp = (t + (light + own + flow + exchange - radiated) / SKIN_CAPACITY * dt).max(3.0);
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
