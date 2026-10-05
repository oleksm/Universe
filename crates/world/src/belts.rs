//! A system's belts as they are (the registry's `seeding.asteroids`, after
//! the Sun's): every rock going round the star on its own orbit, about a
//! million km apart (a kilometre across), far more of them the smaller, made
//! from the seed only where something looks.
//!
//! How a rock is found without making the belt: rocks lie on **rings**, each
//! of one exact orbit size, so a ring turns as one and a patch of it stays a
//! patch for ever (the belt shears, rings don't). Each **size class** (a
//! decade of size: 15-150 m, 150 m-1.5 km...) has its own rings and
//! patches, spaced so a patch holds some 64 rocks: the many small ones close,
//! the few big ones far apart. A patch's rocks are made from its number, the
//! same every time. Rocks near a point now are those of the rings that come
//! within reach of it and of the patches of those rings over it now. Each
//! rock's own eccentricity (up to its class's ring spacing over its orbit)
//! and its tilt spread the rocks between rings, above and below.
//!
//! Simplified: a small rock's orbit is nearly round (the Sun's belt rocks
//! reach 0.1 to 0.15, as only the biggest do here), so a search stays small.
//! A trojan swarm is a cloud of rings within 5% of its giant's orbit; those
//! off its exact orbit drift from its point, a twentieth of a turn in some
//! ten of the giant's years.

use std::f64::consts::{PI, TAU};

use glam::DVec3;
use universe_physics::Orbit;

use crate::belt::{RockClass, Structure};
use crate::rng::{mix, Rng};
use crate::system::{BodyKind, StarSystem};
use crate::units::{AU, G};

/// The rocks a patch holds, about.
pub const PER_PATCH: f64 = 64.0;
/// The size classes: each a decade, from the smallest up (m across); the last open, to this.
pub const LARGEST: f64 = 200_000.0;
/// The most any belt rock's orbit is stretched.
pub const MOST_ECCENTRIC: f64 = 0.2;

/// What kind of belt.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum BeltKind {
    /// Between two resonances of the first gas giant.
    Main,
    /// Sharing giant `giant`'s orbit, 60° ahead of it (`lead`) or behind.
    Trojan { giant: usize, lead: bool },
    /// The icy belt past the outermost giant.
    Outer,
}

/// One belt of a system.
#[derive(Clone, Debug)]
pub struct Belt {
    pub kind: BeltKind,
    /// From and to (m from the star).
    pub inner: f64,
    pub outer: f64,
    /// Its size classes, each with its own rings.
    pub classes: Vec<SizeClass>,
    /// For a trojan swarm: the angle along its orbit it spreads either side of its point (rad).
    pub spread: f64,
    /// Where its patches are measured from at the epoch: a longitude (rad; a
    /// trojan point's, else 0).
    pub centre: f64,
    /// The pull its rocks go round by (m³/s²): the star's, or for a swarm the
    /// giant's orbit's own, so it keeps pace with its giant for good.
    pub mu: f64,
    /// How tilted its rocks' orbits are, typically (rad).
    pub tilt: f64,
    /// How many of its rocks are a kilometre across or more.
    pub over_1km: f64,
    /// The size curve's slope: N(>D) ∝ D^-slope.
    pub slope: f64,
    /// The smallest rock (m across).
    pub smallest: f64,
}

/// One size class of a belt: its rocks from `lo` to `hi` m across, how many,
/// and its rings (orbit sizes, m) `spacing` apart, each cut into patches as long.
#[derive(Clone, Debug)]
pub struct SizeClass {
    pub lo: f64,
    pub hi: f64,
    pub count: f64,
    pub spacing: f64,
    pub rings: Vec<f64>,
    /// The most its rocks' orbits are stretched: about a ring's spacing over the orbit.
    pub eccentric: f64,
}

/// A patch: belt `belt`, size class `class`, ring `ring`, `segment` along it
/// (counted from the ring's start at the epoch).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, serde::Serialize, serde::Deserialize)]
pub struct Patch {
    pub belt: u16,
    pub class: u8,
    pub ring: u32,
    pub segment: u32,
}

/// A belt rock: its orbit round the star, its size and class, and its seed
/// (for its shape and make-up, made when it's looked at).
#[derive(Clone, Debug)]
pub struct BeltRock {
    pub orbit: Orbit,
    /// m across.
    pub diameter: f64,
    pub class: RockClass,
    pub seed: u64,
}

