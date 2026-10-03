// The PBR models in the shadow map (see `shadow.wgsl`): depth only.

struct Light {
    view_proj: mat4x4<f32>,
};

@group(0) @binding(0) var<uniform> light: Light;

struct In {
    @location(0) pos: vec3<f32>,
    @location(4) c0: vec4<f32>,
    @location(5) c1: vec4<f32>,
    @location(6) c2: vec4<f32>,
    @location(7) t: vec4<f32>,
};

@vertex
fn vs_pbr_shadow(v: In) -> @builtin(position) vec4<f32> {
    let p = v.c0.xyz * v.pos.x + v.c1.xyz * v.pos.y + v.c2.xyz * v.pos.z + v.t.xyz;
    return light.view_proj * vec4<f32>(p, 1.0);
}
