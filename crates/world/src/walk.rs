//! Walking on real geometry: a person as a body that stands on and bumps
//! into what's actually there — a modelled hull's own surfaces (its walk
//! mesh, every triangle it's drawn with), buildings, other ships, the ground.
//!
//! The body: two spheres (knee and head, `BODY_RADIUS`) above the feet, kept
//! out of every surface, sliding along walls; the feet set down on the floor
//! a short ray finds below (steps up to `STEP`, floors up to `SLOPE` steep),
//! falling under gravity when there's none.

use std::collections::HashMap;

use glam::{DQuat, DVec3, Vec3};

/// How wide a person is (m), for walls.
pub const BODY_RADIUS: f64 = 0.35;
/// How tall (m): the head sphere's top.
pub const HEIGHT: f64 = 1.8;
/// The highest step walked up without a jump (m): the knee sphere sits above it.
pub const STEP: f64 = 0.45;
/// The steepest floor stood on (its normal's cosine with up: 50°).
pub const SLOPE: f64 = 0.643;
/// How far the feet follow a floor that falls away under them, walking (m): stairs down.
const SNAP: f64 = 0.3;
/// The longest move between collision checks (m): less than a body's radius, so nothing is passed through.
const SUBSTEP: f64 = 0.15;
/// The grid's cell (m).
const CELL: f32 = 1.0;

/// A model's surfaces to walk on and bump into: its triangles (its own frame,
/// metres), in a grid for finding those near a point.
pub struct WalkMesh {
    tris: Vec<[Vec3; 3]>,
    grid: HashMap<(i32, i32, i32), Vec<u32>>,
    /// Its bounds.
    pub lo: DVec3,
    pub hi: DVec3,
}

impl std::fmt::Debug for WalkMesh {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "WalkMesh({} triangles)", self.tris.len())
    }
}

impl PartialEq for WalkMesh {
    fn eq(&self, other: &Self) -> bool {
        self.tris == other.tris
    }
}

fn cell(p: Vec3) -> (i32, i32, i32) {
    let c = (p / CELL).floor();
    (c.x as i32, c.y as i32, c.z as i32)
}

impl WalkMesh {
    pub fn new(tris: &[[DVec3; 3]]) -> Self {
        let tris: Vec<[Vec3; 3]> = tris.iter().filter(|t| (t[1] - t[0]).cross(t[2] - t[0]).length_squared() > 1e-12).map(|t| t.map(|p| p.as_vec3())).collect();
        let mut grid: HashMap<(i32, i32, i32), Vec<u32>> = HashMap::new();
        let (mut lo, mut hi) = (Vec3::splat(f32::MAX), Vec3::splat(f32::MIN));
        for (k, t) in tris.iter().enumerate() {
            let (a, b) = (t[0].min(t[1]).min(t[2]), t[0].max(t[1]).max(t[2]));
            lo = lo.min(a);
            hi = hi.max(b);
            let (c0, c1) = (cell(a), cell(b));
            for x in c0.0..=c1.0 {
                for y in c0.1..=c1.1 {
                    for z in c0.2..=c1.2 {
                        grid.entry((x, y, z)).or_default().push(k as u32);
                    }
                }
            }
        }
        WalkMesh { tris, grid, lo: lo.as_dvec3(), hi: hi.as_dvec3() }
    }

    /// Where its surfaces cross the level plane `y` (its frame): line segments
    /// (x, z) — a cross-section, as a deck plan draws the hull.
    pub fn section_y(&self, y: f64) -> Vec<[glam::DVec2; 2]> {
        self.section(1, y).into_iter().map(|[a, b]| [glam::DVec2::new(a.x, a.z), glam::DVec2::new(b.x, b.z)]).collect()
    }