/// A point's longitude round the star (rad), as an orbit's is counted.
pub fn longitude(p: DVec3) -> f64 {
    (-p.z).atan2(p.x)
}

fn resonance(r: &str) -> f64 {
    let (p, q) = r.split_once(':').map(|(p, q)| (p.parse::<f64>().unwrap_or(1.0), q.parse::<f64>().unwrap_or(1.0))).unwrap_or((1.0, 1.0));
    (q / p).powf(2.0 / 3.0)
}

/// The belts of `sys` (its frost line `frost`, m), by the registry's seeding
/// record and the system's own giants.
pub fn belts(sys: &StarSystem, frost: f64) -> Vec<Belt> {
    let reg = crate::registry::registry();
    let Some(laws) = reg.seeding.iter().find(|s| s.identity.key == "seeding.asteroids") else { return Vec::new() };
    let (mb, tr, ob, sizes) = (&laws.main_belt, &laws.trojans, &laws.outer_belt, &laws.sizes);
    let slope = sizes.exponent.unwrap_or(1.89);
    let smallest = sizes.smallest.unwrap_or(15.0);
    let a_of = |i: usize| sys.bodies[i].rail.orbit.as_ref().map_or(0.0, |o| o.semi_major_axis);
    let mut giants: Vec<usize> = (0..sys.bodies.len()).filter(|&i| sys.bodies[i].rail.parent == Some(0) && matches!(sys.bodies[i].kind, BodyKind::GasGiant | BodyKind::IceGiant)).collect();
    giants.sort_by(|&a, &b| a_of(a).total_cmp(&a_of(b)));
    let ring_area = |lo: f64, hi: f64| PI * (hi * hi - lo * lo);
    // (A belt's size classes over `lo` to `hi` (its ground: `arc` of a turn), `over_1km` rocks a km
    // or more, none within `width` of a gap: each decade's rings spaced so a patch holds `PER_PATCH`.)
    let classes = |lo: f64, hi: f64, arc: f64, over_1km: f64, gaps: &[f64], width: f64| -> Vec<SizeClass> {
        let ground = arc / 2.0 * (hi * hi - lo * lo);
        let count = |d: f64| over_1km * (d / 1000.0).powf(-slope);
        let mut out = Vec::new();
        let mut d = smallest;
        while d < LARGEST {
            let top = (d * 10.0).min(LARGEST);
            let top = if top * 10.0 > LARGEST { LARGEST } else { top };
            let n = count(d) - count(top);
            let spacing = (PER_PATCH * ground / n.max(1e-9)).sqrt().clamp(1.0e6, ((hi - lo) / 2.0).max(1.0e6));
            let k = ((hi - lo) / spacing).floor().max(1.0) as usize;
            let rings = (0..k).map(|j| lo + (j as f64 + 0.5) * (hi - lo) / k as f64).filter(|a| !gaps.iter().any(|g| (a - g).abs() < width)).collect();
            out.push(SizeClass { lo: d, hi: top, count: n, spacing, rings, eccentric: (spacing / lo.max(1.0)).min(MOST_ECCENTRIC) });
            d = top;
        }
        out
    };
    let star_mu = G * sys.bodies[0].mass;
    let mut out = Vec::new();
    // The main belt: between the first gas giant's resonances (or round the frost line with none).
    let gas = giants.iter().copied().find(|&g| sys.bodies[g].kind == BodyKind::GasGiant && a_of(g) > 0.8 * frost).or(giants.first().copied());
    let (inner, outer, gaps, width) = match gas {
        Some(g) => {
            let a = a_of(g);
            let r = |s: &Option<String>, d: &str| resonance(s.as_deref().unwrap_or(d));
            // (The Kirkwood gaps: the giant's 3:1, 5:2 and 7:3, each 0.007 of its orbit wide either side.)
            (a * r(&mb.inner_resonance, "4:1"), a * r(&mb.outer_resonance, "2:1"), vec![a * resonance("3:1"), a * resonance("5:2"), a * resonance("7:3")], 0.007 * a)
        }
        None => (mb.no_giant_inner.unwrap_or(0.8) * frost, mb.no_giant_outer.unwrap_or(1.3) * frost, Vec::new(), 0.0),
    };
    if outer > inner {
        let sun = ring_area(mb.inner_edge.unwrap_or(3.08e11), mb.outer_edge.unwrap_or(4.89e11));
        let over_1km = mb.count_over_1km.unwrap_or(1.2e6) * ring_area(inner, outer) / sun;
        out.push(Belt { kind: BeltKind::Main, inner, outer, classes: classes(inner, outer, TAU, over_1km, &gaps, width), spread: 0.0, centre: 0.0, mu: star_mu, tilt: 9f64.to_radians(), over_1km, slope, smallest });
    }
    // Each giant's two swarms, ahead and behind, as many as its mass to Jupiter's.
    for &g in &giants {
        let a = a_of(g);
        let per_swarm = tr.count_over_1km.unwrap_or(1.0e6) / 2.0 * sys.bodies[g].mass / tr.giant_mass.unwrap_or(1.898e27);
        let spread = tr.spread.map_or(26f64.to_radians(), |d| d.rad());
        let Some(orbit) = sys.bodies[g].rail.orbit.as_ref() else { continue };
        let at = longitude(orbit.position(0.0));
        for lead in [true, false] {
            let centre = at + if lead { PI / 3.0 } else { -PI / 3.0 };
            // (A swarm is a cloud round its point: rings within 5% of the giant's orbit.)
            let (inner, outer) = (a * 0.95, a * 1.05);
            out.push(Belt { kind: BeltKind::Trojan { giant: g, lead }, inner, outer, classes: classes(inner, outer, 2.0 * spread, per_swarm, &[], 0.0), spread, centre, mu: orbit.mu, tilt: 10f64.to_radians(), over_1km: per_swarm, slope, smallest });
        }
    }
    // The outer belt: between the outermost giant's resonances.
    if let Some(&last) = giants.last() {
        let a = a_of(last);
        let r = |s: &Option<String>, d: &str| resonance(s.as_deref().unwrap_or(d));
        let (inner, outer) = (a / r(&ob.inner_resonance, "3:2"), a / r(&ob.outer_resonance, "2:1"));
        let sun = ring_area(ob.inner_edge.unwrap_or(5.9e12), ob.outer_edge.unwrap_or(7.18e12));
        // (Counted over 100 km; over 1 km by the size curve.)
        let over_1km = ob.count_over_100km.unwrap_or(1.0e5) * 100f64.powf(slope) * ring_area(inner, outer) / sun;
        out.push(Belt { kind: BeltKind::Outer, inner, outer, classes: classes(inner, outer, TAU, over_1km, &[], 0.0), spread: 0.0, centre: 0.0, mu: star_mu, tilt: ob.thickness.map_or(10f64.to_radians(), |d| d.rad()), over_1km, slope, smallest });
    }
    out
}

