//! Defence turrets: fixed guns around stations, on gate rings, and on the
//! ground around spaceports, generated from the seed (not every place has
//! them: see `turrets`). Each has its own radar and fire control, and fires
//! on any aggressed ship within `TURRET_RANGE` (twice a ship's gun range),
//! with the same slugs a ship's gun fires — never on the innocent. Their
//! coverage is traffic control's real shelter.

use glam::{DQuat, DVec3};

use crate::gate::{GATE_RADIUS, RING_TUBE};
use crate::rng::{mix, Rng};
use crate::ship::ShipState;
use crate::station::STATION_SIZE;
use crate::system::{BodyKind, StarSystem};
use crate::traffic::Facility;
use crate::weapons::{Armed, Slug, GUN_MUZZLE, SLUG_LIFETIME};

/// How far a turret reaches (m): twice a ship's gun range.
pub const TURRET_RANGE: f64 = 6_000.0;
/// Rounds per second.
pub const TURRET_RATE: f64 = 5.0;
/// Turrets' ids in combat start here (ships' are small numbers).
pub const TURRET_ID: usize = 1 << 40;

/// A turret: fixed to `body` at `local` (the body's own, rotating frame),
/// defending `facility`.
#[derive(Clone, Debug, PartialEq)]
pub struct Turret {
    pub facility: Facility,
    pub body: usize,
    pub local: DVec3,
}

impl Turret {
    /// Where it is and how it moves at `t` (bodies at `positions`).
    pub fn motion(&self, sys: &StarSystem, t: f64, positions: &[DVec3]) -> (DVec3, DVec3) {
        let b = &sys.bodies[self.body];
        let r = b.rotation(t) * self.local;
        (positions[self.body] + r, sys.velocity(self.body, t) + b.angular_velocity().cross(r))
    }
}

/// Combat id of turret `k` of `system`.
pub fn turret_id(system: usize, k: usize) -> usize {
    TURRET_ID + system * 256 + k
}

/// The system and index of a turret by its combat id, if it is one.
pub fn turret_of(id: usize) -> Option<(usize, usize)> {
    (id >= TURRET_ID).then(|| ((id - TURRET_ID) / 256, (id - TURRET_ID) % 256))
}

/// Points spread around a circle of `radius` in the plane across `axis`.
fn ring(axis: DVec3, radius: f64, n: usize, phase: f64) -> impl Iterator<Item = DVec3> {
    let across = axis.any_orthonormal_vector();
    (0..n).map(move |i| DQuat::from_axis_angle(axis, phase + i as f64 * std::f64::consts::TAU / n as f64) * across * radius)
}

/// The turrets of star system `system` (`sys`), for the galaxy `seed`:
/// most stations have 2–4, half the gates 2–3 on the ring, half the
/// spaceports 3–4 on the ground around the pads.
pub fn turrets(seed: u64, system: usize, sys: &StarSystem) -> Vec<Turret> {
    let mut rng = Rng::new(mix(seed, 0x5a4_7000 + system as u64));
    let mut out = Vec::new();
    for (i, b) in sys.bodies.iter().enumerate() {
        match b.kind {
            BodyKind::Station if rng.range(0.0, 1.0) < 0.7 => {
                let n = 2 + rng.range(0.0, 3.0) as usize;
                // Around its middle, clear of the hull (its spin axis is local +Y).
                out.extend(ring(DVec3::Y, STATION_SIZE * 1.6, n, rng.range(0.0, 1.0)).map(|p| Turret { facility: Facility::Station(i), body: i, local: p }));
            }
            BodyKind::Gate if rng.range(0.0, 1.0) < 0.5 => {
                let n = 2 + rng.range(0.0, 2.0) as usize;
                out.extend(ring(DVec3::Y, GATE_RADIUS + RING_TUBE * 2.0, n, rng.range(0.0, 1.0)).map(|p| Turret { facility: Facility::Gate(i), body: i, local: p }));
            }
            _ => {}
        }
    }
    for (p, sp) in sys.spaceports.iter().enumerate() {
        if rng.range(0.0, 1.0) >= 0.5 {
            continue;
        }
        let n = 3 + rng.range(0.0, 2.0) as usize;
        let b = &sys.bodies[sp.body];
        let r = b.rail.radius;
        for off in ring(sp.direction, 450.0, n, rng.range(0.0, 1.0)) {
            let dir = (sp.direction * r + off).normalize();
            out.push(Turret { facility: Facility::Spaceport(p), body: sp.body, local: dir * (b.surface_radius(dir) + 6.0) });
        }
    }
    out
}

/// Smoothing of a track's acceleration estimate (per second of returns).
const ACCEL_RATE: f64 = 3.0;

/// Turret fire control's track on a ship: its velocity at the last look,
/// and its acceleration as estimated from how that changes (the radar
/// measures motion, not intentions), so a burning ship is led, not just a
/// coasting one.
#[derive(Clone, Copy, Debug)]
pub struct TurretTrack {
    time: f64,
    velocity: DVec3,
    acceleration: DVec3,
}

impl TurretTrack {
    fn update(track: Option<TurretTrack>, velocity: DVec3, now: f64) -> TurretTrack {
        match track {
            // A stale track starts over.
            Some(tr) if now > tr.time && now - tr.time < 1.0 => {
                let dt = now - tr.time;
                let k = 1.0 - (-ACCEL_RATE * dt).exp();
                let acceleration = tr.acceleration + ((velocity - tr.velocity) / dt - tr.acceleration) * k;
                TurretTrack { time: now, velocity, acceleration }
            }
            Some(tr) if now <= tr.time => tr,
            _ => TurretTrack { time: now, velocity, acceleration: DVec3::ZERO },
        }
    }
}

