//! The plant viewer, an app of its own (`freefall --app plant [layout key]`): a works snapped together from
//! its records (`world::plant`), no layout model. Its units stand at their bays as their models (a box at
//! their size until one is had); its runs are swept along their links' paths at their outside, bending at
//! their elbows' radius, on stands down to the ground; at each port a run ends on or taps stands its joint,
//! its family template's model scaled to the connector (a disc at its size where the template has none).
//! It runs no clocks. Up and down pick a layout, ENTER opens it; a right drag or left and right turn, the
//! wheel zooms, ESC closes.

use std::f64::consts::FRAC_PI_2;

use universe_engine::apps::{App, Next};
use universe_engine::glam::{DQuat, DVec3, Vec2, Vec3};
use universe_engine::{Camera, Color, Context, Frame, KeyCode, Light, MouseButton, Transform};
use universe_sim::world::plant::{self, joint_scale, template_model, Laid, Plant, TemplateModel};
use universe_sim::world::registry::{registry, Connector, RunCarries, RunKind};

use crate::palette::{DIM, PANEL, TEXT};

/// A point in the layout's frame (x along, y across, z up) in the view's (y up).
fn view(p: DVec3) -> DVec3 {
    DVec3::new(p.x, p.z, -p.y)
}

/// The colour a run is drawn in, by what it carries.
fn colour(l: &Laid) -> Color {
    carried(l.run.carries)
}

fn carried(c: Option<RunCarries>) -> Color {
    match c {
        Some(RunCarries::Gas) => Color::hex(0xc9c27a),
        Some(RunCarries::Liquid) => Color::hex(0x5f9fd6),
        Some(RunCarries::Bulk) => Color::hex(0x9a8f84),
        Some(RunCarries::Power) => Color::hex(0xd9854a),
        Some(RunCarries::Signal) => Color::hex(0x8ccf8c),
        None => Color::hex(0xb0b0b0),
    }
}

/// Whether a run's section is a box (a belt, a casing, a bus) rather than round.
fn boxed(l: &Laid) -> bool {
    matches!(l.run.kind, RunKind::Belt | RunKind::DragConveyor | RunKind::Busbar | RunKind::ConductorRail)
}

/// The run's centreline as laid: straight along each leg, round each corner on an arc at its elbow's radius
/// (a cable tray's: its riser's), cut short where the legs are too short for it. In the layout's frame.
pub fn centreline(l: &Laid) -> Vec<DVec3> {
    let radius = l.run.elbow.as_ref().map(|e| e.radius).or(l.run.riser.as_ref().map(|r| r.radius)).unwrap_or(0.0);
    let p = &l.path;
    let mut out = vec![p[0]];
    for i in 1..p.len().saturating_sub(1) {
        let (a, b) = (p[i] - p[i - 1], p[i + 1] - p[i]);
        let (la, lb) = (a.length(), b.length());
        let (u, v) = (a / la.max(1e-9), b / lb.max(1e-9));
        let turn = u.dot(v).clamp(-1.0, 1.0).acos();
        if turn < 1f64.to_radians() || radius <= 0.0 {
            out.push(p[i]);
            continue;
        }
        // (The arc meets each leg r tan(turn/2) from the corner, no more than half the leg.)
        let t = (radius * (turn / 2.0).tan()).min(la / 2.0).min(lb / 2.0);
        let (start, end) = (p[i] - u * t, p[i] + v * t);
        let steps = ((turn / 10f64.to_radians()).ceil() as usize).max(2);
        // A quadratic through the corner: tangent to both legs, close to the arc at these turns.
        for k in 0..=steps {
            let s = k as f64 / steps as f64;
            out.push(start * (1.0 - s) * (1.0 - s) + p[i] * 2.0 * s * (1.0 - s) + end * s * s);
        }
    }
    out.push(p[p.len() - 1]);
    out
}

