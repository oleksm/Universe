//! The flight computer's allocation: from the push and the turn wanted, how
//! hard each of a hull's thrusters fires (0..1). Each thruster pushes along
//! its own line, at its own place, so it gives a force and a torque about the
//! centre of mass; what a ship can do — strafe or not, how fast it turns
//! about each axis — follows from where its thrusters are.
//!
//! Solved as a small bounded least-squares problem: the settings `u` (each
//! 0..1) that come closest to the wanted force and torque, each measured as
//! the acceleration it makes (the push weighted above the turn, so the
//! throttle the pilot sets is honoured first). Projected coordinate descent,
//! warm-started from the last settings.

use glam::{DMat3, DVec3};

use crate::ship::Thruster;

/// The turn's weight against the push: a rad/s² of angular acceleration
/// counts as much as this many m/s².
const TURN_WEIGHT: f64 = 4.0;
/// Sweeps of the solver.
const SWEEPS: usize = 12;

/// One thruster's force and torque at full thrust (body frame, about `com`).
fn column(t: &Thruster, com: DVec3) -> (DVec3, DVec3) {
    let f = t.push * t.thrust;
    (f, (t.at - com).cross(f))
}

/// The settings for `thrusters` (warm-started from and written to `u`) that
/// come closest to `force` (N) and `torque` (N·m), body frame, for a ship
/// of `mass` with `inertia` about its centre of mass `com`. What they give.
pub fn allocate(thrusters: &[Thruster], com: DVec3, mass: f64, inertia: DMat3, force: DVec3, torque: DVec3, u: &mut Vec<f64>) -> (DVec3, DVec3) {
    let n = thrusters.len();
    u.resize(n, 0.0);
    // Rows in acceleration: force / mass, and the turn's angular acceleration
    // (per axis, by its moment of inertia) weighted.
    let rot = DVec3::new(inertia.x_axis.x, inertia.y_axis.y, inertia.z_axis.z).recip() * TURN_WEIGHT;
    let row = |f: DVec3, t: DVec3| -> [f64; 6] {
        let (a, b) = (f / mass, t * rot);
        [a.x, a.y, a.z, b.x, b.y, b.z]
    };
    let cols: Vec<[f64; 6]> = thrusters.iter().map(|t| {
        let (f, q) = column(t, com);
        row(f, q)
    }).collect();
    let want = row(force, torque);
    // The residual: what the settings give, less what's wanted.
    let mut r = [0.0; 6];
    for (c, &ui) in cols.iter().zip(u.iter()) {
        for k in 0..6 {
            r[k] += c[k] * ui;
        }
    }
    for k in 0..6 {
        r[k] -= want[k];
    }
    for _ in 0..SWEEPS {
        for (i, c) in cols.iter().enumerate() {
            let h: f64 = c.iter().map(|x| x * x).sum();
            if h <= 0.0 {
                continue;
            }
            let g: f64 = c.iter().zip(&r).map(|(a, b)| a * b).sum();
            let next = (u[i] - g / h).clamp(0.0, 1.0);
            let d = next - u[i];
            if d != 0.0 {
                for k in 0..6 {
                    r[k] += c[k] * d;
                }
                u[i] = next;
            }
        }
    }
    thrusters.iter().zip(u.iter()).fold((DVec3::ZERO, DVec3::ZERO), |(f, q), (t, &ui)| {
        let (cf, cq) = column(t, com);
        (f + cf * ui, q + cq * ui)
    })
}

