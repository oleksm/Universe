//! Drawing terrain: a colored globe for seeing continents from orbit, and a
//! local surface grid around the camera for judging height near the ground.
//! Both follow the simulation's terrain, so they show what you'd land on.

use universe_engine::glam::DVec3;
use universe_engine::{Color, Frame, WireModel};
use universe_sim::{Body, Ground, TerrainKind};

use crate::scene::color;

/// Color for a kind of ground on a body.
pub fn surface_color(body: &Body, kind: TerrainKind, surface: Ground) -> Color {
    if body.terrain.as_ref().is_some_and(|t| t.canonical_surface()) { return Color::rgb(0.45, 0.45, 0.45); }
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

/// The colour of a world's sky: an Earth-like one's blue, others' their own, paler.
pub fn sky_color(body: &Body) -> [f32; 3] {
    match body.terrain.as_ref().map(|t| t.kind) {
        Some(TerrainKind::Terran) => [0.35, 0.6, 1.0],
        _ => {
            let [r, g, b] = body.color;
            [0.4 + 0.6 * r, 0.4 + 0.6 * g, 0.4 + 0.6 * b]
        }
    }
}

/// A world's air as drawn over its ground (see `Frame::with_air`): the optical
/// depth of its column straight up in red, green and blue, and the shell it's
/// drawn as (m). Earth's column (1.225 kg/m³, 8.5 km scale height) is about
/// 0.15, 0.23, 0.37: its sky's colour, scattered, over a little grey haze.
/// More air (denser, or standing taller under weaker gravity), deeper.
pub fn air(body: &Body) -> Option<([f32; 3], f32)> {
    let a = body.rail.atmosphere.as_ref()?;
    let column = (a.surface_density * a.scale_height / (1.225 * 8_500.0)) as f32;
    let sky = sky_color(body);
    let depth = sky.map(|c| (0.34 * c + 0.03) * column);
    // (Even density two scale heights up: the same column, mountains inside it.)
    Some((depth, (2.0 * a.scale_height) as f32))
}

/// Texels a side of each face of a world's surface map.
pub const MAP_SIZE: u32 = 512;

/// A world's surface map (see `GlobeMap`): its terrain sampled on a cube,
/// height in units of its relief and crater-ness (or, below zero, how much
/// a spaceport's plain it is), made on all cores.
pub fn globe_map(body: &Body) -> Option<universe_engine::GlobeMap> {
    let terrain = body.terrain.as_ref()?;
    let n = MAP_SIZE as usize;
    let amp = terrain.amplitude.max(1.0);
    let mut texels = vec![[0.0f32; 2]; 6 * n * n];
    let threads = std::thread::available_parallelism().map_or(4, |c| c.get()).max(1);
    let rows = (6 * n).div_ceil(threads);
    std::thread::scope(|s| {
        for (k, chunk) in texels.chunks_mut(rows * n).enumerate() {
            s.spawn(move || {
                for (i, t) in chunk.iter_mut().enumerate() {
                    let row = k * rows + i / n;
                    let (face, y, x) = (row / n, (row % n) as u32, (i % n) as u32);
                    let dir = universe_engine::GlobeMap::direction(MAP_SIZE, face, x, y);
                    let (h, inside) = terrain.height_and_crater_coarse(dir);
                    // (A port's plain marked in place of crater-ness, below zero.)
                    let plain = terrain.port_plain(dir);
                    *t = [(h / amp) as f32, if plain > 0.01 { -plain as f32 } else { inside as f32 }];
                }
            });
        }
    });
    let map = universe_engine::GlobeMap::new(MAP_SIZE, texels);
    // A world grown by the planet simulation: its own colour, texel by texel (a world drawn from
    // its lines: the flat ground under them, else its bake's).
    if terrain.canonical_surface() { return Some(map); }
    let Some(image) = universe_sim::world::worlds::lines_colour(&body.key).or_else(|| terrain.colour()) else { return Some(map) };
    let mut colors = vec![[0u8; 4]; 6 * n * n];
    std::thread::scope(|s| {
        for (k, chunk) in colors.chunks_mut(rows * n).enumerate() {
            let image = &image;
            s.spawn(move || {
                for (i, c) in chunk.iter_mut().enumerate() {
                    let row = k * rows + i / n;
                    let (face, y, x) = (row / n, (row % n) as u32, (i % n) as u32);
                    let [r, g, b] = image.at(universe_engine::GlobeMap::direction(MAP_SIZE, face, x, y));
                    *c = [r, g, b, 255];
                }
            });
        }
    });
    Some(map.with_colors(colors))
}

/// A world drawn from its vector lines (see `worlds::lines_draw`): its level lines, shores, rivers,
/// peaks and lows, on a unit globe just over its highest ground, and its highest peaks and lowest
/// lows to pin. None: it has no lines.
pub fn lines(body: &Body) -> Option<(WireModel, universe_sim::world::worlds::LinesDraw)> {
    if body.terrain.as_ref()?.canonical_surface() { return None; }
    let d = universe_sim::world::worlds::lines_draw(&body.key, lines_lift(body))?;
    let mut m = WireModel::default();
    m.positions = d.positions.iter().map(|&p| universe_engine::glam::Vec3::from(p)).collect();
    m.colors = d.colors.clone();
    m.edges = d.edges.clone();
    Some((m, d))
}

/// Where a world's lines are drawn from afar: a globe just over its highest ground (in radii).
pub fn lines_lift(body: &Body) -> f64 {
    1.0 + (body.terrain.as_ref().map_or(0.0, |t| t.max_height()) + 1_000.0) / body.rail.radius
}

/// A world's globe's brightness as the shader takes it (see `Frame::with_globe`): below zero for a
/// world drawn from its lines (its slopes steepened in the shading, no made-up detail).
pub fn globe_bright(body: &Body) -> f32 {
    let bright = FILL * 2.5;
    if body.terrain.as_ref().is_some_and(|t| t.drawn_from_lines()) { -bright } else { bright }
}

/// The palette a world's surface map is drawn with (see `Frame::with_globe`).
pub fn globe_kind(body: &Body) -> f32 {
    // Renderer palette 3: measured surface, unknown materials/water, no synthetic detail.
    if body.terrain.as_ref().is_some_and(|t| t.canonical_surface()) { return 3.0; }
    match body.terrain.as_ref().map(|t| t.kind) {
        Some(TerrainKind::Terran) | None => 0.0,
        Some(TerrainKind::Dry) => 1.0,
        Some(TerrainKind::Cratered) => 2.0,
    }
}

/// A unit globe (latitude/longitude lines) displaced by the terrain and
/// colored by what's under each point. Built once per body.
pub fn globe(body: &Body, detail: u32) -> Option<WireModel> {
    let terrain = body.terrain.as_ref()?;
    let mut m = WireModel::globe(24, 14, detail);
    // (White: its colour comes from its surface map, per pixel.)
    m.colors = vec![[1.0; 4]; m.positions.len()];
    let r = body.rail.radius;
    // (A whole globe from the coarse heights: its vertices are hundreds of km apart, and the fine
    // tiles under them all (every one of a world's: gigabytes) would be read for nothing.)
    for p in &mut m.positions {
        let h = terrain.surface_coarse(p.as_dvec3());
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
/// `lines`: just the grid's lines, over the ground's patches (else its own shaded ground too).
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
            if !lines && i + 1 < side && j + 1 < side {
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
