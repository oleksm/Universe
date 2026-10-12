use std::collections::HashSet;
use std::f32::consts::{PI, TAU};

use glam::{DVec3, Quat, Vec3};

/// A world's surface as a cube map (six faces of `size`², in the GPU's
/// cube face order +X −X +Y −Y +Z −Z, rows top down): per texel its height
/// (in units of the world's relief, ocean below 0) and how far inside a
/// crater it is (0..1). Drawn with `Frame::with_globe`: colour, coasts and
/// slopes worked out per pixel from it. Canonical palette 4 uses absolute visible
/// height / relief and a known-water mask (0 dry, 1 wet); palette 3 leaves the
/// mask zero for unknown water.
pub struct GlobeMap {
    id: u64,
    pub size: u32,
    pub texels: Vec<[f32; 2]>,
    /// The world's own colour (sRGB) texel by texel, where it has one (a world grown by the
    /// planet simulation): drawn in place of the palette.
    pub colors: Option<Vec<[u8; 4]>>,
    pub(crate) uploads: Vec<GlobeUpload>,
}

impl GlobeMap {
    pub fn new(size: u32, texels: Vec<[f32; 2]>) -> Self {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
        assert_eq!(texels.len(), (6 * size * size) as usize);
        let uploads = globe_uploads(size, &texels, None);
        GlobeMap { id: NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed), size, texels, colors: None, uploads }
    }

    /// With its own colours (sRGB, as many as its texels).
    pub fn with_colors(mut self, colors: Vec<[u8; 4]>) -> Self {
        assert_eq!(colors.len(), self.texels.len());
        self.uploads = globe_uploads(self.size, &self.texels, Some(&colors));
        self.colors = Some(colors);
        self
    }

    /// CPU staging payload retained until GPU uploads complete (and for later reload).
    pub fn upload_bytes(&self) -> usize { self.uploads.iter().map(|u| u.bytes.capacity()).sum() }

    pub fn id(&self) -> u64 {
        self.id
    }

    /// The direction (unit) through texel centre `(x, y)` of `face`.
    pub fn direction(size: u32, face: usize, x: u32, y: u32) -> glam::DVec3 {
        let u = 2.0 * (x as f64 + 0.5) / size as f64 - 1.0;
        let v = 2.0 * (y as f64 + 0.5) / size as f64 - 1.0;
        let d = match face {
            0 => glam::DVec3::new(1.0, -v, -u),
            1 => glam::DVec3::new(-1.0, -v, u),
            2 => glam::DVec3::new(u, 1.0, v),
            3 => glam::DVec3::new(u, -1.0, -v),
            4 => glam::DVec3::new(u, -v, 1.0),
            _ => glam::DVec3::new(-u, -v, -1.0),
        };
        d.normalize()
    }
}

/// Fully prepared mip data: no filtering or float conversion on the render thread.
pub(crate) struct GlobeUpload {
    pub color: bool,
    pub face: u32,
    pub mip: u32,
    pub side: u32,
    pub bytes: Vec<u8>,
}
fn globe_uploads(size: u32, texels: &[[f32; 2]], colors: Option<&[[u8; 4]]>) -> Vec<GlobeUpload> {
    assert!(size.is_power_of_two());
    let _scope = universe_prof::scope("assets/globe/mips");
    let n = size as usize;
    let mut out = Vec::new();
    for face in 0..6 {
        let mut height = texels[face*n*n..(face+1)*n*n].to_vec();
        let mut color = colors.map_or_else(|| vec![[0; 4]; n*n], |c| c[face*n*n..(face+1)*n*n].to_vec());
        let mut side = n;
        let mut mip = 0;
        loop {
            let half: Vec<u16> = height.iter().flat_map(|t| [crate::renderer::half(t[0]), crate::renderer::half(t[1])]).collect();
            out.push(GlobeUpload { color: false, face: face as u32, mip, side: side as u32, bytes: bytemuck::cast_slice(&half).to_vec() });
            out.push(GlobeUpload { color: true, face: face as u32, mip, side: side as u32, bytes: bytemuck::cast_slice(&color).to_vec() });
            if side == 1 { break; }
            let next = side / 2;
            let indices = |i: usize| { let a = (i/next*2)*side + i%next*2; [a,a+1,a+side,a+side+1] };
            height = (0..next*next).map(|i| {
                let [a,b,c,d] = indices(i).map(|j| height[j]);
                [(a[0]+b[0]+c[0]+d[0])/4., (a[1]+b[1]+c[1]+d[1])/4.]
            }).collect();
            color = (0..next*next).map(|i| {
                let [a,b,c,d] = indices(i).map(|j| color[j]);
                std::array::from_fn(|k| ((a[k] as u32+b[k] as u32+c[k] as u32+d[k] as u32+2)/4) as u8)
            }).collect();
            side = next;
            mip += 1;
        }
    }
    out
}

