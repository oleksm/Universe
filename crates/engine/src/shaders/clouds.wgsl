// A world's clouds (the planet lab's): where they are, of which kind, as its climate has them
// month by month (planet-sim stages/clouds.py, baked by tools/clouds_bake.py), with its El Niño,
// moving with its winds, lit by its sun, seen through its air. The reference these rules follow,
// and the look they were worked out on: planet-sim tools/cloud_reference.py.
//
// - Three shells: low at the cloud base (the condensation level, ~0.4 km over the sea, up to ~3 km
//   over deserts), frontal at 4 km, high (anvils, cirrus) 2 km under the tropopause. Each holds its
//   kinds' sky fractions for the month, blended between months by the year's phase, plus El
//   Niño's response × the world's index.
// - Shapes by kind; coverage so the clouded area equals the fraction at every zoom (the noise
//   equalised by its spread; octaves under 1.5 pixels fade out and the spread is renormalised):
//   low as cumulus (~3 km clouds, even at large scales) where the cover is low, stratocumulus
//   (~40 km cells in mesoscale patterns) where high; deep convection in ~25 km cells clustered
//   at ~600 km; fronts in bands drawn out along the wind; cirrus streaked along the jet.
// - Moving with the wind (700 hPa; the jet for the high shell), each octave renewed over its
//   lifetime ∝ its size^⅔ (Kolmogorov: a 1,000 km system ~3 days, a 1 km cumulus ~30 min), the
//   noise of two phases crossfaded (never the clouds: their edges stay crisp as they change).
// - Light: optical depth by kind × thickness (thin at the edges, ~1.5 spreads in at full); a
//   plane-parallel cloud's reflectance R = (1−g)τ / (2 + (1−g)τ), g = 0.85 (two-stream), its
//   diffuse transmission 1 − R: bright tops, dark bases under thick cloud; thin edges scatter
//   forward toward the sun (silver linings); the sky's light on the bases; shadows on the ground.
// - Seen through the air: each cloud's light carried to the eye by the air's march (the bake's
//   tables), so far clouds haze and redden as the ground does.
//
// Entries (the caller binds the bake's three textures and passes the world's frame):
// - clouds_over(c, eye, d, t_end, ...) -> vec3: the light c seen along the ray eye + t·d at t_end
//   (the ground, or 1e30 for the sky) with the clouds in front of it composited over it.
// - clouds_shadow(p, ...) -> f32: the share of the sun's direct light reaching the ground at p.
//
// Status: draft on `planet`, validated with naga; the plumbing is the integrator's.

struct Clouds {
    // The year's phase in months, 0 ≤ month < 12 (the world's own months: its year / 12).
    month: f32,
    // El Niño's index now (clouds.json "enso": the series, a value a world month; 0 without one).
    enso: f32,
    // Seconds of world time, wrapped by the caller every CLOUD_WRAP_S (f32 keeps ~1 s there).
    time_s: f32,
    // 0: off; else the clouds.json format's version (1: the air map's A is the jet's wind; 2: the
    // water the clouds can hold).
    on: f32,
};

const CLOUD_WRAP_S: f32 = 1048576.0;
const CLOUD_G: f32 = 0.85;
const CLOUD_LIFE_1000KM: f32 = 259200.0;
const CLOUD_TILE: vec2<i32> = vec2<i32>(360, 180);
const CLOUD_VALUE_SD: f32 = 0.185;
const CLOUD_CELL_SD: f32 = 0.17;
const CLOUD_CELL_MEAN: f32 = 0.482;
// The kinds' optical depths (clouds.json "kinds").
const TAU_LOW: f32 = 10.0;
const TAU_DEEP: f32 = 40.0;
const TAU_FRONTAL: f32 = 20.0;
const TAU_CIRRUS: f32 = 0.4;
const MID_SHELL_M: f32 = 4000.0;
// How much of the climate's cloud is drawn (the fractions × it): the game's taste, not the
// climate's (it keeps its own figures); a uniform later.
const CLOUD_AMOUNT: f32 = 0.75;
// Quality knobs (planet-sim tools/worth_it.py sweeps them: cost against perceptual difference):
// CLOUD_BAND: octaves finer than this many pixels are left out (higher: cheaper, softer);
// CLOUD_RELIEF: the sunlit/shaded relief sample (0: off).
const CLOUD_BAND: f32 = 1.5;
const CLOUD_RELIEF: f32 = 1.0;
// Debug: 1 draws the clouds' cost (noise evaluations a pixel) as a heatmap instead of the image:
// blue none … green ~16 … yellow ~64 … red 256+ (log scale).
const CLOUD_HEATMAP: f32 = 0.0;
var<private> cl_cost: f32 = 0.0;

struct CloudField {
    frac: vec4<f32>,  // low, deep, frontal, cirrus
    lcl_m: f32,
    trop_m: f32,
    u700: f32,
    ujet: f32,
    // The water the clouds can hold (0–1): the kinds' optical depths × it (format 2; 1 before).
    water: f32,
};

// --- The bake's maps --------------------------------------------------------------------------

fn cl_latlon(dir: vec3<f32>) -> vec2<f32> {
    // (The world maps' own convention: world_uv.)
    return vec2<f32>(asin(clamp(dir.y, -1.0, 1.0)), atan2(-dir.z, dir.x));
}

// One month's tile read bilinearly at (lat, lon) radians, wrapping in longitude inside the tile.
fn cl_tile(t: texture_2d<f32>, m: i32, ll: vec2<f32>) -> vec4<f32> {
    let o = vec2<i32>((m % 4) * CLOUD_TILE.x, (m / 4) * CLOUD_TILE.y);
    let x = (ll.y / 6.2831853 + 0.5) * f32(CLOUD_TILE.x) - 0.5;
    let y = clamp((0.5 - ll.x / 3.1415927) * f32(CLOUD_TILE.y) - 0.5, 0.0, f32(CLOUD_TILE.y - 1));
    let x0 = i32(floor(x));
    let y0 = i32(floor(y));
    let fx = x - floor(x);
    let fy = y - floor(y);
    let xa = ((x0 % CLOUD_TILE.x) + CLOUD_TILE.x) % CLOUD_TILE.x;
    let xb = (xa + 1) % CLOUD_TILE.x;
    let yb = min(y0 + 1, CLOUD_TILE.y - 1);
    let a = mix(textureLoad(t, o + vec2<i32>(xa, y0), 0), textureLoad(t, o + vec2<i32>(xb, y0), 0), fx);
    let b = mix(textureLoad(t, o + vec2<i32>(xa, yb), 0), textureLoad(t, o + vec2<i32>(xb, yb), 0), fx);
    return mix(a, b, fy);
}

