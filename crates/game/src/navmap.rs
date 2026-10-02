//! The navigation map: a schematic chart of the current system and a list of
//! places to dock or land. Pick one to lock it as the nav target.

use std::f32::consts::TAU;
use std::sync::Arc;

use universe_engine::glam::{DVec3, Vec2};
use universe_engine::{text_size, Color, Context, Frame, KeyCode, GLYPH};
use universe_sim::{BodyKind, NavTarget, StarSystem};

use crate::scene::color;
use crate::{fmt, App};

const TEXT: Color = Color::hex(0xdcebf2);
const DIM: Color = Color::hex(0x7d93a0);
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
    system: Arc<StarSystem>,
    positions: Vec<DVec3>,
    /// The body the ship is near, if we're browsing the ship's own system.
    ship_body: Option<usize>,
    /// Places with defence turrets.
    defended: Vec<NavTarget>,
    /// Up or down held this long (s): the cursor repeats (see `repeat`).
    held: f32,
    /// Showing the hypernet: the chart to scale, its relays and their reach.
    pub network: bool,
}

/// Systems you can browse: the ship's first, then the gate network.
fn browsable(app: &App) -> Vec<usize> {
    let mut v = vec![app.v.ship_system];
    let mut net: Vec<usize> = app.charts.gate_links.iter().flat_map(|&(a, b)| [a, b]).collect();
    net.sort();
    net.dedup();
    v.extend(net.into_iter().filter(|&s| s != app.v.ship_system));
    v
}

impl NavMap {
    pub fn open(app: &mut App) -> Self {
        let system = app.charts.system(app.v.ship_system);
        let view = app.v.ship_system;
        let mut map = Self { selected: 0, view, settler_seed: 1, entries: Vec::new(), system, positions: Vec::new(), ship_body: None, defended: Vec::new(), held: 0.0, network: false };
        map.refresh(app);
        // Start on the current target if there is one.
        if let Some(t) = app.v.avionics.nav_target {
            map.selected = map.entries.iter().position(|e| e.target == t).unwrap_or(0);
        }
        map
    }

    fn here(&self, app: &App) -> bool {
        self.view == app.v.ship_system
    }

