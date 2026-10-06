//! The ground below the ~150 m level, down to ~1 m (layer C of planet-sim's
//! docs/terrain-relief-plan.md; the lab's): a height offset over the 150 m ground, grown from
//! what the bake knows there. One pure, deterministic function: this Rust (f64) is the truth for
//! physics on every client and the server; the GPU's twin (shaders/ground_detail.wgsl) is held to
//! it within 5 cm. The reference it was worked out on: planet-sim tools/ground_reference.py.
//!
//! - Gullies and spurs, the erosion filter's way (Runevision 2023, after Quilez's gradient-aligned
//!   noise): octaves from 256 m to 2 m, each laying its gullies across the slope of everything
//!   built so far (the 150 m ground's gradient and the octaves before it), from points scattered
//!   one to a cell over a 3 × 3 neighbourhood (Gaussian-weighted), so finer gullies follow and
//!   branch off coarser ones; deeper where steep, none on flats, smoothed on the floors of big
//!   channels and under ice.
//! - Strata: on steep, layered rock, its bedding (~18 m) weathered into ledges and risers.
//! - A band limit: octaves under ~2 × `band_m` drop out (fading over an octave), so a coarse patch
//!   reads the ground smoothed, never aliased; physics asks at ~0.5 m.
//!
//! Where: whole metres on the cube face's grid (`cell`, i64: exact for any lattice the octaves
//! use, all powers of two) plus the fraction within that metre (`frac`): the GPU passes its tile's
//! corner in whole metres plus a small local offset, and reaches the same lattice cells and hashes
//! bit for bit (integer hashes; no floating point in them).

/// What the bake knows at the point (the 150 m ground and its fields: planet-sim fz150/fd150).
#[derive(Clone, Copy, Debug, Default)]
pub struct Fields {
    /// The 150 m ground's gradient (m/m) along the face's x and y (its u and v).
    pub grad: [f64; 2],
    /// The ground's height there (m): the strata follow it.
    pub height: f64,
    /// log10 of the drainage area (m²): channels' floors smoothed.
    pub log_area: f64,
    /// Glacier cover (0–1): ice smooths.
    pub ice: f64,
    /// How layered the rock is (0–1; by its unit: sediments high, granite none).
    pub layered: f64,
}

/// Where on the face: whole metres and the fraction of the metre (0–1).
#[derive(Clone, Copy, Debug)]
pub struct At {
    pub cell: [i64; 2],
    pub frac: [f64; 2],
}

const OCTAVES: u32 = 8; // 256 m … 2 m
const TOP_LOG2: u32 = 8;
const AMP_PER_M: f64 = 0.05;
const GAIN: f64 = 0.7;
const STRATA_M: f64 = 18.0;

/// An integer hash to [0, 1) (24 bits: exact in f32 too).
pub fn hash01(x: i64, y: i64, seed: u32, k: u32) -> f64 {
    let mut h = (x as u32).wrapping_mul(0x8da6_b343) ^ (y as u32).wrapping_mul(0xd816_3841) ^ seed.wrapping_mul(0xcb1a_b31f) ^ k.wrapping_mul(0x1656_67b1);
    h ^= h >> 15;
    h = h.wrapping_mul(0x2c1b_3c6d);
    h ^= h >> 12;
    h = h.wrapping_mul(0x297a_2d39);
    h ^= h >> 15;
    (h >> 8) as f64 / 16_777_216.0
}

