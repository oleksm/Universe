//! A ship on the ground (`docs/ground-contact-plan.md`): its landing legs and
//! hull as Dogma's contact points, the step's contacts, and sleep.
//!
//! Touching ground that lets it land (a world's ground, by its rule) a ship is
//! not locked: it stays a free body under gravity, thrust and the ground's
//! push at every point that touches (`universe_physics::contact`), so it rocks
//! on its legs, tips, slides, rests where and how it comes to rest. Once still
//! for `SETTLE_TIME` with nothing pushing it, it sleeps: welded where it lies,
//! in the pose it lies in (`ShipState::Landed`), costing nothing. A command to
//! its engines or thrusters wakes it.

use glam::DVec3;
use universe_physics::contact::{self, Contacts, Ground, MassProps, Strut};
use universe_physics::{surface_radius_at, Collider, OnRails, RigidBody, Weld};

use crate::events::ShipEvent;
use crate::rules::{Part, Pose, Rule, Rules};
use crate::ship::{Ship, SHIP_RADIUS};
use crate::system::Body;

/// The contact substep (s): a leg's spring and damper are resolved within it.
pub const CONTACT_STEP: f64 = 0.01;
/// Still for this long (s) with nothing pushing: asleep.
pub const SETTLE_TIME: f64 = 1.0;
/// Still: slower than this over the ground (m/s) and turning slower than this (rad/s).
const STILL_SPEED: f64 = 0.05;
const STILL_SPIN: f64 = 0.01;
/// The hull strikes the ground faster than this (m/s): it is damaged.
const STRIKE: f64 = 1.0;
/// Off the ground this long (s): lifted off (told).
const LIFTED: f64 = 0.3;
/// Off the ground longer than this (s): flown, so coming down again is a landing.
const AIRBORNE: f64 = 2.0;
/// How far round a point the ground's slope is sampled (m).
const SLOPE_SPAN: f64 = 1.0;

/// A ship's contact with the ground, between steps (for the instruments and
/// for sleep). Not saved.
#[derive(Clone, Debug, Default)]
pub struct Touching {
    /// The ground's body.
    pub body: usize,
    /// How long it has lain still (s).
    pub still: f64,
    /// The last step's contacts.
    pub last: Contacts,
    /// Were its feet on the ground at the last step?
    pub feet_down: bool,
    /// Woken on the ground by its pilot (not come down from flight): coming
    /// to rest again leaves its systems as they are.
    pub woken: bool,
    /// How long it has been off the ground since it last touched (s), and
    /// whether it has touched in this stay.
    pub airborne: f64,
    pub touched: bool,
}

/// The legs it lands on, in order (a leg's bit in `Ship::broken_legs` is its
/// place here): its fitted devices' legs; else the hull's gear (its `gear_*`
/// nodes, sharing its record's legs); else, on a hull with neither, four legs
/// drawn down `SHIP_RADIUS` under its centre, as the game has always stood
/// such hulls.
pub fn struts(ship: &Ship) -> Vec<Strut> {
    if let Some(d) = &ship.devices
        && !d.fit.legs.is_empty()
    {
        return d.fit.legs.iter().map(|l| Strut { foot: l.foot, holds: l.holds, stroke: l.stroke, sink_rate: l.sink_rate }).collect();
    }
    let spec = ship.spec();
    let legs = crate::legs::of_spec(spec);
    let feet: Vec<DVec3> = spec.shape().nodes(crate::shape::Role::Gear).map(|g| g.at).collect();
    let modelled = spec.visual.is_some() || spec.shape_own.is_some();
    let feet = if !feet.is_empty() && modelled {
        feet
    } else if spec.shape_own.is_some() {
        // (A draft with no gear placed: it lies on its hull.)
        return Vec::new();
    } else {
        let s = SHIP_RADIUS * 0.5;
        vec![DVec3::new(s, -SHIP_RADIUS, s), DVec3::new(-s, -SHIP_RADIUS, s), DVec3::new(s, -SHIP_RADIUS, -s), DVec3::new(-s, -SHIP_RADIUS, -s)]
    };
    let n = feet.len() as f64;
    // (A hull whose record doesn't say what its legs are made of: each leg
    // made to take three times its share of the full ship's weight on Earth,
    // over half a metre, for the airworthiness sink rate. Marked so in the plan.)
    let full = (spec.dry_mass + spec.fuel_capacity) * universe_physics::laws::STANDARD_GRAVITY;
    let (holds, stroke, sink) = match legs {
        Some(l) => (l.force / n, l.stroke * l.efficiency, l.designed),
        None => (3.0 * full / n, 0.5, 3.05),
    };
    feet.into_iter().map(|foot| Strut { foot, holds, stroke, sink_rate: sink }).collect()
}

