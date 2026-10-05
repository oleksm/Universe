use universe_engine::glam::{DVec3, Vec3};
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
    /// Each hull in each paint scheme (`SCHEMES`), [hull][scheme].
    pub painted: Vec<Vec<Mesh>>,
    /// Each hull's detail: its engine bells (dark metal), its canopy (glass).
    pub hull_bells: Vec<Mesh>,
    pub hull_glass: Vec<Mesh>,
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
            hulls: content().hulls.iter().map(|(_, h)| Mesh::new(bevelled(h.shape()))).collect(),
            painted: content().hulls.iter().map(|(_, h)| SCHEMES.iter().map(|s| Mesh::new(painted(h.shape(), s))).collect()).collect(),
            hull_lights: content().hulls.iter().map(|(_, h)| nav_lights(h.shape())).collect(),
            hull_bells: content().hulls.iter().map(|(_, h)| Mesh::new(bells(h))).collect(),
            hull_glass: content().hulls.iter().map(|(_, h)| Mesh::new(canopy(h.shape()))).collect(),
            gate: Mesh::new(gate_ring()),
        }
    }
}

/// A hull's inside as laid out (the shipyard's studio), as one mesh in its
/// frame: floors and walls (trimmed to the hull) in flat shades, edges drawn.
/// Unlit: inside the hull it's in the hull's shadow, and there are no lamps
/// yet. Made again when the plan changes.
/// Walls as a mesh to draw in our hull's frame (the interior studio's walled tubes,
/// walked through): each triangle in its colour (floor, wall, ceiling, panel by
/// panel), each panel's seams drawn. Made again when they change.
pub fn walls(faces: &[crate::interior::WallFace]) -> Option<Mesh> {
    use std::sync::Mutex;
    type Faces = Vec<crate::interior::WallFace>;
    static BUILT: Mutex<Option<(Faces, Mesh)>> = Mutex::new(None);
    if faces.is_empty() {
        return None;
    }
    let mut built = BUILT.lock().unwrap_or_else(|e| e.into_inner());
    if let Some((f, m)) = built.as_ref()
        && f.as_slice() == faces
    {
        return Some(m.clone());
    }
    let mut m = WireModel::default();
    for (t, c, seams) in faces {
        let base = m.positions.len() as u32;
        m.positions.extend(t.iter().map(|p| p.as_vec3()));
        m.colors.extend([*c; 3]);
        m.faces.push([base, base + 1, base + 2]);
        // (Its panel's seams, darker by the line tint against the panel's shade.)
        for (k, seam) in seams.iter().enumerate() {
            if *seam {
                m.edges.push([base + k as u32, base + (k as u32 + 1) % 3]);
            }
        }
    }
    let mesh = Mesh::new(m);
    *built = Some((faces.to_vec(), mesh.clone()));
    Some(mesh)
}

pub fn layout(plan: &universe_sim::world::deckplan::DeckPlan, shape: &universe_sim::world::shape::Shape) -> Option<Mesh> {
    use std::sync::Mutex;
    use universe_sim::world::deckplan;
    static BUILT: Mutex<Option<(deckplan::DeckPlan, Mesh)>> = Mutex::new(None);
    let mut built = BUILT.lock().unwrap_or_else(|e| e.into_inner());
    if let Some((p, m)) = built.as_ref()
        && p == plan
    {
        return Some(m.clone());
    }
    let walk = shape.walk.as_ref()?;
    let sides: Vec<_> = plan.decks.iter().map(|d| deckplan::deck_sides(walk, d.floor)).collect();
    let b = deckplan::build(plan, &sides);
    let mut m = WireModel::default();
    for (quad, floor) in &b.panels {
        // (Walls across the ship a shade lighter than those along it: the room reads.)
        let across = (quad[1] - quad[0]).cross(quad[2] - quad[0]).normalize_or_zero().z.abs() > 0.5;
        let shade = if *floor { 0.44 } else if across { 0.34 } else { 0.28 };
        let c = [shade * 0.95, shade, shade * 1.08, 1.0];
        let base = m.positions.len() as u32;
        m.positions.extend(quad.iter().map(|p| p.as_vec3()));
        m.colors.extend([c; 4]);
        m.faces.push([base, base + 1, base + 2]);
        m.faces.push([base, base + 2, base + 3]);
        // (Floors in strips: edges only on walls, or the floor's a mesh of lines.)
        if !*floor {
            for k in 0..4u32 {
                m.edges.push([base + k, base + (k + 1) % 4]);
            }
        }
    }
    // The floors' slabs: their undersides and edges, a shade under the floor, unlined.
    for quad in &b.slabs {
        let c = [0.36 * 0.95, 0.36, 0.36 * 1.08, 1.0];
        let base = m.positions.len() as u32;
        m.positions.extend(quad.iter().map(|p| p.as_vec3()));
        m.colors.extend([c; 4]);
        m.faces.push([base, base + 1, base + 2]);
        m.faces.push([base, base + 2, base + 3]);
    }
    let mesh = Mesh::new(m);
    *built = Some((plan.clone(), mesh.clone()));
    Some(mesh)
}

