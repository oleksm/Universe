//! Closed triangle meshes: the convex hull of a set of points, and what a
//! solid of that shape is physically — its volume, centre of mass and inertia
//! (at a uniform density), its size and its silhouette.

use std::collections::BTreeSet;

use glam::{DMat3, DVec3};

/// A closed mesh: triangles wound counter-clockwise seen from outside, and
/// the edges worth drawing (a face's outline, not the seams inside it).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Mesh {
    pub points: Vec<DVec3>,
    pub faces: Vec<[u32; 3]>,
    pub edges: Vec<[u32; 2]>,
}

/// What a solid of uniform density 1 is: its volume (m³), centre of mass,
/// and inertia tensor about the centre of mass (per unit density).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MassProperties {
    pub volume: f64,
    pub centroid: DVec3,
    pub inertia: DMat3,
}

impl Mesh {
    /// The convex hull of `points`: every face a triangle fan over its
    /// polygon, with the polygon's outline as edges. Coplanar points merge
    /// into one polygon.
    pub fn convex_hull(points: &[DVec3]) -> Mesh {
        let mut m = Mesh { points: points.to_vec(), ..Default::default() };
        let scale = points.iter().map(|p| p.length()).fold(0.0, f64::max).max(1e-9);
        let eps = scale * 1e-7;
        let n = points.len();
        let mut planes: Vec<(DVec3, f64)> = Vec::new();
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
                    if !planes.iter().any(|(pn, pd)| pn.dot(normal) > 1.0 - 1e-9 && (pd - d).abs() < eps) {
                        planes.push((normal, d));
                    }
                }
            }
        }
        let mut edges = BTreeSet::new();
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

    /// Triangulates a convex polygon (vertex indices, any order) facing
    /// `normal`; its vertices in winding order.
    fn add_polygon(&mut self, verts: &[u32], normal: DVec3) -> Vec<u32> {
        let pos = |i: u32| self.points[i as usize];
        let center = verts.iter().map(|&i| pos(i)).sum::<DVec3>() / verts.len() as f64;
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

    /// Scaled by `s` about the origin.
    pub fn scaled(mut self, s: f64) -> Mesh {
        for p in &mut self.points {
            *p *= s;
        }
        self
    }

    /// The radius of the sphere about the origin that encloses it (m).
    pub fn bound(&self) -> f64 {
        self.points.iter().map(|p| p.length()).fold(0.0, f64::max)
    }

    /// Its extent along each axis: (min, max).
    pub fn extent(&self) -> (DVec3, DVec3) {
        self.points.iter().fold((DVec3::splat(f64::INFINITY), DVec3::splat(f64::NEG_INFINITY)), |(lo, hi), &p| (lo.min(p), hi.max(p)))
    }

    /// Its outer surface area (m²).
    pub fn area(&self) -> f64 {
        self.faces.iter().map(|f| self.normal(f).length() * 0.5).sum()
    }

    /// The area of its silhouette seen along `dir` (m²): half its faces'
    /// projected areas (a closed convex surface covers its outline twice).
    pub fn silhouette(&self, dir: DVec3) -> f64 {
        let d = dir.normalize_or_zero();
        0.5 * self.faces.iter().map(|f| (self.normal(f).dot(d) * 0.5).abs()).sum::<f64>()
    }

    /// A face's corners.
    fn corners(&self, f: &[u32; 3]) -> (DVec3, DVec3, DVec3) {
        (self.points[f[0] as usize], self.points[f[1] as usize], self.points[f[2] as usize])
    }

    /// A face's normal, as long as twice its area.
    fn normal(&self, f: &[u32; 3]) -> DVec3 {
        let (a, b, c) = self.corners(f);
        (b - a).cross(c - a)
    }

    /// Volume, centre of mass and inertia of the solid it bounds, at a
    /// uniform density of 1 (exact, from the tetrahedra its faces make with
    /// the origin; scale the inertia by the density for a real solid).
    pub fn mass_properties(&self) -> MassProperties {
        let mut volume = 0.0;
        let mut first = DVec3::ZERO;
        // Second moments: ∫x², ∫y², ∫z², ∫xy, ∫yz, ∫zx.
        let (mut xx, mut yy, mut zz, mut xy, mut yz, mut zx) = (0.0, 0.0, 0.0, 0.0, 0.0, 0.0);
        for f in &self.faces {
            let (a, b, c) = self.corners(f);
            let v = a.dot(b.cross(c)) / 6.0;
            volume += v;
            first += v * (a + b + c) / 4.0;
            // For a tetrahedron with a vertex at the origin: ∫x_i x_j = v/20 (Σ x_i x_j over its corners + (Σx_i)(Σx_j)).
            let s = a + b + c;
            let m = |i: usize, j: usize| v / 20.0 * (a[i] * a[j] + b[i] * b[j] + c[i] * c[j] + s[i] * s[j]);
            xx += m(0, 0);
            yy += m(1, 1);
            zz += m(2, 2);
            xy += m(0, 1);
            yz += m(1, 2);
            zx += m(2, 0);
        }
        let centroid = if volume.abs() > 1e-12 { first / volume } else { DVec3::ZERO };
        // About the origin, then moved to the centre of mass (parallel axes).
        let about_origin = DMat3::from_cols(DVec3::new(yy + zz, -xy, -zx), DVec3::new(-xy, xx + zz, -yz), DVec3::new(-zx, -yz, xx + yy));
        let c = centroid;
        let shift = DMat3::from_cols(DVec3::new(c.y * c.y + c.z * c.z, -c.x * c.y, -c.z * c.x), DVec3::new(-c.x * c.y, c.x * c.x + c.z * c.z, -c.y * c.z), DVec3::new(-c.z * c.x, -c.y * c.z, c.x * c.x + c.y * c.y));
        MassProperties { volume, centroid, inertia: about_origin - shift * volume }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cube(half: f64, offset: DVec3) -> Mesh {
        let mut pts = Vec::new();
        for x in [-half, half] {
            for y in [-half, half] {
                for z in [-half, half] {
                    pts.push(DVec3::new(x, y, z) + offset);
                }
            }
        }
        Mesh::convex_hull(&pts)
    }

    #[test]
    fn a_cube_is_a_cube_wherever_it_is() {
        let off = DVec3::new(3.0, -2.0, 5.0);
        let m = cube(1.0, off);
        assert_eq!(m.faces.len(), 12, "six squares, two triangles each");
        assert_eq!(m.edges.len(), 12, "the outlines, not the diagonals");
        let p = m.mass_properties();
        assert!((p.volume - 8.0).abs() < 1e-9, "{}", p.volume);
        assert!(p.centroid.distance(off) < 1e-9, "{}", p.centroid);
        // A cube of side 2 and mass 8: I = m (a² + a²) / 12 = 8 × 8 / 12 about each axis.
        let i = 8.0 * 8.0 / 12.0;
        assert!((p.inertia.x_axis.x - i).abs() < 1e-9 && (p.inertia.y_axis.y - i).abs() < 1e-9 && (p.inertia.z_axis.z - i).abs() < 1e-9, "{:?}", p.inertia);
        assert!(p.inertia.x_axis.y.abs() < 1e-9, "no products of inertia");
        assert!((m.area() - 24.0).abs() < 1e-9);
        assert!((m.silhouette(DVec3::X) - 4.0).abs() < 1e-9, "a 2×2 square edge-on");
    }

    #[test]
    fn a_long_box_turns_harder_end_over_end_than_about_its_length() {
        let pts: Vec<DVec3> = [-1.0, 1.0].iter().flat_map(|&x| [-1.0, 1.0].iter().flat_map(move |&y| [-5.0, 5.0].iter().map(move |&z| DVec3::new(x, y, z)))).collect();
        let p = Mesh::convex_hull(&pts).mass_properties();
        assert!((p.volume - 40.0).abs() < 1e-9);
        assert!(p.inertia.x_axis.x > 10.0 * p.inertia.z_axis.z, "end over end vs. rolling: {:?}", p.inertia);
    }
}
