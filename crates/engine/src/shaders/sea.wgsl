// The sea's look (the planet lab's): the water's own colour by depth, the sky reflected by Fresnel,
// and the sun's glint spread by the wind's waves. Physics, not a palette:
// - Water absorbs red far faster than blue (pure water: a ≈ 0.45, 0.06, 0.015 per m in red, green,
//   blue; Pope & Fry 1997); light scattered back from within the water column (molecules and fine
//   particles: b_b ~ 0.002–0.004 per m) sets the deep sea's colour, and where the bottom is near,
//   it shows through: a seabed reflectance seen through twice the depth (down and back up).
// - Reflection: Fresnel for water (n = 1.33, Schlick's form: 2% looking straight down, a mirror at
//   grazing angles), of the sky the caller sees along the reflected ray (the lab's air_sky).
// - The sun's glint: wave facets' slopes are Gaussian with variance σ² = 0.003 + 0.00512·U (U the
//   wind at 12.5 m, m/s; Cox & Munk 1954): a calm sea a tight bright glint, a windy one wide and dim.
//   The wind is the bake's: globe_spec holds the sea's calmness, 1 − U/9 m/s clipped to 0.35…1
//   (planet-sim tools/globe_look.py), so U = 9·(1 − calm).
// - Close up, the waves' own relief as normal detail, its slope spread by the wind, faded below a
//   pixel.
//
// One entry: sea_material(SeaIn) -> SeaOut {colour (linear, the light leaving the sea toward the
// eye, in the sun's units), and nothing else}: it already holds the reflection and the glint.
//
// Status: draft on `planet`, validated with naga; waiting on the integrator's slot (as the ground's).

struct SeaIn {
    // The water's depth under this point (m, > 0).
    depth_m: f32,
    // The wind (m/s): 9·(1 − calm) from globe_spec's calmness.
    wind_ms: f32,
    // Unit vectors, any frame (the same for all): toward the eye, the sea's up (the world's radial),
    // toward the sun.
    to_eye: vec3<f32>,
    up: vec3<f32>,
    sun_dir: vec3<f32>,
    // The sun's light (rgb, the scene's units) and the sky's light along the reflected ray (rgb).
    sun: vec3<f32>,
    sky: vec3<f32>,
    // The light the water's body is lit by from above (sun and sky together; rgb).
    down: vec3<f32>,
    // Where on the sea (m, steady on the surface: the caller's wrapped `micro`) and a pixel's size (m).
    q: vec3<f32>,
    pixel_m: f32,
};

const SEA_ABSORB: vec3<f32> = vec3<f32>(0.45, 0.06, 0.015);   // per m, pure water (Pope & Fry 1997)
const SEA_BACK: vec3<f32> = vec3<f32>(0.0025, 0.003, 0.0035);  // per m, backscatter (molecules + fine particles)
const SEA_BED: vec3<f32> = vec3<f32>(0.25, 0.22, 0.17);        // a sandy bottom's reflectance
const SEA_F0: f32 = 0.02;                                       // Fresnel at normal incidence, water

fn sea_hash(p: vec3<f32>) -> f32 {
    var q = fract(p * 0.3183099 + vec3<f32>(0.1));
    q = q * 17.0;
    return fract(q.x * q.y * q.z * (q.x + q.y + q.z)) * 2.0 - 1.0;
}

fn sea_noise(x: vec3<f32>) -> f32 {
    let i = floor(x);
    var f = fract(x);
    f = f * f * (3.0 - 2.0 * f);
    return mix(mix(mix(sea_hash(i), sea_hash(i + vec3<f32>(1.0, 0.0, 0.0)), f.x),
                   mix(sea_hash(i + vec3<f32>(0.0, 1.0, 0.0)), sea_hash(i + vec3<f32>(1.0, 1.0, 0.0)), f.x), f.y),
               mix(mix(sea_hash(i + vec3<f32>(0.0, 0.0, 1.0)), sea_hash(i + vec3<f32>(1.0, 0.0, 1.0)), f.x),
                   mix(sea_hash(i + vec3<f32>(0.0, 1.0, 1.0)), sea_hash(i + vec3<f32>(1.0, 1.0, 1.0)), f.x), f.y), f.z);
}

