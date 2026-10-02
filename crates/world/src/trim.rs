//! Balancing a ship, at a shipyard: some of the fuel aboard pumped out to
//! trim cells at the hull's ends (moving the centre of mass), and each main
//! nozzle's share of the drive set (moving the centre of thrust), so the
//! main drive pushes through the centre of mass and doesn't turn the ship.
//!
//! The trim cells hold a share (`TRIM_SHARE`) of whatever fuel is aboard:
//! as it burns, they drain with the rest. Side by side mains can't swing
//! their thrust up or down (that's the cells' work); the cells can't move
//! more than they hold.

use std::borrow::Cow;

use glam::{DMat3, DVec3};
use serde::{Deserialize, Serialize};

use crate::ship::{ClassSpec, Thruster, ThrusterRole};

/// The most of the fuel aboard the trim cells take, all of them together.
pub const TRIM_SHARE: f64 = 0.15;
/// The least share of the drive a main nozzle can be set to.
pub const MAIN_SHARE_MIN: f64 = 0.7;

/// A ship's trim (see the module): along each axis, how much of
/// `TRIM_SHARE` of the fuel goes to that end's cell (−1..1: x port to
/// starboard, y down to up, z fore to aft); each main nozzle's share of the
/// drive (in the order of its thrusters; missing ones full).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Trim {
    pub cells: DVec3,
    pub mains: Vec<f64>,
}

impl Trim {
    /// Within its limits: the cells together no more than they hold, the
    /// mains' shares `MAIN_SHARE_MIN`..1.
    pub fn clamped(&self) -> Trim {
        let c = self.cells.clamp(DVec3::splat(-1.0), DVec3::ONE);
        let total = c.x.abs() + c.y.abs() + c.z.abs();
        Trim { cells: if total > 1.0 { c / total } else { c }, mains: self.mains.iter().map(|s| s.clamp(MAIN_SHARE_MIN, 1.0)).collect() }
    }

    /// Main nozzle `k`'s share (counting the mains only).
    pub fn main(&self, k: usize) -> f64 {
        self.mains.get(k).copied().unwrap_or(1.0)
    }

    /// A key for it, to the hundredth (for caches).
    pub fn key(&self) -> i64 {
        let q = |v: f64| (v * 100.0).round() as i64;
        let mut h = q(self.cells.x) * 1_000_003 + q(self.cells.y) * 10_007 + q(self.cells.z);
        for &s in &self.mains {
            h = h.wrapping_mul(131).wrapping_add(q(s));
        }
        h
    }
}

/// Where a hull's trim cells sit: the six ends of it, along each axis from
/// its tanks, as far out as the hull goes (+x, −x, +y, −y, +z, −z).
pub fn cells(shape: &crate::shape::Shape, tank_at: DVec3) -> [DVec3; 6] {
    let (lo, hi) = shape.mesh.extent();
    let mut out = [tank_at; 6];
    for (k, cell) in out.iter_mut().enumerate() {
        let axis = k / 2;
        let end = if k % 2 == 0 { hi[axis] } else { lo[axis] };
        // From the end in toward the tanks, the first point inside the hull (with a margin).
        let mut p = tank_at;
        for i in 0..=40 {
            let mut q = tank_at;
            q[axis] = end + (tank_at[axis] - end) * (0.05 + 0.95 * i as f64 / 40.0);
            if shape.inside(q, DVec3::ZERO).is_some() {
                p = q;
                break;
            }
        }
        *cell = p;
    }
    out
}

/// The fuel's masses and places with `trim`: what stays in the tanks, and
/// what each cell holds.
fn fuel_at(spec: &ClassSpec, fuel: f64, trim: &Trim) -> [(f64, DVec3); 4] {
    let t = trim.clamped();
    let moved = fuel * TRIM_SHARE;
    let cell = |axis: usize, v: f64| (moved * v.abs(), spec.trim_cells[axis * 2 + usize::from(v < 0.0)]);
    let (x, y, z) = (cell(0, t.cells.x), cell(1, t.cells.y), cell(2, t.cells.z));
    [(fuel - x.0 - y.0 - z.0, spec.tank_at), x, y, z]
}

/// Its centre of mass with `fuel` and `load` aboard, trimmed by `trim`.
pub fn centre_of_mass(spec: &ClassSpec, fuel: f64, load: f64, trim: &Trim) -> DVec3 {
    let parts = fuel_at(spec, fuel, trim);
    let mass = spec.dry_mass + fuel + load;
    (spec.dry_com * spec.dry_mass + spec.hold_at * load + parts.iter().map(|(m, at)| *at * *m).sum::<DVec3>()) / mass
}

/// Its inertia about that centre of mass (kg·m²).
pub fn inertia(spec: &ClassSpec, fuel: f64, load: f64, trim: &Trim) -> DMat3 {
    let com = centre_of_mass(spec, fuel, load, trim);
    let point = |m: f64, r: DVec3| (DMat3::IDENTITY * r.length_squared() - DMat3::from_cols(r * r.x, r * r.y, r * r.z)) * m;
    let mut i = spec.dry_inertia + point(spec.dry_mass, spec.dry_com - com) + point(load, spec.hold_at - com);
    for (m, at) in fuel_at(spec, fuel, trim) {
        i += point(m, at - com);
    }
    i
}

