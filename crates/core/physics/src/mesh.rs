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
        // (Incrementally, each point added to the hull so far: fast for a model's tens of thousands of points. A flat
        // set, with no solid to start from, by every plane through three of its points.)
        match hull_triangles(points) {
            Some(faces) => Self::from_hull_faces(points, &faces),
            None => Self::convex_hull_exhaustive(points),
        }
    }

    /// The hull's faces (each its corners, all in one plane) as polygons: their outline's corners only (a point inside
    /// a face or along a side dropped), each triangulated as a fan, its outline as edges.
    fn from_hull_faces(points: &[DVec3], faces: &[(DVec3, Vec<u32>)]) -> Mesh {
        let mut m = Mesh { points: points.to_vec(), ..Default::default() };
        let mut edges = BTreeSet::new();
        for (normal, face) in faces {
            let normal = *normal;
            let mut face = face.clone();
            face.sort_unstable();
            face.dedup();
            let outline = outline(points, &face, normal);
            if outline.len() < 3 {
                continue;
            }
            let ring = m.add_polygon(&outline, normal);
            for w in 0..ring.len() {
                let (a, b) = (ring[w], ring[(w + 1) % ring.len()]);
                edges.insert((a.min(b), a.max(b)));
            }
        }
        m.edges = edges.into_iter().map(|(a, b)| [a, b]).collect();
        m
    }

    /// `convex_hull` by every plane through three points: exact for any set, flat ones included, but O(n⁴): for a
    /// handful of points only.
    fn convex_hull_exhaustive(points: &[DVec3]) -> Mesh {
        let mut m = Mesh { points: points.to_vec(), ..Default::default() };
        let scale = points.iter().map(|p| p.length()).fold(0.0, f64::max).max(1e-9);
        let eps = scale * 1e-7;
        let n = points.len();
        // (Each plane once, by the points on it: a face's near-twin planes
        // (points rounded from f32, far from the origin) hold the same points.)
        let mut planes: Vec<(DVec3, f64, Vec<u32>)> = Vec::new();
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
                    let face: Vec<u32> = (0..n as u32).filter(|&i| (normal.dot(points[i as usize]) - d).abs() < eps).collect();
                    if !planes.iter().any(|(_, _, f)| *f == face) {
                        planes.push((normal, d, face));
                    }
                }
            }
        }
        let mut edges = BTreeSet::new();
        for (normal, _, face) in planes {
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

/// The corners of a flat face's outline (`face`: its points, all in the plane facing `normal`): their convex hull in
/// that plane, points inside or along a side dropped.
fn outline(points: &[DVec3], face: &[u32], normal: DVec3) -> Vec<u32> {
    let u = normal.any_orthonormal_vector();
    let v = normal.cross(u);
    let mut at: Vec<(f64, f64, u32)> = face.iter().map(|&i| (points[i as usize].dot(u), points[i as usize].dot(v), i)).collect();
    at.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.total_cmp(&b.1)));
    let cross = |o: (f64, f64, u32), a: (f64, f64, u32), b: (f64, f64, u32)| (a.0 - o.0) * (b.1 - o.1) - (a.1 - o.1) * (b.0 - o.0);
    let span = at.iter().fold(0.0f64, |m, p| m.max(p.0.abs()).max(p.1.abs())).max(1e-9);
    let tiny = span * span * 1e-12;
    let mut hull: Vec<(f64, f64, u32)> = Vec::new();
    for pass in 0..2 {
        let start = hull.len();
        let it: Box<dyn Iterator<Item = &(f64, f64, u32)>> = if pass == 0 { Box::new(at.iter()) } else { Box::new(at.iter().rev()) };
        for &p in it {
            while hull.len() >= start + 2 && cross(hull[hull.len() - 2], hull[hull.len() - 1], p) <= tiny {
                hull.pop();
            }
            hull.push(p);
        }
        hull.pop();
    }
    hull.into_iter().map(|p| p.2).collect()
}

