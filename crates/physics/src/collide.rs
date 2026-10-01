//! Colliders and the facts they report. A collider says *that* a body touched
//! something, where and how fast; never what it means. Every moving body is a
//! sphere of its own radius here.

use glam::DVec3;

use crate::body::RigidBody;
use crate::rails::{velocity, Frame, OnRails};
use crate::surface::{max_radius, surface_radius_at};

/// A rail body's shape, for contact.
#[derive(Clone, Debug)]
pub enum Collider {
    /// Nothing to touch.
    None,
    /// A sphere of the body's radius, raised by its surface's height function if it has one.
    Surface,
    Blocks(Blocks),
    Ring(Ring),
}

/// A surface body smaller than this (m) is small: see `Collider::is_small`.
pub const SMALL_BODY: f64 = 50_000.0;

impl Collider {
    /// Small shapes (structures, and surface bodies of `radius` under
    /// `SMALL_BODY`: asteroids) need short substeps nearby, or a body steps
    /// right over them.
    pub fn is_small(&self, radius: f64) -> bool {
        match self {
            Collider::Blocks(_) | Collider::Ring(_) => true,
            Collider::Surface => radius < SMALL_BODY,
            Collider::None => false,
        }
    }
}

/// A solid built of boxes, each axis-aligned in the body's own frame
/// (metres), some of whose tops are decks: a structure. Touching a deck's
/// top is reported as touching that deck; anything else, the hull.
#[derive(Clone, Debug)]
pub struct Blocks {
    /// Each box's (min, max) corners.
    pub boxes: Vec<(DVec3, DVec3)>,
    /// The boxes whose tops are decks (`Feature::Deck(k)`: the k-th of these).
    pub decks: Vec<usize>,
    /// Radius (m) of a sphere around the body's centre enclosing them all.
    pub bound: f64,
}

impl Blocks {
    pub fn new(boxes: Vec<(DVec3, DVec3)>, decks: Vec<usize>) -> Self {
        let bound = boxes.iter().flat_map(|&(lo, hi)| [lo.abs(), hi.abs()]).fold(DVec3::ZERO, |m, c| m.max(c)).length();
        Self { boxes, decks, bound }
    }

    /// Contact of a sphere of `radius` at `pos`, moving at `vel`, with this
    /// solid as body `body` posed at `frame`: with the box it's nearest, if
    /// within `radius` of it.
    pub fn contact(&self, body: usize, frame: &Frame, pos: DVec3, vel: DVec3, radius: f64) -> Option<Contact> {
        let local = frame.local(pos);
        // (distance from the box, negative inside: how deep; the box; local outward normal)
        let mut best: Option<(f64, usize, DVec3)> = None;
        for (k, &(lo, hi)) in self.boxes.iter().enumerate() {
            let q = local.clamp(lo, hi);
            let (d, n) = if q != local {
                let off = local - q;
                (off.length(), off / off.length())
            } else {
                // Inside: out through the face it's least deep behind.
                let faces = [(local.x - lo.x, DVec3::NEG_X), (hi.x - local.x, DVec3::X), (local.y - lo.y, DVec3::NEG_Y), (hi.y - local.y, DVec3::Y), (local.z - lo.z, DVec3::NEG_Z), (hi.z - local.z, DVec3::Z)];
                let (depth, n) = faces.into_iter().min_by(|a, b| a.0.total_cmp(&b.0)).expect("six faces");
                (-depth, n)
            };
            if d < radius && best.is_none_or(|b| d < b.0) {
                best = Some((d, k, n));
            }
        }
        let (_, k, n) = best?;
        let feature = match self.decks.iter().position(|&d| d == k) {
            Some(deck) if n.y > 0.7 => Feature::Deck(deck),
            _ => Feature::Hull,
        };
        let surface_velocity = frame.velocity_at(pos);
        Some(Contact { body, feature, normal: frame.rotation * n, local, surface_velocity, relative_velocity: vel - surface_velocity })
    }
}

/// A torus lying in its local XZ plane around the local Y axis; its opening is a trigger.
#[derive(Clone, Copy, Debug)]
pub struct Ring {
    /// Radius of the tube's centerline (m).
    pub radius: f64,
    /// Radius of the tube (m).
    pub tube: f64,
}