/// Is a ship worth a turret's round: aggressed, and there to be hit?
fn fair_game(a: &Armed) -> bool {
    a.aggressed && matches!(a.ship.state, ShipState::Flying | ShipState::Landed { .. })
}

impl crate::world::World {
    /// The turrets of `system`, and where they are and how they move now.
    pub fn turret_motions(&self, system: usize) -> Vec<(Turret, DVec3, DVec3)> {
        self.turret_motions_at(system, self.time)
    }

    /// The turrets of `system` at time `t`.
    pub fn turret_motions_at(&self, system: usize, t: f64) -> Vec<(Turret, DVec3, DVec3)> {
        let sys = self.system(system);
        let list = self.turrets_of(system);
        let positions = self.rails_at(system, t);
        list.iter().map(|tu| {
            let (p, v) = tu.motion(&sys, t, &positions);
            (tu.clone(), p, v)
        }).collect()
    }

    /// Is `point` in `system` covered by a turret (within its reach, and
    /// `margin` more)?
    pub fn covered(&self, system: usize, point: DVec3, margin: f64) -> bool {
        self.turret_motions(system).iter().any(|(_, p, _)| p.distance(point) < TURRET_RANGE + margin)
    }

    /// Turrets fire on aggressed ships in reach, over the `dt` seconds just
    /// run: rounds go into `fired`.
    pub(crate) fn turrets_fire(&mut self, ships: &[Armed], dt: f64, fired: &mut Vec<Slug>) {
        let now = self.time;
        // Track every aggressor (for its acceleration); forget the rest.
        self.turret_tracks.retain(|id, _| ships.iter().any(|a| a.id == *id && fair_game(a)));
        for a in ships.iter().filter(|a| fair_game(a)) {
            let tr = TurretTrack::update(self.turret_tracks.get(&a.id).copied(), a.ship.velocity, now);
            self.turret_tracks.insert(a.id, tr);
        }
        let mut systems: Vec<usize> = ships.iter().filter(|a| fair_game(a)).map(|a| a.system).collect();
        systems.sort();
        systems.dedup();
        for system in systems {
            let sys = self.system(system);
            let mut positions = Vec::new();
            sys.positions(now, &mut positions);
            for (k, (_, at, velocity)) in self.turret_motions(system).into_iter().enumerate() {
                let id = turret_id(system, k);
                let cooldown = self.turret_cooldowns.entry(id).or_insert(0.0);
                *cooldown -= dt;
                if *cooldown > 0.0 {
                    continue;
                }
                // The nearest aggressor in reach with a clear line of fire
                // (not through its own station, the ground, anything).
                let clear = |p: DVec3| {
                    let d = p - at;
                    universe_physics::ray(&sys.bodies, &positions, at + d.normalize() * 10.0, d.normalize(), d.length() - 30.0, now, &[]).is_none()
                };
                let target = ships
                    .iter()
                    .filter(|a| a.system == system && fair_game(a) && a.ship.position.distance(at) < TURRET_RANGE && clear(a.ship.position))
                    .min_by(|a, b| a.ship.position.distance(at).total_cmp(&b.ship.position.distance(at)));
                let Some(target) = target else {
                    *cooldown = 0.0;
                    continue;
                };
                // Lead on its acceleration less gravity's (which pulls the round alike).
                let accel = self.turret_tracks.get(&target.id).map_or(DVec3::ZERO, |t| t.acceleration) - sys.gravity(target.ship.position, &positions);
                let lead = universe_physics::intercept(at, velocity, target.ship.position, target.ship.velocity, accel, GUN_MUZZLE, SLUG_LIFETIME);
                if let Some((aim, _, _)) = lead {
                    let projectile = universe_physics::Projectile { position: at + aim * 8.0, velocity: velocity + aim * GUN_MUZZLE };
                    fired.push(Slug { system, owner: id, projectile, age: 0.0 });
                    *cooldown += 1.0 / TURRET_RATE;
                }
            }
        }
    }

    pub(crate) fn turrets_of(&self, system: usize) -> std::sync::Arc<Vec<Turret>> {
        if let Some(t) = self.frozen_turrets(system) {
            return t;
        }
        if let Some(t) = self.turrets.lock().unwrap_or_else(|e| e.into_inner()).get(&system) {
            return t.clone();
        }
        let sys = self.system(system);
        let t = std::sync::Arc::new(turrets(self.galaxy.seed, system, &sys));
        self.turrets.lock().unwrap_or_else(|e| e.into_inner()).insert(system, t.clone());
        t
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::World;

    #[test]
    fn some_places_are_defended_and_it_is_the_same_every_time() {
        let w = World::new(1984);
        let system = w.home_system;
        let sys = w.system(system);
        let t = turrets(1984, system, &sys);
        assert_eq!(t, turrets(1984, system, &sys));
        let places = sys.bodies.iter().filter(|b| matches!(b.kind, BodyKind::Station | BodyKind::Gate)).count() + sys.spaceports.len();
        let mut facilities: Vec<_> = t.iter().map(|x| x.facility).collect();
        facilities.dedup();
        let defended = facilities.len();
        assert!(defended > 0 && defended < places, "{defended} of {places} places defended");
    }
}
