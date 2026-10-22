//! The pilot's readouts: what the ship is doing, its approach, docking, landing and transit, the route and
//! queues, the phase banner and what to do next.

use super::*;

pub(super) fn pilot_info(app: &App, lines: &mut Vec<(String, Color)>, alerts: &mut Vec<(String, Color)>) {
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
pub(super) fn prospect_info(app: &App, lines: &mut Vec<(String, Color)>) {
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
pub(super) fn cargo_panel(frame: &mut Frame, app: &App) {
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
    frame.hud_rect(pos - 8.0, Vec2::new(width, lines.len() as f32 * LINE) + 16.0, crate::palette::panel(0.9));
    frame.hud_box(pos - 8.0, Vec2::new(width, lines.len() as f32 * LINE) + 16.0, HUD.scale(0.6));
    for (k, (text, c)) in lines.iter().enumerate() {
        frame.text(pos + Vec2::new(0.0, k as f32 * LINE), text, *c);
    }
}

/// The collision warning: what the path hits and when.
pub(super) fn collision_info(app: &App, lines: &mut Vec<(String, Color)>) {
    let Some(c) = app.collision.as_ref().filter(|_| impact_shown(app)).and_then(|p| p.collision.as_ref()) else { return };
    let left = (c.time - (app.v.time - app.collision_at)).max(0.0) / app.warp().max(1.0);
    lines.push((format!("COLLISION {} IN {}  AT {}", c.what.to_uppercase(), fmt::countdown(left), fmt::speed(c.speed)), RED));
}

/// The radar: how many ships it sees, and the locked one's range, closing
/// speed and what its transponder says.
pub(super) fn radar_info(app: &App, lines: &mut Vec<(String, Color)>) {
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
pub(super) fn route_info(app: &App, lines: &mut Vec<(String, Color)>) {
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
            universe_sim::NavTarget::Asteroid(_) | universe_sim::NavTarget::Rig(_) => "UNDERWAY".into(),
        }
    } else {
        "UNDERWAY".into()
    };
    lines.push((format!("ROUTE {}/{} -> {name}  AUTO: {stage}", n + 1, r.stops.len()), AMBER));
}

/// Clearance, guidance numbers and hints for docking or landing.
pub(super) fn approach_info(app: &App, lines: &mut Vec<(String, Color)>) {
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
pub(super) fn queue_info(app: &App, lines: &mut Vec<(String, Color)>) {
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

pub(super) fn mode_label(autopilot: bool, phase: universe_sim::Phase) -> String {
    if autopilot { format!("AUTO {}", phase.label()) } else { "MANUAL  K=AUTO".into() }
}

/// What it takes to lift off: powering up first, if the ship's parked.
pub(super) fn lift_off_hint(ship: &universe_sim::world::Ship) -> String {
    if ship.powered { "SHIFT+E TO LIFT OFF".into() } else { format!("FLIGHT SYSTEMS DOWN - {} TO POWER UP", crate::keys::key(crate::keys::Act::Systems)) }
}

pub(super) fn docking_info(app: &App, station: usize, st: &DockingStatus, lines: &mut Vec<(String, Color)>) {
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

pub(super) fn transit_info(app: &App, gate: usize, st: &universe_sim::GateStatus, lines: &mut Vec<(String, Color)>) {
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

pub(super) fn landing_info(app: &App, port: usize, st: &LandingStatus, lines: &mut Vec<(String, Color)>) {
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
    let g = sys.bodies[p.body].surface_gravity();
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

pub(super) fn phase_banner(frame: &mut Frame, app: &App) {
    use universe_sim::Phase;
    let Some(approach) = &app.approach else { return };
    let plan = app.plan.current.as_ref();
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
        Some(p) if p.arrives => format!("ETA {}", fmt::countdown(app.plan.eta_shown.unwrap_or(0.0))),
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
pub(super) fn action_lines(app: &App, relative_velocity: DVec3, g: &Guidance, lines: &mut Vec<(String, Color)>) {
    let Some(plan) = &app.plan.current else { return };
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
pub(super) fn thrust_hint(app: &App, relative_velocity: DVec3, g: &Guidance) -> String {
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
