//! The hyper layer's laws (invented, by written rule: `docs/physics.md`):
//! the medium's slack, a field's draw and the speed a power buys, a throat's
//! upkeep. Devices built on them (hyperdrives, gates, relays) are the world's.

use crate::laws::{HYPER_RATE, P_FLOOR, P_PUSH, SPEED_OF_LIGHT, TUBE_EPS, TUBE_GAMMA, TUBE_HOLD, TUBE_K, TUBE_RHO, TUBE_T_LY, V_BEST_C, V_OPEN_C};

/// The medium's slack `d` metres from the nearest surface: 0 stiff (deep in
/// a system), 1 slack (between the stars).
pub fn slack(d: f64) -> f64 {
    ((HYPER_RATE * d.max(0.0)) / (V_OPEN_C * SPEED_OF_LIGHT)).powi(2).min(1.0)
}

/// The medium's limit on speed `d` metres from the nearest surface (m/s).
pub fn limit(d: f64) -> f64 {
    HYPER_RATE * d
}

/// The power a field holding `mass` kg at `speed` draws where the slack is
/// `s`, through a device of efficiency `eta` (W).
pub fn field_draw(mass: f64, s: f64, speed: f64, eta: f64) -> f64 {
    mass * s * (P_FLOOR + P_PUSH * (speed / (V_BEST_C * SPEED_OF_LIGHT)).powi(3)) / eta
}

/// The fastest a field holding `mass` kg can go where the slack is `s`, on
/// `power` W through a device of efficiency `eta` (m/s); `None` if it can't
/// be held at all.
pub fn field_speed(mass: f64, s: f64, power: f64, eta: f64) -> Option<f64> {
    let usable = power * eta;
    let hold = mass * s * P_FLOOR;
    if hold > usable {
        return None;
    }
    if s <= 0.0 {
        return Some(f64::INFINITY);
    }
    Some(V_BEST_C * SPEED_OF_LIGHT * ((usable - hold) / (mass * s * P_PUSH)).cbrt())
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
