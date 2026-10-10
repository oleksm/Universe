//! The hyperdrive: a fictional device, with explicit rules.
//!
//! While engaged it carries the ship along a heading (the nose, or one
//! commanded) at a speed set by the throttle and the room to move: the
//! drive's top speed (its product's), held near bodies by its avionics'
//! governor (K × the distance to the nearest surface, or to a commanded
//! destination so as never to overshoot it). It burns fuel by the way it goes (`hyper::field_cost`), the
//! same anywhere: what limits how far a ship goes is the fuel it carries. It moves relative to a reference frame
//! (commanded, or the dominant body's), since targets ride along with their
//! planets at tens of km/s. Its interlock (a feature of the drive, not a law)
//! never carries the ship into a body.
//! Dropping out leaves the ship with the commanded exit velocity (or
//! co-moving with the dominant body), engines at zero.
//!
//! Hyperdrive speed is defined per real second; the world clock (and the
//! frame's own motion) runs at `warp` game seconds per real second.

use glam::DVec3;
use universe_physics::segment_distance;

use crate::events::ShipEvent;
use crate::galaxy::Galaxy;
use crate::ship::{HyperdriveCommand, Ship};
use crate::system::{BodyKind, StarSystem};
use crate::units::SUN_RADIUS;
// (Its constants are the physics sheet's: the Dogma registry, standards/Dogma.)
pub use crate::sheet::GROUND_MARGIN;
use universe_physics::hyper::field_cost;


/// Engage or disengage as commanded (only in flight). Both ways the engine
/// is set back to zero: hyperdrive speed follows the throttle, so carrying
/// full main-engine throttle in (or out) would fling the ship away.
pub fn switch(sys: &StarSystem, ship: &mut Ship, c: &HyperdriveCommand, t: f64, positions: &[DVec3], events: &mut Vec<ShipEvent>) {
    if !ship.is_flying() || c.engage == ship.hyperdrive {
        return;
    }
    if ship.hyperdrive {
        drop_out(sys, ship, c.exit_velocity, t, positions, true, events);
    } else if ship.fuel <= 0.0 {
        events.push(ShipEvent::OutOfFuel);
        return;
    } else if ship.hyper_jam > 0.0 {
        // Hits disrupt the drive: it won't engage for a while.
        events.push(ShipEvent::HyperdriveJammed { seconds: ship.hyper_jam });
        return;
    } else {
        ship.hyper_engaged = Some((t, ship.throttle, ship.velocity));
        ship.hyperdrive = true;
        events.push(ShipEvent::HyperdriveEngaged);
    }
    ship.throttle = 0.0;
}

/// A drive dropping out this soon after it was switched on hasn't really
/// run (a planet in the way): the ship keeps the throttle and velocity it had (s).
const FALSE_START: f64 = 1.0;

/// Out of hyperdrive with `exit` velocity, or co-moving with the dominant body.
/// (`ordered`: by its pilot, not on its own: it doesn't give back what it had.)
fn drop_out(sys: &StarSystem, ship: &mut Ship, exit: Option<DVec3>, t: f64, positions: &[DVec3], ordered: bool, events: &mut Vec<ShipEvent>) {
    ship.velocity = match exit {
        Some(v) => v,
        None => sys.velocity(sys.dominant(ship.position, positions), t),
    };
    ship.hyperdrive = false;
    ship.throttle = 0.0;
    if let Some((since, throttle, velocity)) = ship.hyper_engaged.take()
        && !ordered
        && t - since < FALSE_START
    {
        (ship.throttle, ship.velocity) = (throttle, velocity);
    }
    events.push(ShipEvent::HyperdriveDisengaged);
}