/// The height offset (m) over the 150 m ground at `at`, for `seed` (the world's), detail no finer
/// than `band_m` allows.
pub fn detail(at: At, f: &Fields, seed: u32, band_m: f64) -> f64 {
    let slope = (f.grad[0] * f.grad[0] + f.grad[1] * f.grad[1]).sqrt();
    let steep = (slope / 0.5).clamp(0.0, 1.2);
    // (Big channels' floors and ice: smooth.)
    let damp = (1.0 - smoothstep(6.0, 8.0, f.log_area)) * (1.0 - 0.7 * f.ice.clamp(0.0, 1.0));
    let mut z = 0.0;
    let mut g = f.grad;
    let mut amp_k = 1.0;
    for o in 0..OCTAVES {
        let lg = TOP_LOG2 - o;
        let lam = (1u64 << lg) as f64;
        // Band limit: full above 4 × band, gone below 2 ×.
        let keep = ((lam / band_m.max(1e-3) - 2.0) / 2.0).clamp(0.0, 1.0);
        if keep <= 0.0 {
            break;
        }
        // The cell and the position within it, in cells (exact: λ a power of two).
        let ci = [at.cell[0] >> lg, at.cell[1] >> lg];
        let fx = [((at.cell[0] & ((1i64 << lg) - 1)) as f64 + at.frac[0]) / lam, ((at.cell[1] & ((1i64 << lg) - 1)) as f64 + at.frac[1]) / lam];
        // Across the slope (the gradient so far, turned a quarter).
        let gl = (g[0] * g[0] + g[1] * g[1]).sqrt().max(1e-9);
        let across = [-g[1] / gl, g[0] / gl];
        let (mut acc, mut dacc, mut wsum) = (0.0, 0.0, 0.0);
        for dy in -1..=1i64 {
            for dx in -1..=1i64 {
                let cx = ci[0] + dx;
                let cy = ci[1] + dy;
                let px = dx as f64 + hash01(cx, cy, seed, 2 * o);
                let py = dy as f64 + hash01(cx, cy, seed, 2 * o + 1);
                let d = [fx[0] - px, fx[1] - py];
                let w = (-(d[0] * d[0] + d[1] * d[1]) * 4.0).exp();
                let ph = (d[0] * across[0] + d[1] * across[1]) * std::f64::consts::TAU;
                acc += w * ph.cos();
                dacc += w * -ph.sin();
                wsum += w;
            }
        }
        let a = amp_k * lam * AMP_PER_M * steep * damp * keep;
        z += a * acc / wsum;
        // (The octave's own slope, across: the next octave's gullies follow it.)
        let s = a * dacc / wsum * std::f64::consts::TAU / lam;
        g = [g[0] + s * across[0], g[1] + s * across[1]];
        amp_k *= GAIN;
    }
    // Strata: ledges on steep, layered rock (bedding horizontal: contour-following).
    let keep_s = ((STRATA_M / band_m.max(1e-3) - 2.0) / 2.0).clamp(0.0, 1.0);
    if keep_s > 0.0 && f.layered > 0.0 {
        let bed = (f.height + z) / STRATA_M;
        let saw = bed - bed.floor();
        let ledge = STRATA_M * 0.35 * ((saw / 0.25).min(1.0) - saw);
        z += f.layered * ledge * ((slope - 0.35) / 0.4).clamp(0.0, 1.0) * keep_s;
    }
    z
}

