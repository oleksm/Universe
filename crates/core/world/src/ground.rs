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
use universe_physics::laws::{FOOT_FRICTION, HULL_FRICTION};
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
/// The hull strikes the ground faster than this (m/s): it is damaged (Dogma's law).
const STRIKE: f64 = universe_physics::laws::HULL_CRUMPLE_SPEED;
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
        return d.fit.legs.iter().map(|l| Strut { foot: l.foot, holds: l.holds, stroke: l.stroke, sink_rate: l.sink_rate, spring: l.spring, damper: l.damper }).collect();
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
    feet.into_iter().map(|foot| Strut { foot, holds, stroke, sink_rate: sink, spring: None, damper: None }).collect()
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

/// What a ship is on the ground, worked out once for its design and fit
/// (kept on the ship, made again when either changes): its legs and hull
/// points, how far it reaches, the footprint it stands on. From it, without
/// flying anything: the slope it tips over on, the slope it slides on, the
/// hardest landing its legs take (see `Profile::max_sink`).
#[derive(Debug)]
pub struct Profile {
    /// What it was made for: the design's address and its legs' fingerprint.
    key: (usize, u64),
    pub struts: Vec<Strut>,
    pub hull: std::sync::Arc<Vec<universe_physics::Sphere>>,
    /// The farthest any contact point is from its origin (m).
    pub reach: f64,
    /// How far below its origin it stands, upright on level ground (its feet, else its belly; m).
    pub stand: f64,
    /// What it stands on, seen from below (body x, z): its feet, else its
    /// lowest hull corners; a convex polygon, anticlockwise.
    pub footprint: Vec<glam::DVec2>,
    /// Hardest landings already worked out, by (mass in thousandths, centre in 10 cm, gravity cm/s², broken legs).
    sinks: std::sync::Mutex<Vec<(SinkKey, f64)>>,
    /// Rest poses already worked out, by centre (10 cm).
    rests: std::sync::Mutex<Vec<([i64; 3], Rest)>>,
}

/// (Mass in thousandths, centre in 10 cm, gravity cm/s², broken legs.)
type SinkKey = (i64, [i64; 3], i64, u64);

/// How a ship comes to rest on level ground, its centre of mass where it is
/// (see `Profile::rest`): its tilt, how high its centre stands, and what it
/// stands on then (ground x, z under its centre: a convex polygon).
#[derive(Clone, Debug)]
pub struct Rest {
    pub tilt: glam::DQuat,
    pub height: f64,
    pub footprint: Vec<glam::DVec2>,
}

impl Profile {
    fn of(ship: &Ship, key: (usize, u64)) -> Self {
        let struts = struts(ship);
        let hull = hull(ship);
        let reach = struts.iter().map(|s| s.foot.length()).chain(hull.iter().map(|p| p.at.length() + p.radius)).fold(0.0, f64::max);
        let (stand, feet): (f64, Vec<glam::DVec2>) = if struts.is_empty() {
            // On its hull: its lowest corners, within a band widened (0.3 m,
            // doubling) until they make a footprint of some size: a hull rests
            // on its lowest face, near enough level.
            let low = hull.iter().map(|p| p.radius - p.at.y).fold(0.0, f64::max);
            let mut band = 0.3;
            let lowest = |band: f64| convex(hull.iter().filter(|p| p.radius - p.at.y > low - band).map(|p| glam::DVec2::new(p.at.x, p.at.z)).collect());
            while area(&lowest(band)) < 1.0 && band < reach {
                band *= 2.0;
            }
            (low, lowest(band))
        } else {
            (struts.iter().map(|s| -s.foot.y).fold(0.0, f64::max), struts.iter().map(|s| glam::DVec2::new(s.foot.x, s.foot.z)).collect())
        };
        Self { key, struts, hull, reach, stand, footprint: convex(feet), sinks: Default::default(), rests: Default::default() }
    }

