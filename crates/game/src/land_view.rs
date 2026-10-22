//! A world whose ground is a land package (`worlds::land`), drawn by the terrain subsystem (`universe_terrain`) exactly as
//! `freefall --app planetlod <package>` draws it: opened on a thread of its own when the eye comes within
//! `OPEN_RADII` of the world, let go past `CLOSE_RADII` (its tile cache with it). The package keeps its
//! frame; the world places it (its centre, spin and the scale from the package's radius to its own) and
//! the renderer is given the eye in the package's frame.

use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver};
use universe_engine::glam::{DQuat, DVec3};
use universe_engine::{Camera, Frame, Transform};
use universe_sim::world::worlds::land;

use universe_terrain::Ground;

/// Opened within this many radii of the world's centre, let go beyond the other.
const OPEN_RADII: f64 = 30.0;
const CLOSE_RADII: f64 = 40.0;

enum State {
    Opening(Receiver<Result<Ground, String>>),
    Open(Box<Ground>),
    Failed,
}

pub struct LandView {
    /// (system cache, body) it is for.
    pub at: (usize, usize),
    state: State,
    /// The body's radius over the package's.
    scale: f64,
    /// When it last drew (for the renderer's step).
    last: Option<std::time::Instant>,
    /// Frames drawn since the log line written each second, and when that was.
    log: (u32, Option<std::time::Instant>),
}

/// Where the package lies: the body's centre and spin, turned from the package's frame, scaled to it.
fn body_transform(center: DVec3, rotation: DQuat, scale: f64) -> Transform {
    Transform { position: center, rotation: (rotation * land::to_body()).as_quat(), scale }
}

/// The eye in the package's frame.
fn local_camera(cam: &Camera, body: &Transform) -> Camera {
    let r = body.rotation.as_dquat().normalize();
    Camera {
        position: r.inverse() * (cam.position - body.position) / body.scale,
        orientation: (r.inverse() * cam.orientation.as_dquat()).as_quat(),
        fov_y: cam.fov_y,
        near: (cam.near as f64 / body.scale) as f32,
    }
}

impl LandView {
    /// Keep `slot` on the world whose ground is a land package nearest the eye, within OPEN_RADII.
    pub fn maintain(slot: &mut Option<LandView>, system: &universe_sim::StarSystem, cache: usize, positions: &[DVec3], eye: DVec3) {
        let away = |i: usize| positions.get(i).map_or(f64::INFINITY, |c| c.distance(eye)) / system.bodies[i].rail.radius;
        if let Some(v) = slot
            && (v.at.0 != cache || v.at.1 >= system.bodies.len() || away(v.at.1) > CLOSE_RADII)
        {
            *slot = None;
        }
        if slot.is_some() {
            return;
        }
        let near = (0..system.bodies.len())
            .filter(|&i| away(i) < OPEN_RADII)
            .filter_map(|i| Some((i, system.bodies[i].terrain.as_ref()?.land()?.1)))
            .min_by(|a, b| away(a.0).total_cmp(&away(b.0)));
        let Some((i, scale)) = near else { return };
        let key = system.bodies[i].key.clone();
        let Some(Ok((dir, _))) = land::ground(&key) else { return };
        let (tx, rx) = mpsc::sync_channel(1);
        crate::background::spawn(move || {
            let t0 = std::time::Instant::now();
            let app = Ground::open(&PathBuf::from(&dir));
            match &app {
                Ok(_) => log::info!("{key}: land package open in {:.1} s ({})", t0.elapsed().as_secs_f64(), dir.display()),
                Err(e) => log::warn!("{key}: land package not opened: {e}"),
            }
            let _ = tx.send(app);
        });
        *slot = Some(LandView { at: (cache, i), state: State::Opening(rx), scale, last: None, log: (0, None) });
    }

    /// The renderer's step for this frame and its ground drawn, the eye `frame.camera`; false: not open yet.
    pub fn draw(&mut self, frame: &mut Frame, center: DVec3, rotation: DQuat, physics: impl Fn(DVec3) -> f64) -> bool {
        if let State::Opening(rx) = &self.state {
            match rx.try_recv() {
                Ok(Ok(app)) => self.state = State::Open(Box::new(app)),
                Ok(Err(_)) | Err(mpsc::TryRecvError::Disconnected) => self.state = State::Failed,
                Err(mpsc::TryRecvError::Empty) => {}
            }
        }
        let State::Open(app) = &mut self.state else { return false };
        let now = std::time::Instant::now();
        let dt = self.last.map_or(0.0, |l| (now - l).as_secs_f64());
        self.last = Some(now);
        let size = frame.size();
        let aspect = (size.x / size.y.max(1.0)) as f64;
        let body = body_transform(center, rotation, self.scale);
        let cam = local_camera(&frame.camera, &body);
        app.update(&cam, aspect, dt);
        app.draw(frame, &cam, &body);
        self.log.0 += 1;
        let since = *self.log.1.get_or_insert(now);
        if (now - since).as_secs_f64() >= 1.0 {
            let alt = cam.position.length() * self.scale - self.scale * app.package_radius();
            // (The ground under the eye as drawn and as contact finds it, body metres over its radius.)
            let up = cam.position.normalize();
            let drawn = app.drawn_ground_at_world(up).map(|h| h * self.scale);
            let solid = physics(rotation.inverse().normalize() * (body.rotation.as_dquat().normalize() * up));
            log::info!("LAND fps={:.1} alt_m={:.0} drawn_ground_m={} contact_ground_m={solid:.2} {}", self.log.0 as f64 / (now - since).as_secs_f64(), alt, drawn.map_or("none".into(), |d| format!("{d:.2}")), app.pyramid_line().unwrap_or_default());
            self.log = (0, Some(now));
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The eye taken into the package's frame and the package's points placed back land where they were.
    #[test]
    fn land_view_places_the_package_and_the_eye_consistently() {
        let body = body_transform(DVec3::new(1e11, -2e10, 3e9), DQuat::from_rotation_z(0.7) * DQuat::from_rotation_x(-0.3), 6_281_370.0 / 6_371_000.0);
        let cam = Camera { position: body.position + DVec3::new(3e6, 5e6, -4.1e6), orientation: universe_engine::glam::Quat::from_rotation_y(0.4), fov_y: 1.0, near: 0.1 };
        let local = local_camera(&cam, &body);
        let back = universe_terrain::placed(&body, local.position).position;
        assert!((back - cam.position).length() < 1e-3, "{}", (back - cam.position).length());
        // (The view direction too: what is ahead of the eye in the scene is ahead of it in the package.)
        let ahead = cam.position + cam.forward().as_dvec3() * 1000.0;
        let ahead_local = body.rotation.as_dquat().normalize().inverse() * (ahead - body.position) / body.scale;
        let f = local.forward().as_dvec3();
        assert!((ahead_local - local.position).normalize().dot(f) > 0.999_999);
    }
}
