use std::f64::consts::TAU;

use glam::{DQuat, DVec3};
use universe_physics::{Collider, Ephemeris, OnRails, Orbit, RailBody, Surface};

use crate::galaxy::{GalaxyStar, StarClass};
use crate::names;
use crate::rng::{mix, Rng};
use crate::terrain::{Terrain, TerrainKind};
use crate::units::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BodyKind {
    Star,
    Rocky,
    GasGiant,
    IceGiant,
    Moon,
    Station,
    /// A ring gate linked to a gate in another star system.
    Gate,
    /// An asteroid: a field's remnant, or a fragment of its swarm (see `belt`).
    Asteroid,
    /// A world of rock or ice heavy enough to have pulled itself round, too
    /// small to clear its orbit (see `small_bodies`).
    DwarfPlanet,
    /// A rock knocked out of the belt onto an orbit among the rocky planets.
    CrossingAsteroid,
    /// A body a giant has caught: far out, tilted, often going round backward.
    CapturedMoon,
    /// An ice body wandering among the giants.
    Centaur,
    /// An ice body on a long orbit that brings it in close to the star.
    Comet,
}

impl BodyKind {
    pub fn label(self) -> &'static str {
        match self {
            BodyKind::Star => "star",
            BodyKind::Rocky => "rocky planet",
            BodyKind::GasGiant => "gas giant",
            BodyKind::IceGiant => "ice giant",
            BodyKind::Moon => "moon",
            BodyKind::Station => "station",
            BodyKind::Gate => "gate",
            BodyKind::Asteroid => "asteroid",
            BodyKind::DwarfPlanet => "dwarf planet",
            BodyKind::CrossingAsteroid => "crossing asteroid",
            BodyKind::CapturedMoon => "captured moon",
            BodyKind::Centaur => "centaur",
            BodyKind::Comet => "comet",
        }
    }

    /// A rock (its shape, class and make-up in `Body::rock`): an asteroid,
    /// or a small body that hasn't pulled itself round.
    pub fn is_rock(self) -> bool {
        matches!(self, BodyKind::Asteroid | BodyKind::CrossingAsteroid | BodyKind::CapturedMoon | BodyKind::Centaur | BodyKind::Comet)
    }

    /// A planet (not a moon, a structure or an asteroid).
    pub fn is_planet(self) -> bool {
        matches!(self, BodyKind::Rocky | BodyKind::GasGiant | BodyKind::IceGiant)
    }

    /// Can a ship touch down here (slowly) without being destroyed?
    /// (Stations are docked with through their slot instead; see `station`.)
    pub fn landable(self) -> bool {
        matches!(self, BodyKind::Rocky | BodyKind::Moon | BodyKind::DwarfPlanet)
    }

    /// Does it pull on ships? (Stations and gates are too light to matter.)
    /// Does it pull? Everything with mass does: a station's 10⁹ kg pulls a
    /// ship 500 m off at about 3×10⁻⁷ m/s², a gate's 10¹⁰ kg ten times that
    /// (never enough to count as the dominant body).
    pub fn massive(self) -> bool {
        true
    }

    /// Its shape, for Dogma: a station's hull with its docking
    /// slot, a gate's ring, or a (terrain) surface.
    fn collider(self) -> Collider {
        match self {
            BodyKind::Station => Collider::Blocks(crate::station::hull()),
            BodyKind::Gate => Collider::Ring(crate::gate::ring()),
            _ => Collider::Surface,
        }
    }

    /// Artificial structures: not obstacles for the hyperdrive's surface clearance.
    pub fn artificial(self) -> bool {
        matches!(self, BodyKind::Station | BodyKind::Gate)
    }
}

#[derive(Clone, Debug)]
pub struct Body {
    pub name: String,
    pub kind: BodyKind,
    pub mass: f64,
    pub color: [f32; 3],
    /// Ring inner/outer radius, if any.
    pub rings: Option<(f64, f64)>,
    /// For gates: the galaxy index of the star system it leads to.
    pub link: Option<usize>,
    /// Surface heights, for rocky planets and moons.
    pub terrain: Option<crate::terrain::Terrain>,
    /// How it moves (parent, orbit, spin), pulls and collides: Dogma's part.
    pub rail: RailBody,
    /// For asteroids: what it is, and its shape.
    pub rock: Option<std::sync::Arc<crate::belt::Rock>>,
}

