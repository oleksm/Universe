//! Surface-to-air missiles, from the defence turrets' launchers. A missile
//! is a device in flight: launched at a target (the turret's gunner names
//! it), it burns its motor at `MISSILE_ACCEL` for `MISSILE_BURN` seconds,
//! steering on its predicted miss (zero-effort-miss guidance, the form of
//! proportional navigation that allows for its own motor: where the target
//! would pass, the time to go reckoned with the motor's push, and a push
//! across the line of sight of `NAV_GAIN` × miss / time-to-go²), and coasts
//! after. Its proximity fuse fires within
//! `MISSILE_FUSE` of the target, and the blast takes `MISSILE_BLAST` (most
//! of a hull, and the hyperdrive jams). A missile that loses its target (in
//! hyperdrive, through a gate, docked, destroyed) destroys itself; one that
//! runs into a body, or outlives `MISSILE_LIFETIME`, is gone.

use universe_protocol::ShipId;
use glam::DVec3;

use crate::system::StarSystem;

/// The motor's push (m/s², about 6 g), and how long it burns (s): 3.6 km/s
/// all told, a hundred kilometres' reach.
pub const MISSILE_ACCEL: f64 = 60.0;
pub const MISSILE_BURN: f64 = 60.0;
/// Gone after this long (s).
pub const MISSILE_LIFETIME: f64 = 150.0;
/// The proximity fuse (m), and the blast's energy at the target (J).
pub const MISSILE_FUSE: f64 = 40.0;
pub const MISSILE_BLAST: f64 = 15.0e6;
/// A launcher fires once every this long (s), and keeps at most this many in the air.
pub const LAUNCH_RELOAD: f64 = 10.0;
pub const IN_FLIGHT: usize = 2;
/// How far a gunner launches at (m).
pub const MISSILE_RANGE: f64 = 80_000.0;
/// Proportional navigation's gain.
const NAV_GAIN: f64 = 3.0;

/// A missile in flight.
#[derive(Clone, Copy, Debug)]
pub struct Missile {
    pub system: usize,
    /// The launcher's turret id, and the target's ship id.
    pub owner: ShipId,
    pub target: ShipId,
    pub position: DVec3,
    pub velocity: DVec3,
    pub age: f64,
}

/// What a missile closing on its target makes of it: the target's place
/// and motion now, if it can still be had.
pub struct Mark {
    pub position: DVec3,
    pub velocity: DVec3,
}

impl Missile {
    /// The motor's push now, toward `mark` (zero once burnt out, or with no mark).
    pub fn thrust(&self, mark: Option<&Mark>) -> DVec3 {
        let Some(m) = mark.filter(|_| self.age < MISSILE_BURN) else { return DVec3::ZERO };
        let r = m.position - self.position;
        let d = r.length().max(1.0);
        let los = r / d;
        let v = m.velocity - self.velocity;
        // The time to go, closing under the motor's push: d = c t + a t²/2.
        let closing = -v.dot(los);
        let a = MISSILE_ACCEL * 0.9;
        let t_go = ((closing * closing + 2.0 * a * d).sqrt() - closing) / a;
        // Where it would pass us, across the line of sight, if nothing changed.
        let miss = (r + v * t_go).reject_from(los);
        let lateral = miss * (NAV_GAIN / (t_go * t_go).max(0.01));
        // What's left of the motor drives it along the line of sight.
        let along = (MISSILE_ACCEL * MISSILE_ACCEL - lateral.length_squared()).max(0.0).sqrt();
        (lateral + los * along).clamp_length_max(MISSILE_ACCEL)
    }

    /// `dt` seconds of flight among `sys`'s bodies (at `positions`): its
    /// motor and gravity. False if it ran into one.
    pub fn fly(&mut self, sys: &StarSystem, positions: &[DVec3], mark: Option<&Mark>, t: f64, dt: f64) -> bool {
        let a = self.thrust(mark) + sys.gravity(self.position, positions);
        self.velocity += a * dt;
        self.position += self.velocity * dt;
        self.age += dt;
        // (Into the ground itself, not just under its highest peaks: a SAM
        // site's missile leaves from the ground.)
        !sys.bodies.iter().zip(positions).any(|(b, &c)| {
            let d = c.distance(self.position);
            !b.kind.artificial() && d < b.max_radius() && d < b.surface_radius_at(c, self.position, t)
        })
    }

    /// Did it pass within the fuse of `mark` over the last `dt` (closest approach)?
    pub fn fuse(&self, mark: &Mark, dt: f64) -> bool {
        let r = mark.position - self.position;
        let v = mark.velocity - self.velocity;
        // Back over the step: the nearest the two came.
        let s = if v.length_squared() > 1e-9 { (r.dot(v) / v.length_squared()).clamp(0.0, dt) } else { 0.0 };
        (r - v * s).length() < MISSILE_FUSE
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testkit::Probe;
    use crate::units::AU;

    #[test]
    fn a_missile_homes_on_a_crossing_ship_and_its_fuse_fires() {
        let mut p = Probe::new(42);
        let sys = p.sys();
        let mut positions = Vec::new();
        sys.positions(0.0, &mut positions);
        // Far out, a ship crossing at 300 m/s, 30 km off.
        let at = DVec3::new(0.0, 5.0 * AU, 0.0);
        let mut target = Mark { position: at + DVec3::new(30_000.0, 0.0, 0.0), velocity: DVec3::new(0.0, 0.0, 300.0) };
        let mut m = Missile { system: 0, owner: ShipId(1), target: ShipId(2), position: at, velocity: DVec3::ZERO, age: 0.0 };
        let dt = 1.0 / 60.0;
        let mut t = 0.0;
        while t < MISSILE_LIFETIME {
            target.position += target.velocity * dt;
            assert!(m.fly(&sys, &positions, Some(&target), 0.0, dt));
            t += dt;
            if m.fuse(&target, dt) {
                break;
            }
        }
        eprintln!("fused after {t:.1} s, closing at {:.0} m/s", (m.velocity - target.velocity).length());
        assert!(m.fuse(&target, dt) && t < 60.0, "{t} s");
        // Lost (the target gone into hyperdrive, say): it coasts.
        let v = m.velocity;
        m.fly(&sys, &positions, None, 0.0, 1.0);
        assert!((m.velocity - v).length() < 0.01, "no mark, no steering");
        let _ = &mut p;
    }
}
