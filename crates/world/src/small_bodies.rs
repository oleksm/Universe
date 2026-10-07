//! A system's small bodies, from its seed: the main belt's largest body,
//! asteroids knocked onto orbits that cross the rocky planets', moons the
//! giants have caught, centaurs among the giants, the outer belt's dwarf
//! planets, and comets (returning ones thrown in by the giants, and one from
//! the far cloud). Each follows what is known of the Sun's (the registry's
//! vocabulary); every figure is the registry's, from its seeding records
//! (`seeding.asteroids` for the belts, `seeding.small-bodies` for these).
//!
//! Made after a system's planets, moons and fields; appended to its bodies,
//! so nothing made before them moves.

use std::f64::consts::{PI, TAU};
use std::sync::Arc;

use glam::{DQuat, DVec3};
use universe_physics::Orbit;

use crate::belt::{Rock, RockClass, RockShape, Structure};
use crate::rng::{mix, Rng};
use crate::system::{Body, BodyKind, StarSystem};
use crate::terrain::{Terrain, TerrainKind};
use crate::units::{AU, G, SUN_MASS};

/// A resonance `p:q` with a planet: where the period is p/q of the planet's,
/// as a share of its orbit.
fn resonance(r: &str) -> f64 {
    let (p, q) = r.split_once(':').map(|(p, q)| (p.parse::<f64>().unwrap_or(1.0), q.parse::<f64>().unwrap_or(1.0))).unwrap_or((1.0, 1.0));
    (q / p).powf(2.0 / 3.0)
}

/// A ring's area, from `lo` to `hi` (any unit).
fn ring(lo: f64, hi: f64) -> f64 {
    hi * hi - lo * lo
}