/// An imported hull's model (its glTF file), loaded once and kept; its
/// `*Ramp*` meshes part 1 (drawn swung down: see `scene::hull`).
pub fn pbr(path: &str) -> Option<universe_engine::PbrModel> {
    use std::collections::HashMap;
    use std::sync::{Mutex, OnceLock};
    static LOADED: OnceLock<Mutex<HashMap<String, Option<universe_engine::PbrModel>>>> = OnceLock::new();
    let mut loaded = LOADED.get_or_init(Default::default).lock().unwrap_or_else(|e| e.into_inner());
    loaded
        .entry(path.to_string())
        .or_insert_with(|| {
            let r = std::fs::read(path).map_err(|e| e.to_string()).and_then(|b| universe_engine::PbrModel::load_gltf_parts(&b, &["Ramp"]));
            r.map_err(|e| log::warn!("model {path}: {e}")).ok()
        })
        .clone()
}

impl Models {
    /// A ship's model in paint scheme `scheme` (see `SCHEMES`).
    pub fn painted(&self, ship: &universe_sim::world::Ship, scheme: usize) -> &Mesh {
        let k = ship.class.index().min(self.painted.len() - 1);
        &self.painted[k][scheme.min(SCHEMES.len() - 1)]
    }

    /// A ship's model: its hull's.
    pub fn hull(&self, ship: &universe_sim::world::Ship) -> &Mesh {
        &self.hulls[ship.class.index()]
    }

    /// A ship's engine bells and canopy (see `bells`, `canopy`).
    pub fn detail(&self, ship: &universe_sim::world::Ship) -> (&Mesh, &Mesh) {
        let k = ship.class.index().min(self.hull_bells.len() - 1);
        (&self.hull_bells[k], &self.hull_glass[k])
    }

    /// A ship's navigation lights (see `nav_lights`).
    pub fn lights(&self, ship: &universe_sim::world::Ship) -> [universe_engine::glam::DVec3; 3] {
        self.hull_lights.get(ship.class.index()).copied().unwrap_or_default()
    }
}

/// A hull's engine bells: at each main nozzle a cone flaring out along the
/// exhaust (a ring of faces, open at the mouth), sized by its thrust.
fn bells(h: &universe_sim::world::ship::ClassSpec) -> WireModel {
    use universe_sim::world::ship::ThrusterRole;
    let mut m = WireModel::default();
    let mains: Vec<&universe_sim::world::ship::Thruster> = h.thrusters.iter().filter(|t| t.role == ThrusterRole::Main).collect();
    // (No bell wider than leaves a gap to the next one.)
    let room = mains.iter().flat_map(|a| mains.iter().filter(move |b| !std::ptr::eq(*a, **b)).map(move |b| a.at.distance(b.at))).fold(f64::INFINITY, f64::min);
    for t in mains {
        let out = (-t.push).normalize().as_vec3();
        let mouth = (0.0026 * t.thrust.sqrt()).min(room * 0.44) as f32;
        let (throat, length) = (mouth * 0.55, mouth * 1.4);
        let a = out.any_orthonormal_vector();
        let b = out.cross(a);
        let at = t.at.as_vec3() - out * (length * 0.35);
        let n = 14;
        let base = m.positions.len() as u32;
        for k in 0..n {
            let ang = k as f32 / n as f32 * std::f32::consts::TAU;
            let r = a * ang.cos() + b * ang.sin();
            m.positions.push(at + r * throat);
            m.positions.push(at + out * length + r * mouth);
            // (Heat-tinted metal: bronze at the throat, blued steel at the lip.)
            m.colors.push([0.66, 0.46, 0.24, 1.0]);
            m.colors.push([0.24, 0.29, 0.42, 1.0]);
        }
        let v = |k: u32, end: u32| base + (k % n) * 2 + end;
        for k in 0..n {
            // (Wound to face out of the cone.)
            m.faces.push([v(k, 0), v(k + 1, 0), v(k + 1, 1)]);
            m.faces.push([v(k, 0), v(k + 1, 1), v(k, 1)]);
            m.edges.push([v(k, 1), v(k + 1, 1)]);
            if k % 2 == 0 {
                m.edges.push([v(k, 0), v(k, 1)]);
            }
        }
    }
    m
}

