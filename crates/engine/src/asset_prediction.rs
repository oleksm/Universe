//! Conservative visual demand in wall-clock seconds. Values are renderer tuning,
//! never collision or physical visibility. A swept sphere catches lateral motion.
use crate::assets::Demand;
use glam::DVec3;
use std::time::Duration;

#[derive(Clone, Copy, Debug)]
pub struct View {
    pub position: DVec3,
    /// World metres per wall-clock second, including simulation time scaling.
    pub velocity: DVec3,
    pub forward: DVec3,
    pub angular_speed: f64,
    pub horizon_seconds: f64,
}
impl View {
    pub fn demand(
        &self,
        position: DVec3,
        velocity: DVec3,
        radius: f64,
        visible_distance: f64,
    ) -> Option<Demand> {
        if !self.horizon_seconds.is_finite()
            || !self.angular_speed.is_finite()
            || !self.forward.is_finite()
            || !self.position.is_finite()
            || !self.velocity.is_finite()
            || !position.is_finite()
            || !velocity.is_finite()
            || !radius.is_finite()
            || !visible_distance.is_finite()
        {
            return None;
        }
        let p = position - self.position;
        let v = velocity - self.velocity;
        let reach = visible_distance.max(0.) + radius.max(0.);
        let distance = p.length();
        let facing = p.normalize_or_zero().dot(self.forward.normalize_or_zero());
        let horizon = self.horizon_seconds.clamp(0., 30.);
        let turn = (self.angular_speed.abs() * horizon).min(std::f64::consts::PI);
        if distance <= reach {
            let facing_view = facing >= (1.2 + turn).min(std::f64::consts::PI).cos();
            // Keep only a short rear neighbourhood warm. Re-requesting the
            // entire rear visibility radius every frame would prevent eviction.
            if !facing_view && distance > reach * 0.25 {
                return None;
            }
            return Some(Demand {
                class: if facing_view { 1 } else { 2 },
                time_to_need: Duration::ZERO,
                ..Default::default()
            });
        }
        // Closest swept approach, followed by the first sphere intersection.
        let a = v.length_squared();
        if a > 1e-12 {
            let b = p.dot(v);
            let c = p.length_squared() - reach * reach;
            let disc = b * b - a * c;
            if b < 0. && disc >= 0. {
                let enter = c / (-b + disc.sqrt());
                if enter.is_finite() && enter <= horizon {
                    return Some(Demand {
                        class: 2,
                        time_to_need: Duration::from_secs_f64(enter.max(0.)),
                        ..Default::default()
                    });
                }
            }
        }
        // Modest surrounding coverage makes camera turns cheap without loading
        // every distant object. It is speculative and expires unless renewed.
        (distance <= reach * 1.5 && (facing >= 0. || turn > 0.2)).then_some(Demand {
            class: 3,
            time_to_need: Duration::from_secs_f64(horizon),
            ..Default::default()
        })
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn view() -> View {
        View {
            position: DVec3::ZERO,
            velocity: DVec3::ZERO,
            forward: DVec3::NEG_Z,
            angular_speed: 0.,
            horizon_seconds: 5.,
        }
    }
    #[test]
    fn swept_lateral_and_receding() {
        let v = view();
        let d = v
            .demand(DVec3::new(20., 0., 0.), DVec3::new(-4., 0., 0.), 0., 10.)
            .unwrap();
        assert_eq!(d.class, 2);
        assert!((d.time_to_need.as_secs_f64() - 2.5).abs() < 1e-9);
        assert!(
            v.demand(DVec3::new(20., 0., 0.), DVec3::X, 0., 10.)
                .is_none()
        );
    }
    #[test]
    fn stationary_nearby_turn_and_large_silhouette() {
        let mut v = view();
        assert_eq!(
            v.demand(DVec3::Z * 2., DVec3::ZERO, 0., 10.).unwrap().class,
            2
        );
        assert!(v.demand(DVec3::Z * 8., DVec3::ZERO, 0., 10.).is_none());
        v.angular_speed = 1.;
        assert_eq!(
            v.demand(DVec3::Z * 2., DVec3::ZERO, 0., 10.).unwrap().class,
            1
        );
        assert!(
            v.demand(DVec3::NEG_Z * 100., DVec3::ZERO, 95., 10.)
                .is_some()
        );
    }
}
