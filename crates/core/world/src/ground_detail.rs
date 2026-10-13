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
