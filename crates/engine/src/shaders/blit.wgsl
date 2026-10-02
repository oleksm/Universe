// Composites the scene (HDR light, tone-mapped for the screen here), the
// HUD layer over it (upscaled crisp), and the front layer (what's nearest
// the eye, also HDR) over both.
@group(0) @binding(0) var scene_tex: texture_2d<f32>;
@group(0) @binding(1) var hud_tex: texture_2d<f32>;
@group(0) @binding(2) var nearest: sampler;
@group(0) @binding(3) var front_tex: texture_2d<f32>;
@group(0) @binding(4) var soft: sampler;

struct VertexOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

// Fullscreen triangle.
@vertex
fn vs_main(@builtin(vertex_index) i: u32) -> VertexOut {
    let uv = vec2<f32>(f32((i << 1u) & 2u), f32(i & 2u));
    return VertexOut(vec4<f32>(uv * vec2<f32>(2.0, -2.0) + vec2<f32>(-1.0, 1.0), 0.0, 1.0), uv);
}

// The light as a camera takes it: a filmic curve (ACES, fitted), bright
// highlights rolling off instead of clipping.
fn film(x: vec3<f32>) -> vec3<f32> {
    let e = x * 1.15;
    return clamp((e * (2.51 * e + 0.03)) / (e * (2.43 * e + 0.59) + 0.14), vec3<f32>(0.0), vec3<f32>(1.0));
}

@fragment
fn fs_main(in: VertexOut) -> @location(0) vec4<f32> {
    let scene = textureSample(scene_tex, soft, in.uv);
    // The HUD layer holds premultiplied color (alpha-blended onto transparent black).
    let hud = textureSample(hud_tex, nearest, in.uv);
    // So does the front layer (in light, before the curve).
    let front = textureSample(front_tex, soft, in.uv);
    let fa = max(front.a, 1e-4);
    let world = film(scene.rgb) * (1.0 - front.a) + film(front.rgb / fa) * front.a;
    return vec4<f32>(world * (1.0 - hud.a) + hud.rgb, 1.0);
}
