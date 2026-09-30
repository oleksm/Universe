//! Whole flights through the orchestrator, as the game runs them: the
//! avionics (docking, landing and gate autopilots, the hyperdrive
//! autopilot, the route autopilot, the flight planner) flying a ship in the
//! world, tick by tick, through `Universe::step`.

use glam::{DQuat, DVec3};
use universe_sim::avionics::hyperdrive::HYPER_ARRIVE_PORT;
use universe_sim::world::gate::GATE_RADIUS;
use universe_sim::{Approach, BodyKind, Controls, Event, GateFrame, NavTarget, PadFrame, Phase, Plan, ShipEvent, ShipState, StationFrame, Stop, Universe};

/// Fly the docking computer from `setup`'s position; returns simulated seconds to dock.
fn autodock(mut u: Universe) -> f64 {
    u.toggle_autopilot();
    assert!(u.avionics.clearance.is_some_and(|d| d.autopilot), "clearance should be granted: {:?}", u.events);
    let start = u.world.time;
    let mut phase = Phase::Approach;
    for _ in 0..(60 * 60 * 3) {
        u.step(1.0 / 60.0, 10.0, &Controls::default());
        if let Some(d) = u.avionics.clearance
            && d.phase != phase
        {
            phase = d.phase;
            eprintln!("t+{:.0}s phase {:?} status {:?}", u.world.time - start, phase, u.docking_status().map(|s| s.1));
        }
        match u.ship.state {
            ShipState::Landed { .. } => return u.world.time - start,
            ShipState::Destroyed { .. } => panic!("crashed: {:?}", u.events),
            ShipState::Flying | ShipState::Transit { .. } => {}
        }
    }
    panic!("did not dock within 30 min; status {:?}", u.docking_status());
}

#[test]
fn docking_computer_docks_from_spawn() {
    let secs = autodock(Universe::new(42));
    eprintln!("docked after {secs:.0} s");
}

#[test]
fn docking_computer_goes_around_from_behind() {
    // Start on the far side of the station from the slot.
    let mut u = Universe::new(42);
    let sys = u.ship_system();
    let station = sys.station().unwrap();
    let mut pos = Vec::new();
    sys.positions(u.world.time, &mut pos);
    let frame = StationFrame::new(&sys, station, u.world.time, &pos);
    u.ship.position = frame.center - frame.axis() * 3000.0 + frame.slot_long() * 200.0;
    u.ship.velocity = frame.velocity;
    let secs = autodock(u);
    eprintln!("docked from behind after {secs:.0} s");
}

/// Autopilot to the home planet's spaceport; returns simulated seconds to touchdown.
fn autoland(mut u: Universe, warp: f64) -> f64 {
    let sys = u.ship_system();
    let planet = sys.bodies[sys.station().unwrap()].rail.parent.unwrap();
    let port = sys.spaceports.iter().position(|p| p.body == planet).expect("home planet has a spaceport");
    u.set_nav_target(Some(NavTarget::Spaceport(port)));
    u.toggle_autopilot();
    assert!(u.avionics.clearance.is_some_and(|c| c.autopilot), "landing clearance: {:?}", u.events);
    let start = u.world.time;
    let mut phase = Phase::Approach;
    let mut next_report = 0.0;
    for _ in 0..(60 * 60 * 20) {
        u.step(1.0 / 60.0, warp, &Controls::default());
        let hd_events: Vec<Event> = u.events.iter().filter(|e| matches!(e, Event::Ship(ShipEvent::HyperdriveEngaged) | Event::Ship(ShipEvent::HyperdriveDisengaged) | Event::HyperdriveArrived { .. })).cloned().collect();
        for e in hd_events {
            let sys = u.ship_system();
            let mut pos = Vec::new();
            sys.positions(u.world.time, &mut pos);
            let dom = sys.dominant(u.ship.position, &pos);
            let alt = u.ship.position.distance(pos[dom]) - sys.bodies[dom].rail.radius;
            let pad = PadFrame::new(&sys, port, u.world.time, &pos);
            eprintln!("  t+{:>5.0}s {e:?}: near {} alt {:.0} km, pad {:.0} km", u.world.time - start, sys.bodies[dom].name, alt / 1000.0, pad.pad.distance(u.ship.position) / 1000.0);
        }
        u.events.retain(|e| !matches!(e, Event::Ship(ShipEvent::HyperdriveEngaged) | Event::Ship(ShipEvent::HyperdriveDisengaged) | Event::HyperdriveArrived { .. }));
        if let Some(Approach::Land { status, .. }) = u.approach()
            && (status.phase != phase || u.world.time - start > next_report)
        {
            phase = status.phase;
            next_report = u.world.time - start + 120.0;
            let surface_alt = (u.ship.position - status.pad.body_center).length() - status.pad.body_radius;
            eprintln!(
                "t+{:>5.0}s {:?} range {:>10.0} surf-alt {:>8.0} want {:>6.0} m/s speed {:>6.0} vspd {:>7.1} hspd {:>7.1} tilt {:>4.0}deg thr {:.2}",
                u.world.time - start,
                status.phase,
                status.range,
                surface_alt,
                status.guidance.desired_velocity.length(),
                status.relative_velocity.length(),
                status.vertical_speed,
                status.horizontal_speed,
                status.tilt.to_degrees(),
                u.ship.throttle
            );
        }
        match u.ship.state {
            ShipState::Landed { .. } => {
                assert!(
                    u.events.iter().any(|e| matches!(e, Event::Ship(ShipEvent::LandedAtPort { .. }))),
                    "landed off the pad: {:?}",
                    u.events
                );
                return u.world.time - start;
            }
            ShipState::Destroyed { .. } => panic!("crashed: {:?}", u.events),
            ShipState::Flying | ShipState::Transit { .. } => {}
        }
    }
    panic!("did not land; approach {:?}", u.approach());
}

