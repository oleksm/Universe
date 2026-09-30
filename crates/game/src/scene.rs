use universe_engine::glam::{DVec3, Vec2};
use universe_engine::{text_size, Color, Frame, Transform, WireModel};
use universe_sim::names::star_name;
use universe_sim::units::LIGHT_YEAR;
use universe_sim::docking::{corridor_half, APPROACH_HEIGHT};
use universe_sim::landing::ENTRY_ALTITUDE;
use universe_sim::world::spaceport::PAD_RADIUS;
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
    galaxy(frame, app);
    if matches!(app.u.ship.state, ShipState::Transit { .. }) && app.mode == Mode::Pilot {
        transit_tunnel(frame, app);
        return;
    }
    if app.show_orbits {
        orbits(frame, app);
    }
    // The system's star lights everything in it.
    frame.light = app.view.system.bodies.iter().position(|b| b.kind == BodyKind::Star).map(|i| app.view.positions[i]);
    bodies(frame, app);
    spaceports(frame, app);
    if app.view.origin == app.u.ship_system {
        match &app.approach {
            Some(Approach::Dock { station, status }) => docking_guide(frame, app, *station, status),
            Some(Approach::Land { status, .. }) => landing_guide(frame, app, status),
            Some(Approach::Transit { gate, status }) => transit_guide(frame, app, *gate, status),
            None => {}
        }
    }
    ship(frame, app);
    crafts(frame, app);
    weapons_fire(frame, app);
    if app.show_labels {
        labels(frame, app);
    }
}

