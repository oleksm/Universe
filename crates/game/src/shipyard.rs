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
//! keeps the plan as it is; BALANCE — the ship flown now: where its centre
//! of mass sits against its main drive's thrust, and its trim (fuel pumped
//! to the trim cells, each main's share of the drive): ↑/↓ row, ←/→ set it
//! (SHIFT: five times), ENTER on AUTO-BALANCE works the trim out for the
//! load aboard, ENTER on TRIM THE SHIP (docked) has it done. The shipyard
//! key or ESC close.

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
    Balance,
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
    /// The design page's cursor: the name (0), a knob (1..), commissioning (last).
    knob: usize,
    /// Typing the design's name.
    naming: bool,
    held_side: f32,
    /// The balance page: the trim worked on, and its row.
    trim: universe_sim::world::trim::Trim,
    trim_row: usize,
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
        let mut y = Shipyard { page: Page::Plan, hull: app.ship.class, fit: fit_of(app), slot: 0, choice: 0, held: 0.0, hull_pick: 0, plan_pick: 0, armed: false, knob: 0, naming: false, held_side: 0.0, trim: app.ship.trim.clone(), trim_row: 0 };
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

    /// Work the trim out for the load aboard (AUTO-BALANCE).
    pub fn auto_balance(&mut self, app: &App) {
        let ship = &app.ship;
        self.trim = universe_sim::world::trim::balance(ship.spec(), ship.fuel, ship.cargo + ship.hopper);
    }

    /// Open on the balance page, at row `row`.
    pub fn balancing(app: &App, row: usize) -> Self {
        Shipyard { page: Page::Balance, trim_row: row, ..Shipyard::new(app) }
    }

    fn spec(&self) -> &'static ClassSpec {
        content().get(self.hull)
    }

    /// Typing a name (the keyboard is the name's).
    pub fn naming(&self) -> bool {
        self.naming
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
    // Naming the design: letters, digits, spaces and dashes; BACKSPACE; ENTER or ESC done.
    if let Some(y) = app.shipyard.as_mut().filter(|y| y.naming) {
        if input.pressed(KeyCode::Enter) || input.pressed(KeyCode::Escape) {
            y.naming = false;
        } else if input.pressed(KeyCode::Backspace) {
            app.design.name.pop();
        } else {
            for c in input.typed.chars().filter(|c| c.is_ascii_alphanumeric() || *c == ' ' || *c == '-') {
                if app.design.name.len() < 20 {
                    app.design.name.push(c.to_ascii_uppercase());
                }
            }
        }
        return true;
    }
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
            Page::Plans => Page::Balance,
            Page::Balance => Page::Plan,
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
        Page::Balance => {
            use universe_sim::world::trim;
            let ship = &app.ship;
            let s = ship.spec();
            let mains = s.thrusters.iter().filter(|t| t.role == universe_sim::world::ship::ThrusterRole::Main).count();
            y.trim.mains.resize(mains, 1.0);
            // (Rows: the three cells, each main, AUTO-BALANCE, TRIM THE SHIP.)
            let rows = 3 + mains + 2;
            step(&mut y.trim_row, rows);
            let side = input.axis(KeyCode::ArrowLeft, KeyCode::ArrowRight);
            let turns = crate::navmap::repeat(&mut y.held_side, side != 0.0, input.pressed(KeyCode::ArrowLeft) || input.pressed(KeyCode::ArrowRight), ctx.dt);
            if side != 0.0 && turns > 0 {
                let by = side as f64 * if shift { 0.1 } else { 0.02 } * turns as f64;
                match y.trim_row {
                    r @ 0..=2 => {
                        // (Rows: fore–aft, down–up, port–starboard.)
                        let axis = [2, 1, 0][r];
                        y.trim.cells[axis] += by;
                    }
                    r if r < 3 + mains => y.trim.mains[r - 3] += by * 0.5,
                    _ => {}
                }
                y.trim = y.trim.clamped();
            }
            if input.pressed(KeyCode::Enter) {
                if y.trim_row == 3 + mains {
                    y.trim = trim::balance(s, ship.fuel, ship.cargo + ship.hopper);
                    app.say("BALANCED FOR THE LOAD ABOARD".into());
                } else if y.trim_row == 4 + mains {
                    if docked {
                        app.engine.send(Command::Trim(y.trim.clone()));
                    } else {
                        app.say("TRIM DOCKED AT A STATION".into());
                    }
                }
            }
        }
        Page::Design => {
            use universe_sim::world::design::KNOBS;
            // Rows: the name, the knobs, commissioning.
            step(&mut y.knob, KNOBS.len() + 2);
            let (right, left) = (input.down(KeyCode::ArrowRight), input.down(KeyCode::ArrowLeft));
            let turns = crate::navmap::repeat(&mut y.held_side, right || left, input.pressed(KeyCode::ArrowRight) || input.pressed(KeyCode::ArrowLeft), ctx.dt);
            let row = y.knob;
            if turns > 0 && (1..=KNOBS.len()).contains(&row) {
                let by = if right { 1.0 } else { -1.0 } * turns as f64 * if shift { 5.0 } else { 1.0 };
                app.design.turn(row - 1, by);
            }
            if input.pressed(KeyCode::Enter) && row == 0 {
                y.naming = true;
            }
            if input.pressed(KeyCode::Enter) && row == KNOBS.len() + 1 {
                // Commissioned: a hull for good, by its name (or numbered among yours), planned from at once.
                let mut d = app.design.clone();
                let n = app.designs.len() + 1;
                if d.name.trim().is_empty() || d.name == "DESIGN" {
                    d.name = format!("DESIGN {n}");
                }
                if app.designs.iter().any(|x| x.name == d.name && x != &d) {
                    d.name = format!("{} {n}", d.name);
                }
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

/// The modules the design row `row` moves (or adds), to light on the drawings.
fn design_picks(row: usize) -> Vec<&'static str> {
    // (Rows: the name, then the knobs in order; see `KNOBS`.)
    let knob = universe_sim::world::design::KNOBS.get(row.wrapping_sub(1)).map_or("", |k| k.label);
    match knob {
        "ENGINE ROOM AT" => vec!["power", "drive", "hyperdrive"],
        "TANK AT" => vec!["tank"],
        "HOLD AT" | "CARGO RACKS" => vec!["cargo", "cargo_2", "cargo_3"],
        "BRIDGE AT" => vec!["computer", "transponder", "sensors", "life", "avionics"],
        "THRUSTERS CENTRE" | "THRUSTER SPREAD" => vec!["thrusters"],
        "LIFT CENTRE" => vec!["lift"],
        "HARDPOINTS" => vec!["hardpoint_1", "hardpoint_2", "hardpoint_3"],
        "UTILITY SLOTS" => vec!["utility", "utility_2"],
        "SIZE CLASS" => vec!["power", "drive", "thrusters", "lift", "tank", "hyperdrive", "cargo", "cargo_2", "cargo_3"],
        _ => Vec::new(),
    }
}

/// `text` in lines of at most `width` characters, broken between words.
fn wrap(text: &str, width: usize) -> Vec<String> {
    let mut lines: Vec<String> = Vec::new();
    for word in text.split_whitespace() {
        match lines.last_mut() {
            Some(l) if l.len() + 1 + word.len() <= width => {
                l.push(' ');
                l.push_str(word);
            }
            _ => lines.push(word.to_string()),
        }
    }
    lines
}

/// What a kind of slot is for, and what a bigger or a smaller module in it trades.
fn slot_help(kind: universe_sim::world::modules::SlotKind) -> (&'static str, &'static str, &'static str) {
    use universe_sim::world::modules::SlotKind::*;
    match kind {
        Power => ("THE PLANT: POWER FOR EVERYTHING THAT DRAWS IT.", "ROOM FOR MORE THAT DRAWS POWER. HEAVIER, DEARER.", "LIGHTER, CHEAPER; MUST STILL COVER THE DRAW."),
        Drive => ("THE MAIN DRIVE: THE PUSH ALONG THE NOSE.", "MORE PUSH: QUICKER OUT OF A GRAVITY WELL, A HEAVIER LOAD MOVED. HEAVIER, BURNS MORE AT FULL.", "LIGHTER, SIPS FUEL; SLOWER TO GET MOVING."),
        Thrusters => ("THE THRUSTER QUADS: SHOVES EVERY WAY AND MOST OF THE TURNING.", "FASTER TURNS AND SHOVES; HEAVIER, DRAW MORE.", "LIGHTER; SLUGGISH TO TURN AND TO DOCK."),
        Lift => ("THE BELLY LIFT: HOLDING THE SHIP UP OVER A WORLD.", "HOVER AND LAND LOADED ON HEAVIER WORLDS. HEAVIER, DRAWS MORE.", "LIGHTER; TOO LITTLE AND IT CAN'T HOVER LOADED."),
        Tank => ("THE FUEL TANK.", "LONGER BETWEEN FILLS. HEAVIER WHEN FULL.", "LIGHTER; SHORTER RANGE."),
        Cargo => ("CARGO RACKS: THE HOLD.", "MORE TO HAUL. A FULL HOLD IS MASS TO PUSH, LIFT AND BALANCE.", "LIGHTER, NIMBLER; LESS TO SELL."),
        Hyperdrive => ("THE HYPERDRIVE: ACROSS A SYSTEM FAST.", "A BIGGER SHIP'S DRIVE. HEAVIER, DRAWS MORE.", "LIGHTER; (EMPTY: NO HYPERDRIVE AT ALL)."),
        Computer => ("THE FLIGHT COMPUTER: HOW FAST IT LETS THE SHIP TURN.", "QUICKER TURN RATES ALLOWED.", "GENTLER LIMITS."),
        Transponder => ("THE TRANSPONDER: WHO YOU ARE TO TRAFFIC CONTROL.", "", ""),
        Sensors => ("THE SENSORS: HOW FAR YOU SEE SHIPS.", "SEE FURTHER.", "LIGHTER; SEE LESS."),
        LifeSupport => ("LIFE SUPPORT.", "", ""),
        Hardpoint => ("A GUN MOUNT.", "MORE FIREPOWER. MASS AT THE NOSE.", "(EMPTY: LIGHTER, NOTHING TO FIGHT WITH.)"),
        Utility => ("A GEAR SLOT: THE MINING RIG.", "THE RIG: DIG ASTEROIDS. MASS ON THE SPINE.", "(EMPTY: LIGHTER, NO DIGGING.)"),
        Avionics => ("THE NAV COMPUTER: WHICH AUTOPILOTS YOU HAVE.", "MORE AUTOPILOTS (DOCK, LAND, GATE, FOLLOW, HYPERDRIVE, ROUTE).", "FEWER: MORE BY HAND."),
    }
}

fn what(m: &Module) -> String {
    match &m.does {
        Does::PowerPlant { output } => format!("{:.1} MW", output / 1e6),
        Does::Drive { thrust } | Does::Thrusters { thrust } | Does::Lift { thrust } => format!("{:.0} KN A NOZZLE", thrust / 1e3),
        Does::Tank { capacity } | Does::Rack { capacity } => fmt::tonnes(*capacity),
        Does::Cabin { seats } => format!("{seats} SEATS"),
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
    // The hull's room inside, and what its modules take of it.
    let inside = s.shape().solid.volume;
    let taken: f64 = s.fit.iter().map(|(_, m)| c.get(*m).volume).sum();
    vec![
        ("HULL", s.name.clone()),
        ("DRY MASS", fmt::tonnes(s.dry_mass)),
        ("SPACE", format!("{taken:.0} OF {inside:.0} M3 USED")),
        ("TANK / HOLD", format!("{} / {} {:.0} M3", fmt::tonnes(s.fuel_capacity), fmt::tonnes(s.hold_capacity), s.hold_volume)),
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
        ("SEATS", if s.seats > 0 { format!("{} PASSENGERS", s.seats) } else { "NONE".into() }),
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
    let picture = crate::thrusterpanel::Picture { spec: s, jets: &[], com: s.centre_of_mass(s.fuel_capacity, 0.0), mounts: true, picked: &[], labels: &[] };
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

/// The balance page: the ship flown now, its trim as worked on (rows to
/// set), how it stands with it (the thrust line against the centre of mass,
/// the thrusters' burn to hold it straight; with the load aboard, and tank
/// full and empty), a bubble level of it magnified, and the ship drawn with
/// its trim cells (lit by what they hold) and its centre of thrust.
fn draw_balance(frame: &mut Frame, app: &App, y: &Shipyard) {
    use universe_sim::world::ship::ThrusterRole;
    use universe_sim::world::trim::{self, TRIM_SHARE};
    let ship = &app.ship;
    let s = ship.spec();
    let load = ship.cargo + ship.hopper;
    let t = &y.trim;
    let mains = t.mains.len();
    let top = 12.0 + LINE * 2.0;
    frame.text(Vec2::new(12.0, top - LINE), &format!("BALANCE: {} - THE MAIN DRIVE SHOULD PUSH THROUGH THE CENTRE OF MASS", s.name.to_uppercase()), DIM);
    // The rows.
    let side = |v: f64, neg: &str, pos: &str| if v.abs() < 0.005 { "LEVEL".to_string() } else { format!("{:.0}% {}", v.abs() * 100.0, if v < 0.0 { neg } else { pos }) };
    let moved = |v: f64| fmt::tonnes(ship.fuel * TRIM_SHARE * v.abs());
    let mut rows: Vec<(String, String)> = vec![
        ("FORE / AFT CELLS".into(), format!("{:>10}  {}", side(t.cells.z, "FORE", "AFT"), moved(t.cells.z))),
        ("DOWN / UP CELLS".into(), format!("{:>10}  {}", side(t.cells.y, "DOWN", "UP"), moved(t.cells.y))),
        ("PORT / STARBOARD".into(), format!("{:>10}  {}", side(t.cells.x, "PORT", "STBD"), moved(t.cells.x))),
    ];
    for k in 0..mains {
        rows.push((format!("MAIN {} SHARE", k + 1), format!("{:>9.0}%", t.main(k) * 100.0)));
    }
    rows.push(("AUTO-BALANCE".into(), "FOR THE LOAD ABOARD".into()));
    rows.push(("TRIM THE SHIP".into(), if station(app).is_some() { "HERE".into() } else { "(DOCKED AT A STATION)".into() }));
    for (k, (label, value)) in rows.iter().enumerate() {
        let here = k == y.trim_row;
        frame.text(Vec2::new(12.0, top + k as f32 * LINE), &format!("{}{:<18} {}", if here { ">" } else { " " }, label, value), if here { SELECT } else { TEXT });
    }
    // How it stands: with the load aboard (the worked trim, and the ship's now), tank full, tank dry.
    let mut yy = top + (rows.len() as f32 + 1.0) * LINE;
    let mut put = |text: String, c: Color| {
        frame.text(Vec2::new(12.0, yy), &text, c);
        yy += LINE;
    };
    let say = |i: trim::Imbalance| {
        let cm = |v: f64, neg: &str, pos: &str| if v.abs() < 0.0005 { "ON IT".to_string() } else { format!("{:.1} CM {}", v.abs() * 100.0, if v < 0.0 { neg } else { pos }) };
        format!("THRUST {} / {}, BURN {:.2}%", cm(i.miss.y, "BELOW", "ABOVE"), cm(i.miss.x, "LEFT", "RIGHT"), i.burn * 100.0)
    };
    let now = trim::imbalance(s, ship.fuel, load, &ship.trim);
    let worked = trim::imbalance(s, ship.fuel, load, t);
    put("THE THRUST LINE AGAINST THE CENTRE OF MASS; BURN: THE THRUSTERS HOLDING IT STRAIGHT".into(), DIM);
    put(format!("AS TRIMMED NOW   {}", say(now)), TEXT);
    put(format!("THIS TRIM        {}", say(worked)), if worked.burn < now.burn - 1e-5 { BETTER } else { TEXT });
    put(format!("  TANK FULL      {}", say(trim::imbalance(s, s.fuel_capacity, load, t))), DIM);
    put(format!("  TANK NEAR DRY  {}", say(trim::imbalance(s, s.fuel_capacity * 0.1, load, t))), DIM);
    put(format!("LOAD ABOARD: FUEL {} OF {}, CARGO {}", fmt::tonnes(ship.fuel), fmt::tonnes(s.fuel_capacity), fmt::tonnes(load)), DIM);
    let help = "TRIM CELLS: UP TO 15% OF THE FUEL ABOARD, PUMPED TO THE HULL'S ENDS: THEY MOVE THE CENTRE OF MASS. THEY DRAIN AS THE FUEL BURNS, SO A TRIM HOLDS BEST AT THE LOAD IT WAS SET FOR: TRIM AGAIN AFTER LOADING. MAIN SHARES: TURN ONE NOZZLE DOWN TO SWING THE THRUST SIDEWAYS (FOR LESS DRIVE). SIDE BY SIDE MAINS CAN'T MOVE IT UP OR DOWN: THAT'S THE CELLS' WORK.";
    yy += LINE * 0.5;
    for h in wrap(help, 56) {
        frame.text(Vec2::new(12.0, yy), &h, DIM);
        yy += LINE;
    }
    // The bubble level: the centre of mass at the middle, the thrust line's
    // point a dot, 2 mm a pixel.
    let size = frame.size();
    let (bw, bh) = (150.0f32, 150.0f32);
    let at = Vec2::new(size.x - bw - 16.0, top + 8.0);
    frame.hud_box(at, Vec2::new(bw, bh), DIM.scale(0.6));
    frame.text(at + Vec2::new(6.0, 4.0), "LEVEL (x500)", DIM);
    let mid = at + Vec2::new(bw, bh) * 0.5;
    frame.hud_line(mid - Vec2::new(8.0, 0.0), mid + Vec2::new(8.0, 0.0), Color::hex(0xff60ff));
    frame.hud_line(mid - Vec2::new(0.0, 8.0), mid + Vec2::new(0.0, 8.0), Color::hex(0xff60ff));
    frame.hud_ellipse(mid, Vec2::splat(6.0), 16, DIM);
    for (i, c) in [(now, DIM), (worked, SELECT)] {
        let d = (Vec2::new(i.miss.x as f32, -i.miss.y as f32) * 500.0).clamp(Vec2::splat(-bw * 0.45), Vec2::splat(bw * 0.45));
        frame.hud_rect(mid + d - Vec2::splat(2.0), Vec2::splat(4.0), c);
    }
    // The ship from above and from the side: its trim cells, the centre of thrust.
    let com = trim::centre_of_mass(s, ship.fuel, load, t);
    let picture = crate::thrusterpanel::Picture { spec: s, jets: &[], com, mounts: true, picked: &["tank"], labels: &[] };
    let (vw, vh) = (((size.x - 24.0) * 0.25).floor(), (size.y * 0.42).floor());
    let vy = at.y + bh + 12.0;
    let vx = size.x - 2.0 * vw - 20.0;
    crate::thrusterpanel::view(frame, &picture, Vec2::new(vx, vy), Vec2::new(vw, vh), DVec3::X, DVec3::NEG_Z, "FROM ABOVE (NOSE UP)");
    crate::thrusterpanel::view(frame, &picture, Vec2::new(vx + vw + 8.0, vy), Vec2::new(vw, vh), DVec3::Y, DVec3::NEG_Z, "FROM THE SIDE (TOP RIGHT)");
    let thrust: f64 = s.thrusters.iter().filter(|t| t.role == ThrusterRole::Main).map(|t| t.thrust).sum::<f64>().max(1.0);
    let centre = trim::thrusters(s, t).iter().filter(|t| t.role == ThrusterRole::Main).map(|t| t.at * t.thrust).sum::<DVec3>() / thrust;
    // (Each cell lit by its share; the centre of thrust amber.)
    let fill = [t.cells.x.max(0.0), (-t.cells.x).max(0.0), t.cells.y.max(0.0), (-t.cells.y).max(0.0), t.cells.z.max(0.0), (-t.cells.z).max(0.0)];
    let (lo, hi) = s.shape().mesh.extent();
    let reach = (hi - lo).length() * 0.5;
    for (box_at, across, up) in [(Vec2::new(vx, vy), DVec3::X, DVec3::NEG_Z), (Vec2::new(vx + vw + 8.0, vy), DVec3::Y, DVec3::NEG_Z)] {
        let scale = (vw.min(vh) as f64 * 0.42) / reach;
        let c = box_at + Vec2::new(vw, vh) * 0.5;
        let to = |q: DVec3| c + Vec2::new((q.dot(across) * scale) as f32, -(q.dot(up) * scale) as f32);
        for (cell, f) in s.trim_cells.iter().zip(fill) {
            let p = to(*cell);
            let col = if f > 0.0 { Color::hex(0x60c0ff) } else { Color::hex(0x60c0ff).scale(0.35) };
            frame.hud_box(p - Vec2::splat(2.5 + 2.0 * f as f32), Vec2::splat(5.0 + 4.0 * f as f32), col);
        }
        let p = to(centre);
        frame.hud_ellipse(p, Vec2::splat(4.0), 12, Color::hex(0xffb050));
    }
}

/// The design page: the numbers a hull is drawn up from, its own numbers
/// (and what's wrong with it), and it drawn; commissioning it at the end.
fn draw_design(frame: &mut Frame, app: &App, y: &Shipyard) {
    use universe_sim::world::design::KNOBS;
    let top = 12.0 + LINE * 2.0;
    let d = &app.design;
    frame.text(Vec2::new(12.0, top - LINE), "A HULL OF YOUR OWN: WHERE THE MASS SITS AND WHERE THE THRUSTERS PUSH ARE YOURS TO BALANCE", DIM);
    // Its name.
    {
        let here = y.knob == 0;
        let cursor = if y.naming && (app.v.time * 2.0).fract() < 0.5 { "_" } else { "" };
        let shown = if y.naming { format!("{}{cursor}", d.name) } else { d.name.clone() };
        frame.text(Vec2::new(12.0, top), &format!("{}{:<16} {:>12}", if here { ">" } else { " " }, "NAME", shown), if y.naming { BETTER } else if here { SELECT } else { TEXT });
    }
    let top = top + LINE * 1.5;
    for (k, knob) in KNOBS.iter().enumerate() {
        let here = k + 1 == y.knob;
        let v = d.knob(k);
        let value = if knob.step >= 1.0 { format!("{v:.0}") } else { format!("{v:+.2}") };
        let value = if knob.label == "FINS" { if v > 0.5 { "YES".into() } else { "NO".into() } } else { value };
        frame.text(Vec2::new(12.0, top + k as f32 * LINE), &format!("{}{:<16} {:>8}", if here { ">" } else { " " }, knob.label, value), if here { SELECT } else { TEXT });
    }
    let last = KNOBS.len();
    let here = y.knob == last + 1;
    frame.text(Vec2::new(12.0, top + (last as f32 + 0.5) * LINE), &format!("{}COMMISSION IT", if here { ">" } else { " " }), if here { SELECT } else { BETTER });
    // What the row picked is, and what turning it either way does.
    let (about, more, less) = match y.knob {
        0 => ("ITS NAME, AS THE HULL LIST AND YOUR SHIPS WILL CARRY IT.", "ENTER TO TYPE IT, ENTER AGAIN WHEN DONE.", ""),
        k if k <= last => (KNOBS[k - 1].about, KNOBS[k - 1].more, KNOBS[k - 1].less),
        _ => ("MAKE IT A HULL FOR GOOD: ON THE HULLS LIST, BUILT AT ANY STATION'S SHIPYARD.", "", ""),
    };
    let mut yy = top + (last as f32 + 2.0) * LINE;
    for (head, text, col) in [("", about, TEXT), ("UP: ", more, BETTER), ("DOWN: ", less, BETTER)] {
        if text.is_empty() {
            continue;
        }
        for line in wrap(&format!("{head}{text}"), 64) {
            frame.text(Vec2::new(12.0, yy), &line, col);
            yy += LINE;
        }
    }
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
            let picked = design_picks(y.knob);
            let picture = crate::thrusterpanel::Picture { spec: s, jets: &[], com: s.centre_of_mass(s.fuel_capacity, s.hold_capacity / 2.0), mounts: true, picked: &picked, labels: &[] };
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
            let on_commission = y.knob == universe_sim::world::design::KNOBS.len() + 1;
            let on_name = y.knob == 0;
            let goes = app.design.spec().is_ok();
            (
                "DESIGN",
                vec![
                    cell("UP DN", "PICK", Lamp::Off),
                    cell("LT RT", "TURN IT", can(!on_commission && !on_name)),
                    cell("SHIFT", "TURN x5", can(!on_commission && !on_name)),
                    if y.naming { cell("ENTER", "DONE NAMING", Lamp::Busy) } else if on_name { cell("ENTER", "NAME IT", Lamp::Off) } else { cell("ENTER", "COMMISSION", can(on_commission && goes)) },
                    cell("TAB", "PLANS", Lamp::Off),
                    close,
                ],
            )
        }
        Page::Balance => {
            let mains = y.trim.mains.len();
            (
                "BALANCE",
                vec![
                    cell("UP DN", "PICK", Lamp::Off),
                    cell("LT RT", "SET IT", can(y.trim_row < 3 + mains)),
                    cell("SHIFT", "SET x5", can(y.trim_row < 3 + mains)),
                    cell("ENTER", if y.trim_row == 4 + mains { "TRIM THE SHIP" } else { "AUTO-BALANCE" }, can(y.trim_row == 3 + mains || (y.trim_row == 4 + mains && docked))),
                    cell("TAB", "PLAN", Lamp::Off),
                    close,
                ],
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
    let tabs = [(Page::Plan, "PLAN"), (Page::Hulls, "HULLS"), (Page::Design, "DESIGN"), (Page::Plans, "PLANS"), (Page::Balance, "BALANCE")];
    let tab = Vec2::new(86.0, 14.0);
    let x0 = size.x - tabs.len() as f32 * (tab.x + 2.0) - 8.0;
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
        Page::Balance => return draw_balance(frame, app, y),
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
    // What the slot's for, and bigger or smaller.
    let (about, more, less) = slot_help(slot.kind);
    let mut hy = yy + LINE * 1.5;
    for (head, text, col) in [("", about, TEXT), ("BIGGER: ", more, BETTER), ("SMALLER: ", less, BETTER)] {
        if text.is_empty() {
            continue;
        }
        for line in wrap(&format!("{head}{text}"), 60) {
            frame.text(Vec2::new(12.0, hy), &line, col);
            hy += LINE;
        }
    }
    // Where it all sits.
    if let Ok(s) = &preview {
        let w = ((size.x - x - 24.0) / 2.0).floor();
        let at = Vec2::new(x, top + (list.len() + 8) as f32 * LINE);
        let h = (size.y - at.y - 12.0 - 4.5 * LINE).max(60.0);
        let picked = [slot.name.as_str()];
        let picture = crate::thrusterpanel::Picture { spec: s, jets: &[], com: s.centre_of_mass(s.fuel_capacity, s.hold_capacity / 2.0), mounts: true, picked: &picked, labels: &[] };
        crate::thrusterpanel::view(frame, &picture, at, Vec2::new(w, h), DVec3::X, DVec3::NEG_Z, "FROM ABOVE");
        crate::thrusterpanel::turning(frame, &picture, at + Vec2::new(w + 12.0, 0.0), Vec2::new(w, h), app.v.time * 0.4, "");
    }
}
