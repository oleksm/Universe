//! Small fixtures shared by the kernel's tests.

use glam::{DQuat, DVec3};

use crate::collide::Collider;
use crate::orbit::Orbit;
use crate::rails::{OnRails, RailBody};
use crate::surface::Surface;

/// A plain attracting body with a surface collider and a one-day spin.
pub fn body(parent: Option<usize>, orbit: Option<Orbit>, mu: f64, radius: f64) -> RailBody {
    RailBody { parent, orbit, mu, attracts: true, radius, tilt: DQuat::IDENTITY, day: 86_400.0, collider: Collider::Surface }
}

/// Sea 500 m deep over the +X hemisphere, ground 1 km up elsewhere.
pub struct Hemispheres;

impl Surface for Hemispheres {
    fn height(&self, dir: DVec3) -> f64 {
        if dir.x > 0.0 { -500.0 } else { 1000.0 }
    }

    fn surface(&self, dir: DVec3) -> f64 {
        self.height(dir).max(0.0)
    }

    fn max_height(&self) -> f64 {
        1000.0
    }
}

/// A rail body with the `Hemispheres` surface.
pub struct Ocean(pub RailBody);

impl OnRails for Ocean {
    fn rail(&self) -> &RailBody {
        &self.0
    }

    fn surface(&self) -> Option<&dyn Surface> {
        Some(&Hemispheres)
    }
}