/// One frame under hyperdrive, the world having moved on to `t` (`positions`
/// at `t`), ship in system `system` with its `neighbours` (nearby stars).
#[allow(clippy::too_many_arguments)]
pub fn cruise(
    galaxy: &Galaxy,
    neighbours: &[usize],
    system: usize,
    sys: &StarSystem,
    ship: &mut Ship,
    cmd: &HyperdriveCommand,
    t: f64,
    positions: &[DVec3],
    real_dt: f64,
    warp: f64,
    events: &mut Vec<ShipEvent>,
) {
    if !cmd.engage {
        // (Carried along with the frame for the frame, as the bodies were.)
        let base = cmd.frame_velocity.unwrap_or_else(|| sys.velocity(sys.dominant(ship.position, positions), t));
        ship.position += base * (real_dt * warp);
        drop_out(sys, ship, cmd.exit_velocity, t, positions, true, events);
        return;
    }
    let p = ship.position;

    // Nearest surface: (clearance, radius, center, body index or None for other stars).
    let mut nearest = (f64::INFINITY, 0.0, DVec3::ZERO, None);
    for (i, (b, &bp)) in sys.bodies.iter().zip(positions).enumerate() {
        let d = bp.distance(p) - b.rail.radius;
        if !b.kind.artificial() && d < nearest.0 {
            nearest = (d, b.rail.radius, bp, Some(i));
        }
    }
    for &n in neighbours {
        let r = galaxy.stars[n].class.radius_suns() * SUN_RADIUS;
        let c = galaxy.offset(system, n);
        let d = c.distance(p) - r;
        if d < nearest.0 {
            nearest = (d, r, c, None);
        }
    }
    let (clearance, radius, center, body) = nearest;

    let m = ship.mass();
    let eta = ship.spec().hyper_efficiency.max(1e-6);
    let dir = cmd.heading.unwrap_or_else(|| ship.forward());
    // The frame it moves in, and where that carries it this frame: even
    // dropping out, it's been carried along (the bodies have moved on to `t`).
    let base = match cmd.frame_velocity {
        Some(v) => v,
        None => sys.velocity(sys.dominant(p, positions), t),
    };
    let carried = p + base * (real_dt * warp);

    // Obstacle dead ahead: drop out at a safe distance instead of crawling
    // toward its surface. Stars get a wide margin; planets and moons drop
    // you at the top of their air (or just above their highest ground). Near misses fly on past, and so does heading for a
    // destination whose line of approach is clear. (Not when the heading is
    // being steered around obstacles already.)
    let to = center - p;
    let along = to.dot(dir);
    let miss = (to - dir * along).length();
    let is_star = body.is_none_or(|i| sys.bodies[i].kind == BodyKind::Star);
    let margin = match body.map(|i| &sys.bodies[i]) {
        Some(b) if !is_star => {
            let ground = b.max_radius() - b.rail.radius + GROUND_MARGIN;
            b.rail.atmosphere.map_or(ground, |a| a.top.max(ground))
        }
        _ => 3.0 * radius,
    };
    let approach_clear = cmd.destination.is_some_and(|d| {
        Some(d.body) == body && segment_distance(p, d.point + (d.point - center).normalize() * 1000.0, center) > radius
    });
    if !cmd.steering && clearance < margin && along > 0.0 && miss < 1.5 * radius && !approach_clear {
        ship.position = carried;
        drop_out(sys, ship, cmd.exit_velocity, t, positions, false, events);
        return;
    }

    // Speed grows with room to move: distance to the nearest surface, and
    // to the destination if there is one, so we never overshoot it.
    let room = cmd.destination.map_or(clearance, |d| clearance.min(p.distance(d.point)));
    // As fast as the drive goes (its product's), held near bodies by its avionics' governor if fitted.
    let top = ship.spec().hyper_top;
    let cap = ship.spec().governor.map_or(top, |k| (k * room.max(1000.0)).min(top));
    let speed = cap * ship.throttle.max(0.02);
    // What it costs: the field's energy for the way it goes, from the tank (`field_cost`).
    let burnt = ship.hyper_fuel(field_cost(m, speed, eta) * speed * real_dt);
    if burnt >= ship.fuel {
        ship.fuel = 0.0;
        events.push(ShipEvent::OutOfFuel);
        ship.position = carried;
        drop_out(sys, ship, cmd.exit_velocity, t, positions, false, events);
        return;
    }
    ship.fuel -= burnt;
    ship.velocity = base + dir * speed;
    let next = carried + dir * speed * real_dt.min(0.1);
    // Interlock: never hyperdrive into a planet, moon or star.
    // Its avionics' interlock: never into a body (none fitted: it goes where it's pointed).
    let hit = sys.bodies.iter().zip(positions).find(|(b, c)| !b.kind.artificial() && c.distance(next) < b.max_radius() + ship.spec().interlock.unwrap_or(0.0));
    if let Some((b, _)) = hit {
        if ship.spec().interlock.is_none() {
            ship.position = next;
            crate::damage::destroy(ship, &b.name, events);
            return;
        }
        ship.position = carried;
        drop_out(sys, ship, cmd.exit_velocity, t, positions, false, events);
        return;
    }
    ship.position = next;
}

