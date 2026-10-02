use universe_engine::glam::{DQuat, DVec3, Vec2};
use universe_engine::{text_size, Color, Frame, Light, Transform};
use universe_sim::names::star_name;
use universe_sim::units::LIGHT_YEAR;
use universe_sim::docking::APPROACH_HEIGHT;
use universe_sim::landing::ENTRY_ALTITUDE;
use universe_sim::world::spaceport::{pad_direction, PADS, PAD_RADIUS};
use universe_sim::gate::APPROACH_DISTANCE;
use universe_sim::{Action, Approach, BodyKind, DockingStatus, GateFrame, GateStatus, LandingStatus, Plan, ShipState, StationFrame};

use crate::{ship_visible, terrain_view, App, Mode};

pub const LABEL: Color = Color::hex(0x7a8a9a);
pub const SHIP_COLOR: Color = Color::hex(0xdcebf2);

pub fn color(c: [f32; 3]) -> Color {
    Color::rgb(c[0], c[1], c[2])
}

pub fn draw(frame: &mut Frame, app: &App) {
    let starlight = universe_prof::time("draw/scene/sky", || sky(frame, app));
    universe_prof::time("draw/scene/galaxy", || galaxy(frame, app, starlight));
    if matches!(app.ship.state, ShipState::Transit { .. }) && app.mode == Mode::Pilot {
        transit_tunnel(frame, app);
        return;
    }
    if app.show_grid {
        universe_prof::time("draw/scene/orbits", || orbits(frame, app));
    }
    // The system's star lights everything in it: its colour, and its
    // luminosity (1 at 1 AU from a sun-like star), fading with distance.
    let class = app.view.system.class;
    let tint = class.color();
    let top = tint.iter().copied().fold(0.0, f32::max);
    frame.light = app.view.system.bodies.iter().position(|b| b.kind == BodyKind::Star).map(|i| Light {
        position: app.view.positions[i],
        color: tint.map(|c| c / top),
        luminosity: class.luminosity(),
        reference: universe_sim::units::AU,
        radius: app.view.system.bodies[i].rail.radius,
    });
    // Planets and moons hide the sun from what's behind them.
    frame.eclipsers = app.view.system.bodies.iter().enumerate().filter(|(_, b)| !b.kind.artificial() && b.kind != BodyKind::Star).map(|(i, b)| (app.view.positions[i], b.rail.radius)).collect();
    // Ships, stations, gates and rocks shadow each other near the eye.
    frame.shadow_reach = SHADOW_REACH;
    frame.reflector = reflector(frame, app);
    universe_prof::time("draw/scene/bodies", || bodies(frame, app));
    universe_prof::time("draw/scene/asteroids", || crate::rocks::draw(frame, app));
    universe_prof::time("draw/scene/dust", || dust(frame, app));
    if app.mode == Mode::Pilot
        && let Some(s) = crate::rocks::scan(app)
    {
        crate::rocks::mark(frame, &s);
    }
    if app.mode == Mode::Pilot {
        crate::mining::draw_scene(frame, app);
    }
    universe_prof::time("draw/scene/rigs", || crate::rig::draw(frame, app));
    // (In the observer's view too: guidance shows wherever it's on.)
    crate::followguide::draw(frame, app);
    universe_prof::time("draw/scene/spaceports", || spaceports(frame, app));
    if app.view.origin == app.v.ship_system {
        match &app.approach {
            Some(Approach::Dock { station, status }) => docking_guide(frame, app, *station, status),
            Some(Approach::Land { port, status }) => {
                holding_circle(frame, app, *port);
                landing_guide(frame, app, status)
            }
            Some(Approach::Transit { gate, status }) => transit_guide(frame, app, *gate, status),
            None => {}
        }
    }
    universe_prof::time("draw/scene/collision path", || collision_path(frame, app));
    universe_prof::time("draw/scene/ship", || ship(frame, app));
    if matches!(app.v.crew.place, universe_sim::world::Place::Aboard { .. }) && app.mode == Mode::Pilot {
        crate::onfoot::interior(frame, app);
    }
    crate::onfoot::ramp(frame, app);
    universe_prof::time("draw/scene/crafts", || crafts(frame, app));
    gate_flashes(frame, app);
    universe_prof::time("draw/scene/weapons fire", || weapons_fire(frame, app));
    if app.show_labels {
        universe_prof::time("draw/scene/labels", || labels(frame, app));
    }
}

/// The planet or moon filling most of the sky from here: its day side lights
/// the shaded side of anything near it (see `Reflector`).
fn reflector(frame: &Frame, app: &App) -> Option<universe_engine::Reflector> {
    let cam = frame.camera.position;
    let sys = &app.view.system;
    let (i, _) = sys
        .bodies
        .iter()
        .enumerate()
        .filter(|(_, b)| !b.kind.artificial() && b.kind != BodyKind::Star)
        .map(|(i, b)| (i, (b.rail.radius / app.view.positions[i].distance(cam).max(1.0)).powi(2)))
        .max_by(|a, b| a.1.total_cmp(&b.1))?;
    let b = &sys.bodies[i];
    // Share of the light it sends back: clouds and seas, bare rock, dust, cloud tops, ice.
    let albedo = match (b.kind, b.terrain.as_ref().map(|t| t.kind)) {
        (_, Some(universe_sim::TerrainKind::Terran)) => 0.30,
        (BodyKind::GasGiant, _) => 0.50,
        (BodyKind::IceGiant, _) => 0.45,
        (BodyKind::Moon, _) => 0.12,
        _ => 0.15,
    };
    let [r, g, bl] = b.color;
    let top = r.max(g).max(bl).max(1e-3);
    // (Its colour paled by clouds and haze: half way to white.)
    let pale = |c: f32| 0.5 + 0.5 * c / top;
    Some(universe_engine::Reflector { center: app.view.positions[i], radius: b.rail.radius, albedo, color: [pale(r), pale(g), pale(bl)] })
}

/// Top of a world's atmosphere, for the sky's colour (m).
const ATMOSPHERE: f64 = 100_000.0;

/// How far from the eye ships, stations, gates and rocks cast shadows (m).
const SHADOW_REACH: f64 = 2_500.0;

/// Day and night in the sky. Inside the atmosphere of a world with air
/// (Terran), the sky takes the sun's light: blue by day, red and orange at
/// twilight, black at night, fading out with altitude. The stars wash out by
/// day. Airless worlds and space: a black sky, stars always. Returns how much
/// of the starlight shows (0..1).
fn sky(frame: &mut Frame, app: &App) -> f32 {
    frame.clear = Color::BLACK;
    let sys = &app.view.system;
    let Some(star) = sys.bodies.iter().position(|b| b.kind == BodyKind::Star) else { return 1.0 };
    let cam = frame.camera.position;
    let sun = app.view.positions[star];
    let t = app.now();
    let air = sys.bodies.iter().enumerate().find_map(|(i, b)| {
        let terran = b.terrain.as_ref().is_some_and(|tr| tr.kind == universe_sim::TerrainKind::Terran);
        let center = app.view.positions[i];
        let altitude = cam.distance(center) - b.surface_radius_at(center, cam, t);
        (terran && altitude < ATMOSPHERE).then_some((center, altitude))
    });
    let Some((center, altitude)) = air else { return 1.0 };
    let up = (cam - center).normalize();
    let elevation = up.dot((sun - cam).normalize()) as f32; // sine of the sun's height
    let thick = (1.0 - altitude / ATMOSPHERE).clamp(0.0, 1.0) as f32;
    let day = ((elevation + 0.05) / 0.3).clamp(0.0, 1.0);
    let twilight = (-(elevation / 0.08).powi(2)).exp();
    let bright = Light { position: sun, color: [1.0; 3], luminosity: sys.class.luminosity(), reference: universe_sim::units::AU, radius: 0.0 }.intensity_at(cam);
    let tint = sys.class.color();
    let blue = [0.22, 0.42, 0.85];
    let dusk = [0.85, 0.38, 0.16];
    let c = |j: usize| (blue[j] * day * bright * (0.6 + 0.4 * tint[j]) + dusk[j] * twilight * 0.35 * tint[j]) * thick;
    frame.clear = Color([c(0), c(1), c(2), 1.0]);
    1.0 - 0.97 * day * thick
}