/// The convex hull of `points` as outward triangles (indices into `points`), built incrementally: a first
/// tetrahedron, then each point outside the hull so far replaces the faces it sees with a fan from their horizon to
/// it; returned as its faces (each the corners of the triangles in one plane). Its points on a grid of 2⁻²⁰ m (about a micrometre, finer than a model's f32 points) and its side tests exact
/// in integers: no tolerance, so the faces a point sees are always one patch and the hull stays convex. None where
/// the points span no solid (all on a plane, a line or one point) or reach past the grid's ±8,000 km.
fn hull_triangles(points: &[DVec3]) -> Option<Vec<(DVec3, Vec<u32>)>> {
    let n = points.len();
    if n < 4 || points.iter().any(|p| !p.is_finite() || p.abs().max_element() > 8.0e6) {
        return None;
    }
    let g: Vec<[i64; 3]> = points.iter().map(|p| (*p * 1048576.0).round().to_array().map(|v| v as i64)).collect();
    let sub = |a: [i64; 3], b: [i64; 3]| [(a[0] - b[0]) as i128, (a[1] - b[1]) as i128, (a[2] - b[2]) as i128];
    // (Which side of the plane through a, b, c point q is: positive along (b − a) × (c − a). Exact.)
    let orient = |a: usize, b: usize, c: usize, q: usize| -> i128 {
        let (u, v, w) = (sub(g[b], g[a]), sub(g[c], g[a]), sub(g[q], g[a]));
        u[0] * (v[1] * w[2] - v[2] * w[1]) - u[1] * (v[0] * w[2] - v[2] * w[0]) + u[2] * (v[0] * w[1] - v[1] * w[0])
    };
    let collinear = |a: usize, b: usize, c: usize| {
        let (u, v) = (sub(g[b], g[a]), sub(g[c], g[a]));
        u[1] * v[2] - u[2] * v[1] == 0 && u[2] * v[0] - u[0] * v[2] == 0 && u[0] * v[1] - u[1] * v[0] == 0
    };
    // (The first tetrahedron: any four points not on one plane.)
    let a = 0;
    let b = (1..n).find(|&i| g[i] != g[a])?;
    let c = (1..n).find(|&i| !collinear(a, b, i))?;
    let d = (1..n).find(|&i| orient(a, b, c, i) != 0)?;
    // (Faces wound so that the tetrahedron's other corner is behind each: outward.)
    let wind = |t: [usize; 3], other: usize| if orient(t[0], t[1], t[2], other) > 0 { [t[0], t[2], t[1]] } else { t };
    let mut faces: Vec<[usize; 3]> = vec![wind([a, b, c], d), wind([a, b, d], c), wind([a, c, d], b), wind([b, c, d], a)];
    // (Farthest from the first corner first: the hull grows fast and most points then fall inside it.)
    let mut order: Vec<usize> = (0..n).filter(|i| ![a, b, c, d].contains(i)).collect();
    let from = points[a];
    order.sort_by(|&i, &j| points[j].distance_squared(from).total_cmp(&points[i].distance_squared(from)));
    let mut seen: Vec<bool> = Vec::new();
    let mut horizon: std::collections::HashSet<(usize, usize)> = std::collections::HashSet::new();
    for q in order {
        seen.clear();
        seen.extend(faces.iter().map(|t| orient(t[0], t[1], t[2], q) > 0));
        if !seen.iter().any(|&v| v) {
            continue;
        }
        // (The horizon: the seen faces' edges whose twins belong to faces not seen.)
        horizon.clear();
        for (k, t) in faces.iter().enumerate() {
            if seen[k] {
                for e in [(t[0], t[1]), (t[1], t[2]), (t[2], t[0])] {
                    if !horizon.remove(&(e.1, e.0)) {
                        horizon.insert(e);
                    }
                }
            }
        }
        let mut k = 0;
        faces.retain(|_| {
            let keep = !seen[k];
            k += 1;
            keep
        });
        faces.extend(horizon.iter().map(|&(u, v)| [u, v, q]));
    }
    // (Its faces: the triangles joined across edges whose two sides lie exactly in one plane; each face's corners.)
    let by_edge: std::collections::HashMap<(usize, usize), usize> = faces.iter().enumerate().flat_map(|(k, t)| [((t[0], t[1]), k), ((t[1], t[2]), k), ((t[2], t[0]), k)]).collect();
    let mut up: Vec<usize> = (0..faces.len()).collect();
    fn root(up: &mut [usize], mut x: usize) -> usize {
        while up[x] != x {
            up[x] = up[up[x]];
            x = up[x];
        }
        x
    }
    for (k, t) in faces.iter().enumerate() {
        for (u, v) in [(t[0], t[1]), (t[1], t[2]), (t[2], t[0])] {
            if let Some(&o) = by_edge.get(&(v, u)) {
                let w = faces[o].into_iter().find(|&x| x != u && x != v).unwrap_or(u);
                if orient(t[0], t[1], t[2], w) == 0 {
                    let (x, y) = (root(&mut up, k), root(&mut up, o));
                    up[x] = y;
                }
            }
        }
    }
    // (Each face's outward normal from its largest triangle, wound outward as they all are.)
    let mut groups: std::collections::BTreeMap<usize, (DVec3, f64, Vec<u32>)> = std::collections::BTreeMap::new();
    for (k, t) in faces.iter().enumerate() {
        let r = root(&mut up, k);
        let n = (points[t[1]] - points[t[0]]).cross(points[t[2]] - points[t[0]]);
        let g = groups.entry(r).or_insert((DVec3::ZERO, -1.0, Vec::new()));
        if n.length() > g.1 {
            g.0 = n.normalize_or_zero();
            g.1 = n.length();
        }
        g.2.extend(t.iter().map(|&v| v as u32));
    }
    Some(groups.into_values().map(|(n, _, v)| (n, v)).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The incremental hull is the exhaustive one (the same faces' area and edges) on random clouds; on a cube with a
    /// point in a face's middle and one inside, it's the cube (area 6, its 12 edges: the exhaustive one fans the
    /// face's middle point into its outline and loses a quarter of the face); a flat set goes the exhaustive way.
    #[test]
    fn convex_hull_matches_exhaustive() {
        let cube: Vec<DVec3> = (0..8).map(|i| DVec3::new((i & 1) as f64, (i >> 1 & 1) as f64, (i >> 2) as f64)).chain([DVec3::new(0.5, 0.5, 1.0), DVec3::splat(0.5)]).collect();
        let m = Mesh::convex_hull(&cube);
        assert!((m.area() - 6.0).abs() < 1e-12 && m.edges.len() == 12, "area {}, {} edges", m.area(), m.edges.len());
        let mut x = 12345u64;
        let mut r = || {
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            (x % 10_000) as f64 / 1000.0 - 5.0
        };
        for _ in 0..4 {
            let c: Vec<DVec3> = (0..40).map(|_| DVec3::new(r(), r(), r())).collect();
            let (fast, slow) = (Mesh::convex_hull(&c), Mesh::convex_hull_exhaustive(&c));
            assert!((fast.area() - slow.area()).abs() < 1e-9 * slow.area(), "area {} vs {}", fast.area(), slow.area());
            assert_eq!(fast.edges, slow.edges);
        }
        let flat: Vec<DVec3> = (0..6).map(|i| DVec3::new(i as f64, (i * i % 5) as f64, 0.0)).collect();
        assert!(hull_triangles(&flat).is_none());
    }
}
