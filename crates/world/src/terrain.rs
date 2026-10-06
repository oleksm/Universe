//! Sketchy but physical terrain: a height function over the surface of rocky
//! planets and moons. The same function drives collisions and drawing, so what
//! you see is what you land on.
//!
//! Heights are relative to the body's base radius. Oceans (on Earth-like
//! worlds) are a flat surface at height 0 over anything below it.

use glam::DVec3;

use crate::rng::Rng;

/// Flat around a spaceport out to this ground distance, blending back to
/// natural terrain by `PAD_FLAT_OUTER` (m).
const PAD_FLAT_INNER: f64 = 4_000.0;
const PAD_FLAT_OUTER: f64 = 40_000.0;

/// The small-scale relief (see `Terrain::relief`): its octaves, the first's
/// frequency (a feature about 1/RELIEF_FREQ radii across: 25 km on a big
/// world), how much finer each next, the slope of each, and how much
/// rougher the roughest ground is than that.
const RELIEF_OCTAVES: u32 = 8;
const RELIEF_FREQ: f64 = 300.0;
const RELIEF_LACUNARITY: f64 = 2.1;
const RELIEF_SLOPE: f64 = 0.035;
const RELIEF_ROUGHEST: f64 = 1.6;
/// The plain round a spaceport: no small relief within this (m), all of it by that.
const RELIEF_FLAT_INNER: f64 = 20_000.0;
const RELIEF_FLAT_OUTER: f64 = 60_000.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TerrainKind {
    /// Oceans, continents, mountains, a few craters.
    Terran,
    /// Dry rock: hills, mountains, some craters.
    Dry,
    /// Airless moon: heavily cratered.
    Cratered,
}

/// What the ground is like at a point, for drawing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ground {
    Ocean,
    Lowland,
    Highland,
    Peak,
    Crater,
}

#[derive(Clone, Debug)]
struct Crater {
    dir: DVec3,
    /// Radius as a chord length on the unit sphere.
    radius: f64,
    depth: f64,
}

#[derive(Clone, Debug)]
pub struct Terrain {
    pub kind: TerrainKind,
    /// Typical height of the tallest features (m).
    pub amplitude: f64,
    seed: u64,
    body_radius: f64,
    craters: Vec<Crater>,
    /// Spaceport directions (body frame), where the ground is flattened.
    pads: Vec<DVec3>,
    /// The ground as the planet simulation grew it, where the body has a bake (`worlds`):
    /// these heights in place of the seed's.
    baked: Option<std::sync::Arc<crate::worlds::Heights>>,
}

impl Terrain {
    pub fn new(kind: TerrainKind, body_radius: f64, seed: u64) -> Self {
        let mut rng = Rng::new(seed);
        let amplitude = (body_radius * 0.0007).clamp(1500.0, 7000.0);
        let count = match kind {
            TerrainKind::Terran => 4,
            TerrainKind::Dry => 25,
            TerrainKind::Cratered => 70,
        };
        let craters = (0..count)
            .map(|_| {
                let radius = 10f64.powf(rng.range(-2.3, -0.9));
                Crater { dir: rng.unit_vector(), radius, depth: (radius * body_radius * 0.1).min(amplitude * 0.8) }
            })
            .collect();
        Self { kind, amplitude, seed, body_radius, craters, pads: Vec::new(), baked: None }
    }

    /// The ground is the bake's `heights` from now (its craters and noise gone).
    pub fn bake(&mut self, heights: std::sync::Arc<crate::worlds::Heights>) {
        self.amplitude = heights.max / 1.3;
        self.craters.clear();
        self.baked = Some(heights);
    }

    /// The world's true colour, if its bake has one (read now; see `worlds::Heights::colour`).
    pub fn colour(&self) -> Option<crate::worlds::Equirect> {
        self.baked.as_ref()?.colour()
    }

    /// The air's tables from the world's bake (see `worlds::Heights::air_luts`).
    pub fn bake_air_luts(&self) -> Option<crate::worlds::AirLuts> {
        self.baked.as_ref()?.air_luts()
    }

    /// The world's air from its bake, packed for the engine (see `worlds::Heights::air`).
    pub fn bake_air(&self) -> Option<[f32; 16]> {
        self.baked.as_ref()?.air()
    }

    /// Image `name` of the world's bake, as RGBA8 (see `worlds::Heights::image`).
    pub fn bake_image(&self, name: &str) -> Option<(usize, usize, Vec<u8>)> {
        self.baked.as_ref()?.image(name)
    }

