//! Shared rate controller and bounded actuator allocation for rigid craft.
//! All vectors use one caller-selected right-handed body frame; spin is NOT
//! world angular velocity. No navigation, target attitude or input-device state.
use glam::{DMat3, DVec3};
use crate::ship::Thruster;

/// Physical state at the start of the span. Inertia is about `com` and must be
/// invertible with positive principal moments; mass must be positive.
#[derive(Clone, Copy, Debug)]
pub struct Body {
    pub mass: f64,
    /// Same origin/frame as the actuator positions (zero for COM-centred craft).
    pub com: DVec3,
    pub inertia: DMat3,
    /// Angular velocity in body coordinates, rad/s.
    pub spin: DVec3,
}

#[derive(Clone, Copy, Debug)]
pub struct Demand {
    /// Body force in newtons; caller supplies translation/collective policy.
    pub force: DVec3,
    /// Body rates in rad/s. None means no turn correction; Some(ZERO) brakes spin.
    pub want_rates: Option<DVec3>,
    /// Some fixes Main-role nozzles together at this fraction; None lets all
    /// actuators participate freely. This also preserves live RCS warm-start decay.
    pub drive: Option<f64>,
    /// False shuts every jet off (e.g. live ship has no propellant).
    pub enabled: bool,
}

#[derive(Clone, Copy, Debug)]
pub struct Output {
    /// Achieved force and torque, not demand; caller applies linear dynamics.
    pub force: DVec3,
    pub torque: DVec3,
    /// Body spin after this span, including the legacy rate overshoot clamp.
    pub spin: DVec3,
}

/// Normalized per-axis command to rates; limits are nonnegative rad/s from the
/// caller's flight computer. Axis meaning belongs to its body-frame adapter.
pub fn rates(input: DVec3, limits: DVec3) -> DVec3 {
    input.clamp(DVec3::splat(-1.0), DVec3::ONE) * limits
}

/// Live translation policy in the canonical ship frame (-Z nose, +Y lift,
/// +X right). Returns desired force and grouped main-drive fraction. Authority
/// is the caller's balanced main/lift/side force, not raw summed thrust.
/// Use a basis adapter for a Studio craft with a different forward axis.
pub fn force_demand(throttle: f64, translation: DVec3, authority: crate::ship::Authority, main_thrust: f64, push: bool) -> (DVec3, f64) {
    if !push { return (DVec3::ZERO, 0.0); }
    let c = translation.clamp(DVec3::splat(-1.0), DVec3::ONE);
    let force = DVec3::NEG_Z * (throttle.clamp(0.0, 1.0) * authority.main)
        + DVec3::new(c.x * authority.side, c.y * if c.y > 0.0 { authority.lift } else { authority.side }, c.z * authority.side);
    let level = throttle.clamp(0.0, 1.0) * authority.main / main_thrust.max(1.0);
    (force, level)
}

/// One assisted-control span (dt >= 0). `actuators` carry effective current
/// thrust limits, including caller-owned fuel/power/air derating. Their order
/// must match persistent `jets`, which is the allocator's warm-start state.
/// Fractions are 0..1; the existing allocator and its main/RCS/lift semantics
/// are shared unchanged. Resource consumption is the caller's responsibility.
///
/// Preserves live approximation: no gyroscopic coupling; per-axis rate crossing
/// is clamped after allocation. Caller updates body-to-world orientation using
/// (orientation * Quat::from_scaled_axis(output.spin * dt)).normalize().
pub fn step(actuators: &[Thruster], body: Body, demand: Demand, dt: f64, jets: &mut Vec<f64>) -> Output {
    let torque = demand.want_rates.map_or(DVec3::ZERO, |want| {
        body.inertia * ((want - body.spin) / dt.max(crate::ship::TURN_RESPONSE))
    });
    let (force, torque) = if !demand.enabled || (demand.force == DVec3::ZERO && torque.length_squared() < 1e-6) {
        jets.iter_mut().for_each(|j| *j = 0.0);
        (DVec3::ZERO, DVec3::ZERO)
    } else {
        crate::thrusters::allocate_with(actuators, body.com, body.mass, body.inertia, demand.drive, demand.force, torque, jets)
    };
    Output { force, torque, spin: integrate_rates(body.inertia, body.spin, torque, demand.want_rates, dt) }
}

/// Integrate the torque actually delivered after resource limits. No gyroscopic
/// coupling, matching live flight; None disables assisted rate-crossing clamps.
pub fn integrate_rates(inertia: DMat3, spin: DVec3, torque: DVec3, want_rates: Option<DVec3>, dt: f64) -> DVec3 {
    let mut next = spin + inertia.inverse() * torque * dt;
    if let Some(want) = want_rates {
        let toward = |w: f64, n: f64, t: f64| if (t - w) * (t - n) < 0.0 { t } else { n };
        next = DVec3::new(toward(spin.x, next.x, want.x), toward(spin.y, next.y, want.y), toward(spin.z, next.z, want.z));
    }
    next
}

/// Unassisted live firing: bits select the first 64 actuators at full effective
/// thrust. No allocation, rate braking or main-drive grouping. Studio may pass
/// already derated actuators, or integrate its realized torque separately.
pub fn manual(actuators: &[Thruster], body: Body, held: u64, enabled: bool, dt: f64, jets: &mut Vec<f64>) -> Output {
    jets.resize(actuators.len(), 0.0);
    let (mut force, mut torque) = (DVec3::ZERO, DVec3::ZERO);
    for (k, t) in actuators.iter().enumerate() {
        let on = k < 64 && held & (1 << k) != 0 && enabled;
        jets[k] = if on { 1.0 } else { 0.0 };
        if on {
            let f = t.push * t.thrust;
            force += f;
            torque += (t.at - body.com).cross(f);
        }
    }
    Output { force, torque, spin: integrate_rates(body.inertia, body.spin, torque, None, dt) }
}
