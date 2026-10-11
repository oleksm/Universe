//! Fitted equipment visuals shared by flight and the observer studio.
//! Geometry stays in authored glTF metres; COM is subtracted exactly once.
use glam::{DMat4, DVec3};
use serde::Deserialize;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::{Arc, Mutex, OnceLock};
use universe_engine::{Frame, PbrModel, Transform};
use universe_sim::world::{Ship, assets, content::content};

#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct Binding {
    slot: String,
    transform: [f64; 16],
    #[serde(default)]
    replace_nodes: Vec<String>,
    /// Replacements apply only to this exact fitted item, not any future refit.
    #[serde(default)]
    equipment: Option<String>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Overlay {
    hull_sha256: String,
    bindings: Vec<Binding>,
}
struct Hull {
    nodes: BTreeMap<String, DMat4>,
    bindings: Vec<Binding>,
}
#[derive(Default)]
struct Cache {
    hulls: HashMap<String, Option<Arc<Hull>>>,
    items: HashMap<(String, String), Option<(PbrModel, DMat4)>>,
    warnings: HashSet<String>,
    announced: HashSet<(String, String)>,
}
impl Cache {
    fn warn(&mut self, message: String) {
        if self.warnings.insert(message.clone()) {
            log::warn!("mounted equipment: {message}");
        }
    }
    fn hull(&mut self, path: &str) -> Option<Arc<Hull>> {
        if !self.hulls.contains_key(path) {
            let read = || -> Result<_, String> {
                let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
                let nodes = assets::mounts::frames(&bytes)?;
                let mut bindings = Vec::new();
                // A reviewed development socket fixture; production uses hull nodes.
                // Hash refusal prevents applying a frame to a different hull export.
                if let Some(file) = std::env::var_os("UNIVERSE_EQUIPMENT_MOUNTS") {
                    let overlay: Overlay =
                        serde_json::from_slice(&std::fs::read(file).map_err(|e| e.to_string())?)
                            .map_err(|e| e.to_string())?;
                    if overlay.hull_sha256 != universe_sim::world::worlds::sha256(&bytes) {
                        return Err("mount overlay hull hash mismatch".into());
                    }
                    let mut seen = HashSet::new();
                    for b in &overlay.bindings {
                        if !seen.insert(&b.slot) {
                            return Err(format!("duplicate overlay slot {}", b.slot));
                        }
                        assets::mounts::rigid(DMat4::from_cols_array(&b.transform))?;
                        if !b.replace_nodes.is_empty() {
                            return Err("native replacement requires a complete motion/collision consumer; static overlay held".into());
                        }
                    }
                    bindings = overlay.bindings;
                }
                Ok(Arc::new(Hull { nodes, bindings }))
            };
            let result = match read() {
                Ok(v) => Some(v),
                Err(e) => {
                    self.warn(format!("{path}: {e}"));
                    None
                }
            };
            self.hulls.insert(path.into(), result);
        }
        self.hulls.get(path).cloned().flatten()
    }
    fn item(&mut self, key: &str, slot: &str) -> Option<(PbrModel, DMat4)> {
        let k = (key.to_string(), slot.to_string());
        if !self.items.contains_key(&k) {
            let read = || -> Result<_, String> {
                let m = crate::equipment_visual::get(key).ok_or("no usable installed visual")?;
                if m.has_motion {
                    return Err("motion contract requires articulated mounted consumer".into());
                }
                let inverse_primary = assets::mounts::placement(DMat4::IDENTITY, &m.nodes, slot)?;
                Ok((m.model.clone(), inverse_primary))
            };
            let result = match read() {
                Ok(v) => Some(v),
                Err(e) => {
                    self.warn(format!("{key} at {slot}: {e}"));
                    None
                }
            };
            self.items.insert(k.clone(), result);
        }
        self.items.get(&k).cloned().flatten()
    }
}

pub struct Mounted {
    draws: Vec<(String, PbrModel, DMat4)>,
}
impl Mounted {
    pub fn slot_origin(&self, slot: &str) -> Option<DVec3> {
        self.draws
            .iter()
            .find(|(s, _, _)| s == slot)
            .map(|(_, _, m)| m.w_axis.truncate())
    }

    pub fn draw(&self, frame: &mut Frame, hull: &Transform, centre: DVec3) {
        for (_, model, matrix) in &self.draws {
            let Ok((at, rotation)) = assets::mounts::rigid(*matrix) else {
                continue;
            };
            frame.model_pbr(
                model,
                &Transform {
                    position: hull.position + hull.rotation.as_dquat() * (at - centre),
                    rotation: hull.rotation * rotation.as_quat(),
                    scale: 1.0,
                },
            );
        }
    }
}

/// Resolve from the ship's current fitted spec on every frame (including refits),
/// caching immutable packages. No catalog-wide auto-fit or automatic recentering.
pub fn for_ship(ship: &Ship) -> Mounted {
    static CACHE: OnceLock<Mutex<Cache>> = OnceLock::new();
    let mut cache = CACHE
        .get_or_init(Default::default)
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    let mut result = Mounted { draws: Vec::new() };
    let spec = ship.spec();
    let Some(path) = spec.visual.as_deref() else {
        return result;
    };
    let Some(hull) = cache.hull(path) else {
        return result;
    };
    let Hull { nodes, bindings } = hull.as_ref();
    for (slot, module) in &spec.fit {
        let key = &content().get(*module).key;
        if assets::visual(key).is_none() {
            continue;
        }
        let binding = bindings.iter().find(|b| &b.slot == slot);
        if binding.is_some_and(|b| b.equipment.as_ref().is_some_and(|e| e != key)) {
            cache.warn(format!("{slot}: overlay is for another fitted item"));
            continue;
        }
        let mount = binding
            .map(|b| DMat4::from_cols_array(&b.transform))
            .or_else(|| nodes.get(&format!("mount_{slot}")).copied());
        let Some(mount) = mount else {
            cache.warn(format!(
                "{key} at {slot}: missing mount_{slot}; native drawing retained"
            ));
            continue;
        };
        if let Err(e) = assets::mounts::rigid(mount) {
            cache.warn(format!("{slot}: {e}"));
            continue;
        }
        let Some((model, primary)) = cache.item(key, slot) else {
            continue;
        };
        let matrix = mount * primary;
        if assets::mounts::rigid(matrix).is_err() {
            continue;
        }
        if cache
            .announced
            .insert((path.into(), format!("{slot}:{key}")))
        {
            log::info!(
                "mounted equipment: drawing {key} at {slot}, model-frame origin {:?}",
                matrix.w_axis.truncate()
            );
        }
        result.draws.push((slot.clone(), model, matrix));
    }

    result
}
