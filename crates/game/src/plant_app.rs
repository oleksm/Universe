//! The plant viewer, an app of its own (`freefall --app plant [layout key]`): a works snapped together from
//! its records (`world::plant`), no layout model. Its units stand at their bays as their models (a box at
//! their size until one is had); its runs are swept along their links' paths at their outside, bending at
//! their elbows' radius, on stands down to the ground, and once their family's kit model is had laid of
//! its pieces (elbows, stands, caps, tees, a conveyor's terminals and idlers); at each port a run ends on or taps stands its joint,
//! its family template's model scaled to the connector (a disc at its size where the template has none).
//! It runs no clocks. Up and down pick a layout, ENTER opens it; a right drag or left and right turn, the
//! wheel zooms, ESC closes.

use std::f64::consts::FRAC_PI_2;
use std::sync::OnceLock;

use universe_engine::apps::{App, Next};
use universe_engine::glam::{DMat3, DQuat, DVec3, Vec2, Vec3};
use universe_engine::{Camera, Color, Context, Frame, KeyCode, Light, MouseButton, Transform};
use universe_sim::world::plant::{self, joint_scale, template_model, Laid, Plant, TemplateModel};
use universe_sim::world::registry::{registry, Connector, Run, RunCarries, RunKind, TemplateDraws};

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

type Face = ([DVec3; 3], DVec3, Color);

/// A run as drawn: swept whole (its straights, arcs, stands as posts, caps as plates), and once its kit
/// model is had, swept only where no piece serves, the rest of it the model's pieces.
struct RunView {
    swept: Vec<Face>,
    /// Its kit model's key and the run that model was built at (a sweep-and-pieces template's).
    kit: Option<(&'static str, &'static Run)>,
    pieces: OnceLock<Pieces>,
}

/// A run laid of its kit's pieces: the model, each piece's draw, and what is still swept.
struct Pieces {
    model: crate::visuals::Loaded,
    draws: Vec<Draw>,
    swept: Vec<Face>,
}

/// One piece of a kit model drawn: its part, its place, its stretch along the model's own axes.
struct Draw {
    part: u8,
    at: Transform,
    axes: Vec3,
}

/// The pieces a kit's run model holds: part k + 1 is `PIECES[k]`. (A node takes the first name it holds:
/// `support_head` before `head`, and `tee` last, as `steel` holds it.)
const PIECES: &[&str] = &["support_head", "support_leg", "head", "tail", "idler", "return_roll", "elbow", "riser", "cap", "straight", "rung", "tee"];

fn part(name: &str) -> u8 {
    PIECES.iter().position(|p| *p == name).map_or(0, |k| k as u8 + 1)
}

/// The model a run's pieces come from and the run it was built at: a sweep-and-pieces template's own, or
/// until it has one its `model_of` member's.
fn kit(l: &Laid) -> Option<(&'static str, &'static Run)> {
    let t = l.template.filter(|t| t.draws == TemplateDraws::SweepAndPieces)?;
    match template_model(t)? {
        TemplateModel::Own { reference, .. } => Some((t.identity.key.as_str(), registry().run(reference?)?)),
        TemplateModel::Of { member, .. } => Some((member, registry().run(member)?)),
    }
}

/// A piece at `at`, its own x, y, z along `x`, `y`, `z` (the layout's frame, square to each other),
/// stretched along them by `s`. (A model's x, y, z as built are glTF's x, -z, y, as the layout's are the
/// view's.)
fn place(part: u8, at: DVec3, (x, y, z): (DVec3, DVec3, DVec3), s: DVec3) -> Draw {
    let turn = DMat3::from_cols(view(x), view(z), -view(y));
    Draw { part, at: Transform { position: view(at), rotation: DQuat::from_mat3(&turn).as_quat(), scale: 1.0 }, axes: Vec3::new(s.x as f32, s.z as f32, s.y as f32) }
}

/// Axes for a piece along `x`: y level across it, z up from it (x straight up: y the layout's y).
fn axes_along(x: DVec3) -> (DVec3, DVec3, DVec3) {
    let x = x.normalize_or_zero();
    let y = DVec3::Z.cross(x).try_normalize().unwrap_or(DVec3::Y);
    (x, y, x.cross(y))
}

/// Level axes for a piece along `x`'s way in plan (straight up: the layout's x).
fn level(x: DVec3) -> (DVec3, DVec3, DVec3) {
    axes_along(DVec3::new(x.x, x.y, 0.0).try_normalize().unwrap_or(DVec3::X))
}

