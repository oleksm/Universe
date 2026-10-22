use universe_engine::glam::{DVec3, Vec2, Vec3Swizzles};
use universe_engine::{text_size, Color, Context, Frame, GLYPH};
use universe_sim::world::weapons::{gun_on, within_gimbal, GIMBAL_LIMIT};
use universe_sim::world::station::DECK_SPEED;
use universe_sim::{Action, Approach, BodyKind, DockingStatus, Guidance, LandingStatus, ShipState};

use crate::observer::Focus;
use crate::scene::color;
use crate::{fmt, App, Mode};
use crate::palette::{AMBER, DIM, PANEL, RED, TEXT as HUD};

mod actions;
mod feeds;
mod markers;
mod pilot;
pub(crate) use actions::*;
pub(crate) use feeds::*;
pub(crate) use markers::*;
pub(crate) use pilot::*;

/// The key's letter, lit in an action's name.
const HOT: Color = Color::hex(0xffffa0);
/// Colors shared with the 3D guidance: predicted path (cyan) and guidance path (magenta).
const PREDICT: Color = Color::hex(0x40c0ff);
const GUIDE_PATH: Color = Color::hex(0xff60ff);
/// A lighter backing for text over the world: readable on a bright sky, the view still through it.
const SOFT_PANEL: Color = Color([0.01, 0.015, 0.022, 0.55]);
const LINE: f32 = GLYPH + 2.0;

pub fn draw(frame: &mut Frame, app: &App, ctx: &Context) {
    // The map covers the screen. (HUD fills are drawn before HUD lines, so
    // anything else drawn now would show through it.)
    if let Some(panel) = &app.panels.economy_panel {
        crate::economy::draw(frame, app, panel);
        return;
    }
    if app.news_panel {
        crate::newspanel::draw(frame, app);
        return;
    }
    if app.panels.standards.is_some() {
        crate::standards::draw(frame, app);
        return;
    }
    if let Some(map) = &app.panels.galaxy_map {
        crate::galaxymap::draw(frame, app, map);
        return;
    }
    if let Some(map) = &app.panels.nav_map {
        crate::navmap::draw(frame, app, map);
        return;
    }
    if let Some(m) = &app.panels.market {
        crate::market::draw(frame, app, m);
        return;
    }
    if let Some(pick) = app.passengers.filter(|_| app.mode == Mode::Pilot) {
        crate::passengers::draw(frame, app, pick);
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
    if let Some(body) = app.view.system.bodies.iter().find(|b| b.terrain.as_ref().is_some_and(|t| t.canonical_surface())) {
        let palette = crate::pgs_preview::palette(body);
        let label = if palette.is_some_and(|p| p.materials()) { "PGS1 ROCK PILOT: VISUAL TUNING" }
            else if palette.is_some() { "PGS1 DIAGNOSTIC CATEGORY COLOURS" }
            else { "PGS1 NEUTRAL TERRAIN / KNOWN WATER" };
        frame.text_boxed(Vec2::new(4.0, y), label, AMBER, SOFT_PANEL);
        y += LINE + 6.0;
    }
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
    let (bars, c, text) = match &app.net.status {
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
