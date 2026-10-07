//! A hull's landing legs, as its parts make them (SFO 15, Shock): each leg a
//! strut, a column of its stock's tube, taking what its material yields at or
//! buckles under (Euler, pinned ends), whichever is less; the legs together
//! stopping the ship over the shortest strut's stroke, at their strut
//! efficiency (`design.strut_efficiency`). The same reckoning as the
//! registry's structure report, for the ship as it is: its own mass (cargo
//! and all) on the ground it sets down on.
//!
//! The hardest landing its legs take: the energy they can soak up, less the
//! work of holding the ship's weight over the stroke, against what the ship
//! brings down: v = √(2 (F − m g) s η / m). Faster than this a leg fails.
//! What a landing does to everything aboard is its jolt: v² / (2 s η).

use std::collections::HashMap;

use crate::registry::{Part, PartLimitsLoadCase, Registry};
use universe_physics::laws::STANDARD_GRAVITY as G0;

/// A hull's legs.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Legs {
    /// The force the legs take together before one fails (N).
    pub force: f64,
    /// The shortest strut's stroke (m), and the struts' efficiency.
    pub stroke: f64,
    pub efficiency: f64,
    /// The sink speed they're designed for (m/s).
    pub designed: f64,
}

impl Legs {
    /// The fastest a ship of `mass` (kg) may sink onto ground of gravity `g`
    /// (m/s²) without a leg failing (m/s): none where its weight alone is more
    /// than they take.
    pub fn hardest(&self, mass: f64, g: f64) -> f64 {
        let spare = self.force - mass * g;
        if spare <= 0.0 { 0.0 } else { (2.0 * spare * self.stroke * self.efficiency / mass).sqrt() }
    }

    /// What a landing at sink speed `v` puts on everything aboard (g).
    pub fn jolt(&self, v: f64) -> f64 {
        v * v / (2.0 * self.stroke * self.efficiency) / G0
    }
}

/// The legs of a ship of `spec`: its hull's record's (by its key, or the
/// model it was imported from).
pub fn of_spec(spec: &crate::ship::ClassSpec) -> Option<Legs> {
    of(&crate::ship::hull_record(spec)?.identity.key)
}

/// The legs of hull `key`, if its record says what they're built of (its
/// parts with a landing load case, their stock's size and material).
pub fn of(key: &str) -> Option<Legs> {
    static LEGS: std::sync::OnceLock<HashMap<String, Option<Legs>>> = std::sync::OnceLock::new();
    LEGS.get_or_init(|| {
        let reg = crate::registry::registry();
        reg.hulls.iter().map(|h| (h.identity.key.clone(), reckon(reg, &h.identity.key))).collect()
    })
    .get(key)
    .copied()
    .flatten()
}

fn reckon(reg: &Registry, key: &str) -> Option<Legs> {
    let h = reg.hull(&key)?;
    let (designed, efficiency) = (h.design.landing_speed?, h.design.strut_efficiency.unwrap_or(1.0));
    let longest = |p: &Part| [p.physical.length, p.physical.width, p.physical.height].into_iter().flatten().fold(0.0, f64::max);
    // What one strut takes: what its material yields at, or buckles under, whichever is less.
    let takes = |p: &Part| -> Option<f64> {
        let item = &p.made_from.first()?.item;
        let stock = reg.stock(item)?;
        let made_of = &stock.made_from.first()?.item;
        let material = reg.material(made_of)?;
        let (d, w) = (stock.size.diameter?, stock.size.wall.unwrap_or(0.0));
        let (sy, e) = (material.mechanical.yield_strength?, material.mechanical.youngs_modulus?);
        let di = if w > 0.0 { d - 2.0 * w } else { 0.0 };
        let (area, inertia) = (std::f64::consts::PI * (d * d - di * di) / 4.0, std::f64::consts::PI * (d.powi(4) - di.powi(4)) / 64.0);
        let l = longest(p);
        Some((sy * area).min(std::f64::consts::PI.powi(2) * e * inertia / (l * l)))
    };
    // Each leg (a part made of parts, as many as it's fitted, or a landing-gear product fitted to a
    // gear slot: `equipment.gear.*`, built of its parts), its struts the parts with a landing load case.
    let (mut count, mut most, mut stroke) = (0u32, f64::INFINITY, f64::INFINITY);
    let fitted_gear = h.fit.iter().filter(|f| reg.equipment(&f.item).is_some_and(|e| matches!(e.function, crate::registry::EquipmentFunction::LandingGear(_)))).map(|f| f.item.clone());
    let mut legs: Vec<(String, u32)> = reg.built_of(key).into_iter().map(|(p, n)| (p.identity.key.clone(), n)).collect();
    for item in fitted_gear {
        match legs.iter_mut().find(|(k, _)| *k == item) {
            Some(l) => l.1 += 1,
            None => legs.push((item, 1)),
        }
    }
    for (leg, n) in legs {
        let struts: Vec<&Part> = reg.built_of(&leg).into_iter().map(|(p, _)| p).filter(|p| p.limits.load_case == Some(PartLimitsLoadCase::Landing)).collect();
        if struts.is_empty() {
            continue;
        }
        count += n;
        stroke = stroke.min(struts.iter().map(|p| longest(p)).fold(f64::INFINITY, f64::min));
        for p in &struts {
            most = most.min(takes(p)?);
        }
    }
    (count > 0 && most.is_finite() && stroke.is_finite()).then_some(Legs { force: most * count as f64, stroke, efficiency, designed })
}

/// How a ship of `spec`, `mass` kg, stands to land on ground of gravity `g`
/// (m/s²): its lift against its weight there, and the hardest landing its
/// legs take (None: no legs in its record).
pub fn ground_check(spec: &crate::ship::ClassSpec, mass: f64, g: f64) -> (f64, Option<f64>) {
    let lift = if g > 0.0 { spec.lift_thrust / (mass * g) } else { f64::INFINITY };
    (lift, of_spec(spec).map(|l| l.hardest(mass, g)))
}

/// A body's surface gravity (m/s²).
pub fn surface_gravity(body: &crate::system::Body) -> f64 {
    crate::units::G * body.mass / body.rail.radius.max(1.0).powi(2)
}
