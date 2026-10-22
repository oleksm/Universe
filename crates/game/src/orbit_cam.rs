//! The orbiting eye every viewer has (the observer, the planet, scene, plant and factory apps): turned
//! by a drag and the left and right keys, tilted by the drag (and up and down where a viewer wants),
//! zoomed by the wheel (and W and S where it wants), looking at a target from `distance` along its
//! yaw and pitch.

use std::f64::consts::FRAC_PI_2;
use universe_engine::glam::{DQuat, DVec3};
use universe_engine::{Camera, Input, KeyCode, MouseButton};

/// A drag turns by this much a pixel (rad); a key by `KEY_TURN` a second; a wheel step zooms by `ZOOM`.
const DRAG: f64 = 0.006;
const KEY_TURN: f64 = 1.2;
const ZOOM: f64 = 0.85;
/// W and S zoom as many wheel steps a second.
const KEY_ZOOM: f64 = 4.0;

#[derive(Clone, Copy, Debug, serde::Serialize, serde::Deserialize)]
pub struct OrbitCam {
    pub yaw: f64,
    pub pitch: f64,
    pub distance: f64,
}

/// Straight up or down, nearly: the most a pitch tilts.
pub const PITCH_LIMIT: f64 = FRAC_PI_2 - 0.01;

impl OrbitCam {
    pub fn new(yaw: f64, pitch: f64, distance: f64) -> Self {
        Self { yaw, pitch, distance }
    }

    /// Turned and tilted by a drag with any of `buttons` held, the tilt within `pitch` (rad).
    pub fn drag(&mut self, input: &Input, buttons: &[MouseButton], pitch: (f64, f64)) {
        if buttons.iter().any(|&b| input.button_down(b)) {
            self.yaw -= input.mouse_delta.x as f64 * DRAG;
            self.pitch = (self.pitch + input.mouse_delta.y as f64 * DRAG).clamp(pitch.0, pitch.1);
        }
    }

    /// Turned by the left and right keys over `dt` (s); tilted by up and down too if `tilt` (within `pitch`).
    pub fn keys(&mut self, input: &Input, dt: f64, tilt: Option<(f64, f64)>) {
        self.yaw += input.axis(KeyCode::ArrowRight, KeyCode::ArrowLeft) as f64 * KEY_TURN * dt;
        if let Some((lo, hi)) = tilt {
            self.pitch = (self.pitch + input.axis(KeyCode::ArrowDown, KeyCode::ArrowUp) as f64 * KEY_TURN * dt).clamp(lo, hi);
        }
    }

    /// Zoomed by the wheel (and W and S over `dt`, if `keys`), kept within `range` (m).
    pub fn zoom(&mut self, input: &Input, dt: f64, keys: bool, range: (f64, f64)) {
        let steps = input.scroll as f64 + if keys { input.axis(KeyCode::KeyS, KeyCode::KeyW) as f64 * dt * KEY_ZOOM } else { 0.0 };
        self.distance = (self.distance * ZOOM.powf(steps)).clamp(range.0, range.1);
    }

    /// The way from the target to the eye, in `frame`.
    pub fn direction(&self, frame: DQuat) -> DVec3 {
        frame * DVec3::new(self.pitch.cos() * self.yaw.sin(), self.pitch.sin(), self.pitch.cos() * self.yaw.cos())
    }

    /// The eye looking at `target`, up along `frame`'s +Y.
    pub fn camera(&self, target: DVec3, frame: DQuat) -> Camera {
        let mut camera = Camera { position: target + self.direction(frame) * self.distance, ..Default::default() };
        camera.look_at(target, (frame * DVec3::Y).as_vec3());
        camera
    }
}
