//! The interior studio: the hull we fly as a blueprint in the round, thin
//! see-through lines (every crease and edge of it), turned and looked into with
//! the mouse. The first step of laying out its insides, large to small.

use std::sync::{mpsc, Arc};

use universe_engine::glam::{Vec2, Vec3};
use universe_engine::{Color, Context, Frame, KeyCode, MouseButton};

use crate::App;

/// The blueprint's paper, its lines, its ground grid, its writing.
const PAPER: Color = Color([0.02, 0.07, 0.15, 1.0]);
const LINE: [f32; 3] = [0.55, 0.8, 1.0];
const GROUND: Color = Color([0.35, 0.6, 0.9, 0.12]);
const LABEL: Color = Color([0.7, 0.85, 1.0, 1.0]);
const DIMENSION: Color = Color([0.75, 0.88, 1.0, 0.75]);
/// The access plan: its lines and points, the hull's own points, what's lit, the work plane.
const PATH: Color = Color([0.4, 1.0, 0.75, 0.95]);
const ANCHOR: Color = Color([1.0, 0.6, 0.3, 1.0]);
const PICKED: Color = Color([1.0, 0.85, 0.35, 1.0]);
const PLANE: Color = Color([0.4, 1.0, 0.75, 0.35]);

/// The camera's field of view up and down (rad).
const FOV: f32 = 0.85;

#[derive(Default)]
pub struct Interior {
    /// The hull's lines, for which hull; and being worked out (on a thread of their
    /// own: they take a moment), for which.
    lines: Option<(String, Arc<Hull>)>,
    job: Option<(String, mpsc::Receiver<Hull>)>,
    /// The camera: round the ship (rad), up from level (rad), how far (m, none: to
    /// fit it), and the point it looks at (the ship's frame; none: its middle).
    yaw: f32,
    pitch: f32,
    distance: Option<f32>,
    target: Option<Vec3>,
    /// Time, for the spinner (s).
    spin: f32,
    /// The access plan: its points and the lines between them (session only).
    plan: Plan,
    /// The tool in hand, the work plane's height (its frame, m; none: the hatch's),
    /// a line's first point, and where a press began (a click, if it doesn't move).
    tool: Tool,
    plane: Option<f32>,
    from: Option<usize>,
    press: Option<Vec2>,
    /// The point, or else the line, under the cursor; the cursor (HUD pixels).
    hover: Option<Hover>,
    cursor: Vec2,
}

/// Points and the lines between them: where access must reach, and the ways it
/// goes, first as lines (their room comes later).
#[derive(Default)]
struct Plan {
    /// For which hull (its fixed points are taken from it).
    hull: String,
    points: Vec<Point>,
    lines: Vec<(usize, usize)>,
}

/// A point of the plan: where it is (the hull's frame), and the model's name for
/// it if it's one of the hull's own (the hatch, the cockpit...: those stay).
struct Point {
    at: Vec3,
    name: Option<String>,
}

#[derive(Clone, Copy, Default, PartialEq)]
enum Tool {
    /// Only looking (turning, moving, nearer and farther).
    #[default]
    Look,
    /// A click on the work plane puts a point there.
    Point,
    /// A click on a point, then on another, joins them.
    Line,
}

#[derive(Clone, Copy, PartialEq)]
enum Hover {
    Point(usize),
    Line(usize),
}

/// The hull as the studio draws it: its lines and its bounds (its frame, m).
pub struct Hull {
    lines: Vec<[Vec3; 2]>,
    /// Where it bends gently (round things): edges with their faces' normals, drawn
    /// where they're its outline from the camera.
    bends: Vec<[Vec3; 4]>,
    lo: Vec3,
    hi: Vec3,
}

impl Interior {
    pub fn new() -> Self {
        Interior { yaw: 0.9, pitch: 0.35, ..Default::default() }
    }

