//! The market screen (G): the markets of the system you're in, what each
//! produces and wants, at what price, what it bans, and your hold. You can
//! look at any of them from anywhere in the system (it's local knowledge:
//! other systems' markets you learn by going there), and trade at the one
//! you're docked or landed at.
//!
//! Keys: ←/→ market, ↑/↓ item (PgUp/PgDn), + or ENTER buy, - or DEL sell
//! (SHIFT: 10 at a time), G or ESC close.

use universe_sim::Command;
use universe_engine::glam::Vec2;
use universe_engine::{Color, Context, Frame, KeyCode, GLYPH};
use universe_sim::services::market::{Quote, Side};
use universe_sim::world::Facility;

use crate::App;

const TEXT: Color = Color::hex(0xdcebf2);
const DIM: Color = Color::hex(0x7d93a0);
const SELECT: Color = Color::hex(0xffc040);
const AMBER: Color = Color::hex(0xffb040);
const ROWS: usize = 34;

/// One row: an offer here, or something in the hold this market doesn't trade.
#[derive(Clone)]
pub struct Row {
    pub item: usize,
    pub quote: Option<Quote>,
}

pub struct MarketView {
    /// The markets in the system, and which one is shown.
    pub markets: Vec<(Facility, String)>,
    pub shown: usize,
    pub selected: usize,
    pub scroll: usize,
    /// Refreshed every frame from the world.
    pub rows: Vec<Row>,
    pub docked: Option<Facility>,
    /// How old its quotes are (see `MarketView::age`).
    pub age: Option<f64>,
    /// Up or down held this long (s): the cursor repeats.
    held: f32,
}

impl MarketView {
    /// Open on the market we're docked at, else the first in the system.
    pub fn open(app: &mut App) -> Self {
        let markets = app.v.markets.clone();
        let docked = app.v.docked_market;
        let shown = docked.and_then(|d| markets.iter().position(|(f, _)| *f == d)).unwrap_or(0);
        let mut v = MarketView { markets, shown, selected: 0, scroll: 0, rows: Vec::new(), docked, age: None, held: 0.0 };
        v.refresh(app);
        v
    }

    fn facility(&self) -> Option<Facility> {
        self.markets.get(self.shown).map(|(f, _)| *f)
    }

    /// Quotes and the hold, as the engine last saw them; and ask it to keep
    /// watching the market shown.
    pub fn refresh(&mut self, app: &mut App) {
        self.docked = app.v.docked_market;
        let Some(f) = self.facility() else { return };
        if app.v.market.as_ref().map(|m| m.market) != Some(f) {
            app.engine.send(Command::WatchMarket(Some(f)));
            return;
        }
        let m = app.v.market.as_ref().expect("watched");
        let mut rows: Vec<Row> = m.quotes.iter().map(|q| Row { item: q.offer.item, quote: Some(*q) }).collect();
        // Not listed: the exchange may still take it, while it has room.
        rows.extend(m.held.iter().map(|&(item, quote)| Row { item, quote }));
        self.rows = rows;
        self.age = m.age;
        self.selected = self.selected.min(self.rows.len().saturating_sub(1));
    }
}

/// Handle the market's keys. Returns false when it should close.
pub fn input(app: &mut App, ctx: &Context) -> bool {
    let Some(mut v) = app.market.take() else { return false };
    let input = &ctx.input;
    if input.pressed(KeyCode::Escape) || crate::keys::pressed(input, crate::keys::Act::Market) {
        app.engine.send(Command::WatchMarket(None));
        return false;
    }
    let n = v.markets.len().max(1);
    if input.pressed(KeyCode::ArrowRight) || input.pressed(KeyCode::ArrowLeft) {
        v.shown = (v.shown + if input.pressed(KeyCode::ArrowRight) { 1 } else { n - 1 }) % n;
        v.selected = 0;
        v.scroll = 0;
        crate::sound::click(ctx, 900.0);
    }
    v.refresh(app);
    let rows = v.rows.len().max(1);
    let step = |k: KeyCode, d: usize| if input.pressed(k) { d } else { 0 };
    // (Up and down, held, repeat.)
    let (held_down, held_up) = (input.down(KeyCode::ArrowDown), input.down(KeyCode::ArrowUp));
    let arrows = crate::navmap::repeat(&mut v.held, held_down || held_up, input.pressed(KeyCode::ArrowDown) || input.pressed(KeyCode::ArrowUp), ctx.dt) as usize;
    let down = if held_down { arrows } else { 0 } + step(KeyCode::PageDown, ROWS);
    let up = if held_up && !held_down { arrows } else { 0 } + step(KeyCode::PageUp, ROWS);
    if down > 0 {
        v.selected = (v.selected + down).min(rows - 1);
    }
    if up > 0 {
        v.selected = v.selected.saturating_sub(up);
    }
    if v.selected < v.scroll {
        v.scroll = v.selected;
    } else if v.selected >= v.scroll + ROWS {
        v.scroll = v.selected + 1 - ROWS;
    }
    let lots = if input.down(KeyCode::ShiftLeft) || input.down(KeyCode::ShiftRight) { 10 } else { 1 };
    let buy = input.pressed(KeyCode::Equal) || input.pressed(KeyCode::NumpadAdd) || input.pressed(KeyCode::Enter);
    let sell = input.pressed(KeyCode::Minus) || input.pressed(KeyCode::NumpadSubtract) || input.pressed(KeyCode::Delete);
    if (buy || sell)
        && let (Some(f), Some(row)) = (v.facility(), v.rows.get(v.selected))
    {
        let units = if buy { lots } else { -lots };
        // (What came of it comes back as an event.)
        app.engine.send(Command::Trade { market: f, item: row.item, units });
        v.refresh(app);
    }
    app.market = Some(v);
    true
}

