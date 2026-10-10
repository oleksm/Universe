struct Globals {
    // Camera-relative world -> clip (reversed-Z, infinite far plane).
    view_proj: mat4x4<f32>,
    // Low-res pixel coords -> clip.
    hud_proj: mat4x4<f32>,
    // Camera-relative world -> the shadow map's near and far cascades (the tight and ground ones below).
    shadow_near: mat4x4<f32>,
    shadow_far: mat4x4<f32>,
    // x, y: a texel of each cascade (metres); z: shadows on (1) or not;
    // w: tint what's in shadow red (checking them).
    shadow: vec4<f32>,
    // Graphics toggles (1 on): textures, normal maps, occlusion, emission; specular, planet light, tone map.
    look: vec4<f32>,
    look2: vec4<f32>,
    // The tight cascade round what's looked at; x: a texel of it (metres), y: in use.
    shadow_tight: mat4x4<f32>,
    shadow2: vec4<f32>,
    // The environment (see renderer.rs: not used here).
    env_sun: vec4<f32>,
    env_world: vec4<f32>,
    env_world_color: vec4<f32>,
    env_mode: vec4<f32>,
    env_sky: vec4<f32>,
    // x: the angle a pixel spans (radians) at the screen's middle; y: how far the world's maps
    // are faded in (0..1); z: 1 where its air's tables are bound.
    view: vec4<f32>,
    // Clip → camera-relative world.
    inv_view_proj: mat4x4<f32>,
    // The world whose maps are bound: its centre from the eye (m), its radius (m; 0: none).
    world_at: vec4<f32>,
    // The scene's frame to that world's own (its turn undone), for its clouds.
    world_to_body: mat4x4<f32>,
    // The ground cascade (the ground's patches alone, tens of kilometres round the eye);
    // shadow2.z: a texel of it (metres), shadow2.w: in use.
    shadow_ground: mat4x4<f32>,
};

@group(0) @binding(0) var<uniform> g: Globals;
@group(1) @binding(0) var shadow_map: texture_depth_2d_array;
@group(1) @binding(1) var shadow_cmp: sampler_comparison;
// Worlds' surfaces (see `GlobeMap`): height (in relief units) and crater-ness.
@group(1) @binding(2) var globe_maps: texture_cube_array<f32>;
@group(1) @binding(3) var globe_soft: sampler;
// Their own colours where they have them (alpha 0: none; the palette instead).
@group(1) @binding(6) var globe_colors: texture_cube_array<f32>;
// The full-resolution maps of the world near the eye (see `worldmaps.rs`): equirectangular,
// row 0 north, column 0 at −180° of longitude atan2(−z, x) in the world's own frame. Bound for
// the globe layer g.look2.w − 1; a texel of nothing (alpha 0) where a world lacks one.
@group(2) @binding(0) var world_color: texture_2d<f32>;
@group(2) @binding(1) var world_ground: texture_2d<f32>;
@group(2) @binding(2) var world_normal: texture_2d<f32>;
@group(2) @binding(3) var world_climate: texture_2d<f32>;
@group(2) @binding(4) var world_rock: texture_2d<f32>;
@group(2) @binding(5) var world_soft: sampler;
@group(2) @binding(6) var world_exact: sampler;
// Its air (the lab's `Air`, air.wgsl; `on` 0: none).
@group(2) @binding(7) var<uniform> world_air: Air;
// The sea's calmness (globe_spec: 1 − wind / 9 m/s).
@group(2) @binding(8) var world_spec: texture_2d<f32>;
// Its air's tables (the lab's, air.wgsl's `_lut` entries), where g.view.z is 1, and their sampler.
@group(2) @binding(9) var world_air_t: texture_2d<f32>;
@group(2) @binding(10) var world_air_ms: texture_2d<f32>;
@group(2) @binding(11) var world_air_smp: sampler;
// Its clouds (the lab's, clouds.wgsl): the maps by month, El Niño's change, the air they sit in,
// read exactly; and the clouds now (`on` 0: none).
@group(2) @binding(12) var world_cm: texture_2d<f32>;
@group(2) @binding(13) var world_ce: texture_2d<f32>;
@group(2) @binding(14) var world_ca: texture_2d<f32>;
@group(2) @binding(15) var<uniform> world_clouds: Clouds;
// Its clouds cached (see cloudcache.rs): the levels' frames and fades, the copy now and the one before.
@group(2) @binding(16) var<uniform> world_cc: CloudCache;
@group(2) @binding(17) var world_cct: texture_2d_array<f32>;
@group(2) @binding(18) var world_cct_old: texture_2d_array<f32>;

/// How many times a world drawn from its lines has its slopes steepened in its shading.
const LINES_SLOPE_SHADE: f32 = 30.0;

fn world_turn() -> mat3x3<f32> {
    return mat3x3<f32>(g.world_to_body[0].xyz, g.world_to_body[1].xyz, g.world_to_body[2].xyz);
}

// The bound world's clouds over colour `c` along view direction `d` (from the eye), to `t_end` (m).
fn world_clouds_over(c: vec3<f32>, d: vec3<f32>, t_end: f32, sun_dir: vec3<f32>, sun: vec3<f32>) -> vec3<f32> {
    if (world_air.on < 0.5 || world_clouds.on < 0.5) {
        return c;
    }
    return clouds_over_cached(c, vec3<f32>(0.0), d, t_end, g.world_at.xyz, world_turn(), sun_dir, sun, g.view.x, world_clouds, world_air, world_cm, world_ce, world_ca, world_air_t, world_air_ms, world_air_smp, world_cc, world_cct, world_cct_old, world_air_smp);
}