impl OnRails for Body {
    fn rail(&self) -> &RailBody {
        &self.rail
    }

    fn surface(&self) -> Option<&dyn Surface> {
        match (&self.terrain, &self.rock) {
            (Some(t), _) => Some(t as &dyn Surface),
            (_, Some(r)) => Some(&r.shape as &dyn Surface),
            _ => None,
        }
    }
}

impl Body {
    pub fn rotation(&self, t: f64) -> DQuat {
        self.rail.rotation(t)
    }

    pub fn angular_velocity(&self) -> DVec3 {
        self.rail.angular_velocity()
    }

    /// Distance from the center to the surface in a body-frame direction (m).
    pub fn surface_radius(&self, local_dir: DVec3) -> f64 {
        universe_physics::surface_radius(self, local_dir)
    }

    /// Surface radius under a world-space point at time `t` (`center` = body position).
    pub fn surface_radius_at(&self, center: DVec3, p: DVec3, t: f64) -> f64 {
        universe_physics::surface_radius_at(self, center, p, t)
    }

    /// Nothing on the surface reaches higher than this (m from the center).
    pub fn max_radius(&self) -> f64 {
        universe_physics::max_radius(self)
    }
}

/// A landing pad on the surface of a planet or moon.
#[derive(Clone, Debug)]
pub struct Spaceport {
    pub name: String,
    pub body: usize,
    /// Unit vector to the pad in the body's rotating frame.
    pub direction: DVec3,
}

pub struct StarSystem {
    /// Index of this system's star in the galaxy.
    pub index: usize,
    pub name: String,
    pub class: StarClass,
    /// Its star's luminosity (suns): its own, by its seeded mass.
    pub luminosity: f64,
    /// Parents always come before their children; body 0 is the star.
    pub bodies: Vec<Body>,
    pub spaceports: Vec<Spaceport>,
    /// Asteroid fields (see `belt`).
    pub fields: Vec<crate::belt::Field>,
    /// Which of `bodies` are its small bodies (see `small_bodies`): the last ones made.
    pub small: std::ops::Range<usize>,
    /// Its belts as they are, every rock on its own orbit (see `belts`), and
    /// the seed their rocks are made from.
    pub belts: Vec<crate::belts::Belt>,
    pub belt_seed: u64,
    /// Belt patches looked at: their bodies (the system's, then the patch's rocks), kept a while.
    pub patches: std::sync::Mutex<std::collections::HashMap<usize, std::sync::Arc<Vec<Body>>>>,
}

const ROCKY_COLORS: [[f32; 3]; 4] = [[0.8, 0.5, 0.3], [0.65, 0.65, 0.65], [0.85, 0.75, 0.5], [0.75, 0.4, 0.35]];
const TERRAN_COLORS: [[f32; 3]; 2] = [[0.3, 0.85, 0.45], [0.3, 0.6, 1.0]];
const GAS_COLORS: [[f32; 3]; 3] = [[0.9, 0.7, 0.45], [0.85, 0.6, 0.4], [0.8, 0.75, 0.6]];
const ICE_COLORS: [[f32; 3]; 2] = [[0.5, 0.85, 1.0], [0.4, 0.55, 1.0]];
const MOON_COLORS: [[f32; 3]; 3] = [[0.7, 0.7, 0.7], [0.6, 0.58, 0.55], [0.75, 0.7, 0.6]];

/// A planet or moon as given, with nothing made for it yet (terrain and air come after).
#[allow(clippy::too_many_arguments)]
pub(crate) fn natural(name: String, kind: BodyKind, mass: f64, radius: f64, day: f64, color: [f32; 3], rings: Option<(f64, f64)>, parent: usize, orbit: Orbit, tilt: DQuat) -> Body {
    Body {
        name,
        kind,
        mass,
        color,
        rings,
        link: None,
        terrain: None,
        rail: RailBody { parent: Some(parent), orbit: Some(orbit), mu: G * mass, attracts: kind.massive(), radius, day, tilt, collider: kind.collider(), atmosphere: None, pulled_by: Vec::new() },
        rock: None,
    }
}

