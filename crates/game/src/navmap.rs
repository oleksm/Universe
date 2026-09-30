//! The navigation map: a schematic chart of the current system and a list of
//! places to dock or land. Pick one to lock it as the nav target.

use std::f32::consts::TAU;
use std::rc::Rc;

use universe_engine::glam::{DVec3, Vec2};
use universe_engine::{text_size, Color, Context, Frame, KeyCode, GLYPH};
use universe_sim::{BodyKind, NavTarget, StarSystem};

use crate::scene::color;
use crate::{fmt, App};

const TEXT: Color = Color::hex(0x30ff60);
const DIM: Color = Color::hex(0x178a38);
const SELECT: Color = Color::hex(0xffc040);
const LOCKED: Color = Color::hex(0xff60ff);

pub struct Entry {
    pub target: NavTarget,
    pub name: String,
    pub kind: &'static str,
    pub distance: f64,
}

pub struct NavMap {
    pub selected: usize,
    /// The system being browsed (galaxy index): the ship's, or another in the gate network.
    view: usize,
    /// Seed for the next settler route loaded with G.
    settler_seed: u64,
    entries: Vec<Entry>,
    system: Rc<StarSystem>,
    positions: Vec<DVec3>,
    /// The body the ship is near, if we're browsing the ship's own system.
    ship_body: Option<usize>,
}

/// Systems you can browse: the ship's first, then the gate network.
fn browsable(app: &App) -> Vec<usize> {
    let mut v = vec![app.u.ship_system];
    let mut net: Vec<usize> = app.u.world.gate_links.iter().flat_map(|&(a, b)| [a, b]).collect();
    net.sort();
    net.dedup();
    v.extend(net.into_iter().filter(|&s| s != app.u.ship_system));
    v
}

impl NavMap {
    pub fn open(app: &mut App) -> Self {
        let system = app.u.ship_system();
        let view = app.u.ship_system;
        let mut map = Self { selected: 0, view, settler_seed: 1, entries: Vec::new(), system, positions: Vec::new(), ship_body: None };
        map.refresh(app);
        // Start on the current target if there is one.
        if let Some(t) = app.u.avionics.nav_target {
            map.selected = map.entries.iter().position(|e| e.target == t).unwrap_or(0);
        }
        map
    }

    fn here(&self, app: &App) -> bool {
        self.view == app.u.ship_system
    }

    fn refresh(&mut self, app: &mut App) {
        self.system = app.u.system(self.view);
        self.system.positions(app.u.world.time, &mut self.positions);
        let here = self.here(app);
        let ship = app.u.ship.position;
        self.ship_body = here.then(|| self.system.dominant(ship, &self.positions));
        let mut entries = Vec::new();
        for (i, b) in self.system.bodies.iter().enumerate() {
            if b.kind == BodyKind::Station {
                entries.push((NavTarget::Station(i), b.name.clone(), "STATION"));
            }
        }
        for (i, b) in self.system.bodies.iter().enumerate() {
            if b.kind == BodyKind::Gate {
                entries.push((NavTarget::Gate(i), b.name.clone(), "GATE"));
            }
        }
        for (i, p) in self.system.spaceports.iter().enumerate() {
            let on = &self.system.bodies[p.body].name;
            entries.push((NavTarget::Spaceport(i), format!("{} ({on})", p.name), "SPACEPORT"));
        }
        let away = (!here).then(|| app.u.distance_ly(app.u.ship_system, self.view));
        self.entries = entries
            .into_iter()
            .map(|(target, name, kind)| {
                // Elsewhere, everything is "that many light years away".
                let distance = match away {
                    Some(_) => f64::INFINITY,
                    None => app.u.target_position(target).map_or(f64::INFINITY, |p| p.distance(ship)),
                };
                Entry { target, name: name.to_uppercase(), kind, distance }
            })
            .collect();
        if away.is_none() {
            self.entries.sort_by(|a, b| a.distance.total_cmp(&b.distance));
        }
        self.selected = self.selected.min(self.entries.len().saturating_sub(1));
    }
}

