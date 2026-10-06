use universe_engine::glam::{DVec3, Vec2, Vec3Swizzles};
use universe_engine::{text_size, Color, Context, Frame, GLYPH};
use universe_sim::world::weapons::{gun_on, within_gimbal, GIMBAL_LIMIT};
use universe_sim::world::station::DECK_SPEED;
use universe_sim::{Action, Approach, BodyKind, DockingStatus, Guidance, LandingStatus, ShipState};

use crate::observer::Focus;
use crate::scene::color;
use crate::{fmt, App, Mode};

const HUD: Color = Color::hex(0xdcebf2);
const DIM: Color = Color::hex(0x7d93a0);
const AMBER: Color = Color::hex(0xffc040);
/// The key's letter, lit in an action's name.
const HOT: Color = Color::hex(0xffffa0);
const RED: Color = Color::hex(0xff4040);
/// Colors shared with the 3D guidance: predicted path (cyan) and guidance path (magenta).
const PREDICT: Color = Color::hex(0x40c0ff);
const GUIDE_PATH: Color = Color::hex(0xff60ff);
const PANEL: Color = Color([0.012, 0.018, 0.026, 0.85]);
/// A lighter backing for text over the world: readable on a bright sky, the view still through it.
const SOFT_PANEL: Color = Color([0.01, 0.015, 0.022, 0.55]);
const LINE: f32 = GLYPH + 2.0;

pub fn draw(frame: &mut Frame, app: &App, ctx: &Context) {
    // The map covers the screen. (HUD fills are drawn before HUD lines, so
    // anything else drawn now would show through it.)
    if let Some(panel) = &app.economy_panel {
        crate::economy::draw(frame, app, panel);
        return;
    }
    if app.news_panel {
        crate::newspanel::draw(frame, app);
        return;
    }
    if app.standards.is_some() {
        crate::standards::draw(frame, app);
        return;
    }
    if let Some(map) = &app.galaxy_map {
        crate::galaxymap::draw(frame, app, map);
        return;
    }
    if let Some(map) = &app.nav_map {
        crate::navmap::draw(frame, app, map);
        return;
    }
    if let Some(m) = &app.market {
        crate::market::draw(frame, app, m);
        return;
    }
    if let Some(pick) = app.passengers.filter(|_| app.mode == Mode::Pilot) {
        crate::passengers::draw(frame, app, pick);
        return;
    }
    if let Some(y) = &app.shipyard {
        universe_prof::time("draw/studio", || crate::shipyard::draw(frame, app, y));
        return;
    }
    // The thrusters panel: a screen of its own (in the pilot's seat).
    if app.show_thrusters && app.mode == Mode::Pilot && app.v.crew.seated() {
        crate::thrusterpanel::draw(frame, app);
        return;
    }
    // (Not in a gate's transit: the push draws its own suns.)
    if !matches!(app.ship.state, ShipState::Transit { .. }) {
        universe_prof::time("draw/hud/sun glare", || sun_glare(frame, app));
    }
    if app.mode == Mode::Pilot {
        crate::manual::draw(frame, app);
    }
    // What's going on (route, approach, the lock, where we are), in a column
    // at the left; warnings in a stack at the top centre.
    let mut lines: Vec<(String, Color)> = Vec::new();
    let mut alerts: Vec<(String, Color)> = Vec::new();
    match app.mode {
        Mode::Observer => observer_info(app, &mut lines),
        Mode::Pilot if !app.v.crew.seated() => crate::onfoot::hud(frame, app, &mut lines, app.reach),
        // (In a gate's transit: between the stars, none of the system's markers.)
        Mode::Pilot if matches!(app.ship.state, ShipState::Transit { .. }) => pilot_info(app, &mut lines, &mut alerts),
        Mode::Pilot => {
            pilot_info(app, &mut lines, &mut alerts);
            crate::followguide::lines(app, &mut lines);
            approach_info(app, &mut lines);
            universe_prof::time("draw/hud/pilot overlay", || pilot_overlay(frame, app));
            target_marker(frame, app);
            universe_prof::time("draw/hud/contact marker", || contact_marker(frame, app));
            missile_markers(frame, app);
            universe_prof::time("draw/hud/turret markers", || turret_markers(frame, app));
            impact_label(frame, app);
            phase_banner(frame, app);
            crate::followguide::banner(frame, app);
            universe_prof::time("draw/hud/scanner", || scanner(frame, app));
            crate::mining::draw_hud(frame, app);
            cargo_panel(frame, app);
            crate::lock::draw(frame, app);
            crate::orbitpick::draw(frame, app);
        }
    }
    // The top line is for status: the hypernet badge at its left, the
    // guidance banner when one's up; the buttons always start under it.
    let top = banner_y(app) + GLYPH + 10.0;
    let size = frame.size();
    let mut y = top;
    if app.mode == Mode::Pilot && app.v.crew.seated() {
        y += mode_bar(frame, app, Vec2::new(4.0, top)) + 2.0;
    }
    match app.mode {
        Mode::Pilot if app.v.crew.seated() => action_grid(frame, app),
        Mode::Pilot => draw_grid(frame, "ON FOOT", &on_foot_cells()),
        Mode::Observer => draw_grid(frame, "OBSERVER", &observer_cells()),
    }
    // The status strip: mode, clock, time rate, system; credits at the right.
    y += status_strip(frame, app, Vec2::new(4.0, y)) + 6.0;
    let under_strip = y;
    let credits = format!("{:.0} CR", app.v.credits);
    frame.text_boxed(Vec2::new(size.x - text_size(&credits).x - 6.0, top + 2.0), &credits, HUD, SOFT_PANEL);
    if app.mode == Mode::Pilot {
        net_badge(frame, app, Vec2::new(6.0, banner_y(app) + 2.0));
    }
    if app.mode == Mode::Pilot && app.v.crew.seated() && !matches!(app.ship.state, ShipState::Transit { .. }) {
        y += instruments(frame, app, Vec2::new(4.0, y)) + 6.0;
    }
    text_column(frame, Vec2::new(4.0, y), &lines);

    // Debug (F3): performance, traffic, trades; then the profiler as well.
    let mut right = under_strip;
    if app.debug > 0 {
        right = perf(frame, app, ctx, right) + 6.0;
        right = trade_feed(frame, app, right) + 6.0;
    }
    if universe_prof::enabled() {
        right = profile_panel(frame, right) + 6.0;
    }
    universe_prof::time("draw/hud/news feed", || news_feed(frame, app, right));
    frame.text(Vec2::new(size.x - 7.0 * GLYPH - 4.0, size.y - GLYPH - 4.0), "F1 HELP", DIM);

    // Warnings: a stack at the top centre, under the guidance banner and the status strip.
    let mut y = (top + 40.0).max(under_strip);
    for (text, c) in &alerts {
        let w = text_size(text).x;
        frame.text_boxed(Vec2::new(((size.x - w) / 2.0).floor(), y), text, *c, PANEL);
        y += LINE + 4.0;
    }
    // Messages under them, a third of the way down.
    let mut y = (y + 8.0).max(size.y * 0.3);
    for m in &app.messages {
        let c = if m.ttl < 1.0 { AMBER.scale(m.ttl) } else { AMBER };
        for line in m.text.lines() {
            let w = text_size(line).x;
            frame.text(Vec2::new((size.x - w) / 2.0, y), line, c);
            y += LINE;
        }
    }

    if let ShipState::Transit { to, remaining, .. } = &app.ship.state {
        let name = universe_sim::names::star_name(app.charts.galaxy.stars[*to].seed).to_uppercase();
        let text = format!("GATE TRANSIT TO {name} - ARRIVING IN {}", fmt::lag(*remaining));
        frame.text_boxed(((size - text_size(&text)) / 2.0).floor(), &text, AMBER, PANEL);
    }
    if app.show_help {
        help(frame);
    }
}

/// The status strip (one line, under the mode bar): mode, clock, time
/// rate, the system. Its height.
fn status_strip(frame: &mut Frame, app: &App, at: Vec2) -> f32 {
    let ship = &app.ship;
    let mode = match app.mode {
        Mode::Observer => "OBSERVER",
        Mode::Pilot if ship.armed => "COMBAT",
        Mode::Pilot if app.mining.on => "MINING",
        Mode::Pilot => "PILOT",
    };
    let top = if app.mode == Mode::Pilot && ship.armed { RED } else { HUD };
    let warp = if app.paused {
        "PAUSED".to_string()
    } else if app.last_step.warp_limited {
        format!("TIME {} LIMITED", fmt::warp(app.warp()))
    } else {
        format!("TIME {}", fmt::warp(app.warp()))
    };
    let sys = &app.view.system;
    let home = if app.view.origin == app.charts.home_system { "  HOME" } else { "" };
    let place = format!("   {} ({}){home}", sys.name.to_uppercase(), sys.class.letter());
    let first = format!("{mode}  {}  {warp}", fmt::clock(app.v.time));
    let size = text_size(&first) + Vec2::new(text_size(&place).x, 0.0);
    frame.hud_rect(at, size + Vec2::new(8.0, 6.0), SOFT_PANEL);
    let p = frame.text(at + Vec2::new(4.0, 3.0), &first, top);
    frame.text(p, &place, DIM);
    size.y + 6.0
}

/// A column of lines on a soft dark backing.
fn text_column(frame: &mut Frame, at: Vec2, lines: &[(String, Color)]) {
    if lines.is_empty() {
        return;
    }
    let w = lines.iter().map(|(t, _)| text_size(t).x).fold(0.0, f32::max);
    frame.hud_rect(at, Vec2::new(w + 8.0, lines.len() as f32 * LINE + 6.0), SOFT_PANEL);
    for (i, (text, c)) in lines.iter().enumerate() {
        frame.text(at + Vec2::new(4.0, 3.0 + i as f32 * LINE), text, *c);
    }
}

/// Where we are and how we're moving against what's near: (its name,
/// altitude, whether it's ground speed, speed, vertical speed).
fn readout(app: &App) -> Option<(String, f64, bool, f64, f64)> {
    let r = app.view.reference?;
    let b = &app.view.system.bodies[r];
    let offset = app.view.ship_pos - app.view.positions[r];
    // Height above the ground actually under us (terrain, or sea level).
    let altitude = offset.length() - b.surface_radius_at(app.view.positions[r], app.view.ship_pos, app.v.time);
    let mut rel_vel = app.ship.velocity - app.view.system.velocity(r, app.v.time);
    // Close to the ground, speed relative to the rotating surface is what matters.
    let surface = altitude < 0.05 * b.rail.radius;
    if surface {
        rel_vel -= b.angular_velocity().cross(offset);
    }
    Some((b.name.to_uppercase(), altitude, surface, rel_vel.length(), rel_vel.dot(offset.normalize())))
}

/// The flight instruments (left): where, how high, how fast; thrust, fuel,
/// hull as bars; mass, load and drive. Its height.
/// The air's temperature where the ship is, if it's in a world's air (K).
fn air_outside(app: &App) -> Option<f64> {
    let sys = &app.view.system;
    let p = app.view.ship_pos;
    let (i, b) = sys.bodies.iter().enumerate().filter(|(_, b)| b.rail.atmosphere.is_some()).min_by(|a, b| app.view.positions[a.0].distance(p).total_cmp(&app.view.positions[b.0].distance(p)))?;
    let altitude = app.view.positions[i].distance(p) - b.rail.radius;
    (altitude < b.rail.atmosphere?.top).then(|| universe_sim::world::climate::air_temperature(sys, i, &app.view.positions, app.now(), p))
}

/// One row of the instruments, drawn at a place.
type Row = Box<dyn Fn(&mut Frame, Vec2)>;

fn instruments(frame: &mut Frame, app: &App, at: Vec2) -> f32 {
    let ship = &app.ship;
    const W: f32 = 214.0;
    const LABEL: f32 = 54.0;
    const ROW: f32 = LINE + 1.0;
    let mut rows: Vec<Row> = Vec::new();
    let text_row = |label: &'static str, value: String, c: Color| -> Row {
        Box::new(move |frame: &mut Frame, p: Vec2| {
            frame.text(p, label, DIM);
            frame.text(p + Vec2::new(LABEL, 0.0), &value, c);
        })
    };
    let bar_row = |label: &'static str, x: f64, value: String, c: Color| -> Row {
        Box::new(move |frame: &mut Frame, p: Vec2| {
            frame.text(p, label, DIM);
            let (bp, bs) = (p + Vec2::new(LABEL, 1.0), Vec2::new(80.0, GLYPH - 2.0));
            frame.hud_rect(bp, bs, DIM.scale(0.25));
            frame.hud_rect(bp, Vec2::new(bs.x * x.clamp(0.0, 1.0) as f32, bs.y), c);
            frame.text(p + Vec2::new(LABEL + 88.0, 0.0), &value, c);
        })
    };
    if let Some((name, alt, surface, speed, vertical)) = readout(app) {
        rows.push(text_row("NEAR", name, HUD));
        rows.push(text_row("ALT", fmt::distance(alt), HUD));
        rows.push(text_row(if surface { "GND" } else { "SPD" }, fmt::speed(speed), HUD));
        rows.push(text_row("VSPD", fmt::speed(vertical), HUD));
    }
    let thr = if ship.hyperdrive { AMBER } else { HUD };
    rows.push(bar_row("THR", ship.throttle, format!("{:.0}%", ship.throttle * 100.0), thr));
    let fuel = ship.fuel / ship.spec().fuel_capacity;
    let fc = if fuel < 0.1 { RED } else if fuel < 0.25 { AMBER } else { HUD };
    rows.push(bar_row("FUEL", fuel, format!("{:.1} T", ship.fuel / 1000.0), fc));
    let cap = ship.spec().capacitor_capacity;
    if cap > 0.0 {
        let k = ship.energy / cap;
        rows.push(bar_row("CAP", k, format!("{:.1} GJ", ship.energy / 1e9), if k < 0.15 { AMBER } else { HUD }));
    }
    let hurt = app.hit_age < 0.25 || ship.hull < 0.3;
    rows.push(bar_row("HULL", ship.hull, format!("{:.0}%", ship.hull * 100.0), if hurt { RED } else { HUD }));
    // Its skin's temperature, and the air's outside when in it.
    let hot = ship.skin_temp > universe_sim::world::heat::SKIN_LIMIT * 0.4;
    let outside = air_outside(app).map(|t| format!("  AIR {}", crate::fmt::temperature(t))).unwrap_or_default();
    rows.push(text_row("TEMP", format!("{}{outside}", crate::fmt::temperature(ship.skin_temp)), if hot { AMBER } else { HUD }));
    if ship.armed {
        let heat = if ship.laser_overheated { RED } else { AMBER };
        rows.push(bar_row("LASER", ship.laser_heat, if ship.laser_overheated { "HOT".into() } else { format!("GUN {}", ship.ammo) }, heat));
    }
    // Our standing with this system's authority (a settled system's).
    if app.view.system.station().is_some() || !app.view.system.spaceports.is_empty() {
        let s = app.v.standing;
        let c = if s <= -10.0 { RED } else if s >= 10.0 { HUD } else { DIM };
        rows.push(text_row("STAND", format!("{s:+.0} {}", universe_sim::standing::label(s)), c));
    }
    rows.push(text_row("MASS", format!("{:.1} T  LOAD {:.1} T", ship.mass() / 1000.0, ship.cargo / 1000.0), DIM));
    rows.push(text_row("DRIVE", format!("{:.1} M/S2", ship.main_accel()), DIM));
    if !app.contacts.is_empty() {
        rows.push(text_row("RADAR", format!("{} IN {}", app.contacts.len(), fmt::distance(universe_sim::world::radar::range(app.ship.spec()))), DIM));
    }
    let h = rows.len() as f32 * ROW + 8.0;
    frame.hud_rect(at, Vec2::new(W, h), SOFT_PANEL);
    frame.hud_line(at, at + Vec2::new(W, 0.0), DIM.scale(0.6));
    for (i, row) in rows.iter().enumerate() {
        row(frame, at + Vec2::new(6.0, 4.0 + i as f32 * ROW));
    }
    h
}

