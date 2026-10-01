use std::path::Path;

use glam::camera::rh::proj::directx::orthographic;
use glam::UVec2;

use crate::frame::{Frame, Vertex};
use crate::gpu::Gpu;

const COLOR_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;
const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Globals {
    view_proj: [[f32; 4]; 4],
    hud_proj: [[f32; 4]; 4],
}

/// Growable GPU vertex buffer, re-filled every frame.
struct DynBuffer {
    label: &'static str,
    buffer: wgpu::Buffer,
    capacity: u64,
    count: u32,
}

impl DynBuffer {
    fn new(device: &wgpu::Device, label: &'static str) -> Self {
        let capacity = 4096;
        Self { label, buffer: Self::alloc(device, label, capacity), capacity, count: 0 }
    }

    fn alloc(device: &wgpu::Device, label: &str, size: u64) -> wgpu::Buffer {
        device.create_buffer(&wgpu::BufferDescriptor {
            label: Some(label),
            size,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        })
    }

    fn upload(&mut self, gpu: &Gpu, data: &[Vertex]) {
        let bytes: &[u8] = bytemuck::cast_slice(data);
        if bytes.len() as u64 > self.capacity {
            self.capacity = (bytes.len() as u64).next_power_of_two();
            self.buffer = Self::alloc(&gpu.device, self.label, self.capacity);
        }
        if !bytes.is_empty() {
            gpu.queue.write_buffer(&self.buffer, 0, bytes);
        }
        self.count = data.len() as u32;
    }

    fn draw(&self, pass: &mut wgpu::RenderPass<'_>, pipeline: &wgpu::RenderPipeline) {
        if self.count == 0 {
            return;
        }
        pass.set_pipeline(pipeline);
        pass.set_vertex_buffer(0, self.buffer.slice(..));
        pass.draw(0..self.count, 0..1);
    }
}

/// Offscreen targets: the low-res scene, the HUD layer at `hud_scale` times that
/// resolution, and a composite of both (used for screenshots).
struct Target {
    size: UVec2,
    hud_size: UVec2,
    color: wgpu::TextureView,
    depth: wgpu::TextureView,
    hud: wgpu::TextureView,
    composite: wgpu::TextureView,
    blit: wgpu::BindGroup,
}

pub(crate) struct Renderer {
    /// How long the last `render` waited for the next surface texture (vsync).
    pub(crate) wait: std::time::Duration,
    low_height: u32,
    hud_scale: u32,
    /// Fixed framebuffer aspect ratio (for reproducible screenshots), else follow the window.
    forced_aspect: Option<f32>,
    target: Target,
    globals: wgpu::Buffer,
    globals_bind: wgpu::BindGroup,
    blit_layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    sky_pipe: wgpu::RenderPipeline,
    solid_pipe: wgpu::RenderPipeline,
    line_pipe: wgpu::RenderPipeline,
    point_pipe: wgpu::RenderPipeline,
    hud_tri_pipe: wgpu::RenderPipeline,
    hud_pipe: wgpu::RenderPipeline,
    blit_pipe: wgpu::RenderPipeline,
    /// Same as `blit_pipe`, but into the RGBA composite texture for screenshots.
    capture_pipe: wgpu::RenderPipeline,
    sky: DynBuffer,
    solids: DynBuffer,
    lines: DynBuffer,
    points: DynBuffer,
    hud_tris: DynBuffer,
    hud: DynBuffer,
}

