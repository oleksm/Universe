//! The economy panel (5): an instrument for balance. Every settlement the
//! registry describes, as its report reached us over the hypernet: how its
//! works ran (each one's rate against flat out, and what held it back), and
//! its market in full: the stock lying in its warehouse, what its works take
//! of each a day, and the prices it asks and pays.

use universe_engine::glam::Vec2;
use universe_engine::{text_size, Color, Context, Frame, KeyCode};
use universe_sim::services::economy::Place;

use crate::App;

const TEXT: Color = Color::hex(0xdcebf2);
const DIM: Color = Color::hex(0x7d93a0);
const WARN: Color = Color::hex(0xffb030);

/// The panel's cursor.
#[derive(Default)]
pub struct EconomyPanel {
    pub selected: usize,
    held: f32,
    /// The zoning view of the selected place's ground, while open (Enter).
    pub zoning: Option<crate::zoning::Zoning>,
    /// Its works' modules, while open (Tab): the one under the cursor.
    pub module: Option<usize>,
}

/// The modules of place `p`'s works, in order: (works, its setup).
fn modules(app: &App, p: &Place) -> Vec<(usize, usize)> {
    app.v.works.iter().enumerate().filter(|(_, w)| w.site == p.site).flat_map(|(k, w)| (0..w.setups.len()).map(move |s| (k, s))).collect()
}

/// Keys while open. False when it should close.
pub fn input(app: &mut App, ctx: &Context) -> bool {
    let input = &ctx.input;
    // The zoning view, while open, has the keys (Esc back to the list).
    if let Some(mut z) = app.economy_panel.as_mut().and_then(|p| p.zoning.take()) {
        let keep = crate::zoning::input(app, &mut z, ctx);
        if let Some(p) = app.economy_panel.as_mut() {
            p.zoning = keep.then_some(z);
        }
        return true;
    }
    if crate::keys::pressed(input, crate::keys::Act::Economy) || input.pressed(KeyCode::Escape) {
        return false;
    }
    if input.pressed(KeyCode::Enter) {
        let selected = app.economy_panel.as_ref().map_or(0, |p| p.selected);
        let z = app.v.economy.get(selected).and_then(|place| crate::zoning::Zoning::open(app, place));
        if let (Some(z), Some(p)) = (z, app.economy_panel.as_mut()) {
            p.zoning = Some(z);
            return true;
        }
    }
    // Its works' modules (Tab): up and down a module, left and right what it's set to make
    // (Shift: ten at a time), sent as its owner's choice (refused if not yours).
    let selected = app.economy_panel.as_ref().map_or(0, |p| p.selected);
    if input.pressed(KeyCode::Tab)
        && let Some(p) = app.economy_panel.as_mut()
    {
        p.module = if p.module.is_some() { None } else { Some(0) };
        return true;
    }
    if let Some(at) = app.economy_panel.as_ref().and_then(|p| p.module) {
        let rows = app.v.economy.get(selected).map(|p| modules(app, p)).unwrap_or_default();
        let n = rows.len().max(1);
        let (left, right) = (input.pressed(KeyCode::ArrowLeft), input.pressed(KeyCode::ArrowRight));
        if (left || right)
            && let Some(&(k, s)) = rows.get(at)
        {
            let setup = &app.v.works[k].setups[s];
            let count = universe_sim::world::recipes::of(&setup.module.identity.key).len();
            if count > 0 {
                // (Nothing, then each thing it can make, round.)
                let step = if input.down(KeyCode::ShiftLeft) || input.down(KeyCode::ShiftRight) { 10 } else { 1 };
                let now = setup.recipe.map_or(0, |r| r + 1) as i64;
                let next = (now + if right { step } else { -step }).rem_euclid(count as i64 + 1) as usize;
                app.engine.send(universe_sim::Command::SetUp { works: k, setup: s, recipe: next.checked_sub(1) });
            }
        }
        let Some(panel) = &mut app.economy_panel else { return false };
        let (down, up) = (input.down(KeyCode::ArrowDown), input.down(KeyCode::ArrowUp));
        let steps = crate::navmap::repeat(&mut panel.held, down || up, input.pressed(KeyCode::ArrowDown) || input.pressed(KeyCode::ArrowUp), ctx.dt);
        for _ in 0..steps {
            panel.module = Some(if down { (at + 1) % n } else { (at + n - 1) % n });
        }
        return true;
    }
    let n = app.v.economy.len().max(1);
    let Some(panel) = &mut app.economy_panel else { return false };
    let (down, up) = (input.down(KeyCode::ArrowDown), input.down(KeyCode::ArrowUp));
    let steps = crate::navmap::repeat(&mut panel.held, down || up, input.pressed(KeyCode::ArrowDown) || input.pressed(KeyCode::ArrowUp), ctx.dt);
    for _ in 0..steps {
        panel.selected = if down { (panel.selected + 1) % n } else { (panel.selected + n - 1) % n };
    }
    true
}

fn place_name(app: &App, p: &Place) -> String {
    let sys = app.charts.system(p.system);
    format!("{} - {}", sys.name.to_uppercase(), p.facility.name(&sys).to_uppercase())
}