/// Handle map keys. Returns false when the map should close.
pub fn input(app: &mut App, ctx: &Context) -> bool {
    let Some(mut map) = app.nav_map.take() else { return false };
    let input = &ctx.input;
    // Browse systems.
    let systems = browsable(app);
    let i = systems.iter().position(|&s| s == map.view).unwrap_or(0);
    if input.pressed(KeyCode::ArrowRight) || input.pressed(KeyCode::ArrowLeft) {
        let step = if input.pressed(KeyCode::ArrowRight) { 1 } else { systems.len() - 1 };
        map.view = systems[(i + step) % systems.len()];
        map.selected = 0;
        crate::sound::click(ctx, 900.0);
    }
    map.refresh(app);
    let input = &ctx.input;
    let n = map.entries.len().max(1);
    if input.pressed(KeyCode::ArrowDown) || input.pressed(KeyCode::KeyS) {
        map.selected = (map.selected + 1) % n;
        crate::sound::click(ctx, 1200.0);
    }
    if input.pressed(KeyCode::ArrowUp) || input.pressed(KeyCode::KeyW) {
        map.selected = (map.selected + n - 1) % n;
        crate::sound::click(ctx, 1200.0);
    }
    if input.pressed(KeyCode::Delete) {
        app.u.set_nav_target(None);
    }
    // Route editing.
    if input.pressed(KeyCode::KeyA)
        && let Some(e) = map.entries.get(map.selected)
        && e.kind != "GATE"
    {
        app.u.avionics.route.stops.push(universe_sim::Stop { system: map.view, target: e.target });
        crate::sound::click(ctx, 1500.0);
    }
    if input.pressed(KeyCode::Backspace) && app.u.avionics.route.pop().is_some() {
        crate::sound::click(ctx, 700.0);
    }
    if input.pressed(KeyCode::KeyC) {
        app.u.avionics.route.clear();
        crate::sound::click(ctx, 500.0);
    }
    if input.pressed(KeyCode::KeyG) {
        // A reproducible settler route: each press loads the next seed.
        app.u.avionics.route.clear();
        app.u.avionics.route.stops = app.u.settler_route(map.settler_seed, 10);
        map.settler_seed += 1;
        crate::sound::chime(ctx);
    }
    let input = &ctx.input;
    if (input.pressed(KeyCode::Enter) || input.pressed(KeyCode::Space))
        && let Some(e) = map.entries.get(map.selected)
    {
        if map.here(app) {
            app.u.set_nav_target(Some(e.target));
            return false;
        }
        app.say("LOCK TARGETS IN THIS SYSTEM - OR ADD TO THE ROUTE (A)".into());
    }
    if input.pressed(KeyCode::KeyM) || input.pressed(KeyCode::Escape) {
        return false;
    }
    app.nav_map = Some(map);
    true
}

pub fn draw(frame: &mut Frame, app: &App, map: &NavMap) {
    let size = frame.size();
    frame.hud_rect(Vec2::ZERO, size, Color([0.0, 0.02, 0.01, 1.0]));
    let line = GLYPH + 4.0;

    // Target list.
    let mut y = 16.0;
    let systems = browsable(app);
    let k = systems.iter().position(|&s| s == map.view).unwrap_or(0) + 1;
    let whose = if map.here(app) { "  (YOU ARE HERE)".to_string() } else { format!("  {:.1} LY AWAY", app.u.distance_ly(app.u.ship_system, map.view)) };
    let title = format!("NAVIGATION - {} SYSTEM{whose}   < {k}/{} >", map.system.name.to_uppercase(), systems.len());
    frame.text(Vec2::new(16.0, y), &title, TEXT);
    y += line * 2.0;
    if map.entries.is_empty() {
        frame.text(Vec2::new(16.0, y), "NOTHING TO DOCK OR LAND AT HERE", DIM);
    }
    let route_has = |t: NavTarget| app.u.avionics.route.stops.iter().any(|s| s.system == map.view && s.target == t);
    for (i, e) in map.entries.iter().enumerate() {
        let locked = map.here(app) && app.u.avionics.nav_target == Some(e.target);
        let c = if i == map.selected { SELECT } else if locked { LOCKED } else { TEXT };
        let cursor = if i == map.selected { ">" } else { " " };
        let mark = if locked { "*" } else if route_has(e.target) { "+" } else { " " };
        let dist = if e.distance.is_finite() { fmt::distance(e.distance) } else { String::new() };
        let row = format!("{cursor}{mark} {:<34} {:<9} {:>10}", e.name, e.kind, dist);
        frame.text(Vec2::new(16.0, y), &row, c);
        y += line;
    }

    // The route.
    y += line;
    let r = &app.u.avionics.route;
    let state = if r.active { "FLYING - K TO STOP" } else if r.stops.is_empty() { "EMPTY" } else { "K IN FLIGHT TO START" };
    frame.text(Vec2::new(16.0, y), &format!("ROUTE ({} STOPS) - {state}", r.stops.len()), TEXT);
    y += line;
    for i in 0..r.stops.len() {
        let name = app.route_labels.get(i).cloned().unwrap_or_default();
        let c = if r.active && i == r.next { SELECT } else if i < r.next { DIM } else { TEXT };
        frame.text(Vec2::new(16.0, y), &format!("{:>2}. {name}", i + 1), c);
        y += line;
    }

    let help = "UP/DOWN SELECT  LEFT/RIGHT SYSTEM  ENTER LOCK TARGET  DEL CLEAR TARGET  M CLOSE\n\
                A ADD TO ROUTE  BKSP REMOVE LAST  C CLEAR ROUTE  G LOAD A SETTLER ROUTE";
    frame.text(Vec2::new(16.0, size.y - 2.0 * line - 8.0), help, DIM);

    chart(frame, map, Vec2::new(size.x * 0.76, size.y * 0.5), (size.x * 0.22).min(size.y * 0.42));
}