    /// A sample plan for a look (dev scenarios): from the hatch, a run of points on
    /// its level to under the cockpit and up to it, and another to the engines; the
    /// point tool in hand.
    pub fn sample(&mut self, key: &str, shape: &universe_sim::world::shape::Shape) {
        self.seed(key, shape);
        let at = |name: &str, plan: &Plan| plan.points.iter().position(|p| p.name.as_deref() == Some(name));
        let (Some(hatch), Some(cockpit), Some(engines)) = (at("HATCH", &self.plan), at("COCKPIT", &self.plan), at("ENGINES", &self.plan)) else { return };
        let (h, c, e) = (self.plan.points[hatch].at, self.plan.points[cockpit].at, self.plan.points[engines].at);
        let add = |p: Vec3, plan: &mut Plan| {
            plan.points.push(Point { at: p, name: None });
            plan.points.len() - 1
        };
        let a = add(Vec3::new(h.x, h.y, (h.z + c.z) * 0.5), &mut self.plan);
        let b = add(Vec3::new(c.x, h.y, c.z), &mut self.plan);
        let d = add(Vec3::new(e.x, h.y, (h.z + e.z) * 0.5), &mut self.plan);
        self.plan.lines.extend([(hatch, a), (a, b), (b, cockpit), (hatch, d), (d, engines)]);
        self.tool = Tool::Point;
    }

    /// Turned to look from `yaw`, `pitch` (rad; dev scenarios).
    pub fn turned(yaw: f32, pitch: f32) -> Self {
        Interior { yaw, pitch, ..Default::default() }
    }

    /// The hull's lines, asked for once a hull and picked up when they're done.
    fn refresh(&mut self, key: &str, shape: &universe_sim::world::shape::Shape) {
        if let Some((k, rx)) = &self.job
            && let Ok(h) = rx.try_recv()
        {
            self.lines = Some((k.clone(), Arc::new(h)));
            self.job = None;
        }
        let Some(mesh) = shape.walk.as_ref() else { return };
        if self.lines.as_ref().is_none_or(|(k, _)| k != key) && self.job.as_ref().is_none_or(|(k, _)| k != key) {
            let (tx, rx) = mpsc::channel();
            let mesh = mesh.clone();
            std::thread::spawn(move || {
                // (Its creases of 30° or more, less the tiniest bevels: the shape, not its
                // grain.)
                let lines = mesh.creases(30.0).into_iter().filter(|[a, b]| a.distance(*b) >= 0.02).collect();
                tx.send(Hull { lines, bends: mesh.bends(2.0, 30.0), lo: mesh.lo.as_vec3(), hi: mesh.hi.as_vec3() }).ok();
            });
            self.job = Some((key.into(), rx));
        }
    }

    /// The plan's points from the hull's own places, once a hull: its hatch, its
    /// cockpit, its mounts and docks, its main engines (their middle).
    fn seed(&mut self, key: &str, shape: &universe_sim::world::shape::Shape) {
        if self.plan.hull == key {
            return;
        }
        use universe_sim::world::shape::Role;
        let mut points: Vec<Point> = shape
            .nodes
            .iter()
            .filter(|n| matches!(n.role, Role::Hatch | Role::Cockpit | Role::Mount | Role::Dock))
            .map(|n| Point { at: n.at.as_vec3(), name: Some(n.name.to_uppercase().replace('_', " ")) })
            .collect();
        let engines: Vec<Vec3> = shape.nodes(Role::Nozzle).filter(|n| n.name.starts_with("nozzle_main")).map(|n| n.at.as_vec3()).collect();
        if !engines.is_empty() {
            points.push(Point { at: engines.iter().copied().sum::<Vec3>() / engines.len() as f32, name: Some("ENGINES".into()) });
        }
        self.plan = Plan { hull: key.into(), points, lines: Vec::new() };
    }

    fn hull(&self, key: &str) -> Option<Arc<Hull>> {
        self.lines.as_ref().filter(|(k, _)| k == key).map(|(_, h)| h.clone())
    }
}

/// The camera, for the hull drawn in a `size` screen: where it is, its axes
/// (right, up, forward) and its focal length (pixels).
struct Camera {
    eye: Vec3,
    right: Vec3,
    up: Vec3,
    forward: Vec3,
    focal: f32,
    centre: Vec2,
}

