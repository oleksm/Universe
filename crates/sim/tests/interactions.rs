//! Interaction tests: a few ships placed in one situation, run for a few
//! minutes of game time (seconds of real time), checked for the outcome. On
//! failure they print the flight recorder's incidents, so the cause is there
//! to read. These replace whole-traffic runs for developing traffic behaviour.

use glam::{DQuat, DVec3};
use universe_sim::avionics::nav::PadSlot;
use universe_sim::world::spaceport::{pad_at, PADS};
use universe_sim::world::{Facility, GateFrame, StationFrame};
use universe_sim::{BodyKind, Controls, NavTarget, PadFrame, Phase, ShipState, Universe};

/// A world with `n` plain settlers (no pirates, traders or routes), all in
/// the home system, and our own ship parked out of the way.
fn bench(n: usize) -> Universe {
    let mut u = Universe::new(1984);
    u.spawn_settlers(n, 1);
    u.ship.position += DVec3::new(0.0, 0.0, 5.0e7);
    let home = u.ship_system;
    for c in &mut u.crafts {
        c.system = home;
        c.avionics.pirate = false;
        c.trader = false;
        c.avionics.route.active = false;
        c.avionics.route.dwell_until = None;
    }
    u
}

/// Put craft `i` in flight at `position`, moving at `velocity`, facing `toward`.
fn place(u: &mut Universe, i: usize, position: DVec3, velocity: DVec3, toward: DVec3) {
    let s = &mut u.crafts[i].ship;
    s.state = ShipState::Flying;
    s.hyperdrive = false;
    s.position = position;
    s.velocity = velocity;
    s.orientation = universe_sim::ship::facing((toward - position).normalize(), DVec3::Y);
}

/// Clear craft `i` for `target` and hand it to the autopilot.
fn cleared(u: &mut Universe, i: usize, target: NavTarget) {
    u.craft_set_nav_target(i, Some(target));
    assert!(u.craft_request_clearance(i), "craft {i} cleared");
    u.craft_toggle_autopilot(i);
}

/// Run the world at 1× for up to `seconds` of game time, until `done`; true if it was.
fn run(u: &mut Universe, seconds: f64, mut done: impl FnMut(&Universe) -> bool) -> bool {
    for _ in 0..(seconds * 60.0) as usize {
        u.step_world(1.0 / 60.0, 1.0, &Controls::default());
        if done(u) {
            return true;
        }
    }
    false
}

/// The recorder's incidents, for a failure message.
fn incidents(u: &Universe) -> String {
    u.recorder.incidents.iter().map(|i| i.to_string()).collect::<Vec<_>>().join("\n")
}

fn positions(u: &mut Universe) -> (std::rc::Rc<universe_sim::StarSystem>, Vec<DVec3>) {
    let sys = u.ship_system();
    let mut p = Vec::new();
    sys.positions(u.world.time, &mut p);
    (sys, p)
}

#[test]
fn two_ships_at_one_gate_take_turns() {
    let mut u = bench(2);
    let (sys, pos) = positions(&mut u);
    let gate = sys.bodies.iter().position(|b| b.kind == BodyKind::Gate).unwrap();
    let f = GateFrame::new(&sys, gate, u.world.time, &pos);
    let side = f.axis().any_orthonormal_vector();
    for (i, offset) in [(0, 600.0), (1, -600.0)] {
        place(&mut u, i, f.center - f.axis() * 9_000.0 + side * offset, f.velocity, f.center);
        cleared(&mut u, i, NavTarget::Gate(gate));
    }
    let mut waited = false;
    let through = run(&mut u, 900.0, |u| {
        waited |= u.crafts.iter().any(|c| c.avionics.corridor_denied);
        u.crafts.iter().all(|c| matches!(c.ship.state, ShipState::Transit { .. }) || c.system != u.ship_system)
    });
    assert!(through, "both through the gate\n{}", incidents(&u));
    assert!(waited, "one waited its turn");
    assert!(u.recorder.incidents.is_empty(), "no wrecks\n{}", incidents(&u));
}