    /// How it comes to rest on level ground, its centre of mass at `com`
    /// (body frame), on what it stands on (its feet, else its hull, taken as
    /// rigid): the tilt, within 30 degrees of upright, that brings its centre
    /// lowest, as a body settles. Worked out without flying; kept.
    pub fn rest(&self, com: DVec3) -> Rest {
        let key = [(com.x * 10.0).round() as i64, (com.y * 10.0).round() as i64, (com.z * 10.0).round() as i64];
        if let Some((_, r)) = self.rests.lock().unwrap_or_else(|e| e.into_inner()).iter().find(|(k, _)| *k == key) {
            return r.clone();
        }
        let points: Vec<DVec3> = if self.struts.is_empty() { self.hull.iter().map(|p| p.at - DVec3::Y * p.radius).collect() } else { self.struts.iter().map(|s| s.foot).collect() };
        // Its centre's height over the ground, tilted by `q`: its lowest point on the ground.
        let height = |q: glam::DQuat| -> f64 {
            let low = points.iter().map(|p| (q * *p).y).fold(f64::INFINITY, f64::min);
            (q * com).y - low
        };
        let tilt = |pitch: f64, roll: f64| glam::DQuat::from_rotation_x(pitch.to_radians()) * glam::DQuat::from_rotation_z(roll.to_radians());
        // (By the degree, then by the tenth round the best.)
        let mut best = (height(glam::DQuat::IDENTITY), 0.0, 0.0);
        for pitch in (-30..=30).map(f64::from) {
            for roll in (-30..=30).map(f64::from) {
                let h = height(tilt(pitch, roll));
                if h < best.0 - 1e-9 {
                    best = (h, pitch, roll);
                }
            }
        }
        let (_, p0, r0) = best;
        for dp in -10..=10 {
            for dr in -10..=10 {
                let (pitch, roll) = (p0 + f64::from(dp) * 0.1, r0 + f64::from(dr) * 0.1);
                let h = height(tilt(pitch, roll));
                if h < best.0 - 1e-9 {
                    best = (h, pitch, roll);
                }
            }
        }
        let (h, q) = (best.0, tilt(best.1, best.2));
        let low = points.iter().map(|p| (q * *p).y).fold(f64::INFINITY, f64::min);
        let c = q * com;
        let footprint = convex(points.iter().map(|p| q * *p).filter(|p| p.y < low + 0.05).map(|p| glam::DVec2::new(p.x - c.x, p.z - c.z)).collect());
        let rest = Rest { tilt: q, height: h, footprint };
        let mut memo = self.rests.lock().unwrap_or_else(|e| e.into_inner());
        if memo.len() >= 8 {
            memo.remove(0);
        }
        memo.push((key, rest.clone()));
        rest
    }

    /// The steepest slope (rad) it stands on as it rests (`rest`), its centre
    /// of mass at `com`, without tipping over what it stands on: the angle at
    /// which its centre comes over the nearest edge.
    pub fn tip_slope(&self, com: DVec3) -> f64 {
        let r = self.rest(com);
        let n = r.footprint.len();
        if n < 3 {
            return 0.0;
        }
        let mut inside = f64::INFINITY;
        for k in 0..n {
            let (a, b) = (r.footprint[k], r.footprint[(k + 1) % n]);
            let edge = b - a;
            inside = inside.min(edge.perp_dot(-a) / edge.length().max(1e-9));
        }
        if inside <= 0.0 || r.height <= 0.0 { 0.0 } else { (inside / r.height).atan() }
    }

    /// The steepest slope (rad) it stands on without sliding: its feet's grip (or its hull's).
    pub fn slide_slope(&self) -> f64 {
        if self.struts.is_empty() { HULL_FRICTION.atan() } else { FOOT_FRICTION.atan() }
    }

    /// The fastest it may come down (m/s) on ground of gravity `g` (m/s²),
    /// `mass` kg with its centre at `com`, without a leg giving way: each
    /// leg's spring and damper stopping its share (as the legs share its
    /// weight standing: nearer the centre, more) before the force passes what
    /// it holds or the leg runs out of stroke. Without legs, the hull's strike.
    /// On the safe side: coming down, the body's pitch spreads the blow a
    /// little (the flight finds legs holding up to about a tenth more).
    pub fn max_sink(&self, mass: f64, com: DVec3, g: f64, broken: u64) -> f64 {
        let legs: Vec<&Strut> = self.struts.iter().enumerate().filter(|(k, _)| *k >= 64 || broken & (1 << k) == 0).map(|(_, s)| s).collect();
        if legs.is_empty() {
            return STRIKE;
        }
        // (Within a thousandth of its mass and 10 cm of its centre: the same answer, near enough.)
        let key = ((mass.max(1.0).ln() * 1000.0).round() as i64, [(com.x * 10.0).round() as i64, (com.y * 10.0).round() as i64, (com.z * 10.0).round() as i64], (g * 100.0).round() as i64, broken);
        let mut memo = self.sinks.lock().unwrap_or_else(|e| e.into_inner());
        if let Some((_, v)) = memo.iter().find(|(k, _)| *k == key) {
            return *v;
        }
        let v = shares(&legs, com).iter().zip(&legs).map(|(share, s)| if *share <= 0.0 { f64::INFINITY } else { strut_limit(s, mass * share, g) }).fold(f64::INFINITY, f64::min);
        // (A few kept: fuel burns and cargo shifts slowly.)
        if memo.len() >= 8 {
            memo.remove(0);
        }
        memo.push((key, v));
        v
    }

