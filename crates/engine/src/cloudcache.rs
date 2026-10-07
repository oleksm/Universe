//! The bound world's clouds, cached (the lab's design: clouds.wgsl, "The cloud cache"): each
//! shell's smooth noise over four nested levels round the point under the eye (8,000, 800, 80 and
//! 8 km across), 512² texels a level, so a pixel's clouds are a few texture
//! reads, not the noise. Filled by a compute pass a few layers a frame; refreshed round-robin (the
//! clouds move slowly); re-centred when the eye has moved an eighth of the inner level (1 km); each change
//! eased in from the copy before it over `FADE_S`, never at once.

use glam::DVec3;

/// Texels a side, layers (four levels, three shells each).
const N: u32 = 512;
const LAYERS: u32 = 12;
/// Each level's half-width (m), outer to inner.
const HALF: [f64; 4] = [4.0e6, 4.0e5, 4.0e4, 4.0e3];
/// Layers filled a frame.
const PER_FRAME: u32 = 2;
/// How long a change takes to come in (s).
const FADE_S: f32 = 1.5;
/// Re-centred when the point under the eye is this share of the inner level's width from the centre.
const RECENTRE: f64 = 1.0 / 8.0;

/// The shaders' `CloudCache`.
#[repr(C)]
#[derive(Clone, Copy, Default, bytemuck::Pod, bytemuck::Zeroable)]
struct Uniform {
    center: [f32; 4],
    e1: [f32; 4],
    e2: [f32; 4],
    half_m: [f32; 4],
    fade: [f32; 4],
    old_center: [f32; 4],
    old_e1: [f32; 4],
    old_e2: [f32; 4],
}

/// A frame of the levels: centre (body-fixed unit direction) and its tangent axes.
#[derive(Clone, Copy)]
struct Frame3 {
    c: DVec3,
    e1: DVec3,
    e2: DVec3,
}

impl Frame3 {
    fn at(c: DVec3) -> Self {
        let e1 = c.any_orthonormal_vector();
        Frame3 { c, e1, e2: c.cross(e1) }
    }
}

/// What's being filled: these levels, and how many of their layers are done.
struct Job {
    levels: Vec<usize>,
    next: usize,
}

pub(crate) struct CloudCache {
    new: wgpu::Texture,
    before: wgpu::Texture,
    pub new_view: wgpu::TextureView,
    pub old_view: wgpu::TextureView,
    store_view: wgpu::TextureView,
    pub uniform: wgpu::Buffer,
    pipe: wgpu::ComputePipeline,
    layout: wgpu::BindGroupLayout,
    /// The maps' set it was filled for (their id), the frames now and before, the fades.
    world: Option<u64>,
    radius: f64,
    frame: Option<Frame3>,
    old: Option<Frame3>,
    fade: [f32; 4],
    job: Option<Job>,
    /// The level refreshed next (round-robin).
    turn: usize,
    last: std::time::Instant,
}

