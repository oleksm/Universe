use universe_engine::glam::{DVec3, Vec2, Vec3Swizzles};
use universe_engine::{text_size, Color, Context, Frame, GLYPH};
use universe_sim::world::radar::RADAR_RANGE;
use universe_sim::world::weapons::{gun_on, within_gimbal, GIMBAL_LIMIT};
use universe_sim::world::station::{MAX_DOCK_SPEED, MAX_ROLL_ERROR, STATION_SIZE};
use universe_sim::{Action, Approach, BodyKind, DockingStatus, Guidance, LandingStatus, ShipState};

use crate::observer::Focus;
use crate::scene::color;
use crate::{fmt, App, Mode};

const HUD: Color = Color::hex(0x30ff60);
const DIM: Color = Color::hex(0x178a38);
const AMBER: Color = Color::hex(0xffc040);
const RED: Color = Color::hex(0xff4040);
/// Colors shared with the 3D guidance: predicted path (cyan) and guidance path (magenta).
const PREDICT: Color = Color::hex(0x40c0ff);
const GUIDE_PATH: Color = Color::hex(0xff60ff);
const PANEL: Color = Color([0.0, 0.03, 0.01, 0.85]);
const LINE: f32 = GLYPH + 2.0;

pub fn draw(frame: &mut Frame, app: &App, ctx: &Context) {
    // The map covers the screen. (HUD fills are drawn before HUD lines, so
    // anything else drawn now would show through it.)
    if let Some(map) = &app.nav_map {
        crate::navmap::draw(frame, app, map);
        return;
    }
    if let Some(m) = &app.market {
        crate::market::draw(frame, app, m);
        return;
    }
    let mut lines: Vec<(String, Color)> = Vec::new();
    status(app, &mut lines);
    match app.mode {
        Mode::Observer => observer_info(app, &mut lines),
        Mode::Pilot if !app.u.crew.seated() => crate::onfoot::hud(frame, app, &mut lines, app.reach),
        Mode::Pilot => {
            pilot_info(app, &mut lines);
            approach_info(app, &mut lines);
            pilot_overlay(frame, app);
            target_marker(frame, app);
            contact_marker(frame, app);
            impact_label(frame, app);
            phase_banner(frame, app);
            scanner(frame, app);
        }
    }
    // Leave room for the phase banner across the top while docking/landing.
    let top = if app.mode == Mode::Pilot && app.approach.is_some() { 22.0 } else { 4.0 };
    let mut y = top;
    if app.mode == Mode::Pilot && app.u.crew.seated() {
        y += action_grid(frame, app, Vec2::new(4.0, top)) + 4.0;
    }
    for (text, c) in &lines {
        frame.text(Vec2::new(4.0, y), text, *c);
        y += LINE;
    }

    let size = frame.size();
    perf(frame, app, ctx, top);
    kill_feed(frame, app, top + 8.0 * LINE);
    trade_feed(frame, app, top + 15.0 * LINE);
    frame.text(Vec2::new(size.x - 7.0 * GLYPH - 4.0, size.y - GLYPH - 4.0), "F1 HELP", DIM);

    // Messages go below the status block so they never overlap it.
    let mut y = (top + lines.len() as f32 * LINE + 8.0).max(size.y * 0.3);
    for m in &app.messages {
        let c = if m.ttl < 1.0 { AMBER.scale(m.ttl) } else { AMBER };
        for line in m.text.lines() {
            let w = text_size(line).x;
            frame.text(Vec2::new((size.x - w) / 2.0, y), line, c);
            y += LINE;
        }
    }

    if let ShipState::Transit { to, remaining, .. } = &app.u.ship.state {
        let name = universe_sim::names::star_name(app.u.world.galaxy.stars[*to].seed).to_uppercase();
        let text = format!("GATE TRANSIT TO {name} - ARRIVING IN {remaining:.1} S");
        frame.text_boxed(((size - text_size(&text)) / 2.0).floor(), &text, AMBER, PANEL);
    }
    if app.show_help {
        help(frame);
    }
}