/// Every star in the galaxy as a sky point, dimmed by distance.
fn galaxy(frame: &mut Frame, app: &App, starlight: f32) {
    let g = &app.charts.galaxy;
    let origin = g.stars[app.view.origin].position;
    let cam = frame.camera.position;
    let star = |s: &universe_sim::galaxy::GalaxyStar| {
        let rel = (s.position - origin) * LIGHT_YEAR - cam;
        let d_ly = rel.length() / LIGHT_YEAR;
        let flux = s.class.luminosity() / (d_ly * d_ly).max(1e-9);
        let brightness = ((flux.log10() + 8.0) / 6.0).clamp(0.2, 1.0) as f32;
        (rel.normalize().as_vec3(), color(s.class.color()).scale(brightness))
    };
    let others = || g.stars.iter().enumerate().filter(|(i, _)| *i != app.view.origin).map(|(_, s)| s); // ours is drawn as a body
    if cam.length() > 1.0e14 {
        // Zoomed out toward the galaxy's scale: the stars shift as we move.
        for s in others() {
            let (dir, c) = star(s);
            frame.sky_point(dir, c.scale(starlight));
        }
    } else {
        // Within a system they don't: worked out once per system.
        let mut cache = app.sky_cache.borrow_mut();
        if cache.as_ref().is_none_or(|(o, _)| *o != app.view.origin) {
            *cache = Some((app.view.origin, others().map(star).collect()));
        }
        for &(dir, c) in &cache.as_ref().expect("filled").1 {
            frame.sky_point(dir, c.scale(starlight));
        }
    }
    // The gate network, once we're zoomed out far enough to see it as a map.
    if app.mode == Mode::Observer && cam.length() > 1.0e15 {
        let amber = Color::hex(0xffc040);
        let mut nodes = Vec::new();
        for &(a, b) in &app.charts.gate_links {
            let pa = (g.stars[a].position - origin) * LIGHT_YEAR;
            let pb = (g.stars[b].position - origin) * LIGHT_YEAR;
            frame.line(pa, pb, amber.scale(0.6));
            nodes.extend([a, b]);
        }
        nodes.sort();
        nodes.dedup();
        for n in nodes {
            let at = (g.stars[n].position - origin) * LIGHT_YEAR;
            if let Some(p) = frame.project(at) {
                frame.hud_ellipse(p, Vec2::splat(5.0), 10, amber);
                frame.text(p + Vec2::new(8.0, 4.0), &star_name(g.stars[n].seed).to_uppercase(), amber);
            }
        }
    }
}

/// Between gates: rings rushing past, straight ahead.
fn transit_tunnel(frame: &mut Frame, app: &App) {
    let cam = frame.camera.position;
    let fwd = frame.camera.forward().as_dvec3();
    let spacing = 150.0;
    let shift = (app.now() * 900.0) % spacing;
    for k in 0..28 {
        let z = k as f64 * spacing - shift + 20.0;
        let fade = (1.0 - z / (28.0 * spacing)) as f32;
        let c = if k % 2 == 0 { Color::hex(0xffc040) } else { Color::hex(0x40c0ff) };
        frame.circle(cam + fwd * z, fwd, 260.0, 40, c.scale(fade.max(0.1)));
    }
}

/// How long a gate flashes as a ship goes through it, and before one comes
/// out of it (s).
const GATE_FLASH: f64 = 2.5;

/// Other ships going through the gates here: a flash where one crosses the
/// ring and is gone (a burst in the ring's plane, a streak out along its
/// axis), and one where another is about to come through (the ring's light
/// gathering to a point).
fn gate_flashes(frame: &mut Frame, app: &App) {
    use universe_sim::world::gate::TRANSIT_TIME;
    let sys = &app.view.system;
    let here = app.view.origin;
    for c in app.v.crafts.iter() {
        let ShipState::Transit { to, from, remaining, local_offset, .. } = c.ship.state else { continue };
        // (Leaving here: the gate to where it's going; coming here: the gate from where it was.)
        let (gate, since, leaving) = if from == here {
            (sys.gate_to(to), TRANSIT_TIME - remaining, true)
        } else if to == here {
            (sys.gate_to(from), GATE_FLASH - remaining, false)
        } else {
            continue;
        };
        let Some(g) = gate else { continue };
        if !(0.0..GATE_FLASH).contains(&since) {
            continue;
        }
        let f = GateFrame::new(sys, g, app.now(), &app.view.positions);
        let axis = f.axis();
        let at = f.center + f.rotation * local_offset;
        if frame.projected_radius(at, 400.0) < 0.5 {
            continue;
        }
        let k = since / GATE_FLASH;
        // A starburst in the ring's plane, white at its heart.
        let u = f.rotation * DVec3::X;
        let v = axis.cross(u);
        let burst = |frame: &mut Frame, length: f64, ring: f64, bright: f32| {
            for i in 0..12 {
                let a = i as f64 * std::f64::consts::TAU / 12.0;
                let d = u * a.cos() + v * a.sin();
                let reach = if i % 2 == 0 { length } else { length * 0.55 };
                frame.line(at + d * 15.0, at + d * reach, Color::WHITE.scale(bright));
            }
            frame.circle(at, axis, ring, 40, Color::hex(0x80e0ff).scale(bright));
            frame.circle(at, axis, ring * 0.6, 32, Color::hex(0xffd080).scale(bright * 0.8));
            frame.point(at, Color::WHITE.scale(bright));
        };
        if leaving {
            // Out: the burst flaring and spreading, and a streak out through the ring.
            let bright = (1.0 - k) as f32;
            burst(frame, 150.0 + 650.0 * k, 50.0 + 800.0 * k, bright);
            // (Through it, the way it leads.)
            frame.line(at, at + axis * 4000.0 * (1.0 - k), Color::WHITE.scale(bright));
        } else {
            // In: light gathering to a point where it will come out.
            burst(frame, 150.0 + 650.0 * (1.0 - k), 50.0 + 800.0 * (1.0 - k), (k as f32).max(0.25));
        }
    }
}

/// Transit guidance: the ring's axis through both sides, the run-in point on
/// our side, our drift, and the flight plan tunnel.
fn transit_guide(frame: &mut Frame, app: &App, gate: usize, st: &GateStatus) {
    let f = GateFrame::new(&app.view.system, gate, app.now(), &app.view.positions);
    let axis = f.axis();
    let c = if st.in_corridor { GUIDE_OK } else { GUIDE_OFF };
    frame.line(f.center - axis * APPROACH_DISTANCE, f.center + axis * APPROACH_DISTANCE, c.scale(0.4));
    // The run-in starts behind the ring (it's one way: along its axis).
    let p = f.center - axis * APPROACH_DISTANCE;
    // Arrowheads along the axis: the way through.
    let (e1, e2) = (f.rotation * DVec3::X, f.rotation * DVec3::Z);
    for k in 1..4 {
        let tip = f.center - axis * (APPROACH_DISTANCE * k as f64 / 4.0);
        for d in [e1, -e1, e2, -e2] {
            frame.line(tip, tip - axis * 200.0 + d * 150.0, c.scale(0.6));
        }
    }
    for d in [e1, e2, axis] {
        frame.line(p - d * 120.0, p + d * 120.0, c);
    }
    if app.ship.is_flying() {
        drift_line(frame, app.view.ship_pos, st.relative_velocity);
        if let Some(plan) = &app.plan {
            plan_path(frame, app, plan, app.now(), app.view.ship_pos);
        }
    }
}

fn orbits(frame: &mut Frame, app: &App) {
    let sys = &app.view.system;
    let cam = frame.camera.position;
    for b in &sys.bodies {
        let (Some(parent), Some(orbit)) = (b.rail.parent, &b.rail.orbit) else { continue };
        let center = app.view.positions[parent];
        // Moon and station orbits only when we're near their planet.
        if parent != 0 && center.distance(cam) > orbit.semi_major_axis * 40.0 {
            continue;
        }
        let dim = if b.kind == BodyKind::Station { 0.2 } else { 0.3 };
        let segments = if parent == 0 { 160 } else { 64 };
        let pts: Vec<DVec3> = orbit.path(segments).map(|p| center + p).collect();
        let c = color(b.color).scale(dim);
        for k in 0..pts.len() {
            frame.line(pts[k], pts[(k + 1) % pts.len()], c);
        }
    }
}

/// A terrain globe bigger than this on screen (radius, px) gets its full mesh.
const GLOBE_FULL_PX: f32 = 90.0;

