//! Benchmarks, plan accuracy and debugging printouts, run through the
//! orchestrator (mostly `#[ignore]`d: `cargo test -p universe-sim --release
//! <name> -- --ignored --nocapture`).

use glam::DVec3;
use universe_sim::{docking, Approach, Controls, GateFrame, NavTarget, PadFrame, ShipState, StationFrame, Universe};

/// `cargo test -p universe-sim --release bench_navigation -- --ignored --nocapture`
#[test]
#[ignore]
fn bench_navigation_costs() {
    use std::hint::black_box;
    use std::time::Instant;
    let per = |label: &str, n: u32, f: &mut dyn FnMut()| {
        let t = Instant::now();
        for _ in 0..n {
            f();
        }
        let us = t.elapsed().as_secs_f64() * 1e6 / n as f64;
        eprintln!("{label:<44} {us:>10.2} us");
        us
    };

    let mut u = Universe::new(1984);
    let sys = u.ship_system();
    eprintln!("home system: {} bodies", sys.bodies.len());
    let mut pos = Vec::new();
    let mut t = 0.0;
    per("body positions (all bodies, one time)", 100_000, &mut || {
        t += 1.0;
        sys.positions(black_box(t), &mut pos);
    });
    let p = u.ship.position;
    per("gravity sum at a point", 100_000, &mut || {
        black_box(sys.gravity(black_box(p), &pos));
    });
    let planet = sys.bodies[sys.station().unwrap()].rail.parent.unwrap();
    let body = &sys.bodies[planet];
    let terrain = body.terrain.as_ref().unwrap();
    let mut d = DVec3::new(0.3, 0.4, 0.5).normalize();
    per("terrain height lookup", 100_000, &mut || {
        d = (d + DVec3::splat(1e-4)).normalize();
        black_box(terrain.surface(d));
    });

    let frame = 1.0 / 60.0;
    let frames = 600;
    let run = |label: &str, u: &mut Universe, warp: f64| {
        let t = Instant::now();
        for _ in 0..frames {
            u.step(frame, warp, &Controls::default());
        }
        let us = t.elapsed().as_secs_f64() * 1e6 / frames as f64;
        eprintln!("{label:<44} {us:>10.2} us/frame");
    };

    let mut v = Universe::new(1984);
    v.ship.position += DVec3::new(0.0, 0.0, 200_000.0); // away from the station's fine stepping
    run("ship coasting in orbit, warp 1", &mut v, 1.0);
    run("ship coasting in orbit, warp 1000", &mut v, 1000.0);
    let mut v = Universe::new(1984);
    run("ship near a station (fine steps), warp 1", &mut v, 1.0);
    let mut v = Universe::new(1984);
    v.toggle_autopilot();
    run("docking autopilot, warp 1", &mut v, 1.0);
    let mut v = Universe::new(1984);
    let port = v.ship_system().spaceports.iter().position(|sp| sp.body == planet).unwrap();
    v.set_nav_target(Some(NavTarget::Spaceport(port)));
    v.toggle_autopilot();
    run("landing autopilot, warp 1", &mut v, 1.0);
    run("landing autopilot, warp 100", &mut v, 100.0);
    let mut v = Universe::new(1984);
    let (dest, _) = v.gate_links_of(v.world.home_system)[0].clone();
    let g = v.ship_system().gate_to(dest).unwrap();
    v.set_nav_target(Some(NavTarget::Gate(g)));
    v.toggle_hyperdrive();
    v.ship.throttle = 0.3;
    run("hyperdrive (targeted), warp 1", &mut v, 1.0);
    let mut v = Universe::new(1984);
    let sysv = v.ship_system();
    sysv.positions(v.world.time, &mut pos);
    let up = (v.ship.position - pos[planet]).normalize();
    v.ship.position = pos[planet] + up * (sysv.bodies[planet].surface_radius_at(pos[planet], pos[planet] + up, v.world.time) + 3000.0);
    v.ship.velocity = sysv.velocity(planet, v.world.time) + sysv.bodies[planet].angular_velocity().cross(v.ship.position - pos[planet]);
    v.ship.throttle = 0.4;
    v.ship.orientation = universe_sim::ship::facing(up, up.any_orthonormal_vector());
    run("near the ground (terrain collision), warp 1", &mut v, 1.0);

    let mut v = Universe::new(1984);
    v.set_nav_target(Some(NavTarget::Spaceport(port)));
    v.request_clearance();
    per("flight planner (landing from orbit)", 50, &mut || {
        black_box(v.plan());
    });
    let mut v = Universe::new(1984);
    v.request_clearance();
    per("flight planner (docking)", 50, &mut || {
        black_box(v.plan());
    });
    let _ = &mut u;
}

