//! Optional terrain visuals use the engine scheduler. Requesting never samples
//! terrain or reads files; the scene keeps its base globe until a lease is ready.
use std::sync::{Arc, OnceLock, atomic::{AtomicU64, Ordering}};
use std::time::Duration;
use universe_engine::{Camera, GlobeMap, Mesh, WireModel, assets::{AssetCache, Config, Loaded, Snapshot}};
use universe_engine::glam::DVec3;
use universe_sim::StarSystem;

pub struct Globe {
    pub full: Mesh,
    pub coarse: Mesh,
    pub map: Arc<GlobeMap>,
    pub lines: Option<(Mesh, Arc<universe_sim::world::worlds::LinesDraw>)>,
    // Keep the identity alive: pointer reuse cannot alias a resident prepared system.
    _system: Arc<StarSystem>,
}
static CACHE: OnceLock<AssetCache<Globe>> = OnceLock::new();
static REVISION: AtomicU64 = AtomicU64::new(0);
fn cache() -> &'static AssetCache<Globe> {
    CACHE.get_or_init(|| AssetCache::new(Config { workers: 2, max_entries: 64, budget_bytes: 256 * 1024 * 1024, ..Config::default() }))
}
pub fn invalidate() { REVISION.fetch_add(1, Ordering::SeqCst); }
pub fn maintain() {
    if let Some(c) = CACHE.get() {
        // Queued visible work can become irrelevant after a camera/scene change.
        // Running jobs publish only into their immutable identity, never a new view.
        for e in c.snapshot().entries {
            if e.state == "queued" && e.idle_seconds > 0.25 { c.cancel(&e.key); }
        }
        c.maintain();
    }
}
pub fn snapshot() -> Snapshot { CACHE.get().map_or_else(Snapshot::default, AssetCache::snapshot) }

pub fn request(system: Arc<StarSystem>, i: usize, camera: &Camera, position: DVec3) -> Option<Arc<Globe>> {
    let body = system.bodies.get(i)?;
    body.terrain.as_ref()?;
    let radius = body.rail.radius;
    // Use the common measured motion/turn prediction, including rear release
    // and time-to-need scheduling. Tiny distant globes keep their base sphere.
    let mut demand = crate::visuals::prediction(camera).demand(position, DVec3::ZERO, radius, radius*2000.)?;
    if position.distance(camera.position) < radius*6. { demand.class = 0; }
    let epoch = REVISION.load(Ordering::SeqCst);
    let identity = format!("globe:{:p}:{i}:{epoch}", Arc::as_ptr(&system));
    demand.estimated_bytes = 96 * 1024 * 1024;
    demand.estimated_cost = Duration::from_secs(1);
    cache().request(identity, demand, move || prepare(system, i, epoch))
}
fn current(epoch: u64) -> Result<(), String> {
    if REVISION.load(Ordering::SeqCst) == epoch { Ok(()) } else { Err("globe invalidated during preparation".into()) }
}
fn prepare(system: Arc<StarSystem>, i: usize, epoch: u64) -> Result<Loaded<Globe>, String> {
    current(epoch)?;
    // Development fixture: delay preparation without sleeping the requesting/UI thread.
    if let Some(delay) = std::env::var("UNIVERSE_GLOBE_LOAD_DELAY_MS").ok().and_then(|s| s.parse::<u64>().ok()) {
        std::thread::sleep(Duration::from_millis(delay.min(60_000)));
    }
    current(epoch)?;
    let _scope = universe_prof::scope("assets/globe/build");
    let body = &system.bodies[i];
    let full = crate::terrain_view::globe(body, 8).ok_or("no terrain")?;
    let coarse = crate::terrain_view::globe(body, 2).ok_or("no terrain")?;
    // Two scheduler workers, one sampling thread each; never spawn every host core.
    let map = crate::terrain_view::globe_map_with_threads(body, 1).ok_or("no terrain map")?;
    let lines = crate::terrain_view::lines(body);
    let mut bytes = map.upload_bytes() + mesh_bytes(&full) + mesh_bytes(&coarse) + map.texels.capacity()*8 + map.colors.as_ref().map_or(0, |c| c.capacity()*4);
    if let Some((mesh, lines)) = &lines {
        bytes += mesh_bytes(mesh) + lines.positions.capacity()*12 + lines.colors.capacity()*16 + lines.edges.capacity()*8 + lines.pins.capacity()*std::mem::size_of::<universe_sim::world::worlds::Pin>();
    }
    // A stretch edit can happen while sampling. Never publish a mixed generation,
    // including changing away and back to the same value while a worker runs.
    current(epoch)?;
    log::info!("globe ready: {} ({bytes} bytes)", body.key);
    Ok(Loaded { bytes, value: Globe { full: full.into(), coarse: coarse.into(), map: Arc::new(map), lines: lines.map(|(mesh, lines)| (mesh.into(), Arc::new(lines))), _system: system } })
}
fn mesh_bytes(m: &WireModel) -> usize {
    m.positions.capacity()*12 + m.edges.capacity()*8 + m.faces.capacity()*12 + m.colors.capacity()*16 + m.data.capacity()*16
}