fn bodies(frame: &mut Frame, app: &App) {
    let sys = &app.view.system;
    let t = app.now();
    let cam = frame.camera.position;
    for (i, b) in sys.bodies.iter().enumerate() {
        // (Asteroids: see `rocks`.)
        if b.kind == BodyKind::Asteroid {
            continue;
        }
        let center = app.view.positions[i];
        let c = color(b.color);
        let px = frame.projected_radius(center, b.rail.radius);
        let rotation = b.rotation(t).as_quat();

        if b.kind == BodyKind::Gate {
            if frame.projected_radius(center, b.rail.radius) > 0.8 {
                let t = Transform { position: center, rotation, scale: 1.0 };
                let metal = Color::hex(0x767d85);
                frame.with_surface(0.45, 44.0, 0.0, |frame| frame.model_shaded(&app.models.gate, &t, metal.scale(0.7), metal));
                gate_lights(frame, center, b.rotation(app.now()), app.now());
            } else {
                frame.point(center, c);
            }
            continue;
        }
        if b.kind == BodyKind::Station {
            if px > 0.8 {
                let t = Transform { position: center, rotation, scale: 1.0 };
                frame.with_surface(0.2, 24.0, 0.0, |frame| frame.model_shaded(&app.models.station, &t, HULL.scale(0.75), HULL));
                station_lights(frame, app, i, center, b.rotation(app.now()));
            } else {
                frame.point(center, c.scale(0.8));
            }
            continue;
        }
        if px < 1.5 {
            frame.point(center, if b.kind == BodyKind::Star { c } else { c.scale(0.9) });
            continue;
        }

        if let Some((full, coarse)) = app.globes.get(&(app.view.origin, i)) {
            // Small on screen: the coarse mesh does (a sixteenth of the triangles).
            let globe = if px > GLOBE_FULL_PX { full } else { coarse };
            // Terrain world: colored globe; near the surface, a local grid on
            // the ground (the globe drops a hair so the grid sits on top).
            let near = cam.distance(center) - b.rail.radius < terrain_view::near_altitude(b);
            let scale = if near { b.rail.radius * 0.998 } else { b.rail.radius };
            universe_prof::time("draw/scene/bodies/globe mesh", || frame.no_shadow(|frame| frame.model_colored_shaded(globe, &Transform { position: center, rotation, scale }, if app.show_grid { grid_detail(px) } else { 0.0 }, terrain_view::FILL * 2.5)));
            if near {
                universe_prof::time("draw/scene/bodies/surface grid", || terrain_view::surface_grid(frame, b, center, t, None, app.show_grid));
                // On foot here: a fine grid underfoot.
                if let universe_sim::world::Place::Outside { body, .. } = app.v.crew.place
                    && body == i
                    && app.view.origin == app.v.ship_system
                {
                    terrain_view::surface_grid(frame, b, center, t, Some(4.0), app.show_grid);
                }
            }
            if frame.projected_radius(center, b.rail.radius) > 150.0 {
                universe_prof::time("draw/scene/bodies/crater rims", || terrain_view::crater_rims(frame, b, center, t));
            }
        } else {
            let (model, fill): (&universe_engine::Mesh, Color) = match b.kind {
                BodyKind::Star => (&app.models.star, c),
                BodyKind::Rocky => (&app.models.rocky, c.scale(0.4)),
                BodyKind::GasGiant | BodyKind::IceGiant => (&app.models.giant, c.scale(0.45)),
                _ => (&app.models.moon, c.scale(0.35)),
            };
            let at = Transform { position: center, rotation, scale: b.rail.radius };
            // The star shines (a faint grid only when it fills the view);
            // everything else is lit by it.
            frame.no_shadow(|frame| {
                if b.kind == BodyKind::Star {
                    // (Its surface white-hot, no grid on it: it blooms.)
                    let hot = Color([fill.0[0] * 5.0 + 1.0, fill.0[1] * 5.0 + 1.0, fill.0[2] * 5.0 + 1.0, 1.0]);
                    frame.model(model, &at, Color([0.0, 0.0, 0.0, 0.0]), hot);
                } else {
                    frame.model_shaded_faded(model, &at, c, fill, if app.show_grid { grid_detail(px) } else { 0.0 });
                }
            });
        }

        // Air: its rim lit by the sun; settled worlds, their lights by night.
        if b.rail.atmosphere.is_some() {
            atmosphere(frame, app, i, center);
        }
        city_lights(frame, app, i, center);

        // True silhouette outline, plus a halo for stars.
        let to = center - cam;
        let d = to.length();
        if d > b.rail.radius {
            let dir = to / d;
            // (A planet's own surface shows it: no outline. The star: its halo, for now.)
            let rings: &[(f64, f32)] = if b.kind == BodyKind::Star { &[(1.12, 0.45), (1.3, 0.2)] } else { &[] };
            for &(k, brightness) in rings {
                let r = b.rail.radius * k;
                if d > r {
                    let center_offset = center - dir * (r * r / d);
                    let radius = r * (1.0 - (r * r) / (d * d)).sqrt();
                    frame.circle(center_offset, dir, radius, 96, c.scale(brightness));
                }
            }
        }

        if let Some((inner, outer)) = b.rings {
            let normal = b.rail.tilt * DVec3::Y;
            for k in 0..4 {
                let r = inner + (outer - inner) * k as f64 / 3.0;
                frame.circle(center, normal, r, 96, c.scale(0.55 - 0.1 * k as f32));
            }
        }
    }
}

/// How much of a body's latitude/longitude grid to show at `px` pixels of
/// radius on screen: none when small (a lit disc and its outline circle are
/// enough), fading in as it grows, and never more than a light touch (the
/// shading carries the shape).
fn grid_detail(px: f32) -> f32 {
    0.5 * ((px - 80.0) / 420.0).clamp(0.0, 1.0)
}

pub const TRAFFIC: Color = Color::hex(0x50d8ff);
/// Ships that are aggressed (fair game).
pub const AGGRESSED: Color = Color::hex(0xff4040);
/// Hull plating of stations (and ships of no livery), as lit by the star.
const HULL: Color = Color::hex(0x6c7278);
/// Metal: a hull's glint in the sun (strength, sharpness).
const METAL: (f32, f32) = (0.35, 36.0);

/// A ship's paint scheme by its trade (see `models::SCHEMES`; ours the first).
fn livery(name: &str) -> usize {
    match name.split(' ').next().unwrap_or("") {
        "" => 0,
        "Trader" => 1,
        "Pirate" => 2,
        "Miner" => 3,
        "Shuttle" => 4,
        _ => 5,
    }
}

/// A world's air seen edge-on: a band round its limb, the sky's colour
/// where the sun lights it (a warm band at the terminator), dark at night,
/// fading out as it thins with height.
fn atmosphere(frame: &mut Frame, app: &App, i: usize, center: DVec3) {
    let b = &app.view.system.bodies[i];
    let cam = frame.camera.position;
    let to = center - cam;
    let d = to.length();
    let r = b.rail.radius;
    if d <= r * 1.02 {
        return;
    }
    let Some(star) = app.view.system.bodies.iter().position(|b| b.kind == BodyKind::Star) else { return };
    let sun = (app.view.positions[star] - center).normalize();
    let dir = to / d;
    // The limb: the circle where the line of sight grazes the surface.
    let limb_c = center - dir * (r * r / d);
    let limb_r = r * (1.0 - (r * r) / (d * d)).sqrt();
    let thick = 1.0 + 0.035_f64.max(100_000.0 / r);
    let (u, v) = (dir.any_orthonormal_vector(), dir.cross(dir.any_orthonormal_vector()));
    // (Earth-like: sky blue; others their own colour, paler.)
    let sky = match b.terrain.as_ref().map(|t| t.kind) {
        Some(universe_sim::TerrainKind::Terran) => [0.35, 0.6, 1.0],
        _ => {
            let [cr, cg, cb] = b.color;
            [0.4 + 0.6 * cr, 0.4 + 0.6 * cg, 0.4 + 0.6 * cb]
        }
    };
    let n = 96;
    let point = |k: usize, s: f64| {
        let a = k as f64 / n as f64 * std::f64::consts::TAU;
        let out = u * a.cos() + v * a.sin();
        (limb_c + out * limb_r * s, out)
    };
    let light = |out: DVec3| {
        // Lit by how far round to the sun this edge is; warm at the terminator.
        let day = out.dot(sun);
        let lit = ((day + 0.25) / 1.25).clamp(0.0, 1.0) as f32;
        let warm = (1.0 - (day.abs() / 0.3)).clamp(0.0, 1.0) as f32;
        let k = 2.4 * lit;
        [k * (sky[0] + 0.8 * warm), k * (sky[1] + 0.25 * warm), k * sky[2] * (1.0 - 0.5 * warm)]
    };
    for k in 0..n {
        let ((a0, o0), (a1, o1)) = (point(k, 1.0), point(k + 1, 1.0));
        let ((b0, _), (b1, _)) = (point(k, thick), point(k + 1, thick));
        let (l0, l1) = (light(o0), light(o1));
        frame.glow_triangle([a0, a1, b1], [l0, l1, [0.0; 3]]);
        frame.glow_triangle([a0, b1, b0], [l0, [0.0; 3], [0.0; 3]]);
    }
}

/// A settled world's lights by night: round each of its spaceports, towns
/// scattered about the city, as many as it has people; only where the sun's down.
fn city_lights(frame: &mut Frame, app: &App, i: usize, center: DVec3) {
    let sys = &app.view.system;
    let b = &sys.bodies[i];
    let Some(star) = sys.bodies.iter().position(|b| b.kind == BodyKind::Star) else { return };
    let rot = b.rotation(app.now());
    let sun = (app.view.positions[star] - center).normalize();
    // Too far to show a light from: skip.
    if frame.projected_radius(center, b.rail.radius) < 4.0 {
        return;
    }
    for (k, port) in sys.spaceports.iter().enumerate().filter(|(_, p)| p.body == i) {
        let people = app.v.economy.iter().find(|p| p.system == app.view.origin && p.facility == universe_sim::world::Facility::Spaceport(k)).map_or(0.0, |p| p.population);
        if people <= 0.0 {
            continue;
        }
        let towns = (people * 1.5).clamp(6.0, 120.0) as usize;
        let (e1, e2) = universe_sim::world::spaceport::tangent(port.direction);
        for t in 0..towns {
            let h = |s: u64| {
                let mut x = (k as u64 * 7919 + t as u64 * 104_729 + s).wrapping_mul(0x9E37_79B9_7F4A_7C15);
                x ^= x >> 29;
                (x % 10_000) as f64 / 10_000.0
            };
            // (Most close in, some far out along the roads.)
            let spread = 0.12 * h(1).powf(1.8);
            let a = h(2) * std::f64::consts::TAU;
            let local = (port.direction + (e1 * a.cos() + e2 * a.sin()) * spread).normalize();
            let world = rot * local;
            let night = -world.dot(sun);
            if night < 0.05 {
                continue;
            }
            let glow = (night.min(0.4) / 0.4) as f32 * (0.6 + 1.4 * h(3) as f32) * if t == 0 { 2.5 } else { 1.0 };
            frame.glow(center + world * (b.surface_radius(local) + 50.0), 1500.0, [2.2 * glow, 1.6 * glow, 0.8 * glow], 0.8);
        }
    }
}

