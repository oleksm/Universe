//! Defence turrets: fixed guns around stations, on gate rings, and on the
//! ground around spaceports, generated from the seed (not every place has
//! them: see `turrets`). In the core a turret is hardware: a gun that slews
//! toward where it's told to aim (at `TURRET_SLEW`), fires the same slugs a
//! ship's gun does at `TURRET_RATE` while its trigger is held, along wherever
//! it actually points. Who it shoots at is its gunner's business — a client,
//! run by the defence service — and the law's (who's fair game).

use glam::{DQuat, DVec3};

use crate::gate::{GATE_RADIUS, RING_TUBE};
use crate::rng::{mix, Rng};
use crate::station::STATION_SIZE;
use crate::system::{BodyKind, StarSystem};
use crate::traffic::Facility;
use universe_protocol::TurretCommand;

use crate::weapons::{Slug, GUN_MUZZLE};

/// How far a turret reaches (m): twice a ship's gun range.
pub const TURRET_RANGE: f64 = 6_000.0;
/// A spaceport's or a station's turrets reach farther: SAMs covering the
/// approach and the holding circle (their rounds fly 30 km).
pub const PORT_TURRET_RANGE: f64 = 20_000.0;
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
    /// How far it reaches (m).
    pub fn range(&self) -> f64 {
        match self.facility {
            Facility::Spaceport(_) | Facility::Station(_) => PORT_TURRET_RANGE,
            _ => TURRET_RANGE,
        }
    }

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
/// every station 3–4 SAMs round its platform, half the gates 2–3 on the
/// ring, half the spaceports 3–4 on the ground around the pads.
pub fn turrets(seed: u64, system: usize, sys: &StarSystem) -> Vec<Turret> {
    let mut rng = Rng::new(mix(seed, 0x5a4_7000 + system as u64));
    let mut out = Vec::new();
    for (i, b) in sys.bodies.iter().enumerate() {
        match b.kind {
            BodyKind::Station => {
                let n = 3 + rng.range(0.0, 2.0) as usize;
                // Round the platform in its deck's plane, clear of it.
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

/// How fast a turret's gun turns to its aim (rad/s).
pub const TURRET_SLEW: f64 = 4.0;

/// A turret gun's state: where it points (a world direction), and its
/// orders and cooldown.
#[derive(Clone, Copy, Debug)]
pub struct TurretGun {
    pub aim: DVec3,
    pub orders: TurretCommand,
    pub cooldown: f64,
    /// Until the missile launcher is ready again (s).
    pub reload: f64,
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
        self.turret_motions(system).iter().any(|(t, p, _)| p.distance(point) < t.range() + margin)
    }

    /// Give turret `id`'s gun its orders (they hold until changed).
    pub fn command_turret(&mut self, id: usize, c: TurretCommand) {
        let gun = self.turret_guns.entry(id).or_insert(TurretGun { aim: c.aim.unwrap_or(DVec3::Y), orders: c, cooldown: 0.0, reload: 0.0 });
        gun.orders = c;
    }

    /// Turret `id`'s gun, as its gunner's sensors read it (None: never commanded).
    pub fn turret_gun(&self, id: usize) -> Option<TurretGun> {
        self.turret_guns.get(&id).copied()
    }

    /// The turrets' guns over the `dt` seconds just run: each slews toward
    /// its aim, and fires along where it points while its trigger is held.
    /// Rounds go into `fired`.
    pub(crate) fn turrets_fire(&mut self, dt: f64, fired: &mut Vec<Slug>) {
        let mut ids: Vec<usize> = self.turret_guns.keys().copied().collect();
        ids.sort_unstable();
        let mut motions: std::collections::HashMap<usize, Vec<(Turret, DVec3, DVec3)>> = std::collections::HashMap::new();
        for id in ids {
            let Some((system, k)) = turret_of(id) else { continue };
            let gun = self.turret_guns.get_mut(&id).expect("listed");
            if let Some(want) = gun.orders.aim.and_then(|a| a.try_normalize()) {
                let angle = gun.aim.angle_between(want);
                let step = (TURRET_SLEW * dt).min(angle);
                if angle > 1e-9 {
                    let axis = gun.aim.cross(want).try_normalize().unwrap_or_else(|| gun.aim.any_orthonormal_vector());
                    gun.aim = (DQuat::from_axis_angle(axis, step) * gun.aim).normalize();
                }
            }
            gun.cooldown = (gun.cooldown - dt).max(if gun.orders.fire { f64::NEG_INFINITY } else { 0.0 });
            gun.reload = (gun.reload - dt).max(0.0);
            // The launcher: a missile at the named ship as it reloads, so
            // many in the air at once.
            if let Some(target) = gun.orders.launch
                && gun.reload <= 0.0
                && self.missiles.iter().filter(|m| m.owner == id).count() < crate::missiles::IN_FLIGHT
            {
                gun.reload = crate::missiles::LAUNCH_RELOAD;
                let now = self.time;
                let here = motions.entry(system).or_insert_with(|| self.turret_motions_at(system, now));
                if let Some(&(_, at, velocity)) = here.get(k) {
                    let up = (at - self.rails_at(system, now)[self.turrets_of(system)[k].body]).normalize_or(DVec3::Y);
                    self.missiles.push(crate::missiles::Missile { system, owner: id, target, position: at + up * 15.0, velocity: velocity + up * 60.0, age: 0.0 });
                }
            }
            let gun = self.turret_guns.get_mut(&id).expect("listed");
            if !gun.orders.fire || gun.cooldown > 0.0 {
                continue;
            }
            let (aim, now) = (gun.aim, self.time);
            let here = motions.entry(system).or_insert_with(|| self.turret_motions_at(system, now));
            let Some(&(_, at, velocity)) = here.get(k) else { continue };
            let gun = self.turret_guns.get_mut(&id).expect("listed");
            while gun.cooldown <= 0.0 {
                let projectile = universe_physics::Projectile { position: at + aim * 8.0, velocity: velocity + aim * GUN_MUZZLE };
                fired.push(Slug { system, owner: id, projectile, age: 0.0 });
                gun.cooldown += 1.0 / TURRET_RATE;
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

#[cfg(test)]
mod stations {
    use super::*;

    #[test]
    fn every_station_has_sams_covering_its_approach() {
        let w = crate::World::new(1984);
        let mut systems: Vec<usize> = w.gate_links.iter().flat_map(|&(a, b)| [a, b]).collect();
        systems.sort_unstable();
        systems.dedup();
        let mut stations = 0;
        for &s in &systems {
            let sys = w.system(s);
            let t = turrets(w.galaxy.seed, s, &sys);
            for (i, b) in sys.bodies.iter().enumerate().filter(|(_, b)| b.kind == BodyKind::Station) {
                let mine: Vec<_> = t.iter().filter(|t| t.facility == Facility::Station(i)).collect();
                assert!(mine.len() >= 3, "{}: {} turrets", b.name, mine.len());
                assert!(mine.iter().all(|t| t.range() >= 20_000.0));
                stations += 1;
            }
        }
        assert!(stations > 0);
    }
}
