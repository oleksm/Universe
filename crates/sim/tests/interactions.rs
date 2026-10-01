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
    // Pilots' commands reach their ships two ticks late, as they will once
    // pilots run apart from the world.
    u.command_delay = 2;
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

fn positions(u: &mut Universe) -> (std::sync::Arc<universe_sim::StarSystem>, Vec<DVec3>) {
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

/// A defended place in the home system (a station's or spaceport's turret,
/// not a gate's: a ship there could drift through the ring): the turret,
/// where it is and how it moves, and the way out from its body.
fn a_turret(u: &mut Universe) -> (universe_sim::world::turrets::Turret, DVec3, DVec3, DVec3) {
    let home = u.ship_system;
    let (sys, pos) = positions(u);
    let (t, at, v) = u
        .world
        .turret_motions(home)
        .into_iter()
        .find(|(t, _, _)| !matches!(t.facility, Facility::Gate(_)))
        .expect("the home system has a defended station or port");
    let _ = sys;
    let out = (at - pos[t.body]).normalize();
    (t, at, v, out)
}

#[test]
fn pirates_leave_ships_under_the_guns_alone_and_hunt_in_the_open() {
    use universe_sim::world::turrets::TURRET_RANGE;
    for (out, hunts) in [(TURRET_RANGE * 0.5, false), (TURRET_RANGE + 25_000.0, true)] {
        let mut u = bench(2);
        let (_, at, v, side) = a_turret(&mut u);
        let prey = at + side * out;
        place(&mut u, 0, prey + side.any_orthonormal_vector() * 8_000.0, v, prey);
        place(&mut u, 1, prey, v, at);
        u.crafts[0].avionics.pirate = true;
        run(&mut u, 5.0, |_| false);
        assert_eq!(u.crafts[0].avionics.hunting.is_some(), hunts, "prey {out:.0} m from the turret");
    }
}

#[test]
fn turrets_shoot_the_aggressor_and_spare_the_innocent() {
    let mut u = bench(2);
    let (_, at, v, side) = a_turret(&mut u);
    place(&mut u, 0, at + side * 2_500.0, v, at);
    place(&mut u, 1, at + side * 2_600.0 + side.any_orthonormal_vector() * 60.0, v, at);
    { let now = u.world.time; u.law.declare(universe_sim::craft_id(0), now + 600.0, now, universe_sim::protocol::Cause::Rules); }
    let down = run(&mut u, 60.0, |u| !u.crafts[0].ship.is_flying());
    assert!(down, "the aggressor is shot down (hull {:.2})", u.crafts[0].ship.hull);
    assert!(u.crafts[1].ship.hull > 0.99, "the innocent untouched ({:.2})", u.crafts[1].ship.hull);
    let kill = u.kills.last().expect("a kill");
    assert!(kill.killer_name.starts_with("SAM TURRET"), "{}", kill.killer_name);
}

#[test]
fn a_ship_under_fire_runs_for_the_guns() {
    let mut u = bench(2);
    // Well clear of the place's body (a ground port's planet would pull them down).
    let (turret, at, v, side) = a_turret(&mut u);
    let prey = at + side * 1_500_000.0;
    place(&mut u, 0, prey + side.any_orthonormal_vector() * 6_000.0, v, prey);
    place(&mut u, 1, prey, v, at);
    u.crafts[0].avionics.pirate = true;
    let hit = run(&mut u, 120.0, |u| u.crafts[1].ship.hull < 1.0);
    assert!(hit, "the pirate strikes\n{}", incidents(&u));
    let r = &u.crafts[1].avionics.route;
    let heading = r.stops.get(r.next).map(|s| s.target);
    let havens: Vec<_> = u.world.turret_motions(u.ship_system).into_iter().map(|(t, _, _)| t.facility).collect();
    assert!(heading.is_some_and(|h| havens.contains(&h)), "heading for a defended place: {heading:?}, turret at {:?}", turret.facility);
    assert!(r.active);
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


#[test]
fn a_ship_keeps_at_range_from_another_as_it_accelerates_away() {
    use universe_sim::avionics::follow::{Anchor, Manoeuvre};
    let mut u = bench(2);
    let (_, at, v, side) = a_turret(&mut u);
    // Well out from the place's body, in its gravity all the same.
    let lead = at + side * 1_500_000.0;
    let across = side.any_orthonormal_vector();
    place(&mut u, 1, lead, v + across * 40.0, lead + across);
    place(&mut u, 0, lead - side * 8_000.0, v, lead);
    u.crafts[1].ship.throttle = 0.03;
    u.craft_follow(0, Anchor::Ship(universe_sim::craft_id(1)), Manoeuvre::KeepAt(2_000.0));
    run(&mut u, 120.0, |_| false);
    let (a, b) = (&u.crafts[0].ship, &u.crafts[1].ship);
    let d = a.position.distance(b.position);
    let rel = (a.velocity - b.velocity).length();
    assert!((d - 2_000.0).abs() < 150.0 && rel < 5.0, "at {d:.0} m, {rel:.1} m/s relative\n{}", incidents(&u));
    assert!(u.crafts[0].avionics.following.is_some());
    assert!(u.recorder.incidents.is_empty(), "no wrecks\n{}", incidents(&u));
}

#[test]
fn a_ship_orbits_a_station_at_the_radius_asked() {
    use universe_sim::avionics::follow::{orbit_speed, Anchor, Manoeuvre};
    let mut u = bench(1);
    let (sys, pos) = positions(&mut u);
    let station = sys.station().unwrap();
    let f = StationFrame::new(&sys, station, u.world.time, &pos);
    let start = f.center + f.axis().any_orthonormal_vector() * 9_000.0;
    place(&mut u, 0, start, f.velocity, f.center);
    u.craft_follow(0, Anchor::Place(NavTarget::Station(station)), Manoeuvre::Orbit(5_000.0));
    run(&mut u, 200.0, |_| false);
    // Settled: on the circle, going round at the orbit speed, for the next minute.
    let mut swept = 0.0;
    let mut last: Option<DVec3> = None;
    for _ in 0..60 {
        run(&mut u, 1.0, |_| false);
        let (sys, pos) = positions(&mut u);
        let f = StationFrame::new(&sys, station, u.world.time, &pos);
        let s = &u.crafts[0].ship;
        let off = s.position - f.center;
        assert!((off.length() - 5_000.0).abs() < 300.0, "{:.0} m out\n{}", off.length(), incidents(&u));
        let rel = s.velocity - f.velocity;
        let tangential = (rel - off.normalize() * rel.dot(off.normalize())).length();
        assert!((tangential - orbit_speed(5_000.0, s.side_accel())).abs() < 15.0, "going round at {tangential:.0} m/s");
        if let Some(l) = last {
            swept += l.angle_between(off);
        }
        last = Some(off);
    }
    assert!(swept > 0.5, "went round ({swept:.2} rad in a minute)");
    assert!(u.recorder.incidents.is_empty(), "no wrecks\n{}", incidents(&u));
}

#[test]
fn a_ship_orbits_a_moving_ship() {
    use universe_sim::avionics::follow::{Anchor, Manoeuvre};
    let mut u = bench(2);
    let (_, at, v, side) = a_turret(&mut u);
    let centre = at + side * 1_500_000.0;
    let across = side.any_orthonormal_vector();
    place(&mut u, 1, centre, v + across * 60.0, centre + across);
    place(&mut u, 0, centre + side * 3_000.0, v, centre);
    u.craft_follow(0, Anchor::Ship(universe_sim::craft_id(1)), Manoeuvre::Orbit(2_000.0));
    run(&mut u, 150.0, |_| false);
    for _ in 0..30 {
        run(&mut u, 1.0, |_| false);
        let d = u.crafts[0].ship.position.distance(u.crafts[1].ship.position);
        assert!((d - 2_000.0).abs() < 200.0, "{d:.0} m from it\n{}", incidents(&u));
    }
    assert!(u.recorder.incidents.is_empty(), "no wrecks\n{}", incidents(&u));
}

#[test]
fn turrets_shoot_down_an_aggressed_player_burning_hard_at_the_edge_of_their_reach() {
    let mut u = bench(0);
    let (_, at, v, side) = a_turret(&mut u);
    u.ship.state = ShipState::Flying;
    u.ship.position = at + side * 5_500.0;
    u.ship.velocity = v;
    u.ship.orientation = universe_sim::ship::facing(side.any_orthonormal_vector(), side);
    u.ship.throttle = 1.0;
    { let now = u.world.time; u.law.declare(universe_sim::PLAYER, now + 600.0, now, universe_sim::protocol::Cause::Rules); }
    let down = run(&mut u, 20.0, |u| u.kills.iter().any(|k| k.victim == universe_sim::PLAYER));
    assert!(down, "shot down (hull {:.2})", u.ship.hull);
}

/// Out in the open, clear of any turret (but in a place's gravity): a point,
/// how the place moves, and two directions across.
fn open_space(u: &mut Universe) -> (DVec3, DVec3, DVec3, DVec3) {
    let (_, at, v, side) = a_turret(u);
    let across = side.any_orthonormal_vector();
    (at + side * 1_500_000.0, v, across, side.cross(across))
}

#[test]
fn a_lone_settler_leaves_an_aggressor_alone() {
    let mut u = bench(2);
    let (p, v, a, b) = open_space(&mut u);
    place(&mut u, 0, p, v, p + a);
    place(&mut u, 1, p + b * 4_000.0, v, p);
    { let now = u.world.time; u.law.declare(universe_sim::craft_id(0), now + 600.0, now, universe_sim::protocol::Cause::Rules); }
    run(&mut u, 10.0, |_| false);
    assert!(u.crafts[1].avionics.hunting.is_none(), "one against one: no fight");
}

#[test]
fn settlers_gang_up_on_an_aggressor_and_shoot_it_down() {
    let mut u = bench(6);
    let (p, v, a, b) = open_space(&mut u);
    place(&mut u, 0, p, v, p + a);
    { let now = u.world.time; u.law.declare(universe_sim::craft_id(0), now + 600.0, now, universe_sim::protocol::Cause::Rules); }
    for k in 1..6 {
        let dir = (a * (k as f64).cos() + b * (k as f64).sin()).normalize();
        place(&mut u, k, p + dir * 4_000.0, v, p);
    }
    let down = run(&mut u, 240.0, |u| !u.crafts[0].ship.is_flying());
    let defenders = u.crafts[1..].iter().filter(|c| c.avionics.hunting.is_some()).count();
    assert!(down, "the aggressor is shot down (hull {:.2}, {} defending, {} defences)\n{}", u.crafts[0].ship.hull, defenders, u.traffic.defences, incidents(&u));
    assert!(u.traffic.defences >= 2, "they went after it together ({})", u.traffic.defences);
    assert!((1..u.crafts.len()).all(|i| !u.law.aggressed(universe_sim::craft_id(i), u.world.time)), "shooting the aggressor is no crime");
    assert!(u.kills.iter().any(|k| k.victim == universe_sim::craft_id(0) && k.killer != universe_sim::PLAYER));
    // Standing down, they slow and keep clear of each other.
    run(&mut u, 30.0, |_| false);
    assert!(!u.recorder.incidents.iter().any(|i| i.cause == "COLLISION"), "no collisions after\n{}", incidents(&u));
}

#[test]
fn an_aggressed_player_is_judged_like_anyone() {
    let mut u = bench(5);
    let (p, v, a, b) = open_space(&mut u);
    u.ship.state = ShipState::Flying;
    u.ship.position = p;
    u.ship.velocity = v;
    { let now = u.world.time; u.law.declare(universe_sim::PLAYER, now + 600.0, now, universe_sim::protocol::Cause::Rules); }
    for k in 0..5 {
        let dir = (a * (k as f64).cos() + b * (k as f64).sin()).normalize();
        place(&mut u, k, p + dir * 4_000.0, v, p);
    }
    let hit = run(&mut u, 120.0, |u| u.ship.hull < 1.0);
    assert!(hit, "the settlers turn on us ({} defences)\n{}", u.traffic.defences, incidents(&u));
}

#[test]
fn pirates_hunt_the_player_too() {
    let mut u = bench(1);
    let (p, v, a, _) = open_space(&mut u);
    u.ship.state = ShipState::Flying;
    u.ship.position = p;
    u.ship.velocity = v;
    place(&mut u, 0, p + a * 8_000.0, v, p);
    u.crafts[0].avionics.pirate = true;
    run(&mut u, 5.0, |_| false);
    assert_eq!(u.crafts[0].avionics.hunting.map(|h| h.target), Some(universe_sim::PLAYER));
}

/// A world with air in the home system: its index, and its centre and velocity now.
fn airy_world(u: &mut Universe) -> (usize, DVec3, DVec3, f64) {
    let (sys, pos) = positions(u);
    let i = sys.bodies.iter().position(|b| b.rail.atmosphere.is_some()).expect("the home system has a world with air");
    (i, pos[i], sys.velocity(i, u.world.time), sys.bodies[i].rail.radius)
}

#[test]
fn a_steep_dive_into_the_air_burns_the_ship_up() {
    let mut u = bench(1);
    let (_, center, v, radius) = airy_world(&mut u);
    let up = DVec3::Y;
    // 120 km up, falling straight down at 7.5 km/s.
    place(&mut u, 0, center + up * (radius + 120_000.0), v - up * 7_500.0, center);
    let gone = run(&mut u, 60.0, |u| !u.crafts[0].ship.is_flying());
    assert!(gone, "wrecked");
    let i = u.recorder.incidents.last().expect("filed");
    assert_eq!(i.cause, "RE-ENTRY HEAT", "burnt up before it hit the ground\n{i}");
}

#[test]
fn the_air_slows_a_falling_ship_to_its_terminal_velocity() {
    let mut u = bench(1);
    let (body, center, v, _) = airy_world(&mut u);
    let up = DVec3::Y;
    // 6 km up over the ground (well clear of the hills), dropping at 400 m/s, engines off.
    let (sys, _) = positions(&mut u);
    let ground = sys.bodies[body].surface_radius(up);
    let at = center + up * (ground + 6_000.0);
    let air = v + sys.bodies[body].angular_velocity().cross(at - center);
    place(&mut u, 0, at, air - up * 400.0, center);
    run(&mut u, 15.0, |_| false);
    let (sys, pos) = positions(&mut u);
    let s = &u.crafts[0].ship;
    let air = sys.velocity(body, u.world.time) + sys.bodies[body].angular_velocity().cross(s.position - pos[body]);
    let speed = (s.velocity - air).length();
    // (In vacuum it would be falling at ~550 m/s by now.)
    assert!(s.is_flying() && speed < 300.0, "slowed from 400 to {speed:.0} m/s\n{}", incidents(&u));
}

#[test]
fn ships_waiting_for_a_corridor_know_their_place_in_line() {
    let mut u = bench(3);
    let (sys, pos) = positions(&mut u);
    let station = sys.station().unwrap();
    let f = StationFrame::new(&sys, station, u.world.time, &pos);
    let side = f.axis().any_orthonormal_vector();
    for i in 0..3 {
        let at = f.on_axis(7_000.0 + 1_500.0 * i as f64) + side * 300.0 * i as f64;
        place(&mut u, i, at, f.velocity, f.center);
        cleared(&mut u, i, NavTarget::Station(station));
    }
    run(&mut u, 1.0, |_| false);
    let mut places: Vec<Option<usize>> = u.crafts.iter().map(|c| c.avionics.corridor_ahead).collect();
    places.sort();
    assert_eq!(places, vec![None, Some(1), Some(2)], "one in the corridor, then one and two ahead");
}

#[test]
fn a_crafts_commands_reach_its_ship_two_ticks_late() {
    let mut u = bench(1);
    let (p, v, a, _) = open_space(&mut u);
    place(&mut u, 0, p, v, p + a);
    // Following a far point makes its pilot burn at once.
    let station = positions(&mut u).0.station().unwrap();
    u.craft_follow(0, universe_sim::avionics::follow::Anchor::Place(NavTarget::Station(station)), universe_sim::avionics::follow::Manoeuvre::KeepAt(2_000.0));
    let mut throttles = Vec::new();
    for _ in 0..4 {
        run(&mut u, 1.0 / 60.0, |_| false);
        let s = &u.crafts[0].ship;
        throttles.push((s.throttle, s.rcs.length()));
    }
    let moved = |(t, r): (f64, f64)| t > 0.0 || r > 0.0;
    assert!(!moved(throttles[0]) && !moved(throttles[1]), "nothing reaches the devices for two ticks: {throttles:?}");
    assert!(moved(throttles[2]) || moved(throttles[3]), "then it does: {throttles:?}");
}

#[test]
fn the_law_rules_a_pirate_fair_game_from_its_first_hit_with_the_evidence() {
    let mut u = bench(2);
    let (p, v, a, _) = open_space(&mut u);
    place(&mut u, 0, p + a * 6_000.0, v, p);
    place(&mut u, 1, p, v, p + a);
    u.crafts[0].avionics.pirate = true;
    let (pirate, prey) = (universe_sim::craft_id(0), universe_sim::craft_id(1));
    let ruled = run(&mut u, 120.0, |u| u.law.aggressed(pirate, u.world.time));
    assert!(ruled, "the pirate struck and the law saw it\n{}", incidents(&u));
    let now = u.world.time;
    let r = *u.law.standing(pirate, now).expect("on record");
    assert_eq!((r.evidence.shooter, r.evidence.target), (pirate, prey), "the hit is the evidence");
    match r.cause {
        universe_sim::protocol::Cause::Event { tick, index } => {
            assert_eq!(tick, u.tick, "ruled the tick of the hit");
            let (struck, e) = &u.log[index as usize];
            assert_eq!(*struck, prey);
            assert!(matches!(e, universe_sim::world::ShipEvent::Hit { by, weapon: true, .. } if *by == pirate), "{e:?}");
        }
        other => panic!("caused by {other:?}"),
    }
    assert!(!u.law.aggressed(prey, now), "the victim is innocent");
}

#[test]
fn a_trade_is_booked_in_the_ledger_with_its_request_as_cause_and_the_ship_weighs_its_hold() {
    use universe_sim::services::{Asset, Party};
    let mut u = bench(0);
    let (sys, _) = positions(&mut u);
    let station = sys.station().unwrap();
    let home = u.ship_system;
    u.ship = u.world.ship_on(home, Facility::Station(station), 0);
    let f = Facility::Station(station);
    let before = u.credits();
    let (quotes, _) = u.market_quotes(f);
    let q = quotes.iter().find(|q| q.buy.is_some() && q.level >= 3.0).expect("something to buy");
    let item = q.offer.item;
    let paid = u.trade(f, item, 3).expect("bought");
    assert!((u.credits() - (before - paid)).abs() < 1e-6);
    assert_eq!(u.hold(), vec![(item, 3)]);
    assert!((u.ship.cargo - 3.0 * u.world.goods[item].mass).abs() < 1e-6, "the core's mass follows the hold");
    // Both legs journalled, caused by our request.
    let legs: Vec<_> = u.ledger.journal.iter().rev().take(2).collect();
    assert!(legs.iter().all(|e| matches!(e.cause, universe_sim::protocol::Cause::Message { sender: 0, .. })), "{legs:?}");
    assert!(legs.iter().any(|e| e.asset == Asset::Credits && e.from == Party::Pilot(0)));
    assert!(legs.iter().any(|e| e.asset == Asset::Goods(item) && e.to == Party::Pilot(0)));
    assert!(u.ledger.balanced(), "nothing made or lost");
    // Undocked, the market won't trade.
    let far = u.ship.position + DVec3::X * 50_000.0;
    place_player(&mut u, far);
    assert!(u.trade(f, item, -1).is_err());
}

fn place_player(u: &mut Universe, at: DVec3) {
    u.ship.state = ShipState::Flying;
    u.ship.position = at;
}