impl Belt {
    /// How many of its rocks are `d` m across or more.
    pub fn count_over(&self, d: f64) -> f64 {
        self.over_1km * (d / 1000.0).powf(-self.slope)
    }

    /// How many patches ring `ring` of class `c` is cut into.
    fn segments(&self, c: &SizeClass, ring: usize) -> u32 {
        let arc = if self.spread > 0.0 { 2.0 * self.spread } else { TAU };
        ((c.rings[ring] * arc / c.spacing).round() as u32).max(1)
    }

    /// The angle a patch of ring `ring` of class `c` covers (rad), and where the ring's first starts.
    fn arc(&self, c: &SizeClass, ring: usize) -> (f64, f64) {
        let full = if self.spread > 0.0 { 2.0 * self.spread } else { TAU };
        (full / self.segments(c, ring) as f64, if self.spread > 0.0 { -self.spread } else { 0.0 })
    }
}

/// The rocks of `patch` of `sys` (its belts `belts`; its star's `seed`).
pub fn patch_rocks(sys: &StarSystem, belts: &[Belt], patch: Patch, seed: u64) -> Vec<BeltRock> {
    let Some(belt) = belts.get(patch.belt as usize) else { return Vec::new() };
    let Some(class) = belt.classes.get(patch.class as usize) else { return Vec::new() };
    let ring = patch.ring as usize;
    let Some(&a) = class.rings.get(ring) else { return Vec::new() };
    let mut rng = Rng::new(mix(mix(mix(mix(seed ^ 0xbe17, patch.belt as u64), patch.class as u64), patch.ring as u64), patch.segment as u64));
    let mu = belt.mu;
    // As many as its share of its class's ground holds.
    let patches: f64 = (0..class.rings.len()).map(|k| belt.segments(class, k) as f64).sum();
    let expected = class.count / patches;
    let n = (expected + rng.normal() * expected.sqrt()).round().max(0.0) as usize;
    let (width, start) = belt.arc(class, ring);
    let centre = belt.centre;
    let frost = 2.7 * AU * sys.luminosity.sqrt();
    let zone = if a < 0.93 * frost { "warm" } else if a < 1.04 * frost { "frost_line" } else { "cold" };
    let zone = if matches!(belt.kind, BeltKind::Outer) { "outer" } else if matches!(belt.kind, BeltKind::Trojan { .. }) { "trojans" } else { zone };
    let classes: Vec<(RockClass, f64)> = RockClass::all()
        .filter(|c| c.described())
        .map(|c| {
            let f = &c.record().found;
            (c, match zone { "warm" => f.warm, "frost_line" => f.frost_line, "cold" => f.cold, "trojans" => f.trojans, _ => f.outer }.unwrap_or(0.0))
        })
        .filter(|(_, w)| *w > 0.0)
        .collect();
    let total: f64 = classes.iter().map(|c| c.1).sum();
    let pick = |rng: &mut Rng| {
        let mut u = rng.range(0.0, total.max(1e-12));
        for (c, w) in &classes {
            if u < *w {
                return *c;
            }
            u -= w;
        }
        classes.last().map_or(RockClass::of("rock-class.icy"), |c| c.0)
    };
    (0..n)
        .map(|_| {
            let lambda = centre + start + (patch.segment as f64 + rng.f64()) * width;
            let e = rng.range(0.0, class.eccentric);
            // (Tilts as a belt's are: most a few degrees, some far more.)
            let i = (belt.tilt * (-2.0 * rng.f64().max(1e-12).ln()).sqrt() * 0.7).min(belt.tilt * 3.0);
            let (node, peri) = (rng.range(0.0, TAU), rng.range(0.0, TAU));
            let m0 = lambda - node - peri;
            // (Its size within its class, by the size curve.)
            let (lo, hi) = (class.lo.powf(-belt.slope), class.hi.powf(-belt.slope));
            let diameter = (lo + (hi - lo) * rng.f64()).powf(-1.0 / belt.slope);
            let class = if classes.is_empty() { RockClass::of("rock-class.stony") } else { pick(&mut rng) };
            BeltRock { orbit: Orbit::new(a, e, i, node, peri, m0.rem_euclid(TAU), mu), diameter, class, seed: rng.next_u64() }
        })
        .collect()
}

