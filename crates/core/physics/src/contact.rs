//! Resting on and sliding over solid ground: the forces at every point of a
//! body that touches it (Dogma's contact laws, `docs/ground-contact-plan.md`).
//!
//! A body touches by its feet (each the pad of a landing leg: an oleo strut,
//! a spring and a damper over its stroke) and by its hull (its contact
//! spheres). Every point that is in the ground pushes back along the ground's
//! normal there (its slope, not the vertical) and grips by Coulomb friction.
//! The sum of these forces, and their torques about the centre of mass, move
//! and turn the body through its mass and inertia: so it rocks on its legs,
//! tips over its feet, slides down a slope steeper than its grip, rests on its
//! belly.
//!
//! The strut, from the figures a leg carries (`holds`, `stroke`, `sink_rate`):
//! its spring reaches `holds` at the end of its stroke; its damper takes 0.7
//! of `holds` at its design sink rate (the registry's rule, where a leg's
//! record doesn't give its own). Past its stroke it is bottomed: the
//! hull's hard contact. Hard contacts (the hull, a bottomed strut) and
//! friction are worked out as the push that brings the point's speed to what
//! it should be by the end of the step, so a step of any size stays stable.

use glam::{DMat3, DQuat, DVec3};

use crate::body::RigidBody;
use crate::laws::{FOOT_FRICTION, HULL_FRICTION, HULL_REST_SPEED, HULL_RESTITUTION};

/// A landing leg: where its pad is (body frame, fully extended), what it holds
/// before it gives way (N), its stroke (m), the sink rate it's designed for
/// (m/s), and its spring (N/m) and damper (N s/m) where its record gives them.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Strut {
    pub foot: DVec3,
    pub holds: f64,
    pub stroke: f64,
    pub sink_rate: f64,
    pub spring: Option<f64>,
    pub damper: Option<f64>,
}

impl Strut {
    /// Its spring (N/m): its record's, else `holds` at the end of the stroke.
    pub fn stiffness(&self) -> f64 {
        self.spring.unwrap_or(self.holds / self.stroke.max(1e-3))
    }

    /// Its damper (N s/m): its record's, else 0.7 of `holds` at the design
    /// sink rate (the registry's rule for landing gear: dropped at its sink
    /// rate it peaks near `holds` and uses most of its stroke).
    pub fn damping(&self) -> f64 {
        self.damper.unwrap_or(0.7 * self.holds / self.sink_rate.max(0.1))
    }
}

/// What the body is: its mass (kg), centre of mass and inertia about it (body frame).
#[derive(Clone, Copy, Debug)]
pub struct MassProps {
    pub mass: f64,
    pub com: DVec3,
    pub inertia: DMat3,
}

/// The ground under a point: how deep the point is in it (m; negative: over
/// it), its outward normal there and its own velocity there.
#[derive(Clone, Copy, Debug)]
pub struct Ground {
    pub depth: f64,
    pub normal: DVec3,
    pub velocity: DVec3,
}

/// The forces of a step's contacts.
#[derive(Clone, Debug, Default)]
pub struct Contacts {
    /// The whole force (world frame, N) and its torque about the centre of mass (body frame, N m).
    pub force: DVec3,
    pub torque: DVec3,
    /// How many points touch, of them feet.
    pub touching: usize,
    pub feet: usize,
    /// Each strut's force along its normal (N); zero for one off the ground.
    pub struts: Vec<f64>,
    /// How far each strut is pushed in (m), and how fast its foot is going into the ground (m/s).
    pub compression: Vec<f64>,
    pub sinks: Vec<f64>,
    /// The fastest a hull point (or a bottomed strut) struck the ground at, into it (m/s).
    pub strike: f64,
}

/// Hard contacts pull a point out of the ground this share of its depth per step.
const SETTLE: f64 = 0.2;
/// And no faster than this (m/s): a deep overlap eases out instead of springing.
const SETTLE_SPEED: f64 = 1.0;
/// Clear of the ground by this much (m), beyond what it moves in a step: not touching.
const CLEAR: f64 = 0.5;

