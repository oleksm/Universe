//! The economy panel (5): an instrument for balance. Across the settled
//! places — what's made, used and short a day, by kind — then every place
//! (its kind, people, what it's shortest of), and the one under the cursor
//! in full: each kind it trades, its stock and how many days that covers,
//! its price against usual, what it makes, uses and goes short of a day.

use universe_engine::glam::Vec2;
use universe_engine::{text_size, Color, Context, Frame, KeyCode};
use universe_sim::services::economy::{lines, Place, COVER_DAYS};
use universe_sim::world::goods::Category;

use crate::App;

const TEXT: Color = Color::hex(0xdcebf2);
const DIM: Color = Color::hex(0x7d93a0);
const WARN: Color = Color::hex(0xffb030);
const BAD: Color = Color::hex(0xff5040);

/// The panel's cursor.
#[derive(Default)]
pub struct EconomyPanel {
    pub selected: usize,
    held: f32,
}

/// Days a place's stock of line `i` covers (what it uses, or for what it
/// only makes, its output).
fn cover(p: &Place, c: Category) -> f64 {
    let rate = p.needs(c).max(p.makes(c));
    if rate > 0.0 { p.stock_of(c) / rate } else { f64::INFINITY }
}

/// The kind a place is shortest of (days of cover), among what it uses.
fn shortest(p: &Place) -> Option<(Category, f64)> {
    Category::all().filter(|&c| p.needs(c) > 0.0).map(|c| (c, cover(p, c))).min_by(|a, b| a.1.total_cmp(&b.1))
}

