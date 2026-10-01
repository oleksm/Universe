//! The ship planner, and the shipyard. Open anywhere (the shipyard key):
//! a plan — a hull, and a module in each of its slots, from the whole
//! catalogue — set against the ship flown now, number by number (mass,
//! power, the pushes as accelerations, lift against a g full and empty,
//! the turn, the balance, how long the tank lasts at full drive and
//! hovering, the price), and drawn: where its modules sit, its thrusters,
//! its centre of mass. Plans are kept (in the save). Docked at a station,
//! it's the shipyard: what's carried here and at what price, and the plan
//! built here — the hull bought if it's another, then each slot refitted.
//!
//! Pages (TAB): PLAN — ↑/↓ slot, ←/→ module, ENTER put it in the plan,
//! SHIFT+ENTER (docked, twice) build the plan here; HULLS — ↑/↓, ENTER plan
//! from that hull; PLANS — ↑/↓, ENTER load, DELETE drop, the first row
//! keeps the plan as it is. The shipyard key or ESC close.

use serde::{Deserialize, Serialize};
use universe_engine::glam::{DVec3, Vec2};
use universe_engine::{Color, Context, Frame, KeyCode};
use universe_sim::world::content::{content, Handle};
use universe_sim::world::modules::{Does, Module, BASE_BLOCKS};
use universe_sim::world::ship::{fitted, ClassSpec, Fit, Hull, Slot, EXHAUST_VELOCITY};
use universe_sim::world::Facility;
use universe_sim::Command;

use crate::keys::{key, Act};
use crate::{fmt, App};

const TEXT: Color = Color::hex(0x30ff60);
const DIM: Color = Color::hex(0x178a38);
const SELECT: Color = Color::hex(0xffc040);
const RED: Color = Color::hex(0xff4040);
const BETTER: Color = Color::hex(0x60ffd0);
const LINE: f32 = 12.0;
/// A standard g (m/s²), to set lift against.
const G: f64 = 9.81;

/// A plan as kept: its hull and what's in each slot, by content key (so it
/// keeps across content changes, as far as its parts do).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SavedPlan {
    pub hull: String,
    pub fit: Vec<(String, String)>,
}

impl SavedPlan {
    fn of(hull: Hull, fit: &Fit) -> Self {
        let c = content();
        SavedPlan { hull: c.get(hull).key.clone(), fit: fit.iter().map(|(s, m)| (s.clone(), c.get(*m).key.clone())).collect() }
    }

