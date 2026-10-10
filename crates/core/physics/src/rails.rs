//! Bodies on rails: each pair (a body and its parent) moves on an exact
//! Kepler orbit about its shared centre of mass, and each body spins about a
//! tilted axis. A body's own orbit is its place relative to its parent; its
//! parent is pulled back the other way by its share of their mass (the
//! reflex: a star wobbling round its system's balance point, a planet round
//! its moons'). The root's system's centre of mass stays at the origin.
//! Their motion is a function of time only, so nothing that happens in the
//! world can disturb it.

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
    /// Its children that pull it back (index, share of its subsystem's
    /// mass): set by `settle` once the set of bodies is made or changed.
    pub pulled_by: Vec<(usize, f64)>,
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

/// Works out who pulls whom back (`RailBody::pulled_by`), for a set of
/// bodies just made or changed: each attracting child, by its subsystem's
/// share of its parent's (its mass with everything riding on it). Bodies too
/// light to matter (they don't attract) pull nothing back.
pub fn settle(rails: &mut [&mut RailBody]) {
    let mut sub: Vec<f64> = rails.iter().map(|b| if b.attracts { b.mu } else { 0.0 }).collect();
    // (Children come after their parents: gather from the leaves up.)
    for c in (0..rails.len()).rev() {
        if let Some(p) = rails[c].parent {
            sub[p] += sub[c];
        }
    }
    for b in rails.iter_mut() {
        b.pulled_by.clear();
    }
    for c in 0..rails.len() {
        if let (Some(p), Some(_)) = (rails[c].parent, &rails[c].orbit)
            && sub[c] > 0.0
        {
            let share = sub[c] / sub[p].max(f64::MIN_POSITIVE);
            rails[p].pulled_by.push((c, share));
        }
    }
}

/// What body `i`'s children pull it back by (`of`: their orbits' position,
/// velocity or acceleration).
fn pulled_back<B: OnRails>(bodies: &[B], i: usize, of: &dyn Fn(&Orbit) -> DVec3) -> DVec3 {
    bodies[i].rail().pulled_by.iter().filter_map(|&(c, share)| bodies[c].rail().orbit.as_ref().map(|o| of(o) * share)).sum()
}

/// Each body's place relative to its parent (zero for the root), and what
/// its children pull it back by: (relative, reflex), by `of` (its orbit's
/// position, velocity or acceleration).
fn relative_and_reflex<B: OnRails>(bodies: &[B], of: impl Fn(&Orbit) -> DVec3) -> (Vec<DVec3>, Vec<DVec3>) {
    let rel: Vec<DVec3> = bodies.iter().map(|b| b.rail().orbit.as_ref().filter(|_| b.rail().parent.is_some()).map_or(DVec3::ZERO, &of)).collect();
    let back = (0..bodies.len()).map(|i| bodies[i].rail().pulled_by.iter().map(|&(c, share)| rel[c] * share).sum()).collect();
    (rel, back)
}

/// Positions of all bodies (relative to the root system's centre of mass) at time `t`.
pub fn positions<B: OnRails>(bodies: &[B], t: f64, out: &mut Vec<DVec3>) {
    // Each one's place relative to its parent first; then, parents before
    // children, each on its parent less its children's pull (theirs still
    // relative: they come after it).
    out.clear();
    out.extend(bodies.iter().map(|b| b.rail().orbit.as_ref().filter(|_| b.rail().parent.is_some()).map_or(DVec3::ZERO, |o| o.position(t))));
    for (i, b) in bodies.iter().enumerate() {
        let b = b.rail();
        let back: DVec3 = b.pulled_by.iter().map(|&(c, share)| out[c] * share).sum();
        let base = b.parent.map_or(DVec3::ZERO, |p| out[p]);
        out[i] = base + out[i] - back;
    }
}

/// Position of body `i` alone (as `positions` has it, without the others').
pub fn position<B: OnRails>(bodies: &[B], i: usize, t: f64) -> DVec3 {
    chain(bodies, i, &|o: &Orbit| o.position(t))
}

/// Velocity of body `i` relative to the root system's centre of mass.
pub fn velocity<B: OnRails>(bodies: &[B], i: usize, t: f64) -> DVec3 {
    chain(bodies, i, &|o: &Orbit| o.state(t).1)
}

/// Body `i`'s place (or velocity: `of`) up its chain of parents, each less
/// what its children pull it back by.
fn chain<B: OnRails>(bodies: &[B], i: usize, of: &dyn Fn(&Orbit) -> DVec3) -> DVec3 {
    let b = bodies[i].rail();
    let own = match (b.parent, &b.orbit) {
        (Some(parent), Some(orbit)) => chain(bodies, parent, of) + of(orbit),
        _ => DVec3::ZERO,
    };
    own - pulled_back(bodies, i, of)
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
        let (rp, bp) = relative_and_reflex(bodies, |o| o.position(t));
        let (rv, bv) = relative_and_reflex(bodies, |o| o.state(t).1);
        // (On Keplerian rails a pair's separation is accelerated by their gravity alone.)
        let (ra, ba) = relative_and_reflex(bodies, |o| {
            let r = o.position(t);
            -r * (o.mu / r.length().powi(3))
        });
        for (i, b) in bodies.iter().enumerate() {
            let (p, v, a) = match b.rail().parent {
                Some(parent) => (pos[parent] + rp[i], vel[parent] + rv[i], acc[parent] + ra[i]),
                None => (DVec3::ZERO, DVec3::ZERO, DVec3::ZERO),
            };
            pos.push(p - bp[i]);
            vel.push(v - bv[i]);
            acc.push(a - ba[i]);
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
        let mut bodies = [body(None, None, 1.3e20, 7.0e8), body(Some(0), Some(planet.clone()), 4.0e14, 6.4e6), body(Some(1), Some(moon.clone()), 4.9e12, 1.7e6)];
        settle(&mut bodies.iter_mut().collect::<Vec<_>>());
        let mut p = Vec::new();
        positions(&bodies, 5000.0, &mut p);
        // A moon rides its planet; each pair about its shared centre of mass.
        let close = |a: DVec3, b: DVec3| (a - b).length() < 1e-6 * b.length().max(1.0);
        assert!(close(p[2] - p[1], moon.position(5000.0)));
        assert!(close(p[1] - p[0], planet.position(5000.0) * (1.3e20 / (1.3e20 + 4.0e14 + 4.9e12)) + planet.position(5000.0) * ((4.0e14 + 4.9e12) / (1.3e20 + 4.0e14 + 4.9e12)) - moon.position(5000.0) * (4.9e12 / (4.0e14 + 4.9e12))));
        // The system's centre of mass stays put.
        let mus = [1.3e20, 4.0e14, 4.9e12];
        let centre: DVec3 = p.iter().zip(mus).map(|(x, m)| *x * m).sum::<DVec3>() / mus.iter().sum::<f64>();
        assert!(centre.length() < 1.0, "the centre of mass moved {} m", centre.length());
        for (i, at) in p.iter().enumerate() {
            assert!(close(position(&bodies, i, 5000.0), *at), "body {i} alone");
        }
        // And moves not at all (velocities, by their masses).
        let v: DVec3 = (0..3).map(|i| velocity(&bodies, i, 5000.0) * mus[i]).sum::<DVec3>() / mus.iter().sum::<f64>();
        assert!(v.length() < 1e-6, "{}", v.length());

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
