// The ground's look close up (the planet lab's rules, ported from its viewer's ground shader in
// planet-sim's demo/index.html): what the ground is made of at each pixel, from the world's own
// maps, not a texture painted on it. Plants and soil from the world's ground map (its biomes,
// without rock and snow); woodland and open ground as a mosaic; wet valley floors greener; rock
// showing on steep and high ground in its unit's own colour, banded by its beds; scree below the
// steep ground; sand on low, gentle coasts where it isn't frozen; snow where the year is cold at
// that height.
//
// The engine samples the maps (equirectangular, per world) and works out the geometry; this file
// only turns them into a colour. Its one entry: `ground_material(GroundIn) -> vec3<f32>`, a
// linear-light albedo for land (the sea is the engine's own).
//
// Map encodings (planet-sim-surface/1, docs/survey-format.md in planet-sim):
//   globe_ground.jpg  sRGB texture: sampled, it is linear already (the engine passes it so)
//   climate.png       R: year-mean temperature at sea level, °C = R·100 − 50 (R in 0…1)
//                     G: rain, m a year = G·4
//   rockid.png        R: rock unit index × 8 (0…255, nearest-sampled: unit = round(R·255 / 8))
//   rv_*.png          below the rivers (rows 1025–1537, cols 0–512, half resolution): wetness (R),
//                     scree (G), bare rock (B), each 0…1
//
// Called by fs_mesh (scene.wgsl, which this file is prepended to) where a world's maps are bound,
// over land, faded in under a pixel of ~2.5 km (the integrator's slot, 88d74e01).

struct GroundIn {
    // The world's ground colour here (linear light, from its sRGB texture): plants and soil, no
    // rock, no snow.
    ground: vec3<f32>,
    // The same read softened (a few texels' worth: a lower mip), for where the 600 m surface
    // fields take over the local pattern; pass `ground` again if there's no such read.
    ground_soft: vec3<f32>,
    // Year-mean temperature at sea level (°C) and rain (m a year), from climate.png.
    t_sea_c: f32,
    rain_m: f32,
    // The rock unit here (rockid.png's index: geology.UNITS order in planet-sim).
    unit: u32,
    // The 600 m surface fields (rv tiles), each 0…1, and 1 where they are present (else 0: the
    // rules then lean on slope and height alone).
    wet: f32,
    scree: f32,
    bare: f32,
    surface_on: f32,
    // Height above the sea (m), and the ground's true slope (rise over run, the relief's
    // exaggeration taken out).
    h_m: f32,
    slope: f32,
    // How far the ground stands above (+) or below (−) its surroundings at ~8 km, about −1…1
    // (ridges and hollows: woodland favours hollows); 0 if unknown.
    rel: f32,
    // Where on the ground (m), for the rules' noise (any frame steady on the ground, e.g. the
    // engine's wrapped `micro`: its period is far above the noise's longest wavelength, 2 km),
    // and the size of a pixel there (m), to fade out noise finer than a pixel.
    q: vec3<f32>,
    // Where on the world, absolute (m, body-fixed: the unit direction × the radius): for the
    // octaves of 64 m and up, which must not repeat (q's 4 km period showed as a grid of blotches
    // from high up); f32 keeps them to a few mm there.
    qw: vec3<f32>,
    pixel_m: f32,
    // The rock's colour here, blended over the rock map's four nearest texels by their bilinear
    // weights (ground_rock_blend): the map is read nearest for its units, and unblended, each
    // unit's colour would show as a square texel (~10 km) wherever rock shows at all. Left zero,
    // the unit's own colour is used.
    rock_c: vec3<f32>,
};

// The rock colour over the rock map's four nearest texels (units u00 u10 u01 u11 at the texel
// corners about the point, f the point's fraction between them): for GroundIn.rock_c.
fn ground_rock_blend(u00: u32, u10: u32, u01: u32, u11: u32, f: vec2<f32>) -> vec3<f32> {
    let c0 = mix(ROCK[min(u00, 19u)], ROCK[min(u10, 19u)], f.x);
    let c1 = mix(ROCK[min(u01, 19u)], ROCK[min(u11, 19u)], f.x);
    return mix(c0, c1, f.y);
}

// Bare rock's colour by unit (linear light, geology.UNITS order: planet-sim look.TRUE).
const ROCK: array<vec3<f32>, 20> = array<vec3<f32>, 20>(
    vec3<f32>(0.102, 0.089, 0.076), vec3<f32>(0.195, 0.138, 0.112), vec3<f32>(0.434, 0.352, 0.287), vec3<f32>(0.392, 0.305, 0.231),
    vec3<f32>(0.150, 0.156, 0.107), vec3<f32>(0.305, 0.262, 0.223), vec3<f32>(0.527, 0.413, 0.352), vec3<f32>(0.076, 0.058, 0.045),
    vec3<f32>(0.672, 0.604, 0.468), vec3<f32>(0.617, 0.413, 0.195), vec3<f32>(0.122, 0.112, 0.080), vec3<f32>(0.413, 0.122, 0.065),
    vec3<f32>(0.262, 0.065, 0.034), vec3<f32>(0.305, 0.262, 0.223), vec3<f32>(0.238, 0.205, 0.171), vec3<f32>(0.058, 0.061, 0.076),
    vec3<f32>(0.133, 0.114, 0.102), vec3<f32>(0.468, 0.423, 0.361), vec3<f32>(0.552, 0.539, 0.503), vec3<f32>(0.195, 0.150, 0.112));

