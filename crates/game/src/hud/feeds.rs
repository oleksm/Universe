//! The feeds down the right: news, trades, performance and the profile; and the help.

use super::*;

/// What's come to us as news, over the hypernet or within our own comm's
/// hearing: each kill reported, each outlet's digest; when we heard it.
pub(crate) struct NewsItem<'a> {
    pub heard: f64,
    pub time: f64,
    pub system: usize,
    pub what: News<'a>,
}

pub(crate) enum News<'a> {
    /// A kill: its line, and whether it was ours (we did it, or it was us).
    Kill(String, bool),
    Digest(&'a universe_sim::newsroom::Digest),
}

/// Everything heard, newest first.
pub(crate) fn news_items(app: &App) -> Vec<NewsItem<'_>> {
    let mut items: Vec<NewsItem> = app
        .v
        .kills
        .iter()
        .filter_map(|k| {
            let heard = app.net.news.heard(&universe_sim::news::Key::kill(k))?;
            let ours = k.killer == universe_sim::PLAYER || k.victim == universe_sim::PLAYER;
            let line = if k.weapon == "COLLISION" {
                format!("{} WRECKED IN A COLLISION WITH {}", k.victim_name, k.killer_name)
            } else {
                format!("{} DESTROYED {} - {}", k.killer_name, k.victim_name, k.weapon)
            };
            Some(NewsItem { heard, time: k.time, system: k.system, what: News::Kill(line, ours) })
        })
        .collect();
    if let Some(room) = &app.net.newsroom {
        items.extend(room.digests.iter().filter_map(|d| Some(NewsItem { heard: app.net.news.heard(&d.key())?, time: d.time, system: d.system, what: News::Digest(d) })));
    }
    items.sort_by(|a, b| b.heard.total_cmp(&a.heard));
    items
}

/// The newsfeed, right side: what's newly come to us (kills reported,
/// digests), newest on top, each for `NEWS_SHOWN` real seconds from when we
/// heard it: where it's from, and how long it was on its way.
pub(super) fn news_feed(frame: &mut Frame, app: &App, top: f32) -> f32 {
    let now = app.v.time;
    let shown = NEWS_SHOWN * app.warp().max(1.0);
    let mut rows = Vec::new();
    for item in news_items(app).iter().filter(|i| now - i.heard < shown).take(4) {
        let fade = 1.0 - 0.6 * ((now - item.heard) / shown) as f32;
        let from = news_from(app, item.system, item.time, item.heard);
        match &item.what {
            News::Kill(line, ours) => rows.push((format!("{line}{from}"), if *ours { RED } else { AMBER }.scale(fade))),
            News::Digest(d) => {
                let c = Color::hex(0x60ffb0).scale(fade);
                rows.push((format!("NEWS - {}{from}", d.outlet), c));
                if let Some(h) = d.headlines.first() {
                    rows.push((format!("  {h}"), c.scale(0.85)));
                }
            }
        }
    }
    right_column(frame, top, &rows) + 6.0
}

/// Real seconds an item stays in the newsfeed.
pub(super) const NEWS_SHOWN: f64 = 15.0;

/// Trades, right side below the kills: who bought or sold how many of what,
/// where, for how much, and their cargo and credits after. Those in the
/// system in view (and ours), each for `TRADE_SHOWN` real seconds.
pub(super) fn trade_feed(frame: &mut Frame, app: &App, top: f32) -> f32 {
    let now = app.v.time;
    let shown = TRADE_SHOWN * app.warp().max(1.0);
    // (As heard over the hypernet; shown from then.)
    let mut recent: Vec<(&universe_sim::TradeRecord, f64)> = app.v.trade_log.iter().filter_map(|r| Some((r, app.net.news.heard(&universe_sim::news::Key::trade(r))?))).filter(|(_, heard)| now - heard < shown).collect();
    recent.sort_by(|a, b| a.1.total_cmp(&b.1));
    let mut rows = Vec::new();
    for (r, heard) in recent.iter().rev().take(6) {
        let age = ((now - heard) / shown) as f32;
        let after = format!("CARGO {:.1} T, {:.0} CR", r.cargo / 1000.0, r.credits);
        let text = match &r.deal {
            universe_sim::Deal::Bought => format!("{} BOUGHT {} {} FOR {:.0} CR - {after}", r.trader, r.units, r.item, r.amount),
            universe_sim::Deal::Sold => format!("{} SOLD {} {} FOR {:.0} CR - {after}", r.trader, r.units, r.item, r.amount),
            universe_sim::Deal::Heading { to, expect } => format!("{} HEADS FOR {to} - EXPECTS +{expect:.0} CR", r.trader),
            universe_sim::Deal::MovingOn { to } => format!("{} FINDS NOTHING HERE - MOVES ON TO {to}", r.trader),
        };
        let text = format!("{text}{}", news_from(app, r.system, r.time, *heard));
        let base = if r.trader == "YOU" { HUD } else { Color::hex(0x60c0ff) };
        let c = base.scale(1.0 - 0.7 * age.max(0.0));
        rows.push((text, c));
    }
    right_column(frame, top, &rows)
}

