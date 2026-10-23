// Composites the scene (HDR light, tone-mapped for the screen here), the
// HUD layer over it (upscaled crisp), and the front layer (what's nearest
// the eye, also HDR) over both.
@group(0) @binding(0) var scene_tex: texture_2d<f32>;
@group(0) @binding(1) var hud_tex: texture_2d<f32>;
@group(0) @binding(2) var nearest: sampler;
@group(0) @binding(3) var front_tex: texture_2d<f32>;
@group(0) @binding(4) var soft: sampler;

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
    // Faces shaded on the screen's colours (Frame::view_shade): on, ambient, diffuse, sea; the world's centre
    // from the eye and its sea level (m).
    view_shade: vec4<f32>,
    view_world: vec4<f32>,
    // The normals' exaggeration given (land, sea) and its taper by a pixel's ground (far, near: m).
    view_light: vec4<f32>,
    // After the curve: the unsharp mask's amount (0: none).
    post: vec4<f32>,
};
@group(0) @binding(5) var<uniform> g: Globals;

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
    // (Off: the light straight to the screen, clipped, gamma-encoded.)
    if (g.look2.z < 0.5) {
        return pow(clamp(x * EXPOSURE, vec3<f32>(0.0), vec3<f32>(1.0)), vec3<f32>(1.0 / 2.2));
    }
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
    // (A look on the curve, as Blender's "Punchy": more contrast and colour; plain AgX is flat,
    // and the user's worlds read blurry and bleak without it.)
    v = pow(max(v, vec3<f32>(0.0)), vec3<f32>(1.35));
    let luma = dot(v, vec3<f32>(0.2126, 0.7152, 0.0722));
    v = vec3<f32>(luma) + 1.4 * (v - vec3<f32>(luma));
    // (Already display-encoded: the screen's format isn't sRGB; no linearising.)
    return clamp(agx_inv * v, vec3<f32>(0.0), vec3<f32>(1.0));
}

// The camera's exposure: scene light to the screen, before the curve.
const EXPOSURE: f32 = 0.9;

// The scene on the screen, unsharp-masked where asked (`Frame::sharpen`): the picture less a one-pixel blur of it,
// times the amount, added back, on the encoded colours, as the planet lab's views are.
fn scene_on_screen(uv: vec2<f32>) -> vec3<f32> {
    let amount = g.post.x;
    if (amount <= 0.0) {
        return film(textureSample(scene_tex, soft, uv).rgb);
    }
    let size = vec2<i32>(textureDimensions(scene_tex));
    let at = clamp(vec2<i32>(uv * vec2<f32>(size)), vec2<i32>(0), size - 1);
    var blur = vec3<f32>(0.0);
    var centre = vec3<f32>(0.0);
    for (var dy = -1; dy <= 1; dy++) {
        for (var dx = -1; dx <= 1; dx++) {
            let c = film(textureLoad(scene_tex, clamp(at + vec2<i32>(dx, dy), vec2<i32>(0), size - 1), 0).rgb);
            blur += c * f32((2 - abs(dx)) * (2 - abs(dy)));
            if (dx == 0 && dy == 0) { centre = c; }
        }
    }
    return clamp(centre + amount * (centre - blur / 16.0), vec3<f32>(0.0), vec3<f32>(1.0));
}

@fragment
fn fs_main(in: VertexOut) -> @location(0) vec4<f32> {
    // The HUD layer holds premultiplied color (alpha-blended onto transparent black).
    let hud = textureSample(hud_tex, soft, in.uv);
    // So does the front layer (in light, before the curve).
    let front = textureSample(front_tex, soft, in.uv);
    let fa = max(front.a, 1e-4);
    let world = scene_on_screen(in.uv) * (1.0 - front.a) + film(front.rgb / fa) * front.a;
    return vec4<f32>(world * (1.0 - hud.a) + hud.rgb, 1.0);
}

@fragment
fn fs_copy(in: VertexOut) -> @location(0) vec4<f32> {
    return textureSample(scene_tex, soft, in.uv);
}
