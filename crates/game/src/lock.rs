//! Locking on: T. A tap locks what's nearest the crosshair; held, it lists
//! what can be locked, nearest first: the mouse (or the wheel) moves the
//! cursor, and letting go of T locks the one under it. What can be locked
//! is the mode's: in navigation everything within the sensors' reach
//! (ships, asteroids big and small, stations, gates, spaceports — a place
//! locked becomes the nav target); in combat the ships; in mining the
//! rocks the prospect found.

use universe_engine::glam::{DVec3, Vec2};
use universe_engine::{Color, Context, Frame};
use universe_sim::Command;

use crate::{fmt, App};

/// Held longer than this (s), T lists instead of locking what's ahead.
const HOLD: f32 = 0.25;
/// Mouse travel per row of the list (device units).
const ROW_TRAVEL: f32 = 40.0;
/// A tap in mining mode locks the rock within this of the nose (rad).
const CONE: f64 = 0.17;

/// What a lock is on.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Pick {
    /// A radar contact, by its id.
    Ship(usize),
    /// A rock: body among a field's bodies.
    Rock(usize, usize),
    /// A station, gate or spaceport (locked: the nav target).
    Place(universe_sim::NavTarget),
}

/// One line of the list.
pub struct Candidate {
    pub pick: Pick,
    pub name: String,
    pub detail: String,
    /// To its surface (m), and where it is.
    pub distance: f64,
    pub at: DVec3,
}

/// T's state: how long it's been held, and the list's cursor.
#[derive(Default)]
pub struct Picker {
    held: f32,
    /// (Dev scenarios: the list stays up.)
    shown_for_dev: bool,
    index: usize,
    travel: f32,
}

impl Picker {
    /// (Dev scenarios: as if T had been held a while.)
    pub fn hold_for_show(&mut self) {
        self.held = 1.0;
        self.shown_for_dev = true;
    }

    pub fn listing(&self) -> bool {
        self.held >= HOLD
    }
}

/// What T can lock now, nearest first.
pub fn candidates(app: &App) -> Vec<Candidate> {
    use crate::hud::{active_mode, ShipMode};
    let mode = active_mode(app);
    if mode == ShipMode::Mining {
        return crate::mining::rows(app)
            .into_iter()
            .map(|r| Candidate { pick: Pick::Rock(r.field, r.body), name: r.name, detail: r.detail, distance: r.distance, at: r.at })
            .collect();
    }
    let mut list: Vec<Candidate> = app
        .contacts
        .iter()
        .map(|c| Candidate {
            pick: Pick::Ship(c.blip.id),
            name: c.name.to_uppercase(),
            detail: format!("SHIP  {}", c.activity),
            distance: c.blip.distance,
            at: c.blip.position,
        })
        .collect();
    // Navigation: everything else within the sensors' reach too.
    if mode == ShipMode::Nav && app.view.origin == app.v.ship_system && app.view.positions.len() >= app.view.system.bodies.len() {
        let sys = &app.view.system;
        let ship = app.view.ship_pos;
        let reach = universe_sim::world::RADAR_RANGE;
        let t = app.now();
        let mut place = |target: universe_sim::NavTarget, what: &str, at: DVec3| {
            let d = at.distance(ship);
            if d < reach {
                list.push(Candidate { pick: Pick::Place(target), name: target.name(sys).to_uppercase(), detail: what.to_string(), distance: d, at });
            }
        };
        for (i, b) in sys.bodies.iter().enumerate() {
            match b.kind {
                universe_sim::BodyKind::Station => place(universe_sim::NavTarget::Station(i), "STATION", app.view.positions[i]),
                universe_sim::BodyKind::Gate => place(universe_sim::NavTarget::Gate(i), "GATE", app.view.positions[i]),
                _ => {}
            }
        }
        for p in 0..sys.spaceports.len() {
            if let Some(at) = universe_sim::NavTarget::Spaceport(p).position(sys, t, &app.view.positions) {
                place(universe_sim::NavTarget::Spaceport(p), "SPACEPORT", at);
            }
        }
        // Asteroids: the remnants, and the swarms of fields in reach.
        for (f, field) in sys.fields.iter().enumerate() {
            if app.view.positions[field.body].distance(ship) > field.extent + reach {
                continue;
            }
            let bodies = sys.field_bodies(f);
            for i in sys.field_rocks(f) {
                let (at, _) = sys.field_body_state(f, i, t);
                let b = &bodies[i];
                let d = at.distance(ship) - b.rail.radius;
                if d < reach
                    && let Some(r) = &b.rock
                {
                    let what = format!("ASTEROID {} {}", r.class.letter(), crate::fmt::distance(b.rail.radius * 2.0));
                    list.push(Candidate { pick: Pick::Rock(f, i), name: b.name.to_uppercase(), detail: what, distance: d.max(0.0), at });
                }
            }
        }
    }
    list.sort_by(|a, b| a.distance.total_cmp(&b.distance));
    list
}

/// What's locked now.
pub fn locked(app: &App) -> Option<Pick> {
    let a = &app.v.avionics;
    a.contact.map(Pick::Ship).or(a.rock_lock.map(|(f, b)| Pick::Rock(f, b))).or(a.nav_target.map(Pick::Place))
}

fn lock(app: &mut App, pick: Pick) {
    app.engine.send(match pick {
        Pick::Ship(id) => Command::LockContact(id),
        Pick::Rock(f, b) => Command::LockRock(Some((f, b))),
        Pick::Place(t) => Command::SetNavTarget(Some(t)),
    });
}