    /// Is the ground the planet simulation's?
    pub fn baked(&self) -> bool {
        self.baked.is_some()
    }

    /// The bake's height at `dir`, a port's plain levelled to the ground at the port (or the
    /// sea, where the ground is under it) and blending back by `PAD_FLAT_OUTER`.
    fn baked_height(&self, h: &std::sync::Arc<crate::worlds::Heights>, dir: DVec3, detail: crate::worlds::Detail) -> (f64, bool) {
        let (mut out, mut whole) = h.at_detail(dir, detail);
        for p in &self.pads {
            let ground = dir.distance(*p) * self.body_radius;
            let t = ((ground - PAD_FLAT_INNER) / (PAD_FLAT_OUTER - PAD_FLAT_INNER)).clamp(0.0, 1.0);
            if t < 1.0 {
                let w = t * t * (3.0 - 2.0 * t);
                let (at, w2) = h.at_detail(*p, detail);
                let at = at.max(0.0);
                whole &= w2;
                out = at + (out - at) * w;
            }
        }
        (out, whole)
    }

    /// The surface height (as `surface`) for drawing: the bake's fine tiles as far as they're
    /// read, the rest asked for in the background; and whether it's the whole of it.
    pub fn surface_view(&self, dir: DVec3) -> (f64, bool) {
        match &self.baked {
            Some(h) => {
                let (v, whole) = self.baked_height(h, dir, crate::worlds::Detail::Loaded);
                (if self.kind == TerrainKind::Terran { v.max(0.0) } else { v }, whole)
            }
            None => (self.surface(dir), true),
        }
    }

    /// The surface height (as `surface`) from a bake's 5 km heights alone (a whole globe's).
    pub fn surface_coarse(&self, dir: DVec3) -> f64 {
        match &self.baked {
            Some(h) => {
                let v = self.baked_height(h, dir, crate::worlds::Detail::Coarse).0;
                if self.kind == TerrainKind::Terran { v.max(0.0) } else { v }
            }
            None => self.surface(dir),
        }
    }

    /// The bake's surface fields at `dir` for drawing (wetness, scree, bare rock: see
    /// `worlds::Heights::surface_at`), as far as they're read; and whether that's all of it.
    /// None: no bake, or none there (the sea).
    pub fn surface_fields_view(&self, dir: DVec3) -> (Option<[f32; 3]>, bool) {
        match &self.baked {
            Some(h) => h.surface_at(dir, crate::worlds::Detail::Loaded),
            None => (None, true),
        }
    }

    /// `height_and_crater` for a whole globe's map: a bake's 5 km heights alone.
    pub fn height_and_crater_coarse(&self, dir: DVec3) -> (f64, f64) {
        match &self.baked {
            Some(h) => (self.baked_height(h, dir, crate::worlds::Detail::Coarse).0, 0.0),
            None => self.height_and_crater(dir),
        }
    }

