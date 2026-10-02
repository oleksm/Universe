//! Drawing terrain: a colored globe for seeing continents from orbit, and a
//! local surface grid around the camera for judging height near the ground.
//! Both follow the simulation's terrain, so they show what you'd land on.

use universe_engine::glam::DVec3;
use universe_engine::{Color, Frame, WireModel};
use universe_sim::{Body, Ground, TerrainKind};

use crate::scene::color;

/// Color for a kind of ground on a body.
pub fn surface_color(body: &Body, kind: TerrainKind, surface: Ground) -> Color {
    let base = color(body.color);
    let lighten = |c: Color, k: f32| {
        let [r, g, b, a] = c.0;
        Color([r + (1.0 - r) * k, g + (1.0 - g) * k, b + (1.0 - b) * k, a])
    };
    match (kind, surface) {
        (_, Ground::Ocean) => Color::rgb(0.12, 0.35, 0.95),
        (TerrainKind::Terran, Ground::Lowland) => Color::rgb(0.3, 0.75, 0.35),
        (TerrainKind::Terran, Ground::Highland) => Color::rgb(0.6, 0.6, 0.3),
        (_, Ground::Peak) => lighten(base, 0.7),
        (_, Ground::Highland) => lighten(base, 0.2),
        (_, Ground::Crater) => base.scale(0.35),
        (_, Ground::Lowland) => base.scale(0.85),
    }
}

/// The ground's colour at `dir` (body frame, unit), continuous: an
/// Earth-like world's seas darker the deeper (turquoise in the shallows),
/// its coasts sand, its land green to brown with height, snow on the peaks
/// and ice at the poles; other worlds their colour, lighter high, darker in
/// craters; a little variation everywhere (no flat fills).
pub fn ground_color(body: &Body, terrain: &universe_sim::world::terrain::Terrain, dir: DVec3) -> Color {
    let h = terrain.raw_height(dir);
    let a = terrain.amplitude.max(1.0);
    let t = (h / a) as f32;
    let mix = |p: [f32; 3], q: [f32; 3], k: f32| {
        let k = k.clamp(0.0, 1.0);
        [p[0] + (q[0] - p[0]) * k, p[1] + (q[1] - p[1]) * k, p[2] + (q[2] - p[2]) * k]
    };
    // (A little variation: a hash of where.)
    let jitter = {
        let q = (dir * 900.0).round();
        let x = (q.x as i64).wrapping_mul(73_856_093) ^ (q.y as i64).wrapping_mul(19_349_663) ^ (q.z as i64).wrapping_mul(83_492_791);
        0.94 + 0.12 * ((x.rem_euclid(1000)) as f32 / 1000.0)
    };
    let polar = ((dir.y.abs() as f32 - 0.86) / 0.08).clamp(0.0, 1.0);
    let c = match terrain.kind {
        TerrainKind::Terran if h < 0.0 => {
            let deep = (-t / 0.5).clamp(0.0, 1.0);
            mix([0.12, 0.42, 0.62], [0.03, 0.12, 0.36], deep)
        }
        TerrainKind::Terran => {
            let land = if t < 0.03 {
                mix([0.72, 0.68, 0.5], [0.24, 0.45, 0.2], t / 0.03)
            } else if t < 0.45 {
                mix([0.24, 0.45, 0.2], [0.42, 0.36, 0.24], (t - 0.03) / 0.42)
            } else {
                mix([0.42, 0.36, 0.24], [0.9, 0.9, 0.92], (t - 0.45) / 0.35)
            };
            land.map(|c| c * jitter)
        }
        _ => {
            let base = color(body.color).0;
            let (_, inside) = terrain.classify(dir);
            let shade = 0.75 + 0.35 * (t * 0.5 + 0.5).clamp(0.0, 1.0);
            let crater = if inside > 0.25 { 0.6 } else { 1.0 };
            [base[0], base[1], base[2]].map(|c| c * shade * crater * jitter)
        }
    };
    let c = mix(c, [0.92, 0.94, 0.97], polar);
    Color([c[0], c[1], c[2], 1.0])
}

/// A unit globe (latitude/longitude lines) displaced by the terrain and
/// colored by what's under each point. Built once per body.
pub fn globe(body: &Body, detail: u32) -> Option<WireModel> {
    let terrain = body.terrain.as_ref()?;
    let mut m = WireModel::globe(24, 14, detail);
    m.colors = m.positions.iter().map(|p| ground_color(body, terrain, p.as_dvec3()).0).collect();
    let r = body.rail.radius;
    for p in &mut m.positions {
        let h = terrain.surface(p.as_dvec3());
        *p *= (1.0 + h / r) as f32;
    }
    Some(m)
}

/// Cells each way from the center of the local surface grid.
const N: i32 = 24;

