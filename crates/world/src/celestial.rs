//! The charted world, as the celestial registry has it (Freefall Facts'
//! `standards/Celestial`: the seed's settings from the registry itself
//! (`crate::registry`), the systems still generated into
//! `content/base/celestial.ron` by `tools/standards/build.py`).
//!
//! The seed makes every system. What the registry has as **curated** or
//! **frozen** is then taken as its record says, in place of what the seed
//! made: the record is the truth.
//! - A body: its mass, size, day, orbit (size, shape, tilt), axis, rings,
//!   terrain, relief, air and colour.
//! - A system: its star, and its roster of planets and moons (one the seed
//!   makes with no record is not there; one written that the seed doesn't
//!   make is added).
//! - A field of asteroids: how many rocks, how far they spread, their class.
//!
//! A **seeded** record is what the seed made, written down: the game makes
//! that itself, and a test holds the two together, so the charted world
//! can't change unnoticed when the code does.

use glam::{DQuat, DVec3};
use serde::Deserialize;
use universe_physics::Orbit;

use crate::galaxy::StarClass;
use crate::system::{BodyKind, StarSystem};
use crate::terrain::{Terrain, TerrainKind};
use crate::units::{G, SUN_MASS};

/// A system written out, found by its number among the seed's stars.
#[derive(Clone, Debug, Deserialize)]
pub struct System {
    pub system: String,
    pub index: usize,
    /// "seeded", "curated" or "frozen".
    pub status: String,
    pub star: Star,
    pub bodies: Vec<Body>,
    pub fields: Vec<Field>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Star {
    /// O, B, A, F, G, K or M.
    pub class: String,
    /// Times the Sun's.
    pub mass: f64,
    pub luminosity: f64,
}

/// One natural body as its record says (kg, m, s, rad).
#[derive(Clone, Debug, Deserialize)]
pub struct Body {
    pub name: String,
    /// "seeded", "curated" or "frozen".
    pub status: String,
    pub kind: String,
    pub parent: String,
    pub mass: f64,
    pub radius: f64,
    pub day: f64,
    pub semi_major_axis: Option<f64>,
    pub eccentricity: Option<f64>,
    pub inclination: Option<f64>,
    pub tilt: f64,
    /// "terran", "dry" or "cratered".
    pub terrain: Option<String>,
    pub relief: Option<f64>,
    /// Surface density (kg/m³), scale height (m), top (m).
    pub atmosphere: Option<(f64, f64, f64)>,
    pub colour: (f32, f32, f32),
    pub rings: Option<(f64, f64)>,
    /// A frozen body's made landscape: the file it is kept in. Carried, not
    /// read yet (the planet-evolution work defines what is in it).
    pub landscape: Option<String>,
}

/// A field of asteroids as its record says.
#[derive(Clone, Debug, Deserialize)]
pub struct Field {
    pub name: String,
    pub status: String,
    pub count: usize,
    /// How far it spreads round its remnant (m).
    pub extent: f64,
    pub class: String,
}

fn taken_over(status: &str) -> bool {
    status != "seeded"
}

fn star_class(letter: &str) -> Option<StarClass> {
    [StarClass::O, StarClass::B, StarClass::A, StarClass::F, StarClass::G, StarClass::K, StarClass::M].into_iter().find(|c| c.letter().to_string() == letter)
}

impl Body {
    /// Whether the record stands in place of the seed's body.
    pub fn overrides(&self) -> bool {
        taken_over(&self.status)
    }

    fn terrain_kind(&self) -> Result<Option<TerrainKind>, String> {
        Ok(match self.terrain.as_deref() {
            None => None,
            Some("terran") => Some(TerrainKind::Terran),
            Some("dry") => Some(TerrainKind::Dry),
            Some("cratered") => Some(TerrainKind::Cratered),
            Some(k) => return Err(format!("celestial.ron '{}': no terrain '{k}'", self.name)),
        })
    }

