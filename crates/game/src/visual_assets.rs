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
    pub model: PbrModel,
    pub faces: Arc<Vec<crate::equipment_visual::Face>>,
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

// Provisional reservation, not a proven decoder peak bound. The engine refuses
// publication if retained allocations exceed this; malformed/compressed inputs
// still require decoder-side limits. No filesystem metadata on the request path.
const RESERVATION: usize = 256 * 1024 * 1024;
pub fn request(key: &str, class: u8) -> Option<Arc<Visual>> {
    let record = assets::visual(key)?;
    let identity = format!(
        "{}:{}:{}:{}",
        key, record.format, record.path, record.manifest_sha256
    );
    let key = key.to_owned();
    cache().request(
        identity,
        Demand {
            class,
            time_to_need: if class > 1 {
                Duration::from_secs(2)
            } else {
                Duration::ZERO
            },
            estimated_bytes: RESERVATION,
            ..Demand::default()
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
fn prepare(record: &universe_sim::world::registry::Visual) -> Result<Visual, String> {
    let package = assets::model(record)?;
    // A sidecar can describe future motion while the GLB remains a
    // valid neutral rigid model. Do not silently ignore skin/morph data.
    let (nodes, neutral_error) = match assets::mounts::require_static(&package.glb)
        .and_then(|()| assets::mounts::frames(&package.glb))
    {
        Ok(nodes) => (nodes, None),
        Err(e) => (BTreeMap::new(), Some(e)),
    };
    let model = PbrModel::load_gltf(&package.glb)?;
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
    let mut faces = Vec::new();
    for primitive in &model.data.primitives {
        let material = model.data.materials.get(primitive.material);
        for idx in primitive.indices.chunks_exact(3) {
            let vs = [idx[0], idx[1], idx[2]].map(|i| primitive.vertices[i as usize]);
            faces.push(crate::equipment_visual::Face {
                positions: vs.map(|v| Vec3::from(v.pos)),
                normals: vs.map(|v| Vec3::from(v.normal)),
                colour: universe_engine::Color(
                    material.map_or([0.6, 0.6, 0.6, 1.], |m| m.base_color),
                ),
                double_sided: material.is_some_and(|m| m.double_sided),
                blend: material.is_some_and(|m| m.blend),
                alpha_cutoff: material.and_then(|m| m.alpha_cutoff),
            });
        }
    }
    Ok(Visual {
        faces: Arc::new(faces),
        model,
        centre: (lo + hi) * 0.5,
        nodes,
        has_motion: package.has_motion,
        lo: glam::DVec3::from(package.lo),
        hi: glam::DVec3::from(package.hi),
        neutral_error,
    })
}
fn resident_bytes(v: &Visual) -> usize {
    let d = &v.model.data;
    std::mem::size_of::<Visual>()
        + d.primitives.capacity() * std::mem::size_of::<universe_engine::pbr::Primitive>()
        + d.materials.capacity() * std::mem::size_of::<universe_engine::pbr::Material>()
        + d.images.capacity() * std::mem::size_of::<universe_engine::pbr::Image>()
        + d.primitives.iter().map(|p| p.vertices.capacity() * std::mem::size_of::<universe_engine::pbr::PbrVertex>() + p.indices.capacity() * 4).sum::<usize>()
        + d.images.iter().map(|i| i.rgba.capacity()).sum::<usize>()
        + v.faces.capacity() * std::mem::size_of::<crate::equipment_visual::Face>()
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
        std::thread::sleep(Duration::from_millis(50));
        cache.maintain();
        assert_eq!(
            cache.snapshot().resident_bytes,
            bytes,
            "scene leases pin resident data"
        );
        drop(warm);
        drop(cold);
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
    #[test]
    fn corrupt_manifest_fails_in_worker_and_reports_reason() {
        use std::time::Instant;
        if assets::store().is_none() {
            return;
        }
        let original = assets::visual("equipment.engine.ch.s2").unwrap();
        let record = universe_sim::world::registry::Visual {
            format: original.format.clone(),
            path: original.path.clone(),
            manifest_sha256: "0".repeat(64),
            version: original.version,
            triangles: original.triangles,
            note: None,
        };
        let cache = AssetCache::new(Config::default());
        assert!(
            cache
                .request(
                    "corrupt".into(),
                    Demand {
                        estimated_bytes: RESERVATION,
                        ..Default::default()
                    },
                    move || {
                        let value = prepare(&record)?;
                        let bytes = resident_bytes(&value);
                        Ok(Loaded { value, bytes })
                    }
                )
                .is_none()
        );
        let until = Instant::now() + Duration::from_secs(5);
        loop {
            let s = cache.snapshot();
            if s.failures > 0 {
                assert_eq!(s.loads, 0);
                assert_eq!(s.resident_bytes, 0);
                assert_eq!(s.reserved_bytes, 0);
                assert_eq!(s.entries[0].state, "failed");
                assert!(s.entries[0].error.as_ref().is_some_and(|e| !e.is_empty()));
                break;
            }
            assert!(Instant::now() < until, "{s:?}");
            std::thread::sleep(Duration::from_millis(2));
        }
    }
}
