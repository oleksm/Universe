//! Working an asteroid: hitting one, anchoring to one, digging it.
//!
//! An asteroid a kilometre across pulls with a ten-thousandth of a g; you
//! can't land on it, you hold on. The anchor is a harpoon on a tether: it
//! reaches `ANCHOR_REACH` from the hull, and it holds only if the ship is
//! drifting with the rock's surface there (slower than `ANCHOR_SPEED`
//! against it: a spinning rock's surface moves). Held, the ship rides the
//! rock's orbit and spin, its devices idle, until the anchor lets go and
//! leaves it drifting with that surface.
//!
//! Touching a rock is a collision like any other: a bounce, and the energy
//! lost in it goes into the hull (the rock, a billion times heavier, doesn't
//! notice).
//!
//! The excavator digs while anchored, at the rate its power buys against the
//! energy it takes to break a kilogram of that rock loose — scooping a rubble
//! pile's gravel is cheap, cutting a stone harder, cutting metal hardest —
//! and no faster than it can carry spoil (`EXCAVATOR_POWER`,
//! `EXCAVATOR_THROUGHPUT`, `specific_energy`). What it digs is the rock's
//! ore (`ore`), into the hopper, and a tonne at a time into the hold. A rock
//! holds what its mass holds: the world remembers what's been dug out of
//! each (`World::dug`), and a rock worked out stays worked out.

use glam::DVec3;
use universe_physics::{Contact, RigidBody, Weld};

use crate::belt::{Rock, RockClass, Structure};
use crate::damage;
use crate::events::ShipEvent;
use crate::goods::{Ore, TONNE};
use crate::ship::{Ship, ShipState, SHIP_RADIUS};
use crate::system::{Body, BodyKind, StarSystem};

/// The excavator's power (W)...
pub const EXCAVATOR_POWER: f64 = 300_000.0;
/// ...and the most spoil it can carry off (kg/s).
pub const EXCAVATOR_THROUGHPUT: f64 = 10.0;
/// An M-type this rich in platinum-group metals (ppm) yields PGM-rich ore.
const PGM_RICH: f64 = 30.0;

/// Energy to break a kilogram of `rock` loose (J/kg): gravel scoops up;
/// solid ice cuts easily, stone harder, nickel-iron hardest.
pub fn specific_energy(rock: &Rock) -> f64 {
    match (rock.structure, rock.class) {
        (Structure::Rubble, _) => 2_000.0,
        (Structure::Monolith, RockClass::Icy) => 20_000.0,
        (Structure::Monolith, RockClass::Carbonaceous) => 30_000.0,
        (Structure::Monolith, RockClass::Stony) => 60_000.0,
        (Structure::Monolith, RockClass::Metallic) => 400_000.0,
    }
}

/// How fast the excavator digs `rock` (kg/s).
pub fn dig_rate(rock: &Rock) -> f64 {
    (EXCAVATOR_POWER / specific_energy(rock)).min(EXCAVATOR_THROUGHPUT)
}

/// What digging `rock` yields.
pub fn ore(rock: &Rock) -> Ore {
    match rock.class {
        RockClass::Icy => Ore::WaterIce,
        RockClass::Carbonaceous => Ore::Carbonaceous,
        RockClass::Stony => Ore::Stony,
        RockClass::Metallic if rock.composition.pgm_ppm >= PGM_RICH => Ore::Pgm,
        RockClass::Metallic => Ore::NickelIron,
    }
}

/// The anchor reaches this far from the hull (m)...
pub const ANCHOR_REACH: f64 = 30.0;
/// ...and holds if the ship drifts slower than this against the surface (m/s).
pub const ANCHOR_SPEED: f64 = 0.5;
/// Closing on a rock to anchor, a ship holds this far off its surface (m).
pub const CLOSE_STANDOFF: f64 = 12.0;
/// Share of the closing speed kept in a bounce off a rock.
pub const RESTITUTION: f64 = 0.3;
/// Slower than this, touching a rock just eases the ship off it (m/s).
const IMPACT: f64 = 0.3;

