use glam::DVec3;
use serde::{Deserialize, Serialize};

use crate::rng::{mix, Rng};
use crate::units::LIGHT_YEAR;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum StarClass {
    O,
    B,
    A,
    F,
    G,
    K,
    M,
}

impl StarClass {
    /// Rough main-sequence (mass, radius) in solar units.
    fn params(self) -> (f64, f64) {
        match self {
            StarClass::O => (30.0, 10.0),
            StarClass::B => (6.0, 4.0),
            StarClass::A => (2.0, 1.7),
            StarClass::F => (1.3, 1.3),
            StarClass::G => (1.0, 1.0),
            StarClass::K => (0.75, 0.8),
            StarClass::M => (0.3, 0.4),
        }
    }

    pub fn mass_suns(self) -> f64 {
        self.params().0
    }

    pub fn radius_suns(self) -> f64 {
        self.params().1
    }

    /// Luminosity in solar units (mass-luminosity relation).
    pub fn luminosity(self) -> f64 {
        self.mass_suns().powf(3.5)
    }

    pub fn color(self) -> [f32; 3] {
        match self {
            StarClass::O => [0.62, 0.72, 1.0],
            StarClass::B => [0.72, 0.8, 1.0],
            StarClass::A => [0.85, 0.9, 1.0],
            StarClass::F => [1.0, 0.98, 0.92],
            StarClass::G => [1.0, 0.93, 0.7],
            StarClass::K => [1.0, 0.78, 0.5],
            StarClass::M => [1.0, 0.55, 0.38],
        }
    }

    pub fn letter(self) -> char {
        format!("{self:?}").chars().next().unwrap()
    }

    /// A star of the Sun's neighbourhood, by the real mix of classes (main
    /// sequence, near the Sun: about three in four are red dwarfs).
    fn random(rng: &mut Rng) -> Self {
        let x = rng.f64();
        match x {
            _ if x < 0.765 => StarClass::M,
            _ if x < 0.886 => StarClass::K,
            _ if x < 0.962 => StarClass::G,
            _ if x < 0.992 => StarClass::F,
            _ if x < 0.998 => StarClass::A,
            _ if x < 0.99997 => StarClass::B,
            _ => StarClass::O,
        }
    }
}

#[derive(Clone, Debug)]
pub struct GalaxyStar {
    /// Galactic position in light years; the galactic plane is XZ.
    pub position: DVec3,
    pub class: StarClass,
    pub seed: u64,
}

#[derive(Clone)]
pub struct Galaxy {
    pub seed: u64,
    pub stars: Vec<GalaxyStar>,
}

/// The charted region: a cube this many light years a side, at the real
/// density of stars near the Sun (one per about 250 cubic light years:
/// neighbours 4-6 ly apart). One region for now; more, generated from the
/// seed as they're reached, later.
pub const REGION: f64 = 200.0;
/// Stars per cubic light year near the Sun (about 0.14 per cubic parsec):
/// the density at the region's centre, which the galaxy's shape is scaled to.
pub const STAR_DENSITY: f64 = 0.004;
/// Where the region sits in the galaxy (ly from its centre, in its plane):
/// the outer disc between the arms, about where the Sun is in ours.
pub const REGION_CENTRE: DVec3 = DVec3::new(4000.0, 0.0, 0.0);
/// The galaxy's stars come in cubes this many light years a side, each from
/// the seed and its place: as many as the density there says.
pub const SECTOR: f64 = 100.0;

/// A sector's place: which cube of `SECTOR` light years (x, y, z).
pub type Sector = [i32; 3];

/// The galaxy's shape (relative density; 1 at the disc's middle): an
/// exponential disc (scale length 2,600 ly, height 300 ly) with two
/// logarithmic arms (pitch 13°) three to five times denser than between
/// them, fading out past 8,000 ly, and a central bulge.
fn shape(p: DVec3) -> f64 {
    let r = (p.x * p.x + p.z * p.z).sqrt();
    let disc = (-r / 2600.0).exp() * (-p.y.abs() / 300.0).exp() * (1.0 - (r - 8000.0) / 1500.0).clamp(0.0, 1.0) * arms(p);
    let bulge = 4.0 * (-(r * r) / (2.0 * 700.0 * 700.0) - p.y * p.y / (2.0 * 350.0 * 350.0)).exp();
    disc + bulge
}

/// How much the arms crowd a place's stars: 0.4 between them, 2.4 on one.
pub fn arms(p: DVec3) -> f64 {
    let r = (p.x * p.x + p.z * p.z).sqrt().max(300.0);
    let pitch = 13f64.to_radians().tan();
    let along = p.z.atan2(p.x) - (r / 300.0).ln() / pitch;
    // (Two arms, half a turn apart: the angle off the nearer.)
    let off = along - (along / std::f64::consts::PI).round() * std::f64::consts::PI;
    0.4 + 2.0 * (-off * off / (2.0 * 0.28 * 0.28)).exp()
}