/// A hull's canopy: a low glass wedge on the top of its nose (found from its
/// shape: a quarter of the way back, on its top there).
fn canopy(s: &Shape) -> WireModel {
    let pts = &s.mesh.points;
    let (lo, hi) = s.mesh.extent();
    let len = (hi.z - lo.z) as f32;
    let (z0, z1) = (lo.z as f32 + 0.12 * len, lo.z as f32 + 0.32 * len);
    // Its top and half-width over that stretch.
    let near: Vec<&universe_engine::glam::DVec3> = pts.iter().filter(|p| (p.z as f32) >= z0 - 0.1 * len && (p.z as f32) <= z1 + 0.1 * len).collect();
    let top = near.iter().map(|p| p.y as f32).fold(f32::NEG_INFINITY, f32::max);
    let half = near.iter().map(|p| p.x.abs() as f32).fold(0.0, f32::max) * 0.55;
    if !top.is_finite() || half <= 0.0 {
        return WireModel::default();
    }
    let _ = top;
    // The hull's top at (x, z): the highest any part reaches there.
    let surface = |x: f32, z: f32| -> Option<f32> {
        s.solids
            .iter()
            .filter_map(|planes| {
                let (mut lo, mut hi) = (f64::NEG_INFINITY, f64::INFINITY);
                for &(n, d) in planes {
                    let rest = d - n.x * x as f64 - n.z * z as f64;
                    if n.y > 1e-9 {
                        hi = hi.min(rest / n.y);
                    } else if n.y < -1e-9 {
                        lo = lo.max(rest / n.y);
                    } else if rest < 0.0 {
                        return None;
                    }
                }
                (hi >= lo).then_some(hi as f32)
            })
            .fold(None, |a: Option<f32>, b| Some(a.map_or(b, |a| a.max(b))))
    };
    // As wide as the top there lets it sit (each corner on the hull), narrowing till it does.
    let mut half = half;
    let corners = |half: f32| [(-half * 0.5, z0), (half * 0.5, z0), (half, z1), (-half, z1)].map(|(x, z)| surface(x, z));
    while corners(half).iter().any(|c| c.is_none()) && half > 0.05 {
        half *= 0.8;
    }
    let ys = corners(half).map(|c| c.unwrap_or(0.0));
    let floor = ys.iter().copied().fold(f32::INFINITY, f32::min);
    let h = (half * 0.3).min(len * 0.022);
    // (Set low: sunk well into the plating, its ridge just over the highest of them.)
    let y = |k: usize| ys[k] - h * 0.6;
    let ridge = ys.iter().copied().fold(f32::NEG_INFINITY, f32::max) + h * 0.55;
    let _ = floor;
    let mut m = WireModel::default();
    // Base corners (front pair narrower), the ridge.
    let p = [
        Vec3::new(-half * 0.5, y(0), z0), Vec3::new(half * 0.5, y(1), z0), Vec3::new(half, y(2), z1), Vec3::new(-half, y(3), z1),
        Vec3::new(-half * 0.35, ridge, z0 + 0.4 * (z1 - z0)), Vec3::new(half * 0.35, ridge, z0 + 0.4 * (z1 - z0)), Vec3::new(half * 0.6, ridge - h * 0.1, z1), Vec3::new(-half * 0.6, ridge - h * 0.1, z1),
    ];
    m.positions.extend_from_slice(&p);
    for (quad, n) in [([0, 1, 5, 4], Vec3::new(0.0, 0.6, -1.0)), ([1, 2, 6, 5], Vec3::X), ([3, 0, 4, 7], Vec3::NEG_X), ([4, 5, 6, 7], Vec3::Y), ([2, 3, 7, 6], Vec3::Z)] {
        let ring = m.add_polygon(&quad, n);
        for w in 0..4 {
            m.edges.push([ring[w], ring[(w + 1) % 4]]);
        }
    }
    m
}