pub enum RingCrossing {
    None,
    /// Went through the opening.
    Through,
    /// Touched the tube.
    Hit,
}

impl Ring {
    /// Did a sphere of `radius`, moving from `prev` to `pos` over `dt` seconds,
    /// pass through the opening or touch the tube? `frame` is the ring at the
    /// end of the step; the start is measured against where the ring was then
    /// (it may be moving at km/s in its orbit).
    pub fn crossing(&self, frame: &Frame, prev: DVec3, pos: DVec3, dt: f64, radius: f64) -> RingCrossing {
        let (l0, l1) = (frame.local(prev + frame.velocity * dt), frame.local(pos));
        let radial = |l: DVec3| (l.x * l.x + l.z * l.z).sqrt();
        let reach = self.tube + radius;
        // Touching the tube itself.
        let tube = |l: DVec3| ((radial(l) - self.radius).powi(2) + l.y * l.y).sqrt();
        if tube(l1) < reach {
            return RingCrossing::Hit;
        }
        if l0.y.signum() == l1.y.signum() || l0.y == l1.y {
            return RingCrossing::None;
        }
        let u = l0.y / (l0.y - l1.y);
        let r = radial(l0.lerp(l1, u));
        if r < self.radius - reach {
            RingCrossing::Through
        } else if r < self.radius + reach {
            RingCrossing::Hit
        } else {
            RingCrossing::None
        }
    }

    /// The fact for a crossing by a sphere now at `pos` moving at `vel`.
    fn fact(&self, body: usize, frame: &Frame, crossing: RingCrossing, pos: DVec3, vel: DVec3) -> Option<Fact> {
        let surface_velocity = frame.velocity_at(pos);
        let relative_velocity = vel - surface_velocity;
        match crossing {
            RingCrossing::None => None,
            RingCrossing::Through => Some(Fact::Trigger { body, relative_velocity }),
            RingCrossing::Hit => {
                // Out of the tube, from the nearest point on its centerline.
                let local = frame.local(pos);
                let flat = DVec3::new(local.x, 0.0, local.z).try_normalize().unwrap_or(DVec3::X);
                let normal = frame.rotation * (local - flat * self.radius).normalize_or(DVec3::Y);
                Some(Fact::Contact(Contact { body, feature: Feature::Ring, normal, local, surface_velocity, relative_velocity }))
            }
        }
    }
}

/// Which part of a collider was touched.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Feature {
    /// A body's surface; `liquid` if what was touched there is liquid.
    Surface { liquid: bool },
    /// A structure's outer faces.
    Hull,
    /// A structure's deck (by index): its top.
    Deck(usize),
    /// A ring's tube.
    Ring,
}

/// A moving body touching a rail body's collider.
#[derive(Clone, Copy, Debug)]
pub struct Contact {
    /// Index of the rail body touched.
    pub body: usize,
    pub feature: Feature,
    /// Unit normal (world), pointing out of the collider.
    pub normal: DVec3,
    /// Where the moving body is in the collider's frame: the unit direction in
    /// the body's frame for a surface, meters for a structure or a ring.
    pub local: DVec3,
    /// Velocity of the collider's material at the contact.
    pub surface_velocity: DVec3,
    /// The moving body's velocity relative to that.
    pub relative_velocity: DVec3,
}

/// Something that happened to a moving body during a substep.
#[derive(Clone, Copy, Debug)]
pub enum Fact {
    Contact(Contact),
    /// Passed through a trigger (a ring's opening) of rail body `body`.
    Trigger { body: usize, relative_velocity: DVec3 },
}

/// Contact of a sphere at `pos` (moving at `vel`) with the surface of body `i`.
pub fn surface_contact<B: OnRails>(bodies: &[B], i: usize, t: f64, positions: &[DVec3], pos: DVec3, vel: DVec3, radius: f64) -> Option<Contact> {
    let b = &bodies[i];
    let center = positions[i];
    // Cheap sphere test first; the height function only when inside the tallest peaks.
    let d = center.distance(pos);
    if !(d < max_radius(b) + radius && d < surface_radius_at(b, center, pos, t) + radius) {
        return None;
    }
    let rail = b.rail();
    let offset = pos - center;
    let surface_velocity = velocity(bodies, i, t) + rail.angular_velocity().cross(offset);
    let normal = offset.normalize();
    let local = rail.rotation(t).inverse() * normal;
    let liquid = b.surface().is_some_and(|s| s.liquid(local));
    Some(Contact { body: i, feature: Feature::Surface { liquid }, normal, local, surface_velocity, relative_velocity: vel - surface_velocity })
}