#[test]
fn autopilot_lands_from_orbit() {
    let secs = autoland(Universe::new(42), 20.0);
    eprintln!("landed after {secs:.0} s");
}

#[test]
fn autopilot_lands_in_the_game_universe() {
    let secs = autoland(Universe::new(1984), 20.0);
    eprintln!("game universe: landed after {secs:.0} s");
}

#[test]
fn autopilot_lands_from_far_side_of_planet() {
    let mut u = Universe::new(42);
    let sys = u.ship_system();
    let planet = sys.bodies[sys.station().unwrap()].rail.parent.unwrap();
    let port = sys.spaceports.iter().position(|p| p.body == planet).unwrap();
    let mut pos = Vec::new();
    sys.positions(u.world.time, &mut pos);
    let pad = PadFrame::new(&sys, port, u.world.time, &pos);
    // Hover-still (in the rotating frame) 1000 km up, over the antipode.
    u.ship.position = pad.body_center - pad.up * (pad.body_radius + 1.0e6);
    u.ship.velocity = pad.frame_velocity(u.ship.position);
    let secs = autoland(u, 50.0);
    eprintln!("landed from the far side after {secs:.0} s");
}

#[test]
fn autopilot_lands_from_high_above() {
    let mut u = Universe::new(42);
    let sys = u.ship_system();
    let planet = sys.bodies[sys.station().unwrap()].rail.parent.unwrap();
    let port = sys.spaceports.iter().position(|p| p.body == planet).unwrap();
    let mut pos = Vec::new();
    sys.positions(u.world.time, &mut pos);
    let pad = PadFrame::new(&sys, port, u.world.time, &pos);
    u.ship.position = pad.pad + pad.up * 50_000.0;
    u.ship.velocity = pad.frame_velocity(u.ship.position);
    autoland(u, 10.0);
}

/// A spaceport on a different planet of the home system than the one we start at.
fn far_port(u: &mut Universe) -> usize {
    let sys = u.ship_system();
    let home_planet = sys.bodies[sys.station().unwrap()].rail.parent.unwrap();
    sys.spaceports.iter().position(|p| p.body != home_planet && sys.bodies[p.body].rail.parent == Some(0)).expect("another planet with a port")
}

/// A port on another planet whose pad faces the ship (a straight line to it is clear).
fn facing_port(u: &mut Universe) -> usize {
    let sys = u.ship_system();
    let home_planet = sys.bodies[sys.station().unwrap()].rail.parent.unwrap();
    let mut pos = Vec::new();
    sys.positions(u.world.time, &mut pos);
    (0..sys.spaceports.len())
        .filter(|&i| sys.spaceports[i].body != home_planet)
        .max_by(|&a, &b| {
            let facing = |i: usize| {
                let pad = PadFrame::new(&sys, i, u.world.time, &pos);
                pad.up.dot((u.ship.position - pad.body_center).normalize())
            };
            facing(a).total_cmp(&facing(b))
        })
        .unwrap()
}

