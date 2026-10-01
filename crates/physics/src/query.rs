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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testkit::body;

    #[test]
    fn gravity_sums_attracting_bodies_and_dominant_picks_the_strongest() {
        let mut light = body(None, None, 1.0e20, 10.0);
        light.attracts = false;
        let bodies = [body(None, None, 4.0e14, 6.4e6), body(Some(0), None, 4.9e12, 1.7e6), light];
        let positions = [DVec3::ZERO, DVec3::X * 4.0e8, DVec3::X * 1.0e7];
        let p = DVec3::X * 3.9e8;
        let g = gravity(&bodies, p, &positions);
        let expected = -DVec3::X * (4.0e14 / (3.9e8f64 * 3.9e8)) + DVec3::X * (4.9e12 / 1.0e14);
        assert!((g - expected).length() < 1e-12 * expected.length() + 1e-15);
        assert_eq!(dominant(&bodies, p, &positions), 1, "the moon dominates 10,000 km from it");
        assert_eq!(dominant(&bodies, DVec3::X * 1.0e7, &positions), 0, "a body that doesn't attract never dominates");
    }

}