/// Its hull's contact points: the corners of its convex parts (its true
/// surface, not the spheres that loosely cover it for a first touch); a hull
/// made of no parts, its spheres. Worked out once per shape.
pub fn hull(ship: &Ship) -> std::sync::Arc<Vec<universe_physics::Sphere>> {
    use std::collections::{HashMap, HashSet};
    use std::sync::{Arc, Mutex, OnceLock, Weak};
    type Hulls = Mutex<HashMap<usize, (Option<Weak<crate::shape::Shape>>, Arc<Vec<universe_physics::Sphere>>)>>;
    static HULLS: OnceLock<Hulls> = OnceLock::new();
    let spec = ship.spec();
    let shape = spec.shape();
    let id = shape as *const crate::shape::Shape as usize;
    let own = spec.shape_own.as_ref().map(Arc::downgrade);
    let cache = HULLS.get_or_init(Default::default);
    {
        let cache = cache.lock().unwrap_or_else(|e| e.into_inner());
        // (A design's own shape only while it lives: its address may be reused after.)
        if let Some((weak, h)) = cache.get(&id)
            && weak.as_ref().is_none_or(|w| w.upgrade().is_some_and(|s| Arc::as_ptr(&s) as usize == id))
        {
            return h.clone();
        }
    }
    let mut seen = HashSet::new();
    let mut points: Vec<universe_physics::Sphere> = Vec::new();
    for r in &shape.part_points {
        for p in &shape.mesh.points[r.clone()] {
            let key = ((p.x * 10.0).round() as i64, (p.y * 10.0).round() as i64, (p.z * 10.0).round() as i64);
            if seen.insert(key) {
                points.push(universe_physics::Sphere { at: *p, radius: 0.0 });
            }
        }
    }
    if points.is_empty() {
        points = shape.spheres.clone();
    }
    let h = Arc::new(points);
    let mut cache = cache.lock().unwrap_or_else(|e| e.into_inner());
    cache.retain(|_, (w, _)| w.as_ref().is_none_or(|w| w.strong_count() > 0));
    cache.insert(id, (own, h.clone()));
    h
}

/// What Dogma needs of the ship's mass.
pub fn mass_props(ship: &Ship) -> MassProps {
    MassProps { mass: ship.mass(), com: ship.centre_of_mass(), inertia: ship.inertia() }
}

/// The ground of body `i` under `p` at `t` (bodies at `positions`).
pub fn ground_at(bodies: &[Body], i: usize, t: f64, positions: &[DVec3], p: DVec3) -> Ground {
    let b = &bodies[i];
    let center = positions[i];
    let v = universe_physics::velocity(bodies, i, t);
    let spin = b.rail().angular_velocity();
    contact::ground_of(center, |q| v + spin.cross(q - center), |q| surface_radius_at(b, center, q, t), p, SLOPE_SPAN)
}

/// The body whose ground the ship could touch within `reach` metres (plus its
/// own size), if its rule lets ships land there: the world's ground.
pub fn near(bodies: &[Body], rules: &Rules, positions: &[DVec3], ship: &RigidBody, reach: f64) -> Option<usize> {
    let size = ship.parts.iter().map(|s| s.at.length() + s.radius).fold(ship.radius.max(SHIP_RADIUS * 1.5), f64::max);
    (0..bodies.len().min(positions.len())).find(|&i| {
        matches!(bodies[i].rail.collider, Collider::Surface)
            && lands(rules, i)
            && ship.position.distance(positions[i]) < universe_physics::max_radius(&bodies[i]) + size + reach
    })
}

/// Does body `i`'s ground take ships by contact (a world's ground, not a deck)?
pub fn lands(rules: &Rules, i: usize) -> bool {
    matches!(rules.get(i, Part::Ground), Some(Rule::Lock { pose: Pose::Ground, .. }))
}

/// The contact part of a flight step: Dogma's body's contacts with the
/// ground of `body`, worked each substep.
pub struct Step {
    pub body: usize,
    pub mass: MassProps,
    pub struts: Vec<Strut>,
    /// Its hull's contact points (see `hull`).
    pub hull: std::sync::Arc<Vec<universe_physics::Sphere>>,
    /// The spin the flight computer turned the ship by over the whole step
    /// (already taken): contact turns it by the rest.
    pub spin0: DVec3,
    pub touching: Touching,
}

impl Step {
    pub fn new(ship: &Ship, body: usize) -> Self {
        let touching = ship.ground.clone().filter(|t| t.body == body).unwrap_or(Touching { body, ..Default::default() });
        Self { body, mass: mass_props(ship), struts: struts(ship), hull: hull(ship), spin0: ship.angular_velocity, touching }
    }

