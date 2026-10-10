//! Rigs: movable works, brought to where something too large to lift off a
//! world is to be built (as an oil rig is), with no ground, zoning or
//! administration of their own (the registry's `rig` records). Each is a body
//! in a low circular orbit round the body its record says it's at. It has no
//! model yet, so it is its size as a box (the user's rule: no model, a box at
//! its dimensions): the registry gives a rig no size, so the box holds what
//! it's built of (its modules' volume, packed at `PACKING`: worked out, said
//! so). A ship comes alongside and rests on its top; its owner trades there.

use std::f64::consts::TAU;

use glam::{DQuat, DVec3};
use universe_physics::{Blocks, Collider, Orbit, RailBody};

use crate::system::{Body, BodyKind, StarSystem};
use crate::units::G;

/// How much of a rig's box its modules fill. Invented.
pub const PACKING: f64 = 0.5;
/// Its orbit's size, in its world's radii (as a station's). Invented: a rig
/// record says no orbit.
pub const ORBIT: f64 = 1.6;

/// The registry's rigs at bodies of `sys`, added as bodies (after all the
/// rest, so nothing made before them moves).
pub(crate) fn add(sys: &mut StarSystem) {
    let reg = crate::registry::registry();
    for r in reg.settlements.iter().filter(|s| s.kind == crate::registry::SettlementKind::Rig) {
        let Some(at) = r.at.as_deref() else { continue };
        let Some(world) = sys.bodies.iter().position(|b| b.key == at) else { continue };
        // What it's built of: its lines' modules and its own, by their sizes and masses.
        let modules = r.lines.iter().flat_map(|l| l.modules.iter().map(|m| (m.module.as_str(), m.count))).chain(r.modules.iter().map(|m| (m.module.as_str(), m.count)));
        let (mut volume, mut mass) = (0.0, 0.0);
        for (k, n) in modules {
            let Some(m) = reg.module(k) else { continue };
            let p = &m.physical;
            volume += p.length.unwrap_or(0.0) * p.width.unwrap_or(0.0) * p.height.unwrap_or(0.0) * n as f64;
            mass += p.mass.unwrap_or(0.0) * n as f64;
        }
        if volume <= 0.0 {
            continue;
        }
        // (A box twice as long as it is wide and high.)
        let side = (volume / PACKING / 2.0).cbrt();
        let half = DVec3::new(side / 2.0, side / 2.0, side);
        let mass = mass.max(1.0e8);
        let w = &sys.bodies[world];
        let phase = r.identity.key.bytes().fold(0u64, |h, b| h.wrapping_mul(31).wrapping_add(b as u64)) as f64 / u64::MAX as f64 * TAU;
        let orbit = Orbit::new(w.rail.radius * ORBIT, 0.0, 0.0, 0.0, 0.0, phase, w.rail.mu);
        let (day, tilt) = (orbit.period(), DQuat::from_rotation_arc(DVec3::Y, orbit.normal()));
        sys.bodies.push(Body {
            name: r.identity.name.clone(),
            key: r.identity.key.clone(),
            kind: BodyKind::Rig,
            mass,
            color: [0.75, 0.8, 0.85],
            rings: None,
            link: None,
            terrain: None,
            rail: RailBody {
                parent: Some(world),
                orbit: Some(orbit),
                mu: G * mass,
                attracts: true,
                radius: half.length(),
                day,
                tilt,
                // (Its top a deck: a ship rests on it.)
                collider: Collider::Blocks(Blocks::new(vec![(-half, half)], vec![0])),
                atmosphere: None,
                pulled_by: Vec::new(),
            },
            rock: None,
        });
    }
}

/// A rig's box: its half extents (m), in its own frame.
pub fn half(body: &Body) -> Option<DVec3> {
    match &body.rail.collider {
        Collider::Blocks(b) if body.kind == BodyKind::Rig => b.boxes.first().map(|(lo, hi)| (*hi - *lo) / 2.0),
        _ => None,
    }
}
