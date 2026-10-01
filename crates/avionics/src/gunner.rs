//! The gunner: the program that runs a defence turret (a client like any
//! pilot, run by the defence service). It sees what the turret's sensors and
//! the law tell it — where the turret is and how its gun points, the ships
//! that are fair game around it — picks the nearest it has a clear line on,
//! tracks it (for its acceleration, as fire control does), leads it, lays
//! the gun and fires once the gun is on. It knows what it sees is a moment
//! old and that its orders reach the gun later still (`latency`), so it
//! leads from where things will be then.

use glam::DVec3;
use universe_protocol::TurretCommand;
use universe_world::weapons::{GUN_MUZZLE, SLUG_LIFETIME};

use crate::fire_control::{lead, Track};

/// Fire once the gun is within this of the lead (rad): about a hull's width at its reach.
pub const ON_TARGET: f64 = 0.004;

/// A ship the gunner may fire on: fair game, as the law has it, and seen.
#[derive(Clone, Copy, Debug)]
pub struct Quarry {
    pub id: usize,
    pub position: DVec3,
    pub velocity: DVec3,
}

/// One turret's gunner: its track on the ship it's after.
#[derive(Clone, Debug, Default)]
pub struct Gunner {
    pub track: Option<Track>,
}

impl Gunner {
    /// Orders for the gun, at `t`: the turret at `at` moving at `velocity`,
    /// its gun pointing along `gun`; `quarry` in sight; `clear(p)` says
    /// whether the line of fire to `p` is open; `gravity(p)` the pull there;
    /// orders take effect `latency` seconds from now.
    #[allow(clippy::too_many_arguments)]
    pub fn orders(
        &mut self,
        t: f64,
        at: DVec3,
        velocity: DVec3,
        gun: DVec3,
        reach: f64,
        quarry: &[Quarry],
        clear: impl Fn(DVec3) -> bool,
        gravity: impl Fn(DVec3) -> DVec3,
        latency: f64,
    ) -> TurretCommand {
        let target = quarry
            .iter()
            .filter(|q| q.position.distance(at) < reach && clear(q.position))
            .min_by(|a, b| a.position.distance(at).total_cmp(&b.position.distance(at)));
        let Some(q) = target else {
            self.track = None;
            return TurretCommand { aim: None, fire: false, launch: None };
        };
        Track::update(&mut self.track, q.id, q.position, q.velocity, t);
        let track = self.track.expect("tracking");
        // Its acceleration less gravity's (which pulls the round alike).
        let accel = track.acceleration - gravity(q.position);
        // Where it and we will be when the orders reach the gun.
        let ahead = |p: DVec3, v: DVec3, a: DVec3| (p + v * latency + a * (0.5 * latency * latency), v + a * latency);
        let (tp, tv) = ahead(q.position, q.velocity, track.acceleration);
        let (mp, _) = ahead(at, velocity, DVec3::ZERO);
        let Some(s) = lead(mp, velocity, tp, tv, accel, GUN_MUZZLE, SLUG_LIFETIME) else {
            return TurretCommand { aim: Some((q.position - at).normalize()), fire: false, launch: None };
        };
        TurretCommand { aim: Some(s.aim), fire: gun.angle_between(s.aim) < ON_TARGET, launch: None }
    }
}