const SAND: vec3<f32> = vec3<f32>(0.56, 0.48, 0.34);
const SNOW: vec3<f32> = vec3<f32>(0.85, 0.89, 0.94);
const LAPSE: f32 = 0.0065;   // K a metre

// Value noise, −1…1 (the viewer's dnoise: smooth, on the integer lattice).
fn gm_hash(p: vec3<f32>) -> f32 {
    var q = fract(p * 0.3183099 + vec3<f32>(0.1, 0.1, 0.1));
    q = q * 17.0;
    return fract(q.x * q.y * q.z * (q.x + q.y + q.z)) * 2.0 - 1.0;
}

// Noise that repeats with the ground's coordinate: `q` (the caller's `micro`) wraps every
// MICRO_PERIOD m (the engine's), so an octave of wavelength `lam` (a power of two, at
// most MICRO_PERIOD) has its lattice wrapped to MICRO_PERIOD / lam cells, or it jumps on the wrap's
// planes (which drew ~4 km squares). `seed` picks an independent pattern (in the hash, not the
// position). `wrap`: which axes wrap (1) or not (0).

fn gm_cell(i: vec3<f32>, per: vec3<f32>, wrap: vec3<f32>, seed: f32) -> f32 {
    let w = select(i, i - per * floor(i / per), wrap > vec3<f32>(0.5));
    return gm_hash(w + vec3<f32>(seed * 13.1, seed * 7.3, seed * 3.7));
}

fn gm_pnoise_w(q: vec3<f32>, lam: f32, seed: f32, wrap: vec3<f32>) -> f32 {
    let x = q / lam;
    let per = vec3<f32>(round(MICRO_PERIOD / lam));
    let i = floor(x);
    var f = fract(x);
    f = f * f * (3.0 - 2.0 * f);
    return mix(mix(mix(gm_cell(i, per, wrap, seed), gm_cell(i + vec3<f32>(1.0, 0.0, 0.0), per, wrap, seed), f.x),
                   mix(gm_cell(i + vec3<f32>(0.0, 1.0, 0.0), per, wrap, seed), gm_cell(i + vec3<f32>(1.0, 1.0, 0.0), per, wrap, seed), f.x), f.y),
               mix(mix(gm_cell(i + vec3<f32>(0.0, 0.0, 1.0), per, wrap, seed), gm_cell(i + vec3<f32>(1.0, 0.0, 1.0), per, wrap, seed), f.x),
                   mix(gm_cell(i + vec3<f32>(0.0, 1.0, 1.0), per, wrap, seed), gm_cell(i + vec3<f32>(1.0, 1.0, 1.0), per, wrap, seed), f.x), f.y), f.z);
}

fn gm_pnoise(q: vec3<f32>, lam: f32, seed: f32) -> f32 {
    return gm_pnoise_w(q, lam, seed, vec3<f32>(1.0));
}

// An octave of wavelength lam at this point of the ground: the absolute position (no repeat) for
// lam ≥ 64 m, the wrapped one (exact close up) below.
fn gm_at(i: GroundIn, lam: f32, seed: f32) -> f32 {
    if (lam >= 64.0) {
        return gm_pnoise_w(i.qw, lam, seed, vec3<f32>(0.0));
    }
    return gm_pnoise(i.q, lam, seed);
}

// Noise kept to what a pixel can show: an octave of wavelength `lam` fades out below ~3 pixels.
fn gm_fade(lam: f32, pixel_m: f32, pixels: f32) -> f32 {
    return clamp(lam / (pixel_m * pixels) - 1.0, 0.0, 1.0);
}