/// The market screen (covers the view).
pub fn draw(frame: &mut Frame, app: &App, v: &MarketView) {
    let size = frame.size();
    frame.hud_rect(Vec2::ZERO, size, Color([0.012, 0.018, 0.026, 1.0]));
    let line = GLYPH + 3.0;
    let x = 16.0;
    let mut y = 12.0;
    let Some((f, name)) = v.markets.get(v.shown) else {
        frame.text(Vec2::new(x, y), "NO MARKETS IN THIS SYSTEM", TEXT);
        return;
    };
    let title = format!("MARKET - {}   < {}/{} >", name.to_uppercase(), v.shown + 1, v.markets.len());
    frame.text(Vec2::new(x, y), &title, TEXT);
    y += line;
    let here = v.docked == Some(*f);
    // Elsewhere: its prices as its board reached us over the hypernet.
    let (status, sc) = match (here, v.age) {
        (true, _) => ("DOCKED HERE - TRADING OPEN".to_string(), SELECT),
        (false, None) => ("NO WORD OF ITS PRICES REACHES US - DOCK OR LAND HERE TO SEE".to_string(), AMBER),
        (false, Some(a)) if a.is_infinite() => ("ITS PRICES AS LONG KNOWN (NO NEWER BOARD HAS REACHED US) - DOCK OR LAND TO TRADE".to_string(), AMBER),
        (false, Some(a)) => (format!("ITS BOARD AS IT REACHED US OVER THE HYPERNET: {} OLD - DOCK OR LAND TO TRADE", crate::fmt::lag(a)), DIM),
    };
    frame.text(Vec2::new(x, y), &status, sc);
    y += line;
    let ship = &app.v.ship;
    frame.text(Vec2::new(x, y), &format!("CREDITS {:.0}   HOLD {:.1} / {:.1} T", app.v.credits, ship.cargo / 1000.0, ship.spec().hold_capacity / 1000.0), TEXT);
    y += line * 1.6;
    let header = format!("  {:<30} {:<11} {:<6} {:>10} {:>10} {:>9} {:>6}", "STOCK", "KIND", "TRADE", "ASK CR/T", "BID CR/T", "STOCK/ROOM T", "HELD T");
    frame.text(Vec2::new(x, y), &header, DIM);
    y += line;
    for (i, row) in v.rows.iter().enumerate().skip(v.scroll).take(ROWS) {
        let item = &app.charts.goods[row.item];
        let held = app.v.hold.iter().find(|h| h.0 == row.item).map_or(0, |h| h.1);
        let (side, buy, sell, level) = match &row.quote {
            Some(q) => (
                if q.offer.side == Side::Sells { "SELLS" } else { "BUYS" },
                q.buy.map_or("-".to_string(), |p| format!("{p:.1}")),
                format!("{:.1}", q.sell),
                format!("{:.0}", q.level),
            ),
            None => ("-", "-".into(), "-".into(), String::new()),
        };
        let cursor = if i == v.selected { ">" } else { " " };
        let held_s = if held > 0 { held.to_string() } else { String::new() };
        let mut name = item.name.to_uppercase();
        name.truncate(30);
        let text = format!("{cursor} {:<30} {:<11} {:<6} {:>10} {:>10} {:>12} {:>6}", name, item.kind_name(), side, buy, sell, level, held_s);
        let c = if i == v.selected {
            SELECT
        } else if row.quote.is_none() {
            DIM
        } else {
            TEXT
        };
        frame.text(Vec2::new(x, y), &text, c);
        y += line;
    }
    let help = "<-/-> MARKET   UP/DOWN ITEM   + OR ENTER BUY   - OR DEL SELL   (SHIFT: 10)   G CLOSE";
    frame.text(Vec2::new(x, size.y - line - 6.0), help, DIM);
}
