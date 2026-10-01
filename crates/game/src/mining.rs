//! Mining mode (1): the mining action panel, and the prospector's pulse.
//! Prospecting (2) sends a pulse out to `PULSE_RANGE`: a shell that grows
//! round the ship, lighting up every rock it passes. What it found is listed
//! (numbered, nearest first: class, structure, size, range, ore and how fast
//! it digs) and tagged with its number in the view; T locks one (see
//! `lock`). With a rock locked: N keeps at a range from it, U orbits it, 3
//! approaches it (closes to just off its surface, turning with it), Y
//! anchors, H digs.

use universe_engine::glam::{DVec3, Vec2};
use universe_engine::{Color, Context, Frame, KeyCode};
use universe_sim::world::mining;
use universe_sim::Command;

use crate::{fmt, App};

/// The pulse reaches this far (m)...
pub const PULSE_RANGE: f64 = 30_000.0;
/// ...in this long (real s).
const PULSE_SECS: f32 = 2.0;
/// Rows of the results list shown (and numbers tagged in the view).
const SHOWN: usize = 12;

const PULSE: Color = Color::hex(0x40e0ff);

#[derive(Default)]
pub struct Mining {
    pub on: bool,
    pub prospect: Option<Prospect>,
    /// (Dev scenarios: a pulse to send once the view is up, this old.)
    show: Option<f32>,
}

/// A pulse sent, and what it found: (field, body among its bodies, range
/// when the pulse went out).
pub struct Prospect {
    pub system: usize,
    pub age: f32,
    /// Where it went out from (the view's coordinates).
    pub origin: DVec3,
    pub found: Vec<(usize, usize, f64)>,
}

impl Prospect {
    /// How far the shell has reached (m).
    fn reach(&self) -> f64 {
        PULSE_RANGE * (self.age / PULSE_SECS).min(1.0) as f64
    }
}

/// One rock found, as it is now.
pub struct Row {
    pub field: usize,
    pub body: usize,
    pub name: String,
    pub detail: String,
    pub distance: f64,
    pub at: DVec3,
    pub color: [f32; 3],
}

/// The rocks the pulse has reached so far, nearest first.
pub fn rows(app: &App) -> Vec<Row> {
    let Some(p) = &app.mining.prospect else { return Vec::new() };
    if p.system != app.v.ship_system || app.view.origin != app.v.ship_system {
        return Vec::new();
    }
    let sys = &app.view.system;
    let t = app.now();
    let ship = app.view.ship_pos;
    let mut rows: Vec<Row> = p
        .found
        .iter()
        .filter(|f| f.2 <= p.reach())
        .filter_map(|&(field, body, _)| {
            let bodies = sys.field_bodies(field);
            let b = bodies.get(body)?;
            let r = b.rock.as_ref()?;
            let (at, _) = sys.field_body_state(field, body, t);
            let ore = &app.charts.goods[mining::ore(r).item()];
            let solid = if r.structure == universe_sim::world::belt::Structure::Rubble { "RUBBLE" } else { "SOLID" };
            let detail = format!("{:<7}{:>6} {solid:<6} {:<10}{:>4.1}KG/S", r.class.letter(), fmt::distance(b.rail.radius * 2.0), ore_short(&ore.name), mining::dig_rate(r));
            Some(Row { field, body, name: b.name.to_uppercase(), detail, distance: at.distance(ship) - b.rail.radius, at, color: b.color })
        })
        .collect();
    rows.sort_by(|a, b| a.distance.total_cmp(&b.distance));
    rows
}

fn ore_short(name: &str) -> String {
    name.to_uppercase().replace("ASTEROID ", "").replace(" ORE", "")
}

/// Mining mode's keys, once a frame (in the pilot's seat).
pub fn input(app: &mut App, ctx: &Context) {
    let input = &ctx.input;
    if let Some(p) = &mut app.mining.prospect {
        p.age += ctx.dt;
    }
    if let Some(age) = app.mining.show
        && app.view.positions.len() >= app.view.system.bodies.len()
    {
        app.mining.show = None;
        prospect(app);
        if let Some(p) = &mut app.mining.prospect {
            p.age = age;
        }
    }
    if input.pressed(KeyCode::Digit1) {
        app.mining.on = !app.mining.on;
        app.say(if app.mining.on { "MINING MODE - 2 PROSPECT, T LOCK, 3 APPROACH, Y ANCHOR, H DIG".into() } else { "MINING MODE OFF".into() });
    }
    if !app.mining.on {
        return;
    }
    if input.pressed(KeyCode::Digit2) {
        prospect(app);
    }
    if input.pressed(KeyCode::Digit3) {
        match app.v.avionics.rock_lock {
            Some((field, body)) => app.engine.send(Command::CloseOn { field, body }),
            None => app.say("APPROACH: LOCK A ROCK FIRST (T)".into()),
        }
    }
}

