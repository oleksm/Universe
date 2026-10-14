//! The factory layout studio, an app of its own (`docs/miniapps-plan.md` step 8): a works from the
//! registry (`facility.*`) laid out on its parcel. Its industrial modules (the works' own and its
//! lines') stand as boxes at their records' sizes (no models yet: the user's rule), first in rows
//! from the parcel's corner, then where they are put: a click picks one, a drag moves it on a 0.5 m
//! grid, R turns it a quarter, CTRL+S saves the layout (beside the save, `factories/`). What's
//! outside the parcel or overlapping is drawn red, and counted. It runs no clocks: a layout is a
//! document, not a world. Up and down pick a works, ENTER opens it; a right drag or left and right
//! turn, the wheel zooms, ESC closes.

use std::f64::consts::FRAC_PI_2;

use serde::{Deserialize, Serialize};
use universe_engine::apps::{App, Next};
use universe_engine::glam::{DQuat, DVec3, Vec2};
use universe_engine::{Camera, Color, Context, Frame, KeyCode, Light, MouseButton};

use crate::palette::{DIM, PANEL, TEXT};

/// The grid modules are placed on (m).
const GRID: f64 = 0.5;
/// The gap left between modules laid out in rows at first (m).
const GAP: f64 = 10.0;

/// One module placed: which, where its centre stands on the parcel's ground (m), turned a quarter
/// `turns` times.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Placed {
    pub module: String,
    pub at: [f64; 2],
    pub turns: u8,
}

/// A works' layout, as saved.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Layout {
    pub facility: String,
    pub modules: Vec<Placed>,
}

/// A module's size (length, width, height: m) as its record gives it.
fn size_of(key: &str) -> Option<(String, [f64; 3])> {
    let reg = universe_sim::world::registry::registry();
    let m = reg.records.modules.iter().find(|m| m.identity.key == key)?;
    let p = &m.physical;
    let dims = match (p.length, p.width, p.height, p.envelope) {
        (Some(l), Some(w), Some(h), _) => [l, w, h],
        (_, _, _, Some(e)) => e,
        _ => return None,
    };
    Some((m.identity.name.clone(), dims))
}

fn layout_path(facility: &str) -> std::path::PathBuf {
    crate::save::data_dir().join("freefall").join("factories").join(format!("{facility}.layout.json"))
}

/// A module as laid out now: its name, footprint centre, half size (x along, z across, y up).
struct Shown {
    name: String,
    at: DVec3,
    half: DVec3,
}

struct Open {
    name: String,
    layout: Layout,
    /// The parcel's outline (m, on the ground), centred on its middle; none when it has none.
    outline: Vec<[f64; 2]>,
    shown: Vec<Shown>,
    /// Modules the records give no size for (not placed).
    no_size: Vec<String>,
    picked: Option<usize>,
    dragging: bool,
    unsaved: bool,
    note: String,
}

pub struct FactoryApp {
    /// Every works: key, name.
    list: Vec<(String, String)>,
    selected: usize,
    open: Option<Open>,
    yaw: f64,
    pitch: f64,
    distance: f64,
    /// The cursor's ray on the ground, last frame (for picking and dragging).
    ground: Option<DVec3>,
    camera: Camera,
}

impl FactoryApp {
    /// The studio, with works `key` open (none: the first).
    pub fn new(key: Option<&str>) -> Result<Self, String> {
        let reg = universe_sim::world::registry::registry();
        let list: Vec<(String, String)> = reg.records.facilities.iter().filter(|f| !f.modules.is_empty() || f.lines.iter().any(|l| !l.modules.is_empty())).map(|f| (f.identity.key.clone(), f.identity.name.clone())).collect();
        if list.is_empty() {
            return Err("no works with modules in the registry".into());
        }
        let selected = match key {
            Some(k) => list.iter().position(|(key, _)| key == k).ok_or_else(|| format!("no works {k} with modules in the registry"))?,
            None => 0,
        };
        let mut app = FactoryApp { list, selected, open: None, yaw: 0.7, pitch: 0.7, distance: 600.0, ground: None, camera: Camera::default() };
        app.go(selected);
        Ok(app)
    }

