use universe_engine::glam::{DVec3, Vec2};
use universe_engine::{text_size, Color, Frame, Light, Transform};
use universe_sim::names::star_name;
use universe_sim::units::LIGHT_YEAR;
use universe_sim::docking::{corridor_half, APPROACH_HEIGHT};
use universe_sim::landing::ENTRY_ALTITUDE;
use universe_sim::world::spaceport::{pad_direction, PADS, PAD_RADIUS};
use universe_sim::world::station::STATION_SIZE;
use universe_sim::gate::APPROACH_DISTANCE;
use universe_sim::{Action, Approach, BodyKind, DockingStatus, GateFrame, GateStatus, LandingStatus, Plan, ShipState, StationFrame};

use crate::{ship_visible, terrain_view, App, Mode};

pub const LABEL: Color = Color::hex(0x7a8a9a);
pub const SHIP_COLOR: Color = Color::hex(0x60ff90);

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
    });
    frame.reflector = reflector(frame, app);
    universe_prof::time("draw/scene/bodies", || bodies(frame, app));
    universe_prof::time("draw/scene/asteroids", || crate::rocks::draw(frame, app));
    if app.mode == Mode::Pilot
        && let Some(s) = crate::rocks::scan(app)
    {
        crate::rocks::mark(frame, &s);
    }
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
    Some(universe_engine::Reflector { center: app.view.positions[i], radius: b.rail.radius, albedo, color: [r / top, g / top, bl / top] })
}

/// Top of a world's atmosphere, for the sky's colour (m).
const ATMOSPHERE: f64 = 100_000.0;

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
    let bright = Light { position: sun, color: [1.0; 3], luminosity: sys.class.luminosity(), reference: universe_sim::units::AU }.intensity_at(cam);
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