impl Camera {
    fn of(i: &Interior, h: &Hull, size: Vec2) -> Self {
        let middle = (h.lo + h.hi) * 0.5;
        let target = i.target.unwrap_or(middle);
        let radius = (h.hi - h.lo).length() * 0.5;
        let focal = size.y * 0.5 / (FOV * 0.5).tan();
        // (Far enough back that the whole hull fits, unless zoomed.)
        let distance = i.distance.unwrap_or(radius / (FOV * 0.5).tan() * 0.8);
        let back = Vec3::new(i.pitch.cos() * i.yaw.sin(), i.pitch.sin(), i.pitch.cos() * i.yaw.cos());
        let eye = target + back * distance;
        let forward = (target - eye).normalize();
        let right = forward.cross(Vec3::Y).normalize();
        let up = right.cross(forward);
        Camera { eye, right, up, forward, focal, centre: size * 0.5 }
    }

    /// The way into the scene through screen point `q`.
    fn ray(&self, q: Vec2) -> Vec3 {
        let d = (q - self.centre) / self.focal;
        (self.forward + self.right * d.x - self.up * d.y).normalize()
    }

    /// Where `p` lands on screen, and how far in front it is (none: behind).
    fn project(&self, p: Vec3) -> Option<(Vec2, f32)> {
        let d = p - self.eye;
        let z = d.dot(self.forward);
        (z > 0.1).then(|| (self.centre + Vec2::new(d.dot(self.right), -d.dot(self.up)) * (self.focal / z), z))
    }
}

/// The toolbar: key, name, what it does.
const TOOLBAR: [(&str, &str, Action); 6] = [
    ("V", "LOOK", Action::Tool(Tool::Look)),
    ("P", "POINT", Action::Tool(Tool::Point)),
    ("L", "LINE", Action::Tool(Tool::Line)),
    ("[", "PLANE DOWN", Action::Plane(-0.5)),
    ("]", "PLANE UP", Action::Plane(0.5)),
    ("DEL", "REMOVE", Action::Remove),
];

#[derive(Clone, Copy, PartialEq)]
enum Action {
    Tool(Tool),
    Plane(f32),
    Remove,
}

/// Where toolbar button `k` is (one row along the top).
fn button(k: usize) -> (Vec2, Vec2) {
    (Vec2::new(12.0 + k as f32 * 124.0, 30.0), Vec2::new(120.0, 16.0))
}

fn button_at(q: Vec2) -> Option<Action> {
    (0..TOOLBAR.len()).find(|&k| {
        let (p, c) = button(k);
        q.x >= p.x && q.x <= p.x + c.x && q.y >= p.y && q.y <= p.y + c.y
    }).map(|k| TOOLBAR[k].2)
}

/// The work plane's height (its frame): as set, or the hatch's (where the crew come in).
fn plane_of(i: &Interior, h: &Hull) -> f32 {
    i.plane.unwrap_or_else(|| i.plan.points.iter().find(|p| p.name.as_deref() == Some("HATCH")).map_or((h.lo.y + h.hi.y) * 0.5, |p| p.at.y))
}

/// What's under the cursor: a point (within 8 px), or else a line (within 5 px).
fn hover_at(i: &Interior, cam: &Camera, q: Vec2) -> Option<Hover> {
    let screen: Vec<Option<Vec2>> = i.plan.points.iter().map(|p| cam.project(p.at).map(|s| s.0)).collect();
    let near = screen.iter().enumerate().filter_map(|(k, s)| s.map(|s| (k, s.distance(q)))).filter(|(_, d)| *d < 8.0).min_by(|a, b| a.1.total_cmp(&b.1));
    if let Some((k, _)) = near {
        return Some(Hover::Point(k));
    }
    i.plan.lines.iter().enumerate().filter_map(|(k, &(a, b))| {
        let (pa, pb) = (screen[a]?, screen[b]?);
        let ab = pb - pa;
        let t = ((q - pa).dot(ab) / ab.length_squared().max(1e-6)).clamp(0.0, 1.0);
        let d = (pa + ab * t).distance(q);
        (d < 5.0).then_some((k, d))
    }).min_by(|a, b| a.1.total_cmp(&b.1)).map(|(k, _)| Hover::Line(k))
}

