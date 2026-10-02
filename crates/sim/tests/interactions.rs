//! Interaction tests: a few ships placed in one situation, run for a few
//! minutes of game time (seconds of real time), checked for the outcome. On
//! failure they print the flight recorder's incidents, so the cause is there
//! to read. These replace whole-traffic runs for developing traffic behaviour.

use glam::{DQuat, DVec3};
use universe_sim::avionics::nav::PadSlot;
use universe_sim::world::spaceport::{pad_at, PADS};
use universe_sim::world::{Facility, StationFrame};
use universe_sim::{BodyKind, Controls, NavTarget, PadFrame, Phase, ShipState, Universe};

/// A world with `n` plain settlers (no pirates, traders or routes), all in
/// the home system, and our own ship parked out of the way.
fn bench(n: usize) -> Universe {
    let mut u = Universe::new(1984);
    // (In flight, not parked on the home station's deck.)
    u.respawn();
    u.step_world(1.0 / 60.0, 1.0, &Controls::default());
    u.events.clear();
    u.spawn_settlers(n, 1);
    u.ship.position += DVec3::new(0.0, 0.0, 5.0e7);
    let home = u.ship_system;
    for c in &mut u.crafts {
        c.system = home;
    }
    for p in u.pilots().iter_mut() {
        p.trader = false;
        p.avionics.pirate = false;
        p.avionics.route.active = false;
        p.avionics.route.dwell_until = None;
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
fn a_ship_holds_while_the_pads_are_full_then_lands_on_the_one_freed() {
    let mut u = bench(PADS + 1);
    let (sys, pos) = positions(&mut u);
    let planet = sys.bodies[sys.station().unwrap()].rail.parent.unwrap();
    let port = sys.spaceports.iter().position(|p| p.body == planet).unwrap();
    let home = u.ship_system;
    // One on every pad, staying there (a long stop, on the pad).
    for k in 0..PADS {
        u.crafts[k].ship = u.world.ship_on(home, Facility::Spaceport(port), k);
        let mut pilots = u.pilots();
        let r = &mut pilots[k].avionics.route;
        r.stops = vec![universe_sim::Stop { system: home, target: NavTarget::Spaceport(port) }];
        r.next = 0;
        r.active = true;
        r.dwell_until = Some(1.0e12);
        drop(pilots);
    }
    // One more, 20 km over the port.
    let pad = PadFrame::new(&sys, port, u.world.time, &pos);
    let at = pad.pad + pad.up * 20_000.0;
    place(&mut u, PADS, at, pad.frame_velocity(at), pad.pad);
    run(&mut u, 0.1, |_| false); // traffic control sees the pads taken
    cleared(&mut u, PADS, NavTarget::Spaceport(port));
    let slot = u.pilots()[PADS].avionics.clearance.unwrap().pad;
    assert!(matches!(slot, PadSlot::Hold(0)), "first in line: {slot:?}");
    // It holds, and doesn't come down.
    run(&mut u, 120.0, |_| false);
    let c = u.pilots()[PADS].avionics.clearance.unwrap();
    assert_eq!(c.phase, Phase::Hold);
    // On the holding circle round the port: 12 km up, 10 km out, level,
    // flying round it.
    use universe_sim::landing::{HOLD_ALTITUDE, HOLD_RADIUS, HOLD_SPEED};
    let pad = PadFrame::new(&sys, port, u.world.time, &positions(&mut u).1);
    let s = &u.crafts[PADS].ship;
    let rel = s.position - pad.pad;
    let (up, out) = (rel.dot(pad.up), (rel - pad.up * rel.dot(pad.up)).length());
    let level = (s.orientation * DVec3::Y).dot((s.position - pad.body_center).normalize());
    let speed = (s.velocity - pad.frame_velocity(s.position)).length();
    eprintln!("holding: {up:.0} m up, {out:.0} m out, level {level:.3}, {speed:.0} m/s");
    assert!((up - HOLD_ALTITUDE).abs() < 1_000.0 && (out - HOLD_RADIUS).abs() < 1_000.0, "on the circle: {up:.0} m up, {out:.0} m out\n{}", incidents(&u));
    assert!(level > 0.95, "level, belly down ({level:.3})");
    assert!((speed - HOLD_SPEED).abs() < 30.0, "flying round it ({speed:.0} m/s)");
    // Pad 5's ship leaves (to the far side of the planet): it's freed, and ours.
    let far = pad.body_center - pad.up * (pad.body_radius + 50_000.0);
    place(&mut u, 5, far, pad.frame_velocity(far), pad.body_center);
    let landed = run(&mut u, 600.0, |u| matches!(u.crafts[PADS].ship.state, ShipState::Landed { .. }));
    assert!(landed, "landed once a pad was free\n{}", incidents(&u));
    let ShipState::Landed { local_position, .. } = u.crafts[PADS].ship.state else { unreachable!() };
    assert_eq!(pad_at(&sys, port, local_position.normalize()), Some(5), "on the freed pad");
    assert!(u.recorder.incidents.is_empty(), "no wrecks\n{}", incidents(&u));
    // Traffic control's journal says why: the waiting ship's own requests
    // (queued, then granted), and the pad freed by the rules as its ship left.
    use universe_sim::protocol::Cause;
    use universe_sim::services::atc::What;
    let (waiter, leaver) = (universe_sim::craft_id(PADS), universe_sim::craft_id(5));
    let j = &u.atc.journal;
    let mine = |c: &Cause| matches!(c, Cause::Message { sender, .. } if *sender == waiter as u64);
    assert!(j.iter().any(|c| c.ship == waiter && matches!(c.what, What::PadQueued { .. }) && mine(&c.cause)), "queued, at its request");
    assert!(j.iter().any(|c| c.ship == waiter && c.what == What::PadGranted { system: home, port: Facility::Spaceport(port), pad: 5 } && mine(&c.cause)), "granted, at its request");
    assert!(j.iter().any(|c| c.ship == leaver && c.what == What::PadFreed { system: home, port: Facility::Spaceport(port), pad: 5 } && c.cause == Cause::Rules), "freed as it left");
}

#[test]
fn a_docking_and_a_launch_share_the_corridor() {
    let mut u = bench(2);
    let (sys, pos) = positions(&mut u);
    let station = sys.station().unwrap();
    let home = u.ship_system;
    // Craft 0 docked, about to leave for the gate; craft 1 inbound, 6 km above the deck.
    u.crafts[0].ship = u.world.ship_on(home, Facility::Station(station), 0);
    let gate = sys.bodies.iter().position(|b| b.kind == BodyKind::Gate).unwrap();
    let mut pilots = u.pilots();
    let r = &mut pilots[0].avionics.route;
    r.stops = vec![
        universe_sim::Stop { system: home, target: NavTarget::Station(station) },
        universe_sim::Stop { system: home, target: NavTarget::Gate(gate) },
    ];
    r.next = 0;
    r.active = true;
    r.dwell_until = Some(u.world.time + 5.0);
    drop(pilots);
    let f = StationFrame::new(&sys, station, u.world.time, &pos);
    place(&mut u, 1, f.center + f.up() * 6_000.0, f.velocity, f.center);
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
        u.pilots()[0].avionics.pirate = true;
        run(&mut u, 5.0, |_| false);
        assert_eq!(u.pilots()[0].avionics.hunting.is_some(), hunts, "prey {out:.0} m from the turret");
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
    let kill = u.records.kills.last().expect("a kill");
    assert!(kill.killer_name.starts_with("SAM TURRET"), "{}", kill.killer_name);
}

#[test]
fn sams_reach_the_aggressor_far_beyond_the_guns() {
    let mut u = bench(1);
    let (_, at, v, side) = a_turret(&mut u);
    // 40 km out: well beyond any gun, in the missiles' reach.
    let far = at + side * 40_000.0;
    place(&mut u, 0, far, v, far + side.any_orthonormal_vector());
    { let now = u.world.time; u.law.declare(universe_sim::craft_id(0), now + 600.0, now, universe_sim::protocol::Cause::Rules); }
    let down = run(&mut u, 60.0, |u| !u.crafts[0].ship.is_flying());
    assert!(down, "the missiles bring it down (hull {:.2}, {} in the air)", u.crafts[0].ship.hull, u.world.missiles.len());
    let kill = u.records.kills.last().expect("a kill");
    assert_eq!(kill.weapon, "MISSILE");
}

#[test]
fn a_ship_under_fire_runs_for_the_guns() {
    let mut u = bench(2);
    // Well clear of the place's body (a ground port's planet would pull them down).
    let (turret, at, v, side) = a_turret(&mut u);
    let prey = at + side * 1_500_000.0;
    place(&mut u, 0, prey + side.any_orthonormal_vector() * 6_000.0, v, prey);
    place(&mut u, 1, prey, v, at);
    u.pilots()[0].avionics.pirate = true;
    let hit = run(&mut u, 120.0, |u| u.crafts[1].ship.hull < 1.0);
    assert!(hit, "the pirate strikes\n{}", incidents(&u));
    let r = u.pilots()[1].avionics.route.clone();
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
    assert!(u.pilots()[0].avionics.following.is_some());
    assert!(u.recorder.incidents.is_empty(), "no wrecks\n{}", incidents(&u));
}

/// Out in the open, clear of any turret (but in a place's gravity): a point,
/// how the place moves, and two directions across.
fn open_space(u: &mut Universe) -> (DVec3, DVec3, DVec3, DVec3) {
    let (_, at, v, side) = a_turret(u);
    let across = side.any_orthonormal_vector();
    (at + side * 1_500_000.0, v, across, side.cross(across))
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
    let defenders = u.pilots()[1..].iter().filter(|p| p.avionics.hunting.is_some()).count();
    assert!(down, "the aggressor is shot down (hull {:.2}, {} defending, {} defences)\n{}", u.crafts[0].ship.hull, defenders, u.pool().tally.defences.load(std::sync::atomic::Ordering::Relaxed), incidents(&u));
    let posses = u.pool().tally.defences.load(std::sync::atomic::Ordering::Relaxed);
    assert!(posses >= 2, "they went after it together ({posses})");
    assert!((1..u.crafts.len()).all(|i| !u.law.aggressed(universe_sim::craft_id(i), u.world.time)), "shooting the aggressor is no crime");
    assert!(u.records.kills.iter().any(|k| k.victim == universe_sim::craft_id(0) && k.killer != universe_sim::PLAYER));
    // Standing down, they slow and keep clear of each other.
    run(&mut u, 30.0, |_| false);
    assert!(!u.recorder.incidents.iter().any(|i| i.cause == "COLLISION"), "no collisions after\n{}", incidents(&u));
}

#[test]
fn the_law_rules_a_pirate_fair_game_from_its_first_hit_with_the_evidence() {
    let mut u = bench(2);
    let (p, v, a, _) = open_space(&mut u);
    place(&mut u, 0, p + a * 6_000.0, v, p);
    place(&mut u, 1, p, v, p + a);
    u.pilots()[0].avionics.pirate = true;
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

#[test]
fn a_trader_asks_for_quotes_decides_and_trades_at_its_stop() {
    let mut u = bench(1);
    let (sys, _) = positions(&mut u);
    let home = u.ship_system;
    let station = sys.station().unwrap();
    // Docked at the station, its stop just begun: it asks the market, then
    // sells, buys and picks where next, all its own decisions.
    u.crafts[0].ship = u.world.ship_on(home, Facility::Station(station), 0);
    {
        let mut pilots = u.pilots();
        let p = &mut pilots[0];
        p.trader = true;
        let r = &mut p.avionics.route;
        r.stops = vec![universe_sim::Stop { system: home, target: NavTarget::Station(station) }];
        r.next = 0;
        r.active = true;
        r.dwell_until = None;
    }
    let name = u.crafts[0].name.to_uppercase();
    run(&mut u, 3.0, |u| u.records.trades.iter().any(|t| t.trader == name));
    let mine: Vec<_> = u.records.trades.iter().filter(|t| t.trader == name).map(|t| format!("{:?} {} {}", t.deal, t.units, t.item)).collect();
    eprintln!("{name}: {mine:?} (stops {})", u.records.stats.stops);
    assert!(!mine.is_empty(), "the trader traded or declared where it's going");
}

#[test]
fn a_miner_digs_ore_into_its_hold_the_rock_remembers_and_a_station_buys_it() {
    use universe_sim::services::{Asset, Party};
    use universe_sim::world::{mining, physics, ShipCommands};
    let mut u = bench(0);
    let (sys, _) = positions(&mut u);
    let home = u.ship_system;
    // A fragment the excavator digs at full speed (gravel), in some field.
    let t = u.world.time;
    let (f, bodies, i) = (0..sys.fields.len())
        .find_map(|f| {
            let bodies = sys.field_bodies(f);
            let i = (sys.bodies.len()..bodies.len()).find(|&i| bodies[i].rock.as_ref().is_some_and(|r| mining::dig_rate(r) >= mining::EXCAVATOR_THROUGHPUT))?;
            Some((f, bodies, i))
        })
        .expect("a rubble pile");
    let mut pos = Vec::new();
    physics::positions(&bodies[..], t, &mut pos);
    let b = &bodies[i];
    let up = DVec3::Y;
    let at = pos[i] + up * (b.surface_radius(b.rotation(t).inverse() * up) + universe_sim::world::ship::SHIP_RADIUS + 10.0);
    place_player(&mut u, at);
    u.ship.velocity = physics::velocity(&bodies[..], i, t) + b.angular_velocity().cross(at - pos[i]);
    u.ship.angular_velocity = DVec3::ZERO;
    u.command(&ShipCommands { anchor: Some(true), ..u.ship.holding() });
    for _ in 0..10 {
        u.step_world(1.0 / 60.0, 1.0, &Controls::default());
    }
    assert!(matches!(u.ship.state, ShipState::Anchored { .. }), "{:?}", u.events);
    // Anchored, the pilot can get up and walk about the ship.
    u.walk(&universe_sim::world::WalkCommands { interact: true, ..Default::default() }, 1.0 / 60.0);
    u.walk(&universe_sim::world::WalkCommands::default(), 1.0 / 60.0);
    assert!(!u.crew.seated(), "up out of the seat");
    u.walk(&universe_sim::world::WalkCommands { interact: true, ..Default::default() }, 1.0 / 60.0);
    u.command(&ShipCommands { excavate: Some(true), ..u.ship.holding() });
    // A tonne at 10 kg/s.
    for _ in 0..(105 * 60) {
        u.step_world(1.0 / 60.0, 1.0, &Controls::default());
    }
    let item = mining::ore(b.rock.as_ref().unwrap()).item();
    assert_eq!(u.hold(), vec![(item, 1)], "a tonne of {} in the hold", u.world.goods[item].name);
    assert_eq!(u.world.dug(home, f, i), 1000.0, "the rock remembers");
    let entry = u.ledger.journal.iter().rev().find(|e| e.asset == Asset::Goods(item)).unwrap();
    assert!(entry.from == Party::World && matches!(entry.cause, universe_sim::protocol::Cause::Event { .. }), "{entry:?}");
    assert!(u.ledger.balanced());

    // Docked at the station, it sells.
    let station = sys.station().unwrap();
    let cargo = u.ship.cargo;
    u.ship = u.world.ship_on(home, Facility::Station(station), 0);
    u.ship.cargo = cargo;
    let paid = u.trade(Facility::Station(station), item, -1).expect("the station buys ore");
    assert!(paid < 0.0 && u.hold().is_empty());
}

/// One settler made a miner (its route: a field of the home system, then the
/// station), flying among the field, 15 km from its remnant.
fn miner_by_its_field() -> Universe {
    let mut u = bench(1);
    let home = u.ship_system;
    let charts = u.world.charts();
    let stops = universe_sim::miner::route(&charts, home, 7).expect("fields at home");
    let NavTarget::Asteroid(remnant) = stops[0].target else { panic!() };
    {
        let mut p = u.pilots();
        p[0].miner = true;
        p[0].avionics.route = universe_sim::avionics::route::Route { stops, next: 0, active: true, dwell_until: None, departing: false, stay: None, hangar_ordered: 0.0 };
    }
    let (sys, pos) = positions(&mut u);
    let c = &mut u.crafts[0];
    // (A miner flies a ship with a mining rig.)
    c.ship.class = universe_sim::world::content::content().handle("hull.prospector").unwrap();
    c.ship.state = ShipState::Flying;
    c.ship.position = pos[remnant] + DVec3::X * (sys.bodies[remnant].max_radius() + 15_000.0);
    c.ship.velocity = sys.velocity(remnant, u.world.time);
    u
}

#[test]
fn a_miner_closes_on_a_rock_anchors_digs_and_when_full_goes_to_sell() {
    let mut u = miner_by_its_field();
    let digging = |u: &Universe| matches!(u.crafts[0].ship.state, ShipState::Anchored { .. }) && u.crafts[0].ship.excavator;
    let mut ticks = 0;
    while !digging(&u) && ticks < 60 * 900 {
        u.step_world(1.0 / 60.0, 1.0, &Controls::default());
        ticks += 1;
    }
    eprintln!("anchored and digging after {} s", ticks / 60);
    assert!(digging(&u), "{:?} {:?}", u.crafts[0].ship.state, u.recorder.incidents);
    assert!(u.crafts[0].ship.hull > 0.99, "it hit nothing on the way");
    // Nearly full: it fills up, lets go, and its route moves on to the market.
    u.crafts[0].ship.cargo = u.crafts[0].ship.spec().hold_capacity - 300.0;
    for _ in 0..60 * 60 {
        u.step_world(1.0 / 60.0, 1.0, &Controls::default());
    }
    let route = u.pilots()[0].avionics.route.clone();
    assert!(u.crafts[0].ship.is_flying() && route.active && route.next == 1, "{:?} {route:?}", u.crafts[0].ship.state);
}

#[test]
fn a_miner_sells_its_ore_at_the_market_then_heads_out_again() {
    use universe_sim::services::{Asset, Party};
    let mut u = miner_by_its_field();
    let me = universe_sim::craft_id(0);
    let home = u.ship_system;
    let market = u.pilots()[0].avionics.route.stops[1].target;
    let NavTarget::Station(station) = market else { panic!("a station market") };
    // Docked at its market with ten tonnes of ore, the route at that stop.
    u.crafts[0].ship = u.world.ship_on(home, Facility::Station(station), 0);
    let ore = universe_sim::world::goods::Ore::Carbonaceous.item();
    u.ledger.settle(Party::Pilot(me), Asset::Goods(ore), 10.0, u.tick, universe_sim::protocol::Cause::Rules);
    u.crafts[0].ship.cargo = 10_000.0;
    u.pilots()[0].avionics.route.next = 1;
    let credits = u.craft_credits(0);
    for _ in 0..60 * 30 {
        u.step_world(1.0 / 60.0, 1.0, &Controls::default());
    }
    assert!(u.ledger.hold(me).is_empty(), "sold: {:?}", u.ledger.hold(me));
    assert!(u.craft_credits(0) > credits + 100.0, "paid {}", u.craft_credits(0) - credits);
    let route = u.pilots()[0].avionics.route.clone();
    assert!(route.active && route.next == 0 && matches!(route.stops[0].target, NavTarget::Asteroid(_)), "a new trip: {route:?}");
}

#[test]
fn a_pilot_closes_on_a_rock_and_anchors() {
    use universe_sim::world::{mining, ShipCommands};
    let mut u = bench(0);
    let (sys, _) = positions(&mut u);
    let f = 0;
    let bodies = sys.field_bodies(f);
    let i = sys.field_rocks(f).find(|&i| i != sys.fields[f].body && bodies[i].rail.radius > 20.0).expect("a rock");
    let t = u.world.time;
    let (c, v) = sys.field_body_state(f, i, t);
    place_player(&mut u, c + DVec3::new(0.6, 0.3, 0.7).normalize() * 1200.0);
    u.ship.velocity = v;
    u.close_on(f, i);
    let ready = |u: &Universe| {
        let (c, v) = sys.field_body_state(f, i, u.world.time);
        let b = &bodies[i];
        let gap = u.ship.position.distance(c) - b.surface_radius_at(c, u.ship.position, u.world.time) - universe_sim::world::ship::SHIP_RADIUS;
        let drift = (u.ship.velocity - v - b.angular_velocity().cross(u.ship.position - c)).length();
        gap < mining::ANCHOR_REACH * 0.8 && drift < 0.6 * mining::ANCHOR_SPEED
    };
    let mut ticks = 0;
    while !ready(&u) && ticks < 60 * 300 {
        u.step_world(1.0 / 60.0, 1.0, &Controls::default());
        ticks += 1;
    }
    eprintln!("in reach after {} s", ticks / 60);
    assert!(ready(&u) && u.ship.hull > 0.99, "{:?}", u.events);
    u.command(&ShipCommands { anchor: Some(true), ..u.ship.holding() });
    for _ in 0..5 {
        u.step_world(1.0 / 60.0, 1.0, &Controls::default());
    }
    assert!(matches!(u.ship.state, ShipState::Anchored { .. }), "{:?}", u.events);
}


#[test]
fn a_pilot_refuels_at_a_station_from_its_stock_and_pays_its_price() {
    let mut u = bench(0);
    let (sys, _) = positions(&mut u);
    let home = u.ship_system;
    let station = sys.station().unwrap();
    u.ship = u.world.ship_on(home, Facility::Station(station), 0);
    let full = u.ship.spec().fuel_capacity;
    u.ship.fuel = full - 4000.0;
    let stock = u.markets.economy.place(home, Facility::Station(station)).unwrap().stock_of(universe_sim::world::goods::Category::fuel());
    let credits = u.credits();
    u.refuel_player();
    assert!((u.ship.fuel - full).abs() < 1e-6, "full: {}", u.ship.fuel);
    let paid = credits - u.credits();
    assert!((paid - 4.0 * universe_sim::services::market::FUEL_PRICE).abs() < 4.0 * 60.0 * 0.5, "paid {paid}");
    let left = u.markets.economy.place(home, Facility::Station(station)).unwrap().stock_of(universe_sim::world::goods::Category::fuel());
    assert!((stock - left - 4.0).abs() < 1e-6, "four tonnes from the station's stock");
    assert!(u.ledger.balanced());
}

#[test]
fn orbit_at_a_chosen_range_no_closer_than_the_structure_allows() {
    use universe_sim::avionics::follow::Manoeuvre;
    let mut u = bench(0);
    let place = universe_sim::NavTarget::Station(u.ship_system().station().expect("a station at home"));
    u.set_nav_target(Some(place));
    u.follow(universe_sim::FollowKind::Orbit, Some(30_000.0));
    assert!(matches!(u.avionics().following.map(|f| f.manoeuvre), Some(Manoeuvre::Orbit(r)) if r == 30_000.0));
    // Closer than a station allows: as close as it does.
    u.follow(universe_sim::FollowKind::Orbit, Some(500.0));
    let r = u.avionics().following.map(|f| f.manoeuvre.range()).unwrap();
    assert!(r > 500.0 && r < 5_000.0, "{r}");
}

#[test]
fn a_long_stay_at_a_port_or_station_is_spent_in_its_hangar_and_the_pad_freed() {
    use universe_sim::world::spaceport::PADS;
    let station = bench(1).ship_system().station().unwrap();
    for (port, then) in [(Facility::Spaceport(0), NavTarget::Station(station)), (Facility::Station(station), NavTarget::Spaceport(0))] {
        let mut u = bench(1);
        let home = u.ship_system;
        u.crafts[0].ship = u.world.ship_on(home, port, 2);
        {
            let mut pilots = u.pilots();
            let r = &mut pilots[0].avionics.route;
            r.stops = vec![universe_sim::Stop { system: home, target: port }, universe_sim::Stop { system: home, target: then }];
            r.next = 0;
            r.active = true;
            r.dwell_until = None;
            r.stay = Some(200.0);
        }
        let held = |u: &Universe| (0..PADS).filter(|&k| u.atc.owners(home, port)[k] == Some(universe_sim::craft_id(0))).count();
        run(&mut u, 5.0, |_| false);
        assert_eq!(held(&u), 1, "{port:?}: on its pad");
        // After the turnaround (time to trade): it taxis off, the pad freed at
        // once; then in the hangar, out of sight.
        let taxiing = run(&mut u, 70.0, |u| u.crafts[0].ship.taxi.is_some());
        assert!(taxiing, "{port:?}: taxiing to the hangar");
        run(&mut u, 2.0, |_| false);
        assert_eq!(held(&u), 0, "{port:?}: its pad freed for others");
        let inside = run(&mut u, 90.0, |u| u.crafts[0].ship.hangar.is_some());
        assert!(inside, "{port:?}: into the hangar");
        assert!(matches!(u.crafts[0].ship.state, ShipState::Landed { .. }));
        // Its stay up: out onto a pad traffic control gives, and off.
        let gone = run(&mut u, 200.0, |u| u.crafts[0].ship.is_flying());
        assert!(gone, "{port:?}: out of the hangar and away (hangar {:?})", u.crafts[0].ship.hangar);
        assert!(u.crafts[0].ship.hangar.is_none());
        assert!(u.recorder.incidents.is_empty(), "{port:?}: no wrecks\n{}", incidents(&u));
    }
}

#[test]
fn a_ship_is_refitted_at_a_station_and_what_it_carries_counts() {
    use universe_sim::world::content::content;
    let mut u = bench(0);
    let home = u.ship_system;
    let station = u.ship_system().station().unwrap();
    let m = |k: &str| content().handle::<universe_sim::world::modules::Module>(k).unwrap();
    // Not docked: refused.
    assert!(u.refit("cargo", Some(m("rack.s2"))).is_err());
    u.ship = u.world.ship_on(home, Facility::Station(station), 0);
    let credits = u.credits();
    // Smaller racks: lighter, a smaller hold, and the old racks sold back;
    // at this station's price (its maker's home is some gates off), built
    // from the station's machinery.
    use universe_sim::services::outfitter;
    let here = Facility::Station(station);
    let settled = outfitter::settled(&u.markets.economy.places);
    let offer = outfitter::offer(u.world.galaxy.seed, &u.world.gate_links, &settled, home, here, content().get(m("rack.s2")));
    let machinery = |u: &Universe| u.markets.economy.place(home, here).unwrap().stock_of(universe_sim::world::goods::Category::of("goods.machinery").unwrap());
    let before = machinery(&u);
    let cost = u.refit("cargo", Some(m("rack.s2"))).unwrap();
    assert!((cost - (offer.price - 0.6 * 3000.0)).abs() < 1e-6, "{cost} at {} hops", offer.hops);
    assert!((machinery(&u) - (before - 0.8 + 0.75)).abs() < 1e-6, "0.8 t built, half the old 1.5 t back");
    assert!((u.credits() - (credits - cost)).abs() < 1e-6);
    assert_eq!(u.ship.spec().hold_capacity, 10_000.0);
    assert_eq!(u.ship.spec().dry_mass, 60_000.0 - 1500.0 + 800.0);
    // The gun out: it doesn't fire.
    u.refit("hardpoint_1", None).unwrap();
    assert!(!u.ship.spec().has(universe_sim::world::modules::Gear::Gun));
    // A base block can't go: the plant.
    let e = u.refit("power", None).unwrap_err();
    assert!(e.contains("Power"), "{e}");
    // A basic nav computer: it docks and lands, but runs no route.
    u.refit("avionics", Some(m("nav.basic.s1"))).unwrap();
    assert!(u.ship.spec().runs(universe_sim::world::modules::Feature::Docking) && !u.ship.spec().runs(universe_sim::world::modules::Feature::Route));
    // Saved and loaded, the fit stays.
    let json = serde_json::to_string(&u.save()).unwrap();
    assert!(json.contains("nav.basic.s1"));
    let mut back = bench(0);
    back.load(serde_json::from_str(&json).unwrap());
    assert_eq!(back.ship.spec().hold_capacity, 10_000.0);
    assert!(!back.ship.spec().runs(universe_sim::world::modules::Feature::Route));
}

#[test]
fn a_ship_is_bought_at_a_station_trading_in_the_old_one() {
    use universe_sim::services::{Asset, Party};
    use universe_sim::world::content::content;
    let mut u = bench(0);
    let home = u.ship_system;
    let station = u.ship_system().station().unwrap();
    let hauler = content().handle("hull.hauler").unwrap();
    assert!(u.buy_hull(hauler).is_err(), "not docked");
    u.ship = u.world.ship_on(home, Facility::Station(station), 0);
    u.ledger.settle(Party::Pilot(universe_sim::PLAYER), Asset::Credits, 500_000.0, u.tick, universe_sim::protocol::Cause::Rules);
    u.ship.cargo = 3_000.0;
    let (price, trade_in) = u.hull_offer(universe_sim::PLAYER, hauler).unwrap();
    let metals = |u: &Universe| u.markets.economy.place(home, Facility::Station(station)).unwrap().stock_of(universe_sim::world::goods::Category::of("goods.metals").unwrap());
    let before = metals(&u);
    let cost = u.buy_hull(hauler).unwrap();
    assert!((cost - (price - trade_in)).abs() < 1e-6 && trade_in > 0.0, "{cost} = {price} - {trade_in}");
    assert!((u.credits() - (500_000.0 - cost)).abs() < 1e-6);
    assert_eq!(u.ship.class, hauler);
    assert_eq!(u.ship.spec().hold_capacity, 150_000.0);
    assert_eq!(u.ship.cargo, 3_000.0, "the cargo moved over");
    assert!((before - metals(&u) - 118.0).abs() < 1e-6, "its frame built from the station's metals");
    assert!(u.buy_hull(hauler).is_err(), "that's the ship we have");
}

#[test]
fn a_hull_is_mended_at_a_station_and_a_lost_ship_is_insured() {
    use universe_sim::services::{Asset, Party};
    use universe_sim::world::content::content;
    let mut u = bench(0);
    let home = u.ship_system;
    let station = u.ship_system().station().unwrap();
    u.ship = u.world.ship_on(home, Facility::Station(station), 0);
    let me = Party::Pilot(universe_sim::PLAYER);
    let cause = universe_sim::protocol::Cause::Rules;
    // Half a hull, and the credits for a fifth of it: mended a fifth.
    u.ship.hull = 0.5;
    let full = u.ship.spec().frame.price * 0.3;
    u.ledger.settle(me, Asset::Credits, full * 0.2, u.tick, cause);
    let (cost, hull) = u.repair(universe_sim::PLAYER).unwrap();
    assert!((cost - full * 0.2).abs() < 1e-6 && (hull - 0.7).abs() < 1e-9, "{cost} {hull}");
    // Rich: mended whole.
    u.ledger.settle(me, Asset::Credits, 1e6, u.tick, cause);
    assert_eq!(u.repair(universe_sim::PLAYER).unwrap().1, 1.0);
    // An interceptor lost: the same again, for the excess.
    let interceptor = content().handle("hull.interceptor").unwrap();
    u.ship.class = interceptor;
    let value = Universe::ship_value(&u.ship);
    let before = u.credits();
    u.respawn();
    run(&mut u, universe_sim::world::damage::RESPAWN_TIME + 1.0, |_| false);
    assert_eq!(u.ship.class, interceptor);
    assert!((u.credits() - (before - 0.1 * value)).abs() < 1.0, "paid the excess: {} of {before}", u.credits());
    // Broke, lost again: a basic ship.
    u.ledger.settle(me, Asset::Credits, 0.0, u.tick, cause);
    u.respawn();
    run(&mut u, universe_sim::world::damage::RESPAWN_TIME + 1.0, |_| false);
    assert_eq!(u.ship.class, universe_sim::world::ship::starting_hull());
}

#[test]
fn a_cola_from_the_vending_machine_between_the_pads() {
    use universe_sim::world::crew::Reach;
    let mut u = bench(0);
    let home = u.ship_system;
    let port = 0;
    u.ship = u.world.ship_on(home, Facility::Spaceport(port), 5);
    let sys = u.ship_system();
    let body = sys.spaceports[port].body;
    let d = universe_sim::world::spaceport::vending_direction(&sys, port);
    let (north, _) = universe_sim::world::spaceport::tangent(d);
    // Away from it: no machine to hand.
    let far = (d * sys.bodies[body].rail.radius + north * 60.0).normalize();
    u.crew.place = universe_sim::world::Place::Outside { body, position: far * sys.bodies[body].surface_radius(far), velocity: DVec3::ZERO, yaw: 0.0, pitch: 0.0 };
    let before = u.credits();
    u.vend(0);
    assert_eq!(u.credits(), before, "nothing bought out of reach");
    // By it: a cola, paid to the port's market.
    let near = (d * sys.bodies[body].rail.radius + north * 2.0).normalize();
    u.crew.place = universe_sim::world::Place::Outside { body, position: near * sys.bodies[body].surface_radius(near), velocity: DVec3::ZERO, yaw: 0.0, pitch: 0.0 };
    assert_eq!(u.pilot_reach(), Some(Reach::Vending(port)));
    u.vend(0);
    assert!((before - u.credits() - 2.0).abs() < 1e-9, "a cola for 2 CR");
    assert!(u.events.iter().any(|e| matches!(e, universe_sim::avionics::Event::Vended { what, .. } if what == "COLA")));
}

#[test]
fn manual_thrusters_fire_only_what_is_held_and_nothing_steadies_the_ship() {
    use universe_sim::world::ship::ThrusterRole;
    use universe_sim::world::ShipCommands;
    let mut u = bench(0);
    let spec = u.ship.spec();
    let k = spec.thrusters.iter().position(|t| t.nozzle.ends_with("nose_left_side")).expect("a nose thruster");
    let mains: u64 = spec.thrusters.iter().enumerate().filter(|(_, t)| t.role == ThrusterRole::Main).map(|(i, _)| 1u64 << i).sum();
    u.ship.angular_velocity = DVec3::ZERO;
    u.command(&ShipCommands { manual: Some(true), ..u.ship.holding() });
    // The stick does nothing now.
    u.step_world(1.0 / 60.0, 1.0, &Controls { pitch: 1.0, yaw: 1.0, roll: 1.0 });
    assert!(u.ship.angular_velocity.length() < 1e-9, "no stick: {}", u.ship.angular_velocity);
    // One nose thruster held: it alone fires, and the ship yaws.
    u.command(&ShipCommands { jets: Some(1 << k), ..u.ship.holding() });
    for _ in 0..30 {
        u.step_world(1.0 / 60.0, 1.0, &Controls::default());
    }
    assert!(u.ship.jets.iter().enumerate().all(|(i, &j)| (j > 0.0) == (i == k)), "{:?}", u.ship.jets);
    assert!(u.ship.angular_velocity.length() > 1e-3, "turning: {}", u.ship.angular_velocity);
    // Let go (the order reaching it a tick on): nothing steadies it; it turns on as it was.
    u.command(&ShipCommands { jets: Some(0), ..u.ship.holding() });
    for _ in 0..3 {
        u.step_world(1.0 / 60.0, 1.0, &Controls::default());
    }
    let spin = u.ship.angular_velocity;
    for _ in 0..30 {
        u.step_world(1.0 / 60.0, 1.0, &Controls::default());
    }
    assert!((u.ship.angular_velocity - spin).length() < 1e-6 * spin.length().max(1.0) + 1e-9, "{} vs {spin}", u.ship.angular_velocity);
    // The mains held: it speeds up along its nose.
    let v0 = u.ship.velocity;
    let nose = u.ship.orientation * DVec3::NEG_Z;
    u.command(&ShipCommands { jets: Some(mains), ..u.ship.holding() });
    for _ in 0..60 {
        u.step_world(1.0 / 60.0, 1.0, &Controls::default());
    }
    assert!((u.ship.velocity - v0).dot(nose) > 5.0, "mains push: {}", (u.ship.velocity - v0).dot(nose));
    // Back on the flight computer: nothing held.
    u.command(&ShipCommands { manual: Some(false), ..u.ship.holding() });
    for _ in 0..3 {
        u.step_world(1.0 / 60.0, 1.0, &Controls::default());
    }
    assert!(!u.ship.manual && u.ship.held == 0);
}

#[test]
fn a_ship_balanced_at_the_yard_goes_straight_on_its_mains_alone() {
    use universe_sim::world::ship::ThrusterRole;
    use universe_sim::world::ShipCommands;
    // Mains alone from a standstill, manual, for two seconds: how fast it's turning then.
    let spin_after = |trimmed: bool| {
        let mut u = bench(0);
        let station = u.ship_system().station().unwrap();
        let home = u.ship_system;
        u.ship = u.world.ship_on(home, Facility::Station(station), 0);
        assert!(u.set_trim(universe_sim::world::trim::Trim::default()).is_ok(), "docked: the yard trims it");
        if trimmed {
            let s = u.ship.spec();
            let t = universe_sim::world::trim::balance(s, u.ship.fuel, u.ship.cargo);
            u.set_trim(t).unwrap();
        }
        // Out in free space, still.
        u.respawn();
        u.step_world(1.0 / 60.0, 1.0, &Controls::default());
        u.ship.position += DVec3::new(0.0, 0.0, 5.0e7);
        u.ship.angular_velocity = DVec3::ZERO;
        let mains: u64 = u.ship.spec().thrusters.iter().enumerate().filter(|(_, t)| t.role == ThrusterRole::Main).map(|(i, _)| 1u64 << i).sum();
        u.command(&ShipCommands { manual: Some(true), ..u.ship.holding() });
        u.command(&ShipCommands { jets: Some(mains), ..u.ship.holding() });
        for _ in 0..120 {
            u.step_world(1.0 / 60.0, 1.0, &Controls::default());
        }
        u.ship.angular_velocity.length()
    };
    let (off, on) = (spin_after(false), spin_after(true));
    eprintln!("turning after 2 s on the mains: untrimmed {off:.5} rad/s, trimmed {on:.7} rad/s");
    assert!(off > 1e-3, "untrimmed, it turns: {off}");
    assert!(on < off * 0.02, "trimmed, it goes straight: {on} against {off}");
}

#[test]
fn goods_bought_where_made_sell_dearer_where_needed() {
    use universe_sim::world::goods::Category;
    let mut u = bench(0);
    let home = u.ship_system;
    let sys = u.ship_system();
    let food = Category::of("goods.food").unwrap();
    // A farm world that makes food, a station that needs it: both at their usual stock.
    let places = u.markets.economy.places.clone();
    let maker = places.iter().find(|p| p.system == home && p.sells(food)).expect("a farm at home");
    let user = places.iter().find(|p| p.system == home && !p.sells(food) && p.needs(food) > 0.0).expect("somewhere that needs food");
    let item = u.markets.goods().iter().position(|g| g.category == food).unwrap();
    let now = u.world.time;
    let bought = u.markets.quote_for(home, &sys, maker.facility, item, now).and_then(|q| q.buy).expect("the farm sells food");
    let sold = u.markets.quote_for(home, &sys, user.facility, item, now).map(|q| q.sell).expect("the station buys food");
    eprintln!("food: bought at {bought:.0}, sold at {sold:.0} ({:+.0}%)", 100.0 * (sold / bought - 1.0));
    assert!(sold > bought * 1.4, "hauling it pays: {bought} -> {sold}");
    // Sold back where it was bought: a loss.
    let back = u.markets.quote_for(home, &sys, maker.facility, item, now).unwrap().sell;
    assert!(back < bought);
}

#[test]
fn passengers_book_passage_board_a_cabin_and_settle_where_they_booked_for_the_fare() {
    use universe_sim::world::content::content;
    let mut u = bench(0);
    let home = u.ship_system;
    let station = u.ship_system().station().unwrap();
    let here = Facility::Station(station);
    // The station hungry: a thousand of its people waiting to leave.
    {
        let p = u.markets.economy.place_mut(home, here).unwrap();
        p.fed = 0.6;
        p.waiting = 1.0;
    }
    let bookings = u.bookings(home, here);
    assert!(!bookings.is_empty(), "they book passage somewhere fed");
    let b = bookings[0];
    let to = (b.system, b.to);
    // Docked without a cabin: no seats.
    u.ship = u.world.ship_on(home, here, 0);
    assert!(u.board_passengers(universe_sim::PLAYER, here, to).is_err(), "no cabin, no passengers");
    // A cabin in a cargo slot: 30 seats.
    u.refit("cargo", Some(content().handle("cabin.s3").unwrap())).unwrap();
    let n = u.board_passengers(universe_sim::PLAYER, here, to).unwrap();
    assert_eq!(n, 30.min(b.people));
    assert_eq!(u.ship.passengers, n);
    let waiting = u.markets.economy.place(home, here).unwrap().waiting;
    assert!((waiting - (1.0 - n as f64 / 1000.0)).abs() < 1e-9, "they left the station");
    // Landed anywhere else: refused. Where they booked: they settle, and the fare's paid.
    assert!(u.land_passengers(universe_sim::PLAYER, here).is_err());
    let (people, credits) = (u.markets.economy.place(to.0, to.1).unwrap().population, u.credits());
    u.ship = { let mut s = u.world.ship_on(to.0, to.1, 1); s.class = u.ship.class; s.fit = u.ship.fit.clone(); s.passengers = u.ship.passengers; s.bound_for = u.ship.bound_for; s.fare = u.ship.fare; s.refresh(); s };
    u.ship_system = to.0;
    let paid = u.land_passengers(universe_sim::PLAYER, to.1).unwrap();
    assert!((paid - b.fare * n as f64).abs() < 1e-9);
    assert!((u.credits() - credits - paid).abs() < 1e-6, "the fares paid");
    assert!((u.markets.economy.place(to.0, to.1).unwrap().population - people - n as f64 / 1000.0).abs() < 1e-9, "they settled");
    assert_eq!(u.ship.passengers, 0);
    assert!(u.ledger.balanced());
}
