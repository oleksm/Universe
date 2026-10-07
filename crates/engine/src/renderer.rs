use std::path::Path;

use glam::camera::rh::proj::directx::orthographic;
use glam::{UVec2, Vec3};

use std::collections::HashMap;

use crate::frame::{Frame, Instance, Vertex};
use crate::model::Mesh;
use crate::gpu::Gpu;

const COLOR_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;
/// The scene's light, before it's tone-mapped for the screen (HDR).
const SCENE_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;
/// Samples a pixel of the scene (antialiasing).
const SAMPLES: u32 = 4;
/// Screenshots' size: the same every run.
const SHOT: UVec2 = UVec2::new(1920, 1080);
const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Globals {
    view_proj: [[f32; 4]; 4],
    hud_proj: [[f32; 4]; 4],
    /// Camera-relative world → each shadow cascade (see `Shadows`).
    shadow_near: [[f32; 4]; 4],
    shadow_far: [[f32; 4]; 4],
    /// A texel of each cascade (metres); shadows on (1) or not.
    shadow: [f32; 4],
    /// `Graphics`, 1 on, 0 off: textures, normal maps, occlusion, emission;
    look: [f32; 4],
    /// specular, planet light, tone map; w: the bound world's globe layer + 1 (0: none).
    look2: [f32; 4],
    /// The tight cascade round what's looked at (see `Frame::shadow_focus`),
    /// and x: a texel of it (metres), y: in use (1) or not.
    shadow_tight: [[f32; 4]; 4],
    shadow2: [f32; 4],
    /// The environment (see `env.rs`): the sun's direction from the eye and
    /// its light here (w); the world nearest's centre from the eye and radius;
    /// its colour and albedo (w; 0: none); the mode
    /// (x: 0 space, 1 studio) and the stars' glow (y); the sky's colour in
    /// an atmosphere (as drawn behind everything).
    env_sun: [f32; 4],
    env_world: [f32; 4],
    env_world_color: [f32; 4],
    env_mode: [f32; 4],
    env_sky: [f32; 4],
    /// x: the angle a pixel spans (radians) at the screen's middle; y: how far the world's maps
    /// are faded in (0..1); z: 1 where its air's tables are bound.
    view: [f32; 4],
    /// Clip → camera-relative world (for a full-screen pass's view directions).
    inv_view_proj: [[f32; 4]; 4],
    /// The world whose maps are bound: its centre from the eye (m) and radius (m; 0: none).
    world_at: [f32; 4],
    /// The scene's frame to the bound world's own (its turn undone), for its clouds.
    world_to_body: [[f32; 4]; 4],
    /// The ground cascade (see `GROUND_HALF`): camera-relative world → it; its
    /// texel and in use are `shadow2`'s z and w.
    shadow_ground: [[f32; 4]; 4],
}

/// The shadow map's side (texels), each of its four cascades (see `Shadows`).
use crate::shaders::SHADOW_SIZE;
/// How far toward the light (and away) a shadow box reaches from the eye
/// (m): what casts from up to this far sunward of it.
const SHADOW_DEPTH: f64 = 8_000.0;
/// The ground cascade: a box this far each way across the light round the
/// eye (m), and `GROUND_DEPTH` along it, drawn from the ground's patches
/// only: a mountain's shadow tens of kilometres long, on the ground and on
/// what stands on it. (Its texel: 15 m.)
const GROUND_HALF: f64 = 30_000.0;
const GROUND_DEPTH: f64 = 80_000.0;

/// Globe maps' texels a face side, layers (worlds at once), mip levels.
use crate::shaders::GLOBE_SIZE;
const GLOBE_LAYERS: u32 = 16;
const GLOBE_MIPS: u32 = 10;

/// The globe maps on the GPU: which map holds each layer, and the frame it
/// was last drawn (the oldest gives way to a new one).
struct Globes {
    texture: wgpu::Texture,
    /// Their own colours (sRGB; alpha 0 where a world has none), layer for layer.
    colors: wgpu::Texture,
    layers: Vec<Option<(u64, u64)>>,
}

impl Globes {
    /// Whether `map` is in a layer already.
    fn resident(&self, map: &crate::model::GlobeMap) -> bool {
        self.layers.iter().any(|l| matches!(l, Some((id, _)) if *id == map.id()))
    }

    /// The layer holding `map` for frame `now`, uploading it if it's new
    /// into the oldest layer not drawn this frame. None: every layer is
    /// someone's this frame (a layer given out is never overwritten before
    /// the frame's drawn: that showed one world's ground as another's).
    fn layer(&mut self, gpu: &Gpu, map: &crate::model::GlobeMap, now: u64) -> Option<u32> {
        if let Some(k) = self.layers.iter().position(|l| matches!(l, Some((id, _)) if *id == map.id())) {
            self.layers[k] = Some((map.id(), now));
            return Some(k as u32);
        }
        let k = (0..self.layers.len()).filter(|&k| self.layers[k].is_none_or(|(_, used)| used != now)).min_by_key(|&k| self.layers[k].map_or(0, |(_, used)| used + 1))?;
        self.layers[k] = Some((map.id(), now));
        self.upload(gpu, map, k as u32);
        Some(k as u32)
    }

    fn upload(&self, gpu: &Gpu, map: &crate::model::GlobeMap, layer: u32) {
        let n = map.size as usize;
        assert_eq!(map.size, GLOBE_SIZE, "globe maps are {GLOBE_SIZE} a side");
        for face in 0..6 {
            let mut level: Vec<[f32; 2]> = map.texels[face * n * n..(face + 1) * n * n].to_vec();
            let mut side = n;
            for mip in 0..GLOBE_MIPS {
                let bytes: Vec<u16> = level.iter().flat_map(|t| [half(t[0]), half(t[1])]).collect();
                gpu.queue.write_texture(
                    wgpu::TexelCopyTextureInfo { texture: &self.texture, mip_level: mip, origin: wgpu::Origin3d { x: 0, y: 0, z: layer * 6 + face as u32 }, aspect: wgpu::TextureAspect::All },
                    bytemuck::cast_slice(&bytes),
                    wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(side as u32 * 4), rows_per_image: Some(side as u32) },
                    wgpu::Extent3d { width: side as u32, height: side as u32, depth_or_array_layers: 1 },
                );
                if side == 1 {
                    break;
                }
                // (The next level: each texel the mean of four.)
                let half_side = side / 2;
                level = (0..half_side * half_side)
                    .map(|i| {
                        let (x, y) = (i % half_side * 2, i / half_side * 2);
                        let at = |dx: usize, dy: usize| level[(y + dy) * side + x + dx];
                        let (a, b, c, d) = (at(0, 0), at(1, 0), at(0, 1), at(1, 1));
                        [(a[0] + b[0] + c[0] + d[0]) / 4.0, (a[1] + b[1] + c[1] + d[1]) / 4.0]
                    })
                    .collect();
                side = half_side;
            }
        }
        // Its own colours, or none (cleared: the layer may have held another world's).
        for face in 0..6 {
            let mut level: Vec<[u8; 4]> = match &map.colors {
                Some(c) => c[face * n * n..(face + 1) * n * n].to_vec(),
                None => vec![[0; 4]; n * n],
            };
            let mut side = n;
            for mip in 0..GLOBE_MIPS {
                gpu.queue.write_texture(
                    wgpu::TexelCopyTextureInfo { texture: &self.colors, mip_level: mip, origin: wgpu::Origin3d { x: 0, y: 0, z: layer * 6 + face as u32 }, aspect: wgpu::TextureAspect::All },
                    bytemuck::cast_slice(&level),
                    wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(side as u32 * 4), rows_per_image: Some(side as u32) },
                    wgpu::Extent3d { width: side as u32, height: side as u32, depth_or_array_layers: 1 },
                );
                if side == 1 {
                    break;
                }
                let half_side = side / 2;
                level = (0..half_side * half_side)
                    .map(|i| {
                        let (x, y) = (i % half_side * 2, i / half_side * 2);
                        let at = |dx: usize, dy: usize| level[(y + dy) * side + x + dx];
                        let (a, b, c, d) = (at(0, 0), at(1, 0), at(0, 1), at(1, 1));
                        std::array::from_fn(|k| ((a[k] as u32 + b[k] as u32 + c[k] as u32 + d[k] as u32 + 2) / 4) as u8)
                    })
                    .collect();
                side = half_side;
            }
        }
    }
}