    /// Craters as (center direction, rim radius as a chord on the unit sphere).
    pub fn crater_rims(&self) -> impl Iterator<Item = (DVec3, f64)> + '_ {
        self.craters.iter().map(|c| (c.dir, c.radius))
    }

    pub fn add_pad(&mut self, dir: DVec3) {
        self.pads.push(dir.normalize());
    }

    /// Upper bound on the surface height (m).
    pub fn max_height(&self) -> f64 {
        match &self.baked {
            Some(h) => h.max,
            None => self.amplitude * 1.3 + self.relief_max(),
        }
    }

    /// The small-scale relief's amplitude (m) at octave `o` (see `relief`):
    /// the same slope at every scale, so ground stays rough however near.
    fn relief_amp(&self, o: u32) -> f64 {
        RELIEF_SLOPE * self.body_radius / (RELIEF_FREQ * RELIEF_LACUNARITY.powi(o as i32))
    }

    /// The highest the relief stands (m): each octave's half-plain,
    /// half-ridged noise reaches at most 0.3 of its amplitude above zero
    /// (its gullies go deeper), at the roughest.
    fn relief_max(&self) -> f64 {
        0.3 * (0..RELIEF_OCTAVES).map(|o| self.relief_amp(o)).sum::<f64>() * RELIEF_ROUGHEST
    }

    /// Small-scale relief (m): hills, ridges and gullies from tens of
    /// kilometres down to tens of metres, rough in the mountains and gentle
    /// on the lowlands (`rough` 0..1).
    fn relief(&self, dir: DVec3, rough: f64) -> f64 {
        let mut h = 0.0;
        let mut f = RELIEF_FREQ;
        for o in 0..RELIEF_OCTAVES {
            let n = value_noise(self.seed.wrapping_add(0x51ed + o as u64 * 0x2545), dir * f);
            // (Half ridged: sharp crests and gullies, like eroded ground.)
            let ridged = 0.6 - n.abs() * 1.6;
            h += self.relief_amp(o) * (0.5 * n + 0.5 * ridged);
            f *= RELIEF_LACUNARITY;
        }
        h * (0.25 + (RELIEF_ROUGHEST - 0.25) * rough)
    }

    fn noise(&self, p: DVec3) -> f64 {
        value_noise(self.seed, p)
    }

    /// Fractal noise in -1..1.
    fn fbm(&self, p: DVec3, octaves: u32) -> f64 {
        let (mut sum, mut amp, mut freq, mut norm) = (0.0, 1.0, 1.0, 0.0);
        for o in 0..octaves {
            sum += amp * value_noise(self.seed.wrapping_add(o as u64 * 0x9e37), p * freq);
            norm += amp;
            amp *= 0.5;
            freq *= 2.03;
        }
        sum / norm
    }

    /// Crater contribution (m) and how "inside a crater" the point is (0..1).
    fn craters(&self, dir: DVec3) -> (f64, f64) {
        let (mut h, mut inside) = (0.0, 0.0f64);
        for c in &self.craters {
            let d = dir.distance(c.dir);
            if d > c.radius * 1.6 {
                continue;
            }
            let x = d / c.radius;
            if x < 1.0 {
                h -= c.depth * (1.0 - x * x);
                inside = inside.max(1.0 - x);
            }
            h += c.depth * 0.35 * (-((x - 1.0) / 0.2).powi(2)).exp();
        }
        (h, inside)
    }

    /// Natural height before flattening for pads (m), and crater-ness.
    fn natural(&self, dir: DVec3) -> (f64, f64) {
        let base = self.fbm(dir * 1.7, 7);
        // Mountain ranges: sharp ridges, only on higher ground.
        let ridge = (1.0 - self.noise(dir * 5.3 + DVec3::splat(17.0)).abs()).powi(3);
        let continents = match self.kind {
            TerrainKind::Terran => base * 0.8 + 0.05,
            _ => base * 0.6,
        };
        let mountains = ridge * continents.max(0.0) * 1.2;
        let (crater, inside) = self.craters(dir);
        // (Rougher the higher and the more mountainous.)
        let rough = (continents.max(0.0) * 1.5 + mountains * 2.0).min(1.0);
        // (A spaceport stands on a wide plain: the small relief dies down round it.)
        let relief = self.relief(dir, rough) * self.flat_within(dir, RELIEF_FLAT_INNER, RELIEF_FLAT_OUTER);
        ((continents + mountains) * self.amplitude + crater + relief, inside)
    }

    /// Ground height (m), ignoring oceans. Flattened to 0 around spaceports.
    pub fn raw_height(&self, dir: DVec3) -> f64 {
        if let Some(h) = &self.baked {
            return self.baked_height(h, dir, crate::worlds::Detail::Full).0;
        }
        self.natural(dir).0 * self.pad_flat(dir)
    }

    /// Ground height (as `raw_height`) and how far inside a crater (0..1), together.
    pub fn height_and_crater(&self, dir: DVec3) -> (f64, f64) {
        if let Some(h) = &self.baked {
            return (self.baked_height(h, dir, crate::worlds::Detail::Full).0, 0.0);
        }
        let (h, inside) = self.natural(dir);
        (h * self.pad_flat(dir), inside)
    }

    /// How much a spaceport's plain this is (1 on its flat, 0 away from it).
    pub fn port_plain(&self, dir: DVec3) -> f64 {
        1.0 - self.pad_flat(dir)
    }

    /// How much of the natural height stands here (0 on a spaceport's flat, 1 away from them).
    fn pad_flat(&self, dir: DVec3) -> f64 {
        self.flat_within(dir, PAD_FLAT_INNER, PAD_FLAT_OUTER)
    }

    /// 0 within `inner` (m, over the ground) of a spaceport, rising smoothly to 1 by `outer`.
    fn flat_within(&self, dir: DVec3, inner: f64, outer: f64) -> f64 {
        let mut w: f64 = 1.0;
        for p in &self.pads {
            let ground = dir.distance(*p) * self.body_radius;
            let t = ((ground - inner) / (outer - inner)).clamp(0.0, 1.0);
            w = w.min(t * t * (3.0 - 2.0 * t));
        }
        w
    }

    /// The solid (or liquid) surface height (m): oceans fill anything below 0.
    pub fn surface(&self, dir: DVec3) -> f64 {
        let h = self.raw_height(dir);
        if self.kind == TerrainKind::Terran { h.max(0.0) } else { h }
    }

    pub fn is_ocean(&self, dir: DVec3) -> bool {
        self.kind == TerrainKind::Terran && self.raw_height(dir) < 0.0
    }

    /// What the ground is like here (for colors), and its height.
    pub fn classify(&self, dir: DVec3) -> (Ground, f64) {
        let h = self.raw_height(dir);
        if self.kind == TerrainKind::Terran && h < 0.0 {
            return (Ground::Ocean, 0.0);
        }
        let inside = if self.baked.is_some() { 0.0 } else { self.natural(dir).1 };
        let kind = if inside > 0.25 {
            Ground::Crater
        } else if h > 0.7 * self.amplitude {
            Ground::Peak
        } else if h > 0.3 * self.amplitude {
            Ground::Highland
        } else {
            Ground::Lowland
        };
        (kind, h)
    }
}

