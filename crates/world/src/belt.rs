//! Asteroids, placed where physics puts them:
//!
//! - **The main belt** lies inside the first gas giant's orbit, between its
//!   4:1 and 2:1 resonances (for the Sun and Jupiter: 2.1–3.3 AU), with the
//!   Kirkwood gaps at the 3:1, 5:2 and 7:3 resonances left empty. With no
//!   giant to stir it, a belt stays around the frost line. Stony and metallic
//!   bodies inside, dark carbonaceous ones farther out.
//! - **Trojans** share a giant's orbit 60° ahead (L4) and behind (L5):
//!   dark, organic- and ice-rich.
//! - **An icy outer belt** lies past the outermost giant, from its 3:2
//!   resonance out (Neptune's: 39–48 AU).
//!
//! A belt is almost empty. Where the rocks are worth going to is a
//! **field**: a family's largest remnant left by a collision, with a swarm
//! of fragments still bound to it. The swarm lies well inside the
//! remnant's Hill sphere, so its fragments orbit the remnant, not the star:
//! Kepler orbits of hours to days, at centimetres to a metre a second.
//! Families are of one kind: every fragment shares its parent's class.
//!
//! The remnant is one of the system's bodies (it pulls, faintly). Its swarm
//! is generated from the seed the first time anything comes near it (see
//! `StarSystem::field_bodies`). Sizes follow a power law: many boulders, a
//! few hundred-metre rocks, the remnant kilometres across.

use std::f64::consts::{PI, TAU};
use std::sync::{Arc, OnceLock};

use glam::{DQuat, DVec3};
use universe_physics::{Collider, Orbit, RailBody, Surface};

use crate::names;
use crate::rng::{mix, Rng};
use crate::system::{Body, BodyKind, StarSystem};
use crate::units::*;

/// Smallest fragment in a swarm (m across).
pub const SMALLEST: f64 = 15.0;
/// A swarm reaches at most this far from its remnant (m)...
const SWARM_MAX: f64 = 60_000.0;
/// ...and no farther than this share of the remnant's Hill radius (orbits
/// out there are stable against the star's tides).
const SWARM_HILL: f64 = 0.3;

/// What an asteroid is made of, by its spectrum.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum RockClass {
    /// C-type: clays with water bound in, carbon and organics. Coal-dark.
    Carbonaceous,
    /// S-type: silicates with nickel-iron grains.
    Stony,
    /// M-type: nickel-iron, with platinum-group metals.
    Metallic,
    /// Comet-like: water ice and other frozen volatiles, with dust.
    Icy,
}

impl RockClass {
    pub fn label(self) -> &'static str {
        match self {
            RockClass::Carbonaceous => "C-TYPE CARBONACEOUS",
            RockClass::Stony => "S-TYPE STONY",
            RockClass::Metallic => "M-TYPE METALLIC",
            RockClass::Icy => "ICY",
        }
    }

    /// Its kind, for lists (nine characters at most).
    pub fn letter(self) -> &'static str {
        match self {
            RockClass::Carbonaceous => "C-TYPE",
            RockClass::Stony => "S-TYPE",
            RockClass::Metallic => "M-TYPE",
            RockClass::Icy => "ICY",
        }
    }

    /// Bulk density (kg/m³): a rubble pile is a third or more empty space.
    pub fn density(self, structure: Structure) -> f64 {
        match (self, structure) {
            (RockClass::Carbonaceous, Structure::Rubble) => 1300.0,
            (RockClass::Carbonaceous, Structure::Monolith) => 2100.0,
            (RockClass::Stony, Structure::Rubble) => 2300.0,
            (RockClass::Stony, Structure::Monolith) => 3300.0,
            (RockClass::Metallic, Structure::Rubble) => 4500.0,
            (RockClass::Metallic, Structure::Monolith) => 7500.0,
            (RockClass::Icy, Structure::Rubble) => 700.0,
            (RockClass::Icy, Structure::Monolith) => 950.0,
        }
    }

    /// The share of light it reflects: C-types are coal-dark, ice bright.
    pub fn albedo(self) -> f32 {
        match self {
            RockClass::Carbonaceous => 0.05,
            RockClass::Stony => 0.22,
            RockClass::Metallic => 0.15,
            RockClass::Icy => 0.6,
        }
    }

    fn color(self) -> [f32; 3] {
        match self {
            RockClass::Carbonaceous => [0.42, 0.4, 0.38],
            RockClass::Stony => [0.72, 0.6, 0.45],
            RockClass::Metallic => [0.7, 0.72, 0.78],
            RockClass::Icy => [0.75, 0.88, 1.0],
        }
    }
}

