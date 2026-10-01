// The shadow map: the meshes that cast, seen from the light (an
// orthographic box round the eye; depth only).

struct Light {
    view_proj: mat4x4<f32>,
};

@group(0) @binding(0) var<uniform> light: Light;

struct MeshIn {
    @location(0) pos: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) color: vec4<f32>,
    @location(3) c0: vec4<f32>,
    @location(4) c1: vec4<f32>,
    @location(5) c2: vec4<f32>,
    @location(6) t: vec4<f32>,
};

@vertex
fn vs_shadow(v: MeshIn) -> @builtin(position) vec4<f32> {
    let p = v.c0.xyz * v.pos.x + v.c1.xyz * v.pos.y + v.c2.xyz * v.pos.z + v.t.xyz;
    return light.view_proj * vec4<f32>(p, 1.0);
}