fn cloud_field(dir: vec3<f32>, cl: Clouds, cm: texture_2d<f32>, ce: texture_2d<f32>, ca: texture_2d<f32>) -> CloudField {
    // (The lookup jittered by ~1.5° of smooth noise: read straight, the 1° cells' bilinear
    // contours drew stepped edges from afar, where the fine noise has faded.)
    let wq = dir * 24.0;
    let jit = vec2<f32>(cl_value(wq) - 0.5, cl_value(wq + vec3<f32>(41.0, 17.0, 5.0)) - 0.5) * 0.052;
    let ll = cl_latlon(dir) + vec2<f32>(jit.x, jit.y / max(cos(asin(clamp(dir.y, -1.0, 1.0))), 0.2));
    let m0 = i32(floor(cl.month)) % 12;
    let m1 = (m0 + 1) % 12;
    let f = fract(cl.month);
    let fr = mix(cl_tile(cm, m0, ll), cl_tile(cm, m1, ll), f);
    let en = mix(cl_tile(ce, m0, ll), cl_tile(ce, m1, ll), f) - vec4<f32>(128.0 / 255.0);
    let air = mix(cl_tile(ca, m0, ll), cl_tile(ca, m1, ll), f) * 255.0;
    var out: CloudField;
    out.frac = clamp(fr + cl.enso * en, vec4<f32>(0.0), vec4<f32>(1.0)) * CLOUD_AMOUNT;
    out.lcl_m = air.r * 16.0;
    out.trop_m = air.g * 80.0;
    out.u700 = (air.b - 128.0) * 0.5;
    if (cl.on > 1.5) {
        // (Format 2: the jet ~2.5× the 700 hPa wind: the thermal wind grows with height.)
        out.ujet = 2.5 * out.u700;
        out.water = clamp(air.a / 255.0, 0.02, 1.0);
    } else {
        out.ujet = air.a - 128.0;
        out.water = 1.0;
    }
    return out;
}

// --- Noise ------------------------------------------------------------------------------------

fn cl_hash(p: vec3<f32>) -> f32 {
    var q = fract(p * vec3<f32>(0.1031, 0.1030, 0.0973));
    q += dot(q, q.yxz + 33.33);
    return fract((q.x + q.y) * q.z);
}

fn cl_value(x: vec3<f32>) -> f32 {
    let i = floor(x);
    var f = fract(x);
    f = f * f * (3.0 - 2.0 * f);
    return mix(mix(mix(cl_hash(i), cl_hash(i + vec3<f32>(1.0, 0.0, 0.0)), f.x),
                   mix(cl_hash(i + vec3<f32>(0.0, 1.0, 0.0)), cl_hash(i + vec3<f32>(1.0, 1.0, 0.0)), f.x), f.y),
               mix(mix(cl_hash(i + vec3<f32>(0.0, 0.0, 1.0)), cl_hash(i + vec3<f32>(1.0, 0.0, 1.0)), f.x),
                   mix(cl_hash(i + vec3<f32>(0.0, 1.0, 1.0)), cl_hash(i + vec3<f32>(1.0, 1.0, 1.0)), f.x), f.y), f.z);
}

// Worley: 1 − the distance to the nearest feature point (bright cell centres).
fn cl_cells(x: vec3<f32>) -> f32 {
    let i = floor(x);
    var best = 9.0;
    for (var dz = -1; dz <= 1; dz++) {
        for (var dy = -1; dy <= 1; dy++) {
            for (var dx = -1; dx <= 1; dx++) {
                let c = i + vec3<f32>(f32(dx), f32(dy), f32(dz));
                let fp = c + vec3<f32>(cl_hash(c), cl_hash(c + vec3<f32>(17.1, 0.0, 0.0)), cl_hash(c + vec3<f32>(0.0, 31.7, 0.0)));
                best = min(best, length(x - fp));
            }
        }
    }
    return clamp(1.0 - best, 0.0, 1.0);
}

// A fixed rotation per octave (about a different axis each, by an irrational angle).
fn cl_turn(o: i32) -> mat3x3<f32> {
    let a = 0.9 + f32(o) * 2.39996;
    let ax = normalize(vec3<f32>(sin(f32(o) * 1.7 + 0.3), cos(f32(o) * 2.3 + 1.1), sin(f32(o) * 0.7 + 2.0)));
    let c = cos(a);
    let s = sin(a);
    let t = 1.0 - c;
    return mat3x3<f32>(
        vec3<f32>(t * ax.x * ax.x + c, t * ax.x * ax.y + s * ax.z, t * ax.x * ax.z - s * ax.y),
        vec3<f32>(t * ax.x * ax.y - s * ax.z, t * ax.y * ax.y + c, t * ax.y * ax.z + s * ax.x),
        vec3<f32>(t * ax.x * ax.z + s * ax.y, t * ax.y * ax.z - s * ax.x, t * ax.z * ax.z + c));
}

struct Spectrum {
    top_km: f32,
    octaves: i32,
    gain: f32,
    stretch: f32,
    cell_km: f32,  // the one octave drawn as cells (0: none)
};

// A kind's noise at q (km, body-fixed), centred, over (x) and its spread (y) over the octaves a
// pixel of `pix_km` shows; each octave carried east by `wind` (m/s) and renewed over its lifetime.
fn cl_shape(sp: Spectrum, q: vec3<f32>, east: vec3<f32>, wind: f32, t: f32, pix_km: f32, seed: f32) -> vec2<f32> {
    var lam = sp.top_km;
    var a = 1.0;
    var n = 0.0;
    var v2 = 0.0;
    for (var o = 0; o < sp.octaves; o++) {
        let f = clamp(lam / (CLOUD_BAND * pix_km) - 1.0, 0.0, 1.0);
        if (f <= 0.0) {
            break;
        }
        let is_cell = sp.cell_km > 0.0 && abs(log2(lam / sp.cell_km)) < 0.5;
        let life = CLOUD_LIFE_1000KM * pow(lam / 1000.0, 2.0 / 3.0);
        let ph = t / life;
        var acc = 0.0;
        var w2 = 0.0;
        for (var k = 0; k < 2; k++) {
            let kk = f32(k) * 0.5;
            let fr = fract(ph + kk);
            let w = 1.0 - abs(2.0 * fr - 1.0);
            // (A phase near the end of its crossfade adds next to nothing: skipped, half the noise
            // most of the time.)
            if (w < 0.12) {
                continue;
            }
            // (Each renewal and each phase a fresh pattern; offsets kept bounded for f32.)
            // (Each renewal, phase and octave a fresh pattern: an offset in the octave's own cells,
            // small. Added in km before, up to ~60,000 km, it took the fine octaves' digits away in
            // f32: they stepped, and the triangles showed it as facets.)
            let renew = (floor(ph + kk) % 61.0) * 7.31 + kk * 13.7 + seed * 3.1 + f32(o) * 5.3;
            var x = q - east * (wind * fr * life * 0.001);
            if (lam > 100.0) {
                x = vec3<f32>(x.x / sp.stretch, x.y, x.z / sp.stretch);
            }
            // (Each octave turned its own way: value noise and cells on one cubic lattice line up
            // with its axes, the clouds' square bias.)
            let z = cl_turn(o) * (x / lam) + vec3<f32>(renew, renew * 0.61, renew * 1.37);
            var v: f32;
            cl_cost += select(1.0, 27.0 / 8.0, is_cell);
            if (is_cell) {
                v = cl_cells(z) - CLOUD_CELL_MEAN;
            } else {
                v = cl_value(z) - 0.5;
            }
            acc += w * v;
            w2 += w * w;
        }
        let sd = select(CLOUD_VALUE_SD, CLOUD_CELL_SD, is_cell);
        n += a * f * acc / sqrt(max(w2, 1e-6));
        v2 += (a * f * sd) * (a * f * sd);
        lam *= 0.5;
        a *= sp.gain;
    }
    return vec2<f32>(n, sqrt(max(v2, 1e-12)));
}

