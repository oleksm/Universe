use universe_engine::glam::Vec3;
use universe_engine::{Mesh, WireModel};
use universe_sim::world::content::content;
use universe_sim::world::shape::Shape;

pub struct Models {
    pub rocky: Mesh,
    pub giant: Mesh,
    pub moon: Mesh,
    pub star: Mesh,
    pub station: Mesh,
    /// Each hull's, in the content's order (see `hull`).
    pub hulls: Vec<Mesh>,
    pub gate: Mesh,
}

impl Models {
    pub fn new() -> Self {
        Self {
            rocky: Mesh::new(WireModel::globe(12, 7, 4)),
            giant: Mesh::new(WireModel::globe(8, 13, 3)), // many parallels read as cloud bands
            moon: Mesh::new(WireModel::globe(8, 5, 4)),
            star: Mesh::new(WireModel::globe(16, 9, 3)),
            station: Mesh::new(wire(shape("shape.coriolis"))),
            hulls: content().hulls.iter().map(|(_, h)| Mesh::new(wire(h.shape()))).collect(),
            gate: Mesh::new(gate_ring()),
        }
    }
}

impl Models {
    /// A ship's model: its hull's.
    pub fn hull(&self, ship: &universe_sim::world::Ship) -> &Mesh {
        &self.hulls[ship.class.index()]
    }
}

fn shape(key: &str) -> &'static Shape {
    let c = content();
    c.get(c.handle(key).unwrap_or_else(|| panic!("the content has no shape '{key}'")))
}

/// A shape as the renderer draws it: its mesh (faces hide what's behind,
/// edges are drawn) and its detail lines.
pub fn wire(s: &Shape) -> WireModel {
    let v = |p: universe_engine::glam::DVec3| p.as_vec3();
    let mut m = WireModel { positions: s.mesh.points.iter().map(|&p| v(p)).collect(), edges: s.mesh.edges.clone(), faces: s.mesh.faces.clone(), colors: Vec::new() };
    for l in &s.loops {
        m.add_loop(&l.iter().map(|&p| v(p)).collect::<Vec<Vec3>>());
    }
    m
}

/// A ring gate, in meters: a square-section ring in the XZ plane (axis +Y),
/// with struts every few segments.
fn gate_ring() -> WireModel {
    use universe_sim::world::gate::{GATE_RADIUS, RING_TUBE};
    let (r, t) = (GATE_RADIUS as f32, RING_TUBE as f32);
    let n = 48u32;
    let section = [(-t, -t), (t, -t), (t, t), (-t, t)]; // (radial, axial) corners
    let mut m = WireModel::default();
    for k in 0..n {
        let a = k as f32 / n as f32 * std::f32::consts::TAU;
        let dir = Vec3::new(a.cos(), 0.0, a.sin());
        for (dr, dy) in section {
            m.positions.push(dir * (r + dr) + Vec3::Y * dy);
        }
    }
    let v = |k: u32, s: u32| (k % n) * 4 + s % 4;
    for k in 0..n {
        for s in 0..4 {
            m.faces.push([v(k, s), v(k, s + 1), v(k + 1, s + 1)]);
            m.faces.push([v(k, s), v(k + 1, s + 1), v(k + 1, s)]);
            m.edges.push([v(k, s), v(k + 1, s)]);
            if k % 4 == 0 {
                m.edges.push([v(k, s), v(k, s + 1)]);
            }
        }
    }
    m
}