/// Where a hull's navigation lights go: the port and starboard tips (its
/// farthest points left and right), and the strobe at its top rear.
fn nav_lights(s: &Shape) -> [universe_engine::glam::DVec3; 3] {
    let pts = &s.mesh.points;
    let pick = |key: &dyn Fn(&universe_engine::glam::DVec3) -> f64| pts.iter().copied().max_by(|a, b| key(a).total_cmp(&key(b))).unwrap_or_default();
    [pick(&|p| -p.x), pick(&|p| p.x), pick(&|p| p.z + 0.5 * p.y)]
}

/// The corners of the convex solid `planes` bound (each n·p ≤ d), its
/// edges cut back by a chamfer `bevel` deep where its faces meet at an angle.
fn chamfered(planes: &[(DVec3, f64)], bevel: f64) -> Vec<Vec3> {
    chamfered_cut(planes, bevel, &[])
}

/// `chamfered`, cut by `cuts` too (planes that bound it, unbevelled: a
/// slab of the solid between paint lines).
fn chamfered_cut(planes: &[(DVec3, f64)], bevel: f64, cuts: &[(DVec3, f64)]) -> Vec<Vec3> {
    // Its faces' planes, each once.
    let mut faces: Vec<(DVec3, f64)> = Vec::new();
    for &(n, d) in planes {
        if !faces.iter().any(|&(m, e)| m.dot(n) > 1.0 - 1e-6 && (e - d).abs() < 1e-4) {
            faces.push((n, d));
        }
    }
    let corners = |ps: &[(DVec3, f64)]| -> Vec<DVec3> {
        let mut out: Vec<DVec3> = Vec::new();
        for i in 0..ps.len() {
            for j in i + 1..ps.len() {
                for k in j + 1..ps.len() {
                    let (a, b, c) = (ps[i], ps[j], ps[k]);
                    let det = a.0.dot(b.0.cross(c.0));
                    if det.abs() < 1e-9 {
                        continue;
                    }
                    let p = (b.0.cross(c.0) * a.1 + c.0.cross(a.0) * b.1 + a.0.cross(b.0) * c.1) / det;
                    if ps.iter().all(|&(n, d)| n.dot(p) <= d + 1e-6) && !out.iter().any(|q| q.distance(p) < 1e-5) {
                        out.push(p);
                    }
                }
            }
        }
        out
    };
    let points = corners(&faces);
    // Where two faces share an edge (two corners on both) at an angle: a chamfer's plane.
    let mut all = faces.clone();
    for i in 0..faces.len() {
        for j in i + 1..faces.len() {
            let (a, b) = (faces[i], faces[j]);
            if a.0.dot(b.0) > 0.97 {
                continue;
            }
            let on = |p: &&DVec3| (a.0.dot(**p) - a.1).abs() < 1e-5 && (b.0.dot(**p) - b.1).abs() < 1e-5;
            if points.iter().filter(on).count() < 2 {
                continue;
            }
            let n = (a.0 + b.0).normalize();
            let reach = points.iter().map(|p| n.dot(*p)).fold(f64::NEG_INFINITY, f64::max);
            all.push((n, reach - bevel));
        }
    }
    all.extend_from_slice(cuts);
    corners(&all).into_iter().map(|p| p.as_vec3()).collect()
}

/// A paint scheme: the plating's colour, the accent's (a band round the
/// fuselage, the wing tips), where along the ship the band runs (shares of
/// its length from the nose).
pub struct Scheme {
    pub base: [f32; 3],
    /// Wings and fins (flat parts): a second tone, for contrast with the body.
    pub wings: [f32; 3],
    pub accent: [f32; 3],
    pub band: (f32, f32),
}