pub fn draw(frame: &mut Frame, app: &App, panel: &EconomyPanel) {
    if let Some(z) = &panel.zoning {
        crate::zoning::draw(frame, app, z);
        return;
    }
    let size = frame.size();
    frame.hud_rect(Vec2::ZERO, size, Color([0.012, 0.018, 0.026, 1.0]));
    let line = 11.0;
    // (Each place as its last report reached us over the hypernet, and how old.)
    let heard: Vec<(&Place, Option<f64>)> = app
        .v
        .economy
        .iter()
        .enumerate()
        .map(|(i, live)| {
            let (snap, age) = app.v.economy_heard.get(i).map_or((None, None), |(s, a)| (Some(s), *a));
            let known = snap.and_then(|s| s.iter().find(|q| q.system == live.system && q.facility == live.facility)).unwrap_or(live);
            (known, age)
        })
        .collect();
    let places: Vec<&Place> = heard.iter().map(|h| h.0).collect();
    let goods = &app.charts.goods;
    let mut y = 12.0;
    let tonnes = |p: &Place| p.stock.total() / 1000.0;
    frame.text(
        Vec2::new(12.0, y),
        &format!("ECONOMY - {} SETTLEMENTS THE REGISTRY DESCRIBES   AS HEARD OVER THE HYPERNET   ({} CLOSES, UP/DOWN PLACE, ENTER ITS GROUND, TAB ITS MODULES)", places.len(), crate::keys::key(crate::keys::Act::Economy)),
        TEXT,
    );
    y += line * 1.5;
    let made: f64 = places.iter().flat_map(|p| p.made.values()).sum::<f64>() / 1000.0;
    let used: f64 = places.iter().flat_map(|p| p.used.values()).sum::<f64>() / 1000.0;
    frame.text(Vec2::new(12.0, y), &format!("MADE {made:.0} T/DAY   USED {used:.0} T/DAY   IN WAREHOUSES {:.0} T", places.iter().map(|p| tonnes(p)).sum::<f64>()), DIM);
    y += line * 1.5;

    // The places, under their headers.
    frame.text(Vec2::new(12.0, y), &format!(" {:<9} {:<14} {:>5} {:>9} {:>6} {:>4}", "SYSTEM", "PLACE", "WORKS", "WAREHOUSE", "AGE", "HELD"), DIM);
    y += line;
    let top = y;
    let shown = ((size.y - top - 20.0) / line) as usize;
    let first = panel.selected.saturating_sub(shown.saturating_sub(1));
    let age = |a: Option<f64>| match a {
        Some(a) if a.is_infinite() => "LONG".to_string(),
        Some(a) => crate::fmt::lag(a),
        None => "NO WORD".to_string(),
    };
    // (Its works, a ground's or a rig's, as they ran.)
    let works_of = |p: &Place| {
        let site = p.site;
        app.v.works.iter().filter(move |w| w.site == site)
    };
    for (k, &p) in places.iter().enumerate().skip(first).take(shown) {
        let works = works_of(p).filter(|w| w.last.is_some()).count();
        let held = works_of(p).filter(|w| w.last.as_ref().is_some_and(|r| r.held_by.is_some())).count();
        let mark = if k == panel.selected { ">" } else { " " };
        let sys = app.charts.system(p.system);
        // (+ 0.0: an empty warehouse shows 0, not -0.)
        let store = if p.warehouse.is_some() { format!("{:.0} T", tonnes(p).max(0.0) + 0.0) } else { "NONE".into() };
        let text = format!("{mark}{:<9} {:<14} {:>5} {:>9} {:>6} {:>4}", sys.name.to_uppercase().chars().take(9).collect::<String>(), p.name.to_uppercase().chars().take(14).collect::<String>(), works, store, age(heard[k].1), held);
        frame.text(Vec2::new(12.0, y), &text, if k == panel.selected { TEXT } else if held > 0 { WARN } else { DIM });
        y += line;
    }

    // The place under the cursor, in full.
    let Some(&p) = places.get(panel.selected) else { return };
    let x = (size.x * 0.47).floor();
    let mut y = top;
    frame.text(Vec2::new(x, y), &place_name(app, p), TEXT);
    y += line;
    let report = match heard[panel.selected].1 {
        Some(a) if a.is_infinite() => "ITS REPORT: LONG KNOWN".to_string(),
        Some(a) => format!("ITS REPORT AS IT REACHED US: {} OLD", crate::fmt::lag(a)),
        None => "NO WORD OF IT REACHES US: WHAT'S LONG KNOWN".to_string(),
    };
    frame.text(Vec2::new(x, y), &report, DIM);
    y += line * 1.5;
    // Its works.
    frame.text(Vec2::new(x, y), &format!("{:<24} {:>5}  {}", "WORKS", "RATE", "HELD BY"), DIM);
    y += line;
    for w in works_of(p) {
        // (Floored: a works held back never reads 100%.)
        let (rate, why) = w.last.as_ref().map_or((String::from("-"), String::new()), |r| (format!("{:.0}%", (r.rate * 100.0).floor()), r.held_by.clone().unwrap_or_default()));
        frame.text(Vec2::new(x, y), &format!("{:<24} {:>5}  {}", w.name.to_uppercase().chars().take(24).collect::<String>(), rate, why), if why.is_empty() { TEXT } else { WARN });
        y += line;
    }
    y += line * 0.5;
    // Its works' modules (Tab), and what each is set to make.
    if let Some(at) = panel.module {
        let rows = modules(app, p);
        frame.text(Vec2::new(x, y), &format!(" {:<24} {:>5}  {}", "MODULE", "COUNT", "SET TO MAKE"), DIM);
        y += line;
        let bottom = size.y - 30.0 - line * 2.0;
        let room = ((bottom - y) / line).floor().max(2.0) as usize;
        // (From the first row that keeps the cursor's on screen, works headings counted.)
        let lines = |from: usize, to: usize| (from..=to.min(rows.len().saturating_sub(1))).map(|r| 1 + usize::from(r == from || rows[r].0 != rows[r - 1].0)).sum::<usize>();
        let first = (0..=at).find(|&f| lines(f, at) <= room).unwrap_or(at);
        let mut last = usize::MAX;
        for (r, &(k, s)) in rows.iter().enumerate().skip(first) {
            if y > bottom {
                break;
            }
            let w = &app.v.works[k];
            if k != last {
                let ground = match w.site {
                    universe_sim::services::economy::Site::Ground(g) => app.v.land.grounds.get(g),
                    universe_sim::services::economy::Site::Rig(..) => None,
                };
                let theirs = ground.and_then(|g| g.works.get(w.works).and_then(|x| g.lots.iter().find(|l| l.number == x.parcel))).map(|l| l.owner.clone());
                let yours = matches!(theirs, Some(universe_sim::services::land::Owner::Party(universe_sim::services::Party::Pilot(universe_sim::PLAYER))));
                frame.text(Vec2::new(x, y), &format!(" {}{}", w.name.to_uppercase(), if yours { " (YOURS)" } else { "" }), DIM);
                y += line;
                last = k;
            }
            let setup = &w.setups[s];
            let what = match setup.recipe() {
                Some(r) => app.charts.goods[r.makes].name.to_uppercase(),
                None if universe_sim::world::recipes::of(&setup.module.identity.key).is_empty() => "-".to_string(),
                None => "NOTHING".to_string(),
            };
            let mark = if r == at { ">" } else { " " };
            frame.text(Vec2::new(x, y), &format!("{mark} {:<23} {:>5}  {}", setup.module.identity.name.to_uppercase().chars().take(23).collect::<String>(), setup.count, what.chars().take(30).collect::<String>()), if r == at { TEXT } else { DIM });
            y += line;
        }
        let note = "TAB BACK   UP/DOWN MODULE   LEFT/RIGHT WHAT IT MAKES (SHIFT: 10)   ONLY ITS OWNER SETS IT";
        frame.text(Vec2::new((size.x - text_size(note).x) / 2.0, size.y - 14.0), note, DIM);
        return;
    }
    // Its market: what lies in the warehouse, and what its works take.
    if p.warehouse.is_none() {
        frame.text(Vec2::new(x, y), "NO WAREHOUSE: NO MARKET", DIM);
        return;
    }
    frame.text(Vec2::new(x, y), &format!("{:<18} {:>6} {:>6} {:>6} {:>6} {:>6} {:>6}", "STOCK", "HELD", "TAKES", "MADE", "USED", "ASK", "BID"), DIM);
    y += line;
    let mut items: Vec<usize> = p.stock.stock.keys().copied().chain(p.wants.iter().map(|(i, _)| *i)).chain(p.made.keys().copied()).collect();
    items.sort_unstable();
    items.dedup();
    for i in items {
        if y > size.y - 30.0 {
            break;
        }
        let g = &goods[i];
        let q = p.price(g);
        let short = q.wanted && q.stock < p.need(i) / 1000.0;
        let text = format!(
            "{:<18} {:>6.0} {:>6.0} {:>6.0} {:>6.0} {:>6} {:>6.0}",
            g.name.to_uppercase().chars().take(18).collect::<String>(),
            q.stock * g.mass / 1000.0,
            p.need(i) / 1000.0,
            p.made.get(&i).copied().unwrap_or(0.0) / 1000.0,
            p.used.get(&i).copied().unwrap_or(0.0) / 1000.0,
            q.ask.map_or("-".into(), |a| format!("{a:.0}")),
            q.bid
        );
        frame.text(Vec2::new(x, y), &text, if short { WARN } else { TEXT });
        y += line;
    }
    let note = "HELD: T IN THE WAREHOUSE. TAKES, MADE, USED: T A DAY. ASK, BID: CR A T. SHIPS CARRY EVERYTHING BETWEEN SETTLEMENTS.";
    frame.text(Vec2::new((size.x - text_size(note).x) / 2.0, size.y - 14.0), note, DIM);
}
