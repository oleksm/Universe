//! Named start-up scenarios for quickly checking features visually:
//! `UNIVERSE_SCENARIO=galaxy UNIVERSE_SCREENSHOT=out.png cargo run`

use universe_engine::glam::DVec3;
use universe_sim::ship::SHIP_RADIUS;
use universe_sim::{BodyKind, Controls, Event, GateFrame, NavTarget, PadFrame, Phase, ShipCommands, ShipEvent, ShipState, StationFrame};

use crate::observer::Focus;
use crate::{App, Mode};

pub const SCENARIOS: &str = "system inner planet giant rings galaxy neighbours cockpit hyper landed cleared approach offcourse autodock docked lost navmap landing padview autoland holding touchdown gate gateauto transit gatearrive network lowflight moon routemap route traffic follow radar contacts gunnery aboard outside collision pirates market trades noon dusk night sun sam";

pub fn apply(app: &mut App, name: &str) {
    let home = app.engine.universe().world.home_system;
    let sys = app.engine.universe().system(home);
    let t = app.engine.universe().world.time;
    let mut positions = Vec::new();
    sys.positions(t, &mut positions);
    let observe = |app: &mut App, body: usize, distance: f64, pitch: f64| {
        app.mode = Mode::Observer;
        app.observer.focus = Focus::Body { system: home, body };
        app.observer.distance = distance;
        app.observer.pitch = pitch;
    };
    let outer = sys.bodies.iter().filter_map(|b| b.rail.orbit.as_ref().filter(|_| b.rail.parent == Some(0) && b.kind.is_planet())).map(|o| o.apoapsis()).fold(0.0, f64::max);
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
            for n in std::iter::once(home).chain(app.engine.universe().world.galaxy.nearest(home, 60)) {
                let s = app.engine.universe().system(n);
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
            let u = app.engine.universe();
            let up = (u.ship.position - positions[planet]).normalize();
            let dir_to = |n: usize| (u.world.galaxy.offset(home, n) - u.ship.position).normalize();
            let candidates = u.world.galaxy.nearest(home, 12);
            let target = candidates.iter().copied().find(|&n| dir_to(n).dot(up) > 0.3).unwrap_or(candidates[0]);
            let dir = dir_to(target);
            u.ship.orientation = universe_engine::glam::DQuat::from_rotation_arc(DVec3::NEG_Z, dir);
            u.toggle_hyperdrive();
            u.command(&ShipCommands { throttle: 1.0, ..u.ship.holding() });
            for _ in 0..4000 {
                u.step_world(1.0 / 60.0, 1.0, &Controls::default());
                if !u.ship.hyperdrive {
                    break;
                }
            }
        }
        "landed" => {
            // Drop the ship just above the station's planet, matching its surface motion.
            app.mode = Mode::Pilot;
            let b = &sys.bodies[planet];
            let normal = (app.engine.universe().ship.position - positions[planet]).normalize();
            let offset = normal * (b.rail.radius + SHIP_RADIUS + 3.0);
            app.engine.universe().ship.position = positions[planet] + offset;
            app.engine.universe().ship.velocity = sys.velocity(planet, t) + b.angular_velocity().cross(offset);
            app.engine.universe().ship.orientation = universe_engine::glam::DQuat::from_rotation_arc(DVec3::NEG_Z, normal.any_orthonormal_vector());
            for _ in 0..120 {
                app.engine.universe().step_world(1.0 / 60.0, 1.0, &Controls::default());
            }
        }
        "approach" | "lost" | "approachkeep" => {
            // Cleared to dock, flying manually. "approach": 2.5 km out, a little off the axis, facing in.
            // "lost": facing away from the station, to show the off-screen marker.
            app.mode = Mode::Pilot;
            let f = StationFrame::new(&sys, station, t, &positions);
            app.engine.universe().ship.position = f.on_axis(2500.0) + f.slot_long() * 180.0 + f.slot_short() * 60.0;
            app.engine.universe().ship.velocity = f.velocity - f.axis() * 30.0;
            let facing = if name == "lost" { f.axis() } else { -f.axis() };
            let roll = universe_engine::glam::DQuat::from_axis_angle(facing, 0.35);
            app.engine.universe().ship.orientation = roll * universe_engine::glam::DQuat::from_rotation_arc(DVec3::NEG_Z, facing);
            if name != "lost" {
                app.engine.universe().request_clearance();
            }
            // Cleared, and keeping station on the station meanwhile.
            if name == "approachkeep" {
                let u = app.engine.universe();
                u.set_nav_target(Some(universe_sim::NavTarget::Station(station)));
                u.follow(universe_sim::FollowKind::KeepAt);
                for _ in 0..60 * 3 {
                    u.step_world(1.0 / 60.0, 1.0, &Controls::default());
                }
            }
        }
        "cleared" => {
            // The spawn point, with docking clearance granted.
            app.mode = Mode::Pilot;
            app.engine.universe().request_clearance();
        }
        "offcourse" => {
            // Cleared, but 6 km off to the side and drifting sideways at 40 m/s.
            app.mode = Mode::Pilot;
            let f = StationFrame::new(&sys, station, t, &positions);
            app.engine.universe().ship.position = f.on_axis(2000.0) + f.slot_long() * 6000.0;
            app.engine.universe().ship.velocity = f.velocity + f.slot_short() * 40.0;
            let look = (f.center - app.engine.universe().ship.position).normalize();
            app.engine.universe().ship.orientation = universe_engine::glam::DQuat::from_rotation_arc(DVec3::NEG_Z, look);
            app.engine.universe().request_clearance();
        }
        "galaxymap" | "galaxyzoom" => {
            // The galaxy map, having been to the gate network's systems.
            app.mode = Mode::Pilot;
            let links = app.engine.universe().world.gate_links.clone();
            for (a, b) in links {
                app.explored.insert(a);
                app.explored.insert(b);
            }
            let mut map = crate::galaxymap::GalaxyMap::open(app, universe_engine::glam::Vec2::new(960.0, 540.0));
            if name == "galaxyzoom" {
                map.zoom(400.0);
            }
            app.galaxy_map = Some(map);
        }
        "help" => {
            app.mode = Mode::Pilot;
            app.show_help = true;
        }
        "economy" => {
            app.mode = Mode::Pilot;
            app.economy_panel = Some(Default::default());
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
                app.engine.universe().ship.position = pad.pad + pad.up * 12_000.0 + pad.up.any_orthonormal_vector() * 6000.0;
                app.engine.universe().ship.velocity = pad.frame_velocity(app.engine.universe().ship.position) - pad.up * 40.0;
                let fwd = pad.up.any_orthonormal_vector().cross(pad.up);
                app.engine.universe().ship.orientation = universe_sim::ship::upright(pad.up, fwd);
            }
            app.engine.universe().set_nav_target(Some(NavTarget::Spaceport(port)));
            app.engine.universe().request_clearance();
            if name == "autoland" || name == "touchdown" {
                app.engine.universe().toggle_autopilot();
                for _ in 0..60 * 60 * 30 {
                    app.engine.universe().step_world(1.0 / 60.0, 20.0, &Controls::default());
                    let low = matches!(app.engine.universe().approach(), Some(universe_sim::Approach::Land { ref status, .. })
                        if status.phase == Phase::Descent && status.altitude < 1500.0);
                    if (name == "autoland" && low) || !app.engine.universe().ship.is_flying() {
                        break;
                    }
                }
            }
        }
        "holding" => {
            // The port's nine pads taken (long stops), and we're queued: the
            // holding circle 12 km over the port, two minutes in (traffic control
            // knows the pads are taken: it hands them out, not the ships on them).
            app.mode = Mode::Pilot;
            let port = sys.spaceports.iter().position(|p| p.body == planet).expect("spaceport on the station's planet");
            let u = app.engine.universe();
            for k in 0..universe_sim::world::spaceport::PADS {
                u.crafts[k].ship = u.world.ship_on(home, universe_sim::world::Facility::Spaceport(port), k);
                u.crafts[k].system = home;
                let now = u.world.time;
                u.atc.request_pad(home, port, universe_sim::craft_id(k), now);
            }
            {
                let mut pilots = u.pilots();
                for p in pilots.iter_mut().take(universe_sim::world::spaceport::PADS) {
                    p.avionics.route.stops = vec![universe_sim::Stop { system: home, target: NavTarget::Spaceport(port) }];
                    p.avionics.route.next = 0;
                    p.avionics.route.active = true;
                    p.avionics.route.dwell_until = Some(1.0e12);
                    p.avionics.pirate = false;
                }
            }
            let pad = PadFrame::new(&sys, port, t, &positions);
            u.ship.position = pad.pad + pad.up * 20_000.0;
            u.ship.velocity = pad.frame_velocity(u.ship.position);
            u.step_world(1.0 / 60.0, 1.0, &Controls::default());
            u.set_nav_target(Some(NavTarget::Spaceport(port)));
            u.request_clearance();
            u.toggle_autopilot();
            for _ in 0..60 * 120 {
                u.step_world(1.0 / 60.0, 1.0, &Controls::default());
            }
        }
        "asteroid" | "swarm" | "navlock" | "orbitrock" => {
            // By the first field's remnant (sun behind), or among its swarm,
            // moving with it, facing it; it's the nav target.
            app.mode = Mode::Pilot;
            let f = &sys.fields[0];
            let rock = f.body;
            let (at, v) = (positions[rock], sys.velocity(rock, t));
            let sun = -at.normalize();
            let side = sun.cross(DVec3::Y).normalize();
            let off = if name == "swarm" { (sun * 0.6 + side).normalize() * (sys.bodies[rock].rail.radius + 15_000.0) } else { (sun + side * 0.3).normalize() * (sys.bodies[rock].rail.radius + 3000.0) };
            let u = app.engine.universe();
            u.ship.position = at + off;
            u.ship.velocity = v;
            u.ship.angular_velocity = DVec3::ZERO;
            u.ship.orientation = universe_sim::ship::facing(-off.normalize(), DVec3::Y);
            u.set_nav_target(Some(NavTarget::Asteroid(rock)));
            if name == "navlock" {
                app.picker.hold_for_show();
            }
            if name == "orbitrock" {
                // Orbiting the remnant, a minute on.
                let u = app.engine.universe();
                u.follow(universe_sim::FollowKind::Orbit);
                for _ in 0..60 * 60 {
                    u.step_world(1.0 / 60.0, 1.0, &Controls::default());
                }
            }
        }
        "mining" | "cargo" | "prospect" | "closing" | "closing10" | "closingwatch" | "minemode" | "pulse" | "picklist" => {
            // By a rubble fragment of a home field, drifting with its
            // surface: "prospect" 300 m off it; "mining" anchored 15 m off
            // and digging for half a minute.
            app.mode = Mode::Pilot;
            let t = app.engine.universe().world.time;
            let (bodies, i) = (0..sys.fields.len())
                .find_map(|f| {
                    let bodies = sys.field_bodies(f);
                    let i = (sys.bodies.len()..bodies.len()).find(|&i| bodies[i].rail.radius > 20.0 && bodies[i].rock.as_ref().is_some_and(|r| universe_sim::world::mining::dig_rate(r) >= 10.0))?;
                    Some((bodies, i))
                })
                .expect("a rubble fragment");
            let mut pos = Vec::new();
            universe_sim::world::physics::positions(&bodies[..], t, &mut pos);
            let b = &bodies[i];
            let sun = -pos[i].normalize();
            let up = (sun + sun.any_orthonormal_vector() * 0.8).normalize();
            let gap = match name {
                "mining" | "cargo" => 15.0,
                "closing" | "closing10" | "closingwatch" => 800.0,
                _ => 300.0,
            };
            let at = pos[i] + up * (b.surface_radius(b.rotation(t).inverse() * up) + universe_sim::world::ship::SHIP_RADIUS + gap);
            let u = app.engine.universe();
            u.ship.position = at;
            u.ship.velocity = universe_sim::world::physics::velocity(&bodies[..], i, t) + b.angular_velocity().cross(at - pos[i]);
            u.ship.angular_velocity = DVec3::ZERO;
            // Spine to the rock, nose along its surface (as the approach leaves it).
            u.ship.orientation = universe_sim::ship::facing(up.any_orthonormal_vector(), -up);
            if name == "cargo" {
                app.show_cargo = true;
            }
            if matches!(name, "minemode" | "pulse" | "picklist") {
                // Mining mode, a little farther off: prospected (or mid-pulse), a rock locked, or T held.
                u.ship.position = at + up * 2000.0;
                app.mining.on = true;
                let age = if name == "pulse" { -0.45 } else { 5.0 };
                crate::mining::prospect_for_show(app, age);
                if name == "picklist" {
                    app.picker.hold_for_show();
                } else if let Some(f) = (0..sys.fields.len()).find(|&f| sys.field_rocks(f).any(|j| j == i) && sys.field_bodies(f).len() == bodies.len()) {
                    app.engine.universe().cockpit().lock_rock(Some((f, i)));
                }
            }
            let u = app.engine.universe();
            if matches!(name, "closing" | "closing10" | "closingwatch") {
                // Closing on it, ninety (or ten) seconds on.
                let f = (0..sys.fields.len()).find(|&f| sys.field_bodies(f).len() == bodies.len() && sys.field_rocks(f).any(|j| j == i)).unwrap();
                u.close_on(f, i);
                for _ in 0..60 * if name == "closing" { 90 } else { 10 } {
                    u.step_world(1.0 / 60.0, 1.0, &Controls::default());
                }
                if name == "closingwatch" {
                    app.mode = Mode::Observer;
                    app.observer.focus = Focus::Ship;
                    app.observer.distance = 400.0;
                    app.observer.pitch = 0.4;
                }
            }
            if matches!(name, "mining" | "cargo") {
                u.command(&ShipCommands { anchor: Some(true), ..u.ship.holding() });
                for _ in 0..10 {
                    u.step_world(1.0 / 60.0, 1.0, &Controls::default());
                }
                u.command(&ShipCommands { excavate: Some(true), ..u.ship.holding() });
                for _ in 0..60 * if name == "cargo" { 105 } else { 30 } {
                    u.step_world(1.0 / 60.0, 1.0, &Controls::default());
                }
            }
        }
        "gate" | "gateauto" | "transit" | "gatearrive" => {
            // A gate out of the home system: cleared for transit, 8 km out, off to one side.
            app.mode = Mode::Pilot;
            let (dest, _) = app.engine.universe().gate_links_of(home)[0].clone();
            let g = sys.gate_to(dest).unwrap();
            let f = GateFrame::new(&sys, g, t, &positions);
            app.engine.universe().ship.position = f.center + f.axis() * 8000.0 + (f.rotation * DVec3::X) * 2500.0;
            app.engine.universe().ship.velocity = f.velocity;
            let look = (f.center - app.engine.universe().ship.position).normalize();
            app.engine.universe().ship.orientation = universe_sim::ship::facing(look, f.rotation * DVec3::Z);
            app.engine.universe().set_nav_target(Some(NavTarget::Gate(g)));
            app.engine.universe().request_clearance();
            if name != "gate" {
                app.engine.universe().toggle_autopilot();
                for _ in 0..60 * 60 * 10 {
                    app.engine.universe().step_world(1.0 / 60.0, 10.0, &Controls::default());
                    let running = matches!(app.engine.universe().approach(), Some(universe_sim::Approach::Transit { ref status, .. })
                        if status.phase == Phase::Final && status.distance < 1500.0);
                    let stop = match name {
                        "gateauto" => running,
                        "transit" => matches!(app.engine.universe().ship.state, ShipState::Transit { remaining, .. } if remaining < 3.5),
                        _ => app.engine.universe().ship_system != home && app.engine.universe().ship.is_flying(),
                    };
                    if stop {
                        break;
                    }
                }
            }
        }
        "sam" | "samfar" => {
            // Aggressed, 9 km out from a defended station or port, looking at
            // its turrets (far: 40 km out, the missiles on their way up).
            app.mode = Mode::Pilot;
            if let Some((t, at, v)) = app.engine.universe().world.turret_motions(home).into_iter().find(|(t, _, _)| !matches!(t.facility, universe_sim::world::Facility::Gate(_))) {
                let out = (at - positions[t.body]).normalize();
                app.engine.universe().ship.position = at + out * if name == "samfar" { 40_000.0 } else { 9_000.0 };
                app.engine.universe().ship.velocity = v;
                app.engine.universe().ship.orientation = universe_sim::ship::facing(-out, out.any_orthonormal_vector());
                let u = app.engine.universe();
                let now = u.world.time;
                u.law.declare(universe_sim::PLAYER, now + 600.0, now, universe_sim::protocol::Cause::Rules);
                if name == "samfar" {
                    for _ in 0..60 * 26 {
                        u.step_world(1.0 / 60.0, 1.0, &Controls::default());
                    }
                }
            }
        }
        "orbit" => {
            // 7 km off the home station, orbiting it at 5 km.
            app.mode = Mode::Pilot;
            let station = sys.station().expect("home station");
            let f = StationFrame::new(&sys, station, app.engine.universe().world.time, &positions);
            let side = f.axis().any_orthonormal_vector();
            app.engine.universe().ship.position = f.center + side * 7_000.0;
            app.engine.universe().ship.velocity = f.velocity;
            app.engine.universe().ship.orientation = universe_sim::ship::facing(-side, f.axis());
            app.engine.universe().set_nav_target(Some(NavTarget::Station(station)));
            app.engine.universe().follow(universe_sim::FollowKind::Orbit);
        }
        "sunclose" => {
            // A tenth of an AU from the star, facing it.
            app.mode = Mode::Pilot;
            let u = app.engine.universe();
            let out = u.ship.position.normalize();
            u.ship.position = out * 0.1 * universe_sim::units::AU;
            let look = -out;
            u.ship.orientation = universe_sim::ship::facing((look + look.any_orthonormal_vector() * 0.15).normalize(), look.any_orthonormal_vector());
        }
        "sun" => {
            // Facing the star from the home orbit.
            app.mode = Mode::Pilot;
            let look = (positions[0] - app.engine.universe().ship.position).normalize();
            app.engine.universe().ship.orientation = universe_sim::ship::facing((look + look.any_orthonormal_vector() * 0.15).normalize(), look.any_orthonormal_vector());
        }
        "noon" | "dusk" | "night" => {
            // 2 km over the home planet, level, looking along the ground: the
            // sun overhead, on the horizon, or below it.
            app.mode = Mode::Pilot;
            let b = &sys.bodies[planet];
            let sun = (positions[0] - positions[planet]).normalize();
            let side = sun.any_orthonormal_vector();
            let angle: f64 = match name {
                "noon" => 0.35,
                "dusk" => std::f64::consts::FRAC_PI_2 - 0.02,
                _ => 2.4,
            };
            let up = (sun * angle.cos() + side * angle.sin()).normalize();
            let center = positions[planet];
            let ground = b.surface_radius_at(center, center + up, t);
            app.engine.universe().ship.position = center + up * (ground + 2_000.0);
            app.engine.universe().ship.velocity = sys.velocity(planet, t) + b.angular_velocity().cross(app.engine.universe().ship.position - center);
            // Look toward the sun's side along the horizon, a little down.
            let ahead = (sun - up * sun.dot(up)).normalize_or(side);
            let look = (ahead - up * 0.08).normalize();
            app.engine.universe().ship.orientation = universe_sim::ship::facing(look, up);
        }
        "lowflight" => {
            // Skimming 6 km over the home planet, 800 km from the port, looking ahead.
            app.mode = Mode::Pilot;
            let b = &sys.bodies[planet];
            let rot = b.rotation(t);
            // Find mountains: the highest of a few hundred spots in a wide patch.
            let start = rot.inverse() * (app.engine.universe().ship.position - positions[planet]).normalize();
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
            app.engine.universe().ship.position = positions[planet] + up * (b.surface_radius_at(positions[planet], positions[planet] + up, t) + 6000.0);
            let to_peak = rot * peak - up;
            let fwd = (to_peak - up * to_peak.dot(up)).normalize();
            app.engine.universe().ship.velocity = sys.velocity(planet, t) + b.angular_velocity().cross(app.engine.universe().ship.position - positions[planet]) + fwd * 200.0;
            app.engine.universe().ship.orientation = universe_sim::ship::facing(fwd - up * 0.15, up);
            app.chase_cam = false;
        }
        "moon" => {
            let moon = sys.bodies.iter().position(|b| b.kind == BodyKind::Moon && b.terrain.is_some()).unwrap_or(planet);
            observe(app, moon, sys.bodies[moon].rail.radius * 2.2, 0.4);
        }
        "routemap" => {
            app.mode = Mode::Pilot;
            let stops = app.engine.universe().settler_route(7, 10);
            app.engine.universe().cockpit().route_set(stops);
            app.nav_map = Some(crate::navmap::NavMap::open(app));
        }
        "route" => {
            // A settler route, flown headless through its first stops, then shown mid-leg.
            app.mode = Mode::Pilot;
            let stops = app.engine.universe().settler_route(7, 10);
            app.engine.universe().cockpit().route_set(stops);
            app.engine.universe().toggle_route();
            for _ in 0..60 * 60 * 60 {
                app.engine.universe().step_world(1.0 / 60.0, 20.0, &Controls::default());
                if app.engine.universe().avionics().route.next >= 3 && app.engine.universe().ship.hyperdrive {
                    break;
                }
            }
        }
        "traffic" => {
            // Let the settlers get going, then watch the home station.
            for _ in 0..60 * 60 * 2 {
                app.engine.universe().step_world(1.0 / 60.0, 3.0, &Controls::default());
            }
            observe(app, station, 25_000.0, 0.35);
        }
        "crowd" => {
            // A crowded place without the wait: 300 of the home system's
            // crafts flying their routes within 40 km of the station, and us
            // 8 km out looking at it (for profiling).
            app.mode = Mode::Pilot;
            let st = positions[station];
            let v = sys.velocity(station, t);
            let home = app.engine.universe().ship_system;
            let mut rng = 7u64;
            let mut next = || {
                rng = rng.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
                ((rng >> 11) as f64 / (1u64 << 53) as f64) * 2.0 - 1.0
            };
            let u = app.engine.universe();
            let picked: Vec<usize> = (0..u.crafts.len()).filter(|&i| u.crafts[i].system == home).take(300).collect();
            for &i in &picked {
                let c = &mut u.crafts[i];
                let off = DVec3::new(next(), next(), next()).normalize_or(DVec3::X) * (5_000.0 + 35_000.0 * next().abs());
                c.ship.state = ShipState::Flying;
                c.ship.hyperdrive = false;
                c.ship.position = st + off;
                c.ship.velocity = v + DVec3::new(next(), next(), next()) * 30.0;
            }
            let mut pilots = u.pilots();
            for &i in &picked {
                let r = &mut pilots[i].avionics.route;
                r.active = !r.stops.is_empty();
                r.dwell_until = None;
                r.departing = false;
            }
            drop(pilots);
            let side = (app.engine.universe().ship.position - st).normalize_or(DVec3::X);
            app.engine.universe().ship.position = st + side * 8_000.0;
            app.engine.universe().ship.velocity = v;
            app.engine.universe().ship.orientation = universe_sim::ship::facing(-side, side.any_orthonormal_vector());
        }
        "collision" => {
            // Collision warning on, 3 km from the station and closing at 80 m/s, a little off its center.
            app.mode = Mode::Pilot;
            let st = positions[station];
            let toward = (st - app.engine.universe().ship.position).normalize();
            let side = toward.any_orthonormal_vector();
            app.engine.universe().ship.position = st - toward * 3_000.0 + side * 150.0;
            app.engine.universe().ship.velocity = sys.velocity(station, t) + toward * 80.0;
            let look = (st - app.engine.universe().ship.position).normalize();
            app.engine.universe().ship.orientation = universe_sim::ship::facing(look + side * 0.25, side);
            app.engine.universe().cockpit().collision_warning(true);
        }
        "pirates" => {
            // A pirate goes after a trader near us; run until it's destroyed
            // (the kill feed shows it), watching from alongside.
            app.mode = Mode::Pilot;
            // 30 km out from the station (out of its shelter), and we watch from there.
            app.engine.universe().ship.position += DVec3::new(30_000.0, 0.0, 0.0);
            let (sysi, pos, vel) = (app.engine.universe().ship_system, app.engine.universe().ship.position, app.engine.universe().ship.velocity);
            app.engine.universe().spawn_settlers(2, 7);
            let n = app.engine.universe().crafts.len();
            for (k, c) in app.engine.universe().crafts[n - 2..].iter_mut().enumerate() {
                c.system = sysi;
                c.ship.state = ShipState::Flying;
                c.ship.hyperdrive = false;
                c.ship.position = pos + DVec3::new(-2_000.0 + 5_000.0 * k as f64, 1_500.0, -3_000.0);
                c.ship.velocity = vel;
                let number = c.name.split(' ').next_back().unwrap_or("").to_string();
                c.name = format!("{} {number}", if k == 0 { "Pirate" } else { "Trader" });
            }
            for (k, p) in app.engine.universe().pilots()[n - 2..].iter_mut().enumerate() {
                p.avionics.pirate = k == 0;
                p.trader = k == 1;
                p.avionics.route.active = false;
                p.avionics.route.dwell_until = None;
            }
            // Mid-fight: the pirate aggressed, the trader's hull going.
            for _ in 0..60 * 120 {
                app.engine.universe().step_world(1.0 / 60.0, 1.0, &Controls::default());
                if app.engine.universe().crafts[n - 1].ship.hull < 0.97 {
                    break;
                }
            }
            let pirate = app.engine.universe().crafts[n - 2].ship.position;
            let look = (pirate - app.engine.universe().ship.position).normalize();
            app.engine.universe().ship.orientation = universe_sim::ship::facing(look, look.any_orthonormal_vector());
            log::info!("scenario pirates: kills {:?}", app.engine.universe().records.kills.last());
        }
        "aboard" => {
            // Out of the seat, at the back of the cabin looking forward up the corridor.
            app.mode = Mode::Pilot;
            use universe_sim::world::crew::DECK;
            app.engine.universe().crew.place = universe_sim::world::Place::Aboard { position: DVec3::new(0.0, DECK, 8.5), yaw: 0.0, pitch: 0.05 };
        }
        "outside" => {
            // Land on the pad, step out, turn round to look at the ship.
            apply(app, "touchdown");
            use universe_sim::world::crew::HATCH;
            app.engine.universe().crew.place = universe_sim::world::Place::Aboard { position: HATCH, yaw: 0.0, pitch: 0.0 };
            app.engine.universe().walk(&universe_sim::world::WalkCommands { interact: true, ..Default::default() }, 0.02);
            // Walk away from the ship a while, then face it.
            for _ in 0..300 {
                app.engine.universe().walk(&universe_sim::world::WalkCommands { forward: 1.0, ..Default::default() }, 0.02);
            }
            app.engine.universe().walk(&universe_sim::world::WalkCommands { yaw: std::f64::consts::PI, pitch: 0.15, ..Default::default() }, 0.02);
            log::info!("scenario outside: crew {:?}", app.engine.universe().crew.place);
        }
        "radar" | "contacts" | "gunnery" => {
            // Fly alongside a settler under way, 6 km behind and to the side
            // of it, lock it on the radar and face it.
            for _ in 0..60 * 60 * 2 {
                app.engine.universe().step_world(1.0 / 60.0, 3.0, &Controls::default());
            }
            let (ship_system, pos) = (app.engine.universe().ship_system, app.engine.universe().ship.position);
            let near = app.engine.universe().crafts.iter().filter(|c| c.system == ship_system && c.ship.is_flying() && !c.ship.hyperdrive).min_by(|a, b| {
                a.ship.position.distance(pos).total_cmp(&b.ship.position.distance(pos))
            });
            if let Some(c) = near {
                let (p, v) = (c.ship.position, c.ship.velocity);
                let back = v.try_normalize().unwrap_or(DVec3::X);
                app.engine.universe().ship.position = p - back * 5_000.0 + back.any_orthonormal_vector() * 3_000.0;
                app.engine.universe().ship.velocity = v;
            }
            app.mode = Mode::Pilot;
            // "contacts": the same, but nothing locked (every ship marked).
            let lock = if name == "contacts" { app.engine.universe().contacts().into_iter().next() } else { app.engine.universe().lock_next_contact() };
            if let Some(c) = lock {
                let to = (c.blip.position - app.engine.universe().ship.position).normalize();
                app.engine.universe().ship.orientation = universe_sim::ship::facing(to, to.any_orthonormal_vector());
            }
            if name == "gunnery" {
                // Track it for two seconds, then fire the gun and laser at the
                // lead (the tracers are in flight for the screenshot).
                for _ in 0..120 {
                    app.engine.universe().step_world(1.0 / 60.0, 1.0, &Controls::default());
                    let contacts = app.engine.universe().contacts();
                    app.fire = app.engine.universe().fire_control(&contacts);
                }
                // The nose 2° off the lead: the gimbal lays the gun on it.
                if let Some((_, Some(sol))) = app.fire {
                    let off = universe_engine::glam::DQuat::from_rotation_z(2f64.to_radians()) * sol.aim;
                    app.engine.universe().ship.orientation = universe_sim::ship::facing(off, sol.aim.any_orthonormal_vector());
                }
                let u = app.engine.universe();
                u.command(&ShipCommands { arm: Some(true), ..u.ship.holding() });
                app.engine.universe().ship.arming = 0.0;
                app.engine.universe().ship.triggers = universe_sim::world::Triggers { gun: true, laser: true };
            }
        }
        "follow" => {
            // Follow a settler that's on an approach (docking, landing or a gate run).
            app.mode = Mode::Observer;
            for _ in 0..60 * 60 * 5 {
                app.engine.universe().step_world(1.0 / 60.0, 3.0, &Controls::default());
                if let Some(i) = app.engine.universe().crafts.iter().position(|c| c.ship.is_flying() && !c.ship.hyperdrive && c.status.clearance.is_some_and(|x| x.autopilot)) {
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
            app.engine.universe().toggle_autopilot();
            for _ in 0..60 * 60 * 3 {
                app.engine.universe().step_world(1.0 / 60.0, 10.0, &Controls::default());
                let final_run = app.engine.universe().docking_status().is_some_and(|(_, s)| s.phase == universe_sim::Phase::Final && s.height < 2200.0);
                if (name == "autodock" && final_run) || matches!(app.engine.universe().ship.state, ShipState::Landed { .. }) {
                    break;
                }
            }
        }
        "trades" => {
            // Traffic under way until traders trade here (the feed shows them).
            app.mode = Mode::Pilot;
            for _ in 0..60 * 60 * 60 {
                app.engine.universe().step_world(1.0 / 60.0, 5.0, &Controls::default());
                let u = app.engine.universe();
                let now = u.world.time;
                let n = u.records.trades.iter().rev().take_while(|r| now - r.time < 5.0).filter(|r| r.system == home).count();
                if n >= 2 {
                    break;
                }
            }
            let u = app.engine.universe();
            log::info!("scenario trades: {} trades so far, last {:?}", u.records.stats.trades, u.records.trades.last().map(|r| (&r.trader, &r.item)));
        }
        "market" => {
            // Docked at the home station, the market open; bought ten of something.
            apply(app, "docked");
            let u = app.engine.universe();
            if let Some(f) = u.docked_market() {
                let (quotes, _) = u.market_quotes(f);
                if let Some(q) = quotes.iter().find(|q| q.buy.is_some()) {
                    let item = q.offer.item;
                    let r = u.trade(f, item, 10);
                    log::info!("scenario market: bought 10 of {}: {r:?}", u.world.goods[item].name);
                }
                app.engine.send(universe_sim::Command::WatchMarket(Some(f)));
            }
            app.engine.refresh();
            app.v = app.engine.view();
            app.market = Some(crate::market::MarketView::open(app));
        }
        other => log::warn!("unknown scenario {other:?}; try one of: {SCENARIOS}"),
    }
    app.messages.clear();
    let u = app.engine.universe();
    log::info!("scenario {name}: pending events {:?}, clearance {:?}", u.events, u.avionics().clearance);

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
