//! The galaxy map (U, from the navigation map): every star of the galaxy
//! from above, coloured by class; the systems we've been to bright and
//! named; the settled ones' gate links; where we are; and how much of it
//! all we've seen. Wheel (or W/S) zooms, arrows pan, HOME comes back to us,
//! U/M/ESC close.

use universe_engine::glam::{DVec2, Vec2};
use universe_engine::{Color, Context, Frame, KeyCode};
use universe_sim::names::star_name;

use crate::scene::color;
use crate::App;

const TEXT: Color = Color::hex(0xdcebf2);
const DIM: Color = Color::hex(0x7d93a0);
const GATE: Color = Color::hex(0xffc040);
const YOU: Color = Color::hex(0x60ffff);

/// Light years from us to the map's top edge when it opens.
const DEFAULT_REACH: f64 = 50.0;

/// Where the map looks (light years, the galaxy's plane) and how close (px
/// per light year); the galaxy's glow, gathered once (see `Glow`).
pub struct GalaxyMap {
    center: DVec2,
    scale: f64,
    glow: Glow,
}

/// Cells a side of the glow image, and how far it reaches (ly, each way from the centre).
const GLOW_CELLS: usize = 256;
const GLOW_REACH: f64 = 10_500.0;

/// The galaxy as light: every star's glow gathered on a grid over its
/// plane, softened, toned (warm in the bulge, bluish in the disc, pink
/// where stars bunch in the arms); each cell corner's colour and opacity.
struct Glow {
    corners: Vec<[f32; 4]>,
}

impl Glow {
    fn gather(app: &App) -> Self {
        let n = GLOW_CELLS + 1;
        let cell = 2.0 * GLOW_REACH / GLOW_CELLS as f64;
        let mut light = vec![0.0f32; n * n];
        for s in &app.charts.galaxy.stars {
            let (x, y) = ((s.position.x + GLOW_REACH) / cell, (s.position.z + GLOW_REACH) / cell);
            if x < 0.0 || y < 0.0 || x >= (n - 1) as f64 || y >= (n - 1) as f64 {
                continue;
            }
            // (Shared among the four corners round it.)
            let (i, j) = (x as usize, y as usize);
            let (fx, fy) = ((x - i as f64) as f32, (y - j as f64) as f32);
            light[j * n + i] += (1.0 - fx) * (1.0 - fy);
            light[j * n + i + 1] += fx * (1.0 - fy);
            light[(j + 1) * n + i] += (1.0 - fx) * fy;
            light[(j + 1) * n + i + 1] += fx * fy;
        }
        let blur = |src: &[f32], r: i32| -> Vec<f32> {
            let w: Vec<f32> = (-r..=r).map(|k| (-(k * k) as f32 / (0.5 * (r * r) as f32 + 0.5)).exp()).collect();
            let total: f32 = w.iter().sum();
            let mut a = vec![0.0f32; n * n];
            let mut b = vec![0.0f32; n * n];
            for y in 0..n {
                for x in 0..n {
                    a[y * n + x] = (-r..=r).map(|k| src[y * n + (x as i32 + k).clamp(0, n as i32 - 1) as usize] * w[(k + r) as usize]).sum::<f32>() / total;
                }
            }
            for y in 0..n {
                for x in 0..n {
                    b[y * n + x] = (-r..=r).map(|k| a[(y as i32 + k).clamp(0, n as i32 - 1) as usize * n + x] * w[(k + r) as usize]).sum::<f32>() / total;
                }
            }
            b
        };
        let fine = blur(&light, 1);
        let soft = blur(&light, 4);
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

/// A star's place on the map: its position seen from above (x, z).
fn flat(app: &App, i: usize) -> DVec2 {
    let p = app.charts.galaxy.stars[i].position;
    DVec2::new(p.x, p.z)
}

impl GalaxyMap {
    /// Centred on us, `DEFAULT_REACH` light years to the top and bottom.
    pub fn open(app: &App, size: Vec2) -> Self {
        Self { center: flat(app, app.v.ship_system), scale: size.y as f64 * 0.5 / DEFAULT_REACH, glow: Glow::gather(app) }
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
    // Every star: far out a fine point (a real pixel), near a soft glowing disc in its class's colour.
    let px_per_ly = map.scale as f32;
    for (i, s) in galaxy.stars.iter().enumerate() {
        let p = map.to_screen(size, flat(app, i));
        if !on_screen(p) {
            continue;
        }
        let c = color(s.class.color());
        if px_per_ly < 0.08 {
            // (A sprinkle, over the glow.)
            if i % 3 == 0 {
                frame.hud_rect(p, Vec2::splat(0.5), c.scale(0.8));
            }
        } else {
            let r = (0.8 + px_per_ly * 0.08).min(6.0);
            let [cr, cg, cb, _] = c.0;
            frame.hud_glow(p, r * 2.5, 12, Color([cr, cg, cb, 0.35]), Color([cr, cg, cb, 0.0]));
            frame.hud_glow(p, r * 0.6, 8, Color([1.0, 1.0, 1.0, 0.9]), Color([cr, cg, cb, 0.6]));
        }
    }
    // The settled systems' gate links.
    for &(a, b) in &app.charts.gate_links {
        frame.hud_line(map.to_screen(size, flat(app, a)), map.to_screen(size, flat(app, b)), GATE.scale(0.6));
    }
    // Where we've been: bright, named when there's room.
    // (Named once they'd stand apart: the settled ones are light years from each other.)
    let named = map.scale > 1.0;
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
    let title = format!("GALAXY - {total} STARS   {seen} EXPLORED ({:.3}%)   {settled} SETTLED", seen as f64 / total as f64 * 100.0);
    frame.text(Vec2::new(16.0, 16.0), &title, TEXT);
    frame.text(Vec2::new(16.0, 30.0), "ABOUT 19,000 LIGHT YEARS ACROSS. EVERY STAR CAN BE REACHED BY HYPERDRIVE.", DIM);
    // A scale bar: a round number of light years about 120 px long.
    let ly = 120.0 / map.scale;
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