#[cfg(test)]
mod tests {
    

    use super::*;
    use crate::ship::ShipCommands;
    use crate::testkit::Probe;

    /// Commands the hyperdrive every frame: fixed orders, or a dive straight
    /// at body `at` (steering, so the drive's own look-ahead stays out of it).
    struct Orders {
        orders: HyperdriveCommand,
        at: Option<usize>,
    }

    impl Orders {
        fn commands(&self, ship: &Ship, positions: &[DVec3]) -> ShipCommands {
            let mut orders = self.orders;
            if let Some(at) = self.at {
                orders.heading = Some((positions[at] - ship.position).normalize());
            }
            ShipCommands { hyperdrive: Some(orders), ..ship.holding() }
        }
    }

    /// A frame: the orders given (as a pilot would, before the step), then the step.
    fn frame(p: &mut Probe, orders: &mut Orders) {
        let positions = p.positions();
        let c = orders.commands(&p.ship, &positions);
        p.world.step_ship(&mut p.ship, &mut p.system, &c, 1.0 / 60.0, 1.0, &mut p.events);
    }

    #[test]
    fn between_the_stars_the_field_burns_fuel_by_the_way_it_goes() {
        let mut p = Probe::new(42);
        let pos = p.positions();
        // Two light years out from the home star: the same field, the same law.
        p.ship.position = pos[0] + DVec3::X * 2.0 * crate::units::LIGHT_YEAR;
        p.ship.velocity = DVec3::ZERO;
        p.toggle_hyperdrive();
        p.set_throttle(1.0 / 3.0);
        let (before, at) = (p.ship.fuel, p.ship.position);
        p.step(1.0 / 60.0, 1.0);
        assert!(p.ship.hyperdrive, "{:?}", p.events);
        let gone = p.ship.position.distance(at);
        let speed = gone * 60.0;
        let expected = p.ship.hyper_fuel(universe_physics::hyper::field_cost(p.ship.mass(), speed, p.ship.spec().hyper_efficiency) * gone);
        assert!(gone > 1.0e9 && ((before - p.ship.fuel) - expected).abs() < 1e-4 * expected.max(1e-9), "{} kg for {gone:e} m (expected {expected})", before - p.ship.fuel);
    }

    #[test]
    fn the_interlock_never_lets_the_drive_into_a_body() {
        let mut p = Probe::new(42);
        let sys = p.sys();
        let planet = sys.bodies[sys.station().unwrap()].rail.parent.unwrap();
        p.toggle_hyperdrive();
        p.set_throttle(1.0);
        let mut dive = Orders { orders: HyperdriveCommand { steering: true, ..HyperdriveCommand::CRUISE }, at: Some(planet) };
        for _ in 0..(60 * 60) {
            frame(&mut p, &mut dive);
            if !p.ship.hyperdrive {
                break;
            }
        }
        assert!(!p.ship.hyperdrive, "the interlock should have dropped the ship out");
        assert!(p.ship.is_flying() && p.events.contains(&ShipEvent::HyperdriveDisengaged));
        let pos = p.positions();
        let above = p.ship.position.distance(pos[planet]) - sys.bodies[planet].max_radius();
        eprintln!("interlock: dropped out {above:.0} m above the highest terrain");
        assert!(above > 0.0, "never inside the body");
    }

}