/// Plan ETA vs. how long the autopilot really takes.
/// `cargo test -p universe-sim --release eta_accuracy -- --ignored --nocapture`
#[test]
#[ignore]
fn eta_accuracy() {
    let run = |label: &str, mut u: Universe, done: &dyn Fn(&Universe) -> bool| {
        u.toggle_autopilot();
        let start = u.world.time;
        let eta0 = u.plan().map(|p| p.points.last().unwrap().time).unwrap_or(f64::NAN);
        let mut samples = Vec::new();
        for i in 0..(60 * 60 * 30) {
            if i % 600 == 0
                && let Some(p) = u.plan()
            {
                samples.push((u.world.time - start, u.world.time - start + p.points.last().unwrap().time));
            }
            u.step(1.0 / 60.0, 5.0, &Controls::default());
            if done(&u) {
                break;
            }
        }
        let actual = u.world.time - start;
        eprintln!("{label}: planned {eta0:.0} s, actual {actual:.0} s ({:+.0}%)", (actual / eta0 - 1.0) * 100.0);
        for (t, arrive) in samples.iter().step_by((samples.len() / 8).max(1)) {
            eprintln!("    at {t:>6.0} s the plan said arrival at {arrive:>6.0} s");
        }
    };
    let landed = |u: &Universe| matches!(u.ship.state, ShipState::Landed { .. });
    run("docking from spawn", Universe::new(1984), &landed);

    let mut u = Universe::new(1984);
    let sys = u.ship_system();
    let planet = sys.bodies[sys.station().unwrap()].rail.parent.unwrap();
    let port = sys.spaceports.iter().position(|p| p.body == planet).unwrap();
    u.set_nav_target(Some(NavTarget::Spaceport(port)));
    run("landing from orbit", u, &landed);

    let mut u = Universe::new(1984);
    let home = u.world.home_system;
    let sys = u.ship_system();
    let (dest, _) = u.gate_links_of(home)[0].clone();
    let g = sys.gate_to(dest).unwrap();
    let mut pos = Vec::new();
    sys.positions(u.world.time, &mut pos);
    let frame = GateFrame::new(&sys, g, u.world.time, &pos);
    u.ship.position = frame.center + (frame.rotation * DVec3::new(1.0, 0.5, 0.3)).normalize() * 30_000.0;
    u.ship.velocity = frame.velocity;
    u.set_nav_target(Some(NavTarget::Gate(g)));
    run("gate from 30 km", u, &|u: &Universe| matches!(u.ship.state, ShipState::Transit { .. }));
}

/// Reproduce the HUD's ETA countdown frame by frame (60 fps, plan rebuilt
/// 10×/s) and return the biggest frame-to-frame jump (real seconds).
fn eta_jumps(label: &str, mut u: Universe, warp: f64, max_frames: usize) -> f64 {
    u.toggle_autopilot();
    // No plan (and no ETA) while the autopilot is in hyperdrive; measure the rest.
    let mut plan = u.plan();
    let mut shown: Vec<f64> = Vec::new();
    for frame in 0..max_frames {
        if frame % 6 == 0 {
            plan = u.plan();
            if let Some(p) = &plan {
                assert!(p.arrives, "{label}: a plan failed to arrive at frame {frame}");
            } else {
                shown.clear(); // a hyperjump: the countdown starts fresh afterwards
            }
        }
        if let Some(p) = &plan {
            let left = p.points.last().unwrap().time - (u.world.time - p.start);
            shown.push(left / warp);
        }
        u.step(1.0 / 60.0, warp, &Controls::default());
        if !u.ship.is_flying() {
            break;
        }
    }
    let worst = shown.windows(2).map(|w| (w[1] - w[0]).abs()).fold(0.0, f64::max);
    eprintln!("{label}: {} frames, first ETA {:.0} s, biggest jump {worst:.2} s", shown.len(), shown.first().copied().unwrap_or(0.0));
    worst
}