/// The hypernet badge, at `at` (the top line's left): signal bars by our lag
/// from the backbone (green under a second, yellow under a minute, red past
/// it; grey and crossed off the net), and the lag itself.
fn net_badge(frame: &mut Frame, app: &App, at: Vec2) {
    const GREEN: Color = Color::hex(0x60ff90);
    const YELLOW: Color = Color::hex(0xffd040);
    let (bars, c, text) = match &app.net {
        Some((lag, _)) if *lag < 1.0 => (4, GREEN, fmt::lag(*lag)),
        Some((lag, _)) if *lag < 60.0 => (3, YELLOW, fmt::lag(*lag)),
        Some((lag, _)) => (1, RED, fmt::lag(*lag)),
        None => (0, DIM, "OFF".to_string()),
    };
    let label = text;
    let w = 22.0 + text_size(&label).x;
    frame.hud_rect(at - 2.0, Vec2::new(w, GLYPH) + 4.0, SOFT_PANEL);
    // Four bars, rising; lit as many as the signal's worth.
    for k in 0..4 {
        let h = 3.0 + k as f32 * 2.5;
        let p = at + Vec2::new(k as f32 * 4.0, GLYPH - h);
        frame.hud_rect(p, Vec2::new(3.0, h), if k < bars { c } else { DIM.scale(0.35) });
    }
    if bars == 0 {
        frame.hud_line(at + Vec2::new(0.0, 1.0), at + Vec2::new(14.0, GLYPH - 1.0), DIM);
        frame.hud_line(at + Vec2::new(14.0, 1.0), at + Vec2::new(0.0, GLYPH - 1.0), DIM);
    }
    frame.text(at + Vec2::new(20.0, 0.0), &label, c);
}

fn observer_info(app: &App, lines: &mut Vec<(String, Color)>) {
    let cam = app.camera.position;
    match app.observer.focus {
        Focus::Ship => {
            lines.push((format!("FOCUS {}", app.ship.spec().name), HUD));
            ship_readout(app, lines);
        }
        Focus::Craft(i) => {
            let Some(c) = app.v.crafts.get(i) else { return };
            let stage = c.stage;
            lines.push((format!("FOCUS {}  ({}/{})  T NEXT", c.name.to_uppercase(), i + 1, app.v.crafts.len()), HUD));
            lines.push((format!("ROUTE STOP {}/{}  {stage}", c.route_next + 1, c.route_stops), DIM));
            lines.push((format!("SPEED {}", fmt::speed(c.ship.velocity.length())), DIM));
        }
        Focus::Body { body, .. } => {
            let b = &app.view.system.bodies[body];
            lines.push((format!("FOCUS {}  {}", b.name.to_uppercase(), b.kind.label().to_uppercase()), HUD));
            let dist = app.view.positions[body].distance(cam) - b.rail.radius;
            lines.push((format!("RADIUS {}  DIST {}", fmt::distance(b.rail.radius), fmt::distance(dist)), DIM));
            if let Some(o) = &b.rail.orbit {
                lines.push((format!("ORBIT {}  PERIOD {}", fmt::distance(o.semi_major_axis), fmt::duration(o.period())), DIM));
            }
            if body == 0 && app.view.origin != app.v.ship_system {
                let ly = app.charts.distance_ly(app.v.ship_system, app.view.origin);
                lines.push((format!("{ly:.1} LY FROM SHIP"), DIM));
            }
        }
    }
}

fn ship_readout(app: &App, lines: &mut Vec<(String, Color)>) {
    if let Some((name, altitude, surface, speed, vertical)) = readout(app) {
        lines.push((format!("NEAR {name}  ALT {}", fmt::distance(altitude)), DIM));
        let label = if surface { "SRF SPD" } else { "SPD" };
        lines.push((format!("{label} {}  VSPD {}", fmt::speed(speed), fmt::speed(vertical)), DIM));
    }
}

fn pilot_info(app: &App, lines: &mut Vec<(String, Color)>, alerts: &mut Vec<(String, Color)>) {
    route_info(app, lines);
    radar_info(app, lines);
    collision_info(app, alerts);
    prospect_info(app, lines);
    let ship = &app.ship;
    // Off balance (its load, or what's fitted, off the thrusters' centre):
    // what its lift and drive still give without turning it.
    {
        let (a, s) = (ship.authority(), ship.spec());
        let (lift, main) = (a.lift / s.lift_thrust.max(1.0), a.main / s.main_thrust.max(1.0));
        // (A stock ship keeps 92% and more, empty to full: under 90% it's off.)
        if lift < 0.9 || main < 0.9 {
            let c = if lift < 0.75 || main < 0.75 { RED } else { AMBER };
            alerts.push((format!("OFF BALANCE - LIFT {:.0}%  DRIVE {:.0}%", lift * 100.0, main * 100.0), c));
        }
    }
    // Fuel, once it's running down.
    let fuel = ship.fuel / ship.spec().fuel_capacity;
    if fuel < 0.25 {
        let s = ship.spec();
        let hours = ship.fuel / (s.main_thrust / s.exhaust_of(universe_sim::world::ship::ThrusterRole::Main)) / 3600.0;
        alerts.push((format!("FUEL {:.0}%  {:.1} H OF FULL BURN LEFT - REFUEL AT A MARKET", fuel * 100.0, hours), if fuel < 0.1 { RED } else { AMBER }));
    }
    let now = app.v.time;
    if let Some(until) = app.v.aggressed_until {
        let left = (until - now) / app.warp().max(1.0);
        alerts.push((format!("AGGRESSED {} - FAIR GAME TO ANYONE", fmt::countdown(left)), RED));
    }
    // An enemy of this system: its guns fire, its docks refuse.
    if app.v.standing <= universe_sim::standing::HOSTILE {
        alerts.push((format!("ENEMY OF {} - ITS GUNS FIRE, ITS DOCKS REFUSE", app.view.system.name.to_uppercase()), RED));
    }
    // Missiles after us: how many, and the nearest's time to reach us.
    let inbound: Vec<(f64, f64)> = app
        .v
        .missiles
        .iter()
        .filter(|m| m.4 && m.0 == app.v.ship_system)
        .map(|m| {
            let r = app.view.ship_pos - m.1;
            (r.length(), (m.2 - ship.velocity).dot(r.normalize_or_zero()))
        })
        .collect();
    if let Some(&(d, closing)) = inbound.iter().min_by(|a, b| a.0.total_cmp(&b.0)) {
        let eta = if closing > 1.0 { format!(", IMPACT IN {}", fmt::countdown(d / closing)) } else { String::new() };
        let blink = (app.now() * 4.0).fract() < 0.6;
        alerts.push((format!("MISSILE LOCK - {} INBOUND, NEAREST {}{eta}", inbound.len(), fmt::distance(d)), if blink { RED } else { RED.scale(0.5) }));
    }
    if ship.armed && !ship.weapons_hot() {
        alerts.push((format!("WEAPONS PRIMING  {:.1} S", ship.arming), AMBER));
    }
    // The klaxon's words (see `sound::alarms`), blinking. (Low fuel has its own line above.)
    let blink = (app.now() * 2.0).fract() < 0.6;
    if ship.hull < crate::sound::HULL_CRITICAL {
        alerts.push(("HULL CRITICAL".into(), if blink { RED } else { RED.scale(0.5) }));
    }
    // The skin, once air has heated it past what sunlight does.
    let skin = ship.skin_temp;
    if skin > universe_sim::world::heat::SKIN_LIMIT * 0.4 {
        use universe_sim::world::heat::SKIN_LIMIT;
        let (note, c) = if skin >= SKIN_LIMIT - 1.0 {
            ("  HULL BURNING - SLOW DOWN OR CLIMB", RED)
        } else if skin > SKIN_LIMIT * 0.8 {
            ("  NEAR THE LIMIT", RED)
        } else {
            ("", AMBER)
        };
        alerts.push((format!("SKIN {skin:.0} K / {SKIN_LIMIT:.0} K{note}"), c));
    }
    match &ship.state {
        ShipState::Landed { body, local_position, .. } => {
            let b = &app.view.system.bodies[*body];
            let port = app.view.system.port_at(*body, local_position.normalize()).map(|p| &app.view.system.spaceports[p]);
            if let Some(p) = port {
                lines.push((format!("LANDED AT {} ({})", p.name.to_uppercase(), b.name.to_uppercase()), AMBER));
                lines.push((lift_off_hint(ship), DIM));
            } else if b.kind == BodyKind::Station {
                let pad = universe_sim::world::station::pad_at(*local_position).map_or_else(String::new, |k| format!(" - PAD {}", k + 1));
                lines.push((format!("DOCKED AT {}{pad}", b.name.to_uppercase()), AMBER));
                lines.push((lift_off_hint(ship), DIM));
            } else {
                lines.push((format!("LANDED ON {}", b.name.to_uppercase()), AMBER));
                lines.push((lift_off_hint(ship), DIM));
            }
        }
        ShipState::Destroyed { respawn_in } => lines.push((format!("DESTROYED  RESPAWN IN {respawn_in:.0}"), RED)),
        ShipState::Transit { to, remaining, .. } => {
            let name = universe_sim::names::star_name(app.charts.galaxy.stars[*to].seed).to_uppercase();
            lines.push((format!("GATE TRANSIT TO {name}  {}", fmt::lag(*remaining)), AMBER));
        }
        ShipState::Anchored { field, body, .. } => {
            let name = app.view.system.field_bodies(*field)[*body].name.to_uppercase();
            lines.push((format!("ANCHORED TO {name}"), AMBER));
        }
        ShipState::Flying => {}
    }
}

/// The prospector: the rock scanned (class, size, spin, range, our drift
/// against it), its make-up up close, and digging it once anchored.
fn prospect_info(app: &App, lines: &mut Vec<(String, Color)>) {
    use universe_sim::world::mining;
    let Some(s) = crate::rocks::scan(app) else { return };
    let r = &s.rock;
    let anchored = matches!(app.ship.state, ShipState::Anchored { .. });
    lines.push((
        format!("ROCK {}  {} {}  {} ACROSS  SPIN {}", s.name, r.class.letter(), r.structure.label(), fmt::distance(s.radius * 2.0), fmt::duration(s.day)),
        HUD,
    ));
    if !anchored {
        let ready = s.gap < crate::mining::rig(app).anchor_reach && s.drift < crate::mining::rig(app).anchor_speed;
        let c = if ready { HUD } else if s.gap < crate::mining::rig(app).anchor_reach { AMBER } else { DIM };
        let closing = app.following.as_ref().is_some_and(|f| matches!(f.0, universe_sim::avionics::follow::Manoeuvre::Surface(_)));
        let hint = if ready {
            format!("  {} TO ANCHOR (MINING)", crate::keys::key(crate::keys::Act::Anchor))
        } else if closing {
            "  CLOSING IN".into()
        } else if s.gap < crate::mining::rig(app).anchor_reach {
            "  MATCH ITS DRIFT".into()
        } else if app.v.avionics.rock_lock.is_some() {
            format!("  {} TO ZERO IN (MINING)", crate::keys::key(crate::keys::Act::ZeroIn))
        } else {
            format!("  {} TO LOCK", crate::keys::key(crate::keys::Act::Lock))
        };
        lines.push((format!("RANGE {}  DRIFT {:.2} M/S{hint}", fmt::distance(s.gap.max(0.0)), s.drift), c));
    }
    if s.gap < crate::rocks::SURVEY_RANGE || anchored {
        let k = &r.composition;
        let pct = |x: f64| x * 100.0;
        let mut parts = Vec::new();
        for (name, x) in [("WATER", k.water), ("ORGANICS", k.organics), ("SILICATES", k.silicates), ("NI-FE", k.metal), ("VOLATILES", k.volatiles)] {
            if x > 0.005 {
                parts.push(format!("{name} {:.0}%", pct(x)));
            }
        }
        if k.pgm_ppm > 0.0 {
            parts.push(format!("PGM {:.0} PPM", k.pgm_ppm));
        }
        lines.push((parts.join("  "), DIM));
        let ore = &app.charts.goods[mining::ore(r).item()];
        lines.push((format!("ORE {}  DIG {:.1} KG/S  ({:.0} KJ/KG)", ore.name.to_uppercase(), crate::mining::rig(app).dig_rate(r), mining::specific_energy(r) / 1000.0), DIM));
    } else {
        lines.push((format!("SPECTRUM {}  (SURVEY WITHIN {})", r.class.label(), fmt::distance(crate::rocks::SURVEY_RANGE)), DIM));
    }
    if anchored {
        let left = (s.mass - app.v.dug - app.ship.hopper).max(0.0);
        let hopper: String = (0..10).map(|i| if (i as f64) < app.ship.hopper / 100.0 - 0.01 { '#' } else { '.' }).collect();
        let state = if app.ship.excavator { "DIGGING".to_string() } else { format!("{} TO DIG", crate::keys::key(crate::keys::Act::Excavate)) };
        lines.push((format!("{state}  HOPPER [{hopper}]  HOLD {:.1}/{:.0} T  ROCK LEFT {}", app.ship.cargo / 1000.0, app.ship.spec().hold_capacity / 1000.0, fmt::tonnes(left)), if app.ship.excavator { AMBER } else { HUD }));
        if app.ship.excavator {
            // The flow, and when the next tonne goes into the hold.
            let rate = crate::mining::rig(app).dig_rate(r);
            let next = (universe_sim::world::goods::TONNE - app.ship.hopper) / rate;
            let ore = &app.charts.goods[mining::ore(r).item()];
            lines.push((format!("EXTRACTING {} {rate:.1} KG/S ({:.0} T/H)  NEXT TONNE IN {}", ore.name.to_uppercase(), rate * 3.6, fmt::countdown(next)), AMBER));
        }
    }
}