// The bound world's air on the ground and its sky: by its tables where it has them.
fn world_air_ground(c: vec3<f32>, p: vec3<f32>, sun_dir: vec3<f32>, sun: vec3<f32>) -> vec3<f32> {
    if (g.view.z > 0.5) {
        return air_ground_lut(c, p, g.world_at.xyz, sun_dir, sun, world_air, world_air_t, world_air_ms, world_air_smp);
    }
    return air_ground(c, p, g.world_at.xyz, sun_dir, sun, world_air);
}

fn world_air_sky(d: vec3<f32>, sun_dir: vec3<f32>, sun: vec3<f32>) -> vec3<f32> {
    if (g.view.z > 0.5) {
        return air_sky_lut(d, g.world_at.xyz, sun_dir, sun, world_air, world_air_t, world_air_ms, world_air_smp);
    }
    return air_sky(d, g.world_at.xyz, sun_dir, sun, world_air);
}

// Where on a world's maps the direction `dir` (its own frame) falls.
fn world_uv(dir: vec3<f32>) -> vec2<f32> {
    let lon = atan2(-dir.z, dir.x);
    let lat = asin(clamp(dir.y, -1.0, 1.0));
    return vec2<f32>(lon / 6.2831853 + 0.5, 0.5 - lat / 3.1415927);
}

// The mip level of a world map `width` texels round for a pixel spanning `footprint` radians.
fn world_lod(width: u32, footprint: f32) -> f32 {
    return max(log2(max(footprint * f32(width) / 6.2831853, 1e-6)), 0.0);
}

struct VertexIn {
    @location(0) pos: vec3<f32>,
    @location(1) color: vec4<f32>,
};

struct VertexOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) color: vec4<f32>,
};

@vertex
fn vs_world(v: VertexIn) -> VertexOut {
    return VertexOut(g.view_proj * vec4<f32>(v.pos, 1.0), v.color);
}

// Lines are pulled slightly toward the camera so edges lying on occluding
// faces win the depth test (reversed-Z: larger depth = nearer).
@vertex
fn vs_line(v: VertexIn) -> VertexOut {
    var clip = g.view_proj * vec4<f32>(v.pos, 1.0);
    clip.z *= 1.003;
    return VertexOut(clip, v.color);
}

// Directions at infinity: w = 0 removes translation and lands on the far plane.
@vertex
fn vs_sky(v: VertexIn) -> VertexOut {
    var clip = g.view_proj * vec4<f32>(v.pos, 0.0);
    clip.z = 0.0;
    return VertexOut(clip, v.color);
}

@vertex
fn vs_hud(v: VertexIn) -> VertexOut {
    return VertexOut(g.hud_proj * vec4<f32>(v.pos.xy, 0.0, 1.0), v.color);
}

@fragment
fn fs_color(in: VertexOut) -> @location(0) vec4<f32> {
    return in.color;
}

// Meshes: kept on the GPU, placed and lit here per instance (see
// `frame::Instance`). Flat faces (each triangle's vertices carry its normal),
// edges dimmed by the light on their ends (their direction from the center),
// the reflecting planet's light by how much of it each surface sees, both
// lights through the eye's adaptation as on the CPU before.

struct MeshIn {
    @location(0) pos: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) color: vec4<f32>,
    @location(3) c0: vec4<f32>,
    @location(4) c1: vec4<f32>,
    @location(5) c2: vec4<f32>,
    @location(6) t: vec4<f32>,
    @location(7) line_tint: vec4<f32>,
    @location(8) fill_tint: vec4<f32>,
    @location(9) light_dir: vec4<f32>,
    @location(10) light_color: vec4<f32>,
    @location(11) refl_dir: vec4<f32>,
    @location(12) refl_color: vec4<f32>,
    // x: glint (0 matte .. 1), y: its sharpness (a power), z: glow.
    @location(13) material: vec4<f32>,
    // A globe's map: layer + 1 (0 none), kind, relief (m), brightness.
    @location(14) globe: vec4<f32>,
    // Where its vertices are on the world, in radii: pos * w + xyz.
    @location(15) globe_at: vec4<f32>,
    // The model's per-vertex data (the near ground's surface fields: wet, scree, bare, read).
    @location(16) data: vec4<f32>,
};
// (A patch's origin wrapped to the fine grain's period (m) rides in c0.w, c1.w, c2.w.)

// How the eye adapts to the planet's light on a face (the planet's own `ADAPT`).
const FILL_ADAPT: f32 = 0.3;

fn view_factor(cos_b: f32, s_in: f32) -> f32 {
    let s = min(s_in, 1.0);
    let c = 1.0 - sqrt(1.0 - s);
    return s * max((cos_b + c) / (1.0 + c), 0.0);
}

// The planet's light (k2) on a surface facing `n`.
fn fill(v: MeshIn, n: vec3<f32>) -> vec3<f32> {
    var k2 = 0.0;
    if (v.refl_dir.w > 0.0) {
        k2 = max(pow(v.refl_color.w * view_factor(dot(n, v.refl_dir.xyz), v.refl_dir.w), FILL_ADAPT) - 0.12, 0.0) / 0.88;
    }
    return k2 * v.refl_color.rgb;
}

// (The sun through the shadow map: `sunlit`, in light.wgsl.)

