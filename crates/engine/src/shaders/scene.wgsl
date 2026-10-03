struct Globals {
    // Camera-relative world -> clip (reversed-Z, infinite far plane).
    view_proj: mat4x4<f32>,
    // Low-res pixel coords -> clip.
    hud_proj: mat4x4<f32>,
    // Camera-relative world -> the shadow map's two cascades (near, far).
    shadow_near: mat4x4<f32>,
    shadow_far: mat4x4<f32>,
    // x, y: a texel of each cascade (metres); z: shadows on (1) or not;
    // w: tint what's in shadow red (checking them).
    shadow: vec4<f32>,
    // Graphics toggles (1 on): textures, normal maps, occlusion, emission; specular, planet light, tone map.
    look: vec4<f32>,
    look2: vec4<f32>,
};

@group(0) @binding(0) var<uniform> g: Globals;
@group(1) @binding(0) var shadow_map: texture_depth_2d_array;
@group(1) @binding(1) var shadow_cmp: sampler_comparison;
// Worlds' surfaces (see `GlobeMap`): height (in relief units) and crater-ness.
@group(1) @binding(2) var globe_maps: texture_cube_array<f32>;
@group(1) @binding(3) var globe_soft: sampler;

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
};
// (A patch's origin wrapped to the fine grain's period (m) rides in c0.w, c1.w, c2.w.)

const EXPOSURE: f32 = 0.3;

fn view_factor(cos_b: f32, s_in: f32) -> f32 {
    let s = min(s_in, 1.0);
    let c = 1.0 - sqrt(1.0 - s);
    return s * max((cos_b + c) / (1.0 + c), 0.0);
}

// The planet's light (k2) on a surface facing `n`.
fn fill(v: MeshIn, n: vec3<f32>) -> vec3<f32> {
    var k2 = 0.0;
    if (v.refl_dir.w > 0.0) {
        k2 = max(pow(v.refl_color.w * view_factor(dot(n, v.refl_dir.xyz), v.refl_dir.w), EXPOSURE) - 0.12, 0.0) / 0.88;
    }
    return k2 * v.refl_color.rgb;
}

// What the shadow map says of the sun at `p` (camera-relative), on a
// surface facing `n`: 1 lit, 0 in shadow (2x2 filtered at the edge).
// Looked up a texel and a half off the surface, so it doesn't shadow itself.
fn sunlit(p: vec3<f32>, n: vec3<f32>) -> f32 {
    if (g.shadow.z == 0.0) {
        return 1.0;
    }
    let near = g.shadow_near * vec4<f32>(p + n * g.shadow.x * 1.5, 1.0);
    let a = vec2<f32>(near.x * 0.5 + 0.5, 0.5 - near.y * 0.5);
    if (all(a > vec2<f32>(0.01)) && all(a < vec2<f32>(0.99)) && near.z > 0.0 && near.z < 1.0) {
        return pcf(a, 0, near.z);
    }
    let far = g.shadow_far * vec4<f32>(p + n * g.shadow.y * 1.5, 1.0);
    let b = vec2<f32>(far.x * 0.5 + 0.5, 0.5 - far.y * 0.5);
    if (all(b > vec2<f32>(0.0)) && all(b < vec2<f32>(1.0)) && far.z > 0.0 && far.z < 1.0) {
        return pcf(b, 1, far.z);
    }
    return 1.0;
}

// A shadow map texel (its uv): 1 / SHADOW_SIZE (renderer.rs).
const SHADOW_TEXEL: f32 = 1.0 / 4096.0;

// 3x3 compared samples (each itself filtered 2x2): soft edges, no stair steps.
fn pcf(uv: vec2<f32>, layer: i32, depth: f32) -> f32 {
    var lit = 0.0;
    for (var y = -1; y <= 1; y++) {
        for (var x = -1; x <= 1; x++) {
            lit += textureSampleCompareLevel(shadow_map, shadow_cmp, uv + vec2<f32>(f32(x), f32(y)) * SHADOW_TEXEL, layer, depth);
        }
    }
    return lit / 9.0;
}

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
};