/// Transit guidance: the ring's axis through both sides, the run-in point on
/// our side, our drift, and the flight plan tunnel.
fn transit_guide(frame: &mut Frame, app: &App, gate: usize, st: &GateStatus) {
    let f = GateFrame::new(&app.view.system, gate, app.now(), &app.view.positions);
    let axis = f.axis();
    let c = if st.in_corridor { GUIDE_OK } else { GUIDE_OFF };
    frame.line(f.center - axis * APPROACH_DISTANCE, f.center + axis * APPROACH_DISTANCE, c.scale(0.4));
    let side = if (app.view.ship_pos - f.center).dot(axis) >= 0.0 { 1.0 } else { -1.0 };
    let p = f.center + axis * side * APPROACH_DISTANCE;
    let (e1, e2) = (f.rotation * DVec3::X, f.rotation * DVec3::Z);
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
                frame.model_shaded(&app.models.gate, &t, c, c.scale(0.45));
            } else {
                frame.point(center, c);
            }
            continue;
        }
        if b.kind == BodyKind::Station {
            if px > 0.8 {
                let t = Transform { position: center, rotation, scale: STATION_SIZE };
                frame.model_shaded(&app.models.station, &t, Color::WHITE, HULL);
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
            universe_prof::time("draw/scene/bodies/globe mesh", || frame.model_colored_shaded(globe, &Transform { position: center, rotation, scale }, if app.show_grid { grid_detail(px) } else { 0.0 }, terrain_view::FILL * 2.5));
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
            if b.kind == BodyKind::Star {
                frame.model(model, &at, c.scale(0.3 * grid_detail(px * 0.5)), fill);
            } else {
                frame.model_shaded_faded(model, &at, c, fill, if app.show_grid { grid_detail(px) } else { 0.0 });
            }
        }

        // True silhouette outline, plus a halo for stars.
        let to = center - cam;
        let d = to.length();
        if d > b.rail.radius {
            let dir = to / d;
            let rings: &[(f64, f32)] = if b.kind == BodyKind::Star { &[(1.0, 1.0), (1.12, 0.45), (1.3, 0.2)] } else { &[(1.0, 1.0)] };
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
/// Hull plating of ships and stations, as lit by the star.
const HULL: Color = Color::hex(0x5a6068);

/// Other ships in the system being viewed.
fn crafts(frame: &mut Frame, app: &App) {
    let cam = frame.camera.position;
    for (i, c) in app.v.crafts.iter().enumerate() {
        let visible = c.ship.is_flying() || matches!(c.ship.state, ShipState::Landed { .. });
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
        frame.model_shaded(&app.models.ship, &t, tc, HULL);
        if c.ship.throttle > 0.0 {
            let back = turned * DVec3::Z;
            frame.line(pos + back * 16.0, pos + back * (26.0 + 40.0 * c.ship.throttle), Color::hex(0xffa040));
        }
        if pos.distance(cam) < 20_000.0
            && let Some(p) = frame.project(pos)
        {
            frame.text(p + Vec2::new(6.0, -14.0), &c.name.to_uppercase(), tc.scale(0.8));
        }
    }
}

/// The collision warning: the path ahead (drawn with its reference as it
/// moves), cyan fading to red toward an impact, and a red cross where it hits.
fn collision_path(frame: &mut Frame, app: &App) {
    let Some(p) = &app.collision else { return };
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

pub const GUIDE_OK: Color = Color::hex(0x30ff60);
pub const GUIDE_OFF: Color = Color::hex(0xffc040);

/// The approach corridor: gates along the docking axis, shaped and rolled like the slot.
fn docking_guide(frame: &mut Frame, app: &App, station: usize, status: &DockingStatus) {
    let f = StationFrame::new(&app.view.system, station, app.now(), &app.view.positions);
    let c = if status.in_corridor { GUIDE_OK } else { GUIDE_OFF };
    let (long, short) = (f.slot_long(), f.slot_short());

    frame.line(f.on_axis(STATION_SIZE), f.on_axis(APPROACH_HEIGHT), c.scale(0.35));
    for h in [650.0, 900.0, 1250.0, 1700.0, 2250.0, 3000.0, APPROACH_HEIGHT] {
        let (hl, hs) = corridor_half(h);
        let center = f.on_axis(h);
        let corners = [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)].map(|(a, b)| center + long * (a * hl) + short * (b * hs));
        // The gate the ship is heading for next is brightest.
        let next = h < status.height && h > status.height - 700.0;
        let k = if next { 1.0 } else { 0.55 };
        for i in 0..4 {
            frame.line(corners[i], corners[(i + 1) % 4], c.scale(k));
        }
    }
    // Approach point marker: a 3D cross where the final run begins.
    let p = f.on_axis(APPROACH_HEIGHT);
    let arm = 120.0;
    for d in [long, short, f.axis()] {
        frame.line(p - d * arm, p + d * arm, c);
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
    let new_place = plan.anchor(center_now, now);
    // Eased from the plan before, at the same moment of absolute time (so
    // rebuilds glide rather than jump).
    let prev = app.plan_prev.as_ref().filter(|_| app.plan_blend < 1.0);
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

    app.guide.draw(frame, center_now, now, ship);
}

/// The guide frames: gates set in space along the planned route, fixed in
/// the target's own frame (they ride and turn with the station, gate or
/// planet), at set distances back from the goal along the path: closer
/// together near it. Where the ship is doesn't move them, so holding still
/// they hold still, and flying on you go through them. A new plan moves a
/// frame only if its spot moved by more than a quarter of its spacing.
#[derive(Default)]
pub struct Guide {
    target: Option<universe_sim::NavTarget>,
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

    /// A new plan for `target`.
    pub fn update(&mut self, plan: &Plan, target: universe_sim::NavTarget) {
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
        let mut i = n - 2;
        for k in 0.. {
            let (d, gap) = rung(k);
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
            let new = GuideFrame { from_goal: d, at, along, facing, size: (0.35 * gap).clamp(15.0, 1.0e4), action: a.action };
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
            let pc = if ours == Some(k) { Color::hex(0x60ff90) } else if owner.is_some() { Color::hex(0xffa040).scale(0.7) } else { c };
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

const GRID: Color = Color::hex(0x1d5a30);
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
    // our hull: it's drawn over everything, the HUD included. Docked, the
    // eye is back past the station, so it's drawn in the scene like the rest.
    let docked = matches!(app.ship.state, ShipState::Landed { body, .. } if app.view.system.bodies[body].kind == BodyKind::Station);
    // (Not over a full-screen panel: the nav map or the market.)
    let panel = app.nav_map.is_some() || app.market.is_some();
    if app.mode == Mode::Pilot && app.chase_cam && !docked && !panel {
        frame.in_front(|frame| frame.model_shaded(&app.models.ship, &t, SHIP_COLOR, HULL));
    } else {
        frame.model_shaded(&app.models.ship, &t, SHIP_COLOR, HULL);
    }
    // Landed on a body: the landing legs, down to the ground.
    if let ShipState::Landed { body, .. } = app.ship.state
        && app.view.system.bodies[body].kind != BodyKind::Station
    {
        let (b, center) = (&app.view.system.bodies[body], app.view.positions[body]);
        let o = app.place(crate::Who::Me).1;
        for leg in [DVec3::new(-7.0, -3.2, 8.0), DVec3::new(7.0, -3.2, 8.0), DVec3::new(0.0, -2.2, -12.0)] {
            let top = pos + o * leg;
            let dir = (top - center).normalize();
            let foot = center + dir * b.surface_radius_at(center, top, app.now());
            frame.line(top, foot, SHIP_COLOR.scale(0.7));
            let side = o * DVec3::X * 1.5;
            frame.line(foot - side, foot + side, SHIP_COLOR.scale(0.7));
        }
    }
    if app.ship.hyperdrive || app.ship.throttle > 0.0 {
        // Exhaust streak.
        let back = app.place(crate::Who::Me).1 * DVec3::Z;
        let len = 10.0 + 40.0 * app.ship.throttle;
        frame.line(pos + back * 16.0, pos + back * (16.0 + len), Color::hex(0xffa040));
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
        labels.add(frame, app.view.ship_pos, "COBRA", SHIP_COLOR.scale(0.8));
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
