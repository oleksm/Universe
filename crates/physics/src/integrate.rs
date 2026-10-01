//! The integrator: every free body moves by the same rule. Leapfrog
//! (kick-drift-kick) under the rail bodies' gravity plus whatever acceleration
//! its `Driver` applies, in adaptive substeps, with contacts checked after
//! each substep and reported as facts. In an atmosphere, drag follows each
//! substep (exactly, so any step is stable: see `atmosphere::drag`).

use glam::DVec3;

use crate::atmosphere::{air_at, drag};
use crate::body::RigidBody;
use crate::collide::{detect, Fact};
use crate::ops::bounce;
use crate::query::{dominant, gravity};
use crate::rails::{positions as rail_positions, Ephemeris, OnRails};

/// Upper bound on substeps per call; beyond this the call covers less time
/// than asked (see `Outcome::limited`).
pub const MAX_SUBSTEPS: u32 = 2000;
/// Within this distance of a small collider (see `Collider::is_small`)...
pub const FINE_RANGE: f64 = 30_000.0;
/// ...substeps are no longer than this (s), so contacts are caught precisely.
pub const FINE_STEP: f64 = 0.05;

/// What pushes a body, and what happens when it touches something. Supplied
/// by the caller: the kernel only integrates and reports.
pub trait Driver {
    /// Applied acceleration (force / mass, m/s²; gravity excluded) for the
    /// substep of `h` seconds starting at `t`, with the rail bodies at
    /// `positions`. May also turn `body`.
    fn applied(&mut self, body: &mut RigidBody, t: f64, h: f64, positions: &[DVec3]) -> DVec3;

    /// A substep ended with `fact`: how the integration goes on.
    fn respond(&mut self, _fact: &Fact, _body: &RigidBody) -> Response {
        Response::Stop
    }
}

#[derive(Clone, Copy, Debug)]
pub enum Response {
    /// Carry on as if nothing happened.
    Continue,
    /// End the call here (the fact is in the `Outcome`).
    Stop,
    /// Bounce off the contact (see `ops::bounce`) and carry on.
    Bounce { restitution: f64, separation: f64, push: f64 },
}

/// One call's worth of time: from `t`, for `dt` seconds, in substeps no longer
/// than `max_h` (pass infinity for no limit beyond the kernel's own), and no
/// longer than `contact_step` within `FINE_RANGE` of a small collider (flight
/// uses `FINE_STEP`; a coarse look-ahead may pass more, trading contact
/// precision for speed).
#[derive(Clone, Copy, Debug)]
pub struct Span {
    pub t: f64,
    pub dt: f64,
    pub max_h: f64,
    pub contact_step: f64,
}

#[derive(Clone, Copy, Debug)]
pub struct Outcome {
    /// Time at the end of the last substep taken.
    pub time: f64,
    /// Seconds covered.
    pub simulated: f64,
    /// Ran out of substeps before covering `dt`.
    pub limited: bool,
    /// The fact the driver stopped on, if it did.
    pub fact: Option<Fact>,
}

/// One kick-drift-kick (leapfrog) step of `h`: stable over long orbits.
/// `accel(p, end)` is the acceleration at `p` at the start (`end == false`)
/// or the end of the step.
pub fn leapfrog(pos: &mut DVec3, vel: &mut DVec3, h: f64, mut accel: impl FnMut(DVec3, bool) -> DVec3) {
    *vel += accel(*pos, false) * (h * 0.5);
    *pos += *vel * h;
    *vel += accel(*pos, true) * (h * 0.5);
}