fn random_tilt(rng: &mut Rng, max_degrees: f64) -> DQuat {
    DQuat::from_rotation_y(rng.range(0.0, TAU)) * DQuat::from_rotation_x(rng.range(0.0, max_degrees).to_radians())
}

/// A gate's path keeps at least this clear of anything else orbiting with
/// it (m): its run-in and the way out are open space.
pub const GATE_CLEARANCE: f64 = 100_000.0;

impl StarSystem {
    pub fn generate(index: usize, star: &GalaxyStar) -> Self {
        let mut rng = Rng::new(star.seed);
        let name = names::star_name(star.seed);
        let class = star.class;
        // (Its first draw: the same as `GalaxyStar::mass_suns`'s.)
        let star_mass = class.mass_suns() * SUN_MASS * rng.range(0.85, 1.15);
        let lum = star.luminosity();
        let star_radius = class.radius_suns() * SUN_RADIUS * rng.range(0.85, 1.15);
        let star_mu = G * star_mass;
        let mut bodies = vec![Body {
            name: name.clone(),
            kind: BodyKind::Star,
            mass: star_mass,
            color: class.color(),
            rings: None,
            link: None,
            terrain: None,
            rail: RailBody {
                parent: None,
                orbit: None,
                mu: star_mu,
                attracts: BodyKind::Star.massive(),
                radius: star_radius,
                day: rng.range(20.0, 35.0) * DAY,
                tilt: random_tilt(&mut rng, 7.0),
                collider: BodyKind::Star.collider(),
                atmosphere: None,
                pulled_by: Vec::new(),
            },
            rock: None,
        }];

        let planet_count = match class {
            StarClass::M => rng.int(1, 6),
            _ => rng.int(2, 9),
        };
        let frost_line = 2.7 * AU * lum.sqrt();
        let habitable = AU * lum.sqrt();
        let mut a = (0.25 * AU * lum.sqrt() * rng.range(0.7, 1.3)).max(star_radius * 8.0);
        let mut station_parent: Option<(usize, f64)> = None; // (body, closeness to habitable zone)

        for p in 0..planet_count {
            let planet_name = format!("{name} {}", (b'b' + p as u8) as char);
            let (kind, mass, radius, color, day, rings) = if a < frost_line {
                let m = 10f64.powf(rng.range(-1.3, 0.7));
                let temperate = (a / habitable).ln().abs() < 0.4;
                let color = if temperate { *rng.pick(&TERRAN_COLORS) } else { *rng.pick(&ROCKY_COLORS) };
                (BodyKind::Rocky, m * EARTH_MASS, m.powf(0.28) * EARTH_RADIUS, color, rng.range(10.0, 60.0) * HOUR, None)
            } else if rng.chance(0.6) {
                let r = rng.range(8.0, 12.0) * EARTH_RADIUS;
                let rings = rng.chance(0.45).then(|| (r * rng.range(1.3, 1.6), r * rng.range(1.9, 2.5)));
                let m = rng.range(40.0, 1000.0) * EARTH_MASS;
                (BodyKind::GasGiant, m, r, *rng.pick(&GAS_COLORS), rng.range(9.0, 16.0) * HOUR, rings)
            } else {
                let r = rng.range(3.5, 4.3) * EARTH_RADIUS;
                let rings = rng.chance(0.2).then_some((r * 1.5, r * 2.0));
                let m = rng.range(10.0, 25.0) * EARTH_MASS;
                (BodyKind::IceGiant, m, r, *rng.pick(&ICE_COLORS), rng.range(14.0, 20.0) * HOUR, rings)
            };
            let e = rng.range(0.0, 0.07);
            let orbit = Orbit::new(
                a,
                e,
                rng.range(0.0, 3.0).to_radians(),
                rng.range(0.0, TAU),
                rng.range(0.0, TAU),
                rng.range(0.0, TAU),
                // (A pair's separation orbits by their combined mass.)
                star_mu + G * mass,
            );
            let planet = bodies.len();
            bodies.push(Body {
                name: planet_name.clone(),
                kind,
                mass,
                color,
                rings,
                link: None,
                terrain: None,
                rail: RailBody {
                    parent: Some(0),
                    orbit: Some(orbit),
                    mu: G * mass,
                    attracts: kind.massive(),
                    radius,
                    day,
                    tilt: random_tilt(&mut rng, 30.0),
                    collider: kind.collider(),
                    atmosphere: None,
                    pulled_by: Vec::new(),
                },
                rock: None,
            });

            // Moons, kept well inside the planet's Hill sphere.
            let hill = a * (1.0 - e) * (mass / (3.0 * star_mass)).cbrt();
            let moon_count = match kind {
                BodyKind::Rocky if mass > 0.3 * EARTH_MASS => rng.int(0, 2),
                BodyKind::GasGiant => rng.int(2, 6),
                BodyKind::IceGiant => rng.int(1, 4),
                _ => 0,
            };
            let mut moon_a = radius * rng.range(3.0, 6.0) + rings.map_or(0.0, |r| r.1);
            for m in 0..moon_count as usize {
                if moon_a > 0.35 * hill {
                    break;
                }
                let moon_radius = if kind == BodyKind::Rocky {
                    radius * rng.range(0.12, 0.3)
                } else {
                    EARTH_RADIUS * rng.range(0.08, 0.45)
                };
                let moon_mass = 3000.0 * 4.0 / 3.0 * std::f64::consts::PI * moon_radius.powi(3);
                let (e, inclination, node, periapsis, m0) = (rng.range(0.0, 0.02), rng.range(0.0, 5.0).to_radians(), rng.range(0.0, TAU), rng.range(0.0, TAU), rng.range(0.0, TAU));
                // (Tides round a close moon's orbit: none keeps a stretch that heats it past the most active moon known.)
                let e = e.min(crate::conditions::tidal_eccentricity_limit(mass, moon_radius, moon_a));
                let orbit = Orbit::new(moon_a, e, inclination, node, periapsis, m0, G * (mass + moon_mass));
                bodies.push(Body {
                    name: format!("{planet_name} {}", names::roman(m)),
                    kind: BodyKind::Moon,
                    mass: moon_mass,
                    color: *rng.pick(&MOON_COLORS),
                    rings: None,
                    link: None,
                    terrain: None,
                    rail: RailBody {
                        parent: Some(planet),
                        mu: G * moon_mass,
                        attracts: BodyKind::Moon.massive(),
                        radius: moon_radius,
                        day: orbit.period(), // tidally locked
                        orbit: Some(orbit),
                        tilt: DQuat::IDENTITY,
                        collider: BodyKind::Moon.collider(),
                        atmosphere: None,
                        pulled_by: Vec::new(),
                    },
                    rock: None,
                });
                moon_a *= rng.range(1.4, 2.0);
            }

            let closeness = (a / habitable).ln().abs();
            // (Not a scorched inner world: its starlight at most four times Earth's, or a hull cooks.)
            if kind == BodyKind::Rocky && a >= 0.5 * habitable && station_parent.is_none_or(|(_, c)| closeness < c) {
                station_parent = Some((planet, closeness));
            }
            a *= rng.range(1.5, 2.1);
        }

        let mut system = Self { index, name, class, luminosity: lum, bodies, spaceports: Vec::new(), fields: Vec::new(), small: 0..0, belts: Vec::new(), belt_seed: mix(star.seed, 0xbe175), patches: Default::default() };
        // (What the registry has curated or frozen stands in place of what the seed made: see `celestial`.)
        // (A body taken off the system's roster there takes the others' numbers with it.)
        if let Some(moved) = crate::celestial::apply(&mut system, crate::celestial::Stage::Bodies, star.seed) {
            station_parent = station_parent.and_then(|(p, c)| moved[p].map(|n| (n, c)));
        }
        if let Some((planet, _)) = station_parent {
            system.add_station(planet, &mut rng);
        }
        system.add_terrain(star.seed);
        crate::celestial::apply(&mut system, crate::celestial::Stage::Surfaces, star.seed);
        system.add_spaceports(star.seed);
        crate::belt::add_fields(&mut system, frost_line, star.seed);
        crate::small_bodies::add(&mut system, frost_line, star.seed);
        system.belts = crate::belts::belts(&system, frost_line);
        crate::celestial::apply(&mut system, crate::celestial::Stage::Rocks, star.seed);
        system.settle();
        system
    }

