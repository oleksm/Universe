use std::collections::HashSet;
use std::f32::consts::{PI, TAU};

use glam::{DVec3, Quat, Vec3};

/// A model to draw, kept on the GPU: uploaded the first time it's drawn
/// and reused every frame after, by its id. Cheap to clone (shared).
#[derive(Clone, Debug)]
pub struct Mesh {
    id: u64,
    model: std::sync::Arc<WireModel>,
}

impl Mesh {
    pub fn new(model: WireModel) -> Self {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
        Mesh { id: NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed), model: std::sync::Arc::new(model) }
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
}

impl WireModel {
    /// The nearest distance along a ray (model space; `dir` normalised) at
    /// which it hits a face, if it does.
    pub fn ray_hit(&self, origin: Vec3, dir: Vec3) -> Option<f32> {
        let mut best: Option<f32> = None;
        for f in &self.faces {
            let [a, b, c] = f.map(|i| self.positions[i as usize]);
            let (e1, e2) = (b - a, c - a);
            let h = dir.cross(e2);
            let det = e1.dot(h);
            if det.abs() < 1e-9 {
                continue;
            }
            let inv = 1.0 / det;
            let s = origin - a;
            let u = s.dot(h) * inv;
            if !(0.0..=1.0).contains(&u) {
                continue;
            }
            let q = s.cross(e1);
            let v = dir.dot(q) * inv;
            if v < 0.0 || u + v > 1.0 {
                continue;
            }
            let t = e2.dot(q) * inv;
            if t > 0.0 && best.is_none_or(|b| t < b) {
                best = Some(t);
            }
        }
        best
    }

    /// The radius of the sphere about the origin holding every vertex.
    pub fn radius(&self) -> f32 {
        self.positions.iter().map(|p| p.length()).fold(0.0, f32::max)
    }

    /// Unit sphere with `meridians` longitude lines and `parallels` latitude lines
    /// (excluding the poles). `detail` subdivides each line so it stays round.
    pub fn globe(meridians: u32, parallels: u32, detail: u32) -> Self {
        let lon_seg = meridians * detail;
        let lat_seg = (parallels + 1) * detail;
        let mut m = WireModel::default();

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
                m.faces.push([a, c, b]);
                m.faces.push([b, c, d]);
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

    /// Convex hull of `points`, Elite style: every hull face becomes an occluder
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
mod ray_tests {
    use super::*;

    #[test]
    fn a_ray_hits_the_near_face_of_a_cube_and_misses_beside_it() {
        let corners: Vec<Vec3> = (0..8).map(|i| Vec3::new(if i & 1 == 0 { -1.0 } else { 1.0 }, if i & 2 == 0 { -1.0 } else { 1.0 }, if i & 4 == 0 { -1.0 } else { 1.0 })).collect();
        let cube = WireModel::convex_hull(&corners);
        let hit = cube.ray_hit(Vec3::new(0.2, 0.3, 10.0), Vec3::NEG_Z).unwrap();
        assert!((hit - 9.0).abs() < 1e-4, "{hit}");
        assert!(cube.ray_hit(Vec3::new(1.5, 0.0, 10.0), Vec3::NEG_Z).is_none());
        assert!(cube.ray_hit(Vec3::new(0.0, 0.0, 10.0), Vec3::Z).is_none());
        assert!((cube.radius() - 3f32.sqrt()).abs() < 1e-5);
    }
}
