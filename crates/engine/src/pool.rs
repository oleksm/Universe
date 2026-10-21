//! The pooled mesh store: many small immutable meshes (a planet's clusters, a few hundred triangles
//! each) in one GPU buffer, each a range of it, shared vertices and an index list, drawn all at
//! once (one draw for the faces of every pooled mesh in a layer, one for their edges).
//!
//! A mesh is made on any thread (`PooledMesh::new`: packed there, ready to copy), copied into the
//! pool the first frame it's drawn or prepared (`Frame::prepare_pooled`), and its range let go
//! when its last copy is dropped. Drawn in groups (`Frame::pool_group`): one placement's rotation
//! and scale, tints and light for every member, each member at its own origin (exact: made
//! camera-relative in f64, as every draw).
//!
//! In the buffer, a mesh is a header of `HEADER` words, then its vertices (position as three f32;
//! a normal, octahedral in two snorm16, if it has normals; a colour, four unorm8, if it has
//! colours), its faces' indices (two u16 a word, or u32 if it has more than 65,536 vertices), its
//! edges' (the same), its faces' own colours (one word a face) and its vertices' morph (the way
//! to its parent's shape, three f32, and the band over which it is blended in, near and far in
//! km, two f16). The shader reads them by index (`vs_pool` in scene.wgsl): faces flat-shaded
//! from their own corners unless the mesh is smooth.

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use glam::Vec3;

/// A mesh for the pool, as built: shared vertices, faces and edges by index.
#[derive(Clone, Debug, Default)]
pub struct PoolModel {
    pub positions: Vec<Vec3>,
    /// Triangles, wound counter-clockwise seen from outside.
    pub faces: Vec<[u32; 3]>,
    pub edges: Vec<[u32; 2]>,
    /// Per vertex (empty: none). Shaded smooth by them; edges are lit by them. A smooth mesh, or
    /// one with edges, given none gets its faces' (area-weighted).
    pub normals: Vec<Vec3>,
    /// Per vertex, sRGB as given (empty: white). Edges take them; faces too unless `face_colors`.
    pub colors: Vec<[u8; 4]>,
    /// Per face (empty: none): each face one colour (the flat look).
    pub face_colors: Vec<[u8; 4]>,
    /// Per vertex (empty: none): each vertex moved toward its parent's shape as the eye nears its
    /// band's far end (see `Morph`), faces and edges alike.
    pub morph: Vec<Morph>,
    /// Shaded smooth (by the normals), not face by face.
    pub smooth: bool,
}

/// A vertex's morph: `offset` (model units) is the way to its parent's shape; it is all the way
/// there at `far` metres from the eye and not at all at `near` (smooth between).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Morph {
    pub offset: Vec3,
    pub near: f32,
    pub far: f32,
}

pub(crate) const HEADER: u32 = 8;
const F_NORMALS: u32 = 1;
const F_COLORS: u32 = 2;
const F_FACE_COLORS: u32 = 4;
const F_MORPH: u32 = 8;
const F_U16: u32 = 16;
const F_SMOOTH: u32 = 32;

/// A mesh in the pool (or waiting to go in). Cheap to clone; its range is let go with its last copy.
#[derive(Clone, Debug)]
pub struct PooledMesh(Arc<Inner>);

#[derive(Debug)]
struct Inner {
    id: u64,
    /// As copied into the pool.
    words: Vec<u32>,
    faces: u32,
    edges: u32,
    radius: f32,
    resident: AtomicUsize,
}

/// Meshes whose every copy is gone, for the renderer to let their ranges go.
pub(crate) static GONE: std::sync::Mutex<Vec<u64>> = std::sync::Mutex::new(Vec::new());

impl Drop for Inner {
    fn drop(&mut self) {
        if let Ok(mut g) = GONE.lock() {
            g.push(self.id);
        }
    }
}

impl PooledMesh {
    pub fn new(m: &PoolModel) -> Self {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
        let radius = m.positions.iter().map(|p| p.length()).fold(0.0, f32::max);
        let words = pack(m);
        Self(Arc::new(Inner { id: NEXT.fetch_add(1, Ordering::Relaxed), words, faces: m.faces.len() as u32, edges: m.edges.len() as u32, radius, resident: AtomicUsize::new(0) }))
    }

    pub fn id(&self) -> u64 {
        self.0.id
    }

    /// Copied into the pool (by at least one renderer).
    pub fn gpu_resident(&self) -> bool {
        self.0.resident.load(Ordering::Acquire) > 0
    }

    /// How far its farthest point is from its origin (unscaled).
    pub fn radius(&self) -> f32 {
        self.0.radius
    }

