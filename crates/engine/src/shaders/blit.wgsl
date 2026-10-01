// Composites the low-res scene, the (higher-res) HUD layer over it, and the
// front layer (what's nearest the eye) over both, upscaled with nearest filtering.
@group(0) @binding(0) var scene_tex: texture_2d<f32>;
@group(0) @binding(1) var hud_tex: texture_2d<f32>;
@group(0) @binding(2) var nearest: sampler;
@group(0) @binding(3) var front_tex: texture_2d<f32>;

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

@fragment
fn fs_main(in: VertexOut) -> @location(0) vec4<f32> {
    let scene = textureSample(scene_tex, nearest, in.uv);
    // The HUD layer holds premultiplied color (alpha-blended onto transparent black).
    let hud = textureSample(hud_tex, nearest, in.uv);
    // So does the front layer.
    let front = textureSample(front_tex, nearest, in.uv);
    let under = scene.rgb * (1.0 - hud.a) + hud.rgb;
    return vec4<f32>(under * (1.0 - front.a) + front.rgb, 1.0);
}
