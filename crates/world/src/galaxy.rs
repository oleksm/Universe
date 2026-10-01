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

    fn random(rng: &mut Rng) -> Self {
        // Real stellar populations are dominated by M dwarfs; skewed toward
        // brighter classes here so the sky is more varied.
        let x = rng.f64();
        match x {
            _ if x < 0.40 => StarClass::M,
            _ if x < 0.65 => StarClass::K,
            _ if x < 0.82 => StarClass::G,
            _ if x < 0.92 => StarClass::F,
            _ if x < 0.975 => StarClass::A,
            _ if x < 0.997 => StarClass::B,
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

pub const GALAXY_STARS: usize = 40_000;

impl Galaxy {
    /// A two-armed spiral with a central bulge.
    pub fn generate(seed: u64, count: usize) -> Self {
        let mut rng = Rng::new(seed);
        let pitch = 13f64.to_radians().tan();
        let stars = (0..count)
            .map(|i| {
                let kind = rng.f64();
                let position = if kind < 0.12 {
                    // Bulge.
                    DVec3::new(rng.normal() * 700.0, rng.normal() * 350.0, rng.normal() * 700.0)
                } else {
                    let r = (-2600.0 * (1.0 - rng.f64()).ln()).clamp(300.0, 9000.0);
                    let angle = if kind < 0.35 {
                        rng.range(0.0, std::f64::consts::TAU) // inter-arm disc
                    } else {
                        let arm = (rng.next_u64() % 2) as f64 * std::f64::consts::PI;
                        arm + (r / 300.0).ln() / pitch + rng.normal() * 0.28
                    };
                    let jitter = DVec3::new(rng.normal(), 0.0, rng.normal()) * 120.0;
                    DVec3::new(r * angle.cos(), rng.normal() * 90.0, r * angle.sin()) + jitter
                };
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
