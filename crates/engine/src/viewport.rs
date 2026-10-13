//! Shared camera projection, unprojection and viewport coordinates.
use crate::camera::Camera;
use glam::{DVec3, Mat4, Vec2, Vec3};

#[derive(Clone, Copy, Debug)]
pub enum Projection {
    Perspective,
    Orthographic { height: f32, far: f32 },
}
impl Default for Projection {
    fn default() -> Self {
        Self::Perspective
    }
}
#[derive(Clone, Copy, Debug)]
pub struct PixelRect {
    pub origin: Vec2,
    pub size: Vec2,
}
#[derive(Clone, Copy, Debug)]
pub struct Ray {
    pub origin: DVec3,
    pub direction: DVec3,
}
#[derive(Clone, Copy, Debug)]
pub struct View {
    pub camera: Camera,
    pub projection: Projection,
    pub rect: PixelRect,
    pub layers: u64,
}
impl View {
    pub fn matrix(&self) -> Mat4 {
        self.projection
            .matrix(&self.camera, self.rect.size.x / self.rect.size.y)
    }
    pub fn project(&self, point: DVec3) -> Option<Vec3> {
        let c = self.matrix() * self.camera.relative(point).extend(1.);
        if c.w <= 0. {
            return None;
        }
        let p = c.truncate() / c.w;
        if !(0.0..=1.0).contains(&p.z) {
            return None;
        }
        let xy = self.rect.origin + Vec2::new((p.x + 1.) * 0.5, (1. - p.y) * 0.5) * self.rect.size;
        Some(xy.extend(p.z))
    }
    pub fn ray(&self, pixel: Vec2) -> Option<Ray> {
        if !pixel.is_finite() || !self.rect.size.is_finite() || self.rect.size.min_element() <= 0. {
            return None;
        }
        let q = (pixel - self.rect.origin) / self.rect.size;
        if q.min_element() < 0. || q.max_element() > 1. {
            return None;
        }
        let inverse = self.matrix().inverse();
        let near = inverse * Vec3::new(q.x * 2. - 1., 1. - q.y * 2., 1.).extend(1.);
        let mid = inverse * Vec3::new(q.x * 2. - 1., 1. - q.y * 2., 0.5).extend(1.);
        let a = (near.truncate() / near.w).as_dvec3();
        let b = (mid.truncate() / mid.w).as_dvec3();
        let direction = (b - a).try_normalize()?;
        Some(Ray {
            origin: self.camera.position + a,
            direction,
        })
    }
}
impl Projection {
    pub fn matrix(self, camera: &Camera, aspect: f32) -> Mat4 {
        match self {
            Self::Perspective => camera.view_proj(aspect),
            Self::Orthographic { height, far } => {
                assert!(height.is_finite() && height > 0. && far.is_finite() && far > camera.near);
                glam::camera::rh::proj::directx::orthographic(
                    -height * aspect * 0.5,
                    height * aspect * 0.5,
                    -height * 0.5,
                    height * 0.5,
                    far,
                    camera.near,
                ) * Mat4::from_quat(camera.orientation.inverse())
            }
        }
    }
}
