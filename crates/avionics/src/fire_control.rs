//! Fire control: a track on the locked radar contact, and where to aim the
//! gun to hit it.
//!
//! The track is built from successive radar returns: velocity as measured,
//! acceleration estimated from how the velocity changes, so it needs a little
//! time before it can be trusted (`TRACK_TIME`). The lead is solved in the
//! shooter's frame: a slug leaves at the ship's velocity plus the muzzle
//! speed along the aim, and gravity pulls slug and target alike over a short
//! flight, so what's left is the target's relative motion (with its
//! acceleration less gravity's: the caller takes that off the track's
//! measured acceleration) against the muzzle speed.

use glam::DVec3;

/// Seconds of radar returns before the track gives a firing solution.
pub const TRACK_TIME: f64 = 1.5;
/// Smoothing of the acceleration estimate (per second of returns).
const ACCEL_RATE: f64 = 3.0;

/// What fire control knows about one contact.
#[derive(Clone, Copy, Debug)]
pub struct Track {
    /// The contact's radar id.
    pub id: usize,
    /// When tracking began, and the latest return (world time).
    pub since: f64,
    pub last: f64,
    pub position: DVec3,
    pub velocity: DVec3,
    /// Estimated from successive returns (m/s^2).
    pub acceleration: DVec3,
}

impl Track {
    /// Take a radar return for contact `id` at time `t`: continue the track
    /// on it, or start a new one.
    pub fn update(track: &mut Option<Track>, id: usize, position: DVec3, velocity: DVec3, t: f64) {
        match track {
            Some(tr) if tr.id == id && t > tr.last => {
                let dt = t - tr.last;
                let measured = (velocity - tr.velocity) / dt;
                let k = 1.0 - (-ACCEL_RATE * dt).exp();
                tr.acceleration += (measured - tr.acceleration) * k;
                (tr.position, tr.velocity, tr.last) = (position, velocity, t);
            }
            Some(tr) if tr.id == id => {}
            _ => *track = Some(Track { id, since: t, last: t, position, velocity, acceleration: DVec3::ZERO }),
        }
    }

    /// 0..1: how far along the track is to giving a solution.
    pub fn quality(&self) -> f64 {
        ((self.last - self.since) / TRACK_TIME).clamp(0.0, 1.0)
    }

    pub fn ready(&self) -> bool {
        self.quality() >= 1.0
    }
}

/// Where to aim.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Solution {
    /// Unit direction to point the gun (the nose).
    pub aim: DVec3,
    /// The intercept, relative to the shooter now (m): aim at it.
    pub offset: DVec3,
    /// Slug flight time to the intercept (s).
    pub time: f64,
}

/// Lead for a slug leaving at `muzzle` m/s relative to a shooter at
/// `own_position` moving at `own_velocity`, against a target at `position`
/// moving at `velocity` and accelerating at `acceleration`. None when the
/// slug can't catch it within `max_time` seconds.
pub fn lead(own_position: DVec3, own_velocity: DVec3, position: DVec3, velocity: DVec3, acceleration: DVec3, muzzle: f64, max_time: f64) -> Option<Solution> {
    universe_physics::intercept(own_position, own_velocity, position, velocity, acceleration, muzzle, max_time).map(|(aim, offset, time)| Solution { aim, offset, time })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_slug_fired_along_the_lead_meets_the_target() {
        let (own_p, own_v) = (DVec3::ZERO, DVec3::new(50.0, 0.0, 0.0));
        let (p, v, a) = (DVec3::new(0.0, 0.0, -4_000.0), DVec3::new(200.0, 30.0, 0.0), DVec3::new(0.0, 10.0, 0.0));
        let s = lead(own_p, own_v, p, v, a, 3_000.0, 10.0).expect("catchable");
        let slug = own_p + (own_v + s.aim * 3_000.0) * s.time;
        let target = p + v * s.time + a * (0.5 * s.time * s.time);
        assert!(slug.distance(target) < 0.5, "missed by {}", slug.distance(target));
    }

}