/// Motes of dust round the eye, at rest in the frame of the body nearest
/// (scattered one to a cell of a grid there, so they hold still), each a
/// short streak along our motion against them: drift, braking and slip seen
/// at a glance. Fainter with distance; none in the hyperdrive.
fn dust(frame: &mut Frame, app: &App) {
    const CELL: f64 = 50.0;
    const REACH: f64 = 120.0;
    let ship = &app.v.ship;
    if app.mode != Mode::Pilot || ship.hyperdrive || !app.v.crew.seated() {
        return;
    }
    let sys = &app.view.system;
    let cam = frame.camera.position;
    let Some(i) = (0..sys.bodies.len()).min_by(|&a, &b| {
        let d = |k: usize| app.view.positions[k].distance(cam) - sys.bodies[k].rail.radius;
        d(a).total_cmp(&d(b))
    }) else {
        return;
    };
    let origin = app.view.positions[i];
    // (At rest with the body as it turns: near a world, with its ground and air.)
    let v = ship.velocity - sys.velocity(i, app.now()) - sys.bodies[i].angular_velocity().cross(cam - origin);
    let speed = v.length();
    // (Nearly still against them: a mote, not a streak.)
    let along = if speed > 0.3 { -v / speed } else { DVec3::Y };
    let streak = if speed > 3.0 { (speed * 0.05).clamp(0.4, 22.0) } else { 0.12 };
    let local = cam - origin;
    let c0 = (local / CELL).floor();
    let n = (REACH / CELL).ceil() as i64;
    let hash = |x: i64, y: i64, z: i64, k: u64| {
        let mut h = (x as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ (y as u64).wrapping_mul(0xC2B2_AE3D_27D4_EB4F) ^ (z as u64).wrapping_mul(0x1656_67B1_9E37_79F9) ^ k;
        h ^= h >> 31;
        h = h.wrapping_mul(0xBF58_476D_1CE4_E5B9);
        (h >> 11) as f64 / (1u64 << 53) as f64
    };
    for dx in -n..=n {
        for dy in -n..=n {
            for dz in -n..=n {
                let (x, y, z) = (c0.x as i64 + dx, c0.y as i64 + dy, c0.z as i64 + dz);
                let off = DVec3::new(hash(x, y, z, 1), hash(x, y, z, 2), hash(x, y, z, 3));
                let p = origin + (DVec3::new(x as f64, y as f64, z as f64) + off) * CELL;
                let d = p.distance(cam);
                if d > REACH || d < 2.0 {
                    continue;
                }
                // (Faint: about a tenth there, fading out toward the reach.)
                let fade = (1.0 - d / REACH).powf(0.6) as f32;
                let c = Color([0.8, 0.85, 0.9, 0.2 * fade]);
                frame.line(p, p + along * streak, c);
            }
        }
    }
}

/// A station's lights: a lamp at each pad's corners (the pad we're
/// cleared for green, the rest a dim warm white), its window band lit warm,
/// the hangar's mouth spilling light, red beacons blinking on the
/// structure's top corners.
fn station_lights(frame: &mut Frame, app: &App, body: usize, center: DVec3, rot: DQuat) {
    use universe_sim::world::station::{pad_local, DECK_FROM, DECK_HALF, DECK_TOP, STRUCTURE_FROM, STRUCTURE_TOP};
    let at = |p: DVec3| center + rot * p;
    let ours = app.v.avionics.clearance.and_then(|c| match (c.target, c.pad) {
        (universe_sim::NavTarget::Station(s), universe_sim::avionics::nav::PadSlot::Pad(k)) if s == body => Some(k),
        _ => None,
    });
    let deck = DECK_TOP + 0.6;
    for k in 0..universe_sim::world::spaceport::PADS {
        let c = pad_local(k);
        let (light, least) = if ours == Some(k) { ([0.3, 5.0, 0.9], 2.0) } else { ([1.8, 1.4, 0.9], 1.3) };
        for (dx, dz) in [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)] {
            frame.glow(at(DVec3::new(c.x + dx * 35.0, deck, c.z + dz * 35.0)), 1.2, light, least);
        }
    }
    // The window band, over the hangar: warm, along the face over the deck.
    let face = DECK_FROM + 1.0;
    let w = DECK_HALF - 30.0;
    for k in 0..24 {
        let x = -w + 2.0 * w * (k as f64 + 0.5) / 24.0;
        frame.glow(at(DVec3::new(x, DECK_TOP + 162.0, face)), 4.0, [1.6, 1.2, 0.7], 0.9);
    }
    // The hangar's mouth.
    frame.glow(at(DVec3::new(0.0, DECK_TOP + 30.0, face)), 70.0, [0.5, 0.42, 0.3], 0.0);
    // Beacons on the structure's top corners, blinking together.
    if (app.now() * 0.5).fract() < 0.12 {
        for (x, z) in [(-DECK_HALF, STRUCTURE_FROM), (DECK_HALF, STRUCTURE_FROM), (-DECK_HALF, DECK_FROM), (DECK_HALF, DECK_FROM)] {
            frame.glow(at(DVec3::new(x, STRUCTURE_TOP + 2.0, z)), 3.0, [6.0, 0.3, 0.2], 1.6);
        }
    }
}

/// A gate's running lights: the side it's entered from steady white, the
/// side it leaves by green, a pulse chasing round it (the way through).
fn gate_lights(frame: &mut Frame, center: DVec3, rot: DQuat, now: f64) {
    use universe_sim::world::gate::{GATE_RADIUS, RING_TUBE};
    let n = 32;
    for k in 0..n {
        let a = k as f64 / n as f64 * std::f64::consts::TAU;
        let dir = DVec3::new(a.cos(), 0.0, a.sin()) * GATE_RADIUS;
        // (The gate's axis, +Y, faces where it goes.)
        frame.glow(center + rot * (dir - DVec3::Y * RING_TUBE * 1.05), 14.0, [3.5, 3.5, 3.8], 2.2);
        let chase = ((k as f64 / n as f64 - now * 0.25).rem_euclid(1.0) * 8.0).fract();
        let lit = if chase < 0.25 { 6.0 } else { 1.2 };
        frame.glow(center + rot * (dir + DVec3::Y * RING_TUBE * 1.05), 14.0, [0.15 * lit, 1.0 * lit, 0.4 * lit], 2.2);
    }
}

/// A ship's lights: white strobes on its wing tips flashing together, its
/// tail strobe between them, each ship on its own beat (`seed`).
fn nav_lights(frame: &mut Frame, lights: [DVec3; 3], pos: DVec3, turned: DQuat, now: f64, seed: usize) {
    let at = |p: DVec3| pos + turned * p;
    // (A short flash every three seconds or so: there, not distracting.)
    let beat = (now * 0.33 + seed as f64 * 0.137).fract();
    let white = [4.5, 4.5, 5.0];
    if beat < 0.03 {
        frame.glow(at(lights[0]), 0.8, white, 1.6);
        frame.glow(at(lights[1]), 0.8, white, 1.6);
    }
    if (0.5..0.53).contains(&beat) {
        frame.glow(at(lights[2]), 0.9, white, 1.8);
    }
}

/// A hull drawn solid in its paint (colours in the mesh), metal glinting in the sun.
fn hull_model(frame: &mut Frame, mesh: &universe_engine::Mesh, t: &Transform) {
    frame.with_surface(METAL.0, METAL.1, 0.0, |frame| frame.model_colored_shaded(mesh, t, 0.55, 0.62));
}

/// A ship's detail: its engine bells (dark, heat-stained metal) and its
/// canopy (dark glass, glinting).
fn hull_detail(frame: &mut Frame, (bells, glass): (&universe_engine::Mesh, &universe_engine::Mesh), t: &Transform) {
    // (Painted per corner, heat-tinted. Too small for the shadow map to cast
    // them cleanly: they cast none.)
    frame.no_shadow(|frame| frame.with_surface(0.9, 40.0, 0.0, |frame| frame.model_colored_shaded(bells, t, 0.35, 1.0)));
    let tint = Color::hex(0x1c2630);
    // (Glass glinting in the sun; a faint trace of the cockpit's own light through it.)
    frame.with_surface(1.4, 90.0, 0.08, |frame| frame.model_shaded(glass, t, tint.scale(1.6), tint));
}

/// Other ships in the system being viewed.
fn crafts(frame: &mut Frame, app: &App) {
    let cam = frame.camera.position;
    let mut names: Vec<(f64, DVec3, String, Color)> = Vec::new();
    for (i, c) in app.v.crafts.iter().enumerate() {
        let visible = (c.ship.is_flying() || matches!(c.ship.state, ShipState::Landed { .. } | ShipState::Anchored { .. })) && c.ship.hangar.is_none();
        if c.system != app.view.origin || !visible {
            continue;
        }
        let (pos, turned) = app.place(crate::Who::Craft(i));
        if frame.projected_radius(pos, 25.0) < 1.0 {
            let tc = if c.aggressed { AGGRESSED } else { TRAFFIC };
            frame.point(pos, tc.scale(0.8));
            continue;
        }
        let t = Transform { position: pos, rotation: turned.as_quat(), scale: 1.0 };
        let tc = if c.aggressed { AGGRESSED } else { TRAFFIC };
        hull_model(frame, app.models.painted(&c.ship, livery(&c.name)), &t);
        hull_detail(frame, app.models.detail(&c.ship), &t);
        nav_lights(frame, app.models.lights(&c.ship), pos, turned, app.now(), i);
        jets(frame, &c.ship, pos, turned, app.now(), i);
        if pos.distance(cam) < 5_000.0 {
            names.push((pos.distance(cam), pos, c.name.to_uppercase(), tc.scale(0.8)));
        }
    }
    // Their names, nearest first, none over another (a crowd shows its nearest).
    names.sort_by(|a, b| a.0.total_cmp(&b.0));
    let mut placed = Labels { placed: Vec::new() };
    for (_, pos, name, c) in names {
        placed.add(frame, pos, &name, c);
    }
}

