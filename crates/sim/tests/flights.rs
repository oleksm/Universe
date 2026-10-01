//! Whole flights through the orchestrator, as the game runs them: the
//! avionics (docking, landing and gate autopilots, the hyperdrive
//! autopilot, the route autopilot, the flight planner) flying a ship in the
//! world, tick by tick, through `Universe::step`.

use glam::{DQuat, DVec3};
use universe_sim::avionics::hyperdrive::HYPER_ARRIVE_PORT;
use universe_sim::world::gate::GATE_RADIUS;
use universe_sim::{Approach, Controls, Event, GateFrame, NavTarget, PadFrame, Phase, ShipEvent, ShipState, Stop, Universe};

/// Fly the docking computer from `setup`'s position; returns simulated seconds to dock.
/// A new universe with the player in flight by the home station (4 km
/// behind it on its orbit), not parked on its deck: where these flights start.
fn flying(seed: u64) -> Universe {
    let mut u = Universe::new(seed);
    u.respawn();
    u.step_world(1.0 / 60.0, 1.0, &Controls::default());
    u.events.clear();
    u
}

fn autodock(mut u: Universe) -> f64 {
    u.toggle_autopilot();
    assert!(u.avionics().clearance.is_some_and(|d| d.autopilot), "clearance should be granted: {:?}", u.events);
    let start = u.world.time;
    let mut phase = Phase::Approach;
    for _ in 0..(60 * 60 * 3) {
        u.step_world(1.0 / 60.0, 10.0, &Controls::default());
        if let Some(d) = u.avionics().clearance
            && d.phase != phase
        {
            phase = d.phase;
            eprintln!("t+{:.0}s phase {:?} status {:?}", u.world.time - start, phase, u.docking_status().map(|s| s.1));
        }
        match u.ship.state {
            ShipState::Landed { .. } => return u.world.time - start,
            ShipState::Destroyed { .. } => panic!("crashed: {:?}", u.events),
            ShipState::Flying | ShipState::Transit { .. } | ShipState::Anchored { .. } => {}
        }
    }
    panic!("did not dock within 30 min; status {:?}", u.docking_status());
}

#[test]
fn docking_computer_docks_from_spawn() {
    let secs = autodock(flying(42));
    eprintln!("docked after {secs:.0} s");
}

/// Autopilot to the home planet's spaceport; returns simulated seconds to touchdown.
fn autoland(mut u: Universe, warp: f64) -> f64 {
    let sys = u.ship_system();
    let planet = sys.bodies[sys.station().unwrap()].rail.parent.unwrap();
    let port = sys.spaceports.iter().position(|p| p.body == planet).expect("home planet has a spaceport");
    u.set_nav_target(Some(NavTarget::Spaceport(port)));
    u.toggle_autopilot();
    assert!(u.avionics().clearance.is_some_and(|c| c.autopilot), "landing clearance: {:?}", u.events);
    let start = u.world.time;
    let mut phase = Phase::Approach;
    let mut next_report = 0.0;
    for _ in 0..(60 * 60 * 20) {
        u.step_world(1.0 / 60.0, warp, &Controls::default());
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
            ShipState::Flying | ShipState::Transit { .. } | ShipState::Anchored { .. } => {}
        }
    }
    panic!("did not land; approach {:?}", u.approach());
}

#[test]
fn autopilot_lands_from_orbit() {
    let secs = autoland(flying(42), 20.0);
    eprintln!("landed after {secs:.0} s");
}

/// A spaceport on a different planet of the home system than the one we start at.
fn far_port(u: &mut Universe) -> usize {
    let sys = u.ship_system();
    let home_planet = sys.bodies[sys.station().unwrap()].rail.parent.unwrap();
    sys.spaceports.iter().position(|p| p.body != home_planet && sys.bodies[p.body].rail.parent == Some(0)).expect("another planet with a port")
}