/// Where node `name` of `model` lies, for the piece drawn at `at` on `axes` stretched by `s` (layout frame).
fn node_at(model: &crate::visuals::Loaded, name: &str, at: DVec3, (x, y, z): (DVec3, DVec3, DVec3), s: DVec3) -> Option<DVec3> {
    let g = model.nodes.get(name)?.w_axis.truncate();
    Some(at + x * g.x * s.x + y * -g.z * s.y + z * g.y * s.z)
}

fn run_view(l: &Laid) -> RunView {
    let mut swept = Vec::new();
    let line: Vec<DVec3> = centreline(l).into_iter().map(view).collect();
    sweep_lines(l, &[line], &mut swept);
    let [w, h] = l.run.outside;
    // Its stands: a post from under the run to the ground.
    let mut f = Vec::new();
    let post = (0.25 * w.max(h)).clamp(0.08, 0.4);
    for s in &l.supports {
        let top = s.z - h / 2.0;
        if top > 0.3 {
            tube(&[view(DVec3::new(s.x, s.y, 0.0)), view(DVec3::new(s.x, s.y, top))], post, post, true, 0.0, &mut f);
        }
    }
    swept.extend(f.into_iter().map(|(t, n)| (t, n, Color::hex(0x6d747b))));
    // Its blind ends: a plate a little wider than the run.
    let mut f = Vec::new();
    for (at, out) in &l.caps {
        tube(&[view(*at), view(*at + *out * 0.05)], w * 1.1, h * 1.1, boxed(l), 0.0, &mut f);
    }
    swept.extend(f.into_iter().map(|(t, n)| (t, n, colour(l).scale(0.7))));
    RunView { swept, kit: kit(l), pieces: OnceLock::new() }
}

/// `lines` (view frame) swept at the run's outside, as many circuits as it has side by side.
fn sweep_lines(l: &Laid, lines: &[Vec<DVec3>], out: &mut Vec<Face>) {
    let [w, h] = l.run.outside;
    let mut f = Vec::new();
    for line in lines.iter().filter(|line| line.len() > 1) {
        for c in 0..l.count {
            tube(line, w, h, boxed(l), (c as f64 - (l.count - 1) as f64 / 2.0) * w * 1.2, &mut f);
        }
    }
    out.extend(f.into_iter().map(|(t, n)| (t, n, colour(l))));
}