/// The collision warning: the path ahead (drawn with its reference as it
/// moves), cyan fading to red toward an impact, and a red cross where it hits.
fn collision_path(frame: &mut Frame, app: &App) {
    let Some(p) = app.collision.as_ref().filter(|_| crate::hud::impact_shown(app)) else { return };
    if app.view.origin != app.v.ship_system || app.mode != Mode::Pilot {
        return;
    }
    let anchor = app.view.positions[p.reference];
    let hit = p.collision.as_ref();
    let end = p.path.last().map_or(1.0, |x| x.0).max(1e-6);
    for w in p.path.windows(2) {
        let k = (w[1].0 / end) as f32;
        let c = if hit.is_some() { Color::hex(0x40c0ff).lerp(Color::hex(0xff3030), k) } else { Color::hex(0x40c0ff).scale(1.0 - 0.6 * k) };
        frame.line(anchor + w[0].1, anchor + w[1].1, c);
    }
    if let Some(c) = hit {
        let at = anchor + c.offset;
        let size = (at.distance(frame.camera.position) * 0.02).max(5.0);
        let red = Color::hex(0xff3030);
        let (u, v) = (frame.camera.orientation.as_dquat() * DVec3::X, frame.camera.orientation.as_dquat() * DVec3::Y);
        frame.line(at - (u + v) * size, at + (u + v) * size, red);
        frame.line(at - (u - v) * size, at + (u - v) * size, red);
    }
}

/// Slugs in flight as short tracers (streaked along their motion relative to
/// us), and this frame's laser beams.
fn weapons_fire(frame: &mut Frame, app: &App) {
    let own = app.ship.velocity;
    // (Carried to the moment drawn by their own motion.)
    let back = app.now() - app.v.time;
    for &(system, p, velocity) in &app.v.slugs {
        if system != app.view.origin {
            continue;
        }
        let p = p + velocity * back;
        let rel = velocity - own;
        let streak = rel.normalize_or_zero() * (rel.length() * 0.02).clamp(4.0, 60.0);
        frame.line(p - streak, p, Color::hex(0xffd060));
        frame.point(p, Color::hex(0xffe080));
    }
    // Missiles: a dart along their flight, and while the motor burns, its plume.
    for &(system, p, velocity, burning, _) in &app.v.missiles {
        if system != app.view.origin {
            continue;
        }
        let p = p + velocity * back;
        let along = velocity.normalize_or(DVec3::Y);
        frame.line(p - along * 6.0, p + along * 2.0, Color::hex(0xe0e0e0));
        if burning {
            let flicker = 0.7 + 0.3 * ((app.now() * 37.0).sin() * 0.5 + 0.5);
            frame.line(p - along * 6.0, p - along * 40.0, Color::hex(0xffa040).scale(flicker as f32));
        }
        frame.point(p, if burning { Color::hex(0xffd080) } else { Color::hex(0xa0a0a0) });
    }
    for beam in &app.v.beams {
        if beam.system != app.view.origin {
            continue;
        }
        frame.line(beam.from, beam.to, Color::hex(0xff4030));
        if beam.hit {
            frame.point(beam.to, Color::hex(0xffffa0));
        }
    }
}

pub const GUIDE_OK: Color = Color::hex(0xdcebf2);
pub const GUIDE_OFF: Color = Color::hex(0xffc040);

/// The approach to our pad: its square on the deck, squares to come down
/// through along its vertical, the point where the descent begins; our
/// drift and the flight plan.
fn docking_guide(frame: &mut Frame, app: &App, station: usize, status: &DockingStatus) {
    let f = StationFrame::new(&app.view.system, station, app.now(), &app.view.positions);
    let c = if status.lined_up { GUIDE_OK } else { GUIDE_OFF };
    let (e1, e2, up) = (f.rotation * DVec3::X, f.rotation * DVec3::Z, f.up());
    let pad = f.pad(status.pad);
    let deck = pad - up * (universe_sim::world::ship::SHIP_RADIUS - 1.0);
    let square = |frame: &mut Frame, at: DVec3, half: f64, color: Color| {
        let corners = [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)].map(|(a, b)| at + e1 * (a * half) + e2 * (b * half));
        for i in 0..4 {
            frame.line(corners[i], corners[(i + 1) % 4], color);
        }
    };
    square(frame, deck, 40.0, Color::hex(0xdcebf2));
    frame.line(deck, pad + up * APPROACH_HEIGHT, c.scale(0.35));
    for h in [60.0, 150.0, 300.0, 600.0, 1000.0, 1500.0, APPROACH_HEIGHT] {
        // The square the ship comes down through next is brightest.
        let next = h < status.height && h > status.height - 400.0;
        square(frame, pad + up * h, 30.0 + h * 0.03, c.scale(if next { 1.0 } else { 0.55 }));
    }
    // Where the descent begins: a 3D cross.
    let p = pad + up * APPROACH_HEIGHT;
    for d in [e1, e2, up] {
        frame.line(p - d * 120.0, p + d * 120.0, c);
    }

    if app.ship.is_flying() {
        drift_line(frame, app.view.ship_pos, status.relative_velocity);
        if let Some(plan) = &app.plan {
            plan_path(frame, app, plan, app.now(), app.view.ship_pos);
        }
    }
}

/// Straight-line drift relative to the target for 30 s, dashed, with a tick every 5 s.
fn drift_line(frame: &mut Frame, ship: DVec3, v: DVec3) {
    if v.length() <= 0.5 {
        return;
    }
    let tick_axis = v.normalize().any_orthonormal_vector();
    for k in 0..30 {
        if k % 2 == 0 {
            frame.line(ship + v * k as f64, ship + v * (k + 1) as f64, PREDICT);
        }
        if k % 5 == 4 {
            let at = ship + v * (k + 1) as f64;
            let size = (v.length() * 0.6).clamp(3.0, 60.0);
            frame.line(at - tick_axis * size, at + tick_axis * size, PREDICT);
        }
    }
}

pub const BURN: Color = Color::hex(0xffa040);

pub fn action_color(a: Action) -> Color {
    match a {
        Action::Burn(_) => BURN,
        Action::Thrusters => GUIDE_PATH,
        Action::Coast => GUIDE_PATH.scale(0.45),
    }
}

/// The flight plan as a tunnel: a center line along the path, and frames
/// shaped and turned like the ship at that point (wide like the wings, with a
/// nose tick), so the pilot sees ahead of time where to be and which way to face.
/// Where the plan's reference (station, gate, or a port's planet) is now.
pub fn plan_reference(app: &App) -> Option<DVec3> {
    let target = app.v.avionics.clearance?.target;
    let i = match target {
        universe_sim::NavTarget::Station(s) | universe_sim::NavTarget::Gate(s) | universe_sim::NavTarget::Asteroid(s) => s,
        universe_sim::NavTarget::Spaceport(p) => app.view.system.spaceports.get(p)?.body,
    };
    app.view.positions.get(i).copied()
}

/// Distance still to fly along the planned path.
pub fn path_length(plan: &Plan) -> f64 {
    plan.points.windows(2).map(|w| w[0].position.distance(w[1].position)).sum()
}

/// Where `plan` has the ship at absolute time `abs` (in the plan's own frame), if it reaches then.
fn plan_sample(plan: &Plan, abs: f64) -> Option<DVec3> {
    let at = abs - plan.start;
    let pts = &plan.points;
    if pts.is_empty() || at < 0.0 || at > pts.last()?.time {
        return None;
    }
    let j = pts.iter().rposition(|p| p.time <= at)?;
    let (a, b) = (pts[j], pts[(j + 1).min(pts.len() - 1)]);
    let span = b.time - a.time;
    let u = if span > 0.0 { ((at - a.time) / span).clamp(0.0, 1.0) } else { 0.0 };
    Some(a.position.lerp(b.position, u))
}

fn plan_path(frame: &mut Frame, app: &App, plan: &Plan, now: f64, ship: DVec3) {
    // The plan may be a few frames old: carry it along with its reference.
    let Some(center_now) = plan_reference(app) else { return };
    // (While a follow program has the guide, its own drawing shows it.)
    if !matches!(app.guide.key(), Some(GuideKey::Clearance(_))) {
        return;
    }
    guided_path(frame, app, plan, center_now, app.plan_prev.as_deref().filter(|_| app.plan_blend < 1.0), now, ship);
}

/// A planned path and its guide frames, drawn the one way for every kind of
/// guidance (landing, docking, gates, the follow programs): the plan, its
/// reference's center now, the plan before (to ease from).
pub fn guided_path(frame: &mut Frame, app: &App, plan: &Plan, center_now: DVec3, prev: Option<&Plan>, now: f64, ship: DVec3) {
    guided_path_with(frame, app, plan, center_now, prev, now, ship, true);
}

