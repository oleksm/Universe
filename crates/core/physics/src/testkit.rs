//! Small fixtures shared by Dogma's tests.

use glam::DQuat;

use crate::collide::Collider;
use crate::orbit::Orbit;
use crate::rails::RailBody;

/// A plain attracting body with a surface collider and a one-day spin.
pub fn body(parent: Option<usize>, orbit: Option<Orbit>, mu: f64, radius: f64) -> RailBody {
    RailBody { parent, orbit, mu, attracts: true, radius, tilt: DQuat::IDENTITY, day: 86_400.0, collider: Collider::Surface, atmosphere: None, pulled_by: Vec::new() }
}