/// The ship touched asteroid `rock` (contact `c`): the energy lost goes into
/// the hull. False if that wrecked it.
pub fn strike(ship: &mut Ship, rock: &Body, c: &Contact, events: &mut Vec<ShipEvent>) -> bool {
    let closing = -c.relative_velocity.dot(c.normal);
    if closing < IMPACT {
        return true;
    }
    let joules = 0.5 * ship.mass() * closing * closing * (1.0 - RESTITUTION * RESTITUTION);
    let damage = joules / ship.spec().hull_strength;
    ship.hull = (ship.hull - damage).max(0.0);
    events.push(ShipEvent::StruckRock { body: rock.name.clone(), speed: closing, damage });
    if ship.hull <= 0.0 {
        damage::destroy(ship, &rock.name, events);
        return false;
    }
    true
}

/// How far the hull is from asteroid `i`'s surface (m), at `t`.
pub fn clearance(bodies: &[Body], i: usize, t: f64, positions: &[DVec3], p: DVec3) -> f64 {
    let b = &bodies[i];
    p.distance(positions[i]) - b.surface_radius_at(positions[i], p, t) - SHIP_RADIUS
}

/// Fire the anchor at the nearest asteroid among field `field`'s bodies, at `t`.
pub fn anchor(field: Option<(usize, std::sync::Arc<Vec<Body>>)>, ship: &mut Ship, t: f64, events: &mut Vec<ShipEvent>) {
    let fail = |events: &mut Vec<ShipEvent>, why: &str| events.push(ShipEvent::AnchorFailed { why: why.to_string() });
    if !matches!(ship.state, ShipState::Flying) || ship.hyperdrive {
        return fail(events, "NOT IN FREE FLIGHT");
    }
    let Some((f, bodies)) = field else { return fail(events, "NOTHING IN REACH") };
    let mut positions = Vec::with_capacity(bodies.len());
    universe_physics::positions(&bodies[..], t, &mut positions);
    let nearest = (0..bodies.len())
        .filter(|&i| bodies[i].kind == BodyKind::Asteroid && positions[i].distance(ship.position) < bodies[i].max_radius() + SHIP_RADIUS + ANCHOR_REACH)
        .map(|i| (i, clearance(&bodies, i, t, &positions, ship.position)))
        .min_by(|a, b| a.1.total_cmp(&b.1));
    let Some((i, _)) = nearest.filter(|(_, gap)| *gap < ANCHOR_REACH) else { return fail(events, "NOTHING IN REACH") };
    let b = &bodies[i];
    let surface = universe_physics::velocity(&bodies[..], i, t) + b.angular_velocity().cross(ship.position - positions[i]);
    let drift = (ship.velocity - surface).length();
    if drift > ANCHOR_SPEED {
        return fail(events, &format!("DRIFTING {drift:.1} M/S"));
    }
    let weld = Weld::capture(&bodies[..], i, t, &positions, &ship.rigid());
    ship.state = ShipState::Anchored { field: f, body: i, local_position: weld.local_position, local_orientation: weld.local_orientation };
    ship.throttle = 0.0;
    ship.rcs = DVec3::ZERO;
    events.push(ShipEvent::Anchored { body: b.name.clone() });
}

/// Let go: drifting with the surface where the ship was held. The
/// excavator stops; loose ore in the hopper drifts off.
pub fn release(ship: &mut Ship, events: &mut Vec<ShipEvent>) {
    if matches!(ship.state, ShipState::Anchored { .. }) {
        ship.state = ShipState::Flying;
        ship.excavator = false;
        ship.hopper = 0.0;
        events.push(ShipEvent::AnchorReleased);
    }
}