/// T, once a frame. True while the list has the mouse (it doesn't steer).
pub fn input(app: &mut App, ctx: &Context) -> bool {
    let input = &ctx.input;
    if app.picker.shown_for_dev {
        return true;
    }
    if crate::keys::down(input, crate::keys::Act::Lock) {
        if app.picker.held == 0.0 {
            // The cursor starts on what's locked, if it's listed.
            let now = locked(app);
            app.picker.index = candidates(app).iter().position(|c| Some(c.pick) == now).unwrap_or(0);
            app.picker.travel = 0.0;
        }
        app.picker.held += ctx.dt.max(1e-4);
        if !app.picker.listing() {
            return false;
        }
        let n = candidates(app).len();
        app.picker.travel += input.mouse_delta.y - input.scroll * ROW_TRAVEL;
        let rows = (app.picker.travel / ROW_TRAVEL).trunc();
        app.picker.travel -= rows * ROW_TRAVEL;
        app.picker.index = (app.picker.index as i64 + rows as i64).clamp(0, n.saturating_sub(1) as i64) as usize;
        return true;
    }
    if app.picker.held == 0.0 {
        return false;
    }
    let listed = app.picker.listing();
    app.picker.held = 0.0;
    if listed {
        if let Some(c) = candidates(app).into_iter().nth(app.picker.index) {
            lock(app, c.pick);
        }
    } else if crate::hud::active_mode(app) != crate::hud::ShipMode::Combat {
        // What's nearest the nose, within the cone (in mining, of the prospected rocks).
        let (ship, nose) = (app.view.ship_pos, app.ship.forward());
        let off = |c: &Candidate| nose.angle_between(c.at - ship);
        match candidates(app).into_iter().filter(|c| off(c) < CONE).min_by(|a, b| off(a).total_cmp(&off(b))) {
            Some(c) => lock(app, c.pick),
            None if app.mining.on => app.say(if app.mining.prospect.is_some() { "NO PROSPECTED ROCK AHEAD - HOLD T FOR THE LIST".into() } else { "PROSPECT FIRST (2)".into() }),
            None => app.say("NOTHING AHEAD - HOLD T FOR THE LIST".into()),
        }
    } else {
        // Lock what's in the beam around the crosshair (the ring shows it for a moment).
        app.beam_shown = 1.5;
        app.engine.send(Command::LockInBeam);
    }
    false
}

const LIST: Color = Color::hex(0x60ffa0);

/// The list, while T is held: on the right, the cursor's line bright, and
/// brackets on the one under it in the view.
pub fn draw(frame: &mut Frame, app: &App) {
    if !app.picker.listing() {
        return;
    }
    let list = candidates(app);
    let size = frame.size();
    let line = 12.0;
    let shown = 20usize;
    let first = app.picker.index.saturating_sub(shown - 1);
    let row = |k: usize, c: &Candidate, mark: &str| format!("{mark}{:>2} {:<12} {} {:>7}", k + 1, truncate(&c.name, 12), truncate(&c.detail, 40), fmt::distance(c.distance.max(0.0)));
    let width = list.iter().enumerate().skip(first).take(shown).map(|(k, c)| universe_engine::text_size(&row(k, c, " ")).x).fold(360.0, f32::max);
    let pos = Vec2::new(size.x - width - 12.0, 230.0);
    let rows = list.len().clamp(1, shown) as f32;
    frame.hud_rect(pos - 6.0, Vec2::new(width, (rows + 2.0) * line) + 12.0, Color([0.0, 0.03, 0.0, 0.85]));
    frame.hud_box(pos - 6.0, Vec2::new(width, (rows + 2.0) * line) + 12.0, LIST.scale(0.6));
    let what = match crate::hud::active_mode(app) {
        crate::hud::ShipMode::Mining => "ROCKS",
        crate::hud::ShipMode::Combat => "SHIPS",
        crate::hud::ShipMode::Nav => "IN REACH",
    };
    frame.text(pos, &format!("LOCK {what} - MOUSE/WHEEL, LET GO OF T"), LIST);
    if list.is_empty() {
        frame.text(pos + Vec2::new(0.0, 2.0 * line), if app.mining.on { "NOTHING PROSPECTED - 2 TO PROSPECT" } else { "NOTHING IN REACH" }, LIST.scale(0.6));
        return;
    }
    let now = locked(app);
    for (k, c) in list.iter().enumerate().skip(first).take(shown) {
        let y = pos.y + (2 + k - first) as f32 * line;
        let here = k == app.picker.index;
        let mark = if here { ">" } else if Some(c.pick) == now { "*" } else { " " };
        let text = row(k, c, mark);
        frame.text(Vec2::new(pos.x, y), &text, if here { LIST } else { LIST.scale(0.55) });
    }
    if let Some(c) = list.get(app.picker.index)
        && let Some(p) = frame.project(c.at)
    {
        let k = 14.0;
        for (sx, sy) in [(-1.0, -1.0), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)] {
            let corner = p + Vec2::new(sx * k, sy * k);
            frame.hud_line(corner, corner - Vec2::new(sx * 6.0, 0.0), LIST);
            frame.hud_line(corner, corner - Vec2::new(0.0, sy * 6.0), LIST);
        }
    }
}

fn truncate(s: &str, n: usize) -> String {
    s.chars().take(n).collect()
}