    /// Will it come to rest standing, set down on ground of slope `slope`
    /// (rad) and gravity `g` at `sink` m/s, its centre of mass at `com`?
    pub fn stands(&self, mass: f64, com: DVec3, g: f64, slope: f64, sink: f64, broken: u64) -> bool {
        sink <= self.max_sink(mass, com, g, broken) && slope <= self.tip_slope(com).min(self.slide_slope())
    }
}

/// How `legs` (alike springs under a rigid body) share a weight whose
/// centre is at `com`, standing level: each its fraction, the fractions
/// summing to one and balancing about the centre (the least-squares plane
/// of their loads; fewer than three legs, or in a line, alike).
fn shares(legs: &[&Strut], com: DVec3) -> Vec<f64> {
    let n = legs.len() as f64;
    let (mut sx, mut sz, mut sxx, mut szz, mut sxz) = (0.0, 0.0, 0.0, 0.0, 0.0);
    for s in legs {
        let (x, z) = (s.foot.x, s.foot.z);
        (sx, sz, sxx, szz, sxz) = (sx + x, sz + z, sxx + x * x, szz + z * z, sxz + x * z);
    }
    let m = glam::DMat3::from_cols(DVec3::new(n, sx, sz), DVec3::new(sx, sxx, sxz), DVec3::new(sz, sxz, szz));
    if legs.len() < 3 || m.determinant().abs() < 1e-9 {
        return vec![1.0 / n; legs.len()];
    }
    let abc = m.inverse() * DVec3::new(1.0, com.x, com.z);
    legs.iter().map(|s| abc.x + abc.y * s.foot.x + abc.z * s.foot.z).collect()
}

/// The fastest a strut carrying `share` kg under `g` stops its share
/// without passing what it holds or its stroke (m/s): its spring and damper
/// run over the stroke, the speed found by halving.
fn strut_limit(s: &Strut, share: f64, g: f64) -> f64 {
    let (k, c) = (s.stiffness(), s.damping());
    let holds_at = |v0: f64| -> bool {
        let (mut x, mut v, h) = (0.0f64, v0, 1e-3);
        for _ in 0..20_000 {
            let f = k * x + c * v.max(0.0);
            if f > s.holds || x > s.stroke {
                return false;
            }
            v += (g - f / share) * h;
            x += v * h;
            if v <= 0.0 {
                return true;
            }
        }
        true
    };
    if !holds_at(0.0) {
        return 0.0;
    }
    let (mut lo, mut hi) = (0.0, 50.0);
    for _ in 0..30 {
        let mid = 0.5 * (lo + hi);
        if holds_at(mid) { lo = mid } else { hi = mid }
    }
    lo
}

/// The area of a convex polygon (m²).
fn area(poly: &[glam::DVec2]) -> f64 {
    (0..poly.len()).map(|k| poly[k].perp_dot(poly[(k + 1) % poly.len()])).sum::<f64>().abs() * 0.5
}

/// The convex hull of `points`, anticlockwise (Andrew's monotone chain).
fn convex(mut points: Vec<glam::DVec2>) -> Vec<glam::DVec2> {
    points.sort_by(|a, b| a.x.total_cmp(&b.x).then(a.y.total_cmp(&b.y)));
    points.dedup_by(|a, b| a.distance_squared(*b) < 1e-6);
    if points.len() < 3 {
        return points;
    }
    let cross = |o: glam::DVec2, a: glam::DVec2, b: glam::DVec2| (a - o).perp_dot(b - o);
    let mut hull: Vec<glam::DVec2> = Vec::new();
    for pass in 0..2 {
        let start = hull.len();
        let it: Box<dyn Iterator<Item = &glam::DVec2>> = if pass == 0 { Box::new(points.iter()) } else { Box::new(points.iter().rev()) };
        for &p in it {
            while hull.len() >= start + 2 && cross(hull[hull.len() - 2], hull[hull.len() - 1], p) <= 0.0 {
                hull.pop();
            }
            hull.push(p);
        }
        hull.pop();
    }
    hull
}

/// The ship's ground profile: kept on it, made again when its design or legs change.
pub fn profile(ship: &mut Ship) -> std::sync::Arc<Profile> {
    let key = profile_key(ship);
    if let Some(p) = &ship.contact
        && p.key == key
    {
        return p.clone();
    }
    let p = std::sync::Arc::new(Profile::of(ship, key));
    ship.contact = Some(p.clone());
    p
}

/// The ship's ground profile as it has it, or worked out now (not kept: a
/// ship looked at, not flown).
pub fn profile_of(ship: &Ship) -> std::sync::Arc<Profile> {
    let key = profile_key(ship);
    match &ship.contact {
        Some(p) if p.key == key => p.clone(),
        _ => std::sync::Arc::new(Profile::of(ship, key)),
    }
}

