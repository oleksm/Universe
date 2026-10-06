//! The charted world, as the celestial registry has it (Freefall Facts'
//! `standards/Celestial`, read from `crate::registry`).
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
use universe_physics::Orbit;

use crate::belt::RockClass;
use crate::galaxy::StarClass;
use crate::registry::{Body as RegBody, BodyIdentityKind as RegKind, BodySurfaceTerrain as RegTerrain, InGame, PopulationIdentityKind as PopulationKind, Provenance, Registry};
use crate::system::{BodyKind, StarSystem};
use crate::terrain::{Terrain, TerrainKind};
use crate::units::{G, SUN_MASS};

/// A system written out, found by its number among the seed's stars: the
/// registry's records of it, in the game's terms (a body's parent by name,
/// angles in radians, a star's mass and light in Suns).
#[derive(Clone, Debug)]
pub struct System {
    pub system: String,
    pub index: usize,
    pub status: Provenance,
    pub star: Star,
    pub bodies: Vec<Body>,
    pub fields: Vec<Field>,
}

#[derive(Clone, Debug)]
pub struct Star {
    /// O, B, A, F, G, K or M.
    pub class: String,
    /// Times the Sun's.
    pub mass: f64,
    /// Times the Sun's.
    pub luminosity: f64,
}

/// One natural body as its record says (kg, m, s, rad).
#[derive(Clone, Debug)]
pub struct Body {
    /// The registry's key: what the seed's body is matched by (`system::Body::key`).
    pub key: String,
    /// What its people call it: the seed's body takes this name.
    pub name: String,
    pub status: Provenance,
    pub kind: BodyKind,
    /// What it goes round, by key.
    pub parent: String,
    pub mass: f64,
    pub radius: f64,
    pub day: f64,
    pub semi_major_axis: Option<f64>,
    pub eccentricity: Option<f64>,
    pub inclination: Option<f64>,
    pub tilt: f64,
    pub terrain: Option<TerrainKind>,
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
#[derive(Clone, Debug)]
pub struct Field {
    pub name: String,
    pub status: Provenance,
    pub count: usize,
    /// How far it spreads round its remnant (m).
    pub extent: f64,
    pub class: RockClass,
}

fn taken_over(status: Provenance) -> bool {
    status != Provenance::Seeded
}

fn star_class(letter: &str) -> Option<StarClass> {
    [StarClass::O, StarClass::B, StarClass::A, StarClass::F, StarClass::G, StarClass::K, StarClass::M].into_iter().find(|c| c.letter().to_string() == letter)
}

impl Body {
    /// Whether the record stands in place of the seed's body.
    pub fn overrides(&self) -> bool {
        taken_over(self.status)
    }
}

/// The systems the registry writes out, each with its star, the bodies and
/// fields of asteroids the game makes (not the small bodies and regions it
/// doesn't make yet). Made once from the records.
pub fn systems() -> &'static [System] {
    static SYSTEMS: std::sync::OnceLock<Vec<System>> = std::sync::OnceLock::new();
    SYSTEMS.get_or_init(|| {
        let reg = crate::registry::registry();
        reg.systems.iter().map(|s| system(reg, s)).collect()
    })
}

/// The part of a key after its kind and its system: `body.treistun.x` is in `treistun`.
fn in_system<'a>(key: &'a str, kind: &str, system: &str) -> bool {
    key.strip_prefix(kind).and_then(|k| k.strip_prefix('.')).and_then(|k| k.strip_prefix(system)).is_some_and(|k| k.starts_with('.'))
}