    fn body_kind(&self) -> Option<BodyKind> {
        [BodyKind::Rocky, BodyKind::GasGiant, BodyKind::IceGiant, BodyKind::Moon, BodyKind::Asteroid].into_iter().find(|k| k.label() == self.kind)
    }
}

impl System {
    pub fn check(&self) -> Result<(), String> {
        let known = |s: &str| ["seeded", "curated", "frozen"].contains(&s);
        if !known(&self.status) {
            return Err(format!("celestial.ron '{}': no status '{}'", self.system, self.status));
        }
        if star_class(&self.star.class).is_none() {
            return Err(format!("celestial.ron '{}': no star class '{}'", self.system, self.star.class));
        }
        for b in &self.bodies {
            if !known(&b.status) {
                return Err(format!("celestial.ron '{}': no status '{}'", b.name, b.status));
            }
            if !(b.mass > 0.0 && b.radius > 0.0 && b.day != 0.0) {
                return Err(format!("celestial.ron '{}': its mass, radius and day must be given", b.name));
            }
            if b.body_kind().is_none() {
                return Err(format!("celestial.ron '{}': no kind '{}'", b.name, b.kind));
            }
            b.terrain_kind()?;
        }
        for f in &self.fields {
            if !known(&f.status) || crate::belt::RockClass::named(&f.class).is_none() {
                return Err(format!("celestial.ron '{}': its status or its class is not one there is", f.name));
            }
        }
        Ok(())
    }
}

/// When in a system's making its records are applied.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Stage {
    /// Star, planets and moons made: the star, the roster, and each body's
    /// mass, size, day, orbit, axis and rings.
    Bodies,
    /// Terrain and air made: their terrain, relief, air and colour.
    Surfaces,
    /// Asteroids made: their mass, size and day; and the fields.
    Rocks,
}

/// Takes what the registry has curated or frozen of `sys`, at `stage` of its
/// making. `seed`: the star's (what is made anew is made from it). Where
/// bodies were taken off its roster: each old number's new one.
pub fn apply(sys: &mut StarSystem, stage: Stage, seed: u64) -> Option<Vec<Option<usize>>> {
    let rec = crate::content::content().celestial.iter().find(|s| s.index == sys.index)?;
    apply_records(sys, rec, stage, seed)
}

/// A body's pull changed: its own orbit and what goes round it follow (a
/// pair's separation orbits by their combined mass).
fn repull(sys: &mut StarSystem, i: usize, mass: f64) {
    let by = G * mass - sys.bodies[i].rail.mu;
    sys.bodies[i].mass = mass;
    sys.bodies[i].rail.mu = G * mass;
    for (j, b) in sys.bodies.iter_mut().enumerate() {
        if j == i || b.rail.parent == Some(i) {
            if let Some(o) = &b.rail.orbit {
                b.rail.orbit = Some(o.reshaped(o.semi_major_axis, o.eccentricity, o.mu + by));
            }
        }
    }
}

/// An axis leaning `angle` (rad) from upright, the way `as_was` leans.
fn leaning(as_was: DQuat, angle: f64) -> DQuat {
    let axis = as_was * DVec3::Y;
    let flat = DVec3::new(axis.x, 0.0, axis.z);
    let toward = if flat.length_squared() > 1e-18 { flat.normalize() } else { DVec3::X };
    DQuat::from_axis_angle(DVec3::Y.cross(toward).normalize(), angle)
}

