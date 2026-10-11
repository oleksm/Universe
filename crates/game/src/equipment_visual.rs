//! Shared immutable, hash-checked equipment geometry. Consumers decide whether
//! a neutral preview or a fully operational mounted mechanism is permitted.
use glam::{DMat4, Vec3};
use std::{
    collections::{BTreeMap, HashMap},
    sync::{Arc, Mutex, OnceLock},
};
use universe_engine::PbrModel;
use universe_sim::world::assets;

pub struct Visual {
    pub model: PbrModel,
    pub centre: Vec3,
    pub nodes: BTreeMap<String, DMat4>,
    pub has_motion: bool,
}

pub fn get(key: &str) -> Option<Arc<Visual>> {
    static CACHE: OnceLock<Mutex<HashMap<String, Option<Arc<Visual>>>>> = OnceLock::new();
    let mut cache = CACHE
        .get_or_init(Default::default)
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    cache
        .entry(key.into())
        .or_insert_with(|| {
            let record = assets::visual(key)?;
            let read = || -> Result<Visual, String> {
                let package = assets::model(record)?;
                // A sidecar can describe future motion while the GLB remains a
                // valid neutral rigid model. Do not silently ignore skin/morph data.
                assets::mounts::require_static(&package.glb)?;
                let nodes = assets::mounts::frames(&package.glb)?;
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
                Ok(Visual {
                    model,
                    centre: (lo + hi) * 0.5,
                    nodes,
                    has_motion: package.has_motion,
                })
            };
            match read() {
                Ok(v) => {
                    log::info!("equipment visual: loaded {key} for neutral drawing");
                    Some(Arc::new(v))
                }
                Err(e) => {
                    log::warn!("equipment visual: {key}: {e}; retaining design envelope");
                    None
                }
            }
        })
        .clone()
}
