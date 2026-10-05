//! Kepler orbits: the exact two-body motion that rail bodies follow.

use std::f64::consts::TAU;

use glam::{DMat3, DVec3};

/// A Keplerian orbit around a parent with gravitational parameter `mu`.
/// Positions are relative to the parent. The reference plane is XZ (Y is up).
#[derive(Clone, Debug)]
pub struct Orbit {
    pub semi_major_axis: f64,
    pub eccentricity: f64,
    pub mu: f64,
    /// Mean anomaly at t = 0.
    pub mean_anomaly_epoch: f64,
    /// Perifocal (periapsis direction, 90° ahead, normal) to world.
    basis: DMat3,
    mean_motion: f64,
}

impl Orbit {
    pub fn new(a: f64, e: f64, inclination: f64, ascending_node: f64, periapsis_arg: f64, m0: f64, mu: f64) -> Self {
        // Classic Z-up rotation, then swap to the engine's Y-up frame: (x, y, z) -> (x, z, -y).
        let z_up = DMat3::from_rotation_z(ascending_node)
            * DMat3::from_rotation_x(inclination)
            * DMat3::from_rotation_z(periapsis_arg);
        let y_up = DMat3::from_cols(DVec3::X, DVec3::NEG_Z, DVec3::Y);
        Self {
            semi_major_axis: a,
            eccentricity: e,
            mu,
            mean_anomaly_epoch: m0,
            basis: y_up * z_up,
            mean_motion: (mu / (a * a * a)).sqrt(),
        }
    }

    /// The orbit through position `r` with velocity `v` (relative to the
    /// parent) at time `t`. Elliptic orbits only (`v` below escape speed).
    pub fn from_state(r: DVec3, v: DVec3, mu: f64, t: f64) -> Self {
        let h = r.cross(v);
        let e_vec = v.cross(h) / mu - r.normalize();
        let e = e_vec.length();
        let a = 1.0 / (2.0 / r.length() - v.length_squared() / mu);
        let w = h.normalize();
        // Periapsis direction (any in-plane direction for a circle: then
        // the eccentric anomaly is measured from `r` itself).
        let circular = e < 1e-12;
        let p = if circular { r.normalize() } else { e_vec / e };
        let q = w.cross(p);
        let ea = if circular { 0.0 } else { (r.dot(v) / (e * (mu * a).sqrt())).atan2((1.0 - r.length() / a) / e) };
        let mean_motion = (mu / (a * a * a)).sqrt();
        let m = ea - e * ea.sin();
        Self {
            semi_major_axis: a,
            eccentricity: if circular { 0.0 } else { e },
            mu,
            mean_anomaly_epoch: (m - mean_motion * t).rem_euclid(TAU),
            basis: DMat3::from_cols(p, q, w),
            mean_motion,
        }
    }

    /// This orbit with another size, shape or pull: the same plane, the same
    /// way round, the same place along it at t = 0.
    pub fn reshaped(&self, a: f64, e: f64, mu: f64) -> Self {
        Self { semi_major_axis: a, eccentricity: e, mu, mean_anomaly_epoch: self.mean_anomaly_epoch, basis: self.basis, mean_motion: (mu / (a * a * a)).sqrt() }
    }

    /// How far its plane is tilted from the reference plane (rad).
    pub fn inclination(&self) -> f64 {
        self.normal().y.clamp(-1.0, 1.0).acos()
    }

    /// This orbit tilted to `inclination` (rad) about its own line of nodes:
    /// the same size and shape, the same nodes.
    pub fn inclined(&self, inclination: f64) -> Self {
        let n = self.normal();
        let node = DVec3::Y.cross(n);
        let axis = if node.length_squared() > 1e-24 { node.normalize() } else { self.basis.x_axis };
        let mut o = self.clone();
        o.basis = DMat3::from_axis_angle(axis, inclination - self.inclination()) * self.basis;
        o
    }

    /// The orbit's normal (unit): the way its angular momentum points.
    pub fn normal(&self) -> DVec3 {
        (self.basis * DVec3::Z).normalize()
    }

