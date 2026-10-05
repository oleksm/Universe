//! Shapes: one geometry each, that everything reads (content:
//! `shapes.ron`; see `docs/content.md`). The physics takes its solid —
//! volume, centre of mass, inertia, silhouette, bounds — and its named
//! places: where modules mount, where built-in thrusters sit and which way
//! they fire, where the landing gear touches, the docking ports, the
//! cockpit. The renderer draws its mesh and detail lines.
//!
//! A shape is built from points (their convex hull), or — when the first
//! model arrives from Blender — from a glTF export, into the same `Shape`.
//! Metres; −Z forward, +Y up; centred on its centre of mass as loaded (what
//! it's built with is moved to match), so a body turns about it.

use glam::{DQuat, DVec3};
use serde::Deserialize;
use universe_physics::{MassProperties, Mesh};

/// What a named place on a shape is, by its name's prefix.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Role {
    /// `mount_<slot>`: where a module attaches.
    Mount,
    /// `nozzle_<name>`: a thruster built into the hull; `dir` is where its exhaust goes.
    Nozzle,
    /// `gear_<name>`: a landing contact.
    Gear,
    /// `dock_<name>`: a docking port or slot.
    Dock,
    /// `cockpit`: where the pilot sits.
    Cockpit,
    /// `hatch`: where the crew go in and out; `dir` down its ramp (straight down: a stair aft).
    Hatch,
}

impl Role {
    fn of(name: &str) -> Option<Role> {
        let roles = [("mount_", Role::Mount), ("nozzle_", Role::Nozzle), ("gear_", Role::Gear), ("dock_", Role::Dock)];
        roles.iter().find(|(p, _)| name.starts_with(p) && name.len() > p.len()).map(|r| r.1).or((name == "cockpit").then_some(Role::Cockpit))
            .or((name == "hatch").then_some(Role::Hatch))
    }
}

/// A named place on a shape (metres, in the shape's frame).
#[derive(Clone, Debug, PartialEq)]
pub struct Node {
    pub name: String,
    pub role: Role,
    pub at: DVec3,
    /// A unit direction: a nozzle's exhaust, a port's way out, the pilot's view.
    pub dir: DVec3,
}

/// A shape of the loaded content.
#[derive(Clone, Debug, PartialEq)]
pub struct Shape {
    pub key: String,
    /// Its surface (metres).
    pub mesh: Mesh,
    /// Detail lines drawn on it.
    pub loops: Vec<Vec<DVec3>>,
    pub nodes: Vec<Node>,
    /// Its solid, at a uniform density of 1.
    pub solid: MassProperties,
    /// Its shape for contact: spheres covering it (empty: a body's own radius).
    pub spheres: Vec<universe_physics::Sphere>,
    /// Its convex parts as solids (body, wings…): each the planes of its
    /// faces, `n·p ≤ d` inside (n unit, outward). For ship against ship.
    pub solids: Vec<Vec<(DVec3, f64)>>,
    /// Each convex part's points in `mesh.points` (the body first).
    pub part_points: Vec<std::ops::Range<usize>>,
    /// Each convex part's reach and probes (as `solids`).
    pub parts: Vec<Part>,
    /// Where its centre of mass was in the frame it was made in (it's
    /// centred on it since): a model made in that frame is drawn shifted by −this.
    pub made_centre: DVec3,
    /// A modelled hull's own surfaces, to walk on and bump into (its frame);
    /// none for a shape made from points.
    pub walk: Option<std::sync::Arc<crate::walk::WalkMesh>>,
    /// Its ramp, if it has one that swings down (walked on too).
    pub ramp: Option<Ramp>,
    /// A modelled hull's named parts and the box round each (its frame: least and
    /// most corners), to find places by (the ore scoop, a gear bay...).
    pub pieces: Vec<(String, DVec3, DVec3)>,
    /// Its glass and screens one by one (a mesh of windows split where its pieces
    /// don't touch): (the mesh's name, least corner, most corner), its frame.
    pub islands: Vec<(String, DVec3, DVec3)>,
}

/// A ramp hinged to a hull (its frame): it swings down about `axis` through
/// `hinge` (by a positive angle, its far end going down) till it meets the
/// ground; `length` from the hinge to its far end.
#[derive(Clone, Debug, PartialEq)]
pub struct Ramp {
    pub hinge: DVec3,
    pub axis: DVec3,
    pub length: f64,
    pub walk: std::sync::Arc<crate::walk::WalkMesh>,
}