    /// Open works `k`: its saved layout, else its modules in rows from the parcel's corner.
    fn go(&mut self, k: usize) {
        let reg = universe_sim::world::registry::registry();
        let key = self.list[k].0.clone();
        let Some(f) = reg.records.facilities.iter().find(|f| f.identity.key == key) else { return };
        let mut outline: Vec<[f64; 2]> = reg.records.parcels.iter().find(|p| p.identity.key == f.parcel).map(|p| p.outline.clone()).unwrap_or_default();
        if !outline.is_empty() {
            let n = outline.len() as f64;
            let (cx, cz) = (outline.iter().map(|p| p[0]).sum::<f64>() / n, outline.iter().map(|p| p[1]).sum::<f64>() / n);
            for p in &mut outline {
                *p = [p[0] - cx, p[1] - cz];
            }
        }
        // Its modules: the works' own and its lines', each as many as it has.
        let mut wanted: Vec<String> = vec![];
        for e in &f.modules {
            wanted.extend(std::iter::repeat_n(e.module.clone(), e.count as usize));
        }
        for l in &f.lines {
            for m in &l.modules {
                wanted.extend(std::iter::repeat_n(m.module.clone(), m.count as usize));
            }
        }
        let saved: Option<Layout> = std::fs::read(layout_path(&key)).ok().and_then(|b| serde_json::from_slice(&b).ok()).filter(|l: &Layout| l.facility == key);
        let mut no_size = vec![];
        let layout = match saved {
            Some(l) => l,
            None => {
                // In rows from the parcel's corner (its outline's least x and z), wrapping at its width.
                let (x0, z0, x1) = if outline.is_empty() {
                    (-200.0, -200.0, 200.0)
                } else {
                    (outline.iter().map(|p| p[0]).fold(f64::MAX, f64::min), outline.iter().map(|p| p[1]).fold(f64::MAX, f64::min), outline.iter().map(|p| p[0]).fold(f64::MIN, f64::max))
                };
                let (mut x, mut z, mut row) = (x0 + GAP, z0 + GAP, 0.0f64);
                let mut modules = vec![];
                for m in &wanted {
                    let Some((_, [l, w, _])) = size_of(m) else { continue };
                    if x + l > x1 - GAP && x > x0 + GAP {
                        (x, z, row) = (x0 + GAP, z + row + GAP, 0.0);
                    }
                    modules.push(Placed { module: m.clone(), at: [snap(x + l / 2.0), snap(z + w / 2.0)], turns: 0 });
                    x += l + GAP;
                    row = row.max(w);
                }
                Layout { facility: key.clone(), modules }
            }
        };
        for m in &wanted {
            if size_of(m).is_none() && !no_size.contains(m) {
                no_size.push(m.clone());
            }
        }
        let span = outline.iter().map(|p| p[0].hypot(p[1])).fold(150.0, f64::max);
        self.distance = span * 2.2;
        let mut open = Open { name: f.identity.name.clone(), layout, outline, shown: vec![], no_size, picked: None, dragging: false, unsaved: false, note: String::new() };
        open.reshow();
        self.open = Some(open);
    }

    /// The point on the ground under the cursor, from this frame's camera.
    fn ground_at(&self, ctx: &Context) -> Option<DVec3> {
        let size = ctx.hud_size.as_vec2().max(Vec2::ONE);
        let ndc = ctx.input.cursor / size * 2.0 - Vec2::ONE;
        let t = (self.camera.fov_y * 0.5).tan();
        let dir = (self.camera.forward() + self.camera.right() * ndc.x * t * size.x / size.y - self.camera.up() * ndc.y * t).as_dvec3();
        let p = self.camera.position;
        (dir.y < -1e-6).then(|| p + dir * (-p.y / dir.y))
    }
}

