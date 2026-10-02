//! The hyper layer's laws (invented, by written rule: `docs/physics.md`):
//! the medium's slack, a field's draw and the speed a power buys, a throat's
//! upkeep. Devices built on them (hyperdrives, gates, relays) are the world's.

use crate::laws::{GATE_P0, GATE_S0, GATE_TAU, HYPER_BIT_MASS, HYPER_RATE, MAX_TRANSIT_SPEED, P_FLOOR, P_PUSH, SPEED_OF_LIGHT, TRANSIT_TIME, V_BEST_C, V_OPEN_C};

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

/// The power holding a throat open across `span_ly` light years takes (W).
pub fn throat_upkeep(span_ly: f64) -> f64 {
    GATE_P0 * (span_ly / GATE_S0).powi(3)
}

/// A transit's energy for `mass` kg across a throat `span` metres long (J).
pub fn transit_energy(mass: f64, span: f64) -> f64 {
    GATE_TAU * mass * span
}

/// A hyper-signal is a field round each bit, held while it crosses: the
/// energy (J) a relay of efficiency `eta` spends sending one bit `length`
/// metres where the slack is `s`, at the push's best speed (`v*`). Deep in
/// a system (`s` near 0) next to nothing; between stars every bit pays the wall.
pub fn bit_energy(s: f64, length: f64, eta: f64) -> f64 {
    HYPER_BIT_MASS * s * (P_FLOOR + P_PUSH) * length / (V_BEST_C * SPEED_OF_LIGHT) / eta
}

/// What a relay of `power` W at `eta` can send `length` m through slack `s`
/// (bits a second): its power over each bit's energy.
pub fn relay_throughput(power: f64, eta: f64, s: f64, length: f64) -> f64 {
    power * eta / bit_energy(s, length, eta.max(1e-9)).max(1e-30)
}

/// A throat's length (m): what matter crosses in `TRANSIT_TIME` under `MAX_TRANSIT_SPEED`.
pub fn throat_length() -> f64 {
    MAX_TRANSIT_SPEED * TRANSIT_TIME
}

/// A signal through a throat (s): its length at light speed.
pub fn throat_signal_time() -> f64 {
    throat_length() / SPEED_OF_LIGHT
}