/// `guided_path`, with its frames or (`frames` false) the line alone.
#[allow(clippy::too_many_arguments)]
pub fn guided_path_with(frame: &mut Frame, app: &App, plan: &Plan, center_now: DVec3, prev: Option<&Plan>, now: f64, ship: DVec3, frames: bool) {
    let new_place = plan.anchor(center_now, now);
    // Eased from the plan before, at the same moment of absolute time (so
    // rebuilds glide rather than jump).
    let w = app.plan_blend as f64;
    let blend = |p: DVec3, abs: f64| -> DVec3 {
        let now_pos = new_place(p);
        match prev.and_then(|old| plan_sample(old, abs).map(|q| old.anchor(center_now, now)(q))) {
            Some(old) => old.lerp(now_pos, w),
            None => now_pos,
        }
    };
    let pts = &plan.points;
    if pts.len() < 2 {
        return;
    }
    // Center line, colored by what the ship is doing; dashed while coasting.
    for (i, w) in pts.windows(2).enumerate() {
        let c = action_color(w[0].action);
        if !(w[0].action == Action::Coast && i % 2 == 1) {
            frame.line(blend(w[0].position, plan.start + w[0].time), blend(w[1].position, plan.start + w[1].time), c);
        }
    }

    if frames {
        app.guide.draw(frame, center_now, now, ship);
    }
}

/// The guide frames: gates set in space along the planned route, fixed in
/// the target's own frame (they ride and turn with the station, gate or
/// planet), at set distances back from the goal along the path: closer
/// together near it. Where the ship is doesn't move them, so holding still
/// they hold still, and flying on you go through them. A new plan moves a
/// frame only if its spot moved by more than a quarter of its spacing.
/// An evenly spaced frame's half-width (m): an arch the ship flies through
/// with room to spare (its height is 0.4 of that).
const ARCH: f64 = 3.5 * universe_sim::world::ship::SHIP_RADIUS;

/// What a guide's frames lead to: a clearance's target, or what a follow
/// program follows (the same frames, one way of drawing guidance).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum GuideKey {
    Clearance(universe_sim::NavTarget),
    Follow(universe_sim::avionics::follow::Anchor),
}

#[derive(Default)]
pub struct Guide {
    target: Option<GuideKey>,
    spin: DVec3,
    /// By rung on the ladder of distances from the goal.
    frames: std::collections::BTreeMap<u32, GuideFrame>,
    /// Distance from the goal along the latest plan to the ship.
    left: f64,
}

#[derive(Clone, Copy)]
struct GuideFrame {
    /// Distance back from the goal along the path (m).
    from_goal: f64,
    /// In the target's frame: where, which way the path goes, which way the ship should face.
    at: DVec3,
    along: DVec3,
    facing: DVec3,
    size: f64,
    action: Action,
}

/// The ladder: rung `k`'s distance from the goal, and the gap to the next.
fn rung(k: u32) -> (f64, f64) {
    let gap = |d: f64| (0.25 * d).clamp(60.0, 20_000.0);
    let mut d = 60.0;
    for _ in 0..k {
        d += gap(d);
    }
    (d, gap(d))
}

/// Most frames ahead shown.
const MAX_FRAMES: usize = 16;

impl Guide {
    pub fn clear(&mut self) {
        *self = Guide::default();
    }

    /// What its frames lead to.
    pub fn key(&self) -> Option<GuideKey> {
        self.target
    }

    /// A new plan for `target`.
    pub fn update(&mut self, plan: &Plan, target: GuideKey) {
        self.update_spaced(plan, target, None);
    }

    /// A new plan for `target`, with frames `even` apart along it (None:
    /// the ladder, closer together near the goal). Evenly spaced frames sit
    /// exactly where the plan puts them: a plan that holds its ground (an
    /// orbit's, on a fixed grid of angles) holds them still.
    pub fn update_spaced(&mut self, plan: &Plan, target: GuideKey, even: Option<f64>) {
        if self.target != Some(target) {
            self.clear();
            self.target = Some(target);
        }
        // A plan short of the goal has no ladder to measure from: keep what's set.
        if !plan.arrives || plan.points.len() < 2 {
            return;
        }
        self.spin = plan.spin;
        let to_frame = universe_engine::glam::DQuat::from_scaled_axis(-plan.spin * plan.start);
        let body: Vec<DVec3> = plan.points.iter().map(|p| to_frame * (p.position - plan.center)).collect();
        let n = body.len();
        let mut from_end = vec![0.0; n];
        for i in (0..n - 1).rev() {
            from_end[i] = from_end[i + 1] + body[i].distance(body[i + 1]);
        }
        self.left = from_end[0];
        // (A path that isn't a number — a planner gone wrong — guides nowhere.)
        if !self.left.is_finite() || even.is_some_and(|g| !(g.is_finite() && g > 0.0)) {
            self.clear();
            return;
        }
        if even.is_some() {
            self.frames.clear();
        }
        let mut i = n - 2;
        for k in 0..4096 {
            let (d, gap) = match even {
                Some(g) => (k as f64 * g, g),
                None => rung(k),
            };
            if d >= self.left {
                self.frames.retain(|&r, _| r < k);
                break;
            }
            while i > 0 && from_end[i] < d {
                i -= 1;
            }
            let span = from_end[i] - from_end[i + 1];
            let u = if span > 0.0 { (from_end[i] - d) / span } else { 0.0 };
            let at = body[i].lerp(body[i + 1], u);
            let Some(along) = (body[i + 1] - body[i]).try_normalize() else { continue };
            let (a, b) = (&plan.points[i], &plan.points[i + 1]);
            let facing = to_frame * (a.orientation.slerp(b.orientation, u) * DVec3::NEG_Z);
            // (Evenly spaced: a row of arches the ship's size, whatever the
            // range, to fly through; the ladder's narrow toward the goal.)
            let size = if even.is_some() { ARCH } else { 0.35 * gap };
            let new = GuideFrame { from_goal: d, at, along, facing, size: size.clamp(15.0, 1.0e4), action: a.action };
            // A frame stays while the new path still goes through it (near
            // its rung): rebuilt plans trace the same route, give or take.
            let near = |p: DVec3| {
                (0..n - 1)
                    .filter(|&j| from_end[j + 1] <= d + 2.0 * gap && from_end[j] >= d - 2.0 * gap)
                    .map(|j| universe_sim::physics::segment_distance(body[j], body[j + 1], p))
                    .fold(f64::INFINITY, f64::min)
            };
            match self.frames.get_mut(&k) {
                Some(old) if near(old.at) < 0.25 * gap => old.action = new.action,
                _ => {
                    self.frames.insert(k, new);
                }
            }
        }
    }

    /// The frames still ahead of the ship, the target's center at `center`, at `now`.
    fn draw(&self, frame: &mut Frame, center: DVec3, now: f64, ship: DVec3) {
        let turn = universe_engine::glam::DQuat::from_scaled_axis(self.spin * now);
        let cam = frame.camera.position;
        let mut first = true;
        let ahead = self.frames.values().rev().filter(|g| g.from_goal < self.left).filter(|g| (ship - (center + turn * g.at)).dot(turn * g.along) < 0.0);
        for g in ahead.take(MAX_FRAMES) {
            let position = center + turn * g.at;
            let along = turn * g.along;
            // Far away, keep a minimum on-screen size so the route stays readable.
            let size = g.size.max(position.distance(cam) * 0.01);
            let radial = (position - center).normalize_or(DVec3::Y);
            let level = if along.dot(radial).abs() < 0.95 { radial } else { radial.any_orthonormal_vector() };
            let right = along.cross(level).normalize();
            let up = right.cross(along);
            let (w, h) = (right * size, up * size * 0.4);
            let c = action_color(g.action);
            let corners = [position - w - h, position + w - h, position + w + h, position - w + h];
            for i in 0..4 {
                frame.line(corners[i], corners[(i + 1) % 4], c);
            }
            // Where the ship will face here: a tick from the center.
            frame.line(position, position + turn * g.facing * size * 0.5, c.scale(0.6));
            // Label the next frame ahead with its distance.
            if first {
                first = false;
                if let Some(p) = frame.project(position + up * size * 0.4) {
                    let label = crate::fmt::distance(position.distance(ship));
                    frame.text(p + universe_engine::glam::Vec2::new(4.0, -12.0), &label, c);
                }
            }
        }
    }
}

/// Landing: the free-fall prediction (with impact point), the guidance path,
/// and the descent column above the pad.
/// Waiting for a pad: the holding circle over the port (a faint ring, drawn
/// as it is), and our place on it.
fn holding_circle(frame: &mut Frame, app: &App, port: usize) {
    use universe_sim::landing::{hold_place, hold_ring};
    let Some(universe_sim::avionics::nav::PadSlot::Hold(n)) = app.v.avionics.clearance.map(|c| c.pad) else { return };
    let now = app.now();
    let pad = universe_sim::PadFrame::new(&app.view.system, port, now, &app.view.positions);
    let segments = 180;
    let ring: Vec<DVec3> = (0..=segments).map(|k| hold_ring(&pad, k as f64 / segments as f64 * std::f64::consts::TAU)).collect();
    for w in ring.windows(2) {
        frame.line(w[0], w[1], GUIDE_PATH.scale(0.35));
    }
    // Our place, and which way it goes.
    let (place, _, along) = hold_place(&pad, n, now);
    // (Only while joining: once on it, it would sit round the camera.)
    if place.distance(app.ship.position) < 2000.0 {
        return;
    }
    let size = (place.distance(frame.camera.position) * 0.01).max(60.0);
    let up = pad.up;
    let side = along.cross(up).normalize_or_zero();
    let corners = [place + side * size, place + up * size, place - side * size, place - up * size];
    for i in 0..4 {
        frame.line(corners[i], corners[(i + 1) % 4], GUIDE_PATH);
    }
    frame.line(place, place + along * size * 2.0, GUIDE_PATH.scale(0.7));
}

