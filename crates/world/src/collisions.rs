//! Ships colliding with each other: Dogma finds the pairs whose
//! bounding spheres touched during the frame (`universe_physics::contacts`);
//! then their shapes — each hull's convex parts, swept through the frame
//! (exactly: no speed passes through unseen) — say whether they really met,
//! where, and which way (a wing
//! clips another; a ship passes over another's back untouched). They bounce, with
//! momentum conserved and `RESTITUTION` of the closing speed kept, and the
//! energy lost goes into both hulls, half each (see `damage`). A nudge dents;
//! a hard hit destroys them both. A ship resting on the ground stands firm
//! (it takes its half of the damage, but doesn't move). Ships docked inside a
//! station are out of the way; ships in hyperdrive or between gates aren't in
//! normal space.

use std::collections::HashMap;

use glam::DVec3;
use universe_physics::{bounce_pair, contacts, Mover};

use crate::damage;
use crate::events::ShipEvent;
use crate::ship::{Ship, ShipState};
use crate::system::BodyKind;
use crate::weapons::Armed;
use crate::world::World;

/// Share of the closing speed kept in a bounce.
pub const RESTITUTION: f64 = 0.3;
/// Slower than this, touching ships just ease apart (m/s).
const IMPACT: f64 = 0.5;
/// Did ships `a` and `b`, whose bounding spheres met `before_end` seconds
/// before the end of the frame, touch? Each ship moves as it was, turned as
/// it is now, so against the other each point of its surface (corners, the
/// middles of edges) goes in a straight line. Every part of one against
/// every part of the other, over the time their spheres overlap
/// (`overlap`): when each point's line enters the other part, solved from
/// the part's faces, so however fast they pass, none goes through unseen.
/// At the first moment one is inside the other: which way (unit, from `b`
/// to `a`: out of the face it went in by), how fast they closed along it,
/// and how deep they overlap at the end (m).
fn shapes_touch(a: &Ship, b: &Ship, before_end: f64) -> Option<(DVec3, f64, f64)> {
    let (sa, sb) = (a.spec().shape(), b.spec().shape());
    let v = a.velocity - b.velocity;
    // Where part `i` of a's centre is from part `j` of b's at the end.
    let apart = |i: usize, j: usize| a.position + a.orientation * sa.parts[i].centre - b.position - b.orientation * sb.parts[j].centre;
    // Each way round: a's part `i` against b's part `j`, and b's `j` against
    // a's `i`. (Its pose, its part; the other's pose, shape and part; which way round.)
    let ways = |i: usize, j: usize| [(a, &sa.parts[i], b, sb, j, 1.0), (b, &sb.parts[j], a, sa, i, -1.0)];
    // The first moment from `t0` to `t1` (s, ≤ 0: before the end) a point of
    // one is inside the other part, and out which way.
    let entry = |i: usize, j: usize, t0: f64, t1: f64| -> Option<(f64, DVec3)> {
        let mut first: Option<(f64, DVec3)> = None;
        for (from, part, into, shape, k, sign) in ways(i, j) {
            let back = into.orientation.inverse();
            // (How its points move against the other: a's against b, or b's against a.)
            let w = back * (v * sign);
            for &q in &part.probes {
                // Where it is at the end, in the other's frame; inside while every face has it behind.
                let p = back * (from.position + from.orientation * q - into.position);
                let (mut lo, mut hi, mut by) = (t0, t1, None);
                for &(n, d) in &shape.solids[k] {
                    let (along, room) = (n.dot(w), d - n.dot(p));
                    if along.abs() < 1e-12 {
                        if room <= 0.0 {
                            hi = f64::NEG_INFINITY;
                            break;
                        }
                    } else if along > 0.0 {
                        hi = hi.min(room / along);
                    } else if room / along > lo {
                        (lo, by) = (room / along, Some(n));
                    }
                }
                if lo < hi && first.is_none_or(|(t, _)| lo < t) {
                    // In by face `by`; inside from the start: as `inside` judges it.
                    let n = by.or_else(|| shape.inside_part(k, p + w * lo, w).map(|(_, n)| n));
                    if let Some(n) = n {
                        // (Out of the face it's in by: a's point in b, out toward a; b's in a, out toward b.)
                        first = Some((lo, into.orientation * n * sign));
                    }
                }
            }
        }
        first
    };
    // The deepest a point of one is in the other part at the end.
    let deepest = |i: usize, j: usize| -> f64 {
        let mut best: f64 = 0.0;
        for (from, part, into, shape, k, sign) in ways(i, j) {
            let back = into.orientation.inverse();
            for &q in &part.probes {
                let p = back * (from.position + from.orientation * q - into.position);
                if let Some((depth, _)) = shape.inside_part(k, p, back * (v * sign)) {
                    best = best.max(depth);
                }
            }
        }
        best
    };
    let mut first: Option<(f64, DVec3)> = None;
    for i in 0..sa.parts.len() {
        for j in 0..sb.parts.len() {
            let Some((t0, t1)) = overlap(apart(i, j), v, sa.parts[i].radius + sb.parts[j].radius, -before_end) else { continue };
            if let Some((t, n)) = entry(i, j, t0, t1)
                && first.is_none_or(|(f, _)| t < f)
            {
                first = Some((t, n));
            }
        }
    }
    let (t, normal) = first?;
    let closing = -v.dot(normal);
    // Already overlapping but moving apart: nothing to do.
    if closing <= 0.0 && t > -before_end {
        return None;
    }
    let depth = (0..sa.parts.len())
        .flat_map(|i| (0..sb.parts.len()).map(move |j| (i, j)))
        .filter(|&(i, j)| apart(i, j).length() <= sa.parts[i].radius + sb.parts[j].radius)
        .map(|(i, j)| deepest(i, j))
        .fold(0.0, f64::max);
    Some((normal, closing.max(0.0), depth))
}

