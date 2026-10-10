//! The hunter: a pirate ship's program. It flies its route like anyone else
//! (docking and landing at stations and spaceports, which is where the prey
//! is), watching its radar. When a trader shows up within `HUNT_RANGE` while
//! it's flying in normal space, it drops the route, arms, and goes after that
//! one ship, sticking to it until it's destroyed, docks at a station, leaves
//! the system, gets away beyond `LOSE_RANGE`, or `GIVE_UP` passes. Then it
//! goes safe and carries on with its route. Ships in shelter (docked,
//! landed, or under a defence turret's guns) are left alone, prey that
//! reaches shelter has escaped, and the hunter itself keeps out of the
//! turrets' reach (it's aggressed once it strikes: they'd shoot it).
//!
//! The same program flies a lawful ship's defence: anyone who isn't a pirate
//! judges an aggressor that comes near (fight or flight, see `judge`), and
//! when the odds are on its side — friends close by, its hull sound, its
//! nerve good — it goes after the aggressor with the others (shooting the
//! aggressed is no crime), breaking off when the aggressor is no longer fair
//! game, gets away, or its own hull runs low. The player is one of the ships
//! like any other: hunted by pirates, judged by everyone when aggressed.
//!
//! It flies by the same rules as anyone: `ShipCommands` to its engine,
//! thrusters and weapons, fire control's lead for the gun, and the stick
//! (attitude) it returns for the frame.

use universe_protocol::ShipId;
use glam::{DQuat, DVec3};
use serde::{Deserialize, Serialize};
use universe_world::ship::{facing, Controls, Ship, ShipCommands, Triggers};
use universe_world::StarSystem;
use universe_world::weapons::{standard_gun, within_gimbal, SLUG_LIFETIME};

use crate::avionics::Avionics;
use crate::bus::Bus;
use crate::docking::attitude;
use crate::events::Event;
use crate::fire_control::lead;

/// A ship this close to a defence turret's reach counts as sheltered: the
/// hunter won't go after it (m, beyond the turret's range).
pub const SHELTER_MARGIN: f64 = 2_000.0;
/// And it keeps this far outside the turrets' reach itself (m).
const GUNS_MARGIN: f64 = 1_500.0;
/// A trader this close gets hunted (m).
pub const HUNT_RANGE: f64 = 20_000.0;
/// Farther than this, the prey got away (m).
pub const LOSE_RANGE: f64 = 100_000.0;
/// A hunt is given up after this long (s).
pub const GIVE_UP: f64 = 600.0;
/// Where the hunter holds while attacking (m from the prey).
pub const STANDOFF: f64 = 900.0;
/// Guns fire inside this range (m).
pub const GUN_RANGE: f64 = 3_000.0;
/// Fastest it closes on the prey (m/s).
const MAX_CLOSING: f64 = 400.0;
/// It stays at least this high over the ground (m).
const FLOOR: f64 = 3_000.0;
/// And this far clear of stations and gates (m, beyond their size).
const STANDOFF_STRUCTURE: f64 = 1_500.0;
/// Time to turn the engine round to brake (s): about π at the turn rate.
const TURN_AROUND: f64 = 3.2;
/// It keeps at least this far from other ships (m), looking this far ahead (s).
const SEPARATION: f64 = 400.0;
const SEPARATION_LOOK: f64 = 10.0;
/// Standing down, it first slows to within this of the local traffic (m/s).
const SETTLED: f64 = 5.0;
/// Seconds of rest after a hunt before looking for the next.
pub const REST: f64 = 60.0;
/// A lawful ship judges an aggressor this close (m).
pub const DEFEND_RANGE: f64 = 12_000.0;
/// Friends (and the aggressor's friends) count within this of the aggressor (m).
const POSSE_RANGE: f64 = 10_000.0;
/// Its side must be this many times stronger (hull for hull, by its nerve) to fight.
const ODDS: f64 = 2.0;
/// It won't start a fight on less hull than this, and breaks off below `FLEE_HULL`.
const FIGHT_HULL: f64 = 0.5;
pub const FLEE_HULL: f64 = 0.35;
/// A defence ends when the aggressor gets this far away (m), or after this long (s).
const DEFEND_LOSE: f64 = 30_000.0;
const DEFEND_GIVE_UP: f64 = 300.0;