// A mesh's face or edge, lit per pixel: its colour, the sun's light on it
// (before shadow) and the planet's, the ambient; where it is and faces.
struct MeshOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) color: vec4<f32>,
    @location(1) sun: vec3<f32>,
    @location(2) fill: vec3<f32>,
    @location(3) ambient: f32,
    @location(4) at: vec3<f32>,
    @location(5) normal: vec3<f32>,
    // The sun's direction (xyz) and its light here (rgb), unshaded; the surface.
    @location(6) sun_dir: vec3<f32>,
    @location(7) sun_light: vec3<f32>,
    @location(8) material: vec4<f32>,
    // Where on the model (its own frame), and its globe map (see MeshIn).
    @location(9) local: vec3<f32>,
    @location(10) @interpolate(flat) globe: vec4<f32>,
    // A patch of ground (its vertices in metres): 1 / the world's radius; a whole globe 1.
    @location(11) @interpolate(flat) patch_scale: f32,
    // Where on the ground for its fine grain (m, wrapped; exact).
    @location(12) micro: vec3<f32>,
    // A globe's air (see `Frame::with_air`): its world's center (camera-relative)
    // and radius (m); its shell (m, 0 none) and its depths, packed.
    @location(13) @interpolate(flat) air_center: vec4<f32>,
    @location(14) @interpolate(flat) air: vec2<f32>,
    // Straight up from the world where this is (as drawn: the eye's frame).
    @location(15) up: vec3<f32>,
    // The vertex's data, as the model gives it (the near ground's surface fields).
    @location(16) data: vec4<f32>,
};

// (The fine grain repeats every `MICRO_PERIOD` metres: written by shaders.rs, as frame.rs has it.)

// The lattice's value at `q`, wrapped every `n` cells.
fn whash(q: vec3<i32>, n: i32) -> f32 {
    return hash3(((q % n) + n) % n);
}

// Value noise in 0..1 on a lattice that wraps every `n` cells.
fn wnoise(p: vec3<f32>, n: i32) -> f32 {
    let f = floor(p);
    let t = p - f;
    let s = t * t * (3.0 - 2.0 * t);
    let i = vec3<i32>(f);
    let a = mix(whash(i, n), whash(i + vec3<i32>(1, 0, 0), n), s.x);
    let b = mix(whash(i + vec3<i32>(0, 1, 0), n), whash(i + vec3<i32>(1, 1, 0), n), s.x);
    let c = mix(whash(i + vec3<i32>(0, 0, 1), n), whash(i + vec3<i32>(1, 0, 1), n), s.x);
    let d = mix(whash(i + vec3<i32>(0, 1, 1), n), whash(i + vec3<i32>(1, 1, 1), n), s.x);
    return mix(mix(a, b, s.y), mix(c, d, s.y), s.z);
}

// The ground's fine grain (a patch's, up close): octaves from 128 m down to
// half a metre, each fading in as it grows to a few pixels across (`pixel`,
// m). x: a colour detail (-1..1); y: a height (m), the same slope at every
// scale (pebbles, ripples, hummocks); exact, from `m` (see `micro`).
fn micro_detail(m: vec3<f32>, pixel: f32) -> vec2<f32> {
    var sum = vec2<f32>(0.0);
    var amp = 0.5;
    var cells = 32;
    for (var o = 0; o < 9; o++) {
        let size = MICRO_PERIOD / f32(cells);
        let fade = clamp(size / (pixel * 6.0) - 1.0, 0.0, 1.0);
        if (fade > 0.0) {
            let v = wnoise(m / size, cells) * 2.0 - 1.0;
            let r = mix(v, 0.6 - abs(v) * 1.6, 0.5);
            sum += fade * vec2<f32>(amp * r, r * size * 0.12);
        }
        amp *= 0.6;
        cells *= 2;
    }
    return sum;
}

// Value noise in 0..1 (a hash on the lattice, smoothly blended).
fn hash3(p: vec3<i32>) -> f32 {
    var h = u32(p.x) * 73856093u ^ u32(p.y) * 19349663u ^ u32(p.z) * 83492791u;
    h = (h ^ (h >> 13u)) * 1274126177u;
    h = h ^ (h >> 16u);
    return f32(h & 0xffffffu) / 16777216.0;
}

fn vnoise(p: vec3<f32>) -> f32 {
    let f = floor(p);
    let t = p - f;
    let s = t * t * (3.0 - 2.0 * t);
    let i = vec3<i32>(f);
    let a = mix(hash3(i), hash3(i + vec3<i32>(1, 0, 0)), s.x);
    let b = mix(hash3(i + vec3<i32>(0, 1, 0)), hash3(i + vec3<i32>(1, 1, 0)), s.x);
    let c = mix(hash3(i + vec3<i32>(0, 0, 1)), hash3(i + vec3<i32>(1, 0, 1)), s.x);
    let d = mix(hash3(i + vec3<i32>(0, 1, 1)), hash3(i + vec3<i32>(1, 1, 1)), s.x);
    return mix(mix(a, b, s.y), mix(c, d, s.y), s.z);
}

// A world's own colour against the palette's brightness (the palette's colours are dim).
const OWN_COLOR: f32 = 1.0;

