//! Ships colliding with each other: Dogma finds the pairs whose
//! bounding spheres touched during the frame (`universe_physics::contacts`);
//! then their shapes — each hull's contact spheres, where they were through
//! the frame — say whether they really met, where, and which way (a wing
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
/// Moments through the frame at which two ships' shapes are compared.
const SAMPLES: usize = 6;

/// Did ships `a` and `b`, whose bounding spheres met `before_end` seconds
/// before the end of the frame, touch? Their hulls compared at moments from
/// then to the end (each ship moving as it was, turned as it is now): a
/// corner or edge of one inside a solid part of the other. At the first
/// moment they do: which way (unit, from `b` to `a`: out of the face it went
/// in by), how fast they closed along it, and how deep they overlap at the
/// end (m).
fn shapes_touch(a: &Ship, b: &Ship, before_end: f64) -> Option<(DVec3, f64, f64)> {
    let (sa, sb) = (a.spec().shape(), b.spec().shape());
    let (ra, rb) = (sa.mesh.bound(), sb.mesh.bound());
    let v = a.velocity - b.velocity;
    // The deepest a point of one is inside the other at time `t` (s, ≤ 0: before the end).
    let deepest = |t: f64| -> Option<(f64, DVec3)> {
        let (pa, pb) = (a.position + a.velocity * t, b.position + b.velocity * t);
        let mut best: Option<(f64, DVec3)> = None;
        // One's points in the other: (its pose, its shape; the other's pose and shape; which way round).
        for (from, ofrom, shape_from, into, ointo, shape_into, reach, sign) in [(pa, a.orientation, sa, pb, b.orientation, sb, rb, 1.0), (pb, b.orientation, sb, pa, a.orientation, sa, ra, -1.0)] {
            let back = ointo.inverse();
            // (How the point moves against the other: a's against b, or b's against a.)
            let moving = back * (v * sign);
            for q in shape_from.probes() {
                let w = from + ofrom * q;
                if w.distance(into) > reach {
                    continue;
                }
                if let Some((depth, n)) = shape_into.inside(back * (w - into), moving)
                    && best.is_none_or(|(d, _)| depth > d)
                {
                    // (Out of the face it's in by: a's point in b, out toward a; b's in a, out toward b.)
                    best = Some((depth, ointo * n * sign));
                }
            }
        }
        best
    };
    for k in 0..=SAMPLES {
        let t = -before_end * (1.0 - k as f64 / SAMPLES as f64);
        if let Some((_, normal)) = deepest(t) {
            let closing = -v.dot(normal);
            let depth = deepest(0.0).map_or(0.0, |(d, _)| d);
            // Already overlapping but moving apart: nothing to do.
            if closing <= 0.0 && k > 0 {
                return None;
            }
            return Some((normal, closing.max(0.0), depth));
        }
    }
    None
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