/// This frame's input. False: close it.
pub fn input(app: &mut App, ctx: &Context, interior: &mut Interior) -> bool {
    let input = &ctx.input;
    let spec = app.ship.spec();
    interior.spin += ctx.dt;
    interior.refresh(&spec.key, spec.shape());
    interior.seed(&spec.key, spec.shape());
    if input.pressed(KeyCode::Escape) {
        if interior.from.take().is_some() {
            return true;
        }
        return false;
    }
    let Some(h) = interior.hull(&spec.key) else { return true };
    let size = ctx.hud_size.as_vec2();
    let cam = Camera::of(interior, &h, size);
    let cursor = input.cursor;
    interior.cursor = cursor;
    let d = input.mouse_delta;
    let clicked = if input.button_pressed(MouseButton::Left) { button_at(cursor) } else { None };
    // The tools, and the work plane up and down.
    for (key, tool) in [(KeyCode::KeyV, Tool::Look), (KeyCode::KeyP, Tool::Point), (KeyCode::KeyL, Tool::Line)] {
        if input.pressed(key) || clicked == Some(Action::Tool(tool)) {
            interior.tool = tool;
            interior.from = None;
        }
    }
    let lift = if input.pressed(KeyCode::BracketRight) || clicked == Some(Action::Plane(0.5)) {
        0.5
    } else if input.pressed(KeyCode::BracketLeft) || clicked == Some(Action::Plane(-0.5)) {
        -0.5
    } else {
        0.0
    };
    if lift != 0.0 {
        interior.plane = Some((plane_of(interior, &h) + lift).clamp(h.lo.y, h.hi.y));
    }
    interior.hover = hover_at(interior, &cam, cursor);
    // Removed: what's under the cursor (the hull's own points stay); a point takes
    // its lines with it.
    if input.pressed(KeyCode::Delete) || clicked == Some(Action::Remove) {
        match interior.hover {
            Some(Hover::Point(k)) if interior.plan.points[k].name.is_none() => {
                interior.plan.points.remove(k);
                interior.plan.lines.retain(|&(a, b)| a != k && b != k);
                for l in &mut interior.plan.lines {
                    l.0 -= usize::from(l.0 > k);
                    l.1 -= usize::from(l.1 > k);
                }
                interior.from = None;
            }
            Some(Hover::Line(k)) => {
                interior.plan.lines.remove(k);
            }
            _ => {}
        }
        interior.hover = None;
    }
    if clicked.is_some() {
        return true;
    }
    // A left press: a click if it ends where it began (a point placed or picked),
    // else a turn.
    if input.button_pressed(MouseButton::Left) {
        interior.press = Some(cursor);
    }
    let dragged = interior.press.is_some_and(|p| p.distance(cursor) > 4.0);
    if input.button_down(MouseButton::Left) && (dragged || interior.tool == Tool::Look) {
        interior.yaw -= d.x * 0.008;
        interior.pitch = (interior.pitch + d.y * 0.008).clamp(-1.5, 1.5);
    }
    if !input.button_down(MouseButton::Left)
        && let Some(_) = interior.press.take()
        && !dragged
    {
        match interior.tool {
            Tool::Point => {
                let ray = cam.ray(cursor);
                let y = plane_of(interior, &h);
                if ray.y.abs() > 1e-4 {
                    let t = (y - cam.eye.y) / ray.y;
                    if t > 0.0 {
                        let at = cam.eye + ray * t;
                        let at = Vec3::new((at.x * 4.0).round() / 4.0, y, (at.z * 4.0).round() / 4.0);
                        interior.plan.points.push(Point { at, name: None });
                    }
                }
            }
            Tool::Line => {
                if let Some(Hover::Point(k)) = interior.hover {
                    match interior.from {
                        Some(a) if a != k => {
                            if !interior.plan.lines.iter().any(|&(p, q)| (p, q) == (a, k) || (p, q) == (k, a)) {
                                interior.plan.lines.push((a, k));
                            }
                            // (On from there: a run of lines, point to point.)
                            interior.from = Some(k);
                        }
                        _ => interior.from = Some(k),
                    }
                }
            }
            Tool::Look => {}
        }
    }
    let radius = (h.hi - h.lo).length() * 0.5;
    let distance = (cam.eye - interior.target.unwrap_or((h.lo + h.hi) * 0.5)).length();
    if input.button_down(MouseButton::Right) || input.button_down(MouseButton::Middle) {
        let per_pixel = distance / cam.focal;
        let target = interior.target.unwrap_or((h.lo + h.hi) * 0.5);
        interior.target = Some(target + (-cam.right * d.x + cam.up * d.y) * per_pixel);
    }
    if input.scroll != 0.0 {
        interior.distance = Some((distance * 0.88f32.powf(input.scroll)).clamp(radius * 0.05, radius * 8.0));
    }
    // HOME: the view as it was (the plan kept).
    if input.pressed(KeyCode::Home) {
        let fresh = Interior::new();
        (interior.yaw, interior.pitch, interior.distance, interior.target) = (fresh.yaw, fresh.pitch, None, None);
    }
    true
}