    /// One substep of `h` at `t`: the contacts' acceleration (world frame);
    /// `rigid` is turned by their torque. Legs that give way are broken on `ship`.
    pub fn apply(&mut self, bodies: &[Body], t: f64, positions: &[DVec3], rigid: &mut RigidBody, ship: &mut Ship, events: &mut Vec<ShipEvent>) -> DVec3 {
        let h = CONTACT_STEP;
        let i = self.body;
        let c = contact::forces(&|p| Some(ground_at(bodies, i, t, positions, p)), rigid, &self.mass, &self.struts, &self.hull, ship.broken_legs, h);
        // Legs pushed past what they hold give way: the ship comes down on that corner.
        for (k, (&f, s)) in c.struts.iter().zip(&self.struts).enumerate() {
            if f > s.holds && k < 64 && ship.broken_legs & (1 << k) == 0 {
                ship.broken_legs |= 1 << k;
                events.push(ShipEvent::LegGaveWay { leg: k, force: f, holds: s.holds });
            }
        }
        // Touching down: as hard as its fastest foot came down.
        let down = c.feet > 0;
        if down
            && !self.touching.feet_down
            && let Some((s, sink)) = self.struts.iter().zip(&c.sinks).filter(|(_, v)| **v > 0.0).max_by(|a, b| a.1.total_cmp(b.1))
            && *sink > s.sink_rate
        {
            let jolt = sink * sink / (2.0 * s.stroke) / universe_physics::laws::STANDARD_GRAVITY;
            events.push(ShipEvent::HardLanding { sink: *sink, jolt });
        }
        self.touching.feet_down = down;
        contact::turn(rigid, &self.mass, c.torque, h);
        contact::rotate_about(rigid, self.mass.com, (rigid.angular_velocity - self.spin0) * h);
        let accel = c.force / self.mass.mass;
        let strike = self.touching.last.strike.max(c.strike);
        self.touching.last = Contacts { strike, ..c };
        accel
    }

    /// After the step (`dt` long, ending at `t`): still, or not; asleep once
    /// still long enough with nothing pushing. Returns whether it slept.
    #[allow(clippy::too_many_arguments)]
    pub fn finish(mut self, sys: &crate::system::StarSystem, rules: &Rules, ship: &mut Ship, t: f64, positions: &[DVec3], dt: f64, events: &mut Vec<ShipEvent>) -> bool {
        if self.touching.last.touching == 0 {
            // (Off the ground a moment, a hop, is still the same stay; flown off, a new one.)
            let before = self.touching.airborne;
            self.touching.airborne += dt;
            if self.touching.touched && before < LIFTED && self.touching.airborne >= LIFTED {
                events.push(ShipEvent::TookOff);
            }
            self.touching.still = 0.0;
            if self.touching.airborne > AIRBORNE {
                (self.touching.woken, self.touching.touched) = (false, false);
            }
            ship.ground = Some(self.touching);
            return false;
        }
        self.touching.airborne = 0.0;
        self.touching.touched = true;
        // The hull (or a leg pushed through its stroke) struck the ground: the
        // energy it lost crumples it, as in a collision.
        let strike = self.touching.last.strike;
        if strike > STRIKE {
            let lost = 0.5 * ship.mass() * strike * strike * (1.0 - universe_physics::laws::HULL_RESTITUTION.powi(2));
            ship.hull = (ship.hull - lost / ship.spec().hull_strength).max(0.0);
            events.push(ShipEvent::Bumped);
            if ship.hull <= 0.0 {
                crate::damage::destroy(ship, "THE GROUND", events);
                ship.ground = None;
                return false;
            }
        }
        let g = ground_at(&sys.bodies, self.body, t, positions, ship.position);
        // (Pushed by its own engines: more than a whisper of thrust. The flight
        // computer's rate hold alone, damping the last rocking, is no push.)
        let pushing = ship.powered && ship.applied.0.length() > ship.mass() * 0.02;
        let still = (ship.velocity - g.velocity).length() < STILL_SPEED && ship.angular_velocity.length() < STILL_SPIN;
        self.touching.still = if still && !pushing { self.touching.still + dt } else { 0.0 };
        self.touching.last.strike = 0.0;
        if self.touching.still < SETTLE_TIME {
            ship.ground = Some(self.touching);
            return false;
        }
        let weld = Weld::capture(&sys.bodies, self.body, t, positions, &ship.rigid());
        crate::rules::rest(rules, sys, ship, weld, !self.touching.woken, t, events);
        ship.ground = Some(self.touching);
        true
    }
}

/// Is anything of its own pushing or turning it (what its engines and
/// thrusters really give)?
pub fn pushed(ship: &Ship) -> bool {
    ship.powered && (ship.applied.0.length_squared() > 0.0 || ship.applied.1.length_squared() > 0.0)
}