    /// Who pulls whom back among its bodies (see `universe_physics::settle`):
    /// after they're made or changed.
    pub fn settle(&mut self) {
        settle_bodies(&mut self.bodies);
    }

    /// Terrain for rocky planets (oceans on temperate ones) and moons.
    fn add_terrain(&mut self, seed: u64) {
        for (i, b) in self.bodies.iter_mut().enumerate() {
            let kind = match b.kind {
                BodyKind::Rocky if TERRAN_COLORS.contains(&b.color) => TerrainKind::Terran,
                BodyKind::Rocky => TerrainKind::Dry,
                BodyKind::Moon => TerrainKind::Cratered,
                _ => continue,
            };
            b.terrain = Some(Terrain::new(kind, b.rail.radius, crate::rng::mix(seed, 0x7465_7272 + i as u64)));
            // Temperate worlds have air like Earth's.
            if kind == TerrainKind::Terran {
                let g = b.rail.mu / (b.rail.radius * b.rail.radius);
                b.rail.atmosphere = Some(universe_physics::Atmosphere::earthlike(g));
            }
        }
    }

    /// One spaceport on every rocky planet, and on about half the large moons.
    /// Uses its own random stream so adding ports doesn't reshuffle the planets.
    fn add_spaceports(&mut self, seed: u64) {
        let mut rng = Rng::new(crate::rng::mix(seed, 0x706f_7274));
        for (i, b) in self.bodies.iter().enumerate() {
            let wanted = match b.kind {
                BodyKind::Rocky => true,
                BodyKind::Moon => b.rail.radius > 800_000.0 && rng.chance(0.5),
                _ => false,
            };
            if !wanted {
                continue;
            }
            // Keep away from the poles, where "down the spin axis" gets awkward.
            let lat = rng.range(-1.0, 1.0);
            let lon = rng.range(0.0, TAU);
            let r = (1.0 - lat * lat * 0.64).sqrt();
            let direction = DVec3::new(r * lon.cos(), lat * 0.8, r * lon.sin()).normalize();
            let name = format!("Port {}", names::star_name(rng.next_u64()));
            self.spaceports.push(Spaceport { name, body: i, direction });
        }
        // Level the ground around each port.
        for sp in self.spaceports.clone() {
            if let Some(t) = &mut self.bodies[sp.body].terrain {
                t.add_pad(sp.direction);
            }
        }
    }

