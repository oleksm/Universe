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
};

@group(0) @binding(0) var<uniform> g: Globals;
@group(1) @binding(0) var shadow_map: texture_depth_2d_array;
@group(1) @binding(1) var shadow_cmp: sampler_comparison;

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
};

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
        return textureSampleCompareLevel(shadow_map, shadow_cmp, a, 0, near.z);
    }
    let far = g.shadow_far * vec4<f32>(p + n * g.shadow.y * 1.5, 1.0);
    let b = vec2<f32>(far.x * 0.5 + 0.5, 0.5 - far.y * 0.5);
    if (all(b > vec2<f32>(0.0)) && all(b < vec2<f32>(1.0)) && far.z > 0.0 && far.z < 1.0) {
        return textureSampleCompareLevel(shadow_map, shadow_cmp, b, 1, far.z);
    }
    return 1.0;
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
};

@fragment
fn fs_mesh(in: MeshOut) -> @location(0) vec4<f32> {
    let seen = sunlit(in.at, normalize(in.normal));
    let light = min(in.sun * seen + in.fill, vec3<f32>(4.0));
    if (g.shadow.w > 0.0 && seen < 0.5 && max(in.sun.r, max(in.sun.g, in.sun.b)) > 0.0) {
        return vec4<f32>(0.8, 0.0, 0.0, in.color.a);
    }
    return vec4<f32>(in.color.rgb * (vec3<f32>(in.ambient) + (1.0 - in.ambient) * light), in.color.a);
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
    return MeshOut(g.view_proj * vec4<f32>(p, 1.0), v.color * v.fill_tint, k * v.light_color.rgb, fill(v, n), v.light_dir.w, p, n);
}

@vertex
fn vs_mesh_line(v: MeshIn) -> MeshOut {
    let n = turn(v, v.normal);
    let k = sqrt(max(dot(n, v.light_dir.xyz), 0.0));
    let p = place(v);
    var clip = g.view_proj * vec4<f32>(p, 1.0);
    clip.z *= 1.003;
    return MeshOut(clip, v.color * v.line_tint, k * v.light_color.rgb, fill(v, n), v.light_color.w, p, n);
}
