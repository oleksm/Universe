//! The hunter: a pirate ship's program. It flies its route like anyone else
//! (docking and landing at stations and spaceports, which is where the prey
//! is), watching its radar. When a trader shows up within `HUNT_RANGE` while
//! it's flying in normal space, it drops the route, arms, and goes after that
//! one ship, sticking to it until it's destroyed, docks at a station, leaves
//! the system, gets away beyond `LOSE_RANGE`, or `GIVE_UP` passes. Then it
//! goes safe and carries on with its route. Ships in shelter (docked,
//! landed, or close under a station or gate: `SHELTER`) are left alone, and
//! prey that reaches shelter has escaped.
//!
//! It flies by the same rules as anyone: `ShipCommands` to its engine,
//! thrusters and weapons, fire control's lead for the gun, and the stick
//! (attitude) it returns for the frame.

use glam::{DQuat, DVec3};
use serde::{Deserialize, Serialize};
use universe_world::ship::{facing, Controls, Ship, ShipCommands, Triggers};
use universe_world::StarSystem;
use universe_world::weapons::{within_gimbal, GUN_MUZZLE, LASER_FOCUS, SLUG_LIFETIME};

use crate::avionics::Avionics;
use crate::bus::Bus;
use crate::docking::attitude;
use crate::events::Event;
use crate::fire_control::lead;

/// Within this of a station or gate, a ship is in shelter (m).
pub const SHELTER: f64 = 6_000.0;
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
/// Seconds of rest after a hunt before looking for the next.
pub const REST: f64 = 60.0;

/// Another ship, as the hunter's radar and transponder see it.
#[derive(Clone, Copy, Debug)]
pub struct Sighting {
    pub id: usize,
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
}

/// The hunt under way.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Hunt {
    /// The prey's id.
    pub target: usize,
    /// When it began (world time).
    pub since: f64,
    /// The collision warning's last look (world time), and which way to
    /// break off if it saw something ahead.
    #[serde(default)]
    pub checked: f64,
    #[serde(default)]
    pub break_off: Option<DVec3>,
}

/// How far ahead a hunter's collision warning looks (s), and how often (s).
const LOOK_AHEAD: f64 = 25.0;
const LOOK_EVERY: f64 = 0.25;

/// Does a pirate in this state want its radar picture this frame?
pub fn wants_sightings(a: &Avionics, ship: &Ship, now: f64) -> bool {
    a.pirate && (a.hunting.is_some() || (ship.is_flying() && !ship.hyperdrive && !a.route.departing && now >= a.rest_until))
}

