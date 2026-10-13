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

// One camera sample per scene frame; this is demand metadata, not a second cache.
static VIEW: std::sync::Mutex<
    Option<(
        universe_engine::asset_prediction::View,
        std::time::Instant,
        String,
    )>,
> = std::sync::Mutex::new(None);
pub fn begin_view(camera: &universe_engine::Camera, velocity: DVec3, scene: String) {
    let now = std::time::Instant::now();
    let mut state = VIEW.lock().unwrap();
    let mut angular_speed = 0.;
    let mut velocity = velocity;
    if let Some((old, at, key)) = state.as_ref() {
        let dt = now.duration_since(*at).as_secs_f64();
        let displacement = camera.position - old.position;
        let jumped = *key != scene || (displacement - velocity * dt).length() > 100_000.;
        if jumped {
            crate::visual_assets::cancel_speculation();
        }
        if !jumped && dt > 1e-4 && dt < 1. {
            velocity = displacement / dt;
            angular_speed = old
                .forward
                .dot(camera.forward().as_dvec3())
                .clamp(-1., 1.)
                .acos()
                / dt;
        }
    }
    *state = Some((
        universe_engine::asset_prediction::View {
            position: camera.position,
            velocity,
            forward: camera.forward().as_dvec3(),
            angular_speed,
            horizon_seconds: 5.,
        },
        now,
        scene,
    ));
}
impl Visuals {
    pub fn spatial(
        &self,
        key: &str,
        position: DVec3,
        velocity: DVec3,
        radius: f64,
        reach: f64,
    ) -> Option<Loaded> {
        let state = VIEW.lock().unwrap();
        let demand = state
            .as_ref()?
            .0
            .demand(position, velocity, radius, reach)?;
        drop(state);
        crate::visual_assets::request_demand(key, demand)
    }
}

/// The same measured camera prediction used by model and procedural-world assets.
/// Before the first scene draw (or after a jump), use a stationary current view.
pub(super) fn prediction(camera: &universe_engine::Camera) -> universe_engine::asset_prediction::View {
    let state = VIEW.lock().unwrap();
    if let Some((view, at, _)) = state.as_ref()
        && at.elapsed().as_secs_f64() < 1.
        && view.position.distance(camera.position) < 100_000.
    {
        return universe_engine::asset_prediction::View { position: camera.position, forward: camera.forward().as_dvec3(), ..*view };
    }
    universe_engine::asset_prediction::View { position: camera.position, velocity: DVec3::ZERO, forward: camera.forward().as_dvec3(), angular_speed: 0., horizon_seconds: 5. }
}
