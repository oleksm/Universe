//! Textured, physically based models (glTF 2.0): what an artist makes in a
//! modelling tool (Blender), drawn as it was made. Each model is primitives
//! (vertices with normals, tangents and texture coordinates, and triangles)
//! and materials (glTF's metallic-roughness: base colour, metallic and
//! roughness, a normal map, emission), each with optional textures. Lit as
//! meshes are (the sun, its shadows, the reflecting planet: see
//! `frame::Instance`), shaded per pixel by `pbr.wgsl`.

use std::collections::HashMap;
use std::sync::Arc;

use glam::{Vec2, Vec3};

/// A model to draw, kept on the GPU once drawn. Cheap to clone (shared).
#[derive(Clone, Debug)]
pub struct PbrModel {
    id: u64,
    pub data: Arc<PbrData>,
    radius: f32,
}

#[derive(Debug, Default)]
pub struct PbrData {
    pub primitives: Vec<Primitive>,
    pub materials: Vec<Material>,
    pub images: Vec<Image>,
}

#[derive(Debug)]
pub struct Primitive {
    pub vertices: Vec<PbrVertex>,
    pub indices: Vec<u32>,
    pub material: usize,
    /// Which part of the model it belongs to (0: the body; a moving part,
    /// drawn on its own transform: see `load_gltf_parts`).
    pub part: u8,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct PbrVertex {
    pub pos: [f32; 3],
    pub normal: [f32; 3],
    /// xyz: along the texture's u; w: the bitangent's handedness (±1).
    pub tangent: [f32; 4],
    pub uv: [f32; 2],
}

/// glTF's metallic-roughness material; textures by index into the images.
#[derive(Clone, Debug)]
pub struct Material {
    pub base_color: [f32; 4],
    pub metallic: f32,
    pub roughness: f32,
    pub emissive: [f32; 3],
    pub normal_scale: f32,
    pub base_tex: Option<usize>,
    /// Roughness in G, metallic in B (glTF's packing).
    pub mr_tex: Option<usize>,
    pub normal_tex: Option<usize>,
    pub emissive_tex: Option<usize>,
    /// Ambient occlusion in R (how much light from round about reaches), and its strength.
    pub occlusion_tex: Option<usize>,
    pub occlusion_strength: f32,
    /// glTF's alpha mask: below this, the pixel isn't drawn (decals,
    /// grilles cut from a texture); None: opaque.
    pub alpha_cutoff: Option<f32>,
    /// glTF's alpha blend: see-through by its base colour's alpha (glass),
    /// drawn after what's solid.
    pub blend: bool,
    /// Seen from both sides (glTF's `doubleSided`): a surface with no
    /// thickness (a nozzle's bell, a fin) shows from behind too.
    pub double_sided: bool,
}

/// An image, RGBA8 rows top down.
#[derive(Debug)]
pub struct Image {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

impl PbrModel {
    pub fn new(data: PbrData) -> Self {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
        let radius = data.primitives.iter().flat_map(|p| &p.vertices).map(|v| Vec3::from(v.pos).length()).fold(0.0, f32::max);
        PbrModel { id: NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed), data: Arc::new(data), radius }
    }

    pub fn id(&self) -> u64 {
        self.id
    }

    /// How far its farthest point is from its origin (its units: metres).
    pub fn radius(&self) -> f32 {
        self.radius
    }

    /// A model from a glTF file's bytes (`.glb`, or `.gltf` with its data
    /// embedded): every mesh in the default scene, placed by its node.
    pub fn load_gltf(bytes: &[u8]) -> Result<Self, String> {
        Self::load_gltf_parts(bytes, &[])
    }