impl Ramp {
    /// Its turn swung down by `angle` (rad): its points `p` go to `hinge + turn·(p − hinge)`.
    pub fn turn(&self, angle: f64) -> DQuat {
        DQuat::from_axis_angle(self.axis, angle)
    }
}

impl Shape {
    pub fn nodes(&self, role: Role) -> impl Iterator<Item = &Node> {
        self.nodes.iter().filter(move |n| n.role == role)
    }

    pub fn node(&self, name: &str) -> Option<&Node> {
        self.nodes.iter().find(|n| n.name == name)
    }

    /// Is point `p` (its own frame), moving at `v` (the same), inside it?
    /// If so: how deep, and out which way — the face it came in by (of
    /// those it's moving in through, the one it would back out of
    /// soonest), or failing that the one it's least deep behind.
    pub fn inside(&self, p: DVec3, v: DVec3) -> Option<(f64, DVec3)> {
        let mut best: Option<(f64, DVec3)> = None;
        for k in 0..self.solids.len() {
            if let Some((depth, n)) = self.inside_part(k, p, v)
                && best.is_none_or(|(d, _)| depth > d)
            {
                best = Some((depth, n));
            }
        }
        best
    }

    /// Like `inside`, for its convex part `k` alone.
    pub fn inside_part(&self, k: usize, p: DVec3, v: DVec3) -> Option<(f64, DVec3)> {
        // (Outside the sphere round its corners: outside it.)
        let part = &self.parts[k];
        if p.distance_squared(part.centre) > part.radius * part.radius {
            return None;
        }
        let planes = &self.solids[k];
        let out = planes.iter().map(|&(n, d)| n.dot(p) - d).fold(f64::NEG_INFINITY, f64::max);
        if out >= 0.0 {
            return None;
        }
        let came_in = planes
            .iter()
            .filter(|&&(n, _)| v.dot(n) < -1e-9)
            .map(|&(n, d)| ((d - n.dot(p)) / -v.dot(n), d - n.dot(p), n))
            .min_by(|a, b| a.0.total_cmp(&b.0))
            .map(|(_, depth, n)| (depth, n));
        let least = planes.iter().map(|&(n, d)| (d - n.dot(p), n)).min_by(|a, b| a.0.total_cmp(&b.0)).expect("a solid has faces");
        Some(came_in.unwrap_or(least))
    }
}

/// A convex part of a shape, for contact: the sphere round its corners,
/// and the points of its surface tested against another solid (its
/// corners and the middles of its edges). Its own shape's frame.
#[derive(Clone, Debug, PartialEq)]
pub struct Part {
    pub centre: DVec3,
    pub radius: f64,
    pub probes: Vec<DVec3>,
}

/// Fitted contact spheres stand this far apart across a shape's plan (m).
pub const SPHERE_SPACING: f64 = 7.0;

/// Spheres covering a convex `mesh`, for contact: on a grid `g` apart across
/// its plan (x, z), one per column through it, centred in the column's
/// height and big enough to take in its whole cell, top to bottom.
pub fn fit_spheres(mesh: &Mesh, g: f64) -> Vec<universe_physics::Sphere> {
    // The hull's faces as planes: n·p ≤ d inside.
    let planes: Vec<(DVec3, f64)> = mesh
        .faces
        .iter()
        .filter_map(|f| {
            let (a, b, c) = (mesh.points[f[0] as usize], mesh.points[f[1] as usize], mesh.points[f[2] as usize]);
            let n = (b - a).cross(c - a).try_normalize()?;
            Some((n, n.dot(a)))
        })
        .collect();
    // The hull's height along the vertical line at (x, z), if it meets it.
    let column = |x: f64, z: f64| -> Option<(f64, f64)> {
        let (mut lo, mut hi) = (f64::NEG_INFINITY, f64::INFINITY);
        for &(n, d) in &planes {
            let rest = d - n.x * x - n.z * z;
            if n.y > 1e-9 {
                hi = hi.min(rest / n.y);
            } else if n.y < -1e-9 {
                lo = lo.max(rest / n.y);
            } else if rest < 0.0 {
                return None;
            }
        }
        (hi > lo).then_some((lo, hi))
    };
    let (min, max) = mesh.extent();
    let reach = g * std::f64::consts::FRAC_1_SQRT_2;
    let mut out = Vec::new();
    let (nx, nz) = (((max.x - min.x) / g).ceil() as i64, ((max.z - min.z) / g).ceil() as i64);
    for i in 0..nx {
        for k in 0..nz {
            let (x, z) = (min.x + (i as f64 + 0.5) * g, min.z + (k as f64 + 0.5) * g);
            // The hull's height across the cell: at its centre and corners
            // (with a margin for what's between).
            let range = [(0.0, 0.0), (0.5, 0.5), (-0.5, 0.5), (0.5, -0.5), (-0.5, -0.5)]
                .iter()
                .filter_map(|&(dx, dz)| column(x + dx * g, z + dz * g))
                .fold(None, |acc: Option<(f64, f64)>, (lo, hi)| Some(acc.map_or((lo, hi), |(a, b)| (a.min(lo), b.max(hi)))));
            let Some((lo, hi)) = range else { continue };
            let half = 0.5 * (hi - lo);
            out.push(universe_physics::Sphere { at: DVec3::new(x, 0.5 * (lo + hi), z), radius: (half * half + reach * reach).sqrt() + 0.3 });
        }
    }
    out
}