    pub fn faces(&self) -> u32 {
        self.0.faces
    }

    pub fn edges(&self) -> u32 {
        self.0.edges
    }

    /// Its size in the pool (bytes).
    pub fn bytes(&self) -> usize {
        self.0.words.len() * 4
    }

    pub(crate) fn words(&self) -> &[u32] {
        &self.0.words
    }
}

fn oct(n: Vec3) -> u32 {
    let n = n / (n.x.abs() + n.y.abs() + n.z.abs()).max(1e-20);
    let (x, y) = if n.z >= 0.0 { (n.x, n.y) } else { ((1.0 - n.y.abs()) * n.x.signum(), (1.0 - n.x.abs()) * n.y.signum()) };
    let s = |v: f32| ((v.clamp(-1.0, 1.0) * 32767.0).round() as i16) as u16 as u32;
    s(x) | (s(y) << 16)
}

fn half(v: f32) -> u32 {
    crate::renderer::half(v) as u32
}

/// The mesh as the pool holds it (see the module's notes).
fn pack(m: &PoolModel) -> Vec<u32> {
    let n = m.positions.len();
    let normals: Vec<Vec3> = if !m.normals.is_empty() {
        assert_eq!(m.normals.len(), n, "a normal per vertex");
        m.normals.clone()
    } else if m.smooth || !m.edges.is_empty() {
        let mut sum = vec![Vec3::ZERO; n];
        for f in &m.faces {
            let [a, b, c] = f.map(|i| m.positions[i as usize]);
            let w = (b - a).cross(c - a);
            for &i in f {
                sum[i as usize] += w;
            }
        }
        // (A vertex on no face, a bare line's: lit from the origin outward, as a mesh's.)
        sum.iter().zip(&m.positions).map(|(s, p)| s.try_normalize().unwrap_or_else(|| p.normalize_or_zero())).collect()
    } else {
        Vec::new()
    };
    assert!(m.colors.is_empty() || m.colors.len() == n, "a colour per vertex");
    assert!(m.face_colors.is_empty() || m.face_colors.len() == m.faces.len(), "a colour per face");
    assert!(m.morph.is_empty() || m.morph.len() == n, "a morph per vertex");
    let u16s = n <= 1 << 16;
    let mut flags = 0;
    for (on, f) in [(!normals.is_empty(), F_NORMALS), (!m.colors.is_empty(), F_COLORS), (!m.face_colors.is_empty(), F_FACE_COLORS), (!m.morph.is_empty(), F_MORPH), (u16s, F_U16), (m.smooth, F_SMOOTH)] {
        if on {
            flags |= f;
        }
    }
    let stride = 3 + (flags & F_NORMALS != 0) as usize + (flags & F_COLORS != 0) as usize;
    let index_words = |count: usize| if u16s { count.div_ceil(2) } else { count };
    let mut w: Vec<u32> = vec![0; HEADER as usize];
    w.reserve(n * stride + index_words(m.faces.len() * 3) + index_words(m.edges.len() * 2) + m.face_colors.len() + m.morph.len() * 4);
    for i in 0..n {
        w.extend(m.positions[i].to_array().map(f32::to_bits));
        if let Some(nm) = normals.get(i) {
            w.push(oct(*nm));
        }
        if let Some(c) = m.colors.get(i) {
            w.push(u32::from_le_bytes(*c));
        }
    }
    let indices = |w: &mut Vec<u32>, list: &mut dyn Iterator<Item = u32>| {
        let at = w.len() as u32;
        let list: Vec<u32> = list.collect();
        assert!(list.iter().all(|&i| (i as usize) < n), "an index past the vertices");
        if u16s {
            w.extend(list.chunks(2).map(|p| p[0] | p.get(1).map_or(0, |b| b << 16)));
        } else {
            w.extend(list);
        }
        at
    };
    let faces_at = indices(&mut w, &mut m.faces.iter().flatten().copied());
    let edges_at = indices(&mut w, &mut m.edges.iter().flatten().copied());
    let face_colors_at = w.len() as u32;
    w.extend(m.face_colors.iter().map(|c| u32::from_le_bytes(*c)));
    let morph_at = w.len() as u32;
    for k in &m.morph {
        w.extend(k.offset.to_array().map(f32::to_bits));
        w.push(half(k.near / 1000.0) | (half(k.far / 1000.0) << 16));
    }
    w[..HEADER as usize].copy_from_slice(&[n as u32, m.faces.len() as u32, m.edges.len() as u32, flags, faces_at, edges_at, face_colors_at, morph_at]);
    w
}

