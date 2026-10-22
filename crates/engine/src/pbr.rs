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
    primitive_centres: Arc<Vec<Vec3>>,
    bounds: [Vec3; 2],
    leases: Vec<Lease>,
}

// Opaque owner lease: deliberately excludes its contents from model diagnostics.
#[derive(Clone)]
struct Lease(Arc<dyn Send + Sync>);
impl std::fmt::Debug for Lease {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("Lease")
            .field(&Arc::strong_count(&self.0))
            .finish()
    }
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
        let radius = data
            .primitives
            .iter()
            .flat_map(|p| &p.vertices)
            .map(|v| Vec3::from(v.pos).length())
            .fold(0.0, f32::max);
        let mut bounds = [Vec3::splat(f32::INFINITY), Vec3::splat(f32::NEG_INFINITY)];
        for v in data.primitives.iter().flat_map(|p| &p.vertices) {
            let p = Vec3::from(v.pos);
            bounds[0] = bounds[0].min(p);
            bounds[1] = bounds[1].max(p);
        }
        let primitive_centres = Arc::new(data.primitives.iter().map(|p| {
            let mut lo = Vec3::splat(f32::INFINITY); let mut hi = Vec3::splat(f32::NEG_INFINITY);
            for v in &p.vertices { let v = Vec3::from_array(v.pos); lo = lo.min(v); hi = hi.max(v); }
            if p.vertices.is_empty() { Vec3::ZERO } else { (lo+hi)*0.5 }
        }).collect());
        PbrModel {
            primitive_centres,
            bounds,
            leases: Vec::new(),
            id: NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
            data: Arc::new(data),
            radius,
        }
    }

    /// Retain a cache owner through frame submission, background preparation and
    /// incremental upload. Attach to a temporary draw clone, never to the model
    /// stored inside that owner (which would create a reference cycle).
    pub fn with_lease(&self, owner: Arc<dyn Send + Sync>) -> Self {
        let mut model = self.clone();
        model.leases.push(Lease(owner));
        model
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
        let _profile = universe_prof::scope("assets/gltf decode");
        let (doc, buffers, images) = bounded_import(bytes)?;
        let mut data = PbrData {
            images,
            ..Default::default()
        };
        let tex = |t: Option<gltf::texture::Texture>| t.map(|t| t.source().index());
        for m in doc.materials() {
            let pbr = m.pbr_metallic_roughness();
            data.materials.push(Material {
                base_color: pbr.base_color_factor(),
                metallic: pbr.metallic_factor(),
                roughness: pbr.roughness_factor(),
                // (Times its strength, KHR_materials_emissive_strength: lamps far brighter than 1.)
                emissive: m
                    .emissive_factor()
                    .map(|c| c * m.emissive_strength().unwrap_or(1.0)),
                normal_scale: m.normal_texture().map_or(1.0, |n| n.scale()),
                base_tex: tex(pbr.base_color_texture().map(|i| i.texture())),
                mr_tex: tex(pbr.metallic_roughness_texture().map(|i| i.texture())),
                normal_tex: tex(m.normal_texture().map(|n| n.texture())),
                emissive_tex: tex(m.emissive_texture().map(|i| i.texture())),
                occlusion_tex: tex(m.occlusion_texture().map(|o| o.texture())),
                occlusion_strength: m.occlusion_texture().map_or(1.0, |o| o.strength()),
                alpha_cutoff: (m.alpha_mode() == gltf::material::AlphaMode::Mask)
                    .then(|| m.alpha_cutoff().unwrap_or(0.5)),
                blend: m.alpha_mode() == gltf::material::AlphaMode::Blend,
                double_sided: m.double_sided(),
            });
        }
        let fallback = data.materials.len();
        let scene = doc
            .default_scene()
            .or_else(|| doc.scenes().next())
            .ok_or("no scene")?;
        for node in scene.nodes() {
            walk(&node, glam::Mat4::IDENTITY, &buffers, parts, 0, &mut data);
        }
        // (A primitive with no material: a plain grey one.)
        if data.primitives.iter().any(|p| p.material == usize::MAX) {
            data.materials.push(Material {
                base_color: [0.6, 0.6, 0.6, 1.0],
                metallic: 0.0,
                roughness: 0.6,
                emissive: [0.0; 3],
                normal_scale: 1.0,
                base_tex: None,
                mr_tex: None,
                normal_tex: None,
                emissive_tex: None,
                occlusion_tex: None,
                occlusion_strength: 1.0,
                alpha_cutoff: None,
                blend: false,
                double_sided: false,
            });
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

fn walk(
    node: &gltf::Node,
    parent: glam::Mat4,
    buffers: &[gltf::buffer::Data],
    parts: &[&str],
    part: u8,
    data: &mut PbrData,
) {
    let m = parent * glam::Mat4::from_cols_array_2d(&node.transform().matrix());
    let part = node
        .name()
        .and_then(|n| parts.iter().position(|p| n.contains(p)))
        .map_or(part, |k| k as u8 + 1);
    let normal_m = glam::Mat3::from_mat4(m).inverse().transpose();
    // (Collision parts, `COL_*`, are the hull's shape for physics, not drawn.)
    let hidden = node.name().is_some_and(|n| n.starts_with("COL_"));
    if let Some(mesh) = node.mesh().filter(|_| !hidden) {
        for prim in mesh.primitives() {
            let r = prim.reader(|b| Some(&buffers[b.index()]));
            let Some(pos) = r.read_positions() else {
                continue;
            };
            let pos: Vec<Vec3> = pos.map(|p| m.transform_point3(Vec3::from(p))).collect();
            let n = pos.len();
            let normals: Vec<Vec3> = r.read_normals().map_or_else(
                || vec![Vec3::Y; n],
                |it| {
                    it.map(|v| (normal_m * Vec3::from(v)).normalize_or_zero())
                        .collect()
                },
            );
            let uvs: Vec<Vec2> = r.read_tex_coords(0).map_or_else(
                || vec![Vec2::ZERO; n],
                |it| it.into_f32().map(Vec2::from).collect(),
            );
            let indices: Vec<u32> = r
                .read_indices()
                .map_or_else(|| (0..n as u32).collect(), |it| it.into_u32().collect());
            let tangents: Vec<[f32; 4]> = match r.read_tangents() {
                Some(it) => it
                    .map(|t| {
                        let d = (glam::Mat3::from_mat4(m) * Vec3::new(t[0], t[1], t[2]))
                            .normalize_or_zero();
                        [d.x, d.y, d.z, t[3]]
                    })
                    .collect(),
                None => tangents(&pos, &normals, &uvs, &indices),
            };
            let vertices = (0..n)
                .map(|i| PbrVertex {
                    pos: pos[i].to_array(),
                    normal: normals[i].to_array(),
                    tangent: tangents[i],
                    uv: uvs[i].to_array(),
                })
                .collect();
            data.primitives.push(Primitive {
                vertices,
                indices,
                material: prim.material().index().unwrap_or(usize::MAX),
                part,
            });
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
            let tt = (t[i] - n * n.dot(t[i]))
                .try_normalize()
                .unwrap_or_else(|| n.any_orthonormal_vector());
            let w = if n.cross(tt).dot(b[i]) < 0.0 {
                -1.0
            } else {
                1.0
            };
            [tt.x, tt.y, tt.z, w]
        })
        .collect()
}

/// Embedded GLB only: bound input, accessor expansion and image dimensions
/// before decoding. Logical payload ceilings exclude allocator/driver overhead.
fn bounded_import(
    bytes: &[u8],
) -> Result<(gltf::Document, Vec<gltf::buffer::Data>, Vec<Image>), String> {
    const INPUT: usize = 64 * 1024 * 1024;
    const EXPANDED: usize = 128 * 1024 * 1024;
    if bytes.len() > INPUT {
        return Err("GLB exceeds 64 MiB input limit".into());
    }
    let gltf = gltf::Gltf::from_slice(bytes).map_err(|e| e.to_string())?;
    let blob = gltf.blob.ok_or("asset requires embedded GLB buffer")?;
    let doc = gltf.document;
    if doc.buffers().count() > 1 {
        return Err("GLB must use one embedded buffer".into());
    }
    if doc
        .buffers()
        .any(|b| !matches!(b.source(), gltf::buffer::Source::Bin) || b.length() > blob.len())
    {
        return Err("external or oversized GLB buffer".into());
    }
    let mut parents = std::collections::HashSet::new();
    for node in doc.nodes() {
        for child in node.children() {
            if !parents.insert(child.index()) {
                return Err("shared child in GLB hierarchy".into());
            }
        }
    }
    for scene in doc.scenes() {
        let mut pending: Vec<_> = scene.nodes().map(|n| (n, 0usize)).collect();
        let mut seen = std::collections::HashSet::new();
        while let Some((node, depth)) = pending.pop() {
            if depth > 128 || !seen.insert(node.index()) {
                return Err("cyclic, repeated or overdeep GLB hierarchy".into());
            }
            pending.extend(node.children().map(|n| (n, depth + 1)));
        }
    }
    let mut expanded = blob.len();
    // Count each scene instance conservatively; CPU faces add three positions
    // and normals per triangle on the shared consumer side.
    for node in doc.nodes() {
        if let Some(mesh) = node.mesh() {
            for p in mesh.primitives() {
                let n = p.get(&gltf::Semantic::Positions).map_or(0, |a| a.count());
                let indices = p.indices().map_or(n, |a| a.count());
                expanded = expanded
                    .checked_add(n.checked_mul(96).ok_or("geometry size overflow")?)
                    .and_then(|v| v.checked_add(indices.checked_mul(96)?))
                    .ok_or("geometry size overflow")?;
            }
        }
    }
    let mut image_ranges = Vec::new();
    for image in doc.images() {
        let gltf::image::Source::View { view, .. } = image.source() else {
            return Err("external/data URI image not supported by bounded GLB loader".into());
        };
        let end = view
            .offset()
            .checked_add(view.length())
            .ok_or("image range overflow")?;
        let data = blob
            .get(view.offset()..end)
            .ok_or("image outside GLB buffer")?;
        let reader = image::ImageReader::new(std::io::Cursor::new(data))
            .with_guessed_format()
            .map_err(|e| e.to_string())?;
        let (w, h) = reader.into_dimensions().map_err(|e| e.to_string())?;
        if w == 0 || h == 0 || w > 8192 || h > 8192 {
            return Err("image exceeds 8192 pixel dimension limit".into());
        }
        expanded = expanded
            .checked_add(
                (w as usize)
                    .checked_mul(h as usize)
                    .and_then(|n| n.checked_mul(16))
                    .ok_or("image size overflow")?,
            )
            .ok_or("asset size overflow")?;
        image_ranges.push((view.offset(), end, w, h));
    }
    if expanded > EXPANDED {
        return Err("asset exceeds 128 MiB expanded payload estimate".into());
    }
    let mut images = Vec::new();
    for (start, end, w, h) in image_ranges {
        let mut reader = image::ImageReader::new(std::io::Cursor::new(&blob[start..end]))
            .with_guessed_format()
            .map_err(|e| e.to_string())?;
        let mut limits = image::Limits::default();
        limits.max_image_width = Some(8192);
        limits.max_image_height = Some(8192);
        limits.max_alloc = Some(EXPANDED as u64);
        reader.limits(limits);
        let rgba = reader
            .decode()
            .map_err(|e| e.to_string())?
            .into_rgba8()
            .into_raw();
        images.push(Image {
            width: w,
            height: h,
            rgba,
        });
    }
    Ok((doc, vec![gltf::buffer::Data(blob)], images))
}

/// The mip chain of an RGBA8 image: each level half the last, box filtered
/// (in linear light for colour textures: `srgb`).
pub(crate) fn mips(img: &Image, srgb: bool) -> Vec<(u32, u32, Vec<u8>)> {
    let to_lin = |c: u8| {
        if srgb {
            (c as f32 / 255.0).powf(2.2)
        } else {
            c as f32 / 255.0
        }
    };
    let from_lin = |v: f32| {
        ((if srgb { v.max(0.0).powf(1.0 / 2.2) } else { v }) * 255.0 + 0.5).clamp(0.0, 255.0) as u8
    };
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
                    next[i] = if c == 3 {
                        (sum / 4.0 * 255.0 + 0.5) as u8
                    } else {
                        from_lin(sum / 4.0)
                    };
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
    used: std::time::Instant,
    bytes: usize,
    placeholder: bool,
}

/// The renderer's side: the pipelines, models on the GPU, this frame's draws.
pub(crate) struct PbrRenderer {
    /// One for each kind of surface (`PIPES`), in the order they're drawn.
    pipes: Vec<wgpu::RenderPipeline>,
    shadow_pipe: wgpu::RenderPipeline,
    material_layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    models: HashMap<u64, GpuModel>,
    prepared: crate::assets::AssetCache<PreparedTextures>,
    pending: HashMap<u64, ModelUpload>,
    retired: Vec<(Arc<std::sync::atomic::AtomicBool>, GpuModel)>,
    retired_pending: Vec<(Arc<std::sync::atomic::AtomicBool>, ModelUpload)>,
    uploads: u64,
    hits: u64,
    idle_for: std::time::Duration,
    evictions: u64,
    instances: wgpu::Buffer,
    capacity: u64,
    /// This frame: (model, its instance's index, the part drawn), and those that cast.
    draws: Vec<(u64, u32, u8)>,
    casters: Vec<(u64, u32, u8)>,
    blended: Vec<(u64, u32, usize, f32)>,
    frames: u64,
    upload_spent: std::time::Duration,
    pub(crate) view_demand: std::collections::HashSet<u64>,
}

const VERTEX_ATTRS: [wgpu::VertexAttribute; 4] =
    wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x3, 2 => Float32x4, 3 => Float32x2];
/// The instance fields the shader uses (by their place in `frame::Instance`):
/// its turn and place, the sun's direction and light, the planet's.
const INSTANCE_ATTRS: [wgpu::VertexAttribute; 8] = [
    wgpu::VertexAttribute {
        format: wgpu::VertexFormat::Float32x4,
        offset: 0,
        shader_location: 4,
    },
    wgpu::VertexAttribute {
        format: wgpu::VertexFormat::Float32x4,
        offset: 16,
        shader_location: 5,
    },
    wgpu::VertexAttribute {
        format: wgpu::VertexFormat::Float32x4,
        offset: 32,
        shader_location: 6,
    },
    wgpu::VertexAttribute {
        format: wgpu::VertexFormat::Float32x4,
        offset: 48,
        shader_location: 7,
    },
    wgpu::VertexAttribute {
        format: wgpu::VertexFormat::Float32x4,
        offset: 96,
        shader_location: 8,
    },
    wgpu::VertexAttribute {
        format: wgpu::VertexFormat::Float32x4,
        offset: 112,
        shader_location: 9,
    },
    wgpu::VertexAttribute {
        format: wgpu::VertexFormat::Float32x4,
        offset: 128,
        shader_location: 10,
    },
    wgpu::VertexAttribute {
        format: wgpu::VertexFormat::Float32x4,
        offset: 144,
        shader_location: 11,
    },
];
/// The pipelines, by kind of surface: solid (one side, two), see-through (one, two).
fn pipe_for(blend: bool, double_sided: bool) -> usize {
    usize::from(blend) * SEE_THROUGH + usize::from(double_sided)
}
/// Where the see-through pipelines start.
const SEE_THROUGH: usize = 2;

impl PbrRenderer {
    fn retire(&mut self, queue: &wgpu::Queue, model: GpuModel) {
        let done = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let flag = done.clone();
        queue
            .on_submitted_work_done(move || flag.store(true, std::sync::atomic::Ordering::Release));
        self.retired.push((done, model));
    }
    pub fn hits(&self) -> u64 {
        self.hits
    }
    pub fn uploads(&self) -> u64 {
        self.uploads
    }
    pub fn evictions(&self) -> u64 {
        self.evictions
    }
    pub fn resident_bytes(&self) -> usize {
        self.models.values().map(|m| m.bytes).sum::<usize>()
            + self.pending.values().map(|p| p.bytes).sum::<usize>()
            + self.retired.iter().map(|(_, m)| m.bytes).sum::<usize>()
            + self
                .retired_pending
                .iter()
                .map(|(_, m)| m.bytes)
                .sum::<usize>()
    }
    pub fn preparing(&self) -> usize {
        self.prepared.pending_count() + self.pending.len()
    }
    /// Models uploaded.
    pub fn model_count(&self) -> usize {
        self.models.len()
    }

    pub(crate) fn new(
        device: &wgpu::Device,
        globals: &wgpu::BindGroupLayout,
        shadows: &wgpu::BindGroupLayout,
        light: &wgpu::BindGroupLayout,
        scene_format: wgpu::TextureFormat,
        depth_format: wgpu::TextureFormat,
        samples: u32,
    ) -> Self {
        let shader = crate::shaders::make(device, "pbr", crate::shaders::pbr());
        let tex_entry = |binding| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Texture {
                sample_type: wgpu::TextureSampleType::Float { filterable: true },
                view_dimension: wgpu::TextureViewDimension::D2,
                multisampled: false,
            },
            count: None,
        };
        let material_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("pbr material"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                tex_entry(1),
                tex_entry(2),
                tex_entry(3),
                tex_entry(4),
                wgpu::BindGroupLayoutEntry {
                    binding: 5,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                tex_entry(6),
            ],
        });
        let buffers = [
            Some(wgpu::VertexBufferLayout {
                array_stride: size_of::<PbrVertex>() as u64,
                step_mode: wgpu::VertexStepMode::Vertex,
                attributes: &VERTEX_ATTRS,
            }),
            Some(wgpu::VertexBufferLayout {
                array_stride: size_of::<crate::frame::Instance>() as u64,
                step_mode: wgpu::VertexStepMode::Instance,
                attributes: &INSTANCE_ATTRS,
            }),
        ];
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("pbr"),
            bind_group_layouts: &[Some(globals), Some(shadows), Some(&material_layout)],
            immediate_size: 0,
        });
        // (Glass: its colour already weighed by its alpha in the shader, what it mirrors
        // not; what's behind shows through by the rest. It doesn't hide what's behind it.)
        let make = |label, see_through: bool, two_sided: bool| {
            let blend = see_through.then_some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING);
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(label),
                layout: Some(&layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some("vs_pbr"),
                    compilation_options: Default::default(),
                    buffers: &buffers,
                },
                primitive: wgpu::PrimitiveState {
                    topology: wgpu::PrimitiveTopology::TriangleList,
                    cull_mode: (!two_sided).then_some(wgpu::Face::Back),
                    ..Default::default()
                },
                depth_stencil: Some(wgpu::DepthStencilState {
                    format: depth_format,
                    depth_write_enabled: Some(!see_through),
                    depth_compare: Some(wgpu::CompareFunction::Greater),
                    stencil: Default::default(),
                    bias: Default::default(),
                }),
                multisample: wgpu::MultisampleState {
                    count: samples,
                    ..Default::default()
                },
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: Some("fs_pbr"),
                    compilation_options: Default::default(),
                    targets: &[Some(wgpu::ColorTargetState {
                        format: scene_format,
                        blend,
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                }),
                multiview_mask: None,
                cache: None,
            })
        };
        let pipes = vec![
            make("pbr", false, false),
            make("pbr two-sided", false, true),
            make("pbr see-through", true, false),
            make("pbr see-through two-sided", true, true),
        ];
        let shadow_shader = crate::shaders::single(device, "pbr_shadow");
        let shadow_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("pbr shadow"),
            bind_group_layouts: &[Some(light), Some(&material_layout)],
            immediate_size: 0,
        });
        let shadow_pipe = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("pbr shadow casters"),
            layout: Some(&shadow_layout),
            vertex: wgpu::VertexState {
                module: &shadow_shader,
                entry_point: Some("vs_pbr_shadow"),
                compilation_options: Default::default(),
                buffers: &buffers,
            },
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: depth_format,
                depth_write_enabled: Some(true),
                depth_compare: Some(wgpu::CompareFunction::LessEqual),
                stencil: Default::default(),
                bias: wgpu::DepthBiasState {
                    constant: 2,
                    slope_scale: 2.0,
                    clamp: 0.0,
                },
            }),
            multisample: Default::default(),
            fragment: Some(wgpu::FragmentState {
                module: &shadow_shader, entry_point: Some("fs_pbr_shadow"),
                compilation_options: Default::default(), targets: &[],
            }),
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
        let instances = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("pbr instances"),
            size: 64 * size_of::<crate::frame::Instance>() as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        PbrRenderer {
            pipes,
            shadow_pipe,
            material_layout,
            sampler,
            models: HashMap::new(),
            prepared: crate::assets::AssetCache::new(Default::default()),
            pending: HashMap::new(),
            retired: Vec::new(),
            retired_pending: Vec::new(),
            uploads: 0,
            hits: 0,
            idle_for: std::time::Duration::from_millis(
                std::env::var("UNIVERSE_ASSET_GPU_IDLE_MS")
                    .ok()
                    .and_then(|s| s.parse::<u64>().ok())
                    .unwrap_or(30_000)
                    .clamp(1, 300_000),
            ),
            evictions: 0,
            instances,
            capacity: 64,
            draws: Vec::new(),
            casters: Vec::new(),
            blended: Vec::new(),
            frames: 0,
            upload_spent: std::time::Duration::ZERO,
            view_demand: Default::default(),
        }
    }

    pub(crate) fn begin_frame(&mut self) {
        self.upload_spent = std::time::Duration::ZERO;
    }

    /// This frame's models up (new ones uploaded, old ones dropped), their
    /// instances, and which cast shadows within `reach` of the eye.
    pub(crate) fn upload(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        draws: &[crate::frame::PbrDraw],
        reach: f32,
        forward: Vec3,
        budget: &mut usize,
    ) {
        self.frames += 1;
        let now = std::time::Instant::now();
        let frame_start = now;
        let mut upload_bytes = 0;

        // Visible resources cannot be retired under pressure while this frame uses them.
        let visible: std::collections::HashSet<_> = draws.iter().map(|d| d.model.id()).chain(self.view_demand.iter().copied()).collect();
        self.retired.retain(|(done, m)| {
            if done.load(std::sync::atomic::Ordering::Acquire) {
                if !m.placeholder {
                    self.evictions += 1;
                }
                false
            } else {
                true
            }
        });
        self.retired_pending
            .retain(|(done, _)| !done.load(std::sync::atomic::Ordering::Acquire));
        let stale: Vec<_> = self
            .pending
            .iter_mut()
            .filter_map(|(id, p)| {
                if visible.contains(id) {
                    p.last_needed = now;
                }
                (now.duration_since(p.last_needed) >= std::time::Duration::from_secs(2))
                    .then_some(*id)
            })
            .collect();
        for id in stale {
            let p = self.pending.remove(&id).unwrap();
            let done = Arc::new(std::sync::atomic::AtomicBool::new(false));
            let flag = done.clone();
            queue.on_submitted_work_done(move || {
                flag.store(true, std::sync::atomic::Ordering::Release)
            });
            self.retired_pending.push((done, p));
        }
        let importance = |id: &u64| {
            draws
                .iter()
                .filter(|d| d.model.id() == *id)
                .map(|d| {
                    let distance = glam::Vec3::from_slice(&d.instance.t[..3]).length().max(1.);
                    (d.reach / distance).max(0.001) as f64
                })
                .fold(0.001, f64::max)
        };
        // (Each upload's place worked out once: unseen last, then the longest unserved, then bytes per importance.)
        let mut keyed: Vec<(bool, bool, f64, u64)> = self
            .pending
            .iter()
            .map(|(&id, p)| (!visible.contains(&id), now.duration_since(p.last_served) <= std::time::Duration::from_secs(1), p.bytes as f64 / importance(&id), id))
            .collect();
        keyed.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(&b.1)).then(a.2.total_cmp(&b.2)).then(a.3.cmp(&b.3)));
        for id in keyed.into_iter().map(|k| k.3) {
            let job = self.pending.get_mut(&id).unwrap();
            while upload_bytes < *budget
                && self.upload_spent + frame_start.elapsed() < std::time::Duration::from_millis(2)
                && !job.tasks.is_empty()
            {
                let sent = job.transfer(queue, (*budget - upload_bytes).min(1024 * 1024));
                if sent == 0 {
                    break;
                }
                upload_bytes += sent;
                job.last_served = now;
            }
            if job.tasks.is_empty() {
                let job = self.pending.remove(&id).unwrap();
                let model = self.gpu_model(device, queue, &job.model, &job.prepared, Some(&job));
                if let Some(old) = self.models.insert(id, model) {
                    self.retire(queue, old);
                }
                self.uploads += 1;
            }
        }
        let expired: Vec<_> = self
            .models
            .iter()
            .filter(|(id, m)| !visible.contains(id) && now.duration_since(m.used) >= self.idle_for)
            .map(|(id, _)| *id)
            .collect();
        for id in expired {
            let old = self.models.remove(&id).unwrap();
            self.retire(queue, old);
        }
        let mut requested = std::collections::HashSet::new();
        for d in draws {
            if let Some(m) = self.models.get_mut(&d.model.id()) {
                m.used = now;
                if !m.placeholder {
                    self.hits += 1;
                    continue;
                }
            }
            if !requested.insert(d.model.id()) {
                continue;
            }
            if !self.models.contains_key(&d.model.id()) {
                let proxy = placeholder(&d.model);
                let mut gpu = self.gpu_model(
                    device,
                    queue,
                    &proxy,
                    &PreparedTextures {
                        textures: HashMap::new(),
                    },
                    None,
                );
                gpu.placeholder = true;
                self.models.insert(d.model.id(), gpu);
            }
            if self.pending.contains_key(&d.model.id()) {
                continue;
            }
            let model = d.model.clone();
            let estimate = model
                .data
                .images
                .iter()
                .map(|i| i.rgba.len())
                .sum::<usize>()
                .saturating_mul(12)
                .saturating_add(4096);
            let Some(prepared) = self.prepared.request(
                format!("pbr-textures:{}", model.id()),
                crate::assets::Demand {
                    estimated_bytes: estimate,
                    ..Default::default()
                },
                move || {
                    let value = prepare_textures(&model.data);
                    let bytes = value.bytes();
                    Ok(crate::assets::Loaded { value, bytes })
                },
            ) else {
                continue;
            };
            let bytes = model_bytes(&d.model.data)
                + prepared.gpu_bytes(
                    device
                        .features()
                        .contains(wgpu::Features::TEXTURE_COMPRESSION_BC),
                );
            if self.upload_spent + frame_start.elapsed() > std::time::Duration::from_millis(2) {
                continue;
            }
            let resident = self.resident_bytes();
            if resident.saturating_add(bytes) > 512 * 1024 * 1024 {
                let mut cold: Vec<_> = self
                    .models
                    .iter()
                    .filter(|(id, _)| !visible.contains(id))
                    .map(|(id, m)| (*id, m.used, m.bytes))
                    .collect();
                cold.sort_by_key(|c| c.1);
                let mut used = resident;
                for (id, _, n) in cold {
                    if used.saturating_add(bytes) <= 512 * 1024 * 1024 {
                        break;
                    }
                    let old = self.models.remove(&id).unwrap();
                    self.retire(queue, old);
                    used -= n;
                }
                if self.resident_bytes().saturating_add(bytes) > 512 * 1024 * 1024 {
                    continue;
                }
            }
            self.pending.insert(
                d.model.id(),
                ModelUpload::new(device, d.model.clone(), prepared),
            );
        }
        self.prepared.maintain();
        self.upload_spent += frame_start.elapsed();
        *budget = budget.saturating_sub(upload_bytes);
        let data: Vec<crate::frame::Instance> = draws.iter().map(|d| d.instance).collect();
        if data.len() as u64 > self.capacity {
            self.capacity = (data.len() as u64).next_power_of_two();
            self.instances = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("pbr instances"),
                size: self.capacity * size_of::<crate::frame::Instance>() as u64,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
        }
        if !data.is_empty() {
            queue.write_buffer(&self.instances, 0, bytemuck::cast_slice(&data));
        }
        self.draws = draws
            .iter()
            .enumerate()
            .map(|(i, d)| (d.model.id(), i as u32, d.part))
            .collect();
        self.blended.clear();
        for (k, d) in draws.iter().enumerate() {
            for (pi, p) in d.model.data.primitives.iter().enumerate() {
                if p.part != d.part || !d.model.data.materials[p.material].blend { continue; }
                // Primitive-centre sorting is shared across models/material pipelines.
                // Intersecting transparent surfaces still require a different algorithm.
                let c = d.model.primitive_centres[pi];
                let inst = &d.instance;
                let at = Vec3::from_slice(&inst.c0[..3])*c.x + Vec3::from_slice(&inst.c1[..3])*c.y
                    + Vec3::from_slice(&inst.c2[..3])*c.z + Vec3::from_slice(&inst.t[..3]);
                self.blended.push((d.model.id(), k as u32, pi, at.dot(forward)));
            }
        }
        self.blended.sort_by(|a,b| b.3.total_cmp(&a.3));
        self.casters = draws
            .iter()
            .enumerate()
            .filter(|(_, d)| {
                d.casts && Vec3::from_slice(&d.instance.t[..3]).length() - d.reach < reach
            })
            .map(|(i, d)| (d.model.id(), i as u32, d.part))
            .collect();
    }

    fn gpu_model(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        model: &PbrModel,
        prepared: &PreparedTextures,
        uploaded: Option<&ModelUpload>,
    ) -> GpuModel {
        use wgpu::util::DeviceExt;
        let data = &model.data;
        // Each image as the textures its uses need: colour ones in sRGB, the rest linear.
        let mut textures: HashMap<(usize, Kind), wgpu::TextureView> = HashMap::new();
        let mut texture = |i: usize, kind: Kind| -> wgpu::TextureView {
            textures
                .entry((i, kind))
                .or_insert_with(|| {
                    uploaded
                        .expect("authored textures uploaded incrementally")
                        .textures[&(i, kind)]
                        .create_view(&Default::default())
                })
                .clone()
        };
        let plain = |px: [u8; 4], kind: Kind| {
            upload_texture(
                device,
                queue,
                &Image {
                    width: 1,
                    height: 1,
                    rgba: px.to_vec(),
                },
                kind,
            )
        };
        let (white_srgb, white, flat) = (
            plain([255; 4], Kind::Colour),
            plain([255; 4], Kind::Data),
            plain([128, 128, 255, 255], Kind::Normal),
        );
        let materials = data
            .materials
            .iter()
            .map(|m| {
                let uniform = MaterialUniform {
                    base_color: m.base_color,
                    params: [
                        m.metallic,
                        m.roughness,
                        m.normal_scale,
                        m.alpha_cutoff.unwrap_or(0.0),
                    ],
                    emissive: [m.emissive[0], m.emissive[1], m.emissive[2], 0.0],
                    extra: [
                        m.occlusion_strength,
                        if m.blend { 1.0 } else { 0.0 },
                        0.0,
                        0.0,
                    ],
                };
                let buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("pbr material"),
                    contents: bytemuck::bytes_of(&uniform),
                    usage: wgpu::BufferUsages::UNIFORM,
                });
                let base = m
                    .base_tex
                    .map_or_else(|| white_srgb.clone(), |i| texture(i, Kind::Colour));
                let mr = m
                    .mr_tex
                    .map_or_else(|| white.clone(), |i| texture(i, Kind::Data));
                let normal = m
                    .normal_tex
                    .map_or_else(|| flat.clone(), |i| texture(i, Kind::Normal));
                // (No texture: the factor alone, as glTF has it. Black here put out every untextured lamp.)
                let emissive = m
                    .emissive_tex
                    .map_or_else(|| white_srgb.clone(), |i| texture(i, Kind::Colour));
                let occlusion = m
                    .occlusion_tex
                    .map_or_else(|| white.clone(), |i| texture(i, Kind::Data));
                device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("pbr material"),
                    layout: &self.material_layout,
                    entries: &[
                        wgpu::BindGroupEntry {
                            binding: 0,
                            resource: buffer.as_entire_binding(),
                        },
                        wgpu::BindGroupEntry {
                            binding: 1,
                            resource: wgpu::BindingResource::TextureView(&base),
                        },
                        wgpu::BindGroupEntry {
                            binding: 2,
                            resource: wgpu::BindingResource::TextureView(&mr),
                        },
                        wgpu::BindGroupEntry {
                            binding: 3,
                            resource: wgpu::BindingResource::TextureView(&normal),
                        },
                        wgpu::BindGroupEntry {
                            binding: 4,
                            resource: wgpu::BindingResource::TextureView(&emissive),
                        },
                        wgpu::BindGroupEntry {
                            binding: 5,
                            resource: wgpu::BindingResource::Sampler(&self.sampler),
                        },
                        wgpu::BindGroupEntry {
                            binding: 6,
                            resource: wgpu::BindingResource::TextureView(&occlusion),
                        },
                    ],
                })
            })
            .collect();
        let primitives = data
            .primitives
            .iter()
            .enumerate()
            .map(|(index, p)| {
                let v = if let Some(u) = uploaded {
                    u.buffers[index].0.clone()
                } else {
                    device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                        label: Some("pbr vertices"),
                        contents: bytemuck::cast_slice(&p.vertices),
                        usage: wgpu::BufferUsages::VERTEX,
                    })
                };
                let i = if let Some(u) = uploaded {
                    u.buffers[index].1.clone()
                } else {
                    device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                        label: Some("pbr indices"),
                        contents: bytemuck::cast_slice(&p.indices),
                        usage: wgpu::BufferUsages::INDEX,
                    })
                };
                let m = &data.materials[p.material];
                (
                    v,
                    i,
                    p.indices.len() as u32,
                    p.material,
                    pipe_for(m.blend, m.double_sided),
                    p.part,
                )
            })
            .collect();
        GpuModel {
            primitives,
            materials,
            used: std::time::Instant::now(),
            bytes: model_bytes(data)
                + prepared.gpu_bytes(
                    device
                        .features()
                        .contains(wgpu::Features::TEXTURE_COMPRESSION_BC),
                ),
            placeholder: false,
        }
    }

    /// The models in the scene pass (its globals and shadows bound already).
    pub(crate) fn draw<'a>(&'a self, pass: &mut wgpu::RenderPass<'a>) {
        if self.draws.is_empty() {
            return;
        }
        pass.set_vertex_buffer(1, self.instances.slice(..));
        // (What's solid first, then the see-through over it.)
        for (kind, pipe) in self.pipes.iter().enumerate().take(SEE_THROUGH) {
            pass.set_pipeline(pipe);
            for &(id, k, part) in &self.draws {
                let Some(m) = self.models.get(&id) else {
                    continue;
                };
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
        for &(id, k, primitive, _) in &self.blended {
            let Some(m) = self.models.get(&id) else { continue; };
            // A pending model can still have placeholder topology.
            if m.placeholder { continue; }
            let (v, i, n, mat, kind, _) = &m.primitives[primitive];
            pass.set_pipeline(&self.pipes[*kind]);
            pass.set_bind_group(2, &m.materials[*mat], &[]);
            pass.set_vertex_buffer(0, v.slice(..));
            pass.set_index_buffer(i.slice(..), wgpu::IndexFormat::Uint32);
            pass.draw_indexed(0..*n, 0, k..k+1);
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
            let Some(m) = self.models.get(&id) else {
                continue;
            };
            // (Glass lets the sun through.)
            for (v, i, n, mat, _, _) in m
                .primitives
                .iter()
                .filter(|p| p.4 < SEE_THROUGH && p.5 == part)
            {
                pass.set_bind_group(1, &m.materials[*mat], &[]);
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

fn model_bytes(data: &PbrData) -> usize {
    data.primitives
        .iter()
        .map(|p| p.vertices.len() * size_of::<PbrVertex>() + p.indices.len() * 4)
        .sum()
}
struct PreparedTexture {
    width: u32,
    height: u32,
    levels: Vec<(u32, u32, Vec<u8>)>,
    compressed: Option<(wgpu::TextureFormat, Vec<Vec<u8>>, u32)>,
}
struct PreparedTextures {
    textures: HashMap<(usize, Kind), PreparedTexture>,
}
impl PreparedTextures {
    fn gpu_bytes(&self, bc: bool) -> usize {
        self.textures
            .values()
            .map(|t| {
                if bc {
                    t.compressed
                        .as_ref()
                        .map(|(_, levels, _)| levels.iter().map(Vec::len).sum::<usize>())
                } else {
                    None
                }
                .unwrap_or_else(|| t.levels.iter().map(|l| l.2.len()).sum())
            })
            .sum()
    }
    fn bytes(&self) -> usize {
        self.textures
            .values()
            .map(|t| {
                t.levels.iter().map(|l| l.2.len()).sum::<usize>()
                    + t.compressed
                        .as_ref()
                        .map_or(0, |(_, ls, _)| ls.iter().map(Vec::len).sum())
            })
            .sum()
    }
}
fn prepare_texture(img: &Image, kind: Kind) -> PreparedTexture {
    let _p = universe_prof::scope("assets/texture prepare");
    let levels = mips(img, kind == Kind::Colour);
    let compressed =
        if img.width % 4 == 0 && img.height % 4 == 0 && img.width >= 64 && img.height >= 64 {
            let (codec, format, block) = match kind {
                Kind::Colour => (
                    texpresso::Format::Bc1,
                    wgpu::TextureFormat::Bc1RgbaUnormSrgb,
                    8,
                ),
                Kind::Data => (texpresso::Format::Bc1, wgpu::TextureFormat::Bc1RgbaUnorm, 8),
                Kind::Normal => (texpresso::Format::Bc5, wgpu::TextureFormat::Bc5RgUnorm, 16),
            };
            let algorithm = if kind == Kind::Data {
                texpresso::Algorithm::RangeFit
            } else {
                texpresso::Algorithm::ClusterFit
            };
            let encoded = levels
                .iter()
                .map(|(w, h, px)| {
                    let mut out = vec![0; codec.compressed_size(*w as usize, *h as usize)];
                    compression_pool().install(|| {
                        codec.compress(
                            px,
                            *w as usize,
                            *h as usize,
                            texpresso::Params {
                                algorithm,
                                ..Default::default()
                            },
                            &mut out,
                        )
                    });
                    out
                })
                .collect();
            Some((format, encoded, block))
        } else {
            None
        };
    PreparedTexture {
        width: img.width,
        height: img.height,
        levels,
        compressed,
    }
}
fn prepare_textures(data: &PbrData) -> PreparedTextures {
    let mut textures = HashMap::new();
    for m in &data.materials {
        for (index, kind) in [
            (m.base_tex, Kind::Colour),
            (m.mr_tex, Kind::Data),
            (m.normal_tex, Kind::Normal),
            (m.emissive_tex, Kind::Colour),
            (m.occlusion_tex, Kind::Data),
        ] {
            if let Some(i) = index {
                textures
                    .entry((i, kind))
                    .or_insert_with(|| prepare_texture(&data.images[i], kind));
            }
        }
    }
    PreparedTextures { textures }
}
fn upload_texture(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    img: &Image,
    kind: Kind,
) -> wgpu::TextureView {
    debug_assert_eq!(
        (img.width, img.height),
        (1, 1),
        "authored textures must be worker-prepared"
    );
    upload_prepared(device, queue, &prepare_texture(img, kind), kind)
}
fn upload_prepared(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    p: &PreparedTexture,
    kind: Kind,
) -> wgpu::TextureView {
    let compressed = p.compressed.as_ref().filter(|_| {
        device
            .features()
            .contains(wgpu::Features::TEXTURE_COMPRESSION_BC)
    });
    let format = compressed.map_or(
        if kind == Kind::Colour {
            wgpu::TextureFormat::Rgba8UnormSrgb
        } else {
            wgpu::TextureFormat::Rgba8Unorm
        },
        |c| c.0,
    );
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("prepared pbr texture"),
        size: wgpu::Extent3d {
            width: p.width,
            height: p.height,
            depth_or_array_layers: 1,
        },
        mip_level_count: p.levels.len() as u32,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    for (level, (w, h, rgba)) in p.levels.iter().enumerate() {
        let (bytes, row, rows, width, height) = if let Some((_, data, block)) = compressed {
            (
                &data[level],
                w.div_ceil(4) * block,
                h.div_ceil(4),
                w.div_ceil(4) * 4,
                h.div_ceil(4) * 4,
            )
        } else {
            (rgba, w * 4, *h, *w, *h)
        };
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: level as u32,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            bytes,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(row),
                rows_per_image: Some(rows),
            },
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );
    }
    texture.create_view(&Default::default())
}

fn compression_pool() -> &'static rayon::ThreadPool {
    static POOL: std::sync::OnceLock<rayon::ThreadPool> = std::sync::OnceLock::new();
    POOL.get_or_init(|| {
        rayon::ThreadPoolBuilder::new()
            .num_threads(2)
            .thread_name(|i| format!("asset compression {i}"))
            .build()
            .expect("texture preparation pool")
    })
}

/// Cheap neutral envelope while authored geometry/textures are becoming ready.
fn placeholder(model: &PbrModel) -> PbrModel {
    let [lo, hi] = model.bounds;
    let lo = if lo.is_finite() {
        lo
    } else {
        Vec3::splat(-0.5)
    };
    let hi = if hi.is_finite() { hi } else { Vec3::splat(0.5) };
    let vertices = (0..8)
        .map(|i| {
            let p = Vec3::new(
                if i & 1 == 0 { lo.x } else { hi.x },
                if i & 2 == 0 { lo.y } else { hi.y },
                if i & 4 == 0 { lo.z } else { hi.z },
            );
            PbrVertex {
                pos: p.to_array(),
                normal: (p - (lo + hi) * 0.5).normalize_or_zero().to_array(),
                tangent: [1., 0., 0., 1.],
                uv: [0.; 2],
            }
        })
        .collect();
    let indices = vec![
        0, 2, 1, 1, 2, 3, 4, 5, 6, 5, 7, 6, 0, 1, 4, 1, 5, 4, 2, 6, 3, 3, 6, 7, 0, 4, 2, 2, 4, 6,
        1, 3, 5, 3, 7, 5,
    ];
    let material = Material {
        base_color: [0.35, 0.38, 0.4, 1.],
        metallic: 0.,
        roughness: 1.,
        emissive: [0.; 3],
        normal_scale: 1.,
        base_tex: None,
        mr_tex: None,
        normal_tex: None,
        emissive_tex: None,
        occlusion_tex: None,
        occlusion_strength: 1.,
        alpha_cutoff: None,
        blend: false,
        double_sided: true,
    };
    PbrModel::new(PbrData {
        primitives: vec![Primitive {
            vertices,
            indices,
            material: 0,
            part: 0,
        }],
        materials: vec![material],
        images: vec![],
    })
}

// Resource allocation is admitted against the residency budget. Payload copies
// are split across frames; the visible envelope survives until atomic publish.
enum Transfer {
    Vertex(usize, usize),
    Index(usize, usize),
    Texture((usize, Kind), usize, u32),
}
struct ModelUpload {
    model: PbrModel,
    prepared: Arc<PreparedTextures>,
    buffers: Vec<(wgpu::Buffer, wgpu::Buffer)>,
    textures: HashMap<(usize, Kind), wgpu::Texture>,
    tasks: std::collections::VecDeque<Transfer>,
    bytes: usize,
    bc: bool,
    last_needed: std::time::Instant,
    last_served: std::time::Instant,
}
impl ModelUpload {
    fn new(device: &wgpu::Device, model: PbrModel, prepared: Arc<PreparedTextures>) -> Self {
        let mut tasks = std::collections::VecDeque::new();
        let buffers = model
            .data
            .primitives
            .iter()
            .enumerate()
            .map(|(i, p)| {
                tasks.push_back(Transfer::Vertex(i, 0));
                tasks.push_back(Transfer::Index(i, 0));
                let make = |label, size, usage| {
                    device.create_buffer(&wgpu::BufferDescriptor {
                        label: Some(label),
                        size,
                        usage: usage | wgpu::BufferUsages::COPY_DST,
                        mapped_at_creation: false,
                    })
                };
                (
                    make(
                        "stream vertex",
                        (p.vertices.len() * size_of::<PbrVertex>()) as u64,
                        wgpu::BufferUsages::VERTEX,
                    ),
                    make(
                        "stream index",
                        (p.indices.len() * 4) as u64,
                        wgpu::BufferUsages::INDEX,
                    ),
                )
            })
            .collect();
        let bc = device
            .features()
            .contains(wgpu::Features::TEXTURE_COMPRESSION_BC);
        let textures = prepared
            .textures
            .iter()
            .map(|(&key, p)| {
                let format = if bc {
                    p.compressed.as_ref().map(|c| c.0)
                } else {
                    None
                }
                .unwrap_or(if key.1 == Kind::Colour {
                    wgpu::TextureFormat::Rgba8UnormSrgb
                } else {
                    wgpu::TextureFormat::Rgba8Unorm
                });
                let t = device.create_texture(&wgpu::TextureDescriptor {
                    label: Some("stream texture"),
                    size: wgpu::Extent3d {
                        width: p.width,
                        height: p.height,
                        depth_or_array_layers: 1,
                    },
                    mip_level_count: p.levels.len() as u32,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format,
                    usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                    view_formats: &[],
                });
                for level in 0..p.levels.len() {
                    tasks.push_back(Transfer::Texture(key, level, 0));
                }
                (key, t)
            })
            .collect();
        let bytes = model_bytes(&model.data) + prepared.gpu_bytes(bc);
        Self {
            model,
            prepared,
            buffers,
            textures,
            tasks,
            bytes,
            bc,
            last_needed: std::time::Instant::now(),
            last_served: std::time::Instant::now(),
        }
    }
    fn transfer(&mut self, queue: &wgpu::Queue, budget: usize) -> usize {
        let _scope = universe_prof::scope("assets/gpu transfer");
        let Some(task) = self.tasks.pop_front() else {
            return 0;
        };
        match task {
            Transfer::Vertex(i, offset) | Transfer::Index(i, offset) => {
                let vertex = matches!(task, Transfer::Vertex(..));
                let p = &self.model.data.primitives[i];
                let bytes: &[u8] = if vertex {
                    bytemuck::cast_slice(&p.vertices)
                } else {
                    bytemuck::cast_slice(&p.indices)
                };
                let n = (bytes.len() - offset).min(budget / 4 * 4);
                if n > 0 {
                    queue.write_buffer(
                        if vertex {
                            &self.buffers[i].0
                        } else {
                            &self.buffers[i].1
                        },
                        offset as u64,
                        &bytes[offset..offset + n],
                    );
                }
                if offset + n < bytes.len() {
                    self.tasks.push_front(if vertex {
                        Transfer::Vertex(i, offset + n)
                    } else {
                        Transfer::Index(i, offset + n)
                    });
                }
                n
            }
            Transfer::Texture(key, level, row) => {
                let p = &self.prepared.textures[&key];
                let (w, h, rgba) = &p.levels[level];
                let compressed = p.compressed.as_ref().filter(|_| self.bc);
                let (bytes, stride, rows, block, width) =
                    if let Some((_, levels, size)) = compressed {
                        (
                            &levels[level],
                            w.div_ceil(4) * size,
                            h.div_ceil(4),
                            4,
                            w.div_ceil(4) * 4,
                        )
                    } else {
                        (rgba, w * 4, *h, 1, *w)
                    };
                let take = (rows - row).min((budget / stride as usize) as u32);
                if take == 0 {
                    self.tasks.push_front(Transfer::Texture(key, level, row));
                    return 0;
                }
                queue.write_texture(
                    wgpu::TexelCopyTextureInfo {
                        texture: &self.textures[&key],
                        mip_level: level as u32,
                        origin: wgpu::Origin3d {
                            x: 0,
                            y: row * block,
                            z: 0,
                        },
                        aspect: wgpu::TextureAspect::All,
                    },
                    &bytes[(row * stride) as usize..((row + take) * stride) as usize],
                    wgpu::TexelCopyBufferLayout {
                        offset: 0,
                        bytes_per_row: Some(stride),
                        rows_per_image: Some(take),
                    },
                    wgpu::Extent3d {
                        width,
                        height: take * block,
                        depth_or_array_layers: 1,
                    },
                );
                if row + take < rows {
                    self.tasks
                        .push_front(Transfer::Texture(key, level, row + take));
                }
                (take * stride) as usize
            }
        }
    }
}

#[cfg(test)]
mod streaming_tests {
    use super::*;
    #[test]
    fn prepared_textures_keep_colour_space_and_all_mips() {
        let img = Image {
            width: 64,
            height: 64,
            rgba: vec![128; 64 * 64 * 4],
        };
        let colour = prepare_texture(&img, Kind::Colour);
        let normal = prepare_texture(&img, Kind::Normal);
        assert_eq!(colour.levels.len(), 7);
        assert_eq!(normal.levels.len(), 7);
        assert_eq!(
            colour.compressed.as_ref().unwrap().0,
            wgpu::TextureFormat::Bc1RgbaUnormSrgb
        );
        assert_eq!(
            normal.compressed.as_ref().unwrap().0,
            wgpu::TextureFormat::Bc5RgUnorm
        );
        for (i, (w, h, _)) in colour.levels.iter().enumerate() {
            assert_eq!(
                colour.compressed.as_ref().unwrap().1[i].len(),
                w.div_ceil(4) as usize * h.div_ceil(4) as usize * 8
            );
        }
    }
}