/// Run hyperdrive until it drops out; returns the distance to the port's pad.
/// With `pilot_aims`, the nose is re-pointed at the pad every frame, like a
/// pilot keeping the target marker on the crosshair.
fn hyper_until_arrival(u: &mut Universe, port: usize, pilot_aims: bool) -> f64 {
    for _ in 0..(60 * 600) {
        if pilot_aims && u.ship.hyperdrive {
            let at = u.target_position(NavTarget::Spaceport(port)).unwrap();
            u.ship.orientation = DQuat::from_rotation_arc(DVec3::NEG_Z, (at - u.ship.position).normalize());
        }
        u.step(1.0 / 60.0, 1.0, &Controls::default());
        if !u.ship.hyperdrive {
            break;
        }
    }
    assert!(!u.ship.hyperdrive, "hyperdrive should have dropped out");
    assert!(u.ship.is_flying(), "{:?}", u.events);
    let sys = u.ship_system();
    let mut pos = Vec::new();
    sys.positions(u.world.time, &mut pos);
    PadFrame::new(&sys, port, u.world.time, &pos).pad.distance(u.ship.position)
}

#[test]
fn manual_hyperdrive_at_a_port_drops_out_there_with_clearance() {
    let mut u = Universe::new(42);
    let port = facing_port(&mut u);
    u.set_nav_target(Some(NavTarget::Spaceport(port)));
    // Point the nose at the pad, as a pilot would at the target marker.
    let at = u.target_position(NavTarget::Spaceport(port)).unwrap();
    u.ship.orientation = DQuat::from_rotation_arc(DVec3::NEG_Z, (at - u.ship.position).normalize());
    u.toggle_hyperdrive();
    u.ship.throttle = 1.0;
    let dist = hyper_until_arrival(&mut u, port, true);
    eprintln!("manual: dropped out {:.0} km from the pad; events {:?}", dist / 1000.0, u.events);
    assert!(dist < HYPER_ARRIVE_PORT, "dropped out too far: {dist}");
    assert!(u.events.iter().any(|e| matches!(e, Event::HyperdriveArrived { .. })));
    assert!(u.avionics.clearance.is_none(), "clearance is the pilot's call, not automatic");
}

#[test]
fn hyperdrive_autopilot_steers_to_the_port() {
    let mut u = Universe::new(42);
    let port = far_port(&mut u);
    u.set_nav_target(Some(NavTarget::Spaceport(port)));
    u.toggle_hyperdrive();
    u.toggle_autopilot();
    assert!(u.avionics.hyper_autopilot);
    u.ship.throttle = 1.0;
    let dist = hyper_until_arrival(&mut u, port, false);
    eprintln!("autopilot: dropped out {:.0} km from the pad", dist / 1000.0);
    assert!(dist < HYPER_ARRIVE_PORT);
}

#[test]
fn plan_reaches_the_pad_and_the_slot() {
    // Landing from orbit.
    let mut u = Universe::new(42);
    let sys = u.ship_system();
    let planet = sys.bodies[sys.station().unwrap()].rail.parent.unwrap();
    let port = sys.spaceports.iter().position(|p| p.body == planet).unwrap();
    u.set_nav_target(Some(NavTarget::Spaceport(port)));
    assert!(u.request_clearance());
    let start = std::time::Instant::now();
    let plan = u.plan().unwrap();
    eprintln!(
        "landing plan: {} points, {:.0} s, arrives {}, first action {:?}, built in {:?}",
        plan.points.len(),
        plan.points.last().unwrap().time,
        plan.arrives,
        plan.points[0].action,
        start.elapsed()
    );
    assert!(plan.arrives, "landing plan should reach the pad");

    // Docking from the spawn point.
    let mut u = Universe::new(42);
    u.set_nav_target(None);
    assert!(u.request_clearance());
    let plan = u.plan().unwrap();
    eprintln!("docking plan: {} points, {:.0} s, arrives {}", plan.points.len(), plan.points.last().unwrap().time, plan.arrives);
    assert!(plan.arrives, "docking plan should reach the slot");
}

/// Position and roll-free "up" of a plan at absolute time `at`.
fn plan_at(plan: &Plan, start: f64, at: f64) -> (DVec3, DVec3) {
    let rel = at - start;
    let i = plan.points.iter().position(|p| p.time >= rel).unwrap_or(plan.points.len() - 1).max(1);
    let (a, b) = (plan.points[i - 1], plan.points[i]);
    let u = ((rel - a.time) / (b.time - a.time).max(1e-9)).clamp(0.0, 1.0);
    (a.position.lerp(b.position, u), a.orientation.slerp(b.orientation, u) * DVec3::Y)
}