fn status(app: &App, lines: &mut Vec<(String, Color)>) {
    let ship = &app.u.ship;
    let mode = match app.mode {
        Mode::Observer => "OBSERVER",
        Mode::Pilot if ship.armed => "COMBAT",
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
    lines.push((format!("{mode}  {}  {warp}", fmt::clock(app.u.world.time)), top));
    let sys = &app.view.system;
    let home = if app.view.origin == app.u.world.home_system { "  HOME" } else { "" };
    lines.push((
        format!("SYSTEM {} ({}) {} PLANETS{home}", sys.name.to_uppercase(), sys.class.letter(), sys.planet_count()),
        DIM,
    ));
    if !app.u.crafts.is_empty() {
        let here = app.u.crafts.iter().filter(|c| c.system == app.view.origin).count();
        let t = &app.u.traffic;
        lines.push((
            format!("TRAFFIC {} SHIPS, {here} HERE  STOPS {} GATES {} CRASHES {}  TRADES {}  KILLS {}", app.u.crafts.len(), t.stops, t.transits, t.crashes, t.trades, t.shot_down),
            DIM,
        ));
    }
}

fn observer_info(app: &App, lines: &mut Vec<(String, Color)>) {
    let cam = app.camera.position;
    match app.observer.focus {
        Focus::Ship => {
            lines.push(("FOCUS COBRA MK III".into(), HUD));
            ship_readout(app, lines);
        }
        Focus::Craft(i) => {
            let Some(c) = app.u.crafts.get(i) else { return };
            let r = &c.avionics.route;
            let stage = match &c.ship.state {
                ShipState::Landed { .. } if r.dwell_until.is_some() => "AT A STOP",
                ShipState::Landed { .. } => "DEPARTING",
                ShipState::Transit { .. } => "GATE TRANSIT",
                ShipState::Destroyed { .. } => "DESTROYED",
                ShipState::Flying if c.ship.hyperdrive => "HYPERDRIVE",
                ShipState::Flying if r.departing => "CLIMBING",
                ShipState::Flying => match c.avionics.clearance.map(|x| x.target) {
                    Some(universe_sim::NavTarget::Station(_)) => "DOCKING",
                    Some(universe_sim::NavTarget::Spaceport(_)) => "LANDING",
                    Some(universe_sim::NavTarget::Gate(_)) => "GATE RUN",
                    None => "UNDERWAY",
                },
            };
            lines.push((format!("FOCUS {}  ({}/{})  T NEXT", c.name.to_uppercase(), i + 1, app.u.crafts.len()), HUD));
            lines.push((format!("ROUTE STOP {}/{}  {stage}", r.next + 1, r.stops.len()), DIM));
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
            if body == 0 && app.view.origin != app.u.ship_system {
                let ly = app.u.distance_ly(app.u.ship_system, app.view.origin);
                lines.push((format!("{ly:.1} LY FROM SHIP"), DIM));
            }
        }
    }
}

fn ship_readout(app: &App, lines: &mut Vec<(String, Color)>) {
    let ship = &app.u.ship;
    if let Some(r) = app.view.reference {
        let b = &app.view.system.bodies[r];
        let offset = app.view.ship_pos - app.view.positions[r];
        // Height above the ground actually under us (terrain, or sea level).
        let altitude = offset.length() - b.surface_radius_at(app.view.positions[r], app.view.ship_pos, app.u.world.time);
        let mut rel_vel = ship.velocity - app.view.system.velocity(r, app.u.world.time);
        // Close to the ground, speed relative to the rotating surface is what matters.
        let surface = altitude < 0.05 * b.rail.radius;
        if surface {
            rel_vel -= b.angular_velocity().cross(offset);
        }
        let vertical = rel_vel.dot(offset.normalize());
        lines.push((format!("NEAR {}  ALT {}", b.name.to_uppercase(), fmt::distance(altitude)), DIM));
        let label = if surface { "SRF SPD" } else { "SPD" };
        lines.push((format!("{label} {}  VSPD {}", fmt::speed(rel_vel.length()), fmt::speed(vertical)), DIM));
    }
}

fn pilot_info(app: &App, lines: &mut Vec<(String, Color)>) {
    route_info(app, lines);
    ship_readout(app, lines);
    radar_info(app, lines);
    collision_info(app, lines);
    let ship = &app.u.ship;
    let bar: String = (0..10).map(|i| if (i as f64) < ship.throttle * 10.0 - 0.01 { '#' } else { '.' }).collect();
    lines.push((format!("THR [{bar}] {:3.0}%", ship.throttle * 100.0), if ship.hyperdrive { AMBER } else { HUD }));
    lines.push((
        format!(
            "MASS {:.1} T  FUEL {:.1} T  CARGO {:.1} T  MAX ACC {:.1} M/S2  {:.0} CR",
            ship.mass() / 1000.0,
            ship.fuel / 1000.0,
            ship.cargo / 1000.0,
            ship.main_accel(),
            app.u.credits
        ),
        DIM,
    ));
    let gauge = |x: f64| -> String { (0..10).map(|i| if (i as f64) < x * 10.0 - 0.01 { '#' } else { '.' }).collect() };
    let hurt = app.hit_age < 0.25 || ship.hull < 0.3;
    if ship.armed {
        let heat = if ship.laser_overheated { " HOT".to_string() } else { String::new() };
        let c = if hurt { RED } else { AMBER };
        lines.push((format!("HULL [{}] {:3.0}%  GUN {}  LASER [{}]{heat}", gauge(ship.hull), ship.hull * 100.0, ship.ammo, gauge(ship.laser_heat)), c));
        if !ship.weapons_hot() {
            lines.push((format!("WEAPONS PRIMING  {:.1} S", ship.arming), AMBER));
        }
    } else {
        let c = if hurt { RED } else { DIM };
        lines.push((format!("HULL [{}] {:3.0}%", gauge(ship.hull), ship.hull * 100.0), c));
    }
    match &ship.state {
        ShipState::Landed { body, local_position, .. } => {
            let b = &app.view.system.bodies[*body];
            let port = app.view.system.port_at(*body, local_position.normalize()).map(|p| &app.view.system.spaceports[p]);
            if let Some(p) = port {
                lines.push((format!("LANDED AT {} ({})", p.name.to_uppercase(), b.name.to_uppercase()), AMBER));
                lines.push(("SHIFT+E TO LIFT OFF".into(), DIM));
            } else if b.kind == BodyKind::Station {
                lines.push((format!("DOCKED AT {}", b.name.to_uppercase()), AMBER));
                lines.push(("W TO LAUNCH".into(), DIM));
            } else {
                lines.push((format!("LANDED ON {}", b.name.to_uppercase()), AMBER));
                lines.push(("SHIFT+E TO LIFT OFF".into(), DIM));
            }
        }
        ShipState::Destroyed { respawn_in } => lines.push((format!("DESTROYED  RESPAWN IN {respawn_in:.0}"), RED)),
        ShipState::Transit { to, remaining, .. } => {
            let name = universe_sim::names::star_name(app.u.world.galaxy.stars[*to].seed).to_uppercase();
            lines.push((format!("GATE TRANSIT TO {name}  {remaining:.1} S"), AMBER));
        }
        ShipState::Flying => {}
    }
}

/// The collision warning: what the path hits and when, or how far it's clear.
fn collision_info(app: &App, lines: &mut Vec<(String, Color)>) {
    let Some(p) = &app.collision else { return };
    match &p.collision {
        Some(c) => {
            let left = (c.time - (app.u.world.time - app.collision_at)).max(0.0) / app.warp().max(1.0);
            lines.push((format!("COLLISION {} IN {}  AT {}", c.what.to_uppercase(), fmt::countdown(left), fmt::speed(c.speed)), RED));
        }
        None if p.clear => lines.push((format!("PATH CLEAR {}", fmt::distance(universe_sim::avionics::collision::RANGE)), DIM)),
        None => lines.push((format!("PATH CLEAR {} (LOOKED THAT FAR)", fmt::distance(p.reach)), DIM)),
    }
}

/// The radar: how many ships it sees, and the locked one's range, closing
/// speed and what its transponder says.
fn radar_info(app: &App, lines: &mut Vec<(String, Color)>) {
    if !app.u.ship.is_flying() && app.contacts.is_empty() {
        return;
    }
    let Some(c) = app.u.locked_contact_in(&app.contacts) else {
        let n = app.contacts.len();
        if n > 0 {
            let s = if n == 1 { "" } else { "S" };
            lines.push((format!("RADAR {n} CONTACT{s} IN {}", fmt::distance(RADAR_RANGE)), DIM));
        }
        return;
    };
    let ship = &app.u.ship;
    let closing = c.blip.closing_speed(ship.position, ship.velocity);
    let trend = if closing >= 0.0 { "CLOSING" } else { "OPENING" };
    lines.push((format!("LOCK {}  {}  {trend} {}", c.name, fmt::distance(c.blip.distance), fmt::speed(closing.abs())), crate::scene::TRAFFIC));
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
    if !app.u.ship.armed {
        return;
    }
    match &app.fire {
        Some((track, _)) if !track.ready() => lines.push((format!("     FIRE CONTROL: TRACKING {:3.0}%", track.quality() * 100.0), AMBER)),
        Some((_, Some(sol))) => {
            let ship = &app.u.ship;
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
    let r = &app.u.avionics.route;
    if r.stops.is_empty() {
        return;
    }
    let n = r.next.min(r.stops.len() - 1);
    let name = app.route_labels.get(n).cloned().unwrap_or_default();
    if !r.active {
        lines.push((format!("ROUTE {}/{} -> {name}", n + 1, r.stops.len()), DIM));
        return;
    }
    let ship = &app.u.ship;
    let stage = if let Some(until) = r.dwell_until {
        format!("AT STOP - LEAVING IN {}", fmt::countdown((until - app.u.world.time) / app.warp().max(1.0)))
    } else if r.departing {
        "CLIMBING".into()
    } else if matches!(ship.state, ShipState::Transit { .. }) {
        "GATE TRANSIT".into()
    } else if ship.hyperdrive {
        "HYPERDRIVE".into()
    } else if let Some(c) = app.u.avionics.clearance {
        match c.target {
            universe_sim::NavTarget::Station(_) => "DOCKING".into(),
            universe_sim::NavTarget::Spaceport(_) => "LANDING".into(),
            universe_sim::NavTarget::Gate(_) => "GATE RUN".into(),
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
            if app.u.ship.is_flying()
                && app.u.avionics.nav_target.is_some()
                && let Some((name, _)) = &app.nav_marker
            {
                lines.push((format!("NAV {name}"), DIM));
            }
        }
        Some(Approach::Dock { station, status }) => docking_info(app, *station, status, lines),
        Some(Approach::Land { port, status }) => landing_info(app, *port, status, lines),
        Some(Approach::Transit { gate, status }) => transit_info(app, *gate, status, lines),
    }
}

fn mode_label(autopilot: bool, phase: universe_sim::Phase) -> String {
    if autopilot { format!("AUTO {}", phase.label()) } else { "MANUAL  K=AUTO".into() }
}

fn docking_info(app: &App, station: usize, st: &DockingStatus, lines: &mut Vec<(String, Color)>) {
    let name = app.view.system.bodies[station].name.to_uppercase();
    lines.push((format!("DOCK {name}  {}", mode_label(st.autopilot, st.phase)), HUD));

    // The speed limit only applies in the corridor; elsewhere show the guidance speed.
    let in_final = st.guidance.final_run;
    let too_fast = in_final && (st.closing > st.speed_limit * 1.1 || (st.height < STATION_SIZE + 300.0 && st.speed > MAX_DOCK_SPEED));
    let target = if in_final {
        format!("LIMIT {}", fmt::speed(st.speed_limit))
    } else {
        format!("GO {}", fmt::speed(st.guidance.desired_velocity.length()))
    };
    lines.push((
        format!("RANGE {}  CLOSING {}  {target}", fmt::distance(st.range), fmt::speed(st.closing)),
        if too_fast { RED } else { HUD },
    ));
    let roll_deg = st.roll_error.to_degrees();
    let roll_bad = st.roll_error > MAX_ROLL_ERROR;
    lines.push((
        format!("AXIS OFFSET {}  ROLL {roll_deg:.0} DEG", fmt::distance(st.offset)),
        if st.in_corridor { HUD } else if roll_bad { RED } else { AMBER },
    ));
    lines.push((format!("REL SPEED {}", fmt::speed(st.speed)), PREDICT));
    action_lines(app, st.relative_velocity, &st.guidance, lines);
    if st.autopilot {
        return;
    }
    let hint = if st.height < 0.0 {
        "BEHIND THE STATION - GO AROUND"
    } else if !st.in_corridor && roll_bad {
        "ROLL TO LINE WINGS UP WITH THE SLOT"
    } else if !st.in_corridor {
        "FLY INTO THE GATES (SHIFT+WASDQE)"
    } else if too_fast {
        "SLOW DOWN"
    } else {
        "ON COURSE - GATES GREEN"
    };
    lines.push((hint.into(), DIM));
}

fn transit_info(app: &App, gate: usize, st: &universe_sim::GateStatus, lines: &mut Vec<(String, Color)>) {
    use universe_sim::gate::TRANSIT_SPEED;
    use universe_sim::world::gate::MAX_TRANSIT_SPEED;
    let name = app.view.system.bodies[gate].name.to_uppercase();
    lines.push((format!("TRANSIT {name}  {}", mode_label(st.autopilot, st.phase)), HUD));
    let too_fast = st.speed > MAX_TRANSIT_SPEED;
    let target = if st.guidance.final_run {
        format!("PASS AT {}, MAX {}", fmt::speed(TRANSIT_SPEED), fmt::speed(MAX_TRANSIT_SPEED))
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
    lines.push((format!("LAND {name}  {}", mode_label(st.autopilot, st.phase)), HUD));

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
            (&["APPROACH CORRIDOR", "ALIGN WITH SLOT", "FINAL RUN"], step)
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
        Some(_) => "ETA > 6 H".into(),
        None => String::new(),
    };

    let parts: Vec<String> = steps.iter().enumerate().map(|(i, s)| format!("{} {s}", i + 1)).collect();
    let sep = "  >  ";
    let full = format!("{}      {eta}", parts.join(sep));
    let size = frame.size();
    let mut x = ((size.x - text_size(&full).x) / 2.0).floor();
    let y = 4.0;
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
    let turn = (app.u.ship.orientation.inverse() * first.aim).normalize();
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
    let dv = app.u.ship.orientation.inverse() * dv_world;
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
    if matches!(app.u.ship.state, ShipState::Landed { .. }) {
        return;
    }
    let c = if app.approach.is_some() { HUD } else { Color::hex(0x60c0ff) };
    bracket(frame, app, name, *target, c);
}

/// Radar contacts in view: a small square on each ship, with its range when
/// near; the locked one gets a bracket like the nav target's.
fn contact_marker(frame: &mut Frame, app: &App) {
    let c = crate::scene::TRAFFIC;
    let size = frame.size();
    for contact in &app.contacts {
        if app.u.avionics.contact == Some(contact.blip.id) {
            continue;
        }
        let Some(p) = frame.project(contact.blip.position).filter(|p| p.x > 0.0 && p.y > 0.0 && p.x < size.x && p.y < size.y) else { continue };
        frame.hud_box(p - Vec2::splat(4.0), Vec2::splat(8.0), c.scale(0.8));
        if contact.blip.distance < 50_000.0 {
            let range = fmt::distance(contact.blip.distance);
            frame.text(p + Vec2::new(-text_size(&range).x / 2.0, 7.0), &range, c.scale(0.7));
        }
    }
    if let Some(locked) = app.u.locked_contact_in(&app.contacts) {
        bracket(frame, app, &locked.name, locked.blip.position, c);
        // Which way it's moving across our view: an arrow off its bracket.
        let v = locked.blip.velocity - app.u.ship.velocity;
        if v.length() > 0.5
            && let (Some(p), Some(q)) = (frame.project(locked.blip.position), frame.project(locked.blip.position + v * 2.0))
            && let Some(dir) = (q - p).try_normalize()
        {
            let (a, b) = (p + dir * 14.0, p + dir * 30.0);
            frame.hud_line(a, b, c);
            frame.hud_line(b, b - dir * 5.0 + dir.perp() * 3.0, c);
            frame.hud_line(b, b - dir * 5.0 - dir.perp() * 3.0, c);
        }
        // The lead (combat mode): fly it into the gimbal ring; fire control
        // lays the gun on it, and the circle doubles up when the gun is on.
        if app.u.ship.armed
            && let Some((_, Some(sol))) = &app.fire
            && let Some(p) = frame.project(app.view.ship_pos + sol.offset)
        {
            let ship = &app.u.ship;
            let c = if within_gimbal(ship, sol.aim) { AMBER } else { AMBER.scale(0.55) };
            frame.hud_ellipse(p, Vec2::splat(5.0), 12, c);
            if gun_on(ship, sol.aim) {
                frame.hud_ellipse(p, Vec2::splat(8.0), 16, c);
                frame.hud_ellipse(p, Vec2::splat(3.0), 8, c);
            }
            frame.hud_line(p - Vec2::new(2.0, 0.0), p + Vec2::new(2.0, 0.0), c);
            frame.hud_line(p - Vec2::new(0.0, 2.0), p + Vec2::new(0.0, 2.0), c);
            if let Some(q) = frame.project(locked.blip.position) {
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
    let ship = &app.u.ship;
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
    if let Some(locked) = app.u.locked_contact_in(&app.contacts)
        && app.sparks.iter().any(|s| s.ours && s.target == universe_sim::craft_id(locked.blip.id) && s.age < 0.6)
        && let Some(p) = frame.project(locked.blip.position)
    {
        frame.text(p + Vec2::new(12.0, -18.0), "HIT", RED);
    }
}

/// The nose crosshair is for fighting and for approaches (putting the nose
/// on the plan's cue); in plain travel it stays out of the way.
fn crosshair_wanted(app: &App) -> bool {
    app.u.ship.armed || app.approach.is_some()
}

/// The collision warning's impact, labelled on screen (or an arrow to it
/// from the edge).
fn impact_label(frame: &mut Frame, app: &App) {
    let Some(p) = &app.collision else { return };
    let Some(c) = &p.collision else { return };
    let at = app.view.positions[p.reference] + c.offset;
    let left = (c.time - (app.u.world.time - app.collision_at)).max(0.0) / app.warp().max(1.0);
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
        let ship = &app.u.ship;
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
            let path_len = app.plan.as_ref().map_or(0.0, crate::scene::path_length);
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
    let rel_vel = app.u.ship.velocity - app.view.system.velocity(r, app.u.world.time);
    if rel_vel.length() > 0.5 && !app.u.ship.hyperdrive {
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

/// Elite's 3D scanner: an ellipse seen in perspective, with height sticks.
/// Range is logarithmic from 1 km (center) to 10^12 m (rim).
fn scanner(frame: &mut Frame, app: &App) {
    let size = frame.size();
    let center = Vec2::new((size.x / 2.0).floor(), size.y - 58.0);
    let radii = Vec2::new(140.0, 42.0);
    frame.hud_rect(center - radii - Vec2::new(6.0, 16.0), radii * 2.0 + Vec2::new(12.0, 28.0), PANEL);
    frame.hud_ellipse(center, radii, 48, HUD);
    frame.hud_ellipse(center, radii * 0.5, 32, DIM);
    frame.hud_line(center - Vec2::new(radii.x, 0.0), center + Vec2::new(radii.x, 0.0), DIM);
    frame.hud_line(center - Vec2::new(0.0, radii.y), center + Vec2::new(0.0, radii.y), DIM);

    let inv = app.u.ship.orientation.inverse();
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
        frame.hud_line(base, top, c.scale(0.6));
        let dot = if b.kind == BodyKind::Star { 5.0 } else { 3.0 };
        frame.hud_rect(top - Vec2::splat(dot / 2.0).floor(), Vec2::splat(dot), c);
    }
    // Other ships the radar sees, as small cyan blips; the locked one boxed.
    let locked = app.u.avionics.contact;
    for contact in &app.contacts {
        let rel: DVec3 = inv * (contact.blip.position - app.view.ship_pos);
        let d = rel.length();
        if d < 1.0 {
            continue;
        }
        let r = ((d / 1.0e3).max(1.0).log10() / 9.0).min(1.0) as f32;
        let (x, y, z) = ((rel.x / d) as f32, (rel.y / d) as f32, (rel.z / d) as f32);
        let base = center + Vec2::new(x * radii.x * r, z * radii.y * r);
        let top = base - Vec2::new(0.0, y * 36.0 * r);
        frame.hud_line(base, top, crate::scene::TRAFFIC.scale(0.5));
        frame.hud_rect(top - Vec2::splat(1.0), Vec2::splat(2.0), crate::scene::TRAFFIC);
        if locked == Some(contact.blip.id) {
            frame.hud_box(top - Vec2::splat(4.0), Vec2::splat(8.0), crate::scene::TRAFFIC);
        }
    }
}

/// How an action grid cell is lit.
#[derive(Clone, Copy, PartialEq)]
enum Lamp {
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
fn action_grid(frame: &mut Frame, app: &App, at: Vec2) -> f32 {
    let (u, ship) = (&app.u, &app.u.ship);
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
    let arms = if ship.weapons_hot() {
        Lamp::Hot
    } else if ship.armed {
        Lamp::Busy
    } else {
        Lamp::Off
    };
    let lock = if a.contact.is_some() {
        Lamp::On
    } else if app.contacts.is_empty() {
        Lamp::Unavailable
    } else {
        Lamp::Off
    };
    let cells = [
        ("R", clearance.0, clearance.1),
        ("K", "AUTO", on(auto)),
        ("J", "HYPER", if flying || ship.hyperdrive { on(ship.hyperdrive) } else { Lamp::Unavailable }),
        ("B", "ARMS", arms),
        ("T", "LOCK", lock),
        ("I", "COLLIDE", if app.collision.as_ref().is_some_and(|p| p.collision.is_some()) { Lamp::Hot } else { on(a.collision_warning) }),
        ("M", "MAP", on(app.nav_map.is_some())),
        ("G", "MARKET", if app.docked_market { Lamp::On } else { Lamp::Off }),
        ("C", if app.chase_cam { "CHASE" } else { "COCKPIT" }, Lamp::Off),
        ("F1", "HELP", on(app.show_help)),
    ];
    const COLS: usize = 5;
    let cell = Vec2::new(84.0, 14.0);
    for (i, (key, label, lamp)) in cells.iter().enumerate() {
        let pos = at + Vec2::new((i % COLS) as f32 * (cell.x + 2.0), (i / COLS) as f32 * (cell.y + 2.0));
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
        frame.text(pos + Vec2::new(3.0 + 8.0 * key.len() as f32 + 5.0, 3.0), label, text);
    }
    let rows = cells.len().div_ceil(COLS) as f32;
    rows * (cell.y + 2.0)
}

/// Kills by weapons fire, right side: who destroyed whom, with what (the
/// last hit). Those in the system in view, and any involving us; each shows
/// for `KILL_SHOWN` real seconds.
fn kill_feed(frame: &mut Frame, app: &App, top: f32) {
    let now = app.u.world.time;
    let shown = KILL_SHOWN * app.warp().max(1.0);
    let lines: Vec<&universe_sim::Kill> = app
        .u
        .kills
        .iter()
        .filter(|k| now - k.time < shown)
        .filter(|k| k.system == app.view.origin || k.killer == universe_sim::PLAYER || k.victim == universe_sim::PLAYER)
        .rev()
        .take(6)
        .collect();
    let size = frame.size();
    for (i, k) in lines.iter().enumerate() {
        let age = ((now - k.time) / shown) as f32;
        let ours = k.killer == universe_sim::PLAYER || k.victim == universe_sim::PLAYER;
        let base = if ours { RED } else { AMBER };
        let text = format!("{} DESTROYED {} - {}", k.killer_name, k.victim_name, k.weapon);
        let c = base.scale(1.0 - 0.7 * age.max(0.0));
        frame.text(Vec2::new(size.x - text_size(&text).x - 4.0, top + i as f32 * LINE), &text, c);
    }
}

/// Trades, right side below the kills: who bought or sold how many of what,
/// where, for how much, and their cargo and credits after. Those in the
/// system in view (and ours), each for `TRADE_SHOWN` real seconds.
fn trade_feed(frame: &mut Frame, app: &App, top: f32) {
    let now = app.u.world.time;
    let shown = TRADE_SHOWN * app.warp().max(1.0);
    let recent: Vec<&universe_sim::TradeRecord> =
        app.u.trade_log.iter().filter(|r| now - r.time < shown && (r.system == app.view.origin || r.trader == "YOU")).rev().take(6).collect();
    let size = frame.size();
    for (i, r) in recent.iter().enumerate() {
        let age = ((now - r.time) / shown) as f32;
        let after = format!("CARGO {:.1} T, {:.0} CR", r.cargo / 1000.0, r.credits);
        let text = match &r.deal {
            universe_sim::Deal::Bought => format!("{} BOUGHT {} {} FOR {:.0} CR - {after}", r.trader, r.units, r.item, r.amount),
            universe_sim::Deal::Sold => format!("{} SOLD {} {} FOR {:.0} CR - {after}", r.trader, r.units, r.item, r.amount),
            universe_sim::Deal::Heading { to, expect } => format!("{} HEADS FOR {to} - EXPECTS +{expect:.0} CR", r.trader),
            universe_sim::Deal::MovingOn { to } => format!("{} FINDS NOTHING HERE - MOVES ON TO {to}", r.trader),
        };
        let base = if r.trader == "YOU" { HUD } else { Color::hex(0x60c0ff) };
        let c = base.scale(1.0 - 0.7 * age.max(0.0));
        frame.text(Vec2::new(size.x - text_size(&text).x - 4.0, top + i as f32 * LINE), &text, c);
    }
}

/// Real seconds a trade stays in the feed.
const TRADE_SHOWN: f64 = 12.0;

/// Real seconds a kill stays in the feed.
const KILL_SHOWN: f64 = 15.0;

/// Performance, top right: the frame, the world tick, the planner, where the
/// frame's time went and what it drew.
fn perf(frame: &mut Frame, app: &App, ctx: &Context, top: f32) {
    let p = &ctx.perf;
    let ships = 1 + app.u.crafts.len();
    let k = |n: u32| if n >= 10_000 { format!("{:.0}K", n as f32 / 1000.0) } else if n >= 1000 { format!("{:.1}K", n as f32 / 1000.0) } else { n.to_string() };
    let mut lines = vec![
        (format!("{:.0} FPS  {:.1} MS", ctx.fps, p.frame_ms), DIM),
        (format!("SIM {:.2} MS  {ships} SHIPS  {:.1} US EACH", app.sim_ms, app.sim_ms * 1000.0 / ships as f32), DIM),
    ];
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
    lines.push((format!("{} LINES {} TRIS {} PTS", k(p.lines), k(p.triangles), k(p.points)), DIM));
    let size = frame.size();
    for (i, (text, c)) in lines.iter().enumerate() {
        frame.text(Vec2::new(size.x - text_size(text).x - 4.0, top + i as f32 * LINE), text, *c);
    }
}

fn help(frame: &mut Frame) {
    let text = "\
GLOBAL
 TAB      OBSERVER / PILOT
 , .      TIME WARP DOWN / UP
 P        PAUSE
 O  L     ORBITS / LABELS
 M        NAVIGATION MAP
 F8       MUTE
 F5  F9   QUICKSAVE / LOAD
 F12      SCREENSHOT
OBSERVER
 DRAG     ROTATE (ARROWS TOO)
 WHEEL    ZOOM (W S TOO)
 [ ]      PREV / NEXT BODY
 N B      NEXT / PREV NEAR STAR
 H  HOME  FOCUS SHIP / HOME STAR
 T        FOLLOW NEXT SETTLER
PILOT
 CLICK    MOUSE FLIGHT (ESC FREE)
 W S      THROTTLE  (Z FULL X CUT)
 A D  Q E ROLL / YAW
 ARROWS   PITCH AND ROLL
 SHIFT+WASDQE  THRUSTERS (E = LIFT)
 J        HYPERDRIVE (DROPS OUT AT NAV TARGET)
 R        REQUEST DOCKING / LANDING
 K        AUTOPILOT: DOCK/LAND, OR IN
          HYPERDRIVE STEER TO TARGET
 T        LOCK THE SHIP IN THE BEAM RING (AGAIN: NEXT)
 I        COLLISION WARNING: PATH, IMPACT, TIME
 B        COMBAT MODE: ARM / SAFE WEAPONS
 SPACE    GUN (FLY THE LEAD INTO THE RING)
 V        LASER (WATCH THE HEAT)
 G        MARKET (THIS SYSTEM; TRADE WHEN DOCKED)
 C        COCKPIT / CHASE VIEW
 F        LEAVE / TAKE THE PILOT'S SEAT
ON FOOT
 WASD     WALK (SHIFT RUN, SPACE JUMP)
 MOUSE    LOOK (ARROWS TOO)
 F        USE: SEAT, HATCH, RAMP
 BKSP     RESPAWN AT HOME";
    let size = frame.size();
    let box_size = text_size(text);
    let pos = ((size - box_size) / 2.0).floor();
    frame.hud_rect(pos - 6.0, box_size + 12.0, Color([0.0, 0.02, 0.0, 0.92]));
    frame.hud_box(pos - 6.0, box_size + 12.0, HUD);
    frame.text(pos, text, HUD);
}
