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

// The light as a camera takes it: AgX (Troy Sobotka's; Blender's default
// view since 4.0, so a model lit there looks the same here). Brights roll off
// toward white without the hue skews of a per-channel curve: orange paint stays
// orange, a lamp burns white at its core. (Fitted polynomial: Benjamin Wrensch's.)
fn agx_contrast(x: vec3<f32>) -> vec3<f32> {
    let x2 = x * x;
    let x4 = x2 * x2;
    return 15.5 * x4 * x2 - 40.14 * x4 * x + 31.96 * x4 - 6.868 * x2 * x + 0.4298 * x2 + 0.1191 * x - 0.00232;
}

fn film(x: vec3<f32>) -> vec3<f32> {
    let agx = mat3x3<f32>(
        vec3<f32>(0.842479062253094, 0.0423282422610123, 0.0423756549057051),
        vec3<f32>(0.0784335999999992, 0.878468636469772, 0.0784336),
        vec3<f32>(0.0792237451477643, 0.0791661274605434, 0.879142973793104),
    );
    let agx_inv = mat3x3<f32>(
        vec3<f32>(1.19687900512017, -0.0528968517574562, -0.0529716355144438),
        vec3<f32>(-0.0980208811401368, 1.15190312990417, -0.0980434501171241),
        vec3<f32>(-0.0990297440797205, -0.0989611768448433, 1.15107367264116),
    );
    let min_ev = -12.47393;
    let max_ev = 4.026069;
    // (Exposure: the meshes' light as it was under the old curve, about.)
    var v = agx * max(x * EXPOSURE, vec3<f32>(1e-10));
    v = clamp(log2(v), vec3<f32>(min_ev), vec3<f32>(max_ev));
    v = (v - min_ev) / (max_ev - min_ev);
    v = agx_contrast(v);
    // (Already display-encoded: the screen's format isn't sRGB; no linearising.)
    return clamp(agx_inv * v, vec3<f32>(0.0), vec3<f32>(1.0));
}

const EXPOSURE: f32 = 0.5;

@fragment
fn fs_main(in: VertexOut) -> @location(0) vec4<f32> {
    let scene = textureSample(scene_tex, soft, in.uv);
    // The HUD layer holds premultiplied color (alpha-blended onto transparent black).
    let hud = textureSample(hud_tex, soft, in.uv);
    // So does the front layer (in light, before the curve).
    let front = textureSample(front_tex, soft, in.uv);
    let fa = max(front.a, 1e-4);
    let world = film(scene.rgb) * (1.0 - front.a) + film(front.rgb / fa) * front.a;
    return vec4<f32>(world * (1.0 - hud.a) + hud.rgb, 1.0);
}