fn landing_guide(frame: &mut Frame, app: &App, st: &LandingStatus) {
    let pad = &st.pad;
    let entry = pad.entry();
    // Descent column from the entry point down to the pad.
    let steps = 12;
    for k in 0..steps {
        if k % 2 == 0 {
            let a = entry - pad.up * (ENTRY_ALTITUDE * k as f64 / steps as f64);
            let b = entry - pad.up * (ENTRY_ALTITUDE * (k + 1) as f64 / steps as f64);
            frame.line(a, b, GUIDE_PATH.scale(0.7));
        }
    }
    let e1 = pad.up.any_orthonormal_vector();
    let e2 = pad.up.cross(e1);
    for d in [e1, e2] {
        frame.line(entry - d * 150.0, entry + d * 150.0, GUIDE_PATH);
    }
    if !app.ship.is_flying() {
        return;
    }

    // Free-fall trajectory in the ground's frame, dashed.
    for (i, w) in st.prediction.windows(2).enumerate() {
        if i % 2 == 0 {
            frame.line(w[0], w[1], PREDICT);
        }
    }
    if let Some(hit) = st.impact {
        let size = (hit.distance(frame.camera.position) * 0.015).max(40.0);
        let (a, b) = (hit.cross(pad.up).try_normalize().unwrap_or(e1), pad.up);
        let t = (a.cross(b)).normalize();
        for d in [a + t, a - t] {
            frame.line(hit - d * size, hit + d * size, Color::hex(0xff4040));
        }
    }
    if let Some(plan) = &app.plan {
        plan_path(frame, app, plan, app.now(), app.view.ship_pos);
    }
}

/// Landing pads: a marked square on the ground, a local grid for judging
/// height, and a light beam, drawn when we're close enough to see them.
fn spaceports(frame: &mut Frame, app: &App) {
    let sys = &app.view.system;
    let cam = frame.camera.position;
    let t = app.now();
    for (i, sp) in sys.spaceports.iter().enumerate() {
        let b = &sys.bodies[sp.body];
        let rot = b.rotation(t);
        let up = rot * sp.direction;
        let pad = app.view.positions[sp.body] + up * b.rail.radius;
        let dist = pad.distance(cam);
        if dist > 400_000.0 {
            continue;
        }
        let targeted = app.v.avionics.nav_target == Some(universe_sim::NavTarget::Spaceport(i));
        let e1 = rot * sp.direction.any_orthonormal_vector();
        let e2 = up.cross(e1);
        let center = app.view.positions[sp.body];
        // A point on the true surface, `x`/`y` meters from the pad along the ground.
        let ground = |x: f64, y: f64| center + (up * b.rail.radius + e1 * x + e2 * y).normalize() * (b.rail.radius + 2.0);

        let c = if targeted { Color::hex(0x60e0ff) } else { Color::hex(0x4090b0) };
        let r = PAD_RADIUS;
        let corners = [ground(-r, -r), ground(r, -r), ground(r, r), ground(-r, r)];
        for k in 0..4 {
            frame.line(corners[k], corners[(k + 1) % 4], c.scale(0.6));
        }
        // The hangar, beside the pads, its door toward them.
        {
            let d = rot * universe_sim::world::spaceport::hangar_direction(sys, i);
            let base = center + d * (b.rail.radius + 2.0);
            let toward = (up - d * up.dot(d)).normalize_or(e1);
            let side = d.cross(toward).normalize();
            let (w, depth, h) = (40.0, 30.0, 25.0);
            let at = |x: f64, z: f64, y: f64| base + side * x + toward * z + d * y;
            let hc = c.scale(0.8);
            for &(y0, y1) in &[(0.0, 0.0), (h, h)] {
                let ring = [at(-w, -depth, y0), at(w, -depth, y0), at(w, depth, y1), at(-w, depth, y1)];
                for k in 0..4 {
                    frame.line(ring[k], ring[(k + 1) % 4], hc);
                }
            }
            for &(x, z) in &[(-w, -depth), (w, -depth), (w, depth), (-w, depth)] {
                frame.line(at(x, z, 0.0), at(x, z, h), hc);
            }
            // The door, on the side facing the pads.
            let door = [at(-22.0, depth, 0.0), at(-22.0, depth, 16.0), at(22.0, depth, 16.0), at(22.0, depth, 0.0)];
            for k in 0..3 {
                frame.line(door[k], door[k + 1], c);
            }
        }
        // The pads: ours bright, taken ones amber, free ones in the port's colour.
        let owners = if app.view.origin == app.v.ship_system { app.v.pads.get(i).copied().unwrap_or_default() } else { Default::default() };
        let ours = match app.v.avionics.clearance {
            Some(cl) if cl.target == universe_sim::NavTarget::Spaceport(i) => match cl.pad {
                universe_sim::avionics::nav::PadSlot::Pad(k) => Some(k),
                _ => None,
            },
            _ => None,
        };
        let half = 35.0;
        for (k, owner) in owners.iter().enumerate().take(PADS) {
            let d = rot * pad_direction(sys, i, k);
            let at = center + d * (b.rail.radius + 2.0);
            let (u, v) = (e1 - d * e1.dot(d), e2 - d * e2.dot(d));
            let (u, v) = (u.normalize() * half, v.normalize() * half);
            let pc = if ours == Some(k) { Color::hex(0xdcebf2) } else if owner.is_some() { Color::hex(0xffa040).scale(0.7) } else { c };
            let sq = [at - u - v, at + u - v, at + u + v, at - u + v];
            for j in 0..4 {
                frame.line(sq[j], sq[(j + 1) % 4], pc);
            }
            if ours == Some(k) {
                frame.line(at - u * 0.6, at + u * 0.6, pc);
                frame.line(at - v * 0.6, at + v * 0.6, pc);
                frame.line(at, at + d * 1500.0, pc.scale(0.8));
            }
        }
        // Beacon.
        frame.line(pad, pad + up * 3000.0, c.scale(if targeted { 0.9 } else { 0.4 }));
        // The vending machine, between the middle pads: a solid red box,
        // its lit front to the north (the way the pads' rows run).
        if dist < 3_000.0 {
            let local = universe_sim::world::spaceport::vending_direction(sys, i);
            let (north, east) = universe_sim::world::spaceport::tangent(local);
            let (d, n, e) = (rot * local, rot * north, rot * east);
            let base = center + d * b.surface_radius_at(center, center + d * b.rail.radius, t);
            // (Half a metre wide each way, 0.4 deep, 2 m tall.)
            let corner = |x: f64, h: f64, y: f64| base + e * (x * 0.5) + n * (y * 0.4) + d * h;
            let (side, edge, glow) = (Color::hex(0x7a1010), Color::hex(0xff4040), Color::hex(0xfff0f0));
            let q = |a: [f64; 3], b: [f64; 3], c: [f64; 3], dd: [f64; 3]| [corner(a[0], a[1], a[2]), corner(b[0], b[1], b[2]), corner(c[0], c[1], c[2]), corner(dd[0], dd[1], dd[2])];
            let faces = [
                q([-1.0, 0.0, 1.0], [1.0, 0.0, 1.0], [1.0, 2.0, 1.0], [-1.0, 2.0, 1.0]),
                q([-1.0, 0.0, -1.0], [1.0, 0.0, -1.0], [1.0, 2.0, -1.0], [-1.0, 2.0, -1.0]),
                q([-1.0, 0.0, -1.0], [-1.0, 0.0, 1.0], [-1.0, 2.0, 1.0], [-1.0, 2.0, -1.0]),
                q([1.0, 0.0, -1.0], [1.0, 0.0, 1.0], [1.0, 2.0, 1.0], [1.0, 2.0, -1.0]),
                q([-1.0, 2.0, -1.0], [1.0, 2.0, -1.0], [1.0, 2.0, 1.0], [-1.0, 2.0, 1.0]),
            ];
            for f in &faces {
                frame.triangle(f[0], f[1], f[2], side);
                frame.triangle(f[0], f[2], f[3], side);
                for k in 0..4 {
                    frame.line(f[k], f[(k + 1) % 4], edge);
                }
            }
            // Its lit front (just proud of the face), and the slot.
            let front = [corner(-0.8, 0.9, 1.02), corner(0.8, 0.9, 1.02), corner(0.8, 1.8, 1.02), corner(-0.8, 1.8, 1.02)];
            frame.triangle(front[0], front[1], front[2], Color::hex(0xd0d8e0));
            frame.triangle(front[0], front[2], front[3], Color::hex(0xd0d8e0));
            for k in 0..4 {
                frame.line(front[k], front[(k + 1) % 4], glow);
            }
            frame.line(corner(-0.5, 0.3, 1.02), corner(0.5, 0.3, 1.02), glow.scale(0.6));
        }

        // Ground grid, 1 km spacing, out to 10 km.
        if dist < 150_000.0 {
            let g = GRID.scale(if dist < 30_000.0 { 1.0 } else { 0.6 });
            let n = 10;
            for k in -n..=n {
                let a = k as f64 * 1000.0;
                for seg in -n..n {
                    let (s0, s1) = (seg as f64 * 1000.0, (seg + 1) as f64 * 1000.0);
                    frame.line(ground(a, s0), ground(a, s1), g);
                    frame.line(ground(s0, a), ground(s1, a), g);
                }
            }
        }
    }
}

