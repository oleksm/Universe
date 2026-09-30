use std::f64::consts::TAU;

use glam::{DQuat, DVec3};

use crate::galaxy::{GalaxyStar, StarClass};
use crate::names;
use crate::orbit::Orbit;
use crate::rng::Rng;
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
        }
    }

    /// Can a ship touch down here (slowly) without being destroyed?
    /// (Stations are docked with through their slot instead; see `docking`.)
    pub fn landable(self) -> bool {
        matches!(self, BodyKind::Rocky | BodyKind::Moon)
    }

    /// Does it pull on ships? (Stations and gates are too light to matter.)
    pub fn massive(self) -> bool {
        !matches!(self, BodyKind::Station | BodyKind::Gate)
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
    pub radius: f64,
    /// Gravitational parameter G * mass.
    pub mu: f64,
    pub color: [f32; 3],
    pub parent: Option<usize>,
    pub orbit: Option<Orbit>,
    /// Sidereal rotation period, seconds.
    pub day: f64,
    /// Orientation of the spin axis (the body's local +Y).
    pub tilt: DQuat,
    /// Ring inner/outer radius, if any.
    pub rings: Option<(f64, f64)>,
    /// For gates: the galaxy index of the star system it leads to.
    pub link: Option<usize>,
    /// Surface heights, for rocky planets and moons.
    pub terrain: Option<crate::terrain::Terrain>,
}

impl Body {
    pub fn rotation(&self, t: f64) -> DQuat {
        self.tilt * DQuat::from_rotation_y((t / self.day).fract() * TAU)
    }

    pub fn angular_velocity(&self) -> DVec3 {
        self.tilt * DVec3::Y * (TAU / self.day)
    }

    /// Distance from the center to the surface in a body-frame direction (m).
    pub fn surface_radius(&self, local_dir: DVec3) -> f64 {
        self.radius + self.terrain.as_ref().map_or(0.0, |t| t.surface(local_dir))
    }

    /// Surface radius under a world-space point at time `t` (`center` = body position).
    pub fn surface_radius_at(&self, center: DVec3, p: DVec3, t: f64) -> f64 {
        match &self.terrain {
            Some(_) => self.surface_radius(self.rotation(t).inverse() * (p - center).normalize()),
            None => self.radius,
        }
    }

