//! The shipyard (docked at a station): the ship's slots and what's fitted,
//! what else would go in the slot picked, and what the ship would be with
//! it — mass, power, thrust, tank and hold, how it turns, the autopilots it
//! runs — before it's bought. The first piece of the ship planner.
//!
//! Keys: ↑/↓ slot, ←/→ module, ENTER fit it, the shipyard key or ESC close.

use universe_engine::glam::Vec2;
use universe_engine::{Color, Context, Frame, KeyCode};
use universe_sim::world::content::{content, Handle};
use universe_sim::world::modules::{Does, Module, BASE_BLOCKS};
use universe_sim::world::ship::{fitted, ClassSpec, Fit, Slot};
use universe_sim::world::Facility;
use universe_sim::Command;

use crate::keys::{key, Act};
use crate::{fmt, App};

const TEXT: Color = Color::hex(0x30ff60);
const DIM: Color = Color::hex(0x178a38);
const SELECT: Color = Color::hex(0xffc040);
const RED: Color = Color::hex(0xff4040);
const BETTER: Color = Color::hex(0x60ffd0);

/// The panel's cursor: the slot, and the module offered for it.
#[derive(Default)]
pub struct Shipyard {
    slot: usize,
    choice: usize,
    held: f32,
}

/// Docked at a station: its name.
fn station(app: &App) -> Option<String> {
    match app.v.docked_market {
        Some(f @ Facility::Station(_)) => Some(f.name(&app.view.system).to_uppercase()),
        _ => None,
    }
}

/// What would go in `slot`: every module of its kind that fits it, and
/// (unless it's a base block) nothing.
fn offers(slot: &Slot) -> Vec<Option<Handle<Module>>> {
    let mut out: Vec<Option<Handle<Module>>> = content().modules.iter().filter(|(_, m)| m.does.slot() == slot.kind && m.size <= slot.size).map(|(h, _)| Some(h)).collect();
    if !BASE_BLOCKS.contains(&slot.kind) {
        out.push(None);
    }
    out
}

fn fit_of(app: &App) -> Fit {
    app.ship.fit.clone().unwrap_or_else(|| content().get(app.ship.class).fit.clone())
}

fn in_slot(fit: &Fit, slot: &Slot) -> Option<Handle<Module>> {
    fit.iter().find(|(s, _)| *s == slot.name).map(|(_, m)| *m)
}

impl Shipyard {
    /// (Dev scenarios: open on slot `slot`, module `choice` picked.)
    pub fn showing(slot: usize, choice: usize) -> Self {
        Shipyard { slot, choice, held: 0.0 }
    }
}

/// Open it (docked at a station), on the first slot.
pub fn open(app: &mut App) -> Option<Shipyard> {
    if station(app).is_none() {
        app.say("SHIPYARD - DOCK AT A STATION".into());
        return None;
    }
    Some(Shipyard { choice: current_choice(app, 0), ..Default::default() })
}

/// Where the module now in slot `k` stands among its offers.
fn current_choice(app: &App, k: usize) -> usize {
    let spec = app.ship.spec();
    let Some(slot) = spec.slots.get(k) else { return 0 };
    let now = in_slot(&fit_of(app), slot);
    offers(slot).iter().position(|o| *o == now).unwrap_or(0)
}

/// Keys while open. False when it should close.
pub fn input(app: &mut App, ctx: &Context) -> bool {
    let input = &ctx.input;
    if crate::keys::pressed(input, Act::Shipyard) || input.pressed(KeyCode::Escape) || station(app).is_none() {
        return false;
    }
    let spec = app.ship.spec();
    let slots = spec.slots.len().max(1);
    let Some(y) = &mut app.shipyard else { return false };
    let (down, up) = (input.down(KeyCode::ArrowDown), input.down(KeyCode::ArrowUp));
    let steps = crate::navmap::repeat(&mut y.held, down || up, input.pressed(KeyCode::ArrowDown) || input.pressed(KeyCode::ArrowUp), ctx.dt);
    let mut moved = false;
    for _ in 0..steps {
        y.slot = if down { (y.slot + 1) % slots } else { (y.slot + slots - 1) % slots };
        moved = true;
    }
    if moved {
        let k = y.slot;
        let c = current_choice(app, k);
        if let Some(y) = &mut app.shipyard {
            y.choice = c;
        }
        return true;
    }
    let Some(slot) = spec.slots.get(y.slot) else { return true };
    let n = offers(slot).len().max(1);
    if input.pressed(KeyCode::ArrowRight) {
        y.choice = (y.choice + 1) % n;
    }
    if input.pressed(KeyCode::ArrowLeft) {
        y.choice = (y.choice + n - 1) % n;
    }
    if input.pressed(KeyCode::Enter) {
        let module = offers(slot).get(y.choice).copied().flatten().map(|h| content().get(h).key.clone());
        app.engine.send(Command::Refit { slot: slot.name.clone(), module });
    }
    true
}

/// The fit with `module` in `slot`.
fn with(fit: &Fit, slot: &Slot, module: Option<Handle<Module>>) -> Fit {
    let mut f: Fit = fit.iter().filter(|(s, _)| *s != slot.name).cloned().collect();
    if let Some(m) = module {
        f.push((slot.name.clone(), m));
    }
    f
}

fn what(m: &Module) -> String {
    match &m.does {
        Does::PowerPlant { output } => format!("{:.1} MW", output / 1e6),
        Does::Drive { thrust } | Does::Thrusters { thrust } | Does::Lift { thrust } => format!("{:.0} KN A NOZZLE", thrust / 1e3),
        Does::Tank { capacity } | Does::Rack { capacity } => fmt::tonnes(*capacity),
        Does::FlightComputer { turn_rate, roll_rate } => format!("TURNS {turn_rate:.1}, ROLLS {roll_rate:.1} RAD/S"),
        Does::Sensors { range } => fmt::distance(*range),
        Does::NavComputer { features } => features.iter().map(|f| format!("{f:?}").to_uppercase()).collect::<Vec<_>>().join(" "),
        _ => String::new(),
    }
}

