// Textured, physically based models (see `pbr.rs`): glTF's metallic-
// roughness, lit by the sun (its shadows) and the reflecting planet's
// light, as meshes are lit (their instance: `frame::Instance`). GGX
// microfacets with Smith's shadowing and Schlick's Fresnel for the glint, a
// Lambert body under it.

struct Globals {
    view_proj: mat4x4<f32>,
    hud_proj: mat4x4<f32>,
    shadow_near: mat4x4<f32>,
    shadow_far: mat4x4<f32>,
    shadow: vec4<f32>,
    // Graphics toggles (1 on): textures, normal maps, occlusion, emission; specular, planet light, tone map.
    look: vec4<f32>,
    look2: vec4<f32>,
};

@group(0) @binding(0) var<uniform> g: Globals;
@group(1) @binding(0) var shadow_map: texture_depth_2d_array;
@group(1) @binding(1) var shadow_cmp: sampler_comparison;

struct Material {
    base_color: vec4<f32>,
    // x metallic, y roughness, z normal scale, w alpha cutoff (0: opaque).
    params: vec4<f32>,
    emissive: vec4<f32>,
    // x occlusion strength.
    extra: vec4<f32>,
};

@group(2) @binding(0) var<uniform> mat: Material;
@group(2) @binding(1) var base_tex: texture_2d<f32>;
@group(2) @binding(2) var mr_tex: texture_2d<f32>;
@group(2) @binding(3) var normal_tex: texture_2d<f32>;
@group(2) @binding(4) var emissive_tex: texture_2d<f32>;
@group(2) @binding(5) var tex_sampler: sampler;
@group(2) @binding(6) var occlusion_tex: texture_2d<f32>;

struct In {
    @location(0) pos: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) tangent: vec4<f32>,
    @location(3) uv: vec2<f32>,
    @location(4) c0: vec4<f32>,
    @location(5) c1: vec4<f32>,
    @location(6) c2: vec4<f32>,
    @location(7) t: vec4<f32>,
    @location(8) light_dir: vec4<f32>,
    @location(9) light_color: vec4<f32>,
    @location(10) refl_dir: vec4<f32>,
    @location(11) refl_color: vec4<f32>,
};

struct Out {
    @builtin(position) clip: vec4<f32>,
    @location(0) at: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) tangent: vec4<f32>,
    @location(3) uv: vec2<f32>,
    // The sun's direction (w: the ambient share) and its light here.
    @location(4) sun_dir: vec4<f32>,
    @location(5) sun_light: vec3<f32>,
    // The reflecting planet's direction (w: how much sky it fills) and light (w: its share).
    @location(6) refl_dir: vec4<f32>,
    @location(7) refl_color: vec4<f32>,
};

fn turn(v: In, d: vec3<f32>) -> vec3<f32> {
    return v.c0.xyz * d.x + v.c1.xyz * d.y + v.c2.xyz * d.z;
}

@vertex
fn vs_pbr(v: In) -> Out {
    let p = turn(v, v.pos) + v.t.xyz;
    let n = normalize(turn(v, v.normal));
    let tg = normalize(turn(v, v.tangent.xyz));
    return Out(g.view_proj * vec4<f32>(p, 1.0), p, n, vec4<f32>(tg, v.tangent.w), v.uv, v.light_dir, v.light_color.rgb, v.refl_dir, v.refl_color);
}

// What the shadow map says of the sun at `p` on a surface facing `n` (as the meshes').
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

const PI: f32 = 3.14159265;

// GGX's spread of microfacet normals (alpha: roughness squared).
fn ggx(nh: f32, alpha: f32) -> f32 {
    let a2 = alpha * alpha;
    let d = nh * nh * (a2 - 1.0) + 1.0;
    return a2 / (PI * d * d);
}

// Smith's masking-shadowing, Schlick-GGX form, for direct light.
fn smith(nv: f32, nl: f32, roughness: f32) -> f32 {
    let k = (roughness + 1.0) * (roughness + 1.0) / 8.0;
    return nv / (nv * (1.0 - k) + k) * nl / (nl * (1.0 - k) + k);
}

fn schlick(f0: vec3<f32>, vh: f32) -> vec3<f32> {
    return f0 + (vec3<f32>(1.0) - f0) * pow(1.0 - vh, 5.0);
}