fn profile_key(ship: &Ship) -> (usize, u64) {
    let spec = ship.spec() as *const crate::ship::ClassSpec as usize;
    let legs = ship.devices.as_ref().map_or(0u64, |d| {
        d.fit.legs.iter().fold(d.fit.legs.len() as u64, |h, l| (h ^ l.foot.x.to_bits() ^ l.foot.y.to_bits().rotate_left(21) ^ l.foot.z.to_bits().rotate_left(42) ^ l.holds.to_bits() ^ l.spring.unwrap_or(0.0).to_bits().rotate_left(7) ^ l.damper.unwrap_or(0.0).to_bits().rotate_left(13)).wrapping_mul(0x100_0000_01b3))
    });
    (spec, legs)
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
    /// Its ground profile (legs, hull points, reach).
    pub profile: std::sync::Arc<Profile>,
    /// The spin the flight computer turned the ship by over the whole step
    /// (already taken): contact turns it by the rest.
    pub spin0: DVec3,
    pub touching: Touching,
}

impl Step {
    pub fn new(ship: &mut Ship, body: usize) -> Self {
        let touching = ship.ground.clone().filter(|t| t.body == body).unwrap_or(Touching { body, ..Default::default() });
        Self { body, mass: mass_props(ship), profile: profile(ship), spin0: ship.angular_velocity, touching }
    }

    /// One substep of `h` at `t`: the contacts' acceleration (world frame);
    /// `rigid` is turned by their torque. Legs that give way are broken on `ship`.
    pub fn apply(&mut self, bodies: &[Body], t: f64, positions: &[DVec3], rigid: &mut RigidBody, ship: &mut Ship, events: &mut Vec<ShipEvent>) -> DVec3 {
        let h = CONTACT_STEP;
        let i = self.body;
        let c = contact::forces(&|p| Some(ground_at(bodies, i, t, positions, p)), rigid, &self.mass, &self.profile.struts, &self.profile.hull, ship.broken_legs, h);
        // Legs pushed past what they hold give way: the ship comes down on that corner.
        for (k, (&f, s)) in c.struts.iter().zip(&self.profile.struts).enumerate() {
            if f > s.holds && k < 64 && ship.broken_legs & (1 << k) == 0 {
                ship.broken_legs |= 1 << k;
                events.push(ShipEvent::LegGaveWay { leg: k, force: f, holds: s.holds });
            }
        }
        // Touching down: as hard as its fastest foot came down.
        let down = c.feet > 0;
        if down
            && !self.touching.feet_down
            && let Some((s, sink)) = self.profile.struts.iter().zip(&c.sinks).filter(|(_, v)| **v > 0.0).max_by(|a, b| a.1.total_cmp(b.1))
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

#[cfg(test)]
mod tests {
    use super::*;
    use glam::DVec2;

    #[test]
    fn a_footprint_is_its_convex_outline_and_tips_where_the_centre_crosses_it() {
        let hull = convex(vec![DVec2::new(6.0, 6.0), DVec2::new(-6.0, 6.0), DVec2::new(0.0, 0.0), DVec2::new(6.0, -6.0), DVec2::new(-6.0, -6.0)]);
        assert_eq!(hull.len(), 4, "the middle point is inside: {hull:?}");
        let mut ship = Ship::new(DVec3::ZERO, DVec3::ZERO, glam::DQuat::IDENTITY);
        let p = profile(&mut ship);
        // The stock hull's four legs drawn 12 m down, 6 m out each way: centred, it tips at atan(6/12).
        assert_eq!(p.footprint.len(), 4);
        assert!((p.tip_slope(DVec3::ZERO) - 0.5f64.atan()).abs() < 1e-9);
        // Its centre 6 m to one side, over the edge: it doesn't stand at all.
        assert_eq!(p.tip_slope(DVec3::new(6.0, 0.0, 0.0)), 0.0);
        assert!(std::sync::Arc::ptr_eq(&p, &profile(&mut ship)), "kept, not made again");
    }

    #[test]
    fn the_hardest_landing_lies_between_what_the_legs_hold_and_what_breaks_them() {
        let mut ship = Ship::new(DVec3::ZERO, DVec3::ZERO, glam::DQuat::IDENTITY);
        let p = profile(&mut ship);
        let com = ship.centre_of_mass();
        let v = p.max_sink(ship.mass(), com, 9.81, 0);
        // (The sandbox drops: 2.4 m/s holds, 6.3 m/s breaks them.)
        assert!(v > 2.4 && v < 6.3, "{v}");
        assert!(p.max_sink(ship.mass(), com, 9.81, 0b1111) == STRIKE, "no legs left: the hull's");
        assert!(p.max_sink(ship.mass(), com, 25.0, 0) < v, "heavier ground, softer landing");
    }
}