/// The patches whose rocks may be within `reach` of `p` (m from the star)
/// at `t`, of rocks `smallest` m across or more: each ring's patches over `p`
/// now, for the rings that come that near.
pub fn patches_near(belts: &[Belt], p: DVec3, t: f64, reach: f64, smallest: f64) -> Vec<Patch> {
    // (Its distance from the star, true; its longitude as seen down onto the belt's plane.)
    let r = p.length();
    let angle = longitude(p);
    let mut out = Vec::new();
    for (b, belt) in belts.iter().enumerate() {
        if r + reach < belt.inner * (1.0 - MOST_ECCENTRIC) || r - reach > belt.outer * (1.0 + MOST_ECCENTRIC) {
            continue;
        }
        // (None of its rocks rises higher over the plane than its steepest tilt takes it.)
        if p.y.abs() - reach > belt.outer * (1.0 + MOST_ECCENTRIC) * (belt.tilt * 3.0).sin() {
            continue;
        }
        for (c, class) in belt.classes.iter().enumerate() {
            if class.hi < smallest {
                continue;
            }
            for (k, &a) in class.rings.iter().enumerate() {
                if (a - r).abs() > a * class.eccentric + reach {
                    continue;
                }
                // Where its patches are now: the ring has turned by its mean motion.
                let n = (belt.mu / a.powi(3)).sqrt();
                let (width, start) = belt.arc(class, k);
                // (Its rocks wander ahead and behind by up to twice their eccentricity; and a tilted
                // rock seems shifted, seen down onto the plane, by up to tan(tilt/2) of how high it is
                // over its distance: one near `p` is no higher than `p` and the reach.)
                let steepest = belt.tilt * 3.0;
                let tilted = (steepest / 2.0).tan() * (p.y.abs() + reach) / r.max(1.0);
                let look = 2.0 * class.eccentric + tilted.min(steepest * steepest / 4.0) + reach / a;
                let segs = belt.segments(class, k) as i64;
                // (Its angle from the ring's start: for a swarm, from its point, within half a turn.)
                let rel = angle - n * t - belt.centre;
                let here = if belt.spread > 0.0 { (rel + PI).rem_euclid(TAU) - PI - start } else { (rel - start).rem_euclid(TAU) };
                let lo = ((here - look) / width).floor() as i64;
                let hi = ((here + look) / width).floor() as i64;
                for s in lo..=hi {
                    let segment = if belt.spread > 0.0 {
                        if !(0..segs).contains(&s) {
                            continue;
                        }
                        s
                    } else {
                        s.rem_euclid(segs)
                    };
                    out.push(Patch { belt: b as u16, class: c as u8, ring: k as u32, segment: segment as u32 });
                }
            }
        }
    }
    out.sort();
    out.dedup();
    out
}

