//! Pilots and the cockpit: a pilot gone quiet trips the dead-man rule, the
//! cockpit flies the ship from the client side, a recorded session replays.

use std::time::{Duration, Instant};

use glam::DVec3;
use universe_sim::{Controls, ShipState, Universe};

fn tick(u: &mut Universe) {
    u.step_world(1.0 / 60.0, 1.0, &Controls::default());
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
    // (In flight, not parked on the home station's deck; the respawn settled.)
    u.respawn();
    tick(&mut u);
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
fn a_recorded_session_replays_to_the_same_world() {
    // A busy session, recorded from the start: settlers leaving, the player's cockpit at work.
    let mut u = Universe::new(1984);
    u.record_inputs();
    u.spawn_settlers(24, 3);
    let now = u.world.time;
    for p in u.pilots().iter_mut() {
        p.avionics.route.dwell_until = Some(now);
    }
    let station = u.ship_system().station().unwrap();
    u.set_nav_target(Some(universe_sim::NavTarget::Station(station)));
    u.toggle_autopilot();
    for _ in 0..600 {
        tick(&mut u);
    }
    let log = u.input_log.clone().unwrap();
    let replayed = Universe::replay(&log);
    eprintln!("{} ticks, {} postings; hash {:x}", log.ticks.len(), log.ticks.iter().map(|t| t.due.len()).sum::<usize>(), u.state_hash());
    assert!(!u.atc.journal.is_empty(), "traffic control was busy");
    assert_eq!(replayed.state_hash(), u.state_hash(), "the replay is the same world");
    // And through a save file.
    let json = serde_json::to_string(&u.world_save().unwrap()).unwrap();
    let loaded = Universe::world_load(&serde_json::from_str(&json).unwrap());
    assert_eq!(loaded.state_hash(), u.state_hash(), "loaded, the same world");
    assert_eq!(loaded.pilots().len(), 24);
}