/// The hold (4): what's in it — each good's units, mass, volume as stowed
/// and worth — the loose ore in the hopper, and the totals against capacity.
fn cargo_panel(frame: &mut Frame, app: &App) {
    if !app.show_cargo {
        return;
    }
    let capacity = app.ship.spec().hold_capacity;
    let goods = &app.charts.goods;
    let mut lines: Vec<(String, Color)> = vec![(format!("CARGO HOLD - {:.0} T CAPACITY   ({} CLOSES)", capacity / 1000.0, crate::keys::key(crate::keys::Act::Cargo)), HUD), (String::new(), HUD)];
    lines.push((format!("{:<26} {:<10} {:>5} {:>8} {:>8} {:>9}", "GOOD", "KIND", "UNITS", "MASS", "VOLUME", "WORTH"), DIM));
    let (mut mass, mut volume, mut worth) = (0.0, 0.0, 0.0);
    for &(item, units) in &app.v.hold {
        let Some(g) = goods.get(item) else { continue };
        let m = g.mass * units as f64;
        let v = m / 1000.0 / g.bulk_density;
        let w = g.price * units as f64;
        (mass, volume, worth) = (mass + m, volume + v, worth + w);
        lines.push((format!("{:<26} {:<10} {:>5} {:>8} {:>6.1}M3 {:>6.0} CR", g.name.to_uppercase().chars().take(26).collect::<String>(), g.kind_name(), units, fmt::tonnes(m), v, w), HUD));
    }
    if app.v.hold.is_empty() {
        lines.push(("(EMPTY)".into(), DIM));
    }
    if app.ship.hopper > 0.5 {
        lines.push((format!("{:<26} {:<10} {:>5} {:>8}", "LOOSE ORE IN THE HOPPER", "", "", fmt::tonnes(app.ship.hopper)), AMBER));
    }
    lines.push((String::new(), HUD));
    let full = (mass + app.ship.hopper) / capacity;
    // (Full by weight or by space, whichever first.)
    let room = app.ship.spec().hold_volume;
    let full = full.max(if room > 0.0 { app.ship.cargo_volume / room } else { 0.0 });
    let bar: String = (0..20).map(|i| if (i as f64) < full * 20.0 - 0.01 { '#' } else { '.' }).collect();
    lines.push((format!("LOADED [{bar}] {} OF {}  {:.1} OF {:.0} M3  WORTH ABOUT {:.0} CR", fmt::tonnes(mass + app.ship.hopper), fmt::tonnes(capacity), volume, room, worth), if full > 0.95 { AMBER } else { HUD }));
    lines.push((format!("SHIP {}  (DRY {}, FUEL {}, CARGO {})", fmt::tonnes(app.ship.mass()), fmt::tonnes(app.ship.spec().dry_mass), fmt::tonnes(app.ship.fuel), fmt::tonnes(app.ship.cargo + app.ship.hopper)), DIM));
    let width = lines.iter().map(|l| text_size(&l.0).x).fold(0.0, f32::max);
    let size = frame.size();
    // (Left, under the status lines: the notices go across the middle.)
    let pos = Vec2::new(12.0, (size.y * 0.42).floor());
    frame.hud_rect(pos - 8.0, Vec2::new(width, lines.len() as f32 * LINE) + 16.0, Color([0.012, 0.018, 0.026, 0.9]));
    frame.hud_box(pos - 8.0, Vec2::new(width, lines.len() as f32 * LINE) + 16.0, HUD.scale(0.6));
    for (k, (text, c)) in lines.iter().enumerate() {
        frame.text(pos + Vec2::new(0.0, k as f32 * LINE), text, *c);
    }
}

/// The collision warning: what the path hits and when.
fn collision_info(app: &App, lines: &mut Vec<(String, Color)>) {
    let Some(c) = app.collision.as_ref().filter(|_| impact_shown(app)).and_then(|p| p.collision.as_ref()) else { return };
    let left = (c.time - (app.v.time - app.collision_at)).max(0.0) / app.warp().max(1.0);
    lines.push((format!("COLLISION {} IN {}  AT {}", c.what.to_uppercase(), fmt::countdown(left), fmt::speed(c.speed)), RED));
}

/// The radar: how many ships it sees, and the locked one's range, closing
/// speed and what its transponder says.
fn radar_info(app: &App, lines: &mut Vec<(String, Color)>) {
    if !app.ship.is_flying() && app.contacts.is_empty() {
        return;
    }
    // (The count is on the instruments.)
    let Some(c) = app.contacts.iter().find(|c| Some(c.blip.id) == app.v.avionics.contact) else { return };
    let ship = &app.ship;
    let closing = c.blip.closing_speed(ship.position, ship.velocity);
    let trend = if closing >= 0.0 { "CLOSING" } else { "OPENING" };
    let (tag, col) = if c.aggressed { ("  AGGRESSED", RED) } else { ("", crate::scene::TRAFFIC) };
    lines.push((format!("LOCK {}{tag}  {}  {trend} {}", c.name, fmt::distance(c.blip.distance), fmt::speed(closing.abs())), col));
    // How fast it crosses our view, and which way it's moving, relative to us
    // in our ship's frame (forward, right, up).
    let r = c.blip.position - ship.position;
    let v = c.blip.velocity - ship.velocity;
    let angular = r.cross(v).length() / r.length_squared().max(1.0);
    let local = ship.orientation.inverse() * v;
    lines.push((
        format!(
            "     ANG {:.2} DEG/S  REL {}  FWD {:+.0} RGT {:+.0} UP {:+.0} M/S",
            angular.to_degrees(),
            fmt::speed(v.length()),
            -local.z,
            local.x,
            local.y
        ),
        crate::scene::TRAFFIC.scale(0.8),
    ));
    let bound = c.destination.as_ref().map(|d| format!(" -> {d}")).unwrap_or_default();
    lines.push((format!("     {}{bound}", c.activity), DIM));
    if ship.armed {
        let col = if c.hull < 0.3 { RED } else { AMBER };
        let bar: String = (0..10).map(|i| if (i as f64) < c.hull * 10.0 - 0.01 { '#' } else { '.' }).collect();
        lines.push((format!("     TARGET HULL [{bar}] {:3.0}%", c.hull * 100.0), col));
    }
    // Fire control (combat mode): tracking, then the gun's lead.
    if !app.ship.armed {
        return;
    }
    match &app.fire {
        Some((track, _)) if !track.ready() => lines.push((format!("     FIRE CONTROL: TRACKING {:3.0}%", track.quality() * 100.0), AMBER)),
        Some((_, Some(sol))) => {
            let ship = &app.ship;
            let state = if gun_on(ship, sol.aim) {
                "GUN ON TARGET"
            } else if within_gimbal(ship, sol.aim) {
                "LAYING GUN"
            } else {
                "FLY THE LEAD INTO THE RING"
            };
            lines.push((format!("     {state}  SLUG FLIGHT {:.1} S", sol.time), AMBER));
        }
        Some((_, None)) => lines.push(("     NO FIRING SOLUTION - OUT OF GUN RANGE".into(), DIM)),
        None => {}
    }
}


/// The route: which stop we're on and what the route autopilot is doing.
fn route_info(app: &App, lines: &mut Vec<(String, Color)>) {
    let r = &app.v.avionics.route;
    if r.stops.is_empty() {
        return;
    }
    let n = r.next.min(r.stops.len() - 1);
    let name = app.route_labels.get(n).cloned().unwrap_or_default();
    if !r.active {
        lines.push((format!("ROUTE {}/{} -> {name}", n + 1, r.stops.len()), DIM));
        return;
    }
    let ship = &app.ship;
    let stage = if let Some(until) = r.dwell_until {
        format!("AT STOP - LEAVING IN {}", fmt::countdown((until - app.v.time) / app.warp().max(1.0)))
    } else if r.departing {
        "CLIMBING".into()
    } else if matches!(ship.state, ShipState::Transit { .. }) {
        "GATE TRANSIT".into()
    } else if ship.hyperdrive {
        "HYPERDRIVE".into()
    } else if let Some(c) = app.v.avionics.clearance {
        match c.target {
            universe_sim::NavTarget::Station(_) => "DOCKING".into(),
            universe_sim::NavTarget::Spaceport(_) => "LANDING".into(),
            universe_sim::NavTarget::Gate(_) => "GATE RUN".into(),
            universe_sim::NavTarget::Asteroid(_) => "UNDERWAY".into(),
        }
    } else {
        "UNDERWAY".into()
    };
    lines.push((format!("ROUTE {}/{} -> {name}  AUTO: {stage}", n + 1, r.stops.len()), AMBER));
}

/// Clearance, guidance numbers and hints for docking or landing.
fn approach_info(app: &App, lines: &mut Vec<(String, Color)>) {
    match &app.approach {
        None => {
            if app.ship.is_flying()
                && app.v.avionics.nav_target.is_some()
                && let Some((name, _)) = &app.nav_marker
            {
                lines.push((format!("NAV {name}"), DIM));
            }
        }
        Some(Approach::Dock { station, status }) => docking_info(app, *station, status, lines),
        Some(Approach::Land { port, status }) => landing_info(app, *port, status, lines),
        Some(Approach::Transit { gate, status }) => transit_info(app, *gate, status, lines),
    }
    queue_info(app, lines);
}

/// Waiting our turn: for a pad (holding over the port), or for a station's
/// or gate's corridor (one ship at a time), and how many pilots are ahead.
fn queue_info(app: &App, lines: &mut Vec<(String, Color)>) {
    let a = &app.v.avionics;
    let Some(c) = a.clearance else { return };
    let pilots = |n: usize| match n {
        0 => "NEXT IN LINE".to_string(),
        1 => "1 PILOT AHEAD".to_string(),
        _ => format!("{n} PILOTS AHEAD"),
    };
    if let universe_sim::avionics::nav::PadSlot::Hold(n) = c.pad {
        // (All pads taken: those ahead are the ones waiting before us.)
        lines.push((format!("QUEUED FOR A PAD - {} - HOLD CLEAR", pilots(n)), AMBER));
    } else if let Some(n) = a.corridor_ahead {
        lines.push((format!("QUEUED FOR THE GATE RUN - {} - HOLD CLEAR", pilots(n)), AMBER));
    }
}

fn mode_label(autopilot: bool, phase: universe_sim::Phase) -> String {
    if autopilot { format!("AUTO {}", phase.label()) } else { "MANUAL  K=AUTO".into() }
}

/// What it takes to lift off: powering up first, if the ship's parked.
fn lift_off_hint(ship: &universe_sim::world::Ship) -> String {
    if ship.powered { "SHIFT+E TO LIFT OFF".into() } else { format!("FLIGHT SYSTEMS DOWN - {} TO POWER UP", crate::keys::key(crate::keys::Act::Systems)) }
}

fn docking_info(app: &App, station: usize, st: &DockingStatus, lines: &mut Vec<(String, Color)>) {
    let name = app.view.system.bodies[station].name.to_uppercase();
    let pad = match app.v.avionics.clearance.map(|c| c.pad) {
        Some(universe_sim::avionics::nav::PadSlot::Hold(n)) => format!("  HOLDING ({n} AHEAD)"),
        _ => format!("  PAD {}", st.pad + 1),
    };
    lines.push((format!("DOCK {name}{pad}  {}", mode_label(st.autopilot, st.phase)), HUD));

    // The speed limit only applies coming down; elsewhere show the guidance speed.
    let in_final = st.guidance.final_run;
    let too_fast = in_final && (st.closing > st.speed_limit * 1.1 || (st.height < 100.0 && st.speed > DECK_SPEED));
    let target = if in_final {
        format!("LIMIT {}", fmt::speed(st.speed_limit))
    } else {
        format!("GO {}", fmt::speed(st.guidance.desired_velocity.length()))
    };
    lines.push((
        format!("RANGE {}  DESCENT {}  {target}", fmt::distance(st.range), fmt::speed(st.closing)),
        if too_fast { RED } else { HUD },
    ));
    lines.push((
        format!("ABOVE PAD {}  OFF ITS LINE {}", fmt::distance(st.height), fmt::distance(st.offset)),
        if st.lined_up { HUD } else { AMBER },
    ));
    lines.push((format!("REL SPEED {}", fmt::speed(st.speed)), PREDICT));
    action_lines(app, st.relative_velocity, &st.guidance, lines);
    if st.autopilot {
        return;
    }
    let hint = if st.height < 0.0 {
        "BELOW THE DECK - GO AROUND AND ABOVE IT"
    } else if !st.lined_up {
        "OVER THE PAD, BELLY TO THE DECK (SHIFT+WASDQE)"
    } else if too_fast {
        "SLOW DOWN"
    } else {
        "ON COURSE - COME DOWN THROUGH THE SQUARES"
    };
    lines.push((hint.into(), DIM));
}

fn transit_info(app: &App, gate: usize, st: &universe_sim::GateStatus, lines: &mut Vec<(String, Color)>) {
    use universe_sim::gate::TRANSIT_SPEED;
    // (The fastest its ring catches a ship: the ring's own spec.)
    let max = universe_sim::world::hypernet::capture_speed(&app.charts.galaxy, &app.view.system, gate);
    let name = app.view.system.bodies[gate].name.to_uppercase();
    lines.push((format!("TRANSIT {name}  {}", mode_label(st.autopilot, st.phase)), HUD));
    let too_fast = st.speed > max;
    let target = if st.guidance.final_run {
        format!("PASS AT {}, MAX {}", fmt::speed(TRANSIT_SPEED), fmt::speed(max))
    } else {
        format!("GO {}", fmt::speed(st.guidance.desired_velocity.length()))
    };
    lines.push((format!("RANGE {}  CLOSING {}  {target}", fmt::distance(st.range), fmt::speed(st.closing)), if too_fast { RED } else { HUD }));
    lines.push((format!("AXIS OFFSET {}", fmt::distance(st.offset)), if st.in_corridor { HUD } else { AMBER }));
    lines.push((format!("REL SPEED {}", fmt::speed(st.speed)), PREDICT));
    action_lines(app, st.relative_velocity, &st.guidance, lines);
    if st.autopilot {
        return;
    }
    let hint = if too_fast {
        "TOO FAST FOR THE GATE - SLOW BELOW 300 M/S"
    } else if !st.in_corridor {
        "LINE UP WITH THE RING'S AXIS"
    } else {
        "ON COURSE - THROUGH THE MIDDLE OF THE RING"
    };
    lines.push((hint.into(), DIM));
}

