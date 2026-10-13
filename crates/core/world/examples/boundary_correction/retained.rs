//! Actual selected input adapter. It never requests or reconstructs absent neighbours.
use crate::{
    contact_core::{Chart, grid::Triangle},
    crease::{area, rect},
};
use glam::{DVec2, DVec3, Vec3};
use serde::Deserialize;
use std::collections::BTreeSet;
#[derive(Deserialize)]
pub struct Snapshot {
    pub source_sha256: String,
    pub radius_m: f64,
    pub eye: [f64; 3],
    pub generation: u64,
    pub patches: Vec<Patch>,
}
#[derive(Deserialize)]
pub struct Patch {
    pub key: [u32; 4],
    pub mesh_id: u64,
    pub origin: [f64; 3],
    pub partial: bool,
    pub positions: Vec<[f32; 3]>,
    pub morph: Vec<[f32; 4]>,
    pub faces: Vec<[u32; 3]>,
}
pub fn triangles(snapshot: &Snapshot, chart: &Chart) -> Result<(Vec<Triangle>, usize), String> {
    if snapshot.radius_m != chart.r
        || snapshot.generation == 0
        || snapshot.patches.is_empty()
        || snapshot.patches.len() > 3000
        || !DVec3::from_array(snapshot.eye).is_finite()
    {
        return Err("invalid resident snapshot binding/cap".into());
    }
    let mut keys = BTreeSet::new();
    for p in &snapshot.patches {
        let [f, l, x, y] = p.key;
        if f >= 6
            || l > 19
            || x >= 1u32 << l
            || y >= 1u32 << l
            || !keys.insert(p.key)
            || p.partial
            || p.mesh_id == 0
        {
            return Err("invalid/partial/duplicate resident patch".into());
        }
    }
    for &[f, l, x, y] in &keys {
        for parent in 0..l {
            if keys.contains(&[f, parent, x >> (l - parent), y >> (l - parent)]) {
                return Err("selected ancestor/descendant overlap".into());
            }
        }
    }
    let eye = DVec3::from_array(snapshot.eye);
    let mut out = Vec::new();
    for p in &snapshot.patches {
        let origin = DVec3::from_array(p.origin);
        if !origin.is_finite()
            || p.positions.len() < 289
            || p.morph.len() != p.positions.len()
            || p.faces.len() != 512
        {
            return Err("resident grid shape".into());
        }
        let mut vertices = Vec::with_capacity(289);
        for i in 0..289 {
            let raw = Vec3::from_array(p.positions[i]);
            let m = p.morph[i];
            if !raw.is_finite() || m.iter().any(|x| !x.is_finite()) || m[3] <= 0. {
                return Err("invalid resident vertex/morph".into());
            }
            let far = (2.4 * 2. * m[3] as f64 * 0.95) as f32;
            let distance = (origin + raw.as_dvec3() - eye).length() as f32;
            let t = ((distance - far * 0.6) / (far * 0.4)).clamp(0., 1.);
            let v = raw + Vec3::new(m[0], m[1], m[2]) * (t * t * (3. - 2. * t));
            vertices.push(origin + v.as_dvec3());
        }
        let mut seen = BTreeSet::new();
        for ids in &p.faces {
            if ids.iter().any(|&i| i >= 289) || !seen.insert(*ids) {
                return Err("invalid resident grid face".into());
            }
            let world = ids.map(|i| vertices[i as usize]);
            // Ignore the opposite hemisphere, never fabricate support for it.
            if world.iter().any(|p| p.dot(chart.c) <= 0.) {
                continue;
            }
            let uv = world
                .iter()
                .map(|p| chart.project(*p))
                .collect::<Result<Vec<_>, _>>()?;
            if area(&rect(uv.clone(), [-64., 64., -64., 64.])) < 1e-12 {
                continue;
            }
            if out.len() >= 32768 {
                return Err("retained triangle cap".into());
            }
            let lo = uv
                .iter()
                .copied()
                .fold(DVec2::splat(f64::INFINITY), DVec2::min);
            let hi = uv
                .iter()
                .copied()
                .fold(DVec2::splat(f64::NEG_INFINITY), DVec2::max);
            let [f, l, x, y] = p.key;
            out.push(Triangle {
                tags: ids.map(|i| {
                    [
                        f,
                        (x * 16 + i % 17) << (19 - l),
                        (y * 16 + i / 17) << (19 - l),
                    ]
                }),
                patch: p.key,
                origin,
                trace: None,
                world,
                uv,
                lo,
                hi,
            });
        }
    }
    if out.is_empty() {
        return Err("no actual resident support".into());
    }
    Ok((out, snapshot.patches.len()))
}
