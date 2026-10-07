//! The galaxy map (U, from the navigation map): every star of the galaxy
//! from above, coloured by class; the systems we've been to bright and
//! named; the settled ones' gate links; where we are; and how much of it
//! all we've seen. Wheel (or W/S) zooms, arrows pan, HOME comes back to us,
//! U/M/ESC close.

use universe_engine::glam::{DVec2, Vec2};
use universe_engine::{text_size, Color, Context, Frame, KeyCode};
use universe_sim::names::star_name;

use crate::scene::color;
use crate::App;
use crate::palette::{DIM, TEXT};

const GATE: Color = Color::hex(0xffc040);
const YOU: Color = Color::hex(0x60ffff);

/// The scale bar's length (px, about) and what it reads when the map opens (ly).
const BAR_PX: f64 = 120.0;
const DEFAULT_BAR: f64 = 5.0;
/// Stars this far above or below our plane fade out (the map's a slab of the region round us).
const SLAB: f64 = 20.0;

/// Where the map looks (light years, the galaxy's plane) and how close (px
/// per light year); the galaxy's glow, gathered once (see `Glow`).
pub struct GalaxyMap {
    center: DVec2,
    scale: f64,
    glow: Glow,
    /// Sectors' stars made so far (their first so many, by sector).
    sectors: std::cell::RefCell<std::collections::HashMap<universe_sim::world::galaxy::Sector, Vec<universe_sim::world::galaxy::GalaxyStar>>>,
    /// Where a drag (left button) last had the cursor.
    dragged_from: Option<Vec2>,
}

/// Cells a side of the glow image, and how far it reaches (ly, each way from the centre).
const GLOW_CELLS: usize = universe_sim::world::galaxy::SHAPE_CELLS;
const GLOW_REACH: f64 = universe_sim::world::galaxy::SHAPE_REACH;

/// The galaxy as light: every star's glow gathered on a grid over its
/// plane, softened, toned (warm in the bulge, bluish in the disc, pink
/// where stars bunch in the arms); each cell corner's colour and opacity.
struct Glow {
    corners: Vec<[f32; 4]>,
}

impl Glow {
    fn gather() -> Self {
        let n = GLOW_CELLS + 1;
        let cell = 2.0 * GLOW_REACH / GLOW_CELLS as f64;
        // (The galaxy's shape, gathered and softened: the same the stars are made by.)
        let grid = universe_sim::world::galaxy::shape_grid();
        let (fine, soft) = (&grid.fine, &grid.soft);
        // (Toned against the bright end of the disc.)
        let mut sorted: Vec<f32> = soft.iter().copied().filter(|v| *v > 0.0).collect();
        sorted.sort_by(f32::total_cmp);
        let bright = sorted.get(sorted.len() * 97 / 100).copied().unwrap_or(1.0).max(1e-3);
        let corners = (0..n * n)
            .map(|k| {
                let (x, y) = ((k % n) as f64 * cell - GLOW_REACH, (k / n) as f64 * cell - GLOW_REACH);
                let r = (x * x + y * y).sqrt();
                let v = 0.6 * fine[k] + 0.4 * soft[k];
                let lit = 1.0 - (-v / bright * 1.1).exp();
                let warm = [1.0, 0.86, 0.64];
                let cool = [0.72, 0.8, 1.0];
                let t = ((r - 900.0) / 4000.0).clamp(0.0, 1.0) as f32;
                let mut c = [0.0f32; 3];
                for i in 0..3 {
                    c[i] = warm[i] + (cool[i] - warm[i]) * t;
                }
                // Clumps standing out of their surroundings in the arms: young, pink.
                let clump = ((fine[k] / soft[k].max(1e-6) - 1.5) / 0.8).clamp(0.0, 1.0) * (((r - 1500.0) / 1500.0).clamp(0.0, 1.0) as f32);
                let pink = [1.0, 0.48, 0.66];
                for i in 0..3 {
                    c[i] += (pink[i] - c[i]) * clump * 0.7;
                }
                [c[0], c[1], c[2], (lit * 0.95).min(0.95)]
            })
            .collect();
        Glow { corners }
    }
}

/// Stars drawn at most a frame: past it, every sector in view is thinned
/// alike (so the arms still show denser).
const STAR_BUDGET: f64 = 30_000.0;
/// Sectors in view past which only the glow is drawn.
const MAX_SECTORS: usize = 6_000;