/// `dt` seconds of the excavator on an anchored ship, `dug` kilograms having
/// been dug out of its rock already (before this ship's hopper).
pub fn excavate(sys: &StarSystem, ship: &mut Ship, dug: f64, dt: f64, events: &mut Vec<ShipEvent>) {
    let ShipState::Anchored { field, body, .. } = ship.state else { return };
    if !ship.excavator {
        return;
    }
    let bodies = sys.field_bodies(field);
    let b = &bodies[body];
    let Some(rock) = b.rock.as_ref() else { return };
    let mut stop = |ship: &mut Ship, why: &str| {
        ship.excavator = false;
        events.push(ShipEvent::ExcavatorStopped { why: why.to_string() });
    };
    let room = ship.hold_room();
    let left = b.mass - dug - ship.hopper;
    if room < 1.0 {
        return stop(ship, "HOLD FULL");
    }
    if left < 1.0 {
        return stop(ship, "ROCK WORKED OUT");
    }
    ship.hopper += (dig_rate(rock) * dt).min(room).min(left);
    let item = ore(rock).item();
    while ship.hopper >= TONNE - 1e-9 {
        ship.hopper = (ship.hopper - TONNE).max(0.0);
        ship.cargo += TONNE;
        events.push(ShipEvent::Mined { field, rock: body, item });
    }
}