/// The ship's numbers, a line each: (label, value).
fn numbers(s: &ClassSpec) -> Vec<(&'static str, String)> {
    let loaded = s.dry_mass + s.fuel_capacity;
    vec![
        ("DRY MASS", fmt::tonnes(s.dry_mass)),
        ("TANK", fmt::tonnes(s.fuel_capacity)),
        ("HOLD", fmt::tonnes(s.hold_capacity)),
        ("POWER", format!("{:.1} OF {:.1} MW", s.power_draw / 1e6, s.power_output / 1e6)),
        ("MAIN DRIVE", format!("{:.1} M/S2", s.main_thrust / loaded)),
        ("THRUSTERS", format!("{:.1} M/S2", s.rcs_thrust / loaded)),
        ("LIFT", format!("{:.1} M/S2", s.lift_thrust / loaded)),
        ("TURNS", format!("{:.1} {:.1} {:.1} RAD/S2", s.turn_accel.x, s.turn_accel.y, s.turn_accel.z)),
        ("AUTOPILOTS", if s.features.is_empty() { "NONE".into() } else { s.features.iter().map(|f| format!("{f:?}").to_uppercase()).collect::<Vec<_>>().join(" ") }),
    ]
}

pub fn draw(frame: &mut Frame, app: &App, y: &Shipyard) {
    let size = frame.size();
    frame.hud_rect(Vec2::ZERO, size, Color([0.0, 0.015, 0.01, 1.0]));
    let line = 12.0;
    let spec = app.ship.spec();
    let fit = fit_of(app);
    let c = content();
    frame.text(
        Vec2::new(12.0, 12.0),
        &format!("SHIPYARD - {} - {}   CREDITS {:.0}   ({} CLOSES, UP/DOWN SLOT, LEFT/RIGHT MODULE, ENTER FIT)", station(app).unwrap_or_default(), spec.name, app.v.credits, key(Act::Shipyard)),
        TEXT,
    );
    // The slots.
    let top = 12.0 + line * 2.0;
    for (k, slot) in spec.slots.iter().enumerate() {
        let here = k == y.slot;
        let m = in_slot(&fit, slot).map(|h| c.get(h));
        let text = format!("{}{:<12} {:<11} S{}  {}", if here { ">" } else { " " }, slot.name.to_uppercase(), format!("{:?}", slot.kind).to_uppercase(), slot.size, m.map_or("(EMPTY)".to_string(), |m| m.name.clone()));
        frame.text(Vec2::new(12.0, top + k as f32 * line), &text, if here { SELECT } else { TEXT });
    }
    // What would go in the slot picked.
    let Some(slot) = spec.slots.get(y.slot) else { return };
    let x = (size.x * 0.52).floor();
    frame.text(Vec2::new(x, top), &format!("FOR {} ({:?}, SIZE {})", slot.name.to_uppercase(), slot.kind, slot.size).to_uppercase(), TEXT);
    let now = in_slot(&fit, slot);
    let list = offers(slot);
    for (k, o) in list.iter().enumerate() {
        let here = k == y.choice;
        let mark = if here { ">" } else if *o == now { "*" } else { " " };
        let text = match o.map(|h| c.get(h)) {
            Some(m) => format!("{mark}{:<20} {:>8} {:>8} {:>7} CR  {}", m.name, fmt::tonnes(m.mass), format!("{:.2} MW", m.power / 1e6), m.price as i64, what(m)),
            None => format!("{mark}(EMPTY)"),
        };
        frame.text(Vec2::new(x, top + (2 + k) as f32 * line), &text, if here { SELECT } else if *o == now { TEXT } else { DIM });
    }
    // The ship as it is, and as it would be.
    let pick = list.get(y.choice).copied().flatten();
    let preview = fitted(app.ship.class, &with(&fit, slot, pick));
    let cost = pick.map_or(0.0, |h| c.get(h).price) - now.map_or(0.0, |h| c.get(h).price * universe_sim::BUYBACK);
    let mut yy = top + (spec.slots.len().max(list.len() + 2) + 2) as f32 * line;
    frame.text(Vec2::new(12.0, yy), &format!("{:<12} {:>22} {:>22}", "", "NOW", "WITH IT"), DIM);
    yy += line;
    let after = preview.as_ref().ok().map(|s| numbers(s));
    for (i, (label, value)) in numbers(spec).into_iter().enumerate() {
        let next = after.as_ref().map(|a| a[i].1.clone()).unwrap_or_default();
        let col = if next.is_empty() || next == value { TEXT } else { BETTER };
        frame.text(Vec2::new(12.0, yy), &format!("{label:<12} {value:>22} {next:>22}"), col);
        yy += line;
    }
    yy += line * 0.5;
    match &preview {
        _ if pick == now => frame.text(Vec2::new(12.0, yy), "FITTED NOW", DIM),
        Ok(_) => frame.text(Vec2::new(12.0, yy), &format!("{} {:.0} CR (THE ONE TAKEN OUT FETCHES {:.0}%)", if cost >= 0.0 { "COSTS" } else { "PAYS" }, cost.abs(), universe_sim::BUYBACK * 100.0), if cost <= app.v.credits { SELECT } else { RED }),
        Err(why) => frame.text(Vec2::new(12.0, yy), &format!("WON'T DO - {}", why.to_uppercase()), RED),
    };
}
