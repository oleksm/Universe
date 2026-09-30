//! Named start-up scenarios for quickly checking features visually:
//! `UNIVERSE_SCENARIO=galaxy UNIVERSE_SCREENSHOT=out.png cargo run`

use universe_engine::glam::DVec3;
use universe_sim::ship::SHIP_RADIUS;
use universe_sim::{BodyKind, Controls, Event, GateFrame, NavTarget, PadFrame, Phase, ShipCommands, ShipEvent, ShipState, StationFrame};

use crate::observer::Focus;
use crate::{App, Mode};

pub const SCENARIOS: &str = "system inner planet giant rings galaxy neighbours cockpit hyper landed cleared approach offcourse autodock docked lost navmap landing padview autoland touchdown gate gateauto transit gatearrive network lowflight moon routemap route traffic follow radar contacts gunnery aboard outside collision pirates market trades noon dusk night sun";

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
        "sun" => {
            // Facing the star from the home orbit.
            app.mode = Mode::Pilot;
            let look = (positions[0] - app.u.ship.position).normalize();
            app.u.ship.orientation = universe_sim::ship::facing((look + look.any_orthonormal_vector() * 0.15).normalize(), look.any_orthonormal_vector());
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
            app.u.ship.position = center + up * (ground + 2_000.0);
            app.u.ship.velocity = sys.velocity(planet, t) + b.angular_velocity().cross(app.u.ship.position - center);
            // Look toward the sun's side along the horizon, a little down.
            let ahead = (sun - up * sun.dot(up)).normalize_or(side);
            let look = (ahead - up * 0.08).normalize();
            app.u.ship.orientation = universe_sim::ship::facing(look, up);
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
        "collision" => {
            // Collision warning on, 3 km from the station and closing at 80 m/s, a little off its center.
            app.mode = Mode::Pilot;
            let st = positions[station];
            let toward = (st - app.u.ship.position).normalize();
            let side = toward.any_orthonormal_vector();
            app.u.ship.position = st - toward * 3_000.0 + side * 150.0;
            app.u.ship.velocity = sys.velocity(station, t) + toward * 80.0;
            let look = (st - app.u.ship.position).normalize();
            app.u.ship.orientation = universe_sim::ship::facing(look + side * 0.25, side);
            app.u.avionics.collision_warning = true;
        }
        "pirates" => {
            // A pirate goes after a trader near us; run until it's destroyed
            // (the kill feed shows it), watching from alongside.
            app.mode = Mode::Pilot;
            let (sysi, pos, vel) = (app.u.ship_system, app.u.ship.position, app.u.ship.velocity);
            app.u.spawn_settlers(2, 7);
            let n = app.u.crafts.len();
            for (k, c) in app.u.crafts[n - 2..].iter_mut().enumerate() {
                c.system = sysi;
                c.ship.state = ShipState::Flying;
                c.ship.hyperdrive = false;
                c.ship.position = pos + DVec3::new(-2_000.0 + 5_000.0 * k as f64, 1_500.0, -3_000.0);
                c.ship.velocity = vel;
                c.avionics.pirate = k == 0;
                c.avionics.route.active = false;
                c.avionics.route.dwell_until = None;
            }
            let kills = app.u.kills.len();
            for _ in 0..60 * 120 {
                app.u.step_world(1.0 / 60.0, 1.0, &Controls::default());
                if app.u.kills.len() > kills {
                    break;
                }
            }
            let pirate = app.u.crafts[n - 2].ship.position;
            let look = (pirate - app.u.ship.position).normalize();
            app.u.ship.orientation = universe_sim::ship::facing(look, look.any_orthonormal_vector());
            log::info!("scenario pirates: kills {:?}", app.u.kills.last());
        }
        "aboard" => {
            // Out of the seat, at the back of the cabin looking forward up the corridor.
            app.mode = Mode::Pilot;
            use universe_sim::world::crew::DECK;
            app.u.crew.place = universe_sim::world::Place::Aboard { position: DVec3::new(0.0, DECK, 8.5), yaw: 0.0, pitch: 0.05 };
        }
        "outside" => {
            // Land on the pad, step out, turn round to look at the ship.
            apply(app, "touchdown");
            use universe_sim::world::crew::HATCH;
            app.u.crew.place = universe_sim::world::Place::Aboard { position: HATCH, yaw: 0.0, pitch: 0.0 };
            app.u.walk(&universe_sim::world::WalkCommands { interact: true, ..Default::default() }, 0.02);
            // Walk away from the ship a while, then face it.
            for _ in 0..300 {
                app.u.walk(&universe_sim::world::WalkCommands { forward: 1.0, ..Default::default() }, 0.02);
            }
            app.u.walk(&universe_sim::world::WalkCommands { yaw: std::f64::consts::PI, pitch: 0.15, ..Default::default() }, 0.02);
            log::info!("scenario outside: crew {:?}", app.u.crew.place);
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
                // The nose 2° off the lead: the gimbal lays the gun on it.
                if let Some((_, Some(sol))) = app.fire {
                    let off = universe_engine::glam::DQuat::from_rotation_z(2f64.to_radians()) * sol.aim;
                    app.u.ship.orientation = universe_sim::ship::facing(off, sol.aim.any_orthonormal_vector());
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
        "trades" => {
            // Traffic under way until traders trade here (the feed shows them).
            app.mode = Mode::Pilot;
            for _ in 0..60 * 60 * 60 {
                app.u.step_world(1.0 / 60.0, 5.0, &Controls::default());
                let n = app.u.trade_log.iter().rev().take_while(|r| app.u.world.time - r.time < 5.0).filter(|r| r.system == home).count();
                if n >= 2 {
                    break;
                }
            }
            log::info!("scenario trades: {} trades so far, last {:?}", app.u.traffic.trades, app.u.trade_log.last().map(|r| (&r.trader, &r.item)));
        }
        "market" => {
            // Docked at the home station, the market open; bought ten of something.
            apply(app, "docked");
            app.market = Some(crate::market::MarketView::open(app));
            if let (Some(f), Some(q)) = (app.u.docked_market(), app.market.as_ref().and_then(|m| m.rows.iter().find(|r| r.quote.is_some_and(|q| q.buy.is_some())).cloned())) {
                let r = app.u.trade(f, q.item, 10);
                log::info!("scenario market: bought 10 of {}: {r:?}", app.u.world.goods[q.item].name);
            }
            let mut m = app.market.take().unwrap();
            m.refresh(app);
            app.market = Some(m);
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