// The fine grain repeats every this many metres (see `MICRO_PERIOD`).
const MICRO_PERIOD: f32 = 4096.0;

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
    let ground = textureSampleGrad(globe_maps, globe_soft, in.local, layer, ldx, ldy);
    // How much of the world a pixel spans (radians), from the eye's metres
    // (precise) on a patch; the radius as drawn (m).
    let radius = select(length(px) / max(length(ldx) / max(length(in.local), 1e-6), 1e-9), 1.0 / in.patch_scale, on_patch);
    let footprint = select((length(ldx) + length(ldy)) / max(length(in.local), 1e-6), (length(px) + length(py)) / radius, on_patch);
    // (Fine detail, for a globe: in its colour, and in the heights its
    // slopes are shaded by — ridges and hollows at any zoom, drawing only.)
    var d = vec3<f32>(0.0);
    if (in.globe.x > 0.5) {
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
    }
    // (A port's plain: dry ground, no beach.)
    h = max(h, 0.03 * smoothstep(0.0, 0.3, plain));
    let land = select(1.0, step(0.0, select(ground.r + d.x * 0.03, h, on_patch)), in.globe.y < 0.5);
    // (Shaded the same near and far: the map's slopes and the fine detail's,
    // over whatever shape the mesh has — till a pixel is a few metres or
    // less: then where on the world is too coarse to take slopes from (they'd
    // speckle), and the mesh's own shape shades it.)
    let pixel = length(px) + length(py);
    let slopes = select(1.0, smoothstep(2.0, 10.0, pixel), on_patch);
    // Up close on a patch, the fine grain instead (exact: it doesn't speckle).
    var grain = vec2<f32>(0.0);
    if (on_patch && in.globe.x > 0.5) {
        grain = micro_detail(in.micro, pixel) * (1.0 - slopes * 0.5);
    }
    let lift = (ground.r * in.globe.z + land * d.y * 0.06 * radius) * slopes + land * grain.y;
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
        albedo = vec4<f32>(globe_color(in.globe.y, h, inside, in.color.rgb, dir, d, select(1.0, 0.0, on_patch)) * (1.0 + 0.25 * grain.x * land) * in.globe.w, in.color.a);
    }
    let seen = sunlit(in.at, n);
    // (A globe lit per pixel: its slopes, its terminator.)
    var sun = in.sun;
    if (in.globe.x > 0.5) {
        sun = max(dot(n, in.sun_dir), 0.0) * in.sun_light;
    }
    let light = min(sun * seen + in.fill, vec3<f32>(4.0));
    if (g.shadow.w > 0.0 && seen < 0.5 && max(in.sun.r, max(in.sun.g, in.sun.b)) > 0.0) {
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
    return vec4<f32>(c, albedo.a);
}

fn place(v: MeshIn) -> vec3<f32> {
    return v.c0.xyz * v.pos.x + v.c1.xyz * v.pos.y + v.c2.xyz * v.pos.z + v.t.xyz;
}

fn turn(v: MeshIn, d: vec3<f32>) -> vec3<f32> {
    return normalize(v.c0.xyz * d.x + v.c1.xyz * d.y + v.c2.xyz * d.z);
}

@vertex
fn vs_mesh(v: MeshIn) -> MeshOut {
    let n = turn(v, v.normal);
    let k = max(dot(n, v.light_dir.xyz), 0.0);
    let p = place(v);
    return MeshOut(g.view_proj * vec4<f32>(p, 1.0), v.color * v.fill_tint, k * v.light_color.rgb, fill(v, n), v.light_dir.w, p, n, v.light_dir.xyz, v.light_color.rgb, v.material, v.pos * v.globe_at.w + v.globe_at.xyz, v.globe, v.globe_at.w, v.pos + vec3<f32>(v.c0.w, v.c1.w, v.c2.w));
}

@vertex
fn vs_mesh_line(v: MeshIn) -> MeshOut {
    let n = turn(v, v.normal);
    let k = sqrt(max(dot(n, v.light_dir.xyz), 0.0));
    let p = place(v);
    var clip = g.view_proj * vec4<f32>(p, 1.0);
    clip.z *= 1.003;
    // (Edges, panel lines: no glint of their own.)
    return MeshOut(clip, v.color * v.line_tint, k * v.light_color.rgb, fill(v, n), v.light_color.w, p, n, v.light_dir.xyz, v.light_color.rgb, vec4<f32>(0.0, 1.0, v.material.z, 0.0), v.pos * v.globe_at.w + v.globe_at.xyz, vec4<f32>(0.0), 1.0, vec3<f32>(0.0));
}