#[test]
fn plan_stays_put_while_the_autopilot_follows_it() {
    for label in ["landing", "docking"] {
        let mut u = Universe::new(1984);
        let sys = u.ship_system();
        let station = sys.station().unwrap();
        let planet = sys.bodies[station].rail.parent.unwrap();
        // Plans are drawn relative to the target as it is when planned:
        // this is that reference body, and its spin (zero for stations).
        let (body, omega) = if label == "landing" {
            let port = sys.spaceports.iter().position(|p| p.body == planet).unwrap();
            u.set_nav_target(Some(NavTarget::Spaceport(port)));
            // Within hyperjump range, so the landing autopilot flies it all.
            let mut pos = Vec::new();
            sys.positions(u.world.time, &mut pos);
            let pad = PadFrame::new(&sys, port, u.world.time, &pos);
            u.ship.position = pad.pad + (pad.up + pad.up.any_orthonormal_vector() * 0.5).normalize() * 150_000.0;
            u.ship.velocity = pad.frame_velocity(u.ship.position);
            (planet, sys.bodies[planet].angular_velocity())
        } else {
            (station, DVec3::ZERO)
        };
        u.toggle_autopilot();
        let mut pos = Vec::new();
        let t0 = u.world.time;
        sys.positions(t0, &mut pos);
        let c0 = pos[body];
        let first = u.plan().unwrap();
        for _ in 0..600 {
            u.step(1.0 / 60.0, 1.0, &Controls::default());
        }
        let t1 = u.world.time;
        sys.positions(t1, &mut pos);
        let c1 = pos[body];
        let second = u.plan().unwrap();
        let end = t0 + first.points.last().unwrap().time.min(second.points.last().unwrap().time + (t1 - t0));
        // Re-express the first plan in the second plan's reference: move with
        // the body, and turn with it for the extra time.
        let spin = DQuat::from_scaled_axis(omega * (t1 - t0));
        let mut worst_pos: f64 = 0.0;
        let mut worst_roll: f64 = 0.0;
        let mut rolled = 0;
        for k in 1..=10 {
            let at = t1 + (end - t1) * k as f64 / 10.0;
            let (p0, up0) = plan_at(&first, t0, at);
            let (p1, up1) = plan_at(&second, t1, at);
            let p0 = c1 + spin * (p0 - c0);
            let up0 = spin * up0;
            let scale = p1.distance(c1 + spin * (u.ship.position - c1)).max(100.0).min(p1.distance(u.ship.position).max(100.0));
            worst_pos = worst_pos.max(p0.distance(p1) / scale);
            let roll = up0.angle_between(up1).to_degrees();
            worst_roll = worst_roll.max(roll);
            if roll > 10.0 {
                rolled += 1;
            }
        }
        eprintln!("{label}: plans 10 s apart differ by at most {:.1}% of distance, {:.1} deg of roll", worst_pos * 100.0, worst_roll);
        assert!(worst_pos < 0.05, "{label}: plan moved {:.1}%", worst_pos * 100.0);
        // A planned turn (e.g. from routing around the planet to heading
        // straight in) can land a few seconds apart in two plans; allow
        // one such sample, but frames must otherwise hold their attitude.
        assert!(rolled <= 1, "{label}: frames rolled at {rolled} of 10 samples (worst {worst_roll:.1} deg)");
    }
}

