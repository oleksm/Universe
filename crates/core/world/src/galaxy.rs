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
    /// sequence, near the Sun: about three in four are red dwarfs), as the
    /// registry has it.
    fn random(rng: &mut Rng) -> Self {
        let x = rng.f64();
        let cuts = charted().class_cuts;
        [StarClass::M, StarClass::K, StarClass::G, StarClass::F, StarClass::A, StarClass::B].into_iter().zip(cuts).find(|&(_, cut)| x < cut).map_or(StarClass::O, |(class, _)| class)
    }
}

#[derive(Clone, Debug)]
pub struct GalaxyStar {
    /// Galactic position in light years; the galactic plane is XZ.
    pub position: DVec3,
    pub class: StarClass,
    pub seed: u64,
}

impl GalaxyStar {
    /// Its mass (suns): its class's, give or take 15% (seeded: its system's first draw).
    pub fn mass_suns(&self) -> f64 {
        self.class.mass_suns() * Rng::new(self.seed).range(0.85, 1.15)
    }

    /// Its luminosity (suns): by its own mass, L ∝ M^3.5 (the main sequence's).
    pub fn luminosity(&self) -> f64 {
        self.mass_suns().powf(3.5)
    }
}

#[derive(Clone)]
pub struct Galaxy {
    pub seed: u64,
    pub stars: Vec<GalaxyStar>,
}

/// The charted region and how its stars are made, from the registry's
/// `seeding.galaxy` (in light years, as the galaxy is laid out): a cube
/// `region` a side, at the real density of stars near the Sun (one per about
/// 250 cubic light years: neighbours 4-6 ly apart), made in cubes `sector` a
/// side, by the real mix of star classes. One region for now; more, generated
/// from the seed as they're reached, later.
pub struct Charted {
    /// The charted region's side (ly).
    pub region: f64,
    /// Stars per cubic light year near the Sun: the density at the region's
    /// centre, which the galaxy's shape is scaled to.
    pub star_density: f64,
    /// The galaxy's stars come in cubes this many light years a side, each
    /// from the seed and its place: as many as the density there says.
    pub sector: f64,
    /// Where each class ends in a draw from 0 to 1: M, K, G, F, A, B (the
    /// rest O).
    class_cuts: [f64; 6],
}

/// The charted region, as the registry has it.
pub fn charted() -> &'static Charted {
    static CHARTED: std::sync::OnceLock<Charted> = std::sync::OnceLock::new();
    CHARTED.get_or_init(|| {
        let g = crate::registry::registry().galaxy().expect("the registry has seeding.galaxy");
        let ly = crate::units::LIGHT_YEAR;
        let m = &g.class_mix;
        let mut cut = 0.0;
        let class_cuts = [m.M, m.K, m.G, m.F, m.A, m.B].map(|share| {
            let share = share.unwrap_or(0.0);
            cut += share;
            cut
        });
        let need = |v: Option<f64>, what: &str| v.unwrap_or_else(|| panic!("seeding.galaxy has no {what}"));
        Charted { region: need(g.region, "region") / ly, star_density: need(g.star_density, "star density") * ly.powi(3), sector: need(g.sector, "sector") / ly, class_cuts }
    })
}

/// Where the region sits in the galaxy (ly from its centre, in its plane): on
/// an arm 4,000 ly out (`shape_stars`' arms run at angle ln(r/300)/tan 13°),
/// the outer disc, about where the Sun is in ours.
/// (Its corners on the sector grid: a whole number of sectors.)
pub const REGION_CENTRE: DVec3 = DVec3::new(-900.0, 0.0, 3900.0);

/// A sector's place: which cube of [`Charted::sector`] light years (x, y, z).
pub type Sector = [i32; 3];

/// The galaxy's shape, sampled: 40,000 stars of a two-armed spiral with a
/// central bulge (the same every world). Gathered on a grid and softened it's
/// the galaxy's light on the map, and the density its stars are made by.
pub fn shape_stars() -> &'static [GalaxyStar] {
    static STARS: std::sync::OnceLock<Vec<GalaxyStar>> = std::sync::OnceLock::new();
    STARS.get_or_init(|| {
        let seed = 1984;
        let mut rng = Rng::new(seed);
        let pitch = 13f64.to_radians().tan();
        (0..40_000)
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
            .collect()
    })
}

/// Cells a side of the shape grid, and how far it reaches (ly, each way from the centre).
pub const SHAPE_CELLS: usize = 256;
pub const SHAPE_REACH: f64 = 10_500.0;

/// The shape's stars gathered on the grid over the plane (each shared among
/// the four corners round it), softened a little (`fine`) and a lot (`soft`):
/// per corner, `(SHAPE_CELLS + 1)²`.
pub struct ShapeGrid {
    pub fine: Vec<f32>,
    pub soft: Vec<f32>,
}