/// Where news is from, if not here, and how old it was when it came, if
/// it was a while on its way.
pub(super) fn news_from(app: &App, system: usize, time: f64, heard: f64) -> String {
    let mut s = String::new();
    if system != app.v.ship_system {
        s += &format!(" - IN {}", universe_sim::names::star_name(app.charts.galaxy.stars[system].seed).to_uppercase());
    }
    if heard - time > 2.0 {
        s += &format!(" ({} AGO)", fmt::lag(heard - time));
    }
    s
}


/// Real seconds a trade stays in the feed.
pub(super) const TRADE_SHOWN: f64 = 12.0;

/// Performance, top right: the frame, the world tick, the planner, where the
/// frame's time went and what it drew.
pub(super) fn perf(frame: &mut Frame, app: &App, ctx: &Context, top: f32) -> f32 {
    let p = &ctx.perf;
    let ships = 1 + app.v.crafts.len();
    let mut lines = vec![
        (format!("{:.0} FPS  {:.1} MS", ctx.fps, p.frame_ms), DIM),
        (format!("SIM {:.2} MS  {ships} SHIPS  {:.1} US EACH", app.sim_ms, app.sim_ms * 1000.0 / ships as f32), DIM),
    ];
    let (apart, late, dropped) = app.v.pilots;
    lines.push((format!("PILOTS {}  LATE {late}  DROPPED {dropped}", if apart { "APART" } else { "LOCKSTEP" }), if dropped > 0 { AMBER } else { DIM }));
    if app.last_step.warp_limited {
        lines.push(("SIM CAN'T KEEP UP".into(), RED));
    }
    if app.plan.current.is_some() {
        let every = (app.plan.cost * 20.0).clamp(0.1, 1.0);
        lines.push((format!("PLAN {:.1} MS EVERY {every:.1} S", app.plan.cost * 1000.0), DIM));
    }
    if app.collision.is_some() {
        lines.push((format!("COLLIDE {:.1} MS EVERY 0.2 S", app.collision_cost * 1000.0), DIM));
    }
    lines.extend(frame_lines(ctx));
    if !app.v.crafts.is_empty() {
        let here = app.v.crafts.iter().filter(|c| c.system == app.view.origin).count();
        let t = &app.v.traffic;
        lines.push((format!("TRAFFIC {} SHIPS, {here} HERE", app.v.crafts.len()), DIM));
        lines.push((format!("STOPS {} GATES {} CRASHES {} COLLISIONS {}", t.stops, t.transits, t.crashes, t.collision_losses), DIM));
        lines.push((format!("TRADES {} KILLS {} POSSES {}/{}", t.trades, t.shot_down, t.defences, t.aggressors_downed), DIM));
    }
    lines.extend(asset_lines(ctx));
    let bottom = right_column(frame, top, &lines);
    frame_graph(frame, p, bottom + 4.0)
}