/// Free ranges of the pool (words), in order, none touching.
#[derive(Debug)]
pub(crate) struct Free(Vec<(u32, u32)>);

impl Free {
    pub fn new(words: u32) -> Self {
        Self(vec![(0, words)])
    }

    /// The first range that fits `len` words.
    pub fn take(&mut self, len: u32) -> Option<u32> {
        let k = self.0.iter().position(|&(_, l)| l >= len)?;
        let (at, l) = self.0[k];
        if l == len {
            self.0.remove(k);
        } else {
            self.0[k] = (at + len, l - len);
        }
        Some(at)
    }

    /// `len` words at `at` free again, joined to their neighbours.
    pub fn give(&mut self, at: u32, len: u32) {
        let k = self.0.partition_point(|&(a, _)| a < at);
        self.0.insert(k, (at, len));
        if k + 1 < self.0.len() && self.0[k].0 + self.0[k].1 == self.0[k + 1].0 {
            self.0[k].1 += self.0[k + 1].1;
            self.0.remove(k + 1);
        }
        if k > 0 && self.0[k - 1].0 + self.0[k - 1].1 == self.0[k].0 {
            self.0[k - 1].1 += self.0[k].1;
            self.0.remove(k);
        }
    }

    /// The free words in all.
    pub fn words(&self) -> u64 {
        self.0.iter().map(|&(_, l)| l as u64).sum()
    }
}

/// A pooled draw as the frame gives it: where (camera-relative), its group, which mesh (its place
/// in the frame's list).
#[derive(Clone, Copy, Debug)]
pub(crate) struct Draw {
    pub t: [f32; 3],
    pub group: u32,
    pub mesh: u32,
    /// Negative: per-vertex distance band. Otherwise a shared tile morph weight.
    pub morph: f32,
}

/// The frame's pooled draws: their groups (one instance each), the meshes they name, and those
/// to copy in without drawing.
#[derive(Default)]
pub(crate) struct PoolFrame {
    pub groups: Vec<(crate::frame::Instance, bool)>,
    pub meshes: Vec<PooledMesh>,
    pub index: HashMap<u64, u32>,
    pub draws: Vec<Draw>,
    pub front: Vec<Draw>,
    pub uploads: Vec<PooledMesh>,
    pub releases: Vec<u64>,
}

impl PoolFrame {
    pub fn mesh(&mut self, m: &PooledMesh) -> u32 {
        *self.index.entry(m.id()).or_insert_with(|| {
            self.meshes.push(m.clone());
            self.meshes.len() as u32 - 1
        })
    }
}

/// A draw as the GPU reads it (`PoolDraw` in scene.wgsl): where, its group, its mesh's header (a
/// word in the pool), the first corner of its faces and of its edges among the layer's.
#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct GpuDraw {
    pub t: [f32; 3],
    pub group: u32,
    pub mesh: u32,
    pub face_start: u32,
    pub edge_start: u32,
    pub morph: f32,
}

/// The pool on the GPU, and this frame's draws.
pub(crate) struct PoolStore {
    pub layout: wgpu::BindGroupLayout,
    buffer: wgpu::Buffer,
    /// Its size (words), and the most it may grow to.
    cap: u32,
    max: u32,
    free: Free,
    /// Each mesh in it: where (words), how long, and its receipt.
    live: HashMap<u64, (u32, u32, Arc<PooledMeshRef>)>,
    draws: wgpu::Buffer,
    draws_cap: u64,
    groups: wgpu::Buffer,
    groups_cap: u64,
    lists: wgpu::Buffer,
    pub bind: wgpu::BindGroup,
    /// This frame's corners to draw: the scene's faces and edges, the front layer's.
    pub scene: (u32, u32),
    pub front: (u32, u32),
}

/// What the store holds of a mesh to tell it it's resident (and no longer when let go).
#[derive(Debug)]
pub(crate) struct PooledMeshRef(std::sync::Weak<Inner>);

impl Drop for PooledMeshRef {
    fn drop(&mut self) {
        if let Some(inner) = self.0.upgrade() { inner.resident.fetch_sub(1, Ordering::Release); }
    }
}

const START_WORDS: u32 = 1 << 20;

