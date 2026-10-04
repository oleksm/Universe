//! Charted systems' bodies, as the celestial registry has them (Freefall Facts'
//! `standards/Celestial`, generated into `content/base/celestial.ron` by
//! `tools/standards/build.py`).
//!
//! The seed makes every system. A body the registry has as **curated** or
//! **frozen** is then taken as its record says, in place of what the seed
//! made: the record is the truth. A **seeded** record is what the seed made,
//! written down: the game makes that body itself, and a test holds the two
//! together, so the charted world can't change unnoticed when the code does.

use serde::Deserialize;

use crate::system::{BodyKind, StarSystem};
use crate::terrain::{Terrain, TerrainKind};
use crate::units::G;

/// A system written out, found by its number among the seed's stars.
#[derive(Clone, Debug, Deserialize)]
pub struct System {
    pub system: String,
    pub index: usize,
    pub bodies: Vec<Body>,
}

/// One natural body as its record says (kg, m, s).
#[derive(Clone, Debug, Deserialize)]
pub struct Body {
    pub name: String,
    /// "seeded", "curated" or "frozen".
    pub status: String,
    pub kind: String,
    pub mass: f64,
    pub radius: f64,
    pub day: f64,
    pub semi_major_axis: Option<f64>,
    pub eccentricity: Option<f64>,
    /// "terran", "dry" or "cratered".
    pub terrain: Option<String>,
    pub relief: Option<f64>,
    /// Surface density (kg/m³), scale height (m), top (m).
    pub atmosphere: Option<(f64, f64, f64)>,
    pub colour: (f32, f32, f32),
    pub rings: Option<(f64, f64)>,
}

impl Body {
    /// Whether the record stands in place of the seed's body.
    pub fn overrides(&self) -> bool {
        self.status != "seeded"
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
}

impl System {
    pub fn check(&self) -> Result<(), String> {
        for b in &self.bodies {
            if !["seeded", "curated", "frozen"].contains(&b.status.as_str()) {
                return Err(format!("celestial.ron '{}': no status '{}'", b.name, b.status));
            }
            if !(b.mass > 0.0 && b.radius > 0.0 && b.day != 0.0) {
                return Err(format!("celestial.ron '{}': its mass, radius and day must be given", b.name));
            }
            b.terrain_kind()?;
        }
        Ok(())
    }
}

/// When in a system's making its records are applied.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Stage {
    /// Planets and moons made: their mass, size, day, orbit and rings.
    Bodies,
    /// Terrain and air made: their terrain, relief, air and colour.
    Surfaces,
    /// Asteroids made: their mass, size and day.
    Rocks,
}

/// Takes `sys`'s curated and frozen bodies from the registry, at `stage` of
/// its making. `seed`: the star's (a changed terrain is made from it as the
/// seeded one was).
pub fn apply(sys: &mut StarSystem, stage: Stage, seed: u64) {
    let Some(rec) = crate::content::content().celestial.iter().find(|s| s.index == sys.index) else { return };
    apply_records(sys, &rec.bodies, stage, seed);
}

/// As `apply`, with the records given.
pub fn apply_records(sys: &mut StarSystem, records: &[Body], stage: Stage, seed: u64) {
    let mut pulls = Vec::new();
    for r in records.iter().filter(|r| r.overrides()) {
        let Some(i) = sys.bodies.iter().position(|b| b.name == r.name) else { continue };
        let rock = sys.bodies[i].kind == BodyKind::Asteroid;
        match stage {
            Stage::Bodies | Stage::Rocks if (stage == Stage::Rocks) == rock => {
                let parent_mu = sys.bodies[i].rail.parent.map(|p| sys.bodies[p].rail.mu);
                let b = &mut sys.bodies[i];
                pulls.push((i, G * r.mass / b.rail.mu));
                b.mass = r.mass;
                b.rail.mu = G * r.mass;
                b.rail.radius = r.radius;
                b.rail.day = r.day;
                if stage == Stage::Bodies {
                    b.rings = r.rings;
                }
                if let (Some(o), Some(mu)) = (&b.rail.orbit, parent_mu) {
                    b.rail.orbit = Some(o.reshaped(r.semi_major_axis.unwrap_or(o.semi_major_axis), r.eccentricity.unwrap_or(o.eccentricity), mu));
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
    // (What goes round a body whose mass changed goes round by its new pull: only those, so
    // nothing else of the seed's world moves.)
    for (p, by) in pulls {
        for b in sys.bodies.iter_mut().filter(|b| b.rail.parent == Some(p)) {
            if let Some(o) = &b.rail.orbit {
                b.rail.orbit = Some(o.reshaped(o.semi_major_axis, o.eccentricity, o.mu * by));
            }
        }
    }
}
