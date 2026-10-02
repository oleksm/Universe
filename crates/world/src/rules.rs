//! Contact rules: what a structure's part does to a body that touches it or
//! passes through it — registered ahead by whoever owns the structure (see
//! `structures`; later, the services), applied here by the core without
//! knowing what a station, a spaceport or a gate is. A decision that has to
//! be taken at the moment of contact can't wait for anyone to be asked, so
//! it's data: speed limits, a pose to lock into, what to say. Every firing
//! is told (`ShipEvent::RuleFired`).

use std::collections::HashMap;

use glam::DVec3;
use universe_physics::{Fact, Feature, Frame, Relative};

use crate::damage;
use crate::events::ShipEvent;
use crate::ship::{upright, Ship, ShipState, SHIP_RADIUS};
use crate::system::StarSystem;

/// Which part of a structure (or body) was touched.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Part {
    /// Solid ground, and liquid (a sea).
    Ground,
    Sea,
    /// A structure's outer faces, and its deck (landing pads).
    Hull,
    Deck,
    /// A ring's tube, and the opening it encloses (passed through, not touched).
    Ring,
    Opening,
}

impl Part {
    pub fn of(f: Feature) -> Part {
        match f {
            Feature::Surface { liquid: false } => Part::Ground,
            Feature::Surface { liquid: true } => Part::Sea,
            Feature::Hull => Part::Hull,
            Feature::Deck(_) => Part::Deck,
            Feature::Ring => Part::Ring,
        }
    }
}

/// What a part does to a body that touches it.
#[derive(Clone, Debug)]
pub enum Rule {
    /// Lock the body to the structure if it arrives slower than `max_speed`:
    /// held there, devices idle. Else it's wrecked by `otherwise`.
    Lock { name: String, max_speed: f64, pose: Pose, says: Says, otherwise: String },
    /// Pass the body through to the structure's twin in system `to`, if
    /// slower than `max_speed` (its motion relative to this one kept for the
    /// other side). Else it's wrecked by `otherwise`.
    Transit { name: String, max_speed: f64, to: usize, says: ShipEvent, otherwise: String },
    /// Bounce off below `max_speed` (during the step, see `bounces`); harder
    /// than that, wrecked by `otherwise`.
    Bounce { name: String, max_speed: f64, otherwise: String },
    /// Wrecked, by `cause`.
    Wreck { name: String, cause: String },
}

/// Where a locked body is held.
#[derive(Clone, Copy, Debug)]
pub enum Pose {
    /// Where it touched a structure's deck, belly down, heading as it was.
    Deck,
    /// Where it touched the ground, belly down.
    Ground,
}

/// What a lock announces.
#[derive(Clone, Debug)]
pub enum Says {
    Always(ShipEvent),
    /// By where on the body it touched: the first zone within its angle of
    /// the body-frame direction, else `otherwise`.
    Zones { zones: Vec<(DVec3, f64, ShipEvent)>, otherwise: ShipEvent },
}

/// How a structure lets go of a body locked to it, once the body's
/// devices are commanded on.
#[derive(Clone, Debug)]
pub enum Release {
    /// A burn with the nose up, or the lift thrusters with the belly down:
    /// lifted `lift` metres clear at `speed`, along `up` (the structure's own
    /// frame: a deck's), or the local vertical (None: a world's ground).
    LiftOff { speed: f64, lift: f64, up: Option<DVec3>, says: ShipEvent },
}

/// The rules of a star system's parts, by (body, part), and how each
/// structure lets go.
#[derive(Clone, Debug, Default)]
pub struct Rules {
    rules: HashMap<(usize, Part), Rule>,
    releases: HashMap<usize, Release>,
}

impl Rules {
    pub fn set_release(&mut self, body: usize, release: Release) {
        self.releases.insert(body, release);
    }

    pub fn release(&self, body: usize) -> Option<&Release> {
        self.releases.get(&body)
    }

    pub fn set(&mut self, body: usize, part: Part, rule: Rule) {
        self.rules.insert((body, part), rule);
    }