/// Its thrusters with the mains at their trimmed shares.
pub fn thrusters<'a>(spec: &'a ClassSpec, trim: &Trim) -> Cow<'a, [Thruster]> {
    if trim.mains.iter().all(|&s| s >= 1.0) {
        return Cow::Borrowed(&spec.thrusters);
    }
    let t = trim.clamped();
    let mut k = 0;
    Cow::Owned(
        spec.thrusters
            .iter()
            .map(|th| {
                if th.role == ThrusterRole::Main {
                    k += 1;
                    Thruster { thrust: th.thrust * t.main(k - 1), ..th.clone() }
                } else {
                    th.clone()
                }
            })
            .collect(),
    )
}

/// How far off balance the main drive is: its torque about the centre of
/// mass at full (N·m, body frame), the distance its thrust line misses the
/// centre of mass by (m: x across, y up), and what holding the nose
/// straight against it costs the thrusters (their push as a share of the drive's).
#[derive(Clone, Copy, Debug, Default)]
pub struct Imbalance {
    pub torque: DVec3,
    pub miss: DVec3,
    pub burn: f64,
}

pub fn imbalance(spec: &ClassSpec, fuel: f64, load: f64, trim: &Trim) -> Imbalance {
    let com = centre_of_mass(spec, fuel, load, trim);
    let ts = thrusters(spec, trim);
    let mains = || ts.iter().filter(|t| t.role == ThrusterRole::Main);
    let thrust: f64 = mains().map(|t| t.thrust).sum();
    if thrust <= 0.0 {
        return Imbalance::default();
    }
    let torque: DVec3 = mains().map(|t| (t.at - com).cross(t.push * t.thrust)).sum();
    // (The centre of thrust: the mains' places, weighted by their thrust.)
    let centre: DVec3 = mains().map(|t| t.at * t.thrust).sum::<DVec3>() / thrust;
    let miss = DVec3::new(centre.x - com.x, centre.y - com.y, 0.0);
    let mass = spec.dry_mass + fuel + load;
    let mut u = Vec::new();
    crate::thrusters::allocate_with(&ts, com, mass, inertia(spec, fuel, load, trim), Some(1.0), DVec3::NEG_Z * thrust, DVec3::ZERO, &mut u);
    let side: f64 = ts.iter().zip(&u).filter(|(t, _)| t.role != ThrusterRole::Main).map(|(t, &x)| t.thrust * x).sum();
    Imbalance { torque, miss, burn: side / thrust }
}

/// The trim that balances it best with `fuel` and `load` aboard: the main
/// drive's torque about the centre of mass as small as it can be made, as
/// little drive given up and as little fuel moved as that allows.
pub fn balance(spec: &ClassSpec, fuel: f64, load: f64) -> Trim {
    let mains = spec.thrusters.iter().filter(|t| t.role == ThrusterRole::Main).count();
    let thrust: f64 = spec.main_thrust.max(1.0);
    let (lo, hi) = spec.shape().mesh.extent();
    let scale = thrust * (hi - lo).length();
    let cost = |t: &Trim| {
        let tq = imbalance_torque(spec, fuel, load, t);
        (tq / scale).length_squared() * 1e6 + 0.02 * t.mains.iter().map(|s| 1.0 - s).sum::<f64>() + 1e-4 * t.cells.length_squared()
    };
    let mut best = Trim { cells: DVec3::ZERO, mains: vec![1.0; mains] };
    let mut now = cost(&best);
    let mut step = 0.25;
    while step > 1e-4 {
        let mut better = false;
        for v in 0..3 + mains {
            for sign in [-1.0, 1.0] {
                let mut t = best.clone();
                if v < 3 {
                    t.cells[v] += sign * step;
                } else {
                    t.mains[v - 3] += sign * step;
                }
                let t = t.clamped();
                let c = cost(&t);
                if c < now - 1e-15 {
                    (best, now, better) = (t, c, true);
                }
            }
        }
        if !better {
            step *= 0.5;
        }
    }
    best
}

/// The main drive's torque at full about the trimmed centre of mass.
fn imbalance_torque(spec: &ClassSpec, fuel: f64, load: f64, trim: &Trim) -> DVec3 {
    let com = centre_of_mass(spec, fuel, load, trim);
    thrusters(spec, trim).iter().filter(|t| t.role == ThrusterRole::Main).map(|t| (t.at - com).cross(t.push * t.thrust)).sum()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::content::content;

    #[test]
    fn a_balanced_drover_pushes_through_its_centre_of_mass() {
        let s = content().hulls.iter().find(|(_, h)| h.key == "hull.drover").map(|(_, h)| h).unwrap();
        let (fuel, load) = (s.fuel_capacity, 0.0);
        let before = imbalance(s, fuel, load, &Trim::default());
        let t = balance(s, fuel, load);
        let after = imbalance(s, fuel, load, &t);
        eprintln!("miss {:.3?} -> {:.4?}; burn {:.4} -> {:.5}; trim {:?}", before.miss, after.miss, before.burn, after.burn, t);
        assert!(before.miss.length() > 0.03, "it starts off balance");
        assert!(after.torque.length() < before.torque.length() * 0.02, "balanced: {:.0} N·m left of {:.0}", after.torque.length(), before.torque.length());
        assert!(after.burn < before.burn * 0.1 + 1e-4, "the thrusters hardly needed: {}", after.burn);
        // (The trim is within its limits.)
        assert_eq!(t, t.clamped());
    }
}