/// The contacts of `body` (with `mass`), its `struts` (those whose bit is
/// set in `broken` have given way) and its `hull` (spheres in its frame; a
/// point is a sphere of no radius), on `ground` over a step of `h` seconds.
pub fn forces(ground: &dyn Fn(DVec3) -> Option<Ground>, body: &RigidBody, mass: &MassProps, struts: &[Strut], hull: &[crate::body::Sphere], broken: u64, h: f64) -> Contacts {
    let rot = body.orientation;
    let spin = rot * body.angular_velocity;
    let inv_inertia = {
        let r = DMat3::from_quat(rot);
        r * mass.inertia.inverse() * r.transpose()
    };
    // The point's mass as a push along `dir` at `r` (from the centre of mass) feels it.
    let felt = |r: DVec3, dir: DVec3| -> f64 {
        let turn = (inv_inertia * r.cross(dir)).cross(r).dot(dir);
        1.0 / (1.0 / mass.mass + turn.max(0.0))
    };
    // A point at `at` (body frame): its arm from the centre of mass and its velocity.
    let point = |at: DVec3| -> (DVec3, DVec3, DVec3) {
        let arm = rot * (at - mass.com);
        let world = body.position + rot * at;
        (world, arm, body.velocity + spin.cross(arm))
    };
    let mut out = Contacts { struts: vec![0.0; struts.len()], compression: vec![0.0; struts.len()], sinks: vec![0.0; struts.len()], ..Default::default() };
    // The ground under its origin, taken as a plane: the whole body clear of
    // it, nothing to do; a point clear of it by more than the ground could
    // rise in its distance across (a slope of one in two) is not looked at.
    let reach = struts.iter().map(|s| s.foot.length()).chain(hull.iter().map(|s| s.at.length() + s.radius)).fold(0.0, f64::max);
    let sweep = (body.velocity.length() + spin.length() * reach) * h;
    let plane = ground(body.position);
    if let Some(g0) = plane
        && -g0.depth > reach + CLEAR + sweep
    {
        return out;
    }
    let clear = |world: DVec3, radius: f64| -> bool {
        plane.is_some_and(|g0| {
            let off = world - body.position;
            let up = off.dot(g0.normal);
            let across = (off - g0.normal * up).length();
            -g0.depth + up - radius > CLEAR + sweep + 0.5 * across
        })
    };
    // (Gathered first: every touching point shares the body, so each hard push
    // is divided among them.)
    struct Touch {
        arm: DVec3,
        rel: DVec3,
        g: Ground,
        /// The strut's spring and damper force, or None: a hard contact.
        soft: Option<f64>,
        depth: f64,
        grip: f64,
        strut: Option<usize>,
    }
    let mut touches = Vec::new();
    for (k, s) in struts.iter().enumerate() {
        if k < 64 && broken & (1 << k) != 0 {
            continue;
        }
        let (world, arm, vel) = point(s.foot);
        if clear(world, 0.0) {
            continue;
        }
        let Some(g) = ground(world).filter(|g| g.depth > 0.0) else { continue };
        let rel = vel - g.velocity;
        let into = -rel.dot(g.normal);
        let x = g.depth.min(s.stroke);
        // (Stable for the step: the spring and damper no stiffer than the point's mass allows.)
        let m = felt(arm, g.normal);
        let k_s = s.stiffness().min(0.5 * m / (h * h));
        let c = s.damping().min(m / h);
        let push = (k_s * x + c * into).max(0.0);
        out.compression[k] = x;
        out.sinks[k] = into.max(0.0);
        touches.push(Touch { arm, rel, g, soft: Some(push), depth: 0.0, grip: FOOT_FRICTION, strut: Some(k) });
        if g.depth > s.stroke {
            // Bottomed: the rest is the hull's.
            touches.push(Touch { arm, rel, g, soft: None, depth: g.depth - s.stroke, grip: FOOT_FRICTION, strut: None });
        }
    }
    for s in hull {
        let (centre, arm0, _) = point(s.at);
        if clear(centre, s.radius) {
            continue;
        }
        let Some(g) = ground(centre) else { continue };
        let depth = g.depth + s.radius;
        if depth <= 0.0 {
            continue;
        }
        // (The touch at the sphere's lowest point, along the ground's normal.)
        let arm = arm0 - g.normal * s.radius;
        let rel = body.velocity + spin.cross(arm) - g.velocity;
        touches.push(Touch { arm, rel, g, soft: None, depth, grip: HULL_FRICTION, strut: None });
    }
    let shared = touches.iter().filter(|t| t.soft.is_none()).count().max(1) as f64;
    let mut torque = DVec3::ZERO;
    for t in &touches {
        let n = t.g.normal;
        let into = -t.rel.dot(n);
        let normal = match t.soft {
            Some(f) => f,
            None => {
                out.strike = out.strike.max(into);
                // To this speed out of the ground by the step's end.
                let bounce = if into > HULL_REST_SPEED { HULL_RESTITUTION * into } else { 0.0 };
                let want = bounce.max((SETTLE * t.depth / h).min(SETTLE_SPEED));
                (felt(t.arm, n) * (want + into) / h / shared).max(0.0)
            }
        };
        if normal <= 0.0 {
            continue;
        }
        // Coulomb: no more than the grip, no more than stops the slide in this step.
        let slide = t.rel - n * t.rel.dot(n);
        let speed = slide.length();
        let friction = if speed > 1e-9 {
            let dir = slide / speed;
            let stop = felt(t.arm, dir) * speed / h / (touches.len() as f64);
            -dir * (t.grip * normal).min(stop)
        } else {
            DVec3::ZERO
        };
        let f = n * normal + friction;
        out.force += f;
        torque += t.arm.cross(f);
        out.touching += 1;
        if let Some(k) = t.strut {
            out.feet += 1;
            out.struts[k] = normal;
        }
    }
    out.torque = rot.inverse() * torque;
    out
}

