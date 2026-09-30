use universe_engine::glam::Vec3;
use universe_engine::WireModel;

pub struct Models {
    pub rocky: WireModel,
    pub giant: WireModel,
    pub moon: WireModel,
    pub star: WireModel,
    pub station: WireModel,
    pub ship: WireModel,
    pub gate: WireModel,
}

impl Models {
    pub fn new() -> Self {
        Self {
            rocky: WireModel::globe(12, 7, 4),
            giant: WireModel::globe(8, 13, 3), // many parallels read as cloud bands
            moon: WireModel::globe(8, 5, 4),
            star: WireModel::globe(16, 9, 3),
            station: coriolis(),
            ship: cobra(),
            gate: gate_ring(),
        }
    }
}

/// Coriolis station: a cuboctahedron (all permutations of (±1, ±1, 0)) with a
/// docking slot. Unit size; scale to ~500 m.
fn coriolis() -> WireModel {
    let mut points = Vec::new();
    for axis in 0..3 {
        for (a, b) in [(1.0, 1.0), (1.0, -1.0), (-1.0, 1.0), (-1.0, -1.0)] {
            let mut v = [0.0f32; 3];
            v[(axis + 1) % 3] = a;
            v[(axis + 2) % 3] = b;
            points.push(Vec3::from_array(v));
        }
    }
    let mut m = WireModel::convex_hull(&points);
    m.add_loop(&[Vec3::new(-0.3, 1.0, -0.08), Vec3::new(0.3, 1.0, -0.08), Vec3::new(0.3, 1.0, 0.08), Vec3::new(-0.3, 1.0, 0.08)]);
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

/// A Cobra Mk III-style trader, in meters. Forward is -Z.
fn cobra() -> WireModel {
    let s = 20.0;
    let points: Vec<Vec3> = [
        [-0.35, 0.0, -1.0],
        [0.35, 0.0, -1.0],
        [-0.35, -0.12, -0.7],
        [0.35, -0.12, -0.7],
        [-0.45, 0.28, 0.8],
        [0.45, 0.28, 0.8],
        [-1.3, -0.02, 0.8],
        [1.3, -0.02, 0.8],
        [-0.5, -0.22, 0.8],
        [0.5, -0.22, 0.8],
        [-0.3, 0.2, 0.0],
        [0.3, 0.2, 0.0],
    ]
    .iter()
    .map(|&p| Vec3::from_array(p) * s)
    .collect();
    let mut m = WireModel::convex_hull(&points);
    // Engine nozzles on the rear plate.
    for x in [-1.0f32, 1.0] {
        let (x0, x1) = (x * 0.1 * s, x * 0.35 * s);
        m.add_loop(&[
            Vec3::new(x0, -0.1 * s, 0.8 * s),
            Vec3::new(x1, -0.1 * s, 0.8 * s),
            Vec3::new(x1, 0.12 * s, 0.8 * s),
            Vec3::new(x0, 0.12 * s, 0.8 * s),
        ]);
    }
    m
}
