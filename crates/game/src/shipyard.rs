//! The shipyard (docked at a station): the ship's slots and what's fitted,
//! what else would go in the slot picked, and what the ship would be with
//! it — mass, power, thrust, tank and hold, how it turns, the autopilots it
//! runs — before it's bought. The first piece of the ship planner.
//!
//! Two pages (TAB): OUTFIT — ↑/↓ slot, ←/→ module, ENTER fit it; HULLS — ↑/↓
//! hull, ENTER twice buy it (the ship traded in). The shipyard key or ESC close.

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
    /// On the hulls page: which, and whether ENTER's been pressed once (to buy it).
    hulls: bool,
    hull: usize,
    armed: bool,
}

/// How this station stands to module `m`: carried, at what price, and
/// whether there's stock to build it (None: not carried here).
fn local(app: &App, m: &Module) -> (universe_sim::services::outfitter::Offer, bool) {
    use universe_sim::services::outfitter;
    let here = app.v.docked_market.unwrap_or(Facility::Station(0));
    let settled = outfitter::settled(&app.v.economy);
    let o = outfitter::offer(app.charts.seed, &app.charts.gate_links, &settled, app.v.ship_system, here, m);
    let (kind, tonnes) = outfitter::materials(m);
    let stocked = app.v.economy.iter().find(|p| p.system == app.v.ship_system && p.facility == here).is_none_or(|p| p.stock_of(kind) >= tonnes);
    (o, stocked)
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
        Shipyard { slot, choice, ..Default::default() }
    }

    /// (Dev scenarios: the hulls page, hull `hull` picked.)
    pub fn showing_hulls(hull: usize) -> Self {
        Shipyard { hulls: true, hull, ..Default::default() }
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
    if input.pressed(KeyCode::Tab) {
        y.hulls = !y.hulls;
        y.armed = false;
        return true;
    }
    if y.hulls {
        let n = content().hulls.len().max(1);
        if input.pressed(KeyCode::ArrowDown) {
            y.hull = (y.hull + 1) % n;
            y.armed = false;
        }
        if input.pressed(KeyCode::ArrowUp) {
            y.hull = (y.hull + n - 1) % n;
            y.armed = false;
        }
        if input.pressed(KeyCode::Enter) {
            if y.armed {
                let key = content().hulls.iter().nth(y.hull).map(|(_, h)| h.key.clone()).unwrap_or_default();
                y.armed = false;
                app.engine.send(Command::BuyHull { hull: key });
            } else {
                y.armed = true;
            }
        }
        return true;
    }
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
        let pick = offers(slot).get(y.choice).copied().flatten();
        match pick.map(|h| (h, local(app, content().get(h)))) {
            Some((_, (o, _))) if !o.carried => app.say("NOT CARRIED HERE".into()),
            Some((_, (_, false))) => app.say("OUT OF STOCK TO BUILD IT".into()),
            _ => app.engine.send(Command::Refit { slot: slot.name.clone(), module: pick.map(|h| content().get(h).key.clone()) }),
        }
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
        ("AUTOPILOTS", if s.features.is_empty() { "NONE".into() } else { s.features.iter().map(|f| format!("{f:?}").to_uppercase().chars().take(4).collect::<String>()).collect::<Vec<_>>().join(" ") }),
    ]
}

/// A hull's price here (frame and stock fit at this station's prices), and
/// what the ship flown now fetches in trade.
fn hull_price(app: &App, h: &ClassSpec) -> (f64, f64) {
    let c = content();
    let price = h.frame.price + h.fit.iter().map(|(_, m)| local(app, c.get(*m)).0.price).sum::<f64>();
    let old = app.ship.spec();
    let trade = universe_sim::BUYBACK * (old.frame.price + old.fit.iter().map(|(_, m)| c.get(*m).price).sum::<f64>());
    (price, trade)
}

