// The ground's runtime detail, the GPU's twin of `world::detail::offset` (the truth, in f64):
// held to it within its AGREE_M by the engine's test (tests/detail_agree.rs). The lab's
// generator; until it lands, a stand-in that adds nothing.
//
// The module that includes this one supplies the tile's samples, indices running two past its
// edges into its neighbours (the halo):
//   fn detail_height(i: i32, j: i32) -> f32        the ground's height there (m: 5 km + 600 m + 150 m)
//   fn detail_fields(i: i32, j: i32) -> vec4<f32>  fd150, raw 0..255 (flow, area, threshold, ice)

// `origin`: the tile's corner on its cube face in whole metres; `at`: the place, metres from
// `origin` along u and v (cell: origin + floor(at), fraction: fract(at)); `spacing`: the
// tile's sample spacing (m); `rock`: the rock unit; `seed`: the world's seed (low, high words);
// `cell`: the band (m): finer than about two cells left out.
// --- The generator (the lab's; the f64 truth: world::ground_detail) ---------------------------
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

// (The rocks' layering, by rock unit: world::detail::LAYERED.)
const DETAIL_LAYERED: array<f32, 20> = array<f32, 20>(0.2, 0.5, 0.0, 0.1, 0.2, 0.3, 0.0, 0.7, 0.8, 0.8, 0.1, 0.6, 0.9, 0.1, 0.3, 0.6, 0.1, 0.0, 0.0, 0.3);

fn detail_bilinear_h(x: f32, y: f32) -> f32 {
    let i = floor(x);
    let j = floor(y);
    let fx = x - i;
    let fy = y - j;
    let a = mix(detail_height(i32(i), i32(j)), detail_height(i32(i) + 1, i32(j)), fx);
    let b = mix(detail_height(i32(i), i32(j) + 1), detail_height(i32(i) + 1, i32(j) + 1), fx);
    return mix(a, b, fy);
}

fn detail_bilinear_f(x: f32, y: f32) -> vec4<f32> {
    let i = floor(x);
    let j = floor(y);
    let fx = x - i;
    let fy = y - j;
    let a = mix(detail_fields(i32(i), i32(j)), detail_fields(i32(i) + 1, i32(j)), fx);
    let b = mix(detail_fields(i32(i), i32(j) + 1), detail_fields(i32(i) + 1, i32(j) + 1), fx);
    return mix(a, b, fy);
}

fn detail(origin: vec2<i32>, at: vec2<f32>, spacing: f32, rock: u32, seed: vec2<u32>, cell: f32) -> f32 {
    let x = at.x / spacing;
    let y = at.y / spacing;
    let grad = vec2<f32>((detail_bilinear_h(x + 1.0, y) - detail_bilinear_h(x - 1.0, y)) / (2.0 * spacing),
                         (detail_bilinear_h(x, y + 1.0) - detail_bilinear_h(x, y - 1.0)) / (2.0 * spacing));
    let fl = detail_bilinear_f(x * 0.5, y * 0.5);
    var layered = 0.3;
    if (rock < 20u) {
        layered = DETAIL_LAYERED[rock];
    }
    let f = GroundFields(grad, detail_bilinear_h(x, y), fl.y / 16.0, fl.w / 255.0, layered);
    let fa = floor(at);
    return ground_detail(origin + vec2<i32>(fa), at - fa, f, seed.x ^ seed.y, cell);
}