/// How an asteroid holds together.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Structure {
    /// Loose boulders and gravel held by their own weak gravity: easy to dig,
    /// and it can't spin faster than about once in 2.2 hours or it flies apart.
    Rubble,
    /// Solid rock or metal: it has to be cut. Small ones can spin in minutes.
    Monolith,
}

impl Structure {
    pub fn label(self) -> &'static str {
        match self {
            Structure::Rubble => "RUBBLE PILE",
            Structure::Monolith => "MONOLITH",
        }
    }
}

/// Mass fractions, and platinum-group metals in parts per million.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Composition {
    pub water: f64,
    /// Carbon and organic compounds.
    pub organics: f64,
    pub silicates: f64,
    /// Nickel-iron.
    pub metal: f64,
    /// Frozen CO₂, ammonia, methane.
    pub volatiles: f64,
    pub pgm_ppm: f64,
}

impl Composition {
    /// A body of `class`, its family's `grade` (0..1: lean to rich) varied by `rng`.
    fn of(class: RockClass, grade: f64, rng: &mut Rng) -> Self {
        let g = (grade + rng.range(-0.15, 0.15)).clamp(0.0, 1.0);
        let lerp = |lo: f64, hi: f64| lo + (hi - lo) * g;
        let (water, organics, metal, volatiles, pgm_ppm) = match class {
            RockClass::Carbonaceous => (lerp(0.05, 0.2), lerp(0.02, 0.06), 0.02, 0.0, lerp(0.2, 1.0)),
            RockClass::Stony => (0.0, 0.0, lerp(0.1, 0.3), 0.0, lerp(1.0, 6.0)),
            RockClass::Metallic => (0.0, 0.0, lerp(0.8, 0.95), 0.0, 10.0 * 6f64.powf(g)),
            RockClass::Icy => (lerp(0.4, 0.7), lerp(0.03, 0.08), 0.0, lerp(0.05, 0.15), 0.0),
        };
        let silicates = 1.0 - water - organics - metal - volatiles;
        Self { water, organics, silicates, metal, volatiles, pgm_ppm }
    }
}

/// An asteroid's shape: an ellipsoid with lumps. Heights are over the
/// body's base radius, as for terrain (see `universe_physics::Surface`).
#[derive(Clone, Debug)]
pub struct RockShape {
    pub radius: f64,
    /// Semi-axes as shares of the radius (body frame x, y, z).
    pub axes: DVec3,
    /// Lumps (m).
    pub lumps: f64,
    seed: u64,
}

impl RockShape {
    fn new(radius: f64, rng: &mut Rng) -> Self {
        // Elongated, as small bodies are (axis ratios of 0.5–0.9).
        let (b, c) = (rng.range(0.65, 0.95), rng.range(0.5, 0.85));
        let scale = (b * c).cbrt().recip();
        Self { radius, axes: DVec3::new(scale, b * scale, c * scale), lumps: radius * rng.range(0.08, 0.18), seed: rng.next_u64() }
    }

    /// Volume (m³).
    pub fn volume(&self) -> f64 {
        4.0 / 3.0 * PI * self.radius.powi(3) * self.axes.x * self.axes.y * self.axes.z
    }
}

impl Surface for RockShape {
    fn height(&self, dir: DVec3) -> f64 {
        let e = 1.0 / (dir / self.axes).length();
        let p = dir * 1.6;
        let lumps = crate::terrain::value_noise(self.seed, p) * 0.7 + crate::terrain::value_noise(self.seed ^ 0x5eed, p * 2.3) * 0.3;
        self.radius * (e - 1.0) + self.lumps * lumps
    }

    fn max_height(&self) -> f64 {
        self.radius * (self.axes.max_element() - 1.0) + self.lumps
    }
}

/// An asteroid: what it is, and its shape.
#[derive(Clone, Debug)]
pub struct Rock {
    pub class: RockClass,
    pub structure: Structure,
    pub composition: Composition,
    /// Bulk density (kg/m³).
    pub density: f64,
    pub shape: RockShape,
}

/// Where a field is.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum FieldKind {
    /// A collisional family in the main belt.
    Family,
    /// A group in a giant's Trojan swarm (body index), 60° ahead (L4) or behind (L5).
    Trojan { planet: usize, lead: bool },
    /// A family in the icy belt past the giants.
    Outer,
}

