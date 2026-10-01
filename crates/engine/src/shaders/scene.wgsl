struct Globals {
    // Camera-relative world -> clip (reversed-Z, infinite far plane).
    view_proj: mat4x4<f32>,
    // Low-res pixel coords -> clip.
    hud_proj: mat4x4<f32>,
};

@group(0) @binding(0) var<uniform> g: Globals;

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

// Brightness per channel: ambient plus the star's (k) and the planet's light.
fn lit(v: MeshIn, n: vec3<f32>, k: f32, ambient: f32) -> vec3<f32> {
    var k2 = 0.0;
    if (v.refl_dir.w > 0.0) {
        k2 = max(pow(v.refl_color.w * view_factor(dot(n, v.refl_dir.xyz), v.refl_dir.w), EXPOSURE) - 0.12, 0.0) / 0.88;
    }
    let light = min(k * v.light_color.rgb + k2 * v.refl_color.rgb, vec3<f32>(1.6));
    return vec3<f32>(ambient) + (1.0 - ambient) * light;
}

fn place(v: MeshIn) -> vec3<f32> {
    return v.c0.xyz * v.pos.x + v.c1.xyz * v.pos.y + v.c2.xyz * v.pos.z + v.t.xyz;
}

fn turn(v: MeshIn, d: vec3<f32>) -> vec3<f32> {
    return normalize(v.c0.xyz * d.x + v.c1.xyz * d.y + v.c2.xyz * d.z);
}

@vertex
fn vs_mesh(v: MeshIn) -> VertexOut {
    let n = turn(v, v.normal);
    let k = max(dot(n, v.light_dir.xyz), 0.0);
    let c = v.color * v.fill_tint;
    return VertexOut(g.view_proj * vec4<f32>(place(v), 1.0), vec4<f32>(c.rgb * lit(v, n, k, v.light_dir.w), c.a));
}

@vertex
fn vs_mesh_line(v: MeshIn) -> VertexOut {
    let n = turn(v, v.normal);
    let k = sqrt(max(dot(n, v.light_dir.xyz), 0.0));
    let c = v.color * v.line_tint;
    var clip = g.view_proj * vec4<f32>(place(v), 1.0);
    clip.z *= 1.003;
    return VertexOut(clip, vec4<f32>(c.rgb * lit(v, n, k, v.light_color.w), c.a));
}