/// Every star in the galaxy as a sky point, dimmed by distance.
fn galaxy(frame: &mut Frame, app: &App) {
    let g = &app.u.world.galaxy;
    let origin = g.stars[app.view.origin].position;
    let cam = frame.camera.position;
    for (i, s) in g.stars.iter().enumerate() {
        if i == app.view.origin {
            continue; // drawn as a body
        }
        let rel = (s.position - origin) * LIGHT_YEAR - cam;
        let d_ly = rel.length() / LIGHT_YEAR;
        let flux = s.class.luminosity() / (d_ly * d_ly).max(1e-9);
        let brightness = ((flux.log10() + 8.0) / 6.0).clamp(0.2, 1.0) as f32;
        frame.sky_point(rel.normalize().as_vec3(), color(s.class.color()).scale(brightness));
    }
    // The gate network, once we're zoomed out far enough to see it as a map.
    if app.mode == Mode::Observer && cam.length() > 1.0e15 {
        let amber = Color::hex(0xffc040);
        let mut nodes = Vec::new();
        for &(a, b) in &app.u.world.gate_links {
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
    let shift = (app.u.world.time * 900.0) % spacing;
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
    let f = GateFrame::new(&app.view.system, gate, app.u.world.time, &app.view.positions);
    let axis = f.axis();
    let c = if st.in_corridor { GUIDE_OK } else { GUIDE_OFF };
    frame.line(f.center - axis * APPROACH_DISTANCE, f.center + axis * APPROACH_DISTANCE, c.scale(0.4));
    let side = if (app.view.ship_pos - f.center).dot(axis) >= 0.0 { 1.0 } else { -1.0 };
    let p = f.center + axis * side * APPROACH_DISTANCE;
    let (e1, e2) = (f.rotation * DVec3::X, f.rotation * DVec3::Z);
    for d in [e1, e2, axis] {
        frame.line(p - d * 120.0, p + d * 120.0, c);
    }
    if app.u.ship.is_flying() {
        drift_line(frame, app.view.ship_pos, st.relative_velocity);
        if let Some(plan) = &app.plan {
            plan_path(frame, app, plan, app.u.world.time, app.view.ship_pos);
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

fn bodies(frame: &mut Frame, app: &App) {
    let sys = &app.view.system;
    let t = app.u.world.time;
    let cam = frame.camera.position;
    for (i, b) in sys.bodies.iter().enumerate() {
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

        if let Some(globe) = app.globes.get(&(app.view.origin, i)) {
            // Terrain world: colored globe; near the surface, a local grid on
            // the ground (the globe drops a hair so the grid sits on top).
            let near = cam.distance(center) - b.rail.radius < terrain_view::near_altitude(b);
            let scale = if near { b.rail.radius * 0.998 } else { b.rail.radius };
            frame.model_colored_shaded(globe, &Transform { position: center, rotation, scale }, 1.0, terrain_view::FILL * 2.5);
            if near {
                terrain_view::surface_grid(frame, b, center, t);
            }
            if frame.projected_radius(center, b.rail.radius) > 40.0 {
                terrain_view::crater_rims(frame, b, center, t);
            }
        } else {
            let (model, fill): (&WireModel, Color) = match b.kind {
                BodyKind::Star => (&app.models.star, c.scale(0.3)),
                BodyKind::Rocky => (&app.models.rocky, c.scale(0.4)),
                BodyKind::GasGiant | BodyKind::IceGiant => (&app.models.giant, c.scale(0.45)),
                _ => (&app.models.moon, c.scale(0.35)),
            };
            let at = Transform { position: center, rotation, scale: b.rail.radius };
            // The star shines; everything else is lit by it.
            if b.kind == BodyKind::Star {
                frame.model(model, &at, c, fill);
            } else {
                frame.model_shaded(model, &at, c, fill);
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

pub const TRAFFIC: Color = Color::hex(0x50d8ff);
/// Hull plating of ships and stations, as lit by the star.
const HULL: Color = Color::hex(0x5a6068);

/// Other ships in the system being viewed.
fn crafts(frame: &mut Frame, app: &App) {
    let cam = frame.camera.position;
    for c in &app.u.crafts {
        let visible = c.ship.is_flying() || matches!(c.ship.state, ShipState::Landed { .. });
        if c.system != app.view.origin || !visible {
            continue;
        }
        let pos = c.ship.position;
        if frame.projected_radius(pos, 25.0) < 1.0 {
            frame.point(pos, TRAFFIC.scale(0.8));
            continue;
        }
        let t = Transform { position: pos, rotation: c.ship.orientation.as_quat(), scale: 1.0 };
        frame.model_shaded(&app.models.ship, &t, TRAFFIC, HULL);
        if c.ship.throttle > 0.0 {
            let back = c.ship.orientation * DVec3::Z;
            frame.line(pos + back * 16.0, pos + back * (26.0 + 40.0 * c.ship.throttle), Color::hex(0xffa040));
        }
        if pos.distance(cam) < 20_000.0
            && let Some(p) = frame.project(pos)
        {
            frame.text(p + Vec2::new(6.0, -14.0), &c.name.to_uppercase(), TRAFFIC.scale(0.8));
        }
    }
}

/// Slugs in flight as short tracers (streaked along their motion relative to
/// us), and this frame's laser beams.
fn weapons_fire(frame: &mut Frame, app: &App) {
    let own = app.u.ship.velocity;
    for slug in &app.u.world.slugs {
        if slug.system != app.view.origin {
            continue;
        }
        let p = slug.projectile.position;
        let rel = slug.projectile.velocity - own;
        let streak = rel.normalize_or_zero() * (rel.length() * 0.02).clamp(4.0, 60.0);
        frame.line(p - streak, p, Color::hex(0xffd060));
        frame.point(p, Color::hex(0xffe080));
    }
    for beam in &app.u.world.beams {
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
    let f = StationFrame::new(&app.view.system, station, app.u.world.time, &app.view.positions);
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

    if app.u.ship.is_flying() {
        drift_line(frame, app.view.ship_pos, status.relative_velocity);
        if let Some(plan) = &app.plan {
            plan_path(frame, app, plan, app.u.world.time, app.view.ship_pos);
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
    let target = app.u.avionics.clearance?.target;
    let i = match target {
        universe_sim::NavTarget::Station(s) | universe_sim::NavTarget::Gate(s) => s,
        universe_sim::NavTarget::Spaceport(p) => app.view.system.spaceports.get(p)?.body,
    };
    app.view.positions.get(i).copied()
}

/// Distance still to fly along the planned path.
pub fn path_length(plan: &Plan) -> f64 {
    plan.points.windows(2).map(|w| w[0].position.distance(w[1].position)).sum()
}

fn plan_path(frame: &mut Frame, app: &App, plan: &Plan, now: f64, ship: DVec3) {
    // The plan may be a few frames old: carry it along with its reference.
    let Some(center_now) = plan_reference(app) else { return };
    let place = plan.anchor(center_now, now);
    let turn = plan.turn(now);
    let pts = &plan.points;
    if pts.len() < 2 {
        return;
    }
    // Center line, colored by what the ship is doing; dashed while coasting.
    for (i, w) in pts.windows(2).enumerate() {
        let c = action_color(w[0].action);
        if !(w[0].action == Action::Coast && i % 2 == 1) {
            frame.line(place(w[0].position), place(w[1].position), c);
        }
    }

    // Frames are gates placed along the route: at fixed moments of absolute
    // time, evenly spaced (`app.frame_step`, about 12 over the route), so they
    // stay put as the ship flies through them and nothing pops in between.
    // Each has a real size in metres (a fifth of the gap to the next), so it
    // grows as you approach, like an object you're flying toward.
    let duration = pts.last().unwrap().time;
    let cam = frame.camera.position;
    let made = plan.start;
    let step = app.frame_step;
    let mut k = (now / step).floor() + 1.0;
    let mut j = 0;
    let mut first = true;
    while k * step - made <= duration {
        let at = k * step - made;
        k += 1.0;
        while j + 1 < pts.len() && pts[j + 1].time < at {
            j += 1;
        }
        let (a, b) = (pts[j], pts[(j + 1).min(pts.len() - 1)]);
        let span = b.time - a.time;
        let u = if span > 0.0 { ((at - a.time) / span).clamp(0.0, 1.0) } else { 0.0 };
        let position = place(a.position.lerp(b.position, u));
        let orientation = turn * a.orientation.slerp(b.orientation, u);
        // Local speed along the plan sets the gap to the next frame.
        let speed = if span > 0.0 { a.position.distance(b.position) / span } else { 0.0 };
        let world = (speed * step * 0.2).clamp(40.0, 1.0e4);
        // Far away, keep a minimum on-screen size so the route stays readable.
        let size = world.max(position.distance(cam) * 0.01);
        let right = orientation * DVec3::X;
        let up = orientation * DVec3::Y;
        let fwd = orientation * DVec3::NEG_Z;
        let (w, h) = (right * size, up * size * 0.4);
        let c = action_color(a.action);
        let corners = [position - w - h, position + w - h, position + w + h, position - w + h];
        for i in 0..4 {
            frame.line(corners[i], corners[(i + 1) % 4], c);
        }
        // Nose tick: which way the ship faces here.
        frame.line(position, position + fwd * size * 0.8, c);
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

/// Landing: the free-fall prediction (with impact point), the guidance path,
/// and the descent column above the pad.
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
    if !app.u.ship.is_flying() {
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
        plan_path(frame, app, plan, app.u.world.time, app.view.ship_pos);
    }
}

/// Landing pads: a marked square on the ground, a local grid for judging
/// height, and a light beam, drawn when we're close enough to see them.
fn spaceports(frame: &mut Frame, app: &App) {
    let sys = &app.view.system;
    let cam = frame.camera.position;
    let t = app.u.world.time;
    for (i, sp) in sys.spaceports.iter().enumerate() {
        let b = &sys.bodies[sp.body];
        let rot = b.rotation(t);
        let up = rot * sp.direction;
        let pad = app.view.positions[sp.body] + up * b.rail.radius;
        let dist = pad.distance(cam);
        if dist > 400_000.0 {
            continue;
        }
        let targeted = app.u.avionics.nav_target == Some(universe_sim::NavTarget::Spaceport(i));
        let e1 = rot * sp.direction.any_orthonormal_vector();
        let e2 = up.cross(e1);
        let center = app.view.positions[sp.body];
        // A point on the true surface, `x`/`y` meters from the pad along the ground.
        let ground = |x: f64, y: f64| center + (up * b.rail.radius + e1 * x + e2 * y).normalize() * (b.rail.radius + 2.0);

        let c = if targeted { Color::hex(0x60e0ff) } else { Color::hex(0x4090b0) };
        let r = PAD_RADIUS;
        let corners = [ground(-r, -r), ground(r, -r), ground(r, r), ground(-r, r)];
        for k in 0..4 {
            frame.line(corners[k], corners[(k + 1) % 4], c);
        }
        // An "H".
        frame.line(ground(-r * 0.4, -r * 0.5), ground(-r * 0.4, r * 0.5), c);
        frame.line(ground(r * 0.4, -r * 0.5), ground(r * 0.4, r * 0.5), c);
        frame.line(ground(-r * 0.4, 0.0), ground(r * 0.4, 0.0), c);
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
    let t = Transform { position: pos, rotation: app.u.ship.orientation.as_quat(), scale: 1.0 };
    frame.model_shaded(&app.models.ship, &t, SHIP_COLOR, HULL);
    if app.u.ship.hyperdrive || app.u.ship.throttle > 0.0 {
        // Exhaust streak.
        let back = app.u.ship.orientation * DVec3::Z;
        let len = 10.0 + 40.0 * app.u.ship.throttle;
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
        };
        // Skip when the body fills the view; the label would sit on top of it.
        if wanted && frame.projected_radius(app.view.positions[i], b.rail.radius) < 60.0 {
            labels.add(frame, app.view.positions[i], &b.name.to_uppercase(), LABEL);
        }
    }

    // Neighbouring stars once we're far enough out to see them as a map.
    if cam.length() > 2.0e14 {
        let g = &app.u.world.galaxy;
        for n in g.nearest(app.view.origin, 10) {
            let at = g.offset(app.view.origin, n);
            let name = star_name(g.stars[n].seed).to_uppercase();
            labels.add(frame, at, &name, LABEL.scale(0.8));
        }
    }
}