// Fine detail on a globe, octaves of noise down to about the pixel, each
// fading in as it grows to a few pixels across (no shimmer): x a colour
// detail (-1..1), y a height detail (in radii: the same slope at every
// scale, so ground keeps its relief however near), z a second, unrelated
// detail (moisture).
fn globe_detail(dir: vec3<f32>, footprint: f32) -> vec3<f32> {
    var sum = vec3<f32>(0.0);
    var amp = 0.5;
    var f = 40.0;
    for (var o = 0; o < 11; o++) {
        // (Slopes from octaves a few pixels across and up; colours, which
        // are cut into patches by thresholds, only from wider ones: no
        // pixel-sized edges to flicker as the eye moves.)
        let fade = clamp(1.0 / (f * footprint * 6.0) - 1.0, 0.0, 1.0);
        if (fade <= 0.0) {
            break;
        }
        let fade_color = clamp(1.0 / (f * footprint * 12.0) - 1.0, 0.0, 1.0);
        let v = vnoise(dir * f) * 2.0 - 1.0;
        // (Half ridged: sharp crests, like eroded ground.)
        let r = mix(v, 0.6 - abs(v) * 1.6, 0.5);
        let w = vnoise(dir * f + vec3<f32>(31.7, 11.3, 5.9)) * 2.0 - 1.0;
        sum += vec3<f32>(fade_color * amp * r, fade * r / f, fade_color * amp * w);
        amp *= 0.55;
        f *= 2.13;
    }
    return sum;
}

// A world's ground colour: an Earth-like one's seas darker the deeper,
// turquoise in the shallows, a crisp coast with surf, sand; its land in
// crisp-edged patches (forest, grass, dry ground, rock, snow) by moisture
// and height; others their own colour, lighter high, darker in craters,
// with outcrops; ice at the poles. `d`: the fine detail (see `globe_detail`).
fn globe_color(kind: f32, h: f32, inside: f32, base: vec3<f32>, dir: vec3<f32>, d: vec3<f32>, ragged: f32) -> vec3<f32> {
    // Palettes 3/4 are canonical previews: grayscale by measured
    // elevation in relief units, never inferred rock, vegetation or water.
    if (kind > 2.5) { return vec3<f32>(0.42 + 0.22 * clamp(h, -1.0, 1.0)); }
    var c: vec3<f32>;
    let polar = abs(dir.y);
    if (kind < 0.5) {
        let t = h + d.x * 0.03 * ragged;
        if (t < 0.0) {
            let deep = clamp(-t / 0.5, 0.0, 1.0);
            c = mix(vec3<f32>(0.1, 0.4, 0.58), vec3<f32>(0.02, 0.1, 0.3), sqrt(deep));
            c = mix(c, vec3<f32>(0.55, 0.7, 0.72), smoothstep(-0.004, 0.0, t) * 0.6);
        } else {
            // Moisture: wetter low, drier high and toward the tropics' deserts.
            let wet = d.z * 1.2 - t * 0.8 + 0.25 - 0.35 * smoothstep(0.15, 0.3, polar) * (1.0 - smoothstep(0.35, 0.5, polar));
            let forest = vec3<f32>(0.1, 0.24, 0.09);
            let grass = vec3<f32>(0.28, 0.42, 0.16);
            let dry = vec3<f32>(0.52, 0.46, 0.33);
            let rock = vec3<f32>(0.36, 0.32, 0.28);
            c = mix(dry, grass, smoothstep(-0.06, 0.02, wet));
            c = mix(c, forest, smoothstep(0.18, 0.24, wet));
            c = mix(c, rock, smoothstep(0.38, 0.44, t + d.x * 0.25));
            c = mix(c, vec3<f32>(0.93, 0.94, 0.96), smoothstep(0.66, 0.7, t + d.x * 0.2 + polar * 0.4));
            // (The beach.)
            c = mix(vec3<f32>(0.76, 0.71, 0.54), c, smoothstep(0.004, 0.012, t));
        }
    } else {
        let shade = 0.72 + 0.4 * clamp(h * 0.5 + 0.5 + d.x * 0.3, 0.0, 1.0);
        let crater = 1.0 - 0.4 * clamp(inside * 4.0 - 1.0, 0.0, 1.0);
        // (Outcrops of darker and paler ground.)
        let mottle = mix(0.9, 1.06, smoothstep(-0.35, 0.35, d.z));
        c = base * shade * crater * mottle;
    }
    c *= 1.0 + 0.22 * d.x;
    let ice = smoothstep(0.86, 0.9, polar + d.x * 0.05);
    return mix(c, vec3<f32>(0.92, 0.94, 0.97), ice);
}

// A column of air's optical depth straight up, in red, green and blue (packed
// a hundredth each in 24 bits: see `Frame::with_air`).
fn air_depth(packed: f32) -> vec3<f32> {
    let q = u32(packed);
    return vec3<f32>(f32(q >> 16u), f32((q >> 8u) & 255u), f32(q & 255u)) * 0.01;
}

// The ground at `p` (camera-relative) seen through its world's air: dimmed by
// what of the air lies between it and the eye (none above the shell, the
// eye's own share inside it), and lit by the sunlit air itself: blue straight
// down, white along the horizon and at the limb; reddened sunlight on it near
// the terminator, none at night. Colours at a globe's own brightness (`bright`).
fn through_air(c: vec3<f32>, p: vec3<f32>, center: vec3<f32>, radius: f32, air: vec2<f32>, sun_dir: vec3<f32>, sun_light: vec3<f32>, bright: f32) -> vec3<f32> {
    let shell = air.x;
    if (shell <= 0.0) {
        return c;
    }
    let beta = air_depth(air.y) / shell;
    let len = length(p);
    let d = p / max(len, 1e-6);
    // Where the line of sight comes into the shell (0 when the eye is in it).
    let top = radius + shell;
    let oc = -center;
    let b = dot(oc, d);
    let cc = dot(oc, oc) - top * top;
    var enter = 0.0;
    if (cc > 0.0) {
        let disc = b * b - cc;
        if (disc <= 0.0) {
            return c;
        }
        enter = max(-b - sqrt(disc), 0.0);
    }
    // (To the world's sphere where the line of sight meets it, not the mesh: a far globe's flat
    // triangles sag kilometres inside it at their middles, more air there than at their edges,
    // and the edges drew as a grid of lines.)
    var ground = len;
    let cg = (length(oc) - radius) * (length(oc) + radius);
    let hg = b * b - cg;
    if (cg > 0.0 && hg > 0.0 && b < 0.0) {
        ground = min(len, cg / (-b + sqrt(hg)));
    }
    let path = max(ground - enter, 0.0);
    let through = exp(-beta * path);
    // The sun on the air here: by its height in this sky, through the air's
    // own slant (reddened low); a soft dusk past the terminator.
    let up = normalize(p - center);
    let mu = dot(up, sun_dir);
    let lit = smoothstep(-0.12, 0.25, mu);
    let reddened = exp(-beta * shell * 0.5 / max(mu + 0.12, 0.06));
    let glow = (1.0 - through) * sun_light * reddened * lit * bright * AIR_GLOW;
    return c * through + glow;
}