impl Renderer {
    pub fn new(gpu: &Gpu, low_height: u32, hud_scale: u32, forced_aspect: Option<f32>) -> Self {
        let device = &gpu.device;

        let globals = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("globals"),
            size: size_of::<Globals>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let globals_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("globals"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });
        let globals_bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("globals"),
            layout: &globals_layout,
            entries: &[wgpu::BindGroupEntry { binding: 0, resource: globals.as_entire_binding() }],
        });

        let scene = device.create_shader_module(wgpu::include_wgsl!("shaders/scene.wgsl"));
        let scene_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("scene"),
            bind_group_layouts: &[Some(&globals_layout)],
            immediate_size: 0,
        });

        let vertex_layout = wgpu::VertexBufferLayout {
            array_stride: size_of::<Vertex>() as u64,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x4],
        };

        let scene_pipeline = |label: &str,
                              vs: &str,
                              topology: wgpu::PrimitiveTopology,
                              depth: Option<(bool, wgpu::CompareFunction)>,
                              blend: wgpu::BlendState| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(label),
                layout: Some(&scene_layout),
                vertex: wgpu::VertexState {
                    module: &scene,
                    entry_point: Some(vs),
                    compilation_options: Default::default(),
                    buffers: &[Some(vertex_layout.clone())],
                },
                primitive: wgpu::PrimitiveState { topology, ..Default::default() },
                depth_stencil: depth.map(|(write, compare)| wgpu::DepthStencilState {
                    format: DEPTH_FORMAT,
                    depth_write_enabled: Some(write),
                    depth_compare: Some(compare),
                    stencil: Default::default(),
                    bias: Default::default(),
                }),
                multisample: Default::default(),
                fragment: Some(wgpu::FragmentState {
                    module: &scene,
                    entry_point: Some("fs_color"),
                    compilation_options: Default::default(),
                    targets: &[Some(wgpu::ColorTargetState {
                        format: COLOR_FORMAT,
                        blend: Some(blend),
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                }),
                multiview_mask: None,
                cache: None,
            })
        };
        use wgpu::CompareFunction as Cmp;
        use wgpu::PrimitiveTopology as Topo;
        let alpha = wgpu::BlendState::ALPHA_BLENDING;
        // Sky points add up, so dense star fields glow like a real galaxy.
        let additive = wgpu::BlendState {
            color: wgpu::BlendComponent {
                src_factor: wgpu::BlendFactor::One,
                dst_factor: wgpu::BlendFactor::One,
                operation: wgpu::BlendOperation::Add,
            },
            alpha: wgpu::BlendComponent::OVER,
        };
        let sky_pipe = scene_pipeline("sky", "vs_sky", Topo::PointList, Some((false, Cmp::Always)), additive);
        let solid_pipe = scene_pipeline("solids", "vs_world", Topo::TriangleList, Some((true, Cmp::Greater)), alpha);
        let line_pipe = scene_pipeline("lines", "vs_line", Topo::LineList, Some((false, Cmp::GreaterEqual)), alpha);
        let point_pipe = scene_pipeline("points", "vs_line", Topo::PointList, Some((false, Cmp::GreaterEqual)), alpha);
        // The HUD has its own layer without depth.
        let hud_tri_pipe = scene_pipeline("hud tris", "vs_hud", Topo::TriangleList, None, alpha);
        let hud_pipe = scene_pipeline("hud", "vs_hud", Topo::LineList, None, alpha);

        let texture_entry = |binding| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Texture {
                sample_type: wgpu::TextureSampleType::Float { filterable: true },
                view_dimension: wgpu::TextureViewDimension::D2,
                multisampled: false,
            },
            count: None,
        };
        let blit_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("blit"),
            entries: &[
                texture_entry(0),
                texture_entry(1),
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });
        let blit = device.create_shader_module(wgpu::include_wgsl!("shaders/blit.wgsl"));
        let blit_layout_pipeline = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("blit"),
            bind_group_layouts: &[Some(&blit_layout)],
            immediate_size: 0,
        });
        let blit_pipeline = |label: &str, format: wgpu::TextureFormat| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(label),
                layout: Some(&blit_layout_pipeline),
                vertex: wgpu::VertexState {
                    module: &blit,
                    entry_point: Some("vs_main"),
                    compilation_options: Default::default(),
                    buffers: &[],
                },
                primitive: Default::default(),
                depth_stencil: None,
                multisample: Default::default(),
                fragment: Some(wgpu::FragmentState {
                    module: &blit,
                    entry_point: Some("fs_main"),
                    compilation_options: Default::default(),
                    targets: &[Some(format.into())],
                }),
                multiview_mask: None,
                cache: None,
            })
        };
        let blit_pipe = blit_pipeline("blit", gpu.config.format);
        let capture_pipe = blit_pipeline("capture", COLOR_FORMAT);
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("nearest"),
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });

        let target = Self::create_target(gpu, low_height, hud_scale, forced_aspect, &blit_layout, &sampler);
        Self {
            wait: std::time::Duration::ZERO,
            low_height,
            hud_scale,
            forced_aspect,
            target,
            globals,
            globals_bind,
            blit_layout,
            sampler,
            sky_pipe,
            solid_pipe,
            line_pipe,
            point_pipe,
            hud_tri_pipe,
            hud_pipe,
            blit_pipe,
            capture_pipe,
            sky: DynBuffer::new(device, "sky"),
            solids: DynBuffer::new(device, "solids"),
            lines: DynBuffer::new(device, "lines"),
            points: DynBuffer::new(device, "points"),
            hud_tris: DynBuffer::new(device, "hud tris"),
            hud: DynBuffer::new(device, "hud"),
        }
    }

    /// Low-res size: fixed height, width follows the window aspect ratio.
    fn low_res_size(gpu: &Gpu, low_height: u32, forced_aspect: Option<f32>) -> UVec2 {
        let aspect = forced_aspect.unwrap_or(gpu.config.width as f32 / gpu.config.height as f32);
        UVec2::new(((low_height as f32 * aspect).round() as u32).max(1), low_height)
    }

    fn create_target(
        gpu: &Gpu,
        low_height: u32,
        hud_scale: u32,
        forced_aspect: Option<f32>,
        layout: &wgpu::BindGroupLayout,
        sampler: &wgpu::Sampler,
    ) -> Target {
        let size = Self::low_res_size(gpu, low_height, forced_aspect);
        let hud_size = size * hud_scale;
        let texture = |label, size: UVec2, format, usage| {
            gpu.device
                .create_texture(&wgpu::TextureDescriptor {
                    label: Some(label),
                    size: wgpu::Extent3d { width: size.x, height: size.y, depth_or_array_layers: 1 },
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format,
                    usage,
                    view_formats: &[],
                })
                .create_view(&Default::default())
        };
        use wgpu::TextureUsages as U;
        let color = texture("low-res color", size, COLOR_FORMAT, U::RENDER_ATTACHMENT | U::TEXTURE_BINDING);
        let depth = texture("low-res depth", size, DEPTH_FORMAT, U::RENDER_ATTACHMENT);
        let hud = texture("hud", hud_size, COLOR_FORMAT, U::RENDER_ATTACHMENT | U::TEXTURE_BINDING);
        let composite = texture("composite", hud_size, COLOR_FORMAT, U::RENDER_ATTACHMENT | U::COPY_SRC);
        let blit = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("blit"),
            layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(&color) },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(&hud) },
                wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::Sampler(sampler) },
            ],
        });
        Target { size, hud_size, color, depth, hud, composite, blit }
    }

    pub fn resize(&mut self, gpu: &Gpu) {
        if Self::low_res_size(gpu, self.low_height, self.forced_aspect) != self.target.size {
            self.target =
                Self::create_target(gpu, self.low_height, self.hud_scale, self.forced_aspect, &self.blit_layout, &self.sampler);
        }
    }

    pub fn low_res(&self) -> UVec2 {
        self.target.size
    }

    pub fn hud_size(&self) -> UVec2 {
        self.target.hud_size
    }

    /// Render `frame`; if `capture` is set, also save the composited image (at HUD resolution) as PNG.
    pub fn render(&mut self, gpu: &mut Gpu, frame: &Frame, capture: Option<&Path>) {
        let size = self.target.size.as_vec2();
        let hud = self.target.hud_size.as_vec2();
        let globals = Globals {
            view_proj: frame.camera.view_proj(size.x / size.y).to_cols_array_2d(),
            hud_proj: orthographic(0.0, hud.x, hud.y, 0.0, -1.0, 1.0).to_cols_array_2d(),
        };
        let upload = universe_prof::scope("render/upload vertices");
        gpu.queue.write_buffer(&self.globals, 0, bytemuck::bytes_of(&globals));
        self.sky.upload(gpu, &frame.sky);
        self.solids.upload(gpu, &frame.solids);
        self.lines.upload(gpu, &frame.lines);
        self.points.upload(gpu, &frame.points);
        self.hud_tris.upload(gpu, &frame.hud_tris);
        self.hud.upload(gpu, &frame.hud);
        drop(upload);

        let acquire = std::time::Instant::now();
        let next = gpu.surface.get_current_texture();
        self.wait = acquire.elapsed();
        let surface_texture = match next {
            wgpu::CurrentSurfaceTexture::Success(t) | wgpu::CurrentSurfaceTexture::Suboptimal(t) => t,
            wgpu::CurrentSurfaceTexture::Outdated | wgpu::CurrentSurfaceTexture::Lost => {
                gpu.surface.configure(&gpu.device, &gpu.config);
                return;
            }
            _ => return,
        };
        let surface_view = surface_texture.texture.create_view(&Default::default());

        let mut encoder = gpu.device.create_command_encoder(&Default::default());
        {
            let [r, g, b, a] = frame.clear.0.map(f64::from);
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("scene"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &self.target.color,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color { r, g, b, a }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.target.depth,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(0.0),
                        store: wgpu::StoreOp::Discard,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_bind_group(0, &self.globals_bind, &[]);
            self.sky.draw(&mut pass, &self.sky_pipe);
            self.solids.draw(&mut pass, &self.solid_pipe);
            self.lines.draw(&mut pass, &self.line_pipe);
            self.points.draw(&mut pass, &self.point_pipe);
        }
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("hud"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &self.target.hud,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT), store: wgpu::StoreOp::Store },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_bind_group(0, &self.globals_bind, &[]);
            self.hud_tris.draw(&mut pass, &self.hud_tri_pipe);
            self.hud.draw(&mut pass, &self.hud_pipe);
        }
        let readback = capture.map(|_| {
            self.composite(&mut encoder, &self.target.composite, &self.capture_pipe);
            self.copy_to_buffer(gpu, &mut encoder)
        });
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("upscale"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &surface_view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(&self.blit_pipe);
            pass.set_bind_group(0, &self.target.blit, &[]);
            pass.draw(0..3, 0..1);
        }
        gpu.queue.submit([encoder.finish()]);
        gpu.queue.present(surface_texture);

        if let (Some(path), Some((buffer, padded_row))) = (capture, readback) {
            match self.save_png(gpu, &buffer, padded_row, path) {
                Ok(()) => log::info!("screenshot saved to {}", path.display()),
                Err(e) => log::error!("screenshot failed: {e}"),
            }
        }
    }

    /// Draw scene + HUD into `view` with `pipeline` (the screenshot path).
    fn composite(&self, encoder: &mut wgpu::CommandEncoder, view: &wgpu::TextureView, pipeline: &wgpu::RenderPipeline) {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("composite"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color::BLACK), store: wgpu::StoreOp::Store },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        pass.set_pipeline(pipeline);
        pass.set_bind_group(0, &self.target.blit, &[]);
        pass.draw(0..3, 0..1);
    }

    fn copy_to_buffer(&self, gpu: &Gpu, encoder: &mut wgpu::CommandEncoder) -> (wgpu::Buffer, u32) {
        let size = self.target.hud_size;
        let padded_row = (size.x * 4).next_multiple_of(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT);
        let buffer = gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("screenshot"),
            size: (padded_row * size.y) as u64,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: self.target.composite.texture(),
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(padded_row),
                    rows_per_image: None,
                },
            },
            wgpu::Extent3d { width: size.x, height: size.y, depth_or_array_layers: 1 },
        );
        (buffer, padded_row)
    }

    fn save_png(&self, gpu: &Gpu, buffer: &wgpu::Buffer, padded_row: u32, path: &Path) -> Result<(), String> {
        let size = self.target.hud_size;
        buffer.slice(..).map_async(wgpu::MapMode::Read, |_| {});
        gpu.device.poll(wgpu::PollType::wait_indefinitely()).map_err(|e| e.to_string())?;
        let mapped = buffer.slice(..).get_mapped_range().map_err(|e| e.to_string())?;
        let mut pixels = Vec::with_capacity((size.x * size.y * 4) as usize);
        for row in mapped.chunks(padded_row as usize) {
            pixels.extend_from_slice(&row[..(size.x * 4) as usize]);
        }
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        }
        let file = std::fs::File::create(path).map_err(|e| e.to_string())?;
        let mut encoder = png::Encoder::new(std::io::BufWriter::new(file), size.x, size.y);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header().map_err(|e| e.to_string())?;
        writer.write_image_data(&pixels).map_err(|e| e.to_string())
    }
}