    /// A platform station in a low circular orbit, turning once an orbit
    /// about the orbit's normal (its deck facing along it, the same side to
    /// the planet). Inserted right after the
    /// planet's moons so parents still precede children.
    fn add_station(&mut self, planet: usize, rng: &mut Rng) {
        let p = &self.bodies[planet];
        let orbit = Orbit::new(p.rail.radius * 1.6, 0.0, rng.range(0.0, 0.3), rng.range(0.0, TAU), 0.0, rng.range(0.0, TAU), p.rail.mu);
        let (day, tilt) = (orbit.period(), DQuat::from_rotation_arc(DVec3::Y, orbit.normal()));
        let station = Body {
            name: format!("{} Station", p.name),
            kind: BodyKind::Station,
            mass: 1.0e9,
            color: [1.0, 1.0, 1.0],
            rings: None,
            link: None,
            terrain: None,
            rail: RailBody {
                parent: Some(planet),
                orbit: Some(orbit),
                mu: G * 1.0e9,
                attracts: BodyKind::Station.massive(),
                radius: 600.0,
                day,
                tilt,
                collider: BodyKind::Station.collider(),
                atmosphere: None,
                pulled_by: Vec::new(),
            },
            rock: None,
        };
        let at = (planet + 1..self.bodies.len()).find(|&i| self.bodies[i].rail.parent != Some(planet)).unwrap_or(self.bodies.len());
        self.bodies.insert(at, station);
        // Fix up parent indices shifted by the insertion.
        for b in &mut self.bodies[at + 1..] {
            if let Some(parent) = &mut b.rail.parent
                && *parent >= at
            {
                *parent += 1;
            }
        }
    }