/// The first fact for `body` after a substep of `h` from `prev`, ending at `t`
/// with the rail bodies at `positions`: ring openings and tubes first, then
/// solids, each in body order.
/// A body with `parts` touches solids by each of its spheres, where each is
/// now and as it moves (its spin included); passing a ring is decided by its
/// centre's path.
pub fn detect<B: OnRails>(bodies: &[B], t: f64, positions: &[DVec3], prev: DVec3, body: &RigidBody, h: f64) -> Option<Fact> {
    if let Some(f) = rings(bodies, t, positions, prev, body, h) {
        return Some(f);
    }
    if body.parts.is_empty() {
        return solids(bodies, t, positions, body.position, body.velocity, body.radius);
    }
    // (Nothing within reach of the whole shape: nothing touched.)
    let bound = body.parts.iter().map(|s| s.at.length() + s.radius).fold(0.0, f64::max);
    solids(bodies, t, positions, body.position, body.velocity, bound)?;
    let spin = body.orientation * body.angular_velocity;
    body.parts.iter().find_map(|s| {
        let off = body.orientation * s.at;
        solids(bodies, t, positions, body.position + off, body.velocity + spin.cross(off), s.radius)
    })
}

/// Ring openings and tubes, for the body's centre path.
fn rings<B: OnRails>(bodies: &[B], t: f64, positions: &[DVec3], prev: DVec3, body: &RigidBody, h: f64) -> Option<Fact> {
    let (pos, vel) = (body.position, body.velocity);
    for (i, b) in bodies.iter().enumerate() {
        let Collider::Ring(ring) = &b.rail().collider else { continue };
        if positions[i].distance(pos) > ring.radius * 2.0 {
            continue;
        }
        let frame = Frame::of(bodies, i, t, positions);
        let crossing = ring.crossing(&frame, prev, pos, h, body.radius);
        if let Some(fact) = ring.fact(i, &frame, crossing, pos, vel) {
            return Some(fact);
        }
    }
    None
}

