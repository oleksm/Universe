//! Shapes: one geometry each, that everything reads (content:
//! `shapes.ron`; see `docs/content.md`). The physics takes its solid —
//! volume, centre of mass, inertia, silhouette, bounds — and its named
//! places: where modules mount, where built-in thrusters sit and which way
//! they fire, where the landing gear touches, the docking ports, the
//! cockpit. The renderer draws its mesh and detail lines.
//!
//! A shape is built from points (their convex hull), or — when the first
//! model arrives from Blender — from a glTF export, into the same `Shape`.
//! Metres; −Z forward, +Y up.

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
}

impl Shape {
    pub fn nodes(&self, role: Role) -> impl Iterator<Item = &Node> {
        self.nodes.iter().filter(move |n| n.role == role)
    }

    pub fn node(&self, name: &str) -> Option<&Node> {
        self.nodes.iter().find(|n| n.name == name)
    }
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
        let solid = mesh.mass_properties();
        if solid.volume <= 0.0 {
            return Err("its hull has no volume (the points are flat)".into());
        }
        let mut nodes = Vec::new();
        for n in self.nodes {
            let role = Role::of(&n.name).ok_or_else(|| format!("node '{}': name it mount_*, nozzle_*, gear_*, dock_* or cockpit", n.name))?;
            let dir = point(n.dir).try_normalize().ok_or_else(|| format!("node '{}' points nowhere", n.name))?;
            if nodes.iter().any(|m: &Node| m.name == n.name) {
                return Err(format!("node '{}' twice", n.name));
            }
            nodes.push(Node { name: n.name, role, at: point(n.at) * s, dir });
        }
        let loops = self.loops.into_iter().map(|l| l.into_iter().map(|p| point(p) * s).collect()).collect();
        Ok(Shape { key: self.key, mesh, loops, nodes, solid })
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
        let nozzles: Vec<_> = cobra.nodes(Role::Nozzle).collect();
        assert_eq!(nozzles.len(), 2);
        assert!(nozzles.iter().all(|n| n.dir == DVec3::Z && (n.at.z - 16.0).abs() < 1e-9), "on the rear plate, firing aft");
        assert!(cobra.node("cockpit").is_some());
        // (Its collision is still the hull's 12 m sphere: the shape takes over in T1.)
        assert!(crate::ship::cobra().radius < cobra.mesh.bound());
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
