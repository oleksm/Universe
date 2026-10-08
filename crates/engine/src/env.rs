//! The environment as light (image-based lighting): a cube map drawn on the
//! GPU each frame from what the frame knows (see `shaders/env.wgsl`), its mip
//! levels for rising roughness (a polished hull mirrors the world below, a
//! rough one glows with it). The model shader reflects it; its diffuse light
//! is the scene's ambient.

/// The specular cube's side (texels) and its mip levels (128 down to 1).
pub const SPEC_SIZE: u32 = 128;
pub const SPEC_MIPS: u32 = 8;
const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Pass {
    face: u32,
    kind: u32,
    roughness: f32,
    samples: u32,
}

pub(crate) struct Env {
    pub spec: wgpu::TextureView,
    pipe: wgpu::RenderPipeline,
    /// Each face and level to draw: its target and its pass's uniforms.
    passes: Vec<(wgpu::TextureView, wgpu::BindGroup)>,
}

impl Env {
    pub fn new(device: &wgpu::Device, globals: &wgpu::BindGroupLayout) -> Self {
        use wgpu::util::DeviceExt;
        let cube = |label, size, mips| {
            device.create_texture(&wgpu::TextureDescriptor {
                label: Some(label),
                size: wgpu::Extent3d { width: size, height: size, depth_or_array_layers: 6 },
                mip_level_count: mips,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: FORMAT,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            })
        };
        let spec_tex = cube("environment (specular)", SPEC_SIZE, SPEC_MIPS);
        let as_cube = |t: &wgpu::Texture| t.create_view(&wgpu::TextureViewDescriptor { dimension: Some(wgpu::TextureViewDimension::Cube), ..Default::default() });
        let face = |t: &wgpu::Texture, layer: u32, mip: u32| {
            t.create_view(&wgpu::TextureViewDescriptor { dimension: Some(wgpu::TextureViewDimension::D2), base_array_layer: layer, array_layer_count: Some(1), base_mip_level: mip, mip_level_count: Some(1), ..Default::default() })
        };
        let pass_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("environment pass"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Uniform, has_dynamic_offset: false, min_binding_size: None },
                count: None,
            }],
        });
        let mut passes = Vec::new();
        let mut add = |view: wgpu::TextureView, p: Pass| {
            let buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor { label: Some("environment pass"), contents: bytemuck::bytes_of(&p), usage: wgpu::BufferUsages::UNIFORM });
            let bind = device.create_bind_group(&wgpu::BindGroupDescriptor { label: Some("environment pass"), layout: &pass_layout, entries: &[wgpu::BindGroupEntry { binding: 0, resource: buffer.as_entire_binding() }] });
            passes.push((view, bind));
        };
        for f in 0..6 {
            for mip in 0..SPEC_MIPS {
                // (Level 0 sharp; the last as rough as it gets.)
                let roughness = mip as f32 / (SPEC_MIPS - 1) as f32;
                add(face(&spec_tex, f, mip), Pass { face: f, kind: 0, roughness, samples: if mip == 0 { 1 } else { 64 } });
            }
        }
        let shader = crate::shaders::single(device, "env");
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor { label: Some("environment"), bind_group_layouts: &[Some(globals), Some(&pass_layout)], immediate_size: 0 });
        let pipe = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("environment"),
            layout: Some(&layout),
            vertex: wgpu::VertexState { module: &shader, entry_point: Some("vs_main"), buffers: &[], compilation_options: Default::default() },
            fragment: Some(wgpu::FragmentState { module: &shader, entry_point: Some("fs_main"), targets: &[Some(wgpu::ColorTargetState { format: FORMAT, blend: None, write_mask: wgpu::ColorWrites::ALL })], compilation_options: Default::default() }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: crate::pipecache::get(),
        });
        Env { spec: as_cube(&spec_tex), pipe, passes }
    }

    /// Draw both cubes for this frame (the globals already written).
    pub fn render(&self, encoder: &mut wgpu::CommandEncoder, globals: &wgpu::BindGroup) {
        for (view, bind) in &self.passes {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("environment"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment { view, depth_slice: None, resolve_target: None, ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color::BLACK), store: wgpu::StoreOp::Store } })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(&self.pipe);
            pass.set_bind_group(0, globals, &[]);
            pass.set_bind_group(1, bind, &[]);
            pass.draw(0..3, 0..1);
        }
    }
}