/// Run hyperdrive until it drops out; returns the distance to the port's pad.
/// With `pilot_aims`, the nose is re-pointed at the pad every frame, like a
/// pilot keeping the target marker on the crosshair.
fn hyper_until_arrival(u: &mut Universe, port: usize, pilot_aims: bool) -> f64 {
    // (The drive engages when the order reaches it, a couple of ticks on.)
    let mut engaged = false;
    for _ in 0..(60 * 600) {
        if pilot_aims && u.ship.hyperdrive {
            let at = u.target_position(NavTarget::Spaceport(port)).unwrap();
            u.ship.orientation = DQuat::from_rotation_arc(DVec3::NEG_Z, (at - u.ship.position).normalize());
        }
        u.step_world(1.0 / 60.0, 1.0, &Controls::default());
        engaged |= u.ship.hyperdrive;
        if engaged && !u.ship.hyperdrive {
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
fn hyperdrive_autopilot_steers_to_the_port() {
    let mut u = flying(42);
    let port = far_port(&mut u);
    u.set_nav_target(Some(NavTarget::Spaceport(port)));
    u.toggle_hyperdrive();
    u.toggle_autopilot();
    assert!(u.avionics().hyper_autopilot);
    // The autopilot opens the throttle itself (engaging zeroes it).
    for _ in 0..30 {
        u.step_world(1.0 / 60.0, 1.0, &Controls::default());
    }
    assert!(u.ship.hyperdrive && u.ship.throttle == 1.0, "throttle {}", u.ship.throttle);
    let dist = hyper_until_arrival(&mut u, port, false);
    eprintln!("autopilot: dropped out {:.0} km from the pad", dist / 1000.0);
    assert!(dist < HYPER_ARRIVE_PORT);
}

#[test]
fn plan_reaches_the_pads() {
    // Landing from orbit.
    let mut u = flying(42);
    let sys = u.ship_system();
    let planet = sys.bodies[sys.station().unwrap()].rail.parent.unwrap();
    let port = sys.spaceports.iter().position(|p| p.body == planet).unwrap();
    u.set_nav_target(Some(NavTarget::Spaceport(port)));
    assert!(u.request_clearance());
    // (Traffic control assigns the pad as the request comes in, a few ticks on.)
    for _ in 0..5 {
        u.step_world(1.0 / 60.0, 1.0, &Controls::default());
    }
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
    let mut u = flying(42);
    u.set_nav_target(None);
    assert!(u.request_clearance());
    for _ in 0..5 {
        u.step_world(1.0 / 60.0, 1.0, &Controls::default());
    }
    let plan = u.plan().unwrap();
    eprintln!("docking plan: {} points, {:.0} s, arrives {}", plan.points.len(), plan.points.last().unwrap().time, plan.arrives);
    assert!(plan.arrives, "docking plan should reach the pad");
}

#[test]
fn autopilot_flies_through_a_gate_and_keeps_its_motion() {
    let mut u = flying(1984);
    let home = u.world.home_system;
    let sys = u.ship_system();
    let (dest, _) = u.gate_links_of(home)[0].clone();
    let g = sys.gate_to(dest).unwrap();
    // Start 30 km from the gate, on its far side (the autopilot goes back
    // through the empty ring to its entry side), co-moving with it.
    let mut pos = Vec::new();
    sys.positions(u.world.time, &mut pos);
    let frame = GateFrame::new(&sys, g, u.world.time, &pos);
    u.ship.position = frame.center + (frame.rotation * DVec3::new(1.0, 0.5, 0.3)).normalize() * 30_000.0;
    u.ship.velocity = frame.velocity;
    u.set_nav_target(Some(NavTarget::Gate(g)));
    u.toggle_autopilot();
    assert!(u.avionics().clearance.is_some_and(|c| c.autopilot), "{:?}", u.events);
    let plan_eta = u.plan().unwrap();
    assert!(plan_eta.arrives, "the plan should reach the gate");
    let mut entered = None;
    for _ in 0..(60 * 60 * 20) {
        u.step_world(1.0 / 60.0, 10.0, &Controls::default());
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
    // The twins face each other: the same way on through space, so relative
    // to the twin it's turned half round (about the ring's X).
    let turned = DQuat::from_rotation_x(std::f64::consts::PI) * entry_local;
    assert!((exit_local - turned).length() < 1.0, "{exit_local} vs {turned}");
    assert!(u.ship.position.distance(out.center) < GATE_RADIUS, "came out of the ring");
    assert!(u.events.iter().any(|e| matches!(e, Event::Ship(ShipEvent::GateArrived { .. }))));
}

/// Fly a route headless; returns (stops reached, completed).
fn fly_route(u: &mut Universe, warp: f64, max_frames: usize) -> (usize, bool) {
    u.toggle_route();
    let (mut reached, mut done) = (0, false);
    for frame in 0..max_frames {
        u.step_world(1.0 / 60.0, warp, &Controls::default());
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
    u.avionics_mut().route.stops = vec![
        Stop { system: home, target: NavTarget::Spaceport(port) },
        Stop { system: home, target: NavTarget::Station(station) },
        far,
    ];
    let (reached, done) = fly_route(&mut u, 10.0, 60 * 60 * 90);
    assert_eq!(reached, 3, "all stops reached");
    assert!(done);
    assert_eq!(u.ship_system, next);
}


#[test]
fn hyperdrive_autopilot_reaches_an_asteroid_field_and_drops_out_moving_with_it() {
    let mut u = flying(42);
    let sys = u.ship_system();
    let field = &sys.fields[0];
    let rock = field.body;
    u.set_nav_target(Some(NavTarget::Asteroid(rock)));
    u.toggle_hyperdrive();
    u.toggle_autopilot();
    u.throttle(0.0, Some(1.0));
    let mut engaged = false;
    let mut ticks = 0;
    while ticks < 60 * 600 && !(engaged && !u.ship.hyperdrive) {
        u.step_world(1.0 / 60.0, 1.0, &Controls::default());
        engaged |= u.ship.hyperdrive;
        ticks += 1;
    }
    let mut pos = Vec::new();
    sys.positions(u.world.time, &mut pos);
    assert!(engaged && !u.ship.hyperdrive && u.ship.is_flying(), "{:?}", u.events);
    let d = u.ship.position.distance(pos[rock]);
    let v = (u.ship.velocity - sys.velocity(rock, u.world.time)).length();
    eprintln!("{} s: dropped out {:.0} km from {} (swarm reaches {:.0} km), {v:.1} m/s off its motion", ticks / 60, d / 1000.0, field.name, field.extent / 1000.0);
    assert!(d > field.extent && d < field.extent + 25_000.0, "outside the swarm, close by");
    assert!(v < 1.0, "moving with it");
}


/// A universe whose player flies hull `key`, its tank full.
fn in_hull(key: &str) -> Universe {
    let mut u = flying(42);
    u.ship.class = universe_sim::world::content::content().handle(key).unwrap();
    u.ship.fuel = u.ship.spec().fuel_capacity;
    u
}

#[test]
fn the_heaviest_and_the_nimblest_hull_land_and_dock_on_the_autopilots() {
    // (The extremes: the others lie between.)
    for key in ["hull.hauler", "hull.interceptor"] {
        let land = autoland(in_hull(key), 20.0);
        let dock = autodock(in_hull(key));
        eprintln!("{key}: landed after {land:.0} s, docked after {dock:.0} s");
    }
}
