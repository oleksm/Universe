// The ground below the ~150 m level, down to ~1 m: the GPU's twin of crates/world/src/
// ground_detail.rs (the truth, f64; this is held to it within 5 cm). Same rules, same integer
// lattice and hashes: see there. Where: whole metres on the cube face (`cell`: the tile's corner
// in whole metres plus floor(the local offset)) and the fraction of the metre (`frac`), so f32
// keeps millimetres anywhere on a face.

struct GroundFields {
    grad: vec2<f32>,   // the 150 m ground's gradient along the face's x, y (m/m)
    height: f32,       // its height (m)
    log_area: f32,     // log10 drainage area (m²)
    ice: f32,          // glacier cover 0–1
    layered: f32,      // how layered the rock is 0–1
};

const GD_OCTAVES: u32 = 8u;
const GD_TOP_LOG2: u32 = 8u;
const GD_AMP_PER_M: f32 = 0.05;
const GD_GAIN: f32 = 0.7;
const GD_STRATA_M: f32 = 18.0;
const GD_TAU: f32 = 6.2831853;

fn gd_hash01(x: i32, y: i32, seed: u32, k: u32) -> f32 {
    var h = (bitcast<u32>(x) * 0x8da6b343u) ^ (bitcast<u32>(y) * 0xd8163841u) ^ (seed * 0xcb1ab31fu) ^ (k * 0x165667b1u);
    h = h ^ (h >> 15u);
    h = h * 0x2c1b3c6du;
    h = h ^ (h >> 12u);
    h = h * 0x297a2d39u;
    h = h ^ (h >> 15u);
    return f32(h >> 8u) / 16777216.0;
}

fn gd_smoothstep(a: f32, b: f32, x: f32) -> f32 {
    let t = clamp((x - a) / (b - a), 0.0, 1.0);
    return t * t * (3.0 - 2.0 * t);
}

// The height offset (m) over the 150 m ground.
fn ground_detail(cell: vec2<i32>, frac: vec2<f32>, f: GroundFields, seed: u32, band_m: f32) -> f32 {
    let slope = length(f.grad);
    let steep = clamp(slope / 0.5, 0.0, 1.2);
    let damp = (1.0 - gd_smoothstep(6.0, 8.0, f.log_area)) * (1.0 - 0.7 * clamp(f.ice, 0.0, 1.0));
    var z = 0.0;
    var g = f.grad;
    var amp_k = 1.0;
    for (var o = 0u; o < GD_OCTAVES; o++) {
        let lg = GD_TOP_LOG2 - o;
        let lam = f32(1u << lg);
        let keep = clamp((lam / max(band_m, 1e-3) - 2.0) / 2.0, 0.0, 1.0);
        if (keep <= 0.0) {
            break;
        }
        let ci = vec2<i32>(cell.x >> lg, cell.y >> lg);
        let mask = i32((1u << lg) - 1u);
        let fx = vec2<f32>((f32(cell.x & mask) + frac.x) / lam, (f32(cell.y & mask) + frac.y) / lam);
        let gl = max(length(g), 1e-9);
        let across = vec2<f32>(-g.y / gl, g.x / gl);
        var acc = 0.0;
        var dacc = 0.0;
        var wsum = 0.0;
        for (var dy = -1; dy <= 1; dy++) {
            for (var dx = -1; dx <= 1; dx++) {
                let cx = ci.x + dx;
                let cy = ci.y + dy;
                let p = vec2<f32>(f32(dx) + gd_hash01(cx, cy, seed, 2u * o), f32(dy) + gd_hash01(cx, cy, seed, 2u * o + 1u));
                let d = fx - p;
                let w = exp(-dot(d, d) * 4.0);
                let ph = dot(d, across) * GD_TAU;
                acc += w * cos(ph);
                dacc += w * -sin(ph);
                wsum += w;
            }
        }
        let a = amp_k * lam * GD_AMP_PER_M * steep * damp * keep;
        z += a * acc / wsum;
        let s = a * dacc / wsum * GD_TAU / lam;
        g = g + s * across;
        amp_k *= GD_GAIN;
    }
    let keep_s = clamp((GD_STRATA_M / max(band_m, 1e-3) - 2.0) / 2.0, 0.0, 1.0);
    if (keep_s > 0.0 && f.layered > 0.0) {
        let bed = (f.height + z) / GD_STRATA_M;
        let saw = bed - floor(bed);
        let ledge = GD_STRATA_M * 0.35 * (min(saw / 0.25, 1.0) - saw);
        z += f.layered * ledge * clamp((slope - 0.35) / 0.4, 0.0, 1.0) * keep_s;
    }
    return z;
}