fn system(reg: &'static Registry, s: &'static crate::registry::System) -> System {
    let name = s.identity.key.strip_prefix("system.").unwrap_or_else(|| panic!("{} isn't a system's key", s.identity.key));
    let made = |g: Option<InGame>| g != Some(InGame::NotMade);
    let bodies: Vec<&RegBody> = reg.bodies.iter().filter(|b| in_system(&b.identity.key, "body", name) && made(b.in_game)).collect();
    let need = |what: &str, key: &str, v: Option<f64>| v.unwrap_or_else(|| panic!("{key}: no {what}"));
    let star = bodies.iter().find(|b| b.identity.kind == RegKind::Star).unwrap_or_else(|| panic!("{}: no star", s.identity.key));
    System {
        system: s.identity.name.clone(),
        index: s.identity.index.unwrap_or_else(|| panic!("{}: no index among the seed's stars", s.identity.key)) as usize,
        status: s.provenance,
        star: Star {
            class: star.star.class.clone().unwrap_or_else(|| panic!("{}: no class", star.identity.key)),
            mass: need("mass", &star.identity.key, star.physical.mass) / SUN_MASS,
            luminosity: need("luminosity", &star.identity.key, star.star.luminosity) / universe_physics::laws::SOLAR_LUMINOSITY,
        },
        bodies: bodies
            .iter()
            .filter(|b| b.identity.kind != RegKind::Star)
            .map(|b| {
                let key = &b.identity.key;
                let kind = match b.identity.kind {
                    RegKind::RockyPlanet => BodyKind::Rocky,
                    RegKind::GasGiant => BodyKind::GasGiant,
                    RegKind::IceGiant => BodyKind::IceGiant,
                    RegKind::Moon => BodyKind::Moon,
                    RegKind::Asteroid => BodyKind::Asteroid,
                    // (Small bodies: the engine seeds them; their records are its export.)
                    RegKind::DwarfPlanet => BodyKind::DwarfPlanet,
                    RegKind::Comet => BodyKind::Comet,
                    RegKind::Centaur => BodyKind::Centaur,
                    RegKind::CrossingAsteroid => BodyKind::CrossingAsteroid,
                    RegKind::CapturedMoon => BodyKind::CapturedMoon,
                    k => panic!("{key}: the game makes no {k:?}"),
                };
                let (o, p, f) = (&b.orbit, &b.physical, &b.surface);
                let colour = f.colour.unwrap_or([0.5; 3]);
                Body {
                    key: b.identity.key.clone(),
                    name: b.identity.name.clone(),
                    status: b.provenance,
                    kind,
                    parent: b.identity.parent.clone().unwrap_or_else(|| panic!("{key}: goes round nothing")),
                    mass: need("mass", key, p.mass),
                    radius: need("radius", key, p.radius),
                    day: need("day", key, p.day),
                    semi_major_axis: o.semi_major_axis,
                    eccentricity: o.eccentricity,
                    inclination: o.inclination.map(|d| d.rad()),
                    tilt: p.tilt.map_or(0.0, |d| d.rad()),
                    terrain: f.terrain.map(|t| match t {
                        RegTerrain::Terran => TerrainKind::Terran,
                        RegTerrain::Dry => TerrainKind::Dry,
                        RegTerrain::Cratered => TerrainKind::Cratered,
                    }),
                    relief: f.relief,
                    atmosphere: Some(&b.atmosphere).filter(|a| a.surface_density.is_some()).map(|a| (need("air density", key, a.surface_density), need("scale height", key, a.scale_height), need("air's top", key, a.top))),
                    colour: (colour[0] as f32, colour[1] as f32, colour[2] as f32),
                    rings: p.rings.map(|[inner, outer]| (inner, outer)),
                    landscape: f.landscape.clone(),
                }
            })
            .collect(),
        fields: reg
            .populations
            .iter()
            .filter(|p| in_system(&p.identity.key, "population", name) && made(p.in_game) && matches!(p.identity.kind, PopulationKind::Family | PopulationKind::Trojan | PopulationKind::Outer))
            .map(|p| {
                let key = &p.identity.key;
                let rocks = &p.rocks;
                let class = rocks.class.as_deref().unwrap_or_else(|| panic!("{key}: no class"));
                Field {
                    name: p.identity.name.clone(),
                    status: p.provenance,
                    count: rocks.count.unwrap_or_else(|| panic!("{key}: no count")) as usize,
                    extent: need("extent", key, rocks.extent),
                    class: RockClass::by_key(class).unwrap_or_else(|| panic!("{key}: no {class}")),
                }
            })
            .collect(),
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
    let rec = systems().iter().find(|s| s.index == sys.index)?;
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
    if stage == Stage::Bodies && taken_over(rec.status) {
        // The star.
        if let Some(class) = star_class(&rec.star.class) {
            sys.class = class;
            sys.bodies[0].color = class.color();
        }
        sys.luminosity = rec.star.luminosity;
        repull(sys, 0, rec.star.mass * SUN_MASS);
        // The roster: planets and moons the registry doesn't have are not there (nor their moons).
        let written = |key: &str| rec.bodies.iter().any(|r| r.key == key);
        let mut keep: Vec<bool> = sys.bodies.iter().map(|b| !matches!(b.kind, BodyKind::Rocky | BodyKind::GasGiant | BodyKind::IceGiant | BodyKind::Moon) || written(&b.key)).collect();
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
                let kind = Some(r.kind).filter(|k| *k != BodyKind::Asteroid)?;
                if sys.bodies.iter().any(|b| b.key == r.key) {
                    return None;
                }
                Some((r, sys.bodies.iter().position(|b| b.key == r.parent)?, kind))
            }) else { break };
            let orbit = Orbit::new(r.semi_major_axis.unwrap_or(1.0), r.eccentricity.unwrap_or(0.0), r.inclination.unwrap_or(0.0), 0.0, 0.0, 0.0, sys.bodies[parent].rail.mu + G * r.mass);
            sys.bodies.push(crate::system::natural(r.name.clone(), kind, r.mass, r.radius, r.day, [r.colour.0, r.colour.1, r.colour.2], r.rings, parent, orbit, leaning(DQuat::IDENTITY, r.tilt)));
            sys.bodies.last_mut().expect("just pushed").key = r.key.clone();
        }
    }
    for r in rec.bodies.iter().filter(|r| r.overrides()) {
        let Some(i) = sys.bodies.iter().position(|b| b.key == r.key) else { continue };
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
                if let Some(kind) = r.terrain {
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
        for f in rec.fields.iter().filter(|f| taken_over(f.status)) {
            crate::belt::curate(sys, &f.name, f.count, f.extent, Some(f.class), seed);
        }
    }
    moved
}