fn landing_info(app: &App, port: usize, st: &LandingStatus, lines: &mut Vec<(String, Color)>) {
    let sys = &app.view.system;
    let p = &sys.spaceports[port];
    let name = format!("{} ({})", p.name, sys.bodies[p.body].name).to_uppercase();
    let pad = match app.v.avionics.clearance.map(|c| c.pad) {
        Some(universe_sim::avionics::nav::PadSlot::Pad(k)) => format!("  PAD {}", k + 1),
        Some(universe_sim::avionics::nav::PadSlot::Hold(n)) => format!("  HOLDING ({n} AHEAD)"),
        _ => String::new(),
    };
    lines.push((format!("LAND {name}{pad}  {}", mode_label(st.autopilot, st.phase)), HUD));
    // What this ground asks of the ship as it is: its lift against its weight here, what its legs take.
    let g = universe_sim::world::legs::surface_gravity(&sys.bodies[p.body]);
    let (lift, legs) = universe_sim::world::legs::ground_check(app.ship.spec(), app.ship.mass(), g);
    let legs = legs.map_or(String::new(), |v| if v > 0.0 { format!("  LEGS TAKE {}", fmt::speed(v)) } else { "  ITS LEGS CAN'T STAND ITS WEIGHT HERE".into() });
    let hover = if lift < 1.0 { "  CAN'T HOVER" } else { "" };
    lines.push((format!("GROUND {g:.1} M/S2  LIFT {lift:.2}x WEIGHT{hover}{legs}"), if lift < 1.0 || legs.contains("CAN'T") { RED } else { DIM }));

    let descent = st.guidance.final_run;
    let sink_target = -st.guidance.desired_velocity.dot(st.pad.up);
    let too_fast = descent && -st.vertical_speed > sink_target * 1.5 + 3.0;
    let target = if descent {
        format!("SINK {}", fmt::speed(sink_target))
    } else {
        format!("GO {}", fmt::speed(st.guidance.desired_velocity.length()))
    };
    lines.push((
        format!("RANGE {}  ALT {}  {target}", fmt::distance(st.range), fmt::distance(st.altitude)),
        if too_fast { RED } else { HUD },
    ));
    let tilt = st.tilt.to_degrees();
    let tilted = descent && tilt > 20.0;
    lines.push((
        format!(
            "VSPD {}  HSPD {}  OFFSET {}  TILT {tilt:.0} DEG",
            fmt::speed(st.vertical_speed),
            fmt::speed(st.horizontal_speed),
            fmt::distance(st.horizontal_distance)
        ),
        if tilted { AMBER } else { HUD },
    ));
    lines.push((format!("REL SPEED {}", fmt::speed(st.relative_velocity.length())), PREDICT));
    action_lines(app, st.relative_velocity, &st.guidance, lines);
    if st.autopilot {
        return;
    }
    let impact_far = st.impact.is_some_and(|i| i.distance(st.pad.pad) > 5_000.0);
    let hint = if descent && tilted {
        "LEVEL OUT - BELLY TO THE GROUND"
    } else if too_fast {
        "SINKING TOO FAST - SHIFT+E TO BRAKE"
    } else if descent {
        "DESCEND ONTO THE PAD - GENTLY"
    } else if impact_far && st.altitude < 50_000.0 {
        "IMPACT AHEAD - PULL UP"
    } else {
        "FOLLOW THE PATH TO THE BEACON"
    };
    lines.push((hint.into(), DIM));
}

/// Top-center banner: the steps of the docking or landing procedure, the
/// current one highlighted, and the time to arrival from the flight plan.
/// Where a guidance banner goes: the very top line.
pub fn banner_y(_app: &App) -> f32 {
    4.0
}

fn phase_banner(frame: &mut Frame, app: &App) {
    use universe_sim::Phase;
    let Some(approach) = &app.approach else { return };
    let plan = app.plan.as_ref();
    // Flying by hand, the plan knows which step we're in; the autopilot keeps its own.
    let phase = |own: Phase, autopilot: bool| if autopilot { own } else { plan.and_then(|p| p.points.first()).map_or(own, |p| p.phase) };
    let (steps, current): (&[&str], usize) = match approach {
        Approach::Dock { status, .. } => {
            let step = match phase(status.phase, status.autopilot) {
                Phase::Approach => 0,
                Phase::Align => 1,
                _ => 2,
            };
            (&["APPROACH", "OVER THE PAD", "SET DOWN"], step)
        }
        Approach::Land { status, .. } => {
            let step = match phase(status.phase, status.autopilot) {
                Phase::Descent => 2,
                // Heading straight for the point above the pad, or still going around the planet?
                _ if status.guidance.waypoint.distance(status.pad.entry()) < 1.0 => 1,
                _ => 0,
            };
            (&["CRUISE AROUND PLANET", "APPROACH PAD", "VERTICAL DESCENT"], step)
        }
        Approach::Transit { status, .. } => {
            let step = match phase(status.phase, status.autopilot) {
                Phase::Approach => 0,
                Phase::Align => 1,
                _ => 2,
            };
            (&["APPROACH GATE", "LINE UP", "TRANSIT RUN"], step)
        }
    };
    let eta = match plan {
        // In real seconds, smoothed (see `App::eta_shown`).
        Some(p) if p.arrives => format!("ETA {}", fmt::countdown(app.eta_shown.unwrap_or(0.0))),
        Some(p) if p.holds => "HOLDING".into(),
        Some(_) => "ETA > 6 H".into(),
        None => String::new(),
    };

    let parts: Vec<String> = steps.iter().enumerate().map(|(i, s)| format!("{} {s}", i + 1)).collect();
    let sep = "  >  ";
    let full = format!("{}      {eta}", parts.join(sep));
    let size = frame.size();
    let mut x = ((size.x - text_size(&full).x) / 2.0).floor();
    let y = banner_y(app);
    frame.hud_rect(Vec2::new(x - 6.0, y - 3.0), Vec2::new(text_size(&full).x + 12.0, GLYPH + 6.0), PANEL);
    for (i, part) in parts.iter().enumerate() {
        let c = match i.cmp(&current) {
            std::cmp::Ordering::Less => DIM.scale(0.7),
            std::cmp::Ordering::Equal => AMBER,
            std::cmp::Ordering::Greater => DIM,
        };
        if i == current {
            frame.hud_box(Vec2::new(x - 3.0, y - 2.0), Vec2::new(text_size(part).x + 6.0, GLYPH + 4.0), AMBER);
        }
        x = frame.text(Vec2::new(x, y), part, c).x;
        if i + 1 < parts.len() {
            x = frame.text(Vec2::new(x, y), sep, DIM).x;
        }
    }
    frame.text(Vec2::new(x + 6.0 * GLYPH, y), &eta, HUD);
}

/// What to do right now, from the flight plan: turn to the frame, burn,
/// thrust or coast.
fn action_lines(app: &App, relative_velocity: DVec3, g: &Guidance, lines: &mut Vec<(String, Color)>) {
    let Some(plan) = &app.plan else { return };
    let Some(first) = plan.points.first() else { return };
    let turn = (app.ship.orientation.inverse() * first.aim).normalize();
    let turn_deg = (2.0 * turn.w.abs().clamp(0.0, 1.0).acos()).to_degrees();
    let now = if turn_deg > 8.0 {
        format!("TURN: NOSE ON (+), MATCH THE FRAME  {turn_deg:.0} DEG")
    } else {
        match first.action {
            Action::Burn(th) => format!("BURN: W  THROTTLE {:.0}%", th * 100.0),
            Action::Thrusters => thrust_hint(app, relative_velocity, g),
            Action::Coast => "COAST - NO THRUST".into(),
        }
    };
    let c = crate::scene::action_color(first.action);
    lines.push((now, if first.action == Action::Coast { GUIDE_PATH } else { c }));
    lines.push((format!("PATH {} TO GO", fmt::distance(crate::scene::path_length(plan))), DIM));
}

/// Which keys to press, and roughly how much, to match the guidance velocity.
fn thrust_hint(app: &App, relative_velocity: DVec3, g: &Guidance) -> String {
    let dv_world = g.desired_velocity - relative_velocity;
    if dv_world.length() > 150.0 {
        return format!("BURN {}: NOSE ON (+), THEN W", fmt::speed(dv_world.length()));
    }
    let dv = app.ship.orientation.inverse() * dv_world;
    let mut parts = Vec::new();
    for (value, neg, pos) in [(dv.x, "A", "D"), (dv.y, "Q", "E"), (-dv.z, "S", "W")] {
        if value.abs() > 1.5 {
            parts.push(format!("{} {:.0}", if value > 0.0 { pos } else { neg }, value.abs()));
        }
    }
    if parts.is_empty() {
        "THRUST: NONE - ON THE PATH".into()
    } else {
        format!("SHIFT+ {} M/S", parts.join("  "))
    }
}

/// Always point the pilot at the nav target (or the station): a diamond on
/// screen, or an arrow at the edge.
fn target_marker(frame: &mut Frame, app: &App) {
    let Some((name, target)) = &app.nav_marker else { return };
    if matches!(app.ship.state, ShipState::Landed { .. }) {
        return;
    }
    let c = if app.approach.is_some() { HUD } else { Color::hex(0x60c0ff) };
    bracket(frame, app, name, *target, c);
}

/// Missiles after us, in view: a red diamond on each, with its range (seen
/// far off, where the missile itself is a speck).
fn missile_markers(frame: &mut Frame, app: &App) {
    let size = frame.size();
    let back = app.now() - app.v.time;
    let ours: Vec<DVec3> = app.v.missiles.iter().filter(|m| m.4 && m.0 == app.view.origin).map(|m| m.1 + m.2 * back).collect();
    let nearest = ours.iter().copied().min_by(|a, b| a.distance(app.view.ship_pos).total_cmp(&b.distance(app.view.ship_pos)));
    for &at in &ours {
        let Some(s) = frame.project(at).filter(|s| s.x > 0.0 && s.y > 0.0 && s.x < size.x && s.y < size.y) else { continue };
        let d = [Vec2::new(0.0, -6.0), Vec2::new(6.0, 0.0), Vec2::new(0.0, 6.0), Vec2::new(-6.0, 0.0)];
        for i in 0..4 {
            frame.hud_line(s + d[i], s + d[(i + 1) % 4], RED);
        }
        // (The range on the nearest only: a salvo comes in close together.)
        if Some(at) == nearest {
            let range = fmt::distance(at.distance(app.view.ship_pos));
            frame.text(s + Vec2::new(-text_size(&range).x / 2.0, 8.0), &range, RED.scale(0.8));
        }
    }
}

/// Radar contacts in view: a small square on each ship, with its range when
/// near; the locked one gets a bracket like the nav target's.
/// Most tags on the radar's contacts at once (the nearest have them).
const TAGS: usize = 6;

fn contact_marker(frame: &mut Frame, app: &App) {
    let size = frame.size();
    // Each contact a small mark; the nearest few a tag beside it (its name
    // close in, its range) where it stands clear of the others' — a crowd
    // shows its nearest, not a heap of text.
    let mut near: Vec<_> = app.contacts.iter().filter(|c| app.v.avionics.contact != Some(c.blip.id)).collect();
    near.sort_by(|a, b| a.blip.distance.total_cmp(&b.blip.distance));
    let mut placed: Vec<(Vec2, Vec2)> = Vec::new();
    // (The nav target's bracket and label keep their place: see `bracket`.)
    if let Some((name, target)) = &app.nav_marker
        && let Some(p) = frame.project(*target)
    {
        let label = format!("{name} {}", fmt::distance(target.distance(app.view.ship_pos)));
        let w = text_size(&label).x;
        placed.push((p - Vec2::splat(12.0), Vec2::splat(24.0)));
        placed.push((p + Vec2::new(-w / 2.0 - 2.0, 12.0), Vec2::new(w + 4.0, GLYPH + 4.0)));
    }
    let mut tags = 0;
    for contact in near {
        let c = if contact.aggressed { RED } else { crate::scene::TRAFFIC };
        let at = app.place(crate::Who::Craft(contact.blip.id)).0;
        let Some(p) = frame.project(at).filter(|p| p.x > 0.0 && p.y > 0.0 && p.x < size.x && p.y < size.y) else { continue };
        // (Fainter the farther.)
        let k = (1.0f32 - (contact.blip.distance / 60_000.0) as f32).clamp(0.45, 1.0);
        frame.hud_box(p - Vec2::splat(3.0), Vec2::splat(6.0), c.scale(0.8 * k));
        if tags >= TAGS || contact.blip.distance > 50_000.0 {
            continue;
        }
        let range = fmt::distance(contact.blip.distance);
        let text = if contact.blip.distance < 5_000.0 { format!("{}  {range}", contact.name.to_uppercase()) } else { range };
        let pos = p + Vec2::new(6.0, -4.0);
        let extent = text_size(&text) + Vec2::new(6.0, 5.0);
        let overlaps = |&(q, e): &(Vec2, Vec2)| pos.x < q.x + e.x && q.x < pos.x + extent.x && pos.y < q.y + e.y && q.y < pos.y + extent.y;
        if placed.iter().any(overlaps) {
            continue;
        }
        placed.push((pos, extent));
        tags += 1;
        frame.text(pos, &text, c.scale(0.75 * k));
    }
    if let Some(locked) = app.contacts.iter().find(|c| Some(c.blip.id) == app.v.avionics.contact) {
        let c = if locked.aggressed { RED } else { crate::scene::TRAFFIC };
        let at = app.place(crate::Who::Craft(locked.blip.id)).0;
        bracket(frame, app, &locked.name, at, c);
        // Which way it's moving across our view: an arrow off its bracket.
        let v = locked.blip.velocity - app.ship.velocity;
        if v.length() > 0.5
            && let (Some(p), Some(q)) = (frame.project(at), frame.project(at + v * 2.0))
            && let Some(dir) = (q - p).try_normalize()
        {
            let (a, b) = (p + dir * 14.0, p + dir * 30.0);
            frame.hud_line(a, b, c);
            frame.hud_line(b, b - dir * 5.0 + dir.perp() * 3.0, c);
            frame.hud_line(b, b - dir * 5.0 - dir.perp() * 3.0, c);
        }
        // The lead (combat mode): fly it into the gimbal ring; fire control
        // lays the gun on it, and the circle doubles up when the gun is on.
        if app.ship.armed
            && let Some((_, Some(sol))) = &app.fire
            && let Some(p) = frame.project(app.view.ship_pos + sol.offset)
        {
            let ship = &app.ship;
            let c = if within_gimbal(ship, sol.aim) { AMBER } else { AMBER.scale(0.55) };
            frame.hud_ellipse(p, Vec2::splat(5.0), 12, c);
            if gun_on(ship, sol.aim) {
                frame.hud_ellipse(p, Vec2::splat(8.0), 16, c);
                frame.hud_ellipse(p, Vec2::splat(3.0), 8, c);
            }
            frame.hud_line(p - Vec2::new(2.0, 0.0), p + Vec2::new(2.0, 0.0), c);
            frame.hud_line(p - Vec2::new(0.0, 2.0), p + Vec2::new(0.0, 2.0), c);
            if let Some(q) = frame.project(app.place(crate::Who::Craft(locked.blip.id)).0) {
                let d = q - p;
                if d.length() > 12.0 {
                    frame.hud_line(p + d.normalize() * 6.0, q - d.normalize() * 10.0, AMBER.scale(0.4));
                }
            }
        }
    }
}