/// When, from `from` (s, ≤ 0) to the end of the frame (0), two spheres whose
/// centres are `rel` apart at the end, moving apart at `v`, are within `r`
/// of each other: `|rel + v t| ≤ r`. None if never.
fn overlap(rel: DVec3, v: DVec3, r: f64, from: f64) -> Option<(f64, f64)> {
    let (vv, rv, gap) = (v.length_squared(), rel.dot(v), rel.length_squared() - r * r);
    let (t0, t1) = if vv < 1e-12 {
        if gap > 0.0 {
            return None;
        }
        (from, 0.0)
    } else {
        let disc = rv * rv - vv * gap;
        if disc < 0.0 {
            return None;
        }
        let s = disc.sqrt();
        ((-rv - s) / vv, (-rv + s) / vv)
    };
    let (t0, t1) = (t0.max(from), t1.min(0.0));
    (t0 <= t1).then_some((t0, t1))
}

impl World {
    /// Collisions between ships over the last `dt` game seconds.
    pub fn collide(&mut self, ships: &mut [Armed], dt: f64) {
        if dt <= 0.0 {
            return;
        }
        // Who takes part, by system.
        let sort = universe_prof::scope("sim/combat/collisions/who");
        let mut by_system: HashMap<usize, Vec<usize>, universe_physics::pairs::CellHash> = HashMap::default();
        // (Each system looked up once.)
        let mut seen: HashMap<usize, std::sync::Arc<crate::system::StarSystem>, universe_physics::pairs::CellHash> = HashMap::default();
        for (k, a) in ships.iter().enumerate() {
            let s = &a.ship;
            let takes_part = match s.state {
                ShipState::Flying => !s.hyperdrive,
                // (In a hangar: indoors, touching nothing outside.)
                ShipState::Landed { .. } if s.hangar.is_some() => false,
                ShipState::Landed { body, .. } => seen.entry(a.system).or_insert_with(|| self.system(a.system)).bodies[body].kind != BodyKind::Station,
                _ => false,
            };
            if takes_part {
                by_system.entry(a.system).or_default().push(k);
            }
        }
        let mut systems: Vec<_> = by_system.into_iter().filter(|(_, v)| v.len() > 1).collect();
        systems.sort_by_key(|(s, _)| *s);
        drop(sort);
        // Each system's pairs found side by side; then put right in order.
        let found: Vec<(Vec<Mover>, Vec<universe_physics::PairContact>)> = {
            use rayon::prelude::*;
            let gather = universe_prof::scope("sim/combat/collisions/movers");
            let movers: Vec<Vec<Mover>> = systems
                .iter()
                .map(|(_, members)| {
                    members
                        .iter()
                        .map(|&k| {
                            let s = &ships[k].ship;
                            let fixed = matches!(s.state, ShipState::Landed { .. });
                            // (Its reach: the sphere round its shape; the shapes decide after.)
                            Mover { id: k, position: s.position, velocity: s.velocity, radius: s.spec().shape().mesh.bound(), mass: if fixed { f64::INFINITY } else { s.mass() } }
                        })
                        .collect()
                })
                .collect();
            drop(gather);
            let _p = universe_prof::scope("sim/combat/collisions/pairs");
            movers.into_par_iter().map(|m| { let c = contacts(&m, dt); (m, c) }).collect()
        };
        let _p = universe_prof::scope("sim/combat/collisions/shapes");
        for (movers, found) in found {
            for c in found {
                let (ma, mb) = (movers[c.a], movers[c.b]);
                let (ia, ib) = (ma.id, mb.id);
                // Their shapes: did they really meet, and which way?
                let Some((normal, closing, gap)) = shapes_touch(ships[ia].ship, ships[ib].ship, c.before_end) else { continue };
                let c = universe_physics::PairContact { normal, closing, ..c };
                let (dva, dvb, lost) = bounce_pair(c.normal, c.closing, ma.mass, mb.mass, RESTITUTION);
                let (ida, idb) = (ships[ia].id, ships[ib].id);
                // Push apart to touching, the lighter (or free) one the more.
                let (wa, wb) = (1.0 / ma.mass, 1.0 / mb.mass);
                for (k, dv, w, other, sign) in [(ia, dva, wa, idb, 1.0), (ib, dvb, wb, ida, -1.0)] {
                    let a = &mut ships[k];
                    if w > 0.0 {
                        a.ship.velocity += dv;
                        if gap > 0.0 {
                            a.ship.position += c.normal * (sign * gap * w / (wa + wb));
                        }
                    }
                    // Resting against each other is no impact.
                    if c.closing >= IMPACT {
                        a.events.push(ShipEvent::Collided { with: other, speed: c.closing });
                        damage::hit(a.ship, lost * 0.5, DVec3::ZERO, other, "COLLISION", a.events);
                    }
                }
            }
        }
    }
}
