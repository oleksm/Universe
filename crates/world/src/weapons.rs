//! Weapons: a ship's gun and laser, and the combat phase that runs once per
//! frame after every ship has moved.
//!
//! Weapons fire only in combat mode: the master arm on (`master_arm`) and the
//! weapons primed (`ARM_TIME`). Traffic control won't clear an armed ship,
//! and a clearance lapses when the ship arms (see `traffic`).
//!
//! The gun fires real slugs (kernel projectiles): they leave the muzzle at
//! the ship's own velocity plus `GUN_MUZZLE` along the nose, fall under
//! gravity, and hit with their kinetic energy relative to the target
//! (½ m v²). Every shot pushes the ship back (momentum), and a hit pushes the
//! target. The laser is a beam at light speed (instant at these ranges): it
//! delivers `LASER_POWER`, spread thinner beyond `LASER_FOCUS`, heating the
//! laser as it fires.

use std::collections::HashMap;

use glam::{DQuat, DVec3};
use universe_physics::{ray, step_projectile, Hit, Projectile, Target};

use crate::damage;
use crate::events::ShipEvent;
use crate::ship::{Ship, ShipState, Triggers, SHIP_RADIUS};
use crate::world::World;

/// Slug speed from the muzzle, relative to the ship (m/s).
pub const GUN_MUZZLE: f64 = 3_000.0;
/// Rounds per second while the trigger is held.
pub const GUN_RATE: f64 = 10.0;
/// Mass of one slug (kg).
pub const SLUG_MASS: f64 = 0.5;
/// Rounds in a full magazine.
pub const GUN_AMMO: u32 = 500;
/// Seconds a slug flies before it's no longer tracked (about 30 km).
pub const SLUG_LIFETIME: f64 = 10.0;
/// Beam power on target at up to `LASER_FOCUS` (W).
pub const LASER_POWER: f64 = 2.0e6;
/// Beyond this the beam spreads: power falls as (focus / range)^2 (m).
pub const LASER_FOCUS: f64 = 2_000.0;
/// Longest reach of the beam (m).
pub const LASER_RANGE: f64 = 30_000.0;
/// Seconds of continuous fire from cold to too hot, and back.
pub const LASER_BURN: f64 = 8.0;
pub const LASER_COOL: f64 = 4.0;
/// After overheating, the laser is locked out until it has cooled to this.
pub const LASER_RESET: f64 = 0.3;

/// How far the gun's gimbal swings off the nose (rad): 4°.
pub const GIMBAL_LIMIT: f64 = 4.0 * std::f64::consts::PI / 180.0;
/// How fast it swings (rad/s): 30°/s.
pub const GIMBAL_RATE: f64 = 30.0 * std::f64::consts::PI / 180.0;
/// The gun is on a direction when it points within this of it (rad): 0.05°.
pub const ON_TARGET: f64 = 0.05 * std::f64::consts::PI / 180.0;

/// Swing the gun toward where it's to be laid (its target, clamped to the
/// gimbal's cone; the nose if none) at the gimbal's rate, over `dt` seconds.
pub fn lay_gun(ship: &mut Ship, dt: f64) {
    let want = ship.gun_target.map_or(DVec3::NEG_Z, |w| (ship.orientation.inverse() * w).normalize_or(DVec3::NEG_Z));
    let off = want.angle_between(DVec3::NEG_Z);
    let want = if off > GIMBAL_LIMIT { DQuat::from_rotation_arc(DVec3::NEG_Z, want).slerp(DQuat::IDENTITY, 1.0 - GIMBAL_LIMIT / off) * DVec3::NEG_Z } else { want };
    let gap = ship.gun_dir.angle_between(want);
    if gap <= GIMBAL_RATE * dt || gap < 1e-12 {
        ship.gun_dir = want;
    } else {
        let turn = DQuat::from_rotation_arc(ship.gun_dir, want);
        ship.gun_dir = (DQuat::IDENTITY.slerp(turn, GIMBAL_RATE * dt / gap) * ship.gun_dir).normalize();
    }
}

/// Is the gun on `dir` (world, unit)?
pub fn gun_on(ship: &Ship, dir: DVec3) -> bool {
    ship.gun_forward().angle_between(dir) < ON_TARGET
}

