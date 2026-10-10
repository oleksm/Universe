use glam::DVec3;

/// SplitMix64: tiny, fast and deterministic across platforms.
#[derive(Clone, Debug)]
pub struct Rng(u64);

/// Derive an independent seed from a parent seed and a child index.
pub fn mix(seed: u64, index: u64) -> u64 {
    Rng(seed ^ index.wrapping_mul(0xa076_1d64_78bd_642f)).next_u64()
}

impl Rng {
    pub fn new(seed: u64) -> Self {
        Rng(seed)
    }

    pub fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }

    /// Uniform in [0, 1).
    pub fn f64(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 / (1u64 << 53) as f64
    }

    pub fn range(&mut self, lo: f64, hi: f64) -> f64 {
        lo + (hi - lo) * self.f64()
    }

    /// Uniform integer in `lo..=hi`.
    pub fn int(&mut self, lo: u32, hi: u32) -> u32 {
        lo + (self.next_u64() % (hi - lo + 1) as u64) as u32
    }

    pub fn chance(&mut self, p: f64) -> bool {
        self.f64() < p
    }

    pub fn pick<'a, T>(&mut self, items: &'a [T]) -> &'a T {
        &items[(self.next_u64() % items.len() as u64) as usize]
    }

    /// Standard normal (Box-Muller).
    pub fn normal(&mut self) -> f64 {
        let u = self.f64().max(1e-300);
        let v = self.f64();
        (-2.0 * u.ln()).sqrt() * (std::f64::consts::TAU * v).cos()
    }

    pub fn unit_vector(&mut self) -> DVec3 {
        let z = self.range(-1.0, 1.0);
        let a = self.range(0.0, std::f64::consts::TAU);
        let r = (1.0 - z * z).sqrt();
        DVec3::new(r * a.cos(), z, r * a.sin())
    }
}