// (density, thickness) where the equalised noise z = n/sd beats the fraction's threshold.
fn cl_cover(z: f32, frac: f32, soft: f32) -> vec2<f32> {
    let u = 1.0 / (1.0 + exp(-1.702 * z));
    let d = clamp((u - (1.0 - frac) + soft) / (2.0 * soft), 0.0, 1.0);
    let z_thr = log(max(1.0 - frac, 1e-4) / max(frac, 1e-4)) / 1.702;
    let core = clamp((z - z_thr) / 1.5, 0.0, 1.0);
    // (And ±35% with the noise itself: under an overcast every point is a core, and a real
    // deck's base is mottled, its depth varying two- to threefold from cell to cell.)
    // (Thickness falls to nothing at a cloud's edge, and its core is thick: edges translucent wisps,
    // cores bright, as real clouds' optical depth runs from ~0 at their edges to tens inside; a
    // floor there drew every edge as an opaque cut-out.)
    return vec2<f32>(d, max(pow(core, 1.2), 0.02) * 1.5 * (0.65 + 0.35 * clamp(z / 1.5, -1.0, 1.0)));
}

// Each shell's (density, optical depth) at body-fixed direction dir: [low, mid, high].
struct CloudCols {
    low: vec2<f32>,
    mid: vec2<f32>,
    high: vec2<f32>,
};

fn cl_east(dir: vec3<f32>) -> vec3<f32> {
    let e = vec3<f32>(dir.z, 0.0, -dir.x);
    return e / max(length(e), 1e-6);
}

fn cloud_columns(dir: vec3<f32>, f: CloudField, radius_km: f32, t: f32, pix_km: f32, which: i32) -> CloudCols {
    let q = dir * radius_km;
    let east = cl_east(dir);
    var out = CloudCols(vec2<f32>(0.0), vec2<f32>(0.0), vec2<f32>(0.0));
    // (which: 0 all, 1 low, 2 mid, 3 high: a shell's own hit needs only its own.)
    var d_deep = vec2<f32>(0.0);
    if (f.frac.y > 0.002 && (which == 0 || which == 1 || which == 3)) {
        let s = cl_shape(Spectrum(600.0, 8, 0.75, 1.0, 25.0), q, east, f.u700, t, pix_km, 3.0);
        d_deep = cl_cover(s.x / s.y, f.frac.y, 0.04);
    }
    if (which == 0 || which == 1) {
        var low = vec2<f32>(0.0);
        if (f.frac.x > 0.002) {
            // Cumulus where the cover is low, stratocumulus where high.
            // (Cumulus fields too take a share of the large structure (clusters, lines, clear
            // gaps between, ~50–400 km), or from orbit they were an even sprinkle of flakes.)
            let w = max(clamp((f.frac.x - 0.35) / 0.3, 0.0, 1.0), 0.45);
            var z = 0.0;
            var sd = 0.0;
            if (w < 1.0) {
                let c = cl_shape(Spectrum(12.0, 6, 0.8, 1.0, 3.0), q, east, f.u700, t, pix_km, 2.0);
                z += (1.0 - w) * c.x / c.y;
                sd += (1.0 - w) * (1.0 - w);
            }
            if (w > 0.0) {
                let s = cl_shape(Spectrum(400.0, 9, 0.78, 1.0, 40.0), q, east, f.u700, t, pix_km, 9.0);
                z += w * s.x / s.y;
                sd += w * w;
            }
            low = cl_cover(z / sqrt(max(sd, 1e-6)), f.frac.x, 0.04);
        }
        // (Under the towers: the convection's own dark base.)
        out.low = vec2<f32>(max(low.x, d_deep.x), (low.x * low.y * 2.0 * TAU_LOW + d_deep.x * TAU_DEEP * 0.5) * f.water);
    }
    if ((which == 0 || which == 2) && f.frac.z > 0.002) {
        let s = cl_shape(Spectrum(2000.0, 8, 0.7, 3.0, 0.0), q, east, f.u700, t, pix_km, 4.0);
        let c = cl_cover(s.x / s.y, f.frac.z, 0.08);
        out.mid = vec2<f32>(c.x, c.x * c.y * 2.0 * TAU_FRONTAL * f.water);
    }
    if (which == 0 || which == 3) {
        var ci = vec2<f32>(0.0);
        if (f.frac.w > 0.002) {
            let s = cl_shape(Spectrum(1000.0, 8, 0.75, 8.0, 0.0), q, east, f.ujet, t, pix_km, 5.0);
            ci = cl_cover(s.x / s.y, f.frac.w, 0.12);
        }
        out.high = vec2<f32>(1.0 - (1.0 - d_deep.x) * (1.0 - ci.x),
                             (d_deep.x * d_deep.y * 2.0 * TAU_DEEP + ci.x * ci.y * 2.0 * TAU_CIRRUS) * f.water);
    }
    return out;
}

// --- Light ------------------------------------------------------------------------------------

// A cloud sheet's light toward the eye per unit sun (rgb `sun`, the engine's units), and its
// opacity: `sunside` when the eye is on the sheet's sunlit side.
fn cl_sheet(tau: f32, mu_sun: f32, mu_view: f32, sunside: bool, cos_phase: f32, sun: vec3<f32>, sky: vec3<f32>) -> vec4<f32> {
    let t = (1.0 - CLOUD_G) * tau;
    let r = t / (2.0 + t);
    let lit = max(mu_sun, 0.0);
    var l = select(1.0 - r, r, sunside) * lit;
    // Thin edges scatter forward toward the sun (single scattering, Henyey–Greenstein g 0.6).
    let g2 = 0.6;
    let hg = (1.0 - g2 * g2) / (4.0 * 3.1415927 * pow(1.0 + g2 * g2 - 2.0 * g2 * cos_phase, 1.5));
    let single = (1.0 - exp(-tau / max(abs(mu_view), 0.05))) * exp(-tau / max(lit, 0.05)) * hg * 3.1415927 * lit;
    let opacity = clamp(1.0 - exp(-0.5 * tau / max(abs(mu_view), 0.08)), 0.0, 1.0);
    // (The sky lights the cloud too: its base never black.)
    let skyl = sky * (0.5 + 0.5 * r);
    return vec4<f32>(sun * (l + 0.6 * single) + skyl, opacity);
}

// The ray's hits on a sphere of radius rs about the origin, from o along d: (near, far), negative
// where none.
fn cl_sphere(o: vec3<f32>, d: vec3<f32>, rs: f32) -> vec2<f32> {
    let b = dot(o, d);
    let c = dot(o, o) - rs * rs;
    let h = b * b - c;
    if (h < 0.0) {
        return vec2<f32>(-1.0);
    }
    let s = sqrt(h);
    return vec2<f32>(-b - s, -b + s);
}

// --- The low clouds as volumes, near the eye ------------------------------------------------------
// Within VOL_RANGE of the eye, while it is within VOL_NEAR of their layer, the low clouds are
// marched as volumes (Schneider 2015's way, on the same fields): where the coverage says cloud, a
// column from the condensation level up to its top (cumulus 0.5–1.5 km deep where the cover is
// low, stratocumulus 0.3–0.4 km where high; its thickness the column's), flat-based, round-topped,
// its edges eroded by 3D noise into billows; extinction so the column's optical depth is the
// sheet's. Light: the sun through the cloud above each point (Beer), with a few octaves of
// multiple scattering (Wrenninge 2013: each fainter, wider, less forward), a two-lobed phase
// (forward 0.8, back −0.3), and the sky's light, dimmer toward the base. Beyond, the sheet, the two
// crossfaded over the range's last 40%.
const VOL_RANGE_M: f32 = 10000.0;
const VOL_NEAR_M: f32 = 6000.0;
const VOL_STEPS: i32 = 64;
// (Off until its shapes pass on screen: bases still ragged, far steps blocky. The sheets draw
// the low clouds meanwhile.)
const VOL_ON: bool = false;