/// Advance `body` through `span`. With an `ephemeris` (valid around
/// `span.t`), rail bodies are extrapolated from it instead of solved exactly
/// at every substep. `positions` is scratch space; on return it holds the rail
/// bodies at `Outcome::time`.
pub fn integrate<B: OnRails>(
    bodies: &[B],
    ephemeris: Option<&Ephemeris>,
    positions: &mut Vec<DVec3>,
    body: &mut RigidBody,
    span: Span,
    driver: &mut impl Driver,
) -> Outcome {
    let place = |t: f64, out: &mut Vec<DVec3>| match ephemeris {
        Some(e) => e.positions(t, out),
        None => rail_positions(bodies, t, out),
    };
    let mut t = span.t;
    place(t, positions);
    let mut max_h = span.max_h;
    if bodies.iter().zip(positions.iter()).any(|(b, p)| b.rail().collider.is_small(b.rail().radius) && p.distance(body.position) < FINE_RANGE) {
        max_h = max_h.min(span.contact_step);
    }
    let mut remaining = span.dt;
    let mut steps = 0;
    let mut limited = false;
    while remaining > 1e-9 {
        if steps >= MAX_SUBSTEPS {
            limited = true;
            break;
        }
        // A hundredth of the dominant body's orbital time scale at this distance.
        let dom = dominant(bodies, body.position, positions);
        let r = body.position.distance(positions[dom]);
        let h = (0.01 * (r * r * r / bodies[dom].rail().mu).sqrt()).clamp(0.01, 3600.0).min(max_h).min(remaining);

        let applied = driver.applied(body, t, h, positions);
        let prev = body.position;
        let end = t + h;
        leapfrog(&mut body.position, &mut body.velocity, h, |p, at_end| {
            if at_end {
                place(end, positions);
            }
            gravity(bodies, p, positions) + applied
        });
        if body.ballistic > 0.0
            && let Some((density, air)) = air_at(bodies, body.position, positions, end)
        {
            body.velocity = drag(body.velocity, air, density, body.ballistic, h);
        }
        t = end;
        remaining -= h;
        steps += 1;

        if let Some(fact) = detect(bodies, t, positions, prev, body, h) {
            match driver.respond(&fact, body) {
                Response::Continue => {}
                Response::Bounce { restitution, separation, push } => {
                    if let Fact::Contact(c) = &fact {
                        bounce(body, c, restitution, separation, push);
                    }
                }
                Response::Stop => return Outcome { time: t, simulated: span.dt - remaining, limited, fact: Some(fact) },
            }
        }
    }
    Outcome { time: t, simulated: span.dt - remaining, limited, fact: None }
}

#[cfg(test)]
mod tests {
    use glam::DQuat;

    use super::*;
    use crate::collide::{Blocks, Collider, Feature, Ring};
    use crate::orbit::Orbit;
    
    use crate::rails::{positions, RailBody};
    use crate::testkit::body;

    /// Coasting: no applied acceleration, stop on any contact.
    struct Coast;

    impl Driver for Coast {
        fn applied(&mut self, _: &mut RigidBody, _: f64, _: f64, _: &[DVec3]) -> DVec3 {
            DVec3::ZERO
        }
    }

    /// Thrust along the nose, turning slowly: something with state to replay.
    struct Wander {
        accel: f64,
        turned: f64,
    }

    impl Driver for Wander {
        fn applied(&mut self, body: &mut RigidBody, _: f64, h: f64, _: &[DVec3]) -> DVec3 {
            self.turned += h;
            body.orientation = (body.orientation * DQuat::from_rotation_y(0.01 * h)).normalize();
            body.orientation * DVec3::NEG_Z * self.accel
        }

        fn respond(&mut self, fact: &Fact, _: &RigidBody) -> Response {
            match fact {
                Fact::Contact(c) if c.feature == Feature::Hull => Response::Bounce { restitution: 0.4, separation: 0.5, push: 2.0 },
                _ => Response::Continue,
            }
        }
    }

    /// A star, a planet, its moon, and a polytope and a ring orbiting the planet.
    fn system() -> Vec<RailBody> {
        let star = body(None, None, 1.3e20, 7.0e8);
        let planet = body(Some(0), Some(Orbit::new(1.5e11, 0.02, 0.01, 0.2, 0.3, 0.4, 1.3e20)), 4.0e14, 6.4e6);
        let moon = body(Some(1), Some(Orbit::new(4.0e8, 0.05, 0.1, 0.5, 0.6, 0.7, 4.0e14)), 4.9e12, 1.7e6);
        let mut cube = body(Some(1), Some(Orbit::new(1.0e7, 0.0, 0.2, 0.0, 0.0, 1.0, 4.0e14)), 1.0, 600.0);
        cube.attracts = false;
        cube.day = 60.0;
        cube.collider = Collider::Blocks(Blocks::new(vec![(DVec3::splat(-500.0), DVec3::splat(500.0))], vec![]));
        let mut hoop = body(Some(1), Some(Orbit::new(2.0e7, 0.0, 0.1, 1.0, 0.0, 2.0, 4.0e14)), 1.0, 1560.0);
        hoop.attracts = false;
        hoop.collider = Collider::Ring(Ring { radius: 1500.0, tube: 60.0 });
        vec![star, planet, moon, cube, hoop]
    }

    fn hash(h: &mut u64, v: DVec3) {
        for x in [v.x, v.y, v.z] {
            *h ^= x.to_bits();
            *h = h.wrapping_mul(0x100_0000_01b3);
        }
    }