/// The rocks `smallest` m across or more within `reach` of `p` at `t`:
/// (their patch, their number in it, the rock, where it is).
pub fn rocks_near(sys: &StarSystem, belts: &[Belt], seed: u64, p: DVec3, t: f64, reach: f64, smallest: f64) -> Vec<(Patch, usize, BeltRock, DVec3)> {
    let mut out = Vec::new();
    for patch in patches_near(belts, p, t, reach, smallest) {
        for (k, rock) in patch_rocks(sys, belts, patch, seed).into_iter().enumerate() {
            if rock.diameter < smallest {
                continue;
            }
            let at = rock.orbit.position(t);
            if at.distance(p) <= reach {
                out.push((patch, k, rock, at));
            }
        }
    }
    out
}

/// A belt rock's density (kg/m³), as its class says for its size.
pub fn density(rock: &BeltRock) -> f64 {
    rock.class.density(if rock.diameter < 200.0 { Structure::Monolith } else { Structure::Rubble })
}

/// Field numbers from here up are belt patches (see `patch_field`): a patch
/// is addressed as a field is (its rocks as `(field, body)`, as a swarm's).
pub const PATCH_FIELD: usize = 1 << 52;

/// Patch `p`'s field number: its belt (8 bits), class (3), ring (17) and
/// segment (24) packed over `PATCH_FIELD`. The same every time, so a rock
/// locked, anchored to or saved is found again.
pub fn patch_field(p: Patch) -> usize {
    PATCH_FIELD | ((p.belt as usize & 0xff) << 44) | ((p.class as usize & 0x7) << 41) | ((p.ring as usize & 0x1_ffff) << 24) | (p.segment as usize & 0xff_ffff)
}

/// The patch field number `f` is, if it's one.
pub fn field_patch(f: usize) -> Option<Patch> {
    (f >= PATCH_FIELD).then(|| Patch { belt: ((f >> 44) & 0xff) as u16, class: ((f >> 41) & 0x7) as u8, ring: ((f >> 24) & 0x1_ffff) as u32, segment: (f & 0xff_ffff) as u32 })
}

/// What a sensor can survey: it makes out a rock `resolves` times its size
/// away (m per m), out to `reach` (m). The fitted sensor's figures (the
/// radar's: a 15 m rock at 150,000 km, a kilometre's at ten million).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Survey {
    pub resolves: f64,
    pub reach: f64,
}

impl Survey {
    /// What ship spec `spec`'s fitted sensors survey (none fitted: nothing).
    pub fn of(spec: &crate::ship::ClassSpec) -> Survey {
        let c = crate::content::content();
        spec.fit
            .iter()
            .find_map(|(_, m)| match c.get(*m).does {
                crate::modules::Does::Sensors { resolves, survey_range, .. } => Some(Survey { resolves, reach: survey_range }),
                _ => None,
            })
            .unwrap_or(Survey { resolves: 0.0, reach: 0.0 })
    }