/// The solids a sphere of `radius` at `pos`, moving at `vel`, touches: the first, in body order.
fn solids<B: OnRails>(bodies: &[B], t: f64, positions: &[DVec3], pos: DVec3, vel: DVec3, radius: f64) -> Option<Fact> {
    for (i, b) in bodies.iter().enumerate() {
        let contact = match &b.rail().collider {
            Collider::None | Collider::Ring(_) => None,
            Collider::Blocks(p) => {
                // Bounding sphere first; the exact shape only when inside it.
                if positions[i].distance(pos) < p.bound + radius {
                    p.contact(i, &Frame::of(bodies, i, t, positions), pos, vel, radius)
                } else {
                    None
                }
            }
            Collider::Surface => surface_contact(bodies, i, t, positions, pos, vel, radius),
        };
        if let Some(c) = contact {
            return Some(Fact::Contact(c));
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use glam::DQuat;

    use super::*;
    use crate::testkit::{body, Ocean};

    fn still() -> Frame {
        Frame { center: DVec3::ZERO, velocity: DVec3::ZERO, rotation: DQuat::IDENTITY, angular_velocity: DVec3::ZERO }
    }

    #[test]
    fn structure_contact_sees_the_spin() {
        let p = Blocks::new(vec![(DVec3::splat(-500.0), DVec3::splat(500.0))], vec![]);
        let f = Frame { angular_velocity: DVec3::Y * 0.1, ..still() };
        let c = p.contact(0, &f, DVec3::new(505.0, 0.0, 0.0), DVec3::ZERO, 12.0).unwrap();
        // The face moves at ω × r under a body at rest.
        assert!(c.surface_velocity.distance(DVec3::new(0.0, 0.0, -50.5)) < 1e-9);
        assert!(c.relative_velocity.distance(DVec3::new(0.0, 0.0, 50.5)) < 1e-9);
    }

    #[test]
    fn surface_contact_reports_liquid() {
        let mut planet = body(None, None, 4.0e14, 6.4e6);
        planet.day = 1.0e4;
        let bodies = [Ocean(planet)];
        let positions = [DVec3::ZERO];
        // The test surface is ocean over the +X hemisphere, ground 1 km up elsewhere.
        let over = |dir: DVec3, h: f64| surface_contact(&bodies, 0, 0.0, &positions, dir * (6.4e6 + h), DVec3::ZERO, 12.0);
        assert!(over(DVec3::X, 20.0).is_none());
        let c = over(DVec3::X, 5.0).expect("touching the sea");
        assert_eq!(c.feature, Feature::Surface { liquid: true });
        assert!(c.normal.distance(DVec3::X) < 1e-12);
        assert!(over(DVec3::NEG_X, 1020.0).is_none());
        let c = over(DVec3::NEG_X, 1005.0).expect("touching the ground");
        assert_eq!(c.feature, Feature::Surface { liquid: false });
        // A body at rest on a spinning planet moves relative to its ground.
        let spin = TAU_OVER_DAY * (6.4e6 + 1005.0);
        assert!((c.relative_velocity.length() - spin).abs() < 1e-6);
    }

    #[test]
    fn a_deck_is_its_top_and_the_rest_is_hull() {
        // A slab with a block standing at one end.
        let p = Blocks::new(vec![(DVec3::new(-300.0, -150.0, -225.0), DVec3::new(300.0, -100.0, 375.0)), (DVec3::new(-300.0, -150.0, -375.0), DVec3::new(300.0, 150.0, -225.0))], vec![0]);
        let at = |x: f64, y: f64, z: f64| p.contact(0, &still(), DVec3::new(x, y, z), DVec3::ZERO, 12.0);
        let c = at(0.0, -90.0, 100.0).expect("on the deck");
        assert_eq!(c.feature, Feature::Deck(0));
        assert!(c.normal.distance(DVec3::Y) < 1e-12);
        assert!(at(0.0, -80.0, 100.0).is_none(), "above it");
        assert!(at(310.0, -50.0, 100.0).is_none(), "past its edge, above it: open");
        let wall = at(0.0, 0.0, -215.0).expect("against the block's face");
        assert_eq!(wall.feature, Feature::Hull);
        assert!(wall.normal.distance(DVec3::Z) < 1e-12);
        assert_eq!(at(0.0, -160.0, 100.0).unwrap().feature, Feature::Hull, "under the deck");
        assert!(at(0.0, 0.0, 600.0).is_none());
    }

    const TAU_OVER_DAY: f64 = std::f64::consts::TAU / 1.0e4;

    #[test]
    fn detect_finds_the_ring_trigger_and_the_ground() {
        let mut planet = body(None, None, 4.0e14, 6.4e6);
        planet.day = 1.0e4;
        let mut hoop = body(Some(0), None, 0.0, 1560.0);
        hoop.attracts = false;
        hoop.collider = Collider::Ring(Ring { radius: 1500.0, tube: 60.0 });
        let bodies = [planet, hoop];
        let positions = [DVec3::ZERO, DVec3::new(0.0, 1.0e7, 0.0)];
        let through = RigidBody::new(DVec3::new(0.0, 1.0e7 - 10.0, 0.0), DVec3::NEG_Y * 20.0, DQuat::IDENTITY, 12.0);
        let prev = through.position + DVec3::Y * 20.0;
        assert!(matches!(detect(&bodies, 0.0, &positions, prev, &through, 1.0), Some(Fact::Trigger { body: 1, .. })));
        let resting = RigidBody::new(DVec3::new(6.4e6 + 5.0, 0.0, 0.0), DVec3::ZERO, DQuat::IDENTITY, 12.0);
        let fact = detect(&bodies, 0.0, &positions, resting.position, &resting, 1.0);
        assert!(matches!(fact, Some(Fact::Contact(Contact { body: 0, feature: Feature::Surface { liquid: false }, .. }))));
        let away = RigidBody::new(DVec3::new(0.0, 5.0e6, 5.0e6), DVec3::ZERO, DQuat::IDENTITY, 12.0);
        assert!(detect(&bodies, 0.0, &positions, away.position, &away, 1.0).is_none());
    }
}