fn snap(v: f64) -> f64 {
    (v / GRID).round() * GRID
}

impl Open {
    /// What's shown, from the layout.
    fn reshow(&mut self) {
        self.shown = self.layout.modules.iter().filter_map(|p| {
            let (name, [l, w, h]) = size_of(&p.module)?;
            let (x, z) = if p.turns % 2 == 0 { (l, w) } else { (w, l) };
            Some(Shown { name, at: DVec3::new(p.at[0], h / 2.0, p.at[1]), half: DVec3::new(x / 2.0, h / 2.0, z / 2.0) })
        }).collect();
    }

    /// Whether module `i` lies wholly in the parcel (true with no outline).
    fn inside(&self, i: usize) -> bool {
        if self.outline.is_empty() {
            return true;
        }
        let s = &self.shown[i];
        [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)].iter().all(|(a, b)| point_in(&self.outline, s.at.x + a * s.half.x, s.at.z + b * s.half.z))
    }

    /// Whether module `i`'s footprint overlaps another's.
    fn overlaps(&self, i: usize) -> bool {
        let a = &self.shown[i];
        self.shown.iter().enumerate().any(|(j, b)| j != i && (a.at.x - b.at.x).abs() < a.half.x + b.half.x - 1e-6 && (a.at.z - b.at.z).abs() < a.half.z + b.half.z - 1e-6)
    }
}

/// Point (x, z) inside polygon `poly` (even-odd).
fn point_in(poly: &[[f64; 2]], x: f64, z: f64) -> bool {
    let mut inside = false;
    let mut j = poly.len() - 1;
    for i in 0..poly.len() {
        let (a, b) = (poly[i], poly[j]);
        if (a[1] > z) != (b[1] > z) && x < (b[0] - a[0]) * (z - a[1]) / (b[1] - a[1]) + a[0] {
            inside = !inside;
        }
        j = i;
    }
    inside
}

impl App for FactoryApp {
    fn name(&self) -> &str {
        "factory"
    }

    fn update(&mut self, ctx: &mut Context) -> Next {
        let input = &ctx.input;
        if input.pressed(KeyCode::Escape) {
            return Next::Close;
        }
        let n = self.list.len();
        if input.pressed(KeyCode::ArrowDown) {
            self.selected = (self.selected + 1) % n;
        }
        if input.pressed(KeyCode::ArrowUp) {
            self.selected = (self.selected + n - 1) % n;
        }
        if input.pressed(KeyCode::Enter) {
            self.go(self.selected);
        }
        if input.button_down(MouseButton::Right) {
            self.yaw -= input.mouse_delta.x as f64 * 0.006;
            self.pitch = (self.pitch + input.mouse_delta.y as f64 * 0.006).clamp(0.1, FRAC_PI_2 - 0.01);
        }
        self.yaw += input.axis(KeyCode::ArrowRight, KeyCode::ArrowLeft) as f64 * 1.2 * ctx.dt as f64;
        self.distance = (self.distance * 0.85f64.powf(input.scroll as f64)).clamp(20.0, 20_000.0);
        let ground = self.ground_at(ctx);
        let ctrl = input.down(KeyCode::ControlLeft) || input.down(KeyCode::ControlRight);
        if let Some(o) = self.open.as_mut() {
            // A click picks what's under the cursor; a drag moves it on the grid.
            if input.button_pressed(MouseButton::Left) {
                o.picked = ground.and_then(|g| o.shown.iter().position(|s| (g.x - s.at.x).abs() <= s.half.x && (g.z - s.at.z).abs() <= s.half.z));
                o.dragging = o.picked.is_some();
            }
            if !input.button_down(MouseButton::Left) {
                o.dragging = false;
            }
            if let (true, Some(i), Some(g), Some(last)) = (o.dragging, o.picked, ground, self.ground) {
                let p = &mut o.layout.modules[i];
                let moved = [snap(p.at[0] + g.x - last.x), snap(p.at[1] + g.z - last.z)];
                // (Only whole grid steps: the rest carries over in the next frame's cursor.)
                if moved != p.at && ((moved[0] - p.at[0]).abs() >= GRID || (moved[1] - p.at[1]).abs() >= GRID) {
                    p.at = moved;
                    o.unsaved = true;
                    o.reshow();
                    self.ground = ground;
                }
            } else {
                self.ground = ground;
            }
            if let Some(i) = o.picked
                && input.pressed(KeyCode::KeyR)
            {
                o.layout.modules[i].turns = (o.layout.modules[i].turns + 1) % 4;
                o.unsaved = true;
                o.reshow();
            }
            if ctrl && input.pressed(KeyCode::KeyS) {
                let path = layout_path(&o.layout.facility);
                let saved = path.parent().map_or(Ok(()), std::fs::create_dir_all).and_then(|_| std::fs::write(&path, serde_json::to_vec_pretty(&o.layout).unwrap_or_default()));
                o.note = match saved {
                    Ok(()) => {
                        o.unsaved = false;
                        "SAVED".into()
                    }
                    Err(e) => format!("NOT SAVED: {e}").to_uppercase(),
                };
            }
        }
        self.camera = self.camera_now();
        Next::Stay
    }