/// Is `dir` (world) within the gimbal's reach?
pub fn within_gimbal(ship: &Ship, dir: DVec3) -> bool {
    ship.forward().angle_between(dir) <= GIMBAL_LIMIT
}

/// Where a round or beam struck a ship this frame (for the pilots to see).
#[derive(Clone, Copy, Debug)]
pub struct Impact {
    pub system: usize,
    pub point: DVec3,
    /// Who fired, and who was hit (ship ids).
    pub by: usize,
    pub target: usize,
    pub laser: bool,
}

/// Seconds from the master arm going on to the weapons being hot.
pub const ARM_TIME: f64 = 2.0;

/// Master arm on (combat mode: the weapons start priming) or off (safe: the
/// triggers are released).
pub fn master_arm(ship: &mut Ship, on: bool, events: &mut Vec<ShipEvent>) {
    if on == ship.armed {
        return;
    }
    ship.armed = on;
    if on {
        ship.arming = ARM_TIME;
        events.push(ShipEvent::WeaponsArming);
    } else {
        ship.arming = 0.0;
        ship.triggers = Triggers::default();
        events.push(ShipEvent::WeaponsSafe);
    }
}

/// A slug in flight.
#[derive(Clone, Copy, Debug)]
pub struct Slug {
    /// Galaxy index of the system it's in.
    pub system: usize,
    /// Who fired it (the caller's ship id).
    pub owner: usize,
    pub projectile: Projectile,
    /// Seconds since it was fired.
    pub age: f64,
}

/// A laser beam fired this frame, for drawing.
#[derive(Clone, Copy, Debug)]
pub struct Beam {
    pub system: usize,
    pub owner: usize,
    pub from: DVec3,
    pub to: DVec3,
    /// It hit something (a ship or a body) at `to`.
    pub hit: bool,
}

/// A ship taking part in the combat phase: its id (the caller's), the system
/// it's in, and its event feed.
pub struct Armed<'a> {
    pub id: usize,
    pub system: usize,
    pub ship: &'a mut Ship,
    pub events: &'a mut Vec<ShipEvent>,
}

/// Can this ship use its weapons (hot, in normal flight)?
fn can_fire(ship: &Ship) -> bool {
    ship.weapons_hot() && matches!(ship.state, ShipState::Flying) && !ship.hyperdrive
}

/// Can this ship be hit (physically present in its system, not inside a hangar)?
fn can_be_hit(ship: &Ship) -> bool {
    matches!(ship.state, ShipState::Flying | ShipState::Landed { .. }) && ship.hangar.is_none()
}