/// Where the nose and the gun point, in any view: a small cross on the nose
/// (in the chase view; the cockpit has its crosshair); in combat mode the
/// gimbal's reach as a ring around it, and the gun's pipper. Plus sparks
/// where hits land, and HIT on the locked target when ours do.
fn gunsight(frame: &mut Frame, app: &App) {
    let ship = &app.ship;
    let from = app.view.ship_pos;
    let far = 1.0e5;
    let col = if ship.weapons_hot() { RED } else if ship.armed { AMBER } else { HUD };
    if let Some(nose) = frame.project(from + ship.forward() * far) {
        if app.chase_cam && crosshair_wanted(app) {
            let k = if ship.armed { 1.0 } else { 0.6 };
            for d in [Vec2::X, Vec2::NEG_X, Vec2::Y, Vec2::NEG_Y] {
                frame.hud_line(nose + d * 3.0, nose + d * 7.0, col.scale(k));
            }
        }
        if ship.armed {
            // The gimbal's cone, projected: its radius on screen.
            let side = ship.orientation * DVec3::X;
            let edge = from + (ship.forward() * GIMBAL_LIMIT.cos() + side * GIMBAL_LIMIT.sin()) * far;
            if let Some(e) = frame.project(edge) {
                let r = e.distance(nose);
                frame.hud_ellipse(nose, Vec2::splat(r), 32, col.scale(0.6));
            }
            if let Some(g) = frame.project(from + ship.gun_forward() * far) {
                frame.hud_ellipse(g, Vec2::splat(3.0), 8, col);
                frame.hud_rect(g - Vec2::splat(0.5), Vec2::ONE, col);
            }
        }
    }
    // The lock beam: a ring of `LOCK_BEAM` around the nose, in combat mode
    // and for a moment after T.
    if (ship.armed || app.beam_shown > 0.0)
        && let Some(nose) = frame.project(from + ship.forward() * far)
    {
        let side = ship.orientation * DVec3::Y;
        let beam = universe_sim::LOCK_BEAM;
        if let Some(e) = frame.project(from + (ship.forward() * beam.cos() + side * beam.sin()) * far) {
            let k = if ship.armed { 0.7 } else { (app.beam_shown / 1.5).min(1.0) };
            let c = crate::scene::TRAFFIC.scale(k);
            let r = e.distance(nose);
            frame.hud_ellipse(nose, Vec2::splat(r), 48, c);
            for d in [Vec2::X, Vec2::NEG_X, Vec2::Y, Vec2::NEG_Y] {
                frame.hud_line(nose + d * (r - 4.0), nose + d * (r + 4.0), c);
            }
        }
    }
    // Sparks where hits landed (in view), ours brighter.
    for s in &app.sparks {
        if s.system != app.view.origin {
            continue;
        }
        let Some(p) = frame.project(s.point) else { continue };
        let k = (1.0 - s.age / SPARK_TIME).max(0.0);
        let c = if s.laser { Color::hex(0xff8060) } else { Color::hex(0xffe080) }.scale(k * if s.ours { 1.0 } else { 0.6 });
        let r = 3.0 + 6.0 * (s.age / SPARK_TIME);
        for i in 0..6 {
            let a = i as f32 * std::f32::consts::TAU / 6.0 + s.age * 3.0;
            let d = Vec2::new(a.cos(), a.sin());
            frame.hud_line(p + d * r * 0.4, p + d * r, c);
        }
    }
    // HIT on the locked target when one of ours lands.
    if let Some(locked) = app.contacts.iter().find(|c| Some(c.blip.id) == app.v.avionics.contact)
        && app.sparks.iter().any(|s| s.ours && s.target == universe_sim::craft_id(locked.blip.id) && s.age < 0.6)
        && let Some(p) = frame.project(app.place(crate::Who::Craft(locked.blip.id)).0)
    {
        frame.text(p + Vec2::new(12.0, -18.0), "HIT", RED);
    }
}

/// The nose crosshair is for fighting and for approaches (putting the nose
/// on the plan's cue); in plain travel it stays out of the way.
fn crosshair_wanted(app: &App) -> bool {
    app.ship.armed || app.approach.is_some()
}

/// The impact warning is navigation's: it shows in Nav mode, where its
/// switch is (it keeps watching in the others, quietly).
pub fn impact_shown(app: &App) -> bool {
    active_mode(app) == ShipMode::Nav
}

/// The collision warning's impact, labelled on screen (or an arrow to it
/// from the edge).
fn impact_label(frame: &mut Frame, app: &App) {
    let Some(p) = app.collision.as_ref().filter(|_| impact_shown(app)) else { return };
    let Some(c) = &p.collision else { return };
    let at = app.view.positions[p.reference] + c.offset;
    let left = (c.time - (app.v.time - app.collision_at)).max(0.0) / app.warp().max(1.0);
    let label = format!("IMPACT {}", fmt::countdown(left));
    bracket(frame, app, &label, at, RED);
}

/// How long a hit's spark shows (s).
pub const SPARK_TIME: f32 = 0.5;

/// A diamond bracket around `target` with its name and range, or an arrow
/// at the screen's edge pointing to it.
fn bracket(frame: &mut Frame, app: &App, name: &str, target: DVec3, c: Color) {
    let range = fmt::distance(target.distance(app.view.ship_pos));
    let size = frame.size();

    if let Some(p) = frame.project(target).filter(|p| p.x > 8.0 && p.y > 8.0 && p.x < size.x - 8.0 && p.y < size.y - 8.0) {
        let k = 10.0;
        let pts = [p + Vec2::new(0.0, -k), p + Vec2::new(k, 0.0), p + Vec2::new(0.0, k), p + Vec2::new(-k, 0.0)];
        for j in 0..4 {
            frame.hud_line(pts[j], pts[(j + 1) % 4], c);
        }
        let label = format!("{name} {range}");
        frame.text(p + Vec2::new(-text_size(&label).x / 2.0, 14.0), &label, c);
        return;
    }
    // Off screen: direction in camera space, pinned to an inset ellipse.
    let cam = &frame.camera;
    let local = (cam.orientation.inverse() * cam.relative(target)).xy();
    let dir = Vec2::new(local.x, -local.y).try_normalize().unwrap_or(Vec2::NEG_Y);
    let center = size / 2.0;
    let edge = center + dir * (size / 2.0 - Vec2::new(40.0, 40.0));
    let side = dir.perp();
    let tip = edge + dir * 10.0;
    frame.hud_line(tip, edge - dir * 6.0 + side * 8.0, c);
    frame.hud_line(tip, edge - dir * 6.0 - side * 8.0, c);
    frame.hud_line(edge - dir * 6.0 + side * 8.0, edge - dir * 6.0 - side * 8.0, c);
    let label_pos = edge - dir * 28.0 - Vec2::new(text_size(&range).x / 2.0, 4.0);
    frame.text(label_pos, &range, c);
}

fn marker(frame: &mut Frame, at: Vec2, c: Color, cross: bool) {
    frame.hud_ellipse(at, Vec2::splat(6.0), 12, c);
    if cross {
        frame.hud_line(at + Vec2::new(-8.0, -8.0), at + Vec2::new(8.0, 8.0), c);
        frame.hud_line(at + Vec2::new(-8.0, 8.0), at + Vec2::new(8.0, -8.0), c);
    } else {
        for d in [Vec2::X, Vec2::NEG_X, Vec2::NEG_Y] {
            frame.hud_line(at + d * 7.0, at + d * 12.0, c);
        }
    }
}

fn pilot_overlay(frame: &mut Frame, app: &App) {
    let size = frame.size();
    let c = (size / 2.0).floor();
    if !app.chase_cam && crosshair_wanted(app) {
        let ship = &app.ship;
        let col = if ship.weapons_hot() { RED } else if ship.armed { AMBER } else { HUD };
        for (a, b) in [(Vec2::new(-14.0, 0.0), Vec2::new(-5.0, 0.0)), (Vec2::new(5.0, 0.0), Vec2::new(14.0, 0.0))] {
            frame.hud_line(c + a, c + b, col);
            frame.hud_line(c + a.perp(), c + b.perp(), col);
        }
    }
    gunsight(frame, app);

    let cam = frame.camera.position;

    // Docking/landing: motion relative to the target, plus where our motion should point.
    let cue = match &app.approach {
        Some(Approach::Dock { status, .. }) => Some((status.relative_velocity, status.guidance)),
        Some(Approach::Land { status, .. }) => Some((status.relative_velocity, status.guidance)),
        Some(Approach::Transit { status, .. }) => Some((status.relative_velocity, status.guidance)),
        None => None,
    };
    if let Some((v, _)) = cue {
        if v.length() > 0.3 {
            if let Some(p) = frame.project(cam + v.normalize() * 1.0e3) {
                marker(frame, p, PREDICT, false);
            }
            if let Some(p) = frame.project(cam - v.normalize() * 1.0e3) {
                marker(frame, p, PREDICT.scale(0.7), true);
            }
        }
        // Nose cue: where the plan wants the nose now. Put the crosshair on it.
        if let Some(first) = app.plan.as_ref().and_then(|p| p.points.first()) {
            let path_len = app.plan.as_deref().map_or(0.0, crate::scene::path_length);
            let nose = first.aim * DVec3::NEG_Z;
            let c = crate::scene::action_color(first.action).scale(1.2);
            let size = frame.size();
            match frame.project(cam + nose * 1.0e3).filter(|p| p.x > 12.0 && p.y > 12.0 && p.x < size.x - 12.0 && p.y < size.y - 12.0) {
                Some(p) => {
                    frame.hud_ellipse(p, Vec2::splat(9.0), 16, c);
                    frame.hud_line(p - Vec2::new(13.0, 0.0), p + Vec2::new(13.0, 0.0), c);
                    frame.hud_line(p - Vec2::new(0.0, 13.0), p + Vec2::new(0.0, 13.0), c);
                    // How far there is still to fly along the planned path.
                    let label = format!("PATH {}", fmt::distance(path_len));
                    frame.text(p + Vec2::new(-text_size(&label).x / 2.0, -26.0), &label, c);
                }
                None => {
                    let local = (frame.camera.orientation.inverse() * nose.as_vec3()).xy();
                    let dir = Vec2::new(local.x, -local.y).try_normalize().unwrap_or(Vec2::NEG_Y);
                    let edge = size / 2.0 + dir * (size / 2.0 - Vec2::new(60.0, 60.0));
                    let side = dir.perp();
                    frame.hud_line(edge + dir * 12.0, edge + side * 9.0, c);
                    frame.hud_line(edge + dir * 12.0, edge - side * 9.0, c);
                    frame.text(edge - dir * 22.0 - Vec2::new(16.0, 4.0), "TURN", c);
                }
            }
        }
        return;
    }

    // Prograde / retrograde relative to the dominant body: the key to landing.
    let Some(r) = app.view.reference else { return };
    let rel_vel = app.ship.velocity - app.view.system.velocity(r, app.v.time);
    if rel_vel.length() > 0.5 && !app.ship.hyperdrive {
        let dir = rel_vel.normalize() * 1.0e3;
        if let Some(p) = frame.project(cam + dir) {
            marker(frame, p, AMBER, false);
        }
        if let Some(p) = frame.project(cam - dir) {
            marker(frame, p, AMBER.scale(0.7), true);
        }
    }

    // Bracket the reference body.
    if let Some(p) = frame.project(app.view.positions[r]) {
        let k = 10.0;
        for (sx, sy) in [(-1.0, -1.0), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)] {
            let corner = p + Vec2::new(sx * k, sy * k);
            frame.hud_line(corner, corner - Vec2::new(sx * 5.0, 0.0), DIM);
            frame.hud_line(corner, corner - Vec2::new(0.0, sy * 5.0), DIM);
        }
    }
}

