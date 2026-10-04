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

    /// Where `p` lands on screen, and how far in front it is (none: behind).
    fn project(&self, p: Vec3) -> Option<(Vec2, f32)> {
        let d = p - self.eye;
        let z = d.dot(self.forward);
        (z > 0.1).then(|| (self.centre + Vec2::new(d.dot(self.right), -d.dot(self.up)) * (self.focal / z), z))
    }
}

/// This frame's input. False: close it.
pub fn input(app: &mut App, ctx: &Context, interior: &mut Interior) -> bool {
    let input = &ctx.input;
    let spec = app.ship.spec();
    interior.spin += ctx.dt;
    interior.refresh(&spec.key, spec.shape());
    if input.pressed(KeyCode::Escape) {
        return false;
    }
    let Some(h) = interior.hull(&spec.key) else { return true };
    let size = ctx.hud_size.as_vec2();
    let cam = Camera::of(interior, &h, size);
    let d = input.mouse_delta;
    // Turned with the left button; moved with the right (or middle); nearer and
    // farther with the wheel; HOME: as it was.
    if input.button_down(MouseButton::Left) {
        interior.yaw -= d.x * 0.008;
        interior.pitch = (interior.pitch + d.y * 0.008).clamp(-1.5, 1.5);
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
    if input.pressed(KeyCode::Home) {
        *interior = Interior { lines: interior.lines.take(), job: interior.job.take(), ..Interior::new() };
    }
    true
}

pub fn draw(frame: &mut Frame, app: &App, place: &str, interior: &Interior) {
    let size = frame.size();
    let spec = app.ship.spec();
    frame.hud_rect(Vec2::ZERO, size, PAPER);
    frame.text(Vec2::new(12.0, 10.0), &format!("{place}   INTERIOR STUDIO - {}", spec.name), LABEL);
    frame.text_scaled(Vec2::new(12.0, size.y - 18.0), "LEFT-DRAG TURNS IT - RIGHT-DRAG MOVES IT - WHEEL: NEARER, FARTHER - HOME: AS IT WAS - ESC CLOSES", LABEL.scale(0.6), 0.7);
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
    // The hull: every line thin and see-through, fainter the farther it is (so its
    // depth reads through it).
    let middle = (cam.eye - (lo + hi) * 0.5).length();
    let radius = (hi - lo).length() * 0.5;
    let line = |frame: &mut Frame, a: Vec3, b: Vec3| {
        let (Some((pa, za)), Some((pb, zb))) = (cam.project(a), cam.project(b)) else { return };
        let near = (((middle + radius) - (za + zb) * 0.5) / (2.0 * radius)).clamp(0.0, 1.0);
        let alpha = 0.10 + 0.32 * near;
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
}
