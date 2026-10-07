// The environment the eye is in, as light from every direction (see `env.rs`):
// drawn into a cube map each frame from what the frame knows (the black sky,
// the faint glow of the stars, the world nearest, lit by the sun on its day
// side), or a studio's soft boxes. Each mip level of the specular cube holds
// it as a surface of rising roughness reflects it (GGX, importance-sampled);
// the diffuse cube holds what a matte face turned that way takes in (its
// cosine-weighted mean). The sun itself isn't in it: it lights directly,
// with its shadows.

struct Globals {
    view_proj: mat4x4<f32>,
    hud_proj: mat4x4<f32>,
    shadow_near: mat4x4<f32>,
    shadow_far: mat4x4<f32>,
    shadow: vec4<f32>,
    look: vec4<f32>,
    look2: vec4<f32>,
    shadow_tight: mat4x4<f32>,
    shadow2: vec4<f32>,
    // The sun's direction (camera frame) and its light here (w: none 0).
    env_sun: vec4<f32>,
    // The world nearest: its centre from the eye (m), its radius (w).
    env_world: vec4<f32>,
    // Its colour (rgb) and albedo (w; 0: no world).
    env_world_color: vec4<f32>,
    // x: 0 space, 1 studio; y: the stars' glow.
    env_mode: vec4<f32>,
    // The sky's colour in an atmosphere (black in space).
    env_sky: vec4<f32>,
};

// Which face, which roughness, specular (0) or diffuse (1).
struct Pass {
    face: u32,
    kind: u32,
    roughness: f32,
    samples: u32,
};

@group(0) @binding(0) var<uniform> g: Globals;
@group(1) @binding(0) var<uniform> pass_: Pass;

struct Out {
    @builtin(position) clip: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) i: u32) -> Out {
    let p = vec2<f32>(f32((i << 1u) & 2u), f32(i & 2u)) * 2.0 - 1.0;
    return Out(vec4<f32>(p, 0.0, 1.0), vec2<f32>(p.x, -p.y));
}

// A cube face's texel to its direction (the WebGPU cube layout: +X -X +Y -Y +Z -Z).
fn direction(face: u32, uv: vec2<f32>) -> vec3<f32> {
    let u = uv.x;
    let v = uv.y;
    switch face {
        case 0u: { return normalize(vec3<f32>(1.0, -v, -u)); }
        case 1u: { return normalize(vec3<f32>(-1.0, -v, u)); }
        case 2u: { return normalize(vec3<f32>(u, 1.0, v)); }
        case 3u: { return normalize(vec3<f32>(u, -1.0, -v)); }
        case 4u: { return normalize(vec3<f32>(u, -v, 1.0)); }
        default: { return normalize(vec3<f32>(-u, -v, -1.0)); }
    }
}

// The light coming from direction `d`.
fn sky(d: vec3<f32>) -> vec3<f32> {
    if (g.env_mode.x > 0.5) {
        // The studio: a bright soft box on the key's side, a broad dimmer one
        // opposite, a grey cyclorama between.
        let key = g.env_sun.xyz;
        let k = dot(d, key);
        let box_key = smoothstep(0.80, 0.93, k) * 3.0;
        let box_fill = smoothstep(0.55, 0.85, dot(d, normalize(-key + vec3<f32>(0.0, 0.35, 0.0)))) * 0.9;
        let grey = 0.18 + 0.22 * (k * 0.5 + 0.5);
        return vec3<f32>(grey + box_key + box_fill);
    }
    // (Under an atmosphere, the sky's colour, as it's drawn behind everything.)
    var c = max(vec3<f32>(g.env_mode.y), g.env_sky.rgb);
    if (g.env_world_color.w > 0.0) {
        let pc = g.env_world.xyz;
        let r = g.env_world.w;
        let t = dot(pc, d);
        let off = pc - d * t;
        let h2 = r * r - dot(off, off);
        if (t > 0.0 && h2 > 0.0) {
            // Its ground where the ray meets it, lit by the sun's height there,
            // as the eye takes light in (irradiance to EXPOSURE, frame.rs: as
            // the sun and the ground are drawn), over the sky's own glow.
            let p = d * (t - sqrt(h2));
            let n = normalize(p - pc);
            let lit = g.env_world_color.w * max(dot(n, g.env_sun.xyz), 0.0);
            c = max(g.env_world_color.rgb * g.env_sun.w * pow(lit, EXPOSURE), c);
        }
    }
    return c;
}

fn hammersley(i: u32, n: u32) -> vec2<f32> {
    var b = i;
    b = (b << 16u) | (b >> 16u);
    b = ((b & 0x55555555u) << 1u) | ((b & 0xAAAAAAAAu) >> 1u);
    b = ((b & 0x33333333u) << 2u) | ((b & 0xCCCCCCCCu) >> 2u);
    b = ((b & 0x0F0F0F0Fu) << 4u) | ((b & 0xF0F0F0F0u) >> 4u);
    b = ((b & 0x00FF00FFu) << 8u) | ((b & 0xFF00FF00u) >> 8u);
    return vec2<f32>(f32(i) / f32(n), f32(b) * 2.3283064365386963e-10);
}

// A frame round `n`: (tangent, bitangent).
fn basis(n: vec3<f32>) -> mat3x3<f32> {
    let up = select(vec3<f32>(0.0, 0.0, 1.0), vec3<f32>(1.0, 0.0, 0.0), abs(n.z) > 0.999);
    let t = normalize(cross(up, n));
    return mat3x3<f32>(t, cross(n, t), n);
}

const PI: f32 = 3.14159265;
// How the eye adapts (frame.rs `EXPOSURE`).
const EXPOSURE: f32 = 0.45;

@fragment
fn fs_main(in: Out) -> @location(0) vec4<f32> {
    let n = direction(pass_.face, in.uv);
    let frame = basis(n);
    if (pass_.roughness <= 0.0) {
        return vec4<f32>(sky(n), 1.0);
    }
    // Specular: GGX-importance-sampled round `n` (seen straight on).
    let a = pass_.roughness * pass_.roughness;
    var sum = vec3<f32>(0.0);
    var weight = 0.0;
    for (var i = 0u; i < pass_.samples; i++) {
        let xi = hammersley(i, pass_.samples);
        let phi = 2.0 * PI * xi.y;
        let ct = sqrt((1.0 - xi.x) / (1.0 + (a * a - 1.0) * xi.x));
        let st = sqrt(1.0 - ct * ct);
        let h = frame * vec3<f32>(st * cos(phi), st * sin(phi), ct);
        let l = 2.0 * dot(n, h) * h - n;
        let nl = dot(n, l);
        if (nl > 0.0) {
            sum += sky(l) * nl;
            weight += nl;
        }
    }
    return vec4<f32>(sum / max(weight, 1e-4), 1.0);
}