/// A field: a family's remnant (one of the system's bodies) and its swarm.
#[derive(Debug)]
pub struct Field {
    pub name: String,
    pub kind: FieldKind,
    /// The remnant: index into the system's bodies.
    pub body: usize,
    /// The swarm reaches this far from the remnant (m).
    pub extent: f64,
    /// Fragments in the swarm.
    pub count: usize,
    grade: f64,
    seed: u64,
    /// The system's bodies followed by the swarm's (see `StarSystem::field_bodies`).
    local: OnceLock<Arc<Vec<Body>>>,
}

impl Field {
    pub fn class(&self, sys: &StarSystem) -> RockClass {
        sys.bodies[self.body].rock.as_ref().map_or(RockClass::Stony, |r| r.class)
    }
}

/// A rock of `class` and size `diameter`, its family's `grade`.
fn rock(class: RockClass, diameter: f64, grade: f64, rng: &mut Rng) -> Rock {
    // Small ones are single stones; big ones rubble (metal ones may be solid).
    let structure = if diameter < 200.0 || (class == RockClass::Metallic && rng.chance(0.5)) { Structure::Monolith } else { Structure::Rubble };
    Rock { class, structure, composition: Composition::of(class, grade, rng), density: class.density(structure), shape: RockShape::new(diameter * 0.5, rng) }
}

/// An asteroid body: `rock`, named `name`, orbiting `parent` on `orbit`.
fn body(name: String, rock: Rock, parent: usize, orbit: Orbit, attracts: bool, rng: &mut Rng) -> Body {
    let mass = rock.density * rock.shape.volume();
    // Rubble piles can't spin faster than ~2.2 h; small stones spin in minutes.
    let day = match rock.structure {
        Structure::Rubble => (rng.range(2.3f64.ln(), 30.0f64.ln())).exp() * HOUR,
        Structure::Monolith => (rng.range(0.05f64.ln(), 10.0f64.ln())).exp() * HOUR,
    };
    Body {
        name,
        kind: BodyKind::Asteroid,
        mass,
        color: rock.class.color(),
        rings: None,
        link: None,
        terrain: None,
        rail: RailBody {
            parent: Some(parent),
            orbit: Some(orbit),
            mu: G * mass,
            attracts,
            radius: rock.shape.radius,
            day,
            tilt: DQuat::from_rotation_arc(DVec3::Y, rng.unit_vector()),
            collider: Collider::Surface,
            atmosphere: None,
            pulled_by: Vec::new(),
        },
        rock: Some(Arc::new(rock)),
    }
}

/// A class for a family at `a`, by where it formed (`frost`: the frost line).
fn family_class(a: f64, frost: f64, rng: &mut Rng) -> RockClass {
    let u = rng.f64();
    if a < 0.9 * frost {
        if u < 0.55 { RockClass::Stony } else if u < 0.8 { RockClass::Carbonaceous } else { RockClass::Metallic }
    } else if u < 0.7 {
        RockClass::Carbonaceous
    } else if u < 0.85 {
        RockClass::Stony
    } else if u < 0.95 {
        RockClass::Metallic
    } else {
        RockClass::Icy
    }
}

