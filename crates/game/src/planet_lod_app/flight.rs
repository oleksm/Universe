//! Terrain-relative flight: broad spatial reference, a one-second spring, and immediate peak clearance.
use super::DVec3;

#[derive(Default)]
pub(super) struct Reference {
    pub height: Option<f64>,
    pub target: Option<f64>,
    velocity: f64,
}
impl Reference {
    /// Heights are world metres (including exaggeration). Altitude is above the reference.
    pub fn update(&mut self, mean: Option<f64>, real: Option<f64>, alt: f64, dt: f64) {
        let mean = mean.filter(|x| x.is_finite());
        let real = real.filter(|x| x.is_finite());
        self.target = mean;
        let Some(mut height) = self.height.or(mean).or(real) else {
            return;
        };
        if let Some(target) = mean {
            // Exact critically damped solution for a held target. ~95% settled after one second.
            let omega = 4.75;
            let dt = dt.max(0.0);
            let error = height - target;
            let c = self.velocity + omega * error;
            let decay = (-omega * dt).exp();
            height = target + (error + c * dt) * decay;
            self.velocity = (self.velocity - omega * c * dt) * decay;
        } else {
            self.velocity = 0.0; // Missing streamed samples must not cause drift or a reset to sea level.
        }
        if let Some(ground) = real {
            let floor = ground - 0.7 * alt;
            if height < floor {
                height = floor;
                self.velocity = self.velocity.max(0.0);
            }
        }
        self.height = Some(height);
    }
}

/// Average the available drawn ground over a disc of radius twice the nominal altitude.
pub(super) fn average(
    up: DVec3,
    alt: f64,
    radius: f64,
    mut sample: impl FnMut(DVec3) -> Option<f64>,
) -> Option<f64> {
    let east = DVec3::Y.cross(up).normalize_or(DVec3::X);
    let north = up.cross(east);
    let angle = (2.0 * alt / radius).clamp(0.0, std::f64::consts::FRAC_PI_2);
    let mut sum = 0.0;
    let mut count = 0;
    let mut add = |dir| {
        if let Some(h) = sample(dir).filter(|x| x.is_finite()) {
            sum += h;
            count += 1;
        }
    };
    add(up);
    // Equal-weight rings approximate an area average, rather than giving distant terrain extra weight.
    for (fraction, n) in [(0.5, 8), (0.8660254037844386, 16)] {
        let a = angle * fraction;
        for i in 0..n {
            let bearing = std::f64::consts::TAU * i as f64 / n as f64;
            add(up * a.cos() + (east * bearing.cos() + north * bearing.sin()) * a.sin());
        }
    }
    (count > 0).then(|| sum / count as f64)
}

/// Radial ray against an actual submitted triangle. Returns world height above the datum.
pub(super) fn triangle_height(up: DVec3, [a, b, c]: [DVec3; 3], radius: f64) -> Option<f64> {
    let (ab, ac) = (b - a, c - a);
    let normal = ab.cross(ac);
    let denom = normal.dot(up);
    if denom.abs() < 1e-12 {
        return None;
    }
    let distance = normal.dot(a) / denom;
    if distance <= 0.0 {
        return None;
    }
    let hit = up * distance - a;
    let (aa, bb, cc) = (ab.dot(ab), ab.dot(ac), ac.dot(ac));
    let det = aa * cc - bb * bb;
    if det <= 0.0 {
        return None;
    }
    let (u, v) = (
        (cc * hit.dot(ab) - bb * hit.dot(ac)) / det,
        (aa * hit.dot(ac) - bb * hit.dot(ab)) / det,
    );
    (u >= -1e-6 && v >= -1e-6 && u + v <= 1.0 + 1e-6).then_some(distance - radius)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn flight_spring_settles_without_overshoot_and_is_frame_rate_independent() {
        let mut results = vec![];
        for fps in [15, 30, 60, 120, 240] {
            let mut s = Reference::default();
            s.update(Some(0.), None, 10000., 0.);
            let mut before = 0.;
            for _ in 0..fps {
                s.update(Some(1000.), None, 10000., 1. / fps as f64);
                let h = s.height.unwrap();
                assert!(h >= before && h <= 1000.);
                before = h;
            }
            results.push(s.height.unwrap());
        }
        assert!(results.iter().all(|h| (*h - results[0]).abs() < 1e-8));
        assert!(results[0] > 949. && results[0] < 951.);
    }
    #[test]
    fn flight_reference_ignores_point_jitter_until_clearance_is_needed() {
        let mut s = Reference::default();
        s.update(Some(2000.), Some(2000.), 10000., 0.);
        for real in [0., 6000., 1000., 7000.] {
            s.update(Some(2000.), Some(real), 10000., 1. / 120.);
            assert_eq!(s.height, Some(2000.));
        }
        s.update(Some(2000.), Some(15000.), 10000., 1. / 120.);
        assert_eq!(s.height, Some(8000.));
        assert!((s.height.unwrap() + 10000. - 15000. - 3000.).abs() < 1e-9);
        s.update(None, None, 10000., 0.5);
        assert_eq!(s.height, Some(8000.));
        for _ in 0..120 {
            s.update(Some(2000.), Some(0.), 10000., 1. / 120.);
        }
        assert!(s.height.unwrap() < 2310. && s.height.unwrap() > 2290.);
    }
    #[test]
    fn flight_footprint_rejects_missing_samples_and_dilutes_small_peaks() {
        let mut sampled = 0;
        let h = average(DVec3::Z, 1000., 6371000., |u| {
            sampled += 1;
            assert!(u.is_normalized());
            assert!(u.dot(DVec3::Z).acos() <= 2000. / 6371000.);
            Some(if sampled == 1 { 2500. } else { 0. })
        })
        .unwrap();
        assert_eq!(sampled, 25);
        assert_eq!(h, 100.);
        assert_eq!(average(DVec3::Z, 1000., 6371000., |_| None), None);
    }
    #[test]
    fn flight_triangle_samples_the_surface_plane() {
        let t = [
            DVec3::new(-10., -10., 1100.),
            DVec3::new(10., -10., 1100.),
            DVec3::new(0., 10., 1100.),
        ];
        assert_eq!(triangle_height(DVec3::Z, t, 1000.), Some(100.));
        assert_eq!(triangle_height(DVec3::X, t, 1000.), None);
        assert_eq!(triangle_height(DVec3::Z, [t[0]; 3], 1000.), None);
    }
}