#[test]
fn eta_counts_down_smoothly() {
    // Docking at the default 2x.
    assert!(eta_jumps("docking", Universe::new(1984), 2.0, 60 * 120) < 0.5);
    // A gate from 30 km.
    let mut u = Universe::new(1984);
    let home = u.world.home_system;
    let sys = u.ship_system();
    let (dest, _) = u.gate_links_of(home)[0].clone();
    let g = sys.gate_to(dest).unwrap();
    let mut pos = Vec::new();
    sys.positions(u.world.time, &mut pos);
    let frame = GateFrame::new(&sys, g, u.world.time, &pos);
    u.ship.position = frame.center + (frame.rotation * DVec3::new(1.0, 0.5, 0.3)).normalize() * 30_000.0;
    u.ship.velocity = frame.velocity;
    u.set_nav_target(Some(NavTarget::Gate(g)));
    assert!(eta_jumps("gate", u, 2.0, 60 * 200) < 0.5);
}

#[test]
#[ignore]
fn eta_counts_down_smoothly_landing() {
    let mut u = Universe::new(1984);
    let sys = u.ship_system();
    let planet = sys.bodies[sys.station().unwrap()].rail.parent.unwrap();
    let port = sys.spaceports.iter().position(|p| p.body == planet).unwrap();
    u.set_nav_target(Some(NavTarget::Spaceport(port)));
    // Warp 20 to keep the run short; jumps are measured in real seconds at that rate.
    let worst = eta_jumps("landing", u, 20.0, 60 * 60 * 10);
    assert!(worst < 5.0);
}

#[test]
#[ignore]
fn debug_hyper_path() {
    let mut u = Universe::new(42);
    let sys = u.ship_system();
    let planet = sys.bodies[sys.station().unwrap()].rail.parent.unwrap();
    let port = sys.spaceports.iter().position(|p| p.body == planet).unwrap();
    u.set_nav_target(Some(NavTarget::Spaceport(port)));
    u.toggle_autopilot();
    for i in 0..(60 * 300) {
        u.step(1.0 / 60.0, 20.0, &Controls::default());
        if i % 60 == 0 && u.ship.hyperdrive {
            let mut pos = Vec::new();
            sys.positions(u.world.time, &mut pos);
            let c = pos[planet];
            let r = sys.bodies[planet].rail.radius;
            let alt = |x: DVec3| (x.distance(c) - r) / 1000.0;
            let way = u.avionics.debug_way.unwrap_or(DVec3::ZERO);
            let pad = PadFrame::new(&sys, port, u.world.time, &pos);
            eprintln!("  {:>4}s alt {:>8.0} km  way alt {:>8.0} km  angle to pad {:>5.1} deg  speed {:>8.0} km/s", i / 60, alt(u.ship.position), alt(way), ((u.ship.position - c).angle_between(pad.up)).to_degrees(), u.ship.velocity.length() / 1000.0);
        }
        if !u.ship.hyperdrive && i > 60 * 5 {
            break;
        }
    }
    eprintln!("R = {:.0} km", sys.bodies[planet].rail.radius / 1000.0);
}

