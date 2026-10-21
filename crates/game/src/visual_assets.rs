//! Shared model preparation for world, Studio and mounted equipment. No file
//! reads or decoding on requesting threads. The returned Arc is a residency lease.
use glam::{DMat4, DVec3, Vec3};
use std::{
    collections::BTreeMap,
    sync::{Arc, OnceLock},
    time::Duration,
};
use universe_engine::{
    PbrModel,
    assets::{AssetCache, Config, Demand, Loaded, Snapshot},
};
use universe_sim::world::assets;

pub struct Visual {
    pub source_sha256: String,
    pub model: PbrModel,
    pub centre: Vec3,
    pub nodes: BTreeMap<String, DMat4>,
    pub has_motion: bool,
    pub lo: DVec3,
    pub hi: DVec3,
    pub neutral_error: Option<String>,
}
static CACHE: OnceLock<AssetCache<Visual>> = OnceLock::new();
fn cache() -> &'static AssetCache<Visual> {
    CACHE.get_or_init(|| AssetCache::new(Config::default()))
}
pub fn cancel_speculation() {
    if let Some(c) = CACHE.get() {
        c.cancel_speculation();
    }
}
pub fn maintain() {
    if let Some(c) = CACHE.get() {
        c.maintain();
    }
}
pub fn snapshot() -> Snapshot {
    CACHE
        .get()
        .map_or_else(Snapshot::default, AssetCache::snapshot)
}