/// A model to draw, kept on the GPU: uploaded the first time it's drawn
/// and reused every frame after, by its id. Cheap to clone (shared).
#[derive(Clone, Debug)]
pub struct Mesh {
    id: u64,
    model: std::sync::Arc<WireModel>,
    /// How far its farthest point is from its origin.
    radius: f32,
}

impl Mesh {
    pub fn new(model: WireModel) -> Self {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
        let radius = model.positions.iter().map(|p| p.length()).fold(0.0, f32::max);
        Mesh { id: NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed), model: std::sync::Arc::new(model), radius }
    }

    /// How far its farthest point is from its origin (unscaled).
    pub fn radius(&self) -> f32 {
        self.radius
    }

    pub fn id(&self) -> u64 {
        self.id
    }
}

impl std::ops::Deref for Mesh {
    type Target = WireModel;
    fn deref(&self) -> &WireModel {
        &self.model
    }
}

impl From<WireModel> for Mesh {
    fn from(m: WireModel) -> Self {
        Mesh::new(m)
    }
}

/// Placement of a model in the world.
#[derive(Clone, Copy, Debug)]
pub struct Transform {
    pub position: DVec3,
    pub rotation: Quat,
    pub scale: f64,
}

impl Default for Transform {
    fn default() -> Self {
        Self { position: DVec3::ZERO, rotation: Quat::IDENTITY, scale: 1.0 }
    }
}

/// A vector model in the style of 80s space games: visible edges plus
/// occluding faces used for hidden-line removal.
#[derive(Clone, Debug, Default)]
pub struct WireModel {
    pub positions: Vec<Vec3>,
    pub edges: Vec<[u32; 2]>,
    /// Triangles; only used for occlusion, never drawn visibly.
    pub faces: Vec<[u32; 3]>,
    /// Optional per-vertex colors (RGBA). Empty means use the colors given when drawing.
    pub colors: Vec<[f32; 4]>,
    /// Optional per-vertex data for the shaders (none: zero). The near ground's: its surface
    /// fields (wet, scree, bare rock, and 1 where they're read).
    pub data: Vec<[f32; 4]>,
    /// Shaded smooth (a curved surface: each corner's normal the average of
    /// the faces meeting there), not face by face.
    pub smooth: bool,
}

impl WireModel {
    /// Unit sphere with `meridians` longitude lines and `parallels` latitude lines
    /// (excluding the poles). `detail` subdivides each line so it stays round.
    pub fn globe(meridians: u32, parallels: u32, detail: u32) -> Self {
        let lon_seg = meridians * detail;
        let lat_seg = (parallels + 1) * detail;
        let mut m = WireModel { smooth: true, ..WireModel::default() };

        // Grid of (lat_seg + 1) rows x lon_seg columns, rows from north to south pole.
        for row in 0..=lat_seg {
            let theta = row as f32 / lat_seg as f32 * PI;
            for col in 0..lon_seg {
                let phi = col as f32 / lon_seg as f32 * TAU;
                m.positions.push(Vec3::new(theta.sin() * phi.cos(), theta.cos(), theta.sin() * phi.sin()));
            }
        }
        let idx = |row: u32, col: u32| row * lon_seg + col % lon_seg;

        for row in 0..lat_seg {
            for col in 0..lon_seg {
                let (a, b) = (idx(row, col), idx(row, col + 1));
                let (c, d) = (idx(row + 1, col), idx(row + 1, col + 1));
                // (Wound counter-clockwise seen from outside, as every face.)
                m.faces.push([a, b, c]);
                m.faces.push([b, d, c]);
                if col % detail == 0 {
                    m.edges.push([a, c]); // meridian
                }
                if row % detail == 0 && row != 0 {
                    m.edges.push([a, b]); // parallel
                }
            }
        }
        m
    }