fn cl_hg(c: f32, g: f32) -> f32 {
    return (1.0 - g * g) / (4.0 * 3.1415927 * pow(max(1.0 + g * g - 2.0 * g * c, 1e-4), 1.5));
}

// A low cloud's depth (m) by its cover: cumulus deep, stratocumulus thin.
fn cl_depth_m(frac_low: f32) -> f32 {
    return mix(1500.0, 400.0, clamp((frac_low - 0.35) / 0.3, 0.0, 1.0));
}

// The volume's light (premultiplied rgb) and opacity along o + t·d, s0 ≤ t ≤ s1 (o from the world's
// centre, m), the fields `f` (read once: they vary over ~100 km), faded out toward VOL_RANGE_M.
fn cl_volume(o: vec3<f32>, d: vec3<f32>, s0: f32, s1: f32, f: CloudField, to_body: mat3x3<f32>, R: f32, t: f32,
             sun_dir: vec3<f32>, sun: vec3<f32>, sky: vec3<f32>, pixel_angle: f32) -> vec4<f32> {
    let depth = cl_depth_m(f.frac.x);
    let base = f.lcl_m;
    let cos_phase = dot(d, sun_dir);
    let phase = mix(cl_hg(cos_phase, -0.3), cl_hg(cos_phase, 0.8), 0.7);
    var acc = vec3<f32>(0.0);
    var tr = 1.0;
    // (Steps closer together near the eye; jittered per pixel against banding.)
    let jit = cl_hash(d * 913.7 + vec3<f32>(t * 0.001));
    var prev = s0;
    for (var i = 0; i < VOL_STEPS; i++) {
        let x = (f32(i) + jit) / f32(VOL_STEPS);
        let tt = s0 + (s1 - s0) * x * x;
        let ds = max(tt - prev, 1.0);
        prev = tt;
        let p = o + d * tt;
        let r = length(p);
        let h = r - R - base;
        if (h < 0.0 || h > depth) {
            continue;
        }
        let up = p / r;
        let dir_b = to_body * up;
        // The column: its cover and thickness from the low shape at this step's own scale.
        let pix_km = max(tt * pixel_angle, ds) * 0.001;
        let col = cloud_columns(dir_b, f, R * 0.001, t, pix_km, 1).low;
        if (col.x <= 0.01) {
            continue;
        }
        let th = clamp(col.y / (2.0 * TAU_LOW), 0.05, 1.0);
        // (Domes: the column as tall as it is thick, low at a cloud's edge, its full depth at the
        // core; extruded to a fixed height it stood in walls.)
        let top = depth * clamp(pow(th * 1.4, 0.8), 0.04, 1.0);
        let hr = h / top;
        if (hr > 1.0) {
            continue;
        }
        // Flat base, round top; billows from 3D noise (~250 m and ~80 m), faded below a pixel.
        var dens = col.x * smoothstep(0.0, 0.03, hr) * (1.0 - smoothstep(0.55, 1.0, hr));
        let q = dir_b * (R + h + base) * 0.001;
        let b1 = cl_value(q / 0.25) * clamp(250.0 / (pix_km * 1500.0) - 1.0, 0.0, 1.0);
        let b2 = cl_value(q / 0.08 + vec3<f32>(7.1)) * clamp(80.0 / (pix_km * 1500.0) - 1.0, 0.0, 1.0);
        // (Bases flat and sharp at the condensation level, as cumulus are: the billows eat the
        // sides and tops only.)
        dens = clamp(dens - (0.45 * b1 + 0.2 * b2) * (1.0 - dens * 0.6) * smoothstep(0.1, 0.4, hr), 0.0, 1.0);
        if (dens <= 0.0) {
            continue;
        }
        // Extinction: the column's optical depth spread over its height.
        let sigma = dens * 2.0 * TAU_LOW * th / top;
        let fade = 1.0 - smoothstep(0.6 * VOL_RANGE_M, VOL_RANGE_M, tt);
        let sg = sigma * fade;
        // The sun through the cloud above, toward it.
        let mu_s = dot(up, sun_dir);
        let tau_s = sigma * (top - h) / max(mu_s, 0.1);
        var ms = 0.0;
        var a_ = 1.0;
        var b_ = 1.0;
        var c_ = 1.0;
        for (var k = 0; k < 3; k++) {
            ms += a_ * exp(-b_ * tau_s) * mix(cl_hg(cos_phase, -0.3 * c_), cl_hg(cos_phase, 0.8 * c_), 0.7);
            a_ *= 0.5;
            b_ *= 0.4;
            c_ *= 0.5;
        }
        // (A cloud's body is bright all through: light scattered many times inside it, ~its
        // albedo of the sun near the top, less toward the base, the steps too coarse to find the
        // thin lit skin themselves.)
        let body = max(mu_s, 0.0) * 0.55 * (0.3 + 0.7 * hr);
        let lit = sun * (ms * 3.1415927 + body) * step(0.0, mu_s) + sky * (0.35 + 0.65 * hr);
        let st = exp(-sg * ds);
        acc += tr * lit * (1.0 - st);
        tr *= st;
        if (tr < 0.01) {
            break;
        }
    }
    return vec4<f32>(acc, 1.0 - tr);
}

