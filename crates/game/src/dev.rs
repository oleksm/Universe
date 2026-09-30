//! Named start-up scenarios for quickly checking features visually:
//! `UNIVERSE_SCENARIO=galaxy UNIVERSE_SCREENSHOT=out.png cargo run`

use universe_engine::glam::DVec3;
use universe_sim::ship::SHIP_RADIUS;
use universe_sim::{BodyKind, Controls, Event, GateFrame, NavTarget, PadFrame, Phase, ShipCommands, ShipEvent, ShipState, StationFrame};

use crate::observer::Focus;
use crate::{App, Mode};

pub const SCENARIOS: &str = "system inner planet giant rings galaxy neighbours cockpit hyper landed cleared approach offcourse autodock docked lost navmap landing padview autoland touchdown gate gateauto transit gatearrive network lowflight moon routemap route traffic follow radar contacts gunnery";

pub fn apply(app: &mut App, name: &str) {
    let home = app.u.world.home_system;
    let sys = app.u.system(home);
    let t = app.u.world.time;
    let mut positions = Vec::new();
    sys.positions(t, &mut positions);
    let observe = |app: &mut App, body: usize, distance: f64, pitch: f64| {
        app.mode = Mode::Observer;
        app.observer.focus = Focus::Body { system: home, body };
        app.observer.distance = distance;
        app.observer.pitch = pitch;
    };
    let outer = sys.bodies.iter().filter_map(|b| b.rail.orbit.as_ref().filter(|_| b.rail.parent == Some(0))).map(|o| o.apoapsis()).fold(0.0, f64::max);
    let station = sys.station().unwrap_or(0);
    let planet = sys.bodies[station].rail.parent.unwrap_or(0);

    match name {
        "system" => observe(app, 0, outer * 2.2, 0.6),
        "inner" => observe(app, 0, outer * 0.25, 0.45),
        "planet" => observe(app, planet, sys.bodies[planet].rail.radius * 6.0, 0.3),
        "giant" => {
            let giant = sys.bodies.iter().position(|b| b.rings.is_some()).or_else(|| sys.bodies.iter().position(|b| b.kind == BodyKind::GasGiant)).unwrap_or(0);
            observe(app, giant, sys.bodies[giant].rail.radius * 7.0, 0.35);
        }
        "rings" => {
            // Nearest ringed planet in the neighbourhood.
            for n in std::iter::once(home).chain(app.u.world.galaxy.nearest(home, 60)) {
                let s = app.u.system(n);
                if let Some(i) = s.bodies.iter().position(|b| b.rings.is_some()) {
                    app.mode = Mode::Observer;
                    app.observer.focus = Focus::Body { system: n, body: i };
                    app.observer.distance = s.bodies[i].rail.radius * 6.0;
                    app.observer.pitch = 0.3;
                    break;
                }
            }
        }
        "galaxy" => observe(app, 0, 2.5e20, 1.0),
        "neighbours" => observe(app, 0, 6.0e17, 0.5),
        "cockpit" => {
            app.mode = Mode::Pilot;
            app.chase_cam = false;
        }
        "hyper" => {
            // Aim at a nearby star on the open-sky side of the planet, and jump.
            app.mode = Mode::Pilot;
            let up = (app.u.ship.position - positions[planet]).normalize();
            let dir_to = |n: usize| (app.u.world.galaxy.offset(home, n) - app.u.ship.position).normalize();
            let candidates = app.u.world.galaxy.nearest(home, 12);
            let target = candidates.iter().copied().find(|&n| dir_to(n).dot(up) > 0.3).unwrap_or(candidates[0]);
            let dir = dir_to(target);
            app.u.ship.orientation = universe_engine::glam::DQuat::from_rotation_arc(DVec3::NEG_Z, dir);
            app.u.toggle_hyperdrive();
            app.u.command(&ShipCommands { throttle: 1.0, ..app.u.ship.holding() });
            for _ in 0..4000 {
                app.u.step(1.0 / 60.0, 1.0, &Controls::default());
                if !app.u.ship.hyperdrive {
                    break;
                }
            }
        }
        "landed" => {
            // Drop the ship just above the station's planet, matching its surface motion.
            app.mode = Mode::Pilot;
            let b = &sys.bodies[planet];
            let normal = (app.u.ship.position - positions[planet]).normalize();
            let offset = normal * (b.rail.radius + SHIP_RADIUS + 3.0);
            app.u.ship.position = positions[planet] + offset;
            app.u.ship.velocity = sys.velocity(planet, t) + b.angular_velocity().cross(offset);
            app.u.ship.orientation = universe_engine::glam::DQuat::from_rotation_arc(DVec3::NEG_Z, normal.any_orthonormal_vector());
            for _ in 0..120 {
                app.u.step(1.0 / 60.0, 1.0, &Controls::default());
            }
        }
        "approach" | "lost" => {
            // Cleared to dock, flying manually. "approach": 2.5 km out, a little off the axis, facing in.
            // "lost": facing away from the station, to show the off-screen marker.
            app.mode = Mode::Pilot;
            let f = StationFrame::new(&sys, station, t, &positions);
            app.u.ship.position = f.on_axis(2500.0) + f.slot_long() * 180.0 + f.slot_short() * 60.0;
            app.u.ship.velocity = f.velocity - f.axis() * 30.0;
            let facing = if name == "lost" { f.axis() } else { -f.axis() };
            let roll = universe_engine::glam::DQuat::from_axis_angle(facing, 0.35);
            app.u.ship.orientation = roll * universe_engine::glam::DQuat::from_rotation_arc(DVec3::NEG_Z, facing);
            if name == "approach" {
                app.u.request_clearance();
            }
        }
        "cleared" => {
            // The spawn point, with docking clearance granted.
            app.mode = Mode::Pilot;
            app.u.request_clearance();
        }
        "offcourse" => {
            // Cleared, but 6 km off to the side and drifting sideways at 40 m/s.
            app.mode = Mode::Pilot;
            let f = StationFrame::new(&sys, station, t, &positions);
            app.u.ship.position = f.on_axis(2000.0) + f.slot_long() * 6000.0;
            app.u.ship.velocity = f.velocity + f.slot_short() * 40.0;
            let look = (f.center - app.u.ship.position).normalize();
            app.u.ship.orientation = universe_engine::glam::DQuat::from_rotation_arc(DVec3::NEG_Z, look);
            app.u.request_clearance();
        }
        "navmap" => {
            app.mode = Mode::Pilot;
            app.nav_map = Some(crate::navmap::NavMap::open(app));
        }
        "landing" | "padview" | "autoland" | "touchdown" => {
            // Target the spaceport on the station's planet and get landing clearance.
            app.mode = Mode::Pilot;
            let port = sys.spaceports.iter().position(|p| p.body == planet).expect("spaceport on the station's planet");
            let pad = PadFrame::new(&sys, port, t, &positions);
            if name == "padview" {
                // 12 km above and 6 km beside the pad, drifting, belly down.
                app.u.ship.position = pad.pad + pad.up * 12_000.0 + pad.up.any_orthonormal_vector() * 6000.0;
                app.u.ship.velocity = pad.frame_velocity(app.u.ship.position) - pad.up * 40.0;
                let fwd = pad.up.any_orthonormal_vector().cross(pad.up);
                app.u.ship.orientation = universe_sim::ship::upright(pad.up, fwd);
            }
            app.u.set_nav_target(Some(NavTarget::Spaceport(port)));
            app.u.request_clearance();
            if name == "autoland" || name == "touchdown" {
                app.u.toggle_autopilot();
                for _ in 0..60 * 60 * 30 {
                    app.u.step(1.0 / 60.0, 20.0, &Controls::default());
                    let low = matches!(app.u.approach(), Some(universe_sim::Approach::Land { ref status, .. })
                        if status.phase == Phase::Descent && status.altitude < 1500.0);
                    if (name == "autoland" && low) || !app.u.ship.is_flying() {
                        break;
                    }
                }
            }
        }
        "gate" | "gateauto" | "transit" | "gatearrive" => {
            // A gate out of the home system: cleared for transit, 8 km out, off to one side.
            app.mode = Mode::Pilot;
            let (dest, _) = app.u.gate_links_of(home)[0].clone();
            let g = sys.gate_to(dest).unwrap();
            let f = GateFrame::new(&sys, g, t, &positions);
            app.u.ship.position = f.center + f.axis() * 8000.0 + (f.rotation * DVec3::X) * 2500.0;
            app.u.ship.velocity = f.velocity;
            let look = (f.center - app.u.ship.position).normalize();
            app.u.ship.orientation = universe_sim::ship::facing(look, f.rotation * DVec3::Z);
            app.u.set_nav_target(Some(NavTarget::Gate(g)));
            app.u.request_clearance();
            if name != "gate" {
                app.u.toggle_autopilot();
                for _ in 0..60 * 60 * 10 {
                    app.u.step(1.0 / 60.0, 10.0, &Controls::default());
                    let running = matches!(app.u.approach(), Some(universe_sim::Approach::Transit { ref status, .. })
                        if status.phase == Phase::Final && status.distance < 1500.0);
                    let stop = match name {
                        "gateauto" => running,
                        "transit" => matches!(app.u.ship.state, ShipState::Transit { remaining, .. } if remaining < 3.5),
                        _ => app.u.ship_system != home && app.u.ship.is_flying(),
                    };
                    if stop {
                        break;
                    }
                }
            }
        }
        "lowflight" => {
            // Skimming 6 km over the home planet, 800 km from the port, looking ahead.
            app.mode = Mode::Pilot;
            let b = &sys.bodies[planet];
            let rot = b.rotation(t);
            // Find mountains: the highest of a few hundred spots in a wide patch.
            let start = rot.inverse() * (app.u.ship.position - positions[planet]).normalize();
            let terrain = b.terrain.as_ref().unwrap();
            let mut rng = 1u64;
            let mut best = (f64::MIN, start);
            for _ in 0..600 {
                rng = rng.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
                let jitter = DVec3::new(((rng >> 11) % 1000) as f64 - 500.0, ((rng >> 23) % 1000) as f64 - 500.0, ((rng >> 37) % 1000) as f64 - 500.0) / 1500.0;
                let d = (start + jitter).normalize();
                let h = terrain.surface(d);
                if h > best.0 {
                    best = (h, d);
                }
            }
            // Stand off from the peak and look toward it.
            let peak = best.1;
            let off = (peak + peak.any_orthonormal_vector() * 0.02).normalize();
            let up = rot * off;
            log::info!("lowflight: peak {:.0} m", best.0);
            app.u.ship.position = positions[planet] + up * (b.surface_radius_at(positions[planet], positions[planet] + up, t) + 6000.0);
            let to_peak = rot * peak - up;
            let fwd = (to_peak - up * to_peak.dot(up)).normalize();
            app.u.ship.velocity = sys.velocity(planet, t) + b.angular_velocity().cross(app.u.ship.position - positions[planet]) + fwd * 200.0;
            app.u.ship.orientation = universe_sim::ship::facing(fwd - up * 0.15, up);
            app.chase_cam = false;
        }
        "moon" => {
            let moon = sys.bodies.iter().position(|b| b.kind == BodyKind::Moon && b.terrain.is_some()).unwrap_or(planet);
            observe(app, moon, sys.bodies[moon].rail.radius * 2.2, 0.4);
        }
        "routemap" => {
            app.mode = Mode::Pilot;
            app.u.avionics.route.stops = app.u.settler_route(7, 10);
            app.nav_map = Some(crate::navmap::NavMap::open(app));
        }
        "route" => {
            // A settler route, flown headless through its first stops, then shown mid-leg.
            app.mode = Mode::Pilot;
            app.u.avionics.route.stops = app.u.settler_route(7, 10);
            app.u.toggle_route();
            for _ in 0..60 * 60 * 60 {
                app.u.step(1.0 / 60.0, 20.0, &Controls::default());
                if app.u.avionics.route.next >= 3 && app.u.ship.hyperdrive {
                    break;
                }
            }
        }
        "traffic" => {
            // Let the settlers get going, then watch the home station.
            for _ in 0..60 * 60 * 2 {
                app.u.step_world(1.0 / 60.0, 3.0, &Controls::default());
            }
            observe(app, station, 25_000.0, 0.35);
        }
        "radar" | "contacts" | "gunnery" => {
            // Fly alongside a settler under way, 6 km behind and to the side
            // of it, lock it on the radar and face it.
            for _ in 0..60 * 60 * 2 {
                app.u.step_world(1.0 / 60.0, 3.0, &Controls::default());
            }
            let (ship_system, pos) = (app.u.ship_system, app.u.ship.position);
            let near = app.u.crafts.iter().filter(|c| c.system == ship_system && c.ship.is_flying() && !c.ship.hyperdrive).min_by(|a, b| {
                a.ship.position.distance(pos).total_cmp(&b.ship.position.distance(pos))
            });
            if let Some(c) = near {
                let (p, v) = (c.ship.position, c.ship.velocity);
                let back = v.try_normalize().unwrap_or(DVec3::X);
                app.u.ship.position = p - back * 5_000.0 + back.any_orthonormal_vector() * 3_000.0;
                app.u.ship.velocity = v;
            }
            app.mode = Mode::Pilot;
            // "contacts": the same, but nothing locked (every ship marked).
            let lock = if name == "contacts" { app.u.contacts().into_iter().next() } else { app.u.lock_next_contact() };
            if let Some(c) = lock {
                let to = (c.blip.position - app.u.ship.position).normalize();
                app.u.ship.orientation = universe_sim::ship::facing(to, to.any_orthonormal_vector());
            }
            if name == "gunnery" {
                // Track it for two seconds, then fire the gun and laser at the
                // lead (the tracers are in flight for the screenshot).
                for _ in 0..120 {
                    app.u.step_world(1.0 / 60.0, 1.0, &Controls::default());
                    let contacts = app.u.contacts();
                    app.fire = app.u.fire_control(&contacts);
                }
                if let Some((_, Some(sol))) = app.fire {
                    app.u.ship.orientation = universe_sim::ship::facing(sol.aim, sol.aim.any_orthonormal_vector());
                }
                app.u.command(&ShipCommands { arm: Some(true), ..app.u.ship.holding() });
                app.u.ship.arming = 0.0;
                app.u.ship.triggers = universe_sim::world::Triggers { gun: true, laser: true };
            }
        }
        "follow" => {
            // Follow a settler that's on an approach (docking, landing or a gate run).
            app.mode = Mode::Observer;
            for _ in 0..60 * 60 * 5 {
                app.u.step_world(1.0 / 60.0, 3.0, &Controls::default());
                if let Some(i) = app.u.crafts.iter().position(|c| c.ship.is_flying() && !c.ship.hyperdrive && c.avionics.clearance.is_some_and(|x| x.autopilot)) {
                    app.observer.focus = Focus::Craft(i);
                    app.observer.distance = 300.0;
                    app.observer.pitch = 0.3;
                    break;
                }
            }
        }
        "network" => {
            app.mode = Mode::Observer;
            app.observer.focus = Focus::Body { system: home, body: 0 };
            app.observer.distance = 4.0e17;
            app.observer.pitch = 0.9;
        }
        "autodock" | "docked" => {
            // Docking computer flying from the spawn point, captured mid-final (or docked).
            app.mode = Mode::Pilot;
            app.u.toggle_autopilot();
            for _ in 0..60 * 60 * 3 {
                app.u.step(1.0 / 60.0, 10.0, &Controls::default());
                let final_run = app.u.docking_status().is_some_and(|(_, s)| s.phase == universe_sim::Phase::Final && s.height < 2200.0);
                if (name == "autodock" && final_run) || matches!(app.u.ship.state, ShipState::Landed { .. }) {
                    break;
                }
            }
        }
        other => log::warn!("unknown scenario {other:?}; try one of: {SCENARIOS}"),
    }
    app.messages.clear();
    log::info!("scenario {name}: pending events {:?}, clearance {:?}", app.u.events, app.u.avionics.clearance);

    // Optional camera override: UNIVERSE_CAM=cockpit | map (observer watching the ship from afar).
    match std::env::var("UNIVERSE_CAM").as_deref() {
        Ok("cockpit") => app.chase_cam = false,
        Ok("map") => {
            app.mode = Mode::Observer;
            app.observer.focus = Focus::Ship;
            app.observer.distance = 7_000.0;
            app.observer.pitch = 0.25;
        }
        _ => {}
    }
}