    pub fn get(&self, body: usize, part: Part) -> Option<&Rule> {
        self.rules.get(&(body, part))
    }

    /// During the step: does touching `part` of `body` at `speed` bounce the
    /// body off (and so the step goes on)?
    pub fn bounces(&self, body: usize, part: Part, speed: f64) -> bool {
        matches!(self.get(body, part), Some(Rule::Bounce { max_speed, .. }) if speed < *max_speed)
    }
}

/// The rule for what the step stopped on, applied to `ship` (in system
/// `system`, at `t`, bodies at `positions`). With no rule, touching anything
/// solid wrecks the ship, as it would.
#[allow(clippy::too_many_arguments)]
pub fn apply(rules: &Rules, sys: &StarSystem, system: usize, ship: &mut Ship, fact: &Fact, t: f64, positions: &[DVec3], events: &mut Vec<ShipEvent>) {
    let (body, part, speed) = match fact {
        Fact::Trigger { body, .. } => (*body, Part::Opening, (ship.velocity - sys.velocity(*body, t)).length()),
        Fact::Contact(c) => (c.body, Part::of(c.feature), c.relative_velocity.length()),
    };
    let fallback = Rule::Wreck { name: sys.bodies[body].name.clone(), cause: sys.bodies[body].name.clone() };
    let rule = rules.get(body, part).unwrap_or(&fallback);
    let fired = |events: &mut Vec<ShipEvent>, name: &str, outcome: &str| events.push(ShipEvent::RuleFired { rule: name.to_string(), outcome: outcome.to_string() });
    match rule {
        Rule::Lock { name, max_speed, pose, says, otherwise } => {
            let Fact::Contact(c) = fact else { return };
            let b = &sys.bodies[body];
            let rot = b.rotation(t);
            let held = match *pose {
                Pose::Deck => (speed < *max_speed).then(|| (crate::station::rest(c.local), upright(rot * DVec3::Y, ship.forward()))),
                Pose::Ground => (speed < *max_speed).then(|| (c.local * (b.surface_radius(c.local) + SHIP_RADIUS), upright(c.normal, ship.forward()))),
            };
            match held {
                Some((local_position, orientation)) => {
                    ship.orientation = orientation;
                    ship.throttle = 0.0;
                    ship.rcs = DVec3::ZERO;
                    ship.angular_velocity = DVec3::ZERO;
                    ship.state = ShipState::Landed { body, local_position, local_orientation: rot.inverse() * orientation };
                    ship.locked_at = t;
                    // Set down: the flight systems power down (the pilot powers up to leave).
                    ship.powered = false;
                    events.push(ShipEvent::SystemsOff);
                    fired(events, name, "locked");
                    events.push(match says {
                        Says::Always(e) => e.clone(),
                        Says::Zones { zones, otherwise } => zones.iter().find(|(d, a, _)| d.angle_between(c.local) < *a).map_or_else(|| otherwise.clone(), |z| z.2.clone()),
                    });
                }
                None => {
                    fired(events, name, "refused: wrecked");
                    damage::destroy(ship, otherwise, events);
                }
            }
        }
        Rule::Transit { name, max_speed, to, says, otherwise } => {
            // One way: against the gate's axis, it's an empty ring.
            let Fact::Trigger { relative_velocity, .. } = fact else { return };
            if relative_velocity.dot(sys.bodies[body].rotation(t) * DVec3::Y) <= 0.0 {
                fired(events, name, "the wrong way: through an empty ring");
                return;
            }
            if speed > *max_speed {
                fired(events, name, "too fast: wrecked");
                events.push(ShipEvent::GateTooFast { speed });
                damage::destroy(ship, otherwise, events);
                return;
            }
            // Relative to the structure's pose and drift (it doesn't spin), kept for the other side.
            let frame = Frame { angular_velocity: DVec3::ZERO, ..Frame::of(&sys.bodies, body, t, positions) };
            let local = Relative::of(&frame, &ship.rigid());
            ship.state = ShipState::Transit {
                to: *to,
                from: system,
                remaining: crate::gate::TRANSIT_TIME,
                local_velocity: local.velocity,
                local_offset: DVec3::new(local.position.x, 0.0, local.position.z),
                local_orientation: local.orientation,
            };
            ship.rcs = DVec3::ZERO;
            ship.throttle = 0.0;
            fired(events, name, "transit");
            events.push(says.clone());
        }
        Rule::Bounce { name, otherwise, .. } => {
            fired(events, name, "too hard: wrecked");
            damage::destroy(ship, otherwise, events);
        }
        Rule::Wreck { name, cause } => {
            fired(events, name, "wrecked");
            damage::destroy(ship, cause, events);
        }
    }
}