/// How a hunt ended.
#[derive(Clone, Copy, Debug, PartialEq)]
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
            Collider::Polytope(p) => p.bound * p.scale + STANDOFF_STRUCTURE,
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
        if !self.pirate {
            return (None, None);
        }
        let ship = bus.ship().clone();
        let now = bus.time();
        // Looking for prey: only while flying in normal space, not on a
        // departure climb.
        if self.hunting.is_none() {
            if !wants_sightings(self, &ship, now) {
                return (None, None);
            }
            let prey = sightings
                .iter()
                .filter(|s| !s.pirate && !s.docked && !s.destroyed && !s.hyperdrive && s.position.distance(ship.position) < HUNT_RANGE)
                .min_by(|a, b| a.position.distance(ship.position).total_cmp(&b.position.distance(ship.position)));
            let Some(prey) = prey else { return (None, None) };
            // Drop everything and go.
            self.hunting = Some(Hunt { target: prey.id, since: now, checked: f64::NEG_INFINITY, break_off: None });
            self.route.active = false;
            self.clearance = None;
            self.nav_target = None;
            self.hyper_autopilot = false;
            let c = ShipCommands { arm: Some(true), rcs: DVec3::ZERO, ..ship.holding() };
            let happened = bus.command(&c);
            self.record(happened, events);
        }
        let hunt = self.hunting.expect("hunting");
        let prey = sightings.iter().find(|s| s.id == hunt.target);
        let end = match prey {
            Some(p) if p.destroyed => Some(HuntEnd::Killed),
            Some(p) if !p.docked && !p.hyperdrive && p.position.distance(ship.position) < LOSE_RANGE && now - hunt.since < GIVE_UP && ship.is_flying() => None,
            _ => Some(HuntEnd::Escaped),
        };
        if let Some(end) = end {
            // Stand down: safe, and first clear of anything it's heading
            // into (the chase may have left it fast and close), then back to
            // the route, resting a while before the next hunt.
            let (sys, positions) = bus.positions();
            let push = self.keep_clear(&sys, &ship, &positions, now);
            if push.length() > 0.01 && ship.is_flying() {
                let (throttle, rcs, nose) = thrust_for(&ship, push, push.normalize());
                let c = ShipCommands { throttle, rcs, weapons: Some(Triggers::default()), arm: Some(false), gun_target: Some(None), ..ship.holding() };
                let happened = bus.command(&c);
                self.record(happened, events);
                return (Some(attitude(&ship, facing(nose, ship.orientation * DVec3::Y), DVec3::ZERO, 1.0 / 60.0)), None);
            }
            self.hunting = None;
            self.track = None;
            self.rest_until = now + REST;
            let c = ShipCommands {
                throttle: 0.0,
                rcs: DVec3::ZERO,
                weapons: Some(Triggers::default()),
                arm: Some(false),
                gun_target: Some(None),
                ..ship.holding()
            };
            let happened = bus.command(&c);
            self.record(happened, events);
            self.route.active = true;
            return (None, Some(end));
        }
        let prey = prey.expect("still hunted");
        (Some(self.attack(bus, &ship, prey, events)), None)
    }

    /// What it takes to keep clear of everything solid: `avoid`'s margins,
    /// and the collision warning a few times a second. If what the ship is
    /// doing now runs into something within `LOOK_AHEAD`, it breaks off away
    /// from it, hard, until the path is clear again.
    fn keep_clear(&mut self, sys: &StarSystem, ship: &Ship, positions: &[DVec3], now: f64) -> DVec3 {
        let mut evade = avoid(sys, ship, positions, now);
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
    fn attack(&mut self, bus: &mut impl Bus, ship: &Ship, prey: &Sighting, events: &mut Vec<Event>) -> Controls {
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
        let evade = self.keep_clear(&sys, ship, &positions, now);
        accel += evade;

        let solution = lead(ship.position, ship.velocity, prey.position, prey.velocity, track.acceleration, GUN_MUZZLE, SLUG_LIFETIME);
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
        let triggers = Triggers { gun: hot && attacking && in_gimbal && d < GUN_RANGE, laser: hot && d < LASER_FOCUS && ship.forward().angle_between(dir) < 0.07 };
        let c = ShipCommands {
            throttle,
            rcs,
            weapons: Some(triggers),
            gun_target: Some(solution.filter(|_| hot).map(|s| s.aim)),
            ..ship.holding()
        };
        let happened = bus.command(&c);
        self.record(happened, events);

        let up_hint = ship.orientation * DVec3::Y;
        let target: DQuat = facing(nose, up_hint);
        attitude(ship, target, DVec3::ZERO, 1.0 / 60.0)
    }
}

/// How to get `accel`: point the engine along it (else `fallback`) and burn
/// when lined up; the thrusters give the rest. Throttle, thrusters, and
/// where the nose should go.
fn thrust_for(ship: &Ship, accel: DVec3, fallback: DVec3) -> (f64, DVec3, DVec3) {
    let want = accel.normalize_or(fallback);
    let lined_up = ship.forward().angle_between(want) < 0.25;
    let throttle = if lined_up { (accel.length() / ship.main_accel()).min(1.0) } else { 0.0 };
    let rest = accel - ship.forward() * (throttle * ship.main_accel());
    let local = ship.orientation.inverse() * rest / ship.side_accel();
    (throttle, local.clamp(DVec3::splat(-1.0), DVec3::ONE), want)
}

/// The ship's velocity relative to the nearest body (the thing it's likely to hit).
fn v_rel_to_nearest(sys: &StarSystem, ship: &Ship, positions: &[DVec3], now: f64) -> DVec3 {
    let i = (0..sys.bodies.len()).min_by(|&a, &b| positions[a].distance(ship.position).total_cmp(&positions[b].distance(ship.position))).unwrap_or(0);
    ship.velocity - sys.velocity(i, now)
}