#[test]
fn a_ship_holds_while_the_pads_are_full_then_lands_on_the_one_freed() {
    let mut u = bench(10);
    let (sys, pos) = positions(&mut u);
    let planet = sys.bodies[sys.station().unwrap()].rail.parent.unwrap();
    let port = sys.spaceports.iter().position(|p| p.body == planet).unwrap();
    let home = u.ship_system;
    // Nine on the nine pads, staying there (a long stop).
    for k in 0..PADS {
        u.crafts[k].ship = u.world.ship_on(home, Facility::Spaceport(port), k);
        let r = &mut u.crafts[k].avionics.route;
        r.stops = vec![universe_sim::Stop { system: home, target: NavTarget::Spaceport(port) }];
        r.next = 0;
        r.active = true;
        r.dwell_until = Some(1.0e12);
    }
    // The tenth, 20 km over the port.
    let pad = PadFrame::new(&sys, port, u.world.time, &pos);
    let at = pad.pad + pad.up * 20_000.0;
    place(&mut u, 9, at, pad.frame_velocity(at), pad.pad);
    run(&mut u, 0.1, |_| false); // traffic control sees the pads taken
    cleared(&mut u, 9, NavTarget::Spaceport(port));
    let slot = u.crafts[9].avionics.clearance.unwrap().pad;
    assert!(matches!(slot, PadSlot::Hold(0)), "first in line: {slot:?}");
    // It holds, and doesn't come down.
    run(&mut u, 120.0, |_| false);
    let c = u.crafts[9].avionics.clearance.unwrap();
    assert_eq!(c.phase, Phase::Hold);
    let alt = u.crafts[9].ship.position.distance(pad.body_center) - pad.body_radius;
    assert!(alt > 3_000.0, "holding high ({alt:.0} m)\n{}", incidents(&u));
    // Pad 5's ship leaves (to the far side of the planet): it's freed, and ours.
    let far = pad.body_center - pad.up * (pad.body_radius + 50_000.0);
    place(&mut u, 5, far, pad.frame_velocity(far), pad.body_center);
    let landed = run(&mut u, 600.0, |u| matches!(u.crafts[9].ship.state, ShipState::Landed { .. }));
    assert!(landed, "landed once a pad was free\n{}", incidents(&u));
    let ShipState::Landed { local_position, .. } = u.crafts[9].ship.state else { unreachable!() };
    assert_eq!(pad_at(&sys, port, local_position.normalize()), Some(5), "on the freed pad");
    assert!(u.recorder.incidents.is_empty(), "no wrecks\n{}", incidents(&u));
}

#[test]
fn a_docking_and_a_launch_share_the_corridor() {
    let mut u = bench(2);
    let (sys, pos) = positions(&mut u);
    let station = sys.station().unwrap();
    let home = u.ship_system;
    // Craft 0 docked, about to leave for the gate; craft 1 inbound, 6 km up the axis.
    u.crafts[0].ship = u.world.ship_on(home, Facility::Station(station), 0);
    let gate = sys.bodies.iter().position(|b| b.kind == BodyKind::Gate).unwrap();
    let r = &mut u.crafts[0].avionics.route;
    r.stops = vec![
        universe_sim::Stop { system: home, target: NavTarget::Station(station) },
        universe_sim::Stop { system: home, target: NavTarget::Gate(gate) },
    ];
    r.next = 0;
    r.active = true;
    r.dwell_until = Some(u.world.time + 5.0);
    let f = StationFrame::new(&sys, station, u.world.time, &pos);
    place(&mut u, 1, f.on_axis(6_000.0), f.velocity, f.center);
    cleared(&mut u, 1, NavTarget::Station(station));
    let done = run(&mut u, 900.0, |u| {
        let docked = matches!(u.crafts[1].ship.state, ShipState::Landed { .. });
        let away = u.crafts[0].ship.is_flying() && u.crafts[0].ship.position.distance(u.crafts[1].ship.position) > 5_000.0;
        docked && away
    });
    assert!(done, "one docked, the other away\n{}", incidents(&u));
    assert!(u.recorder.incidents.is_empty(), "no wrecks\n{}", incidents(&u));
}

#[test]
fn pirates_leave_sheltered_ships_alone_and_hunt_in_the_open() {
    for (out, hunts) in [(4_000.0, false), (30_000.0, true)] {
        let mut u = bench(2);
        let (sys, pos) = positions(&mut u);
        let station = sys.station().unwrap();
        let f = StationFrame::new(&sys, station, u.world.time, &pos);
        let side = f.axis().any_orthonormal_vector();
        let prey = f.center + side * out;
        place(&mut u, 0, prey + f.axis() * 8_000.0, f.velocity, prey);
        place(&mut u, 1, prey, f.velocity, f.center);
        u.crafts[0].avionics.pirate = true;
        run(&mut u, 5.0, |_| false);
        assert_eq!(u.crafts[0].avionics.hunting.is_some(), hunts, "prey {out} m from the station");
    }
}

#[test]
fn the_recorder_files_a_head_on_collision_with_both_traces() {
    let mut u = bench(2);
    let (_, pos) = positions(&mut u);
    // Far out in the open, closing at 60 m/s.
    let at = pos[0] + DVec3::new(0.0, 3.0e11, 0.0);
    let v = DVec3::X * 30.0;
    place(&mut u, 0, at - DVec3::X * 150.0, v, at);
    place(&mut u, 1, at + DVec3::X * 150.0, -v, at);
    for c in &mut u.crafts {
        c.ship.orientation = DQuat::IDENTITY;
    }
    run(&mut u, 10.0, |u| u.recorder.incidents.len() >= 2);
    let text = incidents(&u);
    assert!(text.contains("COLLISION") && text.contains("closing"), "{text}");
    eprintln!("{}", u.recorder.incidents[0]);
}