type Point = (f64, f64, f64);

fn point(p: Point) -> DVec3 {
    DVec3::new(p.0, p.1, p.2)
}

/// A shape as `shapes.ron` has it.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ShapeDef {
    pub key: String,
    #[serde(default = "unit")]
    scale: f64,
    /// Built as the convex hull of these…
    #[serde(default)]
    hull: Vec<Point>,
    /// …and of each of these: more convex parts (wings, pods), joined to
    /// the first. (Where parts overlap, their solid counts twice: keep
    /// overlaps small.)
    #[serde(default)]
    parts: Vec<Vec<Point>>,
    /// Or from a glTF export (`.glb`; empty: none).
    #[serde(default)]
    gltf: String,
    #[serde(default)]
    loops: Vec<Vec<Point>>,
    #[serde(default)]
    nodes: Vec<NodeDef>,
    /// Spheres covering it, for contact: (centre, radius).
    #[serde(default)]
    spheres: Vec<(Point, f64)>,
    /// Its nodes are given about its centre of mass (laid out balanced
    /// about it, as thrusters must be), not in the frame of its points.
    #[serde(default)]
    about_centre: bool,
}

fn unit() -> f64 {
    1.0
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct NodeDef {
    name: String,
    at: Point,
    dir: Point,
}

impl ShapeDef {
    /// One made in code (a design's): its body's points, more convex
    /// parts, detail lines, and named nodes (name, at, dir), all in the
    /// frame of its points (metres).
    pub(crate) fn made(key: String, body: Vec<DVec3>, parts: Vec<Vec<DVec3>>, loops: Vec<Vec<DVec3>>, nodes: Vec<(String, DVec3, DVec3)>) -> Self {
        let p = |v: DVec3| (v.x, v.y, v.z);
        ShapeDef {
            key,
            scale: 1.0,
            hull: body.into_iter().map(p).collect(),
            parts: parts.into_iter().map(|q| q.into_iter().map(p).collect()).collect(),
            gltf: String::new(),
            loops: loops.into_iter().map(|q| q.into_iter().map(p).collect()).collect(),
            nodes: nodes.into_iter().map(|(name, at, dir)| NodeDef { name, at: p(at), dir: p(dir) }).collect(),
            spheres: Vec::new(),
            about_centre: false,
        }
    }

    /// The shape it describes, checked: a real solid, its nodes named for
    /// their roles and pointing somewhere.
    pub fn build(self) -> Result<Shape, String> {
        let s = self.scale;
        if !(s.is_finite() && s > 0.0) {
            return Err(format!("scale must be positive ({s})"));
        }
        // Each part's (points, faces) in the joined mesh.
        let mut ranges = Vec::new();
        let mesh = match (self.gltf.as_str(), self.hull.len()) {
            (file, _) if !file.is_empty() => return Err(format!("glTF shapes ({file}) arrive with the first model from Blender: give it hull points for now")),
            (_, 0..=3) => return Err("needs at least four hull points".into()),
            _ => {
                let mut mesh = Mesh::convex_hull(&self.hull.iter().map(|&p| point(p) * s).collect::<Vec<_>>());
                ranges.push((0, mesh.points.len(), 0, mesh.faces.len()));
                for (k, part) in self.parts.iter().enumerate() {
                    if part.len() < 4 {
                        return Err(format!("part {} needs at least four points", k + 1));
                    }
                    let m = Mesh::convex_hull(&part.iter().map(|&p| point(p) * s).collect::<Vec<_>>());
                    if m.mass_properties().volume <= 0.0 {
                        return Err(format!("part {} has no volume (its points are flat)", k + 1));
                    }
                    let base = mesh.points.len() as u32;
                    ranges.push((mesh.points.len(), mesh.points.len() + m.points.len(), mesh.faces.len(), mesh.faces.len() + m.faces.len()));
                    mesh.points.extend(m.points);
                    mesh.faces.extend(m.faces.iter().map(|f| f.map(|i| i + base)));
                    mesh.edges.extend(m.edges.iter().map(|e| e.map(|i| i + base)));
                }
                mesh
            }
        };
        // (Each part closed, the mesh's solid is theirs together.)
        let mut solid = mesh.mass_properties();
        if solid.volume <= 0.0 {
            return Err("its hull has no volume (the points are flat)".into());
        }
        // Centred on its centre of mass: what moves it turns about it.
        let c = solid.centroid;
        let mut mesh = mesh;
        for p in &mut mesh.points {
            *p -= c;
        }
        solid.centroid = DVec3::ZERO;
        let mut nodes = Vec::new();
        for n in self.nodes {
            let role = Role::of(&n.name).ok_or_else(|| format!("node '{}': name it mount_*, nozzle_*, gear_*, dock_*, cockpit or hatch", n.name))?;
            let dir = point(n.dir).try_normalize().ok_or_else(|| format!("node '{}' points nowhere", n.name))?;
            if nodes.iter().any(|m: &Node| m.name == n.name) {
                return Err(format!("node '{}' twice", n.name));
            }
            let shift = if self.about_centre { DVec3::ZERO } else { c };
            nodes.push(Node { name: n.name, role, at: point(n.at) * s - shift, dir });
        }
        let loops = self.loops.into_iter().map(|l| l.into_iter().map(|p| point(p) * s - c).collect()).collect();
        let mut spheres = Vec::new();
        for (at, r) in self.spheres {
            if !(r.is_finite() && r > 0.0) {
                return Err(format!("a contact sphere's radius must be positive ({r})"));
            }
            spheres.push(universe_physics::Sphere { at: point(at) * s - c, radius: r * s });
        }
        // Each part as a solid: its faces' planes.
        let solids: Vec<Vec<(DVec3, f64)>> = ranges
            .iter()
            .map(|&(_, _, f0, f1)| {
                mesh.faces[f0..f1]
                    .iter()
                    .filter_map(|f| {
                        let (a, b, c) = (mesh.points[f[0] as usize], mesh.points[f[1] as usize], mesh.points[f[2] as usize]);
                        let n = (b - a).cross(c - a).try_normalize()?;
                        Some((n, n.dot(a)))
                    })
                    .collect()
            })
            .collect();
        if spheres.is_empty() {
            // Fitted to each part (its own convex hull) in turn.
            for &(p0, p1, f0, f1) in &ranges {
                let part = Mesh { points: mesh.points[p0..p1].to_vec(), faces: mesh.faces[f0..f1].iter().map(|f| f.map(|i| i - p0 as u32)).collect(), edges: Vec::new() };
                spheres.extend(fit_spheres(&part, SPHERE_SPACING));
            }
        }
        let part_points = ranges.iter().map(|&(p0, p1, _, _)| p0..p1).collect();
        let parts = ranges
            .iter()
            .map(|&(p0, p1, _, _)| {
                let points = &mesh.points[p0..p1];
                let centre = points.iter().sum::<DVec3>() / points.len() as f64;
                let radius = points.iter().map(|p| p.distance(centre)).fold(0.0, f64::max);
                let mut probes = points.to_vec();
                let mine = |e: &[u32; 2]| (p0..p1).contains(&(e[0] as usize));
                probes.extend(mesh.edges.iter().filter(|e| mine(e)).map(|e| (mesh.points[e[0] as usize] + mesh.points[e[1] as usize]) * 0.5));
                Part { centre, radius, probes }
            })
            .collect();
        Ok(Shape { key: self.key, mesh, loops, nodes, solid, spheres, solids, part_points, parts, made_centre: c, walk: None, ramp: None, pieces: Vec::new(), islands: Vec::new() })
    }
}