/// The 3D scanner: an ellipse seen in perspective, with height sticks.
/// Range is logarithmic from 1 km (center) to 10^12 m (rim).
fn scanner(frame: &mut Frame, app: &App) {
    let size = frame.size();
    let center = Vec2::new((size.x / 2.0).floor(), size.y - 58.0);
    let radii = Vec2::new(140.0, 42.0);
    // The scope: a soft dark disc (no box), its range rings faint, the rim
    // a little brighter with ticks round it, a sector ahead, our chevron.
    let ring = Color::hex(0x7fb8cc);
    let disc = |frame: &mut Frame, r: Vec2, inner: Color, outer: Color| {
        let n = 128;
        let at = |k: u32| {
            let a = k as f32 / n as f32 * std::f32::consts::TAU;
            center + Vec2::new(a.cos() * r.x, a.sin() * r.y)
        };
        for k in 0..n {
            let (p, q) = (at(k), at(k + 1));
            frame.hud_triangle_colored([center, p, q], [inner, outer, outer]);
        }
    };
    disc(frame, radii * 1.08, Color([0.015, 0.028, 0.04, 0.78]), Color([0.015, 0.028, 0.04, 0.55]));
    for (k, a) in [(1.0 / 3.0, 0.18), (2.0 / 3.0, 0.22)] {
        frame.hud_ellipse(center, radii * k, 128, Color([ring.0[0], ring.0[1], ring.0[2], a]));
    }
    frame.hud_ellipse(center, radii, 160, Color([ring.0[0], ring.0[1], ring.0[2], 0.55]));
    for k in 0..36 {
        let a = k as f32 / 36.0 * std::f32::consts::TAU;
        let dir = Vec2::new(a.cos(), a.sin());
        let len = if k % 9 == 0 { 0.08 } else { 0.035 };
        frame.hud_line_smooth(center + dir * radii, center + dir * radii * (1.0 - len), Color([ring.0[0], ring.0[1], ring.0[2], 0.5]));
    }
    // Ahead (up the scope): a faint sector.
    for s in [-1.0f32, 1.0] {
        let a = -std::f32::consts::FRAC_PI_2 + s * 0.45;
        frame.hud_line_smooth(center, center + Vec2::new(a.cos() * radii.x, a.sin() * radii.y), Color([ring.0[0], ring.0[1], ring.0[2], 0.16]));
    }
    frame.hud_line_smooth(center - Vec2::new(radii.x, 0.0), center + Vec2::new(radii.x, 0.0), Color([ring.0[0], ring.0[1], ring.0[2], 0.1]));
    let chevron = Color([0.9, 0.95, 1.0, 0.9]);
    frame.hud_line_smooth(center + Vec2::new(-3.0, 2.0), center + Vec2::new(0.0, -3.0), chevron);
    frame.hud_line_smooth(center + Vec2::new(3.0, 2.0), center + Vec2::new(0.0, -3.0), chevron);

    let inv = app.ship.orientation.inverse();
    for (i, b) in app.view.system.bodies.iter().enumerate() {
        let rel: DVec3 = inv * (app.view.positions[i] - app.view.ship_pos);
        let d = rel.length();
        if d < 1.0 {
            continue;
        }
        let r = ((d / 1.0e3).max(1.0).log10() / 9.0).min(1.0) as f32;
        let (x, y, z) = ((rel.x / d) as f32, (rel.y / d) as f32, (rel.z / d) as f32);
        let base = center + Vec2::new(x * radii.x * r, z * radii.y * r);
        let top = base - Vec2::new(0.0, y * 36.0 * r);
        let c = color(b.color);
        // (Above the plane a firm stalk; below, fainter.)
        frame.hud_line(base, top, Color([c.0[0], c.0[1], c.0[2], if y > 0.0 { 0.6 } else { 0.3 }]));
        let dot = if b.kind == BodyKind::Star { 3.2 } else { 2.0 };
        frame.hud_glow(top, dot * 2.2, 10, Color([c.0[0], c.0[1], c.0[2], 0.45]), Color([c.0[0], c.0[1], c.0[2], 0.0]));
        frame.hud_glow(top, dot, 10, c, Color([c.0[0], c.0[1], c.0[2], 0.6]));
    }
    // Other ships the radar sees, as small cyan blips (red: aggressed); the locked one boxed.
    let locked = app.v.avionics.contact;
    for contact in &app.contacts {
        let tc = if contact.aggressed { RED } else { crate::scene::TRAFFIC };
        let rel: DVec3 = inv * (app.place(crate::Who::Craft(contact.blip.id)).0 - app.view.ship_pos);
        let d = rel.length();
        if d < 1.0 {
            continue;
        }
        let r = ((d / 1.0e3).max(1.0).log10() / 9.0).min(1.0) as f32;
        let (x, y, z) = ((rel.x / d) as f32, (rel.y / d) as f32, (rel.z / d) as f32);
        let base = center + Vec2::new(x * radii.x * r, z * radii.y * r);
        let top = base - Vec2::new(0.0, y * 36.0 * r);
        frame.hud_line(base, top, Color([tc.0[0], tc.0[1], tc.0[2], if y > 0.0 { 0.55 } else { 0.25 }]));
        frame.hud_glow(top, 3.0, 8, Color([tc.0[0], tc.0[1], tc.0[2], 0.5]), Color([tc.0[0], tc.0[1], tc.0[2], 0.0]));
        frame.hud_glow(top, 1.3, 8, tc, tc);
        if locked == Some(contact.blip.id) {
            frame.hud_ellipse(top, Vec2::splat(5.0), 16, tc);
        }
    }
}

/// The sun's glare: a white-hot core, a halo, and rays in the star's colour,
/// on screen so it reads at any distance. It grows with how bright the sun is
/// here (see `Light`), and whatever stands in the way hides it: a body,
/// fading in as the sun clears its limb; anything drawn (walls round us,
/// a station, a ship), as much of the disc as it covers. Through a window, it shows.
fn sun_glare(frame: &mut Frame, app: &App) {
    let sys = &app.view.system;
    let Some(star) = sys.bodies.iter().position(|b| b.kind == BodyKind::Star) else { return };
    let sun = app.view.positions[star];
    let cam = frame.camera.position;
    let to_sun = sun - cam;
    let dist = to_sun.length();
    let dir = to_sun / dist;
    // How much of the sun's disc is in sight: what planets and moons leave
    // of it (exactly), and what everything drawn leaves (the depth buffer
    // read over its disc on the GPU: a station's structure, a ship, the hull
    // round the eye, a building alike; a frame or two behind).
    let radius = sys.bodies[star].rail.radius;
    frame.sun_probe = Some((sun - dir * radius * 1.05, frame.projected_radius(sun, radius).max(2.0)));
    let visible = (frame.sun_visible(cam) as f32).min(universe_engine::sun_seen());
    if visible <= 0.0 {
        return;
    }
    let Some(p) = frame.project(sun) else { return };
    let size = frame.size();
    if p.x < -200.0 || p.y < -200.0 || p.x > size.x + 200.0 || p.y > size.y + 200.0 {
        return;
    }
    // The glare goes with the light itself: the square root of the
    // irradiance (1 at 1 AU from a sun-like star), unadapted — a hundred
    // times the light close in is ten times the glare.
    let irradiance = universe_engine::Light { position: sun, color: [1.0; 3], luminosity: sys.luminosity, reference: universe_sim::units::AU, radius: 0.0 }.irradiance_at(cam);
    let [r, g, b] = sys.class.color();
    let disc = frame.projected_radius(sun, sys.bodies[star].rail.radius).max(2.0);
    let k = (irradiance.sqrt() as f32).min(12.0);
    // Looking toward it, the view washes out: a veil over everything, the
    // nearer the star and the more squarely we face it.
    let facing = frame.camera.orientation.as_dquat() * DVec3::NEG_Z;
    let toward = facing.dot(dir).max(0.0).powi(6) as f32;
    let veil = (0.015 * k * toward).min(0.3);
    if veil > 0.005 {
        frame.hud_rect(Vec2::ZERO, size, Color([r.max(0.9), g.max(0.85), b.max(0.8), veil * visible]));
    }
    let k = k / 1.6;
    // The glare: light scattered in the eye, brightest at the sun and
    // falling away smoothly with angle (layered glows, each twice as wide
    // and half as strong: no edge), white near it, the star's colour out.
    let mix = |w: f32| [r + (1.0 - r) * w, g + (1.0 - g) * w, b + (1.0 - b) * w];
    let strength = (0.35 + 0.06 * k).min(0.8) * visible;
    let mut radius = disc * 1.6 + 5.0;
    for j in 0..5 {
        let [cr, cg, cb] = mix((1.0 - j as f32 / 3.0).max(0.0));
        let a = strength * 0.4f32.powi(j);
        frame.hud_glow(p, radius, 40, Color([cr, cg, cb, a]), Color([cr, cg, cb, 0.0]));
        radius *= 1.7;
    }
    // The core: in the scene, just this side of the star, so what stands in
    // front of it (a station's structure, a ship) hides it pixel by pixel;
    // white-hot (it blooms in the tone curve).
    let core = disc * 1.4 + 4.0 + 6.0 * k;
    let radius = sys.bodies[star].rail.radius;
    let centre = sun - dir * radius * 1.05;
    let world = core as f64 * radius / disc as f64 * (dist - radius * 1.05) / dist;
    let (u, v) = (dir.any_orthonormal_vector(), dir.cross(dir.any_orthonormal_vector()));
    let rim = |i: usize| {
        let a = i as f64 * std::f64::consts::TAU / 24.0;
        centre + (u * a.cos() + v * a.sin()) * world
    };
    let (white, edge) = (Color([6.0, 6.0, 5.8, 1.0]), Color([r, g, b, 0.0]));
    for i in 0..24 {
        frame.triangle3([centre, rim(i), rim(i + 1)], [white, edge, edge]);
    }
    // Streaks: many fine ones, uneven, a few longer; fading out.
    let long = disc * 2.0 + 60.0 + 200.0 * k;
    for i in 0..32 {
        let h = ((i as u32).wrapping_mul(2_654_435_761) >> 20) as f32 / 4096.0;
        let a = i as f32 * std::f32::consts::TAU / 32.0 + 0.11 + h * 0.08;
        let len = long * if i % 8 == 0 { 1.0 } else { 0.25 + 0.45 * h };
        let d = Vec2::new(a.cos(), a.sin());
        let w = mix(0.7);
        frame.hud_line2(p + d * disc, p + d * len, Color([w[0], w[1], w[2], (if i % 8 == 0 { 0.16 } else { 0.08 }) * visible]), Color([w[0], w[1], w[2], 0.0]));
    }
    // Lens ghosts: faint coloured discs on the line from the sun through
    // the middle of the view, stronger the nearer it is to the middle.
    let middle = size * 0.5;
    let off = middle - p;
    let near_middle = (1.0 - off.length() / size.length()).clamp(0.0, 1.0);
    for (f, rad, col) in [(0.35f32, 9.0f32, [1.0f32, 0.75, 0.35]), (0.7, 20.0, [0.35, 0.9, 0.6]), (1.15, 13.0, [0.35, 0.55, 1.0]), (1.6, 34.0, [0.7, 0.45, 1.0])] {
        let at = p + off * (1.0 + f);
        let a = 0.07 * near_middle * visible * (0.5 + 0.1 * k).min(1.5);
        frame.hud_glow(at, rad, 24, Color([col[0], col[1], col[2], a]), Color([col[0], col[1], col[2], a * 0.4]));
    }
}

/// Defence turrets: a small marker on each in view within 60 km (SAM by it
/// when close), and, while we're aggressed, the reach of their guns as red rings.
fn turret_markers(frame: &mut Frame, app: &App) {
    let me = app.view.ship_pos;
    let hunted = app.v.aggressed_until.is_some();
    let cam = frame.camera.position;
    let size = frame.size();
    for (turret, at) in &app.turrets {
        let d = at.distance(me);
        if d > 60_000.0 {
            continue;
        }
        let c = if hunted { RED } else { Color::hex(0xff9040) };
        if let Some(p) = frame.project(*at).filter(|p| p.x > 0.0 && p.y > 0.0 && p.x < size.x && p.y < size.y) {
            frame.hud_line(p + Vec2::new(-4.0, 3.0), p + Vec2::new(4.0, 3.0), c);
            frame.hud_line(p + Vec2::new(4.0, 3.0), p + Vec2::new(0.0, -4.0), c);
            frame.hud_line(p + Vec2::new(0.0, -4.0), p + Vec2::new(-4.0, 3.0), c);
            if d < 15_000.0 {
                frame.text(p + Vec2::new(6.0, -4.0), "SAM", c.scale(0.8));
            }
        }
        if hunted {
            // The reach, as a ring seen face on.
            let to = (*at - cam).normalize();
            let (u, v) = (to.any_orthonormal_vector(), to.cross(to.any_orthonormal_vector()));
            let pts: Vec<Option<Vec2>> = (0..=32)
                .map(|i| {
                    let a = i as f64 / 32.0 * std::f64::consts::TAU;
                    frame.project(*at + (u * a.cos() + v * a.sin()) * turret.range())
                })
                .collect();
            for w in pts.windows(2) {
                if let (Some(a), Some(b)) = (w[0], w[1]) {
                    frame.hud_line(a, b, RED.scale(0.5));
                }
            }
        }
    }
}

/// How an action grid cell is lit.
#[derive(Clone, Copy, PartialEq)]
pub(crate) enum Lamp {
    /// Available, not in use.
    Off,
    /// In use.
    On,
    /// In progress (e.g. weapons priming).
    Busy,
    /// Weapons hot.
    Hot,
    /// Not available right now.
    Unavailable,
}