/// Keys while open. False when it should close.
pub fn input(app: &mut App, ctx: &Context) -> bool {
    let input = &ctx.input;
    if crate::keys::pressed(input, crate::keys::Act::Economy) || input.pressed(KeyCode::Escape) {
        return false;
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
    let mut y = 12.0;
    let people: f64 = places.iter().map(|p| p.population).sum();
    let short_places = places.iter().filter(|p| p.short.iter().any(|s| *s > 1e-6)).count();
    frame.text(
        Vec2::new(12.0, y),
        &format!("ECONOMY - {} PLACES, {people:.0}K PEOPLE, {short_places} SHORT OF SOMETHING   AS HEARD OVER THE HYPERNET   ({} CLOSES, UP/DOWN PLACE)", places.len(), crate::keys::key(crate::keys::Act::Economy)),
        TEXT,
    );
    y += line * 1.5;
    // Across all places, per kind.
    let mut totals = vec![(0.0f64, 0.0f64, 0.0f64); lines()];
    for p in places.iter() {
        for (i, t) in totals.iter_mut().enumerate() {
            t.0 += p.made[i];
            t.1 += p.used[i];
            t.2 += p.short[i];
        }
    }
    let (made, used, short): (f64, f64, f64) = totals.iter().fold((0.0, 0.0, 0.0), |a, t| (a.0 + t.0, a.1 + t.1, a.2 + t.2));
    frame.text(Vec2::new(12.0, y), &format!("MADE {made:.0} T/DAY   USED {used:.0} T/DAY   SHORT {short:.1} T/DAY"), if short > 0.05 * used.max(1.0) { WARN } else { DIM });
    y += line;
    let shorts: Vec<String> = Category::all().filter(|&c| totals[c.index()].2 > 0.05).map(|c| format!("{} {:.0}", c.name(), totals[c.index()].2)).collect();
    if !shorts.is_empty() {
        frame.text(Vec2::new(12.0, y), &format!("SHORT: {}", shorts.join("  ")), WARN);
    }
    y += line * 1.5;

    // The places, under their headers.
    frame.text(Vec2::new(12.0, y), &format!(" {:<9} {:<12} {:<7} {:>6} {:>4} {:>7}  {}", "SYSTEM", "PLACE", "KIND", "PEOPLE", "FED", "AGE", "SHORTEST"), DIM);
    y += line;
    let top = y;
    let shown = ((size.y - top - 20.0) / line) as usize;
    let first = panel.selected.saturating_sub(shown.saturating_sub(1));
    let age = |a: Option<f64>| match a {
        Some(a) if a.is_infinite() => "LONG".to_string(),
        Some(a) => crate::fmt::lag(a),
        None => "NO WORD".to_string(),
    };
    for (k, &p) in places.iter().enumerate().skip(first).take(shown) {
        let worst = shortest(p);
        let c = match worst {
            Some((_, d)) if d < 1.0 => BAD,
            Some((_, d)) if d < COVER_DAYS * 0.5 => WARN,
            _ => DIM,
        };
        let mark = if k == panel.selected { ">" } else { " " };
        let sys = app.charts.system(p.system);
        let text = format!(
            "{mark}{:<9} {:<12} {:<7} {:>5.1}K {:>3.0}% {:>7}  {}",
            sys.name.to_uppercase().chars().take(9).collect::<String>(),
            p.facility.name(&sys).to_uppercase().split(" (").next().unwrap_or("").chars().take(12).collect::<String>(),
            p.kind.label().split(' ').next().unwrap_or(""),
            p.population,
            p.fed * 100.0,
            age(heard[k].1),
            worst.map_or(String::new(), |(c, d)| format!("{} {}", c.name(), days(d)))
        );
        frame.text(Vec2::new(12.0, y), &text, if k == panel.selected { TEXT } else { c });
        y += line;
    }

    // The place under the cursor, in full.
    let Some(&p) = places.get(panel.selected) else { return };
    let x = (size.x * 0.54).floor();
    let mut y = top;
    frame.text(Vec2::new(x, y), &format!("{} ({}, {:.1}K PEOPLE)", place_name(app, p), p.kind.label(), p.population), TEXT);
    y += line;
    let report = match heard[panel.selected].1 {
        Some(a) if a.is_infinite() => "ITS REPORT: LONG KNOWN".to_string(),
        Some(a) => format!("ITS REPORT AS IT REACHED US: {} OLD", crate::fmt::lag(a)),
        None => "NO WORD OF IT REACHES US: WHAT'S LONG KNOWN".to_string(),
    };
    frame.text(Vec2::new(x, y), &report, DIM);
    y += line;
    let life = if p.deaths > 0.0 { format!("{:.2}K DYING A DAY", p.deaths) } else { format!("{:+.2}K A DAY", p.growth) };
    frame.text(Vec2::new(x, y), &format!("FED {:.0}%   {:.1}K WAITING TO LEAVE   {life}", p.fed * 100.0, p.waiting), if p.fed < 0.5 { BAD } else if p.fed < 0.9 { WARN } else { DIM });
    y += line * 1.5;
    frame.text(Vec2::new(x, y), &format!("{:<11} {:>5} {:>5} {:>5} {:>5} {:>6} {:>6} {:>5}", "KIND", "TRADE", "STOCK", "COVER", "PRICE", "MADE", "USED", "SHORT"), DIM);
    y += line;
    for c in Category::all().filter(|&c| p.trades(c)) {
        let i = c.index();
        let d = cover(p, c);
        let col = if p.short[i] > 1e-6 { BAD } else if d < COVER_DAYS * 0.5 && p.needs(c) > 0.0 { WARN } else { TEXT };
        let text = format!(
            "{:<11} {:>5} {:>5.0} {:>5} {:>4.2}x {:>6.1} {:>6.1} {:>5.1}",
            c.name(),
            if p.sells(c) { "SELL" } else { "BUY" },
            p.stock_of(c),
            days(d),
            p.factor(c).unwrap_or(1.0),
            // (+ 0.0: nothing made shows 0, not -0.)
            p.made[i] + 0.0,
            p.used[i] + 0.0,
            p.short[i] + 0.0
        );
        frame.text(Vec2::new(x, y), &text, col);
        y += line;
    }
    let note = "STOCK T, COVER IN DAYS, PRICE AGAINST USUAL, MADE/USED/SHORT T A DAY. SHIPS CARRY EVERYTHING.";
    frame.text(Vec2::new((size.x - text_size(note).x) / 2.0, size.y - 14.0), note, DIM);
}

fn days(d: f64) -> String {
    if d.is_infinite() {
        "-".into()
    } else if d < 1.0 {
        format!("{:.0}H", d * 24.0)
    } else {
        format!("{d:.0}D")
    }
}