    /// Add one ring gate per link, orbiting the station's planet (or the first
    /// planet, or the star). `links` are (destination system, destination
    /// name, the way to it). Each gate faces its destination (its axis, the
    /// way through, points at that star; the twin there faces back), and
    /// orbits in a gap: no moon, station or asteroid field comes near its path.
    pub fn add_gates(&mut self, links: &[(usize, String, DVec3)], seed: u64) {
        // (No station: the planet nearest the habitable zone that isn't scorched,
        // starlight at most four times Earth's: a hull at the gate mustn't cook.)
        let habitable = crate::units::AU * self.luminosity.sqrt();
        let temperate = |b: &Body| b.rail.orbit.as_ref().map(|o| o.semi_major_axis);
        let parent = self
            .station()
            .and_then(|s| self.bodies[s].rail.parent)
            .or_else(|| {
                self.bodies
                    .iter()
                    .enumerate()
                    .filter(|(_, b)| b.rail.parent == Some(0) && b.kind.is_planet())
                    .filter_map(|(i, b)| temperate(b).filter(|&a| a >= 0.5 * habitable).map(|a| (i, (a / habitable).ln().abs())))
                    .min_by(|x, y| x.1.total_cmp(&y.1))
                    .map(|(i, _)| i)
            })
            .unwrap_or(0);
        let p_radius = self.bodies[parent].rail.radius;
        let p_mu = self.bodies[parent].rail.mu;
        // What else goes round the parent: the bands of orbit a gate keeps out of.
        let mut taken: Vec<(f64, f64)> = Vec::new();
        for (i, b) in self.bodies.iter().enumerate() {
            let Some(o) = b.rail.orbit.as_ref().filter(|_| b.rail.parent == Some(parent)) else { continue };
            let field = self.fields.iter().find(|f| f.body == i).map_or(0.0, |f| f.extent);
            let reach = b.max_radius() + field + GATE_CLEARANCE;
            taken.push((o.periapsis() - reach, o.apoapsis() + reach));
        }
        for (k, (dest, dest_name, toward)) in links.iter().enumerate() {
            let mut rng = Rng::new(crate::rng::mix(seed, 0x6761_7465 + *dest as u64));
            let mut a = if parent == 0 { AU * rng.range(0.6, 1.4) } else { p_radius * (4.0 + 1.5 * k as f64 + rng.range(0.0, 0.5)) };
            // Out past anything in the way (the bands may overlap: again till clear).
            while let Some(&(_, hi)) = taken.iter().find(|&&(lo, hi)| a > lo && a < hi) {
                a = hi + GATE_CLEARANCE;
            }
            let ring = crate::gate::gate_radius() + crate::gate::ring_tube();
            taken.push((a - ring - GATE_CLEARANCE, a + ring + GATE_CLEARANCE));
            let orbit = Orbit::new(a, 0.0, rng.range(0.0, 0.1), rng.range(0.0, TAU), 0.0, rng.range(0.0, TAU), p_mu);
            // Inertially fixed ring, facing its destination.
            let tilt = DQuat::from_rotation_arc(DVec3::Y, toward.normalize());
            self.bodies.push(Body {
                name: format!("Gate to {dest_name}"),
                kind: BodyKind::Gate,
                mass: 1.0e10,
                color: [1.0, 0.75, 0.3],
                rings: None,
                link: Some(*dest),
                terrain: None,
                rail: RailBody {
                    parent: Some(parent),
                    orbit: Some(orbit),
                    mu: G * 1.0e10,
                    attracts: BodyKind::Gate.massive(),
                    radius: crate::gate::gate_radius() + crate::gate::ring_tube(),
                    day: 1.0e15,
                    tilt,
                    collider: BodyKind::Gate.collider(),
                    atmosphere: None,
                    pulled_by: Vec::new(),
                },
                rock: None,
            });
        }
        self.settle();
    }