// How bright sunlit air is, against ground of the same brightness lit straight on.
const AIR_GLOW: f32 = 0.55;

@fragment
fn fs_mesh(in: MeshOut) -> @location(0) vec4<f32> {
    // (Globe surfacing, sampled whatever the mesh: derivatives need it out
    // here, and it's cheap; used only for a globe.)
    let dir = normalize(in.local);
    let px = dpdx(in.at);
    let py = dpdy(in.at);
    let on_patch = in.patch_scale < 0.5;
    // (Where on the world is good to about a metre only (it's in radii, in
    // 32 bits): on a patch its steps across the screen come from the eye's
    // metres, which are exact.)
    let ldx = select(dpdx(in.local), px * in.patch_scale, on_patch);
    let ldy = select(dpdy(in.local), py * in.patch_scale, on_patch);
    let layer = max(i32(in.globe.x) - 1, 0);
    // How much of the world a pixel spans (radians), from the eye's metres
    // (precise) on a patch; the radius as drawn (m).
    let radius = select(length(px) / max(length(ldx) / max(length(in.local), 1e-6), 1e-9), 1.0 / in.patch_scale, on_patch);
    // (A pixel's size on a patch from the eye's distance, the angle a pixel spans and how obliquely
    // the ground is seen: smooth from triangle to triangle. The screen's derivatives are one value
    // a triangle: a map's level chosen by them steps at every edge, and the slopes shaded from it
    // drew each edge as a dark dash.)
    // (How obliquely from the world's smooth up, not the triangle's flat normal: that stepped the
    // pixel's size, the maps' level and the material's noise at every triangle edge.)
    let oblique = 1.0 / max(abs(dot(normalize(in.up), normalize(-in.at))), 0.25);
    let span = 2.0 * length(in.at) * g.view.x * oblique;
    let footprint = select((length(ldx) + length(ldy)) / max(length(in.local), 1e-6), span / radius, on_patch);
    // (The globe maps' level on a patch from that: a texel spans π/2 / GLOBE_SIZE radians.)
    let globe_lod = max(log2(footprint * GLOBE_SIZE / 1.5707963), 0.0);
    var ground: vec4<f32>;
    if (on_patch) {
        ground = textureSampleLevel(globe_maps, globe_soft, in.local, layer, globe_lod);
    } else {
        ground = textureSampleGrad(globe_maps, globe_soft, in.local, layer, ldx, ldy);
    }
    // (Fine detail, for a globe: in its colour, and in the heights its
    // slopes are shaded by — ridges and hollows at any zoom, drawing only.)
    // (A world drawn from its lines, its brightness given below zero: its slopes steepened in the
    // shading, and no made-up detail: its relief is its lines' alone.)
    let from_lines = in.globe.w < 0.0;
    let canonical = in.globe.y > 2.5;
    var d = vec3<f32>(0.0);
    if (in.globe.x > 0.5 && !from_lines && !canonical) {
        d = globe_detail(dir, footprint);
    }
    // A patch of ground: its height is its own (in relief units), exact —
    // land where it stands above the sea, the sea's depth from the map.
    // (The map's second channel: crater-ness, or below zero a port's plain.)
    let inside = max(ground.g, 0.0);
    let plain = max(-ground.g, 0.0);
    var h = ground.r;
    if (on_patch) {
        // Sea where the map says it's deep enough (smooth: no flicker on its
        // edge), never on a port's plain; else the ground's own height.
        let own = (length(in.local) - 1.0) / max(in.patch_scale * in.globe.z, 1e-12);
        let sea = in.globe.y < 0.5 && ground.r < -0.002 && plain < 0.01 && own < 0.0004;
        h = select(max(own, 0.0), min(ground.r, -0.0005), sea);
        if (canonical) { h = own; }
    }
    // (A port's plain: dry ground, no beach.)
    if (!canonical) { h = max(h, 0.03 * smoothstep(0.0, 0.3, plain)); }
    let land = select(1.0, step(0.0, select(ground.r + d.x * 0.03, h, on_patch)), in.globe.y < 0.5);
    // (Shaded the same near and far: the map's slopes and the fine detail's,
    // over whatever shape the mesh has — till a pixel is a few metres or
    // less: then where on the world is too coarse to take slopes from (they'd
    // speckle), and the mesh's own shape shades it.)
    let pixel = select(length(px) + length(py), span, on_patch);
    let slopes = select(1.0, smoothstep(2.0, 10.0, pixel), on_patch);
    // Up close on a patch, the fine grain instead (exact: it doesn't speckle).
    var grain = vec2<f32>(0.0);
    if (on_patch && in.globe.x > 0.5 && !from_lines && !canonical) {
        grain = micro_detail(in.micro, pixel) * (1.0 - slopes * 0.5);
    }
    // (The map's own slopes on a globe only: a patch stands on the true heights, finer than the
    // map, whose filtering steps (a 20 km texel placed to 1/256) drew a grid of dashes there.)
    let lift = (select(ground.r * in.globe.z, 0.0, on_patch) + land * d.y * 0.06 * radius) * slopes + land * grain.y;
    let hx = dpdx(lift);
    let hy = dpdy(lift);
    var n = normalize(in.normal);
    var albedo = in.color;
    if (in.globe.x > 0.5) {
        // Slopes from the height (surface gradient: no tangents needed).
        let r1 = cross(py, n);
        let r2 = cross(n, px);
        let det = dot(px, r1);
        if (abs(det) > 1e-12) {
            let grad = sign(det) * (hx * r1 + hy * r2);
            n = normalize(abs(det) * n - grad);
        }
        if (canonical && on_patch) {
            let tilt = cross(px, py);
            if (dot(tilt, tilt) > 1e-30) { n = normalize(tilt) * sign(dot(tilt, in.up)); }
        }
        albedo = vec4<f32>(globe_color(in.globe.y, h, inside, in.color.rgb, dir, d, select(1.0, 0.0, on_patch)) * (1.0 + 0.25 * grain.x * land) * abs(in.globe.w), in.color.a);
        // Palette 4's second map channel (patch data.x nearby) is explicit water,
        // never inferred from height. Blue is a diagnostic colour, not a material category.
        if (in.globe.y > 3.5 && select(ground.g, in.data.x, on_patch) > 0.5) {
            albedo = vec4<f32>(vec3<f32>(0.12, 0.35, 0.95) * abs(in.globe.w), in.color.a);
        }
        // A world's own colour, where it has one (grown, not painted): its globe map's, or
        // where its full-resolution maps are bound, theirs; and close up, its ground's material.
        var own: vec4<f32>;
        if (on_patch) {
            own = textureSampleLevel(globe_colors, globe_soft, in.local, layer, globe_lod);
        } else {
            own = textureSampleGrad(globe_colors, globe_soft, in.local, layer, ldx, ldy);
        }
        // Palette 5 uses the exported diagnostic rock legend only on dry/unknown-water
        // ground. Water wins at all LODs; missing labels stay on the neutral palette.
        if (in.globe.y > 4.5 && own.a > 0.5 && select(ground.g, in.data.x, on_patch) <= 0.5) {
            albedo = vec4<f32>(own.rgb * abs(in.globe.w), in.color.a);
        }
        let mapped = in.globe.x > 0.5 && abs(in.globe.x - g.look2.w) < 0.5;
        let uv = world_uv(dir);
        if (mapped && !canonical) {
            let full = textureSampleLevel(world_color, world_soft, uv, world_lod(textureDimensions(world_color).x, footprint));
            if (full.a > 0.5 && own.a > 0.5) {
                own = vec4<f32>(mix(own.rgb, full.rgb, g.view.y), 1.0);
            } else if (full.a > 0.5) {
                own = full;
            }
        }
        if (own.a > 0.5 && !canonical) {
            var rgb = own.rgb;
            // (Close up, over land: the ground's material, faded in as a pixel comes under 2.5 km.)
            let near = smoothstep(2500.0, 1250.0, pixel) * g.view.y;
            if (mapped && land > 0.5 && near > 0.0) {
                let gw = textureDimensions(world_ground).x;
                let lod = world_lod(gw, footprint);
                let ground = textureSampleLevel(world_ground, world_soft, uv, lod);
                if (ground.a > 0.5) {
                    let climate = textureSampleLevel(world_climate, world_soft, uv, 0.0);
                    let unit = u32(round(textureSampleLevel(world_rock, world_exact, uv, 0.0).r * 255.0 / 8.0));
                    // (The slope from the bake's normal map, read bilinearly: the mesh's normal is
                    // flat over each triangle, and the rock its slope uncovers drew each triangle
                    // pair as a square. The map's slopes are steepened RELIEF_SHADE = 8 times
                    // (planet-sim tools/globe_look.py): undone here.)
                    let nm = textureSampleLevel(world_normal, world_soft, uv, lod).rgb * 2.0 - vec3<f32>(1.0);
                    let tan_s = length(nm.xy) / max(nm.z, 0.05) / 8.0;
                    let c = 1.0 / sqrt(1.0 + tan_s * tan_s);
                    var gi: GroundIn;
                    gi.ground = ground.rgb;
                    gi.ground_soft = textureSampleLevel(world_ground, world_soft, uv, lod + 2.5).rgb;
                    gi.t_sea_c = climate.r * 100.0 - 50.0;
                    gi.rain_m = climate.g * 4.0;
                    gi.unit = unit;
                    // (The rock's colour blended over its four nearest texels: read nearest, each
                    // unit would show as a ~10 km square.)
                    let rd = vec2<i32>(textureDimensions(world_rock));
                    let rp = uv * vec2<f32>(rd) - vec2<f32>(0.5);
                    let r0 = vec2<i32>(floor(rp));
                    let rx0 = (r0.x % rd.x + rd.x) % rd.x;
                    let rx1 = (rx0 + 1) % rd.x;
                    let ry0 = clamp(r0.y, 0, rd.y - 1);
                    let ry1 = clamp(r0.y + 1, 0, rd.y - 1);
                    let ru = vec4<f32>(textureLoad(world_rock, vec2<i32>(rx0, ry0), 0).r, textureLoad(world_rock, vec2<i32>(rx1, ry0), 0).r,
                                       textureLoad(world_rock, vec2<i32>(rx0, ry1), 0).r, textureLoad(world_rock, vec2<i32>(rx1, ry1), 0).r);
                    let ri = vec4<u32>(round(ru * 255.0 / 8.0));
                    gi.rock_c = ground_rock_blend(ri.x, ri.y, ri.z, ri.w, fract(rp));
                    gi.h_m = h * in.globe.z;
                    gi.slope = sqrt(1.0 - c * c) / c;
                    // (The slope at the 600 m heights, per vertex: smooth, and true at that scale.)
                    if (abs(in.data.w) >= 1.0) {
                        gi.slope = abs(in.data.w) - 1.0;
                    }
                    // (The 600 m surface fields from the river tiles, per vertex on the patch.)
                    gi.wet = in.data.x;
                    gi.scree = in.data.y;
                    gi.bare = in.data.z;
                    gi.surface_on = select(0.0, 1.0, in.data.w > 0.5);
                    gi.q = in.micro;
                    gi.qw = dir * world_air.radius_m;
                    gi.pixel_m = pixel;
                    rgb = mix(rgb, ground_material(gi), near);
                }
            }
            // (A world drawn from its lines: shaded as a map shades relief, whatever the sun: each
            // pixel brighter or darker by how much more or less than flat ground it faces a light
            // from the upper left of the screen, LINES_SLOPE_SHADE times: slopes of a degree or
            // two, all its lines hold, read.)
            if (from_lines) {
                // (The slope from the ground as drawn, triangle by triangle: its lines' own shape. The
                // mesh's normals on a patch are the world's smooth up, and carry no slope.)
                let up = normalize(in.up);
                var tilt = cross(px, py);
                let right = px - up * dot(px, up);
                if (dot(tilt, tilt) > 1e-30 && dot(right, right) > 1e-30) {
                    tilt = normalize(tilt) * sign(dot(tilt, up));
                    let ahead = cross(up, normalize(right));
                    let l = normalize(up - 0.6 * normalize(right) + 0.4 * ahead);
                    rgb *= clamp(1.0 + LINES_SLOPE_SHADE * (dot(tilt, l) - dot(up, l)), 0.45, 1.8);
                }
            }
            // (A world's own colour is its true albedo: no palette brightness factor (in.globe.w).)
            albedo = vec4<f32>(rgb * (1.0 + 0.12 * d.x * land) * (1.0 + 0.25 * grain.x * land) * OWN_COLOR, in.color.a);
        }
    }
    let seen = sunlit(in.at, n);
    // (A globe lit per pixel: its slopes, its terminator.)
    var sun = in.sun;
    if (in.globe.x > 0.5) {
        sun = max(dot(n, in.sun_dir), 0.0) * in.sun_light;
        // (The bound world's clouds shading its ground.)
        if (abs(in.globe.x - g.look2.w) < 0.5 && world_air.on > 0.5 && world_clouds.on > 0.5) {
            sun *= clouds_shadow_cached(in.at, g.world_at.xyz, world_turn(), in.sun_dir, pixel * 0.001, world_clouds, world_air, world_cm, world_ce, world_ca, world_cc, world_cct, world_cct_old, world_air_smp);
        }
    }
    let light = min(sun * seen + in.fill, vec3<f32>(MAX_LIGHT));
    if (g.shadow.w > 0.0 && seen < 0.5 && max(sun.r, max(sun.g, sun.b)) > 0.0) {
        return vec4<f32>(0.8, 0.0, 0.0, in.color.a);
    }
    var c = albedo.rgb * (vec3<f32>(in.ambient) + (1.0 - in.ambient) * light);
    // The sun's glint: where the surface turns the light to the eye (Blinn).
    if (in.material.x > 0.0 && dot(n, in.sun_dir) > 0.0) {
        let h = normalize(in.sun_dir + normalize(-in.at));
        c += in.sun_light * in.material.x * pow(max(dot(n, h), 0.0), in.material.y) * seen;
    }
    // What glows of itself (windows, lamps, hot metal).
    c += albedo.rgb * in.material.z;
    // The sea of the world whose maps and air are bound: the lab's (sea.wgsl), its water by depth,
    // the sky reflected, the sun's glint by the wind; in place of the sea's albedo and glint, faded
    // in with the maps.
    if (in.globe.x > 0.5 && abs(in.globe.x - g.look2.w) < 0.5 && world_air.on > 0.5 && land < 0.5) {
        let calm = textureSampleLevel(world_spec, world_soft, world_uv(dir), world_lod(textureDimensions(world_spec).x, footprint));
        // (Open water only: the map is 0 on land and on sea ice, and 0.35..1 on open water. A
        // one-channel map reads alpha 1 everywhere, so not that.)
        if (calm.r > 0.1) {
            let up = normalize(in.up);
            var si: SeaIn;
            si.depth_m = max(-ground.r * in.globe.z, 0.5);
            si.wind_ms = 9.0 * (1.0 - clamp(calm.r, 0.35, 1.0));
            si.to_eye = normalize(-in.at);
            si.up = up;
            si.sun_dir = in.sun_dir;
            si.sun = in.sun_light * seen;
            // (The sky's light along the reflected ray and straight up, as seen from the eye: near
            // the sea, near enough; the lab's tables will look from the water itself.)
            si.sky = world_air_sky(reflect(-si.to_eye, up), in.sun_dir, in.sun_light);
            si.down = si.sun * max(dot(up, in.sun_dir), 0.0) + world_air_sky(up, in.sun_dir, in.sun_light) * 1.5707963;
            si.q = in.micro;
            si.pixel_m = pixel;
            c = mix(c, sea_material(si), g.view.y);
        }
    }
    if (in.globe.x > 0.5) {
        // The world whose maps are bound, through its own air (the lab's scattering); others
        // through the plain haze.
        if (abs(in.globe.x - g.look2.w) < 0.5 && world_air.on > 0.5) {
            c = world_air_ground(c, in.at, in.sun_dir, in.sun_light);
            c = world_clouds_over(c, normalize(in.at), length(in.at), in.sun_dir, in.sun_light);
        } else {
            c = through_air(c, in.at, in.air_center.xyz, in.air_center.w, in.air, in.sun_dir, in.sun_light, abs(in.globe.w));
        }
    }
    return vec4<f32>(c, albedo.a);
}

