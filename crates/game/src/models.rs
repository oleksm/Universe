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

/// The platform station's boxes, in metres (its own frame).
fn station_boxes() -> Vec<(Vec3, Vec3)> {
    use universe_sim::world::station::{hull, DECK_FROM, DECK_HALF, DECK_TOP, STRUCTURE_FROM, STRUCTURE_TOP};
    // Its blocks (what's solid to a ship), then what's built on them:
    // the control tower over the deck, the roof's machinery and masts,
    // radiators out from the structure's ends, a rim round the deck, beams
    // under it. (Detail: a ship meets the blocks, not these.)
    let mut boxes: Vec<(Vec3, Vec3)> = hull().boxes.iter().map(|&(lo, hi)| (lo.as_vec3(), hi.as_vec3())).collect();
    let (top, front, back, half, deck) = (STRUCTURE_TOP as f32, DECK_FROM as f32, STRUCTURE_FROM as f32, DECK_HALF as f32, DECK_TOP as f32);
    let bx = |x0: f32, y0: f32, z0: f32, x1: f32, y1: f32, z1: f32| (Vec3::new(x0, y0, z0), Vec3::new(x1, y1, z1));
    // The control tower, at the front corner.
    boxes.push(bx(170.0, top, front - 80.0, 290.0, top + 70.0, front - 4.0));
    boxes.push(bx(185.0, top + 70.0, front - 70.0, 275.0, top + 82.0, front - 14.0));
    // Roof machinery: a long low tier and plant boxes.
    boxes.push(bx(-260.0, top, back + 20.0, 120.0, top + 22.0, front - 30.0));
    for k in 0..4 {
        let x = -240.0 + k as f32 * 90.0;
        boxes.push(bx(x, top + 22.0, back + 40.0, x + 50.0, top + 40.0, back + 80.0));
    }
    // Masts, and their cross-arms.
    for (x, z, h) in [(-200.0, back + 110.0, 150.0), (60.0, back + 120.0, 110.0), (230.0, front - 40.0, 70.0)] {
        boxes.push(bx(x - 2.0, top + 22.0, z - 2.0, x + 2.0, top + 22.0 + h, z + 2.0));
        boxes.push(bx(x - 18.0, top + 10.0 + h, z - 1.0, x + 18.0, top + 13.0 + h, z + 1.0));
    }
    // Radiator fins out from each end.
    for side in [-1.0f32, 1.0] {
        for k in 0..5 {
            let z = back + 15.0 + k as f32 * 28.0;
            let (x0, x1) = if side > 0.0 { (half, half + 110.0) } else { (-half - 110.0, -half) };
            boxes.push(bx(x0, deck - 30.0, z, x1, top - 30.0, z + 4.0));
        }
    }
    // A low rim round the deck's open edges.
    let far = universe_sim::world::station::DECK_TO as f32;
    boxes.push(bx(-half, deck, far - 3.0, half, deck + 4.0, far));
    boxes.push(bx(-half, deck, front, -half + 3.0, deck + 4.0, far));
    boxes.push(bx(half - 3.0, deck, front, half, deck + 4.0, far));
    // Beams under the deck, along and across.
    let under = -150.0f32;
    for k in 0..5 {
        let x = -240.0 + k as f32 * 120.0;
        boxes.push(bx(x - 5.0, under - 18.0, front, x + 5.0, under, far - 10.0));
    }
    for k in 0..4 {
        let z = front + 40.0 + k as f32 * 150.0;
        boxes.push(bx(-half + 20.0, under - 14.0, z - 4.0, half - 20.0, under - 4.0, z + 4.0));
    }
    boxes
}

/// The platform station, in metres (its own frame, see `world::station`):
/// the deck and the main structure as boxes (their faces hide what's
/// behind), the pads marked on the deck, the hangar door and a band of
/// windows on the structure's face.
fn platform() -> WireModel {
    use universe_sim::world::station::{pad_local, DECK_FROM, DECK_HALF, DECK_TOP};
    let mut m = WireModel::default();
    let boxes = station_boxes();
    for &(lo, hi) in &boxes {
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
        // (A face may lie on two boxes, one sitting on another: out of either.)
        let boxes = station_boxes();
        let m = platform();
        for f in &m.faces {
            let [a, b, c] = f.map(|i| m.positions[i as usize]);
            let (mid, n) = ((a + b + c) / 3.0, (b - a).cross(c - a));
            let on = |&&(lo, hi): &&(Vec3, Vec3)| mid.cmpge(lo - 0.01).all() && mid.cmple(hi + 0.01).all();
            assert!(boxes.iter().filter(on).any(|&(lo, hi)| n.dot(mid - (lo + hi) / 2.0) > 0.0), "station: a face wound inward at {mid}");
        }
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
