//! Surfaces: a height function over a rail body's sphere. The same function
//! drives contact and whatever draws it, so what is seen is what is touched.

use glam::DVec3;

use crate::rails::OnRails;

/// Heights over a body's base radius, by unit direction in the body's own
/// (rotating) frame.
pub trait Surface {
    /// Height of the solid ground (m), under any liquid.
    fn height(&self, dir: DVec3) -> f64;

    /// Height of what a body touches here (m): the ground, or the flat surface
    /// of a liquid lying over it.
    fn surface(&self, dir: DVec3) -> f64 {
        self.height(dir)
    }

    /// Nothing reaches higher than this (m).
    fn max_height(&self) -> f64;

    /// Is what a body touches here liquid?
    fn liquid(&self, dir: DVec3) -> bool {
        self.surface(dir) > self.height(dir)
    }
}

/// Distance from the center to the surface in a body-frame direction (m).
pub fn surface_radius<B: OnRails + ?Sized>(b: &B, local_dir: DVec3) -> f64 {
    b.rail().radius + b.surface().map_or(0.0, |s| s.surface(local_dir))
}

/// Surface radius under a world-space point at time `t` (`center` = body position).
pub fn surface_radius_at<B: OnRails + ?Sized>(b: &B, center: DVec3, p: DVec3, t: f64) -> f64 {
    match b.surface() {
        Some(_) => surface_radius(b, b.rail().rotation(t).inverse() * (p - center).normalize()),
        None => b.rail().radius,
    }
}

/// Nothing on the surface reaches higher than this (m from the center).
pub fn max_radius<B: OnRails + ?Sized>(b: &B) -> f64 {
    b.rail().radius + b.surface().map_or(0.0, |s| s.max_height())
}