    /// Convex hull of `points`, solid-wireframe style: every hull face becomes an occluder
    /// and its outline becomes edges. Coplanar points merge into one polygon.
    pub fn convex_hull(points: &[Vec3]) -> Self {
        let mut m = WireModel { positions: points.to_vec(), ..Default::default() };
        let scale = points.iter().map(|p| p.length()).fold(0.0, f32::max).max(1e-6);
        let eps = scale * 1e-4;
        let n = points.len();
        let mut planes: Vec<(Vec3, f32)> = Vec::new();

        for i in 0..n {
            for j in i + 1..n {
                for k in j + 1..n {
                    let (a, b, c) = (points[i], points[j], points[k]);
                    let Some(mut normal) = (b - a).cross(c - a).try_normalize() else { continue };
                    let mut d = normal.dot(a);
                    let sides = points.iter().map(|p| normal.dot(*p) - d);
                    if sides.clone().all(|s| s <= eps) {
                    } else if sides.clone().all(|s| s >= -eps) {
                        normal = -normal;
                        d = -d;
                    } else {
                        continue;
                    }
                    if !planes.iter().any(|(pn, pd)| pn.dot(normal) > 1.0 - 1e-5 && (pd - d).abs() < eps) {
                        planes.push((normal, d));
                    }
                }
            }
        }

        let mut edges = HashSet::new();
        for (normal, d) in planes {
            let face: Vec<u32> = (0..n as u32).filter(|&i| (normal.dot(points[i as usize]) - d).abs() < eps).collect();
            let ring = m.add_polygon(&face, normal);
            for w in 0..ring.len() {
                let (a, b) = (ring[w], ring[(w + 1) % ring.len()]);
                edges.insert((a.min(b), a.max(b)));
            }
        }
        m.edges = edges.into_iter().map(|(a, b)| [a, b]).collect();
        m
    }

    /// Triangulate a convex polygon given as unordered vertex indices.
    /// Returns the vertices in winding order around `normal`.
    pub fn add_polygon(&mut self, verts: &[u32], normal: Vec3) -> Vec<u32> {
        let pos = |i: u32| self.positions[i as usize];
        let center = verts.iter().map(|&i| pos(i)).sum::<Vec3>() / verts.len() as f32;
        let u = (pos(verts[0]) - center).normalize();
        let v = normal.normalize().cross(u);
        let angle = |i: u32| {
            let p = pos(i) - center;
            p.dot(v).atan2(p.dot(u))
        };
        let mut ring = verts.to_vec();
        ring.sort_by(|&a, &b| angle(a).total_cmp(&angle(b)));
        for k in 1..ring.len().saturating_sub(1) {
            self.faces.push([ring[0], ring[k], ring[k + 1]]);
        }
        ring
    }

    /// Add a closed loop of detail lines (hatches, engines, docking slots...).
    /// Place it on a hull face so the face hides it from behind.
    pub fn add_loop(&mut self, points: &[Vec3]) {
        let base = self.positions.len() as u32;
        self.positions.extend_from_slice(points);
        let n = points.len() as u32;
        for i in 0..n {
            self.edges.push([base + i, base + (i + 1) % n]);
        }
    }
}

#[cfg(test)]
mod winding {
    use super::*;

    /// Every face of a mesh convex about `centre` faces away from it.
    fn outward_about(m: &WireModel, centre: impl Fn(Vec3) -> Vec3) {
        for f in &m.faces {
            let [a, b, c] = f.map(|i| m.positions[i as usize]);
            let n = (b - a).cross(c - a);
            let mid = (a + b + c) / 3.0;
            // (A pole's slivers have no area: nothing to light.)
            if n.length() < 1e-5 {
                continue;
            }
            assert!(n.dot(mid - centre(mid)) > 0.0, "a face wound inward at {mid}");
        }
    }

    #[test]
    fn globes_and_hulls_wind_their_faces_outward() {
        outward_about(&WireModel::globe(8, 5, 4), |_| Vec3::ZERO);
        let cube: Vec<Vec3> = (0..8).map(|k| Vec3::new((k & 1) as f32, (k >> 1 & 1) as f32, (k >> 2 & 1) as f32) + 3.0).collect();
        outward_about(&WireModel::convex_hull(&cube), |_| Vec3::splat(3.5));
    }
}

#[cfg(test)]
mod globe_mip_tests {
    use super::*;
    #[test]
    fn globe_prepared_mips_preserve_height_colour_and_clear_reused_layers() {
        let values = [[1., 0.], [2., 0.], [3., 0.], [4., 0.]].repeat(6);
        let map = GlobeMap::new(2, values);
        assert_eq!(map.uploads.len(), 24);
        assert!(map.uploads.iter().filter(|u| u.color).all(|u| u.bytes.iter().all(|b| *b == 0)));
        for u in map.uploads.iter().filter(|u| !u.color && u.mip == 1) {
            assert_eq!(u.bytes, bytemuck::cast_slice::<u16, u8>(&[crate::renderer::half(2.5), 0]));
        }
        let map = map.with_colors([[0, 4, 8, 255], [4, 8, 12, 255], [8, 12, 16, 255], [12, 16, 20, 255]].repeat(6));
        for u in map.uploads.iter().filter(|u| u.color && u.mip == 1) { assert_eq!(u.bytes, [6,10,14,255]); }
        assert_eq!(map.upload_bytes(), 6*(4+1)*8);
    }
}
