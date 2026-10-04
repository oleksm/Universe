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
    /// the point a path being laid runs on from, and where a press began (a click,
    /// if it doesn't move).
    tool: Tool,
    plane: Option<f32>,
    from: Option<usize>,
    press: Option<Vec2>,
    /// The plane's handle held (dragged up or down); the right button pressed to
    /// stop a path (so it doesn't move the view too).
    lifting: bool,
    stopped: bool,
    /// The point, or else the line, under the cursor, and the one picked (REMOVE
    /// takes it out); the cursor (HUD pixels).
    hover: Option<Hover>,
    pick: Option<Hover>,
    cursor: Vec2,
    /// The plan as it was before each change (UNDO goes back one), and the ones
    /// undone (REDO).
    undo: Vec<Plan>,
    redo: Vec<Plan>,
}

/// Points and the lines between them: where access must reach, and the ways it
/// goes, first as lines (their room comes later).
#[derive(Clone, Default, PartialEq)]
struct Plan {
    /// For which hull (its fixed points are taken from it).
    hull: String,
    points: Vec<Point>,
    lines: Vec<(usize, usize)>,
}

/// A point of the plan: where it is (the hull's frame), and the model's name for
/// it if it's one of the hull's own (the hatch, the cockpit...: those stay).
#[derive(Clone, PartialEq)]
struct Point {
    at: Vec3,
    name: Option<String>,
}