impl CloudCache {
    pub fn new(device: &wgpu::Device) -> Self {
        let texture = |label| {
            device.create_texture(&wgpu::TextureDescriptor {
                label: Some(label),
                size: wgpu::Extent3d { width: N, height: N, depth_or_array_layers: LAYERS },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba16Float,
                usage: wgpu::TextureUsages::STORAGE_BINDING | wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_SRC | wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
            })
        };
        let (new, before) = (texture("cloud cache"), texture("cloud cache (before)"));
        let array = wgpu::TextureViewDescriptor { dimension: Some(wgpu::TextureViewDimension::D2Array), ..Default::default() };
        let (new_view, old_view, store_view) = (new.create_view(&array), before.create_view(&array), new.create_view(&array));
        let buffer = |label, size| device.create_buffer(&wgpu::BufferDescriptor { label: Some(label), size, usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST, mapped_at_creation: false });
        let uniform = buffer("cloud cache", size_of::<Uniform>() as u64);
        let u = |binding| wgpu::BindGroupLayoutEntry { binding, visibility: wgpu::ShaderStages::COMPUTE, ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Uniform, has_dynamic_offset: false, min_binding_size: None }, count: None };
        let t = |binding| wgpu::BindGroupLayoutEntry { binding, visibility: wgpu::ShaderStages::COMPUTE, ty: wgpu::BindingType::Texture { sample_type: wgpu::TextureSampleType::Float { filterable: true }, view_dimension: wgpu::TextureViewDimension::D2, multisampled: false }, count: None };
        let store = wgpu::BindGroupLayoutEntry { binding: 2, visibility: wgpu::ShaderStages::COMPUTE, ty: wgpu::BindingType::StorageTexture { access: wgpu::StorageTextureAccess::WriteOnly, format: wgpu::TextureFormat::Rgba16Float, view_dimension: wgpu::TextureViewDimension::D2Array }, count: None };
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor { label: Some("cloud cache fill"), entries: &[u(0), u(1), store, t(3), t(4), t(5), u(6)] });
        let module = crate::shaders::make(device, "cloud cache fill", crate::shaders::cloud_cache());
        let pl = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor { label: Some("cloud cache fill"), bind_group_layouts: &[Some(&layout)], immediate_size: 0 });
        let pipe = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor { label: Some("cloud cache fill"), layout: Some(&pl), module: &module, entry_point: Some("cloud_cache_fill"), compilation_options: Default::default(), cache: None });
        CloudCache { new, before, new_view, old_view, store_view, uniform, pipe, layout, world: None, radius: 1.0, frame: None, old: None, fade: [1.0; 4], job: None, turn: 0, last: std::time::Instant::now() }
    }

    /// This frame's part: the world bound (its maps' id, radius, the clouds' uniform and maps), the
    /// point under the eye (body-fixed direction). Fills, refreshes, re-centres; writes the uniform.
    /// None for `world`: no clouds bound (the cache is let go).
    #[allow(clippy::too_many_arguments)]
    pub fn frame(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, encoder: &mut wgpu::CommandEncoder, world: Option<(u64, f64, &wgpu::Buffer, &[wgpu::TextureView; 3])>, under: DVec3) {
        let dt = self.last.elapsed().as_secs_f32().min(0.25);
        self.last = std::time::Instant::now();
        let Some((id, radius, clouds, maps)) = world else {
            self.world = None;
            self.frame = None;
            self.job = None;
            queue.write_buffer(&self.uniform, 0, bytemuck::bytes_of(&Uniform::default()));
            return;
        };
        if self.world != Some(id) {
            (self.world, self.radius, self.frame, self.old, self.fade, self.job, self.turn) = (Some(id), radius, None, None, [1.0; 4], None, 0);
        }
        // (A change being eased in.)
        let filling: Vec<usize> = self.job.as_ref().map_or(Vec::new(), |j| j.levels.clone());
        for (l, f) in self.fade.iter_mut().enumerate() {
            if !filling.contains(&l) && self.frame.is_some() {
                *f = (*f + dt / FADE_S).min(1.0);
            }
        }
        // (Nothing filling and nothing easing in: the next job. A re-centring first, if the eye's
        // gone far enough; else the next level round.)
        if self.job.is_none() && self.fade.iter().all(|f| *f >= 1.0) {
            let off = self.frame.map(|f| (under - f.c * under.dot(f.c)).length() * self.radius);
            let recentre = off.is_none_or(|o| o > RECENTRE * 2.0 * HALF[3]);
            let levels = if recentre { vec![0, 1, 2, 3] } else { vec![self.turn] };
            if !recentre {
                self.turn = (self.turn + 1) % HALF.len();
            }
            // (What's there now kept as the copy before, with its frame; the new frame set.)
            encoder.copy_texture_to_texture(self.new.as_image_copy(), self.before.as_image_copy(), wgpu::Extent3d { width: N, height: N, depth_or_array_layers: LAYERS });
            self.old = self.frame;
            if recentre {
                self.frame = Some(Frame3::at(under));
            }
            for &l in &levels {
                self.fade[l] = 0.0;
            }
            self.job = Some(Job { levels, next: 0 });
        }
        let frame = self.frame.expect("a frame once a job's begun");
        self.write(queue, frame);
        // A few of the job's layers filled.
        if let Some(job) = self.job.as_mut() {
            let layers: Vec<u32> = job.levels.iter().flat_map(|&l| (0..3).map(move |s| (l * 3 + s) as u32)).collect();
            let take: Vec<u32> = layers.iter().copied().skip(job.next).take(PER_FRAME as usize).collect();
            job.next += take.len();
            for &layer in &take {
                // (Each dispatch its own offset buffer: the queue's writes all land before the encoder runs.)
                let off = device.create_buffer(&wgpu::BufferDescriptor { label: Some("cloud cache layer"), size: 16, usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST, mapped_at_creation: false });
                queue.write_buffer(&off, 0, bytemuck::cast_slice(&[layer, 0u32, 0, 0]));
                let bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("cloud cache fill"),
                    layout: &self.layout,
                    entries: &[
                        wgpu::BindGroupEntry { binding: 0, resource: self.uniform.as_entire_binding() },
                        wgpu::BindGroupEntry { binding: 1, resource: clouds.as_entire_binding() },
                        wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::TextureView(&self.store_view) },
                        wgpu::BindGroupEntry { binding: 3, resource: wgpu::BindingResource::TextureView(&maps[0]) },
                        wgpu::BindGroupEntry { binding: 4, resource: wgpu::BindingResource::TextureView(&maps[1]) },
                        wgpu::BindGroupEntry { binding: 5, resource: wgpu::BindingResource::TextureView(&maps[2]) },
                        wgpu::BindGroupEntry { binding: 6, resource: off.as_entire_binding() },
                    ],
                });
                let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor { label: Some("cloud cache fill"), timestamp_writes: None });
                pass.set_pipeline(&self.pipe);
                pass.set_bind_group(0, &bind, &[]);
                pass.dispatch_workgroups(N / 8, N / 8, 1);
            }
            if job.next >= layers.len() {
                // (The first fill has nothing before it to ease from.)
                if self.old.is_none() {
                    self.fade = [1.0; 4];
                }
                self.job = None;
            }
        }
    }

    fn write(&self, queue: &wgpu::Queue, frame: Frame3) {
        let v = |d: DVec3, w: f32| [d.x as f32, d.y as f32, d.z as f32, w];
        let old = self.old.unwrap_or(frame);
        // (Data once the first fill is done.)
        let has = (self.old.is_some() || self.job.is_none()) as u32 as f32;
        let u = Uniform {
            center: v(frame.c, self.radius as f32),
            e1: v(frame.e1, has),
            e2: v(frame.e2, 0.0),
            half_m: HALF.map(|h| h as f32),
            fade: self.fade,
            old_center: v(old.c, self.radius as f32),
            old_e1: v(old.e1, 1.0),
            old_e2: v(old.e2, 0.0),
        };
        queue.write_buffer(&self.uniform, 0, bytemuck::bytes_of(&u));
    }
}