const GRID: Color = Color::hex(0x3a4a56);
const PREDICT: Color = Color::hex(0x40c0ff);
const GUIDE_PATH: Color = Color::hex(0xff60ff);

fn ship(frame: &mut Frame, app: &App) {
    if !ship_visible(app) {
        return;
    }
    let pos = app.view.ship_pos;
    if frame.projected_radius(pos, 25.0) < 1.0 {
        frame.point(pos, SHIP_COLOR);
        return;
    }
    let t = Transform { position: pos, rotation: app.ship.orientation.as_quat(), scale: 1.0 };
    // Seen from just behind it (chase view), nothing is nearer the eye than
    // our hull: it's drawn over everything, the HUD included.
    // (Not over a full-screen panel: the nav map or the market.)
    let panel = app.nav_map.is_some() || app.galaxy_map.is_some() || app.economy_panel.is_some() || app.market.is_some() || app.passengers.is_some() || app.shipyard.is_some() || app.show_cargo || app.show_thrusters || app.picker.listing() || app.mining.on;
    // (Over a rock the camera stands off to the side: no need either.)
    let panel = panel || matches!(app.ship.state, ShipState::Anchored { .. });
    let turned = app.place(crate::Who::Me).1;
    if app.mode == Mode::Pilot && app.chase_cam && !panel {
        // (Its jets too: drawn behind the hull, they'd be hidden by it.)
        frame.in_front(|frame| {
            hull_model(frame, app.models.painted(&app.ship, 0), &t);
            hull_detail(frame, app.models.detail(&app.ship), &t);
            jets(frame, &app.ship, pos, turned, app.now(), usize::MAX);
        });
        nav_lights(frame, app.models.lights(&app.ship), pos, turned, app.now(), 7);
    } else {
        hull_model(frame, app.models.painted(&app.ship, 0), &t);
        hull_detail(frame, app.models.detail(&app.ship), &t);
        nav_lights(frame, app.models.lights(&app.ship), pos, turned, app.now(), 7);
        jets(frame, &app.ship, pos, turned, app.now(), usize::MAX);
    }
    // Landed on a body: the landing legs, down to the ground.
    if let ShipState::Landed { body, .. } = app.ship.state
        && app.view.system.bodies[body].kind != BodyKind::Station
    {
        let (b, center) = (&app.view.system.bodies[body], app.view.positions[body]);
        let o = app.place(crate::Who::Me).1;
        for leg in app.ship.spec().shape().nodes(universe_sim::world::shape::Role::Gear) {
            let top = pos + o * leg.at;
            let dir = (top - center).normalize();
            let foot = center + dir * b.surface_radius_at(center, top, app.now());
            frame.line(top, foot, SHIP_COLOR.scale(0.7));
            let side = o * DVec3::X * 1.5;
            frame.line(foot - side, foot + side, SHIP_COLOR.scale(0.7));
        }
    }
    if app.ship.hyperdrive {
        // The drive's wake (it has no jets).
        let back = turned * DVec3::Z;
        frame.line(pos + back * 16.0, pos + back * 26.0, Color::hex(0xffa040));
    }
}

/// Plume length per √newton of thrust (m): a full Drover drive nozzle
/// (1.3 MN) about 40 m, a thruster quad's (60 kN) about 8 m.
const PLUME: f64 = 0.035;
/// The glow at a nozzle's mouth, radius per √newton (m): what's seen of a
/// drive from behind.
const GLOW: f64 = 0.0016;

/// A ship's thrusters firing: from each nozzle along its exhaust, a plume
/// as long as its thrust makes it (√ of the force it's giving: the main
/// drive's long, the thrusters' short), and a glow at its mouth;
/// flickering. `seed` sets the flicker apart ship from ship.
fn jets(frame: &mut Frame, ship: &universe_sim::world::Ship, pos: DVec3, turned: DQuat, now: f64, seed: usize) {
    use universe_sim::world::ship::ThrusterRole;
    for (k, (t, &u)) in ship.spec().thrusters.iter().zip(&ship.jets).enumerate() {
        if u < 0.02 {
            continue;
        }
        let force = t.thrust * u;
        let flicker = 0.8 + 0.2 * ((now * 31.0 + k as f64 * 1.7 + seed as f64 * 0.37).sin());
        let from = pos + turned * t.at;
        let out = turned * -t.push;
        // (A drive's plume blue-white; the thrusters' cold gas pale.)
        let hot = if t.role == ThrusterRole::Rcs { Color::rgb(0.75, 0.78, 0.82) } else { Color::rgb(0.55, 0.72, 1.0) }.scale((0.4 + 0.4 * u) as f32);
        // The plume: a fan of lines from the mouth's rim to a point downstream.
        let reach = PLUME * force.sqrt() * flicker;
        let mouth = GLOW * t.thrust.sqrt();
        if t.role == ThrusterRole::Rcs {
            frame.line(from, from + out * reach, hot);
        } else {
            // A drive's plume: faint in vacuum, a short blue cone of light
            // fading and narrowing away from the bell.
            let length = reach.min(mouth * 14.0);
            let k = (u * flicker) as f32;
            for i in 1..=6 {
                let f = i as f64 / 6.0;
                let fade = (1.0 - f as f32).powi(2);
                frame.glow(from + out * (length * f), mouth * (1.1 - 0.6 * f), [0.35 * k * fade, 0.75 * k * fade, 2.2 * k * fade], 0.0);
            }
        }
        // The forge glow in the bell: blue, white-hot at its heart, as bright as the drive's set.
        if t.role != ThrusterRole::Rcs {
            let k = (u * flicker) as f32;
            frame.glow(from + out * 0.3, mouth * 1.5 * (0.6 + 0.4 * u), [0.9 * k, 1.8 * k, 5.0 * k], 2.0);
            frame.glow(from + out * 0.3, mouth * 0.6 * (0.6 + 0.4 * u), [3.0 * k, 4.0 * k, 6.0 * k], 1.2);
        } else {
            frame.point(from, Color::rgb(0.8, 0.82, 0.86));
        }
    }
}

/// Places labels in priority order, skipping any that would overlap one already placed.
struct Labels {
    placed: Vec<(Vec2, Vec2)>,
}

impl Labels {
    fn add(&mut self, frame: &mut Frame, at: DVec3, text: &str, c: Color) {
        let Some(p) = frame.project(at) else { return };
        let size = frame.size();
        if p.x < -50.0 || p.y < 0.0 || p.x > size.x || p.y > size.y {
            return;
        }
        let pos = p + Vec2::new(4.0, -10.0);
        let extent = text_size(text) + Vec2::new(4.0, 2.0);
        let overlaps = |&(q, e): &(Vec2, Vec2)| pos.x < q.x + e.x && q.x < pos.x + extent.x && pos.y < q.y + e.y && q.y < pos.y + extent.y;
        if self.placed.iter().any(overlaps) {
            return;
        }
        self.placed.push((pos, extent));
        frame.text(pos, text, c);
    }
}

fn labels(frame: &mut Frame, app: &App) {
    let sys = &app.view.system;
    let cam = frame.camera.position;
    let mut labels = Labels { placed: Vec::new() };
    if app.mode == Mode::Observer && ship_visible(app) {
        labels.add(frame, app.view.ship_pos, &app.ship.spec().name, SHIP_COLOR.scale(0.8));
    }
    if let crate::observer::Focus::Body { body, system } = app.observer.focus
        && system == app.view.origin
        && app.mode == Mode::Observer
    {
        let b = &sys.bodies[body];
        if frame.projected_radius(app.view.positions[body], b.rail.radius) < 60.0 {
            labels.add(frame, app.view.positions[body], &b.name.to_uppercase(), LABEL.scale(1.3));
        }
    }
    for (i, b) in sys.bodies.iter().enumerate() {
        let near_parent = b.rail.parent.is_some_and(|p| app.view.positions[p].distance(cam) < b.rail.orbit.as_ref().map_or(0.0, |o| o.semi_major_axis * 25.0));
        let wanted = match b.kind {
            BodyKind::Star => true,
            BodyKind::Rocky | BodyKind::GasGiant | BodyKind::IceGiant => true,
            BodyKind::Moon | BodyKind::Station | BodyKind::Gate => near_parent,
            // A field's remnant: from within a few million km.
            BodyKind::Asteroid => app.view.positions[i].distance(cam) < 5.0e9,
        };
        // Skip when the body fills the view; the label would sit on top of it.
        if wanted && frame.projected_radius(app.view.positions[i], b.rail.radius) < 60.0 {
            labels.add(frame, app.view.positions[i], &b.name.to_uppercase(), LABEL);
        }
    }

    // Neighbouring stars once we're far enough out to see them as a map.
    if cam.length() > 2.0e14 {
        let g = &app.charts.galaxy;
        for n in g.nearest(app.view.origin, 10) {
            let at = g.offset(app.view.origin, n);
            let name = star_name(g.stars[n].seed).to_uppercase();
            labels.add(frame, at, &name, LABEL.scale(0.8));
        }
    }
}