pub fn shape_grid() -> &'static ShapeGrid {
    static GRID: std::sync::OnceLock<ShapeGrid> = std::sync::OnceLock::new();
    GRID.get_or_init(|| {
        let n = SHAPE_CELLS + 1;
        let cell = 2.0 * SHAPE_REACH / SHAPE_CELLS as f64;
        let mut light = vec![0.0f32; n * n];
        for s in shape_stars() {
            let (x, y) = ((s.position.x + SHAPE_REACH) / cell, (s.position.z + SHAPE_REACH) / cell);
            if x < 0.0 || y < 0.0 || x >= (n - 1) as f64 || y >= (n - 1) as f64 {
                continue;
            }
            let (i, j) = (x as usize, y as usize);
            let (fx, fy) = ((x - i as f64) as f32, (y - j as f64) as f32);
            light[j * n + i] += (1.0 - fx) * (1.0 - fy);
            light[j * n + i + 1] += fx * (1.0 - fy);
            light[(j + 1) * n + i] += (1.0 - fx) * fy;
            light[(j + 1) * n + i + 1] += fx * fy;
        }
        let blur = |src: &[f32], r: i32| -> Vec<f32> {
            let w: Vec<f32> = (-r..=r).map(|k| (-(k * k) as f32 / (0.5 * (r * r) as f32 + 0.5)).exp()).collect();
            let total: f32 = w.iter().sum();
            let mut a = vec![0.0f32; n * n];
            let mut b = vec![0.0f32; n * n];
            for y in 0..n {
                for x in 0..n {
                    a[y * n + x] = (-r..=r).map(|k| src[y * n + (x as i32 + k).clamp(0, n as i32 - 1) as usize] * w[(k + r) as usize]).sum::<f32>() / total;
                }
            }
            for y in 0..n {
                for x in 0..n {
                    b[y * n + x] = (-r..=r).map(|k| a[(y as i32 + k).clamp(0, n as i32 - 1) as usize * n + x] * w[(k + r) as usize]).sum::<f32>() / total;
                }
            }
            b
        };
        ShapeGrid { fine: blur(&light, 1), soft: blur(&light, 4) }
    })
}

/// The shape's light in the plane at (x, z): the grid, between its corners.
fn plane(x: f64, z: f64) -> f64 {
    let g = shape_grid();
    let n = SHAPE_CELLS + 1;
    let cell = 2.0 * SHAPE_REACH / SHAPE_CELLS as f64;
    let (u, v) = ((x + SHAPE_REACH) / cell, (z + SHAPE_REACH) / cell);
    if u < 0.0 || v < 0.0 || u >= (n - 1) as f64 || v >= (n - 1) as f64 {
        return 0.0;
    }
    let (i, j) = (u as usize, v as usize);
    let (fx, fz) = (u - i as f64, v - j as f64);
    let at = |i: usize, j: usize| 0.6 * g.fine[j * n + i] as f64 + 0.4 * g.soft[j * n + i] as f64;
    let a = at(i, j) + (at(i + 1, j) - at(i, j)) * fx;
    let b = at(i, j + 1) + (at(i + 1, j + 1) - at(i, j + 1)) * fx;
    a + (b - a) * fz
}

/// The galaxy's shape (relative density): the plane's light, thinning above
/// and below it (about 150 ly).
fn shape(p: DVec3) -> f64 {
    plane(p.x, p.z) * (-p.y * p.y / (2.0 * 150.0 * 150.0)).exp()
}

/// Stars per cubic light year at `p` (light years, the galaxy's frame): the
/// shape, scaled so the charted region averages the real density.
pub fn density(p: DVec3) -> f64 {
    static REGION_SHAPE: std::sync::OnceLock<f64> = std::sync::OnceLock::new();
    let mean = *REGION_SHAPE.get_or_init(|| {
        let s = region_sectors();
        s.iter().map(|c| shape((DVec3::new(c[0] as f64, c[1] as f64, c[2] as f64) + 0.5) * charted().sector)).sum::<f64>() / s.len() as f64
    });
    charted().star_density * shape(p) / mean
}

fn sector_seed(seed: u64, s: Sector) -> u64 {
    mix(mix(mix(seed ^ 0x7365_6374, s[0] as u32 as u64), s[1] as u32 as u64), s[2] as u32 as u64)
}

/// The sector a place is in.
pub fn sector_of(p: DVec3) -> Sector {
    [(p.x / charted().sector).floor() as i32, (p.y / charted().sector).floor() as i32, (p.z / charted().sector).floor() as i32]
}

/// How many stars a sector holds: its volume at the density at its middle.
pub fn sector_count(seed: u64, s: Sector) -> usize {
    sector_draw(&mut Rng::new(sector_seed(seed, s)), s)
}

fn sector_draw(rng: &mut Rng, s: Sector) -> usize {
    let middle = (DVec3::new(s[0] as f64, s[1] as f64, s[2] as f64) + 0.5) * charted().sector;
    let expected = density(middle) * charted().sector.powi(3);
    (expected.floor() + if rng.f64() < expected.fract() { 1.0 } else { 0.0 }) as usize
}

/// A sector's first `limit` stars (all of them, at most its count), from the
/// seed: spread evenly through it, so the first few are a fair sample.
pub fn sector_stars(seed: u64, s: Sector, limit: usize) -> Vec<GalaxyStar> {
    let base = sector_seed(seed, s);
    let mut rng = Rng::new(base);
    let n = sector_draw(&mut rng, s).min(limit);
    let corner = DVec3::new(s[0] as f64, s[1] as f64, s[2] as f64) * charted().sector;
    (0..n)
        .map(|i| {
            let position = corner + DVec3::new(rng.f64(), rng.f64(), rng.f64()) * charted().sector;
            GalaxyStar { position, class: StarClass::random(&mut rng), seed: mix(base, i as u64) }
        })
        .collect()
}

/// The sectors the charted region is made of.
pub fn region_sectors() -> Vec<Sector> {
    let (lo, hi) = (sector_of(REGION_CENTRE - charted().region / 2.0), sector_of(REGION_CENTRE + charted().region / 2.0 - 1e-6));
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
        let side = (count as f64 / charted().star_density).cbrt();
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