/// As `apply`, with the records given.
pub fn apply_records(sys: &mut StarSystem, rec: &System, stage: Stage, seed: u64) -> Option<Vec<Option<usize>>> {
    let mut moved = None;
    if stage == Stage::Bodies && taken_over(&rec.status) {
        // The star.
        if let Some(class) = star_class(&rec.star.class) {
            sys.class = class;
            sys.bodies[0].color = class.color();
        }
        sys.luminosity = rec.star.luminosity;
        repull(sys, 0, rec.star.mass * SUN_MASS);
        // The roster: planets and moons the registry doesn't have are not there (nor their moons).
        let written = |name: &str| rec.bodies.iter().any(|r| r.name == name);
        let mut keep: Vec<bool> = sys.bodies.iter().map(|b| !matches!(b.kind, BodyKind::Rocky | BodyKind::GasGiant | BodyKind::IceGiant | BodyKind::Moon) || written(&b.name)).collect();
        for i in 0..sys.bodies.len() {
            if sys.bodies[i].rail.parent.is_some_and(|p| !keep[p]) {
                keep[i] = false;
            }
        }
        if keep.contains(&false) {
            let mut next = 0;
            let map: Vec<Option<usize>> = keep.iter().map(|&k| k.then(|| { next += 1; next - 1 })).collect();
            let mut i = 0;
            sys.bodies.retain(|_| { i += 1; keep[i - 1] });
            for b in &mut sys.bodies {
                b.rail.parent = b.rail.parent.and_then(|p| map[p]);
            }
            moved = Some(map);
        }
        // And those written that the seed doesn't make are added, each once what it goes round is there.
        loop {
            let Some((r, parent, kind)) = rec.bodies.iter().find_map(|r| {
                let kind = r.body_kind().filter(|k| *k != BodyKind::Asteroid)?;
                if sys.bodies.iter().any(|b| b.name == r.name) {
                    return None;
                }
                Some((r, sys.bodies.iter().position(|b| b.name == r.parent)?, kind))
            }) else { break };
            let orbit = Orbit::new(r.semi_major_axis.unwrap_or(1.0), r.eccentricity.unwrap_or(0.0), r.inclination.unwrap_or(0.0), 0.0, 0.0, 0.0, sys.bodies[parent].rail.mu + G * r.mass);
            sys.bodies.push(crate::system::natural(r.name.clone(), kind, r.mass, r.radius, r.day, [r.colour.0, r.colour.1, r.colour.2], r.rings, parent, orbit, leaning(DQuat::IDENTITY, r.tilt)));
        }
    }
    for r in rec.bodies.iter().filter(|r| r.overrides()) {
        let Some(i) = sys.bodies.iter().position(|b| b.name == r.name) else { continue };
        let rock = sys.bodies[i].kind == BodyKind::Asteroid;
        match stage {
            Stage::Bodies | Stage::Rocks if (stage == Stage::Rocks) == rock => {
                repull(sys, i, r.mass);
                let b = &mut sys.bodies[i];
                b.rail.radius = r.radius;
                b.rail.day = r.day;
                if stage == Stage::Bodies {
                    b.rings = r.rings;
                    b.rail.tilt = leaning(b.rail.tilt, r.tilt);
                }
                if let Some(o) = &b.rail.orbit {
                    let o = o.reshaped(r.semi_major_axis.unwrap_or(o.semi_major_axis), r.eccentricity.unwrap_or(o.eccentricity), o.mu);
                    b.rail.orbit = Some(r.inclination.map_or(o.clone(), |i| o.inclined(i)));
                }
            }
            Stage::Surfaces if !rock => {
                let b = &mut sys.bodies[i];
                b.color = [r.colour.0, r.colour.1, r.colour.2];
                if let Ok(Some(kind)) = r.terrain_kind() {
                    if b.terrain.as_ref().is_none_or(|t| t.kind != kind) {
                        b.terrain = Some(Terrain::new(kind, b.rail.radius, crate::rng::mix(seed, 0x7465_7272 + i as u64)));
                    }
                }
                if let (Some(t), Some(relief)) = (&mut b.terrain, r.relief) {
                    t.amplitude = relief;
                }
                b.rail.atmosphere = r.atmosphere.map(|(surface_density, scale_height, top)| universe_physics::Atmosphere { surface_density, scale_height, top });
            }
            _ => {}
        }
    }
    if stage == Stage::Rocks {
        for f in rec.fields.iter().filter(|f| taken_over(&f.status)) {
            crate::belt::curate(sys, &f.name, f.count, f.extent, crate::belt::RockClass::named(&f.class), seed);
        }
    }
    moved
}