/// Adds the small bodies of `sys` (its frost line `frost`, m; its star's `seed`).
pub(crate) fn add(sys: &mut StarSystem, frost: f64, seed: u64) {
    let frost = frost / AU;
    let reg = crate::registry::registry();
    let Some(laws) = reg.seeding("seeding.asteroids") else { return };
    let (mb, ob, sizes, zones) = (&laws.main_belt, &laws.outer_belt, &laws.sizes, &laws.zones);
    let sb = crate::belts::small_bodies_record();
    let one = |v: Option<f64>, what: &str| v.unwrap_or_else(|| panic!("seeding.small-bodies: no {what}"));
    let two = |v: Option<[f64; 2]>, what: &str| v.unwrap_or_else(|| panic!("seeding.small-bodies: no {what}"));
    // (A size or a span drawn evenly in its logarithm, between least and most.)
    let log = |r: &mut Rng, [lo, hi]: [f64; 2]| r.range(lo.ln(), hi.ln()).exp();
    let even = |r: &mut Rng, [lo, hi]: [f64; 2]| r.range(lo, hi);
    let tilt = |r: &mut Rng, [lo, hi]: [f64; 2]| r.range(lo, hi).to_radians();
    let star_mass = sys.bodies[0].mass;
    let mu = G * star_mass;
    let warm = sys.luminosity.sqrt();
    let au = |i: usize| sys.bodies[i].rail.orbit.as_ref().map_or(0.0, |o| o.semi_major_axis) / AU;
    let planets: Vec<usize> = (0..sys.bodies.len()).filter(|&i| sys.bodies[i].kind.is_planet() && sys.bodies[i].rail.parent == Some(0)).collect();
    let mut planets = planets;
    planets.sort_by(|&a, &b| au(a).total_cmp(&au(b)));
    let giants: Vec<usize> = planets.iter().copied().filter(|&i| matches!(sys.bodies[i].kind, BodyKind::GasGiant | BodyKind::IceGiant)).collect();
    let rocky: Vec<usize> = planets.iter().copied().filter(|&i| sys.bodies[i].kind == BodyKind::Rocky).collect();
    let mut used: Vec<String> = sys.bodies.iter().map(|b| b.name.clone()).collect();
    let mut name = |rng: &mut Rng| loop {
        let n = crate::names::star_name(rng.next_u64());
        if !used.contains(&n) {
            used.push(n.clone());
            return n;
        }
    };
    let rng = |what: u64| Rng::new(mix(seed, 0x5a11_0000 ^ what));
    let class = |key: &str| RockClass::of(key);
    // A class by where a body formed (warm, frost line, cold), by the classes' records.
    let pick = |rng: &mut Rng, zone: &str| {
        let classes: Vec<(RockClass, f64)> = RockClass::all()
            .map(|c| {
                let f = &c.record().found;
                (c, match zone { "warm" => f.warm, "frost_line" => f.frost_line, _ => f.cold }.unwrap_or(0.0))
            })
            .filter(|(c, w)| *w > 0.0 && c.described())
            .collect();
        let total: f64 = classes.iter().map(|c| c.1).sum();
        let mut u = rng.range(0.0, total);
        for (c, w) in &classes {
            if u < *w {
                return *c;
            }
            u -= w;
        }
        classes.last().map_or(class("rock-class.stony"), |c| c.0)
    };
    let gas = giants.iter().copied().find(|&g| sys.bodies[g].kind == BodyKind::GasGiant && au(g) > 0.8 * frost).or(giants.first().copied());
    let main_res = (mb.inner_resonance.clone().unwrap_or("4:1".into()), mb.outer_resonance.clone().unwrap_or("2:1".into()));
    let belt = match gas {
        Some(g) => (au(g) * resonance(&main_res.0), au(g) * resonance(&main_res.1)),
        None => (mb.no_giant_inner.unwrap_or(0.8) * frost, mb.no_giant_outer.unwrap_or(1.3) * frost),
    };
    let sun_belt = (mb.inner_edge.unwrap_or(3.08e11) / AU, mb.outer_edge.unwrap_or(4.89e11) / AU);
    let share = ring(belt.0, belt.1) / ring(sun_belt.0, sun_belt.1);
    let mut made: Vec<Body> = Vec::new();

    // The main belt's largest body: a share of the belt's mass (the Sun's largest has 39%), by the belt's own mix.
    {
        let mut r = rng(1);
        let (wt, ft) = (zones.warm_to.unwrap_or(0.93), zones.frost_to.unwrap_or(1.04));
        let cuts = [belt.0, (wt * frost).clamp(belt.0, belt.1), (ft * frost).clamp(belt.0, belt.1), belt.1];
        let weights: Vec<f64> = (0..3).map(|i| ring(cuts[i], cuts[i + 1])).collect();
        let total: f64 = weights.iter().sum();
        let u = r.range(0.0, total.max(1e-12));
        let zone = if u < weights[0] { "warm" } else if u < weights[0] + weights[1] { "frost_line" } else { "cold" };
        let c = pick(&mut r, zone);
        let d = c.density(Structure::Monolith);
        let belt_mass = mb.mass.unwrap_or(2.39e21) * share;
        let lb = &sb.largest_body;
        let radius = (3.0 * one(lb.share, "largest_body.share") * belt_mass / (4.0 * PI * d)).cbrt();
        let at = one(lb.position, "largest_body.position");
        let a = r.range(belt.0 + at * (belt.1 - belt.0), belt.1 - at * (belt.1 - belt.0));
        let (e, i) = (even(&mut r, two(lb.eccentricity, "largest_body.eccentricity")), tilt(&mut r, two(lb.tilt, "largest_body.tilt")));
        let n = name(&mut r);
        made.push(if radius >= one(lb.round_above, "largest_body.round_above") { round(n, BodyKind::DwarfPlanet, c, radius, d, a * AU, e, i, 0, mu, &mut r) } else { rock(n, BodyKind::Asteroid, c, radius, Some(d), a * AU, e, i, 0, mu, &mut r) });
    }

    // Crossing asteroids: knocked out of the belt onto orbits that come in among the rocky planets.
    {
        let mut r = rng(2);
        let inner: Vec<usize> = rocky.iter().copied().filter(|&p| au(p) < belt.1).collect();
        let cr = &sb.crossing;
        for _ in 0..if inner.is_empty() { 0 } else { one(cr.count, "crossing.count") as usize } {
            let target = *r.pick(&inner);
            let q = au(target) * even(&mut r, two(cr.nearest, "crossing.nearest"));
            let mut far = r.range(belt.0, belt.1);
            if far <= q {
                far = q * even(&mut r, two(cr.farthest_if_inside, "crossing.farthest_if_inside"));
            }
            let (a, e) = ((q + far) / 2.0, (far - q) / (far + q));
            let i = tilt(&mut r, two(cr.tilt, "crossing.tilt"));
            let radius = log(&mut r, two(cr.radius, "crossing.radius"));
            let c = pick(&mut r, "warm");
            let n = name(&mut r);
            made.push(rock(n, BodyKind::CrossingAsteroid, c, radius, None, a * AU, e, i, 0, mu, &mut r));
        }
    }

    // Captured moons: far out round each giant, tilted, stretched, most going round backward.
    for &g in &giants {
        let mut r = rng(3 ^ (g as u64) << 8);
        let gb = &sys.bodies[g];
        let reach = au(g) * AU * (gb.mass / (3.0 * star_mass)).cbrt();
        // (Outside its own moons: no closer than half again the farthest of them.)
        let farthest = sys.bodies.iter().filter(|m| m.rail.parent == Some(g) && m.kind == BodyKind::Moon).filter_map(|m| m.rail.orbit.as_ref()).map(|o| o.semi_major_axis).fold(0.0, f64::max);
        let cp = &sb.captured;
        let least = (one(cp.nearest, "captured.nearest") * reach).max(one(cp.beyond_moons, "captured.beyond_moons") * farthest);
        let [lo, hi] = two(if gb.kind == BodyKind::GasGiant { cp.count_gas_giant } else { cp.count_other }, "captured.count");
        let count = r.int(lo as u32, hi as u32);
        let most = one(cp.farthest, "captured.farthest");
        for _ in 0..count {
            let (back, e) = (r.chance(one(cp.backward, "captured.backward")), even(&mut r, two(cp.eccentricity, "captured.eccentricity")));
            let near = least / (1.0 - e);
            if near >= most * reach {
                continue;
            }
            let a = r.range(near, most * reach);
            let i = if back { tilt(&mut r, two(cp.tilt_backward, "captured.tilt_backward")) } else { tilt(&mut r, two(cp.tilt_forward, "captured.tilt_forward")) };
            let radius = log(&mut r, two(cp.radius, "captured.radius"));
            // (Mostly primitive, D-type, as the Sun's are; carbonaceous where the record can't be made yet.)
            let primitive = class("rock-class.primitive");
            let c = if r.chance(one(cp.primitive, "captured.primitive")) && primitive.described() { primitive } else { class("rock-class.carbonaceous") };
            let n = name(&mut r);
            made.push(rock(n, BodyKind::CapturedMoon, c, radius, None, a, e, i, g, G * gb.mass, &mut r));
        }
    }

    // Centaurs: ice bodies wandering among the giants.
    if giants.len() >= 2 {
        let mut r = rng(4);
        let ce = &sb.centaurs;
        for _ in 0..one(ce.count, "centaurs.count") as usize {
            let a = r.range(au(giants[0]) * one(ce.inner, "centaurs.inner"), au(*giants.last().unwrap()) * one(ce.outer, "centaurs.outer"));
            let (e, i) = (even(&mut r, two(ce.eccentricity, "centaurs.eccentricity")), tilt(&mut r, two(ce.tilt, "centaurs.tilt")));
            let radius = log(&mut r, two(ce.radius, "centaurs.radius"));
            let n = name(&mut r);
            made.push(rock(n, BodyKind::Centaur, class("rock-class.icy"), radius, None, a * AU, e, i, 0, mu, &mut r));
        }
    }

    // Dwarf planets of the outer belt: the Sun's has perhaps so many; this one's by the ground it covers.
    if let Some(&last) = giants.last() {
        let res = (ob.inner_resonance.clone().unwrap_or("3:2".into()), ob.outer_resonance.clone().unwrap_or("2:1".into()));
        let (lo, hi) = (au(last) / resonance(&res.0), au(last) / resonance(&res.1));
        let sun_outer = (ob.inner_edge.unwrap_or(5.9e12) / AU, ob.outer_edge.unwrap_or(7.18e12) / AU);
        let od = &sb.outer_dwarfs;
        let expect = one(od.sun, "outer_dwarfs.sun") * ring(lo, hi) / ring(sun_outer.0, sun_outer.1);
        let mut r = rng(5);
        for _ in 0..(expect.round() as usize).min(one(od.most, "outer_dwarfs.most") as usize) {
            let a = r.range(lo, hi);
            let (e, i) = (even(&mut r, two(od.eccentricity, "outer_dwarfs.eccentricity")), tilt(&mut r, two(od.tilt, "outer_dwarfs.tilt")));
            let radius = even(&mut r, two(od.radius, "outer_dwarfs.radius"));
            let icy = class("rock-class.icy");
            let n = name(&mut r);
            made.push(round(n, BodyKind::DwarfPlanet, icy, radius, icy.density(Structure::Monolith), a * AU, e, i, 0, mu, &mut r));
        }
    }

    // Comets: returning ones thrown in by the giants (under 200 years), and one from the far cloud.
    {
        let mut r = rng(6);
        let comet_density = sizes.comet_density.unwrap_or(600.0);
        let icy = class("rock-class.icy");
        let (rc, cc) = (&sb.returning_comets, &sb.cloud_comet);
        if let Some(&last) = giants.last() {
            for _ in 0..one(rc.count, "returning_comets.count") as usize {
                let q = warm * even(&mut r, two(rc.nearest, "returning_comets.nearest"));
                let [flo, fhi] = two(rc.farthest, "returning_comets.farthest");
                let far = r.range(au(last) * flo, au(last) * fhi);
                let (a, e) = ((q + far) / 2.0, (far - q) / (far + q));
                let i = tilt(&mut r, two(rc.tilt, "returning_comets.tilt"));
                let radius = even(&mut r, two(rc.radius, "returning_comets.radius"));
                let n = name(&mut r);
                made.push(rock(n, BodyKind::Comet, icy, radius, Some(comet_density), a * AU, e, i, 0, mu, &mut r));
            }
        }
        let q = warm * even(&mut r, two(cc.nearest, "cloud_comet.nearest"));
        let [olo, ohi] = two(cc.orbit, "cloud_comet.orbit");
        let a = r.range(olo / AU, ohi / AU) * (star_mass / SUN_MASS).cbrt();
        let i = tilt(&mut r, two(cc.tilt, "cloud_comet.tilt"));
        let radius = even(&mut r, two(cc.radius, "cloud_comet.radius"));
        let n = name(&mut r);
        made.push(rock(n, BodyKind::Comet, icy, radius, Some(comet_density), a * AU, 1.0 - q / a, i, 0, mu, &mut r));
    }
    let first = sys.bodies.len();
    sys.bodies.extend(made);
    sys.small = first..sys.bodies.len();
}