    /// The registry's radar's (for tests and scenarios).
    pub fn radar() -> Survey {
        let c = crate::content::content();
        match c.handle::<crate::modules::Module>("equipment.sensors.radar.s1").map(|h| &c.get(h).does) {
            Some(crate::modules::Does::Sensors { resolves, survey_range, .. }) => Survey { resolves: *resolves, reach: *survey_range },
            _ => Survey { resolves: 0.0, reach: 0.0 },
        }
    }
}

/// One rock a survey finds: its field and body (for a lock), what it is,
/// how big, and how far.
#[derive(Clone, Debug)]
pub struct Found {
    pub field: usize,
    pub body: usize,
    pub class: RockClass,
    pub diameter: f64,
    pub distance: f64,
}

/// The belt rocks a survey from `p` at `t` resolves: each class of size
/// looked for as far as its smallest resolves, nearest first. Its body
/// number is among the patch's bodies (after the system's own).
pub fn survey(sys: &StarSystem, p: DVec3, t: f64, sensor: Survey) -> Vec<Found> {
    if sensor.resolves <= 0.0 || sensor.reach <= 0.0 {
        return Vec::new();
    }
    let n = sys.bodies.len();
    let mut out = Vec::new();
    let classes = sys.belts.iter().flat_map(|b| b.classes.iter().map(|c| (c.lo, c.hi))).fold(Vec::<(f64, f64)>::new(), |mut v, c| {
        if !v.iter().any(|x| (x.0 - c.0).abs() < 1e-6) {
            v.push(c);
        }
        v
    });
    for (lo, hi) in classes {
        let reach = (lo * sensor.resolves).min(sensor.reach);
        for patch in patches_near(&sys.belts, p, t, reach, lo) {
            if sys.belts[patch.belt as usize].classes[patch.class as usize].lo != lo {
                continue;
            }
            for (k, rock) in patch_rocks(sys, &sys.belts, patch, sys.belt_seed).into_iter().enumerate() {
                let distance = rock.orbit.position(t).distance(p);
                if rock.diameter < hi && distance <= (rock.diameter * sensor.resolves).min(sensor.reach) {
                    out.push(Found { field: patch_field(patch), body: n + k, class: rock.class, diameter: rock.diameter, distance });
                }
            }
        }
    }
    out.sort_by(|a, b| a.distance.total_cmp(&b.distance));
    out
}

/// A belt rock as a body among the system's: its shape and make-up from its
/// seed, its orbit round the star.
pub fn rock_body(sys: &StarSystem, patch: Patch, k: usize, rock: &BeltRock) -> crate::system::Body {
    let mut rng = Rng::new(rock.seed);
    let structure = if rock.diameter < 200.0 { Structure::Monolith } else { Structure::Rubble };
    let shape = crate::belt::RockShape::new(rock.diameter / 2.0, &mut rng);
    let composition = crate::belt::Composition::of(rock.class, rng.f64(), &mut rng);
    let density = density(rock);
    let mass = density * shape.volume();
    let day = (rng.range(2.3f64.ln(), 30.0f64.ln())).exp() * crate::units::HOUR;
    let tilt = glam::DQuat::from_rotation_arc(DVec3::Y, rng.unit_vector());
    let _ = k;
    let name = format!("{} {}", belt_name(&sys.belts[patch.belt as usize]), designation(rock.seed));
    let mut b = crate::system::natural(name, BodyKind::Asteroid, mass, shape.radius, day, rock.class.color(), None, 0, rock.orbit.clone(), tilt);
    b.rail.attracts = false;
    b.rock = Some(std::sync::Arc::new(crate::belt::Rock { class: rock.class, structure, composition, density, shape }));
    b
}

/// A belt's name, for its rocks' and a patch's.
pub fn belt_name(b: &Belt) -> &'static str {
    match b.kind {
        BeltKind::Main => "MB",
        BeltKind::Trojan { lead: true, .. } => "TL",
        BeltKind::Trojan { lead: false, .. } => "TT",
        BeltKind::Outer => "OB",
    }
}

/// A rock's designation: five letters and digits from its seed, as a survey
/// catalogues it (`4F2A1`).
pub fn designation(seed: u64) -> String {
    const DIGITS: &[u8] = b"0123456789ABCDEFGHJKLMNPQRSTUVWXYZ";
    let mut n = seed;
    (0..5)
        .map(|_| {
            let c = DIGITS[(n % DIGITS.len() as u64) as usize] as char;
            n /= DIGITS.len() as u64;
            c
        })
        .collect()
}