/// Dogma collides with the same height function.
impl universe_physics::Surface for Terrain {
    fn height(&self, dir: DVec3) -> f64 {
        self.raw_height(dir)
    }

    fn surface(&self, dir: DVec3) -> f64 {
        Terrain::surface(self, dir)
    }

    fn max_height(&self) -> f64 {
        Terrain::max_height(self)
    }

    fn liquid(&self, dir: DVec3) -> bool {
        self.is_ocean(dir)
    }
}

/// Smooth 3D value noise in -1..1.
pub(crate) fn value_noise(seed: u64, p: DVec3) -> f64 {
    let f = p.floor();
    let t = p - f;
    let s = t * t * (DVec3::splat(3.0) - t * 2.0);
    let (x, y, z) = (f.x as i64, f.y as i64, f.z as i64);
    let h = |dx: i64, dy: i64, dz: i64| lattice(seed, x + dx, y + dy, z + dz);
    let lerp = |a: f64, b: f64, t: f64| a + (b - a) * t;
    let x00 = lerp(h(0, 0, 0), h(1, 0, 0), s.x);
    let x10 = lerp(h(0, 1, 0), h(1, 1, 0), s.x);
    let x01 = lerp(h(0, 0, 1), h(1, 0, 1), s.x);
    let x11 = lerp(h(0, 1, 1), h(1, 1, 1), s.x);
    lerp(lerp(x00, x10, s.y), lerp(x01, x11, s.y), s.z) * 2.0 - 1.0
}

fn lattice(seed: u64, x: i64, y: i64, z: i64) -> f64 {
    let mut v = seed ^ (x as u64).wrapping_mul(0x9e37_79b9_7f4a_7c15);
    v ^= (y as u64).wrapping_mul(0xc2b2_ae3d_27d4_eb4f);
    v ^= (z as u64).wrapping_mul(0x1656_67b1_9e37_79f9);
    v = (v ^ (v >> 31)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    v = (v ^ (v >> 29)).wrapping_mul(0x94d0_49bb_1331_11eb);
    (v >> 11) as f64 / (1u64 << 53) as f64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn heights_are_sane_and_pads_are_flat() {
        let mut t = Terrain::new(TerrainKind::Terran, 6.4e6, 7);
        let pad = DVec3::new(0.3, 0.5, 0.8).normalize();
        t.add_pad(pad);
        let mut rng = Rng::new(1);
        let (mut lo, mut hi, mut ocean) = (f64::MAX, f64::MIN, 0);
        for _ in 0..20_000 {
            let d = rng.unit_vector();
            let h = t.surface(d);
            lo = lo.min(h);
            hi = hi.max(h);
            if t.is_ocean(d) {
                ocean += 1;
            }
        }
        eprintln!("terran: surface {lo:.0}..{hi:.0} m, ocean {:.0}%", ocean as f64 / 200.0);
        assert!(lo >= 0.0, "oceans are a flat surface at 0");
        assert!(hi <= t.max_height() && hi > 1000.0);
        assert!((5_000..17_000).contains(&ocean), "some ocean, some land");
        assert!(t.surface(pad).abs() < 1e-9, "flat at the pad");
    }
}