pub fn draw(frame: &mut Frame, app: &App, place: &str, interior: &Interior) {
    let size = frame.size();
    let spec = app.ship.spec();
    frame.hud_rect(Vec2::ZERO, size, PAPER);
    frame.text(Vec2::new(12.0, 10.0), &format!("{place}   INTERIOR STUDIO - {}", spec.name), LABEL);
    frame.text_scaled(Vec2::new(12.0, size.y - 18.0), "LEFT-DRAG TURNS IT - RIGHT-DRAG MOVES IT - WHEEL: NEARER, FARTHER - HOME: AS IT WAS - ESC CLOSES", LABEL.scale(0.6), 0.7);
    // The toolbar: the tool in hand lit, the button under the cursor brighter.
    {
        use crate::hud::{draw_cell, Lamp};
        for (k, (key, name, action)) in TOOLBAR.iter().enumerate() {
            let (p, c) = button(k);
            let lamp = if *action == Action::Tool(interior.tool) || button_at(interior.cursor) == Some(*action) { Lamp::On } else { Lamp::Off };
            draw_cell(frame, p, c, key, name, lamp);
        }
    }
    let Some(h) = interior.hull(&spec.key) else {
        if spec.shape().walk.is_none() {
            frame.text(size * 0.5 - Vec2::new(200.0, 0.0), &format!("{} HAS NO MODEL TO LAY OUT", spec.name), LABEL);
            return;
        }
        // While its lines are worked out: a spinner.
        let c = size * 0.5;
        for i in 0..12 {
            let a = i as f32 / 12.0 * std::f32::consts::TAU + interior.spin * 6.0;
            let lit = ((i as f32 / 12.0 + interior.spin) % 1.0).max(0.15);
            let d = Vec2::new(a.cos(), a.sin());
            frame.hud_line(c + d * 12.0, c + d * 22.0, LABEL.scale(lit));
        }
        frame.text_scaled(c + Vec2::new(-56.0, 32.0), "DRAWING THE HULL", LABEL.scale(0.8), 0.8);
        return;
    };
    let cam = Camera::of(interior, &h, size);
    // The ground it stands on: a grid of 5 m under it, half as wide again as it is.
    let (lo, hi) = (h.lo, h.hi);
    let pad = (hi - lo) * 0.25;
    let step = 5.0;
    let (x0, x1) = (((lo.x - pad.x) / step).floor() as i32, ((hi.x + pad.x) / step).ceil() as i32);
    let (z0, z1) = (((lo.z - pad.z) / step).floor() as i32, ((hi.z + pad.z) / step).ceil() as i32);
    let ground = |x: f32, z: f32| Vec3::new(x, lo.y, z);
    for i in x0..=x1 {
        let x = i as f32 * step;
        if let (Some((a, _)), Some((b, _))) = (cam.project(ground(x, z0 as f32 * step)), cam.project(ground(x, z1 as f32 * step))) {
            frame.hud_line(a, b, GROUND);
        }
    }
    for i in z0..=z1 {
        let z = i as f32 * step;
        if let (Some((a, _)), Some((b, _))) = (cam.project(ground(x0 as f32 * step, z)), cam.project(ground(x1 as f32 * step, z))) {
            frame.hud_line(a, b, GROUND);
        }
    }
    // Walls of the same grid on its two far sides (from the camera), up past its top:
    // a corner to read its height against.
    let (gx0, gx1, gz0, gz1) = (x0 as f32 * step, x1 as f32 * step, z0 as f32 * step, z1 as f32 * step);
    let top = lo.y + ((hi.y - lo.y + pad.y) / step).ceil() * step;
    let far_x = if cam.eye.x > (lo.x + hi.x) * 0.5 { gx0 } else { gx1 };
    let far_z = if cam.eye.z > (lo.z + hi.z) * 0.5 { gz0 } else { gz1 };
    let seg = |frame: &mut Frame, a: Vec3, b: Vec3, c: Color| {
        if let (Some((pa, _)), Some((pb, _))) = (cam.project(a), cam.project(b)) {
            frame.hud_line(pa, pb, c);
        }
    };
    let rows = ((top - lo.y) / step).round() as i32;
    for k in 0..=rows {
        let y = lo.y + k as f32 * step;
        seg(frame, Vec3::new(far_x, y, gz0), Vec3::new(far_x, y, gz1), GROUND);
        seg(frame, Vec3::new(gx0, y, far_z), Vec3::new(gx1, y, far_z), GROUND);
    }
    for i in z0..=z1 {
        let z = i as f32 * step;
        seg(frame, Vec3::new(far_x, lo.y, z), Vec3::new(far_x, top, z), GROUND);
    }
    for i in x0..=x1 {
        let x = i as f32 * step;
        seg(frame, Vec3::new(x, lo.y, far_z), Vec3::new(x, top, far_z), GROUND);
    }
    // Its measures: its length along the ground on the near side, its beam across
    // the far end, its height up the far corner (against the walls); ticks at their
    // ends, the measure by their middles.
    {
        let near_x = if cam.eye.x > (lo.x + hi.x) * 0.5 { hi.x + 2.0 } else { lo.x - 2.0 };
        let far_x = if cam.eye.x > (lo.x + hi.x) * 0.5 { lo.x - 2.0 } else { hi.x + 2.0 };
        let far_z = if cam.eye.z > (lo.z + hi.z) * 0.5 { lo.z - 2.0 } else { hi.z + 2.0 };
        let dims = [
            (Vec3::new(near_x, lo.y, lo.z), Vec3::new(near_x, lo.y, hi.z), Vec3::X, hi.z - lo.z),
            (Vec3::new(lo.x, lo.y, far_z), Vec3::new(hi.x, lo.y, far_z), Vec3::Z, hi.x - lo.x),
            (Vec3::new(far_x, lo.y, far_z), Vec3::new(far_x, hi.y, far_z), Vec3::X, hi.y - lo.y),
        ];
        for (a, b, across, metres) in dims {
            seg(frame, a, b, DIMENSION);
            for p in [a, b] {
                seg(frame, p - across * 0.6, p + across * 0.6, DIMENSION);
            }
            if let Some((m, _)) = cam.project((a + b) * 0.5) {
                let text = format!("{metres:.1} M");
                let w = text.chars().count() as f32 * universe_engine::frame::GLYPH * 0.8;
                frame.text_scaled(m + Vec2::new(-w / 2.0, -14.0), &text, LABEL, 0.8);
            }
        }
    }
    // The hull: every line thin and see-through, fainter the farther it is (so its
    // depth reads through it).
    let middle = (cam.eye - (lo + hi) * 0.5).length();
    let radius = (hi - lo).length() * 0.5;
    let line = |frame: &mut Frame, a: Vec3, b: Vec3| {
        let (Some((pa, za)), Some((pb, zb))) = (cam.project(a), cam.project(b)) else { return };
        let near = (((middle + radius) - (za + zb) * 0.5) / (2.0 * radius)).clamp(0.0, 1.0);
        let alpha = 0.05 + 0.17 * near;
        frame.hud_line(pa, pb, Color([LINE[0], LINE[1], LINE[2], alpha]));
    };
    for [a, b] in &h.lines {
        line(frame, *a, *b);
    }
    // (Round things' outlines from here: one face toward the camera, one away.)
    for [a, b, n1, n2] in &h.bends {
        let to_eye = cam.eye - *a;
        if n1.dot(to_eye) * n2.dot(to_eye) < 0.0 {
            line(frame, *a, *b);
        }
    }
    // The work plane (placing points): its outline over the hull, at its height.
    let plane = plane_of(interior, &h);
    if interior.tool == Tool::Point {
        let c = [Vec3::new(lo.x, plane, lo.z), Vec3::new(hi.x, plane, lo.z), Vec3::new(hi.x, plane, hi.z), Vec3::new(lo.x, plane, hi.z)];
        for k in 0..4 {
            seg(frame, c[k], c[(k + 1) % 4], PLANE);
        }
        let mut x = (lo.x / 5.0).ceil() * 5.0;
        while x < hi.x {
            seg(frame, Vec3::new(x, plane, lo.z), Vec3::new(x, plane, hi.z), PLANE.scale(0.5));
            x += 5.0;
        }
        let mut z = (lo.z / 5.0).ceil() * 5.0;
        while z < hi.z {
            seg(frame, Vec3::new(lo.x, plane, z), Vec3::new(hi.x, plane, z), PLANE.scale(0.5));
            z += 5.0;
        }
        // Where a click would put a point.
        let ray = cam.ray(interior.cursor);
        if ray.y.abs() > 1e-4 {
            let t = (plane - cam.eye.y) / ray.y;
            if t > 0.0 {
                let at = cam.eye + ray * t;
                let at = Vec3::new((at.x * 4.0).round() / 4.0, plane, (at.z * 4.0).round() / 4.0);
                if let Some((q, _)) = cam.project(at) {
                    frame.hud_box(q - Vec2::splat(3.0), Vec2::splat(6.0), PICKED.scale(0.6));
                }
            }
        }
    }
    // The access plan: its lines bright, its points small squares (the hull's own,
    // named); what's under the cursor and a line's first point lit.
    let plan = &interior.plan;
    for (k, &(a, b)) in plan.lines.iter().enumerate() {
        let col = if interior.hover == Some(Hover::Line(k)) { PICKED } else { PATH };
        seg(frame, plan.points[a].at, plan.points[b].at, col);
    }
    if interior.tool == Tool::Line
        && let Some(a) = interior.from
        && let Some((pa, _)) = cam.project(plan.points[a].at)
    {
        frame.hud_line(pa, interior.cursor, PICKED.scale(0.6));
    }
    for (k, p) in plan.points.iter().enumerate() {
        let Some((q, _)) = cam.project(p.at) else { continue };
        let lit = interior.hover == Some(Hover::Point(k)) || interior.from == Some(k);
        let col = if lit { PICKED } else if p.name.is_some() { ANCHOR } else { PATH };
        frame.hud_rect(q - Vec2::splat(3.0), Vec2::splat(6.0), col);
        if let Some(name) = &p.name {
            frame.text_scaled(q + Vec2::new(6.0, -4.0), name, col, 0.7);
        }
    }
    // What's on: the tool, the plane's height (from the keel), how much is drawn.
    let tool = match interior.tool {
        Tool::Look => "LOOK: DRAG TO TURN",
        Tool::Point => "POINT: CLICK THE PLANE TO PUT ONE ([ ] MOVE IT)",
        Tool::Line => "LINE: CLICK A POINT, THEN ANOTHER (ESC: STOP)",
    };
    frame.text(Vec2::new(12.0, 54.0), &format!("{tool}    PLANE {:.1} M UP    {} POINTS  {} LINES", plane - lo.y, plan.points.len(), plan.lines.len()), LABEL);
}