    /// `load_gltf`, with moving parts: a node whose name holds `parts[k]` (and
    /// what hangs from it) is part `k + 1`, drawn on its own (`Frame::model_pbr_part`).
    pub fn load_gltf_parts(bytes: &[u8], parts: &[&str]) -> Result<Self, String> {
        let (doc, buffers, images) = gltf::import_slice(bytes).map_err(|e| e.to_string())?;
        let mut data = PbrData { images: images.iter().map(to_rgba).collect(), ..Default::default() };
        let tex = |t: Option<gltf::texture::Texture>| t.map(|t| t.source().index());
        for m in doc.materials() {
            let pbr = m.pbr_metallic_roughness();
            data.materials.push(Material {
                base_color: pbr.base_color_factor(),
                metallic: pbr.metallic_factor(),
                roughness: pbr.roughness_factor(),
                // (Times its strength, KHR_materials_emissive_strength: lamps far brighter than 1.)
                emissive: m.emissive_factor().map(|c| c * m.emissive_strength().unwrap_or(1.0)),
                normal_scale: m.normal_texture().map_or(1.0, |n| n.scale()),
                base_tex: tex(pbr.base_color_texture().map(|i| i.texture())),
                mr_tex: tex(pbr.metallic_roughness_texture().map(|i| i.texture())),
                normal_tex: tex(m.normal_texture().map(|n| n.texture())),
                emissive_tex: tex(m.emissive_texture().map(|i| i.texture())),
                occlusion_tex: tex(m.occlusion_texture().map(|o| o.texture())),
                occlusion_strength: m.occlusion_texture().map_or(1.0, |o| o.strength()),
                alpha_cutoff: (m.alpha_mode() == gltf::material::AlphaMode::Mask).then(|| m.alpha_cutoff().unwrap_or(0.5)),
                blend: m.alpha_mode() == gltf::material::AlphaMode::Blend,
                double_sided: m.double_sided(),
            });
        }
        let fallback = data.materials.len();
        let scene = doc.default_scene().or_else(|| doc.scenes().next()).ok_or("no scene")?;
        for node in scene.nodes() {
            walk(&node, glam::Mat4::IDENTITY, &buffers, parts, 0, &mut data);
        }
        // (A primitive with no material: a plain grey one.)
        if data.primitives.iter().any(|p| p.material == usize::MAX) {
            data.materials.push(Material { base_color: [0.6, 0.6, 0.6, 1.0], metallic: 0.0, roughness: 0.6, emissive: [0.0; 3], normal_scale: 1.0, base_tex: None, mr_tex: None, normal_tex: None, emissive_tex: None, occlusion_tex: None, occlusion_strength: 1.0, alpha_cutoff: None, blend: false, double_sided: false });
            for p in &mut data.primitives {
                if p.material == usize::MAX {
                    p.material = fallback;
                }
            }
        }
        if data.primitives.is_empty() {
            return Err("no meshes".into());
        }
        Ok(Self::new(data))
    }
}

fn walk(node: &gltf::Node, parent: glam::Mat4, buffers: &[gltf::buffer::Data], parts: &[&str], part: u8, data: &mut PbrData) {
    let m = parent * glam::Mat4::from_cols_array_2d(&node.transform().matrix());
    let part = node.name().and_then(|n| parts.iter().position(|p| n.contains(p))).map_or(part, |k| k as u8 + 1);
    let normal_m = glam::Mat3::from_mat4(m).inverse().transpose();
    // (Collision parts, `COL_*`, are the hull's shape for physics, not drawn.)
    let hidden = node.name().is_some_and(|n| n.starts_with("COL_"));
    if let Some(mesh) = node.mesh().filter(|_| !hidden) {
        for prim in mesh.primitives() {
            let r = prim.reader(|b| Some(&buffers[b.index()]));
            let Some(pos) = r.read_positions() else { continue };
            let pos: Vec<Vec3> = pos.map(|p| m.transform_point3(Vec3::from(p))).collect();
            let n = pos.len();
            let normals: Vec<Vec3> = r.read_normals().map_or_else(|| vec![Vec3::Y; n], |it| it.map(|v| (normal_m * Vec3::from(v)).normalize_or_zero()).collect());
            let uvs: Vec<Vec2> = r.read_tex_coords(0).map_or_else(|| vec![Vec2::ZERO; n], |it| it.into_f32().map(Vec2::from).collect());
            let indices: Vec<u32> = r.read_indices().map_or_else(|| (0..n as u32).collect(), |it| it.into_u32().collect());
            let tangents: Vec<[f32; 4]> = match r.read_tangents() {
                Some(it) => it.map(|t| { let d = (glam::Mat3::from_mat4(m) * Vec3::new(t[0], t[1], t[2])).normalize_or_zero(); [d.x, d.y, d.z, t[3]] }).collect(),
                None => tangents(&pos, &normals, &uvs, &indices),
            };
            let vertices = (0..n).map(|i| PbrVertex { pos: pos[i].to_array(), normal: normals[i].to_array(), tangent: tangents[i], uv: uvs[i].to_array() }).collect();
            data.primitives.push(Primitive { vertices, indices, material: prim.material().index().unwrap_or(usize::MAX), part });
        }
    }
    for child in node.children() {
        walk(&child, m, buffers, parts, part, data);
    }
}

/// Tangents from the texture's directions over each triangle, summed at
/// its corners and made square to the normal (for files exported without).
fn tangents(pos: &[Vec3], normals: &[Vec3], uvs: &[Vec2], indices: &[u32]) -> Vec<[f32; 4]> {
    let mut t = vec![Vec3::ZERO; pos.len()];
    let mut b = vec![Vec3::ZERO; pos.len()];
    for tri in indices.as_chunks::<3>().0 {
        let [i0, i1, i2] = [tri[0] as usize, tri[1] as usize, tri[2] as usize];
        let (e1, e2) = (pos[i1] - pos[i0], pos[i2] - pos[i0]);
        let (d1, d2) = (uvs[i1] - uvs[i0], uvs[i2] - uvs[i0]);
        let det = d1.x * d2.y - d2.x * d1.y;
        if det.abs() < 1e-12 {
            continue;
        }
        let r = 1.0 / det;
        let (tu, tv) = ((e1 * d2.y - e2 * d1.y) * r, (e2 * d1.x - e1 * d2.x) * r);
        for i in [i0, i1, i2] {
            t[i] += tu;
            b[i] += tv;
        }
    }
    (0..pos.len())
        .map(|i| {
            let n = normals[i];
            let tt = (t[i] - n * n.dot(t[i])).try_normalize().unwrap_or_else(|| n.any_orthonormal_vector());
            let w = if n.cross(tt).dot(b[i]) < 0.0 { -1.0 } else { 1.0 };
            [tt.x, tt.y, tt.z, w]
        })
        .collect()
}

fn to_rgba(img: &gltf::image::Data) -> Image {
    use gltf::image::Format as F;
    let px = (img.width * img.height) as usize;
    let rgba = match img.format {
        F::R8G8B8A8 => img.pixels.clone(),
        F::R8G8B8 => img.pixels.as_chunks::<3>().0.iter().flat_map(|c| [c[0], c[1], c[2], 255]).collect(),
        F::R8G8 => img.pixels.as_chunks::<2>().0.iter().flat_map(|c| [c[0], c[1], 0, 255]).collect(),
        F::R8 => img.pixels.iter().flat_map(|&c| [c, c, c, 255]).collect(),
        // (16-bit channels: their high bytes.)
        F::R16G16B16A16 => img.pixels.as_chunks::<8>().0.iter().flat_map(|c| [c[1], c[3], c[5], c[7]]).collect(),
        F::R16G16B16 => img.pixels.as_chunks::<6>().0.iter().flat_map(|c| [c[1], c[3], c[5], 255]).collect(),
        _ => vec![255; px * 4],
    };
    Image { width: img.width, height: img.height, rgba }
}

/// The mip chain of an RGBA8 image: each level half the last, box filtered
/// (in linear light for colour textures: `srgb`).
pub(crate) fn mips(img: &Image, srgb: bool) -> Vec<(u32, u32, Vec<u8>)> {
    let to_lin = |c: u8| if srgb { (c as f32 / 255.0).powf(2.2) } else { c as f32 / 255.0 };
    let from_lin = |v: f32| ((if srgb { v.max(0.0).powf(1.0 / 2.2) } else { v }) * 255.0 + 0.5).clamp(0.0, 255.0) as u8;
    let mut out = vec![(img.width, img.height, img.rgba.clone())];
    while let Some((w, h, px)) = out.last().filter(|(w, h, _)| *w > 1 || *h > 1) {
        let (nw, nh) = ((*w / 2).max(1), (*h / 2).max(1));
        let mut next = vec![0u8; (nw * nh * 4) as usize];
        for y in 0..nh {
            for x in 0..nw {
                for c in 0..4 {
                    let mut sum = 0.0;
                    for (dx, dy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
                        let (sx, sy) = ((x * 2 + dx).min(w - 1), (y * 2 + dy).min(h - 1));
                        let v = px[((sy * w + sx) * 4 + c) as usize];
                        sum += if c == 3 { v as f32 / 255.0 } else { to_lin(v) };
                    }
                    let i = ((y * nw + x) * 4 + c) as usize;
                    next[i] = if c == 3 { (sum / 4.0 * 255.0 + 0.5) as u8 } else { from_lin(sum / 4.0) };
                }
            }
        }
        out.push((nw, nh, next));
    }
    out
}

/// A material's constants for the shader.
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct MaterialUniform {
    base_color: [f32; 4],
    /// x metallic, y roughness, z normal scale, w alpha cutoff (0: opaque).
    params: [f32; 4],
    emissive: [f32; 4],
    /// x occlusion strength, y 1: see-through (blended).
    extra: [f32; 4],
}

struct GpuModel {
    /// Each primitive: vertices, indices, index count, its material's bind group,
    /// which pipeline draws it (`PIPES`), its part.
    primitives: Vec<(wgpu::Buffer, wgpu::Buffer, u32, usize, usize, u8)>,
    materials: Vec<wgpu::BindGroup>,
    used: u64,
}

/// The renderer's side: the pipelines, models on the GPU, this frame's draws.
pub(crate) struct PbrRenderer {
    /// One for each kind of surface (`PIPES`), in the order they're drawn.
    pipes: Vec<wgpu::RenderPipeline>,
    shadow_pipe: wgpu::RenderPipeline,
    material_layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    models: HashMap<u64, GpuModel>,
    instances: wgpu::Buffer,
    capacity: u64,
    /// This frame: (model, its instance's index, the part drawn), and those that cast.
    draws: Vec<(u64, u32, u8)>,
    casters: Vec<(u64, u32, u8)>,
    frames: u64,
}

const VERTEX_ATTRS: [wgpu::VertexAttribute; 4] = wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x3, 2 => Float32x4, 3 => Float32x2];
/// The instance fields the shader uses (by their place in `frame::Instance`):
/// its turn and place, the sun's direction and light, the planet's.
const INSTANCE_ATTRS: [wgpu::VertexAttribute; 8] = [
    wgpu::VertexAttribute { format: wgpu::VertexFormat::Float32x4, offset: 0, shader_location: 4 },
    wgpu::VertexAttribute { format: wgpu::VertexFormat::Float32x4, offset: 16, shader_location: 5 },
    wgpu::VertexAttribute { format: wgpu::VertexFormat::Float32x4, offset: 32, shader_location: 6 },
    wgpu::VertexAttribute { format: wgpu::VertexFormat::Float32x4, offset: 48, shader_location: 7 },
    wgpu::VertexAttribute { format: wgpu::VertexFormat::Float32x4, offset: 96, shader_location: 8 },
    wgpu::VertexAttribute { format: wgpu::VertexFormat::Float32x4, offset: 112, shader_location: 9 },
    wgpu::VertexAttribute { format: wgpu::VertexFormat::Float32x4, offset: 128, shader_location: 10 },
    wgpu::VertexAttribute { format: wgpu::VertexFormat::Float32x4, offset: 144, shader_location: 11 },
];
/// The pipelines, by kind of surface: solid (one side, two), see-through (one, two).
fn pipe_for(blend: bool, double_sided: bool) -> usize {
    usize::from(blend) * SEE_THROUGH + usize::from(double_sided)
}
/// Where the see-through pipelines start.
const SEE_THROUGH: usize = 2;

