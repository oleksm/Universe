//! Questions about the world that change nothing: gravity, the dominant body,
//! distances, and what would happen to a copy of a body.

use glam::DVec3;

use crate::body::RigidBody;
use crate::integrate::{integrate, Driver, Outcome, Span};
use crate::rails::{Ephemeris, OnRails};

/// Acceleration toward a point mass with gravitational parameter `mu` that
/// lies at offset `d` from here.
pub fn pull(mu: f64, d: DVec3) -> DVec3 {
    let r2 = d.length_squared().max(1.0);
    d * (mu / (r2 * r2.sqrt()))
}

/// Gravitational acceleration at `p` given body positions.
pub fn gravity<B: OnRails>(bodies: &[B], p: DVec3, positions: &[DVec3]) -> DVec3 {
    let mut acc = DVec3::ZERO;
    for (b, &bp) in bodies.iter().zip(positions) {
        let b = b.rail();
        if b.attracts {
            acc += pull(b.mu, bp - p);
        }
    }
    acc
}

/// The body whose gravity dominates at `p` (largest mu / r^2).
pub fn dominant<B: OnRails>(bodies: &[B], p: DVec3, positions: &[DVec3]) -> usize {
    let mut best = (0, 0.0);
    for (i, (b, &bp)) in bodies.iter().zip(positions).enumerate() {
        let b = b.rail();
        if b.attracts {
            let g = b.mu / bp.distance_squared(p).max(1.0);
            if g > best.1 {
                best = (i, g);
            }
        }
    }
    best.0
}

/// Closest distance from point `p` to the segment `a`-`b`.
pub fn segment_distance(a: DVec3, b: DVec3, p: DVec3) -> f64 {
    let ab = b - a;
    let t = ((p - a).dot(ab) / ab.length_squared().max(1e-9)).clamp(0.0, 1.0);
    (a + ab * t).distance(p)
}

/// What would happen: simulate a copy of `body` through `span` under
/// `driver`, with the same integrator and contacts as the real thing, leaving
/// the original untouched. Rail bodies are solved exactly at every substep,
/// or extrapolated from an `ephemeris` valid around `span.t` (as `integrate`).
pub fn simulate<B: OnRails>(bodies: &[B], ephemeris: Option<&Ephemeris>, body: &RigidBody, span: Span, driver: &mut impl Driver) -> (RigidBody, Outcome) {
    let mut copy = *body;
    let mut positions = Vec::with_capacity(bodies.len());
    let outcome = integrate(bodies, ephemeris, &mut positions, &mut copy, span, driver);
    (copy, outcome)
}