impl World {
    /// The missiles in flight over `dt`: each homes on its target (where it
    /// is now, if it's still in normal space in the missile's system) and
    /// bursts on passing within its fuse. The blasts go into `hits`.
    fn fly_missiles(&mut self, ships: &[Armed], dt: f64, hits: &mut Vec<(usize, f64, DVec3, usize, &'static str)>) {
        if self.missiles.is_empty() {
            return;
        }
        let t = self.time;
        let mut missiles = std::mem::take(&mut self.missiles);
        let mut positions = Vec::new();
        let mut positioned = usize::MAX;
        missiles.retain_mut(|m| {
            let sys = self.system(m.system);
            if positioned != m.system {
                sys.positions(t, &mut positions);
                positioned = m.system;
            }
            let mark = ships
                .iter()
                .find(|a| a.id == m.target && a.system == m.system && can_be_hit(a.ship) && !a.ship.hyperdrive)
                .map(|a| crate::missiles::Mark { position: a.ship.position, velocity: a.ship.velocity });
            // (The ships are at the frame's end already, the missile at its
            // start: it steers on where its target was then.)
            let then = mark.as_ref().map(|mk| crate::missiles::Mark { position: mk.position - mk.velocity * dt, velocity: mk.velocity });
            let alive = m.fly(&sys, &positions, then.as_ref(), t, dt);
            if let Some(mk) = &mark
                && m.fuse(mk, dt)
            {
                let push = (mk.position - m.position).normalize_or(DVec3::Y) * 2.0e4;
                hits.push((m.target, crate::missiles::MISSILE_BLAST, push, m.owner, "MISSILE"));
                self.impacts.push(Impact { system: m.system, point: m.position, by: m.owner, target: m.target, laser: false });
                return false;
            }
            // (Its target lost, it destroys itself.)
            alive && mark.is_some() && m.age < crate::missiles::MISSILE_LIFETIME
        });
        self.missiles = missiles;
    }
}

/// Beam power on target at `range` (W).
pub fn laser_power(range: f64) -> f64 {
    if range <= LASER_FOCUS { LASER_POWER } else { LASER_POWER * (LASER_FOCUS / range).powi(2) }
}

impl World {
    /// The combat phase for `dt` game seconds (the frame that just ran; ships
    /// are where it left them): guns fire, slugs fly, lasers cut, and hits
    /// damage ships. Cheap when nobody is shooting.
    pub fn combat(&mut self, ships: &mut [Armed], dt: f64) {
        self.beams.clear();
        self.impacts.clear();
        if dt <= 0.0 {
            return;
        }
        let mut lasers = Vec::new();
        let mut fired = Vec::new();
        for a in ships.iter_mut() {
            let ship = &mut *a.ship;
            // Nothing to do (most ships, most of the time): weapons safe, the
            // gun parked and cooled down, the laser cold.
            if !ship.armed && ship.gun_target.is_none() && ship.gun_dir == DVec3::NEG_Z && ship.gun_cooldown <= 0.0 && ship.laser_heat <= 0.0 {
                continue;
            }
            // Priming.
            if ship.armed && ship.arming > 0.0 {
                ship.arming -= dt;
                if ship.arming <= 0.0 {
                    ship.arming = 0.0;
                    a.events.push(ShipEvent::WeaponsHot);
                }
            }
            let armed = can_fire(ship);
            // The gimbal lays the gun (parked on the nose when there's nothing to lay it on).
            if ship.gun_target.is_some() || ship.gun_dir != DVec3::NEG_Z {
                lay_gun(ship, dt);
            }
            // The gun: rounds at its rate while the trigger is held, each
            // pushing the ship back.
            ship.gun_cooldown -= dt;
            // (Each weapon fires only if it's fitted.)
            let (gun, laser) = (ship.spec().has(crate::modules::Gear::Gun), ship.spec().has(crate::modules::Gear::Laser));
            if !(armed && gun && ship.triggers.gun) {
                ship.gun_cooldown = ship.gun_cooldown.max(0.0);
            }
            while armed && gun && ship.triggers.gun && ship.gun_cooldown <= 0.0 && ship.ammo > 0 {
                let nose = ship.gun_forward();
                // Fired this long before the end of the frame: it has since
                // gained that much on the ship (which is already at the end).
                let late = -ship.gun_cooldown;
                let velocity = ship.velocity + nose * GUN_MUZZLE;
                let position = ship.position + nose * (SHIP_RADIUS + 2.0 + GUN_MUZZLE * late);
                fired.push(Slug { system: a.system, owner: a.id, projectile: Projectile { position, velocity }, age: late });
                ship.velocity -= nose * (SLUG_MASS * GUN_MUZZLE / ship.mass());
                ship.ammo -= 1;
                ship.gun_cooldown += 1.0 / GUN_RATE;
            }
            // The laser heats while it fires and cools when it doesn't.
            if armed && laser && ship.triggers.laser && !ship.laser_overheated {
                ship.laser_heat = (ship.laser_heat + dt / LASER_BURN).min(1.0);
                ship.laser_overheated = ship.laser_heat >= 1.0;
                let dir = ship.gun_forward(); // on the gun's gimbal
                lasers.push((a.id, a.system, ship.position + dir * (SHIP_RADIUS + 1.0), dir));
            } else {
                ship.laser_heat = (ship.laser_heat - dt / LASER_COOL).max(0.0);
                ship.laser_overheated &= ship.laser_heat > LASER_RESET;
            }
        }
        // The missiles in the air fly the frame; then the defence turrets'
        // guns and launchers, as their gunners have set them (what they
        // launch is where it is at the frame's end already: it flies from the next).
        let mut hits: Vec<(usize, f64, DVec3, usize, &'static str)> = Vec::new(); // (ship id, joules, impulse, by, cause)
        self.fly_missiles(ships, dt, &mut hits);
        self.turrets_fire(dt, &mut fired);
        if self.slugs.is_empty() && lasers.is_empty() && hits.is_empty() {
            self.slugs = fired;
            return;
        }

        // Targets by system, where the ships are now.
        let mut targets: HashMap<usize, Vec<Target>> = HashMap::new();
        for a in ships.iter() {
            if can_be_hit(a.ship) {
                targets.entry(a.system).or_default().push(Target { id: a.id, position: a.ship.position, velocity: a.ship.velocity, radius: SHIP_RADIUS });
            }
        }
        let index: HashMap<usize, usize> = ships.iter().enumerate().map(|(i, a)| (a.id, i)).collect();

        // Slugs.
        let t = self.time;
        let mut slugs = std::mem::take(&mut self.slugs);
        let mut positions = Vec::new();
        let mut positioned = usize::MAX;
        let none = Vec::new();
        slugs.retain_mut(|slug| {
            let sys = self.system(slug.system);
            if positioned != slug.system {
                sys.positions(t, &mut positions);
                positioned = slug.system;
            }
            let all = targets.get(&slug.system).unwrap_or(&none);
            // Clear of its own ship's hull for the first moments.
            let fresh = slug.age < 0.5;
            let here: Vec<Target> = all.iter().filter(|tg| !(fresh && tg.id == slug.owner)).copied().collect();
            let step = dt.min(SLUG_LIFETIME - slug.age).max(0.0);
            slug.age += dt;
            match step_projectile(&sys.bodies, &positions, &mut slug.projectile, t - dt, step, &here) {
                Some(Hit::Target { id, relative_velocity, point }) => {
                    let joules = 0.5 * SLUG_MASS * relative_velocity.length_squared();
                    hits.push((id, joules, relative_velocity * SLUG_MASS, slug.owner, "GUNFIRE"));
                    self.impacts.push(Impact { system: slug.system, point, by: slug.owner, target: id, laser: false });
                    false
                }
                Some(Hit::Body { .. }) => false,
                None => slug.age < SLUG_LIFETIME,
            }
        });
        // This frame's rounds are where they are at its end already; they
        // fly from the next frame.
        slugs.extend(fired);
        self.slugs = slugs;

        // Beams.
        for (owner, system, from, dir) in lasers {
            let sys = self.system(system);
            sys.positions(t, &mut positions);
            let all = targets.get(&system).unwrap_or(&none);
            let here: Vec<Target> = all.iter().filter(|tg| tg.id != owner).copied().collect();
            let found = ray(&sys.bodies, &positions, from, dir, LASER_RANGE, t, &here);
            let (to, hit) = match found {
                Some((Hit::Target { id, point, .. }, d)) => {
                    hits.push((id, laser_power(d) * dt, DVec3::ZERO, owner, "LASER FIRE"));
                    self.impacts.push(Impact { system, point, by: owner, target: id, laser: true });
                    (from + dir * d, true)
                }
                Some((Hit::Body { .. }, d)) => (from + dir * d, true),
                None => (from + dir * LASER_RANGE, false),
            };
            self.beams.push(Beam { system, owner, from, to, hit });
        }

        // The hits land (who's to blame is the law's business, from these events).
        for (id, joules, impulse, by, cause) in hits {
            let Some(&i) = index.get(&id) else { continue };
            let a = &mut ships[i];
            damage::hit(a.ship, joules, impulse, by, cause, a.events);
        }
    }
}

#[cfg(test)]
mod tests {
    use glam::DQuat;

