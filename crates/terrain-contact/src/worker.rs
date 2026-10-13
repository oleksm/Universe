//! One contact group from the actual resident selection: the band and every replaced patch.
//! Runs off the draw thread; refuses as a whole, and publishes nothing by itself.
use crate::{
    boundary_correction::{
        cells, ownership,
        retained::{Binding, RetainedPatch, triangles_retained},
    },
    contact_core::Chart,
    crease::{self, Tile},
    replacement::{self, Replacement, Rim},
    sparse_band,
};
use glam::{DVec2, DVec3};
use serde_json::{Value, json};
use std::sync::atomic::{AtomicU64, Ordering};
use universe_world::worlds::pgs::Surface;

/// The core's half width (m): only there is the ground the canonical surface.
pub const CORE_HALF: f64 = 32.;

pub struct Group {
    /// Chart basis (body frame): the footprint's middle and its east and north.
    pub centre: [f64; 3],
    pub east: [f64; 3],
    pub north: [f64; 3],
    pub radius: f64,
    pub generation: u64,
    pub band: Tile,
    pub patches: Vec<Replacement>,
    pub rim: Rim,
    /// Every resident patch the group was built from or depends on: (key, mesh id).
    pub inputs: Vec<([u32; 4], u64)>,
    pub report: Value,
}

impl Group {
    /// Where `dir` (body frame) falls on the group's chart (m), if on its side of the world.
    pub fn chart(&self, dir: DVec3) -> Option<DVec2> {
        let c = DVec3::from_array(self.centre);
        let den = dir.dot(c);
        (den > 0.).then(|| DVec2::new(dir.dot(DVec3::from_array(self.east)), dir.dot(DVec3::from_array(self.north))) * (self.radius / den))
    }
    /// Whether `dir` lies inside the canonical core, `margin` (m) in from its edge.
    pub fn in_core(&self, dir: DVec3, margin: f64) -> bool {
        self.chart(dir).is_some_and(|p| p.x.abs() <= CORE_HALF - margin && p.y.abs() <= CORE_HALF - margin)
    }
    /// Expanded GPU bytes (unindexed 56-byte vertices, per mesh at least the empty placeholders).
    pub fn gpu_bytes(&self) -> usize {
        let mesh = |faces: usize| (faces * 3 * 56).max(40) + 40;
        mesh(self.band.triangles.len()) + mesh(self.rim.faces.len()) + self.patches.iter().map(|p| mesh(p.faces.len())).sum::<usize>()
    }
}

/// The group for an eye at `eye` (body frame, m): its footprint centred under the eye.
/// `cancel` must equal `generation` throughout; a change refuses at the next check.
pub fn group(
    s: &Surface,
    radius: f64,
    eye: [f64; 3],
    generation: u64,
    cancel: &AtomicU64,
    selected: &[Binding],
    patches: &[RetainedPatch],
) -> Result<Group, Value> {
    group_probed(s, radius, eye, generation, cancel, selected, patches, &mut |_| {})
}

/// As `group`, calling `probe` after each stage (a memory or time meter's hook).
#[allow(clippy::too_many_arguments)]
pub fn group_probed(
    s: &Surface,
    radius: f64,
    eye: [f64; 3],
    generation: u64,
    cancel: &AtomicU64,
    selected: &[Binding],
    patches: &[RetainedPatch],
    probe: &mut dyn FnMut(&'static str),
) -> Result<Group, Value> {
    let clock = std::time::Instant::now();
    let eye = DVec3::from_array(eye);
    let c = eye.normalize();
    let chart = Chart::new(c, radius);
    let current = || cancel.load(Ordering::Acquire) == generation;
    let base = crease::build(s, c, radius, "runtime", cancel, generation).map_err(|e| json!({"reason":e}))?;
    probe("base");
    // Optional 8 m cells that collapse in f32 coalesce into one 16 m block; mandatory edges stay.
    let (base, cells_report) = match cells::rebuild_radius(s, c, radius, &base, None, false) {
        Ok(b) => (b, json!({"coalesced":false})),
        Err(e) if e["reason"] == "cell f32 collapse" => {
            let uv: Vec<DVec2> = e["uv"]
                .as_array()
                .into_iter()
                .flatten()
                .map(|v| DVec2::new(v[0].as_f64().unwrap_or(0.), v[1].as_f64().unwrap_or(0.)))
                .collect();
            if uv.len() != 3 {
                return Err(json!({"reason":"cell collapse witness","witness":e}));
            }
            let mut pair = (0, 1);
            for i in 0..3 {
                for j in i + 1..3 {
                    if uv[i].distance_squared(uv[j]) < uv[pair.0].distance_squared(uv[pair.1]) {
                        pair = (i, j)
                    }
                }
            }
            let middle = (uv[pair.0] + uv[pair.1]) * 0.5;
            let centre = [(middle.x / 8.).round() * 8., (middle.y / 8.).round() * 8.];
            let b = cells::rebuild_radius(s, c, radius, &base, Some(centre), false)?;
            (b, json!({"coalesced":true,"block_centre":centre,"old_refusal":e}))
        }
        Err(e) => return Err(e),
    };
    probe("cells");
    if !current() {
        return Err(json!({"reason":"stale generation"}));
    }
    let (mut grid, visits) = triangles_retained(radius, eye.to_array(), generation, selected, patches, &chart)
        .map_err(|e| json!({"reason":e}))?;
    let ownership = ownership::correct(&mut grid, cancel, generation, &chart)?;
    probe("grid");
    for t in &mut grid {
        t.uv = t.world.iter().map(|p| chart.project(*p)).collect::<Result<_, _>>().map_err(|e| json!({"reason":e}))?;
        t.lo = t.uv.iter().copied().fold(DVec2::splat(f64::INFINITY), DVec2::min);
        t.hi = t.uv.iter().copied().fold(DVec2::splat(f64::NEG_INFINITY), DVec2::max);
    }
    let prepared = sparse_band::build_prepared_radius(s, c, radius, "runtime", base, &grid, visits)?;
    probe("band");
    if !current() {
        return Err(json!({"reason":"stale generation"}));
    }
    let band = prepared.tiles.into_iter().next().ok_or_else(|| json!({"reason":"no band"}))?;
    let excised = replacement::excise(&chart, eye, grid, patches, &band)?;
    probe("excised");
    if !current() {
        return Err(json!({"reason":"stale generation"}));
    }
    let report = json!({
        "cells":cells_report,"ownership":ownership,"band_triangles":band.triangles.len(),
        "band_vertices":band.vertices.len(),"band_chain":prepared.chain,"excision":excised.report,
        "cpu_ms":clock.elapsed().as_secs_f64()*1000.,
    });
    let mut g = Group {
        centre: chart.c.to_array(),
        east: chart.e.to_array(),
        north: chart.n.to_array(),
        radius,
        generation,
        band,
        patches: excised.patches,
        rim: excised.rim,
        inputs: patches.iter().map(|p| (p.key, p.mesh_id)).collect(),
        report,
    };
    g.report["gpu_bytes"] = json!(g.gpu_bytes());
    Ok(g)
}