    /// As a plan again (None: its hull is gone; parts gone are left out).
    fn load(&self) -> Option<(Hull, Fit)> {
        let c = content();
        let hull = c.handle::<ClassSpec>(&self.hull)?;
        Some((hull, self.fit.iter().filter_map(|(s, m)| c.handle::<Module>(m).map(|h| (s.clone(), h))).collect()))
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum Page {
    #[default]
    Plan,
    Hulls,
    Plans,
    Design,
}

/// The panel: the plan being worked on, and the cursors.
pub struct Shipyard {
    page: Page,
    hull: Hull,
    fit: Fit,
    slot: usize,
    choice: usize,
    held: f32,
    /// Hulls page, plans page cursors.
    hull_pick: usize,
    plan_pick: usize,
    /// SHIFT+ENTER pressed once: again builds it.
    armed: bool,
    /// The design page's cursor: a knob, or (past them) commissioning.
    knob: usize,
    held_side: f32,
}

/// How this station stands to module `m`: carried, at what price, and
/// whether there's stock to build it.
fn local(app: &App, m: &Module) -> (universe_sim::services::outfitter::Offer, bool) {
    use universe_sim::services::outfitter;
    let here = app.v.docked_market.unwrap_or(Facility::Station(0));
    let settled = outfitter::settled(&app.v.economy);
    let o = outfitter::offer(app.charts.seed, &app.charts.gate_links, &settled, app.v.ship_system, here, m);
    let (kind, tonnes) = outfitter::materials(m);
    let stocked = app.v.economy.iter().find(|p| p.system == app.v.ship_system && p.facility == here).is_none_or(|p| p.stock_of(kind) >= tonnes);
    (o, stocked)
}

/// Docked at a station: its name (it's a shipyard then).
fn station(app: &App) -> Option<String> {
    match app.v.docked_market {
        Some(f @ Facility::Station(_)) if matches!(app.v.ship.state, universe_sim::ShipState::Landed { .. }) => Some(f.name(&app.view.system).to_uppercase()),
        _ => None,
    }
}

/// What would go in `slot`: every module of its kind that fits it, and
/// (unless it's a base block) nothing.
fn offers(slot: &Slot) -> Vec<Option<Handle<Module>>> {
    let mut out: Vec<Option<Handle<Module>>> = content().modules.iter().filter(|(_, m)| m.does.slot() == slot.kind && m.size <= slot.size).map(|(h, _)| Some(h)).collect();
    // Smallest first, then cheapest.
    out.sort_by(|a, b| {
        let (a, b) = (content().get(a.expect("a module")), content().get(b.expect("a module")));
        (a.size, a.price as i64).cmp(&(b.size, b.price as i64))
    });
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

/// The fit with `module` in `slot`.
fn with(fit: &Fit, slot: &Slot, module: Option<Handle<Module>>) -> Fit {
    let mut f: Fit = fit.iter().filter(|(s, _)| *s != slot.name).cloned().collect();
    if let Some(m) = module {
        f.push((slot.name.clone(), m));
    }
    f
}

impl Shipyard {
    /// The plan: the ship flown now.
    fn new(app: &App) -> Self {
        let mut y = Shipyard { page: Page::Plan, hull: app.ship.class, fit: fit_of(app), slot: 0, choice: 0, held: 0.0, hull_pick: 0, plan_pick: 0, armed: false, knob: 0, held_side: 0.0 };
        y.choice = y.current_choice(0);
        y
    }

    /// (Dev scenarios: open on slot `slot`, module `choice` picked.)
    pub fn showing(app: &App, slot: usize, choice: usize) -> Self {
        let y = Shipyard::new(app);
        let n = y.spec().slots.get(slot).map_or(1, |s| offers(s).len());
        Shipyard { slot, choice: choice.min(n.saturating_sub(1)), ..y }
    }

    /// (Dev scenarios: a plan of hull `hull` as sold, slot `slot` swapped for module `module`.)
    pub fn planning(app: &App, hull: &str, slot: &str, module: &str) -> Option<Self> {
        let c = content();
        let h = c.handle::<ClassSpec>(hull)?;
        let s = c.get(h).slots.iter().find(|s| s.name == slot)?.clone();
        let fit = with(&c.get(h).fit, &s, c.handle::<Module>(module));
        let mut y = Shipyard { hull: h, fit, ..Shipyard::new(app) };
        y.choice = y.current_choice(0);
        Some(y)
    }

    /// (Dev scenarios: the design page, knob `knob` picked.)
    pub fn designing(app: &App, knob: usize) -> Self {
        Shipyard { page: Page::Design, knob, ..Shipyard::new(app) }
    }

    /// (Dev scenarios: the hulls page, hull `hull` picked.)
    pub fn showing_hulls(app: &App, hull: usize) -> Self {
        Shipyard { page: Page::Hulls, hull_pick: hull, ..Shipyard::new(app) }
    }

    fn spec(&self) -> &'static ClassSpec {
        content().get(self.hull)
    }

    /// Where the module the plan has in slot `k` stands among its offers.
    fn current_choice(&self, k: usize) -> usize {
        let Some(slot) = self.spec().slots.get(k) else { return 0 };
        let now = in_slot(&self.fit, slot);
        offers(slot).iter().position(|o| *o == now).unwrap_or(0)
    }

    /// The plan, as a ship: its numbers (Err: it won't go together, why).
    fn planned(&self) -> Result<&'static ClassSpec, String> {
        fitted(self.hull, &self.fit)
    }
}

/// Open it: the plan starts as the ship flown now.
pub fn open(app: &mut App) -> Option<Shipyard> {
    Some(Shipyard::new(app))
}

/// What building the plan here takes: the credits (the hull bought, less
/// the ship traded in, if it's another; each module that's different, less
/// what the one taken out fetches), and the parts this station can't
/// supply.
fn build_cost(app: &App, y: &Shipyard) -> (f64, Vec<String>) {
    let c = content();
    let mut cost = 0.0;
    let mut missing = Vec::new();
    // (Another hull: bought with its stock fit, the ship traded in; the plan refits from there.)
    let (from, start): (Hull, Fit) = if y.hull != app.ship.class {
        let s = y.spec();
        let old = app.ship.spec();
        cost += s.frame.price + s.fit.iter().map(|(_, m)| local(app, c.get(*m)).0.price).sum::<f64>();
        cost -= universe_sim::BUYBACK * (old.frame.price + old.fit.iter().map(|(_, m)| c.get(*m).price).sum::<f64>());
        (y.hull, s.fit.clone())
    } else {
        (app.ship.class, fit_of(app))
    };
    for slot in &c.get(from).slots {
        let (was, want) = (in_slot(&start, slot), in_slot(&y.fit, slot));
        if was == want {
            continue;
        }
        if let Some(m) = want.map(|h| c.get(h)) {
            let (offer, stocked) = local(app, m);
            if !offer.carried || !stocked {
                missing.push(m.name.clone());
                continue;
            }
            cost += offer.price;
        }
        if let Some(m) = was.map(|h| c.get(h)) {
            cost -= m.price * universe_sim::BUYBACK;
        }
    }
    (cost, missing)
}

/// Build the plan here: the hull bought (if it's another), then each slot
/// refitted to it (in order: the engine takes them as sent).
pub fn build(app: &mut App) {
    let Some(y) = &app.shipyard else { return };
    let c = content();
    let (hull, fit) = (y.hull, y.fit.clone());
    let start = if hull != app.ship.class {
        app.engine.send(Command::BuyHull { hull: c.get(hull).key.clone() });
        c.get(hull).fit.clone()
    } else {
        fit_of(app)
    };
    for slot in &c.get(hull).slots {
        let want = in_slot(&fit, slot);
        if in_slot(&start, slot) != want {
            app.engine.send(Command::Refit { slot: slot.name.clone(), module: want.map(|h| c.get(h).key.clone()) });
        }
    }
}

/// Keys while open. False when it should close.
pub fn input(app: &mut App, ctx: &Context) -> bool {
    let input = &ctx.input;
    if crate::keys::pressed(input, Act::Shipyard) || input.pressed(KeyCode::Escape) {
        return false;
    }
    let docked = station(app).is_some();
    let shift = input.down(KeyCode::ShiftLeft) || input.down(KeyCode::ShiftRight);
    let plans = app.plans.len();
    let Some(y) = &mut app.shipyard else { return false };
    if input.pressed(KeyCode::Tab) {
        y.page = match y.page {
            Page::Plan => Page::Hulls,
            Page::Hulls => Page::Design,
            Page::Design => Page::Plans,
            Page::Plans => Page::Plan,
        };
        y.armed = false;
        return true;
    }
    let (down, up) = (input.down(KeyCode::ArrowDown), input.down(KeyCode::ArrowUp));
    let steps = crate::navmap::repeat(&mut y.held, down || up, input.pressed(KeyCode::ArrowDown) || input.pressed(KeyCode::ArrowUp), ctx.dt);
    let step = |k: &mut usize, n: usize| {
        let n = n.max(1);
        for _ in 0..steps {
            *k = if down { (*k + 1) % n } else { (*k + n - 1) % n };
        }
    };
    match y.page {
        Page::Design => {
            use universe_sim::world::design::KNOBS;
            step(&mut y.knob, KNOBS.len() + 1);
            let (right, left) = (input.down(KeyCode::ArrowRight), input.down(KeyCode::ArrowLeft));
            let turns = crate::navmap::repeat(&mut y.held_side, right || left, input.pressed(KeyCode::ArrowRight) || input.pressed(KeyCode::ArrowLeft), ctx.dt);
            let k = y.knob;
            if turns > 0 && k < KNOBS.len() {
                let by = if right { 1.0 } else { -1.0 } * turns as f64 * if shift { 5.0 } else { 1.0 };
                app.design.turn(k, by);
            }
            if input.pressed(KeyCode::Enter) && k == KNOBS.len() {
                // Commissioned: a hull for good (numbered among yours), planned from at once.
                let mut d = app.design.clone();
                d.name = format!("DESIGN {}", app.designs.len() + 1);
                match d.commission() {
                    Ok(h) => {
                        if !app.designs.contains(&d) {
                            app.designs.push(d);
                        }
                        app.say(format!("{} COMMISSIONED - PLAN IT, BUILD IT AT A SHIPYARD", content().get(h).name));
                        if let Some(y) = &mut app.shipyard {
                            (y.hull, y.fit, y.slot, y.page) = (h, content().get(h).fit.clone(), 0, Page::Plan);
                            y.choice = y.current_choice(0);
                        }
                    }
                    Err(why) => app.say(format!("WON'T GO TOGETHER - {}", why.to_uppercase())),
                }
            }
        }
        Page::Hulls => {
            step(&mut y.hull_pick, content().hulls.len());
            if input.pressed(KeyCode::Enter)
                && shift
                && let Some((_, s)) = content().hulls.iter().nth(y.hull_pick)
            {
                // A copy to change, on the design board.
                y.page = Page::Design;
                app.design = universe_sim::world::design::Design::after(s);
                app.say(format!("{} ON THE DESIGN BOARD - CHANGE IT, THEN COMMISSION IT", app.design.name));
            } else if input.pressed(KeyCode::Enter)
                && let Some((h, s)) = content().hulls.iter().nth(y.hull_pick)
            {
                // A plan from that hull, as it's sold.
                (y.hull, y.fit, y.slot, y.page) = (h, s.fit.clone(), 0, Page::Plan);
                y.choice = y.current_choice(0);
            }
        }
        Page::Plans => {
            step(&mut y.plan_pick, plans + 1);
            let pick = y.plan_pick;
            if input.pressed(KeyCode::Enter) {
                if pick == 0 {
                    let kept = SavedPlan::of(y.hull, &y.fit);
                    app.plans.push(kept);
                    app.say(format!("PLAN {} KEPT", app.plans.len()));
                } else if let Some((h, f)) = app.plans.get(pick - 1).and_then(|p| p.load()) {
                    let Some(y) = &mut app.shipyard else { return false };
                    (y.hull, y.fit, y.slot, y.page) = (h, f, 0, Page::Plan);
                    y.choice = y.current_choice(0);
                }
            } else if input.pressed(KeyCode::Delete) && pick > 0 && pick <= plans {
                app.plans.remove(pick - 1);
                if let Some(y) = &mut app.shipyard {
                    y.plan_pick -= 1;
                }
            }
        }
        Page::Plan => {
            let slots = y.spec().slots.len();
            let was = y.slot;
            step(&mut y.slot, slots);
            if y.slot != was {
                y.choice = y.current_choice(y.slot);
                y.armed = false;
                return true;
            }
            let Some(slot) = y.spec().slots.get(y.slot).cloned() else { return true };
            let n = offers(&slot).len().max(1);
            y.choice = y.choice.min(n - 1);
            if input.pressed(KeyCode::ArrowRight) {
                y.choice = (y.choice + 1) % n;
            }
            if input.pressed(KeyCode::ArrowLeft) {
                y.choice = (y.choice + n - 1) % n;
            }
            if input.pressed(KeyCode::Enter) && shift {
                // Build it here: docked at a station, twice to be sure.
                if !docked {
                    app.say("BUILD A PLAN DOCKED AT A STATION".into());
                } else if y.planned().is_err() {
                    app.say("THE PLAN WON'T GO TOGETHER".into());
                } else if !y.armed {
                    y.armed = true;
                } else {
                    y.armed = false;
                    build(app);
                }
            } else if input.pressed(KeyCode::Enter) {
                let pick = offers(&slot).get(y.choice).copied().flatten();
                y.fit = with(&y.fit, &slot, pick);
                y.armed = false;
            }
        }
    }
    true
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

/// A ship's numbers, a line each: (label, value).
fn numbers(s: &'static ClassSpec) -> Vec<(&'static str, String)> {
    let c = content();
    let loaded = s.dry_mass + s.fuel_capacity;
    let full = loaded + s.hold_capacity;
    // What its lift and drive give without turning it, a full tank aboard, with the hold empty and full.
    let (a_empty, a_full) = (s.authority(s.fuel_capacity, 0.0), s.authority(s.fuel_capacity, s.hold_capacity));
    let share = |a: f64, b: f64| if b > 0.0 { format!("{:.0}", 100.0 * a / b) } else { "-".into() };
    // How long the tank lasts: the drive at full, and hovering at a g (loaded).
    let hours = |flow: f64| if flow > 0.0 { fmt::duration(s.fuel_capacity / flow) } else { "-".into() };
    let price = s.frame.price + s.fit.iter().map(|(_, m)| c.get(*m).price).sum::<f64>();
    vec![
        ("HULL", s.name.clone()),
        ("DRY MASS", fmt::tonnes(s.dry_mass)),
        ("TANK / HOLD", format!("{} / {}", fmt::tonnes(s.fuel_capacity), fmt::tonnes(s.hold_capacity))),
        ("POWER", format!("{:.1} OF {:.1} MW", s.power_draw / 1e6, s.power_output / 1e6)),
        ("MAIN DRIVE", format!("{:.1} M/S2", a_empty.main / loaded)),
        ("THRUSTERS", format!("{:.1} M/S2", a_empty.side / loaded)),
        ("LIFT", format!("{:.1} G / {:.1} G FULL", a_empty.lift / loaded / G, a_full.lift / full / G)),
        ("TURNS", format!("{:.1} {:.1} {:.1} RAD/S2", s.turn_accel.x, s.turn_accel.y, s.turn_accel.z)),
        // (Lift, then drive: empty hold / full hold.)
        ("BALANCE", format!("L {}/{}% D {}/{}%", share(a_empty.lift, s.lift_thrust), share(a_full.lift, s.lift_thrust), share(a_empty.main, s.main_thrust), share(a_full.main, s.main_thrust))),
        ("FULL DRIVE", format!("{} A TANK", hours(s.main_thrust / EXHAUST_VELOCITY))),
        ("HOVER 1 G", format!("{} A TANK", hours(loaded * G / EXHAUST_VELOCITY))),
        ("AUTOPILOTS", if s.features.is_empty() { "NONE".into() } else { s.features.iter().map(|f| format!("{f:?}").to_uppercase().chars().take(3).collect::<String>()).collect::<Vec<_>>().join(" ") }),
        ("LIST PRICE", format!("{price:.0} CR")),
    ]
}

/// A hull's price here (frame and stock fit at this station's prices).
fn hull_price(app: &App, h: &ClassSpec) -> f64 {
    let c = content();
    h.frame.price + h.fit.iter().map(|(_, m)| local(app, c.get(*m)).0.price).sum::<f64>()
}

/// The hulls page: each hull's numbers and price; ENTER plans from it.
fn draw_hulls(frame: &mut Frame, app: &App, y: &Shipyard) {
    let top = 12.0 + LINE * 2.0;
    frame.text(Vec2::new(12.0, top), &format!("{:<18} {:>7} {:>7} {:>7} {:>8} {:>8} {:>15} {:>10}", "HULL", "DRY", "TANK", "HOLD", "MAIN", "LIFT", "TURNS", "PRICE"), DIM);
    let mine = app.ship.class;
    for (k, (h, s)) in content().hulls.iter().enumerate() {
        let here = k == y.hull_pick;
        let loaded = s.dry_mass + s.fuel_capacity;
        let mark = if here { ">" } else if h == mine { "*" } else { " " };
        // (Yours: designed and commissioned.)
        let name = if s.key.starts_with("design.") { format!("{} (YOURS)", s.name) } else { s.name.clone() };
        let text = format!(
            "{mark}{:<17} {:>7} {:>7} {:>7} {:>8} {:>8} {:>15} {:>10}",
            name.chars().take(17).collect::<String>(),
            fmt::tonnes(s.dry_mass),
            fmt::tonnes(s.fuel_capacity),
            fmt::tonnes(s.hold_capacity),
            format!("{:.0} M/S2", s.main_thrust / loaded),
            format!("{:.1} G", s.lift_thrust / loaded / G),
            format!("{:.1}/{:.1}/{:.1}", s.turn_accel.x, s.turn_accel.y, s.turn_accel.z),
            format!("{:.0} CR", hull_price(app, s))
        );
        frame.text(Vec2::new(12.0, top + (2 + k) as f32 * LINE), &text, if here { SELECT } else if h == mine { TEXT } else { DIM });
    }
    let Some((_, s)) = content().hulls.iter().nth(y.hull_pick) else { return };
    let mut yy = top + (3 + content().hulls.len()) as f32 * LINE;
    frame.text(Vec2::new(12.0, yy), &format!("{} - {} SLOTS. AS SOLD:", s.name, s.slots.len()), TEXT);
    yy += LINE;
    let names: Vec<String> = s.fit.iter().map(|(_, m)| content().get(*m).name.clone()).collect();
    for chunk in names.chunks(5) {
        frame.text(Vec2::new(24.0, yy), &chunk.join(", "), DIM);
        yy += LINE;
    }
    let picture = crate::thrusterpanel::Picture { spec: s, jets: &[], com: s.centre_of_mass(s.fuel_capacity, 0.0), mounts: true, picked: None };
    let size = frame.size();
    let w = ((size.x - 36.0) / 2.0).min(260.0);
    let at = Vec2::new(12.0, yy + LINE);
    let h = (size.y - at.y - 12.0 - 4.5 * LINE).max(80.0);
    crate::thrusterpanel::turning(frame, &picture, at, Vec2::new(w, h), app.v.time * 0.4, "");
    crate::thrusterpanel::view(frame, &picture, at + Vec2::new(w + 12.0, 0.0), Vec2::new(w, h), DVec3::X, DVec3::NEG_Z, "FROM ABOVE");
}

/// The plans page: the plan kept as it is, or one kept before.
fn draw_plans(frame: &mut Frame, app: &App, y: &Shipyard) {
    let top = 12.0 + LINE * 2.0;
    let c = content();
    let first = format!("+ KEEP THIS PLAN ({})", y.spec().name);
    frame.text(Vec2::new(12.0, top), &format!("{}{first}", if y.plan_pick == 0 { ">" } else { " " }), if y.plan_pick == 0 { SELECT } else { TEXT });
    for (k, p) in app.plans.iter().enumerate() {
        let here = y.plan_pick == k + 1;
        let text = match p.load().and_then(|(h, f)| fitted(h, &f).ok().map(|s| (c.get(h), s))) {
            Some((hull, s)) => {
                let loaded = s.dry_mass + s.fuel_capacity;
                format!("PLAN {:<3} {:<16} DRY {:>7}  HOLD {:>7}  MAIN {:>5.1} M/S2  LIFT {:.1} G", k + 1, hull.name, fmt::tonnes(s.dry_mass), fmt::tonnes(s.hold_capacity), s.main_thrust / loaded, s.lift_thrust / loaded / G)
            }
            None => format!("PLAN {:<3} {} (WON'T GO TOGETHER NOW)", k + 1, p.hull.to_uppercase()),
        };
        frame.text(Vec2::new(12.0, top + (k + 2) as f32 * LINE), &format!("{}{text}", if here { ">" } else { " " }), if here { SELECT } else { DIM });
    }
}

/// The design page: the numbers a hull is drawn up from, its own numbers
/// (and what's wrong with it), and it drawn; commissioning it at the end.
fn draw_design(frame: &mut Frame, app: &App, y: &Shipyard) {
    use universe_sim::world::design::KNOBS;
    let top = 12.0 + LINE * 2.0;
    let d = &app.design;
    frame.text(Vec2::new(12.0, top - LINE), "A HULL OF YOUR OWN: WHERE THE MASS SITS AND WHERE THE THRUSTERS PUSH ARE YOURS TO BALANCE", DIM);
    for (k, knob) in KNOBS.iter().enumerate() {
        let here = k == y.knob;
        let v = d.knob(k);
        let value = if knob.step >= 1.0 { format!("{v:.0}") } else { format!("{v:+.2}") };
        let value = if knob.label == "FINS" { if v > 0.5 { "YES".into() } else { "NO".into() } } else { value };
        frame.text(Vec2::new(12.0, top + k as f32 * LINE), &format!("{}{:<16} {:>8}", if here { ">" } else { " " }, knob.label, value), if here { SELECT } else { TEXT });
    }
    let last = KNOBS.len();
    let here = y.knob == last;
    frame.text(Vec2::new(12.0, top + (last as f32 + 0.5) * LINE), &format!("{}COMMISSION IT", if here { ">" } else { " " }), if here { SELECT } else { BETTER });
    // Its numbers, and what's wrong.
    let x = 12.0 + 30.0 * 7.5;
    match d.spec() {
        Ok(s) => {
            let mut yy = top;
            for (label, value) in numbers(s) {
                frame.text(Vec2::new(x, yy), &format!("{label:<11} {value:>23}"), TEXT);
                yy += LINE;
            }
            frame.text(Vec2::new(x, yy), &format!("{:<11} {:>23}", "FRAME", format!("{} {:.0} CR", fmt::tonnes(s.frame.frame_mass), s.frame.price)), DIM);
            yy += LINE * 1.5;
            let loaded = s.dry_mass + s.fuel_capacity;
            let (a, full) = (s.authority(s.fuel_capacity, s.hold_capacity / 2.0), s.authority(s.fuel_capacity, s.hold_capacity));
            let mut warn = |t: String| {
                frame.text(Vec2::new(x, yy), &t, RED);
                yy += LINE;
            };
            if full.lift / (loaded + s.hold_capacity) < G {
                warn("CAN'T HOVER AT 1 G LOADED (SPACE ONLY)".into());
            }
            for (what, got, rated) in [("LIFT", a.lift, s.lift_thrust), ("DRIVE", a.main, s.main_thrust), ("THRUSTERS", a.side, s.rcs_thrust)] {
                if got < 0.85 * rated {
                    warn(format!("OFF BALANCE: {what} KEEPS {:.0}%", 100.0 * got / rated.max(1.0)));
                }
            }
            let at = Vec2::new(x + universe_engine::text_size(&format!("{:<11} {:>23}", "", "")).x + 16.0, top);
            let w = ((frame.size().x - at.x - 20.0) / 2.0).max(100.0);
            let h = (frame.size().y - at.y - 12.0 - 4.5 * LINE).max(80.0);
            let picture = crate::thrusterpanel::Picture { spec: s, jets: &[], com: s.centre_of_mass(s.fuel_capacity, s.hold_capacity / 2.0), mounts: true, picked: None };
            crate::thrusterpanel::view(frame, &picture, at, Vec2::new(w, h), DVec3::X, DVec3::NEG_Z, "FROM ABOVE");
            crate::thrusterpanel::turning(frame, &picture, at + Vec2::new(w + 8.0, 0.0), Vec2::new(w, h), app.v.time * 0.4, "");
        }
        Err(why) => {
            frame.text(Vec2::new(x, top), &format!("WON'T GO TOGETHER - {}", why.to_uppercase()), RED);
        }
    }
}

/// The page's actions, as a panel of cells along the bottom: each with its
/// key and a lamp (lit when it would do something, dim when it can't).
fn actions(frame: &mut Frame, app: &App, y: &Shipyard) {
    use crate::hud::{draw_panel, Lamp};
    let docked = station(app).is_some();
    let can = |ok: bool| if ok { Lamp::Off } else { Lamp::Unavailable };
    let cell = |k: &str, name: &str, lamp: Lamp| (k.to_string(), name.to_string(), lamp);
    let close = cell(&key(Act::Shipyard), "CLOSE", Lamp::Off);
    let (title, cells) = match y.page {
        Page::Plan => {
            let goes = y.planned().is_ok();
            let build = if y.armed { cell("S+ENT", "AGAIN TO BUILD", Lamp::Busy) } else { cell("S+ENT", "BUILD HERE", can(docked && goes)) };
            ("PLAN", vec![cell("UP DN", "SLOT", Lamp::Off), cell("LT RT", "MODULE", Lamp::Off), cell("ENTER", "PUT IN THE PLAN", Lamp::Off), build, cell("TAB", "HULLS", Lamp::Off), close])
        }
        Page::Hulls => ("HULLS", vec![cell("UP DN", "HULL", Lamp::Off), cell("ENTER", "PLAN FROM IT", Lamp::Off), cell("S+ENT", "COPY TO DESIGN", Lamp::Off), cell("TAB", "DESIGN", Lamp::Off), close]),
        Page::Design => {
            let on_commission = y.knob == universe_sim::world::design::KNOBS.len();
            let goes = app.design.spec().is_ok();
            (
                "DESIGN",
                vec![cell("UP DN", "NUMBER", Lamp::Off), cell("LT RT", "TURN IT", can(!on_commission)), cell("SHIFT", "TURN x5", can(!on_commission)), cell("ENTER", "COMMISSION", can(on_commission && goes)), cell("TAB", "PLANS", Lamp::Off), close],
            )
        }
        Page::Plans => {
            let keep = y.plan_pick == 0;
            ("PLANS", vec![cell("UP DN", "PLAN", Lamp::Off), cell("ENTER", if keep { "KEEP THIS PLAN" } else { "LOAD IT" }, Lamp::Off), cell("DEL", "DROP IT", can(!keep)), cell("TAB", "PLAN", Lamp::Off), close])
        }
    };
    draw_panel(frame, &format!("{title} - ACTIONS"), &cells, 3);
}

pub fn draw(frame: &mut Frame, app: &App, y: &Shipyard) {
    let size = frame.size();
    frame.hud_rect(Vec2::ZERO, size, Color([0.0, 0.015, 0.01, 1.0]));
    actions(frame, app, y);
    let c = content();
    let here = station(app);
    let title = match &here {
        Some(s) => format!("SHIPYARD - {s}"),
        None => "SHIP PLANNER".into(),
    };
    frame.text(Vec2::new(12.0, 12.0), &format!("{title}   {:.0} CR", app.v.credits), TEXT);
    // The pages, as tabs (TAB moves on).
    use crate::hud::{draw_cell, Lamp};
    let tabs = [(Page::Plan, "PLAN"), (Page::Hulls, "HULLS"), (Page::Design, "DESIGN"), (Page::Plans, "PLANS")];
    let tab = Vec2::new(86.0, 14.0);
    let x0 = size.x - 4.0 * (tab.x + 2.0) - 8.0;
    let now = tabs.iter().position(|(p, _)| *p == y.page).unwrap_or(0);
    for (k, (page, name)) in tabs.iter().enumerate() {
        // (TAB on the next one along.)
        let key = if k == (now + 1) % tabs.len() { "TAB" } else { "" };
        draw_cell(frame, Vec2::new(x0 + k as f32 * (tab.x + 2.0), 8.0), tab, key, name, if *page == y.page { Lamp::On } else { Lamp::Off });
    }
    match y.page {
        Page::Hulls => return draw_hulls(frame, app, y),
        Page::Plans => return draw_plans(frame, app, y),
        Page::Design => return draw_design(frame, app, y),
        Page::Plan => {}
    }
    let spec = y.spec();
    // The plan's slots.
    let top = 12.0 + LINE * 2.0;
    frame.text(Vec2::new(12.0, top - LINE), &format!("THE PLAN: {}", spec.name), DIM);
    let flown = fit_of(app);
    for (k, slot) in spec.slots.iter().enumerate() {
        let pick = k == y.slot;
        let m = in_slot(&y.fit, slot);
        // (Different from the ship flown now: marked.)
        let changed = y.hull != app.ship.class || in_slot(&flown, slot) != m;
        let text = format!("{}{:<12} {:<11} S{}  {}{}", if pick { ">" } else { " " }, slot.name.to_uppercase(), format!("{:?}", slot.kind).to_uppercase(), slot.size, m.map_or("(EMPTY)".to_string(), |m| c.get(m).name.clone()), if changed { " *" } else { "" });
        frame.text(Vec2::new(12.0, top + k as f32 * LINE), &text, if pick { SELECT } else if changed { BETTER } else { TEXT });
    }
    // What would go in the slot picked.
    let Some(slot) = spec.slots.get(y.slot) else { return };
    let x = (size.x * 0.52).floor();
    frame.text(Vec2::new(x, top), &format!("FOR {} ({:?}, SIZE {})", slot.name.to_uppercase(), slot.kind, slot.size).to_uppercase(), TEXT);
    let now = in_slot(&y.fit, slot);
    let list = offers(slot);
    // (The cursor within this slot's list, whatever it was left at.)
    let choice = y.choice.min(list.len().saturating_sub(1));
    for (k, o) in list.iter().enumerate() {
        let pick = k == choice;
        let mark = if pick { ">" } else if *o == now { "*" } else { " " };
        let (text, sold) = match o.map(|h| c.get(h)) {
            Some(m) => {
                // Docked: this station's price, if it carries it; elsewhere, the list price.
                let (price, sold) = if here.is_some() {
                    let (offer, stocked) = local(app, m);
                    let p = if !offer.carried { "NOT HERE".to_string() } else if !stocked { "NO STOCK".to_string() } else { format!("{} CR", offer.price as i64) };
                    (p, offer.carried && stocked)
                } else {
                    (format!("{} CR", m.price as i64), true)
                };
                (format!("{mark}{:<20} S{} {:>7} {:>8} {:>10}", m.name.chars().take(20).collect::<String>(), m.size, fmt::tonnes(m.mass), format!("{:.2} MW", m.power / 1e6), price), sold)
            }
            None => (format!("{mark}(EMPTY)"), true),
        };
        let col = if pick { SELECT } else if *o == now { TEXT } else if sold { DIM } else { DIM.scale(0.6) };
        frame.text(Vec2::new(x, top + (2 + k) as f32 * LINE), &text, col);
    }
    let pick = list.get(choice).copied().flatten();
    if let Some(m) = pick.map(|h| c.get(h)) {
        let brand = content().handle::<universe_sim::world::modules::Brand>(&m.brand).map(|b| content().get(b));
        let (maker, note) = brand.map_or(("UNBRANDED".to_string(), "MADE ANYWHERE".to_string()), |b| (b.name.clone(), b.note.clone()));
        let yy = top + (3 + list.len()) as f32 * LINE;
        frame.text(Vec2::new(x, yy), &maker, TEXT);
        frame.text(Vec2::new(x, yy + LINE), &note, DIM);
        frame.text(Vec2::new(x, yy + 2.0 * LINE), &what(m), TEXT);
        if here.is_some() {
            let (offer, stocked) = local(app, m);
            let state = if !offer.carried { "NOT CARRIED HERE".to_string() } else if !stocked { "OUT OF STOCK TO BUILD IT".to_string() } else if offer.hops > 0 { format!("{} GATES FROM ITS MAKER'S HOME (+{:.0}%)", offer.hops, offer.hops as f64 * universe_sim::services::outfitter::MARKUP_PER_HOP * 100.0) } else { "MADE HERE".into() };
            frame.text(Vec2::new(x, yy + 3.0 * LINE), &state, if offer.carried && stocked { DIM } else { RED });
        }
    }
    // The ship flown now, and the plan (with the module picked in it).
    let preview = fitted(y.hull, &with(&y.fit, slot, pick));
    let mut yy = top + (spec.slots.len().max(list.len() + 6) + 1) as f32 * LINE;
    frame.text(Vec2::new(12.0, yy), &format!("{:<11} {:>23} {:>23}", "", "YOUR SHIP", "THE PLAN"), DIM);
    yy += LINE;
    let after = preview.as_ref().ok().map(|s| numbers(s));
    for (i, (label, value)) in numbers(app.ship.spec()).into_iter().enumerate() {
        let next = after.as_ref().map(|a| a[i].1.clone()).unwrap_or_default();
        let col = if next.is_empty() || next == value { TEXT } else { BETTER };
        frame.text(Vec2::new(12.0, yy), &format!("{label:<11} {value:>23} {next:>23}"), col);
        yy += LINE;
    }
    yy += LINE * 0.5;
    match &preview {
        Err(why) => {
            frame.text(Vec2::new(12.0, yy), &format!("WON'T GO TOGETHER - {}", why.to_uppercase()), RED);
        }
        Ok(_) if here.is_some() => {
            let (cost, missing) = build_cost(app, y);
            let text = if missing.is_empty() {
                format!("BUILT HERE: {} {:.0} CR", if cost >= 0.0 { "COSTS" } else { "PAYS" }, cost.abs())
            } else {
                format!("NOT HERE: {} (THE REST {:.0} CR)", missing.join(", "), cost)
            };
            frame.text(Vec2::new(12.0, yy), &text, if missing.is_empty() && cost <= app.v.credits { SELECT } else { RED });
        }
        Ok(_) => {
            frame.text(Vec2::new(12.0, yy), "DOCK AT A STATION'S SHIPYARD TO BUILD IT", DIM);
        }
    }
    // Where it all sits.
    if let Ok(s) = &preview {
        let w = ((size.x - x - 24.0) / 2.0).floor();
        let at = Vec2::new(x, top + (list.len() + 8) as f32 * LINE);
        let h = (size.y - at.y - 12.0 - 4.5 * LINE).max(60.0);
        let picture = crate::thrusterpanel::Picture { spec: s, jets: &[], com: s.centre_of_mass(s.fuel_capacity, s.hold_capacity / 2.0), mounts: true, picked: Some(&slot.name) };
        crate::thrusterpanel::view(frame, &picture, at, Vec2::new(w, h), DVec3::X, DVec3::NEG_Z, "FROM ABOVE");
        crate::thrusterpanel::turning(frame, &picture, at + Vec2::new(w + 12.0, 0.0), Vec2::new(w, h), app.v.time * 0.4, "");
    }
}
