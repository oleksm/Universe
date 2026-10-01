//! Working an asteroid: hitting one, anchoring to one.
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

use glam::DVec3;
use universe_physics::{Contact, RigidBody, Weld};

use crate::damage::{self, HULL_STRENGTH};
use crate::events::ShipEvent;
use crate::ship::{Ship, ShipState, SHIP_RADIUS};
use crate::system::{Body, BodyKind, StarSystem};

/// The anchor reaches this far from the hull (m)...
pub const ANCHOR_REACH: f64 = 30.0;
/// ...and holds if the ship drifts slower than this against the surface (m/s).
pub const ANCHOR_SPEED: f64 = 0.5;
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
    let damage = joules / HULL_STRENGTH;
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
pub fn anchor(sys: &StarSystem, field: Option<usize>, ship: &mut Ship, t: f64, events: &mut Vec<ShipEvent>) {
    let fail = |events: &mut Vec<ShipEvent>, why: &str| events.push(ShipEvent::AnchorFailed { why: why.to_string() });
    if !matches!(ship.state, ShipState::Flying) || ship.hyperdrive {
        return fail(events, "NOT IN FREE FLIGHT");
    }
    let Some(f) = field else { return fail(events, "NOTHING IN REACH") };
    let bodies = sys.field_bodies(f);
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

/// Let go: drifting with the surface where the ship was held.
pub fn release(ship: &mut Ship, events: &mut Vec<ShipEvent>) {
    if matches!(ship.state, ShipState::Anchored { .. }) {
        ship.state = ShipState::Flying;
        events.push(ShipEvent::AnchorReleased);
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
            let expected = 0.5 * p.ship.mass() * speed * speed * (1.0 - RESTITUTION * RESTITUTION) / HULL_STRENGTH;
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
}