// The light along the ray eye + t·d (world frame, m) as far as t_end, with the clouds in front
// composited over `c`, the light at t_end. `center`: the world's centre (world frame, m);
// `to_body`: world frame to the world's body-fixed one (its maps'); `sun_dir`, `sun`: as the air's;
// `pixel_angle`: a pixel's angular size (rad); a: the world's air; tl, ml, smp: its tables.
fn clouds_over(c: vec3<f32>, eye: vec3<f32>, d: vec3<f32>, t_end: f32, center: vec3<f32>, to_body: mat3x3<f32>,
               sun_dir: vec3<f32>, sun: vec3<f32>, pixel_angle: f32, cl: Clouds, a: Air,
               cm: texture_2d<f32>, ce: texture_2d<f32>, ca: texture_2d<f32>,
               tl: texture_2d<f32>, ml: texture_2d<f32>, smp: sampler) -> vec3<f32> {
    if (cl.on < 0.5) {
        return c;
    }
    let R = a.radius_m;
    let o = eye - center;
    // The shells' heights: the fields under the eye, or where the ray meets the ground (from
    // orbit), one refinement (a shell is a sphere; its height varies only slowly).
    var probe = normalize(o);
    let g = cl_sphere(o, d, R);
    if (g.x > 0.0) {
        probe = normalize(o + d * g.x);
    }
    let f0 = cloud_field(to_body * probe, cl, cm, ce, ca);
    // (The cloud base is the condensation level over the ground under the clouds, not over the
    // sea: where the ground drawn here stands higher, the low shell stands that much higher, or
    // it ran under the terrain and was cut along its triangles.)
    var ground_h = 0.0;
    if (t_end < 1e29) {
        ground_h = max(length(o + d * t_end) - R, 0.0);
    }
    var radii = array<f32, 3>(R + ground_h + f0.lcl_m, R + max(MID_SHELL_M, ground_h + 1500.0), R + max(f0.trop_m - 2000.0, max(5000.0, ground_h + 3000.0)));
    // Every hit in front of t_end: up to two a shell, sorted near to far.
    var ts = array<f32, 8>(0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0);
    var ks = array<i32, 8>(0, 0, 0, 0, 0, 0, 0, 0);
    var n = 0;
    // The low layer as a volume near the eye (kind 3: one event at its entry, s0 … s1).
    let eye_h = length(o) - R;
    let vol = VOL_ON && cl.on > 0.5 && abs(eye_h - f0.lcl_m) < VOL_NEAR_M + cl_depth_m(f0.frac.x) && f0.frac.x > 0.01;
    var s0 = 0.0;
    var s1 = 0.0;
    if (vol) {
        let lo = cl_sphere(o, d, R + f0.lcl_m);
        let hi = cl_sphere(o, d, R + f0.lcl_m + cl_depth_m(f0.frac.x));
        let lim = min(t_end, VOL_RANGE_M);
        // (The ray's stretch inside the slab between the two spheres, from the eye, as far as lim.)
        if (eye_h > f0.lcl_m + cl_depth_m(f0.frac.x)) {
            s0 = max(hi.x, 0.0);
            s1 = select(hi.y, lo.x, lo.x > 0.0);
        } else if (eye_h < f0.lcl_m) {
            s0 = max(lo.y, 0.0);
            s1 = hi.y;
        } else {
            s0 = 0.0;
            s1 = select(hi.y, lo.x, lo.x > 0.0);
        }
        s1 = min(s1, lim);
        if (hi.x < 0.0 && hi.y < 0.0) {
            s1 = s0;
        }
        if (s1 > s0) {
            ts[n] = s0;
            ks[n] = 3;
            n++;
        }
    }
    for (var k = 0; k < 3; k++) {
        let h = cl_sphere(o, d, radii[k]);
        if (h.x > 0.0 && h.x < t_end) {
            ts[n] = h.x;
            ks[n] = k;
            n++;
        }
        if (h.y > 0.0 && h.y < t_end) {
            ts[n] = h.y;
            ks[n] = k;
            n++;
        }
    }
    for (var i = 1; i < n; i++) {
        var j = i;
        while (j > 0 && ts[j - 1] > ts[j]) {
            let tt = ts[j];
            ts[j] = ts[j - 1];
            ts[j - 1] = tt;
            let kk = ks[j];
            ks[j] = ks[j - 1];
            ks[j - 1] = kk;
            j--;
        }
    }
    // Front to back: the clouds' light, each carried to the eye through the air between.
    var acc = vec3<f32>(0.0);
    var trans = 1.0;
    let t = cl.time_s;
    let cos_phase = dot(d, sun_dir);
    // (The sky's light on the clouds: marched only once a cloud is met.)
    var sky_up = vec3<f32>(0.0);
    var sky_ready = false;
    // (The air between the eye and the clouds, marched once at the first cloud met and used for
    // all: the shells lie a few km apart, the air's march was the costliest part, once a hit.)
    var air_ready = false;
    var air_in = vec3<f32>(0.0);
    var air_tr = vec3<f32>(1.0);
    for (var i = 0; i < n; i++) {
        let p = o + d * ts[i];
        let up = normalize(p);
        let dir_b = to_body * up;
        let f = cloud_field(dir_b, cl, cm, ce, ca);
        let mu_v = dot(up, -d);
        let pix_km = max(ts[i] * pixel_angle / max(abs(mu_v), 0.05), 0.001) * 0.001;
        let k = ks[i];
        if (k == 3) {
            // The volume, carried to the eye through the air as a whole (at its middle): its light
            // dimmed by the air's transmission, the air's own light in front of it by its opacity.
            if (!sky_ready) {
                sky_up = air_sky_lut(normalize(o), center, sun_dir, sun, a, tl, ml, smp);
                sky_ready = true;
            }
            let v = cl_volume(o, d, s0, s1, f, to_body, R, t, sun_dir, sun, sky_up, pixel_angle);
            if (v.w > 0.001) {
                let pm = o + d * (0.5 * (s0 + s1)) + center;
                let air_in = air_ground_lut(vec3<f32>(0.0), pm, center, sun_dir, sun, a, tl, ml, smp);
                let air_tr = air_ground_lut(vec3<f32>(1.0), pm, center, sun_dir, sun, a, tl, ml, smp) - air_in;
                acc += trans * (v.rgb * air_tr + v.w * air_in);
                trans *= 1.0 - v.w;
                if (trans < 0.01) {
                    break;
                }
            }
            continue;
        }
        // (The low sheet gives way to the volume over the volume's range.)
        var keep = 1.0;
        if (k == 0 && vol) {
            keep = smoothstep(0.6 * VOL_RANGE_M, VOL_RANGE_M, ts[i]);
            if (keep <= 0.0) {
                continue;
            }
        }
        let cols = cloud_columns(dir_b, f, R * 0.001, t, pix_km, k + 1);
        var dt = cols.low;
        if (k == 1) {
            dt = cols.mid;
        } else if (k == 2) {
            dt = cols.high;
        }
        if (dt.x <= 0.001) {
            continue;
        }
        let mu_s = dot(up, sun_dir);
        let sunside = (mu_v > 0.0) == (mu_s > 0.0);
        if (!sky_ready) {
            sky_up = air_sky_lut(normalize(o), center, sun_dir, sun, a, tl, ml, smp);
            sky_ready = true;
        }
        var sheet = cl_sheet(dt.y, mu_s, mu_v, sunside, cos_phase, sun, sky_up);
        // Relief: the cloud a little toward the sun (its height's worth): thicker there, this point
        // is on its shaded flank; thinner, a sunlit face. Bright tops, shaded sides, as cumulus
        // and frontal towers look from above (low and frontal shells; none from far orbit, where
        // the relief is under a pixel).
        // (Faded in as the pixel shrinks past 3 km → 1.5 km: switched on at once, it popped.)
        let relief_w = smoothstep(3.0, 1.5, pix_km);
        if (CLOUD_RELIEF > 0.5 && k == 0 && sunside && relief_w > 0.0 && dt.x > 0.05) {
            let sh = normalize(sun_dir - up * mu_s);
            let qn = normalize(up + sh * (1500.0 / R));
            // (At a quarter of the detail: the relief's shading needs only the cloud's bulk.)
            let cols2 = cloud_columns(to_body * qn, f, R * 0.001, t, max(pix_km * 4.0, 0.5), k + 1);
            var d2 = cols2.low;
            if (k == 1) {
                d2 = cols2.mid;
            }
            let rel = clamp((dt.y - d2.y) / max(dt.y + d2.y, 1.0), -1.0, 1.0);
            sheet = vec4<f32>(sheet.rgb * mix(1.0, clamp(1.0 + 0.6 * rel, 0.55, 1.3), relief_w), sheet.w);
        }
        // (A hit just in front of the ground fades in: a shell grazing the terrain blends, never
        // cuts it at its triangles' edges.)
        let near_ground = select(1.0, clamp((t_end - ts[i]) / 300.0, 0.0, 1.0), t_end < 1e29);
        let alpha = sheet.w * dt.x * keep * near_ground;
        // (A cloud passes about 1 − R of the light behind it, diffusely (non-absorbing: two-stream):
        // over bright ground — ice, snow, deserts — the ground's light shows through and the cloud
        // reads as bright as it or brighter, as from orbit, not as a grey sheet over it.)
        let tt_ = (1.0 - CLOUD_G) * dt.y;
        let t_dif = 1.0 - tt_ / (2.0 + tt_);
        // (The air between the eye and the cloud: its light dimmed and hazed as the ground's.)
        if (!air_ready) {
            air_in = air_ground_lut(vec3<f32>(0.0), p + center, center, sun_dir, sun, a, tl, ml, smp);
            air_tr = air_ground_lut(vec3<f32>(1.0), p + center, center, sun_dir, sun, a, tl, ml, smp) - air_in;
            air_ready = true;
        }
        let seen = sheet.rgb * air_tr + air_in;
        acc += trans * alpha * seen;
        trans *= 1.0 - alpha * (1.0 - t_dif);
        if (trans < 0.01) {
            break;
        }
    }
    if (CLOUD_HEATMAP > 0.5) {
        // (Log scale: blue 0, green ~16, yellow ~64, red 256+ evaluations.)
        let x = clamp(log2(1.0 + cl_cost) / 8.0, 0.0, 1.0);
        return vec3<f32>(smoothstep(0.4, 0.9, x), smoothstep(0.0, 0.4, x) * (1.0 - smoothstep(0.7, 1.0, x)), 1.0 - smoothstep(0.0, 0.35, x)) * 2.0;
    }
    return acc + trans * c;
}