/// A star's place on the map: its position seen from above (x, z).
fn flat(app: &App, i: usize) -> DVec2 {
    let p = app.charts.galaxy.stars[i].position;
    DVec2::new(p.x, p.z)
}

impl GalaxyMap {
    /// Centred on us, its scale bar `DEFAULT_BAR` light years.
    pub fn open(app: &App, _size: Vec2) -> Self {
        Self { center: flat(app, app.v.ship_system), scale: BAR_PX / DEFAULT_BAR, glow: Glow::gather(), sectors: Default::default(), dragged_from: None }
    }

    /// (Dev scenarios: closer by `k`.)
    pub fn zoom(&mut self, k: f64) {
        self.scale *= k;
    }

    fn to_screen(&self, size: Vec2, p: DVec2) -> Vec2 {
        let d = (p - self.center) * self.scale;
        size * 0.5 + Vec2::new(d.x as f32, d.y as f32)
    }
}

/// The map's keys. False when it should close.
pub fn input(app: &mut App, ctx: &Context) -> bool {
    let input = &ctx.input;
    if crate::keys::pressed(input, crate::keys::Act::Galaxy) || crate::keys::pressed(input, crate::keys::Act::Map) || input.pressed(KeyCode::Escape) {
        return false;
    }
    let size = ctx.low_res.as_vec2();
    let you = flat(app, app.v.ship_system);
    let Some(map) = &mut app.galaxy_map else { return false };
    let dt = ctx.dt as f64;
    let zoom = input.scroll as f64 * 0.15 + (input.axis(KeyCode::KeyS, KeyCode::KeyW) as f64) * 1.5 * dt;
    map.scale = (map.scale * zoom.exp()).clamp(size.y as f64 / 40_000.0, 400.0);
    let pan = 300.0 / map.scale * dt;
    map.center += DVec2::new(input.axis(KeyCode::ArrowLeft, KeyCode::ArrowRight) as f64, input.axis(KeyCode::ArrowUp, KeyCode::ArrowDown) as f64) * pan;
    if input.pressed(KeyCode::Home) {
        map.center = you;
    }
    // A drag with the left button moves the map under the cursor.
    let at = input.cursor;
    if input.button_pressed(universe_engine::MouseButton::Left) {
        map.dragged_from = Some(at);
    }
    if !input.button_down(universe_engine::MouseButton::Left) {
        map.dragged_from = None;
    }
    if let Some(from) = map.dragged_from {
        let d = at - from;
        map.center -= DVec2::new(d.x as f64, d.y as f64) / map.scale;
        map.dragged_from = Some(at);
    }
    true
}

