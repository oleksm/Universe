//! Bodies on rails: each moves on an exact Kepler orbit around its parent (or
//! stays put at the origin) and spins about a tilted axis. Their motion is a
//! function of time only, so nothing that happens in the world can disturb it.

use std::f64::consts::TAU;

use glam::{DQuat, DVec3};

use crate::collide::Collider;
use crate::orbit::Orbit;
use crate::surface::Surface;

/// A body on rails: how it moves, how hard it pulls, and its shape.
#[derive(Clone, Debug)]
pub struct RailBody {
    /// Index of the body it orbits. Parents always come before their children.
    pub parent: Option<usize>,
    pub orbit: Option<Orbit>,
    /// Gravitational parameter G * mass.
    pub mu: f64,
    /// Pulls on other bodies. Off for bodies too light to matter: they neither
    /// attract nor count as the dominant body.
    pub attracts: bool,
    /// Base radius (m).
    pub radius: f64,
    /// Orientation of the spin axis (the body's local +Y).
    pub tilt: DQuat,
    /// Sidereal rotation period (s).
    pub day: f64,
    pub collider: Collider,
    /// Its air, if it has any.
    pub atmosphere: Option<crate::atmosphere::Atmosphere>,
}

impl RailBody {
    pub fn rotation(&self, t: f64) -> DQuat {
        self.tilt * DQuat::from_rotation_y((t / self.day).fract() * TAU)
    }

    pub fn angular_velocity(&self) -> DVec3 {
        self.tilt * DVec3::Y * (TAU / self.day)
    }
}

/// Anything Dogma can treat as a body on rails: its motion and shape, and
/// the height function of its surface if it has one.
pub trait OnRails {
    fn rail(&self) -> &RailBody;

    fn surface(&self) -> Option<&dyn Surface> {
        None
    }
}

impl OnRails for RailBody {
    fn rail(&self) -> &RailBody {
        self
    }
}

/// Positions of all bodies (relative to the root, body 0) at time `t`.
pub fn positions<B: OnRails>(bodies: &[B], t: f64, out: &mut Vec<DVec3>) {
    out.clear();
    for b in bodies {
        let b = b.rail();
        let p = match (b.parent, &b.orbit) {
            (Some(parent), Some(orbit)) => out[parent] + orbit.position(t),
            _ => DVec3::ZERO,
        };
        out.push(p);
    }
}

/// Velocity of body `i` relative to the root.
pub fn velocity<B: OnRails>(bodies: &[B], i: usize, t: f64) -> DVec3 {
    let b = bodies[i].rail();
    match (b.parent, &b.orbit) {
        (Some(parent), Some(orbit)) => velocity(bodies, parent, t) + orbit.state(t).1,
        _ => DVec3::ZERO,
    }
}

/// Body states at one moment, extrapolated to nearby times with
/// p + v·τ + ½·a·τ². Over a few seconds the error is millimetres
/// (it grows with the cube of τ), far cheaper than re-solving every orbit.
pub struct Ephemeris {
    t0: f64,
    pos: Vec<DVec3>,
    vel: Vec<DVec3>,
    acc: Vec<DVec3>,
}

impl Ephemeris {
    /// Longest span it's used for (s).
    pub const SPAN: f64 = 5.0;

    /// Exact positions, velocities and accelerations of every body at `t`, for
    /// cheap extrapolation over short spans (a frame's worth of substeps).
    pub fn new<B: OnRails>(bodies: &[B], t: f64) -> Self {
        let n = bodies.len();
        let (mut pos, mut vel, mut acc) = (Vec::with_capacity(n), Vec::with_capacity(n), Vec::with_capacity(n));
        for b in bodies {
            let b = b.rail();
            let (p, v, a) = match (b.parent, &b.orbit) {
                (Some(parent), Some(orbit)) => {
                    let (rp, rv) = orbit.state(t);
                    // On Keplerian rails: accelerated only by the parent's gravity.
                    let ra = -rp * (orbit.mu / rp.length().powi(3));
                    (pos[parent] + rp, vel[parent] + rv, acc[parent] + ra)
                }
                _ => (DVec3::ZERO, DVec3::ZERO, DVec3::ZERO),
            };
            pos.push(p);
            vel.push(v);
            acc.push(a);
        }
        Ephemeris { t0: t, pos, vel, acc }
    }

    pub fn positions(&self, t: f64, out: &mut Vec<DVec3>) {
        let tau = t - self.t0;
        out.clear();
        out.extend(self.pos.iter().zip(&self.vel).zip(&self.acc).map(|((p, v), a)| *p + *v * tau + *a * (0.5 * tau * tau)));
    }
}

/// A rail body's pose and motion at one instant.
#[derive(Clone, Copy, Debug)]
pub struct Frame {
    pub center: DVec3,
    pub velocity: DVec3,
    pub rotation: DQuat,
    /// World frame (rad/s).
    pub angular_velocity: DVec3,
}

impl Frame {
    /// Body `i` at time `t`, given all positions at `t`.
    pub fn of<B: OnRails>(bodies: &[B], i: usize, t: f64, positions: &[DVec3]) -> Self {
        let b = bodies[i].rail();
        Self { center: positions[i], velocity: velocity(bodies, i, t), rotation: b.rotation(t), angular_velocity: b.angular_velocity() }
    }

    /// A world point in the body's own axes, relative to its center.
    pub fn local(&self, p: DVec3) -> DVec3 {
        self.rotation.inverse() * (p - self.center)
    }

    /// Velocity of the body's material at world point `p` (includes spin).
    pub fn velocity_at(&self, p: DVec3) -> DVec3 {
        self.velocity + self.angular_velocity.cross(p - self.center)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testkit::body;

    #[test]
    fn children_ride_on_their_parents() {
        let planet = Orbit::new(1.5e11, 0.02, 0.01, 0.2, 0.3, 0.4, 1.3e20);
        let moon = Orbit::new(4.0e8, 0.05, 0.1, 0.5, 0.6, 0.7, 4.0e14);
        let bodies = [body(None, None, 1.3e20, 7.0e8), body(Some(0), Some(planet.clone()), 4.0e14, 6.4e6), body(Some(1), Some(moon.clone()), 4.9e12, 1.7e6)];
        let mut p = Vec::new();
        positions(&bodies, 5000.0, &mut p);
        assert_eq!(p[0], DVec3::ZERO);
        assert_eq!(p[1], planet.position(5000.0));
        assert_eq!(p[2], p[1] + moon.position(5000.0));
        assert_eq!(velocity(&bodies, 2, 5000.0), planet.state(5000.0).1 + moon.state(5000.0).1);

        // The ephemeris is exact at its moment and close nearby.
        let e = Ephemeris::new(&bodies, 5000.0);
        let mut q = Vec::new();
        e.positions(5000.0, &mut q);
        assert_eq!(p, q);
        positions(&bodies, 5000.0 + Ephemeris::SPAN, &mut p);
        e.positions(5000.0 + Ephemeris::SPAN, &mut q);
        for (a, b) in p.iter().zip(&q) {
            assert!(a.distance(*b) < 0.01, "extrapolation off by {} m", a.distance(*b));
        }
    }
}
