//! A ground renderer the game or a bench plugs in (`Config::ground`): it draws a world's surface into the
//! engine's own passes, so the air, sea, clouds, shadows and HUD the engine draws go over and round it.
//!
//! The engine calls it at three points each frame:
//! - `prepare`, before any pass (its own compute work, uploads), with the camera;
//! - `draw_shadow`, in each shadow cascade with the cascade's light bound at group 0 (cascade 3 is the
//!   ground's own: the casters there are the ground alone);
//! - `draw`, in the scene pass after the meshes, with the engine's groups bound: 0 the frame's globals,
//!   1 the shadow maps and globes, 2 the bound world's maps. It may set groups 3 and up for its own.
//!
//! Positions are camera-relative, as everywhere in the engine: the camera's own position is f64
//! (`GroundFrame::camera`), the globals' `view_proj` takes camera-relative metres.

use crate::Camera;

/// What the ground's pipelines must match: the device, the layouts of the groups the engine binds, and
/// the targets' formats.
pub struct GroundSetup<'a> {
    pub device: &'a wgpu::Device,
    pub queue: &'a wgpu::Queue,
    /// The scene pass's group 0 (the globals), 1 (shadow maps, globes, environment) and 2 (world maps).
    pub globals: &'a wgpu::BindGroupLayout,
    pub shadows: &'a wgpu::BindGroupLayout,
    pub world: &'a wgpu::BindGroupLayout,
    /// A shadow cascade's group 0 (its light's matrix).
    pub light: &'a wgpu::BindGroupLayout,
    /// The scene's colour and depth formats and its sample count; the shadow map's depth format.
    pub color: wgpu::TextureFormat,
    pub depth: wgpu::TextureFormat,
    pub samples: u32,
    pub shadow_depth: wgpu::TextureFormat,
    /// The engine's pipeline cache (kept between runs), for the ground's own pipelines.
    pub cache: Option<&'a wgpu::PipelineCache>,
}

/// The frame as the ground sees it.
pub struct GroundFrame<'a> {
    pub device: &'a wgpu::Device,
    pub queue: &'a wgpu::Queue,
    pub camera: Camera,
}

/// A ground renderer.
pub trait GroundPass: Send {
    /// Once, before the first frame.
    fn setup(&mut self, setup: &GroundSetup);
    /// Each frame, before any pass: its own work (evaluation, uploads) recorded on `encoder`.
    fn prepare(&mut self, _frame: &GroundFrame, _encoder: &mut wgpu::CommandEncoder) {}
    /// Into shadow cascade `cascade` (0–3), its light bound at group 0.
    fn draw_shadow(&self, _pass: &mut wgpu::RenderPass<'_>, _cascade: usize) {}
    /// Into the scene, after the meshes, groups 0–2 bound.
    fn draw(&self, pass: &mut wgpu::RenderPass<'_>);
}