    use super::*;
    use crate::testkit::Probe;
    use crate::units::AU;

    /// Two ships far from anything, `gap` apart, the first facing the second.
    fn duel(gap: f64) -> (World, Ship, Ship, usize) {
        let p = Probe::new(42);
        let at = DVec3::new(0.0, 5.0 * AU, 0.0);
        let mut shooter = Ship::new(at, DVec3::ZERO, DQuat::IDENTITY); // facing -Z
        shooter.armed = true;
        let target = Ship::new(at + DVec3::NEG_Z * gap, DVec3::ZERO, DQuat::IDENTITY);
        (p.world, shooter, target, p.system)
    }

    fn frame(world: &mut World, system: usize, a: &mut Ship, b: &mut Ship, ea: &mut Vec<ShipEvent>, eb: &mut Vec<ShipEvent>, dt: f64) {
        world.time += dt;
        for s in [&mut *a, &mut *b] {
            s.position += s.velocity * dt;
        }
        let mut ships = [Armed { id: 1, system, ship: a, events: ea }, Armed { id: 2, system, ship: b, events: eb }];
        world.combat(&mut ships, dt);
    }

    #[test]
    fn slugs_fly_hit_and_push_both_ways() {
        let (mut world, mut a, mut b, sys) = duel(3_000.0);
        let (mut ea, mut eb) = (Vec::new(), Vec::new());
        a.triggers.gun = true;
        for _ in 0..6 {
            frame(&mut world, sys, &mut a, &mut b, &mut ea, &mut eb, 1.0 / 60.0);
        }
        a.triggers.gun = false;
        assert_eq!(a.ammo, GUN_AMMO - 1, "one round in a tenth of a second");
        assert!(a.velocity.z > 0.0, "recoil pushes the shooter back (+Z)");
        // About a second of flight for 3 km.
        for _ in 0..70 {
            frame(&mut world, sys, &mut a, &mut b, &mut ea, &mut eb, 1.0 / 60.0);
        }
        let hit = eb.iter().find_map(|e| match e {
            ShipEvent::Hit { by, damage, .. } => Some((*by, *damage)),
            _ => None,
        });
        let (by, damage) = hit.expect("the slug hits");
        assert_eq!(by, 1);
        let joules = 0.5 * SLUG_MASS * GUN_MUZZLE * GUN_MUZZLE;
        assert!((damage - joules / crate::ship::starter().hull_strength).abs() < 0.01, "{damage}");
        assert!(b.velocity.z < 0.0, "the hit pushes the target away");
        assert!(world.slugs.is_empty());
    }