#[derive(Clone, Copy, Default, PartialEq)]
enum Tool {
    /// Looking (turning, moving, nearer and farther); a click picks a point or line.
    #[default]
    Look,
    /// Laying paths: a click on the work plane puts a point there, joined to the last;
    /// a click on a point joins to it and runs on from it.
    Path,
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
        self.tool = Tool::Path;
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

/// The toolbar: the tools (key, name).
const TOOLBAR: [(&str, &str, Tool); 2] = [("V", "LOOK", Tool::Look), ("P", "PATH", Tool::Path)];

/// Where toolbar button `k` is (one row along the top).
fn button(k: usize) -> (Vec2, Vec2) {
    (Vec2::new(12.0 + k as f32 * 124.0, 30.0), Vec2::new(120.0, 16.0))
}

fn button_at(q: Vec2) -> Option<Tool> {
    (0..TOOLBAR.len()).find(|&k| inside(button(k), q)).map(|k| TOOLBAR[k].2)
}

/// UNDO and REDO, after the tools (key, name, which: false undo, true redo).
const HISTORY: [(&str, &str, bool); 2] = [("^Z", "UNDO", false), ("^Y", "REDO", true)];

fn history_button(k: usize) -> (Vec2, Vec2) {
    button(TOOLBAR.len() + k)
}

fn inside((p, c): (Vec2, Vec2), q: Vec2) -> bool {
    q.x >= p.x && q.x <= p.x + c.x && q.y >= p.y && q.y <= p.y + c.y
}

/// The tool's panel, at the left: where it is and its size.
const PANEL: (Vec2, Vec2) = (Vec2::new(12.0, 56.0), Vec2::new(250.0, 198.0));

/// The panel's actions: the work plane down and up, what's picked out.
#[derive(Clone, Copy, PartialEq)]
enum Action {
    PlaneDown,
    PlaneUp,
    Remove,
}

/// The panel's buttons for the tool in hand (where, its label, what it does).
fn panel_buttons(tool: Tool) -> Vec<((Vec2, Vec2), &'static str, Action)> {
    let (p, c) = PANEL;
    let at = |row: f32, col: f32, w: f32| (Vec2::new(p.x + 8.0 + col, p.y + row), Vec2::new(w, 16.0));
    let w = (c.x - 16.0 - 6.0) / 2.0;
    match tool {
        Tool::Path => vec![(at(150.0, 0.0, w), "PLANE DOWN", Action::PlaneDown), (at(150.0, w + 6.0, w), "PLANE UP", Action::PlaneUp), (at(174.0, 0.0, c.x - 16.0), "REMOVE PICKED", Action::Remove)],
        Tool::Look => vec![(at(150.0, 0.0, c.x - 16.0), "REMOVE PICKED", Action::Remove)],
    }
}

fn panel_button_at(tool: Tool, q: Vec2) -> Option<Action> {
    panel_buttons(tool).into_iter().find(|(r, _, _)| inside(*r, q)).map(|(_, _, a)| a)
}

/// The work plane's handle: the middle of its edge nearest the camera that's on
/// screen (its frame).
fn plane_handle(cam: &Camera, h: &Hull, plane: f32) -> Vec3 {
    let (lo, hi, m) = (h.lo, h.hi, (h.lo + h.hi) * 0.5);
    let edges = [Vec3::new(lo.x, plane, m.z), Vec3::new(hi.x, plane, m.z), Vec3::new(m.x, plane, lo.z), Vec3::new(m.x, plane, hi.z)];
    let size = cam.centre * 2.0;
    let seen = |p: &Vec3| cam.project(*p).is_some_and(|(q, _)| q.x > 20.0 && q.y > 60.0 && q.x < size.x - 20.0 && q.y < size.y - 30.0);
    let mut by_near = edges;
    by_near.sort_by(|a, b| a.distance(cam.eye).total_cmp(&b.distance(cam.eye)));
    by_near.iter().copied().find(seen).unwrap_or(by_near[0])
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

/// This frame's input. False: close it. (A change to the plan is kept for UNDO.)
pub fn input(app: &mut App, ctx: &Context, interior: &mut Interior) -> bool {
    let input = &ctx.input;
    let ctrl = input.down(KeyCode::ControlLeft) || input.down(KeyCode::ControlRight);
    let shift = input.down(KeyCode::ShiftLeft) || input.down(KeyCode::ShiftRight);
    let click = |k: usize| input.button_pressed(MouseButton::Left) && inside(history_button(k), input.cursor);
    let undo = (ctrl && !shift && input.pressed(KeyCode::KeyZ)) || click(0);
    let redo = (ctrl && (input.pressed(KeyCode::KeyY) || (shift && input.pressed(KeyCode::KeyZ)))) || click(1);
    if undo || redo {
        let (from, to) = if undo { (&mut interior.undo, &mut interior.redo) } else { (&mut interior.redo, &mut interior.undo) };
        if let Some(plan) = from.pop() {
            to.push(std::mem::replace(&mut interior.plan, plan));
            interior.from = None;
            interior.pick = None;
            interior.hover = None;
        }
        return true;
    }
    let before = interior.plan.clone();
    let stay = input_plan(app, ctx, interior);
    if interior.plan != before && interior.plan.hull == before.hull {
        interior.undo.push(before);
        interior.redo.clear();
    }
    stay
}

fn input_plan(app: &mut App, ctx: &Context, interior: &mut Interior) -> bool {
    let input = &ctx.input;
    let spec = app.ship.spec();
    interior.spin += ctx.dt;
    interior.refresh(&spec.key, spec.shape());
    interior.seed(&spec.key, spec.shape());
    if input.pressed(KeyCode::Escape) {
        return false;
    }
    let Some(h) = interior.hull(&spec.key) else { return true };
    let size = ctx.hud_size.as_vec2();
    let cam = Camera::of(interior, &h, size);
    let cursor = input.cursor;
    interior.cursor = cursor;
    let d = input.mouse_delta;
    let pressed = input.button_pressed(MouseButton::Left);
    // The tools (toolbar or key); the panel's actions.
    let tool = if pressed { button_at(cursor) } else { None };
    for (key, t) in [(KeyCode::KeyV, Tool::Look), (KeyCode::KeyP, Tool::Path)] {
        if input.pressed(key) || tool == Some(t) {
            interior.tool = t;
            interior.from = None;
        }
    }
    let action = if pressed { panel_button_at(interior.tool, cursor) } else { None };
    let lift = if input.pressed(KeyCode::BracketRight) || action == Some(Action::PlaneUp) {
        0.5
    } else if input.pressed(KeyCode::BracketLeft) || action == Some(Action::PlaneDown) {
        -0.5
    } else {
        0.0
    };
    if lift != 0.0 {
        interior.plane = Some((plane_of(interior, &h) + lift).clamp(h.lo.y, h.hi.y));
    }
    interior.hover = hover_at(interior, &cam, cursor);
    // Removed: what's under the cursor (DEL), or what's picked (REMOVE); the hull's
    // own points stay; a point takes its lines with it.
    let gone = if input.pressed(KeyCode::Delete) { interior.hover.or(interior.pick) } else if action == Some(Action::Remove) { interior.pick } else { None };
    match gone {
        Some(Hover::Point(k)) if interior.plan.points[k].name.is_none() => {
            interior.plan.points.remove(k);
            interior.plan.lines.retain(|&(a, b)| a != k && b != k);
            for l in &mut interior.plan.lines {
                l.0 -= usize::from(l.0 > k);
                l.1 -= usize::from(l.1 > k);
            }
            interior.from = None;
            interior.pick = None;
        }
        Some(Hover::Line(k)) => {
            interior.plan.lines.remove(k);
            interior.pick = None;
        }
        _ => {}
    }
    if gone.is_some() {
        interior.hover = None;
    }
    if tool.is_some() || action.is_some() || (pressed && inside(PANEL, cursor)) {
        return true;
    }
    // The right button: stops a path being laid (and only that, this press).
    if input.button_pressed(MouseButton::Right) && interior.from.is_some() {
        interior.from = None;
        interior.stopped = true;
    }
    if !input.button_down(MouseButton::Right) {
        interior.stopped = false;
    }
    // The plane's handle (laying paths): held, it goes up and down with the mouse.
    let plane = plane_of(interior, &h);
    if pressed && interior.tool == Tool::Path && cam.project(plane_handle(&cam, &h, plane)).is_some_and(|(q, _)| q.distance(cursor) < 10.0) {
        interior.lifting = true;
    }
    if interior.lifting {
        if input.button_down(MouseButton::Left) {
            let handle = plane_handle(&cam, &h, plane);
            let per_pixel = (handle - cam.eye).length() / cam.focal;
            interior.plane = Some((plane - d.y * per_pixel).clamp(h.lo.y, h.hi.y));
        } else {
            interior.lifting = false;
            interior.plane = interior.plane.map(|y| (y * 10.0).round() / 10.0);
        }
        return true;
    }
    // A left press: a click if it ends where it began (a point laid or picked),
    // else a turn.
    if pressed {
        interior.press = Some(cursor);
    }
    let dragged = interior.press.is_some_and(|p| p.distance(cursor) > 4.0);
    if input.button_down(MouseButton::Left) && (dragged || interior.tool == Tool::Look) {
        interior.yaw -= d.x * 0.008;
        interior.pitch = (interior.pitch + d.y * 0.008).clamp(-1.5, 1.5);
    }
    if !input.button_down(MouseButton::Left)
        && interior.press.take().is_some()
        && !dragged
    {
        match interior.tool {
            Tool::Path => {
                // On a point: joined to it, and on from it. Else on the plane: a new
                // point there, joined to the last.
                let to = match interior.hover {
                    Some(Hover::Point(k)) => Some(k),
                    _ => {
                        let ray = cam.ray(cursor);
                        let t = if ray.y.abs() > 1e-4 { (plane - cam.eye.y) / ray.y } else { -1.0 };
                        (t > 0.0).then(|| {
                            let at = cam.eye + ray * t;
                            interior.plan.points.push(Point { at: Vec3::new((at.x * 4.0).round() / 4.0, plane, (at.z * 4.0).round() / 4.0), name: None });
                            interior.plan.points.len() - 1
                        })
                    }
                };
                if let Some(k) = to {
                    if let Some(a) = interior.from
                        && a != k
                        && !interior.plan.lines.iter().any(|&(p, q)| (p, q) == (a, k) || (p, q) == (k, a))
                    {
                        interior.plan.lines.push((a, k));
                    }
                    interior.from = Some(k);
                }
            }
            Tool::Look => interior.pick = interior.hover,
        }
    }
    let radius = (h.hi - h.lo).length() * 0.5;
    let distance = (cam.eye - interior.target.unwrap_or((h.lo + h.hi) * 0.5)).length();
    if (input.button_down(MouseButton::Right) && !interior.stopped) || input.button_down(MouseButton::Middle) {
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
        for (k, (key, name, tool)) in TOOLBAR.iter().enumerate() {
            let (p, c) = button(k);
            let lamp = if *tool == interior.tool || button_at(interior.cursor) == Some(*tool) { Lamp::On } else { Lamp::Off };
            draw_cell(frame, p, c, key, name, lamp);
        }
        for (k, (key, name, redo)) in HISTORY.iter().enumerate() {
            let (p, c) = history_button(k);
            let some = if *redo { !interior.redo.is_empty() } else { !interior.undo.is_empty() };
            let lamp = if !some { Lamp::Unavailable } else if inside((p, c), interior.cursor) { Lamp::On } else { Lamp::Off };
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
    if interior.tool == Tool::Path {
        let c = [Vec3::new(lo.x, plane, lo.z), Vec3::new(hi.x, plane, lo.z), Vec3::new(hi.x, plane, hi.z), Vec3::new(lo.x, plane, hi.z)];
        // (A light sheet, so it reads among the hull's lines.)
        if let [Some((a, _)), Some((b, _)), Some((cc, _)), Some((d, _))] = c.map(|p| cam.project(p)) {
            let fill = [Color([0.4, 1.0, 0.75, 0.06]); 3];
            frame.hud_triangle_colored([a, b, cc], fill);
            frame.hud_triangle_colored([a, cc, d], fill);
        }
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
        // Its handle: dragged up or down.
        let handle = plane_handle(&cam, &h, plane);
        if let Some((q, _)) = cam.project(handle) {
            let held = interior.lifting || q.distance(interior.cursor) < 10.0;
            // (A grip, not a square: a bar across with arrows up and down.)
            let col = if held { PICKED } else { PATH };
            for dy in [-1.5f32, 0.0, 1.5] {
                frame.hud_line(q + Vec2::new(-12.0, dy), q + Vec2::new(12.0, dy), col);
            }
            frame.hud_line(q - Vec2::new(0.0, 14.0), q + Vec2::new(0.0, 14.0), col);
            for dy in [-14.0f32, 14.0] {
                frame.hud_line(q + Vec2::new(0.0, dy), q + Vec2::new(-4.0, dy - 4.0 * dy.signum()), col);
                frame.hud_line(q + Vec2::new(0.0, dy), q + Vec2::new(4.0, dy - 4.0 * dy.signum()), col);
            }
            frame.text_scaled(q + Vec2::new(16.0, -4.0), &format!("PLANE {:.1} M", plane - lo.y), col, 0.7);
        }
        // Where a click would put a point (not over one: that joins to it).
        let ray = cam.ray(interior.cursor);
        if ray.y.abs() > 1e-4 && !matches!(interior.hover, Some(Hover::Point(_))) {
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
        let lit = interior.hover == Some(Hover::Line(k)) || interior.pick == Some(Hover::Line(k));
        seg(frame, plan.points[a].at, plan.points[b].at, if lit { PICKED } else { PATH });
    }
    if interior.tool == Tool::Path
        && let Some(a) = interior.from
        && let Some((pa, _)) = cam.project(plan.points[a].at)
    {
        frame.hud_line(pa, interior.cursor, PICKED.scale(0.6));
    }
    for (k, p) in plan.points.iter().enumerate() {
        let Some((q, _)) = cam.project(p.at) else { continue };
        let lit = interior.hover == Some(Hover::Point(k)) || interior.from == Some(k) || interior.pick == Some(Hover::Point(k));
        let col = if lit { PICKED } else if p.name.is_some() { ANCHOR } else { PATH };
        frame.hud_rect(q - Vec2::splat(3.0), Vec2::splat(6.0), col);
        if let Some(name) = &p.name {
            frame.text_scaled(q + Vec2::new(6.0, -4.0), name, col, 0.7);
        }
    }
    // The tool's panel: what it does and how, its actions, how much is drawn.
    {
        use crate::hud::{draw_cell, Lamp};
        let (p, c) = PANEL;
        frame.hud_rect(p, c, Color([0.02, 0.06, 0.13, 0.92]));
        frame.hud_box(p, c, PLANE.scale(1.5));
        let (title, help) = match interior.tool {
            Tool::Look => ("LOOK", "DRAG TO TURN IT, RIGHT-DRAG TO MOVE IT, WHEEL FOR NEARER OR FARTHER. CLICK A POINT OR A LINE TO PICK IT. DEL TAKES OUT WHAT'S UNDER THE CURSOR."),
            Tool::Path => ("PATH", "CLICK THE PLANE TO LAY A POINT, JOINED TO THE LAST ONE; CLICK A POINT TO JOIN TO IT AND GO ON FROM IT. RIGHT-CLICK STOPS. DRAG THE PLANE'S GRIP (ON ITS NEAR EDGE) UP OR DOWN."),
        };
        frame.text(p + Vec2::new(8.0, 8.0), title, LABEL);
        let mut y = p.y + 28.0;
        for line in crate::fmt::wrap(help, ((c.x - 16.0) / 8.0 * 1.25) as usize) {
            frame.text_scaled(Vec2::new(p.x + 8.0, y), &line, LABEL.scale(0.85), 0.8);
            y += 12.0;
        }
        let pick = match interior.pick {
            Some(Hover::Point(k)) => match &plan.points[k].name {
                Some(n) => format!("PICKED: {n} (THE HULL'S: STAYS)"),
                None => format!("PICKED: A POINT {:.1} M UP", plan.points[k].at.y - lo.y),
            },
            Some(Hover::Line(k)) => {
                let (a, b) = plan.lines[k];
                format!("PICKED: A LINE {:.1} M", plan.points[a].at.distance(plan.points[b].at))
            }
            None => format!("{} POINTS  {} LINES", plan.points.len(), plan.lines.len()),
        };
        frame.text_scaled(Vec2::new(p.x + 8.0, p.y + 126.0), &pick, PICKED.scale(0.9), 0.8);
        if interior.tool == Tool::Path {
            frame.text_scaled(Vec2::new(p.x + 8.0, p.y + 138.0), &format!("PLANE {:.1} M UP", plane - lo.y), PATH, 0.8);
        }
        for (r, name, a) in panel_buttons(interior.tool) {
            let lamp = if inside(r, interior.cursor) { Lamp::On } else { Lamp::Off };
            draw_cell(frame, r.0, r.1, "", name, if a == Action::Remove && interior.pick.is_none() { Lamp::Unavailable } else { lamp });
        }
    }
}