/// `UNIVERSE_SOUND_TEST=1`: play every sound in sequence, for checking levels.
/// Returns false once the sequence is over.
pub fn sound_test(ctx: &universe_engine::Context, t: f64, last_t: f64) -> bool {
    let Some(a) = ctx.audio() else { return false };
    let at = |x: f64| last_t < x && t >= x;
    let events = [
        (0.5, Event::Ship(ShipEvent::TookOff)),
        (1.0, Event::Ship(ShipEvent::Landed { body: String::new(), station: false })),
        (1.8, Event::Ship(ShipEvent::HyperdriveEngaged)),
        (2.8, Event::Ship(ShipEvent::HyperdriveDisengaged)),
        (3.8, Event::Ship(ShipEvent::Crashed { body: String::new() })),
    ];
    for (time, event) in &events {
        if at(*time) {
            crate::sound::event(ctx, event);
        }
    }
    if at(0.2) {
        crate::sound::click(ctx, 1000.0);
    }
    // Engine rumble at full throttle 6-8 s, hyperdrive drone 8.5-10.5 s.
    a.set_engine(if (6.0..8.0).contains(&t) { 1.0 } else { 0.0 });
    a.set_drone(if (8.5..10.5).contains(&t) { 0.7 } else { 0.0 }, 150.0);
    t < 11.0
}