/// The pilot's actions as a grid of lit keys, top left: what each key does
/// and whether it's in use. Returns the grid's height.
fn action_grid(frame: &mut Frame, app: &App) {
    let (u, ship) = (&app.v, &app.ship);
    let flying = ship.is_flying();
    let a = &u.avionics;
    // R: what the key does next: request clearance for the target (or the
    // nearest station), or, holding one, clear it.
    let request = match a.nav_target {
        Some(universe_sim::NavTarget::Spaceport(_)) => "LAND",
        Some(universe_sim::NavTarget::Gate(_)) => "GATE",
        _ => "DOCK",
    };
    let clearance = match a.clearance {
        Some(_) => ("CLEAR", Lamp::On),
        None if !flying || ship.armed || ship.hyperdrive => (request, Lamp::Unavailable),
        None => (request, Lamp::Off),
    };
    let on = |b: bool| if b { Lamp::On } else { Lamp::Off };
    let auto = a.route.active || a.hyper_autopilot || a.clearance.is_some_and(|c| c.autopilot);
    // N / U: keep at range, orbit, with the range when engaged; there's
    // something to follow with a lock or a station or gate as the nav target.
    let anchor = a.contact.is_some() || a.rock_lock.is_some() || matches!(a.nav_target, Some(universe_sim::NavTarget::Station(_) | universe_sim::NavTarget::Gate(_) | universe_sim::NavTarget::Asteroid(_)));
    let follow_cell = |kind: &str, on: Option<f64>| match on {
        Some(r) => (format!("{kind} {}", crate::orbitpick::label(r)), Lamp::On),
        None if anchor && flying && !ship.hyperdrive => (kind.to_string(), Lamp::Off),
        None => (kind.to_string(), Lamp::Unavailable),
    };
    use universe_sim::avionics::follow::Manoeuvre;
    let keep = follow_cell("KEEP", a.following.and_then(|f| match f.manoeuvre { Manoeuvre::KeepAt(r) => Some(r), _ => None }));
    let approach = match a.following.map(|f| f.manoeuvre) {
        Some(Manoeuvre::Surface(_)) => Lamp::On,
        _ if a.rock_lock.is_some() && flying && !ship.hyperdrive => Lamp::Off,
        _ => Lamp::Unavailable,
    };
    let orbit = follow_cell("ORBIT", a.following.and_then(|f| match f.manoeuvre { Manoeuvre::Orbit(r) => Some(r), _ => None }));
    let lock = if a.contact.is_some() || a.rock_lock.is_some() {
        Lamp::On
    } else if app.contacts.is_empty() {
        Lamp::Unavailable
    } else {
        Lamp::Off
    };
    // Y / H: the anchor (a rock in reach) and the excavator (anchored).
    let anchored = matches!(ship.state, ShipState::Anchored { .. });
    let in_reach = || crate::rocks::scan(app).is_some_and(|s| s.gap < crate::mining::rig(app).anchor_reach);
    let anchor_lamp = if anchored {
        Lamp::On
    } else if flying && !ship.hyperdrive && in_reach() {
        Lamp::Off
    } else {
        Lamp::Unavailable
    };
    let dig = if ship.excavator { Lamp::Busy } else if anchored { Lamp::Off } else { Lamp::Unavailable };
    // The active mode's instruments (the mode bar at the top picks it).
    let anchored = matches!(ship.state, ShipState::Anchored { .. });
    let collide = if app.collision.as_ref().is_some_and(|p| p.collision.is_some()) { Lamp::Hot } else { on(a.collision_warning) };
    let view = "CHASE".to_string();
    let hyper = if flying || ship.hyperdrive { on(ship.hyperdrive) } else { Lamp::Unavailable };
    let let_go = if a.following.is_some() { Lamp::Off } else { Lamp::Unavailable };
    use crate::keys::{key, Act};
    let c = |k: &str, l: &str, lamp: Lamp| (k.to_string(), l.to_string(), lamp);
    let b = |act: Act, l: &str, lamp: Lamp| (key(act), l.to_string(), lamp);
    let (mode, cells): (&str, Vec<(String, String, Lamp)>) = match &ship.state {
        ShipState::Destroyed { .. } => ("DESTROYED", vec![c("BKSP", "RESPAWN", Lamp::Off)]),
        ShipState::Transit { .. } => ("GATE TRANSIT", vec![c("TAB", &view, Lamp::Off)]),
        _ if active_mode(app) == ShipMode::Mining => (
            "MINING",
            vec![
                b(Act::Prospect, "PROSPECT", if crate::mining::pulsing(app) { Lamp::Busy } else if flying || anchored { Lamp::Off } else { Lamp::Unavailable }),
                b(Act::Lock, "LOCK", lock),
                b(Act::ZeroIn, "ZERO IN", approach),
                if anchored { b(Act::Anchor, "UNANCHOR", Lamp::On) } else { b(Act::Anchor, "ANCHOR", anchor_lamp) },
                b(Act::Excavate, if ship.excavator { "DIG OFF" } else { "DIG" }, dig),
                b(Act::Keep, &keep.0, keep.1),
                b(Act::Orbit, &orbit.0, orbit.1),
                b(Act::Cancel, "CANCEL", let_go),
            ],
        ),
        _ if active_mode(app) == ShipMode::Combat => (
            "COMBAT",
            vec![
                c("SPC", "GUN", if ship.weapons_hot() { Lamp::Hot } else { Lamp::Busy }),
                b(Act::Laser, "PULSE LASER", if ship.laser_overheated { Lamp::Unavailable } else if ship.weapons_hot() { Lamp::Hot } else { Lamp::Busy }),
                b(Act::Lock, "LOCK", lock),
                b(Act::Keep, &keep.0, keep.1),
                b(Act::Orbit, &orbit.0, orbit.1),
                b(Act::Cancel, "CANCEL", let_go),
            ]
            .into_iter()
            // (Each weapon's button only with it fitted.)
            .filter(|cell| (cell.1 != "GUN" || ship.spec().has(universe_sim::world::modules::Gear::Gun)) && (cell.1 != "PULSE LASER" || ship.spec().has(universe_sim::world::modules::Gear::Laser)))
            .collect(),
        ),
        ShipState::Landed { .. } => {
            // Set down: the pilot's own business — power, fuel, repairs. Only
            // what this place and this ship offer (greyed: offered, not now).
            let at = universe_sim::world::traffic::docked_at(&app.view.system, ship);
            let station = matches!(at, Some(universe_sim::world::Facility::Station(_)));
            let mut cells = vec![b(Act::Systems, if ship.powered { "POWER DOWN" } else { "POWER UP" }, if ship.powered { Lamp::On } else { Lamp::Off })];
            if at.is_some() {
                let room = ship.spec().fuel_capacity - ship.fuel;
                let label = if room < 1.0 { "TANK FULL".to_string() } else if room < 1000.0 { format!("FUEL UP {room:.0} KG") } else { format!("FUEL UP {:.1} T", room / 1000.0) };
                cells.push(b(Act::Refuel, &label, if room >= 1.0 { Lamp::Off } else { Lamp::Unavailable }));
            }
            if station {
                let label = if ship.hull >= 1.0 { "HULL SOUND".to_string() } else { format!("OVERHAUL {:.0}%", ship.hull * 100.0) };
                cells.push(b(Act::Repair, &label, if ship.hull < 1.0 { Lamp::Off } else { Lamp::Unavailable }));
            }
            cells.push(c("S+E", "LIFT OFF", if ship.powered { Lamp::Off } else { Lamp::Unavailable }));
            if !a.route.stops.is_empty() {
                cells.push(b(Act::Autopilot, "AUTOPILOT", on(a.route.active)));
            }
            cells.push(b(Act::Foot, "FOOT", Lamp::Off));
            // (The standards registry: the port's copy.)
            if at.is_some() {
                cells.push(b(Act::Standards, "STANDARDS", if app.standards.is_some() { Lamp::On } else { Lamp::Off }));
            }
            // (Passengers only with a cabin aboard.)
            if app.v.docked_market.is_some() && ship.spec().seats > 0 {
                cells.push(b(Act::Passengers, "PASSENGERS", if app.passengers.is_some() { Lamp::On } else { Lamp::Off }));
            }
            (if station { "DOCKED" } else { "LANDED" }, cells)
        }
        _ if ship.manual => (
            "NAV - MANUAL THRUSTERS",
            vec![b(Act::Manual, "THRUSTERS", Lamp::On), c("NUM", "FIRE A JET", Lamp::Off), c("W", "MAINS", Lamp::Off), c("F7", "THRUSTERS PANEL", if app.show_thrusters { Lamp::On } else { Lamp::Off })],
        ),
        _ if ship.hyperdrive => (
            "NAV - HYPERDRIVE",
            vec![b(Act::Hyperdrive, "HYPERDRIVE OFF", Lamp::On), b(Act::Autopilot, "AUTOPILOT", on(a.hyper_autopilot)), c("W S", "SPEED", Lamp::Off)],
        ),
        _ => (
            "NAV",
            {
                let mut cells = vec![b(Act::Clearance, "DOCKING", clearance.1), b(Act::Autopilot, "AUTOPILOT", on(auto))];
                // (Only with a hyperdrive fitted.)
                if ship.spec().has(universe_sim::world::modules::Gear::Hyperdrive) {
                    cells.push(b(Act::Hyperdrive, "HYPERDRIVE", hyper));
                }
                cells.extend([
                    b(Act::Lock, "LOCK", lock),
                    b(Act::Keep, &keep.0, keep.1),
                    b(Act::Orbit, &orbit.0, orbit.1),
                    b(Act::Cancel, "CANCEL", let_go),
                    b(Act::Proximity, "IMPACT", collide),
                    b(Act::Manual, "THRUSTERS", Lamp::Off),
                ]);
                cells
            },
        ),
    };
    draw_grid(frame, mode, &cells);
}

/// What the ship's instruments are set up for: navigation unless combat or
/// mining is chosen (anchored to a rock, it's mining).
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ShipMode {
    Nav,
    Combat,
    Mining,
}

pub fn active_mode(app: &App) -> ShipMode {
    if app.mining.on || matches!(app.ship.state, ShipState::Anchored { .. }) {
        ShipMode::Mining
    } else if app.ship.armed {
        ShipMode::Combat
    } else {
        ShipMode::Nav
    }
}

/// The mode bar, top left: the modes and screens to enter, the one on lit
/// (its key again goes back to navigation). Returns its height.
fn mode_bar(frame: &mut Frame, app: &App, at: Vec2) -> f32 {
    let m = active_mode(app);
    let lamp = |on: bool| if on { Lamp::On } else { Lamp::Off };
    let combat = if app.ship.weapons_hot() {
        Lamp::Hot
    } else if app.ship.armed {
        Lamp::Busy
    } else {
        Lamp::Off
    };
    use crate::keys::{key, Act};
    let cells: Vec<(String, String, Lamp)> = vec![
        (String::new(), "NAV".into(), lamp(m == ShipMode::Nav)),
        (key(Act::Combat), "COMBAT".into(), combat),
        (key(Act::Mining), "MINING".into(), lamp(m == ShipMode::Mining)),
        (key(Act::Map), "MAP".into(), lamp(app.nav_map.is_some() || app.galaxy_map.is_some())),
        (key(Act::Market), "MARKET".into(), lamp(app.market.is_some())),
        (key(Act::Cargo), "CARGO".into(), lamp(app.show_cargo)),
        (key(Act::Economy), "ECONOMY".into(), lamp(app.economy_panel.is_some())),
        ("F11".into(), "NEWS".into(), lamp(app.news_panel)),
        (key(Act::Shipyard), "SHIPYARD".into(), lamp(app.shipyard.is_some())),
        ("TAB".into(), (if app.mode == Mode::Observer { "WATCH" } else { "CHASE" }).into(), Lamp::Off),
        ("F7".into(), "THRUST".into(), lamp(app.show_thrusters)),
        ("F1".into(), "HELP".into(), lamp(app.show_help)),
        ("F3".into(), "DEBUG".into(), lamp(app.debug > 0)),
    ];
    // (Debug on, or observing: SHIFT+F3, for whoever's helping.)
    let mut cells = cells;
    if app.debug > 0 || app.observing {
        let label = match &app.recording {
            Some(r) => format!("REC {:.0} S", r.seconds()),
            None if app.observing => "OBSERVING".into(),
            None => "OBSERVE".into(),
        };
        let lamp = if app.recording.is_some() { Lamp::Hot } else if app.observing { Lamp::On } else { Lamp::Off };
        cells.push(("S+F3".into(), label, lamp));
    }
    // (A mode only with its gear: combat with a weapon fitted, mining with a rig.)
    use universe_sim::world::modules::Gear;
    let spec = app.ship.spec();
    let cells: Vec<(String, String, Lamp)> = cells
        .into_iter()
        .filter(|c| (c.1 != "COMBAT" || spec.has(Gear::Gun) || spec.has(Gear::Laser)) && (c.1 != "MINING" || spec.has(Gear::MiningRig)))
        .collect();
    // Six to a row, more rows as it grows.
    const PER_ROW: usize = 7;
    let cell = Vec2::new(76.0, 14.0);
    for (i, (key, label, lamp)) in cells.iter().enumerate() {
        let pos = at + Vec2::new((i % PER_ROW) as f32 * (cell.x + 2.0), (i / PER_ROW) as f32 * (cell.y + 2.0));
        draw_cell(frame, pos, cell, key, label, *lamp);
    }
    cells.len().div_ceil(PER_ROW) as f32 * (cell.y + 2.0)
}

pub(crate) fn draw_cell(frame: &mut Frame, pos: Vec2, cell: Vec2, key: &str, label: &str, lamp: Lamp) {
    let (edge, fill, text) = match lamp {
        Lamp::Off => (DIM, PANEL, HUD),
        Lamp::On => (HUD, HUD.scale(0.3), HUD),
        Lamp::Busy => (AMBER, AMBER.scale(0.3), AMBER),
        Lamp::Hot => (RED, RED.scale(0.35), RED),
        Lamp::Unavailable => (DIM.scale(0.5), PANEL, DIM.scale(0.7)),
    };
    frame.hud_rect(pos, cell, fill);
    frame.hud_box(pos, cell, edge);
    frame.text(pos + Vec2::new(3.0, 3.0), key, text.scale(0.8));
    let x = if key.is_empty() { 3.0 } else { 3.0 + 8.0 * key.len() as f32 + 5.0 };
    // The key's letter lit where it stands in the name.
    let at = (key.len() == 1).then(|| label.find(key)).flatten();
    match at {
        Some(i) => {
            let mut p = frame.text(pos + Vec2::new(x, 3.0), &label[..i], text);
            p = frame.text(p, &label[i..i + 1], HOT);
            frame.hud_line(Vec2::new(p.x - 8.0, pos.y + cell.y - 2.0), Vec2::new(p.x - 1.0, pos.y + cell.y - 2.0), HOT);
            frame.text(p, &label[i + 1..], text);
        }
        None => {
            frame.text(pos + Vec2::new(x, 3.0), label, text);
        }
    }
}

fn on_foot_cells() -> Vec<(String, String, Lamp)> {
    [("WASD", "WALK"), ("S+", "RUN"), ("SPC", "JUMP"), ("F", "USE"), ("BKSP", "RESPAWN"), ("F1", "HELP")].iter().map(|(k, l)| (k.to_string(), l.to_string(), Lamp::Off)).collect()
}

fn observer_cells() -> Vec<(String, String, Lamp)> {
    let studio = crate::keys::key(crate::keys::Act::Studio);
    [("TAB", "PILOT"), ("WHL", "ZOOM"), ("[ ]", "BODIES"), ("N B", "STARS"), ("H", "FOCUS SHIP"), ("T", "SETTLER"), (studio.as_str(), "STUDIO"), ("F1", "HELP")]
        .iter()
        .map(|(k, l)| (k.to_string(), l.to_string(), Lamp::Off))
        .collect()
}

/// The action grid, lower left: what the situation is, and each key's
/// cell (lit in use, amber busy, red hot, dim unavailable).
pub(crate) fn draw_grid(frame: &mut Frame, mode: &str, cells: &[(String, String, Lamp)]) {
    draw_panel(frame, &format!("{mode} - INSTRUMENTS"), cells, 3);
}

