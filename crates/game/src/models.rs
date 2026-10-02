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
    /// Each hull's navigation lights (shape frame): port, starboard, the tail strobe.
    pub hull_lights: Vec<[universe_engine::glam::DVec3; 3]>,
    pub gate: Mesh,
}

impl Models {
    pub fn new() -> Self {
        Self {
            rocky: Mesh::new(WireModel::globe(12, 7, 4)),
            giant: Mesh::new(WireModel::globe(8, 13, 3)), // many parallels read as cloud bands
            moon: Mesh::new(WireModel::globe(8, 5, 4)),
            star: Mesh::new(WireModel::globe(16, 9, 3)),
            station: Mesh::new(platform()),
            hulls: content().hulls.iter().map(|(_, h)| Mesh::new(wire(h.shape()))).collect(),
            hull_lights: content().hulls.iter().map(|(_, h)| nav_lights(h.shape())).collect(),
            gate: Mesh::new(gate_ring()),
        }
    }
}

impl Models {
    /// A ship's model: its hull's.
    pub fn hull(&self, ship: &universe_sim::world::Ship) -> &Mesh {
        &self.hulls[ship.class.index()]
    }

    /// A ship's navigation lights (see `nav_lights`).
    pub fn lights(&self, ship: &universe_sim::world::Ship) -> [universe_engine::glam::DVec3; 3] {
        self.hull_lights.get(ship.class.index()).copied().unwrap_or_default()
    }
}

/// Where a hull's navigation lights go: the port and starboard tips (its
/// farthest points left and right), and the strobe at its top rear.
fn nav_lights(s: &Shape) -> [universe_engine::glam::DVec3; 3] {
    let pts = &s.mesh.points;
    let pick = |key: &dyn Fn(&universe_engine::glam::DVec3) -> f64| pts.iter().copied().max_by(|a, b| key(a).total_cmp(&key(b))).unwrap_or_default();
    [pick(&|p| -p.x), pick(&|p| p.x), pick(&|p| p.z + 0.5 * p.y)]
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

/// The platform station, in metres (its own frame, see `world::station`):
/// the deck and the main structure as boxes (their faces hide what's
/// behind), the pads marked on the deck, the hangar door and a band of
/// windows on the structure's face.
fn platform() -> WireModel {
    use universe_sim::world::station::{hull, pad_local, DECK_FROM, DECK_HALF, DECK_TOP};
    let mut m = WireModel::default();
    for &(lo, hi) in &hull().boxes {
        let (lo, hi) = (lo.as_vec3(), hi.as_vec3());
        let base = m.positions.len() as u32;
        for k in 0..8 {
            m.positions.push(Vec3::new(if k & 1 == 0 { lo.x } else { hi.x }, if k & 2 == 0 { lo.y } else { hi.y }, if k & 4 == 0 { lo.z } else { hi.z }));
        }
        // Each face: the corners with that coordinate at its limit.
        for (axis, bit) in [(0usize, 1u32), (1, 2), (2, 4)] {
            for high in [false, true] {
                let corners: Vec<u32> = (0..8u32).filter(|k| (k & bit != 0) == high).map(|k| base + k).collect();
                let mut normal = Vec3::ZERO;
                normal[axis] = if high { 1.0 } else { -1.0 };
                let ring = m.add_polygon(&corners, normal);
                for w in 0..4 {
                    m.edges.push([ring[w], ring[(w + 1) % 4]]);
                }
            }
        }
    }
    let deck = DECK_TOP as f32 + 0.5;
    for k in 0..universe_sim::world::spaceport::PADS {
        let c = pad_local(k).as_vec3();
        let h = 35.0;
        m.add_loop(&[Vec3::new(c.x - h, deck, c.z - h), Vec3::new(c.x + h, deck, c.z - h), Vec3::new(c.x + h, deck, c.z + h), Vec3::new(c.x - h, deck, c.z + h)]);
    }
    // The hangar door, and a band of windows above it, on the face over the deck.
    let face = DECK_FROM as f32 + 0.5;
    let top = DECK_TOP as f32;
    m.add_loop(&[Vec3::new(-90.0, top, face), Vec3::new(90.0, top, face), Vec3::new(90.0, top + 60.0, face), Vec3::new(-90.0, top + 60.0, face)]);
    let w = DECK_HALF as f32 - 30.0;
    m.add_loop(&[Vec3::new(-w, top + 150.0, face), Vec3::new(w, top + 150.0, face), Vec3::new(w, top + 175.0, face), Vec3::new(-w, top + 175.0, face)]);
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

#[cfg(test)]
mod tests {
    use super::*;

    /// Every face of `m` faces out of the convex piece holding it (the
    /// piece found by `piece`, the centre of the face's own solid).
    fn outward(m: &WireModel, what: &str, centre: impl Fn(Vec3) -> Vec3) {
        for f in &m.faces {
            let [a, b, c] = f.map(|i| m.positions[i as usize]);
            let mid = (a + b + c) / 3.0;
            assert!((b - a).cross(c - a).dot(mid - centre(mid)) > 0.0, "{what}: a face wound inward at {mid}");
        }
    }

    #[test]
    fn every_model_winds_its_faces_outward() {
        // The station: each face out of its own box.
        let boxes = universe_sim::world::station::hull().boxes;
        outward(&platform(), "station", |p| {
            let (lo, hi) = boxes.iter().map(|&(lo, hi)| (lo.as_vec3(), hi.as_vec3())).find(|(lo, hi)| p.cmpge(*lo - 0.01).all() && p.cmple(*hi + 0.01).all()).expect("on a box");
            (lo + hi) / 2.0
        });
        // The gate: out of its tube.
        let r = universe_sim::world::gate::GATE_RADIUS as f32;
        outward(&gate_ring(), "gate", |p| Vec3::new(p.x, 0.0, p.z).normalize() * r);
        // Every hull: each face out of its own part.
        for (_, h) in content().hulls.iter() {
            let s = h.shape();
            for f in &s.mesh.faces {
                let [a, b, c] = f.map(|i| s.mesh.points[i as usize]);
                let n = (b - a).cross(c - a);
                // Its part: the solid whose plane it lies in.
                let plane = s.solids.iter().flatten().find(|(pn, d)| (pn.dot(a) - d).abs() < 1e-3 && (pn.dot(b) - d).abs() < 1e-3 && (pn.dot(c) - d).abs() < 1e-3);
                let (pn, _) = plane.unwrap_or_else(|| panic!("{}: a face on no part's plane", h.key));
                assert!(n.dot(*pn) > 0.0, "{}: a face wound inward", h.key);
            }
        }
    }
}