/// Models not drawn for this many frames leave the GPU.
const KEEP: u64 = 600;

impl PbrRenderer {
    /// Models uploaded.
    pub fn model_count(&self) -> usize {
        self.models.len()
    }

    pub(crate) fn new(device: &wgpu::Device, globals: &wgpu::BindGroupLayout, shadows: &wgpu::BindGroupLayout, light: &wgpu::BindGroupLayout, scene_format: wgpu::TextureFormat, depth_format: wgpu::TextureFormat, samples: u32) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor { label: Some("pbr"), source: wgpu::ShaderSource::Wgsl(concat!(include_str!("shaders/light.wgsl"), "\n", include_str!("shaders/pbr.wgsl")).into()) });
        let tex_entry = |binding| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Texture { sample_type: wgpu::TextureSampleType::Float { filterable: true }, view_dimension: wgpu::TextureViewDimension::D2, multisampled: false },
            count: None,
        };
        let material_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("pbr material"),
            entries: &[
                wgpu::BindGroupLayoutEntry { binding: 0, visibility: wgpu::ShaderStages::FRAGMENT, ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Uniform, has_dynamic_offset: false, min_binding_size: None }, count: None },
                tex_entry(1),
                tex_entry(2),
                tex_entry(3),
                tex_entry(4),
                wgpu::BindGroupLayoutEntry { binding: 5, visibility: wgpu::ShaderStages::FRAGMENT, ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering), count: None },
                tex_entry(6),
            ],
        });
        let buffers = [
            Some(wgpu::VertexBufferLayout { array_stride: size_of::<PbrVertex>() as u64, step_mode: wgpu::VertexStepMode::Vertex, attributes: &VERTEX_ATTRS }),
            Some(wgpu::VertexBufferLayout { array_stride: size_of::<crate::frame::Instance>() as u64, step_mode: wgpu::VertexStepMode::Instance, attributes: &INSTANCE_ATTRS }),
        ];
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor { label: Some("pbr"), bind_group_layouts: &[Some(globals), Some(shadows), Some(&material_layout)], immediate_size: 0 });
        // (Glass: its colour already weighed by its alpha in the shader, what it mirrors
        // not; what's behind shows through by the rest. It doesn't hide what's behind it.)
        let make = |label, see_through: bool, two_sided: bool| {
            let blend = see_through.then_some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING);
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(label),
                layout: Some(&layout),
                vertex: wgpu::VertexState { module: &shader, entry_point: Some("vs_pbr"), compilation_options: Default::default(), buffers: &buffers },
                primitive: wgpu::PrimitiveState { topology: wgpu::PrimitiveTopology::TriangleList, cull_mode: (!two_sided).then_some(wgpu::Face::Back), ..Default::default() },
                depth_stencil: Some(wgpu::DepthStencilState { format: depth_format, depth_write_enabled: Some(!see_through), depth_compare: Some(wgpu::CompareFunction::Greater), stencil: Default::default(), bias: Default::default() }),
                multisample: wgpu::MultisampleState { count: samples, ..Default::default() },
                fragment: Some(wgpu::FragmentState { module: &shader, entry_point: Some("fs_pbr"), compilation_options: Default::default(), targets: &[Some(wgpu::ColorTargetState { format: scene_format, blend, write_mask: wgpu::ColorWrites::ALL })] }),
                multiview_mask: None,
                cache: None,
            })
        };
        let pipes = vec![make("pbr", false, false), make("pbr two-sided", false, true), make("pbr see-through", true, false), make("pbr see-through two-sided", true, true)];
        let shadow_shader = device.create_shader_module(wgpu::include_wgsl!("shaders/pbr_shadow.wgsl"));
        let shadow_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor { label: Some("pbr shadow"), bind_group_layouts: &[Some(light)], immediate_size: 0 });
        let shadow_pipe = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("pbr shadow casters"),
            layout: Some(&shadow_layout),
            vertex: wgpu::VertexState { module: &shadow_shader, entry_point: Some("vs_pbr_shadow"), compilation_options: Default::default(), buffers: &buffers },
            primitive: wgpu::PrimitiveState { topology: wgpu::PrimitiveTopology::TriangleList, ..Default::default() },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: depth_format,
                depth_write_enabled: Some(true),
                depth_compare: Some(wgpu::CompareFunction::LessEqual),
                stencil: Default::default(),
                bias: wgpu::DepthBiasState { constant: 2, slope_scale: 2.0, clamp: 0.0 },
            }),
            multisample: Default::default(),
            fragment: None,
            multiview_mask: None,
            cache: None,
        });
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("pbr"),
            address_mode_u: wgpu::AddressMode::Repeat,
            address_mode_v: wgpu::AddressMode::Repeat,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Linear,
            anisotropy_clamp: 16,
            ..Default::default()
        });
        let instances = device.create_buffer(&wgpu::BufferDescriptor { label: Some("pbr instances"), size: 64 * size_of::<crate::frame::Instance>() as u64, usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST, mapped_at_creation: false });
        PbrRenderer { pipes, shadow_pipe, material_layout, sampler, models: HashMap::new(), instances, capacity: 64, draws: Vec::new(), casters: Vec::new(), frames: 0 }
    }

    /// This frame's models up (new ones uploaded, old ones dropped), their
    /// instances, and which cast shadows within `reach` of the eye.
    pub(crate) fn upload(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, draws: &[crate::frame::PbrDraw], reach: f32) {
        self.frames += 1;
        let now = self.frames;
        for d in draws {
            if !self.models.contains_key(&d.model.id()) {
                let m = self.gpu_model(device, queue, &d.model);
                self.models.insert(d.model.id(), m);
            }
            if let Some(m) = self.models.get_mut(&d.model.id()) {
                m.used = now;
            }
        }
        self.models.retain(|_, m| now - m.used < KEEP);
        let data: Vec<crate::frame::Instance> = draws.iter().map(|d| d.instance).collect();
        if data.len() as u64 > self.capacity {
            self.capacity = (data.len() as u64).next_power_of_two();
            self.instances = device.create_buffer(&wgpu::BufferDescriptor { label: Some("pbr instances"), size: self.capacity * size_of::<crate::frame::Instance>() as u64, usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST, mapped_at_creation: false });
        }
        if !data.is_empty() {
            queue.write_buffer(&self.instances, 0, bytemuck::cast_slice(&data));
        }
        self.draws = draws.iter().enumerate().map(|(i, d)| (d.model.id(), i as u32, d.part)).collect();
        self.casters = draws.iter().enumerate().filter(|(_, d)| d.casts && Vec3::from_slice(&d.instance.t[..3]).length() - d.reach < reach).map(|(i, d)| (d.model.id(), i as u32, d.part)).collect();
    }

    fn gpu_model(&self, device: &wgpu::Device, queue: &wgpu::Queue, model: &PbrModel) -> GpuModel {
        use wgpu::util::DeviceExt;
        let data = &model.data;
        // Each image as the textures its uses need: colour ones in sRGB, the rest linear.
        let mut textures: HashMap<(usize, Kind), wgpu::TextureView> = HashMap::new();
        let mut texture = |i: usize, kind: Kind| -> wgpu::TextureView {
            textures.entry((i, kind)).or_insert_with(|| upload_texture(device, queue, &data.images[i], kind)).clone()
        };
        let plain = |px: [u8; 4], kind: Kind| upload_texture(device, queue, &Image { width: 1, height: 1, rgba: px.to_vec() }, kind);
        let (white_srgb, white, flat) = (plain([255; 4], Kind::Colour), plain([255; 4], Kind::Data), plain([128, 128, 255, 255], Kind::Normal));
        let materials = data
            .materials
            .iter()
            .map(|m| {
                let uniform = MaterialUniform { base_color: m.base_color, params: [m.metallic, m.roughness, m.normal_scale, m.alpha_cutoff.unwrap_or(0.0)], emissive: [m.emissive[0], m.emissive[1], m.emissive[2], 0.0], extra: [m.occlusion_strength, if m.blend { 1.0 } else { 0.0 }, 0.0, 0.0] };
                let buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor { label: Some("pbr material"), contents: bytemuck::bytes_of(&uniform), usage: wgpu::BufferUsages::UNIFORM });
                let base = m.base_tex.map_or_else(|| white_srgb.clone(), |i| texture(i, Kind::Colour));
                let mr = m.mr_tex.map_or_else(|| white.clone(), |i| texture(i, Kind::Data));
                let normal = m.normal_tex.map_or_else(|| flat.clone(), |i| texture(i, Kind::Normal));
                // (No texture: the factor alone, as glTF has it. Black here put out every untextured lamp.)
                let emissive = m.emissive_tex.map_or_else(|| white_srgb.clone(), |i| texture(i, Kind::Colour));
                let occlusion = m.occlusion_tex.map_or_else(|| white.clone(), |i| texture(i, Kind::Data));
                device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("pbr material"),
                    layout: &self.material_layout,
                    entries: &[
                        wgpu::BindGroupEntry { binding: 0, resource: buffer.as_entire_binding() },
                        wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(&base) },
                        wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::TextureView(&mr) },
                        wgpu::BindGroupEntry { binding: 3, resource: wgpu::BindingResource::TextureView(&normal) },
                        wgpu::BindGroupEntry { binding: 4, resource: wgpu::BindingResource::TextureView(&emissive) },
                        wgpu::BindGroupEntry { binding: 5, resource: wgpu::BindingResource::Sampler(&self.sampler) },
                        wgpu::BindGroupEntry { binding: 6, resource: wgpu::BindingResource::TextureView(&occlusion) },
                    ],
                })
            })
            .collect();
        let primitives = data
            .primitives
            .iter()
            .map(|p| {
                let v = device.create_buffer_init(&wgpu::util::BufferInitDescriptor { label: Some("pbr vertices"), contents: bytemuck::cast_slice(&p.vertices), usage: wgpu::BufferUsages::VERTEX });
                let i = device.create_buffer_init(&wgpu::util::BufferInitDescriptor { label: Some("pbr indices"), contents: bytemuck::cast_slice(&p.indices), usage: wgpu::BufferUsages::INDEX });
                let m = &data.materials[p.material];
                (v, i, p.indices.len() as u32, p.material, pipe_for(m.blend, m.double_sided), p.part)
            })
            .collect();
        GpuModel { primitives, materials, used: 0 }
    }

    /// The models in the scene pass (its globals and shadows bound already).
    pub(crate) fn draw<'a>(&'a self, pass: &mut wgpu::RenderPass<'a>) {
        if self.draws.is_empty() {
            return;
        }
        pass.set_vertex_buffer(1, self.instances.slice(..));
        // (What's solid first, then the see-through over it.)
        for (kind, pipe) in self.pipes.iter().enumerate() {
            pass.set_pipeline(pipe);
            for &(id, k, part) in &self.draws {
                let Some(m) = self.models.get(&id) else { continue };
                for (v, i, n, mat, pk, p) in &m.primitives {
                    if *pk != kind || *p != part {
                        continue;
                    }
                    pass.set_bind_group(2, &m.materials[*mat], &[]);
                    pass.set_vertex_buffer(0, v.slice(..));
                    pass.set_index_buffer(i.slice(..), wgpu::IndexFormat::Uint32);
                    pass.draw_indexed(0..*n, 0, k..k + 1);
                }
            }
        }
    }

    /// The casters in a shadow cascade's pass (its light bound already).
    pub(crate) fn draw_shadows<'a>(&'a self, pass: &mut wgpu::RenderPass<'a>) {
        if self.casters.is_empty() {
            return;
        }
        pass.set_pipeline(&self.shadow_pipe);
        pass.set_vertex_buffer(1, self.instances.slice(..));
        for &(id, k, part) in &self.casters {
            let Some(m) = self.models.get(&id) else { continue };
            // (Glass lets the sun through.)
            for (v, i, n, _, _, _) in m.primitives.iter().filter(|p| p.4 < SEE_THROUGH && p.5 == part) {
                pass.set_vertex_buffer(0, v.slice(..));
                pass.set_index_buffer(i.slice(..), wgpu::IndexFormat::Uint32);
                pass.draw_indexed(0..*n, 0, k..k + 1);
            }
        }
    }
}

