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

const TEXT: Color = Color::hex(0x40ff80);
const DIM: Color = Color::hex(0x208040);
const GATE: Color = Color::hex(0xffc040);
const YOU: Color = Color::hex(0x60ffff);

/// Where the map looks (light years, the galaxy's plane) and how close (px per light year).
pub struct GalaxyMap {
    center: DVec2,
    scale: f64,
}

/// A star's place on the map: its position seen from above (x, z).
fn flat(app: &App, i: usize) -> DVec2 {
    let p = app.charts.galaxy.stars[i].position;
    DVec2::new(p.x, p.z)
}

impl GalaxyMap {
    /// Centred on us, the whole galaxy in view.
    pub fn open(app: &App, size: Vec2) -> Self {
        Self { center: flat(app, app.v.ship_system), scale: size.y as f64 * 0.9 / 19_000.0 }
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
    if input.pressed(KeyCode::KeyU) || input.pressed(KeyCode::KeyM) || input.pressed(KeyCode::Escape) {
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
    // Every star: a dot in its class's colour, brighter the closer we look.
    let glow = (map.scale * 40.0).clamp(0.25, 1.0) as f32;
    for (i, s) in galaxy.stars.iter().enumerate() {
        let p = map.to_screen(size, flat(app, i));
        if on_screen(p) {
            frame.hud_rect(p.floor(), Vec2::ONE, color(s.class.color()).scale(0.45 * glow));
        }
    }
    // The settled systems' gate links.
    for &(a, b) in &app.charts.gate_links {
        frame.hud_line(map.to_screen(size, flat(app, a)), map.to_screen(size, flat(app, b)), GATE.scale(0.6));
    }
    // Where we've been: bright, named when there's room.
    // (Named once they'd stand apart: the settled ones are light years from each other.)
    let named = map.scale > 2.0;
    for &i in &app.explored {
        let p = map.to_screen(size, flat(app, i));
        if !on_screen(p) {
            continue;
        }
        frame.hud_rect(p.floor() - 1.0, Vec2::splat(3.0), color(galaxy.stars[i].class.color()));
        if named {
            frame.text(p + Vec2::new(5.0, -4.0), &star_name(galaxy.stars[i].seed).to_uppercase(), TEXT.scale(0.8));
        }
    }
    // Us.
    let p = map.to_screen(size, flat(app, app.v.ship_system));
    for k in [6.0, 10.0] {
        frame.hud_ellipse(p, Vec2::splat(k), 16, YOU);
    }
    frame.text(p + Vec2::new(12.0, 4.0), "YOU", YOU);

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
    let at = Vec2::new(16.0, size.y - 40.0);
    frame.hud_line(at, at + Vec2::new(px, 0.0), TEXT);
    for x in [0.0, px] {
        frame.hud_line(at + Vec2::new(x, -4.0), at + Vec2::new(x, 4.0), TEXT);
    }
    frame.text(at + Vec2::new(px + 8.0, -4.0), &format!("{round:.0} LY"), TEXT);
    let help = "WHEEL / W S ZOOM   ARROWS PAN   HOME BACK TO YOU   U M ESC CLOSE";
    frame.text(Vec2::new(size.x - text_size(help).x - 16.0, size.y - 20.0), help, DIM);
}