    #[test]
    fn weapons_fire_only_when_armed_and_primed() {
        let (mut world, mut a, mut b, sys) = duel(3_000.0);
        let (mut ea, mut eb) = (Vec::new(), Vec::new());
        a.triggers.gun = true;
        master_arm(&mut a, false, &mut ea);
        assert_eq!(a.triggers, Triggers::default(), "going safe released the triggers");
        a.triggers.gun = true;
        frame(&mut world, sys, &mut a, &mut b, &mut ea, &mut eb, 0.1);
        assert_eq!(a.ammo, GUN_AMMO, "safe: no shot");
        master_arm(&mut a, true, &mut ea);
        a.triggers.gun = true;
        for _ in 0..18 {
            frame(&mut world, sys, &mut a, &mut b, &mut ea, &mut eb, 0.1);
        }
        assert_eq!(a.ammo, GUN_AMMO, "still priming");
        for _ in 0..3 {
            frame(&mut world, sys, &mut a, &mut b, &mut ea, &mut eb, 0.1);
        }
        assert!(a.ammo < GUN_AMMO, "hot: firing");
        assert!(ea.contains(&ShipEvent::WeaponsArming) && ea.contains(&ShipEvent::WeaponsHot) && ea.contains(&ShipEvent::WeaponsSafe));
    }

    #[test]
    fn the_laser_burns_heats_up_and_destroys() {
        let (mut world, mut a, mut b, sys) = duel(1_000.0);
        let (mut ea, mut eb) = (Vec::new(), Vec::new());
        a.triggers.laser = true;
        let mut t: f64 = 0.0;
        while !matches!(b.state, ShipState::Destroyed { .. }) && t < 20.0 {
            frame(&mut world, sys, &mut a, &mut b, &mut ea, &mut eb, 0.1);
            t += 0.1;
        }
        // 20 MJ of hull at 2 MW: 10 s, but the laser overheats after 8 s.
        // It overheats at 8 s (16 MJ in), is locked out while it cools from 1
        // to LASER_RESET (2.8 s), then finishes the job.
        assert!(matches!(b.state, ShipState::Destroyed { .. }));
        assert!((t - 12.8).abs() < 0.25, "destroyed at {t:.1} s");
        assert!(eb.iter().any(|e| matches!(e, ShipEvent::Crashed { body } if body == "LASER FIRE")));
    }
}
