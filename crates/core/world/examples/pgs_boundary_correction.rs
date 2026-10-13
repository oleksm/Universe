mod boundary_correction;
#[allow(dead_code)]
mod contact_core;
#[allow(dead_code)]
mod crease;
#[allow(dead_code)]
mod sparse_band;
use glam::DVec3;
use serde_json::{Value, json};
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use universe_world::worlds::{pgs::Surface, sha256};
struct Meter;
static LIVE: AtomicUsize = AtomicUsize::new(0);
static PEAK: AtomicUsize = AtomicUsize::new(0);
unsafe impl std::alloc::GlobalAlloc for Meter {
    unsafe fn alloc(&self, l: std::alloc::Layout) -> *mut u8 {
        let p = unsafe { std::alloc::System.alloc(l) };
        if !p.is_null() {
            let n = LIVE.fetch_add(l.size(), Ordering::Relaxed) + l.size();
            PEAK.fetch_max(n, Ordering::Relaxed);
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
            let v = LIVE.fetch_add(n, Ordering::Relaxed) + n;
            PEAK.fetch_max(v, Ordering::Relaxed);
        }
        q
    }
}
#[global_allocator]
static ALLOC: Meter = Meter;
fn run(s: &Surface, c: DVec3, id: &str, snapshot: Option<&boundary_correction::retained::Snapshot>) -> Result<Value, Value> {
    let expected = snapshot.map_or(1, |s| s.generation);
    let generation = AtomicU64::new(expected);
    let base = crease::build(s, c, 6281370., id, &generation, expected).map_err(|e| json!(e))?;
    let cell_control = boundary_correction::cells::rebuild(s, c, &base, None, false);
    let (base, cell_report) = match cell_control {
        Ok(b) => (b, json!({"coalesced":false})),
        Err(ref e) => {
            if e["reason"] != "cell f32 collapse" {
                return Err(e.clone());
            }
            let uv = e["uv"]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| glam::DVec2::new(v[0].as_f64().unwrap(), v[1].as_f64().unwrap()))
                .collect::<Vec<_>>();
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
            let b = boundary_correction::cells::rebuild(s, c, &base, Some(centre), false)?;
            let reverse = boundary_correction::cells::rebuild(s, c, &base, Some(centre), true)?;
            if b.triangles != reverse.triangles || b.vertices != reverse.vertices {
                return Err(json!({"reason":"reverse subdivision counts"}));
            }
            let map = |p: &crease::Pair| {
                p.tiles[0]
                    .uv
                    .iter()
                    .zip(&p.tiles[0].vertices)
                    .map(|(uv, v)| {
                        (
                            (uv[0].to_bits(), uv[1].to_bits()),
                            glam::Vec3::from_array(*v).as_dvec3(),
                        )
                    })
                    .collect::<std::collections::BTreeMap<_, _>>()
            };
            let a = map(&b);
            let z = map(&reverse);
            if !a.keys().eq(z.keys()) {
                return Err(json!({"reason":"reverse shared subdivision identities"}));
            }
            let reverse_gap = a.iter().map(|(k, v)| v.distance(z[k])).fold(0f64, f64::max);
            if reverse_gap > 1e-5 {
                return Err(json!({"reason":"reverse shared subdivision gap","gap_m":reverse_gap}));
            }
            (
                b,
                json!({"coalesced":true,"block_centre":centre,"old_refusal":e,"reverse_gap_m":reverse_gap,"reverse_vertices":reverse.vertices,"reverse_triangles":reverse.triangles}),
            )
        }
    };
    let chart = contact_core::Chart::new(c, 6281370.);
    let (mut grid, visits) =
        match snapshot { Some(input) => boundary_correction::retained::triangles(input, &chart), None => contact_core::grid::retained_compact(s, &chart) }.map_err(|e| json!(e))?;
    let old = sparse_band::chain(&chart, &grid, 64.).err();
    let ownership = boundary_correction::ownership::correct(&mut grid, &generation, expected, &chart)?;
    for t in &mut grid {
        t.uv = t.world.iter().map(|p| chart.project(*p).unwrap()).collect();
        t.lo =
            t.uv.iter()
                .copied()
                .fold(glam::DVec2::splat(f64::INFINITY), glam::DVec2::min);
        t.hi =
            t.uv.iter()
                .copied()
                .fold(glam::DVec2::splat(f64::NEG_INFINITY), glam::DVec2::max);
    }
    let stale = boundary_correction::ownership::correct(&mut grid, &AtomicU64::new(expected.wrapping_add(1)), expected, &chart)
        .unwrap_err();
    let mut result = sparse_band::build_prepared(s, c, id, base, grid, visits)?;
    if let Some(s) = snapshot { result["resident_binding"] = json!({"source_sha256":s.source_sha256,"generation":s.generation,"eye":s.eye,"meshes":s.patches.iter().map(|p|json!({"key":p.key,"mesh_id":p.mesh_id})).collect::<Vec<_>>()}); }
    result["runtime_publishable"] = json!(false);
    result["anchor_direction"] = json!(c.to_array());
    result["input_kind"] = json!(if snapshot.is_some() { "actual_resident_snapshot" } else { "settled_reconstruction" });
    result["correction"] =
        json!({"cells":cell_report,"ownership":ownership,"old_retained_refusal":old,"stale":stale});
    Ok(result)
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let a = std::env::args().skip(1).collect::<Vec<_>>();
    let bytes = std::fs::read(&a[0])?;
    let plan: Value = serde_json::from_slice(&std::fs::read(&a[2])?)?;
    if sha256(&bytes) != plan["source_sha256"].as_str().ok_or("hash")? {
        return Err("hash".into());
    }
    let s = Surface::read(&bytes)?;
    drop(bytes);
    let fs: Value = serde_json::from_slice(&std::fs::read(&a[1])?)?;
    let snapshot = a.iter().position(|a| a == "--snapshot").map(|i| -> Result<boundary_correction::retained::Snapshot, Box<dyn std::error::Error>> { Ok(serde_json::from_slice(&std::fs::read(a.get(i+1).ok_or("snapshot path")?)?)?) }).transpose()?;
    if snapshot.as_ref().is_some_and(|s|s.source_sha256 != plan["source_sha256"].as_str().unwrap_or("")) { return Err("snapshot source binding".into()); }
    let out = std::path::Path::new(&a[3]);
    std::fs::create_dir_all(out)?;
    let mut results = vec![];
    for f in fs["features"].as_array().unwrap() {
        let id = f["id"].as_str().unwrap();
        if let Some(i) = a.iter().position(|a| a == "--feature") { if a.get(i+1).map(String::as_str) != Some(id) {continue;} }
        if !a.iter().any(|a| a == "--all")
            && !["parent_face_join", "canonical_crest_1", "canonical_crest_2"].contains(&id)
        {
            continue;
        }
        let d = &f["queries"][0]["unit_direction"];
        let mut c = DVec3::new(
            d[0].as_f64().unwrap(),
            d[1].as_f64().unwrap(),
            d[2].as_f64().unwrap(),
        );
        if a.iter().any(|a| a == "--offset16") {
            let chart = contact_core::Chart::new(c, 6281370.);
            c = chart.direction(glam::DVec2::new(16., 0.));
        }
        let baseline = LIVE.load(Ordering::Relaxed);
        PEAK.store(baseline, Ordering::Relaxed);
        let start = std::time::Instant::now();
        let r = run(&s, c, id, snapshot.as_ref());
        let peak = PEAK.load(Ordering::Relaxed).saturating_sub(baseline);
        let time = start.elapsed().as_secs_f64() * 1000.;
        match r {Ok(g)=>{let blob=serde_json::to_vec(&g)?;let peak=PEAK.load(Ordering::Relaxed).saturating_sub(baseline);std::fs::write(out.join(format!("{id}.json")),&blob)?;results.push(json!({"feature":id,"status":if peak<=16777216{"built_for_audit"}else{"memory_refused"},"peak_transient_bytes":peak,"cpu_ms":time,"geometry_sha256":sha256(&blob),"correction":g["correction"],"triangles":g["triangles"]}));},Err(e)=>results.push(json!({"feature":id,"status":"refused","witness":e,"peak_transient_bytes":peak,"cpu_ms":time}))}
    }
    std::fs::write(
        out.join("results.json"),
        serde_json::to_vec_pretty(
            &json!({"results":results,"scope":"CPU selected-group correction; source resident excluded from allocator delta; no GPU residency claim"}),
        )?,
    )?;
    Ok(())
}