// The share of the sun's direct light reaching the ground at p (world frame, m): its path to the
// sun through each shell, the clouds' direct transmission (exp(−τ/μ)) plus part of their diffuse
// light (a cloud dims the ground under it, it doesn't black it out).
fn clouds_shadow(p: vec3<f32>, center: vec3<f32>, to_body: mat3x3<f32>, sun_dir: vec3<f32>, pix_km: f32, cl: Clouds,
                 a: Air, cm: texture_2d<f32>, ce: texture_2d<f32>, ca: texture_2d<f32>) -> f32 {
    if (cl.on < 0.5) {
        return 1.0;
    }
    let R = a.radius_m;
    let o = p - center;
    let up = normalize(o);
    let mu = dot(up, sun_dir);
    if (mu <= 0.0) {
        return 1.0;
    }
    let f0 = cloud_field(to_body * up, cl, cm, ce, ca);
    var radii = array<f32, 3>(R + f0.lcl_m, R + MID_SHELL_M, R + max(f0.trop_m - 2000.0, 5000.0));
    var shade = 1.0;
    // (The low and frontal shells: cirrus casts next to nothing.)
    for (var k = 0; k < 2; k++) {
        let h = cl_sphere(o, sun_dir, radii[k]);
        if (h.y <= 0.0) {
            continue;
        }
        let q = normalize(o + sun_dir * h.y);
        let dir_b = to_body * q;
        let f = cloud_field(dir_b, cl, cm, ce, ca);
        // (Shadows need no detail finer than ~1 km: cheaper, and a pixel's size taken per triangle
        // of the ground no longer shows in them as facets.)
        let cols = cloud_columns(dir_b, f, R * 0.001, cl.time_s, max(pix_km, 2.0), k + 1);
        var dt = cols.low;
        if (k == 1) {
            dt = cols.mid;
        } else if (k == 2) {
            dt = cols.high;
        }
        let tau = dt.y;
        let direct = exp(-tau / max(mu, 0.1));
        let tt = (1.0 - CLOUD_G) * tau;
        let diffuse = 1.0 - tt / (2.0 + tt);
        shade *= mix(1.0, direct + 0.7 * (diffuse - direct), dt.x);
    }
    return shade;
}

// --- The cloud cache (the cost: noise per pixel, per shell, per frame was ~6 ms at 1080p) ----------
// The clouds change over minutes, the view every frame: their cover and depth are written into
// textures about the camera, three nested levels a shell (≈ 8,000, 800 and 80 km across at
// 512², texels ~16 km, 1.6 km, 160 m), by a compute pass refreshed a little a frame; the shading
// reads them (a few texture taps) and computes only what is finer than the inner level's texel,
// near the ground. Layout: a 2D array texture, layer = level · 3 + shell (shells: 0 low, 1 frontal,
// 2 high), rgba16float: R cover (0–1), G optical depth.
//
// The plumbing (the integrator's): the texture (512² × 9, storage-write for the pass, sampled
// with a linear clamping sampler for the shading), the CloudCache uniform, and the pass:
// cloud_cache_fill over (512/8, 512/8, layers to refresh) — every layer a few frames apart is
// plenty (the clouds move ~10 m/s: the inner level's texel in ~15 s); a level re-centred (its
// center, e1, e2 rebuilt) when the camera has moved an eighth of its width from it.

const CC_N: i32 = 512;
const CC_LEVELS: i32 = 3;

struct CloudCache {
    // The levels' common centre (body-fixed unit direction) and its tangent frame.
    center: vec4<f32>,
    e1: vec4<f32>,
    e2: vec4<f32>,
    // Each level's half-width (m, along the tangent plane: gnomonic), x: level 0 (outer) … z: 2.
    half_m: vec4<f32>,
    // Each level's crossfade from its old copy (the second texture) to its new one (the first),
    // 0 → 1 over ~1–2 s after a refresh or re-centring: changes come in gradually, never at once.
    // (The levels' old centre and frame, for the old copy: below.)
    fade: vec4<f32>,
    old_center: vec4<f32>,
    old_e1: vec4<f32>,
    old_e2: vec4<f32>,
    // w of center: the world's radius (m); w of e1: 1 when the cache holds data.
};

// A body-fixed direction to the levels' gnomonic plane (m), or a huge value behind it.
fn cc_project(dir: vec3<f32>, cc: CloudCache) -> vec2<f32> {
    let c = dot(dir, cc.center.xyz);
    if (c <= 0.05) {
        return vec2<f32>(1e12);
    }
    return vec2<f32>(dot(dir, cc.e1.xyz), dot(dir, cc.e2.xyz)) / c * cc.center.w;
}

fn cc_unproject(xy: vec2<f32>, cc: CloudCache) -> vec3<f32> {
    return normalize(cc.center.xyz + (cc.e1.xyz * xy.x + cc.e2.xyz * xy.y) / cc.center.w);
}

// The shading's read: (cover, optical depth) of `shell` at `dir` for a pixel of `pix_km`, from
// the finest level that holds the place at a texel no finer than the pixel (bilinear; blended
// into the next level out over its last tenth); (−1, −1) where no level holds it (the caller
// computes it directly then, at the pixel's band: from far away, a few octaves only).
fn cc_read_one(dir: vec3<f32>, shell: i32, pix_km: f32, frame: CloudCache, tex: texture_2d_array<f32>, smp: sampler, use_level: vec3<f32>) -> vec2<f32> {
    let xy = cc_project(dir, frame);
    var out = vec2<f32>(-1.0);
    var used_km = 1e9;
    for (var level = 0; level < CC_LEVELS; level++) {
        let half = frame.half_m[level];
        let a = max(abs(xy.x), abs(xy.y)) / half;
        if (a >= 1.0) {
            continue;
        }
        let texel_km = 2.0 * half / f32(CC_N) * 0.001;
        if (texel_km < pix_km * 0.5 && out.x >= 0.0) {
            break;
        }
        used_km = texel_km;
        let uv = (xy / half) * 0.5 + 0.5;
        let v = textureSampleLevel(tex, smp, uv, level * 3 + shell, 0.0).xy;
        let w = clamp((1.0 - a) / 0.1, 0.0, 1.0);
        out = select(v, mix(out, v, w), out.x >= 0.0);
    }
    if (out.x >= 0.0 && used_km > 2.0 * pix_km && pix_km < 0.08) {
        return vec2<f32>(-1.0);
    }
    return out;
}