// The air's light from beyond everything drawn, the world in view's (the lab's `air_sky`): a
// triangle over the whole screen, at the far plane (reversed: 0), added to what's behind.
struct SkyOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) ndc: vec2<f32>,
};

@vertex
fn vs_air_sky(@builtin(vertex_index) i: u32) -> SkyOut {
    let p = vec2<f32>(f32((i << 1u) & 2u), f32(i & 2u)) * 2.0 - 1.0;
    return SkyOut(vec4<f32>(p, 0.0, 1.0), p);
}

@fragment
fn fs_air_sky(in: SkyOut) -> @location(0) vec4<f32> {
    if (world_air.on < 0.5 || g.world_at.w <= 0.0) {
        discard;
    }
    let q = g.inv_view_proj * vec4<f32>(in.ndc, 0.5, 1.0);
    let d = normalize(q.xyz / q.w);
    let sun_dir = normalize(g.env_sun.xyz);
    let sky = world_air_sky(d, sun_dir, vec3<f32>(g.env_sun.w));
    return vec4<f32>(world_clouds_over(sky, d, 1e30, sun_dir, vec3<f32>(g.env_sun.w)), 1.0);
}

fn place(v: MeshIn) -> vec3<f32> {
    return v.c0.xyz * v.pos.x + v.c1.xyz * v.pos.y + v.c2.xyz * v.pos.z + v.t.xyz;
}

