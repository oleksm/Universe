//! Projectiles: small unpowered bodies (slugs) under gravity, swept against
//! moving spherical targets and the rail bodies, and rays (for beams). The
//! kernel reports what was hit, where and how fast; what that does is the
//! caller's business.

use glam::DVec3;

use crate::collide::Collider;
use crate::query::gravity;
use crate::rails::OnRails;
use crate::surface::surface_radius_at;

/// A small body with no drive of its own.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Projectile {
    pub position: DVec3,
    pub velocity: DVec3,
}

/// A sphere a projectile or ray can hit, moving at constant velocity over a
/// step. `position` is where it is at the *end* of the step.
#[derive(Clone, Copy, Debug)]
pub struct Target {
    /// The caller's id for it.
    pub id: usize,
    pub position: DVec3,
    pub velocity: DVec3,
    pub radius: f64,
}

/// What a projectile or ray met.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Hit {
    /// A target, at `point`, with the projectile moving at `relative_velocity`
    /// to it (zero for a ray).
    Target { id: usize, point: DVec3, relative_velocity: DVec3 },
    /// A rail body (by index), at `point`.
    Body { body: usize, point: DVec3 },
}

/// Where to aim a projectile leaving at `muzzle` m/s relative to a shooter
/// at `own_position` moving at `own_velocity`, to meet a body at `position`
/// moving at `velocity` and accelerating at `acceleration` (gravity, shared
/// by both over a short flight, cancels). The unit aim, the meeting point
/// relative to the shooter now, and the flight time; None if it can't be
/// caught within `max_time` seconds.
pub fn intercept(own_position: DVec3, own_velocity: DVec3, position: DVec3, velocity: DVec3, acceleration: DVec3, muzzle: f64, max_time: f64) -> Option<(DVec3, DVec3, f64)> {
    let r0 = position - own_position;
    let v = velocity - own_velocity;
    let at = |t: f64| r0 + v * t + acceleration * (0.5 * t * t);
    let mut t = r0.length() / muzzle;
    for _ in 0..30 {
        let next = at(t).length() / muzzle;
        if (next - t).abs() < 1e-4 {
            let offset = at(next);
            return (next <= max_time).then(|| (offset.normalize_or_zero(), offset, next));
        }
        t = next;
        if t > max_time * 2.0 {
            return None;
        }
    }
    None
}

/// Longest substep for a projectile (s).
pub const PROJECTILE_STEP: f64 = 0.05;

/// Earliest time in `0..=1` at which a point starting at `rel` (relative to a
/// sphere's center) and moving by `d` over the step is within `radius`.
fn sweep(rel: DVec3, d: DVec3, radius: f64) -> Option<f64> {
    let (a, b, c) = (d.length_squared(), rel.dot(d), rel.length_squared() - radius * radius);
    if c <= 0.0 {
        return Some(0.0);
    }
    if a < 1e-18 || b >= 0.0 {
        return None;
    }
    let disc = b * b - a * c;
    if disc < 0.0 {
        return None;
    }
    let s = (-b - disc.sqrt()) / a;
    (s <= 1.0).then_some(s)
}

/// Is `p` inside rail body `i` (at `positions[i]`) at time `t`? Surfaces
/// count with their terrain; small solids (polytopes) by their bounding
/// sphere; rings and bodies with no collider don't stop anything.
fn inside<B: OnRails>(b: &B, center: DVec3, p: DVec3, t: f64) -> bool {
    match &b.rail().collider {
        Collider::Surface => p.distance(center) < surface_radius_at(b, center, p, t),
        Collider::Blocks(b) => p.distance(center) < b.bound,
        Collider::Ring(_) | Collider::None => false,
    }
}

/// Move `p` over `dt` seconds from `t` under the rail bodies' gravity (bodies
/// at `positions`, held for the step), in substeps of at most
/// `PROJECTILE_STEP`, and return the first thing it hits. The targets move at
/// their velocity, arriving at their `position` at `t + dt`.
pub fn step_projectile<B: OnRails>(bodies: &[B], positions: &[DVec3], p: &mut Projectile, t: f64, dt: f64, targets: &[Target]) -> Option<Hit> {
    let n = (dt / PROJECTILE_STEP).ceil().max(1.0);
    let h = dt / n;
    let mut s = 0.0;
    while s < dt - 1e-12 {
        let start = p.position;
        // Semi-implicit Euler: plenty for seconds of flight.
        p.velocity += gravity(bodies, p.position, positions) * h;
        p.position += p.velocity * h;
        let d = p.position - start;
        // The earliest target touched during the substep.
        let mut best: Option<(f64, Hit)> = None;
        for tg in targets {
            let at_start = tg.position - tg.velocity * (dt - s);
            let rel_d = d - tg.velocity * h;
            if let Some(f) = sweep(start - at_start, rel_d, tg.radius)
                && best.is_none_or(|(b, _)| f < b)
            {
                let point = start + d * f;
                best = Some((f, Hit::Target { id: tg.id, point, relative_velocity: p.velocity - tg.velocity }));
            }
        }
        s += h;
        if let Some((f, hit)) = best {
            p.position = start + d * f;
            return Some(hit);
        }
        for (i, b) in bodies.iter().enumerate() {
            if inside(b, positions[i], p.position, t + s) {
                return Some(Hit::Body { body: i, point: p.position });
            }
        }
    }
    None
}