/// How fast `thrusters` can turn a ship of `mass` and `inertia` about each
/// body axis (rad/s², the weaker way of the two), with no push left over to
/// speak of: its turning envelope.
pub fn turn_envelope(thrusters: &[Thruster], com: DVec3, mass: f64, inertia: DMat3) -> DVec3 {
    let axis = |a: DVec3| {
        let i = (inertia * a).dot(a);
        [1.0, -1.0]
            .iter()
            .map(|&s| {
                let mut u = Vec::new();
                // Ask for far more than it has: what comes back is its best.
                let (_, q) = allocate(thrusters, com, mass, inertia, DVec3::ZERO, a * s * i * 100.0, &mut u);
                (q.dot(a) * s / i).max(0.0)
            })
            .fold(f64::INFINITY, f64::min)
    };
    DVec3::new(axis(DVec3::X), axis(DVec3::Y), axis(DVec3::Z))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ship::{cobra, ThrusterRole};

    fn cobra_now() -> (f64, DVec3, DMat3) {
        let s = crate::ship::Ship::new(DVec3::ZERO, DVec3::ZERO, glam::DQuat::IDENTITY);
        (s.mass(), s.centre_of_mass(), s.inertia())
    }

    #[test]
    fn full_throttle_is_both_main_engines_and_no_turn() {
        let (m, com, i) = cobra_now();
        let mut u = Vec::new();
        let (f, q) = allocate(&cobra().thrusters, com, m, i, DVec3::NEG_Z * cobra().main_thrust, DVec3::ZERO, &mut u);
        assert!((f.z + cobra().main_thrust).abs() < 1e3, "{f}");
        assert!(q.length() < 1e3, "no turn: {q}");
        let mains: Vec<f64> = cobra().thrusters.iter().zip(&u).filter(|(t, _)| t.role == ThrusterRole::Main).map(|(_, &x)| x).collect();
        assert!(mains.iter().all(|&x| x > 0.99), "{mains:?}");
    }

    #[test]
    fn a_pure_turn_fires_couples_and_pushes_nowhere() {
        let (m, com, i) = cobra_now();
        let env = turn_envelope(&cobra().thrusters, com, m, i);
        eprintln!("turning envelope (rad/s²): pitch {:.2}, yaw {:.2}, roll {:.2}", env.x, env.y, env.z);
        assert!(env.x > 0.5 && env.y > 0.3 && env.z > 0.5, "it can turn every way: {env}");
        // A modest yaw: done, with next to no push.
        let want = DVec3::Y * 0.3 * i.y_axis.y;
        let mut u = Vec::new();
        let (f, q) = allocate(&cobra().thrusters, com, m, i, DVec3::ZERO, want, &mut u);
        assert!((q - want).length() < 0.05 * want.length(), "{q} vs {want}");
        assert!(f.length() / m < 0.05, "pushes nowhere: {} m/s²", f.length() / m);
    }

    #[test]
    fn every_way_it_pushes_it_can_push_nearly_full_without_turning() {
        // (A layout that balances about the centre of mass: thrusters placed
        // badly would have to throttle back to keep the ship from turning.)
        let (m, com, i) = cobra_now();
        let c = cobra();
        for (d, full) in [(DVec3::X, c.rcs_thrust), (DVec3::NEG_X, c.rcs_thrust), (DVec3::NEG_Y, c.rcs_thrust), (DVec3::Z, c.rcs_thrust), (DVec3::NEG_Z, c.rcs_thrust), (DVec3::Y, c.lift_thrust)] {
            let mut u = Vec::new();
            let (f, q) = allocate(&c.thrusters, com, m, i, d * full, DVec3::ZERO, &mut u);
            let got = f.dot(d) / full;
            eprintln!("{d}: {:.0}% of {:.0} kN, turning {:.3} rad/s²", got * 100.0, full / 1000.0, (i.inverse() * q).length());
            assert!(got > 0.95, "{d}: only {:.0}%", got * 100.0);
        }
    }

    #[test]
    fn strafing_is_balanced_so_it_does_not_turn_the_ship() {
        let (m, com, i) = cobra_now();
        let mut u = Vec::new();
        let want = DVec3::X * 0.5 * cobra().rcs_thrust;
        let (f, q) = allocate(&cobra().thrusters, com, m, i, want, DVec3::ZERO, &mut u);
        assert!((f - want).length() < 0.05 * want.length(), "{f}");
        assert!(q.y.abs() / i.y_axis.y < 0.02, "no yaw from it: {:.3} rad/s²", q.y / i.y_axis.y);
    }
}