    fn refresh(&mut self, app: &mut App) {
        self.system = app.charts.system(self.view);
        self.system.positions(app.v.time, &mut self.positions);
        let here = self.here(app);
        let ship = app.ship.position;
        self.ship_body = here.then(|| self.system.dominant(ship, &self.positions));
        self.defended = universe_sim::world::turrets::turrets(app.charts.seed, self.view, &self.system).into_iter().map(|t| t.facility).collect();
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
        for f in &self.system.fields {
            let class = self.system.bodies[f.body].rock.as_ref().map_or("", |r| r.class.letter());
            entries.push((NavTarget::Asteroid(f.body), f.name.clone(), class));
        }
        let away = (!here).then(|| app.charts.distance_ly(app.v.ship_system, self.view));
        self.entries = entries
            .into_iter()
            .map(|(target, name, kind)| {
                // Elsewhere, everything is "that many light years away".
                let distance = match away {
                    Some(_) => f64::INFINITY,
                    None => target.position(&self.system, app.v.time, &self.positions).map_or(f64::INFINITY, |p| p.distance(ship)),
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

/// Held this long (s), a key starts repeating...
const REPEAT_AFTER: f32 = 0.35;
/// ...this often (s).
const REPEAT_EVERY: f32 = 0.065;

/// How many steps a held key makes this frame: one when pressed, then
/// (after `REPEAT_AFTER`) one every `REPEAT_EVERY`. `held` keeps the time.
pub fn repeat(held: &mut f32, down: bool, pressed: bool, dt: f32) -> u32 {
    if !down {
        *held = 0.0;
        return 0;
    }
    if pressed {
        *held = 0.0;
        return 1;
    }
    let before = ((*held - REPEAT_AFTER) / REPEAT_EVERY).floor().max(-1.0);
    *held += dt;
    let after = ((*held - REPEAT_AFTER) / REPEAT_EVERY).floor().max(-1.0);
    (after - before).max(0.0) as u32
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
    // Up and down step the cursor; held, they repeat.
    let down = input.down(KeyCode::ArrowDown) || input.down(KeyCode::KeyS);
    let up = input.down(KeyCode::ArrowUp) || input.down(KeyCode::KeyW);
    let steps = repeat(&mut map.held, down || up, input.pressed(KeyCode::ArrowDown) || input.pressed(KeyCode::KeyS) || input.pressed(KeyCode::ArrowUp) || input.pressed(KeyCode::KeyW), ctx.dt);
    for _ in 0..steps {
        map.selected = if down { (map.selected + 1) % n } else { (map.selected + n - 1) % n };
        crate::sound::click(ctx, 1200.0);
    }
    if input.pressed(KeyCode::Delete) || crate::keys::pressed(input, crate::keys::Act::Untarget) {
        app.engine.send(universe_sim::Command::SetNavTarget(None));
    }
    // Route editing.
    if crate::keys::pressed(input, crate::keys::Act::AddStop)
        && let Some(e) = map.entries.get(map.selected)
        && !matches!(e.target, NavTarget::Gate(_) | NavTarget::Asteroid(_))
    {
        app.engine.send(universe_sim::Command::RoutePush(universe_sim::Stop { system: map.view, target: e.target }));
        crate::sound::click(ctx, 1500.0);
    }
    if (input.pressed(KeyCode::Backspace) || crate::keys::pressed(input, crate::keys::Act::DropStop)) && !app.v.avionics.route.stops.is_empty() {
        app.engine.send(universe_sim::Command::RoutePop);
        crate::sound::click(ctx, 700.0);
    }
    if crate::keys::pressed(input, crate::keys::Act::EmptyRoute) {
        app.engine.send(universe_sim::Command::RouteClear);
        crate::sound::click(ctx, 500.0);
    }
    if crate::keys::pressed(input, crate::keys::Act::SettlerRoute) {
        // A reproducible settler route: each press loads the next seed.
        app.engine.send(universe_sim::Command::RouteRandom { seed: map.settler_seed, stops: 10 });
        map.settler_seed += 1;
        crate::sound::chime(ctx);
    }
    let input = &ctx.input;
    if (input.pressed(KeyCode::Enter) || crate::keys::pressed(input, crate::keys::Act::Target))
        && let Some(e) = map.entries.get(map.selected)
    {
        if map.here(app) {
            app.engine.send(universe_sim::Command::SetNavTarget(Some(e.target)));
            return false;
        }
        app.say(format!("LOCK TARGETS IN THIS SYSTEM - OR ADD TO THE ROUTE ({})", crate::keys::key(crate::keys::Act::AddStop)));
    }
    if crate::keys::pressed(input, crate::keys::Act::Network) {
        map.network = !map.network;
    }
    if crate::keys::pressed(input, crate::keys::Act::Map) || input.pressed(KeyCode::Escape) {
        return false;
    }
    // U: out to the whole galaxy.
    if crate::keys::pressed(input, crate::keys::Act::Galaxy) {
        app.galaxy_map = Some(crate::galaxymap::GalaxyMap::open(app, ctx.low_res.as_vec2()));
        return false;
    }
    app.nav_map = Some(map);
    true
}

pub fn draw(frame: &mut Frame, app: &App, map: &NavMap) {
    let size = frame.size();
    frame.hud_rect(Vec2::ZERO, size, Color([0.012, 0.018, 0.026, 1.0]));
    let line = GLYPH + 4.0;

    // Target list.
    let mut y = 16.0;
    let systems = browsable(app);
    let k = systems.iter().position(|&s| s == map.view).unwrap_or(0) + 1;
    let whose = if map.here(app) { "  (YOU ARE HERE)".to_string() } else { format!("  {:.1} LY AWAY", app.charts.distance_ly(app.v.ship_system, map.view)) };
    let title = format!("NAVIGATION - {} SYSTEM{whose}   < {k}/{} >", map.system.name.to_uppercase(), systems.len());
    let end = frame.text(Vec2::new(16.0, y), &title, TEXT);
    let holder = app.v.realm.holder(map.view);
    let held = holder.map_or("   UNCLAIMED".to_string(), |f| format!("   {} SPACE", f.name));
    frame.text(end, &held, holder.map_or(DIM, |f| Color([f.color[0], f.color[1], f.color[2], 1.0])));
    y += line * 2.0;
    if map.entries.is_empty() {
        frame.text(Vec2::new(16.0, y), "NOTHING TO DOCK OR LAND AT HERE", DIM);
    }
    let route_has = |t: NavTarget| app.v.avionics.route.stops.iter().any(|s| s.system == map.view && s.target == t);
    // The network layer: our status over the list, and each place's lag and uplink in it.
    let net = map.network.then(|| network(app, map));
    if let (Some(n), true) = (&net, map.here(app)) {
        let comm = app.ship.spec().comm;
        let (text, c) = match &n.ship {
            Some(s) => (format!("ON THE HYPERNET: {} FROM THE BACKBONE, VIA {}", fmt::lag(s.lag), n.net.nodes[s.via].name.to_uppercase()), lag_color(s.lag)),
            None => ("OFF THE HYPERNET: NO RELAY ON THE NET IN REACH".to_string(), DARK),
        };
        frame.text(Vec2::new(16.0, y), &text, c);
        y += line;
        frame.text(Vec2::new(16.0, y), &format!("YOUR COMM LINKS {}, HEARS {}", fmt::distance(comm.link), fmt::distance(comm.capture)), DIM);
        y += line * 1.5;
    }
    let header = match &net {
        Some(_) => format!("   {:<28} {:<9} {:<3} {:>10} {:>9}", "PLACE", "KIND", "", "DISTANCE", "NET"),
        None => format!("   {:<34} {:<9} {:<3} {:>10}", "PLACE", "KIND", "", "DISTANCE"),
    };
    frame.text(Vec2::new(16.0, y), &header, DIM);
    y += line;
    for (i, e) in map.entries.iter().enumerate() {
        let locked = map.here(app) && app.v.avionics.nav_target == Some(e.target);
        let c = if i == map.selected { SELECT } else if locked { LOCKED } else { TEXT };
        let cursor = if i == map.selected { ">" } else { " " };
        let mark = if locked { "*" } else if route_has(e.target) { "+" } else { " " };
        let dist = if e.distance.is_finite() { fmt::distance(e.distance) } else { String::new() };
        let sam = if map.defended.contains(&e.target) { "SAM" } else { "" };
        let row = match &net {
            Some(n) => {
                let (lag, nc) = place_on_net(app, map, n, e.target);
                let name: String = e.name.chars().take(28).collect();
                frame.text(Vec2::new(16.0, y), &format!("{cursor}{mark} {name:<28} {:<9} {sam:<3} {dist:>10}", e.kind), c);
                // (The net's column in its own colour.)
                frame.text(Vec2::new(16.0, y), &format!("{:>57}{lag:>9}", ""), nc);
                y += line;
                continue;
            }
            None => format!("{cursor}{mark} {:<34} {:<9} {:<3} {:>10}", e.name, e.kind, sam, dist),
        };
        frame.text(Vec2::new(16.0, y), &row, c);
        y += line;
    }

    // The route.
    y += line;
    let r = &app.v.avionics.route;
    let auto = crate::keys::key(crate::keys::Act::Autopilot);
    let state = if r.active { format!("FLYING - {auto} TO STOP") } else if r.stops.is_empty() { "EMPTY".into() } else { format!("{auto} IN FLIGHT TO START") };
    frame.text(Vec2::new(16.0, y), &format!("ROUTE ({} STOPS) - {state}", r.stops.len()), TEXT);
    y += line;
    for i in 0..r.stops.len() {
        let name = app.route_labels.get(i).cloned().unwrap_or_default();
        let c = if r.active && i == r.next { SELECT } else if i < r.next { DIM } else { TEXT };
        frame.text(Vec2::new(16.0, y), &format!("{:>2}. {name}", i + 1), c);
        y += line;
    }

    // The map's actions, as a grid like the flight's (see `keys`).
    {
        use crate::hud::Lamp;
        use crate::keys::{key, Act};
        let route = !app.v.avionics.route.stops.is_empty();
        let off = |b: bool| if b { Lamp::Off } else { Lamp::Unavailable };
        let cells = vec![
            (key(Act::Target), "TARGET".to_string(), off(map.here(app))),
            (key(Act::Untarget), "CLEAR TARGET".to_string(), off(app.v.avionics.nav_target.is_some())),
            (key(Act::AddStop), "ADD STOP".to_string(), Lamp::Off),
            (key(Act::DropStop), "DROP STOP".to_string(), off(route)),
            (key(Act::EmptyRoute), "EMPTY ROUTE".to_string(), off(route)),
            (key(Act::SettlerRoute), "SETTLER ROUTE".to_string(), Lamp::Off),
            (key(Act::Network), "NETWORK".to_string(), if map.network { Lamp::On } else { Lamp::Off }),
            (key(Act::Galaxy), "GALAXY".to_string(), Lamp::Off),
            (key(Act::Map), "MAP".to_string(), Lamp::On),
            ("< >".to_string(), "SYSTEM".to_string(), Lamp::Off),
        ];
        crate::hud::draw_grid(frame, "MAP", &cells);
    }

    let (center, max_r) = (Vec2::new(size.x * 0.76, size.y * 0.5), (size.x * 0.22).min(size.y * 0.42));
    chart(frame, app, map, net.as_ref(), center, max_r);
}

/// The browsed system's hypernet now: its relays' net, the bodies' positions,
/// and (in our own system) our status on it.
pub struct NetNow {
    net: universe_sim::world::hypernet::Net,
    positions: Vec<DVec3>,
    ship: Option<universe_sim::world::hypernet::Status>,
}

fn network(app: &App, map: &NavMap) -> NetNow {
    use universe_sim::world::hypernet::Net;
    let sys = &map.system;
    let t = app.v.time;
    let mut positions = Vec::new();
    sys.positions(t, &mut positions);
    let net = Net::at(sys, app.v.realm.nodes(&app.charts.galaxy, sys), t, &positions);
    let ship = map.here(app).then(|| net.status(sys, &positions, app.ship.position, &app.ship.spec().comm)).flatten();
    NetNow { net, positions, ship }
}

/// How quick a lag is, as a colour: green under a second, cyan under a
/// minute, amber under an hour, orange past it.
fn lag_color(lag: f64) -> Color {
    if lag < 1.0 {
        Color::hex(0x60ffb0)
    } else if lag < 60.0 {
        Color::hex(0x60c0ff)
    } else if lag < 3600.0 {
        Color::hex(0xffc040)
    } else {
        Color::hex(0xff8040)
    }
}
const DARK: Color = Color::hex(0xff5050);

/// A place on the net: its lag from the backbone (a relay's own; at a rock,
/// ours if we were there), in the lag's colour.
fn place_on_net(app: &App, map: &NavMap, n: &NetNow, target: NavTarget) -> (String, Color) {
    use universe_sim::world::hypernet::NodeAt;
    let node = match target {
        NavTarget::Station(b) | NavTarget::Gate(b) => n.net.node(NodeAt::Body(b)),
        NavTarget::Spaceport(k) => n.net.node(NodeAt::Port(k)),
        NavTarget::Asteroid(b) => {
            return match n.net.status(&map.system, &n.positions, n.positions[b], &app.ship.spec().comm) {
                Some(s) => (fmt::lag(s.lag), lag_color(s.lag)),
                None => ("DARK".into(), DARK),
            };
        }
    };
    let Some(k) = node else { return (String::new(), DIM) };
    match n.net.lag[k] {
        Some(l) => (fmt::lag(l), lag_color(l)),
        None => ("DARK".into(), DARK),
    }
}

/// Top-down schematic: planets on evenly spaced rings at their true angles.
/// With the network layer on (`net`), the routes in use over it, each relay
/// by its lag, each rock by whether our comm would be on the net there, and
/// our own line in.
fn chart(frame: &mut Frame, app: &App, map: &NavMap, net: Option<&NetNow>, center: Vec2, max_r: f32) {
    let sys = &map.system;
    let planets: Vec<usize> = (0..sys.bodies.len()).filter(|&i| sys.bodies[i].rail.parent == Some(0) && sys.bodies[i].kind.is_planet()).collect();
    let semi_major = |i: usize| sys.bodies[i].rail.orbit.as_ref().map_or(0.0, |o| o.semi_major_axis);
    let ring = max_r / planets.len().max(1) as f32;
    let angle_of = |v: DVec3| (v.z as f32).atan2(v.x as f32);
    let place = |i: usize| -> Vec2 {
        // An asteroid: between the rings of the planets inside and outside it.
        let k = match planets.iter().position(|&p| p == i) {
            Some(k) => k as f32 + 1.0,
            None => {
                let a = semi_major(i);
                let inside = planets.iter().filter(|&&p| semi_major(p) < a).count();
                let (lo, hi) = (inside.checked_sub(1).map_or(0.0, |k| semi_major(planets[k])), planets.get(inside).map_or(a * 1.3, |&p| semi_major(p)));
                inside as f32 + ((a - lo) / (hi - lo).max(1.0)) as f32
            }
        };
        let a = angle_of(map.positions[i]);
        center + Vec2::new(a.cos(), a.sin()) * ring * k
    };
    // Any point on the chart as the worlds are: at its true angle, its
    // distance from the star placed between the rings as theirs are.
    let chart_at = |p: DVec3| -> Vec2 {
        let v = p - map.positions[0];
        let d = (v.x * v.x + v.z * v.z).sqrt();
        let inside = planets.iter().filter(|&&q| semi_major(q) < d).count();
        let lo = inside.checked_sub(1).map_or(0.0, |k| semi_major(planets[k]));
        let hi = planets.get(inside).map_or_else(|| lo + (lo - inside.checked_sub(2).map_or(0.0, |k| semi_major(planets[k]))).max(1.0), |&q| semi_major(q));
        let k = inside as f32 + ((d - lo) / (hi - lo).max(1.0)) as f32;
        center + Vec2::new((v.z as f32).atan2(v.x as f32).cos(), (v.z as f32).atan2(v.x as f32).sin()) * ring * k
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

    // Names, small and faint (the one picked in the list gets its full title).
    const SMALL: f32 = 0.45;
    let faint = |c: Color| Color([c.0[0], c.0[1], c.0[2], 0.55]);
    let label = |frame: &mut Frame, at: Vec2, name: &str, c: Color| {
        frame.text_scaled(at, &name.to_uppercase(), faint(c), SMALL);
    };
    frame.hud_rect(center - 3.0, Vec2::splat(7.0), color(sys.bodies[0].color));
    // How far apart the rings are (the gap out to each, along the axis to the right).
    let mut inner = 0.0;
    for (k, &p) in planets.iter().enumerate() {
        let (r0, r1) = (ring * k as f32, ring * (k as f32 + 1.0));
        let gap = semi_major(p) - inner;
        inner = semi_major(p);
        // (The unit once; then above and below the axis by turns, to keep apart.)
        let text = if k == 0 { fmt::distance(gap) } else { format!("{:.2}", gap / universe_sim::world::units::AU) };
        let w = text_size(&text).x * SMALL;
        let y = if k % 2 == 0 { -9.0 } else { 3.0 };
        frame.text_scaled(center + Vec2::new((r0 + r1) / 2.0 - w / 2.0, y), &text, DIM.scale(0.8), SMALL);
    }
    for (k, &p) in planets.iter().enumerate() {
        frame.hud_ellipse(center, Vec2::splat(ring * (k as f32 + 1.0)), 64, DIM.scale(0.6));
        let at = place(p);
        label(frame, at + Vec2::new(6.0, 3.0), &sys.bodies[p].name, TEXT);
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
            label(frame, chart_pos(i) + Vec2::new(4.0, 2.0), &b.name, DIM);
        }
    }
    // Asteroid fields: a scatter of dots.
    for f in &sys.fields {
        let at = chart_pos(f.body);
        let c = color(sys.bodies[f.body].color).scale(0.8);
        for (dx, dy) in [(0.0, 0.0), (-3.0, 2.0), (3.0, 1.0), (1.0, -3.0), (-2.0, -2.0)] {
            frame.hud_rect(at + Vec2::new(dx, dy), Vec2::splat(1.0), c);
        }
    }

    // Targets: stations as squares, spaceports as triangles.
    for (i, e) in map.entries.iter().enumerate() {
        let (at, c) = match e.target {
            NavTarget::Station(b) => (chart_pos(b), Color::WHITE),
            NavTarget::Gate(b) => (chart_pos(b), Color::hex(0xffc040)),
            NavTarget::Asteroid(b) => (chart_pos(b), color(sys.bodies[b].color)),
            NavTarget::Spaceport(p) => {
                let body = sys.spaceports[p].body;
                (chart_pos(body) + Vec2::new(0.0, -8.0), Color::hex(0x60c0ff))
            }
        };
        match e.target {
            NavTarget::Station(_) => frame.hud_box(at + Vec2::new(8.0, -3.0), Vec2::splat(6.0), c),
            NavTarget::Gate(_) => frame.hud_ellipse(at, Vec2::splat(5.0), 10, c),
            NavTarget::Asteroid(_) => frame.hud_ellipse(at, Vec2::splat(6.0), 6, c),
            NavTarget::Spaceport(_) => {
                frame.hud_line(at + Vec2::new(-3.0, 0.0), at + Vec2::new(3.0, 0.0), c);
                frame.hud_line(at + Vec2::new(-3.0, 0.0), at + Vec2::new(0.0, -5.0), c);
                frame.hud_line(at + Vec2::new(3.0, 0.0), at + Vec2::new(0.0, -5.0), c);
            }
        }
        if i != map.selected {
            let name = match e.target {
                NavTarget::Spaceport(p) => sys.spaceports[p].name.clone(),
                NavTarget::Station(b) | NavTarget::Gate(b) => sys.bodies[b].name.clone(),
                NavTarget::Asteroid(_) => e.name.clone(),
            };
            label(frame, at + Vec2::new(8.0, -10.0), &name, c);
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

    // The network layer.
    use universe_sim::world::hypernet::NodeAt;
    let node_at = |n: &NetNow, k: usize| match n.net.nodes[k].at {
        NodeAt::Port(p) => chart_pos(sys.spaceports[p].body) + Vec2::new(0.0, -10.0),
        NodeAt::Body(b) if sys.bodies[b].kind == BodyKind::Station => chart_pos(b) + Vec2::new(11.0, 0.0),
        NodeAt::Body(b) => chart_pos(b),
        NodeAt::Beacon { body, .. } => chart_pos(body) + Vec2::new(-11.0, 0.0),
    };
    if let Some(n) = net {
        for (k, up) in n.net.routes() {
            let c = n.net.lag[k].map_or(DARK.scale(0.5), |l| lag_color(l).scale(0.8));
            frame.hud_line_smooth(node_at(n, k), node_at(n, up), c);
        }
        for k in 0..n.net.nodes.len() {
            // (A world's relay not on anyone's way in stands by: dim, not dark.)
            let c = match n.net.lag[k] {
                Some(l) => lag_color(l),
                None if n.net.nodes[k].around.is_some() && !n.net.used[k] => DIM.scale(0.5),
                None => DARK,
            };
            frame.hud_ellipse(node_at(n, k), Vec2::splat(3.0), 8, c);
        }
        // Each relay switched on: its reach, to scale (its true circle through
        // the chart's mapping; one for relays together, a world's and its ports).
        let mut drawn: Vec<DVec3> = Vec::new();
        for (k, node) in n.net.nodes.iter().enumerate().filter(|(k, x)| n.net.used[*k] && x.relay.is_some()) {
            let at = n.net.at[k];
            if drawn.iter().any(|d| d.distance(at) < node.comm.link * 0.05) {
                continue;
            }
            drawn.push(at);
            let c = n.net.lag[k].map_or(DARK, lag_color).scale(0.35);
            const SIDES: usize = 48;
            let pts: Vec<Vec2> = (0..=SIDES).map(|i| {
                let a = i as f64 / SIDES as f64 * std::f64::consts::TAU;
                chart_at(at + DVec3::new(a.cos(), 0.0, a.sin()) * node.comm.link)
            }).collect();
            for w in pts.windows(2) {
                if (w[0] - center).length() < max_r * 1.08 && (w[1] - center).length() < max_r * 1.08 {
                    frame.hud_line_smooth(w[0], w[1], c);
                }
            }
        }
        let comm = app.ship.spec().comm;
        for f in &sys.fields {
            let c = n.net.status(sys, &n.positions, n.positions[f.body], &comm).map_or(DARK, |s| lag_color(s.lag));
            frame.hud_ellipse(chart_pos(f.body), Vec2::splat(10.0), 16, c.scale(0.8));
        }
    }

    // The ship, next to whatever it's orbiting (only in its own system).
    let Some(ship_body) = map.ship_body else { return };
    let you = chart_pos(ship_body) + Vec2::new(-10.0, 6.0);
    if let Some(n) = net {
        let c = n.ship.as_ref().map_or(DARK, |s| lag_color(s.lag));
        if let Some(s) = &n.ship {
            frame.hud_line_smooth(you, node_at(n, s.via), c);
        }
    }
    for i in 0..3 {
        let a = i as f32 / 3.0 * TAU - TAU / 4.0;
        let b = (i + 1) as f32 / 3.0 * TAU - TAU / 4.0;
        frame.hud_line(you + Vec2::new(a.cos(), a.sin()) * 5.0, you + Vec2::new(b.cos(), b.sin()) * 5.0, TEXT);
    }
    frame.text(you + Vec2::new(-28.0, -4.0), "YOU", TEXT);
}