/// The schemes, by who flies them: ours, traders, pirates, miners, shuttles, settlers.
pub const SCHEMES: [Scheme; 6] = [
    Scheme { base: [0.95, 0.96, 0.97], wings: [0.2, 0.22, 0.25], accent: [0.12, 0.22, 0.5], band: (0.30, 0.36) },
    Scheme { base: [0.8, 0.76, 0.68], wings: [0.42, 0.39, 0.35], accent: [0.85, 0.42, 0.1], band: (0.22, 0.27) },
    Scheme { base: [0.26, 0.27, 0.29], wings: [0.5, 0.12, 0.09], accent: [0.62, 0.08, 0.06], band: (0.18, 0.26) },
    Scheme { base: [0.78, 0.62, 0.18], wings: [0.24, 0.24, 0.26], accent: [0.08, 0.08, 0.09], band: (0.12, 0.2) },
    Scheme { base: [0.9, 0.91, 0.92], wings: [0.5, 0.58, 0.68], accent: [0.15, 0.4, 0.75], band: (0.4, 0.46) },
    Scheme { base: [0.66, 0.69, 0.72], wings: [0.32, 0.38, 0.4], accent: [0.1, 0.45, 0.45], band: (0.33, 0.38) },
];

/// A shape painted: its bevelled solid (see `bevelled`) with each panel its
/// own shade of the plating (fitted plate by plate), the bevel strips darker
/// (the gaps), the accent in a band round it and on its outermost tips, and
/// the engine end sooted toward the nozzles. Colours in its vertices (draw
/// it with a white fill).
pub fn painted(s: &Shape, scheme: &Scheme) -> WireModel {
    let (lo, hi) = s.mesh.extent();
    let (z0, len, wide) = (lo.z, (hi.z - lo.z).max(1e-3), hi.x.abs().max(lo.x.abs()));
    // The paint lines along it: the band's edges, where the soot starts.
    let at = |share: f64| z0 + share * len;
    let lines = [f64::NEG_INFINITY, at(scheme.band.0 as f64), at(scheme.band.1 as f64), at(0.8), f64::INFINITY];
    let mut m = WireModel::default();
    for planes in &s.solids {
        let pts = chamfered(planes, 0.0);
        let (plo, phi) = pts.iter().fold((Vec3::splat(f32::INFINITY), Vec3::splat(f32::NEG_INFINITY)), |(lo, hi), p| (lo.min(*p), hi.max(*p)));
        let bevel = ((phi - plo).min_element() as f64 * 0.12).clamp(0.02, 0.5);
        let normals: Vec<Vec3> = planes.iter().map(|(n, _)| n.as_vec3()).collect();
        // (A flat part, wider than it's thick: a wing or fin, in the second tone.)
        let size = phi - plo;
        let wing = size.y < 0.3 * size.x || size.x < 0.3 * size.y;
        for w in lines.windows(2) {
            let (za, zb) = (w[0], w[1]);
            if zb <= plo.z as f64 + 1e-3 || za >= phi.z as f64 - 1e-3 {
                continue;
            }
            let mut cuts = Vec::new();
            if za.is_finite() {
                cuts.push((DVec3::NEG_Z, -za));
            }
            if zb.is_finite() {
                cuts.push((DVec3::Z, zb));
            }
            let slab = chamfered_cut(planes, bevel, &cuts);
            if slab.len() < 4 {
                continue;
            }
            let part = WireModel::convex_hull(&slab);
            for f in &part.faces {
                let [a, b, c] = f.map(|i| part.positions[i as usize]);
                let n = (b - a).cross(c - a).normalize_or_zero();
                // (The cut faces are inside: none drawn.)
                if n.z.abs() > 0.999 && cuts.iter().any(|(cn, _)| cn.as_vec3().dot(n) > 0.999) {
                    continue;
                }
                let mid = (a + b + c) / 3.0;
                let along = ((mid.z as f64 - z0) / len) as f32;
                let panel = normals.iter().any(|p| p.dot(n) > 0.999);
                let shade = {
                    let q = (n * 7.0).round();
                    let h = ((q.x * 73.0 + q.y * 151.0 + q.z * 269.0 + (mid.z * 0.25).floor() * 37.0) as i32).rem_euclid(97) as f32 / 97.0;
                    0.93 + 0.1 * h
                };
                let accent = (along >= scheme.band.0 && along <= scheme.band.1) || mid.x.abs() as f64 > wide * 0.88;
                let mut col = if accent { scheme.accent } else if wing { scheme.wings } else { scheme.base };
                let k = if panel { shade } else { 0.72 };
                let soot = if along > 0.8 { 1.0 - (along - 0.8) / 0.2 * 0.45 } else { 1.0 };
                for c in &mut col {
                    *c *= k * soot;
                }
                let base = m.positions.len() as u32;
                m.positions.extend_from_slice(&[a, b, c]);
                m.colors.extend([[col[0], col[1], col[2], 1.0]; 3]);
                m.faces.push([base, base + 1, base + 2]);
            }
            for e in &part.edges {
                let base = m.positions.len() as u32;
                m.positions.push(part.positions[e[0] as usize]);
                m.positions.push(part.positions[e[1] as usize]);
                m.colors.extend([[0.6, 0.6, 0.62, 1.0]; 2]);
                m.edges.push([base, base + 1]);
            }
        }
    }
    // A long hull's frames: ribs round its main body every few metres, standing proud.
    if len > 60.0
        && let Some(body) = s.solids.first()
    {
        let proud: Vec<(DVec3, f64)> = body.iter().map(|&(n, d)| (n, d + 0.35)).collect();
        let pts = chamfered(body, 0.0);
        let (zl, zh) = pts.iter().fold((f64::INFINITY, f64::NEG_INFINITY), |(l, h), p| (l.min(p.z as f64), h.max(p.z as f64)));
        let col = scheme.base.map(|c| c * 0.78);
        let mut z = zl + 0.18 * (zh - zl);
        while z < zh - 0.12 * (zh - zl) {
            let rib = chamfered_cut(&proud, 0.15, &[(DVec3::NEG_Z, -z), (DVec3::Z, z + 1.0)]);
            if rib.len() >= 4 {
                let part = WireModel::convex_hull(&rib);
                for f in &part.faces {
                    let base = m.positions.len() as u32;
                    m.positions.extend(f.map(|i| part.positions[i as usize]));
                    m.colors.extend([[col[0], col[1], col[2], 1.0]; 3]);
                    m.faces.push([base, base + 1, base + 2]);
                }
            }
            z += 9.0;
        }
    }
    for l in &s.loops {
        let pts: Vec<Vec3> = l.iter().map(|p| p.as_vec3()).collect();
        let base = m.positions.len() as u32;
        m.positions.extend_from_slice(&pts);
        m.colors.extend(std::iter::repeat_n([0.5, 0.5, 0.52, 1.0], pts.len()));
        for k in 0..pts.len() as u32 {
            m.edges.push([base + k, base + (k + 1) % pts.len() as u32]);
        }
    }
    m
}