// The shading's read: (cover, optical depth) of `shell` at `dir` for a pixel of `pix_km`, from
// the finest level that holds the place at a texel no finer than the pixel needs (bilinear; blended
// into the next level out over its last tenth), its new copy (`tex`) crossfaded from its old
// (`tex_old`) by the levels' fade; (−1, −1) where no level holds it, or closer to the clouds than
// the inner level's texel (the caller computes it directly then).
fn cloud_cache_read(dir: vec3<f32>, shell: i32, pix_km: f32, cc: CloudCache, tex: texture_2d_array<f32>,
                    tex_old: texture_2d_array<f32>, smp: sampler) -> vec2<f32> {
    if (cc.e1.w < 0.5) {
        return vec2<f32>(-1.0);
    }
    let now = cc_read_one(dir, shell, pix_km, cc, tex, smp, vec3<f32>(1.0));
    // (The fade the place's level is in: the finest level holding it, as a scalar.)
    let xy = cc_project(dir, cc);
    var fade = 1.0;
    for (var level = 0; level < CC_LEVELS; level++) {
        if (max(abs(xy.x), abs(xy.y)) < cc.half_m[level]) {
            fade = cc.fade[level];
        }
    }
    if (fade >= 0.999 || now.x < 0.0) {
        return now;
    }
    var old = cc;
    old.center = cc.old_center;
    old.e1 = vec4<f32>(cc.old_e1.xyz, 1.0);
    old.e2 = cc.old_e2;
    let before = cc_read_one(dir, shell, pix_km, old, tex_old, smp, vec3<f32>(1.0));
    if (before.x < 0.0) {
        return now;
    }
    return mix(before, now, smoothstep(0.0, 1.0, fade));
}


// --- The same, reading the cloud cache (switch to these once the cache pass runs) -------------

fn clouds_over_cached(c: vec3<f32>, eye: vec3<f32>, d: vec3<f32>, t_end: f32, center: vec3<f32>, to_body: mat3x3<f32>,
               sun_dir: vec3<f32>, sun: vec3<f32>, pixel_angle: f32, cl: Clouds, a: Air,
               cm: texture_2d<f32>, ce: texture_2d<f32>, ca: texture_2d<f32>,
               tl: texture_2d<f32>, ml: texture_2d<f32>, smp: sampler,
                      cc: CloudCache, cct: texture_2d_array<f32>, cct_old: texture_2d_array<f32>, ccs: sampler) -> vec3<f32> {
    if (cl.on < 0.5) {
        return c;
    }
    let R = a.radius_m;
    let o = eye - center;
    // The shells' heights: the fields under the eye, or where the ray meets the ground (from
    // orbit), one refinement (a shell is a sphere; its height varies only slowly).
    var probe = normalize(o);
    let g = cl_sphere(o, d, R);
    if (g.x > 0.0) {
        probe = normalize(o + d * g.x);
    }
    let f0 = cloud_field(to_body * probe, cl, cm, ce, ca);
    // (The cloud base is the condensation level over the ground under the clouds, not over the
    // sea: where the ground drawn here stands higher, the low shell stands that much higher, or
    // it ran under the terrain and was cut along its triangles.)
    var ground_h = 0.0;
    if (t_end < 1e29) {
        ground_h = max(length(o + d * t_end) - R, 0.0);
    }
    var radii = array<f32, 3>(R + ground_h + f0.lcl_m, R + max(MID_SHELL_M, ground_h + 1500.0), R + max(f0.trop_m - 2000.0, max(5000.0, ground_h + 3000.0)));
    // Every hit in front of t_end: up to two a shell, sorted near to far.
    var ts = array<f32, 8>(0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0);
    var ks = array<i32, 8>(0, 0, 0, 0, 0, 0, 0, 0);
    var n = 0;
    // The low layer as a volume near the eye (kind 3: one event at its entry, s0 … s1).
    let eye_h = length(o) - R;
    let vol = VOL_ON && cl.on > 0.5 && abs(eye_h - f0.lcl_m) < VOL_NEAR_M + cl_depth_m(f0.frac.x) && f0.frac.x > 0.01;
    var s0 = 0.0;
    var s1 = 0.0;
    if (vol) {
        let lo = cl_sphere(o, d, R + f0.lcl_m);
        let hi = cl_sphere(o, d, R + f0.lcl_m + cl_depth_m(f0.frac.x));
        let lim = min(t_end, VOL_RANGE_M);
        // (The ray's stretch inside the slab between the two spheres, from the eye, as far as lim.)
        if (eye_h > f0.lcl_m + cl_depth_m(f0.frac.x)) {
            s0 = max(hi.x, 0.0);
            s1 = select(hi.y, lo.x, lo.x > 0.0);
        } else if (eye_h < f0.lcl_m) {
            s0 = max(lo.y, 0.0);
            s1 = hi.y;
        } else {
            s0 = 0.0;
            s1 = select(hi.y, lo.x, lo.x > 0.0);
        }
        s1 = min(s1, lim);
        if (hi.x < 0.0 && hi.y < 0.0) {
            s1 = s0;
        }
        if (s1 > s0) {
            ts[n] = s0;
            ks[n] = 3;
            n++;
        }
    }
    for (var k = 0; k < 3; k++) {
        let h = cl_sphere(o, d, radii[k]);
        if (h.x > 0.0 && h.x < t_end) {
            ts[n] = h.x;
            ks[n] = k;
            n++;
        }
        if (h.y > 0.0 && h.y < t_end) {
            ts[n] = h.y;
            ks[n] = k;
            n++;
        }
    }
    for (var i = 1; i < n; i++) {
        var j = i;
        while (j > 0 && ts[j - 1] > ts[j]) {
            let tt = ts[j];
            ts[j] = ts[j - 1];
            ts[j - 1] = tt;
            let kk = ks[j];
            ks[j] = ks[j - 1];
            ks[j - 1] = kk;
            j--;
        }
    }
    // Front to back: the clouds' light, each carried to the eye through the air between.
    var acc = vec3<f32>(0.0);
    var trans = 1.0;
    let t = cl.time_s;
    let cos_phase = dot(d, sun_dir);
    // (The sky's light on the clouds: marched only once a cloud is met.)
    var sky_up = vec3<f32>(0.0);
    var sky_ready = false;
    // (The air between the eye and the clouds, marched once at the first cloud met and used for
    // all: the shells lie a few km apart, the air's march was the costliest part, once a hit.)
    var air_ready = false;
    var air_in = vec3<f32>(0.0);
    var air_tr = vec3<f32>(1.0);
    for (var i = 0; i < n; i++) {
        let p = o + d * ts[i];
        let up = normalize(p);
        let dir_b = to_body * up;
        let f = cloud_field(dir_b, cl, cm, ce, ca);
        let mu_v = dot(up, -d);
        let pix_km = max(ts[i] * pixel_angle / max(abs(mu_v), 0.05), 0.001) * 0.001;
        let k = ks[i];
        if (k == 3) {
            // The volume, carried to the eye through the air as a whole (at its middle): its light
            // dimmed by the air's transmission, the air's own light in front of it by its opacity.
            if (!sky_ready) {
                sky_up = air_sky_lut(normalize(o), center, sun_dir, sun, a, tl, ml, smp);
                sky_ready = true;
            }
            let v = cl_volume(o, d, s0, s1, f, to_body, R, t, sun_dir, sun, sky_up, pixel_angle);
            if (v.w > 0.001) {
                let pm = o + d * (0.5 * (s0 + s1)) + center;
                let air_in = air_ground_lut(vec3<f32>(0.0), pm, center, sun_dir, sun, a, tl, ml, smp);
                let air_tr = air_ground_lut(vec3<f32>(1.0), pm, center, sun_dir, sun, a, tl, ml, smp) - air_in;
                acc += trans * (v.rgb * air_tr + v.w * air_in);
                trans *= 1.0 - v.w;
                if (trans < 0.01) {
                    break;
                }
            }
            continue;
        }
        // (The low sheet gives way to the volume over the volume's range.)
        var keep = 1.0;
        if (k == 0 && vol) {
            keep = smoothstep(0.6 * VOL_RANGE_M, VOL_RANGE_M, ts[i]);
            if (keep <= 0.0) {
                continue;
            }
        }
        // (From the cache where it holds the place; else computed here.)
        var dt = cloud_cache_read(dir_b, k, pix_km, cc, cct, cct_old, ccs);
        if (dt.x < 0.0) {
            let cols = cloud_columns(dir_b, f, R * 0.001, t, pix_km, k + 1);
            dt = cols.low;
            if (k == 1) {
                dt = cols.mid;
            } else if (k == 2) {
                dt = cols.high;
            }
        }
        if (dt.x <= 0.001) {
            continue;
        }
        let mu_s = dot(up, sun_dir);
        let sunside = (mu_v > 0.0) == (mu_s > 0.0);
        if (!sky_ready) {
            sky_up = air_sky_lut(normalize(o), center, sun_dir, sun, a, tl, ml, smp);
            sky_ready = true;
        }
        var sheet = cl_sheet(dt.y, mu_s, mu_v, sunside, cos_phase, sun, sky_up);
        // Relief: the cloud a little toward the sun (its height's worth): thicker there, this point
        // is on its shaded flank; thinner, a sunlit face. Bright tops, shaded sides, as cumulus
        // and frontal towers look from above (low and frontal shells; none from far orbit, where
        // the relief is under a pixel).
        // (Faded in as the pixel shrinks past 3 km → 1.5 km: switched on at once, it popped.)
        let relief_w = smoothstep(3.0, 1.5, pix_km);
        if (CLOUD_RELIEF > 0.5 && k == 0 && sunside && relief_w > 0.0 && dt.x > 0.05) {
            let sh = normalize(sun_dir - up * mu_s);
            let qn = normalize(up + sh * (1500.0 / R));
            // (At a quarter of the detail: the relief's shading needs only the cloud's bulk.)
            var d2 = cloud_cache_read(to_body * qn, k, max(pix_km * 4.0, 0.5), cc, cct, cct_old, ccs);
            if (d2.x < 0.0) {
                let cols2 = cloud_columns(to_body * qn, f, R * 0.001, t, max(pix_km * 4.0, 0.5), k + 1);
                d2 = cols2.low;
                if (k == 1) {
                    d2 = cols2.mid;
                }
            }
            let rel = clamp((dt.y - d2.y) / max(dt.y + d2.y, 1.0), -1.0, 1.0);
            sheet = vec4<f32>(sheet.rgb * mix(1.0, clamp(1.0 + 0.6 * rel, 0.55, 1.3), relief_w), sheet.w);
        }
        // (A hit just in front of the ground fades in: a shell grazing the terrain blends, never
        // cuts it at its triangles' edges.)
        let near_ground = select(1.0, clamp((t_end - ts[i]) / 300.0, 0.0, 1.0), t_end < 1e29);
        let alpha = sheet.w * dt.x * keep * near_ground;
        // (A cloud passes about 1 − R of the light behind it, diffusely (non-absorbing: two-stream):
        // over bright ground — ice, snow, deserts — the ground's light shows through and the cloud
        // reads as bright as it or brighter, as from orbit, not as a grey sheet over it.)
        let tt_ = (1.0 - CLOUD_G) * dt.y;
        let t_dif = 1.0 - tt_ / (2.0 + tt_);
        // (The air between the eye and the cloud: its light dimmed and hazed as the ground's.)
        if (!air_ready) {
            air_in = air_ground_lut(vec3<f32>(0.0), p + center, center, sun_dir, sun, a, tl, ml, smp);
            air_tr = air_ground_lut(vec3<f32>(1.0), p + center, center, sun_dir, sun, a, tl, ml, smp) - air_in;
            air_ready = true;
        }
        let seen = sheet.rgb * air_tr + air_in;
        acc += trans * alpha * seen;
        trans *= 1.0 - alpha * (1.0 - t_dif);
        if (trans < 0.01) {
            break;
        }
    }
    if (CLOUD_HEATMAP > 0.5) {
        // (Log scale: blue 0, green ~16, yellow ~64, red 256+ evaluations.)
        let x = clamp(log2(1.0 + cl_cost) / 8.0, 0.0, 1.0);
        return vec3<f32>(smoothstep(0.4, 0.9, x), smoothstep(0.0, 0.4, x) * (1.0 - smoothstep(0.7, 1.0, x)), 1.0 - smoothstep(0.0, 0.35, x)) * 2.0;
    }
    return acc + trans * c;
}