@fragment
fn fs_pbr(in: Out) -> @location(0) vec4<f32> {
    // (Textures off: the material's plain values; a texture that's white.)
    let textured = g.look.x > 0.5;
    let base = select(vec4<f32>(1.0), textureSample(base_tex, tex_sampler, in.uv), textured) * mat.base_color;
    // (A cut-out: decals, grilles. Below the cutoff the pixel isn't there.)
    if (mat.params.w > 0.0 && base.a < mat.params.w) {
        discard;
    }
    let mr = select(vec4<f32>(1.0), textureSample(mr_tex, tex_sampler, in.uv), textured);
    // How much of the light from round about reaches here (seams, corners: less).
    let occ = select(1.0, 1.0 + mat.extra.x * (textureSample(occlusion_tex, tex_sampler, in.uv).r - 1.0), g.look.z > 0.5);
    let metallic = clamp(mr.b * mat.params.x, 0.0, 1.0);
    var roughness = clamp(mr.g * mat.params.y, 0.04, 1.0);
    // The normal map, in the surface's tangent frame.
    let tn = textureSample(normal_tex, tex_sampler, in.uv).xyz * 2.0 - vec3<f32>(1.0);
    let ng = normalize(in.normal);
    let t = normalize(in.tangent.xyz - ng * dot(ng, in.tangent.xyz));
    let b = cross(ng, t) * in.tangent.w;
    let n = select(ng, normalize(t * tn.x * mat.params.z + b * tn.y * mat.params.z + ng * tn.z), g.look.y > 0.5);
    // Specular anti-aliasing: where the normal swings across a pixel (fine
    // normal-map detail, far away), the glint would sparkle from frame to
    // frame; widen the roughness by how much it swings (Kaplanyan & Hoffman).
    let dn = 0.25 * (dot(dpdx(n), dpdx(n)) + dot(dpdy(n), dpdy(n)));
    roughness = sqrt(sqrt(min(roughness * roughness * roughness * roughness + min(2.0 * dn, 0.18), 1.0)));
    let v = normalize(-in.at);
    let l = in.sun_dir.xyz;
    let nv = max(dot(n, v), 1e-4);
    let nl = max(dot(n, l), 0.0);
    let f0 = mix(vec3<f32>(0.04), base.rgb, metallic);
    // The light on it as the meshes reckon it (the same units, the same eye):
    // the sun's (in shadow or not), the reflecting planet's, an ambient floor.
    let ambient = in.sun_dir.w * occ;
    let shadow = sunlit(in.at, ng);
    var spec = vec3<f32>(0.0);
    var f = f0;
    if (nl > 0.0) {
        let h = normalize(l + v);
        let nh = max(dot(n, h), 0.0);
        f = schlick(f0, max(dot(v, h), 0.0));
        spec = ggx(nh, roughness * roughness) * smith(nv, nl, roughness) * f / max(4.0 * nv * nl, 1e-4);
    }
    let sun = in.sun_light * nl * shadow;
    // (UNIVERSE_SHADOW_DEBUG: what's in shadow, facing the sun, red; as on the meshes.)
    if (g.shadow.w > 0.0 && shadow < 0.5 && nl > 0.0) {
        return vec4<f32>(0.8, 0.0, 0.0, 1.0);
    }
    // The planet's light: how much of the sky it fills from its side, through
    // the eye's adaptation as on the meshes (see scene.wgsl `fill`).
    var fill = vec3<f32>(0.0);
    if (in.refl_dir.w > 0.0) {
        let s = min(in.refl_dir.w, 1.0);
        let cc = 1.0 - sqrt(1.0 - s);
        let seen = s * max((dot(n, in.refl_dir.xyz) + cc) / (1.0 + cc), 0.0);
        fill = max(pow(in.refl_color.w * seen, 0.3) - 0.12, 0.0) / 0.88 * in.refl_color.rgb * occ;
    }
    // The body (what isn't metal) takes all of it; metal only reflects.
    let body = base.rgb * (1.0 - metallic);
    var c = body * (vec3<f32>(ambient) + (1.0 - ambient) * (sun * (vec3<f32>(1.0) - f) + fill));
    c += spec * PI * sun * g.look2.x;
    // Metal under the same ambient floor (what's round it, dimly reflected).
    c += base.rgb * metallic * ambient;
    // What a glossy surface reflects of the planet: a light the size it looks
    // (an area light). The reflection's lobe widens with roughness; it catches
    // the disc's share of itself (energy kept: a broad lobe, a dim, wide image).
    if (in.refl_dir.w > 0.0 && g.look2.x > 0.5) {
        let r = reflect(-v, n);
        let radius = asin(sqrt(min(in.refl_dir.w, 1.0)));
        let lobe = max(roughness * roughness * 1.2, 0.003);
        let off = acos(clamp(dot(r, in.refl_dir.xyz), -1.0, 1.0));
        let edge = clamp((radius + lobe - off) / (2.0 * lobe), 0.0, 1.0);
        let share = min(1.0, (radius * radius) / (lobe * lobe));
        c += schlick(f0, nv) * in.refl_color.rgb * in.refl_color.w * edge * share * occ;
    }
    c += select(vec4<f32>(1.0), textureSample(emissive_tex, tex_sampler, in.uv), textured).rgb * mat.emissive.rgb * g.look.w;
    return vec4<f32>(c, 1.0);
}