/// A shape as the renderer draws it, its edges bevelled: each part's solid
/// with a chamfer where its faces meet (catching the light), its detail
/// lines as they are.
pub fn bevelled(s: &Shape) -> WireModel {
    let mut m = WireModel::default();
    for planes in &s.solids {
        // (A chamfer a few percent of the part's thinnest way, never more than half a metre.)
        let pts = chamfered(planes, 0.0);
        let (lo, hi) = pts.iter().fold((Vec3::splat(f32::INFINITY), Vec3::splat(f32::NEG_INFINITY)), |(lo, hi), p| (lo.min(*p), hi.max(*p)));
        let thin = (hi - lo).min_element() as f64;
        let bevel = (thin * 0.12).clamp(0.02, 0.5);
        let part = WireModel::convex_hull(&chamfered(planes, bevel));
        let base = m.positions.len() as u32;
        m.positions.extend_from_slice(&part.positions);
        m.faces.extend(part.faces.iter().map(|f| f.map(|i| i + base)));
        m.edges.extend(part.edges.iter().map(|e| e.map(|i| i + base)));
    }
    let v = |p: universe_engine::glam::DVec3| p.as_vec3();
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
    // Plant of different sizes, not in a row: a big low unit, a pair of tall
    // tanks, a long pipe rack, small boxes.
    for (x0, z0, w, d, h) in [(-250.0, back + 30.0, 80.0, 60.0, 14.0), (-150.0, back + 45.0, 22.0, 22.0, 46.0), (-118.0, back + 50.0, 22.0, 22.0, 38.0), (-60.0, back + 30.0, 150.0, 12.0, 10.0), (20.0, back + 70.0, 34.0, 26.0, 20.0), (70.0, back + 36.0, 18.0, 30.0, 12.0)] {
        boxes.push(bx(x0, top + 22.0, z0, x0 + w, top + 22.0 + h, z0 + d));
    }
    // Masts: a slim pole on a footing, a small platform partway up, a thinner
    // tip above (its beacon is a light; see `station_lights`).
    for (x, z, h) in [(-200.0, back + 110.0, 150.0), (60.0, back + 120.0, 110.0), (230.0, front - 40.0, 70.0)] {
        boxes.push(bx(x - 6.0, top + 22.0, z - 6.0, x + 6.0, top + 30.0, z + 6.0));
        boxes.push(bx(x - 2.0, top + 30.0, z - 2.0, x + 2.0, top + 22.0 + h * 0.8, z + 2.0));
        boxes.push(bx(x - 7.0, top + 22.0 + h * 0.6, z - 7.0, x + 7.0, top + 24.0 + h * 0.6, z + 7.0));
        boxes.push(bx(x - 1.0, top + 22.0 + h * 0.8, z - 1.0, x + 1.0, top + 22.0 + h, z + 1.0));
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
        // (Each box with its edges bevelled: a tenth of its thinnest way, two metres at most.)
        let (lo, hi) = (lo.as_dvec3(), hi.as_dvec3());
        let planes = [(DVec3::X, hi.x), (DVec3::NEG_X, -lo.x), (DVec3::Y, hi.y), (DVec3::NEG_Y, -lo.y), (DVec3::Z, hi.z), (DVec3::NEG_Z, -lo.z)];
        let bevel = ((hi - lo).min_element() * 0.1).min(2.0);
        let part = WireModel::convex_hull(&chamfered(&planes, bevel));
        let base = m.positions.len() as u32;
        m.positions.extend_from_slice(&part.positions);
        m.faces.extend(part.faces.iter().map(|f| f.map(|i| i + base)));
        m.edges.extend(part.edges.iter().map(|e| e.map(|i| i + base)));
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

/// Modules standing on a settlement's ground, as one mesh in its frame:
/// x east, y up, z south (metres from its position, the pad grid centre).
/// Each a box on its footprint, its edges bevelled, `height` tall (a part
/// of its own while it goes up), standing on ground that falls away with the
/// body's curve (`radius`), sunk a metre so no gap shows. Built once per
/// `key` and kept.
pub fn ground_blocks(key: String, blocks: &[(&universe_sim::world::settlements::Block, f64)], radius: f64) -> Mesh {
    use std::collections::HashMap;
    use std::sync::{Mutex, OnceLock};
    static BUILT: OnceLock<Mutex<HashMap<String, Mesh>>> = OnceLock::new();
    let mut built = BUILT.get_or_init(Default::default).lock().unwrap_or_else(|e| e.into_inner());
    built
        .entry(key)
        .or_insert_with(|| {
            let mut m = WireModel::default();
            for &(b, height) in blocks {
                let (he, hn) = b.half_extent();
                let (e, n) = b.centre;
                let floor = -(e * e + n * n) / (2.0 * radius) - 1.0;
                let (lo, hi) = (DVec3::new(e - he, floor, -(n + hn)), DVec3::new(e + he, floor + height.max(0.5) + 1.0, -(n - hn)));
                let planes = [(DVec3::X, hi.x), (DVec3::NEG_X, -lo.x), (DVec3::Y, hi.y), (DVec3::NEG_Y, -lo.y), (DVec3::Z, hi.z), (DVec3::NEG_Z, -lo.z)];
                let part = WireModel::convex_hull(&chamfered(&planes, ((hi - lo).min_element() * 0.05).min(1.5)));
                let base = m.positions.len() as u32;
                m.positions.extend_from_slice(&part.positions);
                m.faces.extend(part.faces.iter().map(|f| f.map(|i| i + base)));
                m.edges.extend(part.edges.iter().map(|e| e.map(|i| i + base)));
            }
            Mesh::new(m)
        })
        .clone()
}
