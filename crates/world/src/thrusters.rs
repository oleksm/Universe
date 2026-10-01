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
/// What firing costs, against missing what's wanted: a thruster's use
/// counts this much of its own push squared. Small, so what's asked comes
/// first; but of the settings that give the same, the one that burns the
/// least wins — the drives evenly, no thrusters pushing against each
/// other (many settings give the same push and turn: without it the solver
/// keeps whichever it found, a pair fighting each other included).
const FUEL_WEIGHT: f64 = 1.0e-3;
/// In flight, the thrusters start each step from this share of their last settings.
const FORGET: f64 = 0.8;

/// One thruster's force and torque at full thrust (body frame, about `com`).
fn column(t: &Thruster, com: DVec3) -> (DVec3, DVec3) {
    let f = t.push * t.thrust;
    (f, (t.at - com).cross(f))
}

/// The settings for `thrusters` (warm-started from and written to `u`) that
/// come closest to `force` (N) and `torque` (N·m), body frame, for a ship
/// of `mass` with `inertia` about its centre of mass `com`. What they give.
pub fn allocate(thrusters: &[Thruster], com: DVec3, mass: f64, inertia: DMat3, force: DVec3, torque: DVec3, u: &mut Vec<f64>) -> (DVec3, DVec3) {
    allocate_with(thrusters, com, mass, inertia, None, force, torque, u)
}

