//! CPU review of a complete contact group (band, excision, lock closure) on an exported actual
//! resident snapshot. Usage: contact_group <surface.pgs> <out.json> <snapshot.json>...
//! The export carries only the 512 grid faces; the skirt faces are laid out as the patch maker
//! lays them (four per edge segment over the exported skirt vertices), and labelled so.
use serde_json::json;
use std::sync::{
    Arc,
    atomic::{AtomicU64, AtomicUsize, Ordering},
};
use universe_terrain_contact::{
    RetainedMesh, RetainedPatch,
    boundary_correction::retained::{Patch, Snapshot},
    worker,
};
use universe_world::worlds::{pgs::Surface, sha256};

struct Meter;
static LIVE: AtomicUsize = AtomicUsize::new(0);
static PEAK: AtomicUsize = AtomicUsize::new(0);
unsafe impl std::alloc::GlobalAlloc for Meter {
    unsafe fn alloc(&self, l: std::alloc::Layout) -> *mut u8 {
        let p = unsafe { std::alloc::System.alloc(l) };
        if !p.is_null() {
            PEAK.fetch_max(LIVE.fetch_add(l.size(), Ordering::Relaxed) + l.size(), Ordering::Relaxed);
        }
        p
    }
    unsafe fn dealloc(&self, p: *mut u8, l: std::alloc::Layout) {
        LIVE.fetch_sub(l.size(), Ordering::Relaxed);
        unsafe { std::alloc::System.dealloc(p, l) }
    }
    unsafe fn realloc(&self, p: *mut u8, l: std::alloc::Layout, n: usize) -> *mut u8 {
        let q = unsafe { std::alloc::System.realloc(p, l, n) };
        if !q.is_null() {
            LIVE.fetch_sub(l.size(), Ordering::Relaxed);
            PEAK.fetch_max(LIVE.fetch_add(n, Ordering::Relaxed) + n, Ordering::Relaxed);
        }
        q
    }
}
#[global_allocator]
static ALLOC: Meter = Meter;

struct Laid {
    patch: Patch,
    all: Vec<[u32; 3]>,
}
impl RetainedMesh for Laid {
    fn position(&self, i: usize) -> [f32; 3] {
        self.patch.positions[i]
    }
    fn morph(&self, i: usize) -> [f32; 4] {
        self.patch.morph[i]
    }
    fn faces(&self) -> &[[u32; 3]] {
        &self.patch.faces
    }
    fn vertex_count(&self) -> usize {
        self.patch.positions.len()
    }
    fn all_faces(&self) -> &[[u32; 3]] {
        &self.all
    }
}