/// A float as a half (IEEE binary16), for globe maps (small values; no
/// subnormals: those go to zero).
pub(crate) fn half(x: f32) -> u16 {
    let b = x.to_bits();
    let sign = ((b >> 16) & 0x8000) as u16;
    let e = ((b >> 23) & 0xff) as i32 - 127 + 15;
    let m = ((b & 0x7f_ffff) >> 13) as u16;
    if e <= 0 {
        sign
    } else if e >= 31 {
        sign | 0x7c00
    } else {
        sign | ((e as u16) << 10) | m
    }
}

/// The light's view of what's near the eye, for shadows: four cascades (near:
/// a twenty-fourth of far; far; tight: round what's looked at, centimetres a
/// texel; the ground's: see `GROUND_HALF`), each an orthographic box along
/// the light, depth only.
struct Shadows {
    /// Each cascade's layer of the map, to draw into.
    layers: [wgpu::TextureView; 4],
    /// Each cascade's matrix (uniform), for the pass drawing it.
    lights: [wgpu::Buffer; 4],
    light_binds: [wgpu::BindGroup; 4],
    /// The map and its comparing sampler, for the mesh shaders.
    bind: wgpu::BindGroup,
    pipe: wgpu::RenderPipeline,
    /// The ground cascade's (no depth bias).
    ground_pipe: wgpu::RenderPipeline,
    /// The casters this frame: (mesh, first instance, count).
    runs: Vec<(u64, u32, u32)>,
    /// The ground's patches, casting into the ground cascade (layer 3).
    ground_runs: Vec<(u64, u32, u32)>,
}

/// Camera-relative world → a shadow cascade's clip space: a box `half`
/// metres each way across the light and `depth` along it, round the
/// eye at `eye` (world), its centre held to whole texels (so edges don't
/// crawl as the eye moves); depth 0 at the sunward end.
fn shadow_matrix(eye: glam::DVec3, sun: glam::DVec3, half: f64, depth: f64) -> glam::Mat4 {
    use glam::{DVec3, DVec4};
    let w = sun;
    let u = w.cross(if w.y.abs() < 0.9 { DVec3::Y } else { DVec3::X }).normalize();
    let v = w.cross(u);
    let texel = 2.0 * half / SHADOW_SIZE as f64;
    let snap = |a: f64| (a / texel).round() * texel - a;
    let c = u * snap(eye.dot(u)) + v * snap(eye.dot(v));
    let d = depth;
    let rows = [
        DVec4::new(u.x / half, u.y / half, u.z / half, -c.dot(u) / half),
        DVec4::new(v.x / half, v.y / half, v.z / half, -c.dot(v) / half),
        DVec4::new(-w.x / (2.0 * d), -w.y / (2.0 * d), -w.z / (2.0 * d), 0.5 + c.dot(w) / (2.0 * d)),
        DVec4::W,
    ];
    glam::DMat4::from_cols(rows[0], rows[1], rows[2], rows[3]).transpose().as_mat4()
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
        self.upload_bytes(gpu, bytemuck::cast_slice(data), data.len() as u32);
    }

    fn upload_bytes(&mut self, gpu: &Gpu, bytes: &[u8], count: u32) {
        if bytes.len() as u64 > self.capacity {
            self.capacity = (bytes.len() as u64).next_power_of_two();
            self.buffer = Self::alloc(&gpu.device, self.label, self.capacity);
        }
        if !bytes.is_empty() {
            gpu.queue.write_buffer(&self.buffer, 0, bytes);
        }
        self.count = count;
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

/// A mesh vertex: where it is in the model, the normal it's lit by (a
/// face's own for faces; the direction from the center for edges), its colour.
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct MeshVertex {
    pos: [f32; 3],
    normal: [f32; 3],
    color: [f32; 4],
    /// The model's per-vertex data (`WireModel::data`; zero where it has none).
    data: [f32; 4],
}

/// A mesh on the GPU: its faces (three vertices each) and edges (two each),
/// and the last frame it was drawn (unused ones are dropped).
struct GpuMesh {
    faces: wgpu::Buffer,
    face_vertices: u32,
    edges: wgpu::Buffer,
    edge_vertices: u32,
    used: u64,
}

impl GpuMesh {
    fn new(device: &wgpu::Device, mesh: &Mesh) -> Self {
        use wgpu::util::DeviceExt;
        let color = |i: u32| mesh.colors.get(i as usize).copied().unwrap_or([1.0; 4]);
        let data = |i: u32| mesh.data.get(i as usize).copied().unwrap_or([0.0; 4]);
        let mut faces = Vec::with_capacity(mesh.faces.len() * 3);
        // (Smooth: each corner's normal the faces' meeting there, weighted by their size.)
        let corner_normals: Vec<Vec3> = if mesh.smooth {
            let mut sum = vec![Vec3::ZERO; mesh.positions.len()];
            for f in &mesh.faces {
                let [a, b, c] = f.map(|i| mesh.positions[i as usize]);
                let n = (b - a).cross(c - a);
                for &i in f {
                    sum[i as usize] += n;
                }
            }
            sum.into_iter().map(|n| n.normalize_or_zero()).collect()
        } else {
            Vec::new()
        };
        for f in &mesh.faces {
            let [a, b, c] = f.map(|i| mesh.positions[i as usize]);
            // Faces are wound counter-clockwise seen from outside: that's their normal.
            // (Not "away from the centre": a station or a winged hull isn't convex about it.)
            let n = (b - a).cross(c - a).normalize_or_zero();
            for &i in f {
                let n = corner_normals.get(i as usize).copied().unwrap_or(n);
                faces.push(MeshVertex { pos: mesh.positions[i as usize].to_array(), normal: n.to_array(), color: color(i), data: data(i) });
            }
        }
        let mut edges = Vec::with_capacity(mesh.edges.len() * 2);
        let normals = edge_normals(mesh);
        for (e, n) in mesh.edges.iter().zip(normals) {
            for &i in e {
                let p = mesh.positions[i as usize];
                edges.push(MeshVertex { pos: p.to_array(), normal: n.to_array(), color: color(i), data: data(i) });
            }
        }
        let buffer = |label, data: &[MeshVertex]| {
            device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some(label),
                // (wgpu wants a non-empty buffer.)
                contents: if data.is_empty() { &[0u8; 40] } else { bytemuck::cast_slice(data) },
                usage: wgpu::BufferUsages::VERTEX,
            })
        };
        GpuMesh { faces: buffer("mesh faces", &faces), face_vertices: faces.len() as u32, edges: buffer("mesh edges", &edges), edge_vertices: edges.len() as u32, used: 0 }
    }
}

/// Each edge's normal, to light it by: the faces it bounds, averaged (a
/// crease lit as either side is); a detail line (on a face, not bounding
/// one), the face nearest its middle; a bare line, the way out from the centre.
fn edge_normals(mesh: &Mesh) -> Vec<Vec3> {
    let face_n: Vec<Vec3> = mesh.faces.iter().map(|f| {
        let [a, b, c] = f.map(|i| mesh.positions[i as usize]);
        (b - a).cross(c - a).normalize_or_zero()
    }).collect();
    let mut by_edge: HashMap<(u32, u32), Vec3> = HashMap::new();
    for (f, &n) in mesh.faces.iter().zip(&face_n) {
        for (a, b) in [(f[0], f[1]), (f[1], f[2]), (f[2], f[0])] {
            *by_edge.entry((a.min(b), a.max(b))).or_default() += n;
        }
    }
    mesh.edges.iter().map(|e| {
        let (a, b) = (e[0].min(e[1]), e[0].max(e[1]));
        if let Some(n) = by_edge.get(&(a, b)).and_then(|n| n.try_normalize()) {
            return n;
        }
        let mid = (mesh.positions[a as usize] + mesh.positions[b as usize]) / 2.0;
        let nearest = mesh.faces.iter().zip(&face_n).filter(|(_, n)| **n != Vec3::ZERO).map(|(f, n)| {
            let [p, q, r] = f.map(|i| mesh.positions[i as usize]);
            (distance_to_triangle(mid, p, q, r), *n)
        }).min_by(|x, y| x.0.total_cmp(&y.0));
        nearest.map_or(mid.normalize_or_zero(), |(_, n)| n)
    }).collect()
}