/// Run `l` laid of its kit model's pieces (`docs/requests/blender-plant-kit.md` on fso): a 90 degree corner
/// is its elbow, each support its head with a leg from its `leg_top` to the ground, each blind end its cap,
/// each branch its tee, a conveyor's terminals at its ends with their legs, its idlers and rolls along its
/// straights at their pitch. The straights are swept (a belt's: its straight piece stretched), cut short where a piece draws itself (an elbow's
/// bend, a terminal's transition); what the model has no piece for stays swept. Pieces are scaled to the
/// run from the one the model was built at: across and up by its outside, an elbow by its radius.
fn pieces(l: &Laid, model: crate::visuals::Loaded, at: &Run) -> Pieces {
    let has = |name: &str| model.nodes.contains_key(name);
    let r = l.run;
    let ratio = |a: f64, b: f64| if b > 0.0 { a / b } else { 1.0 };
    let (across, up) = (ratio(r.outside[0], at.outside[0]), ratio(r.outside[1], at.outside[1]));
    let section = DVec3::new(across, across, up);
    let mut draws = Vec::new();
    let mut swept = Vec::new();
    let p = &l.path;
    let first = (p[1] - p[0]).normalize_or_zero();
    let last = (p[p.len() - 1] - p[p.len() - 2]).normalize_or_zero();
    // A leg from `top` to the ground.
    let leg = |draws: &mut Vec<Draw>, top: DVec3, way: DVec3| {
        if has("support_leg") && top.z > 0.05 {
            draws.push(place(part("support_leg"), top, level(way), DVec3::new(across, across, top.z)));
        }
    };
    // The terminals, and how far short of each end the straight stops.
    let (mut start, mut end) = (p[0], p[p.len() - 1]);
    if let (Some(t), true) = (&r.terminals.tail, has("tail")) {
        start += first * t.transition.unwrap_or(0.0);
        let axes = axes_along(first);
        draws.push(place(part("tail"), p[0], axes, section));
        if let Some(top) = node_at(&model, "leg_top_tail", p[0], axes, section) {
            leg(&mut draws, top, first);
        }
    }
    if let (Some(t), true) = (&r.terminals.head, has("head")) {
        end -= last * t.transition.unwrap_or(0.0);
        let axes = axes_along(last);
        draws.push(place(part("head"), p[p.len() - 1], axes, section));
        if let Some(top) = node_at(&model, "leg_top_head", p[p.len() - 1], axes, section) {
            leg(&mut draws, top, last);
        }
    }
    // The straights between the elbows (a corner of another angle bent as swept).
    let radius = r.elbow.as_ref().map_or(0.0, |e| e.radius);
    let built = at.elbow.as_ref().map_or(0.0, |e| e.radius);
    let mut lines: Vec<Vec<DVec3>> = vec![vec![start]];
    for i in 1..p.len() - 1 {
        let (a, b) = (p[i] - p[i - 1], p[i + 1] - p[i]);
        let (before, after) = (a.normalize_or_zero(), b.normalize_or_zero());
        let turn = before.dot(after).clamp(-1.0, 1.0).acos();
        let current = lines.last_mut().expect("a line");
        if has("elbow") && radius > 0.0 && (turn - FRAC_PI_2).abs() < 2f64.to_radians() && a.length() >= radius && b.length() >= radius {
            current.push(p[i] - before * radius);
            // (Its bend in the plane of its legs: a drag conveyor's turns +x to +z as built, any other's +x to +y.)
            let axes = if r.kind == RunKind::DragConveyor { (before, after.cross(before), after) } else { (before, after, before.cross(after)) };
            draws.push(place(part("elbow"), p[i] - before * radius, axes, DVec3::new(ratio(radius, built), ratio(radius, built), up)));
            lines.push(vec![p[i] + after * radius]);
        } else {
            current.push(p[i]);
        }
    }
    lines.last_mut().expect("a line").push(end);
    // (Any other corner bent as the whole sweep bends it.)
    let lines: Vec<Vec<DVec3>> = lines.into_iter().map(|line| {
        let laid = Laid { name: String::new(), run: l.run, template: None, count: 1, path: line, elbows: vec![], supports: vec![], caps: vec![], tees: vec![] };
        centreline(&laid)
    }).collect();
    // The idlers and rolls along the straights, at their pitch.
    for rep in r.repeat.iter().filter(|rep| has(&rep.piece)) {
        for line in &lines {
            for w in line.windows(2) {
                let (d, n) = (w[1] - w[0], ((w[0].distance(w[1]) / rep.pitch).floor() as usize).max(1));
                for k in 0..n {
                    draws.push(place(part(&rep.piece), w[0] + d * ((k as f64 + 0.5) / n as f64), axes_along(d), section));
                }
            }
        }
    }
    // (An open run whose idlers stand along it, a belt, is its straight piece stretched along each leg: its
    // belt and stringers, the template's profile, which a swept box would hide the idlers in.)
    if !r.repeat.is_empty() && has("straight") {
        for w in lines.iter().flat_map(|line| line.windows(2)) {
            draws.push(place(part("straight"), w[0], axes_along(w[1] - w[0]), DVec3::new(w[0].distance(w[1]), across, up)));
        }
    } else {
        let view_lines: Vec<Vec<DVec3>> = lines.iter().map(|line| line.iter().copied().map(view).collect()).collect();
        sweep_lines(l, &view_lines, &mut swept);
    }
    // The supports: a head holding the run, a leg from it to the ground.
    let [w, h] = r.outside;
    let post = (0.25 * w.max(h)).clamp(0.08, 0.4);
    for s in &l.supports {
        let way = plant::along(p, *s);
        if has("support_head") {
            let axes = level(way);
            draws.push(place(part("support_head"), *s, axes, section));
            if let Some(top) = node_at(&model, "leg_top", *s, axes, section) {
                leg(&mut draws, top, way);
            }
        } else if s.z - h / 2.0 > 0.3 {
            let mut f = Vec::new();
            tube(&[view(DVec3::new(s.x, s.y, 0.0)), view(DVec3::new(s.x, s.y, s.z - h / 2.0))], post, post, true, 0.0, &mut f);
            swept.extend(f.into_iter().map(|(t, n)| (t, n, Color::hex(0x6d747b))));
        }
    }
    // The blind ends and the branches.
    for (at, out) in &l.caps {
        if has("cap") {
            draws.push(place(part("cap"), *at, axes_along(*out), section));
        }
    }
    for t in &l.tees {
        let side = (t.branch - t.along * t.branch.dot(t.along)).try_normalize();
        if let (true, Some(y)) = (has("tee"), side) {
            draws.push(place(part("tee"), t.at, (t.along, y, t.along.cross(y)), section));
        }
    }
    Pieces { model, draws, swept }
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
    open: Option<(Plant, Vec<RunView>)>,
    orbit: crate::orbit_cam::OrbitCam,
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
        let mut app = PlantApp { list, selected, open: None, orbit: crate::orbit_cam::OrbitCam::new(0.7, 0.5, 300.0), centre: DVec3::ZERO, camera: Camera::default() };
        app.go(selected);
        Ok(app)
    }

    fn go(&mut self, k: usize) {
        self.open = plant::lay(&self.list[k].0).map(|p| {
            let runs = p.runs.iter().map(run_view).collect();
            (p, runs)
        });
        // (The view on the box round its units and runs.)
        if let Some((p, _)) = &self.open {
            let points = p.units.iter().map(|u| u.at).chain(p.runs.iter().flat_map(|r| r.path.iter().copied()));
            let (lo, hi) = points.fold((DVec3::MAX, DVec3::MIN), |(lo, hi), q| (lo.min(q), hi.max(q)));
            if lo.x <= hi.x {
                self.centre = view((lo + hi) / 2.0);
                self.orbit.distance = (hi - lo).length().max(20.0) * 0.9;
            }
        }
    }

    fn camera_now(&self) -> Camera {
        self.orbit.camera(self.centre, universe_engine::glam::DQuat::IDENTITY)
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
        self.orbit.drag(input, &[MouseButton::Right], (0.05, crate::orbit_cam::PITCH_LIMIT));
        self.orbit.keys(input, ctx.dt as f64, None);
        self.orbit.zoom(input, ctx.dt as f64, false, (5.0, 5_000.0));
        self.camera = self.camera_now();
        Next::Stay
    }

    fn camera(&self) -> Camera {
        self.camera
    }

    fn draw(&self, frame: &mut Frame, _: &Context) {
        frame.light = Some(Light { position: DVec3::new(-3.0e11, 6.0e11, 2.0e11), color: [1.0, 0.97, 0.92], luminosity: 1.0, reference: universe_sim::units::AU, radius: 7.0e8 });
        let grid = Color::hex(0x1d2a36);
        let reach = (self.orbit.distance * 0.8).clamp(100.0, 2000.0);
        let mut v = -reach;
        while v <= reach {
            frame.line(DVec3::new(v, 0.0, -reach), DVec3::new(v, 0.0, reach), grid);
            frame.line(DVec3::new(-reach, 0.0, v), DVec3::new(reach, 0.0, v), grid);
            v += 10.0;
        }
        let Some((p, runs)) = &self.open else { return };
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
        // The runs: their kits' pieces where had, the rest swept, lit by the sun's direction.
        let sun = DVec3::new(-0.4, 0.8, 0.27).normalize();
        let mut kitted = 0;
        for (l, v) in p.runs.iter().zip(runs) {
            if v.pieces.get().is_none()
                && let Some((key, at)) = v.kit
                && let Some(model) = crate::visual_assets::request_parts(key, PIECES).filter(|m| m.neutral_error.is_none())
            {
                let _ = v.pieces.set(pieces(l, model, at));
            }
            let faces = match v.pieces.get() {
                Some(k) => {
                    for d in &k.draws {
                        frame.model_pbr_scaled(&k.model.model.with_lease(k.model.clone()), &d.at, d.axes, d.part);
                    }
                    kitted += 1;
                    &k.swept
                }
                None => &v.swept,
            };
            for (t, n, c) in faces {
                frame.triangle(t[0], t[1], t[2], c.scale((0.45 + 0.55 * n.dot(sun).max(0.0)) as f32));
            }
        }
        // The joints: their templates' models, else a disc at their size.
        let mut bare = 0;
        for j in &p.joints {
            let facing = view(j.facing).normalize_or_zero();
            let model = joint_model(j.connector).and_then(|(key, axes, axis)| crate::visual_assets::request(key, 1).filter(|v| v.neutral_error.is_none()).map(|v| (v, axes, axis)));
            match model {
                Some((v, axes, axis)) => {
                    let rot = DQuat::from_rotation_arc(axis, facing);
                    frame.model_pbr_scaled(&v.model.with_lease(v.clone()), &Transform { position: view(j.at), rotation: rot.as_quat(), scale: 1.0 }, axes, 0);
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
        frame.text(Vec2::new(x, y), &format!("{} UNITS ({models} MODELS, {boxes} BOXES)  {} RUNS ({kitted} OF KIT PIECES)  {:.0} M", p.units.len(), p.runs.len(), p.runs.iter().map(|r| r.length()).sum::<f64>()), DIM);
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
            let (p, v) = app.open.as_ref().unwrap();
            assert!(v.iter().all(|v| !v.swept.is_empty()), "{}: a run not swept", p.layout.identity.key);
            for l in &p.runs {
                let c = centreline(l);
                assert!(c[0].distance(l.path[0]) < 1e-9 && c[c.len() - 1].distance(l.path[l.path.len() - 1]) < 1e-9, "{}: ends", l.name);
                assert!(c.iter().all(|q| q.is_finite()), "{}: finite", l.name);
            }
        }
    }
}