fn clouds_shadow_cached(p: vec3<f32>, center: vec3<f32>, to_body: mat3x3<f32>, sun_dir: vec3<f32>, pix_km: f32, cl: Clouds,
                 a: Air, cm: texture_2d<f32>, ce: texture_2d<f32>, ca: texture_2d<f32>,
                        cc: CloudCache, cct: texture_2d_array<f32>, cct_old: texture_2d_array<f32>, ccs: sampler) -> f32 {
    if (cl.on < 0.5) {
        return 1.0;
    }
    let R = a.radius_m;
    let o = p - center;
    let up = normalize(o);
    let mu = dot(up, sun_dir);
    if (mu <= 0.0) {
        return 1.0;
    }
    let f0 = cloud_field(to_body * up, cl, cm, ce, ca);
    var radii = array<f32, 3>(R + f0.lcl_m, R + MID_SHELL_M, R + max(f0.trop_m - 2000.0, 5000.0));
    var shade = 1.0;
    // (The low and frontal shells: cirrus casts next to nothing.)
    for (var k = 0; k < 2; k++) {
        let h = cl_sphere(o, sun_dir, radii[k]);
        if (h.y <= 0.0) {
            continue;
        }
        let q = normalize(o + sun_dir * h.y);
        let dir_b = to_body * q;
        let f = cloud_field(dir_b, cl, cm, ce, ca);
        // (Shadows need no detail finer than ~1 km: cheaper, and a pixel's size taken per triangle
        // of the ground no longer shows in them as facets.)
        var dt = cloud_cache_read(dir_b, k, max(pix_km, 2.0), cc, cct, cct_old, ccs);
        if (dt.x < 0.0) {
            let cols = cloud_columns(dir_b, f, R * 0.001, cl.time_s, max(pix_km, 2.0), k + 1);
            dt = cols.low;
            if (k == 1) {
                dt = cols.mid;
            } else if (k == 2) {
                dt = cols.high;
            }
        }
        let tau = dt.y;
        let direct = exp(-tau / max(mu, 0.1));
        let tt = (1.0 - CLOUD_G) * tau;
        let diffuse = 1.0 - tt / (2.0 + tt);
        shade *= mix(1.0, direct + 0.7 * (diffuse - direct), dt.x);
    }
    return shade;
}
