//! The action grid and mode bar: the keys that apply now, as lamps.

use super::*;

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
pub(super) fn action_grid(frame: &mut Frame, app: &App) {
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
                cells.push(b(Act::Standards, "STANDARDS", if app.panels.standards.is_some() { Lamp::On } else { Lamp::Off }));
            }
            // (Passengers only with a cabin aboard.)
            if app.v.docked_market.is_some() && ship.spec().seats > 0 {
                cells.push(b(Act::Passengers, "PASSENGERS", if app.passengers.is_some() { Lamp::On } else { Lamp::Off }));
            }
            (if station { "DOCKED" } else { "LANDED" }, cells)
        }
        _ if ship.manual => (
            "NAV - MANUAL THRUSTERS",
            crate::flight_input::keys(ship)
                .into_iter()
                .enumerate()
                .map(|(k, (key, label))| c(&key, label, if k == 0 { Lamp::On } else { Lamp::Off }))
                .chain([c("F7", "THRUSTERS PANEL", if app.show_thrusters { Lamp::On } else { Lamp::Off })])
                .collect(),
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
pub(super) fn mode_bar(frame: &mut Frame, app: &App, at: Vec2) -> f32 {
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
        (key(Act::Map), "MAP".into(), lamp(app.panels.nav_map.is_some() || app.panels.galaxy_map.is_some())),
        (key(Act::Market), "MARKET".into(), lamp(app.panels.market.is_some())),
        (key(Act::Cargo), "CARGO".into(), lamp(app.show_cargo)),
        (key(Act::Economy), "ECONOMY".into(), lamp(app.panels.economy_panel.is_some())),
        ("F11".into(), "NEWS".into(), lamp(app.news_panel)),
        (key(Act::Shipyard), "SHIPYARD".into(), lamp(app.panels.shipyard.is_some())),
        ("TAB".into(), (if app.mode == Mode::Observer { "WATCH" } else { "CHASE" }).into(), Lamp::Off),
        ("F7".into(), "THRUST".into(), lamp(app.show_thrusters)),
        ("F1".into(), "HELP".into(), lamp(app.show_help)),
        ("F3".into(), "DEBUG".into(), lamp(app.debug > 0)),
    ];
    // (Debug on, or observing: SHIFT+F3, for whoever's helping.)
    let mut cells = cells;
    if app.debug > 0 || app.observing.on {
        let label = match &app.recording {
            Some(r) => format!("REC {:.0} S", r.seconds()),
            None if app.observing.on => "OBSERVING".into(),
            None => "OBSERVE".into(),
        };
        let lamp = if app.recording.is_some() { Lamp::Hot } else if app.observing.on { Lamp::On } else { Lamp::Off };
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

pub(super) fn on_foot_cells() -> Vec<(String, String, Lamp)> {
    [("WASD", "WALK"), ("S+", "RUN"), ("SPC", "JUMP"), ("F", "USE"), ("BKSP", "RESPAWN"), ("F1", "HELP")].iter().map(|(k, l)| (k.to_string(), l.to_string(), Lamp::Off)).collect()
}

pub(super) fn observer_cells() -> Vec<(String, String, Lamp)> {
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
