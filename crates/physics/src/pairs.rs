//! Contacts between moving bodies: spheres that touched during a step. Each
//! moved in a straight line over the step (to its `position` at the end), so
//! the test is swept on their relative motion and can't be stepped over. A
//! spatial grid keeps it cheap with many bodies about. The kernel reports the
//! contacts; what they do (bounce, damage) is the caller's business, with
//! `bounce_pair` for the physics of the bounce itself.

use std::collections::HashMap;

use glam::DVec3;

/// A body taking part: a sphere moving at constant velocity over the step,
/// at `position` at its end. `mass` infinite for something fixed in place.
#[derive(Clone, Copy, Debug)]
pub struct Mover {
    /// The caller's id for it.
    pub id: usize,
    pub position: DVec3,
    pub velocity: DVec3,
    pub radius: f64,
    pub mass: f64,
}

/// Two bodies touching: when (seconds before the end of the step), where
/// their centers were then, and how they were moving.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PairContact {
    /// Indices into the movers given.
    pub a: usize,
    pub b: usize,
    /// Unit normal from `b` to `a` at contact.
    pub normal: DVec3,
    /// Speed at which they closed along the normal (m/s, positive: closing).
    pub closing: f64,
    /// How long before the end of the step they touched (s).
    pub before_end: f64,
}

/// Grid cell size (m): larger than any body plus its motion in a step.
const CELL: f64 = 2_000.0;

/// Every pair of `movers` that touched during the last `dt` seconds.
pub fn contacts(movers: &[Mover], dt: f64) -> Vec<PairContact> {
    let key = |p: DVec3| ((p.x / CELL).floor() as i64, (p.y / CELL).floor() as i64, (p.z / CELL).floor() as i64);
    let mut grid: HashMap<(i64, i64, i64), Vec<usize>> = HashMap::new();
    for (i, m) in movers.iter().enumerate() {
        grid.entry(key(m.position)).or_default().push(i);
    }
    // Only what moves can touch anything: each free body looks around it
    // (two fixed ones, landed side by side, are never compared).
    let mut out = Vec::new();
    for (i, a) in movers.iter().enumerate().filter(|(_, m)| m.mass.is_finite()) {
        let (x, y, z) = key(a.position);
        for dx in -1..=1 {
            for dy in -1..=1 {
                for dz in -1..=1 {
                    let Some(cell) = grid.get(&(x + dx, y + dy, z + dz)) else { continue };
                    for &j in cell {
                        let b = &movers[j];
                        // Each free pair once (from the lower index); free–fixed from the free one.
                        if j == i || (b.mass.is_finite() && j < i) {
                            continue;
                        }
                        if let Some(c) = touch(a, b, dt) {
                            let (lo, hi) = if i < j { (i, j) } else { (j, i) };
                            let c = if lo == i { c } else { flip(c) };
                            out.push(PairContact { a: lo, b: hi, ..c });
                        }
                    }
                }
            }
        }
    }
    out.sort_by_key(|p| (p.a, p.b));
    out
}

/// The same contact seen from the other body.
fn flip(c: PairContact) -> PairContact {
    PairContact { normal: -c.normal, ..c }
}

/// Did `a` and `b` touch during the step? (Indices left for the caller.)
fn touch(a: &Mover, b: &Mover, dt: f64) -> Option<PairContact> {
    let reach = a.radius + b.radius;
    let rel_end = a.position - b.position;
    let v = a.velocity - b.velocity;
    // Relative position at the start of the step, moving by v·dt to the end.
    let start = rel_end - v * dt;
    let d = v * dt;
    let (qa, qb, qc) = (d.length_squared(), start.dot(d), start.length_squared() - reach * reach);
    let s = if qc <= 0.0 {
        0.0
    } else {
        if qa < 1e-18 || qb >= 0.0 {
            return None;
        }
        let disc = qb * qb - qa * qc;
        if disc < 0.0 {
            return None;
        }
        let s = (-qb - disc.sqrt()) / qa;
        if s > 1.0 {
            return None;
        }
        s
    };
    let at = start + d * s;
    let normal = at.normalize_or(DVec3::Y);
    let closing = -v.dot(normal);
    // Already overlapping but moving apart: nothing to do.
    if closing <= 0.0 && qc > 0.0 {
        return None;
    }
    Some(PairContact { a: 0, b: 0, normal, closing: closing.max(0.0), before_end: (1.0 - s) * dt })
}

/// The bounce of a contact between bodies of mass `ma` and `mb` (either may
/// be infinite: fixed) with coefficient of restitution `e`: the change of
/// velocity for each (momentum is conserved), and the kinetic energy lost (J).
pub fn bounce_pair(normal: DVec3, closing: f64, ma: f64, mb: f64, e: f64) -> (DVec3, DVec3, f64) {
    let (ia, ib) = (1.0 / ma, 1.0 / mb);
    let j = (1.0 + e) * closing / (ia + ib);
    let reduced = 1.0 / (ia + ib);
    let lost = 0.5 * reduced * closing * closing * (1.0 - e * e);
    (normal * (j * ia), -normal * (j * ib), lost)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn m(id: usize, x: f64, vx: f64) -> Mover {
        Mover { id, position: DVec3::new(x, 0.0, 0.0), velocity: DVec3::new(vx, 0.0, 0.0), radius: 12.0, mass: 90_000.0 }
    }

    #[test]
    fn a_head_on_pass_is_caught_and_bounces_conserving_momentum() {
        // 200 m/s each way, 1 s step: they'd pass right through each other.
        let movers = [m(0, 150.0, 200.0), m(1, -150.0, -200.0)];
        let c = contacts(&movers, 1.0);
        assert_eq!(c.len(), 1);
        let c = c[0];
        assert!((c.closing - 400.0).abs() < 1e-6);
        assert!((c.normal.x + 1.0).abs() < 1e-6, "a was on the -x side when they met");
        let (da, db, lost) = bounce_pair(c.normal, c.closing, 90_000.0, 90_000.0, 0.3);
        let (va, vb) = (movers[0].velocity + da, movers[1].velocity + db);
        assert!((va * 90_000.0 + vb * 90_000.0).length() < 1e-6, "momentum conserved");
        assert!((va.x - (-60.0)).abs() < 1e-6 && (vb.x - 60.0).abs() < 1e-6, "e = 0.3");
        assert!(lost > 0.0);
    }

}