    /// Nothing on the surface reaches higher than this (m from the center).
    pub fn max_radius(&self) -> f64 {
        self.radius + self.terrain.as_ref().map_or(0.0, |t| t.max_height())
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

/// Body states at one moment, extrapolated to nearby times with
/// p + v·τ + ½·a·τ². Over a few seconds the error is millimetres
/// (it grows with the cube of τ), far cheaper than re-solving every orbit.
pub struct Ephemeris {
    t0: f64,
    pos: Vec<DVec3>,
    vel: Vec<DVec3>,
    acc: Vec<DVec3>,
}

impl Ephemeris {
    /// Longest span it's used for (s).
    pub const SPAN: f64 = 5.0;

    pub fn positions(&self, t: f64, out: &mut Vec<DVec3>) {
        let tau = t - self.t0;
        out.clear();
        out.extend(self.pos.iter().zip(&self.vel).zip(&self.acc).map(|((p, v), a)| *p + *v * tau + *a * (0.5 * tau * tau)));
    }
}

pub struct StarSystem {
    /// Index of this system's star in the galaxy.
    pub index: usize,
    pub name: String,
    pub class: StarClass,
    /// Parents always come before their children; body 0 is the star.
    pub bodies: Vec<Body>,
    pub spaceports: Vec<Spaceport>,
}

const ROCKY_COLORS: [[f32; 3]; 4] = [[0.8, 0.5, 0.3], [0.65, 0.65, 0.65], [0.85, 0.75, 0.5], [0.75, 0.4, 0.35]];
const TERRAN_COLORS: [[f32; 3]; 2] = [[0.3, 0.85, 0.45], [0.3, 0.6, 1.0]];
const GAS_COLORS: [[f32; 3]; 3] = [[0.9, 0.7, 0.45], [0.85, 0.6, 0.4], [0.8, 0.75, 0.6]];
const ICE_COLORS: [[f32; 3]; 2] = [[0.5, 0.85, 1.0], [0.4, 0.55, 1.0]];
const MOON_COLORS: [[f32; 3]; 3] = [[0.7, 0.7, 0.7], [0.6, 0.58, 0.55], [0.75, 0.7, 0.6]];

fn random_tilt(rng: &mut Rng, max_degrees: f64) -> DQuat {
    DQuat::from_rotation_y(rng.range(0.0, TAU)) * DQuat::from_rotation_x(rng.range(0.0, max_degrees).to_radians())
}

impl StarSystem {
    pub fn generate(index: usize, star: &GalaxyStar) -> Self {
        let mut rng = Rng::new(star.seed);
        let name = names::star_name(star.seed);
        let class = star.class;
        let lum = class.luminosity();

        let star_mass = class.mass_suns() * SUN_MASS * rng.range(0.85, 1.15);
        let star_radius = class.radius_suns() * SUN_RADIUS * rng.range(0.85, 1.15);
        let star_mu = G * star_mass;
        let mut bodies = vec![Body {
            name: name.clone(),
            kind: BodyKind::Star,
            mass: star_mass,
            radius: star_radius,
            mu: star_mu,
            color: class.color(),
            parent: None,
            orbit: None,
            day: rng.range(20.0, 35.0) * DAY,
            tilt: random_tilt(&mut rng, 7.0),
            rings: None,
            link: None,
                terrain: None,
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
                star_mu,
            );
            let planet = bodies.len();
            bodies.push(Body {
                name: planet_name.clone(),
                kind,
                mass,
                radius,
                mu: G * mass,
                color,
                parent: Some(0),
                orbit: Some(orbit),
                day,
                tilt: random_tilt(&mut rng, 30.0),
                rings,
                link: None,
                terrain: None,
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
                let orbit = Orbit::new(
                    moon_a,
                    rng.range(0.0, 0.02),
                    rng.range(0.0, 5.0).to_radians(),
                    rng.range(0.0, TAU),
                    rng.range(0.0, TAU),
                    rng.range(0.0, TAU),
                    G * mass,
                );
                bodies.push(Body {
                    name: format!("{planet_name} {}", names::roman(m)),
                    kind: BodyKind::Moon,
                    mass: moon_mass,
                    radius: moon_radius,
                    mu: G * moon_mass,
                    color: *rng.pick(&MOON_COLORS),
                    parent: Some(planet),
                    day: orbit.period(), // tidally locked
                    orbit: Some(orbit),
                    tilt: DQuat::IDENTITY,
                    rings: None,
                    link: None,
                terrain: None,
                });
                moon_a *= rng.range(1.4, 2.0);
            }

            let closeness = (a / habitable).ln().abs();
            if kind == BodyKind::Rocky && station_parent.is_none_or(|(_, c)| closeness < c) {
                station_parent = Some((planet, closeness));
            }
            a *= rng.range(1.5, 2.1);
        }

        let mut system = Self { index, name, class, bodies, spaceports: Vec::new() };
        if let Some((planet, _)) = station_parent {
            system.add_station(planet, &mut rng);
        }
        system.add_terrain(star.seed);
        system.add_spaceports(star.seed);
        system
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
            b.terrain = Some(Terrain::new(kind, b.radius, crate::rng::mix(seed, 0x7465_7272 + i as u64)));
        }
    }

    /// One spaceport on every rocky planet, and on about half the large moons.
    /// Uses its own random stream so adding ports doesn't reshuffle the planets.
    fn add_spaceports(&mut self, seed: u64) {
        let mut rng = Rng::new(crate::rng::mix(seed, 0x706f_7274));
        for (i, b) in self.bodies.iter().enumerate() {
            let wanted = match b.kind {
                BodyKind::Rocky => true,
                BodyKind::Moon => b.radius > 800_000.0 && rng.chance(0.5),
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

    /// A Coriolis station in a low circular orbit. Inserted right after the
    /// planet's moons so parents still precede children.
    fn add_station(&mut self, planet: usize, rng: &mut Rng) {
        let p = &self.bodies[planet];
        let orbit = Orbit::new(p.radius * 1.6, 0.0, rng.range(0.0, 0.3), rng.range(0.0, TAU), 0.0, rng.range(0.0, TAU), p.mu);
        let station = Body {
            name: format!("{} Station", p.name),
            kind: BodyKind::Station,
            mass: 1.0e9,
            radius: 600.0,
            mu: G * 1.0e9,
            color: [1.0, 1.0, 1.0],
            parent: Some(planet),
            orbit: Some(orbit),
            day: 60.0,
            tilt: DQuat::IDENTITY,
            rings: None,
            link: None,
                terrain: None,
        };
        let at = (planet + 1..self.bodies.len()).find(|&i| self.bodies[i].parent != Some(planet)).unwrap_or(self.bodies.len());
        self.bodies.insert(at, station);
        // Fix up parent indices shifted by the insertion.
        for b in &mut self.bodies[at + 1..] {
            if let Some(parent) = &mut b.parent
                && *parent >= at
            {
                *parent += 1;
            }
        }
    }

    /// Add one ring gate per link, orbiting the station's planet (or the first
    /// planet, or the star). `links` are (destination system, destination name).
    pub fn add_gates(&mut self, links: &[(usize, String)], seed: u64) {
        let parent = self
            .station()
            .and_then(|s| self.bodies[s].parent)
            .or_else(|| self.bodies.iter().position(|b| b.parent == Some(0)))
            .unwrap_or(0);
        let p_radius = self.bodies[parent].radius;
        let p_mu = self.bodies[parent].mu;
        for (k, (dest, dest_name)) in links.iter().enumerate() {
            let mut rng = Rng::new(crate::rng::mix(seed, 0x6761_7465 + *dest as u64));
            let a = if parent == 0 { AU * rng.range(0.6, 1.4) } else { p_radius * (4.0 + 1.5 * k as f64 + rng.range(0.0, 0.5)) };
            let orbit = Orbit::new(a, 0.0, rng.range(0.0, 0.1), rng.range(0.0, TAU), 0.0, rng.range(0.0, TAU), p_mu);
            // Inertially fixed ring, its axis lying in the orbital plane.
            let tilt = DQuat::from_rotation_y(rng.range(0.0, TAU)) * DQuat::from_rotation_x(std::f64::consts::FRAC_PI_2);
            self.bodies.push(Body {
                name: format!("Gate to {dest_name}"),
                kind: BodyKind::Gate,
                mass: 1.0e10,
                radius: crate::gate::GATE_RADIUS + crate::gate::RING_TUBE,
                mu: G * 1.0e10,
                color: [1.0, 0.75, 0.3],
                parent: Some(parent),
                orbit: Some(orbit),
                day: 1.0e15,
                tilt,
                rings: None,
                link: Some(*dest),
                terrain: None,
            });
        }
    }

    /// The gate in this system leading to system `to`.
    pub fn gate_to(&self, to: usize) -> Option<usize> {
        self.bodies.iter().position(|b| b.kind == BodyKind::Gate && b.link == Some(to))
    }

    pub fn station(&self) -> Option<usize> {
        self.bodies.iter().position(|b| b.kind == BodyKind::Station)
    }

    pub fn planet_count(&self) -> usize {
        self.bodies.iter().filter(|b| b.parent == Some(0)).count()
    }

    /// Positions of all bodies relative to the star at time `t`.
    pub fn positions(&self, t: f64, out: &mut Vec<DVec3>) {
        out.clear();
        for b in &self.bodies {
            let p = match (b.parent, &b.orbit) {
                (Some(parent), Some(orbit)) => out[parent] + orbit.position(t),
                _ => DVec3::ZERO,
            };
            out.push(p);
        }
    }

    /// Exact positions, velocities and accelerations of every body at `t`, for
    /// cheap extrapolation over short spans (a frame's worth of substeps).
    pub fn ephemeris(&self, t: f64) -> Ephemeris {
        let n = self.bodies.len();
        let (mut pos, mut vel, mut acc) = (Vec::with_capacity(n), Vec::with_capacity(n), Vec::with_capacity(n));
        for b in &self.bodies {
            let (p, v, a) = match (b.parent, &b.orbit) {
                (Some(parent), Some(orbit)) => {
                    let (rp, rv) = orbit.state(t);
                    // On Keplerian rails: accelerated only by the parent's gravity.
                    let ra = -rp * (orbit.mu / rp.length().powi(3));
                    (pos[parent] + rp, vel[parent] + rv, acc[parent] + ra)
                }
                _ => (DVec3::ZERO, DVec3::ZERO, DVec3::ZERO),
            };
            pos.push(p);
            vel.push(v);
            acc.push(a);
        }
        Ephemeris { t0: t, pos, vel, acc }
    }

    /// Velocity of body `i` relative to the star.
    pub fn velocity(&self, i: usize, t: f64) -> DVec3 {
        let b = &self.bodies[i];
        match (b.parent, &b.orbit) {
            (Some(parent), Some(orbit)) => self.velocity(parent, t) + orbit.state(t).1,
            _ => DVec3::ZERO,
        }
    }

    /// Gravitational acceleration at `p` given body positions.
    pub fn gravity(&self, p: DVec3, positions: &[DVec3]) -> DVec3 {
        let mut acc = DVec3::ZERO;
        for (b, &bp) in self.bodies.iter().zip(positions) {
            if b.kind.massive() {
                let d = bp - p;
                let r2 = d.length_squared().max(1.0);
                acc += d * (b.mu / (r2 * r2.sqrt()));
            }
        }
        acc
    }

    /// The body whose gravity dominates at `p` (largest mu / r^2).
    pub fn dominant(&self, p: DVec3, positions: &[DVec3]) -> usize {
        let mut best = (0, 0.0);
        for (i, (b, &bp)) in self.bodies.iter().zip(positions).enumerate() {
            if b.kind.massive() {
                let g = b.mu / bp.distance_squared(p).max(1.0);
                if g > best.1 {
                    best = (i, g);
                }
            }
        }
        best.0
    }
}