/// A grid on the ground around the camera, following the terrain. It is fixed
/// to the planet (cells snap to a cube-sphere lattice), sized to reach roughly
/// to the horizon, and fades toward its edges.
/// With `fine` spacing (m), a small grid underfoot instead, for walking.
/// `lines`: draw the grid's lines too (the shaded ground is always drawn).
pub fn surface_grid(frame: &mut Frame, body: &Body, center: DVec3, t: f64, fine: Option<f64>, lines: bool) {
    let Some(terrain) = &body.terrain else { return };
    let rot = body.rotation(t);
    let rel = rot.inverse() * (frame.camera.position - center);
    let dist = rel.length();
    let dir = rel / dist;
    let r = body.rail.radius;
    let alt = dist - body.surface_radius(dir);
    if !(-1000.0..near_altitude(body)).contains(&alt) {
        return;
    }
    // Spacing: a power of two (stable as you move), at least half the
    // altitude, and wide enough that the grid reaches the horizon.
    let horizon = (2.0 * r * alt.max(50.0)).sqrt();
    let want = (alt * 0.5).max(horizon / N as f64);
    let spacing = fine.unwrap_or_else(|| 2f64.powf(want.log2().ceil()).clamp(128.0, 4.0e5));

    // Cube-sphere face under the camera, and grid coordinates on it.
    let a = dir.abs();
    let (n, ua, va) = if a.x >= a.y && a.x >= a.z {
        (DVec3::X * dir.x.signum(), DVec3::Y, DVec3::Z)
    } else if a.y >= a.z {
        (DVec3::Y * dir.y.signum(), DVec3::Z, DVec3::X)
    } else {
        (DVec3::Z * dir.z.signum(), DVec3::X, DVec3::Y)
    };
    let m = dir.dot(n);
    let du = spacing / r;
    let (u0, v0) = ((dir.dot(ua) / m / du).round() * du, (dir.dot(va) / m / du).round() * du);

    // Stay within one cube face's reach (small bodies, coarse spacing).
    let n_cells = N.min((0.9 / du) as i32).max(2);
    let side = (2 * n_cells + 1) as usize;
    let mut pts = Vec::with_capacity(side * side);
    for j in -n_cells..=n_cells {
        for i in -n_cells..=n_cells {
            let d = (n + ua * (u0 + i as f64 * du) + va * (v0 + j as f64 * du)).normalize();
            let (kind, _) = terrain.classify(d);
            let world = center + rot * (d * (r + terrain.surface(d)));
            // Fade toward the edge of the grid.
            let edge = (i.abs().max(j.abs()) as f32 / n_cells as f32).powi(2);
            let c = surface_color(body, terrain.kind, kind).scale(1.0 - 0.8 * edge);
            pts.push((world, c));
        }
    }
    // Sunlight: faces shaded by their true slope to the star (the terrain's
    // relief), lines dimmed on the night side.
    let light = frame.light;
    // The star's brightness here (falls with distance; the eye adapts partly).
    let bright = light.map_or(1.0, |l| l.intensity_at(center));
    let lambert = |p: DVec3, n: DVec3| light.map_or(1.0, |l| n.dot((l.position - p).normalize()).max(0.0) as f32 * bright);
    let shade = |a: DVec3, b: DVec3, c: DVec3| {
        let mut n = (b - a).cross(c - a).normalize_or_zero();
        if n.dot(a - center) < 0.0 {
            n = -n;
        }
        FILL * (0.12 + 2.4 * lambert((a + b + c) / 3.0, n))
    };
    let pts: Vec<(DVec3, Color)> = pts
        .into_iter()
        .map(|(p, c)| (p, c.scale(0.35 + 0.65 * lambert(p, (p - center).normalize()).sqrt())))
        .collect();
    // The fill sits a little below its grid lines (more with distance), so
    // the lines stay on top of it even when seen at a grazing angle.
    let eye = frame.camera.position;
    let sink = |p: DVec3| p - (p - center).normalize() * (p.distance(eye) * 0.002);
    let at = |i: usize, j: usize| pts[j * side + i];
    for j in 0..side {
        for i in 0..side {
            let (p, c) = at(i, j);
            if lines && i + 1 < side {
                let (q, d) = at(i + 1, j);
                frame.line2(p, q, c, d);
            }
            if lines && j + 1 < side {
                let (q, d) = at(i, j + 1);
                frame.line2(p, q, c, d);
            }
            if i + 1 < side && j + 1 < side {
                let (q, cq) = at(i + 1, j);
                let (w, cw) = at(i, j + 1);
                let (z, cz) = at(i + 1, j + 1);
                let (k1, k2) = (shade(p, q, z), shade(p, z, w));
                let [p, q, w, z] = [p, q, w, z].map(sink);
                frame.triangle3([p, q, z], [c.scale(k1), cq.scale(k1), cz.scale(k1)]);
                frame.triangle3([p, z, w], [c.scale(k2), cz.scale(k2), cw.scale(k2)]);
            }
        }
    }
}

/// Below this altitude, show the local surface grid (m).
pub fn near_altitude(body: &Body) -> f64 {
    (body.rail.radius * 0.3).min(1.5e6)
}

/// Crater rims as sketchy circles on the surface.
pub fn crater_rims(frame: &mut Frame, body: &Body, center: DVec3, t: f64) {
    let Some(terrain) = &body.terrain else { return };
    let rot = body.rotation(t);
    let c = color(body.color).scale(0.5);
    for (dir, chord) in terrain.crater_rims() {
        let angle = 2.0 * (chord / 2.0).asin();
        let normal = rot * dir;
        let r = body.rail.radius;
        frame.circle(center + normal * (r * angle.cos() + 50.0), normal, r * angle.sin(), 24, c);
    }
}

/// How bright the ground's fill is, relative to its line color.
pub const FILL: f32 = 0.2;