    /// Its side elevation: the edges seen from either side (x), on the faces seen
    /// from outside (a ray from each to that side gets away), where it creases
    /// (faces meeting at more than 20°) or ends; segments (z, y).
    pub fn elevation_x(&self) -> Vec<[glam::DVec2; 2]> {
        let key = |p: Vec3| ((p.x * 500.0).round() as i64, (p.y * 500.0).round() as i64, (p.z * 500.0).round() as i64);
        let reach = (self.hi - self.lo).length() + 1.0;
        let normal = |t: &[Vec3; 3]| (t[1] - t[0]).cross(t[2] - t[0]).normalize_or_zero();
        // Each edge: the faces along it (their normals), and whether one is seen.
        type Corner = (i64, i64, i64);
        type Edge = (Vec3, Vec3, Vec<Vec3>, bool);
        let mut edges: HashMap<(Corner, Corner), Edge> = HashMap::new();
        for t in &self.tris {
            let n = normal(t);
            let c = ((t[0] + t[1] + t[2]) / 3.0).as_dvec3();
            let seen = n.x.abs() > 0.2 && {
                let side = glam::DVec3::X * f64::from(n.x.signum());
                self.ray(c + side * 0.01, side, reach).is_none()
            };
            for k in 0..3 {
                let (a, b) = (t[k], t[(k + 1) % 3]);
                let (ka, kb) = (key(a), key(b));
                let e = edges.entry(if ka < kb { (ka, kb) } else { (kb, ka) }).or_insert((a, b, Vec::new(), false));
                e.2.push(n);
                e.3 |= seen;
            }
        }
        let crease = 20f32.to_radians().cos();
        edges.into_values().filter(|(_, _, ns, seen)| *seen && (ns.len() == 1 || ns.iter().any(|n| n.dot(ns[0]) < crease))).map(|(a, b, _, _)| [glam::DVec2::new(f64::from(a.z), f64::from(a.y)), glam::DVec2::new(f64::from(b.z), f64::from(b.y))]).collect()
    }

    /// Where its surfaces cross the plane `x = side` (its frame): segments (z, y),
    /// a side section.
    pub fn section_x(&self, side: f64) -> Vec<[glam::DVec2; 2]> {
        self.section(0, side).into_iter().map(|[a, b]| [glam::DVec2::new(a.z, a.y), glam::DVec2::new(b.z, b.y)]).collect()
    }

    /// The segments where its triangles cross the plane where coordinate `axis` is `at`.
    fn section(&self, axis: usize, at: f64) -> Vec<[DVec3; 2]> {
        let mut out = Vec::new();
        for t in &self.tris {
            let p = t.map(|v| v.as_dvec3());
            let d = p.map(|v| v[axis] - at);
            let mut hits = Vec::with_capacity(2);
            for i in 0..3 {
                let (a, b) = (i, (i + 1) % 3);
                if (d[a] < 0.0) != (d[b] < 0.0) {
                    let s = d[a] / (d[a] - d[b]);
                    hits.push(p[a] + (p[b] - p[a]) * s);
                }
            }
            if hits.len() == 2 {
                out.push([hits[0], hits[1]]);
            }
        }
        out
    }

    pub fn len(&self) -> usize {
        self.tris.len()
    }

    pub fn is_empty(&self) -> bool {
        self.tris.is_empty()
    }

    /// Each triangle that may touch the box `lo..hi`, once.
    fn near(&self, lo: DVec3, hi: DVec3, mut f: impl FnMut([DVec3; 3])) {
        let (c0, c1) = (cell(lo.as_vec3()), cell(hi.as_vec3()));
        let mut found = Vec::new();
        for x in c0.0..=c1.0 {
            for y in c0.1..=c1.1 {
                for z in c0.2..=c1.2 {
                    if let Some(v) = self.grid.get(&(x, y, z)) {
                        found.extend_from_slice(v);
                    }
                }
            }
        }
        found.sort_unstable();
        found.dedup();
        for k in found {
            f(self.tris[k as usize].map(|p| p.as_dvec3()));
        }
    }

    /// Each surface within `r` of `c`: (out of it, how deep).
    fn contacts(&self, c: DVec3, r: f64, out: &mut Vec<(DVec3, f64)>) {
        self.near(c - DVec3::splat(r), c + DVec3::splat(r), |t| {
            let q = closest_on_triangle(c, t);
            let d = c.distance(q);
            if d < r {
                let n = if d > 1e-9 { (c - q) / d } else { (t[1] - t[0]).cross(t[2] - t[0]).normalize() };
                out.push((n, r - d));
            }
        });
    }

    /// The nearest surface along a ray (unit `dir`), within `max`: (how far, its normal facing the ray).
    /// (The grid's cells walked in the ray's order, from where it enters the
    /// bounds to where it leaves them or meets something: a long sight line
    /// through the model costs what it crosses.)
    pub fn ray(&self, from: DVec3, dir: DVec3, max: f64) -> Option<(f64, DVec3)> {
        self.ray_face(from, dir, max).map(|(d, n)| (d, if n.dot(dir) > 0.0 { -n } else { n }))
    }