/// The ground as submitted at the job's eye (band, rim, replaced and untouched patches), sampled by
/// rays from the world's centre on a 1 m lattice over the chart's ±128 m: holes, double cover,
/// core contact against the canonical surface, and the exterior against the resident ground.
fn submitted(s: &Surface, g: &worker::Group, patches: &[RetainedPatch], eye: [f64; 3]) -> serde_json::Value {
    use glam::{DVec2, DVec3, Vec3};
    use universe_terrain_contact::boundary_correction::retained::morphed;
    let eye = DVec3::from_array(eye);
    let chart = universe_terrain_contact::Chart::new(DVec3::from_array(g.centre), g.radius);
    let mut drawn: Vec<[DVec3; 3]> = vec![];
    let mut resident: Vec<[DVec3; 3]> = vec![];
    let band_o = DVec3::from_array(g.band.origin);
    for t in &g.band.triangles {
        drawn.push(t.map(|k| band_o + Vec3::from_array(g.band.vertices[k as usize]).as_dvec3()));
    }
    let rim_o = DVec3::from_array(g.rim.origin);
    for t in &g.rim.faces {
        drawn.push(t.map(|k| rim_o + Vec3::from_array(g.rim.positions[k as usize]).as_dvec3()));
    }
    let replaced: std::collections::HashMap<[u32; 4], &universe_terrain_contact::replacement::Replacement> = g.patches.iter().map(|r| (r.key, r)).collect();
    for p in patches {
        let o = DVec3::from_array(p.origin);
        let at = |i: u32| o + morphed(o, Vec3::from_array(p.mesh.position(i as usize)), p.mesh.morph(i as usize), eye).as_dvec3();
        for f in p.mesh.faces() {
            resident.push(f.map(at));
        }
        match replaced.get(&p.key) {
            Some(r) => {
                for f in &r.faces[..r.ground] {
                    drawn.push(f.map(|i| o + morphed(o, Vec3::from_array(r.positions[i as usize]), r.morph[i as usize], eye).as_dvec3()));
                }
            }
            None => drawn.extend(p.mesh.faces().iter().map(|f| f.map(at))),
        }
    }
    let bucket = |tris: &[[DVec3; 3]]| {
        let mut b: std::collections::HashMap<(i32, i32), Vec<(usize, [DVec2; 3])>> = std::collections::HashMap::new();
        for (k, t) in tris.iter().enumerate() {
            let Ok(uv) = t.iter().map(|p| chart.project(*p)).collect::<Result<Vec<_>, _>>() else { continue };
            let lo = uv.iter().copied().fold(DVec2::splat(f64::INFINITY), DVec2::min);
            let hi = uv.iter().copied().fold(DVec2::splat(f64::NEG_INFINITY), DVec2::max);
            if hi.x < -130. || lo.x > 130. || hi.y < -130. || lo.y > 130. {
                continue;
            }
            for x in (lo.x.max(-130.) / 2.).floor() as i32..=(hi.x.min(130.) / 2.).floor() as i32 {
                for y in (lo.y.max(-130.) / 2.).floor() as i32..=(hi.y.min(130.) / 2.).floor() as i32 {
                    b.entry((x, y)).or_default().push((k, [uv[0], uv[1], uv[2]]));
                }
            }
        }
        b
    };
    let (db, rb) = (bucket(&drawn), bucket(&resident));
    let hits = |b: &std::collections::HashMap<(i32, i32), Vec<(usize, [DVec2; 3])>>, tris: &[[DVec3; 3]], q: DVec2| -> Vec<f64> {
        let d = chart.direction(q);
        b.get(&((q.x / 2.).floor() as i32, (q.y / 2.).floor() as i32)).into_iter().flatten().filter_map(|(k, [a, bb, c])| {
            let det = (*bb - *a).perp_dot(*c - *a);
            if det == 0. { return None; }
            let v = (q - *a).perp_dot(*c - *a) / det;
            let w = (*bb - *a).perp_dot(q - *a) / det;
            if v < 0. || w < 0. || v + w > 1. { return None; }
            let t = tris[*k];
            let n = (t[1] - t[0]).cross(t[2] - t[0]);
            Some(n.dot(t[0]) / n.dot(d) - g.radius)
        }).collect()
    };
    let (mut holes, mut doubles, mut samples) = (vec![], vec![], 0);
    let (mut core, mut band, mut exterior) = (0f64, 0f64, 0f64);
    let n = 256;
    for i in 0..n {
        for j in 0..n {
            let q = DVec2::new(-128. + (i as f64 + 0.3137) * 256. / n as f64, -128. + (j as f64 + 0.6271) * 256. / n as f64);
            samples += 1;
            let h = hits(&db, &drawn, q);
            match h.len() {
                0 => { if holes.len() < 5 { holes.push(q.to_array()); } else { holes.push([f64::NAN; 2]); } continue; }
                1 => {}
                _ => { if doubles.len() < 5 { doubles.push(json!({"uv":q.to_array(),"heights":h})); } else { doubles.push(json!(null)); } continue; }
            }
            let m = q.x.abs().max(q.y.abs());
            if m <= 32. {
                let canonical = universe_terrain_contact::contact_core::grid::visible(s, chart.direction(q)).unwrap_or(f64::NAN);
                core = core.max((h[0] - canonical).abs());
            } else if m <= 64. {
                let canonical = universe_terrain_contact::contact_core::grid::visible(s, chart.direction(q)).unwrap_or(f64::NAN);
                band = band.max((h[0] - canonical).abs());
            } else {
                let old = hits(&rb, &resident, q);
                if let [o] = old[..] { exterior = exterior.max((h[0] - o).abs()); }
            }
        }
    }
    json!({"samples":samples,"holes":holes.len(),"hole_examples":holes.iter().take(5).collect::<Vec<_>>(),"double_cover":doubles.len(),"double_examples":doubles.iter().take(5).collect::<Vec<_>>(),
        "core_contact_max_m":core,"band_vs_canonical_max_m":band,"exterior_vs_resident_max_m":exterior,"scope":"CPU, submitted geometry at the job eye; 1 m lattice over ±128 m chart; skirts excluded"})
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let bytes = std::fs::read(&a[0])?;
    let hash = sha256(&bytes);
    let s = Surface::read(&bytes)?;
    drop(bytes);
    let mut rows = vec![];
    for path in &a[2..] {
        let snapshot: Snapshot = serde_json::from_slice(&std::fs::read(path)?)?;
        if snapshot.source_sha256 != hash {
            return Err("snapshot source binding".into());
        }
        let selected = snapshot.selected.unwrap_or_default();
        let patches: Vec<RetainedPatch> = snapshot
            .patches
            .into_iter()
            .map(|p| {
                let mut all = p.faces.clone();
                let skirts = p.positions.len().saturating_sub(289) / 2;
                for w in 0..skirts.saturating_sub(1) as u32 {
                    let (a, b, c, d) = (289 + w * 2, 289 + w * 2 + 2, 289 + w * 2 + 1, 289 + w * 2 + 3);
                    all.extend([[a, b, d], [a, d, c], [a, d, b], [a, c, d]]);
                }
                RetainedPatch { key: p.key, mesh_id: p.mesh_id, origin: p.origin, partial: p.partial, mesh: Arc::new(Laid { patch: p, all }) }
            })
            .collect();
        let generation = AtomicU64::new(snapshot.generation);
        let baseline = LIVE.load(Ordering::Relaxed);
        PEAK.store(baseline, Ordering::Relaxed);
        let clock = std::time::Instant::now();
        let mut stages = serde_json::Map::new();
        let r = worker::group_probed(&s, snapshot.radius_m, snapshot.eye, snapshot.generation, &generation, &selected, &patches, &mut |stage| {
            stages.insert(stage.into(), json!({"live":LIVE.load(Ordering::Relaxed).saturating_sub(baseline),"peak":PEAK.load(Ordering::Relaxed).saturating_sub(baseline)}));
        });
        let ms = clock.elapsed().as_secs_f64() * 1000.;
        let peak = PEAK.load(Ordering::Relaxed).saturating_sub(baseline);
        let stale = worker::group(&s, snapshot.radius_m, snapshot.eye, snapshot.generation, &AtomicU64::new(snapshot.generation + 1), &selected, &patches).err();
        rows.push(match r {
            Ok(g) => json!({"snapshot":path,"status":"built","submitted":submitted(&s, &g, &patches, snapshot.eye),"peak_transient_bytes":peak,"cpu_ms":ms,"stages":stages,"band_triangles":g.band.triangles.len(),"patches_changed":g.patches.len(),"replacement_triangles":g.patches.iter().map(|p|p.faces.len()).sum::<usize>(),"gpu_bytes":g.gpu_bytes(),"report":g.report,"stale_refusal":stale}),
            Err(e) => json!({"snapshot":path,"status":"refused","peak_transient_bytes":peak,"cpu_ms":ms,"stages":stages,"witness":e}),
        });
    }
    std::fs::write(&a[1], serde_json::to_vec_pretty(&json!({"format":"contact-group-review/1","source_sha256":hash,"skirt_faces":"laid out from the patch maker's layout over exported skirt vertices (export lacks them)","rows":rows}))?)?;
    Ok(())
}
