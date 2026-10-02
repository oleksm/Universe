//! The hyperdrive: a fictional device, with explicit rules.
//!
//! While engaged it carries the ship along a heading (the nose, or one
//! commanded) at a speed set by the throttle and the room to move: the
//! distance to the nearest obstacle's surface, and to a commanded destination
//! so as never to overshoot it. It moves relative to a reference frame
//! (commanded, or the dominant body's), since targets ride along with their
//! planets at tens of km/s. Interlock: it never carries the ship into a body.
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
// (Its constants are the physics sheet's: config/physics.ron.)
pub use crate::sheet::{GROUND_MARGIN, HYPER_RATE, INTERLOCK};
use crate::sheet::{ETA_FIELD, P_FLOOR, P_PUSH, SPEED_OF_LIGHT, STIFF_SLACK, V_BEST_C, V_OPEN_C};

/// The medium's slack `d` from the nearest surface (m): 0 stiff (deep in a
/// system), 1 slack (between the stars). See the physics sheet.
pub fn slack(d: f64) -> f64 {
    ((HYPER_RATE * d.max(0.0)) / (V_OPEN_C * SPEED_OF_LIGHT)).powi(2).min(1.0)
}

/// Distance to the nearest natural body's surface in `sys` (m).
fn nearest_surface(sys: &StarSystem, p: DVec3, positions: &[DVec3]) -> f64 {
    sys.bodies.iter().zip(positions).filter(|(b, _)| !b.kind.artificial()).map(|(b, &c)| c.distance(p) - b.rail.radius).fold(f64::INFINITY, f64::min)
}


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
    } else if slack(nearest_surface(sys, ship.position, positions)) >= STIFF_SLACK {
        // Between the stars the medium is slack: a field can't form there.
        events.push(ShipEvent::FieldWontForm);
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

    // The field's power (see the physics sheet): holding it against the
    // medium's slack, and pushing through, from the plant's spare power and
    // the capacitor banks. Short of holding it, it collapses.
    let s = slack(clearance);
    let m = ship.mass();
    let spare = if ship.fuel > 0.0 { ship.spare_power() } else { 0.0 };
    let bank = if ship.energy > 0.0 { ship.spec().capacitor_rate } else { 0.0 };
    let usable = (spare + bank) * ETA_FIELD;
    let hold = m * s * P_FLOOR;
    if hold > usable {
        events.push(if ship.fuel <= 0.0 { ShipEvent::OutOfFuel } else { ShipEvent::FieldCollapsed });
        let base = sys.velocity(sys.dominant(ship.position, positions), t);
        ship.position += base * (real_dt * warp);
        drop_out(sys, ship, cmd.exit_velocity, t, positions, false, events);
        return;
    }
    // (The fastest the rest buys: P_PUSH·(v/v*)³ per kg, scaled by the slack.)
    let v_best = V_BEST_C * SPEED_OF_LIGHT;
    let power_speed = if s > 0.0 { v_best * ((usable - hold) / (m * s * P_PUSH)).cbrt() } else { f64::INFINITY };
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
    let speed = (HYPER_RATE * room.max(1000.0) * ship.throttle.max(0.02)).min(power_speed);
    // What it draws: the plant first, the banks for the rest; the reactor burns fuel for its part.
    let draw = m * s * (P_FLOOR + P_PUSH * (speed / v_best).powi(3)) / ETA_FIELD;
    let from_plant = draw.min(spare);
    ship.fuel = (ship.fuel - crate::ship::reactor_fuel(from_plant * real_dt)).max(0.0);
    ship.energy = (ship.energy - (draw - from_plant) * real_dt).max(0.0);
    ship.velocity = base + dir * speed;
    let next = carried + dir * speed * real_dt.min(0.1);
    // Interlock: never hyperdrive into a planet, moon or star.
    let inside = sys.bodies.iter().zip(positions).any(|(b, &c)| !b.kind.artificial() && c.distance(next) < b.max_radius() + INTERLOCK);
    if inside {
        ship.position = carried;
        drop_out(sys, ship, cmd.exit_velocity, t, positions, false, events);
        return;
    }
    ship.position = next;
}

#[cfg(test)]
mod tests {
    

    use super::*;
    use crate::ship::{Destination, ShipCommands};
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
    fn between_the_stars_the_field_wont_form() {
        let mut p = Probe::new(42);
        let pos = p.positions();
        // Two light years out from the home star: the medium is slack.
        p.ship.position = pos[0] + DVec3::X * 2.0 * crate::units::LIGHT_YEAR;
        p.ship.velocity = DVec3::ZERO;
        p.toggle_hyperdrive();
        p.step(1.0 / 60.0, 1.0);
        assert!(!p.ship.hyperdrive, "it engaged between the stars");
        assert!(p.events.iter().any(|e| matches!(e, ShipEvent::FieldWontForm)), "{:?}", p.events);
    }

    #[test]
    fn heading_out_into_the_slack_the_field_collapses() {
        let mut p = Probe::new(42);
        let pos = p.positions();
        // At the system's edge (60 AU: still stiff enough to form), heading
        // straight out, full: the room grows, and so does the slack, till all
        // its plant and banks can give at once can't hold the field: it
        // collapses (a stock ship's reach: a couple of hundred AU).
        let out = DVec3::X;
        p.ship.position = pos[0] + out * 60.0 * crate::units::AU;
        p.ship.velocity = DVec3::ZERO;
        p.ship.orientation = glam::DQuat::from_rotation_arc(DVec3::NEG_Z, out);
        p.toggle_hyperdrive();
        p.set_throttle(1.0);
        assert!(p.ship.hyperdrive, "it should engage at 60 AU: {:?}", p.events);
        let mut heard = Vec::new();
        for _ in 0..(60 * 120) {
            p.step(1.0 / 60.0, 1.0);
            heard.append(&mut p.events);
            if !p.ship.hyperdrive {
                break;
            }
        }
        let au = p.ship.position.distance(p.positions()[0]) / crate::units::AU;
        eprintln!("collapsed {au:.0} AU out");
        assert!(!p.ship.hyperdrive && heard.iter().any(|e| matches!(e, ShipEvent::FieldCollapsed)), "{heard:?}");
        assert!(au > 60.0 && au < 1_000.0, "{au} AU");
    }

