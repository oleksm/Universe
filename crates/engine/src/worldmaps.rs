//! A world's own maps at full resolution (equirectangular: row 0 north, column 0 at −180° of
//! longitude `atan2(−z, x)` in its own frame), for the ground near it: its colour, the colour of
//! its plants and soil, its normals, its climate and its rock map (`docs/planet-studio-plan.md`).
//! They are encoded where they're made (block-compressed, on the caller's threads: a world's
//! colour is 8192 × 4096) and handed to the renderer whole; one world's set is bound at a time
//! (group 2 of the mesh shaders), the one whose globe layer `Frame::world_maps` names.

use crate::pbr::Image;

/// Which map (its binding in group 2, in this order).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Slot {
    /// The world's true colour (sRGB).
    Color,
    /// Its plants and soil, without rock and snow (sRGB).
    Ground,
    /// Its normals (the relief's light and shade finer than the mesh).
    Normal,
    /// Its climate (the planet simulation's encoding).
    Climate,
    /// Its rock map (each texel a rock unit's number, read exactly).
    Rock,
    /// The sea's calmness (the bake's globe_spec: 1 − wind / 9 m/s, 0.35..1).
    Spec,
}

pub const SLOTS: usize = 6;

/// Each slot's binding in group 2 (the samplers at 5 and 6, the air at 7 came before the sea's map).
const BINDINGS: [u32; SLOTS] = [0, 1, 2, 3, 4, 8];

/// Frames a world's maps take to fade in once bound.
const FADE_FRAMES: f32 = 60.0;

/// One map, encoded for the GPU: its format and each mip level's bytes.
pub struct Encoded {
    pub width: u32,
    pub height: u32,
    pub format: wgpu::TextureFormat,
    /// (width, height, bytes, bytes a row of blocks or texels, rows).
    pub levels: Vec<(u32, u32, Vec<u8>, u32, u32)>,
}

/// A world's maps, encoded.
pub struct WorldMaps {
    id: u64,
    pub maps: [Option<Encoded>; SLOTS],
    /// Its air, as the shaders' `Air` (16 floats; the last 1 if it has air, 0 if none).
    pub air: [f32; 16],
    /// Its air's tables: the sun's transmittance and the higher orders' light, (width, height,
    /// RGBA floats), if baked (bindings 9 and 10, as half floats: filterable everywhere).
    pub air_luts: Option<[(u32, u32, Vec<f32>); 2]>,
}

impl WorldMaps {
    /// With its air's tables.
    pub fn with_air_luts(mut self, luts: Option<[(u32, u32, Vec<f32>); 2]>) -> Self {
        self.air_luts = luts;
        self
    }

    /// Are its air's tables here?
    pub fn has_air_luts(&self) -> bool {
        self.air_luts.is_some()
    }
}

impl WorldMaps {
    /// Encode `maps` (RGBA8 images, by `Slot`; any may be missing): colour, ground and normals
    /// block-compressed with their mips, the climate as it is, the rock map one byte a texel
    /// (its red), no mips (read exactly).
    pub fn new(maps: [Option<Image>; SLOTS], air: Option<[f32; 16]>) -> Self {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
        let mut out: [Option<Encoded>; SLOTS] = Default::default();
        for (k, img) in maps.into_iter().enumerate() {
            let Some(img) = img else { continue };
            out[k] = Some(match k {
                0..=2 | 5 if img.width % 4 == 0 && img.height % 4 == 0 => {
                    let srgb = k < 2;
                    let levels = crate::pbr::mips(&img, srgb);
                    let params = texpresso::Params { algorithm: texpresso::Algorithm::RangeFit, ..Default::default() };
                    let levels = levels
                        .into_iter()
                        .map(|(w, h, px)| {
                            let mut c = vec![0u8; texpresso::Format::Bc1.compressed_size(w as usize, h as usize)];
                            texpresso::Format::Bc1.compress(&px, w as usize, h as usize, params, &mut c);
                            (w, h, c, w.div_ceil(4) * 8, h.div_ceil(4))
                        })
                        .collect();
                    Encoded { width: img.width, height: img.height, format: if srgb { wgpu::TextureFormat::Bc1RgbaUnormSrgb } else { wgpu::TextureFormat::Bc1RgbaUnorm }, levels }
                }
                4 => {
                    let px: Vec<u8> = img.rgba.chunks(4).map(|c| c[0]).collect();
                    Encoded { width: img.width, height: img.height, format: wgpu::TextureFormat::R8Unorm, levels: vec![(img.width, img.height, px, img.width, img.height)] }
                }
                _ => {
                    let srgb = k < 2;
                    let levels = crate::pbr::mips(&img, srgb).into_iter().map(|(w, h, px)| (w, h, px, w * 4, h)).collect();
                    Encoded { width: img.width, height: img.height, format: if srgb { wgpu::TextureFormat::Rgba8UnormSrgb } else { wgpu::TextureFormat::Rgba8Unorm }, levels }
                }
            });
        }
        WorldMaps { id: NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed), maps: out, air: air.unwrap_or([0.0; 16]), air_luts: None }
    }

    pub fn id(&self) -> u64 {
        self.id
    }
}