/// Move `body` on by its contacts' push and turn over `h` seconds: the turn
/// about its centre of mass (so tipping over a foot swings the centre round
/// it, as it does). The force goes in with the step's other forces; this is
/// the turn alone.
pub fn turn(body: &mut RigidBody, mass: &MassProps, torque: DVec3, h: f64) -> DVec3 {
    let dw = mass.inertia.inverse() * torque * h;
    body.angular_velocity += dw;
    dw
}

/// Turn `body` by `rotation` (body frame) about its centre of mass `com`.
pub fn rotate_about(body: &mut RigidBody, com: DVec3, rotation: DVec3) {
    let before = body.orientation * com;
    body.orientation = (body.orientation * DQuat::from_scaled_axis(rotation)).normalize();
    body.position += before - body.orientation * com;
}

/// The ground of a smooth or shaped body at `p`: `radius_at(p)` gives the
/// surface's distance from `center` under `p`; its slope is sampled `span`
/// metres round.
pub fn ground_of(center: DVec3, velocity_at: impl Fn(DVec3) -> DVec3, radius_at: impl Fn(DVec3) -> f64, p: DVec3, span: f64) -> Ground {
    let off = p - center;
    let up = off.normalize_or_zero();
    let r = radius_at(p);
    // (Clear of the ground by more than any slope could close: no need of its slope.)
    if off.length() - r > span {
        return Ground { depth: r - off.length(), normal: up, velocity: velocity_at(p) };
    }
    // Two directions along the ground, and the surface's height a span along each.
    let a = up.any_orthonormal_vector();
    let b = up.cross(a);
    let lift = |d: DVec3| -> DVec3 {
        let q = center + (off + d * span).normalize() * r;
        let h = radius_at(q);
        center + (q - center).normalize() * h
    };
    let here = center + up * r;
    let (pa, pb) = (lift(a), lift(b));
    let mut normal = (pa - here).cross(pb - here).normalize_or_zero();
    if normal.dot(up) < 0.0 {
        normal = -normal;
    }
    if normal == DVec3::ZERO {
        normal = up;
    }
    // (Depth along the normal: on a slope a point is less deep than it is below the surface.)
    let depth = (r - off.length()) * normal.dot(up);
    Ground { depth, normal, velocity: velocity_at(p) }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::body::Sphere;

    const G: f64 = 9.81;

    /// Flat ground at y = 0 tilted by `slope` (radians about z), still.
    fn plane(slope: f64) -> impl Fn(DVec3) -> Option<Ground> {
        let n = DQuat::from_rotation_z(slope) * DVec3::Y;
        move |p: DVec3| Some(Ground { depth: -p.dot(n), normal: n, velocity: DVec3::ZERO })
    }

    fn mc() -> MassProps {
        // A 100 t box, 20 x 8 x 20 m.
        let m = 100_000.0;
        let (x, y, z) = (20.0f64, 8.0f64, 20.0f64);
        let i = DMat3::from_diagonal(DVec3::new(y * y + z * z, x * x + z * z, x * x + y * y) * (m / 12.0));
        MassProps { mass: m, com: DVec3::ZERO, inertia: i }
    }

    fn legs(spread: f64) -> Vec<Strut> {
        [(1., 1.), (1., -1.), (-1., 1.), (-1., -1.)]
            .iter()
            .map(|&(x, z)| Strut { foot: DVec3::new(x * spread, -6.0, z * spread), holds: 1.0e6, stroke: 0.8, sink_rate: 3.05, spring: None, damper: None })
            .collect()
    }

    /// Run `secs` seconds under gravity along -y; returns the body.
    fn run(mut body: RigidBody, mass: &MassProps, struts: &[Strut], ground: &dyn Fn(DVec3) -> Option<Ground>, secs: f64) -> (RigidBody, Contacts) {
        let h = 0.01;
        let mut last = Contacts::default();
        for _ in 0..(secs / h) as usize {
            let c = forces(ground, &body, mass, struts, &body.parts.clone(), 0, h);
            body.velocity += (DVec3::NEG_Y * G + c.force / mass.mass) * h;
            body.position += body.velocity * h;
            let dw = turn(&mut body, mass, c.torque, h);
            let _ = dw;
            let w = body.angular_velocity;
            rotate_about(&mut body, mass.com, w * h);
            last = c;
        }
        (body, last)
    }

    fn standing(y: f64) -> RigidBody {
        RigidBody::new(DVec3::new(0.0, y, 0.0), DVec3::new(0.0, -1.0, 0.0), DQuat::IDENTITY, 12.0)
    }

    #[test]
    fn it_stands_on_a_gentle_slope_and_slides_down_a_steep_one() {
        let m = mc();
        // (Wide legs, so it doesn't tip first.)
        let gentle = plane(10f64.to_radians());
        let start = |s: f64| RigidBody { orientation: DQuat::from_rotation_z(s), ..standing(6.3) };
        let (b, _) = run(start(10f64.to_radians()), &m, &legs(8.0), &gentle, 5.0);
        assert!(b.velocity.length() < 0.05, "held on 10 degrees, at {} m/s", b.velocity.length());
        let steep = plane(40f64.to_radians());
        let (b, _) = run(start(40f64.to_radians()), &m, &legs(8.0), &steep, 3.0);
        assert!(b.velocity.length() > 2.0, "slides on 40 degrees, at {} m/s", b.velocity.length());
    }

    #[test]
    fn a_ship_whose_centre_is_outside_its_feet_tips_over() {
        // (Its hull is its parts' convex hulls: a thin strut far from the origin,
        // its corners rounded from f32, is one box, every face once.)
        let (a, b, r) = (glam::Vec3::new(-1.5, -1.1, 1.5), glam::Vec3::new(-1.0, -0.1, 2.5), 0.05f32);
        let turn = glam::Quat::from_rotation_arc(glam::Vec3::Z, (b - a).normalize());
        let corners: Vec<DVec3> = (0..8).map(|n| ((a + b) * 0.5 + turn * glam::Vec3::new(if n & 1 == 0 { -r } else { r }, if n & 2 == 0 { -r } else { r }, if n & 4 == 0 { -0.75 } else { 0.75 })).as_dvec3()).collect();
        let strut = crate::mesh::Mesh::convex_hull(&corners);
        assert_eq!(strut.faces.len(), 12);
        assert!((strut.mass_properties().volume - 0.1 * 0.1 * 1.5).abs() < 1e-5, "{}", strut.mass_properties().volume);
        let m = MassProps { com: DVec3::new(9.0, 0.0, 0.0), ..mc() };
        let mut hull = RigidBody { parts: std::borrow::Cow::Owned(vec![Sphere { at: DVec3::new(0.0, 0.0, 0.0), radius: 4.0 }]), ..standing(6.2) };
        hull.velocity = DVec3::ZERO;
        let (b, _) = run(hull, &m, &legs(4.0), &plane(0.0), 6.0);
        let up = b.orientation * DVec3::Y;
        assert!(up.y < 0.9, "should have tipped, up is {up}");
    }

    #[test]
    fn a_bouncing_hull_never_gains_energy() {
        let m = mc();
        let mut body = RigidBody { parts: std::borrow::Cow::Owned(vec![Sphere { at: DVec3::ZERO, radius: 4.0 }]), ..standing(30.0) };
        body.velocity = DVec3::new(0.0, -10.0, 0.0);
        let energy = |b: &RigidBody| 0.5 * b.velocity.length_squared() + G * b.position.y;
        let e0 = energy(&body);
        let (b, c) = run(body, &m, &[], &plane(0.0), 1.0);
        assert!(energy(&b) <= e0 + 1e-6);
        let _ = c;
        let (b, _) = run(b, &m, &[], &plane(0.0), 6.0);
        assert!(energy(&b) < e0 && (b.position.y - 4.0).abs() < 0.05);
    }
}