/// The hulls page: each hull's numbers and price, the one picked in full.
fn draw_hulls(frame: &mut Frame, app: &App, y: &Shipyard) {
    let line = 12.0;
    let top = 12.0 + line * 2.0;
    frame.text(Vec2::new(12.0, top), &format!("{:<18} {:>7} {:>7} {:>7} {:>8} {:>8} {:>15} {:>10}", "HULL", "DRY", "TANK", "HOLD", "MAIN", "LIFT", "TURNS", "PRICE"), DIM);
    let mine = app.ship.class;
    for (k, (h, s)) in content().hulls.iter().enumerate() {
        let here = k == y.hull;
        let loaded = s.dry_mass + s.fuel_capacity;
        let (price, _) = hull_price(app, s);
        let mark = if here { ">" } else if h == mine { "*" } else { " " };
        let text = format!(
            "{mark}{:<17} {:>7} {:>7} {:>7} {:>8} {:>8} {:>15} {:>10}",
            s.name,
            fmt::tonnes(s.dry_mass),
            fmt::tonnes(s.fuel_capacity),
            fmt::tonnes(s.hold_capacity),
            format!("{:.0} M/S2", s.main_thrust / loaded),
            format!("{:.0} M/S2", s.lift_thrust / loaded),
            format!("{:.1}/{:.1}/{:.1}", s.turn_accel.x, s.turn_accel.y, s.turn_accel.z),
            format!("{:.0} CR", price)
        );
        frame.text(Vec2::new(12.0, top + (2 + k) as f32 * line), &text, if here { SELECT } else if h == mine { TEXT } else { DIM });
    }
    let Some((h, s)) = content().hulls.iter().nth(y.hull) else { return };
    let mut yy = top + (3 + content().hulls.len()) as f32 * line;
    let (price, trade) = hull_price(app, s);
    let loaded_full = s.dry_mass + s.fuel_capacity + s.hold_capacity;
    frame.text(Vec2::new(12.0, yy), &format!("{} - {} SLOTS. STOCK FIT:", s.name, s.slots.len()), TEXT);
    yy += line;
    let names: Vec<String> = s.fit.iter().map(|(_, m)| content().get(*m).name.clone()).collect();
    for chunk in names.chunks(5) {
        frame.text(Vec2::new(24.0, yy), &chunk.join(", "), DIM);
        yy += line;
    }
    frame.text(Vec2::new(12.0, yy), &format!("FULL HOLD: LIFT {:.0} M/S2 (1 G IS 9.8)  AUTOPILOTS {}", s.lift_thrust / loaded_full, s.features.iter().map(|f| format!("{f:?}").to_uppercase().chars().take(4).collect::<String>()).collect::<Vec<_>>().join(" ")), DIM);
    yy += line * 1.5;
    if h == app.ship.class {
        frame.text(Vec2::new(12.0, yy), "THE SHIP YOU FLY", DIM);
        return;
    }
    let cost = price - trade;
    let text = format!("{price:.0} CR, LESS {trade:.0} FOR YOUR SHIP: {} {:.0} CR", if cost >= 0.0 { "COSTS" } else { "PAYS" }, cost.abs());
    frame.text(Vec2::new(12.0, yy), &text, if cost <= app.v.credits { SELECT } else { RED });
    yy += line;
    if y.armed {
        frame.text(Vec2::new(12.0, yy), "ENTER AGAIN TO BUY IT", SELECT);
    } else {
        frame.text(Vec2::new(12.0, yy), "ENTER TWICE TO BUY IT", DIM);
    }
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
        &format!(
            "SHIPYARD - {} - {}   CREDITS {:.0}   [TAB] {}   ({} CLOSES)",
            station(app).unwrap_or_default(),
            spec.name,
            app.v.credits,
            if y.hulls { "HULLS: UP/DOWN, ENTER TWICE BUY" } else { "OUTFIT: UP/DOWN SLOT, LEFT/RIGHT MODULE, ENTER FIT" },
            key(Act::Shipyard)
        ),
        TEXT,
    );
    if y.hulls {
        return draw_hulls(frame, app, y);
    }
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
        let (text, sold) = match o.map(|h| c.get(h)) {
            Some(m) => {
                let (offer, stocked) = local(app, m);
                let price = if !offer.carried { "-".to_string() } else if !stocked { "NO STOCK".to_string() } else { format!("{} CR", offer.price as i64) };
                (format!("{mark}{:<20} {:>7} {:>8} {:>10}", m.name, fmt::tonnes(m.mass), format!("{:.2} MW", m.power / 1e6), price), (offer.carried && stocked) || *o == now)
            }
            None => (format!("{mark}(EMPTY)"), true),
        };
        let col = if here { SELECT } else if *o == now { TEXT } else if sold { DIM } else { DIM.scale(0.6) };
        frame.text(Vec2::new(x, top + (2 + k) as f32 * line), &text, col);
    }
    // The module picked: who makes it, what it does, whether it's sold here.
    let pick = list.get(y.choice).copied().flatten();
    if let Some(m) = pick.map(|h| c.get(h)) {
        let (offer, stocked) = local(app, m);
        let brand = content().handle::<universe_sim::world::modules::Brand>(&m.brand).map(|b| content().get(b));
        let (maker, note) = brand.map_or(("UNBRANDED".to_string(), "MADE ANYWHERE".to_string()), |b| (b.name.clone(), b.note.clone()));
        let state = if !offer.carried { "NOT CARRIED HERE".to_string() } else if !stocked { "OUT OF STOCK TO BUILD IT".to_string() } else if offer.hops > 0 { format!("{} GATES FROM ITS MAKER'S HOME (+{:.0}%)", offer.hops, offer.hops as f64 * universe_sim::services::outfitter::MARKUP_PER_HOP * 100.0) } else { "MADE HERE".into() };
        let yy = top + (3 + list.len()) as f32 * line;
        frame.text(Vec2::new(x, yy), &maker, TEXT);
        frame.text(Vec2::new(x, yy + line), &note, DIM);
        frame.text(Vec2::new(x, yy + 2.0 * line), &what(m), TEXT);
        frame.text(Vec2::new(x, yy + 3.0 * line), &state, if offer.carried && stocked { DIM } else { RED });
    }
    // The ship as it is, and as it would be.
    let preview = fitted(app.ship.class, &with(&fit, slot, pick));
    let cost = pick.map_or(0.0, |h| local(app, c.get(h)).0.price) - now.map_or(0.0, |h| c.get(h).price * universe_sim::BUYBACK);
    let mut yy = top + (spec.slots.len().max(list.len() + 6) + 2) as f32 * line;
    frame.text(Vec2::new(12.0, yy), &format!("{:<12} {:>30} {:>30}", "", "NOW", "WITH IT"), DIM);
    yy += line;
    let after = preview.as_ref().ok().map(|s| numbers(s));
    for (i, (label, value)) in numbers(spec).into_iter().enumerate() {
        let next = after.as_ref().map(|a| a[i].1.clone()).unwrap_or_default();
        let col = if next.is_empty() || next == value { TEXT } else { BETTER };
        frame.text(Vec2::new(12.0, yy), &format!("{label:<12} {value:>30} {next:>30}"), col);
        yy += line;
    }
    yy += line * 0.5;
    match &preview {
        _ if pick == now => frame.text(Vec2::new(12.0, yy), "FITTED NOW", DIM),
        Ok(_) => frame.text(Vec2::new(12.0, yy), &format!("{} {:.0} CR (THE ONE TAKEN OUT FETCHES {:.0}%)", if cost >= 0.0 { "COSTS" } else { "PAYS" }, cost.abs(), universe_sim::BUYBACK * 100.0), if cost <= app.v.credits { SELECT } else { RED }),
        Err(why) => frame.text(Vec2::new(12.0, yy), &format!("WON'T DO - {}", why.to_uppercase()), RED),
    };
}