    pub fn period(&self) -> f64 {
        TAU / self.mean_motion
    }

    pub fn periapsis(&self) -> f64 {
        self.semi_major_axis * (1.0 - self.eccentricity)
    }

    pub fn apoapsis(&self) -> f64 {
        self.semi_major_axis * (1.0 + self.eccentricity)
    }

    fn eccentric_anomaly(&self, t: f64) -> f64 {
        let m = (self.mean_anomaly_epoch + self.mean_motion * t).rem_euclid(TAU);
        let e = self.eccentricity;
        if e == 0.0 {
            return m; // circular: nothing to solve
        }
        // A good first guess means Newton usually converges in 2-3 steps;
        // near-parabolic (a comet's e of 0.99999) near its closest, Danby's
        // start (M + 0.85 e, toward sin M) and more steps.
        let mut ea = if e < 0.8 { m + e * m.sin() * (1.0 + e * m.cos()) } else { m + 0.85 * e * m.sin().signum() };
        for _ in 0..60 {
            let delta = (ea - e * ea.sin() - m) / (1.0 - e * ea.cos());
            ea -= delta;
            if delta.abs() < 1e-12 {
                break;
            }
        }
        ea
    }

    fn perifocal(&self, ea: f64) -> DVec3 {
        let (a, e) = (self.semi_major_axis, self.eccentricity);
        let b = a * (1.0 - e * e).sqrt();
        self.basis * DVec3::new(a * (ea.cos() - e), b * ea.sin(), 0.0)
    }

    pub fn position(&self, t: f64) -> DVec3 {
        self.perifocal(self.eccentric_anomaly(t))
    }

    /// Position and velocity relative to the parent at time `t`.
    pub fn state(&self, t: f64) -> (DVec3, DVec3) {
        let ea = self.eccentric_anomaly(t);
        let (a, e) = (self.semi_major_axis, self.eccentricity);
        let b = a * (1.0 - e * e).sqrt();
        let rate = self.mean_motion / (1.0 - e * ea.cos());
        let vel = self.basis * DVec3::new(-a * ea.sin() * rate, b * ea.cos() * rate, 0.0);
        (self.perifocal(ea), vel)
    }

    /// `n` points evenly spaced in eccentric anomaly, for drawing the path.
    pub fn path(&self, n: usize) -> impl Iterator<Item = DVec3> + '_ {
        (0..n).map(move |i| self.perifocal(i as f64 / n as f64 * TAU))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn orbit_closes_and_keeps_its_energy() {
        let o = Orbit::new(7.0e6, 0.2, 0.4, 0.3, 1.1, 0.7, 3.986e14);
        let energy = |t: f64| {
            let (p, v) = o.state(t);
            0.5 * v.length_squared() - o.mu / p.length()
        };
        let e0 = energy(0.0);
        for k in 1..=50 {
            let t = k as f64 * o.period() * 0.37;
            assert!(((energy(t) - e0) / e0).abs() < 1e-12, "vis-viva energy drifted at t = {t}");
        }
        // One whole period later the body is back where it started.
        let (p0, p1) = (o.position(123.0), o.position(123.0 + o.period()));
        assert!(p0.distance(p1) < 1e-6 * o.semi_major_axis);
        assert!((p0.length() - o.periapsis()) >= -1e-6 && (p0.length() - o.apoapsis()) <= 1e-6);
        // A comet's near-parabolic orbit (e 0.99999) solves all the way round, its closest pass too.
        let c = Orbit::new(3.0e14, 0.99999, 0.3, 0.0, 0.0, 0.0, 1.3e20);
        for k in 0..2000 {
            let t = k as f64 / 2000.0 * TAU / c.mean_motion;
            let (ea, m) = (c.eccentric_anomaly(t), (c.mean_anomaly_epoch + c.mean_motion * t).rem_euclid(TAU));
            assert!((ea - c.eccentricity * ea.sin() - m).abs() < 1e-9, "t {t}: E {ea} doesn't solve M {m}");
        }
    }

}