/// An orbit round `parent` (of pull `mu`): size `a` (m), `e`, inclination `i`
/// (rad), the rest drawn.
fn orbit(a: f64, e: f64, i: f64, mu: f64, r: &mut Rng) -> Orbit {
    Orbit::new(a, e, i, r.range(0.0, TAU), r.range(0.0, TAU), r.range(0.0, TAU), mu)
}

/// A small body that is a rock: its shape lumpy, its density its own between
/// its class's as a rubble pile and as one piece (or `density`).
#[allow(clippy::too_many_arguments)]
fn rock(name: String, kind: BodyKind, class: RockClass, radius: f64, density: Option<f64>, a: f64, e: f64, i: f64, parent: usize, mu: f64, r: &mut Rng) -> Body {
    let structure = if radius < crate::belts::rocks().rubble_above { Structure::Monolith } else { Structure::Rubble };
    let density = density.unwrap_or_else(|| if radius >= 200_000.0 { class.density(Structure::Monolith) } else { r.range(class.density(Structure::Rubble), class.density(Structure::Monolith)) });
    let shape = RockShape::new(radius, r);
    let composition = crate::belt::Composition::of(class, r.f64(), r);
    let mass = density * shape.volume();
    let [lo, hi] = crate::belts::small_bodies_record().spin.rock.expect("seeding.small-bodies: no spin.rock");
    let day = r.range(lo.ln(), hi.ln()).exp();
    let orbit = orbit(a, e, i, mu + G * mass, r);
    let tilt = DQuat::from_rotation_arc(DVec3::Y, r.unit_vector());
    let mut b = crate::system::natural(name, kind, mass, shape.radius, day, class.color(), None, parent, orbit, tilt);
    b.rock = Some(Arc::new(Rock { class, structure, composition, density, shape }));
    b
}

/// A small body that has pulled itself round: a cratered ball of `class`.
#[allow(clippy::too_many_arguments)]
fn round(name: String, kind: BodyKind, class: RockClass, radius: f64, density: f64, a: f64, e: f64, i: f64, parent: usize, mu: f64, r: &mut Rng) -> Body {
    let mass = density * 4.0 / 3.0 * PI * radius.powi(3);
    let [lo, hi] = crate::belts::small_bodies_record().spin.round.expect("seeding.small-bodies: no spin.round");
    let day = r.range(lo, hi);
    let orbit = orbit(a, e, i, mu + G * mass, r);
    let tilt = DQuat::from_rotation_arc(DVec3::Y, r.unit_vector());
    let mut b = crate::system::natural(name, kind, mass, radius, day, class.color(), None, parent, orbit, tilt);
    b.terrain = Some(Terrain::new(TerrainKind::Cratered, radius, r.next_u64()));
    b
}