#[test]
fn autopilot_flies_through_a_gate_and_keeps_its_motion() {
    let mut u = Universe::new(1984);
    let home = u.world.home_system;
    let sys = u.ship_system();
    let (dest, _) = u.gate_links_of(home)[0].clone();
    let g = sys.gate_to(dest).unwrap();
    // Start 30 km from the gate, co-moving with it.
    let mut pos = Vec::new();
    sys.positions(u.world.time, &mut pos);
    let frame = GateFrame::new(&sys, g, u.world.time, &pos);
    u.ship.position = frame.center + (frame.rotation * DVec3::new(1.0, 0.5, 0.3)).normalize() * 30_000.0;
    u.ship.velocity = frame.velocity;
    u.set_nav_target(Some(NavTarget::Gate(g)));
    u.toggle_autopilot();
    assert!(u.avionics.clearance.is_some_and(|c| c.autopilot), "{:?}", u.events);
    let plan_eta = u.plan().unwrap();
    assert!(plan_eta.arrives, "the plan should reach the gate");
    let mut entered = None;
    for _ in 0..(60 * 60 * 20) {
        u.step(1.0 / 60.0, 10.0, &Controls::default());
        if let ShipState::Transit { local_velocity, .. } = u.ship.state
            && entered.is_none()
        {
            entered = Some(local_velocity);
        }
        if entered.is_some() && u.ship.is_flying() {
            break;
        }
        assert!(!matches!(u.ship.state, ShipState::Destroyed { .. }), "crashed: {:?}", u.events);
    }
    let entry_local = entered.expect("should have entered the gate");
    assert_eq!(u.ship_system, dest, "arrived in the linked system");
    // Same motion relative to the arrival gate.
    let sys = u.ship_system();
    let back = sys.gate_to(home).unwrap();
    sys.positions(u.world.time, &mut pos);
    let out = GateFrame::new(&sys, back, u.world.time, &pos);
    let exit_local = out.rotation.inverse() * (u.ship.velocity - out.velocity);
    eprintln!("entry {entry_local:.1?} exit {exit_local:.1?}; plan eta {:.0} s", plan_eta.points.last().unwrap().time);
    assert!((exit_local - entry_local).length() < 1.0);
    assert!(u.ship.position.distance(out.center) < GATE_RADIUS, "came out of the ring");
    assert!(u.events.iter().any(|e| matches!(e, Event::Ship(ShipEvent::GateArrived { .. }))));
}

#[test]
fn hyperdrive_catches_a_moving_gate_at_part_throttle_and_stops_with_it() {
    let mut u = Universe::new(1984);
    let home = u.world.home_system;
    let sys = u.ship_system();
    let (dest, _) = u.gate_links_of(home)[0].clone();
    let g = sys.gate_to(dest).unwrap();
    u.set_nav_target(Some(NavTarget::Gate(g)));
    u.toggle_hyperdrive();
    u.ship.throttle = 0.3;
    for _ in 0..(60 * 600) {
        let at = u.target_position(NavTarget::Gate(g)).unwrap();
        u.ship.orientation = DQuat::from_rotation_arc(DVec3::NEG_Z, (at - u.ship.position).normalize());
        u.step(1.0 / 60.0, 1.0, &Controls::default());
        if !u.ship.hyperdrive {
            break;
        }
    }
    assert!(!u.ship.hyperdrive, "should arrive at the gate at 30% throttle");
    let mut pos = Vec::new();
    sys.positions(u.world.time, &mut pos);
    let rel = u.ship.velocity - sys.velocity(g, u.world.time);
    let dist = pos[g].distance(u.ship.position);
    eprintln!("arrived {:.1} km from the gate, drifting {:.2} m/s relative", dist / 1000.0, rel.length());
    assert!(dist < 20_000.0);
    assert!(rel.length() < 1.0, "should be at rest relative to the gate");

    // Manual drop-out near the target also matches it.
    u.toggle_hyperdrive();
    u.ship.throttle = 0.1;
    for _ in 0..30 {
        u.step(1.0 / 60.0, 1.0, &Controls::default());
    }
    u.toggle_hyperdrive();
    let rel = u.ship.velocity - sys.velocity(g, u.world.time);
    assert!(rel.length() < 1.0, "manual drop-out near the gate matches it: {:.1} m/s", rel.length());
}

/// Fly a route headless; returns (stops reached, completed).
fn fly_route(u: &mut Universe, warp: f64, max_frames: usize) -> (usize, bool) {
    u.toggle_route();
    let (mut reached, mut done) = (0, false);
    for frame in 0..max_frames {
        u.step(1.0 / 60.0, warp, &Controls::default());
        for e in std::mem::take(&mut u.events) {
            match &e {
                Event::RouteStop { .. } => reached += 1,
                Event::RouteComplete => done = true,
                Event::Ship(ShipEvent::Crashed { .. }) | Event::RouteBlocked { .. } => panic!("route failed at frame {frame}: {e:?}"),
                _ => {}
            }
            if matches!(e, Event::RouteStop { .. } | Event::RouteComplete | Event::Ship(ShipEvent::GateEntered { .. }) | Event::Ship(ShipEvent::GateArrived { .. }) | Event::Ship(ShipEvent::Landed { .. }) | Event::Ship(ShipEvent::LandedAtPort { .. }))
            {
                eprintln!("  t+{:>7.0} s (frame {frame:>6}): {e:?}", u.world.time);
            }
        }
        if done {
            break;
        }
    }
    (reached, done)
}