impl PoolStore {
    pub fn new(device: &wgpu::Device) -> Self {
        let storage = |binding| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::VERTEX,
            ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Storage { read_only: true }, has_dynamic_offset: false, min_binding_size: None },
            count: None,
        };
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("mesh pool"),
            entries: &[storage(0), storage(1), storage(2), storage(3)],
        });
        let max = (device.limits().max_storage_buffer_binding_size.min(device.limits().max_buffer_size) / 4).min(u32::MAX as u64) as u32;
        let cap = START_WORDS.min(max);
        let buffer = Self::pool_buffer(device, cap);
        let draws = Self::storage(device, "pool draws", 4096);
        let groups = Self::storage(device, "pool groups", 4096);
        let lists = Self::storage(device, "pool lists", 64);
        let bind = Self::bind(device, &layout, &buffer, &draws, &groups, &lists);
        Self { layout, buffer, cap, max, free: Free::new(cap), live: HashMap::new(), draws, draws_cap: 4096, groups, groups_cap: 4096, lists, bind, scene: (0, 0), front: (0, 0) }
    }

    fn pool_buffer(device: &wgpu::Device, words: u32) -> wgpu::Buffer {
        device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("mesh pool"),
            size: words as u64 * 4,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        })
    }

    fn storage(device: &wgpu::Device, label: &str, size: u64) -> wgpu::Buffer {
        device.create_buffer(&wgpu::BufferDescriptor { label: Some(label), size, usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST, mapped_at_creation: false })
    }

    fn bind(device: &wgpu::Device, layout: &wgpu::BindGroupLayout, pool: &wgpu::Buffer, draws: &wgpu::Buffer, groups: &wgpu::Buffer, lists: &wgpu::Buffer) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("mesh pool"),
            layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: pool.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: draws.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 2, resource: groups.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 3, resource: lists.as_entire_binding() },
            ],
        })
    }

    /// The pool's size and what of it is in use (bytes), and the meshes in it.
    pub fn usage(&self) -> (u64, u64, usize) {
        (self.cap as u64 * 4, (self.cap as u64 - self.free.words()) * 4, self.live.len())
    }

    /// Copy `m` in if it isn't (growing the pool if it must). False: no room even grown.
    fn put(&mut self, gpu: &crate::gpu::Gpu, m: &PooledMesh) -> bool {
        if self.live.contains_key(&m.id()) {
            return true;
        }
        let len = m.words().len() as u32;
        let at = match self.free.take(len) {
            Some(at) => at,
            None => {
                // (Grown to twice, or what it needs, the old copied over on the GPU: every range
                // stays where it was.)
                let want = (self.cap as u64 * 2).max(self.cap as u64 + len as u64).min(self.max as u64) as u32;
                if want < self.cap + len {
                    log::warn!("mesh pool full ({} MB): a mesh of {} KB not drawn", self.cap / 262_144, len / 256);
                    return false;
                }
                let bigger = Self::pool_buffer(&gpu.device, want);
                let mut enc = gpu.device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("mesh pool grows") });
                enc.copy_buffer_to_buffer(&self.buffer, 0, &bigger, 0, self.cap as u64 * 4);
                gpu.queue.submit([enc.finish()]);
                self.free.give(self.cap, want - self.cap);
                self.buffer = bigger;
                self.cap = want;
                self.bind = Self::bind(&gpu.device, &self.layout, &self.buffer, &self.draws, &self.groups, &self.lists);
                match self.free.take(len) {
                    Some(at) => at,
                    None => return false,
                }
            }
        };
        gpu.queue.write_buffer(&self.buffer, at as u64 * 4, bytemuck::cast_slice(m.words()));
        m.0.resident.fetch_add(1, Ordering::Release);
        self.live.insert(m.id(), (at, len, Arc::new(PooledMeshRef(Arc::downgrade(&m.0)))));
        true
    }

    /// This frame's pool: meshes let go, new ones copied in, the draws laid out for the GPU.
    pub fn frame(&mut self, gpu: &crate::gpu::Gpu, pf: &PoolFrame) {
        let gone = std::mem::take(&mut *GONE.lock().unwrap_or_else(|e| e.into_inner()));
        for id in gone {
            if let Some((at, len, _)) = self.live.remove(&id) {
                self.free.give(at, len);
            }
        }
        for id in &pf.releases {
            if !pf.meshes.iter().any(|m| m.id() == *id) {
                if let Some((at, len, _)) = self.live.remove(id) { self.free.give(at, len); }
            }
        }
        for m in pf.uploads.iter().chain(&pf.meshes) {
            self.put(gpu, m);
        }
        let mut out: Vec<GpuDraw> = Vec::with_capacity(pf.draws.len() + pf.front.len());
        let lay = |draws: &[Draw], out: &mut Vec<GpuDraw>| {
            let (mut faces, mut edges) = (0u32, 0u32);
            for d in draws {
                let m = &pf.meshes[d.mesh as usize];
                let Some(&(at, ..)) = self.live.get(&m.id()) else { continue };
                out.push(GpuDraw { t: d.t, group: d.group, mesh: at, face_start: faces, edge_start: edges, morph: d.morph });
                faces += m.faces() * 3;
                if pf.groups[d.group as usize].1 {
                    edges += m.edges() * 2;
                }
            }
            (faces, edges)
        };
        self.scene = lay(&pf.draws, &mut out);
        let first_front = out.len() as u32;
        self.front = lay(&pf.front, &mut out);
        // (What's drawn of a layer: the range of draws the shader searches, by `instance_index`.)
        let lists: [u32; 8] = [0, first_front, 0, 0, first_front, out.len() as u32 - first_front, 0, 0];
        gpu.queue.write_buffer(&self.lists, 0, bytemuck::cast_slice(&lists));
        if out.is_empty() {
            return;
        }
        let groups: Vec<crate::frame::Instance> = pf.groups.iter().map(|g| g.0).collect();
        let mut rebind = false;
        for (buf, cap, bytes, label) in [(&mut self.draws, &mut self.draws_cap, bytemuck::cast_slice::<_, u8>(&out), "pool draws"), (&mut self.groups, &mut self.groups_cap, bytemuck::cast_slice::<_, u8>(&groups), "pool groups")] {
            if bytes.len() as u64 > *cap {
                *cap = (bytes.len() as u64).next_power_of_two();
                *buf = Self::storage(&gpu.device, label, *cap);
                rebind = true;
            }
            gpu.queue.write_buffer(buf, 0, bytes);
        }
        if rebind {
            self.bind = Self::bind(&gpu.device, &self.layout, &self.buffer, &self.draws, &self.groups, &self.lists);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The free list hands out and joins ranges back; a mesh packs to its header's layout.
    #[test]
    fn pool_ranges_and_packing() {
        let mut f = Free::new(100);
        let (a, b, c) = (f.take(10).unwrap(), f.take(20).unwrap(), f.take(30).unwrap());
        assert_eq!((a, b, c), (0, 10, 30));
        f.give(b, 20);
        assert_eq!(f.take(25), Some(60));
        f.give(a, 10);
        assert_eq!(f.0, vec![(0, 30), (85, 15)]);
        f.give(c, 30);
        f.give(60, 25);
        assert_eq!(f.0, vec![(0, 100)]);
        assert_eq!(f.take(101), None);

        // A quad: four vertices, two faces with their own colours, its outline.
        let m = PoolModel {
            positions: vec![Vec3::ZERO, Vec3::X, Vec3::new(1.0, 1.0, 0.0), Vec3::Y],
            faces: vec![[0, 1, 2], [0, 2, 3]],
            edges: vec![[0, 1], [1, 2], [2, 3], [3, 0]],
            face_colors: vec![[255, 0, 0, 255], [0, 255, 0, 255]],
            ..Default::default()
        };
        let w = pack(&m);
        let h = &w[..HEADER as usize];
        assert_eq!(&h[..3], &[4, 2, 4]);
        assert_eq!(h[3], F_NORMALS | F_FACE_COLORS | F_U16);
        // (Edges want normals: given none, the faces' (+Z here). Four words a vertex.)
        assert_eq!(h[4], HEADER + 4 * 4);
        assert_eq!(w[(HEADER + 3) as usize], oct(Vec3::Z));
        assert_eq!(&w[h[4] as usize..h[4] as usize + 3], &[1 << 16, 2, 2 | (3 << 16)]);
        assert_eq!(h[5], h[4] + 3);
        assert_eq!(w[h[6] as usize + 1], u32::from_le_bytes([0, 255, 0, 255]));
        assert_eq!(w.len(), h[7] as usize);
        // (Under a fifth of the bytes the plain mesh path sends for it, header and all: 56 B a corner.)
        assert!(w.len() * 4 * 5 < (2 * 3 + 4 * 2) * 56);
    }
}

#[cfg(test)]
mod pyramid_residency_tests {
    use super::*;
    #[test]
    fn pyramid_gpu_receipt_does_not_pin_evicted_cpu_geometry() {
        let mesh=PooledMesh::new(&PoolModel::default());
        let id=mesh.id();
        mesh.0.resident.store(1,Ordering::Release);
        let receipt=PooledMeshRef(Arc::downgrade(&mesh.0));
        assert!(mesh.gpu_resident());
        drop(receipt);assert!(!mesh.gpu_resident());
        let receipt=PooledMeshRef(Arc::downgrade(&mesh.0));
        drop(mesh);
        assert!(receipt.0.upgrade().is_none(),"renderer receipt must not keep a streamed tile alive");
        assert!(GONE.lock().unwrap().contains(&id));
    }
}