/// A tube's faces along `line` (view frame): `sides` round (or a box `w` by `h`), each face with its normal.
fn tube(line: &[DVec3], w: f64, h: f64, square: bool, offset: f64, out: &mut Vec<([DVec3; 3], DVec3)>) {
    let ring: Vec<(f64, f64)> = if square { vec![(-0.5, -0.5), (0.5, -0.5), (0.5, 0.5), (-0.5, 0.5)] } else { (0..10).map(|k| { let a = k as f64 / 10.0 * std::f64::consts::TAU; (0.5 * a.cos(), 0.5 * a.sin()) }).collect() };
    let frames: Vec<(DVec3, DVec3, DVec3)> = (0..line.len())
        .map(|i| {
            let t = (line[(i + 1).min(line.len() - 1)] - line[i.saturating_sub(1)]).normalize_or_zero();
            let side = t.cross(DVec3::Y).try_normalize().unwrap_or(DVec3::X);
            let up = side.cross(t);
            (line[i] + side * offset, side, up)
        })
        .collect();
    for i in 0..line.len() - 1 {
        let ((a, sa, ua), (b, sb, ub)) = (frames[i], frames[i + 1]);
        for k in 0..ring.len() {
            let (p, q) = (ring[k], ring[(k + 1) % ring.len()]);
            let at = |c: DVec3, s: DVec3, u: DVec3, r: (f64, f64)| c + s * r.0 * w + u * r.1 * h;
            let (p0, p1, q0, q1) = (at(a, sa, ua, p), at(b, sb, ub, p), at(a, sa, ua, q), at(b, sb, ub, q));
            let n = (sa * (p.0 + q.0) + ua * (p.1 + q.1)).normalize_or_zero();
            out.push(([p0, p1, q1], n));
            out.push(([p0, q1, q0], n));
        }
    }
}

/// A plant's runs, stands and bare joints as faces, in the view's frame, with each face's colour.
struct Swept {
    faces: Vec<([DVec3; 3], DVec3, Color)>,
}

fn sweep(p: &Plant) -> Swept {
    let mut faces = Vec::new();
    let mut add = |f: Vec<([DVec3; 3], DVec3)>, c: Color| faces.extend(f.into_iter().map(|(t, n)| (t, n, c)));
    for l in &p.runs {
        let line: Vec<DVec3> = centreline(l).into_iter().map(view).collect();
        let [w, h] = l.run.outside;
        let mut f = Vec::new();
        // (Circuits side by side on one path: a cable's, a pitch apart.)
        let pitch = w * 1.2;
        for c in 0..l.count {
            tube(&line, w, h, boxed(l), (c as f64 - (l.count - 1) as f64 / 2.0) * pitch, &mut f);
        }
        add(f, colour(l));
        // Its stands: a post from under the run to the ground.
        let mut f = Vec::new();
        let post = (0.25 * w.max(h)).clamp(0.08, 0.4);
        for s in &l.supports {
            let top = s.z - h / 2.0;
            if top > 0.3 {
                tube(&[view(DVec3::new(s.x, s.y, 0.0)), view(DVec3::new(s.x, s.y, top))], post, post, true, 0.0, &mut f);
            }
        }
        add(f, Color::hex(0x6d747b));
        // Its blind ends: a plate a little wider than the run.
        let mut f = Vec::new();
        for (at, out) in &l.caps {
            tube(&[view(*at), view(*at + *out * 0.05)], w * 1.1, h * 1.1, boxed(l), 0.0, &mut f);
        }
        add(f, colour(l).scale(0.7));
    }
    Swept { faces }
}

/// How joint `c` is drawn: its template's model (by key), the model's axes stretched, and which of its axes
/// is the joint's (the line its flow crosses). None: drawn bare.
fn joint_model(c: &Connector) -> Option<(&'static str, Vec3, DVec3)> {
    let t = c.template.as_deref().and_then(|t| registry().template(t))?;
    let ratio = |a: Option<f64>, b: Option<f64>| match (a, b) {
        (Some(a), Some(b)) if b > 0.0 => (a / b) as f32,
        _ => 1.0,
    };
    match template_model(t)? {
        // (The contract's frame: the joint along +Y, its length along X, its width along Z.)
        TemplateModel::Own { reference, .. } => {
            let s = reference.map_or(DVec3::ONE, |r| joint_scale(c, r));
            Some((t.identity.key.as_str(), s.as_vec3(), DVec3::Y))
        }
        // (The older kit's frame: the run's axis along +X, across it Y and Z.)
        TemplateModel::Of { member, .. } => {
            let m = registry().connector(member)?;
            let (a, b) = (&c.physical, &m.physical);
            Some((member, Vec3::new(ratio(a.height, b.height), ratio(a.length, b.length), ratio(a.width, b.width)), DVec3::X))
        }
    }
}

pub struct PlantApp {
    /// Every layout laid of runs: key, name.
    list: Vec<(String, String)>,
    selected: usize,
    open: Option<(Plant, Swept)>,
    yaw: f64,
    pitch: f64,
    distance: f64,
    /// What the camera turns round: the middle of what's laid (view frame).
    centre: DVec3,
    camera: Camera,
}

impl PlantApp {
    /// The viewer, with layout `key` open (none: the first).
    pub fn new(key: Option<&str>) -> Result<Self, String> {
        let list: Vec<(String, String)> = registry().layouts.iter().filter(|l| l.links.iter().any(|k| k.run.is_some())).map(|l| (l.identity.key.clone(), l.identity.name.clone())).collect();
        if list.is_empty() {
            return Err("no layout laid of runs in the registry".into());
        }
        let selected = match key {
            Some(k) => list.iter().position(|(key, _)| key == k).ok_or_else(|| format!("no layout {k} laid of runs"))?,
            None => 0,
        };
        let mut app = PlantApp { list, selected, open: None, yaw: 0.7, pitch: 0.5, distance: 300.0, centre: DVec3::ZERO, camera: Camera::default() };
        app.go(selected);
        Ok(app)
    }

