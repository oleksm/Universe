//! The models of the things the registry describes (`world::assets`), loaded once and drawn where the
//! things stand; a thing without one, or whose package fails its checks, keeps its box.

use std::cell::RefCell;
use std::collections::HashMap;

use universe_engine::glam::{DQuat, DVec3};
use universe_engine::{Frame, PbrModel, Transform};

/// A loaded model and its bounds in its own frame (m).
#[derive(Clone)]
pub struct Loaded {
    pub model: PbrModel,
    pub lo: DVec3,
    pub hi: DVec3,
}

/// The models loaded so far, by record key (None: it has none, or it failed: said once).
#[derive(Default)]
pub struct Visuals {
    loaded: RefCell<HashMap<String, Option<Loaded>>>,
}

impl Visuals {
    /// The model of the record `key`, loaded the first time it's asked for.
    pub fn get(&self, key: &str) -> Option<Loaded> {
        if let Some(l) = self.loaded.borrow().get(key) {
            return l.clone();
        }
        let loaded = universe_sim::world::assets::visual(key).and_then(|v| {
            let made = universe_sim::world::assets::model(v).and_then(|m| PbrModel::load_gltf(&m.glb).map(|model| Loaded { model, lo: DVec3::from(m.lo), hi: DVec3::from(m.hi) }));
            made.map_err(|e| log::warn!("{key}: its model is not drawn ({e}); its box instead")).ok()
        });
        self.loaded.borrow_mut().insert(key.to_string(), loaded.clone());
        loaded
    }
}

/// `l` with its bounds' centre at `centre`, turned `rot` (a thing standing free: a rig, a gate).
pub fn centred(frame: &mut Frame, l: &Loaded, centre: DVec3, rot: DQuat) {
    let mid = (l.lo + l.hi) / 2.0;
    frame.model_pbr(&l.model, &Transform { position: centre - rot * mid, rotation: rot.as_quat(), scale: 1.0 });
}

/// `l` standing on the ground: its bounds' foot centred on `base` (in the ground's frame `ground`: x east,
/// y up, z south), turned `heading` degrees about the up.
pub fn standing(frame: &mut Frame, l: &Loaded, ground: &Transform, base: DVec3, heading: f64) {
    let foot = DVec3::new((l.lo.x + l.hi.x) / 2.0, l.lo.y, (l.lo.z + l.hi.z) / 2.0);
    let rot = ground.rotation.as_dquat() * DQuat::from_rotation_y(heading.to_radians());
    let at = ground.position + ground.rotation.as_dquat() * base - rot * foot;
    frame.model_pbr(&l.model, &Transform { position: at, rotation: rot.as_quat(), scale: 1.0 });
}