/// The belts' fields, added to `sys` (its remnants appended to its bodies).
/// `frost` is the frost line (m). Uses its own random stream, so the
/// planets stay as they were.
pub fn add_fields(sys: &mut StarSystem, frost: f64, seed: u64) {
    let mut rng = Rng::new(mix(seed, 0x6265_6c74));
    let star_mu = sys.bodies[0].rail.mu;
    let star_mass = star_mu / G;
    let planets: Vec<(usize, f64, BodyKind)> = sys
        .bodies
        .iter()
        .enumerate()
        .filter(|(_, b)| b.rail.parent == Some(0))
        .filter_map(|(i, b)| Some((i, b.rail.orbit.as_ref()?.semi_major_axis, b.kind)))
        .collect();
    let giant = |k: BodyKind| matches!(k, BodyKind::GasGiant | BodyKind::IceGiant);
    let jupiter = planets.iter().filter(|p| p.2 == BodyKind::GasGiant && p.1 > frost * 0.8).map(|p| p.1).next().or_else(|| planets.iter().find(|p| giant(p.2)).map(|p| p.1));
    let clear = |a: f64| planets.iter().all(|p| (a - p.1).abs() > 0.1 * p.1);

    // (a, e, inclination, kind, class, remnant diameter)
    let mut wanted: Vec<(Orbit, FieldKind, RockClass, f64)> = Vec::new();
    // The main belt.
    let (inner, outer) = match jupiter {
        Some(aj) => (0.40 * aj, 0.63 * aj),
        None => (0.8 * frost, 1.3 * frost),
    };
    let gaps = [0.4807, 0.5428, 0.5703];
    let families = rng.int(2, 5);
    let mut tries = 0;
    while wanted.len() < families as usize && tries < 40 {
        tries += 1;
        let a = rng.range(inner, outer);
        if !clear(a) || jupiter.is_some_and(|aj| gaps.iter().any(|g| (a / aj - g).abs() < 0.007)) {
            continue;
        }
        let orbit = Orbit::new(a, rng.range(0.02, 0.18), rng.range(0.0, 12.0f64).to_radians(), rng.range(0.0, TAU), rng.range(0.0, TAU), rng.range(0.0, TAU), star_mu);
        let class = family_class(a, frost, &mut rng);
        wanted.push((orbit, FieldKind::Family, class, (rng.range(1.0f64.ln(), 8.0f64.ln())).exp() * 1000.0));
    }
    // Trojans: on the giant's orbit, 60° either way.
    for &(i, _, kind) in planets.iter().filter(|p| giant(p.2)) {
        let chance = if kind == BodyKind::GasGiant { 0.8 } else { 0.35 };
        let Some(orbit) = sys.bodies[i].rail.orbit.clone() else { continue };
        for lead in [true, false] {
            if !rng.chance(chance) {
                continue;
            }
            let (r, v) = orbit.state(0.0);
            let turn = DQuat::from_axis_angle(r.cross(v).normalize(), if lead { PI / 3.0 } else { -PI / 3.0 });
            let at = Orbit::from_state(turn * r, turn * v, star_mu, 0.0);
            let class = if rng.chance(0.5) { RockClass::Carbonaceous } else { RockClass::Icy };
            wanted.push((at, FieldKind::Trojan { planet: i, lead }, class, (rng.range(2.0f64.ln(), 10.0f64.ln())).exp() * 1000.0));
        }
    }
    // The icy belt past the giants, from the outermost one's 3:2 resonance.
    if let Some(edge) = planets.iter().filter(|p| giant(p.2)).map(|p| p.1).reduce(f64::max) {
        for _ in 0..rng.int(1, 2) {
            let a = edge * rng.range(1.31, 1.6);
            let orbit = Orbit::new(a, rng.range(0.02, 0.25), rng.range(0.0, 20.0f64).to_radians(), rng.range(0.0, TAU), rng.range(0.0, TAU), rng.range(0.0, TAU), star_mu);
            wanted.push((orbit, FieldKind::Outer, RockClass::Icy, (rng.range(2.0f64.ln(), 12.0f64.ln())).exp() * 1000.0));
        }
    }

    for (orbit, kind, class, diameter) in wanted {
        let grade = rng.f64();
        let rock = rock(class, diameter, grade, &mut rng);
        let rock_name = names::star_name(rng.next_u64());
        let periapsis = orbit.periapsis();
        let name = match kind {
            FieldKind::Trojan { planet, lead } => format!("{rock_name} group ({} {})", sys.bodies[planet].name, if lead { "L4" } else { "L5" }),
            _ => format!("{rock_name} family"),
        };
        let remnant = body(rock_name, rock, 0, orbit, true, &mut rng);
        let hill = periapsis * (remnant.mass / (3.0 * star_mass)).cbrt();
        let extent = (SWARM_HILL * hill).min(SWARM_MAX).max(remnant.rail.radius * 8.0);
        let count = rng.int(120, 260) as usize;
        sys.fields.push(Field { name, kind, body: sys.bodies.len(), extent, count, grade, seed: rng.next_u64(), local: OnceLock::new() });
        sys.bodies.push(remnant);
    }
}

/// Rock `b` with `kg` dug out of it: smaller by what's gone (its volume
/// goes with its mass), its shape kept.
pub fn shrink(b: &mut Body, kg: f64) {
    let left = (b.mass - kg).max(0.0);
    let s = (left / b.mass).cbrt();
    b.rail.radius *= s;
    if let Some(r) = &mut b.rock {
        let r = Arc::make_mut(r);
        r.shape.radius *= s;
        r.shape.lumps *= s;
    }
    b.mass = left;
    b.rail.mu = G * left;
}

impl StarSystem {
    /// The system's bodies followed by field `f`'s swarm: what a ship near
    /// it moves among (the swarm's bodies are generated on first look).
    pub fn field_bodies(&self, f: usize) -> Arc<Vec<Body>> {
        let field = &self.fields[f];
        field
            .local
            .get_or_init(|| {
                let mut bodies = self.bodies.clone();
                bodies.extend(swarm(self, field));
                crate::system::settle_bodies(&mut bodies);
                Arc::new(bodies)
            })
            .clone()
    }

