//! Whole flights through the orchestrator, as the game runs them: the
//! avionics (docking, landing and gate autopilots, the hyperdrive
//! autopilot, the route autopilot, the flight planner) flying a ship in the
//! world, tick by tick, through `Universe::step`.

use universe_sim::{Controls, NavTarget, Phase, ShipState, Universe};

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