/// What a texture holds: colour (sRGB), other data (linear), a normal map.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
enum Kind {
    Colour,
    Data,
    Normal,
}

fn upload_texture(device: &wgpu::Device, queue: &wgpu::Queue, img: &Image, kind: Kind) -> wgpu::TextureView {
    let srgb = kind == Kind::Colour;
    let levels = mips(img, srgb);
    // Block-compressed where the GPU can (sides whole blocks, not tiny): colour and
    // data as BC1 (an eighth of the memory), normals as BC5 (a quarter; smooth).
    if device.features().contains(wgpu::Features::TEXTURE_COMPRESSION_BC) && img.width % 4 == 0 && img.height % 4 == 0 && img.width >= 64 && img.height >= 64 {
        return upload_compressed(device, queue, img, &levels, kind);
    }
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("pbr texture"),
        size: wgpu::Extent3d { width: img.width, height: img.height, depth_or_array_layers: 1 },
        mip_level_count: levels.len() as u32,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: if srgb { wgpu::TextureFormat::Rgba8UnormSrgb } else { wgpu::TextureFormat::Rgba8Unorm },
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    for (level, (w, h, px)) in levels.iter().enumerate() {
        queue.write_texture(
            wgpu::TexelCopyTextureInfo { texture: &texture, mip_level: level as u32, origin: wgpu::Origin3d::ZERO, aspect: wgpu::TextureAspect::All },
            px,
            wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(w * 4), rows_per_image: Some(*h) },
            wgpu::Extent3d { width: *w, height: *h, depth_or_array_layers: 1 },
        );
    }
    texture.create_view(&Default::default())
}