/// Cast a ray from `from` along the unit `dir` up to `range`: the nearest
/// target or rail body it meets, and how far along. Bodies are sampled for
/// the ray entering them (fine enough for surfaces at beam ranges).
pub fn ray<B: OnRails>(bodies: &[B], positions: &[DVec3], from: DVec3, dir: DVec3, range: f64, t: f64, targets: &[Target]) -> Option<(Hit, f64)> {
    let mut best: Option<(Hit, f64)> = None;
    for tg in targets {
        if let Some(f) = sweep(from - tg.position, dir * range, tg.radius) {
            let d = f * range;
            if best.is_none_or(|(_, b)| d < b) {
                best = Some((Hit::Target { id: tg.id, point: from + dir * d, relative_velocity: DVec3::ZERO }, d));
            }
        }
    }
    // Rail bodies: find where the ray first enters one (bounding sphere, then
    // march to the surface).
    let limit = best.map_or(range, |(_, d)| d);
    for (i, b) in bodies.iter().enumerate() {
        let rail = b.rail();
        let outer = match &rail.collider {
            Collider::Surface => crate::surface::max_radius(b),
            Collider::Blocks(b) => b.bound,
            _ => continue,
        };
        let Some(f) = sweep(from - positions[i], dir * limit, outer) else { continue };
        let mut d = f * limit;
        let step = (outer * 0.002).max(1.0);
        while d <= limit + step {
            if inside(b, positions[i], from + dir * d, t) {
                // Refine between the last point outside and this one.
                let (mut lo, mut hi) = ((d - step).max(0.0), d);
                while hi - lo > 0.5 {
                    let mid = 0.5 * (lo + hi);
                    if inside(b, positions[i], from + dir * mid, t) { hi = mid } else { lo = mid }
                }
                if hi <= limit && best.is_none_or(|(_, bd)| hi < bd) {
                    best = Some((Hit::Body { body: i, point: from + dir * hi }, hi));
                }
                break;
            }
            let p = from + dir * d;
            // Out of the bounding sphere again: missed it.
            if p.distance(positions[i]) > outer + step {
                break;
            }
            d += step;
        }
    }
    best
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rails::RailBody;
    use crate::testkit::body;

    fn empty() -> Vec<RailBody> {
        Vec::new()
    }

    #[test]
    fn a_slug_hits_a_crossing_target_it_would_step_over() {
        // 3 km/s slug, 0.1 s steps (300 m) against a 12 m target crossing its path.
        let mut p = Projectile { position: DVec3::ZERO, velocity: DVec3::X * 3000.0 };
        let target = |t: f64| Target { id: 7, position: DVec3::new(1500.0, -100.0 + 200.0 * t, 0.0), velocity: DVec3::Y * 200.0, radius: 12.0 };
        let mut hit = None;
        let mut t = 0.0;
        while hit.is_none() && t < 1.0 {
            hit = step_projectile(&empty(), &[], &mut p, t, 0.1, &[target(t + 0.1)]);
            t += 0.1;
        }
        match hit {
            Some(Hit::Target { id, point, .. }) => {
                assert_eq!(id, 7);
                assert!((point.x - 1500.0).abs() < 20.0, "{point}");
            }
            other => panic!("expected a hit, got {other:?}"),
        }
    }

    #[test]
    fn a_ray_stops_at_the_nearer_of_a_target_and_a_planet() {
        let bodies = vec![body(None, None, 3.986e14, 6.371e6)];
        let positions = vec![DVec3::ZERO];
        let from = DVec3::X * 6.4e6;
        let toward = DVec3::NEG_X;
        let hit = ray(&bodies, &positions, from, toward, 1.0e5, 0.0, &[]).expect("hits the ground");
        assert!(matches!(hit.0, Hit::Body { body: 0, .. }));
        assert!((hit.1 - 29_000.0).abs() < 50.0, "{}", hit.1);
        let near = Target { id: 3, position: from + toward * 5_000.0, velocity: DVec3::ZERO, radius: 12.0 };
        let hit = ray(&bodies, &positions, from, toward, 1.0e5, 0.0, &[near]).unwrap();
        assert!(matches!(hit.0, Hit::Target { id: 3, .. }));
        assert!((hit.1 - 4_988.0).abs() < 1.0);
    }
}