fn turn(v: MeshIn, d: vec3<f32>) -> vec3<f32> {
    return normalize(v.c0.xyz * d.x + v.c1.xyz * d.y + v.c2.xyz * d.z);
}

// A globe's center (camera-relative) and radius (m), from where its vertices
// are on it (see `globe_at`): a whole globe's is its origin, a patch's its
// origin less its place on the world.
fn air_center(v: MeshIn) -> vec4<f32> {
    let w = max(v.globe_at.w, 1e-12);
    let off = v.c0.xyz * v.globe_at.x + v.c1.xyz * v.globe_at.y + v.c2.xyz * v.globe_at.z;
    return vec4<f32>(v.t.xyz - off / w, length(v.c0.xyz) / w);
}

// (A patch splits when the eye is nearer than `GEOMORPH_SPLIT` times its size: written by
// shaders.rs, the terrain LOD's.)

@vertex
fn vs_mesh(v_in: MeshIn) -> MeshOut {
    var v = v_in;
    var tint = v.color * v.fill_tint;
    // A patch of ground (its vertices in metres): its vertex colour is its way to its parent's
    // shape (rgb, m) and its size (alpha, m). Blended toward it as the eye nears the distance
    // its parent stands in from, so the swap finds the same shape (geomorphing).
    if (v.globe_at.w < 0.5) {
        let far = GEOMORPH_SPLIT * 2.0 * v.color.w * 0.95;
        let m = smoothstep(far * 0.6, far, length(place(v)));
        v.pos = v.pos + v.color.xyz * m;
        tint = v.fill_tint;
    }
    let n = turn(v, v.normal);
    let k = max(dot(n, v.light_dir.xyz), 0.0);
    let p = place(v);
    let local = v.pos * v.globe_at.w + v.globe_at.xyz;
    return MeshOut(g.view_proj * vec4<f32>(p, 1.0), tint, k * v.light_color.rgb, fill(v, n), v.light_dir.w, p, n, v.light_dir.xyz, v.light_color.rgb, v.material, local, v.globe, v.globe_at.w, v.pos + vec3<f32>(v.c0.w, v.c1.w, v.c2.w), air_center(v), vec2<f32>(v.t.w, v.material.w), turn(v, local), v.data);
}

@vertex
fn vs_mesh_line(v: MeshIn) -> MeshOut {
    let n = turn(v, v.normal);
    let k = sqrt(max(dot(n, v.light_dir.xyz), 0.0));
    let p = place(v);
    var clip = g.view_proj * vec4<f32>(p, 1.0);
    clip.z *= 1.003;
    // (Edges, panel lines: no glint of their own.)
    return MeshOut(clip, v.color * v.line_tint, k * v.light_color.rgb, fill(v, n), v.light_color.w, p, n, v.light_dir.xyz, v.light_color.rgb, vec4<f32>(0.0, 1.0, v.material.z, 0.0), v.pos * v.globe_at.w + v.globe_at.xyz, vec4<f32>(0.0), 1.0, vec3<f32>(0.0), vec4<f32>(0.0), vec2<f32>(0.0), n, vec4<f32>(0.0));
}
