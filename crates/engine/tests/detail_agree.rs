//! The ground's runtime detail on the GPU (`shaders/detail.wgsl`) held to the truth in f64
//! (`world::detail::offset`) within `AGREE_M`, over a synthetic tile with its halo. Skipped where
//! there's no GPU.

use universe_world::detail;

/// Samples a side of the synthetic tile (as the ~150 m tiles: `n + 1`), its spacing (m), and the
/// halo past its edges.
const N: i64 = 1024;
const SPACING: f64 = 159.0;
const HALO: i64 = 2;
/// Points tried, a side.
const POINTS: u32 = 16;

fn height(i: i64, j: i64) -> f64 {
    let (x, y) = (i as f64, j as f64);
    2000.0 + 400.0 * (x * 0.013).sin() * (y * 0.021).cos() + 30.0 * (x * 0.31 + y * 0.17).sin()
}

fn fields(i: i64, j: i64) -> [f64; 4] {
    let k = (i * 31 + j * 17).rem_euclid(256) as f64;
    [k, (k * 0.5).floor(), 64.0, if (i + j) % 7 == 0 { 255.0 } else { 0.0 }]
}

#[test]
fn detail_on_the_gpu_agrees_with_the_truth() {
    let instance = wgpu::Instance::default();
    let Ok(adapter) = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default())) else {
        eprintln!("no GPU: skipped");
        return;
    };
    let Ok((device, queue)) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default())) else {
        eprintln!("no GPU device: skipped");
        return;
    };
    let (seed, rock, cell) = (detail::seed("TRD1"), 3u32, detail::PHYSICS_CELL_M);
    // The places tried, spread over the tile and up to its edges.
    let at: Vec<[f64; 2]> = (0..POINTS * POINTS).map(|k| [((k % POINTS) as f64 + 0.37) / POINTS as f64 * N as f64 * SPACING, ((k / POINTS) as f64 + 0.61) / POINTS as f64 * N as f64 * SPACING]).collect();
    // (A tile far out on its face, as Heath's are: millions of metres from the face's corner.)
    let origin = [4_123_456i64, 1_234_567i64];
    let want: Vec<f64> = at.iter().map(|&at| detail::offset(&detail::Site { origin, at, spacing: SPACING, height: &height, fields: &fields, rock, seed, cell })).collect();

    // The tile with its halo, row by row from (-HALO, -HALO): heights, and fields at half as many.
    let side = N + 1 + 2 * HALO;
    let heights: Vec<f32> = (0..side * side).map(|k| height(k % side - HALO, k / side - HALO) as f32).collect();
    let fside = N / 2 + 1 + 2 * HALO;
    let fieldv: Vec<[f32; 4]> = (0..fside * fside).map(|k| fields(k % fside - HALO, k / fside - HALO).map(|v| v as f32)).collect();
    let places: Vec<[f32; 2]> = at.iter().map(|a| [a[0] as f32, a[1] as f32]).collect();
    let harness = format!(
        "{}\n{}",
        include_str!("../src/shaders/detail.wgsl"),
        r#"
struct Params { spacing: f32, cell: f32, rock: u32, side: i32, fside: i32, halo: i32, seed: vec2<u32>, origin: vec2<i32> };
@group(0) @binding(0) var<uniform> p: Params;
@group(0) @binding(1) var<storage, read> heights: array<f32>;
@group(0) @binding(2) var<storage, read> fieldv: array<vec4<f32>>;
@group(0) @binding(3) var<storage, read> places: array<vec2<f32>>;
@group(0) @binding(4) var<storage, read_write> out: array<f32>;
fn detail_height(i: i32, j: i32) -> f32 { return heights[(j + p.halo) * p.side + i + p.halo]; }
fn detail_fields(i: i32, j: i32) -> vec4<f32> { return fieldv[(j + p.halo) * p.fside + i + p.halo]; }
@compute @workgroup_size(64)
fn main(@builtin(global_invocation_id) id: vec3<u32>) {
    if (id.x >= arrayLength(&places)) { return; }
    out[id.x] = detail(p.origin, places[id.x], p.spacing, p.rock, p.seed, p.cell);
}
"#
    );
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor { label: Some("detail agreement"), source: wgpu::ShaderSource::Wgsl(harness.into()) });
    let buffer = |bytes: &[u8], usage: wgpu::BufferUsages| {
        let b = device.create_buffer(&wgpu::BufferDescriptor { label: None, size: bytes.len().max(16) as u64, usage: usage | wgpu::BufferUsages::COPY_DST, mapped_at_creation: false });
        queue.write_buffer(&b, 0, bytes);
        b
    };
    let mut params = Vec::new();
    params.extend_from_slice(&(SPACING as f32).to_le_bytes());
    params.extend_from_slice(&(cell as f32).to_le_bytes());
    params.extend_from_slice(&rock.to_le_bytes());
    params.extend_from_slice(&(side as i32).to_le_bytes());
    params.extend_from_slice(&(fside as i32).to_le_bytes());
    params.extend_from_slice(&(HALO as i32).to_le_bytes());
    params.extend_from_slice(&(seed as u32).to_le_bytes());
    params.extend_from_slice(&((seed >> 32) as u32).to_le_bytes());
    params.extend_from_slice(&(origin[0] as i32).to_le_bytes());
    params.extend_from_slice(&(origin[1] as i32).to_le_bytes());
    // (A uniform struct rounds up to 16 bytes.)
    params.resize(params.len().next_multiple_of(16), 0);
    let storage = wgpu::BufferUsages::STORAGE;
    let p = buffer(&params, wgpu::BufferUsages::UNIFORM);
    let h = buffer(bytemuck::cast_slice(&heights), storage);
    let f = buffer(bytemuck::cast_slice(&fieldv), storage);
    let a = buffer(bytemuck::cast_slice(&places), storage);
    let size = (places.len() * 4) as u64;
    let out = device.create_buffer(&wgpu::BufferDescriptor { label: None, size, usage: storage | wgpu::BufferUsages::COPY_SRC, mapped_at_creation: false });
    let back = device.create_buffer(&wgpu::BufferDescriptor { label: None, size, usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST, mapped_at_creation: false });
    // (Laid out in full: a stand-in reading none of the tile would lose those bindings otherwise.)
    let entry = |binding: u32, ty: wgpu::BufferBindingType| wgpu::BindGroupLayoutEntry { binding, visibility: wgpu::ShaderStages::COMPUTE, ty: wgpu::BindingType::Buffer { ty, has_dynamic_offset: false, min_binding_size: None }, count: None };
    let read = wgpu::BufferBindingType::Storage { read_only: true };
    let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: None,
        entries: &[entry(0, wgpu::BufferBindingType::Uniform), entry(1, read), entry(2, read), entry(3, read), entry(4, wgpu::BufferBindingType::Storage { read_only: false })],
    });
    let pl = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor { label: None, bind_group_layouts: &[Some(&layout)], immediate_size: 0 });
    let pipe = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor { label: None, layout: Some(&pl), module: &shader, entry_point: Some("main"), compilation_options: Default::default(), cache: None });
    let bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: None,
        layout: &layout,
        entries: &[
            wgpu::BindGroupEntry { binding: 0, resource: p.as_entire_binding() },
            wgpu::BindGroupEntry { binding: 1, resource: h.as_entire_binding() },
            wgpu::BindGroupEntry { binding: 2, resource: f.as_entire_binding() },
            wgpu::BindGroupEntry { binding: 3, resource: a.as_entire_binding() },
            wgpu::BindGroupEntry { binding: 4, resource: out.as_entire_binding() },
        ],
    });
    let mut encoder = device.create_command_encoder(&Default::default());
    {
        let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor { label: None, timestamp_writes: None });
        pass.set_pipeline(&pipe);
        pass.set_bind_group(0, &bind, &[]);
        pass.dispatch_workgroups((places.len() as u32).div_ceil(64), 1, 1);
    }
    encoder.copy_buffer_to_buffer(&out, 0, &back, 0, size);
    queue.submit([encoder.finish()]);
    back.slice(..).map_async(wgpu::MapMode::Read, |_| {});
    device.poll(wgpu::PollType::wait_indefinitely()).expect("poll");
    let got: Vec<f32> = bytemuck::cast_slice(&back.slice(..).get_mapped_range().expect("mapped")).to_vec();
    let worst = want.iter().zip(&got).map(|(w, g)| (w - *g as f64).abs()).fold(0.0, f64::max);
    assert!(worst <= detail::AGREE_M, "the GPU's detail is {worst:.3} m off the truth (at most {} m)", detail::AGREE_M);
}