// The waves' tilt of the normal close up: noise slopes over 64 m to 2 m (powers of two: `q` wraps
// every 4096 m, see gm_pnoise), their spread the
// wind's, each faded as it falls below a few pixels.
fn sea_wave_normal(i: SeaIn, sigma: f32) -> vec3<f32> {
    var g = vec2<f32>(0.0);
    var lam = 64.0;
    var amp = 1.0;
    var norm = 0.0;
    for (var o = 0; o < 6; o++) {
        let fade = clamp(lam / (i.pixel_m * 3.0) - 1.0, 0.0, 1.0);
        let e = 0.15 * lam;
        let sd = 20.0 + f32(o) * 3.7;
        let n0 = gm_pnoise(i.q, lam, sd);
        g += amp * fade * vec2<f32>(gm_pnoise(i.q + vec3<f32>(e, 0.0, 0.0), lam, sd) - n0, gm_pnoise(i.q + vec3<f32>(0.0, 0.0, e), lam, sd) - n0) / 0.15;
        norm += amp;
        lam *= 0.5;
        amp *= 0.7;
    }
    // (Slopes scaled to the sea's spread; the up frame's two tangents from any perpendicular.)
    let s = g / max(norm, 1e-6) * sigma;
    let t1 = normalize(cross(i.up, select(vec3<f32>(1.0, 0.0, 0.0), vec3<f32>(0.0, 0.0, 1.0), abs(i.up.x) > 0.9)));
    let t2 = cross(i.up, t1);
    return normalize(i.up - s.x * t1 - s.y * t2);
}

fn sea_material(i: SeaIn) -> vec3<f32> {
    let u = max(i.wind_ms, 0.0);
    let sigma2 = 0.003 + 0.00512 * u;
    let n = sea_wave_normal(i, sqrt(sigma2) * 0.7);
    let cos_v = clamp(dot(n, i.to_eye), 1e-3, 1.0);
    // Fresnel (Schlick).
    let fres = SEA_F0 + (1.0 - SEA_F0) * pow(1.0 - cos_v, 5.0);
    // The water's body: backscatter from the column above the bed, the bed seen through it.
    let k = SEA_ABSORB + SEA_BACK;
    let through = exp(-2.0 * k * max(i.depth_m, 0.0));
    // (The column's reflectance R ≈ 0.33·b_b/(a + b_b): Gordon et al. 1975; clear open ocean ~0.05
    // in blue, ~0.005 in red.)
    let body = 0.33 * SEA_BACK / k * (vec3<f32>(1.0) - through) + SEA_BED * through;
    // (In the engine's units a white Lambert surface lit by `down` shows `down` (no 1/π): the
    // water's body, a reflectance, the same.)
    let water = body * i.down * (1.0 - fres);
    // The sky reflected.
    let refl = i.sky * fres;
    // The sun's glint: facets tilted to send the sun to the eye (half vector), their slope's
    // probability by Cox–Munk, Fresnel at the facet, the usual microfacet geometry.
    let hv = normalize(i.sun_dir + i.to_eye);
    let cos_h = clamp(dot(hv, i.up), 1e-3, 1.0);
    let tan2 = (1.0 - cos_h * cos_h) / (cos_h * cos_h);
    let p_slope = exp(-tan2 / sigma2) / (3.14159265 * sigma2 * pow(cos_h, 4.0));
    let cos_s = max(dot(i.up, i.sun_dir), 0.0);
    let fres_h = SEA_F0 + (1.0 - SEA_F0) * pow(1.0 - clamp(dot(hv, i.to_eye), 0.0, 1.0), 5.0);
    // (A BRDF times irradiance: `sun` is irradiance / π in the engine's units, hence × π.)
    let glint = 3.14159265 * i.sun * fres_h * p_slope / (4.0 * max(dot(i.up, i.to_eye), 0.05)) * step(0.0, cos_s);
    return water + refl + glint;
}