/// The renderer's side: the bound set (group 2) and what it holds.
pub(crate) struct WorldBind {
    pub layout: wgpu::BindGroupLayout,
    pub bind: wgpu::BindGroup,
    linear: wgpu::Sampler,
    nearest: wgpu::Sampler,
    /// The bound world's air (binding 7: the shaders' `Air`).
    air: wgpu::Buffer,
    /// A clamping sampler for its tables (binding 11).
    clamped: wgpu::Sampler,
    /// The maps bound now (their id), if any, and when (frames counted by `fade`).
    pub current: Option<u64>,
    since: u32,
}

impl WorldBind {
    pub fn new(device: &wgpu::Device, queue: &wgpu::Queue) -> Self {
        let tex = |binding: u32| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Texture { sample_type: wgpu::TextureSampleType::Float { filterable: true }, view_dimension: wgpu::TextureViewDimension::D2, multisampled: false },
            count: None,
        };
        let samp = |binding: u32| wgpu::BindGroupLayoutEntry { binding, visibility: wgpu::ShaderStages::FRAGMENT, ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering), count: None };
        let air_entry = wgpu::BindGroupLayoutEntry { binding: 7, visibility: wgpu::ShaderStages::FRAGMENT | wgpu::ShaderStages::VERTEX, ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Uniform, has_dynamic_offset: false, min_binding_size: None }, count: None };
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor { label: Some("world maps"), entries: &[tex(0), tex(1), tex(2), tex(3), tex(4), samp(5), samp(6), air_entry, tex(8), tex(9), tex(10), samp(11)] });
        let clamped = device.create_sampler(&wgpu::SamplerDescriptor { label: Some("world air tables"), mag_filter: wgpu::FilterMode::Linear, min_filter: wgpu::FilterMode::Linear, ..Default::default() });
        let air = device.create_buffer(&wgpu::BufferDescriptor { label: Some("world air"), size: 64, usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST, mapped_at_creation: false });
        // (Round the world in longitude; clamped at the poles.)
        let linear = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("world maps"),
            address_mode_u: wgpu::AddressMode::Repeat,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Linear,
            anisotropy_clamp: 16,
            ..Default::default()
        });
        let nearest = device.create_sampler(&wgpu::SamplerDescriptor { label: Some("world maps, exact"), address_mode_u: wgpu::AddressMode::Repeat, address_mode_v: wgpu::AddressMode::ClampToEdge, ..Default::default() });
        let blank = Self::blank(device, queue);
        let views: Vec<wgpu::TextureView> = (0..SLOTS + 2).map(|_| blank.clone()).collect();
        let bind = Self::group(device, &layout, &views, &linear, &nearest, &air, &clamped);
        WorldBind { layout, bind, linear, nearest, air, clamped, current: None, since: 0 }
    }

    /// A texel of nothing (alpha 0: no map).
    fn blank(device: &wgpu::Device, queue: &wgpu::Queue) -> wgpu::TextureView {
        let t = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("world map (none)"),
            size: wgpu::Extent3d { width: 1, height: 1, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        queue.write_texture(
            wgpu::TexelCopyTextureInfo { texture: &t, mip_level: 0, origin: wgpu::Origin3d::ZERO, aspect: wgpu::TextureAspect::All },
            &[0, 0, 0, 0],
            wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(4), rows_per_image: Some(1) },
            wgpu::Extent3d { width: 1, height: 1, depth_or_array_layers: 1 },
        );
        t.create_view(&Default::default())
    }

    /// `views`: the maps by slot, then the air's two tables (bindings 9 and 10).
    #[allow(clippy::too_many_arguments)]
    fn group(device: &wgpu::Device, layout: &wgpu::BindGroupLayout, views: &[wgpu::TextureView], linear: &wgpu::Sampler, nearest: &wgpu::Sampler, air: &wgpu::Buffer, clamped: &wgpu::Sampler) -> wgpu::BindGroup {
        let mut entries: Vec<wgpu::BindGroupEntry> = views.iter().enumerate().map(|(k, v)| wgpu::BindGroupEntry { binding: if k < SLOTS { BINDINGS[k] } else { 9 + (k - SLOTS) as u32 }, resource: wgpu::BindingResource::TextureView(v) }).collect();
        entries.push(wgpu::BindGroupEntry { binding: 11, resource: wgpu::BindingResource::Sampler(clamped) });
        entries.push(wgpu::BindGroupEntry { binding: 5, resource: wgpu::BindingResource::Sampler(linear) });
        entries.push(wgpu::BindGroupEntry { binding: 6, resource: wgpu::BindingResource::Sampler(nearest) });
        entries.push(wgpu::BindGroupEntry { binding: 7, resource: air.as_entire_binding() });
        device.create_bind_group(&wgpu::BindGroupDescriptor { label: Some("world maps"), layout, entries: &entries })
    }

    /// How far the maps bound now are faded in (0..1, over a second's frames from binding: the
    /// look they bring comes in gently, not in one frame). Counts the frame.
    pub fn fade(&mut self) -> f32 {
        self.since = self.since.saturating_add(1);
        (self.since as f32 / FADE_FRAMES).min(1.0)
    }

    /// Bind `maps` (uploading them, if they aren't bound already).
    pub fn bind(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, maps: &WorldMaps) {
        if self.current == Some(maps.id) {
            return;
        }
        self.since = 0;
        let bc = device.features().contains(wgpu::Features::TEXTURE_COMPRESSION_BC);
        let views: Vec<wgpu::TextureView> = maps
            .maps
            .iter()
            .map(|m| match m {
                Some(e) if bc || !e.format.is_compressed() => {
                    let t = device.create_texture(&wgpu::TextureDescriptor {
                        label: Some("world map"),
                        size: wgpu::Extent3d { width: e.width, height: e.height, depth_or_array_layers: 1 },
                        mip_level_count: e.levels.len() as u32,
                        sample_count: 1,
                        dimension: wgpu::TextureDimension::D2,
                        format: e.format,
                        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                        view_formats: &[],
                    });
                    for (level, (w, h, bytes, row, rows)) in e.levels.iter().enumerate() {
                        // (A compressed level is whole blocks: one smaller than a block is its block's size.)
                        let (w, h) = if e.format.is_compressed() { (w.div_ceil(4) * 4, h.div_ceil(4) * 4) } else { (*w, *h) };
                        queue.write_texture(
                            wgpu::TexelCopyTextureInfo { texture: &t, mip_level: level as u32, origin: wgpu::Origin3d::ZERO, aspect: wgpu::TextureAspect::All },
                            bytes,
                            wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(*row), rows_per_image: Some(*rows) },
                            wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
                        );
                    }
                    t.create_view(&Default::default())
                }
                _ => Self::blank(device, queue),
            })
            .collect();
        queue.write_buffer(&self.air, 0, bytemuck::cast_slice(&maps.air));
        let mut views = views;
        // The air's tables as half floats (filterable without a feature), or none.
        for k in 0..2 {
            views.push(match &maps.air_luts {
                Some(luts) => {
                    let (w, h, px) = &luts[k];
                    let t = device.create_texture(&wgpu::TextureDescriptor {
                        label: Some("world air table"),
                        size: wgpu::Extent3d { width: *w, height: *h, depth_or_array_layers: 1 },
                        mip_level_count: 1,
                        sample_count: 1,
                        dimension: wgpu::TextureDimension::D2,
                        format: wgpu::TextureFormat::Rgba16Float,
                        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                        view_formats: &[],
                    });
                    let half: Vec<u16> = px.iter().map(|v| crate::renderer::half(*v)).collect();
                    queue.write_texture(
                        wgpu::TexelCopyTextureInfo { texture: &t, mip_level: 0, origin: wgpu::Origin3d::ZERO, aspect: wgpu::TextureAspect::All },
                        bytemuck::cast_slice(&half),
                        wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(w * 8), rows_per_image: Some(*h) },
                        wgpu::Extent3d { width: *w, height: *h, depth_or_array_layers: 1 },
                    );
                    t.create_view(&Default::default())
                }
                None => Self::blank(device, queue),
            });
        }
        self.bind = Self::group(device, &self.layout, &views, &self.linear, &self.nearest, &self.air, &self.clamped);
        self.current = Some(maps.id);
    }
}