    /// `ray`, with the surface's own normal (out of its front, as it's wound):
    /// facing the ray if it's met from in front.
    pub fn ray_face(&self, from: DVec3, dir: DVec3, max: f64) -> Option<(f64, DVec3)> {
        // The part of the ray within the bounds.
        let (lo, hi) = (self.lo - DVec3::splat(0.01), self.hi + DVec3::splat(0.01));
        let (mut t0, mut t1) = (0.0f64, max);
        for k in 0..3 {
            if dir[k].abs() < 1e-12 {
                if from[k] < lo[k] || from[k] > hi[k] {
                    return None;
                }
                continue;
            }
            let (a, b) = ((lo[k] - from[k]) / dir[k], (hi[k] - from[k]) / dir[k]);
            t0 = t0.max(a.min(b));
            t1 = t1.min(a.max(b));
        }
        if t0 > t1 {
            return None;
        }
        let size = f64::from(CELL);
        let start = from + dir * t0;
        let mut c = cell(start.as_vec3());
        let step = [dir.x, dir.y, dir.z].map(|d| if d > 0.0 { 1 } else { -1 });
        let next_edge = |k: usize, c: i32| f64::from(c + i32::from(step[k] > 0)) * size;
        let coords = |c: (i32, i32, i32)| [c.0, c.1, c.2];
        let mut t_max = [0.0f64; 3];
        let mut t_delta = [f64::INFINITY; 3];
        for k in 0..3 {
            if dir[k].abs() > 1e-12 {
                t_max[k] = t0 + (next_edge(k, coords(c)[k]) - start[k]) / dir[k];
                t_delta[k] = size / dir[k].abs();
            } else {
                t_max[k] = f64::INFINITY;
            }
        }
        let mut best: Option<(f64, DVec3)> = None;
        let mut seen = std::collections::HashSet::new();
        loop {
            if let Some(v) = self.grid.get(&c) {
                for &i in v {
                    if !seen.insert(i) {
                        continue;
                    }
                    let t = self.tris[i as usize].map(|p| p.as_dvec3());
                    if let Some(d) = ray_triangle(from, dir, t)
                        && d <= max
                        && best.is_none_or(|b| d < b.0)
                    {
                        let n = (t[1] - t[0]).cross(t[2] - t[0]).normalize();
                        best = Some((d, n));
                    }
                }
            }
            // On to the next cell the ray enters (done once what's met is nearer than it).
            let k = if t_max[0] <= t_max[1] && t_max[0] <= t_max[2] { 0 } else if t_max[1] <= t_max[2] { 1 } else { 2 };
            if t_max[k] > t1 || best.is_some_and(|b| b.0 <= t_max[k]) {
                return best;
            }
            match k {
                0 => c.0 += step[0],
                1 => c.1 += step[1],
                _ => c.2 += step[2],
            }
            t_max[k] += t_delta[k];
        }
    }
}

/// The point of triangle `t` nearest `p` (Ericson, Real-Time Collision Detection 5.1.5).
fn closest_on_triangle(p: DVec3, t: [DVec3; 3]) -> DVec3 {
    let [a, b, c] = t;
    let (ab, ac, ap) = (b - a, c - a, p - a);
    let (d1, d2) = (ab.dot(ap), ac.dot(ap));
    if d1 <= 0.0 && d2 <= 0.0 {
        return a;
    }
    let bp = p - b;
    let (d3, d4) = (ab.dot(bp), ac.dot(bp));
    if d3 >= 0.0 && d4 <= d3 {
        return b;
    }
    let vc = d1 * d4 - d3 * d2;
    if vc <= 0.0 && d1 >= 0.0 && d3 <= 0.0 {
        return a + ab * (d1 / (d1 - d3));
    }
    let cp = p - c;
    let (d5, d6) = (ab.dot(cp), ac.dot(cp));
    if d6 >= 0.0 && d5 <= d6 {
        return c;
    }
    let vb = d5 * d2 - d1 * d6;
    if vb <= 0.0 && d2 >= 0.0 && d6 <= 0.0 {
        return a + ac * (d2 / (d2 - d6));
    }
    let va = d3 * d6 - d5 * d4;
    if va <= 0.0 && d4 - d3 >= 0.0 && d5 - d6 >= 0.0 {
        return b + (c - b) * ((d4 - d3) / ((d4 - d3) + (d5 - d6)));
    }
    let denom = 1.0 / (va + vb + vc);
    a + ab * (vb * denom) + ac * (vc * denom)
}