/// A body locked to `body` (at `local_position` in its frame) with its
/// devices as set: let go, if its release says so. Returns whether it did.
#[allow(clippy::too_many_arguments)]
pub fn release(rules: &Rules, sys: &StarSystem, body: usize, local_position: DVec3, ship: &mut Ship, t: f64, events: &mut Vec<ShipEvent>) -> bool {
    let Some(release) = rules.release(body) else { return false };
    // (Inside a hangar, or taxiing, nothing lifts off: out onto a pad first.)
    if ship.hangar.is_some() || ship.taxi.is_some() {
        return false;
    }
    let b = &sys.bodies[body];
    let rot = b.rotation(t);
    match release {
        Release::LiftOff { speed, lift, up, says } => {
            let normal = up.map_or_else(|| (rot * local_position).normalize(), |u| rot * u);
            let nose_up_burn = ship.throttle > 0.05 && ship.forward().dot(normal) > 0.2;
            let lifting = ship.rcs.y > 0.1 && (ship.orientation * DVec3::Y).dot(normal) > 0.5;
            if !(nose_up_burn || lifting) {
                return false;
            }
            ship.velocity += normal * *speed;
            ship.position += normal * *lift;
            ship.state = ShipState::Flying;
            events.push(says.clone());
            true
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    use crate::station::StationFrame;
    
    use crate::testkit::Probe;

    fn fired(events: &[ShipEvent], outcome: &str) -> bool {
        events.iter().any(|e| matches!(e, ShipEvent::RuleFired { outcome: o, .. } if o == outcome))
    }

    #[test]
    fn the_deck_takes_a_gentle_touchdown_on_a_pad_and_says_so() {
        let mut p = Probe::new(42);
        let sys = p.sys();
        let station = sys.station().unwrap();
        let pos = p.positions();
        let f = StationFrame::new(&sys, station, p.world.time, &pos);
        // Just above pad 5, upright, sinking gently.
        p.ship.position = f.pad(5) + f.up() * 8.0;
        p.ship.velocity = f.velocity_at(p.ship.position) - f.up() * 3.0;
        p.ship.orientation = f.landing_orientation();
        for _ in 0..600 {
            p.step(1.0 / 60.0, 1.0);
            if !p.ship.is_flying() {
                break;
            }
        }
        let ShipState::Landed { body, local_position, .. } = p.ship.state else { panic!("{:?}", p.events) };
        assert_eq!(body, station);
        assert_eq!(crate::station::pad_at(local_position), Some(5));
        assert!(fired(&p.events, "locked"), "{:?}", p.events);
        assert!(p.events.iter().any(|e| matches!(e, ShipEvent::Landed { station: true, .. })));
        // Set down, it's parked: powered down, its thrusters and turn don't answer.
        assert!(!p.ship.powered);
        let parked = p.ship.orientation;
        for _ in 0..60 {
            p.ship.rcs = DVec3::Y;
            p.step(1.0 / 60.0, 1.0);
        }
        assert!(matches!(p.ship.state, ShipState::Landed { .. }) && p.ship.jets.is_empty(), "nothing fires powered down");
        assert!(p.ship.orientation.angle_between(parked) < 0.05);
        // Powered up, the lift thrusters take it off the deck.
        p.ship.powered = true;
        for _ in 0..60 {
            p.ship.rcs = DVec3::Y;
            p.step(1.0 / 60.0, 1.0);
        }
        assert!(p.ship.is_flying(), "{:?}", p.events);
    }

}