// Peak reservation complements package and decoder preflight ceilings. Retained
// output above it is refused; allocator/driver overhead is not measured here.
// No filesystem metadata is queried on the requesting thread.
const RESERVATION: usize = 256 * 1024 * 1024;
pub fn request(key: &str, class: u8) -> Option<Arc<Visual>> {
    request_demand(
        key,
        Demand {
            class,
            time_to_need: if class > 1 {
                Duration::from_secs(2)
            } else {
                Duration::ZERO
            },
            ..Default::default()
        },
    )
}
pub fn request_demand(key: &str, mut demand: Demand) -> Option<Arc<Visual>> {
    let record = assets::visual(key)?;
    let identity = format!("package:{}:{}", record.format, record.manifest_sha256);
    let key = key.to_owned();
    cache().request(
        identity,
        {
            demand.estimated_bytes = RESERVATION;
            demand.estimated_cost = Duration::from_secs_f64(
                (record.triangles.unwrap_or(100_000) as f64 / 2_000_000. + 0.01).clamp(0.01, 10.),
            );
            demand
        },
        move || match prepare(record) {
            Ok(value) => {
                if let Some(e) = &value.neutral_error {
                    log::warn!("asset {key}: neutral/mounted view held: {e}");
                }
                let bytes = resident_bytes(&value);
                log::info!("asset ready: {key} ({bytes} bytes)");
                Ok(Loaded { value, bytes })
            }
            Err(e) => {
                log::warn!("asset failed: {key}: {e}; keeping fallback");
                Err(e)
            }
        },
    )
}
/// The registry model of `key` with its named pieces apart (`PbrModel::load_gltf_parts`: a node whose name
/// holds `parts[k]` is part `k + 1`), as a kit's run model is drawn piece by piece.
pub fn request_parts(key: &str, parts: &'static [&'static str]) -> Option<Arc<Visual>> {
    let record = assets::visual(key)?;
    let identity = format!("package:{}:{}:{parts:?}", record.format, record.manifest_sha256);
    let key = key.to_owned();
    cache().request(
        identity,
        Demand { estimated_bytes: RESERVATION, ..Default::default() },
        move || {
            let package = assets::model(record).inspect_err(|e| log::warn!("asset failed: {key}: {e}; keeping fallback"))?;
            let value = prepare_bytes(&package.glb, parts, true, package.lo, package.hi, package.has_motion)?;
            let bytes = resident_bytes(&value);
            Ok(Loaded { value, bytes })
        },
    )
}
/// Native paths are immutable within a process. Decode options participate in
/// identity; the actual bytes' hash is published for hull-overlay validation.
pub fn request_native(path: &std::path::Path, parts: &[&str]) -> Option<Arc<Visual>> {
    let identity = format!("native:{}:{parts:?}", path.display());
    let path = path.to_owned();
    let parts: Vec<String> = parts.iter().map(|p| (*p).into()).collect();
    cache().request(
        identity,
        Demand {
            estimated_bytes: RESERVATION,
            ..Default::default()
        },
        move || {
            use std::io::Read;
            let file = std::fs::File::open(&path).map_err(|e| e.to_string())?;
            let len = file.metadata().map_err(|e| e.to_string())?.len();
            if len > 64 * 1024 * 1024 {
                return Err("native model exceeds 64 MiB".into());
            }
            let mut bytes = Vec::with_capacity(len as usize);
            file.take(len + 1)
                .read_to_end(&mut bytes)
                .map_err(|e| e.to_string())?;
            if bytes.len() as u64 != len {
                return Err("native model changed while read".into());
            }
            let parts: Vec<&str> = parts.iter().map(String::as_str).collect();
            let value = prepare_bytes(&bytes, &parts, false, [0.; 3], [0.; 3], false)?;
            let bytes = resident_bytes(&value);
            Ok(Loaded { value, bytes })
        },
    )
}
fn prepare(record: &universe_sim::world::registry::Visual) -> Result<Visual, String> {
    let package = assets::model(record)?;
    prepare_bytes(
        &package.glb,
        &[],
        true,
        package.lo,
        package.hi,
        package.has_motion,
    )
}
fn prepare_bytes(
    glb: &[u8],
    parts: &[&str],
    package_bounds: bool,
    bound_lo: [f64; 3],
    bound_hi: [f64; 3],
    has_motion: bool,
) -> Result<Visual, String> {
    let model = PbrModel::load_gltf_parts(glb, parts)?;
    let source_sha256 = universe_sim::world::worlds::sha256(glb);
    // A sidecar can describe future motion while the GLB remains a
    // valid neutral rigid model. Do not silently ignore skin/morph data.
    let (nodes, neutral_error) =
        match assets::mounts::require_static(glb).and_then(|()| assets::mounts::frames(glb)) {
            Ok(nodes) => (nodes, None),
            Err(e) => (BTreeMap::new(), Some(e)),
        };
    let (mut lo, mut hi) = (Vec3::splat(f32::INFINITY), Vec3::splat(f32::NEG_INFINITY));
    for v in model.data.primitives.iter().flat_map(|p| &p.vertices) {
        let p = Vec3::from(v.pos);
        if !p.is_finite() {
            return Err("non-finite model vertex".into());
        }
        lo = lo.min(p);
        hi = hi.max(p);
    }
    if !lo.is_finite() || !hi.is_finite() {
        return Err("empty model".into());
    }
    Ok(Visual {
        source_sha256,
        model,
        centre: (lo + hi) * 0.5,
        nodes,
        has_motion,
        lo: if package_bounds {
            glam::DVec3::from(bound_lo)
        } else {
            lo.as_dvec3()
        },
        hi: if package_bounds {
            glam::DVec3::from(bound_hi)
        } else {
            hi.as_dvec3()
        },
        neutral_error,
    })
}
fn resident_bytes(v: &Visual) -> usize {
    let d = &v.model.data;
    std::mem::size_of::<Visual>() + v.source_sha256.capacity()
        + d.primitives.capacity() * std::mem::size_of::<universe_engine::pbr::Primitive>()
        + d.materials.capacity() * std::mem::size_of::<universe_engine::pbr::Material>()
        + d.images.capacity() * std::mem::size_of::<universe_engine::pbr::Image>()
        + d.primitives.iter().map(|p| p.vertices.capacity() * std::mem::size_of::<universe_engine::pbr::PbrVertex>() + p.indices.capacity() * 4).sum::<usize>()
        + d.images.iter().map(|i| i.rgba.capacity()).sum::<usize>()
        // Includes conservative BTree node allocation overhead, not just payload.
        + v.nodes.iter().map(|(k,_)| k.capacity() + std::mem::size_of::<DMat4>() + 128).sum::<usize>()
        + v.neutral_error.as_ref().map_or(0, String::capacity)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn real_asset_cold_warm_and_scene_return_accounting() {
        use std::time::Instant;
        if assets::store().is_none() {
            return;
        }
        let record = assets::visual("equipment.engine.ch.s2").unwrap();
        let cache = AssetCache::new(Config {
            idle_for: Duration::from_millis(30),
            ..Config::default()
        });
        let request = || {
            cache.request(
                "ch-s2".into(),
                Demand {
                    estimated_bytes: RESERVATION,
                    ..Demand::default()
                },
                move || {
                    let value = prepare(record)?;
                    let bytes = resident_bytes(&value);
                    Ok(Loaded { value, bytes })
                },
            )
        };
        let wait = || {
            let until = Instant::now() + Duration::from_secs(15);
            loop {
                if let Some(v) = request() {
                    break v;
                }
                assert!(Instant::now() < until, "{:?}", cache.snapshot());
                std::thread::sleep(Duration::from_millis(2));
            }
        };
        assert!(request().is_none());
        let cold = wait();
        let bytes = resident_bytes(&cold);
        let id = cold.model.id();
        let warm = request().unwrap();
        assert!(Arc::ptr_eq(&cold, &warm));
        assert_eq!(cache.snapshot().loads, 1);
        let mut scene = universe_engine::scene::Scene::default();
        let object = universe_engine::scene::ObjectId(1);
        scene
            .set(
                object,
                universe_engine::scene::Node {
                    geometry: Some(universe_engine::scene::Geometry::Model {
                        model: cold.model.with_lease(cold.clone()),
                        part: 0,
                    }),
                    ..Default::default()
                },
            )
            .unwrap();
        drop(warm);
        drop(cold);
        std::thread::sleep(Duration::from_millis(50));
        cache.maintain();
        assert_eq!(
            cache.snapshot().resident_bytes,
            bytes,
            "scene leases pin resident data"
        );
        let submitted = scene.get(object).unwrap().geometry.clone();
        scene.remove(object);
        cache.maintain();
        assert_eq!(
            cache.snapshot().resident_bytes,
            bytes,
            "submitted PBR handle pins cache after scene removal"
        );
        drop(submitted);
        cache.maintain();
        assert_eq!(cache.snapshot().resident_bytes, 0);
        assert_eq!(cache.snapshot().unloads, 1);
        let returned = wait();
        assert_ne!(returned.model.id(), id);
        let snapshot = cache.snapshot();
        assert_eq!(snapshot.loads, 2);
        assert_eq!(snapshot.reloads, 1);
        assert_eq!(snapshot.resident_bytes, resident_bytes(&returned));
        println!(
            "asset_scene_return {}",
            serde_json::to_string(&snapshot).unwrap()
        );
    }
}
