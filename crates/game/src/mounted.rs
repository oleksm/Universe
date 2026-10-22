//! Fitted equipment visuals shared by flight and the observer studio.
//! Geometry stays in authored glTF metres; COM is subtracted exactly once.
use glam::{DMat4, DVec3};
use serde::Deserialize;
use std::collections::{HashSet};
use std::sync::{Arc, Mutex, OnceLock};
use universe_engine::{Frame, Transform};
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
#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct Overlay {
    hull_sha256: String,
    bindings: Vec<Binding>,
}
// Read development metadata only while the startup factory is gated.
static OVERLAY: OnceLock<Result<Option<Overlay>, String>> = OnceLock::new();
pub fn prepare() {
    OVERLAY.get_or_init(|| {
        let Some(file) = crate::devenv::var_os("UNIVERSE_EQUIPMENT_MOUNTS") else { return Ok(None); };
        use std::io::Read;
        // Development metadata ceiling, not a hull/equipment specification.
        let mut bytes = Vec::new();
        std::fs::File::open(file).map_err(|e| e.to_string())?
            .take(4 * 1024 * 1024 + 1).read_to_end(&mut bytes).map_err(|e| e.to_string())?;
        if bytes.len() > 4 * 1024 * 1024 { return Err("mount overlay exceeds 4 MiB metadata limit".into()); }
        let overlay: Overlay = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
        let mut seen = HashSet::new();
        for b in &overlay.bindings {
            if !seen.insert(&b.slot) { return Err(format!("duplicate overlay slot {}", b.slot)); }
            assets::mounts::rigid(DMat4::from_cols_array(&b.transform))?;
            if !b.replace_nodes.is_empty() {
                return Err("native replacement requires a complete motion/collision consumer; static overlay held".into());
            }
        }
        Ok(Some(overlay))
    });
}
#[derive(Default)]
struct Cache {
    warnings: HashSet<String>,
    announced: HashSet<(String, String)>,
}
impl Cache {
    fn warn(&mut self, message: String) {
        if self.warnings.insert(message.clone()) {
            log::warn!("mounted equipment: {message}");
        }
    }
    fn hull(&mut self, path: &str) -> Option<(Arc<crate::visual_assets::Visual>, &'static [Binding])> {
        let model = crate::visual_assets::request_native(std::path::Path::new(path), &["Ramp"])?;
        let bindings = match OVERLAY.get()? {
            Ok(Some(overlay)) => {
                if overlay.hull_sha256 != model.source_sha256 {
                    self.warn(format!("{path}: mount overlay hull hash mismatch"));
                    return None;
                }
                overlay.bindings.as_slice()
            }
            Ok(None) => &[],
            Err(error) => { self.warn(error.clone()); return None; }
        };
        Some((model, bindings))
    }
    fn item(
        &mut self,
        key: &str,
        slot: &str,
    ) -> Option<(Arc<crate::visual_assets::Visual>, DMat4)> {
        // A pending result is not a rejection. Poll the common service next frame.
        let m = crate::equipment_visual::get(key)?;
        if m.has_motion {
            self.warn(format!(
                "{key} at {slot}: motion contract requires articulated mounted consumer"
            ));
            return None;
        }
        match assets::mounts::placement(DMat4::IDENTITY, &m.nodes, slot) {
            Ok(primary) => Some((m, primary)),
            Err(e) => {
                self.warn(format!("{key} at {slot}: {e}"));
                None
            }
        }
    }
}

pub struct Mounted {
    draws: Vec<(String, Arc<crate::visual_assets::Visual>, DMat4)>,
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
                &model.model.with_lease(model.clone()),
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
    let Some((hull, bindings)) = cache.hull(path) else {
        return result;
    };
    let nodes = &hull.nodes;
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
