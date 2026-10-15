// The PBR models in the shadow map (see `shadow.wgsl`): depth only.

struct Light {
    view_proj: mat4x4<f32>,
};

@group(0) @binding(0) var<uniform> light: Light;

struct In {
    @location(0) pos: vec3<f32>,
    @location(3) uv: vec2<f32>,
    @location(4) c0: vec4<f32>,
    @location(5) c1: vec4<f32>,
    @location(6) c2: vec4<f32>,
    @location(7) t: vec4<f32>,
};

struct Out { @builtin(position) clip: vec4<f32>, @location(0) uv: vec2<f32> };
struct Material { base_color: vec4<f32>, params: vec4<f32>, emissive: vec4<f32>, extra: vec4<f32> };
@group(1) @binding(0) var<uniform> mat: Material;
@group(1) @binding(1) var base_tex: texture_2d<f32>;
@group(1) @binding(5) var tex_sampler: sampler;
@fragment
fn fs_pbr_shadow(v: Out) {
    let alpha = textureSample(base_tex, tex_sampler, v.uv).a * mat.base_color.a;
    if alpha < mat.params.w { discard; }
}
@vertex
fn vs_pbr_shadow(v: In) -> Out {
    let p = v.c0.xyz * v.pos.x + v.c1.xyz * v.pos.y + v.c2.xyz * v.pos.z + v.t.xyz;
    return Out(light.view_proj * vec4<f32>(p, 1.0), v.uv);
}