    fn camera(&self) -> Camera {
        self.camera
    }

    fn draw(&self, frame: &mut Frame, _: &Context) {
        frame.light = Some(Light { position: DVec3::new(-3.0e11, 6.0e11, 2.0e11), color: [1.0, 0.97, 0.92], luminosity: 1.0, reference: universe_sim::units::AU, radius: 7.0e8 });
        // The ground's grid, every 10 m, out past the parcel.
        let reach = (self.distance * 0.8).clamp(100.0, 3000.0);
        let step = if self.distance > 600.0 { 50.0 } else { 10.0 };
        let grid = Color::hex(0x1d2a36);
        let mut v = -reach;
        while v <= reach {
            frame.line(DVec3::new(v, 0.0, -reach), DVec3::new(v, 0.0, reach), grid);
            frame.line(DVec3::new(-reach, 0.0, v), DVec3::new(reach, 0.0, v), grid);
            v += step;
        }
        let Some(o) = &self.open else { return };
        // The parcel's outline.
        let edge = Color::hex(0xe5c778);
        for k in 0..o.outline.len() {
            let (a, b) = (o.outline[k], o.outline[(k + 1) % o.outline.len()]);
            frame.line(DVec3::new(a[0], 0.0, a[1]), DVec3::new(b[0], 0.0, b[1]), edge);
        }
        // Its modules: boxes at their sizes; picked bright, out of place red.
        let (mut outside, mut overlapping, mut area) = (0, 0, 0.0);
        for (i, s) in o.shown.iter().enumerate() {
            let bad_in = !o.inside(i);
            let bad_over = o.overlaps(i);
            outside += bad_in as usize;
            overlapping += bad_over as usize;
            area += 4.0 * s.half.x * s.half.z;
            let colour = if bad_in || bad_over { Color::hex(0xff6a5a) } else if o.picked == Some(i) { Color::hex(0xffffff) } else { Color::hex(0x9fb4c8) };
            crate::scene::named_box(frame, s.at, DQuat::IDENTITY, s.half, &s.name, colour);
        }
        // The panel: the works, and what this one's layout comes to.
        let line = universe_engine::GLYPH + 2.0;
        let (x, mut y) = (12.0, 12.0);
        // (The works round the one picked: `ROWS` of them.)
        const ROWS: usize = 12;
        let first = self.selected.saturating_sub(ROWS / 2).min(self.list.len().saturating_sub(ROWS));
        frame.hud_rect(Vec2::new(x - 6.0, y - 6.0), Vec2::new(470.0, line * (ROWS as f32 + 10.0)), PANEL);
        frame.text(Vec2::new(x, y), &format!("FACTORY LAYOUT   WORKS {} OF {}", self.selected + 1, self.list.len()), TEXT);
        y += line * 1.5;
        for (k, (_, name)) in self.list.iter().enumerate().skip(first).take(ROWS) {
            let mark = if k == self.selected { ">" } else { " " };
            frame.text(Vec2::new(x, y), &format!("{mark}{}", name.to_uppercase()), if k == self.selected { TEXT } else { DIM });
            y += line;
        }
        y += line * 0.5;
        let parcel = if o.outline.is_empty() { 0.0 } else { polygon_area(&o.outline) };
        frame.text(Vec2::new(x, y), &format!("{}{}", o.name.to_uppercase(), if o.unsaved { " *" } else { "" }), TEXT);
        y += line;
        frame.text(Vec2::new(x, y), &format!("{} MODULES  {:.0} M2 OF A {:.0} M2 PARCEL", o.shown.len(), area, parcel), DIM);
        y += line;
        let warn = Color::hex(0xff6a5a);
        frame.text(Vec2::new(x, y), &format!("{outside} OUTSIDE THE PARCEL  {overlapping} OVERLAPPING"), if outside + overlapping > 0 { warn } else { DIM });
        y += line;
        if !o.no_size.is_empty() {
            frame.text(Vec2::new(x, y), &format!("NO SIZE IN THEIR RECORDS: {}", o.no_size.join(", ").to_uppercase()), warn);
            y += line;
        }
        if let Some(i) = o.picked {
            let s = &o.shown[i];
            frame.text(Vec2::new(x, y), &format!("PICKED {}  {:.1} X {:.1} X {:.1} M", s.name.to_uppercase(), s.half.x * 2.0, s.half.z * 2.0, s.half.y * 2.0), TEXT);
            y += line;
        }
        if !o.note.is_empty() {
            frame.text(Vec2::new(x, y), &o.note, edge);
        }
        frame.text(Vec2::new(12.0, frame.size().y - 24.0), "UP/DOWN WORKS  ENTER OPEN  CLICK PICK  DRAG MOVE  R TURN  CTRL+S SAVE  RIGHT-DRAG VIEW  WHEEL ZOOM  ESC CLOSE", DIM);
    }
}

