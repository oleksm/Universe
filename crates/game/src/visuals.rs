//! The models of the things the registry describes (`world::assets`), loaded once and drawn where the
//! things stand; a thing without one, or whose package fails its checks, keeps its box.

use std::sync::Arc;
use universe_engine::glam::{DQuat, DVec3};
use universe_engine::{Frame, Transform};

pub type Loaded = Arc<crate::visual_assets::Visual>;
#[derive(Default)]
pub struct Visuals;
impl Visuals {
    pub fn get(&self, key: &str) -> Option<Loaded> {
        crate::visual_assets::request(key, 1)
    }
}

/// `l` with its bounds' centre at `centre`, turned `rot` (a thing standing free: a rig, a gate).
pub fn centred(frame: &mut Frame, l: &Loaded, centre: DVec3, rot: DQuat) {
    let mid = (l.lo + l.hi) / 2.0;
    frame.model_pbr(
        &l.model.with_lease(l.clone()),
        &Transform {
            position: centre - rot * mid,
            rotation: rot.as_quat(),
            scale: 1.0,
        },
    );
}

/// `l` standing on the ground: its bounds' foot centred on `base` (in the ground's frame `ground`: x east,
/// y up, z south), turned `heading` degrees about the up.
pub fn standing(frame: &mut Frame, l: &Loaded, ground: &Transform, base: DVec3, heading: f64) {
    let foot = DVec3::new((l.lo.x + l.hi.x) / 2.0, l.lo.y, (l.lo.z + l.hi.z) / 2.0);
    let rot = ground.rotation.as_dquat() * DQuat::from_rotation_y(heading.to_radians());
    let at = ground.position + ground.rotation.as_dquat() * base - rot * foot;
    frame.model_pbr(
        &l.model.with_lease(l.clone()),
        &Transform {
            position: at,
            rotation: rot.as_quat(),
            scale: 1.0,
        },
    );
}