/// How far along a ray (unit `dir`) it meets triangle `t`, either side (Möller–Trumbore).
fn ray_triangle(from: DVec3, dir: DVec3, t: [DVec3; 3]) -> Option<f64> {
    let (e1, e2) = (t[1] - t[0], t[2] - t[0]);
    let p = dir.cross(e2);
    let det = e1.dot(p);
    if det.abs() < 1e-12 {
        return None;
    }
    let inv = 1.0 / det;
    let s = from - t[0];
    let u = s.dot(p) * inv;
    if !(0.0..=1.0).contains(&u) {
        return None;
    }
    let q = s.cross(e1);
    let v = dir.dot(q) * inv;
    if v < 0.0 || u + v > 1.0 {
        return None;
    }
    let d = e2.dot(q) * inv;
    (d >= 0.0).then_some(d)
}

/// Something to stand on and bump into, in the walker's frame.
#[derive(Clone, Copy)]
pub enum Collider<'a> {
    /// A model's surfaces, its frame at `at` turned by `rot`.
    Mesh { mesh: &'a WalkMesh, at: DVec3, rot: DQuat },
    /// A box (a building): its centre, its turn, its half sizes.
    Box { at: DVec3, rot: DQuat, half: DVec3 },
    /// A convex solid (a ship without a walk mesh): its faces' planes
    /// (`n·p ≤ d` inside, its own frame), placed at `at` turned by `rot`, and
    /// the sphere round it (its frame).
    Convex { planes: &'a [(DVec3, f64)], at: DVec3, rot: DQuat, centre: DVec3, radius: f64 },
    /// A body's ground: the walker's frame is the body's, centred on it; how
    /// far its surface is from the centre each way.
    Ground(&'a dyn Fn(DVec3) -> f64),
}

impl Collider<'_> {
    fn contacts(&self, c: DVec3, r: f64, out: &mut Vec<(DVec3, f64)>) {
        match self {
            Collider::Mesh { mesh, at, rot } => {
                let start = out.len();
                mesh.contacts(rot.inverse() * (c - *at), r, out);
                for o in &mut out[start..] {
                    o.0 = *rot * o.0;
                }
            }
            Collider::Box { at, rot, half } => {
                let p = rot.inverse() * (c - *at);
                let q = p.clamp(-*half, *half);
                if q == p {
                    // Inside: out the nearest face.
                    let gap = *half - p.abs();
                    let axis = if gap.x <= gap.y && gap.x <= gap.z { DVec3::X } else if gap.y <= gap.z { DVec3::Y } else { DVec3::Z };
                    let n = axis * p.dot(axis).signum();
                    out.push((*rot * n, gap.dot(axis) + r));
                } else if p.distance(q) < r {
                    let d = p.distance(q);
                    out.push((*rot * ((p - q) / d), r - d));
                }
            }
            Collider::Convex { planes, at, rot, centre, radius } => {
                let p = rot.inverse() * (c - *at);
                if p.distance(*centre) > radius + r {
                    return;
                }
                let (n, s) = planes.iter().map(|&(n, d)| (n, n.dot(p) - d)).max_by(|a, b| a.1.total_cmp(&b.1)).unwrap_or((DVec3::Y, f64::MAX));
                if s < r {
                    out.push((*rot * n, r - s));
                }
            }
            Collider::Ground(_) => {}
        }
    }

    fn ray(&self, from: DVec3, dir: DVec3, max: f64) -> Option<(f64, DVec3)> {
        match self {
            Collider::Mesh { mesh, at, rot } => mesh.ray(rot.inverse() * (from - *at), rot.inverse() * dir, max).map(|(d, n)| (d, *rot * n)),
            Collider::Box { at, rot, half } => {
                let (p, v) = (rot.inverse() * (from - *at), rot.inverse() * dir);
                let (mut t0, mut t1, mut n) = (0.0f64, max, DVec3::ZERO);
                for k in 0..3 {
                    if v[k].abs() < 1e-12 {
                        if p[k].abs() > half[k] {
                            return None;
                        }
                        continue;
                    }
                    let (a, b) = ((-half[k] - p[k]) / v[k], (half[k] - p[k]) / v[k]);
                    let (near, far) = if a < b { (a, b) } else { (b, a) };
                    if near > t0 {
                        t0 = near;
                        n = DVec3::ZERO;
                        n[k] = -v[k].signum();
                    }
                    t1 = t1.min(far);
                    if t0 > t1 {
                        return None;
                    }
                }
                (n != DVec3::ZERO).then(|| (t0, *rot * n))
            }
            Collider::Convex { planes, at, rot, .. } => {
                let (p, v) = (rot.inverse() * (from - *at), rot.inverse() * dir);
                let (mut t0, mut t1, mut n) = (0.0f64, max, DVec3::ZERO);
                for &(pn, d) in planes.iter() {
                    let (dist, along) = (d - pn.dot(p), pn.dot(v));
                    if along.abs() < 1e-12 {
                        if dist < 0.0 {
                            return None;
                        }
                        continue;
                    }
                    let t = dist / along;
                    if along < 0.0 {
                        if t > t0 {
                            t0 = t;
                            n = pn;
                        }
                    } else {
                        t1 = t1.min(t);
                    }
                    if t0 > t1 {
                        return None;
                    }
                }
                (n != DVec3::ZERO).then(|| (t0, *rot * n))
            }
            Collider::Ground(radius) => {
                // (Straight down only: the feet's ray.)
                let up = from.normalize();
                if dir.dot(up) > -0.99 {
                    return None;
                }
                let h = from.length() - radius(up);
                (h <= max).then(|| (h.max(0.0), up))
            }
        }
    }
}

/// The surface nearest along a ray among `colliders`.
pub fn ray(colliders: &[Collider], from: DVec3, dir: DVec3, max: f64) -> Option<(f64, DVec3)> {
    colliders.iter().filter_map(|c| c.ray(from, dir, max)).min_by(|a, b| a.0.total_cmp(&b.0))
}

/// A walker this frame: where its feet are and how it moves (its frame's).
pub struct Walker {
    pub feet: DVec3,
    pub velocity: DVec3,
}

/// What it does: the speed it would walk at along the ground (m/s, its frame),
/// a jump's take-off speed (0: none), and how fast it would climb (m/s, up
/// positive) where there's something to climb.
pub struct Stride {
    pub wish: DVec3,
    pub jump: f64,
    pub climb: f64,
}

impl Walker {
    /// On the floor (within a hair of it)?
    pub fn grounded(&self, colliders: &[Collider], up: DVec3) -> bool {
        floor(colliders, self.feet, up, 0.05).is_some()
    }

    /// One step of `dt` s, `up` the way up at a point and `g` gravity there
    /// (m/s²); `climbable` says where there's something to climb (a ladder):
    /// there it holds on, going up or down as it would climb, not falling.
    /// Whether it stands on something after.
    pub fn step(&mut self, colliders: &[Collider], up_at: &dyn Fn(DVec3) -> DVec3, g: f64, climbable: &dyn Fn(DVec3) -> bool, stride: &Stride, dt: f64) -> bool {
        let up = up_at(self.feet);
        let mut v_up = self.velocity.dot(up);
        let mut v_side = self.velocity - up * v_up;
        let was_grounded = v_up <= 0.0 && self.grounded(colliders, up);
        let climbing = climbable(self.feet + up * 0.9);
        if climbing {
            // On a ladder: hands on it, it goes where it climbs and steps.
            v_side = stride.wish - up * stride.wish.dot(up);
            v_up = stride.climb;
        } else if was_grounded {
            // Feet on the floor steer; in the air, what it had carries.
            v_side = stride.wish - up * stride.wish.dot(up);
            v_up = if stride.jump > 0.0 { stride.jump } else { 0.0 };
        }
        if !climbing {
            v_up -= g * dt;
        }
        let motion = (v_side + up * v_up) * dt;
        let n = (motion.length() / SUBSTEP).ceil().max(1.0) as usize;
        let mut grounded = false;
        let mut contacts = Vec::new();
        for _ in 0..n {
            self.feet += motion / n as f64;
            let up = up_at(self.feet);
            // Out of walls (sideways), off ceilings (down), onto ledges it's
            // already over (up): the knee and the head.
            for _ in 0..4 {
                let mut clear = true;
                for h in [STEP + BODY_RADIUS, HEIGHT - BODY_RADIUS] {
                    contacts.clear();
                    for c in colliders {
                        c.contacts(self.feet + up * h, BODY_RADIUS, &mut contacts);
                    }
                    for &(nrm, depth) in &contacts {
                        let rise = nrm.dot(up);
                        let push = if rise > SLOPE {
                            up
                        } else if rise < -0.7 {
                            if v_up > 0.0 {
                                v_up = 0.0;
                            }
                            -up
                        } else {
                            match (nrm - up * rise).try_normalize() {
                                Some(side) => side,
                                None => continue,
                            }
                        };
                        self.feet += push * (depth / push.dot(nrm).max(0.2)).min(BODY_RADIUS);
                        let into = v_side.dot(push);
                        if into < 0.0 && push != up && push != -up {
                            v_side -= push * into;
                        }
                        clear = false;
                    }
                }
                if clear {
                    break;
                }
            }
            // The feet down on what's below: up a step, down one, or falling.
            grounded = false;
            if v_up <= 0.0
                && let Some(d) = floor(colliders, self.feet, up, if was_grounded && !climbing { SNAP } else { 0.02 })
            {
                self.feet += up * (STEP - d);
                v_up = 0.0;
                grounded = true;
            }
        }
        let up = up_at(self.feet);
        self.velocity = v_side - up * v_side.dot(up) + up * v_up;
        grounded
    }
}

/// How far below `feet + up·STEP` a floor flat enough to stand on is, if it's
/// within `STEP + below` of there.
fn floor(colliders: &[Collider], feet: DVec3, up: DVec3, below: f64) -> Option<f64> {
    let from = feet + up * STEP;
    ray(colliders, from, -up, STEP + below).filter(|&(_, n)| n.dot(up) >= SLOPE).map(|(d, _)| d)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A floor 20 m square at y = 0, a wall at x = 3, a 0.3 m step at z < -2.
    fn room() -> WalkMesh {
        let quad = |a: DVec3, b: DVec3, c: DVec3, d: DVec3| [[a, b, c], [a, c, d]];
        let mut t = Vec::new();
        t.extend(quad(DVec3::new(-10.0, 0.0, -2.0), DVec3::new(10.0, 0.0, -2.0), DVec3::new(10.0, 0.0, 10.0), DVec3::new(-10.0, 0.0, 10.0)));
        t.extend(quad(DVec3::new(-10.0, 0.3, -10.0), DVec3::new(10.0, 0.3, -10.0), DVec3::new(10.0, 0.3, -2.0), DVec3::new(-10.0, 0.3, -2.0)));
        t.extend(quad(DVec3::new(-10.0, 0.0, -2.0), DVec3::new(10.0, 0.0, -2.0), DVec3::new(10.0, 0.3, -2.0), DVec3::new(-10.0, 0.3, -2.0)));
        t.extend(quad(DVec3::new(3.0, 0.0, -10.0), DVec3::new(3.0, 0.0, 10.0), DVec3::new(3.0, 4.0, 10.0), DVec3::new(3.0, 4.0, -10.0)));
        WalkMesh::new(&t)
    }

    fn walk(mesh: &WalkMesh, from: DVec3, wish: DVec3, secs: f64) -> Walker {
        let colliders = [Collider::Mesh { mesh, at: DVec3::ZERO, rot: DQuat::IDENTITY }];
        let mut w = Walker { feet: from, velocity: DVec3::ZERO };
        for _ in 0..(secs * 60.0) as usize {
            w.step(&colliders, &|_| DVec3::Y, 9.81, &|_| false, &Stride { wish, jump: 0.0, climb: 0.0 }, 1.0 / 60.0);
        }
        w
    }

    #[test]
    fn stands_slides_along_walls_steps_up_and_falls_to_the_floor() {
        let mesh = room();
        // Dropped from a metre up: on the floor.
        let w = walk(&mesh, DVec3::new(0.0, 1.0, 5.0), DVec3::ZERO, 1.0);
        assert!(w.feet.y.abs() < 0.01, "{:?}", w.feet);
        // Into the wall at a slant: stopped a body's radius off it, still moving along it.
        let w = walk(&mesh, DVec3::new(0.0, 0.0, 5.0), DVec3::new(1.5, 0.0, 1.0), 4.0);
        assert!((w.feet.x - (3.0 - BODY_RADIUS)).abs() < 0.02 && w.feet.z > 8.0, "{:?}", w.feet);
        // Over the step: up on it.
        let w = walk(&mesh, DVec3::new(0.0, 0.0, 0.0), DVec3::new(0.0, 0.0, -1.5), 3.0);
        assert!((w.feet.y - 0.3).abs() < 0.01 && w.feet.z < -3.0, "{:?}", w.feet);
    }
}
