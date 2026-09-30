use glam::camera::rh::proj::directx::perspective_infinite_reverse;
use glam::{DVec3, Mat3, Mat4, Quat, Vec3};

/// A free 6-DOF camera. Looks down its local -Z, with +Y up (right-handed).
#[derive(Clone, Copy, Debug)]
pub struct Camera {
    pub position: DVec3,
    pub orientation: Quat,
    /// Vertical field of view, radians.
    pub fov_y: f32,
    pub near: f32,
}

impl Default for Camera {
    fn default() -> Self {
        Self {
            position: DVec3::ZERO,
            orientation: Quat::IDENTITY,
            fov_y: 60f32.to_radians(),
            near: 0.05,
        }
    }
}

impl Camera {
    pub fn forward(&self) -> Vec3 {
        self.orientation * Vec3::NEG_Z
    }

    pub fn right(&self) -> Vec3 {
        self.orientation * Vec3::X
    }

    pub fn up(&self) -> Vec3 {
        self.orientation * Vec3::Y
    }

    /// Rotate around the camera's own axes (radians).
    pub fn rotate_local(&mut self, pitch: f32, yaw: f32, roll: f32) {
        let q = Quat::from_rotation_y(yaw) * Quat::from_rotation_x(pitch) * Quat::from_rotation_z(roll);
        self.orientation = (self.orientation * q).normalize();
    }

    pub fn look_at(&mut self, target: DVec3, up: Vec3) {
        // Normalize in f64: squaring galactic distances overflows f32.
        let forward = (target - self.position).normalize().as_vec3();
        let right = forward.cross(up).normalize();
        let up = right.cross(forward);
        self.orientation = Quat::from_mat3(&Mat3::from_cols(right, up, -forward));
    }

    /// View-projection for camera-relative positions: rotation only, no translation.
    /// Uses reversed-Z with an infinite far plane for precision at any distance.
    pub fn view_proj(&self, aspect: f32) -> Mat4 {
        let proj = perspective_infinite_reverse(self.fov_y, aspect, self.near);
        proj * Mat4::from_quat(self.orientation.inverse())
    }

    /// World position to camera-relative `f32` position.
    pub fn relative(&self, p: DVec3) -> Vec3 {
        (p - self.position).as_vec3()
    }
}