/// Stars per cubic light year at `p` (light years, the galaxy's frame).
pub fn density(p: DVec3) -> f64 {
    STAR_DENSITY * shape(p) / shape(REGION_CENTRE)
}

/// Stars per square light year looking straight down through the disc at
/// (x, z): the density summed through its thickness (the map's glow).
pub fn column(x: f64, z: f64) -> f64 {
    let r = (x * x + z * z).sqrt();
    let p = DVec3::new(x, 0.0, z);
    let disc = (-r / 2600.0).exp() * (1.0 - (r - 8000.0) / 1500.0).clamp(0.0, 1.0) * arms(p) * 2.0 * 300.0;
    let bulge = 4.0 * (-(r * r) / (2.0 * 700.0 * 700.0)).exp() * (std::f64::consts::TAU).sqrt() * 350.0;
    STAR_DENSITY * (disc + bulge) / shape(REGION_CENTRE)
}

fn sector_seed(seed: u64, s: Sector) -> u64 {
    mix(mix(mix(seed ^ 0x7365_6374, s[0] as u32 as u64), s[1] as u32 as u64), s[2] as u32 as u64)
}

/// The sector a place is in.
pub fn sector_of(p: DVec3) -> Sector {
    [(p.x / SECTOR).floor() as i32, (p.y / SECTOR).floor() as i32, (p.z / SECTOR).floor() as i32]
}

/// How many stars a sector holds: its volume at the density at its middle.
pub fn sector_count(seed: u64, s: Sector) -> usize {
    sector_draw(&mut Rng::new(sector_seed(seed, s)), s)
}

fn sector_draw(rng: &mut Rng, s: Sector) -> usize {
    let middle = (DVec3::new(s[0] as f64, s[1] as f64, s[2] as f64) + 0.5) * SECTOR;
    let expected = density(middle) * SECTOR.powi(3);
    (expected.floor() + if rng.f64() < expected.fract() { 1.0 } else { 0.0 }) as usize
}

/// A sector's first `limit` stars (all of them, at most its count), from the
/// seed: spread evenly through it, so the first few are a fair sample.
pub fn sector_stars(seed: u64, s: Sector, limit: usize) -> Vec<GalaxyStar> {
    let base = sector_seed(seed, s);
    let mut rng = Rng::new(base);
    let n = sector_draw(&mut rng, s).min(limit);
    let corner = DVec3::new(s[0] as f64, s[1] as f64, s[2] as f64) * SECTOR;
    (0..n)
        .map(|i| {
            let position = corner + DVec3::new(rng.f64(), rng.f64(), rng.f64()) * SECTOR;
            GalaxyStar { position, class: StarClass::random(&mut rng), seed: mix(base, i as u64) }
        })
        .collect()
}

/// The sectors the charted region is made of.
pub fn region_sectors() -> Vec<Sector> {
    let (lo, hi) = (sector_of(REGION_CENTRE - REGION / 2.0), sector_of(REGION_CENTRE + REGION / 2.0 - 1e-6));
    let mut out = Vec::new();
    for x in lo[0]..=hi[0] {
        for y in lo[1]..=hi[1] {
            for z in lo[2]..=hi[2] {
                out.push([x, y, z]);
            }
        }
    }
    out
}

impl Galaxy {
    /// The charted region's stars: its sectors', from the seed (the same stars
    /// the map shows there).
    pub fn generate(seed: u64) -> Self {
        let stars = region_sectors().into_iter().flat_map(|s| sector_stars(seed, s, usize::MAX)).collect();
        Self { seed, stars }
    }

    /// A small uniform region of `count` stars at the local density (tests).
    pub fn generate_n(seed: u64, count: usize) -> Self {
        let mut rng = Rng::new(seed);
        let side = (count as f64 / STAR_DENSITY).cbrt();
        let stars = (0..count)
            .map(|i| {
                let position = REGION_CENTRE + DVec3::new(rng.f64() - 0.5, rng.f64() - 0.5, rng.f64() - 0.5) * side;
                GalaxyStar { position, class: StarClass::random(&mut rng), seed: mix(seed, i as u64) }
            })
            .collect();
        Self { seed, stars }
    }

    /// Offset in meters from star `from` to star `to`.
    pub fn offset(&self, from: usize, to: usize) -> DVec3 {
        (self.stars[to].position - self.stars[from].position) * LIGHT_YEAR
    }

    /// Indices of the `k` stars nearest to star `index` (excluding itself), nearest first.
    pub fn nearest(&self, index: usize, k: usize) -> Vec<usize> {
        let origin = self.stars[index].position;
        let mut all: Vec<(f64, usize)> = self
            .stars
            .iter()
            .enumerate()
            .filter(|&(i, _)| i != index)
            .map(|(i, s)| (s.position.distance_squared(origin), i))
            .collect();
        let k = k.min(all.len());
        all.select_nth_unstable_by(k.saturating_sub(1), |a, b| a.0.total_cmp(&b.0));
        all.truncate(k);
        all.sort_by(|a, b| a.0.total_cmp(&b.0));
        all.into_iter().map(|(_, i)| i).collect()
    }
}