/// Another ship, as the hunter's radar and transponder see it.
#[derive(Clone, Copy, Debug)]
pub struct Sighting {
    pub id: ShipId,
    pub position: DVec3,
    pub velocity: DVec3,
    /// One of the hunter's own kind.
    pub pirate: bool,
    /// Sheltered: docked, landed, or close under a station or gate (traffic
    /// control's defences); safe from pirates.
    pub docked: bool,
    /// Wrecked.
    pub destroyed: bool,
    /// In hyperdrive (out of reach).
    pub hyperdrive: bool,
    /// Physically down: docked or landed.
    pub landed: bool,
    /// Aggressed: fair game.
    pub aggressed: bool,
    /// Hull integrity 0..1 (a combat scan reads it).
    pub hull: f64,
}

/// The hunt under way.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Hunt {
    /// The prey's id.
    pub target: ShipId,
    /// When it began (world time).
    pub since: f64,
    /// The collision warning's last look (world time), and which way to
    /// break off if it saw something ahead.
    #[serde(default)]
    pub checked: f64,
    #[serde(default)]
    pub break_off: Option<DVec3>,
    /// A lawful defence against an aggressor (not a pirate's hunt), and
    /// whether the route was flying when it began (it goes back to that).
    #[serde(default)]
    pub lawful: bool,
    #[serde(default)]
    pub resume: bool,
}

/// How far ahead a hunter's collision warning looks (s), and how often (s).
const LOOK_AHEAD: f64 = 25.0;
const LOOK_EVERY: f64 = 0.25;

/// Does a pirate in this state want its radar picture this frame? (A
/// lawful ship wants it while defending, or with an aggressor near: see
/// `may_defend`.)
pub fn wants_sightings(a: &Avionics, ship: &Ship, now: f64) -> bool {
    a.hunting.is_some() || (a.pirate && ship.is_flying() && !ship.hyperdrive && !a.route.departing && now >= a.rest_until)
}

/// Could a lawful ship in this state take on an aggressor?
pub fn may_defend(a: &Avionics, ship: &Ship) -> bool {
    !a.pirate && a.hunting.is_none() && ship.is_flying() && !ship.hyperdrive && ship.hull >= FIGHT_HULL
}

/// Fight or flight: should ship `me` (hull `hull`, carrying `cargo` t, at
/// `position`) take on `aggressor`, given everyone it sees? Its side is its
/// own hull and that of every other lawful ship near the aggressor (who'll
/// be judging the same); the aggressor's is its hull and its fellow pirates'
/// near it. Its nerve (a temperament of its own, steadier with nothing in
/// the hold) must make its side `ODDS` times the stronger.
pub fn judge(me: ShipId, hull: f64, cargo: f64, aggressor: &Sighting, sightings: &[Sighting]) -> bool {
    if hull < FIGHT_HULL {
        return false;
    }
    let near = |s: &&Sighting| s.id != aggressor.id && !s.landed && !s.destroyed && !s.hyperdrive && s.position.distance(aggressor.position) < POSSE_RANGE;
    let friends: f64 = sightings.iter().filter(near).filter(|s| !s.pirate && !s.aggressed).map(|s| s.hull).sum();
    let foes: f64 = aggressor.hull + sightings.iter().filter(near).filter(|s| s.pirate || s.aggressed).map(|s| s.hull).sum::<f64>();
    // 0.6–1.4, the same for this ship against this aggressor every time.
    let h = (me.0 as u64).wrapping_mul(0x9e37_79b9_7f4a_7c15) ^ (aggressor.id.0 as u64).wrapping_mul(0xc2b2_ae3d_27d4_eb4f);
    let h = (h ^ (h >> 31)).wrapping_mul(0x94d0_49bb_1331_11eb);
    let nerve = 0.6 + 0.8 * ((h >> 11) as f64 / (1u64 << 53) as f64);
    let nerve = if cargo > 0.0 { nerve * 0.7 } else { nerve };
    (hull + friends) * nerve >= ODDS * foes
}

/// How a hunt ended.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum HuntEnd {
    Killed,
    Escaped,
}

