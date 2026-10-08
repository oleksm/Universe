//! The shaders' sources, each module as it's put together (its files in order), with the
//! constants the Rust side and the shaders must agree on written once, here, and prepended to
//! every module (`CONSTS`). The engine's test (`tests/shaders.rs`) parses and validates each.

/// The shadow map's side (texels), each cascade's.
pub const SHADOW_SIZE: u32 = 4096;
/// Globe maps' texels a face side.
pub const GLOBE_SIZE: u32 = 512;
/// A ground patch splits when the eye is nearer than this many times its size (the game's
/// terrain LOD; the mesh shader geomorphs toward the parent's shape by it).
pub const GEOMORPH_SPLIT: f64 = 2.4;
/// The clouds' time is world time wrapped every this many seconds (f32 keeps about a second there).
pub const CLOUD_WRAP_S: f64 = 1_048_576.0;

/// The constants, as WGSL, before every module.
pub fn consts() -> String {
    format!(
        "// (Written by shaders.rs: the Rust side's constants.)\n\
         const SHADOW_TEXEL: f32 = {};\n\
         const GLOBE_SIZE: f32 = {:.1};\n\
         const GEOMORPH_SPLIT: f32 = {};\n\
         const MICRO_PERIOD: f32 = {:.1};\n\
         const MAX_LIGHT: f32 = {:.1};\n\
         const ADAPT: f32 = {};\n\
         const SPEC_MIPS: u32 = {}u;\n\
         const CLOUD_WRAP_S: f32 = {:.1};\n\
         const CC_N: i32 = {};\n\
         const CC_LEVELS: i32 = {};\n\n",
        1.0 / SHADOW_SIZE as f64,
        GLOBE_SIZE as f64,
        GEOMORPH_SPLIT,
        crate::frame::MICRO_PERIOD,
        crate::frame::MAX_LIGHT,
        crate::frame::ADAPT,
        crate::env::SPEC_MIPS,
        CLOUD_WRAP_S,
        crate::cloudcache::N,
        crate::cloudcache::HALF.len(),
    )
}

fn module(files: &[&str]) -> String {
    let mut s = consts();
    for f in files {
        s.push_str(f);
        s.push('\n');
    }
    s
}

/// The scene's meshes, globes, ground and sky.
pub fn scene() -> String {
    module(&[
        include_str!("shaders/ground_material.wgsl"),
        include_str!("shaders/air.wgsl"),
        include_str!("shaders/sea.wgsl"),
        include_str!("shaders/clouds.wgsl"),
        include_str!("shaders/light.wgsl"),
        include_str!("shaders/scene.wgsl"),
    ])
}

/// The textured models.
pub fn pbr() -> String {
    module(&[include_str!("shaders/light.wgsl"), include_str!("shaders/pbr.wgsl")])
}

/// The cloud cache's fill.
pub fn cloud_cache() -> String {
    module(&[include_str!("shaders/air.wgsl"), include_str!("shaders/clouds.wgsl"), include_str!("shaders/cloud_cache.wgsl")])
}

/// The modules of one file each: (name, source).
pub fn singles() -> Vec<(&'static str, String)> {
    [
        ("env", include_str!("shaders/env.wgsl")),
        ("sunprobe", include_str!("shaders/sunprobe.wgsl")),
        ("pbr_shadow", include_str!("shaders/pbr_shadow.wgsl")),
        ("shadow", include_str!("shaders/shadow.wgsl")),
        ("hud", include_str!("shaders/hud.wgsl")),
        ("blit", include_str!("shaders/blit.wgsl")),
    ]
    .into_iter()
    .map(|(n, s)| (n, module(&[s])))
    .collect()
}

/// A module for the device.
pub(crate) fn make(device: &wgpu::Device, label: &str, source: String) -> wgpu::ShaderModule {
    device.create_shader_module(wgpu::ShaderModuleDescriptor { label: Some(label), source: wgpu::ShaderSource::Wgsl(source.into()) })
}

/// One of `singles`, by name, for the device.
pub(crate) fn single(device: &wgpu::Device, name: &str) -> wgpu::ShaderModule {
    let (_, s) = singles().into_iter().find(|(n, _)| *n == name).unwrap_or_else(|| panic!("no shader {name}"));
    make(device, name, s)
}