/// `upload_texture`, block-compressed: each mip level encoded (side by side
/// on the CPU's threads) and uploaded as whole blocks.
fn upload_compressed(device: &wgpu::Device, queue: &wgpu::Queue, img: &Image, levels: &[(u32, u32, Vec<u8>)], kind: Kind) -> wgpu::TextureView {
    let started = std::time::Instant::now();
    let (codec, format) = match kind {
        Kind::Colour => (texpresso::Format::Bc1, wgpu::TextureFormat::Bc1RgbaUnormSrgb),
        Kind::Data => (texpresso::Format::Bc1, wgpu::TextureFormat::Bc1RgbaUnorm),
        Kind::Normal => (texpresso::Format::Bc5, wgpu::TextureFormat::Bc5RgUnorm),
    };
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("pbr texture (compressed)"),
        size: wgpu::Extent3d { width: img.width, height: img.height, depth_or_array_layers: 1 },
        mip_level_count: levels.len() as u32,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    // (Colour fitted carefully; occlusion, roughness and metalness, smooth fields, the fast way.)
    let algorithm = if kind == Kind::Data { texpresso::Algorithm::RangeFit } else { texpresso::Algorithm::ClusterFit };
    let params = texpresso::Params { algorithm, ..Default::default() };
    let block = if codec == texpresso::Format::Bc1 { 8 } else { 16 };
    for (level, (w, h, px)) in levels.iter().enumerate() {
        let (bw, bh) = (w.div_ceil(4), h.div_ceil(4));
        let mut out = vec![0u8; codec.compressed_size(*w as usize, *h as usize)];
        codec.compress(px, *w as usize, *h as usize, params, &mut out);
        queue.write_texture(
            wgpu::TexelCopyTextureInfo { texture: &texture, mip_level: level as u32, origin: wgpu::Origin3d::ZERO, aspect: wgpu::TextureAspect::All },
            &out,
            wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(bw * block), rows_per_image: Some(bh) },
            // (Whole blocks: a level smaller than one is its block's size.)
            wgpu::Extent3d { width: bw * 4, height: bh * 4, depth_or_array_layers: 1 },
        );
    }
    log::info!("texture {}x{} {:?}: compressed in {:.2} s", img.width, img.height, kind, started.elapsed().as_secs_f32());
    texture.create_view(&Default::default())
}