impl FactoryApp {
    fn camera_now(&self) -> Camera {
        let dir = DVec3::new(self.pitch.cos() * self.yaw.sin(), self.pitch.sin(), self.pitch.cos() * self.yaw.cos());
        let mut camera = Camera { position: dir * self.distance, ..Default::default() };
        camera.look_at(DVec3::ZERO, universe_engine::glam::Vec3::Y);
        camera
    }
}

fn polygon_area(p: &[[f64; 2]]) -> f64 {
    let mut a = 0.0;
    for k in 0..p.len() {
        let (u, v) = (p[k], p[(k + 1) % p.len()]);
        a += u[0] * v[1] - v[0] * u[1];
    }
    a.abs() / 2.0
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every works with modules opens; its sized modules are laid in rows inside its parcel when
    /// they fit, none overlapping, and a module turned a quarter swaps its footprint.
    #[test]
    fn works_lay_out_from_their_records() {
        let mut app = FactoryApp::new(None).unwrap();
        for k in 0..app.list.len() {
            app.go(k);
            let o = app.open.as_ref().unwrap();
            assert!((0..o.shown.len()).all(|i| !o.overlaps(i)), "{}: laid out overlapping", o.name);
        }
        let o = app.open.as_mut().unwrap();
        if let Some(p) = o.layout.modules.first_mut() {
            let before = o.shown[0].half;
            p.turns = 1;
            o.reshow();
            assert_eq!((o.shown[0].half.x, o.shown[0].half.z), (before.z, before.x));
        }
        assert!(point_in(&[[0.0, 0.0], [10.0, 0.0], [10.0, 10.0], [0.0, 10.0]], 5.0, 5.0));
    }
}
