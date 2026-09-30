//! Drawing terrain: a colored globe for seeing continents from orbit, and a
//! local surface grid around the camera for judging height near the ground.
//! Both follow the simulation's terrain, so they show what you'd land on.

use universe_engine::glam::DVec3;
use universe_engine::{Color, Frame, WireModel};
use universe_sim::{Body, Surface, TerrainKind};

use crate::scene::color;

/// Color for a kind of ground on a body.
pub fn surface_color(body: &Body, kind: TerrainKind, surface: Surface) -> Color {
    let base = color(body.color);
    let lighten = |c: Color, k: f32| {
        let [r, g, b, a] = c.0;
        Color([r + (1.0 - r) * k, g + (1.0 - g) * k, b + (1.0 - b) * k, a])
    };
    match (kind, surface) {
        (_, Surface::Ocean) => Color::rgb(0.12, 0.35, 0.95),
        (TerrainKind::Terran, Surface::Lowland) => Color::rgb(0.3, 0.75, 0.35),
        (TerrainKind::Terran, Surface::Highland) => Color::rgb(0.6, 0.6, 0.3),
        (_, Surface::Peak) => lighten(base, 0.7),
        (_, Surface::Highland) => lighten(base, 0.2),
        (_, Surface::Crater) => base.scale(0.35),
        (_, Surface::Lowland) => base.scale(0.85),
    }
}

/// A unit globe (latitude/longitude lines) displaced by the terrain and
/// colored by what's under each point. Built once per body.
pub fn globe(body: &Body) -> Option<WireModel> {
    let terrain = body.terrain.as_ref()?;
    let mut m = WireModel::globe(24, 14, 4);
    m.colors = m
        .positions
        .iter()
        .map(|p| surface_color(body, terrain.kind, terrain.classify(p.as_dvec3()).0).0)
        .collect();
    let r = body.radius;
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
pub fn surface_grid(frame: &mut Frame, body: &Body, center: DVec3, t: f64) {
    let Some(terrain) = &body.terrain else { return };
    let rot = body.rotation(t);
    let rel = rot.inverse() * (frame.camera.position - center);
    let dist = rel.length();
    let dir = rel / dist;
    let r = body.radius;
    let alt = dist - body.surface_radius(dir);
    if !(-1000.0..near_altitude(body)).contains(&alt) {
        return;
    }
    // Spacing: a power of two (stable as you move), at least half the
    // altitude, and wide enough that the grid reaches the horizon.
    let horizon = (2.0 * r * alt.max(50.0)).sqrt();
    let want = (alt * 0.5).max(horizon / N as f64);
    let spacing = 2f64.powf(want.log2().ceil()).clamp(128.0, 4.0e5);

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
    let at = |i: usize, j: usize| pts[j * side + i];
    for j in 0..side {
        for i in 0..side {
            let (p, c) = at(i, j);
            if i + 1 < side {
                let (q, d) = at(i + 1, j);
                frame.line2(p, q, c, d);
            }
            if j + 1 < side {
                let (q, d) = at(i, j + 1);
                frame.line2(p, q, c, d);
            }
            if i + 1 < side && j + 1 < side {
                let (q, cq) = at(i + 1, j);
                let (w, cw) = at(i, j + 1);
                let (z, cz) = at(i + 1, j + 1);
                let k = FILL;
                frame.triangle3([p, q, z], [c.scale(k), cq.scale(k), cz.scale(k)]);
                frame.triangle3([p, z, w], [c.scale(k), cz.scale(k), cw.scale(k)]);
            }
        }
    }
}

/// Below this altitude, show the local surface grid (m).
pub fn near_altitude(body: &Body) -> f64 {
    (body.radius * 0.3).min(1.5e6)
}

/// Crater rims as sketchy circles on the surface.
pub fn crater_rims(frame: &mut Frame, body: &Body, center: DVec3, t: f64) {
    let Some(terrain) = &body.terrain else { return };
    let rot = body.rotation(t);
    let c = color(body.color).scale(0.5);
    for (dir, chord) in terrain.crater_rims() {
        let angle = 2.0 * (chord / 2.0).asin();
        let normal = rot * dir;
        let r = body.radius;
        frame.circle(center + normal * (r * angle.cos() + 50.0), normal, r * angle.sin(), 24, c);
    }
}

/// How bright the ground's fill is, relative to its line color.
pub const FILL: f32 = 0.2;
