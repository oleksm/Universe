//! Weapons: a ship's gun and laser, and the combat phase that runs once per
//! frame after every ship has moved.
//!
//! Weapons fire only in combat mode: the master arm on (`master_arm`) and the
//! weapons primed (`ARM_TIME`). Traffic control won't clear an armed ship,
//! and a clearance lapses when the ship arms (see `traffic`).
//!
//! The gun fires real slugs (Dogma projectiles): they leave the muzzle at
//! the ship's own velocity plus its gun's muzzle speed along the nose, fall under
//! gravity, and hit with their kinetic energy relative to the target
//! (½ m v²). Every shot pushes the ship back (momentum), and a hit pushes the
//! target. The laser is a beam at light speed (instant at these ranges): it
//! delivers its laser's power, spread thinner beyond its focus, heating the
//! laser as it fires.

use universe_protocol::ShipId;
use std::collections::HashMap;

use glam::{DQuat, DVec3};
use universe_physics::{ray, step_projectile, Hit, Projectile, Target};

use crate::damage;
use crate::events::ShipEvent;
use crate::ship::{Ship, ShipState, Triggers, SHIP_RADIUS};
use crate::world::World;

/// A gun, as its product (its record in the registry) has it: its slug's speed
/// from the muzzle, against the ship (m/s), rounds a second the trigger held,
/// one slug's mass (kg), rounds in a full magazine.
#[derive(Clone, Copy, Debug, PartialEq, serde::Deserialize)]
pub struct Gun {
    pub muzzle: f64,
    pub rate: f64,
    pub slug_mass: f64,
    pub magazine: u32,
}

/// A laser, as its product has it: its beam's power on the target within its
/// focus (W), the focus (m; past it the power falls as the square of the
/// distance), its farthest reach (m), and seconds of firing from cold to too hot,
/// and back.
#[derive(Clone, Copy, Debug, PartialEq, serde::Deserialize)]
pub struct Laser {
    pub power: f64,
    pub focus: f64,
    pub range: f64,
    pub burn: f64,
    pub cool: f64,
    /// After overheating, it's locked out until it has cooled to this share of its heat.
    pub reset: f64,
}

impl Laser {
    /// Its beam's power on a target `range` away (W).
    pub fn power_at(&self, range: f64) -> f64 {
        if range <= self.focus { self.power } else { self.power * (self.focus / range).powi(2) }
    }
}

/// The gun a station's turrets are, and a ship's fire control expects when it
/// has none of its own: the registry's (its first gun).
pub fn standard_gun() -> Gun {
    use crate::modules::Does;
    static GUN: std::sync::OnceLock<Gun> = std::sync::OnceLock::new();
    *GUN.get_or_init(|| {
        let c = crate::content::content();
        c.modules.iter().find_map(|(_, m)| if let Does::Gun(g) = m.does { Some(g) } else { None }).expect("the registry has a gun")
    })
}

/// Seconds a slug flies before it's no longer tracked (about 30 km).
pub const SLUG_LIFETIME: f64 = 10.0;

/// The most a slug's path can bend from straight over a step of `dt` (m),
/// under a pull of `g` where it starts: half the pull times the step
/// squared, the pull taken four times over (it can grow along the path)
/// and a metre a second squared more.
fn slug_bend(g: DVec3, dt: f64) -> f64 {
    0.5 * (4.0 * g.length() + 1.0) * dt * dt
}

/// Could a slug, flying `dt` from where it is, meet target `tg` (which is
/// where it is at the step's end)? Not unless it starts within the
/// target's radius, plus how far they close in the step, plus how far its
/// path bends (`bend`).
fn could_meet(p: &Projectile, tg: &Target, dt: f64, bend: f64) -> bool {
    let at_start = tg.position - tg.velocity * dt;
    (p.position - at_start).length() <= tg.radius + (p.velocity - tg.velocity).length() * dt + bend
}

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
    pub by: ShipId,
    pub target: ShipId,
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
    pub owner: ShipId,
    pub projectile: Projectile,
    /// Seconds since it was fired.
    pub age: f64,
    /// Its mass (kg: its gun's slug).
    pub mass: f64,
}

