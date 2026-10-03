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
/// Stars per cubic light year near the Sun (about 0.14 per cubic parsec).
pub const STAR_DENSITY: f64 = 0.004;
/// Where the region sits in the galaxy (ly from its centre, in its plane):
/// the outer disc, about where the Sun is in ours.
pub const REGION_CENTRE: DVec3 = DVec3::new(4000.0, 0.0, 0.0);

impl Galaxy {
    /// The charted region's stars, from the seed: uniformly through the cube
    /// at the real density, by the real mix of classes.
    pub fn generate(seed: u64) -> Self {
        Self::generate_n(seed, (REGION.powi(3) * STAR_DENSITY).round() as usize)
    }

    /// `generate` with `count` stars (tests: a small region).
    pub fn generate_n(seed: u64, count: usize) -> Self {
        let mut rng = Rng::new(seed);
        let side = REGION * (count as f64 / (REGION.powi(3) * STAR_DENSITY)).cbrt();
        let stars = (0..count)
            .map(|i| {
                let position = REGION_CENTRE + DVec3::new(rng.f64() - 0.5, rng.f64() - 0.5, rng.f64() - 0.5) * side;
                GalaxyStar { position, class: StarClass::random(&mut rng), seed: mix(seed, i as u64) }
            })
            .collect();
        Self { seed, stars }
    }

    /// The galaxy beyond the region, for the map's glow only (never visited,
    /// never stored as stars): a two-armed spiral with a central bulge, `count`
    /// sample points in light years.
    pub fn backdrop(seed: u64, count: usize) -> Vec<DVec3> {
        let mut rng = Rng::new(seed ^ 0x6261_636b);
        let pitch = 13f64.to_radians().tan();
        (0..count)
            .map(|_| {
                let kind = rng.f64();
                if kind < 0.12 {
                    DVec3::new(rng.normal() * 700.0, rng.normal() * 350.0, rng.normal() * 700.0)
                } else {
                    let r = (-2600.0 * (1.0 - rng.f64()).ln()).clamp(300.0, 9000.0);
                    let angle = if kind < 0.35 {
                        rng.range(0.0, std::f64::consts::TAU)
                    } else {
                        let arm = (rng.next_u64() % 2) as f64 * std::f64::consts::PI;
                        arm + (r / 300.0).ln() / pitch + rng.normal() * 0.28
                    };
                    let jitter = DVec3::new(rng.normal(), 0.0, rng.normal()) * 120.0;
                    DVec3::new(r * angle.cos(), rng.normal() * 90.0, r * angle.sin()) + jitter
                }
            })
            .collect()
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