/// Top-down schematic: planets on evenly spaced rings at their true angles.
fn chart(frame: &mut Frame, map: &NavMap, center: Vec2, max_r: f32) {
    let sys = &map.system;
    let planets: Vec<usize> = (0..sys.bodies.len()).filter(|&i| sys.bodies[i].rail.parent == Some(0)).collect();
    let ring = max_r / planets.len().max(1) as f32;
    let angle_of = |v: DVec3| (v.z as f32).atan2(v.x as f32);
    let place = |i: usize| -> Vec2 {
        let k = planets.iter().position(|&p| p == i).unwrap_or(0) as f32 + 1.0;
        let a = angle_of(map.positions[i]);
        center + Vec2::new(a.cos(), a.sin()) * ring * k
    };
    // Where a body sits on the chart: planets on their ring, moons and stations beside their planet.
    let chart_pos = |i: usize| -> Vec2 {
        match sys.bodies[i].rail.parent {
            None => center,
            Some(0) => place(i),
            Some(p) => {
                let a = angle_of(map.positions[i] - map.positions[p]);
                place(p) + Vec2::new(a.cos(), a.sin()) * 14.0
            }
        }
    };

    frame.hud_rect(center - 3.0, Vec2::splat(7.0), color(sys.bodies[0].color));
    for (k, &p) in planets.iter().enumerate() {
        frame.hud_ellipse(center, Vec2::splat(ring * (k as f32 + 1.0)), 64, DIM.scale(0.6));
        let at = place(p);
        let b = &sys.bodies[p];
        let s = match b.kind {
            BodyKind::GasGiant | BodyKind::IceGiant => 7.0,
            _ => 5.0,
        };
        frame.hud_rect(at - (s / 2.0f32).floor(), Vec2::splat(s), color(b.color));
    }
    for (i, b) in sys.bodies.iter().enumerate() {
        if b.kind == BodyKind::Moon {
            frame.hud_rect(chart_pos(i) - 1.0, Vec2::splat(3.0), color(b.color).scale(0.8));
        }
    }

    // Targets: stations as squares, spaceports as triangles.
    for (i, e) in map.entries.iter().enumerate() {
        let (at, c) = match e.target {
            NavTarget::Station(b) => (chart_pos(b), Color::WHITE),
            NavTarget::Gate(b) => (chart_pos(b), Color::hex(0xffc040)),
            NavTarget::Spaceport(p) => {
                let body = sys.spaceports[p].body;
                (chart_pos(body) + Vec2::new(0.0, -8.0), Color::hex(0x60c0ff))
            }
        };
        match e.target {
            NavTarget::Station(_) => frame.hud_box(at + Vec2::new(8.0, -3.0), Vec2::splat(6.0), c),
            NavTarget::Gate(_) => frame.hud_ellipse(at, Vec2::splat(5.0), 10, c),
            NavTarget::Spaceport(_) => {
                frame.hud_line(at + Vec2::new(-3.0, 0.0), at + Vec2::new(3.0, 0.0), c);
                frame.hud_line(at + Vec2::new(-3.0, 0.0), at + Vec2::new(0.0, -5.0), c);
                frame.hud_line(at + Vec2::new(3.0, 0.0), at + Vec2::new(0.0, -5.0), c);
            }
        }
        if i == map.selected {
            let k = 12.0;
            for (sx, sy) in [(-1.0, -1.0), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)] {
                let corner = at + Vec2::new(sx * k, sy * k);
                frame.hud_line(corner, corner - Vec2::new(sx * 5.0, 0.0), SELECT);
                frame.hud_line(corner, corner - Vec2::new(0.0, sy * 5.0), SELECT);
            }
            let label = &e.name;
            frame.text(at + Vec2::new(-text_size(label).x / 2.0, 16.0), label, SELECT);
        }
    }

    // The ship, next to whatever it's orbiting (only in its own system).
    let Some(ship_body) = map.ship_body else { return };
    let you = chart_pos(ship_body) + Vec2::new(-10.0, 6.0);
    for i in 0..3 {
        let a = i as f32 / 3.0 * TAU - TAU / 4.0;
        let b = (i + 1) as f32 / 3.0 * TAU - TAU / 4.0;
        frame.hud_line(you + Vec2::new(a.cos(), a.sin()) * 5.0, you + Vec2::new(b.cos(), b.sin()) * 5.0, TEXT);
    }
    frame.text(you + Vec2::new(-28.0, -4.0), "YOU", TEXT);
}