/// A laser beam fired this frame, for drawing.
#[derive(Clone, Copy, Debug)]
pub struct Beam {
    pub system: usize,
    pub owner: ShipId,
    pub from: DVec3,
    pub to: DVec3,
    /// It hit something (a ship or a body) at `to`.
    pub hit: bool,
}

/// A ship taking part in the combat phase: its id (the caller's), the system
/// it's in, and its event feed.
pub struct Armed<'a> {
    pub id: ShipId,
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
    fn fly_missiles(&mut self, ships: &[Armed], dt: f64, hits: &mut Vec<(ShipId, f64, DVec3, ShipId, &'static str)>) {
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
        let firing = universe_prof::scope("sim/combat/weapons/fire");
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
            // (Each weapon fires only if it's fitted, as its product does.)
            let (gun, laser) = (ship.spec().gun, ship.spec().laser);
            let triggered = armed && gun.is_some() && ship.triggers.gun;
            if !triggered {
                ship.gun_cooldown = ship.gun_cooldown.max(0.0);
            }
            while let Some(g) = gun.filter(|_| triggered && ship.gun_cooldown <= 0.0 && ship.ammo > 0) {
                let nose = ship.gun_forward();
                // Fired this long before the end of the frame: it has since
                // gained that much on the ship (which is already at the end).
                let late = -ship.gun_cooldown;
                let velocity = ship.velocity + nose * g.muzzle;
                let position = ship.position + nose * (SHIP_RADIUS + 2.0 + g.muzzle * late);
                fired.push(Slug { system: a.system, owner: a.id, projectile: Projectile { position, velocity }, age: late, mass: g.slug_mass });
                ship.velocity -= nose * (g.slug_mass * g.muzzle / ship.mass());
                ship.ammo -= 1;
                ship.gun_cooldown += 1.0 / g.rate;
            }
            // The laser heats while it fires and cools when it doesn't.
            match laser {
                Some(l) if armed && ship.triggers.laser && !ship.laser_overheated => {
                    ship.laser_heat = (ship.laser_heat + dt / l.burn).min(1.0);
                    ship.laser_overheated = ship.laser_heat >= 1.0;
                    let dir = ship.gun_forward(); // on the gun's gimbal
                    lasers.push((a.id, a.system, ship.position + dir * (SHIP_RADIUS + 1.0), dir, l));
                }
                _ => {
                    let (cool, reset) = laser.map_or((1.0, 0.0), |l| (l.cool, l.reset));
                    ship.laser_heat = (ship.laser_heat - dt / cool).max(0.0);
                    ship.laser_overheated &= ship.laser_heat > reset;
                }
            }
        }
        // The missiles in the air fly the frame; then the defence turrets'
        // guns and launchers, as their gunners have set them (what they
        // launch is where it is at the frame's end already: it flies from the next).
        drop(firing);
        let mut hits: Vec<(ShipId, f64, DVec3, ShipId, &'static str)> = Vec::new(); // (ship id, joules, impulse, by, cause)
        universe_prof::time("sim/combat/weapons/missiles", || self.fly_missiles(ships, dt, &mut hits));
        universe_prof::time("sim/combat/weapons/turrets", || self.turrets_fire(dt, &mut fired));
        if self.slugs.is_empty() && lasers.is_empty() && hits.is_empty() {
            self.slugs = fired;
            return;
        }

        // Targets by system, where the ships are now.
        let mut targets: HashMap<usize, Vec<Target>> = HashMap::new();
        for a in ships.iter() {
            if can_be_hit(a.ship) {
                targets.entry(a.system).or_default().push(Target { id: a.id.0, position: a.ship.position, velocity: a.ship.velocity, radius: SHIP_RADIUS });
            }
        }
        let index: HashMap<ShipId, usize> = ships.iter().enumerate().map(|(i, a)| (a.id, i)).collect();

        // Slugs: each flies the frame against the ships near enough its
        // path to be met (side by side: they only read the world), then what
        // they hit, in order.
        let t = self.time;
        let flying = universe_prof::scope("sim/combat/weapons/slugs");
        let mut slugs = std::mem::take(&mut self.slugs);
        // Each system's bodies, and where they are, once.
        let mut here: HashMap<usize, (std::sync::Arc<crate::system::StarSystem>, Vec<DVec3>)> = HashMap::new();
        for slug in &slugs {
            here.entry(slug.system).or_insert_with(|| {
                let sys = self.system(slug.system);
                let mut positions = Vec::new();
                sys.positions(t, &mut positions);
                (sys, positions)
            });
        }
        let none = Vec::new();
        let outcomes: Vec<Option<Hit>> = {
            use rayon::prelude::*;
            slugs
                .par_iter_mut()
                .with_min_len(16)
                .map(|slug| {
                    let (sys, positions) = &here[&slug.system];
                    // Clear of its own ship's hull for the first moments.
                    let fresh = slug.age < 0.5;
                    let step = dt.min(SLUG_LIFETIME - slug.age).max(0.0);
                    slug.age += dt;
                    let bend = slug_bend(universe_physics::gravity(&sys.bodies, slug.projectile.position, positions), step);
                    let near: Vec<Target> = targets.get(&slug.system).unwrap_or(&none).iter().filter(|tg| !(fresh && tg.id == slug.owner.0) && could_meet(&slug.projectile, tg, step, bend)).copied().collect();
                    step_projectile(&sys.bodies, positions, &mut slug.projectile, t - dt, step, &near)
                })
                .collect()
        };
        let mut kept = Vec::with_capacity(slugs.len() + fired.len());
        for (slug, outcome) in slugs.into_iter().zip(outcomes) {
            match outcome {
                Some(Hit::Target { id, relative_velocity, point }) => {
                    let id = ShipId(id);
                    let joules = 0.5 * slug.mass * relative_velocity.length_squared();
                    hits.push((id, joules, relative_velocity * slug.mass, slug.owner, "GUNFIRE"));
                    self.impacts.push(Impact { system: slug.system, point, by: slug.owner, target: id, laser: false });
                }
                Some(Hit::Body { .. }) => {}
                None if slug.age < SLUG_LIFETIME => kept.push(slug),
                None => {}
            }
        }
        // This frame's rounds are where they are at its end already; they
        // fly from the next frame.
        kept.extend(fired);
        self.slugs = kept;
        drop(flying);
        let mut positions = Vec::new();
        // Beams.
        let _p = universe_prof::scope("sim/combat/weapons/beams");
        for (owner, system, from, dir, l) in lasers {
            let sys = self.system(system);
            sys.positions(t, &mut positions);
            let all = targets.get(&system).unwrap_or(&none);
            let here: Vec<Target> = all.iter().filter(|tg| tg.id != owner.0).copied().collect();
            let found = ray(&sys.bodies, &positions, from, dir, l.range, t, &here);
            let (to, hit) = match found {
                Some((Hit::Target { id, point, .. }, d)) => {
                    let id = ShipId(id);
                    hits.push((id, l.power_at(d) * dt, DVec3::ZERO, owner, "LASER FIRE"));
                    self.impacts.push(Impact { system, point, by: owner, target: id, laser: true });
                    (from + dir * d, true)
                }
                Some((Hit::Body { .. }, d)) => (from + dir * d, true),
                None => (from + dir * l.range, false),
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
        let mut ships = [Armed { id: ShipId(1), system, ship: a, events: ea }, Armed { id: ShipId(2), system, ship: b, events: eb }];
        world.combat(&mut ships, dt);
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
        // to its reset share (2.8 s), then finishes the job.
        assert!(matches!(b.state, ShipState::Destroyed { .. }));
        assert!((t - 12.8).abs() < 0.25, "destroyed at {t:.1} s");
        assert!(eb.iter().any(|e| matches!(e, ShipEvent::Crashed { body } if body == "LASER FIRE")));
    }
}
