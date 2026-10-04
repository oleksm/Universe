//! Is the sun behind something? Asked of what's actually drawn: the depth
//! buffer, read on the GPU at points over the sun's disc on screen
//! (`shaders/sunprobe.wgsl`), after the scene and after the front layer. The
//! answer comes back a frame or two later (`sun_seen`), for the glare: a
//! station's structure, a ship, a building, the hull round the eye all hide
//! it alike, without each being reckoned on its own.

use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Arc;

/// Points read over the disc (the shader's workgroup).
const POINTS: u64 = 64;
/// Readbacks in flight at once.
const SLOTS: usize = 3;

/// What share of the sun's disc was last seen clear (1: all of it), as f32 bits.
static SEEN: AtomicU32 = AtomicU32::new(0x3f80_0000);

/// What share of the sun's disc the eye last saw clear of everything drawn
/// (1: all of it; 0: none). A frame or two behind; 1 until first asked.
pub fn sun_seen() -> f32 {
    f32::from_bits(SEEN.load(Ordering::Relaxed))
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Params {
    centre: [f32; 2],
    radius: f32,
    threshold: f32,
    size: [f32; 2],
    mode: u32,
    pad: u32,
}

pub(crate) struct SunProbe {
    pipe: wgpu::ComputePipeline,
    layout: wgpu::BindGroupLayout,
    /// The two runs' parameters (after the scene, after the front layer).
    params: [wgpu::Buffer; 2],
    marks: wgpu::Buffer,
    staging: Vec<(wgpu::Buffer, Arc<AtomicBool>, bool)>,
    next: usize,
    /// This frame's run copied out to this slot, to be mapped once submitted.
    copied: Option<usize>,
}

impl SunProbe {
    pub fn new(device: &wgpu::Device) -> Self {
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("sun probe"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Texture { sample_type: wgpu::TextureSampleType::Depth, view_dimension: wgpu::TextureViewDimension::D2, multisampled: true },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry { binding: 1, visibility: wgpu::ShaderStages::COMPUTE, ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Uniform, has_dynamic_offset: false, min_binding_size: None }, count: None },
                wgpu::BindGroupLayoutEntry { binding: 2, visibility: wgpu::ShaderStages::COMPUTE, ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Storage { read_only: false }, has_dynamic_offset: false, min_binding_size: None }, count: None },
            ],
        });
        let shader = device.create_shader_module(wgpu::include_wgsl!("shaders/sunprobe.wgsl"));
        let pl = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor { label: Some("sun probe"), bind_group_layouts: &[Some(&layout)], immediate_size: 0 });
        let pipe = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor { label: Some("sun probe"), layout: Some(&pl), module: &shader, entry_point: Some("main"), compilation_options: Default::default(), cache: None });
        let buffer = |label, size, usage| device.create_buffer(&wgpu::BufferDescriptor { label: Some(label), size, usage, mapped_at_creation: false });
        let uniform = wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST;
        let params = [buffer("sun probe (scene)", size_of::<Params>() as u64, uniform), buffer("sun probe (front)", size_of::<Params>() as u64, uniform)];
        let marks = buffer("sun probe marks", POINTS * 4, wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC);
        let staging = (0..SLOTS).map(|_| (buffer("sun probe readback", POINTS * 4, wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST), Arc::new(AtomicBool::new(false)), false)).collect();
        SunProbe { pipe, layout, params, marks, staging, next: 0, copied: None }
    }

    /// Answers come back: each readback mapped since, read into `sun_seen`.
    pub fn collect(&mut self) {
        for (buffer, ready, busy) in &mut self.staging {
            if *busy && ready.load(Ordering::Acquire) {
                if let Ok(data) = buffer.slice(..).get_mapped_range() {
                    let marks: &[u32] = bytemuck::cast_slice(&data);
                    let seen = marks.iter().filter(|&&m| m != 2).count();
                    let covered = marks.iter().filter(|&&m| m == 1).count();
                    let clear = if seen == 0 { 1.0 } else { 1.0 - covered as f32 / seen as f32 };
                    SEEN.store(clear.to_bits(), Ordering::Relaxed);
                }
                buffer.unmap();
                ready.store(false, Ordering::Release);
                *busy = false;
            }
        }
    }

    /// Where to look this frame: the disc's centre on screen (pixels), its
    /// radius, the depth just this side of the sun, the screen's size.
    pub fn aim(&self, queue: &wgpu::Queue, centre: [f32; 2], radius: f32, threshold: f32, size: [f32; 2]) {
        for (mode, buffer) in self.params.iter().enumerate() {
            let p = Params { centre, radius, threshold, size, mode: mode as u32, pad: 0 };
            queue.write_buffer(buffer, 0, bytemuck::bytes_of(&p));
        }
    }

    /// Read `depth` over the disc: `front` false after the scene (marks set
    /// afresh), true after the front layer (covered there too).
    pub fn run(&self, device: &wgpu::Device, encoder: &mut wgpu::CommandEncoder, depth: &wgpu::TextureView, front: bool) {
        let bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("sun probe"),
            layout: &self.layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(depth) },
                wgpu::BindGroupEntry { binding: 1, resource: self.params[usize::from(front)].as_entire_binding() },
                wgpu::BindGroupEntry { binding: 2, resource: self.marks.as_entire_binding() },
            ],
        });
        let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor { label: Some("sun probe"), timestamp_writes: None });
        pass.set_pipeline(&self.pipe);
        pass.set_bind_group(0, &bind, &[]);
        pass.dispatch_workgroups(1, 1, 1);
    }

    /// The marks copied out for reading back (if a slot is free).
    pub fn copy_out(&mut self, encoder: &mut wgpu::CommandEncoder) {
        let slot = self.next;
        if self.staging[slot].2 {
            return;
        }
        encoder.copy_buffer_to_buffer(&self.marks, 0, &self.staging[slot].0, 0, POINTS * 4);
        self.copied = Some(slot);
        self.next = (slot + 1) % SLOTS;
    }

    /// Once submitted: what was copied out, mapped to be read when it's done.
    pub fn submitted(&mut self) {
        let Some(slot) = self.copied.take() else { return };
        let (buffer, ready, busy) = &mut self.staging[slot];
        *busy = true;
        let ready = ready.clone();
        buffer.slice(..).map_async(wgpu::MapMode::Read, move |r| ready.store(r.is_ok(), Ordering::Release));
    }
}
