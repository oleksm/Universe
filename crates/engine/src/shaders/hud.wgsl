// The HUD's triangles: panels, fills and text in one queue. Each samples
// the font atlas: a glyph's coverage, or (plain fills) its solid patch.
struct Globals {
    view_proj: mat4x4<f32>,
    hud_proj: mat4x4<f32>,
};

@group(0) @binding(0) var<uniform> g: Globals;
@group(1) @binding(0) var atlas: texture_2d<f32>;
@group(1) @binding(1) var atlas_sampler: sampler;

struct HudIn {
    @location(0) pos: vec2<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) color: vec4<f32>,
};

struct HudOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) color: vec4<f32>,
};

@vertex
fn vs_hud(v: HudIn) -> HudOut {
    return HudOut(g.hud_proj * vec4<f32>(v.pos, 0.0, 1.0), v.uv, v.color);
}

@fragment
fn fs_hud(in: HudOut) -> @location(0) vec4<f32> {
    let cover = textureSample(atlas, atlas_sampler, in.uv).r;
    return vec4<f32>(in.color.rgb, in.color.a * cover);
}
