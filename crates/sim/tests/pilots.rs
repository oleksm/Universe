//! NPC pilots apart from the world (re-architecture R6): on time they fly
//! exactly as in lockstep; however slow they are, the tick doesn't wait; a
//! pilot gone quiet trips the dead-man rule.

use std::time::{Duration, Instant};

use glam::DVec3;
use universe_sim::{Controls, ShipState, Universe};

/// Settlers all leaving their stops at once: departures, corridors, hyperdrive.
fn busy(n: usize) -> Universe {
    let mut u = Universe::new(1984);
    u.spawn_settlers(n, 3);
    // (Their time at the first stop up now.)
    let now = u.world.time;
    for p in u.pilots().iter_mut() {
        p.avionics.route.dwell_until = Some(now);
    }
    u
}

fn tick(u: &mut Universe) {
    u.step_world(1.0 / 60.0, 1.0, &Controls::default());
}

#[test]
fn on_time_pilots_apart_fly_exactly_as_in_lockstep() {
    let run = |apart: bool| {
        let mut u = busy(24);
        if apart {
            u.run_pilots_apart(2);
        }
        for _ in 0..900 {
            tick(&mut u);
            // On time: the pool has thought on this tick's view before the next.
            u.pool.wait_for(u.tick);
        }
        let ships: Vec<(usize, [u64; 3])> = u.crafts.iter().map(|c| (c.system, c.ship.position.to_array().map(f64::to_bits))).collect();
        (ships, u.atc.journal.len(), u.records.stats.stops, u.pool.late, u.pool.dropped)
    };
    let (lockstep, apart) = (run(false), run(true));
    let flying = lockstep.0.len();
    eprintln!("24 settlers, 15 s: journal {} changes, late {} dropped {}", apart.1, apart.3, apart.4);
    assert_eq!(apart.3 + apart.4, 0, "on time: nothing late");
    assert_eq!(lockstep, apart, "the same world, to the bit ({flying} ships)");
    assert!(lockstep.1 > 0, "traffic control was busy");
}

#[test]
fn a_slow_pool_never_slows_the_tick() {
    let mut u = busy(50);
    for _ in 0..10 {
        tick(&mut u);
    }
    let start = Instant::now();
    for _ in 0..120 {
        tick(&mut u);
    }
    let lockstep = start.elapsed() / 120;
    u.run_pilots_apart(2);
    u.pool.slow_down(Duration::from_millis(50));
    let t0 = u.world.time;
    // Paced (a millisecond between ticks, not counted), so the pool's
    // postings come in while the world runs, behind it.
    let mut spent = Duration::ZERO;
    for _ in 0..120 {
        let start = Instant::now();
        tick(&mut u);
        spent += start.elapsed();
        std::thread::sleep(Duration::from_millis(1));
    }
    let apart = spent / 120;
    eprintln!("tick: lockstep {lockstep:?}, apart with a pool 50 ms behind {apart:?}; late {} dropped {}", u.pool.late, u.pool.dropped);
    assert!((u.world.time - t0 - 2.0).abs() < 1e-6, "the world ran its 2 s");
    assert!(apart < lockstep * 2 + Duration::from_millis(1), "the tick didn't wait ({apart:?} vs {lockstep:?})");
    assert!(u.pool.late + u.pool.dropped > 0, "the pool's postings came late");
}

#[test]
fn a_silent_pilot_holds_its_controls_then_the_dead_man_rule_cuts_in() {
    let mut u = Universe::new(1984);
    u.spawn_settlers(1, 1);
    // Flying free, far from anything, engine at 30%, weapons armed.
    let (sys, pos, vel) = (u.ship_system, u.ship.position, u.ship.velocity);
    let c = &mut u.crafts[0];
    c.system = sys;
    c.ship.state = ShipState::Flying;
    c.ship.hyperdrive = false;
    c.ship.position = pos + DVec3::new(0.0, 0.0, 1.0e7);
    c.ship.velocity = vel;
    c.ship.throttle = 0.3;
    c.ship.armed = true;
    {
        let mut pilots = u.pilots();
        pilots[0].silent = true;
        pilots[0].avionics.pirate = false;
        pilots[0].avionics.route.active = false;
    }
    let start = u.world.time;
    while u.world.time - start < universe_sim::pilots::DEAD_MAN - 1.0 {
        tick(&mut u);
    }
    let c = &u.crafts[0];
    assert!(!c.dead_man && c.ship.throttle == 0.3 && c.ship.armed, "silent: the ship holds its controls");
    while u.world.time - start < universe_sim::pilots::DEAD_MAN + 1.0 {
        tick(&mut u);
    }
    let c = &u.crafts[0];
    assert!(c.dead_man, "the dead-man rule cut in");
    assert_eq!(c.ship.throttle, 0.0, "engine cut");
    assert!(!c.ship.armed, "weapons safe");
}


#[test]
fn the_cockpit_flies_the_ship_from_the_client_side() {
    use universe_sim::engine::{Command, EngineHandle};
    let mut u = Universe::new(1984);
    u.ship.position += DVec3::new(0.0, 0.0, 1.0e6);
    let station = u.ship_system().station().unwrap();
    let mut e = EngineHandle::new(u);
    e.start();
    e.send(Command::Throttle { delta: 0.0, set: Some(0.5) });
    e.send(Command::SetNavTarget(Some(universe_sim::NavTarget::Station(station))));
    let start = Instant::now();
    let mut ok = false;
    while start.elapsed() < Duration::from_millis(500) {
        e.poll();
        let v = e.view();
        if v.ship.throttle == 0.5 && v.avionics.nav_target.is_some() && v.nav_marker.is_some() {
            ok = true;
            break;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    assert!(ok, "the throttle and nav target went from the client's cockpit to the ship");
}

#[test]
fn the_cockpit_predicts_what_its_orders_will_do_to_the_tick() {
    let mut u = Universe::new(1984);
    u.ship.angular_velocity = DVec3::ZERO;
    tick(&mut u);
    let start = u.ship.orientation;
    let stick = Controls { pitch: 1.0, yaw: 0.0, roll: 0.0 };
    u.step_world(1.0 / 60.0, 1.0, &stick);
    // The stick is pushed: the ship hasn't turned yet (the order is on its way), the prediction has.
    let (_, turned) = u.cockpit().prediction.expect("a prediction");
    assert!(u.ship.orientation.angle_between(start) < 1e-12, "not turned yet");
    assert!(turned.angle_between(glam::DQuat::IDENTITY) > 1e-4, "the prediction turns");
    let predicted = turned * u.ship.orientation;
    // When the order lands, the ship is where the prediction said.
    u.step_world(1.0 / 60.0, 1.0, &stick);
    u.step_world(1.0 / 60.0, 1.0, &stick);
    let off = u.ship.orientation.angle_between(predicted);
    assert!(off < 1e-9, "predicted to within {off:e} rad");
}