fn smoothstep(a: f64, b: f64, x: f64) -> f64 {
    let t = ((x - a) / (b - a)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn steep() -> Fields {
        Fields { grad: [0.45, -0.3], height: 3200.0, log_area: 4.0, ice: 0.0, layered: 0.6 }
    }

    #[test]
    fn deterministic() {
        let at = At { cell: [1_234_567, -7_654_321], frac: [0.25, 0.75] };
        assert_eq!(detail(at, &steep(), 42, 0.5), detail(at, &steep(), 42, 0.5));
    }

    #[test]
    fn continuous_across_metres_and_cells() {
        // Just either side of a whole metre (and of a 256 m cell): within a few cm.
        for &x in &[1_000_000i64, 1_048_576] {
            let a = detail(At { cell: [x - 1, 5000], frac: [0.9999, 0.3] }, &steep(), 7, 0.5);
            let b = detail(At { cell: [x, 5000], frac: [0.0, 0.3] }, &steep(), 7, 0.5);
            assert!((a - b).abs() < 0.05, "{a} vs {b} at {x}");
        }
    }

    #[test]
    fn band_limited_and_flat_ground_untouched() {
        let at = At { cell: [99_999, 88_888], frac: [0.5, 0.5] };
        assert_eq!(detail(at, &steep(), 3, 1000.0), 0.0);
        let flat = Fields { grad: [0.0, 0.0], ..steep() };
        assert!(detail(at, &flat, 3, 0.5).abs() < 1e-9);
    }

    /// The WGSL twin's arithmetic in f32 (shaders/ground_detail.wgsl, line for line): what the GPU
    /// computes, to hold it to this file within 5 cm.
    fn detail_f32(cell: [i32; 2], frac: [f32; 2], f: &Fields, seed: u32, band_m: f32) -> f32 {
        let grad = [f.grad[0] as f32, f.grad[1] as f32];
        let slope = (grad[0] * grad[0] + grad[1] * grad[1]).sqrt();
        let steep = (slope / 0.5).clamp(0.0, 1.2);
        let ss = |a: f32, b: f32, x: f32| {
            let t = ((x - a) / (b - a)).clamp(0.0, 1.0);
            t * t * (3.0 - 2.0 * t)
        };
        let damp = (1.0 - ss(6.0, 8.0, f.log_area as f32)) * (1.0 - 0.7 * (f.ice as f32).clamp(0.0, 1.0));
        let (mut z, mut g, mut amp_k) = (0.0f32, grad, 1.0f32);
        for o in 0..OCTAVES {
            let lg = TOP_LOG2 - o;
            let lam = (1u32 << lg) as f32;
            let keep = ((lam / band_m.max(1e-3) - 2.0) / 2.0).clamp(0.0, 1.0);
            if keep <= 0.0 {
                break;
            }
            let ci = [cell[0] >> lg, cell[1] >> lg];
            let mask = ((1u32 << lg) - 1) as i32;
            let fx = [((cell[0] & mask) as f32 + frac[0]) / lam, ((cell[1] & mask) as f32 + frac[1]) / lam];
            let gl = (g[0] * g[0] + g[1] * g[1]).sqrt().max(1e-9);
            let across = [-g[1] / gl, g[0] / gl];
            let (mut acc, mut dacc, mut wsum) = (0.0f32, 0.0f32, 0.0f32);
            for dy in -1..=1i32 {
                for dx in -1..=1i32 {
                    let (cx, cy) = (ci[0] + dx, ci[1] + dy);
                    let p = [dx as f32 + hash01(cx as i64, cy as i64, seed, 2 * o) as f32, dy as f32 + hash01(cx as i64, cy as i64, seed, 2 * o + 1) as f32];
                    let d = [fx[0] - p[0], fx[1] - p[1]];
                    let w = (-(d[0] * d[0] + d[1] * d[1]) * 4.0).exp();
                    let ph = (d[0] * across[0] + d[1] * across[1]) * std::f32::consts::TAU;
                    acc += w * ph.cos();
                    dacc += w * -ph.sin();
                    wsum += w;
                }
            }
            let a = amp_k * lam * AMP_PER_M as f32 * steep * damp * keep;
            z += a * acc / wsum;
            let s = a * dacc / wsum * std::f32::consts::TAU / lam;
            g = [g[0] + s * across[0], g[1] + s * across[1]];
            amp_k *= GAIN as f32;
        }
        let keep_s = ((STRATA_M as f32 / band_m.max(1e-3) - 2.0) / 2.0).clamp(0.0, 1.0);
        if keep_s > 0.0 && f.layered > 0.0 {
            let bed = (f.height as f32 + z) / STRATA_M as f32;
            let saw = bed - bed.floor();
            let ledge = STRATA_M as f32 * 0.35 * ((saw / 0.25).min(1.0) - saw);
            z += f.layered as f32 * ledge * ((slope - 0.35) / 0.4).clamp(0.0, 1.0) * keep_s;
        }
        z
    }

    #[test]
    fn gpu_twin_within_5cm() {
        // Far out on a face (~9,000 km from its corner), steep layered rock, 40,000 points at 0.5 m.
        let mut worst = 0.0f64;
        for i in 0..200 {
            for j in 0..200 {
                let cell = [9_000_000 + 7 * i, -4_000_000 + 11 * j];
                let frac = [((i * 37) % 100) as f64 / 100.0, ((j * 53) % 100) as f64 / 100.0];
                let a = detail(At { cell: [cell[0] as i64, cell[1] as i64], frac }, &steep(), 9, 0.5);
                let b = detail_f32([cell[0] as i32, cell[1] as i32], [frac[0] as f32, frac[1] as f32], &steep(), 9, 0.5) as f64;
                worst = worst.max((a - b).abs());
            }
        }
        assert!(worst < 0.05, "the f32 twin differs by {worst} m");
    }

    #[test]
    fn relief_in_range() {
        // Over a 512 m square at 2 m: the offset's spread a few metres to a few tens.
        let mut v = Vec::new();
        for i in 0..256 {
            for j in 0..256 {
                v.push(detail(At { cell: [3_000_000 + 2 * i, 4_000_000 + 2 * j], frac: [0.0, 0.0] }, &steep(), 11, 0.5));
            }
        }
        let mean = v.iter().sum::<f64>() / v.len() as f64;
        let sd = (v.iter().map(|x| (x - mean) * (x - mean)).sum::<f64>() / v.len() as f64).sqrt();
        assert!(sd > 1.0 && sd < 40.0, "spread {sd}");
    }
}
