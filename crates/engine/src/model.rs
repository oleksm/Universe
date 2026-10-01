use std::collections::HashSet;
use std::f32::consts::{PI, TAU};

use glam::{DVec3, Quat, Vec3};

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
}

impl WireModel {
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