/// Frame, render-thread and GPU-pass measurements shared by world and Studio.
pub(super) fn frame_lines(ctx: &Context) -> Vec<(String, Color)> {
    let p = &ctx.perf;
    let k = |n: u32| if n >= 10_000 { format!("{:.0}K", n as f32 / 1000.0) } else { n.to_string() };
    let mut lines = Vec::new();
    lines.push((format!("UPD {:.1} DRAW {:.1} RENDER {:.1} IDLE {:.1}", p.update_ms, p.draw_ms, p.render_ms, p.wait_ms), DIM));
    // (The GPU's own time a pass, where it stamps them: what the wait for the screen sums.)
    let passes = universe_engine::gputime::times();
    if !passes.is_empty() {
        let total: f32 = passes.iter().map(|(_, ms)| ms).sum();
        let each: Vec<String> = passes.iter().map(|(n, ms)| format!("{} {ms:.1}", n.to_uppercase())).collect();
        lines.push((format!("GPU {total:.1}: {}", each.join(" ")), DIM));
    }
    let worst = p.history.iter().copied().fold(0.0, f32::max);
    lines.push((format!("WORST {worst:.0} MS OF {}  HITCHES {}", p.history.len(), p.hitches), if p.hitches > 0 { AMBER } else { DIM }));
    lines.push((format!("{} LINES {} TRIS {} PTS", k(p.lines), k(p.triangles), k(p.points)), DIM));
    lines
}

pub(super) fn asset_lines(ctx: &Context) -> Vec<(String, Color)> {
    let mut lines = Vec::new();
    let gpu = &ctx.perf.resources;
    lines.push((format!("ASSET GPU {:.1} MIB UP {} HIT {} DOWN {} PENDING {}", gpu.asset_gpu_bytes as f64/1048576., gpu.asset_uploads, gpu.asset_hits, gpu.asset_unloads, gpu.asset_preparing), DIM));
    let rocks = crate::rocks::snapshot();
    lines.push((format!("ROCK VISUALS LOAD {} QUEUE {} RUN {} ERROR {}", rocks.loads, rocks.queued, rocks.loading, rocks.failures), DIM));
    let globes = crate::globe_assets::snapshot();
    lines.push((format!("WORLD VISUALS LOAD {} QUEUE {} RUN {} ERROR {}", globes.loads, globes.queued, globes.loading, globes.failures), DIM));
    let assets = crate::visual_assets::snapshot();
    lines.push((format!("ASSETS LOAD {} HIT {} UNLOAD {} RELOAD {}", assets.loads, assets.hits, assets.unloads, assets.reloads), DIM));
    lines.push((format!("ASSETS {:.1} MIB + {:.1} RESERVED  QUEUE {} RUN {}", assets.resident_bytes as f64/1048576., assets.reserved_bytes as f64/1048576., assets.queued, assets.loading), DIM));
    lines.push((format!("ASSETS COALESCED {} ERROR {} REJECT {}", assets.coalesced, assets.failures, assets.rejected), if assets.failures + assets.rejected > 0 {AMBER} else {DIM}));
    lines
}

/// Shared diagnostics for the standalone Studio (which has no App/universe).
pub fn studio_assets(frame: &mut Frame, ctx: &Context, debug: u8) {
    if debug == 0 && !universe_prof::enabled() { return; }
    let mut lines = vec![(format!("{:.0} FPS  {:.1} MS",ctx.fps,ctx.perf.frame_ms),DIM)];
    lines.extend(frame_lines(ctx));
    lines.extend(asset_lines(ctx));
    let bottom = right_column(frame, 95.0, &lines);
    let bottom = frame_graph(frame, &ctx.perf, bottom+4.0);
    if debug > 1 || universe_prof::enabled() { profile_panel(frame,bottom+4.0); }
}

/// The last frames' times as bars, against the right edge: green under
/// 1/60 s, amber under a hitch, red over (see `HITCH`); a line at 1/60 s.
pub(super) fn frame_graph(frame: &mut Frame, p: &universe_engine::Perf, top: f32) -> f32 {
    let size = frame.size();
    let (w, h) = (universe_engine::HISTORY as f32, 48.0);
    let x0 = size.x - w - 6.0;
    frame.hud_rect(Vec2::new(x0 - 2.0, top), Vec2::new(w + 4.0, h + 4.0), SOFT_PANEL);
    // (Scale: 100 ms the full height; a stall past it clipped.)
    let full = 100.0;
    for (i, ms) in p.history.iter().enumerate() {
        let bar = (ms / full).min(1.0) * h;
        let c = if *ms > universe_engine::HITCH * 1000.0 { RED } else if *ms > 1000.0 / 59.0 { AMBER } else { Color::hex(0x60ff90) };
        frame.hud_rect(Vec2::new(x0 + i as f32, top + 2.0 + h - bar), Vec2::new(1.0, bar.max(1.0)), c);
    }
    let y60 = top + 2.0 + h - (1000.0 / 60.0) / full * h;
    frame.hud_rect(Vec2::new(x0, y60), Vec2::new(w, 1.0), DIM);
    top + h + 6.0
}