fn ground_material(i: GroundIn) -> vec3<f32> {
    let h = i.h_m;
    let land = step(0.0, h);
    var gcol = select(i.ground, i.ground_soft, i.surface_on > 0.5);
    // (Edges broken by noise from ~2 km to ~16 m, so the rules don't draw contour lines.)
    var n1 = 0.0;
    var a = 0.5;
    var lam = 2048.0;
    for (var o = 0; o < 8; o++) {
        n1 += a * gm_fade(lam, i.pixel_m, 4.0) * gm_at(i, lam, f32(o) * 5.7);
        lam *= 0.5;
        a *= 0.6;
    }
    let t_year = i.t_sea_c - LAPSE * max(h, 0.0);
    let wet = i.wet * i.surface_on;
    // Plants as a mosaic, not a wash: woodland and open ground in patches from ~2 km to ~100 m,
    // woodland favouring hollows and wet ground, in the proportion the climate gives; crowns at
    // ~25 m. Wet valley floors greener, even in dry country.
    {
        let warm = clamp((t_year + 2.0) / 6.0, 0.0, 1.0);
        var cover = clamp((i.rain_m - 0.2) / 0.8, 0.0, 1.0) * clamp((t_year + 4.0) / 10.0, 0.0, 1.0);
        cover = cover + (1.0 - cover) * wet * 0.7 * warm;
        var forest = clamp((i.rain_m - 0.7) / 0.7, 0.0, 1.0) * clamp((t_year + 2.0) / 8.0, 0.0, 1.0);
        forest = clamp(forest + (wet - 0.3) * 0.6 * i.surface_on * warm, 0.0, 1.0);
        gcol = mix(gcol, gcol * vec3<f32>(0.72, 0.9, 0.66), wet * 0.55 * warm);
        let amt = cover * (0.5 + forest * (1.0 - forest) * 2.0) + (1.0 - cover) * 0.3;
        var pn = 0.0;
        var pa = 0.5;
        var pl = 2048.0;
        var pw = 0.0;
        for (var o = 0; o < 6; o++) {
            // (Octaves fade below 4 pixels: under 2 they sparkled as the view moved.)
            pn += pa * gm_fade(pl, i.pixel_m, 4.0) * gm_at(i, pl, 3.1 + f32(o) * 1.7);
            pw += pa;
            pl *= 0.5;
            pa *= 0.62;
        }
        pn = pn / pw * 2.2 - 0.35 * clamp(i.rel, -1.0, 1.0) + (forest - 0.5) * 1.4 * cover;
        // (The woodland edge's softness from the pixel's size, not fwidth: this runs inside the
        // caller's branch, where derivatives are undefined in WGSL.)
        let ew = clamp(0.08 + i.pixel_m / 60.0, 0.08, 1.0);
        let wood = smoothstep(-ew, ew, pn);
        let crown = gm_pnoise(i.q, 32.0, 0.0) * gm_fade(32.0, i.pixel_m, 3.0);
        let open_c = gcol * (1.0 + 0.3 * amt);
        let wood_c = gcol * (1.0 - 0.32 * amt) * mix(vec3<f32>(1.0), vec3<f32>(0.9, 1.03, 0.88), cover) * (1.0 + 0.2 * crown * cover);
        gcol = mix(open_c, wood_c, wood);
    }
    // Rock on steep and high ground, in its unit's colour, banded faintly by its beds.
    let rock_c = select(ROCK[min(i.unit, 19u)], i.rock_c, dot(i.rock_c, vec3<f32>(1.0)) > 0.0);
    var rock_w = clamp(clamp((h - 1800.0) / 1500.0, 0.0, 1.0) * 0.6 + clamp((i.slope - 0.15) / 0.35, 0.0, 1.0) * 0.7 + 0.35 * n1, 0.0, 1.0);
    rock_w = max(rock_w, i.bare * i.surface_on * 0.95);
    // (Strata: by height (not wrapped), varying along the ground over ~2 km.)
    let band = gm_pnoise_w(vec3<f32>(h * 2048.0 / 60.0, i.qw.x, i.qw.z), 2048.0, 9.0, vec3<f32>(0.0));
    var col = mix(gcol, rock_c * 0.62 * (1.0 + 0.18 * band + 0.25 * n1), rock_w * land);
    // Scree: lighter broken rock below the steep ground, grained.
    let grain = gm_pnoise(i.q, 32.0, 1.0) * gm_fade(32.0, i.pixel_m, 3.0);
    col = mix(col, rock_c * 0.8 * (1.0 + 0.3 * grain + 0.2 * n1), i.scree * i.surface_on * 0.75 * land);
    // Sand on low, gentle coasts, where it isn't frozen.
    // (A beach is the strip the waves reach: a few metres above the sea, tens to a couple of
    // hundred metres wide; its rule by height gave kilometres of tan on flat coasts. Seen from far,
    // a pixel holds only its share of beach: faded by ~150 m against the pixel, so the strip
    // neither swells to fill a pixel nor sparkles as the view moves.)
    let beach_h = 2.5 + 1.5 * n1;
    let sand = (1.0 - smoothstep(beach_h * 0.5, beach_h, h)) * smoothstep(0.0, 0.3, h) * clamp(1.0 - i.slope / 0.06, 0.0, 1.0)
        * clamp(t_year / 4.0, 0.0, 1.0) * clamp(150.0 / max(i.pixel_m, 1.0), 0.0, 1.0);
    col = mix(col, SAND, 0.7 * sand);
    // Snow where the year is cold at this height, off the steepest ground.
    // (Snow slides off rock too steep to hold it: the bake's bare-rock field (its 600 m slopes)
    // shows dark faces through a snowfield, as real ranges do.)
    let snow = clamp((-t_year - 2.0) / 4.0 + 0.6 * n1, 0.0, 1.0) * clamp((0.9 - i.slope) / 0.4, 0.0, 1.0)
        * (1.0 - 0.85 * clamp(i.bare * i.surface_on * 1.5, 0.0, 1.0)) * land;
    col = mix(col, SNOW, snow);
    return col;
}