/// How far `p` is from the triangle `a b c`.
fn distance_to_triangle(p: Vec3, a: Vec3, b: Vec3, c: Vec3) -> f32 {
    let n = (b - a).cross(c - a).normalize_or_zero();
    let q = p - n * n.dot(p - a);
    let inside = [(a, b), (b, c), (c, a)].iter().all(|&(u, v)| (v - u).cross(q - u).dot(n) >= 0.0);
    if inside {
        return (p - q).length();
    }
    let seg = |u: Vec3, v: Vec3| {
        let t = ((p - u).dot(v - u) / (v - u).length_squared().max(1e-12)).clamp(0.0, 1.0);
        (p - (u + (v - u) * t)).length()
    };
    seg(a, b).min(seg(b, c)).min(seg(c, a))
}

/// Frames a mesh may go undrawn before it's dropped from the GPU.
const MESH_KEEP: u64 = 600;

/// Offscreen targets: the low-res scene, the HUD layer at `hud_scale` times that
/// resolution, and a composite of both (used for screenshots).
struct Target {
    /// The scene's pixels (the screen's times the render scale: see `render_scale`), and the
    /// screen's (the HUD's layer and the composite are drawn at it, the scene upscaled to it).
    size: UVec2,
    out: UVec2,
    hud_size: UVec2,
    /// The scene drawn (antialiased: `SAMPLES` a pixel), and resolved.
    color_msaa: wgpu::TextureView,
    color: wgpu::TextureView,
    depth: wgpu::TextureView,
    hud: wgpu::TextureView,
    /// The front layer (see `Frame::in_front`): drawn, and resolved.
    front_msaa: wgpu::TextureView,
    front: wgpu::TextureView,
    composite: wgpu::TextureView,
    blit: wgpu::BindGroup,
}

pub(crate) struct Renderer {
    /// The environment as light, drawn each frame (see `env.rs`).
    env: crate::env::Env,
    sunprobe: crate::sunprobe::SunProbe,
    /// Each pass's GPU time, where the GPU can stamp it.
    gputime: Option<crate::gputime::GpuTime>,
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
    linear: wgpu::Sampler,
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
    mesh_pipe: wgpu::RenderPipeline,
    /// The air's light behind everything (a full-screen pass: `air_sky`).
    air_sky_pipe: wgpu::RenderPipeline,
    mesh_line_pipe: wgpu::RenderPipeline,
    shadows: Shadows,
    globes: Globes,
    /// The full-resolution maps of the world near the eye (group 2).
    world: crate::worldmaps::WorldBind,
    meshes: HashMap<u64, GpuMesh>,
    /// This frame's mesh instances: faces, then edges; and the runs to draw
    /// (mesh, first instance, count) for each.
    instances: DynBuffer,
    face_runs: Vec<(u64, u32, u32)>,
    edge_runs: Vec<(u64, u32, u32)>,
    /// The same for the front layer's meshes.
    front_face_runs: Vec<(u64, u32, u32)>,
    front_edge_runs: Vec<(u64, u32, u32)>,
    frames: u64,
    solids: DynBuffer,
    lines: DynBuffer,
    front_lines: DynBuffer,
    points: DynBuffer,
    glows: DynBuffer,
    front_glows: DynBuffer,
    glow_pipe: wgpu::RenderPipeline,
    hud_tris: DynBuffer,
    atlas_bind: wgpu::BindGroup,
    hud: DynBuffer,
    /// Textured, physically based models (glTF).
    pbr: crate::pbr::PbrRenderer,
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
                visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
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