    fn go(&mut self, k: usize) {
        self.open = plant::lay(&self.list[k].0).map(|p| {
            let s = sweep(&p);
            (p, s)
        });
        // (The view on the box round its units and runs.)
        if let Some((p, _)) = &self.open {
            let points = p.units.iter().map(|u| u.at).chain(p.runs.iter().flat_map(|r| r.path.iter().copied()));
            let (lo, hi) = points.fold((DVec3::MAX, DVec3::MIN), |(lo, hi), q| (lo.min(q), hi.max(q)));
            if lo.x <= hi.x {
                self.centre = view((lo + hi) / 2.0);
                self.distance = (hi - lo).length().max(20.0) * 0.9;
            }
        }
    }

    fn camera_now(&self) -> Camera {
        let dir = DVec3::new(self.pitch.cos() * self.yaw.sin(), self.pitch.sin(), self.pitch.cos() * self.yaw.cos());
        let mut camera = Camera { position: self.centre + dir * self.distance, ..Default::default() };
        camera.look_at(self.centre, Vec3::Y);
        camera
    }
}

impl App for PlantApp {
    fn name(&self) -> &str {
        "plant"
    }

    fn status(&self) -> String {
        let p = self.open.as_ref().map(|(p, _)| p);
        serde_json::json!({"layout": p.map(|p| p.layout.identity.key.clone()), "units": p.map(|p| p.units.len()), "runs": p.map(|p| p.runs.len()),
            "joints": p.map(|p| p.joints.len()), "faults": p.map(|p| p.faults.clone())}).to_string()
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
            self.pitch = (self.pitch + input.mouse_delta.y as f64 * 0.006).clamp(0.05, FRAC_PI_2 - 0.01);
        }
        self.yaw += input.axis(KeyCode::ArrowRight, KeyCode::ArrowLeft) as f64 * 1.2 * ctx.dt as f64;
        self.distance = (self.distance * 0.85f64.powf(input.scroll as f64)).clamp(5.0, 5_000.0);
        self.camera = self.camera_now();
        Next::Stay
    }

    fn camera(&self) -> Camera {
        self.camera
    }

    fn draw(&self, frame: &mut Frame, _: &Context) {
        frame.light = Some(Light { position: DVec3::new(-3.0e11, 6.0e11, 2.0e11), color: [1.0, 0.97, 0.92], luminosity: 1.0, reference: universe_sim::units::AU, radius: 7.0e8 });
        let grid = Color::hex(0x1d2a36);
        let reach = (self.distance * 0.8).clamp(100.0, 2000.0);
        let mut v = -reach;
        while v <= reach {
            frame.line(DVec3::new(v, 0.0, -reach), DVec3::new(v, 0.0, reach), grid);
            frame.line(DVec3::new(-reach, 0.0, v), DVec3::new(reach, 0.0, v), grid);
            v += 10.0;
        }
        let Some((p, swept)) = &self.open else { return };
        // The units: their models, else their boxes.
        let (mut models, mut boxes) = (0, 0);
        for u in &p.units {
            let rot = DQuat::from_rotation_y(u.turn);
            match crate::visual_assets::request(&u.unit.identity.key, 1).filter(|v| v.neutral_error.is_none()) {
                Some(v) => {
                    frame.model_pbr(&v.model.with_lease(v.clone()), &Transform { position: view(u.at), rotation: rot.as_quat(), scale: 1.0 });
                    models += 1;
                }
                None => {
                    let ph = &u.unit.physical;
                    let half = DVec3::new(ph.length.unwrap_or(1.0), ph.height.unwrap_or(1.0), ph.width.unwrap_or(1.0)) / 2.0;
                    crate::scene::named_box(frame, view(u.at) + DVec3::Y * half.y, rot, half, &u.unit.identity.name, Color::hex(0x9fb4c8));
                    boxes += 1;
                }
            }
        }
        // The runs, stands and caps, lit by the sun's direction.
        let sun = DVec3::new(-0.4, 0.8, 0.27).normalize();
        for (t, n, c) in &swept.faces {
            frame.triangle(t[0], t[1], t[2], c.scale((0.45 + 0.55 * n.dot(sun).max(0.0)) as f32));
        }
        // The joints: their templates' models, else a disc at their size.
        let mut bare = 0;
        for j in &p.joints {
            let facing = view(j.facing).normalize_or_zero();
            let model = joint_model(j.connector).and_then(|(key, axes, axis)| crate::visual_assets::request(key, 1).filter(|v| v.neutral_error.is_none()).map(|v| (v, axes, axis)));
            match model {
                Some((v, axes, axis)) => {
                    let rot = DQuat::from_rotation_arc(axis, facing);
                    frame.model_pbr_scaled(&v.model.with_lease(v.clone()), &Transform { position: view(j.at), rotation: rot.as_quat(), scale: 1.0 }, axes);
                }
                None => {
                    let ph = &j.connector.physical;
                    let d = ph.width.or(ph.length).unwrap_or(0.3);
                    let mut f = Vec::new();
                    let a = view(j.at);
                    tube(&[a - facing * ph.height.unwrap_or(0.1) / 2.0, a + facing * ph.height.unwrap_or(0.1) / 2.0], d, d, false, 0.0, &mut f);
                    for (t, n) in f {
                        frame.triangle(t[0], t[1], t[2], Color::hex(0xd8d8d8).scale((0.45 + 0.55 * n.dot(sun).max(0.0)) as f32));
                    }
                    bare += 1;
                }
            }
        }
        // The panel.
        let line = universe_engine::GLYPH + 2.0;
        let (x, mut y) = (12.0, 12.0);
        const ROWS: usize = 12;
        let first = self.selected.saturating_sub(ROWS / 2).min(self.list.len().saturating_sub(ROWS));
        frame.hud_rect(Vec2::new(x - 6.0, y - 6.0), Vec2::new(520.0, line * (ROWS as f32 + 9.0)), PANEL);
        frame.text(Vec2::new(x, y), &format!("PLANT   LAYOUT {} OF {}", self.selected + 1, self.list.len()), TEXT);
        y += line * 1.5;
        for (k, (_, name)) in self.list.iter().enumerate().skip(first).take(ROWS) {
            let mark = if k == self.selected { ">" } else { " " };
            frame.text(Vec2::new(x, y), &format!("{mark}{}", name.to_uppercase()), if k == self.selected { TEXT } else { DIM });
            y += line;
        }
        y += line * 0.5;
        frame.text(Vec2::new(x, y), &p.layout.identity.name.to_uppercase(), TEXT);
        y += line;
        frame.text(Vec2::new(x, y), &format!("{} UNITS ({models} MODELS, {boxes} BOXES)  {} RUNS  {:.0} M", p.units.len(), p.runs.len(), p.runs.iter().map(|r| r.length()).sum::<f64>()), DIM);
        y += line;
        frame.text(Vec2::new(x, y), &format!("{} JOINTS ({} BY TEMPLATE, {bare} BARE)  {} STANDS", p.joints.len(), p.joints.len() - bare, p.runs.iter().map(|r| r.supports.len()).sum::<usize>()), DIM);
        y += line;
        let warn = Color::hex(0xff6a5a);
        frame.text(Vec2::new(x, y), &format!("{} FAULTS", p.faults.len()), if p.faults.is_empty() { DIM } else { warn });
        y += line;
        for f in p.faults.iter().take(3) {
            frame.text(Vec2::new(x, y), &f.to_uppercase(), warn);
            y += line;
        }
        // The key: what each colour carries, then the controls.
        let mut at = Vec2::new(12.0, frame.size().y - 24.0);
        for (word, c) in [("GAS", RunCarries::Gas), ("LIQUID", RunCarries::Liquid), ("BULK", RunCarries::Bulk), ("POWER", RunCarries::Power), ("SIGNAL", RunCarries::Signal)] {
            at.x = frame.text(at, word, carried(Some(c))).x + 16.0;
        }
        frame.text(at + Vec2::new(16.0, 0.0), "UP/DOWN LAYOUT  ENTER OPEN  RIGHT-DRAG VIEW  WHEEL ZOOM  ESC CLOSE", DIM);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every layout laid of runs opens and sweeps; a run's laid centreline starts and ends on its path's
    /// ends and passes no farther than its elbow's cut from each corner.
    #[test]
    fn layouts_sweep() {
        let mut app = PlantApp::new(None).unwrap();
        for k in 0..app.list.len() {
            app.go(k);
            let (p, s) = app.open.as_ref().unwrap();
            assert!(!s.faces.is_empty(), "{}: nothing swept", p.layout.identity.key);
            for l in &p.runs {
                let c = centreline(l);
                assert!(c[0].distance(l.path[0]) < 1e-9 && c[c.len() - 1].distance(l.path[l.path.len() - 1]) < 1e-9, "{}: ends", l.name);
                assert!(c.iter().all(|q| q.is_finite()), "{}: finite", l.name);
            }
        }
    }
}