/// Keep clear of everything solid: the ground of every body (`FLOOR` above
/// its highest ground), stations and gates (their size plus
/// `STANDOFF_STRUCTURE`). Within 50 km of that margin, the closing speed is
/// kept to what the engine can stop in the room left; inside it, a push out.
/// The acceleration needed for that (zero when clear).
pub fn avoid(sys: &StarSystem, ship: &Ship, positions: &[DVec3], now: f64) -> DVec3 {
    use universe_physics::Collider;
    let mut accel = DVec3::ZERO;
    for (i, b) in sys.bodies.iter().enumerate() {
        let inner = match &b.rail.collider {
            Collider::Surface => universe_physics::max_radius(b) + FLOOR,
            Collider::Blocks(b) => b.bound + STANDOFF_STRUCTURE,
            Collider::Ring(r) => r.radius + r.tube + STANDOFF_STRUCTURE,
            Collider::None => continue,
        };
        let off = ship.position - positions[i];
        let dist = off.length();
        let out = off / dist;
        let closing = -(ship.velocity - sys.velocity(i, now)).dot(out);
        // Braking to spare: half the engine, less the body's own pull.
        let g = if b.rail.attracts { b.rail.mu / (dist * dist) } else { 0.0 };
        let brake = (0.5 * ship.main_accel() - g).max(1.0);
        // The fastest it may close and still stop short: it may first have to
        // turn the engine round (`TURN_AROUND`), then brake. Look as far as
        // that takes, and then some.
        let v = closing.max(0.0);
        if dist - inner > v * TURN_AROUND + v * v / (2.0 * brake) * 1.5 + 5_000.0 {
            continue;
        }
        let room = (dist - inner).max(0.0);
        let at = brake * TURN_AROUND;
        let allowed = 0.9 * (-at + (at * at + 2.0 * brake * room).sqrt());
        if closing > allowed {
            accel += out * ((closing - allowed) * 1.5);
        }
        if dist < inner {
            // Inside the margin: out, and held up against the pull.
            accel += out * (g + 2.0 * ship.side_accel() * ((inner - dist) / inner.min(5_000.0)).min(1.0));
        }
    }
    accel
}

impl Avionics {
    /// The pirate's program for the frame (`dt` real seconds), given what
    /// its radar sees. While hunting it flies the ship: the stick for the
    /// frame is returned (and the route waits); otherwise None. How a hunt
    /// ended, if it did.
    pub fn hunt(&mut self, bus: &mut impl Bus, sightings: &[Sighting], events: &mut Vec<Event>) -> (Option<Controls>, Option<HuntEnd>) {
        let ship = bus.ship().clone();
        let now = bus.time();
        let near = |s: &&Sighting| s.position.distance(ship.position);
        if self.hunting.is_none() {
            let target = if self.pirate {
                // Looking for prey: only while flying in normal space, not on
                // a departure climb.
                if !wants_sightings(self, &ship, now) {
                    return (None, None);
                }
                sightings
                    .iter()
                    .filter(|s| !s.pirate && !s.docked && !s.destroyed && !s.hyperdrive && s.position.distance(ship.position) < HUNT_RANGE)
                    .min_by(|a, b| near(a).total_cmp(&near(b)))
            } else {
                // An aggressor near: fight, if the odds say so.
                if !may_defend(self, &ship) {
                    return (None, None);
                }
                sightings
                    .iter()
                    .filter(|s| s.aggressed && !s.landed && !s.destroyed && !s.hyperdrive && s.position.distance(ship.position) < DEFEND_RANGE)
                    .min_by(|a, b| near(a).total_cmp(&near(b)))
                    .filter(|a| judge(bus.id(), ship.hull, ship.cargo, a, sightings))
            };
            let Some(prey) = target else { return (None, None) };
            // Drop everything and go.
            let lawful = !self.pirate;
            self.hunting = Some(Hunt { target: prey.id, since: now, checked: f64::NEG_INFINITY, break_off: None, lawful, resume: self.route.active });
            self.route.active = false;
            if self.clearance.take().is_some() {
                events.push(Event::Traffic(universe_world::TrafficEvent::ClearanceCancelled));
            }
            self.nav_target = None;
            self.hyper_autopilot = false;
            let c = ShipCommands { arm: Some(true), rcs: DVec3::ZERO, ..ship.holding() };
            self.command(bus, &c, events);
        }
        let hunt = self.hunting.expect("hunting");
        let prey = sightings.iter().find(|s| s.id == hunt.target);
        // A pirate's prey escapes into shelter; an aggressor only by landing
        // (the turrets will see to it), or by no longer being fair game. A
        // defender with its hull running low breaks off.
        let (lose, give_up) = if hunt.lawful { (DEFEND_LOSE, DEFEND_GIVE_UP) } else { (LOSE_RANGE, GIVE_UP) };
        let end = match prey {
            Some(p) if p.destroyed => Some(HuntEnd::Killed),
            Some(p) if hunt.lawful && (!p.aggressed || p.landed || ship.hull < FLEE_HULL) => Some(HuntEnd::Escaped),
            Some(p) if (hunt.lawful || !p.docked) && !p.hyperdrive && p.position.distance(ship.position) < lose && now - hunt.since < give_up && ship.is_flying() => None,
            _ => Some(HuntEnd::Escaped),
        };
        if let Some(end) = end {
            // Stand down: safe, and first clear of anything it's heading
            // into (the chase may have left it fast and close), then back to
            // the route, resting a while before the next hunt.
            let (sys, positions) = bus.positions();
            let guns = if hunt.lawful { Vec::new() } else { bus.turrets() };
            let mut push = self.keep_clear(&sys, &ship, &positions, now, sightings, &guns);
            // And slowed to the local traffic (the prey's last known motion),
            // so it doesn't coast on at a charge's speed into the others.
            // (That falls as everything does since it was last seen.)
            let g = sys.gravity(ship.position, &positions);
            let settle = self.track.map_or(DVec3::ZERO, |t| t.velocity + g * (now - t.last) - ship.velocity);
            if settle.length() > SETTLED {
                push += (settle * 0.5).clamp_length_max(ship.main_accel());
            }
            if push.length() > 0.01 && ship.is_flying() {
                let (throttle, rcs, nose) = thrust_for(&ship, push, push.normalize());
                let c = ShipCommands { throttle, rcs, weapons: Some(Triggers::default()), arm: Some(false), gun_target: Some(None), ..ship.holding() };
                self.command(bus, &c, events);
                return (Some(attitude(&ship, facing(nose, ship.orientation * DVec3::Y), DVec3::ZERO, 1.0 / 60.0)), None);
            }
            self.hunting = None;
            self.track = None;
            if !hunt.lawful {
                self.rest_until = now + REST;
            }
            let c = ShipCommands {
                throttle: 0.0,
                rcs: DVec3::ZERO,
                weapons: Some(Triggers::default()),
                arm: Some(false),
                gun_target: Some(None),
                ..ship.holding()
            };
            self.command(bus, &c, events);
            self.route.active = !hunt.lawful || hunt.resume;
            return (None, Some(end));
        }
        let prey = prey.expect("still hunted");
        (Some(self.attack(bus, &ship, prey, sightings, events)), None)
    }