/// `allocate`, with the main drive's nozzles (if `drive` is given) held
/// together at that level (0..1): the throttle, as a flight computer
/// sets it. The rest — the thrusters and the lift — make up the push and
/// the turn around it: what the drive turns the ship by, off its centre of
/// mass, they hold off.
#[allow(clippy::too_many_arguments)]
pub fn allocate_with(thrusters: &[Thruster], com: DVec3, mass: f64, inertia: DMat3, drive: Option<f64>, force: DVec3, torque: DVec3, u: &mut Vec<f64>) -> (DVec3, DVec3) {
    let n = thrusters.len();
    u.resize(n, 0.0);
    let held = |i: usize| drive.is_some() && thrusters[i].role == crate::ship::ThrusterRole::Main;
    if let Some(level) = drive {
        for (i, t) in thrusters.iter().enumerate() {
            if t.role == crate::ship::ThrusterRole::Main {
                u[i] = level.clamp(0.0, 1.0);
            } else {
                // (What the thrusters were set to fades: what's still needed
                // comes straight back, a pair pushing against each other
                // dies away. Not the lift: it carries the ship, and from
                // less each step a climb out loses its footing — its
                // imbalances unwind by `FUEL_WEIGHT` alone.)
                if t.role == crate::ship::ThrusterRole::Rcs {
                    u[i] *= FORGET;
                }
            }
        }
    }
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
    // (Each thruster's own push, as an acceleration, squared: its fuel cost's scale.)
    let costs: Vec<f64> = cols.iter().map(|c| FUEL_WEIGHT * (c[0] * c[0] + c[1] * c[1] + c[2] * c[2])).collect();
    for _ in 0..SWEEPS {
        for (i, c) in cols.iter().enumerate() {
            let h: f64 = c.iter().map(|x| x * x).sum::<f64>() + costs[i];
            if h <= 0.0 || held(i) {
                continue;
            }
            let g: f64 = c.iter().zip(&r).map(|(a, b)| a * b).sum::<f64>() + costs[i] * u[i];
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

/// Turning left over that counts, as a share of what the ship can turn by
/// (its weakest axis): less, and its flight computer holds it straight
/// without noticing.
pub const STRAIGHT: f64 = 0.03;

/// How hard `thrusters` can push a ship of `mass` and `inertia` (its centre
/// of mass at `com`) along unit `d`, up to `full` (N), turning it by no more
/// than `straight` (rad/s²): off balance, they give less (N).
#[allow(clippy::too_many_arguments)]
pub fn balanced(thrusters: &[Thruster], com: DVec3, mass: f64, inertia: DMat3, d: DVec3, full: f64, straight: f64) -> f64 {
    balanced_with(thrusters, com, mass, inertia, d, full, straight, false)
}

/// `balanced`, for the main drive (`drive`: its nozzles held together at
/// the level tried, the rest holding the ship straight).
#[allow(clippy::too_many_arguments)]
pub fn balanced_with(thrusters: &[Thruster], com: DVec3, mass: f64, inertia: DMat3, d: DVec3, full: f64, straight: f64, drive: bool) -> f64 {
    if full <= 0.0 {
        return 0.0;
    }
    let inverse = inertia.inverse();
    let mut u = Vec::new();
    let mut try_at = |k: f64| {
        // (From cold, the solver needs more sweeps than a frame's, where it
        // starts from the last; this is worked out once a tonne of load.)
        let mut fq = (DVec3::ZERO, DVec3::ZERO);
        for _ in 0..12 {
            fq = allocate_with(thrusters, com, mass, inertia, drive.then_some(k), d * full * k, DVec3::ZERO, &mut u);
        }
        let (f, q) = fq;
        ((inverse * q).length() < straight).then_some(f.dot(d).max(0.0))
    };
    if let Some(f) = try_at(1.0) {
        return f;
    }
    // The most it can ask for and still go straight.
    let (mut lo, mut hi, mut best) = (0.0, 1.0, 0.0);
    for _ in 0..10 {
        let mid = 0.5 * (lo + hi);
        match try_at(mid) {
            Some(f) => {
                (lo, best) = (mid, f);
            }
            None => hi = mid,
        }
    }
    best
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ship::{starter, ThrusterRole};

    /// The starter at the load it's balanced for: a full tank, its hold half full.
    fn starter_now() -> (f64, DVec3, DMat3) {
        let mut s = crate::ship::Ship::new(DVec3::ZERO, DVec3::ZERO, glam::DQuat::IDENTITY);
        s.cargo = s.spec().hold_capacity / 2.0;
        (s.mass(), s.centre_of_mass(), s.inertia())
    }

    /// `allocate` run to convergence from cold (in flight it starts from the last frame's).
    fn settled(thrusters: &[Thruster], com: DVec3, m: f64, i: DMat3, force: DVec3, torque: DVec3, u: &mut Vec<f64>) -> (DVec3, DVec3) {
        let mut out = (DVec3::ZERO, DVec3::ZERO);
        for _ in 0..12 {
            out = allocate(thrusters, com, m, i, force, torque, u);
        }
        out
    }

    #[test]
    fn full_throttle_is_both_main_engines_and_no_turn() {
        let (m, com, i) = starter_now();
        let mut u = Vec::new();
        // (As in flight: the drive at the throttle, the rest holding it straight.)
        let mut fq = (DVec3::ZERO, DVec3::ZERO);
        for _ in 0..12 {
            fq = allocate_with(&starter().thrusters, com, m, i, Some(1.0), DVec3::NEG_Z * starter().main_thrust, DVec3::ZERO, &mut u);
        }
        let (f, q) = fq;
        assert!((f.z + starter().main_thrust).abs() < 1e3, "{f}");
        assert!((i.inverse() * q).length() < 0.01, "no turn: {q}");
        let mains: Vec<f64> = starter().thrusters.iter().zip(&u).filter(|(t, _)| t.role == ThrusterRole::Main).map(|(_, &x)| x).collect();
        assert!(mains.iter().all(|&x| x > 0.99), "{mains:?}");
    }

    #[test]
    fn a_pure_turn_fires_couples_and_pushes_nowhere() {
        let (m, com, i) = starter_now();
        let env = turn_envelope(&starter().thrusters, com, m, i);
        eprintln!("turning envelope (rad/s²): pitch {:.2}, yaw {:.2}, roll {:.2}", env.x, env.y, env.z);
        assert!(env.x > 0.5 && env.y > 0.3 && env.z > 0.5, "it can turn every way: {env}");
        // A modest yaw: done, with next to no push.
        let want = DVec3::Y * 0.3 * i.y_axis.y;
        let mut u = Vec::new();
        let (f, q) = allocate(&starter().thrusters, com, m, i, DVec3::ZERO, want, &mut u);
        assert!((q - want).length() < 0.05 * want.length(), "{q} vs {want}");
        assert!(f.length() / m < 0.05, "pushes nowhere: {} m/s²", f.length() / m);
    }

    #[test]
    fn every_way_it_pushes_it_can_push_nearly_full_without_turning() {
        // (A layout that balances about the centre of mass: thrusters placed
        // badly would have to throttle back to keep the ship from turning.)
        let (m, com, i) = starter_now();
        let c = starter();
        for (d, full) in [(DVec3::X, c.rcs_thrust), (DVec3::NEG_X, c.rcs_thrust), (DVec3::NEG_Y, c.rcs_thrust), (DVec3::Z, c.rcs_thrust), (DVec3::NEG_Z, c.rcs_thrust), (DVec3::Y, c.lift_thrust)] {
            let mut u = Vec::new();
            let (f, q) = settled(&c.thrusters, com, m, i, d * full, DVec3::ZERO, &mut u);
            let got = f.dot(d) / full;
            eprintln!("{d}: {:.0}% of {:.0} kN, turning {:.3} rad/s²", got * 100.0, full / 1000.0, (i.inverse() * q).length());
            assert!(got > 0.95, "{d}: only {:.0}%", got * 100.0);
        }
    }

    #[test]
    fn going_straight_it_burns_the_least_no_thrusters_fighting() {
        // As the pilot found it: the drives uneven, thrusters pushing against
        // each other (left over from a turn), and now a straight push asked.
        let (m, com, i) = starter_now();
        let c = starter();
        let mut u: Vec<f64> = c.thrusters.iter().map(|t| match (t.role, t.nozzle.as_str()) {
            (ThrusterRole::Main, "nozzle_main_0") => 0.81,
            (ThrusterRole::Main, _) => 0.17,
            (ThrusterRole::Rcs, _) => 0.5,
            _ => 0.2,
        }).collect();
        let want = DVec3::NEG_Z * 0.8 * c.main_thrust;
        // Half a second of flight (each step from the last), the throttle at 80%.
        let mut got = (DVec3::ZERO, DVec3::ZERO);
        for _ in 0..30 {
            got = allocate_with(&c.thrusters, com, m, i, Some(0.8), want, DVec3::ZERO, &mut u);
        }
        let mains: Vec<f64> = c.thrusters.iter().zip(&u).filter(|(t, _)| t.role == ThrusterRole::Main).map(|(_, &x)| x).collect();
        let by = |role: ThrusterRole| c.thrusters.iter().zip(&u).filter(|(t, _)| t.role == role).map(|(t, &x)| t.thrust * x).sum::<f64>();
        let lift_before: f64 = c.thrusters.iter().filter(|t| t.role == ThrusterRole::Lift).map(|t| t.thrust * 0.2).sum();
        assert!((got.0 - want).length() < 0.01 * want.length(), "{:?}", got.0);
        assert!((mains[0] - mains[1]).abs() < 0.02, "the drives evenly: {mains:?}");
        // (A little stays: the couple that holds off the drive's turn, its line
        // a few centimetres off the centre of mass — not always the cheapest.)
        assert!(by(ThrusterRole::Rcs) < 0.02 * want.length(), "no thrusters pushing against each other: {:.0} kN", by(ThrusterRole::Rcs) / 1000.0);
        assert!(by(ThrusterRole::Lift) < lift_before / 5.0, "the lift's imbalance unwinding: {:.0} kN of {:.0}", by(ThrusterRole::Lift) / 1000.0, lift_before / 1000.0);
    }

    #[test]
    fn strafing_is_balanced_so_it_does_not_turn_the_ship() {
        let (m, com, i) = starter_now();
        let mut u = Vec::new();
        let want = DVec3::X * 0.5 * starter().rcs_thrust;
        let (f, q) = allocate(&starter().thrusters, com, m, i, want, DVec3::ZERO, &mut u);
        assert!((f - want).length() < 0.05 * want.length(), "{f}");
        assert!(q.y.abs() / i.y_axis.y < 0.02, "no yaw from it: {:.3} rad/s²", q.y / i.y_axis.y);
    }
}