/// Every body called as its record calls it, by key: last, once everything the seed names after
/// its bodies (fields, stations, gates) is made with the seed's names, which the records keep in
/// `identity.also`.
pub fn rename(sys: &mut StarSystem) {
    let Some(rec) = systems().iter().find(|s| s.index == sys.index) else { return };
    for r in &rec.bodies {
        if let Some(b) = sys.bodies.iter_mut().find(|b| b.key == r.key) {
            b.name = r.name.clone();
        }
    }
}

#[cfg(test)]
mod rematch {
    /// Every body the registry writes matches a body of the seed by key (or is added by the
    /// record, which gives it the key), whatever the record calls it; and a matched body carries
    /// the record's name. Treistun d is Heath D.
    #[test]
    fn every_record_matched_by_key() {
        let galaxy = crate::galaxy::Galaxy::generate(1984);
        let (mut unmatched, mut renamed, mut matched) = (Vec::new(), 0, 0);
        for rec in super::systems() {
            let sys = crate::system::StarSystem::generate(rec.index, &galaxy.stars[rec.index]);
            assert_eq!(sys.name, rec.system, "system {} by index", rec.index);
            for r in &rec.bodies {
                match sys.bodies.iter().find(|b| b.key == r.key) {
                    Some(b) => {
                        assert_eq!(b.name, r.name, "{}: the record's name", r.key);
                        matched += 1;
                        if r.key != crate::system::body_key(&sys.name, &r.name) {
                            renamed += 1;
                        }
                    }
                    None => unmatched.push(r.key.clone()),
                }
            }
            for b in &sys.bodies {
                assert!(!b.key.is_empty(), "{}: {} has no key", sys.name, b.name);
            }
            if sys.name == "Treistun" {
                let d = sys.bodies.iter().find(|b| b.key == "body.treistun.treistun-d").expect("Treistun d by key");
                assert_eq!(d.name, "Heath D");
            }
        }
        assert!(unmatched.is_empty(), "records matching no body: {unmatched:?}");
        eprintln!("rematch: {matched} records matched by key, {renamed} of them renamed");
        assert!(renamed >= 9, "{renamed} renamed");
    }
}