/// (Dev scenarios: a pulse sent `age` seconds ago, once the view is up.)
pub fn prospect_for_show(app: &mut App, age: f32) {
    app.mining.show = Some(age);
}

/// Send a pulse: every rock within its range is found (shown as the shell reaches it).
fn prospect(app: &mut App) {
    if app.mining.prospect.as_ref().is_some_and(|p| p.age < PULSE_SECS) {
        return;
    }
    if app.view.origin != app.v.ship_system || app.view.positions.len() < app.view.system.bodies.len() {
        return;
    }
    let sys = app.view.system.clone();
    let t = app.now();
    let ship = app.view.ship_pos;
    let mut found = Vec::new();
    for (f, field) in sys.fields.iter().enumerate() {
        if app.view.positions[field.body].distance(ship) > field.extent + PULSE_RANGE + 20_000.0 {
            continue;
        }
        let bodies = sys.field_bodies(f);
        for i in sys.field_rocks(f) {
            let (at, _) = sys.field_body_state(f, i, t);
            let d = at.distance(ship) - bodies[i].rail.radius;
            if d < PULSE_RANGE {
                found.push((f, i, d.max(0.0)));
            }
        }
    }
    let n = found.len();
    app.mining.prospect = Some(Prospect { system: app.v.ship_system, age: 0.0, origin: ship, found });
    app.say(if n == 0 { "PROSPECT - NOTHING WITHIN 30 KM".into() } else { format!("PROSPECT - {n} ROCKS WITHIN 30 KM") });
}

/// The pulse's shell, and the rocks found tagged with their numbers.
pub fn draw_scene(frame: &mut Frame, app: &App) {
    let Some(p) = &app.mining.prospect else { return };
    if p.system != app.v.ship_system || app.view.origin != app.v.ship_system {
        return;
    }
    if p.age < PULSE_SECS + 0.6 {
        let r = p.reach();
        let fade = 1.0 - ((p.age - PULSE_SECS).max(0.0) / 0.6);
        let c = PULSE.scale(fade * 0.8);
        let to_cam = (frame.camera.position - p.origin).normalize_or(DVec3::Y);
        frame.circle(p.origin, to_cam, r, 96, c);
        for n in [DVec3::Y, DVec3::X, DVec3::Z] {
            frame.circle(p.origin, n, r, 96, c.scale(0.35));
        }
    }
    if !app.mining.on {
        return;
    }
    for (k, row) in rows(app).iter().enumerate().take(SHOWN) {
        if let Some(at) = frame.project(row.at) {
            let c = crate::scene::color(row.color);
            frame.text(at + Vec2::new(6.0, -4.0), &format!("{}", k + 1), c);
        }
    }
}

/// The action panel's state, for the toolbar.
pub fn pulsing(app: &App) -> bool {
    app.mining.prospect.as_ref().is_some_and(|p| p.age < PULSE_SECS)
}

/// The results list (not while T's list is up): numbered, nearest first.
pub fn draw_hud(frame: &mut Frame, app: &App) {
    if !app.mining.on || app.picker.listing() {
        return;
    }
    let size = frame.size();
    let line = 12.0;
    let list = rows(app);
    let lock = app.v.avionics.rock_lock;
    let title = match &app.mining.prospect {
        None => "MINING - 2 TO PROSPECT".to_string(),
        Some(p) if p.age < PULSE_SECS => format!("PROSPECTING... {}", fmt::distance(p.reach())),
        Some(_) => format!("PROSPECT: {} ROCKS - T LOCKS (HOLD: LIST)", list.len()),
    };
    let lines: Vec<(String, Color)> = list
        .iter()
        .enumerate()
        .take(SHOWN)
        .map(|(k, r)| {
            let locked = lock == Some((r.field, r.body));
            let c = crate::scene::color(r.color);
            (format!("{}{:>2} {:<12} {} {:>7}", if locked { "*" } else { " " }, k + 1, r.name.chars().take(12).collect::<String>(), r.detail, fmt::distance(r.distance.max(0.0))), if locked { c } else { c.scale(0.7) })
        })
        .collect();
    let width = lines.iter().map(|l| universe_engine::text_size(&l.0).x).fold(universe_engine::text_size(&title).x, f32::max);
    let pos = Vec2::new(size.x - width - 12.0, 230.0);
    frame.hud_rect(pos - 6.0, Vec2::new(width, (lines.len() as f32 + 1.5) * line) + 12.0, Color([0.0, 0.02, 0.03, 0.7]));
    frame.text(pos, &title, PULSE);
    for (k, (text, c)) in lines.iter().enumerate() {
        frame.text(Vec2::new(pos.x, pos.y + (k as f32 + 1.5) * line), text, *c);
    }
}