#[test]
#[ignore]
fn debug_landing_start() {
    let mut u = Universe::new(1984);
    let sys = u.ship_system();
    let planet = sys.bodies[sys.station().unwrap()].rail.parent.unwrap();
    let port = sys.spaceports.iter().position(|p| p.body == planet).unwrap();
    u.set_nav_target(Some(NavTarget::Spaceport(port)));
    let mut pos = Vec::new();
    sys.positions(u.world.time, &mut pos);
    let pad = PadFrame::new(&sys, port, u.world.time, &pos);
    u.ship.position = pad.pad + (pad.up + pad.up.any_orthonormal_vector() * 0.5).normalize() * 150_000.0;
    u.ship.velocity = pad.frame_velocity(u.ship.position);
    u.toggle_autopilot();
    let plan = u.plan().unwrap();
    for p in plan.points.iter().take(12) {
        eprintln!("  plan t {:>5.1} {:?} {:?}", p.time, p.phase, p.action);
    }
    for i in 0..600 {
        if i % 30 == 0 {
            let aim = u.approach().and_then(|a| match a { Approach::Land { status, .. } => Some(status.guidance.desired_velocity.length()), _ => None });
            eprintln!("  real t {:>5.1} throttle {:.2} rcs {:.2?} hyper {} want {:?}", i as f64 / 60.0, u.ship.throttle, u.ship.rcs, u.ship.hyperdrive, aim);
        }
        u.step(1.0 / 60.0, 1.0, &Controls::default());
    }
}

#[test]
#[ignore]
fn print_game_landing_plan() {
    let mut u = Universe::new(1984);
    let sys = u.ship_system();
    let planet = sys.bodies[sys.station().unwrap()].rail.parent.unwrap();
    let port = sys.spaceports.iter().position(|p| p.body == planet).unwrap();
    u.set_nav_target(Some(NavTarget::Spaceport(port)));
    assert!(u.request_clearance());
    for frames in [0, 1, 2, 30, 120] {
        let mut v = Universe::new(1984);
        v.set_nav_target(Some(NavTarget::Spaceport(port)));
        v.request_clearance();
        for _ in 0..frames {
            v.step(1.0 / 60.0, 1.0, &Controls::default());
        }
        let p = v.plan().unwrap();
        let l = p.points.last().unwrap();
        eprintln!("after {frames:>3} frames: points {} last t {:.0} arrives {} last phase {:?} action {:?}", p.points.len(), l.time, p.arrives, l.phase, l.action);
    }
    let plan = u.plan().unwrap();
    let last = plan.points.last().unwrap();
    eprintln!("points {} last t {:.0} arrives {} last phase {:?}", plan.points.len(), last.time, plan.arrives, last.phase);
    for p in plan.points.iter().step_by(plan.points.len() / 12 + 1) {
        eprintln!("  t {:>7.0} {:?} {:?}", p.time, p.phase, p.action);
    }
}

#[test]
#[ignore]
fn print_guidance() {
    let mut u = Universe::new(1984);
    let sys = u.ship_system();
    let station = sys.station().unwrap();
    let mut pos = Vec::new();
    sys.positions(u.world.time, &mut pos);
    let f = StationFrame::new(&sys, station, u.world.time, &pos);
    let to_station = |p: DVec3| (f.center - p).normalize();
    let report = |label: &str, p: DVec3| {
        let g = docking::guidance(&f, p, docking::in_final_zone(&f, p), u.ship.side_accel());
        let r = p - f.center;
        eprintln!(
            "{label:10} height {:7.0} lateral {:7.0} | desired {:6.1} m/s, cos(toward station) {:+.2} | waypoint height {:7.0} lateral {:7.0}",
            r.dot(f.axis()),
            (r - f.axis() * r.dot(f.axis())).length(),
            g.desired_velocity.length(),
            g.desired_velocity.normalize_or_zero().dot(to_station(p)),
            (g.waypoint - f.center).dot(f.axis()),
            ((g.waypoint - f.center) - f.axis() * (g.waypoint - f.center).dot(f.axis())).length(),
        );
    };
    report("spawn", u.ship.position);
    report("side 6km", f.center + f.slot_long() * 6000.0);
    report("above 8km", f.on_axis(8000.0));
    report("above 3km", f.on_axis(3000.0) + f.slot_long() * 100.0);
    report("level 2km", f.center + f.slot_short() * 2000.0);
    report("below 3km", f.center - f.axis() * 3000.0);
    report("far 30km", f.center + (f.slot_long() + f.axis() * 0.2).normalize() * 30000.0);
}