    /// Several bodies around the planet, one near the polytope, frames of
    /// 1/60 s at 20x sharing one ephemeris per frame; a hash of every state.
    fn run() -> u64 {
        let bodies = system();
        let mut p = Vec::new();
        positions(&bodies, 0.0, &mut p);
        let (planet, cube) = (p[1], p[3]);
        let v_planet = crate::rails::velocity(&bodies, 1, 0.0);
        let v_cube = crate::rails::velocity(&bodies, 3, 0.0);
        let circular = (4.0e14f64 / 7.0e6).sqrt();
        let mut probes = [
            RigidBody::new(planet + DVec3::X * 7.0e6, v_planet + DVec3::Z * circular, DQuat::IDENTITY, 12.0),
            RigidBody::new(planet + DVec3::Y * 9.0e6, v_planet + DVec3::X * 6000.0, DQuat::from_rotation_x(0.3), 12.0),
            RigidBody::new(cube + DVec3::new(0.0, 2000.0, 300.0), v_cube, DQuat::from_rotation_x(1.2), 12.0),
        ];
        let mut drivers = [Wander { accel: 0.0, turned: 0.0 }, Wander { accel: 3.0, turned: 0.0 }, Wander { accel: 0.5, turned: 0.0 }];
        let mut h = 0xcbf2_9ce4_8422_2325u64;
        let mut t = 0.0;
        for _ in 0..600 {
            let dt = 20.0 / 60.0;
            let e = Ephemeris::new(&bodies, t);
            for (probe, d) in probes.iter_mut().zip(drivers.iter_mut()) {
                let out = integrate(&bodies, Some(&e), &mut p, probe, Span { t, dt, max_h: f64::INFINITY, contact_step: FINE_STEP }, d);
                assert!(!out.limited && out.fact.is_none());
                hash(&mut h, probe.position);
                hash(&mut h, probe.velocity);
            }
            t += dt;
        }
        h
    }

    #[test]
    fn same_inputs_same_world() {
        let (a, b) = (run(), run());
        assert_eq!(a, b, "a multi-body run must be reproducible bit for bit");
    }

    #[test]
    fn a_circular_orbit_stays_circular() {
        let planet = [body(None, None, 4.0e14, 6.4e6)];
        let r0: f64 = 7.0e6;
        let v0 = (4.0e14 / r0).sqrt();
        let mut probe = RigidBody::new(DVec3::X * r0, DVec3::Z * v0, DQuat::IDENTITY, 12.0);
        let energy = |s: &RigidBody| 0.5 * s.velocity.length_squared() - 4.0e14 / s.position.length();
        let e0 = energy(&probe);
        let period = std::f64::consts::TAU * (r0 * r0 * r0 / 4.0e14).sqrt();
        let mut p = Vec::new();
        let mut t = 0.0;
        let mut worst: f64 = 0.0;
        while t < 10.0 * period {
            let out = integrate(&planet, None, &mut p, &mut probe, Span { t, dt: 60.0, max_h: f64::INFINITY, contact_step: FINE_STEP }, &mut Coast);
            assert!(out.fact.is_none());
            t = out.time;
            worst = worst.max((probe.position.length() - r0).abs() / r0);
        }
        eprintln!("10 orbits: worst radius drift {:.2e}, energy drift {:.2e}", worst, (energy(&probe) - e0) / e0);
        assert!(worst < 1e-3);
        assert!(((energy(&probe) - e0) / e0).abs() < 1e-4);
    }

    #[test]
    fn stop_and_bounce_on_contact() {
        let mut still = body(None, None, 1.0, 600.0);
        still.attracts = false;
        still.day = 1.0e15;
        still.collider = Collider::Blocks(Blocks::new(vec![(DVec3::splat(-500.0), DVec3::splat(500.0))], vec![]));
        let bodies = [still];
        let approach = RigidBody::new(DVec3::X * 600.0, DVec3::NEG_X * 5.0, DQuat::IDENTITY, 12.0);
        let mut p = Vec::new();

        // Stopping: the call ends at the contact, reporting it.
        let mut probe = approach;
        let out = integrate(&bodies, None, &mut p, &mut probe, Span { t: 0.0, dt: 60.0, max_h: f64::INFINITY, contact_step: FINE_STEP }, &mut Coast);
        let Some(Fact::Contact(c)) = out.fact else { panic!("expected a contact, got {:?}", out.fact) };
        assert_eq!(c.feature, Feature::Hull);
        assert!(c.normal.distance(DVec3::X) < 1e-12);
        assert!(out.simulated < 60.0 && (probe.position.x - 512.0).abs() < 1.0);

        // Bouncing: back out the way it came, a little slower.
        let mut probe = approach;
        let out = integrate(&bodies, None, &mut p, &mut probe, Span { t: 0.0, dt: 60.0, max_h: f64::INFINITY, contact_step: FINE_STEP }, &mut Wander { accel: 0.0, turned: 0.0 });
        assert!(out.fact.is_none() && (out.simulated - 60.0).abs() < 1e-6);
        assert!(probe.velocity.x > 0.0 && probe.velocity.x < 5.0, "velocity after bouncing {:?}", probe.velocity);
    }

}