    /// The gate in this system leading to system `to`.
    /// (See `GATE_CLEARANCE`.)
    pub fn gate_to(&self, to: usize) -> Option<usize> {
        self.bodies.iter().position(|b| b.kind == BodyKind::Gate && b.link == Some(to))
    }

    pub fn station(&self) -> Option<usize> {
        self.bodies.iter().position(|b| b.kind == BodyKind::Station)
    }

    /// Is a point on `body` (a body-frame direction) on spaceport `port`'s pad?
    pub fn on_pad(&self, port: usize, body: usize, local_dir: DVec3) -> bool {
        self.spaceports.get(port).is_some_and(|sp| {
            sp.body == body && sp.direction.angle_between(local_dir) * self.bodies[body].rail.radius < crate::spaceport::PAD_RADIUS
        })
    }

    /// The spaceport whose pad is at a point on `body` (a body-frame direction), if any.
    pub fn port_at(&self, body: usize, local_dir: DVec3) -> Option<usize> {
        (0..self.spaceports.len()).find(|&p| self.on_pad(p, body, local_dir))
    }

    pub fn planet_count(&self) -> usize {
        self.bodies.iter().filter(|b| b.rail.parent == Some(0) && b.kind.is_planet()).count()
    }

    /// Positions of all bodies relative to the star at time `t`.
    pub fn positions(&self, t: f64, out: &mut Vec<DVec3>) {
        universe_physics::positions(&self.bodies, t, out);
    }

    /// Exact positions, velocities and accelerations of every body at `t`, for
    /// cheap extrapolation over short spans (a frame's worth of substeps).
    pub fn ephemeris(&self, t: f64) -> Ephemeris {
        Ephemeris::new(&self.bodies, t)
    }

    /// Velocity of body `i` relative to the star.
    pub fn velocity(&self, i: usize, t: f64) -> DVec3 {
        universe_physics::velocity(&self.bodies, i, t)
    }

    /// Gravitational acceleration at `p` given body positions.
    pub fn gravity(&self, p: DVec3, positions: &[DVec3]) -> DVec3 {
        universe_physics::gravity(&self.bodies, p, positions)
    }

    /// The body whose gravity dominates at `p` (largest mu / r^2).
    pub fn dominant(&self, p: DVec3, positions: &[DVec3]) -> usize {
        universe_physics::dominant(&self.bodies, p, positions)
    }
}

/// Who pulls whom back among `bodies` (see `universe_physics::settle`).
pub fn settle_bodies(bodies: &mut [Body]) {
    universe_physics::settle(&mut bodies.iter_mut().map(|b| &mut b.rail).collect::<Vec<_>>());
}

#[cfg(test)]
mod small_body_tests {
    use crate::testkit::Probe;
    use crate::system::BodyKind;

    #[test]
    fn a_station_pulls_faintly_and_its_world_still_dominates() {
        let mut p = Probe::new(42);
        let sys = p.sys();
        let pos = p.positions();
        let s = sys.station().expect("a station");
        assert!(sys.bodies[s].rail.attracts, "a station has mass: it pulls");
        // Just off its deck: its pull is real but tiny; the world it orbits is still the dominant body.
        let here = pos[s] + glam::DVec3::Y * 800.0;
        let own = universe_physics::pull(sys.bodies[s].rail.mu, pos[s] - here).length();
        assert!(own > 0.0 && own < 1e-5, "{own} m/s²");
        let d = universe_physics::dominant(&sys.bodies, here, &pos);
        assert!(sys.bodies[d].kind != BodyKind::Station && sys.bodies[d].kind != BodyKind::Gate, "{:?}", sys.bodies[d].kind);
    }
}