    /// Field `f`'s own rocks, as indices among its bodies (see
    /// `field_bodies`): its remnant, and its swarm.
    pub fn field_rocks(&self, f: usize) -> impl Iterator<Item = usize> {
        let n = self.bodies.len();
        std::iter::once(self.fields[f].body).chain(n..n + self.fields[f].count)
    }

    /// Where body `i` among field `f`'s bodies is at `t`, and how it moves
    /// (solving only it and what it orbits, not the whole swarm).
    pub fn field_body_state(&self, f: usize, i: usize, t: f64) -> (DVec3, DVec3) {
        let bodies = self.field_bodies(f);
        (universe_physics::position(&bodies[..], i, t), universe_physics::velocity(&bodies[..], i, t))
    }


    /// The field whose swarm a point is among or near (`positions`: the
    /// system's bodies), if any.
    pub fn field_near(&self, p: DVec3, positions: &[DVec3]) -> Option<usize> {
        self.fields.iter().position(|f| positions[f.body].distance(p) < f.extent + universe_physics::integrate::FINE_RANGE)
    }
}

/// Field `field`'s swarm: fragments orbiting its remnant.
fn swarm(sys: &StarSystem, field: &Field) -> Vec<Body> {
    let mut rng = Rng::new(field.seed);
    let remnant = &sys.bodies[field.body];
    let class = field.class(sys);
    let (r0, mu) = (remnant.rail.radius, remnant.rail.mu);
    let inner = r0 * 3.0;
    (0..field.count)
        .map(|k| {
            // Cumulative sizes N(>D) ∝ D⁻², up to a tenth of the remnant.
            let diameter = (SMALLEST * rng.f64().max(1e-9).powf(-0.5)).min(r0 * 0.2);
            let rock = rock(class, diameter, field.grade, &mut rng);
            let a = inner * (field.extent / inner).powf(rng.f64());
            // Never closer than half again the remnant's radius.
            let e = rng.range(0.0, (1.0 - (1.5 * r0 + rock.shape.radius * 2.0) / a).clamp(0.0, 0.3));
            let orbit = Orbit::new(a, e, (1.0 - 2.0 * rng.f64()).acos(), rng.range(0.0, TAU), rng.range(0.0, TAU), rng.range(0.0, TAU), mu);
            body(format!("{}-{}", remnant.name, k + 1), rock, field.body, orbit, false, &mut rng)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::galaxy::Galaxy;

    /// Systems of the test galaxy that have fields.
    fn systems() -> Vec<StarSystem> {
        let galaxy = Galaxy::generate(1984, 120);
        (0..120).map(|i| StarSystem::generate(i, &galaxy.stars[i])).collect()
    }

    #[test]
    fn belts_lie_where_physics_puts_them() {
        let systems = systems();
        let mut fields = 0;
        for sys in &systems {
            let a = |i: usize| sys.bodies[i].rail.orbit.as_ref().unwrap().semi_major_axis;
            let giants: Vec<usize> = (0..sys.bodies.len()).filter(|&i| sys.bodies[i].rail.parent == Some(0) && matches!(sys.bodies[i].kind, BodyKind::GasGiant | BodyKind::IceGiant)).collect();
            for f in &sys.fields {
                fields += 1;
                let b = &sys.bodies[f.body];
                assert_eq!(b.kind, BodyKind::Asteroid);
                assert!(b.rail.attracts && b.rail.parent == Some(0));
                match f.kind {
                    FieldKind::Trojan { planet, .. } => {
                        // Sharing the giant's orbit, 60° from it.
                        assert!((a(f.body) / a(planet) - 1.0).abs() < 0.02, "{}", f.name);
                        let mut p = Vec::new();
                        sys.positions(0.0, &mut p);
                        let angle = p[f.body].angle_between(p[planet]).to_degrees();
                        assert!((angle - 60.0).abs() < 8.0, "{}: {angle}°", f.name);
                    }
                    FieldKind::Family if !giants.is_empty() => {
                        // Between some giant's 4:1 and 2:1 resonances.
                        assert!(giants.iter().any(|&g| (0.39..0.64).contains(&(a(f.body) / a(g)))), "{}", f.name);
                    }
                    FieldKind::Outer => assert_eq!(f.class(sys), RockClass::Icy),
                    _ => {}
                }
                // The swarm sits well inside the remnant's Hill sphere.
                let hill = b.rail.orbit.as_ref().unwrap().periapsis() * (b.mass / (3.0 * sys.bodies[0].mass)).cbrt();
                assert!(f.extent <= 0.3 * hill + 1.0 || f.extent == b.rail.radius * 8.0);
            }
        }
        assert!(fields > 120, "{fields} fields in 120 systems");
    }

}