    /// What it takes to keep clear of everything solid: `avoid`'s margins,
    /// and the collision warning a few times a second. If what the ship is
    /// doing now runs into something within `LOOK_AHEAD`, it breaks off away
    /// from it, hard, until the path is clear again.
    fn keep_clear(&mut self, sys: &StarSystem, ship: &Ship, positions: &[DVec3], now: f64, others: &[Sighting], guns: &[(DVec3, DVec3, f64)]) -> DVec3 {
        let mut evade = avoid(sys, ship, positions, now);
        // Defence turrets: stay out of their reach, braking in time.
        let brake = 0.5 * ship.main_accel();
        for &(at, velocity, reach) in guns {
            let off = ship.position - at;
            let dist = off.length();
            let edge = reach + GUNS_MARGIN;
            let out = off / dist.max(1.0);
            let closing = -(ship.velocity - velocity).dot(out);
            let allowed = 0.9 * (2.0 * brake * (dist - edge).max(0.0)).sqrt();
            if closing > allowed {
                evade += out * ((closing - allowed) * 1.5);
            }
            if dist < edge {
                evade += out * (2.0 * ship.side_accel() * ((edge - dist) / 3_000.0).min(1.0));
            }
        }
        // Other ships: no closer than `SEPARATION`, now or at the closest
        // approach of the next `SEPARATION_LOOK` seconds as things are going;
        // pushed off the closest-approach line, the harder the sooner.
        for o in others.iter().filter(|o| !o.landed && !o.destroyed && !o.hyperdrive) {
            let off = ship.position - o.position;
            let v = ship.velocity - o.velocity;
            let t = if v.length_squared() > 1e-6 { (-off.dot(v) / v.length_squared()).clamp(0.0, SEPARATION_LOOK) } else { 0.0 };
            let miss = off + v * t;
            let d = miss.length();
            if d < SEPARATION {
                let out = miss.try_normalize().unwrap_or_else(|| v.any_orthonormal_vector());
                let urgency = 1.0 - t / SEPARATION_LOOK;
                let closing = (-v.dot(off.normalize_or_zero())).max(0.0);
                evade += out * (2.0 * ship.side_accel() * (1.0 - d / SEPARATION) * urgency + closing * urgency);
            }
        }
        if let Some(h) = &mut self.hunting
            && now - h.checked >= LOOK_EVERY
        {
            h.checked = now;
            let p = crate::collision::predict_within(sys, ship, now, positions, &[], 50_000.0, LOOK_AHEAD);
            h.break_off = p.collision.map(|c| {
                let at = positions[p.reference] + c.offset;
                (ship.position - at).normalize_or(-ship.velocity.normalize_or(DVec3::Y))
            });
        }
        if let Some(away) = self.hunting.and_then(|h| h.break_off) {
            let toward = -away.dot(v_rel_to_nearest(sys, ship, positions, now));
            evade += away * (ship.main_accel() + toward.max(0.0));
        }
        evade
    }

