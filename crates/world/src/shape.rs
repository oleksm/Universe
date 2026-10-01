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

use glam::DVec3;
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
}

impl Role {
    fn of(name: &str) -> Option<Role> {
        let roles = [("mount_", Role::Mount), ("nozzle_", Role::Nozzle), ("gear_", Role::Gear), ("dock_", Role::Dock)];
        roles.iter().find(|(p, _)| name.starts_with(p) && name.len() > p.len()).map(|r| r.1).or((name == "cockpit").then_some(Role::Cockpit))
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
}

impl Shape {
    pub fn nodes(&self, role: Role) -> impl Iterator<Item = &Node> {
        self.nodes.iter().filter(move |n| n.role == role)
    }

    pub fn node(&self, name: &str) -> Option<&Node> {
        self.nodes.iter().find(|n| n.name == name)
    }
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
    /// Built as the convex hull of these.
    #[serde(default)]
    hull: Vec<Point>,
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
    /// The shape it describes, checked: a real solid, its nodes named for
    /// their roles and pointing somewhere.
    pub fn build(self) -> Result<Shape, String> {
        let s = self.scale;
        if !(s.is_finite() && s > 0.0) {
            return Err(format!("scale must be positive ({s})"));
        }
        let mesh = match (self.gltf.as_str(), self.hull.len()) {
            (file, _) if !file.is_empty() => return Err(format!("glTF shapes ({file}) arrive with the first model from Blender: give it hull points for now")),
            (_, 0..=3) => return Err("needs at least four hull points".into()),
            _ => Mesh::convex_hull(&self.hull.iter().map(|&p| point(p) * s).collect::<Vec<_>>()),
        };
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
            let role = Role::of(&n.name).ok_or_else(|| format!("node '{}': name it mount_*, nozzle_*, gear_*, dock_* or cockpit", n.name))?;
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
        if spheres.is_empty() {
            spheres = fit_spheres(&mesh, SPHERE_SPACING);
        }
        Ok(Shape { key: self.key, mesh, loops, nodes, solid, spheres })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::content::content;

    #[test]
    fn the_cobra_is_a_solid_with_its_nozzles_and_cockpit() {
        let cobra = crate::ship::cobra().shape();
        let (lo, hi) = cobra.mesh.extent();
        assert!((hi.x - lo.x - 52.0).abs() < 1e-9, "52 m across the wings: {}", hi.x - lo.x);
        assert!(cobra.solid.volume > 1_000.0 && cobra.solid.volume < 20_000.0, "{} m³", cobra.solid.volume);
        let mains: Vec<_> = cobra.nodes(Role::Nozzle).filter(|n| n.name.starts_with("nozzle_main")).collect();
        assert_eq!(mains.len(), 2);
        assert!(mains.iter().all(|n| n.dir == DVec3::Z && (n.at.z - hi.z).abs() < 1e-9), "the main drive on the rear plate, firing aft");
        assert!(cobra.solid.centroid.length() < 1e-9 && cobra.mesh.mass_properties().centroid.length() < 1e-6, "centred on its centre of mass");
        assert_eq!(cobra.nodes(Role::Nozzle).count(), 18, "and the thruster quads and belly lift");
        assert!(cobra.node("cockpit").is_some());
        // (Its collision is still the hull's 12 m sphere: the shape takes over in T1.)
        assert!(crate::ship::cobra().radius < cobra.mesh.bound());
    }

    #[test]
    fn the_cobras_contact_spheres_cover_its_hull() {
        let cobra = crate::ship::cobra().shape();
        let s = &cobra.spheres;
        // Its surface, sampled: corners, edge middles, and points across each face.
        let mut samples: Vec<DVec3> = cobra.mesh.points.clone();
        for e in &cobra.mesh.edges {
            samples.push((cobra.mesh.points[e[0] as usize] + cobra.mesh.points[e[1] as usize]) * 0.5);
        }
        for f in &cobra.mesh.faces {
            let (a, b, c) = (cobra.mesh.points[f[0] as usize], cobra.mesh.points[f[1] as usize], cobra.mesh.points[f[2] as usize]);
            for (u, v) in [(1.0 / 3.0, 1.0 / 3.0), (0.6, 0.2), (0.2, 0.6), (0.2, 0.2)] {
                samples.push(a + (b - a) * u + (c - a) * v);
            }
        }
        let worst = samples.iter().map(|p| s.iter().map(|q| p.distance(q.at) - q.radius).fold(f64::INFINITY, f64::min)).fold(f64::NEG_INFINITY, f64::max);
        let fattest = s.iter().map(|q| q.radius).fold(0.0, f64::max);
        eprintln!("{} spheres, the largest {fattest:.1} m; the surface's worst point {worst:+.2} m from one", s.len());
        assert!(worst <= 0.0, "a point of the hull outside every sphere by {worst:.2} m");
        assert!(s.len() < 50, "{} spheres", s.len());
    }

    fn def(source: &str) -> ShapeDef {
        ron::from_str(source).unwrap()
    }

    #[test]
    fn a_shape_that_is_not_a_solid_or_misnames_its_places_is_refused() {
        let flat = def(r#"(key: "s", hull: [(0,0,0), (1,0,0), (0,1,0), (1,1,0)])"#);
        assert!(flat.build().unwrap_err().contains("no volume"));
        let tet = r#"key: "s", hull: [(0,0,0), (1,0,0), (0,1,0), (0,0,1)]"#;
        let bad = def(&format!(r#"({tet}, nodes: [(name: "thruster_1", at: (0,0,0), dir: (0,0,1))])"#));
        assert!(bad.build().unwrap_err().contains("nozzle_"));
        let nowhere = def(&format!(r#"({tet}, nodes: [(name: "nozzle_1", at: (0,0,0), dir: (0,0,0))])"#));
        assert!(nowhere.build().unwrap_err().contains("points nowhere"));
        let gltf = def(r#"(key: "s", gltf: "ship.glb")"#);
        assert!(gltf.build().unwrap_err().contains("Blender"));
        assert!(content().shapes.len() >= 2);
    }
}