pub fn draw(frame: &mut Frame, app: &App, map: &GalaxyMap) {
    let size = frame.size();
    frame.hud_rect(Vec2::ZERO, size, Color([0.0, 0.0, 0.02, 1.0]));
    let galaxy = &app.charts.galaxy;
    let on_screen = |p: Vec2| p.x >= 0.0 && p.y >= 0.0 && p.x < size.x && p.y < size.y;
    // The galaxy's glow, fading out as we come close enough to see its stars apart.
    let cell = 2.0 * GLOW_REACH / GLOW_CELLS as f64;
    let cell_px = (cell * map.scale) as f32;
    let fade = ((22.0 - cell_px) / 16.0).clamp(0.0, 1.0);
    if fade > 0.0 {
        let n = GLOW_CELLS + 1;
        let corner = |i: usize, j: usize| map.to_screen(size, DVec2::new(i as f64 * cell - GLOW_REACH, j as f64 * cell - GLOW_REACH));
        // (Only the cells in view.)
        let lo = ((map.center - DVec2::new(size.x as f64, size.y as f64) * 0.5 / map.scale + GLOW_REACH) / cell).floor().max(DVec2::ZERO);
        let hi = ((map.center + DVec2::new(size.x as f64, size.y as f64) * 0.5 / map.scale + GLOW_REACH) / cell).ceil().min(DVec2::splat(GLOW_CELLS as f64));
        let col = |i: usize, j: usize| {
            let [r, g, b, a] = map.glow.corners[j * n + i];
            Color([r, g, b, a * fade])
        };
        for j in lo.y as usize..hi.y as usize {
            for i in lo.x as usize..hi.x as usize {
                if map.glow.corners[j * n + i][3] + map.glow.corners[(j + 1) * n + i + 1][3] < 0.004 {
                    continue;
                }
                let (p00, p10, p01, p11) = (corner(i, j), corner(i + 1, j), corner(i, j + 1), corner(i + 1, j + 1));
                frame.hud_triangle_colored([p00, p10, p11], [col(i, j), col(i + 1, j), col(i + 1, j + 1)]);
                frame.hud_triangle_colored([p00, p11, p01], [col(i, j), col(i + 1, j + 1), col(i, j + 1)]);
            }
        }
    }
    // The stars, from their sectors, in a slab round our plane (fading with height above or below
    // it): far out a fine point, near a soft glowing disc in its class's colour.
    {
        use universe_sim::world::galaxy::{charted, sector_count, sector_stars};
        let sector = charted().sector;
        let px_per_ly = map.scale as f32;
        let our_y = galaxy.stars[app.v.ship_system].position.y;
        let half = DVec2::new(size.x as f64, size.y as f64) * 0.5 / map.scale;
        let (lo, hi) = (map.center - half, map.center + half);
        let span = |a: f64, b: f64| (a / sector).floor() as i32..=(b / sector).floor() as i32;
        let (xs, ys, zs) = (span(lo.x, hi.x), span(our_y - SLAB, our_y + SLAB), span(lo.y, hi.y));
        let count = xs.clone().count() * ys.clone().count() * zs.clone().count();
        if count > MAX_SECTORS {
            // (Too far out for the sectors: a sprinkle of the shape's own stars, over the glow.)
            for (i, s) in universe_sim::world::galaxy::shape_stars().iter().enumerate() {
                let p = map.to_screen(size, DVec2::new(s.position.x, s.position.z));
                if i % 3 == 0 && on_screen(p) {
                    frame.hud_rect(p, Vec2::splat(0.5), color(s.class.color()).scale(0.8));
                }
            }
        } else {
            // How many would be in view, and the share of each sector's to draw.
            let overlap = |a0: f64, a1: f64, k: i32| ((a1.min((k + 1) as f64 * sector) - a0.max(k as f64 * sector)) / sector).max(0.0);
            let mut cells = Vec::with_capacity(count);
            let mut expected = 0.0;
            for x in xs.clone() {
                for y in ys.clone() {
                    for z in zs.clone() {
                        let n = sector_count(galaxy.seed, [x, y, z]);
                        expected += n as f64 * overlap(lo.x, hi.x, x) * overlap(our_y - SLAB, our_y + SLAB, y) * overlap(lo.y, hi.y, z);
                        cells.push(([x, y, z], n));
                    }
                }
            }
            let share = (STAR_BUDGET / expected.max(1.0)).min(1.0);
            // (Thinned: each drawn star stands for more, so a little brighter.)
            let boost = (1.0 / share.sqrt()).min(3.0) as f32;
            let mut made = map.sectors.borrow_mut();
            if made.len() > 4 * MAX_SECTORS {
                made.clear();
            }
            for (sector, n) in cells {
                let want = ((n as f64 * share).ceil() as usize).min(n).min(200_000);
                let stars = made.entry(sector).or_default();
                if stars.len() < want {
                    *stars = sector_stars(galaxy.seed, sector, want);
                }
                for s in &stars[..want] {
                    let depth = 1.0 - ((s.position.y - our_y).abs() / SLAB) as f32;
                    if depth <= 0.0 {
                        continue;
                    }
                    let p = map.to_screen(size, DVec2::new(s.position.x, s.position.z));
                    if !on_screen(p) {
                        continue;
                    }
                    // (Faded by opacity, never darkened: over the glow a star is a light, not a speck.)
                    let alpha = depth * (1.0 - fade * 0.6);
                    let [r0, g0, b0, _] = color(s.class.color()).0;
                    let c = Color([r0, g0, b0, alpha]);
                    if px_per_ly < 0.5 {
                        let w = |v: f32| (v + (1.0 - v) * 0.35) * boost.min(1.6);
                        frame.hud_rect(p, Vec2::splat(1.0), Color([w(r0), w(g0), w(b0), alpha]));
                    } else {
                        let r = (0.8 + px_per_ly * 0.08).min(6.0);
                        let [cr, cg, cb, _] = c.0;
                        frame.hud_glow(p, r * 2.5, 12, Color([cr, cg, cb, 0.35 * alpha]), Color([cr, cg, cb, 0.0]));
                        frame.hud_glow(p, r * 0.6, 8, Color([1.0, 1.0, 1.0, 0.9 * alpha]), Color([cr, cg, cb, 0.6 * alpha]));
                    }
                }
            }
        }
    }
    // The settled systems' gate links; zoomed in to 50 ly (the scale bar), each lane's length,
    // where it fits along the lane.
    let lengths = BAR_PX / map.scale <= 50.0 + 1e-6;
    for &(a, b) in &app.charts.gate_links {
        let (pa, pb) = (map.to_screen(size, flat(app, a)), map.to_screen(size, flat(app, b)));
        frame.hud_line(pa, pb, GATE.scale(0.6));
        let ly = galaxy.stars[a].position.distance(galaxy.stars[b].position);
        let text = format!("{ly:.1} LY");
        let w = text_size(&text).x * 0.6;
        if lengths && pa.distance(pb) > w + 24.0 {
            let mid = (pa + pb) / 2.0;
            frame.text_scaled(mid - Vec2::new(w / 2.0, 8.0), &text, GATE.scale(0.9), 0.6);
        }
    }
    // Where we've been: bright, named when there's room.
    // (Named once they'd stand apart: the settled ones are light years from each other.)
    let named = map.scale > 6.0;
    for &i in &app.explored {
        let p = map.to_screen(size, flat(app, i));
        if !on_screen(p) {
            continue;
        }
        let [cr, cg, cb, _] = color(galaxy.stars[i].class.color()).0;
        frame.hud_glow(p, 3.0, 10, Color([1.0, 1.0, 1.0, 1.0]), Color([cr, cg, cb, 0.0]));
        if named {
            frame.text(p + Vec2::new(5.0, -4.0), &star_name(galaxy.stars[i].seed).to_uppercase(), TEXT.scale(0.8));
        }
    }
    // Us.
    let p = map.to_screen(size, flat(app, app.v.ship_system));
    frame.hud_ellipse(p, Vec2::splat(9.0), 32, YOU);
    frame.hud_ellipse(p, Vec2::splat(13.0), 32, YOU.scale(0.4));
    frame.text(p + Vec2::new(-12.0, 16.0), "YOU", YOU);

    // How much of it there is, and how much we've seen.
    let total = galaxy.stars.len();
    let settled = {
        let mut s: Vec<usize> = app.charts.gate_links.iter().flat_map(|&(a, b)| [a, b]).collect();
        s.sort_unstable();
        s.dedup();
        s.len()
    };
    let seen = app.explored.len();
    let title = format!("GALAXY   CHARTED REGION {total} STARS   {seen} EXPLORED ({:.3}%)   {settled} SETTLED", seen as f64 / total as f64 * 100.0);
    frame.text(Vec2::new(16.0, 16.0), &title, TEXT);
    frame.text(Vec2::new(16.0, 30.0), "ABOUT 19,000 LIGHT YEARS ACROSS. STARS SHOWN IN A SLAB 40 LY THICK ROUND YOUR PLANE.", DIM);
    // A scale bar: a round number of light years about 120 px long.
    let ly = BAR_PX / map.scale;
    let round = [1.0, 2.0, 5.0].iter().flat_map(|m| (0..6).map(move |e| m * 10f64.powi(e))).filter(|v| *v <= ly).fold(1.0, f64::max);
    let px = (round * map.scale) as f32;
    let at = Vec2::new(16.0, size.y - 76.0);
    frame.hud_line(at, at + Vec2::new(px, 0.0), TEXT);
    for x in [0.0, px] {
        frame.hud_line(at + Vec2::new(x, -4.0), at + Vec2::new(x, 4.0), TEXT);
    }
    frame.text(at + Vec2::new(px + 8.0, -4.0), &format!("{round:.0} LY"), TEXT);
    {
        use crate::hud::Lamp;
        use crate::keys::{key, Act};
        let cells = vec![
            ("WHL".to_string(), "ZOOM".to_string(), Lamp::Off),
            ("ARR".to_string(), "PAN".to_string(), Lamp::Off),
            ("HOME".to_string(), "YOU".to_string(), Lamp::Off),
            (key(Act::Galaxy), "GALAXY".to_string(), Lamp::On),
            (key(Act::Map), "MAP".to_string(), Lamp::Off),
        ];
        crate::hud::draw_grid(frame, "GALAXY", &cells);
    }
}