/// A whole 10-stop settler route, headless.
/// `cargo test -p universe-sim --release settler_route_flies -- --ignored --nocapture`
#[test]
#[ignore]
fn settler_route_flies_to_completion() {
    let mut u = Universe::new(1984);
    u.avionics.route.stops = u.settler_route(7, 10);
    let names: Vec<String> = u.avionics.route.stops.clone().into_iter().map(|s| u.stop_name(s)).collect();
    eprintln!("route: {names:#?}");
    let t0 = u.world.time;
    let (reached, done) = fly_route(&mut u, 20.0, 60 * 60 * 60 * 3);
    eprintln!("reached {reached} stops, complete {done}, {:.1} game hours", (u.world.time - t0) / 3600.0);
    assert!(done);
}

#[test]
fn route_autopilot_lands_docks_and_crosses_a_gate() {
    let mut u = Universe::new(1984);
    let home = u.world.home_system;
    let sys = u.ship_system();
    let station = sys.station().unwrap();
    let planet = sys.bodies[station].rail.parent.unwrap();
    let port = sys.spaceports.iter().position(|p| p.body == planet).unwrap();
    let (next, _) = u.gate_links_of(home)[0].clone();
    let there = u.system(next);
    let far = there
        .station()
        .map(|s| Stop { system: next, target: NavTarget::Station(s) })
        .unwrap_or(Stop { system: next, target: NavTarget::Spaceport(0) });
    u.avionics.route.stops = vec![
        Stop { system: home, target: NavTarget::Spaceport(port) },
        Stop { system: home, target: NavTarget::Station(station) },
        far,
    ];
    let (reached, done) = fly_route(&mut u, 10.0, 60 * 60 * 90);
    assert_eq!(reached, 3, "all stops reached");
    assert!(done);
    assert_eq!(u.ship_system, next);
}

/// Autoland at every moon port in the gate network, from 150 km out.
#[test]
#[ignore]
fn autoland_on_every_moon_port() {
    let base = Universe::new(1984);
    let mut systems: Vec<usize> = base.world.gate_links.iter().flat_map(|&(a, b)| [a, b]).collect();
    systems.sort();
    systems.dedup();
    for s in systems {
        let mut probe = Universe::new(1984);
        let sys = probe.system(s);
        for (p, sp) in sys.spaceports.iter().enumerate() {
            if sys.bodies[sp.body].kind != BodyKind::Moon {
                continue;
            }
            let mut u = Universe::new(1984);
            u.ship_system = s;
            let sys = u.ship_system();
            let mut pos = Vec::new();
            sys.positions(u.world.time, &mut pos);
            let pad = PadFrame::new(&sys, p, u.world.time, &pos);
            let side = pad.up.any_orthonormal_vector();
            u.ship.position = pad.pad + (pad.up * 0.6 + side * 0.8).normalize() * 150_000.0;
            u.ship.velocity = pad.frame_velocity(u.ship.position);
            u.ship.state = ShipState::Flying;
            u.set_nav_target(Some(NavTarget::Spaceport(p)));
            u.toggle_autopilot();
            let t0 = u.world.time;
            let mut outcome = "timeout".to_string();
            for _ in 0..(60 * 60 * 20) {
                u.step(1.0 / 60.0, 10.0, &Controls::default());
                if let Some(e) = u.events.iter().find(|e| matches!(e, Event::Ship(ShipEvent::Crashed { .. }) | Event::Ship(ShipEvent::LandedAtPort { .. }) | Event::Ship(ShipEvent::Landed { .. }))).cloned() {
                    let st = u.approach();
                    outcome = format!("{e:?} after {:.0} s; last status {:?}", u.world.time - t0, st.map(|a| match a {
                        Approach::Land { status, .. } => (status.phase, status.altitude as i64, status.vertical_speed as i64, status.horizontal_speed as i64),
                        _ => (Phase::Approach, 0, 0, 0),
                    }));
                    break;
                }
                u.events.clear();
            }
            let b = &sys.bodies[sp.body];
            eprintln!("{} on {} (R {:.0} km, g {:.2}): {outcome}", sp.name, b.name, b.rail.radius / 1000.0, b.rail.mu / (b.rail.radius * b.rail.radius));
        }
    }
}