    /// Chase and attack `prey` for the frame: the devices' commands, and the stick.
    fn attack(&mut self, bus: &mut impl Bus, ship: &Ship, prey: &Sighting, others: &[Sighting], events: &mut Vec<Event>) -> Controls {
        let (sys, positions) = bus.positions();
        let now = bus.time();
        crate::fire_control::Track::update(&mut self.track, prey.id, prey.position, prey.velocity, now);
        let track = self.track.expect("tracking");

        let r = prey.position - ship.position;
        let d = r.length();
        let dir = r / d.max(1.0);
        let v_rel = ship.velocity - prey.velocity;
        // Close in, braking in time to stop at the standoff; back off if too close.
        let brake = 0.5 * ship.main_accel();
        let gap = d - STANDOFF;
        let closing = if gap > 0.0 { (2.0 * brake * gap).sqrt().min(MAX_CLOSING) } else { gap * 0.2 };
        let mut accel = (dir * closing - v_rel) * 0.6 + track.acceleration;
        // The lawful have nothing to fear from the turrets.
        let guns = if self.hunting.is_some_and(|h| h.lawful) { Vec::new() } else { bus.turrets() };
        let evade = self.keep_clear(&sys, ship, &positions, now, others, &guns);
        accel += evade;

        // Gravity pulls the round as it does the prey: lead on the rest of its acceleration.
        let accel_own = track.acceleration - sys.gravity(prey.position, &positions);
        // (Its own gun's muzzle speed; its own laser's focus.)
        let muzzle = ship.spec().gun.unwrap_or_else(standard_gun).muzzle;
        let focus = ship.spec().laser.map_or(0.0, |l| l.focus);
        let solution = lead(ship.position, ship.velocity, prey.position, prey.velocity, accel_own, muzzle, SLUG_LIFETIME);
        // Keeping clear comes first: then it flies with the engine, not the guns.
        let attacking = d < GUN_RANGE * 1.3 && evade.length() < 0.1 && accel.length() < 2.0 * ship.side_accel();
        let (throttle, rcs, nose) = if attacking {
            // Nose on the lead; the thrusters hold the range.
            let (_, rcs, _) = thrust_for(ship, accel, dir);
            (0.0, rcs, solution.map_or(dir, |s| s.aim))
        } else {
            thrust_for(ship, accel, dir)
        };

        let hot = ship.weapons_hot();
        let in_gimbal = solution.is_some_and(|s| within_gimbal(ship, s.aim));
        let triggers = Triggers { gun: hot && attacking && in_gimbal && d < GUN_RANGE, laser: hot && d < focus && ship.forward().angle_between(dir) < 0.07 };
        let c = ShipCommands {
            throttle,
            rcs,
            weapons: Some(triggers),
            gun_target: Some(solution.filter(|_| hot).map(|s| s.aim)),
            ..ship.holding()
        };
        self.command(bus, &c, events);

        let up_hint = ship.orientation * DVec3::Y;
        let target: DQuat = facing(nose, up_hint);
        attitude(ship, target, DVec3::ZERO, 1.0 / 60.0)
    }
}

/// How to get `accel`: point the engine along it (else `fallback`) and burn
/// when lined up; the thrusters give the rest. Throttle, thrusters, and
/// where the nose should go.
pub(crate) fn thrust_for(ship: &Ship, accel: DVec3, fallback: DVec3) -> (f64, DVec3, DVec3) {
    let want = accel.normalize_or(fallback);
    let lined_up = ship.forward().angle_between(want) < 0.25;
    let throttle = if lined_up { (accel.length() / ship.main_accel()).min(1.0) } else { 0.0 };
    let rest = accel - ship.forward() * (throttle * ship.main_accel());
    (throttle, ship.thruster_command(ship.orientation.inverse() * rest), want)
}

/// The ship's velocity relative to the nearest body (the thing it's likely to hit).
fn v_rel_to_nearest(sys: &StarSystem, ship: &Ship, positions: &[DVec3], now: f64) -> DVec3 {
    let i = (0..sys.bodies.len()).min_by(|&a, &b| positions[a].distance(ship.position).total_cmp(&positions[b].distance(ship.position))).unwrap_or(0);
    ship.velocity - sys.velocity(i, now)
}