/// A panel of action cells (key, name, lamp), `cols` across, bottom left,
/// under `title`. Its height.
pub(crate) fn draw_panel(frame: &mut Frame, title: &str, cells: &[(String, String, Lamp)], cols: usize) -> f32 {
    let cols = cols.max(1);
    // As wide as the longest key and name need.
    let w = cells.iter().map(|(k, l, _)| 8.0 + text_size(k).x + text_size(l).x + 6.0).fold(96.0, f32::max);
    let cell = Vec2::new(w, 14.0);
    let rows = cells.len().div_ceil(cols) as f32;
    let size = frame.size();
    let at = Vec2::new(4.0, size.y - rows * (cell.y + 2.0) - 4.0);
    frame.text(at - Vec2::new(0.0, LINE), title, HUD);
    for (i, (key, label, lamp)) in cells.iter().enumerate() {
        let pos = at + Vec2::new((i % cols) as f32 * (cell.x + 2.0), (i / cols) as f32 * (cell.y + 2.0));
        draw_cell(frame, pos, cell, key, label, *lamp);
    }
    rows * (cell.y + 2.0) + LINE + 4.0
}

/// What's come to us as news, over the hypernet or within our own comm's
/// hearing: each kill reported, each outlet's digest; when we heard it.
pub(crate) struct NewsItem<'a> {
    pub heard: f64,
    pub time: f64,
    pub system: usize,
    pub what: News<'a>,
}

pub(crate) enum News<'a> {
    /// A kill: its line, and whether it was ours (we did it, or it was us).
    Kill(String, bool),
    Digest(&'a universe_sim::newsroom::Digest),
}

/// Everything heard, newest first.
pub(crate) fn news_items(app: &App) -> Vec<NewsItem<'_>> {
    let mut items: Vec<NewsItem> = app
        .v
        .kills
        .iter()
        .filter_map(|k| {
            let heard = app.news.heard(&universe_sim::news::Key::kill(k))?;
            let ours = k.killer == universe_sim::PLAYER || k.victim == universe_sim::PLAYER;
            let line = if k.weapon == "COLLISION" {
                format!("{} WRECKED IN A COLLISION WITH {}", k.victim_name, k.killer_name)
            } else {
                format!("{} DESTROYED {} - {}", k.killer_name, k.victim_name, k.weapon)
            };
            Some(NewsItem { heard, time: k.time, system: k.system, what: News::Kill(line, ours) })
        })
        .collect();
    if let Some(room) = &app.newsroom {
        items.extend(room.digests.iter().filter_map(|d| Some(NewsItem { heard: app.news.heard(&d.key())?, time: d.time, system: d.system, what: News::Digest(d) })));
    }
    items.sort_by(|a, b| b.heard.total_cmp(&a.heard));
    items
}

/// The newsfeed, right side: what's newly come to us (kills reported,
/// digests), newest on top, each for `NEWS_SHOWN` real seconds from when we
/// heard it: where it's from, and how long it was on its way.
fn news_feed(frame: &mut Frame, app: &App, top: f32) -> f32 {
    let now = app.v.time;
    let shown = NEWS_SHOWN * app.warp().max(1.0);
    let mut rows = Vec::new();
    for item in news_items(app).iter().filter(|i| now - i.heard < shown).take(4) {
        let fade = 1.0 - 0.6 * ((now - item.heard) / shown) as f32;
        let from = news_from(app, item.system, item.time, item.heard);
        match &item.what {
            News::Kill(line, ours) => rows.push((format!("{line}{from}"), if *ours { RED } else { AMBER }.scale(fade))),
            News::Digest(d) => {
                let c = Color::hex(0x60ffb0).scale(fade);
                rows.push((format!("NEWS - {}{from}", d.outlet), c));
                if let Some(h) = d.headlines.first() {
                    rows.push((format!("  {h}"), c.scale(0.85)));
                }
            }
        }
    }
    right_column(frame, top, &rows) + 6.0
}

/// Real seconds an item stays in the newsfeed.
const NEWS_SHOWN: f64 = 15.0;

/// Trades, right side below the kills: who bought or sold how many of what,
/// where, for how much, and their cargo and credits after. Those in the
/// system in view (and ours), each for `TRADE_SHOWN` real seconds.
fn trade_feed(frame: &mut Frame, app: &App, top: f32) -> f32 {
    let now = app.v.time;
    let shown = TRADE_SHOWN * app.warp().max(1.0);
    // (As heard over the hypernet; shown from then.)
    let mut recent: Vec<(&universe_sim::TradeRecord, f64)> = app.v.trade_log.iter().filter_map(|r| Some((r, app.news.heard(&universe_sim::news::Key::trade(r))?))).filter(|(_, heard)| now - heard < shown).collect();
    recent.sort_by(|a, b| a.1.total_cmp(&b.1));
    let mut rows = Vec::new();
    for (r, heard) in recent.iter().rev().take(6) {
        let age = ((now - heard) / shown) as f32;
        let after = format!("CARGO {:.1} T, {:.0} CR", r.cargo / 1000.0, r.credits);
        let text = match &r.deal {
            universe_sim::Deal::Bought => format!("{} BOUGHT {} {} FOR {:.0} CR - {after}", r.trader, r.units, r.item, r.amount),
            universe_sim::Deal::Sold => format!("{} SOLD {} {} FOR {:.0} CR - {after}", r.trader, r.units, r.item, r.amount),
            universe_sim::Deal::Heading { to, expect } => format!("{} HEADS FOR {to} - EXPECTS +{expect:.0} CR", r.trader),
            universe_sim::Deal::MovingOn { to } => format!("{} FINDS NOTHING HERE - MOVES ON TO {to}", r.trader),
        };
        let text = format!("{text}{}", news_from(app, r.system, r.time, *heard));
        let base = if r.trader == "YOU" { HUD } else { Color::hex(0x60c0ff) };
        let c = base.scale(1.0 - 0.7 * age.max(0.0));
        rows.push((text, c));
    }
    right_column(frame, top, &rows)
}

/// Where news is from, if not here, and how old it was when it came, if
/// it was a while on its way.
fn news_from(app: &App, system: usize, time: f64, heard: f64) -> String {
    let mut s = String::new();
    if system != app.v.ship_system {
        s += &format!(" - IN {}", universe_sim::names::star_name(app.charts.galaxy.stars[system].seed).to_uppercase());
    }
    if heard - time > 2.0 {
        s += &format!(" ({} AGO)", fmt::lag(heard - time));
    }
    s
}


/// Real seconds a trade stays in the feed.
const TRADE_SHOWN: f64 = 12.0;

/// Performance, top right: the frame, the world tick, the planner, where the
/// frame's time went and what it drew.
fn perf(frame: &mut Frame, app: &App, ctx: &Context, top: f32) -> f32 {
    let p = &ctx.perf;
    let ships = 1 + app.v.crafts.len();
    let k = |n: u32| if n >= 10_000 { format!("{:.0}K", n as f32 / 1000.0) } else if n >= 1000 { format!("{:.1}K", n as f32 / 1000.0) } else { n.to_string() };
    let mut lines = vec![
        (format!("{:.0} FPS  {:.1} MS", ctx.fps, p.frame_ms), DIM),
        (format!("SIM {:.2} MS  {ships} SHIPS  {:.1} US EACH", app.sim_ms, app.sim_ms * 1000.0 / ships as f32), DIM),
    ];
    let (apart, late, dropped) = app.v.pilots;
    lines.push((format!("PILOTS {}  LATE {late}  DROPPED {dropped}", if apart { "APART" } else { "LOCKSTEP" }), if dropped > 0 { AMBER } else { DIM }));
    if app.last_step.warp_limited {
        lines.push(("SIM CAN'T KEEP UP".into(), RED));
    }
    if app.plan.is_some() {
        let every = (app.plan_cost * 20.0).clamp(0.1, 1.0);
        lines.push((format!("PLAN {:.1} MS EVERY {every:.1} S", app.plan_cost * 1000.0), DIM));
    }
    if app.collision.is_some() {
        lines.push((format!("COLLIDE {:.1} MS EVERY 0.2 S", app.collision_cost * 1000.0), DIM));
    }
    lines.push((format!("UPD {:.1} DRAW {:.1} GPU {:.1} IDLE {:.1}", p.update_ms, p.draw_ms, p.render_ms, p.wait_ms), DIM));
    let worst = p.history.iter().copied().fold(0.0, f32::max);
    lines.push((format!("WORST {worst:.0} MS OF {}  HITCHES {}", p.history.len(), p.hitches), if p.hitches > 0 { AMBER } else { DIM }));
    lines.push((format!("{} LINES {} TRIS {} PTS", k(p.lines), k(p.triangles), k(p.points)), DIM));
    if !app.v.crafts.is_empty() {
        let here = app.v.crafts.iter().filter(|c| c.system == app.view.origin).count();
        let t = &app.v.traffic;
        lines.push((format!("TRAFFIC {} SHIPS, {here} HERE", app.v.crafts.len()), DIM));
        lines.push((format!("STOPS {} GATES {} CRASHES {} COLLISIONS {}", t.stops, t.transits, t.crashes, t.collision_losses), DIM));
        lines.push((format!("TRADES {} KILLS {} POSSES {}/{}", t.trades, t.shot_down, t.defences, t.aggressors_downed), DIM));
    }
    let bottom = right_column(frame, top, &lines);
    frame_graph(frame, p, bottom + 4.0)
}

/// The last frames' times as bars, against the right edge: green under
/// 1/60 s, amber under a hitch, red over (see `HITCH`); a line at 1/60 s.
fn frame_graph(frame: &mut Frame, p: &universe_engine::Perf, top: f32) -> f32 {
    let size = frame.size();
    let (w, h) = (universe_engine::HISTORY as f32, 48.0);
    let x0 = size.x - w - 6.0;
    frame.hud_rect(Vec2::new(x0 - 2.0, top), Vec2::new(w + 4.0, h + 4.0), SOFT_PANEL);
    // (Scale: 100 ms the full height; a stall past it clipped.)
    let full = 100.0;
    for (i, ms) in p.history.iter().enumerate() {
        let bar = (ms / full).min(1.0) * h;
        let c = if *ms > universe_engine::HITCH * 1000.0 { RED } else if *ms > 1000.0 / 59.0 { AMBER } else { Color::hex(0x60ff90) };
        frame.hud_rect(Vec2::new(x0 + i as f32, top + 2.0 + h - bar), Vec2::new(1.0, bar.max(1.0)), c);
    }
    let y60 = top + 2.0 + h - (1000.0 / 60.0) / full * h;
    frame.hud_rect(Vec2::new(x0, y60), Vec2::new(w, 1.0), DIM);
    top + h + 6.0
}

/// A column of lines against the right edge, on a soft backing; its bottom.
fn right_column(frame: &mut Frame, top: f32, lines: &[(String, Color)]) -> f32 {
    if lines.is_empty() {
        return top;
    }
    let size = frame.size();
    let w = lines.iter().map(|(t, _)| text_size(t).x).fold(0.0, f32::max);
    frame.hud_rect(Vec2::new(size.x - w - 10.0, top), Vec2::new(w + 8.0, lines.len() as f32 * LINE + 6.0), SOFT_PANEL);
    for (i, (text, c)) in lines.iter().enumerate() {
        frame.text(Vec2::new(size.x - text_size(text).x - 6.0, top + 3.0 + i as f32 * LINE), text, *c);
    }
    top + lines.len() as f32 * LINE + 6.0
}

/// The profiler's report (F3): every scope taking real time, as a tree, with
/// its mean and worst time per frame over the last couple of seconds.
/// The profiler's report, against the right edge under what's there (from
/// `top`), as many rows as fit; its bottom.
fn profile_panel(frame: &mut Frame, top: f32) -> f32 {
    let rows: Vec<universe_prof::Stat> = universe_prof::report().into_iter().filter(|s| s.mean_ms >= 0.02 || s.max_ms >= 1.0).collect();
    let fit = ((frame.size().y - top - 3.0 * LINE) / LINE).max(4.0) as usize;
    let mut text = String::from("PROFILE (F3 F3)            MEAN    MAX  CALLS\n");
    for st in rows.iter().take(fit.min(48)) {
        let depth = st.name.matches('/').count();
        let leaf = st.name.rsplit('/').next().unwrap_or(st.name).to_uppercase();
        let label: String = format!("{}{leaf}", " ".repeat(depth)).chars().take(22).collect();
        let calls = if st.calls >= 1.5 { format!("{:.0}", st.calls) } else { String::new() };
        text += &format!("{label:<22} {:>6.2} {:>6.2} {calls:>6}\n", st.mean_ms, st.max_ms);
    }
    let size = frame.size();
    let box_size = text_size(&text);
    let pos = Vec2::new(size.x - box_size.x - 8.0, top + 4.0).floor();
    frame.hud_rect(pos - 4.0, box_size + 8.0, Color([0.012, 0.018, 0.026, 0.85]));
    frame.hud_box(pos - 4.0, box_size + 8.0, DIM);
    frame.text(pos, &text, HUD);
    pos.y + box_size.y + 4.0
}

fn help(frame: &mut Frame) {
    // The bindings as a tree of scopes (see `keys`): two columns, scopes kept whole.
    let lines = crate::keys::tree();
    let rows: Vec<String> = lines.iter().map(|(d, l)| format!("{}{l}", "  ".repeat(*d))).collect();
    let half = lines.iter().enumerate().filter(|(_, (d, _))| *d == 1).map(|(i, _)| i).find(|&i| i >= rows.len() / 2).unwrap_or(rows.len());
    let (left, right) = rows.split_at(half);
    let (left, right) = (left.join("\n"), right.join("\n"));
    let footer = "THE KEY'S LETTER IS LIT IN THE ACTION'S NAME. F1 CLOSES.";
    let (a, b) = (text_size(&left), text_size(&right));
    let gap = 3.0 * GLYPH;
    let box_size = Vec2::new(a.x + gap + b.x, a.y.max(b.y) + 2.0 * LINE);
    let size = frame.size();
    let pos = ((size - box_size) / 2.0).floor();
    frame.hud_rect(pos - 6.0, box_size + 12.0, Color([0.012, 0.018, 0.026, 0.94]));
    frame.hud_box(pos - 6.0, box_size + 12.0, HUD);
    frame.text(pos, &left, HUD);
    frame.text(pos + Vec2::new(a.x + gap, 0.0), &right, HUD);
    frame.text(pos + Vec2::new(0.0, box_size.y - LINE), footer, DIM);
}