        // (The ground's material, the lab's, beside the scene shader that calls it.)
        let scene = crate::shaders::make(device, "scene", crate::shaders::scene());
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
                              blend: wgpu::BlendState,
                              (format, samples): (wgpu::TextureFormat, u32)| {
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
                multisample: wgpu::MultisampleState { count: samples, ..Default::default() },
                fragment: Some(wgpu::FragmentState {
                    module: &scene,
                    entry_point: Some("fs_color"),
                    compilation_options: Default::default(),
                    targets: &[Some(wgpu::ColorTargetState {
                        format,
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
        // Meshes: their vertices, and one instance record per draw.
        let mesh_layouts = [
            Some(wgpu::VertexBufferLayout {
                array_stride: size_of::<MeshVertex>() as u64,
                step_mode: wgpu::VertexStepMode::Vertex,
                attributes: &wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x3, 2 => Float32x4, 16 => Float32x4],
            }),
            Some(wgpu::VertexBufferLayout {
                array_stride: size_of::<Instance>() as u64,
                step_mode: wgpu::VertexStepMode::Instance,
                attributes: &wgpu::vertex_attr_array![
                    3 => Float32x4, 4 => Float32x4, 5 => Float32x4, 6 => Float32x4, 7 => Float32x4,
                    8 => Float32x4, 9 => Float32x4, 10 => Float32x4, 11 => Float32x4, 12 => Float32x4, 13 => Float32x4,
                    14 => Float32x4, 15 => Float32x4
                ],
            }),
        ];
        // The shadow map: its texture (a layer each cascade), the mesh
        // shaders' view of it, and the pass that draws it.
        let shadow_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("shadow map"),
            size: wgpu::Extent3d { width: SHADOW_SIZE, height: SHADOW_SIZE, depth_or_array_layers: 4 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: DEPTH_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let shadow_sample = shadow_texture.create_view(&wgpu::TextureViewDescriptor { dimension: Some(wgpu::TextureViewDimension::D2Array), ..Default::default() });
        let shadow_layer = |k: u32| {
            shadow_texture.create_view(&wgpu::TextureViewDescriptor { dimension: Some(wgpu::TextureViewDimension::D2), base_array_layer: k, array_layer_count: Some(1), ..Default::default() })
        };
        let shadow_cmp = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("shadow compare"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            compare: Some(wgpu::CompareFunction::LessEqual),
            ..Default::default()
        });
        let shadow_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("shadow map"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture { sample_type: wgpu::TextureSampleType::Depth, view_dimension: wgpu::TextureViewDimension::D2Array, multisampled: false },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry { binding: 1, visibility: wgpu::ShaderStages::FRAGMENT, ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Comparison), count: None },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture { sample_type: wgpu::TextureSampleType::Float { filterable: true }, view_dimension: wgpu::TextureViewDimension::CubeArray, multisampled: false },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry { binding: 3, visibility: wgpu::ShaderStages::FRAGMENT, ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering), count: None },
                // The environment's cube (see `env.rs`).
                wgpu::BindGroupLayoutEntry {
                    binding: 4,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture { sample_type: wgpu::TextureSampleType::Float { filterable: true }, view_dimension: wgpu::TextureViewDimension::Cube, multisampled: false },
                    count: None,
                },
                // Worlds' own colours, beside their maps (see `GlobeMap::colors`).
                wgpu::BindGroupLayoutEntry {
                    binding: 6,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture { sample_type: wgpu::TextureSampleType::Float { filterable: true }, view_dimension: wgpu::TextureViewDimension::CubeArray, multisampled: false },
                    count: None,
                },
            ],
        });
        let env = crate::env::Env::new(device, &globals_layout);
        let sunprobe = crate::sunprobe::SunProbe::new(device);
        let gputime = crate::gputime::GpuTime::new(device, &gpu.queue);
        // Globe maps: a cube array, a layer per world in view (see `GlobeMap`).
        let globe_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("globe maps"),
            size: wgpu::Extent3d { width: GLOBE_SIZE, height: GLOBE_SIZE, depth_or_array_layers: 6 * GLOBE_LAYERS },
            mip_level_count: GLOBE_MIPS,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rg16Float,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let globe_view = globe_texture.create_view(&wgpu::TextureViewDescriptor { dimension: Some(wgpu::TextureViewDimension::CubeArray), ..Default::default() });
        let globe_colors = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("globe colours"),
            size: wgpu::Extent3d { width: GLOBE_SIZE, height: GLOBE_SIZE, depth_or_array_layers: 6 * GLOBE_LAYERS },
            mip_level_count: GLOBE_MIPS,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let globe_colors_view = globe_colors.create_view(&wgpu::TextureViewDescriptor { dimension: Some(wgpu::TextureViewDimension::CubeArray), ..Default::default() });
        let globe_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("globe maps"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Linear,
            ..Default::default()
        });
        let shadow_bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("shadow map"),
            layout: &shadow_layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(&shadow_sample) },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::Sampler(&shadow_cmp) },
                wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::TextureView(&globe_view) },
                wgpu::BindGroupEntry { binding: 3, resource: wgpu::BindingResource::Sampler(&globe_sampler) },
                wgpu::BindGroupEntry { binding: 4, resource: wgpu::BindingResource::TextureView(&env.spec) },
                wgpu::BindGroupEntry { binding: 6, resource: wgpu::BindingResource::TextureView(&globe_colors_view) },
            ],
        });
        let globes = Globes { texture: globe_texture, colors: globe_colors, layers: vec![None; GLOBE_LAYERS as usize] };
        let light_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("shadow light"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX,
                ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Uniform, has_dynamic_offset: false, min_binding_size: None },
                count: None,
            }],
        });
        let light_buffer = || device.create_buffer(&wgpu::BufferDescriptor { label: Some("shadow light"), size: 64, usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST, mapped_at_creation: false });
        let lights = [light_buffer(), light_buffer(), light_buffer(), light_buffer()];
        let light_bind = |b: &wgpu::Buffer| device.create_bind_group(&wgpu::BindGroupDescriptor { label: Some("shadow light"), layout: &light_layout, entries: &[wgpu::BindGroupEntry { binding: 0, resource: b.as_entire_binding() }] });
        let light_binds = [light_bind(&lights[0]), light_bind(&lights[1]), light_bind(&lights[2]), light_bind(&lights[3])];
        let shadow_shader = crate::shaders::single(device, "shadow");
        let shadow_pipe_biased = |label: &str, bias: wgpu::DepthBiasState| device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some(label),
            layout: Some(&device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor { label: Some("shadow"), bind_group_layouts: &[Some(&light_layout)], immediate_size: 0 })),
            vertex: wgpu::VertexState { module: &shadow_shader, entry_point: Some("vs_shadow"), compilation_options: Default::default(), buffers: &mesh_layouts },
            primitive: wgpu::PrimitiveState { topology: wgpu::PrimitiveTopology::TriangleList, ..Default::default() },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: DEPTH_FORMAT,
                depth_write_enabled: Some(true),
                depth_compare: Some(wgpu::CompareFunction::LessEqual),
                stencil: Default::default(),
                bias,
            }),
            multisample: Default::default(),
            fragment: None,
            multiview_mask: None,
            cache: None,
        });
        // (Pushed back a little, more on slopes: no speckle on lit faces.)
        let shadow_pipe = shadow_pipe_biased("shadow casters", wgpu::DepthBiasState { constant: 2, slope_scale: 2.0, clamp: 0.0 });
        // (The ground's: none. Its texels are 15 m and the sun may graze it, where a slope's bias
        // would be hundreds of metres, more than its hills; its receivers step off along their
        // normals instead.)
        let ground_pipe = shadow_pipe_biased("ground shadow casters", wgpu::DepthBiasState::default());
        let shadows = Shadows { layers: [shadow_layer(0), shadow_layer(1), shadow_layer(2), shadow_layer(3)], lights, light_binds, bind: shadow_bind, pipe: shadow_pipe, ground_pipe, runs: Vec::new(), ground_runs: Vec::new() };
        let world_bind = crate::worldmaps::WorldBind::new(device, &gpu.queue);
        let mesh_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("meshes"),
            bind_group_layouts: &[Some(&globals_layout), Some(&shadow_layout), Some(&world_bind.layout)],
            immediate_size: 0,
        });
        let mesh_pipeline = |label: &str, vs: &str, topology: wgpu::PrimitiveTopology, write: bool, compare: wgpu::CompareFunction| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(label),
                layout: Some(&mesh_layout),
                vertex: wgpu::VertexState { module: &scene, entry_point: Some(vs), compilation_options: Default::default(), buffers: &mesh_layouts },
                primitive: wgpu::PrimitiveState { topology, ..Default::default() },
                depth_stencil: Some(wgpu::DepthStencilState {
                    format: DEPTH_FORMAT,
                    depth_write_enabled: Some(write),
                    depth_compare: Some(compare),
                    stencil: Default::default(),
                    bias: Default::default(),
                }),
                multisample: wgpu::MultisampleState { count: SAMPLES, ..Default::default() },
                fragment: Some(wgpu::FragmentState {
                    module: &scene,
                    entry_point: Some("fs_mesh"),
                    compilation_options: Default::default(),
                    targets: &[Some(wgpu::ColorTargetState { format: SCENE_FORMAT, blend: Some(wgpu::BlendState::ALPHA_BLENDING), write_mask: wgpu::ColorWrites::ALL })],
                }),
                multiview_mask: None,
                cache: None,
            })
        };
        let mesh_pipe = mesh_pipeline("mesh faces", "vs_mesh", wgpu::PrimitiveTopology::TriangleList, true, wgpu::CompareFunction::Greater);
        let air_sky_pipe = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("air sky"),
            layout: Some(&mesh_layout),
            vertex: wgpu::VertexState { module: &scene, entry_point: Some("vs_air_sky"), compilation_options: Default::default(), buffers: &[] },
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: Some(wgpu::DepthStencilState { format: DEPTH_FORMAT, depth_write_enabled: Some(false), depth_compare: Some(wgpu::CompareFunction::Always), stencil: Default::default(), bias: Default::default() }),
            multisample: wgpu::MultisampleState { count: SAMPLES, ..Default::default() },
            fragment: Some(wgpu::FragmentState {
                module: &scene,
                entry_point: Some("fs_air_sky"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState { format: SCENE_FORMAT, blend: Some(additive), write_mask: wgpu::ColorWrites::ALL })],
            }),
            multiview_mask: None,
            cache: None,
        });
        let mesh_line_pipe = mesh_pipeline("mesh edges", "vs_mesh_line", wgpu::PrimitiveTopology::LineList, false, wgpu::CompareFunction::GreaterEqual);
        let world = (SCENE_FORMAT, SAMPLES);
        let sky_pipe = scene_pipeline("sky", "vs_sky", Topo::PointList, Some((false, Cmp::Always)), additive, world);
        let solid_pipe = scene_pipeline("solids", "vs_world", Topo::TriangleList, Some((true, Cmp::Greater)), alpha, world);
        let line_pipe = scene_pipeline("lines", "vs_line", Topo::LineList, Some((false, Cmp::GreaterEqual)), alpha, world);
        let point_pipe = scene_pipeline("points", "vs_line", Topo::PointList, Some((false, Cmp::GreaterEqual)), alpha, world);
        // Lights' glows add up, behind what's solid, hiding nothing.
        let glow_pipe = scene_pipeline("glows", "vs_line", Topo::TriangleList, Some((false, Cmp::GreaterEqual)), additive, world);
        // The HUD has its own layer without depth (or antialiasing, or HDR).
        // HUD triangles (panels and text): their own shader, the font's atlas.
        let atlas_data = crate::font::atlas();
        let atlas_texture = {
            use wgpu::util::DeviceExt;
            gpu.device.create_texture_with_data(
                &gpu.queue,
                &wgpu::TextureDescriptor {
                    label: Some("font atlas"),
                    size: wgpu::Extent3d { width: atlas_data.width, height: atlas_data.height, depth_or_array_layers: 1 },
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format: wgpu::TextureFormat::R8Unorm,
                    usage: wgpu::TextureUsages::TEXTURE_BINDING,
                    view_formats: &[],
                },
                wgpu::util::TextureDataOrder::LayerMajor,
                &atlas_data.pixels,
            )
        };
        let atlas_view = atlas_texture.create_view(&Default::default());
        let atlas_sampler = device.create_sampler(&wgpu::SamplerDescriptor { label: Some("font atlas"), mag_filter: wgpu::FilterMode::Linear, min_filter: wgpu::FilterMode::Linear, ..Default::default() });
        let atlas_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("font atlas"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture { sample_type: wgpu::TextureSampleType::Float { filterable: true }, view_dimension: wgpu::TextureViewDimension::D2, multisampled: false },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry { binding: 1, visibility: wgpu::ShaderStages::FRAGMENT, ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering), count: None },
            ],
        });
        let atlas_bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("font atlas"),
            layout: &atlas_layout,
            entries: &[wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(&atlas_view) }, wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::Sampler(&atlas_sampler) }],
        });
        let hud_shader = crate::shaders::single(device, "hud");
        let hud_tri_pipe = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("hud tris"),
            layout: Some(&device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor { label: Some("hud"), bind_group_layouts: &[Some(&globals_layout), Some(&atlas_layout)], immediate_size: 0 })),
            vertex: wgpu::VertexState {
                module: &hud_shader,
                entry_point: Some("vs_hud"),
                compilation_options: Default::default(),
                buffers: &[Some(wgpu::VertexBufferLayout {
                    array_stride: size_of::<crate::frame::HudVertex>() as u64,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &wgpu::vertex_attr_array![0 => Float32x2, 1 => Float32x2, 2 => Float32x4],
                })],
            },
            primitive: wgpu::PrimitiveState { topology: Topo::TriangleList, ..Default::default() },
            depth_stencil: None,
            multisample: Default::default(),
            fragment: Some(wgpu::FragmentState {
                module: &hud_shader,
                entry_point: Some("fs_hud"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState { format: COLOR_FORMAT, blend: Some(alpha), write_mask: wgpu::ColorWrites::ALL })],
            }),
            multiview_mask: None,
            cache: None,
        });
        let hud_pipe = scene_pipeline("hud", "vs_hud", Topo::LineList, None, alpha, (COLOR_FORMAT, 1));

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
                texture_entry(3),
                wgpu::BindGroupLayoutEntry {
                    binding: 4,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                // (The globals: the graphics toggles, the tone map's.)
                wgpu::BindGroupLayoutEntry {
                    binding: 5,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Uniform, has_dynamic_offset: false, min_binding_size: None },
                    count: None,
                },
            ],
        });
        let blit = crate::shaders::single(device, "blit");
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
        let linear = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("linear"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        let target = Self::create_target(gpu, low_height, hud_scale, forced_aspect, &blit_layout, &sampler, &linear, &globals);
        let pbr = crate::pbr::PbrRenderer::new(device, &globals_layout, &shadow_layout, &light_layout, SCENE_FORMAT, DEPTH_FORMAT, SAMPLES);
        Self {
            env,
            sunprobe,
            gputime,
            pbr,
            wait: std::time::Duration::ZERO,
            low_height,
            hud_scale,
            forced_aspect,
            target,
            globals,
            globals_bind,
            blit_layout,
            sampler,
            linear,
            sky_pipe,
            solid_pipe,
            line_pipe,
            point_pipe,
            hud_tri_pipe,
            hud_pipe,
            blit_pipe,
            capture_pipe,
            sky: DynBuffer::new(device, "sky"),
            mesh_pipe,
            air_sky_pipe,
            mesh_line_pipe,
            meshes: HashMap::new(),
            instances: DynBuffer::new(device, "mesh instances"),
            face_runs: Vec::new(),
            edge_runs: Vec::new(),
            front_face_runs: Vec::new(),
            front_edge_runs: Vec::new(),
            shadows,
            globes,
            world: world_bind,
            frames: 0,
            solids: DynBuffer::new(device, "solids"),
            lines: DynBuffer::new(device, "lines"),
            front_lines: DynBuffer::new(device, "front lines"),
            points: DynBuffer::new(device, "points"),
            glows: DynBuffer::new(device, "glows"),
            front_glows: DynBuffer::new(device, "front glows"),
            glow_pipe,
            hud_tris: DynBuffer::new(device, "hud tris"),
            atlas_bind,
            hud: DynBuffer::new(device, "hud"),
        }
    }

    /// Low-res size: fixed height, width follows the window aspect ratio.
    fn low_res_size(gpu: &Gpu, low_height: u32, forced_aspect: Option<f32>) -> UVec2 {
        let aspect = forced_aspect.unwrap_or(gpu.config.width as f32 / gpu.config.height as f32);
        UVec2::new(((low_height as f32 * aspect).round() as u32).max(1), low_height)
    }

    /// The scene's size: the window's, full resolution (a screenshot run: `SHOT`).
    /// How much of the screen's resolution the scene is drawn at (the HUD always at the full):
    /// `UNIVERSE_RENDER_SCALE` (0.25–1), else as much as keeps it to 1440 lines. (A 4K screen
    /// at full resolution shades 8.3 million pixels of air, clouds and sea a frame.)
    fn render_scale(out: UVec2) -> f32 {
        match std::env::var("UNIVERSE_RENDER_SCALE").ok().and_then(|v| v.parse::<f32>().ok()) {
            Some(k) => k.clamp(0.25, 1.0),
            None => (1440.0 / out.y.max(1) as f32).min(1.0),
        }
    }

    fn scene_size(gpu: &Gpu, forced_aspect: Option<f32>) -> UVec2 {
        // (A screenshot run's size: 1920×1080, or `UNIVERSE_SHOT_SIZE` (e.g. 3840x2160).)
        let shot = || std::env::var("UNIVERSE_SHOT_SIZE").ok().and_then(|v| v.split_once('x').and_then(|(w, h)| Some(UVec2::new(w.parse().ok()?, h.parse().ok()?)))).unwrap_or(SHOT);
        if forced_aspect.is_some() { shot() } else { UVec2::new(gpu.config.width.max(1), gpu.config.height.max(1)) }
    }

    #[allow(clippy::too_many_arguments)]
    fn create_target(
        gpu: &Gpu,
        low_height: u32,
        hud_scale: u32,
        forced_aspect: Option<f32>,
        layout: &wgpu::BindGroupLayout,
        sampler: &wgpu::Sampler,
        linear: &wgpu::Sampler,
        globals: &wgpu::Buffer,
    ) -> Target {
        let out = Self::scene_size(gpu, forced_aspect);
        let k = Self::render_scale(out);
        let size = (out.as_vec2() * k).round().as_uvec2().max(UVec2::ONE);
        let hud_size = Self::low_res_size(gpu, low_height, forced_aspect) * hud_scale;
        let texture_n = |label, size: UVec2, format, usage, samples: u32| {
            gpu.device
                .create_texture(&wgpu::TextureDescriptor {
                    label: Some(label),
                    size: wgpu::Extent3d { width: size.x, height: size.y, depth_or_array_layers: 1 },
                    mip_level_count: 1,
                    sample_count: samples,
                    dimension: wgpu::TextureDimension::D2,
                    format,
                    usage,
                    view_formats: &[],
                })
                .create_view(&Default::default())
        };
        let texture = |label, size: UVec2, format, usage| texture_n(label, size, format, usage, 1);
        use wgpu::TextureUsages as U;
        let color_msaa = texture_n("scene (antialiased)", size, SCENE_FORMAT, U::RENDER_ATTACHMENT, SAMPLES);
        let color = texture("scene", size, SCENE_FORMAT, U::RENDER_ATTACHMENT | U::TEXTURE_BINDING);
        // (Read after the scene, for the sun probe.)
        let depth = texture_n("scene depth", size, DEPTH_FORMAT, U::RENDER_ATTACHMENT | U::TEXTURE_BINDING, SAMPLES);
        // (The HUD's layout is `hud_size` pixels; drawn at the screen's full resolution.)
        let hud = texture("hud", out, COLOR_FORMAT, U::RENDER_ATTACHMENT | U::TEXTURE_BINDING);
        let front_msaa = texture_n("front (antialiased)", size, SCENE_FORMAT, U::RENDER_ATTACHMENT, SAMPLES);
        let front = texture("front", size, SCENE_FORMAT, U::RENDER_ATTACHMENT | U::TEXTURE_BINDING);
        let composite = texture("composite", out, COLOR_FORMAT, U::RENDER_ATTACHMENT | U::COPY_SRC);
        let blit = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("blit"),
            layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(&color) },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(&hud) },
                wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::Sampler(sampler) },
                wgpu::BindGroupEntry { binding: 3, resource: wgpu::BindingResource::TextureView(&front) },
                wgpu::BindGroupEntry { binding: 4, resource: wgpu::BindingResource::Sampler(linear) },
                wgpu::BindGroupEntry { binding: 5, resource: globals.as_entire_binding() },
            ],
        });
        Target { size, out, hud_size, color_msaa, color, depth, hud, front_msaa, front, composite, blit }
    }

    pub fn resize(&mut self, gpu: &Gpu) {
        if Self::scene_size(gpu, self.forced_aspect) != self.target.out {
            self.target =
                Self::create_target(gpu, self.low_height, self.hud_scale, self.forced_aspect, &self.blit_layout, &self.sampler, &self.linear, &self.globals);
        }
    }

    pub fn low_res(&self) -> UVec2 {
        self.target.size
    }

    pub fn hud_size(&self) -> UVec2 {
        self.target.hud_size
    }

    /// What it holds on the GPU now.
    pub fn resources(&self) -> crate::app::Resources {
        crate::app::Resources {
            meshes: self.meshes.len(),
            instances: self.instances.count as usize,
            models: self.pbr.model_count(),
            globe_layers: self.globes.layers.iter().filter(|l| l.is_some()).count(),
            globe_capacity: self.globes.layers.len(),
        }
    }

    /// Render `frame`; if `capture` is set, also save the composited image (at HUD resolution) as PNG.
    pub fn render(&mut self, gpu: &mut Gpu, frame: &Frame, capture: Option<&Path>) {
        let size = self.target.size.as_vec2();
        let hud = self.target.hud_size.as_vec2();
        // The sun probe's answers since (see `sunprobe`), and where it looks this frame.
        let _ = gpu.device.poll(wgpu::PollType::Poll);
        self.sunprobe.collect();
        if let Some(t) = self.gputime.as_mut() {
            t.collect();
        }
        let probe = frame.sun_probe.and_then(|(at, radius)| {
            let clip = frame.camera.view_proj(size.x / size.y) * (at - frame.camera.position).as_vec3().extend(1.0);
            (clip.w > 0.0).then(|| {
                let ndc = clip.truncate() / clip.w;
                // (Twice as near as the sun's own core: anything standing in front of it.)
                ([(ndc.x + 1.0) * 0.5 * size.x, (1.0 - ndc.y) * 0.5 * size.y], radius, ndc.z * 2.0)
            })
        });
        if let Some((centre, radius, threshold)) = probe {
            self.sunprobe.aim(&gpu.queue, centre, radius, threshold, size.to_array());
        }
        // The shadow cascades: along the light from the eye, if there's a
        // light and shadows are wanted.
        let sun = frame.light.filter(|_| frame.shadow_reach > 0.0 && frame.graphics.shadows).and_then(|l| (l.position - frame.camera.position).try_normalize());
        // (The near cascade tight round the eye: a ship close by gets a few cm a texel.)
        let (near, far) = (frame.shadow_reach / 24.0, frame.shadow_reach);
        let cascade = |half: f64| sun.map_or(glam::Mat4::IDENTITY, |s| shadow_matrix(frame.camera.position, s, half, SHADOW_DEPTH));
        let ground_on = sun.is_some() && frame.meshes.iter().any(|d| d.ground);
        let shadow_ground = sun.filter(|_| ground_on).map_or(glam::Mat4::IDENTITY, |s| shadow_matrix(frame.camera.position, s, GROUND_HALF, GROUND_DEPTH));
        let (shadow_near, shadow_far) = (cascade(near), cascade(far));
        // (Round what's looked at, if the game says how far off it is: a ship a few
        // metres from the eye gets millimetres a texel, not the near cascade's centimetres.)
        let tight = (frame.shadow_focus > 0.0).then(|| (frame.shadow_focus * 1.2).clamp(6.0, near));
        let shadow_tight = tight.map_or(glam::Mat4::IDENTITY, cascade);
        let texel = |half: f64| (2.0 * half / SHADOW_SIZE as f64) as f32;
        let (gr, on) = (frame.graphics, |b: bool| if b { 1.0f32 } else { 0.0 });
        // A world's full-resolution maps, bound; the globe layer they're for (+ 1; 0: none
        // this frame: its globe map isn't up yet).
        let world_at = frame.world_maps.as_ref().map_or([0.0; 4], |(_, _, c, r)| {
            let d = (*c - frame.camera.position).as_vec3();
            [d.x, d.y, d.z, *r as f32]
        });
        let (world_layer, world_fade, world_luts) = match &frame.world_maps {
            Some((maps, globe, _, _)) => {
                self.world.bind(&gpu.device, &gpu.queue, maps);
                // (`on`: their format's version, for the lab's shader to read their maps by.)
                let clouds = if gr.clouds && maps.has_clouds() && frame.world_clouds[3] > 0.0 { [frame.world_clouds[0], frame.world_clouds[1], frame.world_clouds[2], maps.clouds_format as f32] } else { [0.0; 4] };
                gpu.queue.write_buffer(&self.world.clouds, 0, bytemuck::cast_slice(&clouds));
                (self.globes.layers.iter().position(|l| matches!(l, Some((id, _)) if id == globe)).map_or(0.0, |k| k as f32 + 1.0), self.world.fade(), if maps.has_air_luts() { 1.0 } else { 0.0 })
            }
            None => (0.0, 0.0, 0.0),
        };
        let globals = Globals {
            view_proj: frame.camera.view_proj(size.x / size.y).to_cols_array_2d(),
            hud_proj: orthographic(0.0, hud.x, hud.y, 0.0, -1.0, 1.0).to_cols_array_2d(),
            shadow_near: shadow_near.to_cols_array_2d(),
            shadow_far: shadow_far.to_cols_array_2d(),
            shadow_tight: shadow_tight.to_cols_array_2d(),
            shadow2: [tight.map_or(0.0, texel), if tight.is_some() && sun.is_some() { 1.0 } else { 0.0 }, texel(GROUND_HALF), if ground_on { 1.0 } else { 0.0 }],
            shadow_ground: shadow_ground.to_cols_array_2d(),
            // (w: UNIVERSE_SHADOW_DEBUG tints what's in shadow red, to check them.)
            shadow: [texel(near), texel(far), if sun.is_some() { 1.0 } else { 0.0 }, if crate::devflags::get().shadow_debug { 1.0 } else { 0.0 }],
            look: [on(gr.textures), on(gr.normal_maps), on(gr.occlusion), on(gr.emission)],
            look2: [on(gr.specular), on(gr.planet_light), on(gr.tone_map), world_layer],
            env_sun: frame.light.map_or([0.0; 4], |l| {
                let d = (l.position - frame.camera.position).normalize_or_zero().as_vec3();
                [d.x, d.y, d.z, l.intensity_at(frame.camera.position) * frame.sun_visible(frame.camera.position) as f32]
            }),
            env_world: frame.reflector.map_or([0.0; 4], |w| {
                let c = (w.center - frame.camera.position).as_vec3();
                [c.x, c.y, c.z, w.radius as f32]
            }),
            env_world_color: frame.reflector.filter(|_| gr.planet_light).map_or([0.0; 4], |w| [w.color[0], w.color[1], w.color[2], w.albedo]),
            // (The sky's own glow: the floor the meshes take, so ships and stations agree.)
            env_mode: [if frame.studio { 1.0 } else { 0.0 }, crate::frame::SHADE_AMBIENT, 0.0, 0.0],
            env_sky: frame.clear.0,
            view: [2.0 * (frame.camera.fov_y * 0.5).tan() / self.target.size.y as f32, world_fade, world_luts, 0.0],
            inv_view_proj: frame.camera.view_proj(size.x / size.y).inverse().to_cols_array_2d(),
            world_at,
            world_to_body: glam::Mat4::from_quat(frame.world_turn.inverse().as_quat()).to_cols_array_2d(),
        };
        gpu.queue.write_buffer(&self.shadows.lights[0], 0, bytemuck::cast_slice(&shadow_near.to_cols_array()));
        gpu.queue.write_buffer(&self.shadows.lights[1], 0, bytemuck::cast_slice(&shadow_far.to_cols_array()));
        gpu.queue.write_buffer(&self.shadows.lights[2], 0, bytemuck::cast_slice(&shadow_tight.to_cols_array()));
        gpu.queue.write_buffer(&self.shadows.lights[3], 0, bytemuck::cast_slice(&shadow_ground.to_cols_array()));
        let upload = universe_prof::scope("render/upload vertices");
        gpu.queue.write_buffer(&self.globals, 0, bytemuck::bytes_of(&globals));
        self.sky.upload(gpu, &frame.sky);
        self.solids.upload(gpu, &frame.solids);
        self.lines.upload(gpu, &frame.lines);
        self.front_lines.upload(gpu, &frame.front_lines);
        self.points.upload(gpu, &frame.points);
        self.glows.upload(gpu, &frame.glows);
        self.front_glows.upload(gpu, &frame.front_glows);
        self.hud_tris.upload_bytes(gpu, bytemuck::cast_slice(&frame.hud_tris), frame.hud_tris.len() as u32);
        self.hud.upload(gpu, &frame.hud);
        self.upload_meshes(gpu, frame);
        self.pbr.upload(&gpu.device, &gpu.queue, &frame.pbr, frame.shadow_reach as f32 * 1.5);
        drop(upload);

        let acquire = std::time::Instant::now();
        let next = gpu.surface.get_current_texture();
        self.wait = acquire.elapsed();
        // (No frame from the window (a screen asleep, a window hidden): nothing to show, but a
        // capture asked for is still drawn and saved, from the offscreen composite.)
        let surface_texture = match next {
            wgpu::CurrentSurfaceTexture::Success(t) | wgpu::CurrentSurfaceTexture::Suboptimal(t) => Some(t),
            wgpu::CurrentSurfaceTexture::Outdated | wgpu::CurrentSurfaceTexture::Lost => {
                gpu.surface.configure(&gpu.device, &gpu.config);
                None
            }
            _ => None,
        };
        if surface_texture.is_none() && capture.is_none() {
            return;
        }
        let surface_view = surface_texture.as_ref().map(|t| t.texture.create_view(&Default::default()));

        let mut encoder = gpu.device.create_command_encoder(&Default::default());
        // The environment as light, for this frame.
        self.env.render(&mut encoder, &self.globals_bind);
        // The bound world's clouds cached: filled, refreshed, re-centred under the eye.
        let clouded = frame.world_maps.as_ref().filter(|(maps, ..)| frame.graphics.clouds && maps.has_clouds() && frame.world_clouds[3] > 0.0);
        let under = clouded.map_or(glam::DVec3::Y, |(_, _, c, _)| frame.world_turn.inverse() * (frame.camera.position - *c).normalize_or(glam::DVec3::Y));
        let world = clouded.and_then(|(maps, _, _, r)| Some((maps.id(), *r, &self.world.clouds, self.world.cloud_maps.as_ref()?)));
        self.world.cache.frame(&gpu.device, &gpu.queue, &mut encoder, world, under);
        // The shadow map: each cascade, the casters seen from the light.
        for k in 0..4 {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("shadow map"),
                color_attachments: &[],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.shadows.layers[k],
                    depth_ops: Some(wgpu::Operations { load: wgpu::LoadOp::Clear(1.0), store: wgpu::StoreOp::Store }),
                    stencil_ops: None,
                }),
                timestamp_writes: self.gputime.as_ref().map(|t| t.shadow(k as u32)),
                occlusion_query_set: None,
                multiview_mask: None,
            });
            if sun.is_some() {
                pass.set_bind_group(0, &self.shadows.light_binds[k], &[]);
                // (The ground's own cascade: the ground's patches alone.)
                if k == 3 {
                    self.draw_meshes(&mut pass, &self.shadows.ground_runs, &self.shadows.ground_pipe, |m| (&m.faces, m.face_vertices));
                    continue;
                }
                self.draw_meshes(&mut pass, &self.shadows.runs, &self.shadows.pipe, |m| (&m.faces, m.face_vertices));
                self.pbr.draw_shadows(&mut pass);
            }
        }
        {
            let [r, g, b, a] = frame.clear.0.map(f64::from);
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("scene"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &self.target.color_msaa,
                    depth_slice: None,
                    resolve_target: Some(&self.target.color),
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color { r, g, b, a }),
                        store: wgpu::StoreOp::Discard,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.target.depth,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(0.0),
                        store: if probe.is_some() { wgpu::StoreOp::Store } else { wgpu::StoreOp::Discard },
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: self.gputime.as_ref().map(|t| t.pass(1)),
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_bind_group(0, &self.globals_bind, &[]);
            pass.set_bind_group(1, &self.shadows.bind, &[]);
            pass.set_bind_group(2, &self.world.bind, &[]);
            self.sky.draw(&mut pass, &self.sky_pipe);
            // The air's light, a world's in view (nothing where none is bound, or it's airless).
            pass.set_pipeline(&self.air_sky_pipe);
            pass.draw(0..3, 0..1);
            self.solids.draw(&mut pass, &self.solid_pipe);
            self.draw_meshes(&mut pass, &self.face_runs, &self.mesh_pipe, |m| (&m.faces, m.face_vertices));
            self.pbr.draw(&mut pass);
            // (The textured models take group 2 for their materials: the world's maps back.)
            pass.set_bind_group(2, &self.world.bind, &[]);
            self.lines.draw(&mut pass, &self.line_pipe);
            self.draw_meshes(&mut pass, &self.edge_runs, &self.mesh_line_pipe, |m| (&m.edges, m.edge_vertices));
            self.points.draw(&mut pass, &self.point_pipe);
            self.glows.draw(&mut pass, &self.glow_pipe);
        }
        if probe.is_some() {
            self.sunprobe.run(&gpu.device, &mut encoder, &self.target.depth, false);
        }
        {
            // The front layer: its meshes (and lines), with a fresh depth.
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("front"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &self.target.front_msaa,
                    depth_slice: None,
                    resolve_target: Some(&self.target.front),
                    ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT), store: wgpu::StoreOp::Discard },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.target.depth,
                    depth_ops: Some(wgpu::Operations { load: wgpu::LoadOp::Clear(0.0), store: if probe.is_some() { wgpu::StoreOp::Store } else { wgpu::StoreOp::Discard } }),
                    stencil_ops: None,
                }),
                timestamp_writes: self.gputime.as_ref().map(|t| t.pass(2)),
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_bind_group(0, &self.globals_bind, &[]);
            pass.set_bind_group(1, &self.shadows.bind, &[]);
            pass.set_bind_group(2, &self.world.bind, &[]);
            self.draw_meshes(&mut pass, &self.front_face_runs, &self.mesh_pipe, |m| (&m.faces, m.face_vertices));
            self.front_lines.draw(&mut pass, &self.line_pipe);
            self.draw_meshes(&mut pass, &self.front_edge_runs, &self.mesh_line_pipe, |m| (&m.edges, m.edge_vertices));
            self.front_glows.draw(&mut pass, &self.glow_pipe);
        }
        if probe.is_some() {
            self.sunprobe.run(&gpu.device, &mut encoder, &self.target.depth, true);
            self.sunprobe.copy_out(&mut encoder);
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
                timestamp_writes: self.gputime.as_ref().map(|t| t.pass(3)),
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_bind_group(0, &self.globals_bind, &[]);
            pass.set_bind_group(1, &self.atlas_bind, &[]);
            self.hud_tris.draw(&mut pass, &self.hud_tri_pipe);
            self.hud.draw(&mut pass, &self.hud_pipe);
        }
        let readback = capture.map(|_| {
            self.composite(&mut encoder, &self.target.composite, &self.capture_pipe);
            self.copy_to_buffer(gpu, &mut encoder)
        });
        if let Some(surface_view) = &surface_view {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("upscale"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: surface_view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: self.gputime.as_ref().map(|t| t.pass(4)),
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(&self.blit_pipe);
            pass.set_bind_group(0, &self.target.blit, &[]);
            pass.draw(0..3, 0..1);
        }
        if let Some(t) = self.gputime.as_mut() {
            t.copy_out(&mut encoder);
        }
        gpu.queue.submit([encoder.finish()]);
        self.sunprobe.submitted();
        if let Some(t) = self.gputime.as_mut() {
            t.submitted();
        }
        if let Some(t) = surface_texture {
            gpu.queue.present(t);
        }

        if let (Some(path), Some((buffer, padded_row))) = (capture, readback) {
            match self.save_png(gpu, &buffer, padded_row, path) {
                Ok(()) => log::debug!("screenshot saved to {}", path.display()),
                Err(e) => log::error!("screenshot failed: {e}"),
            }
        }
    }

    /// Meshes for this frame: new ones uploaded, unused ones dropped; the
    /// instances grouped by mesh (faces, then edges) into one buffer.
    fn upload_meshes(&mut self, gpu: &Gpu, frame: &Frame) {
        self.frames += 1;
        let now = self.frames;
        for d in frame.meshes.iter().chain(&frame.front) {
            self.meshes.entry(d.mesh.id()).or_insert_with(|| GpuMesh::new(&gpu.device, &d.mesh)).used = now;
        }
        self.meshes.retain(|_, m| now - m.used < MESH_KEEP);
        let mut data: Vec<Instance> = Vec::with_capacity((frame.meshes.len() + frame.front.len()) * 2);
        (self.face_runs, self.edge_runs) = batch(&frame.meshes, &mut data);
        (self.front_face_runs, self.front_edge_runs) = batch(&frame.front, &mut data);
        // Globe maps: each in a layer (new ones uploaded); instances name the layer.
        // With more maps than layers, those already up and the most drawn (the
        // world underfoot is many patches) come first; the rest go plain this frame.
        let mut uses = vec![0usize; frame.globe_maps.len()];
        for inst in &data {
            if inst.globe[0] > 0.5
                && let Some(u) = uses.get_mut(inst.globe[0] as usize - 1)
            {
                *u += 1;
            }
        }
        let mut order: Vec<usize> = (0..frame.globe_maps.len()).collect();
        order.sort_by_key(|&i| (!self.globes.resident(&frame.globe_maps[i]), std::cmp::Reverse(uses[i])));
        let mut layers: Vec<Option<u32>> = vec![None; frame.globe_maps.len()];
        for i in order {
            layers[i] = self.globes.layer(gpu, &frame.globe_maps[i], now);
        }
        for inst in &mut data {
            if inst.globe[0] > 0.5 {
                inst.globe[0] = layers.get(inst.globe[0] as usize - 1).copied().flatten().map_or(0.0, |l| l as f32 + 1.0);
            }
        }
        // The casters: those within reach of the shadow boxes.
        let reach = frame.shadow_reach as f32 * 1.5;
        let casting: Vec<&crate::frame::MeshDraw> = frame.meshes.iter().chain(&frame.front).filter(|d| d.casts && glam::Vec3::from_slice(&d.instance.t[..3]).length() - d.reach < reach).collect();
        self.shadows.runs = casters(&casting, &mut data);
        let ground: Vec<&crate::frame::MeshDraw> = frame.meshes.iter().filter(|d| d.ground && glam::Vec3::from_slice(&d.instance.t[..3]).length() - d.reach < (GROUND_HALF * 1.5) as f32).collect();
        self.shadows.ground_runs = casters(&ground, &mut data);
        self.instances.upload_bytes(gpu, bytemuck::cast_slice(&data), data.len() as u32);
    }

    fn draw_meshes(&self, pass: &mut wgpu::RenderPass<'_>, runs: &[(u64, u32, u32)], pipeline: &wgpu::RenderPipeline, part: impl Fn(&GpuMesh) -> (&wgpu::Buffer, u32)) {
        if runs.is_empty() {
            return;
        }
        pass.set_pipeline(pipeline);
        pass.set_vertex_buffer(1, self.instances.buffer.slice(..));
        for &(id, first, count) in runs {
            let Some(m) = self.meshes.get(&id) else { continue };
            let (buffer, vertices) = part(m);
            if vertices == 0 {
                continue;
            }
            pass.set_vertex_buffer(0, buffer.slice(..));
            pass.draw(0..vertices, first..first + count);
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
        let size = self.target.out;
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
        let size = self.target.out;
        buffer.slice(..).map_async(wgpu::MapMode::Read, |_| {});
        gpu.device.poll(wgpu::PollType::wait_indefinitely()).map_err(|e| e.to_string())?;
        let mapped = buffer.slice(..).get_mapped_range().map_err(|e| e.to_string())?;
        let mut pixels = Vec::with_capacity((size.x * size.y * 4) as usize);
        for row in mapped.chunks(padded_row as usize) {
            pixels.extend_from_slice(&row[..(size.x * 4) as usize]);
        }
        drop(mapped);
        buffer.unmap();
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        }
        // (Compressed and written off the render thread: a recording captures every frame.)
        let path = path.to_path_buf();
        WRITING.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        std::thread::Builder::new()
            .name("png".into())
            .spawn(move || {
                let _done = Written;
                let write = || -> Result<(), String> {
                    let file = std::fs::File::create(&path).map_err(|e| e.to_string())?;
                    let mut encoder = png::Encoder::new(std::io::BufWriter::new(file), size.x, size.y);
                    encoder.set_color(png::ColorType::Rgba);
                    encoder.set_depth(png::BitDepth::Eight);
                    encoder.set_compression(png::Compression::Fast);
                    let mut writer = encoder.write_header().map_err(|e| e.to_string())?;
                    writer.write_image_data(&pixels).map_err(|e| e.to_string())
                };
                if let Err(e) = write() {
                    log::error!("screenshot {} failed: {e}", path.display());
                }
            })
            .map(|_| ())
            .map_err(|e| e.to_string())
    }
}