/// Held: carried with the rock, at `t`.
pub fn hold(sys: &StarSystem, ship: &mut Ship, t: f64) {
    let ShipState::Anchored { field, body, local_position, local_orientation } = ship.state else { return };
    let bodies = sys.field_bodies(field);
    // (The rock and what it orbits only, not the whole swarm.)
    let mut center = DVec3::ZERO;
    let mut i = Some(body);
    while let Some(k) = i {
        let rail = &bodies[k].rail;
        if let Some(o) = &rail.orbit {
            center += o.position(t);
        }
        i = rail.parent;
    }
    let rail = &bodies[body].rail;
    let rot = rail.rotation(t);
    let offset = rot * local_position;
    let mut rigid: RigidBody = ship.rigid();
    rigid.position = center + offset;
    rigid.velocity = universe_physics::velocity(&bodies[..], body, t) + rail.angular_velocity().cross(offset);
    rigid.orientation = rot * local_orientation;
    rigid.angular_velocity = DVec3::ZERO;
    ship.set_rigid(&rigid);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ship::ShipCommands;
    use crate::testkit::Probe;

    /// A ship `gap` metres off the surface of a fragment of the home system's
    /// first field, drifting with that surface plus `drift` (m/s, along the
    /// outward normal). Returns the probe and the fragment's index.
    fn by_a_rock(gap: f64, drift: f64) -> (Probe, usize) {
        let mut p = Probe::new(1984);
        let sys = p.sys();
        let bodies = sys.field_bodies(0);
        let n = sys.bodies.len();
        let t = p.world.time;
        let mut pos = Vec::new();
        universe_physics::positions(&bodies[..], t, &mut pos);
        // A rock of some size, with room around it.
        let i = (n..bodies.len()).find(|&i| bodies[i].rail.radius > 15.0 && (n..bodies.len()).all(|j| j == i || pos[j].distance(pos[i]) > 2000.0)).expect("a rock");
        let b = &bodies[i];
        let up = DVec3::Y;
        let local = b.rotation(t).inverse() * up;
        let surface = pos[i] + up * b.surface_radius(local);
        p.ship.position = surface + up * (SHIP_RADIUS + gap);
        p.ship.velocity = universe_physics::velocity(&bodies[..], i, t) + b.angular_velocity().cross(p.ship.position - pos[i]) + up * drift;
        p.ship.angular_velocity = DVec3::ZERO;
        p.system = sys.index;
        (p, i)
    }

    fn anchor_cmd(p: &mut Probe, on: bool) {
        let c = ShipCommands { anchor: Some(on), ..p.ship.holding() };
        p.command(&c);
    }

    #[test]
    fn a_rock_is_hit_like_anything_else() {
        // Drifting down onto it: gently, a bounce and a scrape; hard, a wreck.
        for (speed, survives) in [(3.0, true), (40.0, false)] {
            let (mut p, _) = by_a_rock(5.0, -speed);
            for _ in 0..120 {
                p.step(1.0 / 60.0, 1.0);
            }
            let struck = p.events.iter().find_map(|e| match e {
                ShipEvent::StruckRock { speed, damage, .. } => Some((*speed, *damage)),
                _ => None,
            });
            let (closing, damage) = struck.unwrap_or_else(|| panic!("{speed} m/s: no strike in {:?}", p.events));
            assert!((closing - speed).abs() < 0.5, "struck at {closing}");
            let expected = 0.5 * p.ship.mass() * speed * speed * (1.0 - RESTITUTION * RESTITUTION) / p.ship.spec().hull_strength;
            assert!((damage - expected).abs() < expected * 0.3, "{damage} vs {expected}");
            assert_eq!(!p.crashed(), survives, "{speed} m/s: {:?}", p.events);
        }
    }

    #[test]
    fn the_anchor_holds_a_ship_drifting_with_the_surface_and_lets_go() {
        // Too fast against the surface: it doesn't hold.
        let (mut p, _) = by_a_rock(10.0, 2.0);
        anchor_cmd(&mut p, true);
        assert!(matches!(p.ship.state, ShipState::Flying), "{:?}", p.events);
        assert!(matches!(p.events.last(), Some(ShipEvent::AnchorFailed { .. })));

        // With it: held, riding the rock's orbit and spin.
        let (mut p, i) = by_a_rock(10.0, 0.1);
        anchor_cmd(&mut p, true);
        assert!(matches!(p.ship.state, ShipState::Anchored { field: 0, body, .. } if body == i), "{:?}", p.events);
        let sys = p.sys();
        let bodies = sys.field_bodies(0);
        let gap = |p: &mut Probe| {
            let mut pos = Vec::new();
            universe_physics::positions(&bodies[..], p.world.time, &mut pos);
            clearance(&bodies, i, p.world.time, &pos, p.ship.position)
        };
        let before = gap(&mut p);
        for _ in 0..600 {
            p.step(1.0 / 60.0, 1.0);
        }
        assert!(matches!(p.ship.state, ShipState::Anchored { .. }));
        assert!((gap(&mut p) - before).abs() < 0.01, "held where it was");

        // Let go: drifting with the surface there.
        anchor_cmd(&mut p, false);
        assert!(matches!(p.ship.state, ShipState::Flying));
        let t = p.world.time;
        let mut pos = Vec::new();
        universe_physics::positions(&bodies[..], t, &mut pos);
        let surface = universe_physics::velocity(&bodies[..], i, t) + bodies[i].angular_velocity().cross(p.ship.position - pos[i]);
        assert!(p.ship.velocity.distance(surface) < 1e-6);
    }

    #[test]
    fn the_excavator_digs_what_its_power_buys_into_the_hold() {
        let (mut p, i) = by_a_rock(10.0, 0.0);
        anchor_cmd(&mut p, true);
        p.command(&ShipCommands { excavate: Some(true), ..p.ship.holding() });
        assert!(p.ship.excavator);
        let sys = p.sys();
        let bodies = sys.field_bodies(0);
        let rock = bodies[i].rock.clone().unwrap();
        let rate = dig_rate(&rock);
        let mass = p.ship.mass();
        let secs = 2500.0 / rate;
        for _ in 0..(secs * 60.0).round() as usize {
            p.step(1.0 / 60.0, 1.0);
        }
        let tonnes = p.events.iter().filter(|e| matches!(e, ShipEvent::Mined { item, .. } if *item == ore(&rock).item())).count();
        assert_eq!(tonnes, 2, "{rate} kg/s for {secs} s");
        assert!((p.ship.cargo - 2.0 * TONNE).abs() < 1e-6 && (p.ship.hopper - 500.0).abs() < 1.0, "hold {} hopper {}", p.ship.cargo, p.ship.hopper);
        assert!((p.ship.mass() - mass - 2500.0).abs() < 1.0, "the ship weighs what it dug");

        // A full hold stops it; so does a worked-out rock.
        p.ship.cargo = p.ship.spec().hold_capacity - 600.0;
        for _ in 0..(200.0 / rate * 60.0) as usize {
            p.step(1.0 / 60.0, 1.0);
        }
        assert!(!p.ship.excavator && matches!(p.events.last(), Some(ShipEvent::ExcavatorStopped { why }) if why == "HOLD FULL"), "{:?}", p.events.last());
        p.ship.cargo = 0.0;
        p.ship.excavator = true;
        let mut events = Vec::new();
        for _ in 0..1000 {
            excavate(&sys, &mut p.ship, bodies[i].mass - 800.0, 1.0, &mut events);
        }
        assert!(matches!(events.last(), Some(ShipEvent::ExcavatorStopped { why }) if why == "ROCK WORKED OUT"));
        assert!(p.ship.hopper <= 800.0 + 1e-6);

        // Letting go stops it, and the loose ore drifts off.
        p.ship.excavator = true;
        anchor_cmd(&mut p, false);
        assert!(!p.ship.excavator && p.ship.hopper == 0.0);
    }

    #[test]
    fn harder_rock_digs_slower_and_each_class_yields_its_ore() {
        let mut rng = crate::rng::Rng::new(7);
        let dig = |class, d: f64, rng: &mut crate::rng::Rng| {
            let r = crate::belt::rock_for_test(class, d, 0.5, rng);
            (dig_rate(&r), ore(&r))
        };
        let (gravel, _) = dig(RockClass::Stony, 400.0, &mut rng);
        let (stone, s) = dig(RockClass::Stony, 50.0, &mut rng);
        let (ice, i) = dig(RockClass::Icy, 50.0, &mut rng);
        let (c, cc) = dig(RockClass::Carbonaceous, 50.0, &mut rng);
        assert_eq!((gravel, stone, ice), (EXCAVATOR_THROUGHPUT, 5.0, EXCAVATOR_THROUGHPUT));
        assert_eq!((s, i, cc), (Ore::Stony, Ore::WaterIce, Ore::Carbonaceous));
        assert!(c > stone);
        // Solid nickel-iron is the hardest; rich in platinum metals, it's PGM ore.
        let mut metal = crate::belt::rock_for_test(RockClass::Metallic, 50.0, 0.5, &mut rng);
        metal.structure = Structure::Monolith;
        assert!(dig_rate(&metal) < 1.0);
        metal.composition.pgm_ppm = 10.0;
        assert_eq!(ore(&metal), Ore::NickelIron);
        metal.composition.pgm_ppm = 50.0;
        assert_eq!(ore(&metal), Ore::Pgm);
    }

    #[test]
    fn a_rock_dug_into_is_the_smaller_for_it() {
        let (mut p, i) = by_a_rock(5.0, 0.0);
        let sys = p.sys();
        let whole = sys.field_bodies(0)[i].clone();
        // Seven eighths of it dug out: half as wide.
        p.world.mined.insert((sys.index, 0, i), whole.mass * 7.0 / 8.0);
        let now = p.world.field_bodies_now(&sys, 0);
        assert!((now[i].rail.radius / whole.rail.radius - 0.5).abs() < 1e-9);
        assert!((now[i].mass - whole.mass / 8.0).abs() < 1.0);
        // Where its surface was, there's nothing to touch.
        for _ in 0..60 {
            p.step(1.0 / 60.0, 1.0);
        }
        assert!(!p.events.iter().any(|e| matches!(e, ShipEvent::StruckRock { .. })));
        // The gap to its surface grew by what its radius lost.
        let mut pos = Vec::new();
        universe_physics::positions(&now[..], p.world.time, &mut pos);
        let gap = clearance(&now, i, p.world.time, &pos, p.ship.position);
        assert!((gap - (5.0 + whole.rail.radius * 0.5)).abs() < whole.rail.radius * 0.3 + 1.0, "gap {gap}");
    }
}
