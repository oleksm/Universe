use std::sync::Arc;

use winit::window::Window;

/// Owns the wgpu device and the window surface.
pub struct Gpu {
    pub surface: wgpu::Surface<'static>,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub config: wgpu::SurfaceConfiguration,
}

impl Gpu {
    pub fn new(
        window: Arc<Window>,
        display: winit::event_loop::OwnedDisplayHandle,
        vsync: bool,
    ) -> Self {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_with_display_handle_from_env(
            Box::new(display),
        ));
        let surface = instance
            .create_surface(window.clone())
            .expect("failed to create surface");

        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            force_fallback_adapter: false,
            compatible_surface: Some(&surface),
            apply_limit_buckets: false,
        }))
        .expect("no suitable GPU adapter");
        log::info!("GPU: {:?}", adapter.get_info());

        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("device"),
            // (Block-compressed textures where the GPU has them: a model's maps at an
            // eighth to a quarter of the memory.)
            // (And the GPU's own clock at each pass, where it has one: see `gputime`.)
            // (And a cache of compiled pipelines kept between runs, where the driver has one: see
            // `pipecache`.)
            required_features: adapter.features() & (wgpu::Features::TEXTURE_COMPRESSION_BC | wgpu::Features::TIMESTAMP_QUERY | wgpu::Features::PIPELINE_CACHE),
            // (One vertex input and one value between the stages past the defaults' 16: the near
            // ground's surface fields. Every desktop GPU has 28 or more of each.)
            required_limits: wgpu::Limits {
                max_vertex_attributes: adapter.limits().max_vertex_attributes.min(32),
                max_inter_stage_shader_variables: adapter.limits().max_inter_stage_shader_variables.min(32),
                // (A world's maps, tables and clouds beside the shadows and globes: past 16.)
                max_sampled_textures_per_shader_stage: adapter.limits().max_sampled_textures_per_shader_stage.min(32),
                ..wgpu::Limits::default()
            },
            experimental_features: wgpu::ExperimentalFeatures::disabled(),
            memory_hints: wgpu::MemoryHints::Performance,
            trace: wgpu::Trace::Off,
        }))
        .expect("failed to create device");
        crate::pipecache::open(&device, &adapter.get_info());

        let size = window.inner_size();
        let mut config = surface
            .get_default_config(&adapter, size.width.max(1), size.height.max(1))
            .expect("surface not supported by adapter");
        // Prefer a non-sRGB swapchain so palette colors pass through exactly as authored.
        let caps = surface.get_capabilities(&adapter);
        if let Some(f) = caps.formats.iter().find(|f| !f.is_srgb()) {
            config.format = *f;
        }
        config.present_mode = if vsync {
            wgpu::PresentMode::AutoVsync
        } else {
            wgpu::PresentMode::AutoNoVsync
        };
        surface.configure(&device, &config);

        Self { surface, device, queue, config }
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        if width == 0 || height == 0 {
            return;
        }
        self.config.width = width;
        self.config.height = height;
        self.surface.configure(&self.device, &self.config);
    }
}