/// A column of lines against the right edge, on a soft backing; its bottom.
pub(super) fn right_column(frame: &mut Frame, top: f32, lines: &[(String, Color)]) -> f32 {
    if lines.is_empty() {
        return top;
    }
    let size = frame.size();
    let w = lines.iter().map(|(t, _)| text_size(t).x).fold(0.0, f32::max);
    frame.hud_rect(Vec2::new(size.x - w - 10.0, top), Vec2::new(w + 8.0, lines.len() as f32 * LINE + 6.0), SOFT_PANEL);
    for (i, (text, c)) in lines.iter().enumerate() {
        frame.text(Vec2::new(size.x - text_size(text).x - 6.0, top + 3.0 + i as f32 * LINE), text, *c);
    }
    top + lines.len() as f32 * LINE + 6.0
}

/// The profiler's report (F3): every scope taking real time, as a tree, with
/// its mean and worst time per frame over the last couple of seconds.
/// The profiler's report, against the right edge under what's there (from
/// `top`), as many rows as fit; its bottom.
pub(super) fn profile_panel(frame: &mut Frame, top: f32) -> f32 {
    let rows: Vec<universe_prof::Stat> = universe_prof::report().into_iter().filter(|s| s.mean_ms >= 0.02 || s.max_ms >= 1.0).collect();
    let fit = ((frame.size().y - top - 3.0 * LINE) / LINE).max(4.0) as usize;
    let mut text = String::from("PROFILE (F3 F3)            MEAN    MAX  CALLS\n");
    for st in rows.iter().take(fit.min(48)) {
        let depth = st.name.matches('/').count();
        let leaf = st.name.rsplit('/').next().unwrap_or(st.name).to_uppercase();
        let label: String = format!("{}{leaf}", " ".repeat(depth)).chars().take(22).collect();
        let calls = if st.calls >= 1.5 { format!("{:.0}", st.calls) } else { String::new() };
        text += &format!("{label:<22} {:>6.2} {:>6.2} {calls:>6}\n", st.mean_ms, st.max_ms);
    }
    let size = frame.size();
    let box_size = text_size(&text);
    let pos = Vec2::new(size.x - box_size.x - 8.0, top + 4.0).floor();
    frame.hud_rect(pos - 4.0, box_size + 8.0, crate::palette::panel(0.85));
    frame.hud_box(pos - 4.0, box_size + 8.0, DIM);
    frame.text(pos, &text, HUD);
    pos.y + box_size.y + 4.0
}

pub(super) fn help(frame: &mut Frame) {
    // The bindings as a tree of scopes (see `keys`): two columns, scopes kept whole.
    let lines = crate::keys::tree();
    let rows: Vec<String> = lines.iter().map(|(d, l)| format!("{}{l}", "  ".repeat(*d))).collect();
    let half = lines.iter().enumerate().filter(|(_, (d, _))| *d == 1).map(|(i, _)| i).find(|&i| i >= rows.len() / 2).unwrap_or(rows.len());
    let (left, right) = rows.split_at(half);
    let (left, right) = (left.join("\n"), right.join("\n"));
    let footer = "THE KEY'S LETTER IS LIT IN THE ACTION'S NAME. F1 CLOSES.";
    let (a, b) = (text_size(&left), text_size(&right));
    let gap = 3.0 * GLYPH;
    let box_size = Vec2::new(a.x + gap + b.x, a.y.max(b.y) + 2.0 * LINE);
    let size = frame.size();
    let pos = ((size - box_size) / 2.0).floor();
    frame.hud_rect(pos - 6.0, box_size + 12.0, crate::palette::panel(0.94));
    frame.hud_box(pos - 6.0, box_size + 12.0, HUD);
    frame.text(pos, &left, HUD);
    frame.text(pos + Vec2::new(a.x + gap, 0.0), &right, HUD);
    frame.text(pos + Vec2::new(0.0, box_size.y - LINE), footer, DIM);
}