/// Mesh draws as instances (appended to `data`), sorted by mesh: the runs
/// (mesh, first instance, count) for their faces, and for their edges.
type Runs = Vec<(u64, u32, u32)>;
/// The casters' instances, grouped by mesh into runs.
fn casters(draws: &[&crate::frame::MeshDraw], data: &mut Vec<Instance>) -> Runs {
    let mut order: Vec<usize> = (0..draws.len()).collect();
    order.sort_by_key(|&i| draws[i].mesh.id());
    let mut runs: Runs = Vec::new();
    for i in order {
        let id = draws[i].mesh.id();
        match runs.last_mut() {
            Some(r) if r.0 == id => r.2 += 1,
            _ => runs.push((id, data.len() as u32, 1)),
        }
        data.push(draws[i].instance);
    }
    runs
}

fn batch(draws: &[crate::frame::MeshDraw], data: &mut Vec<Instance>) -> (Runs, Runs) {
    let mut order: Vec<usize> = (0..draws.len()).collect();
    order.sort_by_key(|&i| draws[i].mesh.id());
    let mut runs = |pick: &dyn Fn(usize) -> bool| {
        let mut runs: Runs = Vec::new();
        for &i in order.iter().filter(|&&i| pick(i)) {
            let id = draws[i].mesh.id();
            match runs.last_mut() {
                Some(r) if r.0 == id => r.2 += 1,
                _ => runs.push((id, data.len() as u32, 1)),
            }
            data.push(draws[i].instance);
        }
        runs
    };
    let faces = runs(&|_| true);
    let edges = runs(&|i| draws[i].edges);
    (faces, edges)
}

/// Captures still being compressed and written (see `save_png`).
static WRITING: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

/// One fewer being written, when dropped (however the write ended).
struct Written;
impl Drop for Written {
    fn drop(&mut self) {
        WRITING.fetch_sub(1, std::sync::atomic::Ordering::SeqCst);
    }
}

/// Wait (a few seconds at most) for the captures still being written.
pub fn wait_for_writes() {
    let start = std::time::Instant::now();
    while WRITING.load(std::sync::atomic::Ordering::SeqCst) > 0 && start.elapsed() < std::time::Duration::from_secs(10) {
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
}
