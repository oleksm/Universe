//! Opt-in, hash-bound scenery diagnostics. Never changes terrain or physics.
use glam::{DQuat, DVec3, Vec2};
use serde::Deserialize;
use std::{collections::HashSet, sync::OnceLock};
use universe_engine::{Color, Frame, Mesh};
use universe_sim::{
    Body,
    world::worlds::{pgs::Surface, sha256},
};
#[derive(Deserialize)]
struct File {
    #[serde(alias = "version")]
    format: String,
    surface_sha256: String,
    radius_m: f64,
    layers: Vec<Layer>,
    #[serde(default)]
    markers: Vec<Marker>,
}
#[derive(Deserialize)]
struct Marker {
    id: String,
    label: String,
    point: [f64; 4],
}
#[derive(Deserialize)]
struct Layer {
    id: String,
    label: String,
    colour_srgb: [f32; 3],
    points: Vec<[f64; 4]>,
    segments: Vec<[usize; 2]>,
}
struct Debug {
    body: String,
    layers: Vec<Layer>,
    markers: Vec<Marker>,
    lift: f64,
    wire: bool,
    normals: bool,
}
static DEBUG: OnceLock<Debug> = OnceLock::new();
fn colour(rgb: [f32; 3]) -> Color {
    let c = rgb.map(|v| {
        if v <= 0.04045 {
            v / 12.92
        } else {
            ((v + 0.055) / 1.055).powf(2.4)
        }
    });
    Color::rgb(c[0], c[1], c[2])
}
fn validate(file: File, s: &Surface, hash: &str) -> Result<(Vec<Layer>, Vec<Marker>), String> {
    if file.format != "scenery-debug/1"
        || file.surface_sha256 != hash
        || !file.radius_m.is_finite()
        || (file.radius_m - s.radius_m).abs() > 1e-6
    {
        return Err("scenery debug format/surface hash/radius mismatch".into());
    }
    let mut ids = HashSet::new();
    let mut count = 0;
    for l in &file.layers {
        if l.id.is_empty()
            || !ids.insert(&l.id)
            || l.label.is_empty()
            || l.colour_srgb
                .iter()
                .any(|c| !c.is_finite() || !(0.0..=1.0).contains(c))
        {
            return Err("invalid scenery debug layer ID/label/colour".into());
        }
        count += l.segments.len();
        if count > 200_000 {
            return Err(
                "scenery debug exceeds 200000 segment bound; supply a regional sidecar".into(),
            );
        }
        for p in &l.points {
            let d = DVec3::new(p[0], p[1], p[2]);
            if p.iter().any(|v| !v.is_finite()) || (d.length_squared() - 1.).abs() > 1e-8 {
                return Err(format!("{}: invalid direction/height", l.id));
            }
            if (s.query(d)?.height_m - p[3]).abs() > 0.001 {
                return Err(format!(
                    "{}: point differs from canonical terrain height",
                    l.id
                ));
            }
        }
        if l.segments.iter().flatten().any(|&i| i >= l.points.len()) {
            return Err(format!("{}: segment index out of bounds", l.id));
        }
    }
    for m in &file.markers {
        let p = m.point;
        let v = DVec3::new(p[0], p[1], p[2]);
        if m.id.is_empty()
            || m.label.is_empty()
            || p.iter().any(|v| !v.is_finite())
            || (v.length_squared() - 1.).abs() > 1e-8
            || (s.query(v)?.height_m - p[3]).abs() > 0.001
        {
            return Err("invalid scenery debug marker".into());
        }
    }
    Ok((file.layers, file.markers))
}
pub fn configure(body: &str, s: &Surface, hash: &str) -> Result<(), String> {
    let file = std::env::var_os("UNIVERSE_PGS1_DEBUG");
    let mode = std::env::var("UNIVERSE_PGS1_GEOMETRY").unwrap_or_else(|_| "off".into());
    if !["off", "wire", "normals", "both"].contains(&mode.as_str()) {
        return Err("UNIVERSE_PGS1_GEOMETRY: off|wire|normals|both".into());
    }
    if file.is_none() && mode == "off" {
        return Ok(());
    }
    let lift = std::env::var("UNIVERSE_PGS1_DEBUG_LIFT")
        .ok()
        .map(|v| v.parse::<f64>())
        .transpose()
        .map_err(|_| "invalid debug lift")?
        .unwrap_or(2.);
    if !lift.is_finite() || !(0.0..=100.).contains(&lift) {
        return Err("debug display lift must be 0..100m".into());
    }
    let (mut layers, markers) = if let Some(file) = file {
        let bytes = std::fs::read(file).map_err(|e| e.to_string())?;
        log::info!("scenery debug sidecar SHA256 {}", sha256(&bytes));
        validate(
            serde_json::from_slice(&bytes).map_err(|e| e.to_string())?,
            s,
            hash,
        )?
    } else {
        (Vec::new(), Vec::new())
    };
    if let Ok(selected) = std::env::var("UNIVERSE_PGS1_DEBUG_LAYERS") {
        let ids: HashSet<_> = selected.split(',').collect();
        if ids.iter().any(|id| !layers.iter().any(|l| &l.id == id)) {
            return Err("unknown scenery debug layer requested".into());
        }
        layers.retain(|l| ids.contains(l.id.as_str()));
    }
    for l in &layers {
        log::info!(
            "scenery debug {}: {} ({} segments)",
            l.id,
            l.label,
            l.segments.len()
        );
    }
    log::info!(
        "SCENERY DEBUG: display-only radial lift {lift}m, mode {mode}; 30km display range; normal glyphs 100m visual scale"
    );
    DEBUG
        .set(Debug {
            body: body.into(),
            layers,
            markers,
            lift,
            wire: mode == "wire" || mode == "both",
            normals: mode == "normals" || mode == "both",
        })
        .map_err(|_| "scenery debug already configured".into())
}
fn settings(body: &Body) -> Option<&'static Debug> {
    DEBUG.get().filter(|d| d.body == body.key)
}
fn display_point(p: [f64; 4], host_radius: f64, lift: f64) -> DVec3 {
    DVec3::new(p[0], p[1], p[2]).normalize() * (host_radius + p[3] + lift)
}
pub fn draw(frame: &mut Frame, body: &Body, center: DVec3, rotation: DQuat) {
    let Some(d) = settings(body) else { return };
    let eye = rotation.inverse() * (frame.camera.position - center);
    for l in &d.layers {
        let color = colour(l.colour_srgb);
        for &[a, b] in &l.segments {
            let point = |i: usize| {
                let p = l.points[i];
                display_point(p, body.rail.radius, d.lift)
            };
            let (a, b) = (point(a), point(b));
            if a.distance(eye).min(b.distance(eye)) > 30_000. {
                continue;
            }
            frame.line(center + rotation * a, center + rotation * b, color);
        }
    }
    for m in &d.markers {
        let p = m.point;
        let up = DVec3::new(p[0], p[1], p[2]);
        let local = display_point(p, body.rail.radius, d.lift);
        if local.distance(eye) > 30_000. {
            continue;
        }
        let at = center + rotation * local;
        let top = at + rotation * up * 100.;
        frame.line(at, top, Color::rgb(1., 1., 0.));
        frame.circle(top, rotation * up, 20., 12, Color::rgb(1., 1., 0.));
        if let Some(at) = frame.project(top) {
            frame.text(at, &m.label, Color::rgb(1., 1., 0.));
        }
    }
    let x = (frame.size().x - 400.).max(280.);
    let mut y = 90.;
    let mut label = |text: &str, color: Color| {
        frame.text_boxed(Vec2::new(x, y), text, color, Color::rgb(0.05, 0.05, 0.05));
        y += 12.;
    };
    label(
        &format!("DEBUG: LIFT {:.1} M / RANGE 30 KM", d.lift),
        Color::rgb(1., 0.9, 0.3),
    );
    label("DISPLAY ONLY; GLYPHS 100 M", Color::rgb(1., 0.9, 0.3));
    if d.wire {
        label("MAGENTA: RENDERED TRIANGLES", Color::rgb(0.9, 0.3, 0.9));
    }
    if d.normals {
        label(
            "NORMAL: CYAN CANONICAL / MAGENTA RENDER",
            Color::rgb(0., 1., 1.),
        );
        label("SLOPE: GREEN 0 TO RED 1+ RISE/RUN", Color::rgb(0.4, 1., 0.));
    }
    for l in &d.layers {
        label(&l.id, colour(l.colour_srgb));
    }
}
/// Show actual resident patch triangles AFTER the same shader morph. Skirts excluded.
pub fn patch(
    frame: &mut Frame,
    body: &Body,
    mesh: &Mesh,
    origin: DVec3,
    center: DVec3,
    rotation: DQuat,
) {
    let Some(d) = settings(body).filter(|d| d.wire || d.normals) else {
        return;
    };
    let eye = rotation.inverse() * (frame.camera.position - center);
    if origin.distance(eye) > 30_000. {
        return;
    }
    let vertex = |i: usize| {
        let p = mesh.positions[i];
        let c = mesh.colors[i];
        let far = (universe_engine::shaders::GEOMORPH_SPLIT as f32) * 2. * c[3] * 0.95;
        let distance = (origin + p.as_dvec3() - eye).length() as f32;
        let t = ((distance - far * 0.6) / (far * 0.4)).clamp(0., 1.);
        origin + (p + glam::Vec3::new(c[0], c[1], c[2]) * (t * t * (3. - 2. * t))).as_dvec3()
    };
    let world = |p: DVec3| center + rotation * (p + p.normalize() * d.lift);
    for (k, tri) in mesh.faces.iter().take(512).enumerate() {
        let p = tri.map(|i| vertex(i as usize));
        let mid = (p[0] + p[1] + p[2]) / 3.;
        if mid.distance(eye) > 30_000. {
            continue;
        }
        if d.wire {
            for (a, b) in [(0, 1), (1, 2), (2, 0)] {
                frame.line(world(p[a]), world(p[b]), Color::rgb(0.9, 0.3, 0.9));
            }
        }
        if d.normals && k % 128 == 0 {
            let mut n = (p[1] - p[0]).cross(p[2] - p[0]).normalize();
            if n.dot(mid) < 0. {
                n = -n
            }
            if let Some((_, canonical)) = body
                .terrain
                .as_ref()
                .and_then(|t| t.surface_differential(mid.normalize(), body.rail.radius))
            {
                let at = world(mid);
                frame.line(at, at + rotation * n * 100., Color::rgb(1., 0.3, 1.));
                frame.line(
                    at,
                    at + rotation * canonical.normal * 100.,
                    Color::rgb(0., 1., 1.),
                );
                // Tangent glyph: colour and length encode canonical rise/run, diagnostic only.
                let slope = canonical.slope.min(1.) as f32;
                let tangent = canonical.gradient.normalize_or_zero();
                frame.line(
                    at,
                    at + rotation * tangent * (canonical.slope.min(1.) * 100.),
                    Color::rgb(slope, 1. - slope, 0.),
                );
            }
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn source_radius_does_not_move_overlay_off_host_terrain() {
        let p = [0., 1., 0., 5746.5];
        let at = display_point(p, 6281370., 2.);
        assert!((at.length() - 6281370. - p[3] - 2.).abs() < 1e-9);
        assert_eq!(at.normalize(), DVec3::Y);
        assert!((at.length() - 6371000. - p[3]).abs() > 89000.);
    }
    #[test]
    fn sidecar_rejects_wrong_surface_and_invalid_geometry() {
        let s = Surface::read(include_bytes!(
            "../../core/world/tests/fixtures/pgs1-v1/root.pgs"
        ))
        .unwrap();
        let dir = DVec3::X;
        let h = s.query(dir).unwrap().height_m;
        let make = || File {
            format: "scenery-debug/1".into(),
            surface_sha256: "hash".into(),
            radius_m: s.radius_m,
            markers: vec![],
            layers: vec![Layer {
                id: "test".into(),
                label: "fixture".into(),
                colour_srgb: [1., 0., 0.],
                points: vec![[1., 0., 0., h]],
                segments: vec![[0, 0]],
            }],
        };
        assert!(validate(make(), &s, "hash").is_ok());
        assert!(validate(make(), &s, "wrong").is_err());
        let mut bad = make();
        bad.layers[0].points[0][3] += 0.01;
        assert!(validate(bad, &s, "hash").is_err());
        let mut bad = make();
        bad.layers[0].segments[0][1] = 1;
        assert!(validate(bad, &s, "hash").is_err());
        let mut bad = make();
        bad.layers[0].points[0][0] = 2.;
        assert!(validate(bad, &s, "hash").is_err());
    }
}

pub fn active(body: &Body) -> bool {
    settings(body).is_some()
}