    #[test]
    fn untargeted_the_drive_drops_out_at_the_top_of_the_air() {
        let mut p = Probe::new(42);
        let sys = p.sys();
        let pos = p.positions();
        // The nearest planet with air (or any planet), dead ahead.
        let body = (1..sys.bodies.len()).filter(|&i| matches!(sys.bodies[i].kind, BodyKind::Rocky)).min_by_key(|&i| (sys.bodies[i].rail.atmosphere.is_none(), pos[i].distance(p.ship.position) as u64)).expect("a planet");
        let b = &sys.bodies[body];
        p.ship.orientation = glam::DQuat::from_rotation_arc(DVec3::NEG_Z, (pos[body] - p.ship.position).normalize());
        p.toggle_hyperdrive();
        p.set_throttle(1.0);
        for _ in 0..(60 * 60) {
            p.step(1.0 / 60.0, 1.0);
            if !p.ship.hyperdrive {
                break;
            }
        }
        assert!(!p.ship.hyperdrive && p.ship.is_flying(), "{:?}", p.events);
        let pos = p.positions();
        let alt = p.ship.position.distance(pos[body]) - b.rail.radius;
        let ground = b.max_radius() - b.rail.radius + GROUND_MARGIN;
        let expect = b.rail.atmosphere.map_or(ground, |a| a.top.max(ground));
        eprintln!("dropped out at {:.0} km (air top {:?} km)", alt / 1000.0, b.rail.atmosphere.map(|a| a.top / 1000.0));
        assert!(alt > 0.8 * expect && alt < 1.2 * expect, "altitude {alt:.0}, expected about {expect:.0}");
    }

    #[test]
    fn a_drive_that_drops_straight_out_leaves_the_throttle_and_speed_alone() {
        let mut p = Probe::new(42);
        let sys = p.sys();
        let planet = sys.bodies[sys.station().unwrap()].rail.parent.unwrap();
        // Nose down at it, engine full.
        let pos = p.positions();
        let up = (p.ship.position - pos[planet]).normalize();
        let b = &sys.bodies[planet];
        // (Half way down to where an untargeted drive drops out: it won't run.)
        let ground = b.max_radius() - b.rail.radius + GROUND_MARGIN;
        let margin = b.rail.atmosphere.map_or(ground, |a| a.top.max(ground));
        p.ship.position = pos[planet] + up * (b.rail.radius + 0.5 * margin);
        p.ship.velocity = sys.velocity(planet, p.world.time) + up * 30.0;
        p.ship.orientation = glam::DQuat::from_rotation_arc(DVec3::NEG_Z, -up);
        p.ship.state = crate::ship::ShipState::Flying;
        p.ship.hyperdrive = false;
        p.set_throttle(1.0);
        let before = p.ship.velocity;
        p.toggle_hyperdrive();
        for _ in 0..30 {
            p.step(1.0 / 60.0, 1.0);
            if !p.ship.hyperdrive {
                break;
            }
        }
        assert!(!p.ship.hyperdrive, "the planet's in the way: it drops out");
        assert_eq!(p.ship.throttle, 1.0, "the engine as it was");
        assert!((p.ship.velocity - before).length() < 1.0, "the speed as it was");
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

    #[test]
    fn speed_follows_room_and_throttle_and_dropping_out_keeps_the_exit_velocity() {
        let mut p = Probe::new(42);
        p.toggle_hyperdrive();
        p.set_throttle(0.5);
        // A destination 50 km ahead, much nearer than any surface: it sets the room.
        let frame_velocity = DVec3::new(1000.0, -2000.0, 500.0);
        let point = p.ship.position + p.ship.forward() * 50_000.0;
        let orders = HyperdriveCommand {
            steering: true,
            frame_velocity: Some(frame_velocity),
            destination: Some(Destination { point, body: 0 }),
            ..HyperdriveCommand::CRUISE
        };
        let mut o = Orders { orders, at: None };
        frame(&mut p, &mut o);
        let expected = p.ship.forward() * (HYPER_RATE * 50_000.0 * 0.5);
        // (The destination is placed ~1e11 m from the star: allow for rounding there.)
        assert!((p.ship.velocity - frame_velocity).distance(expected) < 1e-6 * expected.length(), "relative velocity {:?}", p.ship.velocity - frame_velocity);

        // Dropping out happens when ordered (before the next step flies on).
        let exit = DVec3::new(-30.0, 40.0, 7.0);
        let c = ShipCommands { hyperdrive: Some(HyperdriveCommand { engage: false, exit_velocity: Some(exit), ..Default::default() }), ..p.ship.holding() };
        let system = p.system;
        p.world.command(&mut p.ship, system, &c, &mut p.events);
        assert!(!p.ship.hyperdrive && p.ship.throttle == 0.0);
        assert_eq!(p.ship.velocity, exit);
        assert!(p.events.contains(&ShipEvent::HyperdriveDisengaged));
    }
}
