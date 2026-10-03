//! The hyper layer's laws (invented, by written rule: `docs/physics.md`):
//! the medium's slack, a field's draw and the speed a power buys, a throat's
//! upkeep. Devices built on them (hyperdrives, gates, relays) are the world's.

use crate::laws::{FIELD_COST, HYPER_RATE, SPEED_OF_LIGHT, TUBE_EPS, TUBE_GAMMA, TUBE_HOLD, TUBE_K, TUBE_RHO, TUBE_T_LY, V_BEST_C, V_TOP_C};

/// The medium's limit on speed `d` metres from the nearest surface (m/s).
pub fn limit(d: f64) -> f64 {
    HYPER_RATE * d
}

/// The energy a field holding `mass` kg takes to go a metre at `speed`,
/// through a device of efficiency `eta` (J/m): from the tank.
pub fn field_cost(mass: f64, speed: f64, eta: f64) -> f64 {
    mass * FIELD_COST * (1.0 + (speed / (V_BEST_C * SPEED_OF_LIGHT)).powi(2)) / eta
}

/// A field's top speed (m/s).
pub fn field_top() -> f64 {
    V_TOP_C * SPEED_OF_LIGHT
}

/// A light year (m), for the tube's per-light-year law.
const LY: f64 = 9.460_730_472_580_8e15;

/// A tube's natural crossing time (s) for `mass` kg over a span of `span` m.
pub fn tube_natural_time(mass: f64, span: f64) -> f64 {
    TUBE_T_LY * (span / LY) * mass.max(1e-9).powf(TUBE_GAMMA)
}

/// The energy (J) to take `mass` kg through a tube `span` m long in `time` s:
/// past the natural time it's steeply dearer; slower, never under `eps·m·S`.
pub fn tube_crossing_energy(mass: f64, span: f64, time: f64) -> f64 {
    TUBE_EPS * mass * span * (tube_natural_time(mass, span) / time.max(1e-12)).min(700.0).exp()
}

/// A tube of `diameter` m: its equivalent mass (kg).
pub fn tube_mass(diameter: f64) -> f64 {
    TUBE_RHO * diameter.powf(TUBE_K)
}

/// Opening a tube of `diameter` m over `span` m at its natural pace (J).
pub fn tube_open_energy(diameter: f64, span: f64) -> f64 {
    let mu = tube_mass(diameter);
    tube_crossing_energy(mu, span, tube_natural_time(mu, span))
}

/// Holding it open (W): its opening over `TUBE_HOLD`.
pub fn tube_hold_power(diameter: f64, span: f64) -> f64 {
    tube_open_energy(diameter, span) / TUBE_HOLD
}
