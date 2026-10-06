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
const PICKED: Color = Color([1.0, 0.85, 0.35, 1.0]);
const PLANE: Color = Color([0.4, 1.0, 0.75, 0.35]);
const CLASH: Color = Color([1.0, 0.3, 0.25, 1.0]);
/// A doorway's frame.
const DOOR_FRAME: Color = Color([1.0, 0.65, 0.2, 1.0]);
/// The deck studio's floors and walls, shown here.
const DECK_FLOOR: Color = Color([1.0, 0.8, 0.5, 0.16]);
const DECK_WALL: Color = Color([1.0, 0.8, 0.5, 0.55]);
/// A walled group's walls (see-through panels).
const WALL: Color = Color([0.75, 0.9, 1.0, 0.28]);

/// The kinds of point, by their colour (the legend's, in its order): the hull's
/// own (named) by what they are, and the ones laid.
const SORTS: [(&str, Color); 7] = [
    ("ENTRY: HATCH, DOORS", Color([1.0, 1.0, 1.0, 1.0])),
    ("DASH", Color([0.6, 1.0, 0.35, 1.0])),
    ("WINDOW", Color([0.45, 0.9, 1.0, 1.0])),
    ("SERVICE", Color([0.8, 0.6, 1.0, 1.0])),
    ("MINING OPENING", Color([1.0, 1.0, 0.3, 1.0])),
    ("MOUNTS, DOCKS, ENGINES", Color([1.0, 0.6, 0.3, 1.0])),
    ("LAID", Color([0.4, 1.0, 0.75, 0.95])),
];

/// A point's kind (its colour, its name in the legend), by its name.
fn sort(name: Option<&str>) -> (Color, &'static str) {
    let k = sort_index(name);
    (SORTS[k].1, SORTS[k].0)
}

/// A point's kind, its number in `SORTS`.
fn sort_index(name: Option<&str>) -> usize {
    match name {
        None => 6,
        Some(n) if n == "HATCH" || n.starts_with("DOOR") => 0,
        Some(n) if n.starts_with("DASH") => 1,
        Some(n) if n.starts_with("WINDOW") => 2,
        Some(n) if n.starts_with("SERVICE") => 3,
        Some(n) if n.starts_with("MINING") => 4,
        Some(_) => 5,
    }
}

/// The layers, each shown or hidden: (name, the layer it's under, its colour).
/// Points' kinds are 3.. in `SORTS`' order.
const LAYERS: [(&str, Option<usize>, Option<Color>); 24] = [
    ("HULL", None, Some(Color([0.55, 0.8, 1.0, 0.8]))),
    ("GRID, MEASURES", None, Some(Color([0.75, 0.88, 1.0, 0.75]))),
    ("POINTS", None, None),
    ("ENTRY", Some(2), Some(SORTS[0].1)),
    ("DASH", Some(2), Some(SORTS[1].1)),
    ("WINDOWS", Some(2), Some(SORTS[2].1)),
    ("SERVICE", Some(2), Some(SORTS[3].1)),
    ("MINING", Some(2), Some(SORTS[4].1)),
    ("MOUNTS, ENGINES", Some(2), Some(SORTS[5].1)),
    ("LAID", Some(2), Some(SORTS[6].1)),
    ("NAMES", Some(2), None),
    ("ACCESS", None, None),
    ("LINES", Some(11), Some(PATH)),
    ("OUTLINES", Some(11), Some(Color([0.4, 1.0, 0.75, 0.45]))),
    ("WALLS", Some(11), Some(Color([0.75, 0.9, 1.0, 0.6]))),
    ("HATCHES", Some(11), Some(Color([1.0, 0.65, 0.2, 1.0]))),
    ("CLASHES", Some(11), Some(Color([1.0, 0.3, 0.25, 1.0]))),
    ("DECKS (2D)", None, None),
    ("FLOORS", Some(17), Some(Color([1.0, 0.8, 0.5, 0.6]))),
    ("WALLS", Some(17), Some(Color([1.0, 0.8, 0.5, 0.9]))),
    ("HOLLOW MAP", None, Some(Color([0.4, 1.0, 0.55, 0.8]))),
    ("REACH", None, Some(Color([0.4, 1.0, 0.55, 0.8]))),
    ("MODULES", None, Some(MODULE)),
    ("FRAME", None, Some(Color([0.4, 1.0, 0.5, 1.0]))),
];

/// Modules placed: their colour.
const MODULE: Color = Color([0.8, 0.6, 1.0, 1.0]);

/// The layers by name, for the drawing (both studios).
pub mod layer {
    pub const HULL: usize = 0;
    pub const GRID: usize = 1;
    pub const NAMES: usize = 10;
    pub const LINES: usize = 12;
    pub const ROOMS: usize = 13;
    pub const WALLS: usize = 14;
    pub const HATCHES: usize = 15;
    pub const CLASHES: usize = 16;
    pub const DECK_FLOORS: usize = 18;
    pub const DECK_WALLS: usize = 19;
    pub const HOLLOW: usize = 20;
    pub const REACH: usize = 21;
    pub const MODULES: usize = 22;
    pub const FRAME: usize = 23;
}

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
    /// The plane's level as the drag has it, before a snap to the selected point.
    lift_free: Option<f32>,
    /// The point, or else the line, under the cursor, and the one picked (REMOVE
    /// takes it out); the cursor (HUD pixels).
    hover: Option<Hover>,
    pick: Option<Hover>,
    /// More lines picked with it (SHIFT, or the rest of a picked line's group).
    more: Vec<usize>,
    cursor: Vec2,
    /// The plan as it was before each change (UNDO goes back one), and the ones
    /// undone (REDO).
    undo: Vec<Plan>,
    redo: Vec<Plan>,
    /// The cross-section new lines get; the slider held (its width 0, height 1).
    profile: Profile,
    slider: Option<usize>,
    /// Points laid dropped to the hull's floor under the work plane.
    snap_floor: bool,
    /// The hatch new doorways get (the DOOR tool's shape, size and slide).
    hatch: Hatch,
    /// What the ship is fitted with (and its hold), to be placed; the one picked (its
    /// number there), the one under the cursor; where they clash (with the hull or a
    /// tube), as last worked out for these blocks.
    fit: Vec<Fitted>,
    module: Option<usize>,
    module_hover: Option<usize>,
    /// The placed module picked (its number among the plan's), the one under the
    /// cursor; how far down the fit's list it's scrolled (rows).
    block: Option<usize>,
    block_hover: Option<usize>,
    fit_top: usize,
    /// A dialog open over the studio: NEW (which hull, or none) or OPEN (which design).
    dialog: Option<Dialog>,
    block_clash: Option<(Vec<Block>, Vec<bool>)>,
    /// FRAME: the stock new members are cut from (its number in `stocks()`), the load
    /// case shown (none: each member's worst), the member under the cursor and the
    /// one picked, where the member being laid starts; what the frame bears, as last
    /// worked out (for which plan), and being worked out (on a thread of its own).
    stock: usize,
    stock_top: usize,
    case: Option<usize>,
    beam_hover: Option<usize>,
    beam_pick: Option<usize>,
    beam_from: Option<Vec3>,
    /// AUTO-SIZE free to pick any material.
    mix: bool,
    /// TRUSS: laying a truss (its first corner, once clicked); its depth and its
    /// stations' spacing (m); AUTO-SIZE being worked out (for which plan).
    truss_mode: bool,
    truss_from: Option<Vec3>,
    depth: f32,
    spacing: f32,
    sizing: Option<(Plan, mpsc::Receiver<Sized>)>,
    bearing: Option<(Plan, Arc<Bearing>)>,
    bearing_job: Option<(Plan, mpsc::Receiver<Bearing>)>,
    /// The layers hidden (by number in `LAYERS`); the panel folded up; groups folded.
    hidden: [bool; LAYERS.len()],
    layers_folded: bool,
    group_folded: [bool; LAYERS.len()],
    /// The hull at the work plane's height: for which height, each metre cell's
    /// middle and what's there (hollow, solid); and being worked out, for which.
    hollow: Option<(f32, Arc<Vec<Cell>>)>,
    hollow_job: Option<(f32, mpsc::Receiver<Vec<Cell>>)>,
    /// A slide under way has its undo step already; SHIFT held (squaring a path).
    sliding: bool,
    shift: bool,
    /// WALK HERE pressed: the next click on a point or a tube is where; and a walk
    /// asked for: feet (the hull's frame) and facing (see `shipyard`).
    walk_armed: bool,
    pub walk: Option<(universe_engine::glam::DVec3, f64)>,
    /// The plan as last checked against the hull, and each line's clashes: where
    /// its room cuts into the hull's walls, structure or machinery.
    checked: Option<(Plan, Vec<Vec<Vec3>>)>,
    /// The plan as its walls were last worked out, and their panels.
    panelled: Option<(Plan, Vec<Panel>)>,
    /// The deck studio's decks as last drawn here: the plan, and its floors and
    /// walls (each a flat outline, the hull's frame; floor or wall).
    decks: Option<(universe_sim::world::deckplan::DeckPlan, Vec<DeckShape>)>,
    /// The hull's deck plan as last saved or opened (none: none; not yet looked
    /// for: none at all), as it is now (none: no decks), and one opened to be put in
    /// the deck studio.
    saved_decks: Option<Option<universe_sim::world::deckplan::DeckPlan>>,
    decks_now: Option<universe_sim::world::deckplan::DeckPlan>,
    load_decks: Option<universe_sim::world::deckplan::DeckPlan>,
    /// The reach check's findings, for which plan; and being worked out.
    reach: Option<(Plan, Arc<Reach>)>,
    reach_job: Option<(Plan, mpsc::Receiver<Reach>)>,
    /// The plan as last saved or opened; closing with unsaved changes asked
    /// (`confirm`); a message for a while (s).
    saved: Option<Plan>,
    confirm: bool,
    message: Option<(String, f32)>,
}

/// A line's room, its cross-section along it round it (the line its axis): the
/// edges to draw (each end's outline and the lines between their corners), and the
/// points through it tested against the hull. None for a bare line.
fn room(a: Vec3, b: Vec3, p: Profile) -> (Vec<[Vec3; 2]>, Vec<Vec3>) {
    let corners = p.corners();
    if corners.is_empty() {
        return (Vec::new(), Vec::new());
    }
    let along = b - a;
    let len = along.length();
    // (Across it: level, and up, as far as the line allows; a vertical line, x and z.)
    let d = along / len.max(1e-6);
    let u = d.cross(Vec3::Y).try_normalize().unwrap_or(Vec3::X);
    let v = u.cross(d);
    let at = |o: Vec3, c: Vec2| o + u * c.x + v * c.y;
    let mut edges = Vec::new();
    let n = corners.len();
    for k in 0..n {
        let (c0, c1) = (corners[k], corners[(k + 1) % n]);
        edges.push([at(a, c0), at(a, c1)]);
        edges.push([at(b, c0), at(b, c1)]);
        edges.push([at(a, c0), at(b, c0)]);
    }
    // (Through it every 25 cm along, at its middle and its corners pulled in a
    // hand's width; its ends left, where it meets a point.)
    let mut samples = Vec::new();
    let steps = (len / 0.25).floor() as usize;
    for i in 1..steps {
        let t = i as f32 * 0.25;
        if t < 0.4 || t > len - 0.4 {
            continue;
        }
        let o = a + d * t;
        samples.push(o);
        for c in &corners {
            let r = c.length();
            samples.push(at(o, *c * ((r - 0.1).max(0.0) / r.max(1e-6))));
        }
    }
    (edges, samples)
}

/// Where each line's room clashes with the hull: cuts into its material (its
/// walls, structure, machinery). (Open to the outside isn't a clash: a bay with its
/// doors open, a hatch.)
fn clashes(mesh: &universe_sim::world::walk::WalkMesh, plan: &Plan) -> Vec<Vec<Vec3>> {
    use universe_sim::world::deckplan::in_material;
    use universe_engine::glam::DVec2;
    plan.lines.iter().enumerate().map(|(k, &(_, _, profile))| {
        let (pa, pb) = plan.axis(k);
        room(pa, pb, profile).1.into_iter().filter(|p| in_material(mesh, DVec2::new(p.x as f64, p.z as f64), p.y as f64)).collect()
    }).collect()
}

/// Which of its placed modules clash: some of it (a few points through it) in the
/// hull's material or outside it, or a tube's room in it.
fn block_clashes(mesh: Option<&universe_sim::world::walk::WalkMesh>, i: &Interior) -> Vec<bool> {
    use universe_engine::glam::DVec2;
    use universe_sim::world::deckplan::{enclosed, in_material};
    let rooms: Vec<Vec3> = (0..i.plan.lines.len()).flat_map(|k| {
        let (pa, pb) = i.plan.axis(k);
        room(pa, pb, i.plan.lines[k].2).1
    }).collect();
    i.plan.blocks.iter().map(|b| {
        let round = i.round(&b.id);
        let h = b.size * 0.5 * if round { 0.55 } else { 0.85 };
        let marks = [-1.0f32, 0.0, 1.0];
        // (No hull: nothing to be in or out of.)
        let hull = mesh.is_some_and(|mesh| marks.into_iter().flat_map(|x| marks.into_iter().flat_map(move |y| marks.into_iter().map(move |z| Vec3::new(x, y, z)))).any(|m| {
            // (A hair off the hull's middle line: rays straight down it slip through the
            // seam between its mirrored halves.)
            let p = b.at + m * h + Vec3::new(0.0137, 0.0, 0.0071);
            let flat = DVec2::new(f64::from(p.x), f64::from(p.z));
            // (Outside: not closed in by the hull's skin all round, any face of it.)
            in_material(mesh, flat, f64::from(p.y)) || !enclosed(mesh, flat, f64::from(p.y), 0.0, true)
        }));
        hull || rooms.iter().any(|p| b.holds(*p, round))
    }).collect()
}

/// Points and the lines between them: where access must reach, and the ways it
/// goes, first as lines (their room comes later).
#[derive(Clone, Default, PartialEq, serde::Serialize, serde::Deserialize)]
struct Plan {
    /// For which hull (its fixed points are taken from it).
    hull: String,
    points: Vec<Point>,
    /// Each line: its two points and its cross-section.
    lines: Vec<(usize, usize, Profile)>,
    /// Lines grouped (each a group's lines), and whether it's walled off: walls
    /// along its tubes' sides, and across their free ends.
    groups: Vec<Group>,
    /// Walled tubes' free ends set apart from what they'd be (closed; open near an
    /// entry): (line, end 0 or 1, how).
    #[serde(default)]
    ends: Vec<EndSet>,
    /// Doorways in walled tubes' sides: (line, how far along it 0-1, on its right?,
    /// its hatch).
    #[serde(default)]
    doors: Vec<SideDoor>,
    /// The ship's modules (and its hold) placed in it.
    #[serde(default)]
    blocks: Vec<Block>,
    /// Its frame: the members that bear its loads.
    #[serde(default)]
    beams: Vec<Beam>,
    /// The hull it's designed in, by key (empty: none, designed from nothing); not
    /// said (older plans): the hull its name is.
    #[serde(default)]
    on: Option<String>,
    /// Landing pads laid in the plan (where it stands on the ground), beside any its
    /// hull has.
    #[serde(default)]
    pads: Vec<Vec3>,
}

/// The hull plan `plan` is designed in, if any.
fn hull_of(plan: &Plan) -> Option<&'static universe_sim::world::ship::ClassSpec> {
    let content = universe_sim::world::content::content();
    let key = plan.on.as_deref().unwrap_or(&plan.hull);
    (!key.is_empty()).then(|| content.handle::<universe_sim::world::ship::ClassSpec>(key)).flatten().map(|h| content.get(h))
}

/// Hulls a design can start in: those modelled inside (a walkable mesh).
fn hulls() -> Vec<&'static universe_sim::world::ship::ClassSpec> {
    universe_sim::world::content::content().hulls.iter().map(|(_, h)| h).filter(|h| h.shape().walk.is_some()).collect()
}

/// A design with no hull: the room it's laid out in (m, its corners).
const NO_HULL: (Vec3, Vec3) = (Vec3::new(-20.0, -10.0, -40.0), Vec3::new(20.0, 15.0, 40.0));

/// Placed modules whose record's size has changed since (neither their volume nor
/// their size's matches it: a stretched one keeps the first, one made the second)
/// made that size, each standing where it stood; how many.
fn resize(blocks: &mut [Block], fit: &[Fitted]) -> usize {
    let mut resized = 0;
    for b in blocks.iter_mut() {
        let Some(f) = fit.iter().find(|f| f.id == kind(&b.id)) else { continue };
        let shape = if f.round { std::f32::consts::PI / 6.0 } else { 1.0 };
        let volume = b.size.x * b.size.y * b.size.z * shape;
        let made = f.size.x * f.size.y * f.size.z * shape;
        let off = |v: f32| (volume - v).abs() > v.max(0.01) * 0.02;
        if off(f.volume) && off(made) {
            let foot = b.at.y - b.size.y * 0.5;
            b.size = f.size;
            b.at.y = foot + b.size.y * 0.5;
            resized += 1;
        }
    }
    resized
}

/// A placed module's kind: its id less any copy number (`#n`).
fn kind(id: &str) -> &str {
    id.split('#').next().unwrap_or(id)
}

/// A member of the frame: its two ends, and what it's cut from (mill stock, by its
/// key in the registry).
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
struct Beam {
    a: Vec3,
    b: Vec3,
    stock: String,
}

/// Mill stock a member can be cut from: a tube or a bar the registry has, its
/// section and its material's figures, and its mass a metre (kg).
#[derive(Clone)]
pub struct Stock {
    key: String,
    name: String,
    /// What it's made of (the material's key): AUTO-SIZE keeps to it.
    of: String,
    section: universe_sim::world::frame::Section,
    material: universe_sim::world::frame::Material,
    per_metre: f64,
}

/// Every tube and bar in the registry whose material says how it bears (its
/// stiffness, yield and tensile strength, density), by material, lightest a
/// metre first.
fn stocks() -> &'static [Stock] {
    static STOCKS: std::sync::OnceLock<Vec<Stock>> = std::sync::OnceLock::new();
    STOCKS.get_or_init(|| {
        use universe_sim::world::frame::{Material, Section};
        let reg = universe_sim::world::registry::registry();
        let mut out: Vec<Stock> = reg.stock.iter().filter(|s| matches!(s.identity.form.as_str(), "tube" | "bar")).filter_map(|s| {
            let d = s.size.diameter?;
            let section = Section { diameter: d, wall: s.size.wall.unwrap_or(d * 0.5).min(d * 0.5) };
            let of = &s.made_from.first()?.item;
            let m = reg.materials.iter().find(|m| m.identity.key == *of)?;
            let k = &m.mechanical;
            let (e, y, t, rho) = (k.youngs_modulus?, k.yield_strength?, k.tensile_strength?, m.mass.density?);
            let shear = k.shear_modulus.unwrap_or(e / (2.0 * (1.0 + k.poissons_ratio.unwrap_or(0.3))));
            let material = Material { stiffness: e, shear, yield_strength: y, tensile_strength: t, density: rho };
            Some(Stock { key: s.identity.key.clone(), name: s.identity.name.to_uppercase(), of: of.clone(), per_metre: section.area() * rho, section, material })
        }).collect();
        // (By material, lightest a metre first in each.)
        out.sort_by(|a, b| a.of.cmp(&b.of).then(a.per_metre.total_cmp(&b.per_metre)));
        out
    })
}

/// The gravity the frame is worked out under, standing (m/s²: a standard g; each
/// port's own is to come).
const STANDARD_G: f64 = 9.80665;

/// The load cases, by name: standing after a landing (the jolt of the hull's design
/// landing on top of the weight), at full main thrust, hovering at full lift.
const CASES: [&str; 3] = ["LANDING", "THRUST", "LIFT"];

/// What the frame bears and how: its joints (its members' ends, those within 5 cm
/// one), its members' mass (kg), and for each load case either how each member
/// does, or why it couldn't be worked out; what isn't carried (a module with no
/// joint in it, a nozzle or a pad with none at it).
#[derive(Clone, Default)]
pub struct Bearing {
    joints: Vec<Vec3>,
    mass: f64,
    cases: Vec<(String, Result<universe_sim::world::frame::Collapse, String>)>,
    loose: Vec<String>,
    /// Each case's acceleration (m/s², what everything aboard feels).
    felt: Vec<f64>,
    /// Which of the frame's members each of the plan's is (none: left out, its stock
    /// unknown or its ends one joint).
    index: Vec<Option<usize>>,
}

impl Bearing {
    /// How the plan's member `k` does in each case it could be worked out in.
    fn outcomes(&self, k: usize) -> Vec<&universe_sim::world::frame::Outcome> {
        let Some(Some(m)) = self.index.get(k) else { return Vec::new() };
        self.cases.iter().filter_map(|(_, r)| r.as_ref().ok()).filter_map(|c| c.members.get(*m)).collect()
    }
}

/// A box truss filling the footprint between `a` and `b` (on the work plane, its
/// bottom) up `depth` m: a chord along each side on the bottom and the top (and a
/// keel down the middle, bottom and top, each 10 m of width), stations every
/// `spacing` m or so (evenly) with their uprights and cross members, every bay
/// braced across on its sides, bottom and top; of `stock`.
fn truss(a: Vec3, b: Vec3, depth: f32, spacing: f32, stock: &str) -> Vec<Beam> {
    let (lo, hi) = (a.min(b), a.max(b));
    let along = (hi.z - lo.z).max(0.5);
    let n = ((along / spacing.max(0.5)).round() as usize).max(1);
    let zs: Vec<f32> = (0..=n).map(|k| lo.z + along * k as f32 / n as f32).collect();
    let lines = ((hi.x - lo.x) / 10.0).ceil().max(1.0) as usize;
    let xs: Vec<f32> = (0..=lines).map(|k| lo.x + (hi.x - lo.x) * k as f32 / lines as f32).collect();
    let ys = [lo.y, lo.y + depth];
    let mut out: Vec<Beam> = Vec::new();
    let mut add = |p: Vec3, q: Vec3| {
        if p.distance(q) > 0.05 && !out.iter().any(|x| (x.a == p && x.b == q) || (x.a == q && x.b == p)) {
            out.push(Beam { a: p, b: q, stock: stock.to_string() });
        }
    };
    let at = |x: f32, y: f32, z: f32| Vec3::new(x, y, z);
    for w in zs.windows(2) {
        for &x in &xs {
            for &y in &ys {
                add(at(x, y, w[0]), at(x, y, w[1]));
            }
            // (Each line's bay braced up its side.)
            add(at(x, ys[0], w[0]), at(x, ys[1], w[1]));
        }
        for xw in xs.windows(2) {
            for &y in &ys {
                add(at(xw[0], y, w[0]), at(xw[1], y, w[1]));
            }
        }
    }
    for &z in &zs {
        for &x in &xs {
            add(at(x, ys[0], z), at(x, ys[1], z));
        }
        for xw in xs.windows(2) {
            for &y in &ys {
                add(at(xw[0], y, z), at(xw[1], y, z));
            }
        }
    }
    out
}

/// Does a member from `a` to `b` pass through one of the plan's walled rooms (its
/// floor and walls aside)?
fn through_room(plan: &Plan, a: Vec3, b: Vec3) -> bool {
    let rooms: Vec<(Vec3, Vec3, Profile)> = (0..plan.lines.len()).filter(|&k| plan.group_of(k).is_some_and(|g| plan.groups[g].walled)).map(|k| {
        let (p, q) = plan.axis(k);
        (p, q, plan.lines[k].2)
    }).collect();
    (1..10).map(|k| a.lerp(b, k as f32 / 10.0)).any(|q| rooms.iter().any(|&(p, e, pr)| {
        let d = e - p;
        let t = (q - p).dot(d) / d.length_squared().max(1e-6);
        let off = q - (p + d * t.clamp(0.0, 1.0));
        t > 0.0 && t < 1.0 && off.y.abs() < pr.height * 0.5 - 0.15 && Vec3::new(off.x, 0.0, off.z).length() < pr.width * 0.5 - 0.1
    }))
}

/// Members mounting each placed module the frame doesn't hold yet (no joint in
/// it): joints along its foot (one each `spacing` m; over a tonne full, at its
/// foot's corners, each braced to the nearest joint below as well, or hung from
/// the nearest above if there's none below), each strutted
/// to the nearest two joints of the frame outside it; a landing leg by its top, to
/// four; never through a room or further than 15 m; and each landing pad with
/// nothing at it, on four legs to the nearest joints; of `stock`.
fn mounts(plan: &Plan, fit: &[Fitted], spacing: f32, stock: &str) -> (Vec<Beam>, usize) {
    let joints: Vec<Vec3> = plan.beams.iter().flat_map(|b| [b.a, b.b]).fold(Vec::new(), |mut v, p| {
        if !v.iter().any(|q: &Vec3| q.distance(p) < 0.05) {
            v.push(p);
        }
        v
    });
    // (The frame as it is, its members split where a mount point lands on one.)
    let mut beams = plan.beams.clone();
    let mut out = Vec::new();
    for blk in &plan.blocks {
        let (lo, hi) = blk.bounds();
        let inside = |q: Vec3| q.cmpge(lo - 0.3).all() && q.cmple(hi + 0.3).all();
        if joints.iter().any(|&q| inside(q)) {
            continue;
        }
        let f = fit.iter().find(|f| f.id == kind(&blk.id));
        let heavy = f.is_some_and(|f| f.mass + f.load > 1000.0);
        // (A landing leg is mounted by its top; the rest by their foot.)
        let leg = f.is_some_and(|f| f.gear.is_some());
        let foot = if leg { blk.at.y + blk.size.y * 0.5 } else { blk.at.y - blk.size.y * 0.5 };
        let n = ((blk.size.z / spacing.max(0.5)).ceil() as usize).max(1);
        // (Its mount points: along its foot's middle; a heavy one (over a tonne,
        // full) at its foot's corners, as its mount's attachment has them, each
        // braced to the nearest joint below too, so its weight runs down a triangle
        // rather than bending a strut.)
        let (hx, hz) = (blk.size.x * 0.5, blk.size.z * 0.5);
        let points: Vec<Vec3> = if heavy && !leg {
            (0..=n).flat_map(|k| {
                let z = blk.at.z - hz + blk.size.z * k as f32 / n as f32;
                [Vec3::new(blk.at.x - hx, foot, z), Vec3::new(blk.at.x + hx, foot, z)]
            }).collect()
        } else {
            (0..n).map(|k| Vec3::new(blk.at.x, foot, blk.at.z - hz + blk.size.z * (k as f32 + 0.5) / n as f32)).collect()
        };
        for m in points {
            // (On a member, part way along it: a joint made there, the member in two;
            // its ends then not strutted to, they'd lie along it.)
            let mut along = Vec::new();
            if let Some(k) = beams.iter().position(|b| {
                let d = b.b - b.a;
                let t = (m - b.a).dot(d) / d.length_squared().max(1e-6);
                t > 0.02 && t < 0.98 && (b.a + d * t).distance(m) < 0.05
            }) {
                let b = beams.remove(k);
                along = vec![b.a, b.b];
                beams.push(Beam { a: b.a, b: m, stock: b.stock.clone() });
                beams.push(Beam { a: m, b: b.b, stock: b.stock });
            }
            let mut near: Vec<Vec3> = joints.iter().copied().filter(|&q| !inside(q) && q.distance(m) <= 15.0 && !through_room(plan, m, q) && !along.contains(&q)).collect();
            near.sort_by(|a, b| a.distance(m).total_cmp(&b.distance(m)));
            // (Braced to the nearest joint below; nothing below, hung from the nearest
            // above.)
            let below = near.iter().copied().find(|q| q.y < m.y - 1.0).or_else(|| near.iter().copied().find(|q| q.y > m.y + 1.0));
            let take = if leg { 4 } else { 2 };
            let mut chosen: Vec<Vec3> = near.into_iter().take(take).collect();
            if let Some(q) = below.filter(|q| heavy && !leg && !chosen.contains(q)) {
                chosen.push(q);
            }
            for q in chosen {
                out.push(Beam { a: m, b: q, stock: stock.to_string() });
            }
        }
    }
    for &pad in &plan.pads {
        if joints.iter().any(|q| q.distance(pad) <= 1.0) {
            continue;
        }
        let mut near: Vec<Vec3> = joints.iter().copied().filter(|&q| q.distance(pad) <= 15.0 && !through_room(plan, pad, q)).collect();
        near.sort_by(|a, b| a.distance(pad).total_cmp(&b.distance(pad)));
        for q in near.into_iter().take(4) {
            out.push(Beam { a: pad, b: q, stock: stock.to_string() });
        }
    }
    let added = out.len();
    beams.extend(out);
    (beams, added)
}

/// The members as AUTO-SIZE leaves them, the rounds it took, and how many need more
/// than the biggest tube their material comes in.
type Sized = (Vec<Beam>, usize, usize);

/// The plan's members sized: each round every load case worked out, and each
/// member cut from the lightest tube of its material that would carry the forces
/// it carries in each case at four fifths of its design limit (the biggest if none
/// would); again, as the forces shift with the members' stiffness, until nothing
/// changes (or `ROUNDS`; after the sixth, only bigger). The members, and the
/// rounds it took.
fn auto_size(plan: &Plan, fit: &[Fitted], spec: Option<&universe_sim::world::ship::ClassSpec>, mix: bool) -> Sized {
    use universe_sim::world::frame::{work, Member};
    let sf = 1.5;
    // (Each material's tubes its own ladder; MIX: every tube, any material.)
    let ladder_of = |key: &str| -> Vec<&'static Stock> {
        let of = stocks().iter().find(|s| s.key == key).map(|s| s.of.as_str()).unwrap_or("");
        let mut l: Vec<&'static Stock> = stocks().iter().filter(|s| (mix || s.of == of) && s.section.wall < s.section.diameter * 0.5).collect();
        l.sort_by(|a, b| a.per_metre.total_cmp(&b.per_metre));
        l
    };
    let mut plan = plan.clone();
    for round in 1..=ROUNDS {
        let b = bearing(&plan, fit, spec, STANDARD_G);
        if !b.cases.iter().any(|(_, r)| r.is_ok()) {
            return (plan.beams, round, 0);
        }
        let mut changed = false;
        let mut maxed = 0;
        for (k, beam) in plan.beams.iter_mut().enumerate() {
            let Some(now) = stocks().iter().find(|s| s.key == beam.stock) else { continue };
            let length = f64::from(beam.a.distance(beam.b));
            let forces: Vec<universe_sim::world::frame::Forces> = b.outcomes(k).iter().map(|o| o.work.forces).collect();
            if forces.is_empty() {
                continue;
            }
            let needed = |s: &Stock| forces.iter().map(|f| work(&Member { a: 0, b: 1, section: s.section, material: s.material }, *f, length, sf).design).fold(0.0, f64::max);
            let ladder = ladder_of(&beam.stock);
            let fits = ladder.iter().find(|s| needed(s) <= 0.8).copied().or_else(|| {
                maxed += usize::from(ladder.last().is_some_and(|s| needed(s) > 1.0));
                ladder.last().copied()
            });
            if let Some(pick) = fits
                && pick.key != beam.stock
                && (round <= 6 || pick.per_metre > now.per_metre)
            {
                beam.stock = pick.key.clone();
                changed = true;
            }
        }
        if !changed || round == ROUNDS {
            return (plan.beams, round, maxed);
        }
    }
    (plan.beams, ROUNDS, 0)
}

/// The most rounds AUTO-SIZE takes.
const ROUNDS: usize = 16;

/// The plan's members braced where `b` finds them past their limit in any case
/// (those 2 m long or more): each split at its middle, the middle tied to the nearest other joint (not
/// through a room), of its own stock. The members, and how many were braced.
fn brace(plan: &Plan, b: &Bearing) -> (Vec<Beam>, usize) {
    // (Long ones only: a member under 2 m isn't helped by halving.)
    let over: Vec<usize> = (0..plan.beams.len()).filter(|&k| plan.beams[k].a.distance(plan.beams[k].b) >= 2.0 && b.outcomes(k).iter().any(|o| o.work.design > 1.0 || o.broken.is_some())).collect();
    let joints: Vec<Vec3> = plan.beams.iter().flat_map(|x| [x.a, x.b]).collect();
    let mut beams = plan.beams.clone();
    let mut extra = Vec::new();
    for &k in over.iter().rev() {
        let m = beams.remove(k);
        let mid = m.a.lerp(m.b, 0.5);
        let tie = joints.iter().copied().filter(|q| q.distance(m.a) > 0.1 && q.distance(m.b) > 0.1 && !through_room(plan, mid, *q)).min_by(|x, y| x.distance(mid).total_cmp(&y.distance(mid)));
        extra.push(Beam { a: m.a, b: mid, stock: m.stock.clone() });
        extra.push(Beam { a: mid, b: m.b, stock: m.stock.clone() });
        if let Some(q) = tie {
            extra.push(Beam { a: mid, b: q, stock: m.stock });
        }
    }
    beams.extend(extra);
    (beams, over.len())
}

/// Loads in balance, free in flight: the `external` forces at joints, and each
/// point mass's inertia as the whole speeds up and turns under them (inertia
/// relief): -m (a + α × r) about the centre of mass, a the forces over the mass,
/// α their moment through the masses' inertia. Nothing left over for whatever
/// holds it.
fn balanced(external: Vec<(usize, universe_engine::glam::DVec3)>, points: &[(usize, f64)], joints: &[universe_engine::glam::DVec3]) -> Vec<(usize, universe_engine::glam::DVec3)> {
    use universe_engine::glam::{DMat3, DVec3};
    let total: f64 = points.iter().map(|p| p.1).sum();
    if total <= 0.0 {
        return external;
    }
    let c = points.iter().fold(DVec3::ZERO, |s, &(j, m)| s + joints[j] * m) / total;
    let force: DVec3 = external.iter().map(|e| e.1).sum();
    let torque: DVec3 = external.iter().map(|&(j, f)| (joints[j] - c).cross(f)).sum();
    let inertia = points.iter().fold(DMat3::ZERO, |i, &(j, m)| {
        let d = joints[j] - c;
        i + (DMat3::IDENTITY * d.length_squared() - DMat3::from_cols(d * d.x, d * d.y, d * d.z)) * m
    });
    let a = force / total;
    let alpha = if inertia.determinant().abs() > 1e-9 { inertia.inverse() * torque } else { DVec3::ZERO };
    let mut out = external;
    out.extend(points.iter().map(|&(j, m)| (j, -(a + alpha.cross(joints[j] - c)) * m)));
    out
}

/// What the plan's frame bears, under each case, for a hull `spec` (gravity `g`).
fn bearing(plan: &Plan, fit: &[Fitted], spec: Option<&universe_sim::world::ship::ClassSpec>, g: f64) -> Bearing {
    use universe_engine::glam::DVec3;
    use universe_sim::world::frame::{collapse, Case, Frame, Member};
    use universe_sim::world::ship::ThrusterRole;
    use universe_sim::world::shape::Role;
    let reg = universe_sim::world::registry::registry();
    let record = spec.and_then(|s| s.visual.as_deref()).and_then(|v| reg.hulls.iter().find(|h| h.model.as_deref() == Some(v)));
    let thrusters: &[universe_sim::world::ship::Thruster] = spec.map_or(&[], |s| s.thrusters.as_slice());
    let sf = record.and_then(|h| h.design.safety_factor).unwrap_or(1.5);
    let mut out = Bearing::default();
    // Its joints, and its members (of stock the registry has).
    let joint = |p: Vec3, joints: &mut Vec<Vec3>| joints.iter().position(|q| q.distance(p) < 0.05).unwrap_or_else(|| {
        joints.push(p);
        joints.len() - 1
    });
    let mut frame = Frame::default();
    let mut joints = Vec::new();
    let mut weight = Vec::new();
    for b in &plan.beams {
        let Some(s) = stocks().iter().find(|s| s.key == b.stock) else {
            out.index.push(None);
            continue;
        };
        let (ja, jb) = (joint(b.a, &mut joints), joint(b.b, &mut joints));
        if ja == jb {
            out.index.push(None);
            continue;
        }
        out.index.push(Some(frame.members.len()));
        let m = Member { a: ja, b: jb, section: s.section, material: s.material };
        let mass = m.mass(b.a.as_dvec3(), b.b.as_dvec3());
        out.mass += mass;
        // (A member's own weight: half at each end.)
        weight.extend([(ja, mass * 0.5), (jb, mass * 0.5)]);
        frame.members.push(m);
    }
    frame.joints = joints.iter().map(|p| p.as_dvec3()).collect();
    out.joints = joints.clone();
    let near = |at: Vec3, r: f32| -> Vec<usize> { joints.iter().enumerate().filter(|(_, q)| q.distance(at) <= r).map(|(k, _)| k).collect() };
    // The masses it carries: each module's (the ore bay: its hold full), shared
    // between the joints in it.
    let full = record.and_then(|h| h.capacity.hold).unwrap_or(0.0);
    let mut masses: Vec<(Vec<usize>, f64)> = weight.iter().map(|&(j, m)| (vec![j], m)).collect();
    for blk in &plan.blocks {
        let Some(f) = fit.iter().find(|f| f.id == kind(&blk.id)) else { continue };
        let (lo, hi) = blk.bounds();
        let at: Vec<usize> = joints.iter().enumerate().filter(|(_, q)| q.cmpge(lo - 0.3).all() && q.cmple(hi + 0.3).all()).map(|(k, _)| k).collect();
        // (Full: its ore, cargo or fuel too.)
        let m = if blk.id == HOLD { full } else { f.mass + f.load };
        if at.is_empty() {
            out.loose.push(format!("{} IS NOT MOUNTED", f.name));
        } else {
            masses.push((at, m));
        }
    }
    let carried: f64 = masses.iter().map(|m| m.1).sum();
    // (The whole ship: its hull's mass, full; with no hull, what the frame carries.)
    let ship = spec.map_or(carried, |s| s.dry_mass + s.fuel_capacity + full).max(carried);
    let share = (carried / ship).min(1.0);
    let anchor = {
        let c = masses.iter().fold(DVec3::ZERO, |s, (js, m)| s + js.iter().map(|&j| frame.joints[j]).sum::<DVec3>() / js.len() as f64 * *m) / carried.max(1e-9);
        (0..frame.joints.len()).min_by(|&a, &b| frame.joints[a].distance(c).total_cmp(&frame.joints[b].distance(c)))
    };
    // (Loads: each mass's, `acc` its acceleration less gravity's pull; and the
    // pushes that carry the frame, at the joints by them.)
    let loads = |acc: DVec3| -> Vec<(usize, DVec3)> {
        masses.iter().flat_map(|(js, m)| js.iter().map(move |&j| (j, (DVec3::new(0.0, -g, 0.0) - acc) * *m / js.len() as f64))).collect()
    };
    // (What pushes: the hull's nozzles, at the joints by them; with no hull, each
    // drive and lift placed, through the joints in it.)
    let mut pushers: Vec<(ThrusterRole, DVec3, Vec<usize>)> = thrusters.iter().filter(|t| matches!(t.role, ThrusterRole::Main | ThrusterRole::Lift)).map(|t| (t.role, t.push * t.thrust, near(t.at.as_vec3(), 1.0))).collect();
    if spec.is_none() {
        for blk in &plan.blocks {
            let Some((dir, thrust)) = fit.iter().find(|f| f.id == kind(&blk.id)).and_then(|f| f.push) else { continue };
            let (lo, hi) = blk.bounds();
            let at: Vec<usize> = joints.iter().enumerate().filter(|(_, q)| q.cmpge(lo - 0.3).all() && q.cmple(hi + 0.3).all()).map(|(k, _)| k).collect();
            let role = if dir.y > 0.5 { ThrusterRole::Lift } else { ThrusterRole::Main };
            pushers.push((role, dir.as_dvec3() * thrust, at));
        }
    }
    let pushes = |role: ThrusterRole, out: &mut Bearing, what: &str| -> (Vec<(usize, DVec3)>, DVec3) {
        let mut pushes = Vec::new();
        let mut total = DVec3::ZERO;
        let mut missed = false;
        for (_, force, at) in pushers.iter().filter(|p| p.0 == role) {
            total += *force;
            if at.is_empty() {
                missed = true;
                continue;
            }
            for &j in at {
                pushes.push((j, *force * share / at.len() as f64));
            }
        }
        if missed {
            out.loose.push(format!("{what} PUSHES ON NOTHING"));
        }
        (pushes, total)
    };
    // Landing: weight and the jolt of the design landing, as its legs give it
    // (world::legs: their stroke and efficiency, the sink they're designed for),
    // held at the landing pads.
    // (Landing legs placed as modules: each one's record.)
    let legs_placed: Vec<(&Block, Leg, String)> = plan.blocks.iter().filter_map(|b| fit.iter().find(|f| f.id == kind(&b.id)).and_then(|f| f.gear.map(|g| (b, g, f.name.clone())))).collect();
    let legs = spec.and_then(universe_sim::world::legs::of_spec);
    // (Landing legs placed: their design sink over the shortest stroke. Else the
    // hull's legs; else the registry's design landing, 3.05 m/s over an assumed
    // 1 m stroke at 0.85.)
    let placed = legs_placed.iter().map(|(_, g, _)| *g).min_by(|a, b| a.1.total_cmp(&b.1));
    let jolt = match (placed, legs) {
        (Some((_, stroke, eff, sink)), _) => sink * sink / (2.0 * stroke * eff),
        (None, Some(l)) => l.jolt(l.designed) * STANDARD_G,
        (None, None) => 3.05f64.powi(2) / (2.0 * 1.0 * 0.85),
    };
    if !legs_placed.is_empty() {
        let each = carried * (g + jolt) / legs_placed.len() as f64;
        for (_, (holds, ..), name) in legs_placed.iter().filter(|(_, (holds, ..), _)| each > *holds) {
            out.loose.push(format!("{name} OVERLOADED LANDING: {:.0} OF {:.0} KN", each / 1000.0, holds / 1000.0));
        }
    }
    let mut pads: Vec<usize> = spec.into_iter().flat_map(|s| s.shape().nodes(Role::Gear)).map(|n| n.at.as_vec3()).chain(plan.pads.iter().copied()).flat_map(|p| near(p, 1.0)).collect();
    // (Landing legs placed: the frame stands on each one's top, at the joints in
    // it; each takes its share of the landing, against what it holds.)
    for (b, _, name) in &legs_placed {
        let (lo, hi) = b.bounds();
        let at: Vec<usize> = joints.iter().enumerate().filter(|(_, q)| q.cmpge(lo - 0.3).all() && q.cmple(hi + 0.3).all()).map(|(k, _)| k).collect();
        if at.is_empty() {
            out.loose.push(format!("{name} HOLDS UP NOTHING"));
        }
        pads.extend(at);
    }
    let landing = if pads.is_empty() {
        out.loose.push("NOTHING STANDS ON THE LANDING PADS".into());
        Err("NOTHING ON THE PADS".to_string())
    } else {
        Ok(Case { loads: loads(DVec3::new(0.0, jolt, 0.0)), held: pads, anchor: None })
    };
    out.felt.push(g + jolt);
    // Full main thrust, and hovering on full lift: the pushes balanced by every
    // mass's inertia, held only to keep it from drifting.
    let (main, total) = pushes(ThrusterRole::Main, &mut out, "THE DRIVE");
    let acc = total / ship;
    // (In flight, far from anything: no weight; the pushes, and every mass's inertia
    // as the ship speeds up and turns under them.)
    let points: Vec<(usize, f64)> = masses.iter().flat_map(|(js, m)| js.iter().map(move |&j| (j, *m / js.len() as f64))).collect();
    let thrust = Case { loads: balanced(main.clone(), &points, &frame.joints), held: Vec::new(), anchor };
    out.felt.push(acc.length());
    let (lift, total) = pushes(ThrusterRole::Lift, &mut out, "THE LIFT");
    let acc = total / ship + DVec3::new(0.0, -g, 0.0);
    // (Hovering: its weight too.)
    let mut external = lift.clone();
    external.extend(points.iter().map(|&(j, m)| (j, DVec3::new(0.0, -g * m, 0.0))));
    let hover = Case { loads: balanced(external, &points, &frame.joints), held: Vec::new(), anchor };
    let _ = loads;
    out.felt.push(acc.length());
    let run = |case: Result<Case, String>, ok: bool| -> Result<universe_sim::world::frame::Collapse, String> {
        let case = case?;
        if !ok {
            return Err("NOTHING PUSHES THE FRAME".into());
        }
        if frame.members.is_empty() {
            return Err("NO MEMBERS".into());
        }
        collapse(&frame, &case, sf).map_err(|_| "NOT HELD TOGETHER: SOME OF IT MOVES FREELY".to_string())
    };
    out.cases = vec![
        (CASES[0].into(), run(landing, true)),
        (CASES[1].into(), run(Ok(thrust), !main.is_empty())),
        (CASES[2].into(), run(Ok(hover), !lift.is_empty())),
    ];
    out
}

/// A module of the ship's fit (or its hold) placed in it: which (its slot; the hold:
/// HOLD), its middle and its size (m: across, up, along: its box, or a tank's ball
/// or egg in it).
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
struct Block {
    id: String,
    at: Vec3,
    size: Vec3,
}

/// What the ship is fitted with, to be placed: its slot (the hold: HOLD), its name,
/// mass (kg) and volume (m³), whether it's round (a tank: a ball, an egg stretched),
/// and the size it starts at (the game's own proportions for its kind; real sizes
/// aren't in the registry yet).
#[derive(Clone)]
pub struct Fitted {
    id: String,
    /// Its equipment record's key (the ore bay: none).
    key: String,
    name: String,
    mass: f64,
    volume: f32,
    round: bool,
    size: Vec3,
    /// A drive's or a lift's push: which way it pushes the ship (a drive forward,
    /// to -z; a lift up), and how hard (N, its record's thrust).
    push: Option<(Vec3, f64)>,
    /// What it carries full (kg): an ore bay's ore, a rack's cargo, a tank's fuel.
    load: f64,
    /// A landing leg's: what it holds (N), its stroke (m), its efficiency, the sink
    /// it's designed for (m/s).
    gear: Option<Leg>,
}

/// A landing leg's figures: what it holds (N), its stroke (m), its efficiency, the
/// sink it's designed for (m/s).
type Leg = (f64, f64, f64, f64);

/// The ore bay's id among the placed blocks.
const HOLD: &str = "HOLD";

/// What `spec` is fitted with, and its ore bay, to be placed: as its hull's record
/// in the registry fits it (found by its model), each item's mass, volume and size
/// from its equipment record; a hull the registry doesn't describe, as the game fits
/// it.
fn fit_of(spec: &universe_sim::world::ship::ClassSpec) -> Vec<Fitted> {
    use universe_sim::world::registry::registry;
    let reg = registry();
    let Some(hull) = spec.visual.as_deref().and_then(|v| reg.hulls.iter().find(|h| h.model.as_deref() == Some(v))) else { return game_fit(spec) };
    let mut out: Vec<Fitted> = hull.fit.iter().filter_map(|f| fitted(reg.equipment.iter().find(|e| e.identity.key == f.item)?, &f.slot)).collect();
    // (Its ore bay: its hold; broad and low, under doors.)
    if let Some(volume) = hull.capacity.hold_volume.filter(|v| *v > 0.0).map(|v| v as f32) {
        let a = Vec3::new(1.2, 0.8, 1.0);
        out.push(Fitted { id: HOLD.into(), key: String::new(), name: "ORE BAY".into(), mass: 0.0, volume, round: false, size: a * (volume / (a.x * a.y * a.z)).cbrt(), push: None, load: 0.0, gear: None });
    }
    out
}

/// Everything a ship can be fitted with, as the registry has it (a design with no
/// hull picks from it, any number of each); not a gate's parts.
fn catalogue() -> &'static [Fitted] {
    static ALL: std::sync::OnceLock<Vec<Fitted>> = std::sync::OnceLock::new();
    ALL.get_or_init(|| {
        let reg = universe_sim::world::registry::registry();
        let mut out: Vec<Fitted> = reg.equipment.iter().filter(|e| e.identity.slot.is_some_and(|s| !matches!(s, universe_sim::world::registry::SlotKind::Gate))).filter_map(|e| fitted(e, &e.identity.key)).collect();
        out.sort_by(|a, b| a.name.cmp(&b.name));
        out
    })
}

/// A piece of equipment as a module to place (`id` what it's placed as): its mass,
/// volume and size from its record (its own size where it says; else its volume in
/// the game's proportions for its kind, a tank a ball).
fn fitted(e: &universe_sim::world::registry::Equipment, id: &str) -> Option<Fitted> {
    use universe_sim::world::registry::EquipmentFunction;
    let content = universe_sim::world::content::content();
    {
        let p = &e.physical;
        // (Tanks and stores of liquid or gas: balls.)
        let round = matches!(e.function, EquipmentFunction::Tank(_) | EquipmentFunction::Store(_));
        let size = match (p.width, p.height, p.length) {
            // (Its own size, where its record says it: across, up, along.)
            (Some(w), Some(h), Some(l)) => Vec3::new(w as f32, h as f32, l as f32),
            _ => {
                let volume = p.volume.unwrap_or(1.0) as f32;
                if round {
                    Vec3::splat((6.0 * volume / std::f32::consts::PI).cbrt())
                } else {
                    // (Its volume, in the game's proportions for its kind, if the game
                    // has it; else a cube.)
                    let a = content.handle::<universe_sim::world::modules::Module>(&e.identity.key).map_or(Vec3::ONE, |h| content.get(h).dims().as_vec3());
                    a * (volume / (a.x * a.y * a.z)).cbrt()
                }
            }
        };
        let volume = p.volume.map_or(size.x * size.y * size.z, |v| v as f32);
        let push = match &e.function {
            EquipmentFunction::Drive(d) => Some((Vec3::NEG_Z, d.thrust)),
            EquipmentFunction::Lift(l) => Some((Vec3::Y, l.thrust)),
            _ => None,
        };
        let load = match &e.function {
            EquipmentFunction::OreBay(b) => b.capacity,
            EquipmentFunction::Rack(r) => r.capacity,
            EquipmentFunction::Tank(t) => t.capacity,
            EquipmentFunction::Store(t) => t.capacity.unwrap_or(0.0),
            _ => 0.0,
        };
        let gear = match &e.function {
            EquipmentFunction::LandingGear(g) => Some((g.holds, g.stroke, g.efficiency, g.sink_rate)),
            _ => None,
        };
        Some(Fitted { id: id.to_string(), key: e.identity.key.clone(), name: e.identity.name.to_uppercase(), mass: p.mass.unwrap_or(0.0), volume, round, size, push, load, gear })
    }
}

/// What `spec` is fitted with as the game fits it (a hull the registry doesn't
/// describe).
fn game_fit(spec: &universe_sim::world::ship::ClassSpec) -> Vec<Fitted> {
    use universe_sim::world::modules::SlotKind;
    let content = universe_sim::world::content::content();
    spec.fit.iter().map(|(slot, h)| {
        let m = content.get(*h);
        let volume = m.volume as f32;
        let round = m.does.slot() == SlotKind::Tank;
        let size = if round { Vec3::splat((6.0 * volume / std::f32::consts::PI).cbrt()) } else { m.dims().as_vec3() };
        Fitted { id: slot.clone(), key: m.key.clone(), name: m.name.to_uppercase(), mass: m.mass, volume, round, size, push: None, load: 0.0, gear: None }
    }).collect()
}

impl Block {
    /// Its two corners.
    fn bounds(&self) -> (Vec3, Vec3) {
        (self.at - self.size * 0.5, self.at + self.size * 0.5)
    }

    /// Is `p` in it (a round one: in its egg)?
    fn holds(&self, p: Vec3, round: bool) -> bool {
        let d = (p - self.at) / (self.size * 0.5).max(Vec3::splat(1e-3));
        if round { d.length_squared() <= 1.0 } else { d.abs().max_element() <= 1.0 }
    }

    /// Where the ray from `from` along `dir` first meets its box (how far), if it does.
    fn hit(&self, from: Vec3, dir: Vec3) -> Option<f32> {
        let (lo, hi) = self.bounds();
        let (mut near, mut far) = (0.0f32, f32::MAX);
        for k in 0..3 {
            if dir[k].abs() < 1e-6 {
                if from[k] < lo[k] || from[k] > hi[k] {
                    return None;
                }
                continue;
            }
            let (a, b) = ((lo[k] - from[k]) / dir[k], (hi[k] - from[k]) / dir[k]);
            near = near.max(a.min(b));
            far = far.min(a.max(b));
        }
        (near <= far).then_some(near)
    }

    /// Its outline to draw: a box's twelve edges; a round one's three rings.
    fn edges(&self, round: bool) -> Vec<[Vec3; 2]> {
        let h = self.size * 0.5;
        if round {
            let n = 32;
            let ring = |f: &dyn Fn(f32) -> Vec3| (0..n).map(|k| {
                let (a, b) = (k as f32 / n as f32 * std::f32::consts::TAU, (k + 1) as f32 / n as f32 * std::f32::consts::TAU);
                [self.at + f(a), self.at + f(b)]
            }).collect::<Vec<_>>();
            let mut out = ring(&|a| Vec3::new(a.cos() * h.x, 0.0, a.sin() * h.z));
            out.extend(ring(&|a| Vec3::new(a.cos() * h.x, a.sin() * h.y, 0.0)));
            out.extend(ring(&|a| Vec3::new(0.0, a.sin() * h.y, a.cos() * h.z)));
            return out;
        }
        let c = |i: usize| self.at + Vec3::new(if i & 1 == 0 { -h.x } else { h.x }, if i & 2 == 0 { -h.y } else { h.y }, if i & 4 == 0 { -h.z } else { h.z });
        [(0, 1), (2, 3), (4, 5), (6, 7), (0, 2), (1, 3), (4, 6), (5, 7), (0, 4), (1, 5), (2, 6), (3, 7)].map(|(a, b)| [c(a), c(b)]).to_vec()
    }

    /// Its surface as triangles, each with which of its edges are seams (a box's
    /// faces' outlines; a round one: none).
    fn faces(&self, round: bool) -> Vec<([Vec3; 3], [bool; 3])> {
        let h = self.size * 0.5;
        if round {
            let (around, up) = (16, 10);
            let at = |i: usize, j: usize| {
                let (a, b) = (i as f32 / around as f32 * std::f32::consts::TAU, j as f32 / up as f32 * std::f32::consts::PI - std::f32::consts::FRAC_PI_2);
                self.at + Vec3::new(a.cos() * b.cos() * h.x, b.sin() * h.y, a.sin() * b.cos() * h.z)
            };
            let mut out = Vec::new();
            for j in 0..up {
                for i in 0..around {
                    let (p, q, r, t) = (at(i, j), at(i + 1, j), at(i + 1, j + 1), at(i, j + 1));
                    out.push(([p, r, q], [false; 3]));
                    out.push(([p, t, r], [false; 3]));
                }
            }
            return out;
        }
        let c = |x: f32, y: f32, z: f32| self.at + Vec3::new(x * h.x, y * h.y, z * h.z);
        let quads = [
            [c(-1., -1., -1.), c(1., -1., -1.), c(1., -1., 1.), c(-1., -1., 1.)],
            [c(-1., 1., -1.), c(-1., 1., 1.), c(1., 1., 1.), c(1., 1., -1.)],
            [c(-1., -1., -1.), c(-1., 1., -1.), c(1., 1., -1.), c(1., -1., -1.)],
            [c(-1., -1., 1.), c(1., -1., 1.), c(1., 1., 1.), c(-1., 1., 1.)],
            [c(-1., -1., -1.), c(-1., -1., 1.), c(-1., 1., 1.), c(-1., 1., -1.)],
            [c(1., -1., -1.), c(1., 1., -1.), c(1., 1., 1.), c(1., -1., 1.)],
        ];
        quads.iter().flat_map(|q| [([q[0], q[1], q[2]], [true, true, false]), ([q[0], q[2], q[3]], [false, true, true])]).collect()
    }
}

/// A doorway in a walled tube's side: its line, how far along it (0-1), on its
/// right?, its hatch.
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
struct SideDoor(usize, f32, bool, #[serde(default)] Hatch);

/// A walled tube's free end set: its line, which end, how it is, its hatch (if a
/// doorway).
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
struct EndSet(usize, usize, End, #[serde(default)] Hatch);

/// A hatch: its doorway's shape and size (standing on the floor), and how its leaf
/// moves (none: an open doorway).
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
struct Hatch {
    section: Section,
    width: f32,
    height: f32,
    slide: Slide,
}

impl Default for Hatch {
    fn default() -> Self {
        Hatch { section: Section::Square, width: DOORWAY.0, height: DOORWAY.1, slide: Slide::Side }
    }
}

/// How a hatch's leaf opens: none (no leaf), sliding up, sliding aside.
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
enum Slide {
    None,
    Up,
    Side,
}

impl Hatch {
    /// Its outline's corners round its middle (across, up), anticlockwise.
    fn corners(&self) -> Vec<Vec2> {
        let section = if self.section == Section::Line { Section::Square } else { self.section };
        Profile { section, width: self.width, height: self.height, stand: false }.corners()
    }

    /// Its window's corners round its middle: the same shape, small, high in the leaf.
    fn window(&self) -> Vec<Vec2> {
        let (w, h) = ((self.width * 0.35).min(0.35), (self.height * 0.2).min(0.4));
        let section = if self.section == Section::Line { Section::Square } else { self.section };
        let lift = self.height * 0.22;
        Profile { section, width: w, height: h, stand: false }.corners().into_iter().map(|c| c + Vec2::new(0.0, lift)).collect()
    }
}

/// A hatch as placed: its leaf's outline and window (closed, the hull's frame), the
/// way it slides fully open, and its middle (who's near opens it).
pub struct Leaf {
    pub outline: Vec<Vec3>,
    pub window: Vec<Vec3>,
    pub open: Vec3,
    pub middle: Vec3,
}

/// How a walled tube's free end is: walled across, a doorway in that wall, or open.
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
enum End {
    Closed,
    Door,
    Open,
}

/// A doorway's width and height (m).
const DOORWAY: (f32, f32) = (0.9, 2.1);

#[derive(Clone, Default, PartialEq, serde::Serialize, serde::Deserialize)]
struct Group {
    lines: Vec<usize>,
    walled: bool,
}

/// Outline `poly` (flat, convex) cut by a plane, the part where `n·p < c` kept.
fn clip(poly: &[Vec3], n: Vec3, c: f32) -> Vec<Vec3> {
    let mut out = Vec::new();
    for k in 0..poly.len() {
        let (p, q) = (poly[k], poly[(k + 1) % poly.len()]);
        let (dp, dq) = (n.dot(p) - c, n.dot(q) - c);
        if dp < 0.0 {
            out.push(p);
        }
        if (dp < 0.0) != (dq < 0.0) {
            out.push(p.lerp(q, dp / (dp - dq)));
        }
    }
    out
}

/// Outline `poly` less what's inside the room `planes` (inside all of them, a hair
/// in: what lies on its surface stays): the pieces left, each flat and convex.
fn outside(poly: Vec<Vec3>, planes: &[(Vec3, f32)]) -> Vec<Vec<Vec3>> {
    const HAIR: f32 = 1e-3;
    let mut out = Vec::new();
    let mut rest = poly;
    for &(n, c) in planes {
        // (Beyond this plane: out of the room, kept; within it, on to the next.)
        let beyond = clip(&rest, -n, -(c - HAIR));
        if beyond.len() >= 3 {
            out.push(beyond);
        }
        rest = clip(&rest, n, c - HAIR);
        if rest.len() < 3 {
            return out;
        }
    }
    out
}

/// A deck plan's floors and walls as flat outlines (the hull's frame): its floors'
/// pieces at their levels, its walls' runs from floor to ceiling.
fn deck_shapes(plan: &universe_sim::world::deckplan::DeckPlan, mesh: &universe_sim::world::walk::WalkMesh) -> Vec<DeckShape> {
    use universe_sim::world::deckplan;
    let mut out = Vec::new();
    for (d, deck) in plan.decks.iter().enumerate() {
        let sides = deckplan::deck_sides(mesh, deck.floor);
        let holes = deckplan::openings(plan, d);
        let (y0, y1) = (deck.floor as f32, (deck.floor + deck.headroom) as f32);
        for poly in &deck.planes {
            for q in deckplan::floor_pieces(poly, &sides, &holes) {
                out.push((q.iter().map(|p| Vec3::new(p.x as f32, y0, p.y as f32)).collect(), true));
            }
        }
        for wall in &deck.walls {
            for run in deckplan::wall_runs(wall, &sides) {
                for w in run.windows(2) {
                    let (a, b) = (w[0].0.as_vec2(), w[1].0.as_vec2());
                    out.push((vec![Vec3::new(a.x, y0, a.y), Vec3::new(b.x, y0, b.y), Vec3::new(b.x, y1, b.y), Vec3::new(a.x, y1, a.y)], false));
                }
            }
        }
    }
    out
}

/// A cell of the hollow map: its middle (x, z) and whether it's hollow (else solid).
type Cell = (Vec2, bool);

/// A deck's floor piece or wall run, flat (its outline) and which.
type DeckShape = (Vec<Vec3>, bool);

/// A tunnel as the deck studio shows it: from, to, its room across and up, walled?
pub type Tunnel = (Vec3, Vec3, Option<(f32, f32)>, bool);

/// The access plan as the deck studio shows it: tunnels (from, to, their room
/// across and up if any, walled?) and points (where, their colour), the hull's frame.
pub struct Access {
    pub tunnels: Vec<Tunnel>,
    pub points: Vec<(Vec3, Color)>,
    /// Each layer shown or not (by number in `LAYERS`: see `layer`).
    pub layers: [bool; LAYERS.len()],
    /// The modules placed: each one's middle, size (across, up, along), whether it's
    /// round, and its name.
    pub modules: Vec<(Vec3, Vec3, bool, String)>,
}

/// A wall triangle to draw: its corners (the hull's frame), its colour, which of
/// its edges are seams.
pub type WallFace = ([universe_engine::glam::DVec3; 3], [f32; 4], [bool; 3]);

/// A piece of a walled tube's wall: its outline (flat, convex), what it is, and
/// whether it's an odd panel along (shaded a little apart, so the way reads).
#[derive(Clone)]
struct Panel {
    outline: Vec<Vec3>,
    part: Part,
    band: bool,
}

#[derive(Clone, Copy, PartialEq)]
enum Part {
    Floor,
    Wall,
    Ceiling,
}

/// A wall panel's length along its tube (m).
const PANEL_LENGTH: f32 = 1.2;

impl Panel {
    /// Its colour inside (unlit flat: the floor dark deck grating, the walls mid
    /// grey-blue, the ceiling light; every other panel a shade apart).
    fn colour(&self) -> [f32; 4] {
        let (base, step) = match self.part {
            Part::Floor => ([0.17, 0.16, 0.14], 0.025),
            Part::Wall => ([0.36, 0.40, 0.46], 0.05),
            Part::Ceiling => ([0.58, 0.60, 0.62], 0.04),
        };
        let k = if self.band { -step } else { 0.0 };
        [base[0] + k, base[1] + k, base[2] + k, 1.0]
    }
}

/// A walled tube: its axis (from `a`, along `d`, `len` long), across it (`u`
/// level, `v` up) and its outline's corners round the axis.
struct Tube {
    a: Vec3,
    d: Vec3,
    len: f32,
    u: Vec3,
    v: Vec3,
    corners: Vec<Vec2>,
    /// Its walls' ends: square across it, or (where it meets one other tube) on the
    /// plane halfway between their ways (a mitre): that plane's normal, at the end.
    mitres: [Option<Vec3>; 2],
    /// Its line, and how each of its ends is (none: joined to another tube there).
    line: usize,
    ends: [Option<End>; 2],
    hatches: [Hatch; 2],
}

impl Tube {
    fn at(&self, s: f32, c: Vec2) -> Vec3 {
        self.a + self.d * s + self.u * c.x + self.v * c.y
    }

    /// Its room as planes (normal, offset: inside where `n·p < c`): its sides, and its
    /// ends square across it.
    fn planes(&self) -> Vec<(Vec3, f32)> {
        let n = self.corners.len();
        let mut planes: Vec<(Vec3, f32)> = (0..n).map(|k| {
            let (c0, c1) = (self.corners[k], self.corners[(k + 1) % n]);
            let e = (c1 - c0).normalize_or_zero();
            // (Out of an anticlockwise outline: its edge turned right.)
            let normal = self.u * e.y - self.v * e.x;
            (normal, normal.dot(self.at(0.0, c0)))
        }).collect();
        planes.push((-self.d, -self.d.dot(self.a)));
        planes.push((self.d, self.d.dot(self.a) + self.len));
        planes
    }

    /// Its level way along (its direction with the climb taken out).
    fn level(&self) -> Vec3 {
        Vec3::new(self.d.x, 0.0, self.d.z).try_normalize().unwrap_or(Vec3::Z)
    }

    /// Where a doorway stands: in its side (`s` along it, on its right?) or in an
    /// end's wall (`s` its end): its middle (standing on the floor there), the way
    /// across its face, and the way through it.
    fn door_frame(&self, s: f32, side: Option<bool>, h: &Hatch) -> (Vec3, Vec3, Vec3) {
        let (floor, _) = self.floor_and_half();
        let middle = self.at(s, Vec2::new(0.0, floor)) + Vec3::Y * (h.height * 0.5);
        match side {
            Some(_) => (middle, self.level(), self.u),
            None => (middle, self.u, self.level()),
        }
    }

    /// A doorway through it as planes: its hatch's outline standing plumb on the
    /// floor (`s` along it), through its wall on that side (side doors), or through
    /// its end's wall, as deep as that leans (end doors).
    fn door_cut(&self, s: f32, side: Option<bool>, h: &Hatch) -> Vec<(Vec3, f32)> {
        let (c, across, through) = self.door_frame(s, side, h);
        let corners = h.corners();
        let n = corners.len();
        let mut planes: Vec<(Vec3, f32)> = (0..n).map(|k| {
            let (p0, p1) = (corners[k], corners[(k + 1) % n]);
            let e = p1 - p0;
            let normal = across * e.y - Vec3::Y * e.x;
            (normal, normal.dot(c + across * p0.x + Vec3::Y * p0.y))
        }).collect();
        let (floor, half) = self.floor_and_half();
        // (Never below its own floor: a sloping tube's floor runs on under its door.)
        planes.push((-self.v, -(self.v.dot(self.a) + floor)));
        match side {
            Some(right) => {
                let (lo, hi) = if right { (0.0, half + 0.5) } else { (-half - 0.5, 0.0) };
                planes.push((-through, -(through.dot(self.a) + lo)));
                planes.push((through, through.dot(self.a) + hi));
            }
            None => {
                let lean = self.d.y.abs() / Vec2::new(self.d.x, self.d.z).length().max(0.1);
                let tall = self.corners.iter().map(|q| q.y).fold(f32::MIN, f32::max) - self.corners.iter().map(|q| q.y).fold(f32::MAX, f32::min);
                let deep = 0.5 + tall * lean;
                planes.push((-through, -(through.dot(c) - deep)));
                planes.push((through, through.dot(c) + deep));
            }
        }
        planes
    }

    /// Its hatch's leaf there, closed: in its wall (side doors) or across its end.
    fn leaf(&self, s: f32, side: Option<bool>, h: &Hatch) -> Option<Leaf> {
        if h.slide == Slide::None {
            return None;
        }
        let (c, across, _) = self.door_frame(s, side, h);
        let (_, half) = self.floor_and_half();
        let out = match side {
            Some(right) => self.u * if right { half } else { -half },
            None => Vec3::ZERO,
        };
        let place = |q: Vec2| c + out + across * q.x + Vec3::Y * q.y;
        let open = match h.slide {
            Slide::Up => Vec3::Y * h.height,
            _ => across * h.width,
        };
        Some(Leaf { outline: h.corners().into_iter().map(place).collect(), window: h.window().into_iter().map(place).collect(), open, middle: c + out })
    }

    /// Its floor's height round its axis, and its half width (m).
    fn floor_and_half(&self) -> (f32, f32) {
        let lo = self.corners.iter().map(|c| c.y).fold(f32::MAX, f32::min);
        let half = self.corners.iter().map(|c| c.x.abs()).fold(0.0, f32::max);
        (lo, half)
    }

    /// Where along it its side line through corner `c` ends at its start (0) or its
    /// end (1): on its mitre plane if it has one, else square across.
    fn reach(&self, end: usize, c: Vec2) -> f32 {
        let s = if end == 0 { 0.0 } else { self.len };
        let Some(n) = self.mitres[end] else { return s };
        let joint = self.a + self.d * s;
        let off = self.u * c.x + self.v * c.y;
        let along = n.dot(self.d);
        if along.abs() < 1e-3 {
            return s;
        }
        // (n·(a + d t + off − joint) = 0.)
        (n.dot(joint - self.a - off) / along).clamp(s - self.len, s + self.len)
    }

}

impl Plan {
    /// Its walled tubes: the lines of its walled groups that have a cross-section.
    fn tubes(&self) -> Vec<Tube> {
        let walled: Vec<usize> = self.groups.iter().filter(|g| g.walled).flat_map(|g| g.lines.iter().copied()).filter(|&k| {
            let (a, b, profile) = self.lines[k];
            profile.section != Section::Line && self.points[a].at.distance(self.points[b].at) > 1e-3
        }).collect();
        // (The way along a walled line, out of point `p`.)
        let out_of = |k: usize, p: usize| {
            let (a, b, _) = self.lines[k];
            let (from, to) = if a == p { (a, b) } else { (b, a) };
            (self.points[to].at - self.points[from].at).normalize_or_zero()
        };
        walled.iter().map(|&k| {
            let (a, b, profile) = self.lines[k];
            let (pa, pb) = self.axis(k);
            let len = pa.distance(pb);
            let d = (pb - pa) / len;
            let u = d.cross(Vec3::Y).try_normalize().unwrap_or(Vec3::X);
            // At each end, met by just one other walled tube (and not straight on): the
            // plane halfway between their ways.
            let mitre = |p: usize, into: Vec3| {
                let others: Vec<usize> = walled.iter().copied().filter(|&j| j != k && (self.lines[j].0 == p || self.lines[j].1 == p)).collect();
                let [j] = others[..] else { return None };
                let on = out_of(j, p);
                let n = (into + on).try_normalize()?;
                (n.dot(into) < 0.999).then_some(n)
            };
            let mitres = [mitre(a, -d).map(|n| -n), mitre(b, d)];
            let ends = [self.free_end(k, a).then(|| self.end_of(k, 0)), self.free_end(k, b).then(|| self.end_of(k, 1))];
            Tube { a: pa, d, len, u, v: u.cross(d), corners: profile.corners(), mitres, line: k, ends, hatches: [self.end_hatch(k, 0), self.end_hatch(k, 1)] }
        }).collect()
    }

    /// Its walls: each walled tube's sides (ending square, or mitred where it meets
    /// one other tube), less what's inside another walled tube, cut exactly along its
    /// surface (where tubes cross or meet, the way through opens); their ends open.
    /// Each piece a flat convex outline.
    fn wall_panels(&self) -> Vec<Panel> {
        let tubes = self.tubes();
        let rooms: Vec<Vec<(Vec3, f32)>> = tubes.iter().map(Tube::planes).collect();
        // Doorways: boxes cutting a tube's wall, from its floor up (in a side, at a way
        // along; in an end's wall, at its middle).
        let mut doorways: Vec<Vec<(Vec3, f32)>> = Vec::new();
        for t in &tubes {
            for &SideDoor(k, at, right, h) in &self.doors {
                if k == t.line {
                    doorways.push(t.door_cut(at * t.len, Some(right), &h));
                }
            }
            for (e, how) in t.ends.iter().enumerate() {
                if *how == Some(End::Door) {
                    doorways.push(t.door_cut(if e == 0 { 0.0 } else { t.len }, None, &t.hatches[e]));
                }
            }
        }
        let mut out = Vec::new();
        for (i, t) in tubes.iter().enumerate() {
            let n = t.corners.len();
            let (lo, hi) = t.corners.iter().fold((f32::MAX, f32::MIN), |m, c| (m.0.min(c.y), m.1.max(c.y)));
            for j in 0..n {
                let (c0, c1) = (t.corners[j], t.corners[(j + 1) % n]);
                // (Floor, wall or ceiling, by where the side is round the axis.)
                let mid = (c0.y + c1.y) * 0.5;
                let part = if mid < lo + (hi - lo) * 0.2 { Part::Floor } else if mid > hi - (hi - lo) * 0.2 { Part::Ceiling } else { Part::Wall };
                let reach = [t.reach(0, c0), t.reach(0, c1), t.reach(1, c1), t.reach(1, c0)];
                let side = vec![t.at(reach[0], c0), t.at(reach[1], c1), t.at(reach[2], c1), t.at(reach[3], c0)];
                // In panels `PANEL_LENGTH` along (from its start), each cut where it's
                // inside another tube.
                let (s0, s1) = (reach.iter().copied().fold(f32::MAX, f32::min), reach.iter().copied().fold(f32::MIN, f32::max));
                let base = t.d.dot(t.a);
                let first = (s0 / PANEL_LENGTH).floor() as i32;
                for k in first..((s1 / PANEL_LENGTH).ceil() as i32) {
                    let (a, b) = (k as f32 * PANEL_LENGTH, (k + 1) as f32 * PANEL_LENGTH);
                    let strip = clip(&clip(&side, t.d, base + b), -t.d, -(base + a));
                    if strip.len() < 3 {
                        continue;
                    }
                    let mut pieces = vec![strip];
                    for (o, room) in rooms.iter().enumerate() {
                        if o != i {
                            pieces = pieces.into_iter().flat_map(|p| outside(p, room)).collect();
                        }
                    }
                    for door in &doorways {
                        pieces = pieces.into_iter().flat_map(|p| outside(p, door)).collect();
                    }
                    out.extend(pieces.into_iter().map(|outline| Panel { outline, part, band: k.rem_euclid(2) == 1 }));
                }
            }
            // Its free ends walled across (not where they're open), less what's inside
            // another tube (one coming through) or a doorway.
            for (e, how) in t.ends.iter().enumerate() {
                if !matches!(how, Some(End::Closed | End::Door)) {
                    continue;
                }
                let s = if e == 0 { 0.0 } else { t.len };
                let mut cap: Vec<Vec3> = t.corners.iter().map(|c| t.at(s, *c)).collect();
                if e == 1 {
                    cap.reverse();
                }
                let mut pieces = vec![cap];
                for (o, room) in rooms.iter().enumerate() {
                    if o != i {
                        pieces = pieces.into_iter().flat_map(|p| outside(p, room)).collect();
                    }
                }
                for door in &doorways {
                    pieces = pieces.into_iter().flat_map(|p| outside(p, door)).collect();
                }
                out.extend(pieces.into_iter().map(|outline| Panel { outline, part: Part::Wall, band: false }));
            }
        }
        out
    }

    /// Its lines that `keep` says stay, the others gone (the groups' kept in step;
    /// a group left with none, gone).
    fn keep_lines(&mut self, keep: impl Fn(usize, &(usize, usize, Profile)) -> bool) {
        let mut to = vec![None; self.lines.len()];
        let mut kept = Vec::new();
        for (k, l) in self.lines.iter().enumerate() {
            if keep(k, l) {
                to[k] = Some(kept.len());
                kept.push(*l);
            }
        }
        self.lines = kept;
        for g in &mut self.groups {
            g.lines = g.lines.iter().filter_map(|&k| to.get(k).copied().flatten()).collect();
        }
        self.groups.retain(|g| !g.lines.is_empty());
        self.ends = self.ends.iter().filter_map(|&EndSet(k, e, how, h)| Some(EndSet(to.get(k).copied().flatten()?, e, how, h))).collect();
        self.doors = self.doors.iter().filter_map(|&SideDoor(k, t, right, h)| Some(SideDoor(to.get(k).copied().flatten()?, t, right, h))).collect();
    }

    /// How line `k`'s end `e` is: as set, else open if it's within 2 m of an entry
    /// (the hatch, a crew door), else closed. (Only a free end has a wall to be.)
    fn end_of(&self, k: usize, e: usize) -> End {
        if let Some(&EndSet(_, _, how, _)) = self.ends.iter().find(|&&EndSet(j, f, _, _)| j == k && f == e) {
            return how;
        }
        let p = if e == 0 { self.lines[k].0 } else { self.lines[k].1 };
        let at = self.points[p].at;
        let entry = self.points.iter().any(|q| q.name.as_deref().is_some_and(|n| n == "HATCH" || n.starts_with("DOOR")) && q.at.distance(at) < 2.0);
        if entry { End::Open } else { End::Closed }
    }

    /// Its hatches' leaves, closed, each where it stands.
    fn leaves(&self) -> Vec<Leaf> {
        let mut out = Vec::new();
        for t in self.tubes() {
            for &SideDoor(k, at, right, h) in &self.doors {
                if k == t.line {
                    out.extend(t.leaf(at * t.len, Some(right), &h));
                }
            }
            for (e, how) in t.ends.iter().enumerate() {
                if *how == Some(End::Door) {
                    out.extend(t.leaf(if e == 0 { 0.0 } else { t.len }, None, &t.hatches[e]));
                }
            }
        }
        out
    }

    /// Line `k`'s end `e`'s hatch (as set; else the usual one).
    fn end_hatch(&self, k: usize, e: usize) -> Hatch {
        self.ends.iter().find(|&&EndSet(j, f, _, _)| j == k && f == e).map_or_else(Hatch::default, |s| s.3)
    }

    /// Is point `p` an end of walled line `k` that no other walled line shares?
    fn free_end(&self, k: usize, p: usize) -> bool {
        !self.groups.iter().filter(|g| g.walled).flat_map(|g| g.lines.iter()).any(|&j| j != k && (self.lines[j].0 == p || self.lines[j].1 == p))
    }

    /// Line `k`'s axis, its ends: its points, or (standing on its line) half its
    /// height over them.
    fn axis(&self, k: usize) -> (Vec3, Vec3) {
        let (a, b, p) = self.lines[k];
        let up = if p.stand && p.section != Section::Line { Vec3::Y * (p.height * 0.5) } else { Vec3::ZERO };
        (self.points[a].at + up, self.points[b].at + up)
    }

    /// The group line `k` is in, if any.
    fn group_of(&self, k: usize) -> Option<usize> {
        self.groups.iter().position(|g| g.lines.contains(&k))
    }
}

/// A line's cross-section: its shape, its width and its height (m), laid round the
/// line.
#[derive(Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
struct Profile {
    section: Section,
    width: f32,
    height: f32,
    /// Stands on its line (the line its floor), rather than round it (its axis).
    #[serde(default)]
    stand: bool,
}

impl Default for Profile {
    fn default() -> Self {
        Profile { section: Section::Line, width: 1.5, height: 2.0, stand: true }
    }
}

/// The sliders' range for a cross-section's width and height (m).
const ROOM_MIN: f32 = 0.3;
const ROOM_MAX: f32 = 6.0;

/// The shape of a line's cross-section.
#[derive(Clone, Copy, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
enum Section {
    /// None: a bare line.
    #[default]
    Line,
    Round,
    Square,
    /// Six-sided, flat at its top and bottom: its height between its flats, its width
    /// across its corners.
    Hex,
    /// Eight-sided, flat at its top, bottom and sides: its width and height between
    /// its flats.
    Oct,
}

impl Section {
    const ALL: [Section; 5] = [Section::Line, Section::Round, Section::Square, Section::Hex, Section::Oct];

    fn name(self) -> &'static str {
        match self {
            Section::Line => "NONE",
            Section::Round => "ROUND",
            Section::Square => "SQUARE",
            Section::Hex => "HEX",
            Section::Oct => "OCT",
        }
    }
}

impl Profile {
    /// Its outline's corners round the line (across, up; m): a round one as 16.
    fn corners(self) -> Vec<Vec2> {
        // (A shape a metre across, stretched to its width and height.)
        let ring = |n: usize, radius: f32, turn: f32| -> Vec<Vec2> { (0..n).map(|k| {
            let a = turn + k as f32 / n as f32 * std::f32::consts::TAU;
            Vec2::new(a.cos(), a.sin()) * radius
        }).collect() };
        let unit = match self.section {
            Section::Line => Vec::new(),
            Section::Round => ring(16, 0.5, 0.0),
            Section::Square => ring(4, 0.5 * std::f32::consts::SQRT_2, std::f32::consts::FRAC_PI_4),
            // (Flat at its top and bottom, its corners at the sides: a metre between its
            // flats, and across its corners.)
            Section::Hex => ring(6, 0.5 / (std::f32::consts::PI / 6.0).cos(), 0.0).into_iter().map(|c| c * Vec2::new((std::f32::consts::PI / 6.0).cos(), 1.0)).collect(),
            Section::Oct => ring(8, 0.5 / (std::f32::consts::PI / 8.0).cos(), std::f32::consts::PI / 8.0),
        };
        unit.into_iter().map(|c| c * Vec2::new(self.width, self.height)).collect()
    }
}

/// A point of the plan: where it is (the hull's frame), and the model's name for
/// it if it's one of the hull's own (the hatch, the cockpit...: those stay).
#[derive(Clone, PartialEq, serde::Serialize, serde::Deserialize)]
struct Point {
    at: Vec3,
    name: Option<String>,
}

/// A dialog over the studio: a new design's hull (or none); a saved design to open
/// (each one's name, and what it's in).
#[derive(Clone, PartialEq)]
enum Dialog {
    New,
    Open(Vec<(String, String)>),
}

#[derive(Clone, Copy, Default, PartialEq)]
enum Tool {
    /// Looking (turning, moving, nearer and farther); a click picks a point or line.
    #[default]
    Look,
    /// Laying paths: a click on the work plane puts a point there, joined to the last;
    /// a click on a point joins to it and runs on from it.
    Path,
    /// Doorways: a click on a tube's end cycles it closed, a doorway, open; on its
    /// side, a doorway there in the wall facing you (again: gone).
    Door,
    /// The ship's modules: one picked from its fit, a click on the work plane puts
    /// it there (or moves it), standing on the plane.
    Modules,
    /// The frame: members laid joint to joint (from landing pads, nozzles, modules'
    /// corners, other joints, or the work plane), of the stock picked; each load
    /// case's forces through it, what's over its limit and what breaks.
    Frame,
}

#[derive(Clone, Copy, PartialEq)]
enum Hover {
    Point(usize),
    Line(usize),
}

/// The hull as the studio draws it: its lines and its bounds (its frame, m).
pub struct Hull {
    /// Its surfaces (to find floors by).
    mesh: Arc<universe_sim::world::walk::WalkMesh>,
    lines: Vec<[Vec3; 2]>,
    /// Where it bends gently (round things): edges with their faces' normals, drawn
    /// where they're its outline from the camera.
    bends: Vec<[Vec3; 4]>,
    lo: Vec3,
    hi: Vec3,
}

impl Interior {
    /// Is layer `k` shown (and the layer it's under)?
    fn shown(&self, k: usize) -> bool {
        !self.hidden[k] && LAYERS[k].1.is_none_or(|p| self.shown(p))
    }

    pub fn new() -> Self {
        Interior { yaw: 0.9, pitch: 0.35, snap_floor: true, depth: 6.0, spacing: 6.0, mix: true, ..Default::default() }
    }

    /// A sample plan for a look (dev scenarios): from the hatch, a run of points on
    /// its level to under the cockpit and up to it, and another to the engines; the
    /// point tool in hand.
    pub fn sample(&mut self, key: &str, shape: &universe_sim::world::shape::Shape) {
        self.seed(key, shape);
        let at = |name: &str, plan: &Plan| plan.points.iter().position(|p| p.name.as_deref() == Some(name));
        let (Some(hatch), Some(cockpit), Some(engines)) = (at("HATCH", &self.plan), at("DASH 1", &self.plan), at("ENGINES", &self.plan)) else { return };
        let (h, c, e) = (self.plan.points[hatch].at, self.plan.points[cockpit].at, self.plan.points[engines].at);
        let add = |p: Vec3, plan: &mut Plan| {
            plan.points.push(Point { at: p, name: None });
            plan.points.len() - 1
        };
        // (The corridor's axis 1 m over the hatch's floor: its 2 m tube stands on it.)
        let a = add(Vec3::new(h.x, h.y + 1.0, (h.z + c.z) * 0.5), &mut self.plan);
        let b = add(Vec3::new(c.x, h.y + 1.0, c.z), &mut self.plan);
        let d = add(Vec3::new(e.x, h.y, (h.z + e.z) * 0.5), &mut self.plan);
        let (round, hex) = (Profile { section: Section::Round, width: 1.6, height: 2.0, stand: false }, Profile { section: Section::Hex, width: 0.9, height: 0.9, stand: false });
        self.plan.lines.extend([(hatch, a, round), (a, b, round), (b, cockpit, round), (hatch, d, hex), (d, engines, hex)]);
        // (The corridor walled off, a group.)
        self.plan.groups.push(Group { lines: vec![0, 1, 2, 3, 4], walled: true });
        self.tool = Tool::Path;
    }

    /// Its walled groups as walls: each tube's sides, two triangles a panel (the
    /// hull's frame), to walk in and bump into.
    pub fn walls(&self) -> Vec<[universe_engine::glam::DVec3; 3]> {
        self.wall_faces().into_iter().map(|(t, _, _)| t).collect()
    }

    /// Its walls as faces to draw: each triangle, its colour, and which of its edges
    /// (first to second, second to third, third to first) are its panel's outline:
    /// its seams.
    pub fn wall_faces(&self) -> Vec<WallFace> {
        let mut out: Vec<WallFace> = self.panels().iter().flat_map(|p| {
            let q = &p.outline;
            let c = p.colour();
            let last = q.len().saturating_sub(2);
            (1..q.len().saturating_sub(1)).map(move |k| ([q[0].as_dvec3(), q[k].as_dvec3(), q[k + 1].as_dvec3()], c, [k == 1, true, k == last]))
        }).collect();
        // (The modules placed: solid, bumped into and stood on.)
        for b in &self.plan.blocks {
            let colour = if b.id == HOLD { [0.42, 0.4, 0.38, 1.0] } else { [0.55, 0.5, 0.62, 1.0] };
            out.extend(b.faces(self.round(&b.id)).into_iter().map(|(t, seams)| (t.map(|p| p.as_dvec3()), colour, seams)));
        }
        out
    }

    /// Is the module in slot `id` round (a tank)?
    fn round(&self, id: &str) -> bool {
        self.fit.iter().any(|f| f.id == kind(id) && f.round)
    }

    /// Its walls' panels: as last worked out for this plan, or worked out now.
    fn panels(&self) -> std::borrow::Cow<'_, [Panel]> {
        match &self.panelled {
            Some((p, panels)) if *p == self.plan => std::borrow::Cow::Borrowed(panels.as_slice()),
            _ => std::borrow::Cow::Owned(self.plan.wall_panels()),
        }
    }

    /// Where one starts at `at` on line `k` (none: a point off any), facing along it:
    /// in a walled tube, on its floor (half its height under its axis: inside it,
    /// not poking out of its top); else there, to settle onto what's under it.
    fn feet(&self, at: Vec3, k: Option<usize>) -> (universe_engine::glam::DVec3, f64) {
        let Some(k) = k else { return ((at + Vec3::Y * 0.05).as_dvec3(), 0.0) };
        let (a, b, profile) = self.plan.lines[k];
        let d = (self.plan.points[b].at - self.plan.points[a].at).normalize_or_zero();
        let walled = self.plan.group_of(k).is_some_and(|g| self.plan.groups[g].walled) && profile.section != Section::Line;
        let floor = if walled && !profile.stand { at - Vec3::Y * (profile.height * 0.5) } else { at };
        ((floor + Vec3::Y * 0.05).as_dvec3(), f64::from(d.x).atan2(f64::from(d.z)))
    }

    /// A walk asked for in the middle of line `k` (dev scenarios), as WALK HERE.
    pub fn walk_line(&mut self, k: usize) {
        if let Some(&(a, b, _)) = self.plan.lines.get(k) {
            self.walk = Some(self.feet(self.plan.points[a].at.lerp(self.plan.points[b].at, 0.5), Some(k)));
        }
    }

    /// Turned to look from `yaw`, `pitch` (rad; dev scenarios).
    pub fn turned(yaw: f32, pitch: f32) -> Self {
        Interior { yaw, pitch, ..Self::new() }
    }

    /// The hull's lines, asked for once a hull and picked up when they're done.
    /// The hull at the work plane's height `y`, worked out again (on a thread) when
    /// it's moved: a metre grid over it, each cell hollow (closed in) or solid.
    fn map_hollow(&mut self, y: f32) {
        if let Some((at, rx)) = &self.hollow_job
            && let Ok(cells) = rx.try_recv()
        {
            self.hollow = Some((*at, Arc::new(cells)));
            self.hollow_job = None;
        }
        let Some((_, h)) = &self.lines else { return };
        if self.hollow_job.is_some() || self.hollow.as_ref().is_some_and(|(at, _)| (at - y).abs() < 0.05) {
            return;
        }
        let (tx, rx) = mpsc::channel();
        let (mesh, lo, hi) = (h.mesh.clone(), h.lo, h.hi);
        job("studio-hollow", move || {
            use universe_sim::world::deckplan::{enclosed, in_material};
            let mut cells = Vec::new();
            let mut z = lo.z.floor() + 0.5;
            while z < hi.z {
                let mut x = lo.x.floor() + 0.5;
                while x < hi.x {
                    let p = universe_engine::glam::DVec2::new(f64::from(x), f64::from(z));
                    if in_material(&mesh, p, f64::from(y)) {
                        cells.push((Vec2::new(x, z), false));
                    } else if enclosed(&mesh, p, f64::from(y), 0.0, false) {
                        cells.push((Vec2::new(x, z), true));
                    }
                    x += 1.0;
                }
                z += 1.0;
            }
            tx.send(cells).ok();
        });
        self.hollow_job = Some((y, rx));
    }

    fn refresh(&mut self) {
        if let Some((k, rx)) = &self.job
            && let Ok(h) = rx.try_recv()
        {
            self.lines = Some((k.clone(), Arc::new(h)));
            self.job = None;
        }
        let key = self.plan.hull.clone();
        // (No hull: the room it's laid out in, empty.)
        let Some(spec) = self.spec() else {
            if self.lines.as_ref().is_none_or(|(k, _)| *k != key) {
                let mesh = Arc::new(universe_sim::world::walk::WalkMesh::new(&[]));
                self.lines = Some((key, Arc::new(Hull { mesh, lines: Vec::new(), bends: Vec::new(), lo: NO_HULL.0, hi: NO_HULL.1 })));
            }
            return;
        };
        let key = key.as_str();
        let Some(mesh) = spec.shape().walk.as_ref() else { return };
        if self.lines.as_ref().is_none_or(|(k, _)| k != key) && self.job.as_ref().is_none_or(|(k, _)| k != key) {
            let (tx, rx) = mpsc::channel();
            let mesh = mesh.clone();
            job("studio-lines", move || {
                // (Its creases of 30° or more, less the tiniest bevels: the shape, not its
                // grain.)
                let lines = mesh.creases(30.0).into_iter().filter(|[a, b]| a.distance(*b) >= 0.02).collect();
                tx.send(Hull { mesh: mesh.clone(), lines, bends: mesh.bends(2.0, 30.0), lo: mesh.lo.as_vec3(), hi: mesh.hi.as_vec3() }).ok();
            });
            self.job = Some((key.into(), rx));
        }
    }

    /// The plan's points from the hull's own places, once a hull: its hatch, its
    /// cockpit, its mounts and docks, its main engines (their middle).
    /// Where a hull's plan is saved.
    fn file(key: &str) -> std::path::PathBuf {
        crate::save::data_dir().join("freefall").join("interiors").join(format!("{key}.json"))
    }

    /// Where its deck plan (the deck studio's) is saved, beside it.
    fn deck_file(key: &str) -> std::path::PathBuf {
        crate::save::data_dir().join("freefall").join("interiors").join(format!("{key}.decks.json"))
    }

    /// The deck plan saved for this plan's hull, if there is one (with decks).
    fn read_decks(&self) -> Option<universe_sim::world::deckplan::DeckPlan> {
        let text = std::fs::read_to_string(Self::deck_file(&self.plan.hull)).ok()?;
        let mut decks: universe_sim::world::deckplan::DeckPlan = serde_json::from_str(&text).map_err(|e| log::warn!("deck plan unreadable: {e}")).ok()?;
        decks.hull = self.plan.hull.clone();
        (!decks.decks.is_empty()).then_some(decks)
    }

    /// Kept in step with the shipyard: its plan seeded (and the saved deck plan put in
    /// the deck studio, the first time, and when opened), the deck studio's decks as
    /// they are now (their floors and walls worked out again when they change).
    pub fn sync(&mut self, ship: &str, shape: &universe_sim::world::shape::Shape, deckplans: &mut Vec<universe_sim::world::deckplan::DeckPlan>, dt: f32) {
        self.seed(ship, shape);
        let key = self.plan.hull.clone();
        let key = key.as_str();
        let walk = self.spec().and_then(|s| s.shape().walk.clone());
        if let Some((_, t)) = self.message.as_mut() {
            *t -= dt;
            if *t <= 0.0 {
                self.message = None;
            }
        }
        if let Some(d) = self.load_decks.take() {
            deckplans.retain(|p| p.hull != key);
            deckplans.push(d);
        }
        let now = deckplans.iter().find(|p| p.hull == key && !p.decks.is_empty()).cloned();
        if self.decks.as_ref().map(|d| &d.0) != now.as_ref() {
            self.decks = match (&now, walk.as_ref()) {
                (Some(plan), Some(mesh)) => Some((plan.clone(), deck_shapes(plan, mesh))),
                _ => None,
            };
        }
        self.decks_now = now;
    }

    /// The plan saved for this plan's hull, its hull's own points moved to where the
    /// model has them now (by name).
    fn read_saved(&self) -> Option<Plan> {
        let text = std::fs::read_to_string(Self::file(&self.plan.hull)).ok()?;
        let mut plan: Plan = serde_json::from_str(&text).map_err(|e| log::warn!("interior plan unreadable: {e}")).ok()?;
        plan.hull = self.plan.hull.clone();
        for p in plan.points.iter_mut() {
            if let Some(fresh) = self.plan.points.iter().find(|q| q.name.is_some() && q.name == p.name) {
                p.at = fresh.at;
            }
        }
        Some(plan)
    }

    /// The plan written to its file, and the deck studio's decks to theirs (none:
    /// that file gone); a message says how it went.
    pub fn save(&mut self) {
        let path = Self::file(&self.plan.hull);
        let decks = Self::deck_file(&self.plan.hull);
        let done = std::fs::create_dir_all(path.parent().expect("a folder"))
            .and_then(|_| std::fs::write(&path, serde_json::to_string_pretty(&self.plan).unwrap_or_default()))
            .and_then(|_| match &self.decks_now {
                Some(d) => std::fs::write(&decks, serde_json::to_string_pretty(d).unwrap_or_default()),
                None => match std::fs::remove_file(&decks) {
                    Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e),
                    _ => Ok(()),
                },
            });
        self.message = Some((
            match done {
                Ok(()) => {
                    self.saved = Some(self.plan.clone());
                    self.saved_decks = Some(self.decks_now.clone());
                    if self.decks_now.is_some() { "SAVED: THIS HULL'S PLAN AND DECKS" } else { "SAVED: THIS HULL'S PLAN" }.to_string()
                }
                Err(e) => format!("NOT SAVED: {e}").to_uppercase(),
            },
            4.0,
        ));
    }

    /// The access plan for the deck studio to show: each tunnel (its line, its
    /// cross-section's width and height, none for a bare line; walled?) and each
    /// point (where, its kind's colour).
    pub fn access(&self) -> Access {
        let plan = &self.plan;
        Access {
            tunnels: plan.lines.iter().enumerate().map(|(k, &(_, _, p))| {
                let room = (p.section != Section::Line).then_some((p.width, p.height));
                let (pa, pb) = plan.axis(k);
                (pa, pb, room, plan.group_of(k).is_some_and(|g| plan.groups[g].walled))
            }).collect(),
            // (Only the kinds of points shown.)
            points: plan.points.iter().filter(|p| self.shown(3 + sort_index(p.name.as_deref()))).map(|p| (p.at, sort(p.name.as_deref()).0)).collect(),
            layers: std::array::from_fn(|k| self.shown(k)),
            modules: plan.blocks.iter().filter_map(|b| self.fit.iter().find(|f| f.id == kind(&b.id)).map(|f| (b.at, b.size, f.round, f.name.clone()))).collect(),
        }
    }

    /// What it has to say for a while (saved, opened), if anything.
    pub fn message(&self) -> Option<&str> {
        self.message.as_ref().map(|m| m.0.as_str())
    }

    /// The MODULES tool in hand, the fit placed in a row down the ship's middle, 8.5 m
    /// over its keel, the first picked (dev scenarios).
    pub fn sample_modules(&mut self, spec: &universe_sim::world::ship::ClassSpec) {
        self.seed(&spec.key, spec.shape());
        // (A design with no hull: a few from the catalogue, one twice, in a row.)
        if self.spec().is_none() {
            self.refit();
            let mut x = -12.0;
            for (n, f) in self.fit.iter().step_by(7).take(7).enumerate() {
                for copy in 1..=if n == 0 { 2 } else { 1 } {
                    self.plan.blocks.push(Block { id: format!("{}#{copy}", f.id), at: Vec3::new(x + f.size.x * 0.5, f.size.y * 0.5, 0.0), size: f.size });
                    x += f.size.x + 1.0;
                }
            }
            self.tool = Tool::Modules;
            self.block = Some(0);
            self.module = Some(0);
            return;
        }
        self.fit = fit_of(spec);
        let Some(mesh) = spec.shape().walk.as_ref() else { return };
        let (lo, hi) = (mesh.lo.as_vec3(), mesh.hi.as_vec3());
        let floor = lo.y + 8.5;
        let mut z = lo.z + 10.0;
        for f in &self.fit {
            self.plan.blocks.push(Block { id: f.id.clone(), at: Vec3::new(0.0, floor + f.size.y * 0.5, z + f.size.z * 0.5), size: f.size });
            z += f.size.z + 0.6;
            if z > hi.z - 4.0 {
                break;
            }
        }
        self.tool = Tool::Modules;
        self.module = Some(0);
    }

    /// What `spec` is fitted with, to be placed (as it is now: refitted, it changes).
    pub fn refit(&mut self) {
        self.fit = match self.spec() {
            Some(spec) => fit_of(spec),
            None => catalogue().to_vec(),
        };
        // (A placed module whose record's size has changed since: made that size,
        // standing where it stood; in the saved plan too: it's the registry's change,
        // not the designer's, so nothing's unsaved by it.)
        let resized = resize(&mut self.plan.blocks, &self.fit);
        if let Some(saved) = self.saved.as_mut() {
            resize(&mut saved.blocks, &self.fit);
        }
        if resized > 0 {
            self.message = Some((format!("{resized} MODULES MADE THE SIZE THE REGISTRY NOW GIVES THEM"), 6.0));
        }
    }

    /// A dialog open (dev scenarios): NEW, or OPEN.
    pub fn show_dialog(&mut self, open: bool) {
        self.dialog = Some(if open { Dialog::Open(Self::designs()) } else { Dialog::New });
    }

    /// Saved design `id` opened (dev scenarios).
    pub fn open_saved(&mut self, id: &str) {
        self.open_design(id);
    }

    /// A new design (dev scenarios): in the hull keyed `hull`, or none.
    pub fn start_new(&mut self, hull: Option<&str>) {
        let spec = hull.and_then(|k| hulls().into_iter().find(|h| h.key == k));
        self.new_design(spec);
    }

    /// The FRAME tool in hand (dev scenarios).
    pub fn frame_tool(&mut self) {
        self.tool = Tool::Frame;
    }

    /// Layer `k` hidden (dev scenarios).
    pub fn hide(&mut self, k: usize) {
        if let Some(h) = self.hidden.get_mut(k) {
            *h = true;
        }
    }

    /// Its hatches' leaves, closed (the walk-through slides them open).
    pub fn leaves(&self) -> Vec<Leaf> {
        self.plan.leaves()
    }

    /// Where the cursor was last seen (HUD pixels).
    pub fn cursor(&self) -> Vec2 {
        self.cursor
    }

    /// Changed since it was last saved or opened?
    pub fn unsaved(&self) -> bool {
        self.saved.as_ref().is_some_and(|s| *s != self.plan) || self.saved_decks.as_ref().is_some_and(|d| *d != self.decks_now)
    }

    /// Asked to close (ESC, the shipyard key): true if it can go now; with unsaved
    /// changes, it asks first (SAVE, DISCARD or keep working).
    pub fn close(&mut self) -> bool {
        if self.unsaved() {
            self.confirm = true;
            false
        } else {
            true
        }
    }

    /// The design opened before any is chosen: the plan saved for the hull being
    /// laid out (`key`), or a fresh one of it.
    fn seed(&mut self, key: &str, shape: &universe_sim::world::shape::Shape) {
        if !self.plan.hull.is_empty() {
            return;
        }
        self.start(key, Some(shape), None);
    }

    /// Design `id` opened: in a hull (its `shape`; `on` its key, if not the design's
    /// own name), or none; the plan saved as `id` if there is one, else fresh (the
    /// hull's own places as points); its decks too.
    fn start(&mut self, id: &str, shape: Option<&universe_sim::world::shape::Shape>, on: Option<String>) {
        let points = match shape {
            Some(shape) => {
            use universe_sim::world::shape::Role;
            let mut points: Vec<Point> = shape
                .nodes
                .iter()
                .filter(|n| matches!(n.role, Role::Hatch | Role::Mount | Role::Dock))
                .map(|n| Point { at: n.at.as_vec3(), name: Some(n.name.to_uppercase().replace('_', " ")) })
                .collect();
            let engines: Vec<Vec3> = shape.nodes(Role::Nozzle).filter(|n| n.name.starts_with("nozzle_main")).map(|n| n.at.as_vec3()).collect();
            if !engines.is_empty() {
                points.push(Point { at: engines.iter().copied().sum::<Vec3>() / engines.len() as f32, name: Some("ENGINES".into()) });
            }
            // From its named parts (as the MC-07 names them; another hull gets those it
            // names alike): where the ore comes in, and where its machinery is serviced.
            let span = |prefix: &str| {
                shape.pieces.iter().filter(|(n, _, _)| n.starts_with(prefix)).fold(None, |b: Option<(Vec3, Vec3)>, (_, lo, hi)| {
                    let (lo, hi) = (lo.as_vec3(), hi.as_vec3());
                    Some(b.map_or((lo, hi), |(l, h)| (l.min(lo), h.max(hi))))
                })
            };
            type At = fn((Vec3, Vec3)) -> Vec3;
            let places: [(&str, &str, At); 8] = [
                ("Hull_EngineBlock", "SERVICE ENGINE BLOCK", |b| (b.0 + b.1) * 0.5),
                // (A leg's strut at its top, in its bay.)
                ("Gear_FL_Strut", "SERVICE GEAR FL", |(lo, hi)| Vec3::new((lo.x + hi.x) * 0.5, hi.y, (lo.z + hi.z) * 0.5)),
                ("Gear_FR_Strut", "SERVICE GEAR FR", |(lo, hi)| Vec3::new((lo.x + hi.x) * 0.5, hi.y, (lo.z + hi.z) * 0.5)),
                ("Gear_RL_Strut", "SERVICE GEAR RL", |(lo, hi)| Vec3::new((lo.x + hi.x) * 0.5, hi.y, (lo.z + hi.z) * 0.5)),
                ("Gear_RR_Strut", "SERVICE GEAR RR", |(lo, hi)| Vec3::new((lo.x + hi.x) * 0.5, hi.y, (lo.z + hi.z) * 0.5)),
                ("HammerL_Hatch", "SERVICE HAMMER L", |b| (b.0 + b.1) * 0.5),
                ("HammerR_Hatch", "SERVICE HAMMER R", |b| (b.0 + b.1) * 0.5),
                // (A clamp's mast at its foot, where it meets the hull.)
                ("ClampFwd_", "SERVICE CLAMP FWD", |(lo, hi)| Vec3::new((lo.x + hi.x) * 0.5, lo.y, (lo.z + hi.z) * 0.5)),
            ];
            for (prefix, name, at) in places {
                if let Some(b) = span(prefix) {
                    points.push(Point { at: at(b), name: Some(name.into()) });
                }
            }
            if let Some((lo, hi)) = span("ClampAft_") {
                points.push(Point { at: Vec3::new((lo.x + hi.x) * 0.5, lo.y, (lo.z + hi.z) * 0.5), name: Some("SERVICE CLAMP AFT".into()) });
            }
            // Where the ore comes in: the scoop's middle, level with the hull round it
            // (its top just outside the scoop, straight down from above).
            if let (Some((lo, hi)), Some(mesh)) = (span("OreScoop_"), shape.walk.as_ref()) {
                let c = (lo + hi) * 0.5;
                let reach = (hi - lo) * 0.5 + Vec3::splat(0.5);
                let top = mesh.hi.y + 1.0;
                let level = [(reach.x, 0.0), (-reach.x, 0.0), (0.0, reach.z), (0.0, -reach.z)]
                    .iter()
                    .filter_map(|(dx, dz)| mesh.ray(universe_engine::glam::DVec3::new((c.x + dx) as f64, top, (c.z + dz) as f64), universe_engine::glam::DVec3::NEG_Y, top - mesh.lo.y).map(|(d, _)| (top - d) as f32))
                    .fold(f32::INFINITY, f32::min);
                if level.is_finite() {
                    points.push(Point { at: Vec3::new(c.x, level, c.z), name: Some("MINING OPENING".into()) });
                }
            }
            // Its doorways (marked in the model: `door_*`); every window (each pane of its glass, less the doors'); and
            // its dash, a point at each screen.
            for n in shape.nodes(Role::Door) {
                points.push(Point { at: n.at.as_vec3(), name: Some(n.name.to_uppercase().replace('_', " ")) });
            }
            let panes = |what: &str| -> Vec<(Vec3, Vec3)> { shape.islands.iter().filter(|(n, _, _)| n.contains(what) && !n.starts_with("CrewDoor")).map(|(_, lo, hi)| (lo.as_vec3(), hi.as_vec3())).collect() };
            // (A window a pane of a square metre or more: smaller glass is a lens or a
            // gauge's. A stand-in until a model names its windows.)
            let area = |(lo, hi): &(Vec3, Vec3)| {
                let mut d = (*hi - *lo).to_array();
                d.sort_by(f32::total_cmp);
                d[1] * d[2]
            };
            let windows: Vec<Vec3> = panes("Glass").into_iter().filter(|p| area(p) >= 1.0).map(|(lo, hi)| (lo + hi) * 0.5).collect();
            for (k, at) in windows.into_iter().enumerate() {
                points.push(Point { at, name: Some(format!("WINDOW {}", k + 1)) });
            }
            for (k, at) in panes("DashScreen").into_iter().map(|(lo, hi)| (lo + hi) * 0.5).enumerate() {
                points.push(Point { at, name: Some(format!("DASH {}", k + 1)) });
            }
                points
            }
            None => Vec::new(),
        };
        self.plan = Plan { hull: id.into(), points, lines: Vec::new(), groups: Vec::new(), ends: Vec::new(), doors: Vec::new(), blocks: Vec::new(), beams: Vec::new(), on, pads: Vec::new() };
        // The plan saved for this hull, if there is one (its hull's own points where
        // the model has them now).
        if let Some(saved) = self.read_saved() {
            self.plan = saved;
        }
        self.saved = Some(self.plan.clone());
        // Its saved decks put in the deck studio (none saved: none).
        let decks = self.read_decks();
        self.saved_decks = Some(decks.clone());
        self.load_decks = Some(decks.unwrap_or_else(|| universe_sim::world::deckplan::DeckPlan { hull: id.into(), decks: Vec::new() }));
        // (Nothing of the last design's carried over.)
        self.undo.clear();
        self.redo.clear();
        (self.pick, self.from, self.module, self.block, self.beam_pick, self.beam_from) = (None, None, None, None, None, None);
        self.more.clear();
    }

    /// A new design, in hull `hull` (none: from nothing), named the first free name
    /// for it.
    fn new_design(&mut self, hull: Option<&'static universe_sim::world::ship::ClassSpec>) {
        let base = hull.map_or("design".to_string(), |h| h.key.clone());
        let id = (if hull.is_some() { 2 } else { 1 }..).map(|n| format!("{base}-{n}")).find(|id| !Self::file(id).exists()).unwrap_or(base);
        self.start(&id, hull.map(|h| h.shape()), Some(hull.map_or(String::new(), |h| h.key.clone())));
        // (Fresh: unsaved until saved.)
        self.saved = None;
        self.saved_decks = None;
        self.message = Some((format!("NEW DESIGN: {}", hull.map_or("NO HULL".to_string(), |h| h.name.to_uppercase())), 4.0));
    }

    /// The designs saved: each one's name and what it's in.
    fn designs() -> Vec<(String, String)> {
        let dir = crate::save::data_dir().join("freefall").join("interiors");
        let mut out: Vec<(String, String)> = std::fs::read_dir(dir).into_iter().flatten().flatten().filter_map(|e| {
            let name = e.file_name().to_string_lossy().into_owned();
            let id = name.strip_suffix(".json").filter(|n| !n.ends_with(".decks"))?.to_string();
            let plan: Plan = serde_json::from_str(&std::fs::read_to_string(e.path()).ok()?).ok()?;
            let plan = Plan { hull: id.clone(), ..plan };
            Some((id, hull_of(&plan).map_or("NO HULL".to_string(), |h| h.name.to_uppercase())))
        }).collect();
        out.sort();
        out
    }

    /// Saved design `id` opened.
    fn open_design(&mut self, id: &str) {
        let text = std::fs::read_to_string(Self::file(id)).unwrap_or_default();
        let on = serde_json::from_str::<Plan>(&text).ok().and_then(|p| p.on);
        let probe = Plan { hull: id.into(), on: on.clone(), ..Default::default() };
        let hull = hull_of(&probe);
        self.start(id, hull.map(|h| h.shape()), on);
        self.message = Some((format!("OPENED {}", id.to_uppercase()), 4.0));
    }

    /// Is something in progress (to cancel): a module picked, a path, member or truss
    /// being laid, TRUSS waiting for its corners, a member picked, WALK HERE armed?
    fn in_progress(&self) -> bool {
        self.module.is_some() || self.block.is_some() || self.from.is_some() || self.beam_from.is_some() || self.truss_from.is_some() || self.truss_mode || self.beam_pick.is_some() || self.walk_armed
    }

    /// What's in progress cancelled; was anything?
    fn cancel(&mut self) -> bool {
        let any = self.in_progress();
        (self.module, self.block, self.from, self.beam_from, self.truss_from, self.beam_pick) = (None, None, None, None, None, None);
        (self.truss_mode, self.walk_armed) = (false, false);
        any
    }

    /// The design's name (what it's saved as).
    pub fn id(&self) -> &str {
        &self.plan.hull
    }

    /// Something to say for a while, under the toolbar.
    pub fn note(&mut self, text: &str) {
        self.message = Some((text.to_string(), 4.0));
    }

    /// The hull this design is in, if any.
    pub fn spec(&self) -> Option<&'static universe_sim::world::ship::ClassSpec> {
        hull_of(&self.plan)
    }

    /// The hull (or, with none, the room) as it's drawn, once worked out.
    fn hull(&self) -> Option<Arc<Hull>> {
        self.lines.as_ref().filter(|(k, _)| *k == self.plan.hull).map(|(_, h)| h.clone())
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
const TOOLBAR: [(&str, &str, Tool); 5] = [("V", "LOOK", Tool::Look), ("P", "PATH", Tool::Path), ("D", "DOOR", Tool::Door), ("M", "MODULES", Tool::Modules), ("R", "FRAME", Tool::Frame)];

/// Where toolbar button `k` is (one row along the top).
fn button(k: usize) -> (Vec2, Vec2) {
    (Vec2::new(12.0 + k as f32 * 78.0, 30.0), Vec2::new(75.0, 16.0))
}

fn button_at(q: Vec2) -> Option<Tool> {
    (0..TOOLBAR.len()).find(|&k| inside(button(k), q)).map(|k| TOOLBAR[k].2)
}

/// UNDO and REDO, after the tools (key, name, which: false undo, true redo).
const HISTORY: [(&str, &str, bool); 2] = [("^Z", "UNDO", false), ("^Y", "REDO", true)];

fn history_button(k: usize) -> (Vec2, Vec2) {
    button(TOOLBAR.len() + k)
}

/// WALK HERE, after UNDO and REDO.
fn walk_button() -> (Vec2, Vec2) {
    button(TOOLBAR.len() + HISTORY.len())
}

/// SAVE (0), OPEN (1) and NEW (2), after WALK HERE.
fn file_button(k: usize) -> (Vec2, Vec2) {
    button(TOOLBAR.len() + HISTORY.len() + 1 + k)
}

/// REACH, after SAVE, OPEN and NEW.
fn reach_button() -> (Vec2, Vec2) {
    button(TOOLBAR.len() + HISTORY.len() + 4)
}

/// A dialog's rows (`n` of them, the last CANCEL), in a box in the middle: the box,
/// then each row.
fn dialog_rows(size: Vec2, n: usize) -> ((Vec2, Vec2), Vec<(Vec2, Vec2)>) {
    let (w, h) = (440.0, 44.0 + n as f32 * 22.0);
    let p = Vec2::new((size.x - w) * 0.5, (size.y - h) * 0.5);
    ((p, Vec2::new(w, h)), (0..n).map(|k| (Vec2::new(p.x + 10.0, p.y + 32.0 + k as f32 * 22.0), Vec2::new(w - 20.0, 18.0))).collect())
}

/// A dialog's choices (what each row says), CANCEL last.
fn dialog_choices(d: &Dialog) -> Vec<String> {
    let mut out: Vec<String> = match d {
        Dialog::New => {
            let mut v: Vec<String> = hulls().iter().map(|h| {
                let (lo, hi) = h.shape().walk.as_ref().map_or((Default::default(), Default::default()), |m| (m.lo, m.hi));
                let d = hi - lo;
                format!("{}   {:.0} X {:.0} X {:.0} M", h.name.to_uppercase(), d.z, d.x, d.y)
            }).collect();
            v.push("NO HULL: FROM NOTHING".into());
            v
        }
        Dialog::Open(list) => list.iter().map(|(id, on)| format!("{}   ({on})", id.to_uppercase())).collect(),
    };
    out.push("CANCEL".into());
    out
}

/// What the reach check found: the floor a walker gets to from the hatch (spots
/// on it), where the floor drops away, and, of the plan, what it gets to.
pub struct Reach {
    floor: Vec<Vec3>,
    drops: Vec<Vec3>,
    /// Each walled tube's line and whether it's got to; each deck and whether;
    /// the named points looked for (service, dash, doors) and whether.
    tubes: Vec<(usize, bool)>,
    decks: Vec<(usize, bool)>,
    points: Vec<(String, bool)>,
}

/// The reach check: from the hatch, every way a walker can go over the hull's
/// floors, the decks and the walled tubes (a step 40 cm at a time: up or down no
/// more than 0.5 m, head room 1.75 m, nothing in the way over a step's height or at
/// the chest); where the floor drops more than that, a drop. Then what of the plan it
/// gets to.
fn reach(hull: &universe_sim::world::walk::WalkMesh, walls: &[[universe_engine::glam::DVec3; 3]], decks: Option<&universe_sim::world::deckplan::DeckPlan>, plan: &Plan) -> Reach {
    use universe_engine::glam::DVec3;
    use universe_sim::world::walk::WalkMesh;
    let walls = (!walls.is_empty()).then(|| WalkMesh::new(walls));
    let built = decks.map(|d| {
        let sides: Vec<_> = d.decks.iter().map(|k| universe_sim::world::deckplan::deck_sides(hull, k.floor)).collect();
        universe_sim::world::deckplan::Walkable::from(&universe_sim::world::deckplan::build(d, &sides))
    });
    let meshes: Vec<&WalkMesh> = std::iter::once(hull).chain(walls.as_ref()).chain(built.as_ref().map(|b| &b.mesh)).collect();
    let ray = |from: DVec3, dir: DVec3, max: f64| meshes.iter().filter_map(|m| m.ray(from, dir, max)).min_by(|a, b| a.0.total_cmp(&b.0));
    // The floor under `p` from `p.y`, within `max` (a surface facing up).
    let floor = |p: DVec3, max: f64| ray(p, DVec3::NEG_Y, max).and_then(|(d, n)| (n.y > 0.6).then_some(p.y - d));
    const STEP: f64 = 0.4;
    let key = |p: DVec3| ((p.x / STEP).round() as i64, (p.z / STEP).round() as i64, (p.y / 0.25).round() as i64);
    let mut out = Reach { floor: Vec::new(), drops: Vec::new(), tubes: Vec::new(), decks: Vec::new(), points: Vec::new() };
    let Some(hatch) = plan.points.iter().find(|p| p.name.as_deref() == Some("HATCH")).map(|p| p.at.as_dvec3()) else { return out };
    let Some(y0) = floor(hatch + DVec3::Y * 0.5, 2.0) else { return out };
    let start = DVec3::new(hatch.x, y0, hatch.z);
    let mut seen = std::collections::HashSet::new();
    let mut todo = std::collections::VecDeque::from([start]);
    seen.insert(key(start));
    while let Some(p) = todo.pop_front() {
        out.floor.push(p.as_vec3());
        if out.floor.len() > 80_000 {
            break;
        }
        for d in [DVec3::X, -DVec3::X, DVec3::Z, -DVec3::Z] {
            // (Nothing in the way at the knee and the chest, a body's width on.)
            if [0.65, 1.4].iter().any(|h| ray(p + DVec3::Y * h, d, STEP + 0.2).is_some()) {
                continue;
            }
            let q = p + d * STEP;
            match floor(q + DVec3::Y * 0.55, 1.1) {
                Some(y) => {
                    let to = DVec3::new(q.x, y, q.z);
                    // (Head room there.)
                    if ray(to + DVec3::Y * 0.05, DVec3::Y, 1.7).is_some() || !seen.insert(key(to)) {
                        continue;
                    }
                    todo.push_back(to);
                }
                None => out.drops.push(q.as_vec3()),
            }
        }
    }
    // What of the plan's got to: a walled tube with floor reached inside it; a deck
    // with floor reached on it; a point with floor reached within 2.5 m.
    let tubes = plan.tubes();
    let inside = |t: &Tube, p: Vec3| t.planes().iter().all(|&(n, c)| n.dot(p + Vec3::Y * 0.3) < c);
    out.tubes = tubes.iter().map(|t| (t.line, out.floor.iter().any(|p| inside(t, *p)))).collect();
    if let Some(d) = decks {
        // (On its floor: at its height and inside one of its floor's outlines.)
        let within = |poly: &[universe_engine::glam::DVec2], x: f64, z: f64| {
            let mut odd = false;
            for k in 0..poly.len() {
                let (a, b) = (poly[k], poly[(k + 1) % poly.len()]);
                if (a.y > z) != (b.y > z) && x < a.x + (b.x - a.x) * (z - a.y) / (b.y - a.y) {
                    odd = !odd;
                }
            }
            odd
        };
        out.decks = d.decks.iter().enumerate().map(|(k, deck)| {
            (k, out.floor.iter().any(|p| (f64::from(p.y) - deck.floor).abs() < 0.3 && deck.planes.iter().any(|poly| within(poly, f64::from(p.x), f64::from(p.z)))))
        }).collect();
    }
    out.points = plan.points.iter().filter_map(|p| {
        let name = p.name.as_deref()?;
        (name.starts_with("SERVICE") || name.starts_with("DASH") || name.starts_with("DOOR")).then(|| (name.to_string(), out.floor.iter().any(|f| f.distance(p.at) < 2.5)))
    }).collect();
    out
}

/// The close dialog: where it is, and its buttons (SAVE AND CLOSE, DISCARD, KEEP
/// WORKING).
fn confirm_box(size: Vec2) -> ((Vec2, Vec2), [(Vec2, Vec2); 3]) {
    let (w, h) = (460.0, 96.0);
    let p = Vec2::new((size.x - w) * 0.5, (size.y - h) * 0.5);
    let bw = (w - 16.0 - 12.0) / 3.0;
    let b = |k: f32| (Vec2::new(p.x + 8.0 + k * (bw + 6.0), p.y + h - 26.0), Vec2::new(bw, 18.0));
    ((p, Vec2::new(w, h)), [b(0.0), b(1.0), b(2.0)])
}

fn inside((p, c): (Vec2, Vec2), q: Vec2) -> bool {
    q.x >= p.x && q.x <= p.x + c.x && q.y >= p.y && q.y <= p.y + c.y
}

/// The tool's panel, at the left: where it is and its size.
const PANEL: (Vec2, Vec2) = (Vec2::new(12.0, 56.0), Vec2::new(250.0, 356.0));

/// Room presets for new lines: name, cross-section, and whether their free ends get
/// doorways (a room's) rather than walls.
const PRESETS: [(&str, Profile, bool); 5] = [
    ("CORRIDOR", Profile { section: Section::Square, width: 1.6, height: 2.4, stand: true }, false),
    ("CRAWLWAY", Profile { section: Section::Hex, width: 1.0, height: 1.0, stand: true }, false),
    ("CABIN", Profile { section: Section::Square, width: 2.6, height: 2.6, stand: true }, true),
    ("GALLEY", Profile { section: Section::Oct, width: 5.0, height: 3.0, stand: true }, true),
    ("BRIDGE", Profile { section: Section::Oct, width: 10.0, height: 4.0, stand: true }, true),
];

/// The panel's actions: the work plane down and up, what's picked out.
#[derive(Clone, Copy, PartialEq)]
enum Action {
    PlaneDown,
    PlaneUp,
    Remove,
    /// The cross-section's shape: for new lines (laying paths), or the picked line's.
    Section(Section),
    /// A room preset for new lines (its number in `PRESETS`).
    Preset(usize),
    /// Standing on its line, or round it (new lines', or the picked ones').
    Stand,
    /// Points laid dropped to the floor, or not.
    SnapFloor,
    /// How new hatches slide.
    Slide(Slide),
    /// The picked module turned a quarter (its width and length swapped).
    Turn,
    /// The load case shown on the frame (none: each member's worst).
    Case(Option<usize>),
    /// Every member past its limit braced (split at its middle, tied to the nearest
    /// joint).
    Brace,
    /// AUTO-SIZE free to pick any material (or each member's own).
    Mix,
    /// Laying a truss (two clicks: its corners on the plane), or not.
    Truss,
    /// Every placed module the frame doesn't hold mounted to it.
    MountAll,
    /// The members sized to their loads.
    AutoSize,
    /// The picked lines made a group; their groups broken up; walled off (or open).
    Group,
    Ungroup,
    Wall,
}

/// The panel's sliders: the cross-section's width (0) and height (1) (a module's,
/// lower down, under its list), each its track (where, its size).
fn sliders(tool: Tool) -> [(Vec2, Vec2); 2] {
    let (p, c) = PANEL;
    let top = match tool {
        Tool::Modules => 264.0,
        Tool::Frame => 262.0,
        _ => 202.0,
    };
    [0.0, 22.0].map(|dy| (Vec2::new(p.x + 92.0, p.y + top + dy), Vec2::new(c.x - 100.0, 10.0)))
}

/// A slider's range (m): a room's, or a module's (a hold is broad).
fn slider_range(tool: Tool) -> (f32, f32) {
    match tool {
        Tool::Modules => (0.3, 16.0),
        // (A truss's depth and its stations' spacing.)
        Tool::Frame => (1.0, 12.0),
        _ => (ROOM_MIN, ROOM_MAX),
    }
}

/// What a person aboard uses a day (kg): oxygen breathed, and water drunk, eaten
/// with and washed in (NASA's BVAD, in standards/sources/research_people_needs.json).
const OXYGEN_A_DAY: f64 = 0.895;
const WATER_A_DAY: f64 = 2.5 + 0.7;

/// The share of a drive's or lift's loss that comes aboard as heat (the rest goes
/// out with its plume: the registry's mounts reckon cooling this way).
const PLUME_SHARE: f64 = 1e-6;

/// The design's budgets and what doesn't work: its mass (dry, full, the frame's),
/// lift against its full weight, power drawn against supplied, heat to shed
/// against radiators and coolant loops, crew and their days of air and water; and
/// each check that fails (a ramp or lift that doesn't reach the ground, an airlock
/// on no room, no way in, an ore path with a gap).
struct Budget {
    lines: Vec<(String, bool)>,
    faults: Vec<String>,
}

/// A record's function's fields, by name.
type Fields = serde_json::Map<String, serde_json::Value>;

/// A placed module's record's figures: its function's kind and fields, and the
/// power it draws (W).
fn figures(f: &Fitted) -> Option<(String, Fields, f64)> {
    let reg = universe_sim::world::registry::registry();
    let e = reg.equipment.iter().find(|e| e.identity.key == f.key)?;
    let serde_json::Value::Object(map) = serde_json::to_value(&e.function).ok()? else { return None };
    let kind = map.get("kind").and_then(|v| v.as_str()).unwrap_or("").to_string();
    Some((kind, map, e.needs.power.unwrap_or(0.0)))
}

fn budget(i: &Interior, frame_mass: f64) -> Budget {
    let num = |m: &serde_json::Map<String, serde_json::Value>, k: &str| m.get(k).and_then(|v| v.as_f64()).unwrap_or(0.0);
    let mut placed: Vec<(&Block, &Fitted, String, Fields, f64)> = Vec::new();
    for b in &i.plan.blocks {
        if let Some(f) = i.fit.iter().find(|f| f.id == kind(&b.id))
            && let Some((k, m, draw)) = figures(f)
        {
            placed.push((b, f, k, m, draw));
        }
    }
    let of = |k: &'static str| placed.iter().filter(move |p| p.2 == k);
    let dry: f64 = i.plan.blocks.iter().filter_map(|b| i.fit.iter().find(|f| f.id == kind(&b.id))).map(|f| f.mass).sum::<f64>() + frame_mass;
    let full = dry + i.plan.blocks.iter().filter_map(|b| i.fit.iter().find(|f| f.id == kind(&b.id))).map(|f| f.load).sum::<f64>();
    let mut lines = Vec::new();
    let t = |kg: f64| format!("{:.1} T", kg / 1000.0);
    lines.push((format!("MASS  DRY {}  FULL {}  (FRAME {})", t(dry), t(full), t(frame_mass)), true));
    let lift: f64 = of("lift").map(|p| num(&p.3, "thrust")).sum();
    if lift > 0.0 {
        let g = lift / (full * STANDARD_G);
        lines.push((format!("LIFT  {}: {g:.2} OF ITS FULL WEIGHT AT 1 G", si(lift, "N")), g > 1.0));
    }
    let supply: f64 = of("power_plant").map(|p| num(&p.3, "output")).sum::<f64>() + of("solar_array").map(|p| num(&p.3, "output")).sum::<f64>();
    let draw: f64 = placed.iter().map(|p| p.4).sum();
    let gear: f64 = of("switchgear").map(|p| num(&p.3, "carries")).sum();
    lines.push((format!("POWER {} DRAWN OF {}{}", si(draw, "W"), si(supply, "W"), if gear > 0.0 { format!(", SWITCHGEAR {}", si(gear, "W")) } else { String::new() }), draw <= supply && (gear == 0.0 || gear >= supply)));
    // (Heat: a plant's loss, a drive's and lift's share at full burn, and every
    // watt drawn.)
    let plants: f64 = of("power_plant").map(|p| { let e = num(&p.3, "efficiency").max(0.01); num(&p.3, "output") * (1.0 / e - 1.0) }).sum();
    let burn: f64 = placed.iter().filter(|p| p.2 == "drive" || p.2 == "lift").map(|p| { let e = num(&p.3, "efficiency").max(0.01); 0.5 * num(&p.3, "thrust") * num(&p.3, "exhaust") * (1.0 / e - 1.0) * PLUME_SHARE }).sum();
    let heat = plants + burn + draw;
    let shed: f64 = of("radiator").map(|p| num(&p.3, "rejects")).sum();
    let loops: f64 = of("coolant_loop").map(|p| num(&p.3, "carries")).sum();
    lines.push((format!("HEAT  {} TO SHED: RADIATORS {}, LOOPS {}", si(heat, "W"), si(shed, "W"), si(loops, "W")), shed >= heat && loops >= heat));
    let crew: f64 = of("cabin").map(|p| num(&p.3, "seats")).sum();
    if crew > 0.0 {
        let air: f64 = of("store").filter(|p| p.3.get("holds").and_then(|v| v.as_str()) == Some("element.o")).map(|p| num(&p.3, "capacity")).sum();
        let water: f64 = of("store").filter(|p| p.3.get("holds").and_then(|v| v.as_str()) == Some("good.water")).map(|p| num(&p.3, "capacity")).sum();
        let (ad, wd) = (air / (OXYGEN_A_DAY * crew), water / (WATER_A_DAY * crew));
        let life = of("life_support").count();
        lines.push((format!("CREW  {crew:.0}: AIR {ad:.0} DAYS, WATER {wd:.0} DAYS STORED{}", if life == 0 { ", NO LIFE SUPPORT" } else { "" }), ad >= 1.0 && wd >= 1.0 && life > 0));
    }
    // Checks: the way in, the way down, the ore's path.
    let mut faults = Vec::new();
    let ground = of("landing_gear").map(|p| p.0.at.y - p.0.size.y * 0.5).fold(f32::INFINITY, f32::min);
    for p in placed.iter().filter(|p| p.2 == "ramp" || p.2 == "cargo_lift") {
        let foot = p.0.at.y - p.0.size.y * 0.5;
        let reach = num(&p.3, if p.2 == "ramp" { "reach" } else { "travel" }) as f32;
        if ground.is_finite() && foot - ground > reach + 0.05 {
            faults.push(format!("{} REACHES {reach:.1} M; ITS FOOT IS {:.1} M UP", p.1.name, foot - ground));
        }
    }
    let rooms: Vec<Vec3> = (0..i.plan.lines.len()).filter(|&k| i.plan.group_of(k).is_some_and(|g| i.plan.groups[g].walled)).flat_map(|k| {
        let (a, b) = i.plan.axis(k);
        room(a, b, i.plan.lines[k].2).1
    }).collect();
    let locks: Vec<_> = of("airlock").collect();
    if crew > 0.0 && locks.is_empty() {
        faults.push("NO WAY IN: NO AIRLOCK".into());
    }
    for p in &locks {
        let (lo, hi) = p.0.bounds();
        if !rooms.iter().any(|q| q.cmpge(lo - 1.0).all() && q.cmple(hi + 1.0).all()) {
            faults.push(format!("{} OPENS ON NO ROOM", p.1.name));
        }
    }
    if let Some(rig) = of("mining_rig").next() {
        // (By the rig: within 2 m of it, its nearest side.)
        let (rlo, rhi) = rig.0.bounds();
        let off = |q: Vec3| (q.clamp(rlo, rhi) - q).length();
        let scoop = of("handling").filter(|p| p.1.name.contains("SCOOP")).map(|p| p.0).min_by(|a, b| off(a.at).total_cmp(&off(b.at))).map(|b| (b.at, { let (lo, hi) = b.bounds(); let c = rig.0.at.clamp(lo, hi); off(c) }));
        let bay = of("ore_bay").map(|p| p.0.at).chain(i.plan.blocks.iter().filter(|b| b.id == HOLD).map(|b| b.at)).next();
        match (scoop, bay) {
            (None, _) => faults.push("ORE: NO SCOOP BY THE RIG".into()),
            (_, None) => faults.push("ORE: NOWHERE TO PUT IT (NO ORE BAY)".into()),
            (Some((s, near)), Some(b)) => {
                let reach: f64 = of("handling").filter(|p| p.1.name.contains("CONVEYOR")).map(|p| num(&p.3, "reach")).sum();
                let gap = f64::from(s.distance(b));
                if near > 2.0 {
                    faults.push(format!("ORE: THE SCOOP IS {near:.0} M FROM THE RIG"));
                } else if reach + 2.0 < gap {
                    faults.push(format!("ORE: SCOOP TO BAY {gap:.0} M, CONVEYORS REACH {reach:.0} M"));
                }
            }
        }
    }
    Budget { lines, faults }
}

/// A figure with its unit, at a readable size: 900000 N as 900 KN, 1e7 m/s as
/// 10000 KM/S.
fn si(v: f64, unit: &str) -> String {
    if v == 0.0 {
        return format!("0 {unit}");
    }
    let a = v.abs();
    let (scale, prefix) = if a >= 1e12 { (1e12, "T") } else if a >= 1e9 { (1e9, "G") } else if a >= 1e6 { (1e6, "M") } else if a >= 1e3 && unit != "KG" { (1e3, "K") } else { (1.0, "") };
    let n = v / scale;
    let digits = if n.abs() >= 100.0 || n.fract() == 0.0 { 0 } else if n.abs() >= 10.0 { 1 } else { 2 };
    format!("{n:.digits$} {prefix}{unit}")
}

/// A module's sheet, beside the panel: its name, maker, kind and mount; its shape,
/// turning; its size, mass, volume and the power it draws; its record's figures;
/// its description. Placed and stretched (`block`): the size it's placed at too.
fn draw_sheet(frame: &mut Frame, interior: &Interior, f: &Fitted, block: Option<&Block>) {
    let reg = universe_sim::world::registry::registry();
    let e = reg.equipment.iter().find(|e| e.identity.key == f.key);
    let (pp, pc) = PANEL;
    let (p, c) = (Vec2::new(pp.x + pc.x + 8.0, pp.y), Vec2::new(236.0, 300.0));
    frame.hud_rect(p, c, Color([0.02, 0.06, 0.13, 0.92]));
    frame.hud_box(p, c, MODULE.scale(0.7));
    frame.text_scaled(p + Vec2::new(8.0, 6.0), &f.name, MODULE, 0.8);
    let name = |key: &str| reg.names.get(key).map_or(key.to_string(), |n| n.to_uppercase());
    let line = match e {
        Some(e) => {
            let fits = reg.equipment.iter().find(|x| x.identity.key == f.key).and_then(|x| x.fits.clone()).map_or(String::new(), |m| format!(" - FITS {}", name(&m)));
            format!("{}{}", name(&e.identity.maker), fits)
        }
        None => "THE HULL'S OWN".to_string(),
    };
    frame.text_scaled(p + Vec2::new(8.0, 19.0), &line.chars().take(44).collect::<String>(), LABEL.scale(0.7), 0.55);
    // Its shape, turning (a box, or a tank's ball or egg), in a window of its own.
    let (wp, wc) = (p + Vec2::new(8.0, 32.0), Vec2::new(c.x - 16.0, 84.0));
    frame.hud_box(wp, wc, LABEL.scale(0.25));
    let size = block.map_or(f.size, |b| b.size);
    let shape = Block { id: String::new(), at: Vec3::ZERO, size };
    let (yaw, tilt) = (interior.spin * 0.5, 0.45f32);
    let turn = |q: Vec3| {
        let (sy, cy) = yaw.sin_cos();
        let r = Vec3::new(q.x * cy + q.z * sy, q.y, -q.x * sy + q.z * cy);
        let (st, ct) = tilt.sin_cos();
        Vec2::new(r.x, -(r.y * ct - r.z * st))
    };
    let k = (wc.y * 0.85) / size.length().max(1e-3);
    let mid = wp + wc * 0.5;
    for [a, b] in shape.edges(f.round) {
        frame.hud_line(mid + turn(a) * k, mid + turn(b) * k, MODULE);
    }
    // Its size, mass, volume, what it draws.
    let mut y = p.y + 122.0;
    let dims = format!("L {:.2} X W {:.2} X H {:.2} M", f.size.z, f.size.x, f.size.y);
    frame.text_scaled(Vec2::new(p.x + 8.0, y), &dims, LABEL, 0.6);
    y += 11.0;
    if let Some(b) = block.filter(|b| (b.size - f.size).length() > 0.01) {
        frame.text_scaled(Vec2::new(p.x + 8.0, y), &format!("PLACED AS {:.2} X {:.2} X {:.2} M", b.size.z, b.size.x, b.size.y), PICKED.scale(0.9), 0.6);
        y += 11.0;
    }
    let draws = e.and_then(|e| e.needs.power).filter(|w| *w > 0.0).map_or(String::new(), |w| format!("   DRAWS {}", si(w, "W")));
    frame.text_scaled(Vec2::new(p.x + 8.0, y), &format!("MASS {}   VOLUME {:.1} M3{draws}", si(f.mass, "KG"), f.volume), LABEL, 0.6);
    y += 15.0;
    // Its record's figures (what it does), each with its unit.
    if let Some(serde_json::Value::Object(fields)) = e.and_then(|e| serde_json::to_value(&e.function).ok()) {
        let kind = fields.get("kind").and_then(|v| v.as_str()).unwrap_or("").replace('_', " ").to_uppercase();
        frame.text_scaled(Vec2::new(p.x + 8.0, y), &kind, MODULE.scale(0.9), 0.6);
        y += 11.0;
        let unit = |field: &str| match field {
            "thrust" | "torque" if field == "thrust" => "N",
            "torque" => "N M",
            "momentum" => "N M S",
            "holds" if fields.get("stroke").is_some() || kind == "DOCKING" => "N",
            "output" | "power" | "beam_power" | "rejects" | "transfers" | "carries" | "rate" if field != "rate" || kind == "BATTERY" => "W",
            "stores" => "J",
            "exhaust" | "sink_rate" | "muzzle_speed" => "M/S",
            "stroke" | "extended" | "range" | "focus" | "resolves" | "survey_range" | "anchor_reach" | "capture" | "link" | "passage" | "reach" | "travel" | "width" | "height" => "M",
            "area" => "M2",
            "temperature" => "K",
            "pressure" | "head" => "PA",
            "persons" => "PEOPLE",
            "cycle" | "opens_in" => "S",
            "air_lost" | "load" => "KG",
            "flow" if kind == "COMPRESSOR" => "M3/S",
            "flow" => "KG/S",
            "volume" => "M3",
            "fill_density" => "KG/M3",
            "slug_mass" => "KG",
            "capacity" if kind == "CAPACITOR" => "J",
            "capacity" if kind == "COMM" || kind == "HYPER RELAY" || kind == "GATE RELAY" => "/S",
            "capacity" => "KG",
            "burn" | "cool" | "lag" | "cadence" => "S",
            "rate" => "/S",
            "throughput" => "KG/S",
            "excavator_power" => "W",
            "anchor_speed" => "M/S",
            _ => "",
        };
        for (field, v) in fields.iter().filter(|(k, _)| *k != "kind").take(7) {
            let shown = match v {
                serde_json::Value::Number(n) => {
                    let x = n.as_f64().unwrap_or(0.0);
                    if field == "efficiency" { format!("{:.0}%", x * 100.0) } else { si(x, unit(field)) }
                }
                serde_json::Value::String(t) => name(t),
                serde_json::Value::Bool(b) => if *b { "YES".into() } else { "NO".into() },
                serde_json::Value::Array(a) => format!("{} OF THEM", a.len()),
                _ => continue,
            };
            let label = field.replace('_', " ").to_uppercase();
            frame.text_scaled(Vec2::new(p.x + 12.0, y), &format!("{label}: {shown}").chars().take(42).collect::<String>(), LABEL.scale(0.85), 0.55);
            y += 10.0;
        }
    }
    // Its description.
    if let Some(d) = e.and_then(|e| e.identity.description.as_deref()) {
        y += 4.0;
        for l in crate::fmt::wrap(&d.to_uppercase(), 42).into_iter().take(((p.y + c.y - 6.0 - y) / 10.0).max(0.0) as usize) {
            frame.text_scaled(Vec2::new(p.x + 8.0, y), &l, LABEL.scale(0.7), 0.55);
            y += 10.0;
        }
    }
}

/// The FRAME panel's stock list: row `k`'s place.
fn stock_row(k: usize) -> (Vec2, Vec2) {
    let (p, c) = PANEL;
    (Vec2::new(p.x + 6.0, p.y + 82.0 + k as f32 * 10.5), Vec2::new(c.x - 12.0, 10.5))
}

/// The FRAME panel's stock list: how many rows show (the rest scrolled to with the
/// wheel).
const STOCK_ROWS: usize = 5;

/// Where a frame's members can start or end besides its own joints: the landing
/// pads, the main and lift nozzles, each placed module's corners and middle (and
/// its faces' middles).
fn bearers(i: &Interior) -> Vec<Vec3> {
    use universe_sim::world::shape::Role;
    use universe_sim::world::ship::ThrusterRole;
    let mut out: Vec<Vec3> = i.plan.pads.clone();
    if let Some(spec) = i.spec() {
        out.extend(spec.shape().nodes(Role::Gear).map(|n| n.at.as_vec3()));
        out.extend(spec.thrusters.iter().filter(|t| matches!(t.role, ThrusterRole::Main | ThrusterRole::Lift)).map(|t| t.at.as_vec3()));
    }
    for b in &i.plan.blocks {
        let h = b.size * 0.5;
        for x in [-1.0f32, 0.0, 1.0] {
            for y in [-1.0f32, 0.0, 1.0] {
                for z in [-1.0f32, 0.0, 1.0] {
                    // (Corners, the middle and the faces' middles: not the edges'.)
                    let zeros = [x, y, z].iter().filter(|v| **v == 0.0).count();
                    if zeros != 1 {
                        out.push(b.at + Vec3::new(x, y, z) * h);
                    }
                }
            }
        }
    }
    out
}

/// What a click at `q` would join a member to: a joint of the frame, or a bearer,
/// within 10 px of it on screen.
fn frame_snap(i: &Interior, cam: &Camera, q: Vec2) -> Option<Vec3> {
    let mut at: Vec<Vec3> = i.plan.beams.iter().flat_map(|b| [b.a, b.b]).collect();
    at.extend(bearers(i));
    at.into_iter().filter_map(|p| cam.project(p).map(|(s, _)| (p, s.distance(q)))).filter(|(_, d)| *d < 10.0).min_by(|a, b| a.1.total_cmp(&b.1)).map(|(p, _)| p)
}

/// The member under the cursor (within 6 px of its line on screen), if any.
fn beam_at(i: &Interior, cam: &Camera, q: Vec2) -> Option<usize> {
    i.plan.beams.iter().enumerate().filter_map(|(k, b)| {
        let (sa, sb) = (cam.project(b.a)?.0, cam.project(b.b)?.0);
        let ab = sb - sa;
        let t = ((q - sa).dot(ab) / ab.length_squared().max(1e-6)).clamp(0.0, 1.0);
        Some((k, (sa + ab * t).distance(q)))
    }).filter(|(_, d)| *d < 6.0).min_by(|a, b| a.1.total_cmp(&b.1)).map(|(k, _)| k)
}

/// The MODULES panel's list: how many rows show (the rest scrolled to with the wheel).
const FIT_ROWS: usize = 19;

/// The MODULES panel's list: row `k`'s place (the fit, one a row).
fn module_row(k: usize) -> (Vec2, Vec2) {
    let (p, c) = PANEL;
    (Vec2::new(p.x + 6.0, p.y + 40.0 + k as f32 * 10.5), Vec2::new(c.x - 12.0, 10.5))
}

/// The panel's buttons for the tool in hand (where, its label, what it does).
fn panel_buttons(tool: Tool) -> Vec<((Vec2, Vec2), &'static str, Action)> {
    let (p, c) = PANEL;
    let at = |row: f32, col: f32, w: f32| (Vec2::new(p.x + 8.0 + col, p.y + row), Vec2::new(w, 16.0));
    let w = (c.x - 16.0 - 6.0) / 2.0;
    // (Each as wide as its name, the row's spare width shared out.)
    let letters: usize = Section::ALL.iter().map(|s| s.name().len()).sum();
    let gap = 4.0;
    let spare = (c.x - 16.0 - gap * (Section::ALL.len() - 1) as f32 - letters as f32 * 8.0) / Section::ALL.len() as f32;
    let mut x = 0.0;
    let shapes: Vec<_> = Section::ALL.iter().map(|s| {
        let w = s.name().len() as f32 * 8.0 + spare;
        let b = (at(174.0, x, w), s.name(), Action::Section(*s));
        x += w + gap;
        b
    }).collect();
    let shapes = shapes.into_iter();
    match tool {
        Tool::Frame => {
            let w3 = (c.x - 16.0 - 12.0) / 3.0;
            let w4 = (c.x - 16.0 - 18.0) / 4.0;
            let cases = [("WORST", Action::Case(None)), ("LANDING", Action::Case(Some(0))), ("THRUST", Action::Case(Some(1))), ("LIFT", Action::Case(Some(2)))];
            cases.into_iter().enumerate().map(|(k, (n, a))| (at(156.0, k as f32 * (w4 + 6.0), w4), n, a)).chain([(at(310.0, 0.0, w3), "TRUSS", Action::Truss), (at(310.0, w3 + 6.0, w3), "BRACE", Action::Brace), (at(310.0, 2.0 * (w3 + 6.0), w3), "REMOVE", Action::Remove), (at(332.0, 0.0, w3), "MOUNT ALL", Action::MountAll), (at(332.0, w3 + 6.0, w3), "AUTO-SIZE", Action::AutoSize), (at(332.0, 2.0 * (w3 + 6.0), w3), "MIX", Action::Mix)]).collect()
        }
        Tool::Modules => {
            let w3 = (c.x - 16.0 - 12.0) / 3.0;
            vec![(at(310.0, 0.0, w), "TURN", Action::Turn), (at(310.0, w + 6.0, w), "REMOVE", Action::Remove), (at(332.0, 0.0, w3), "PLANE -", Action::PlaneDown), (at(332.0, w3 + 6.0, w3), "PLANE +", Action::PlaneUp), (at(332.0, 2.0 * (w3 + 6.0), w3), "SNAP FLOOR", Action::SnapFloor)]
        }
        Tool::Door => {
            // (A hatch's shape: not NONE; how it slides.)
            let w3 = (c.x - 16.0 - 12.0) / 3.0;
            shapes.filter(|b| b.2 != Action::Section(Section::Line)).chain([(at(246.0, 0.0, w3), "SLIDE UP", Action::Slide(Slide::Up)), (at(246.0, w3 + 6.0, w3), "SLIDE SIDE", Action::Slide(Slide::Side)), (at(246.0, 2.0 * (w3 + 6.0), w3), "NO LEAF", Action::Slide(Slide::None))]).collect()
        }
        Tool::Path => {
            // (Presets in two rows: two, then three.)
            let w3 = (c.x - 16.0 - 12.0) / 3.0;
            let presets = PRESETS.iter().enumerate().map(|(k, (name, _, _))| {
                if k < 2 { (at(308.0, k as f32 * (w + 6.0), w), *name, Action::Preset(k)) } else { (at(330.0, (k - 2) as f32 * (w3 + 6.0), w3), *name, Action::Preset(k)) }
            });
            shapes.chain([(at(246.0, 0.0, w3), "PLANE -", Action::PlaneDown), (at(246.0, w3 + 6.0, w3), "PLANE +", Action::PlaneUp), (at(246.0, 2.0 * (w3 + 6.0), w3), "SNAP FLOOR", Action::SnapFloor), (at(270.0, 0.0, w), "REMOVE PICKED", Action::Remove), (at(270.0, w + 6.0, w), "ON LINE", Action::Stand)]).chain(presets).collect()
        }
        Tool::Look => {
            let w3 = (c.x - 16.0 - 12.0) / 3.0;
            shapes.chain([(at(246.0, 0.0, w3), "GROUP", Action::Group), (at(246.0, w3 + 6.0, w3), "UNGROUP", Action::Ungroup), (at(246.0, 2.0 * (w3 + 6.0), w3), "WALL OFF", Action::Wall), (at(270.0, 0.0, w), "REMOVE PICKED", Action::Remove), (at(270.0, w + 6.0, w), "ON LINE", Action::Stand)]).collect()
        }
    }
}

fn panel_button_at(tool: Tool, q: Vec2) -> Option<Action> {
    panel_buttons(tool).into_iter().find(|(r, _, _)| inside(*r, q)).map(|(_, _, a)| a)
}

/// The work plane's handle: on its edge nearest the camera, the corner that edge
/// runs to on the right (its frame); failing that one on screen, the next edge's.
fn plane_handle(cam: &Camera, h: &Hull, plane: f32) -> Vec3 {
    let (lo, hi) = (h.lo, h.hi);
    let c = [Vec3::new(lo.x, plane, lo.z), Vec3::new(hi.x, plane, lo.z), Vec3::new(hi.x, plane, hi.z), Vec3::new(lo.x, plane, hi.z)];
    let mut edges: Vec<(Vec3, Vec3)> = (0..4).map(|k| (c[k], c[(k + 1) % 4])).collect();
    edges.sort_by(|a, b| ((a.0 + a.1) * 0.5).distance(cam.eye).total_cmp(&((b.0 + b.1) * 0.5).distance(cam.eye)));
    let size = cam.centre * 2.0;
    // (On screen, and not under the panel or the legend.)
    let on = |q: Vec2| q.x > 20.0 && q.y > 60.0 && q.x < size.x - 20.0 && q.y < size.y - 30.0 && !inside(legend_rect(size), q) && !inside(PANEL, q);
    let right = |(a, b): (Vec3, Vec3)| match (cam.project(a), cam.project(b)) {
        (Some((qa, _)), Some((qb, _))) => Some(if qa.x >= qb.x { (a, qa) } else { (b, qb) }),
        (Some((qa, _)), None) => Some((a, qa)),
        (None, Some((qb, _))) => Some((b, qb)),
        _ => None,
    };
    edges.iter().filter_map(|e| right(*e)).find(|(_, q)| on(*q)).map_or(c[0], |(p, _)| p)
}

/// The legend, at the bottom right: where it is and its size.
fn legend_rect(size: Vec2) -> (Vec2, Vec2) {
    layers_rect(layers_corner(size), LAYERS.len())
}

/// The studios' work done aside: on a thread of its own, named (so it can be
/// told apart from the world's, in the observer and in `top -H`).
pub fn job(name: &str, work: impl FnOnce() + Send + 'static) {
    std::thread::Builder::new().name(name.into()).spawn(work).ok();
}

/// Where the layers panel's bottom right corner is in this studio.
fn layers_corner(size: Vec2) -> Vec2 {
    Vec2::new(size.x - 12.0, size.y - 26.0)
}

/// The layers panel with `rows` rows under its header, its bottom right at
/// `corner`: where it is and its size.
fn layers_rect(corner: Vec2, rows: usize) -> (Vec2, Vec2) {
    let height = LAYER_ROW * (rows + 1) as f32 + 8.0;
    (Vec2::new(corner.x - LAYERS_WIDTH, corner.y - height), Vec2::new(LAYERS_WIDTH, height))
}

/// The layers panel: its width and a row's height (px).
const LAYERS_WIDTH: f32 = 140.0;
const LAYER_ROW: f32 = 11.0;

/// The layers panel's rows shown: none folded up (the panel), a group's folded
/// (its own row only).
fn layer_rows(i: &Interior) -> Vec<usize> {
    if i.layers_folded {
        return Vec::new();
    }
    (0..LAYERS.len()).filter(|&k| LAYERS[k].1.is_none_or(|p| !i.group_folded[p])).collect()
}

/// The layers panel's header (a click folds it up, or out), and the place of the
/// `n`th of `rows` rows under it.
fn layers_header(corner: Vec2, rows: usize) -> (Vec2, Vec2) {
    let (p, c) = layers_rect(corner, rows);
    (Vec2::new(p.x + 2.0, p.y + 2.0), Vec2::new(c.x - 4.0, LAYER_ROW))
}

fn layer_row(corner: Vec2, rows: usize, n: usize) -> (Vec2, Vec2) {
    let (p, c) = layers_rect(corner, rows);
    (Vec2::new(p.x + 4.0, p.y + 4.0 + LAYER_ROW * (n + 1) as f32), Vec2::new(c.x - 8.0, LAYER_ROW))
}

/// The layers panel drawn, its bottom right at `corner` (`cursor` lights what's
/// under it).
pub fn draw_layers(frame: &mut Frame, interior: &Interior, corner: Vec2, cursor: Vec2) {
    // The layers: a header (folds the panel), then a row each: a group's arrow (folds
    // its rows), a checkbox (ticked: shown), its key's colour, its name (under
    // another, set in; not shown, dim).
    {
        let rows = layer_rows(interior);
        let (p, c) = layers_rect(corner, rows.len());
        frame.hud_rect(p, c, LAYERS_BACK);
        frame.hud_box(p, c, LAYERS_EDGE);
        let (hp, hc) = layers_header(corner, rows.len());
        let fold = if interior.layers_folded { "+" } else { "-" };
        let head = if inside((hp, hc), cursor) { PICKED } else { LAYERS_HEAD };
        frame.text_scaled(hp + Vec2::new(3.0, 1.5), &format!("{fold} LAYERS"), head, 0.62);
        for (n, &k) in rows.iter().enumerate() {
            let (name, parent, colour) = LAYERS[k];
            let (q, qc) = layer_row(corner, rows.len(), n);
            let group = (k + 1 < LAYERS.len()) && LAYERS[k + 1].1 == Some(k);
            let on = interior.shown(k);
            let x = q.x + if parent.is_some() { 10.0 } else { 0.0 };
            if group {
                let arrow = if interior.group_folded[k] { ">" } else { "v" };
                frame.text_scaled(Vec2::new(q.x - 1.0, q.y + 1.5), arrow, LAYERS_HEAD.scale(0.8), 0.55);
            }
            // (The checkbox: a box, ticked when it's on.)
            let b = Vec2::new(x + 8.0, q.y + 2.0);
            let box_col = if on { LAYERS_TICK } else { LAYERS_TEXT.scale(0.45) };
            frame.hud_box(b, Vec2::splat(7.0), box_col);
            if !interior.hidden[k] {
                frame.hud_line(b + Vec2::new(1.5, 3.5), b + Vec2::new(3.0, 5.5), box_col);
                frame.hud_line(b + Vec2::new(3.0, 5.5), b + Vec2::new(6.0, 1.0), box_col);
            }
            if let Some(col) = colour {
                frame.hud_rect(Vec2::new(b.x + 10.0, q.y + 4.5), Vec2::new(6.0, 2.0), if on { col } else { col.scale(0.35) });
            }
            let text = if inside((q, qc), cursor) { PICKED } else if on { LAYERS_TEXT } else { LAYERS_TEXT.scale(0.4) };
            frame.text_scaled(Vec2::new(b.x + 19.0, q.y + 1.5), name, text, 0.58);
        }
    }
}

/// A click at `cursor` on the layers panel (its bottom right at `corner`): the
/// header folds it, a group's arrow folds its rows, a row shows or hides its layer.
/// True if it was on the panel.
pub fn layers_click(interior: &mut Interior, corner: Vec2, cursor: Vec2) -> bool {
    let rows = layer_rows(interior);
    if inside(layers_header(corner, rows.len()), cursor) {
        interior.layers_folded = !interior.layers_folded;
        return true;
    }
    if let Some(n) = (0..rows.len()).find(|&n| inside(layer_row(corner, rows.len(), n), cursor)) {
        let k = rows[n];
        let group = (k + 1 < LAYERS.len()) && LAYERS[k + 1].1 == Some(k);
        if group && cursor.x < layer_row(corner, rows.len(), n).0.x + 10.0 {
            interior.group_folded[k] = !interior.group_folded[k];
        } else {
            interior.hidden[k] = !interior.hidden[k];
        }
        return true;
    }
    inside(layers_rect(corner, rows.len()), cursor)
}

/// The layers panel's own colours: slate, a muted amber edge and heading, soft ticks
/// (apart from the blueprint, quiet).
const LAYERS_BACK: Color = Color([0.06, 0.07, 0.09, 0.62]);
const LAYERS_EDGE: Color = Color([0.85, 0.7, 0.45, 0.35]);
const LAYERS_HEAD: Color = Color([0.9, 0.78, 0.55, 0.85]);
const LAYERS_TEXT: Color = Color([0.82, 0.84, 0.86, 0.85]);
const LAYERS_TICK: Color = Color([0.65, 0.9, 0.75, 0.9]);

/// The globe at the top right: its middle and radius (px).
fn globe(size: Vec2) -> (Vec2, f32) {
    (Vec2::new(size.x - 64.0, 100.0), 38.0)
}

/// The globe's axis ends as seen: each axis's direction both ways, where its end
/// is on screen, and how near the camera it points (to draw the far ones first).
fn globe_ends(cam: &Camera, size: Vec2) -> Vec<(Vec3, Vec2, f32)> {
    let (c, r) = globe(size);
    [Vec3::X, Vec3::Y, Vec3::Z, -Vec3::X, -Vec3::Y, -Vec3::Z]
        .into_iter()
        .map(|a| (a, c + Vec2::new(a.dot(cam.right), -a.dot(cam.up)) * r * 0.8, -a.dot(cam.forward)))
        .collect()
}

/// The lines picked: the one picked and those with it.
fn picked_lines(i: &Interior) -> Vec<usize> {
    let mut lines: Vec<usize> = i.more.clone();
    if let Some(Hover::Line(k)) = i.pick
        && !lines.contains(&k)
    {
        lines.insert(0, k);
    }
    lines.retain(|&k| k < i.plan.lines.len());
    lines
}

/// The selected point's level: the one picked, or the one a path runs on from.
fn selected_level(i: &Interior) -> Option<f32> {
    let k = match (i.pick, i.from) {
        (Some(Hover::Point(k)), _) => k,
        (_, Some(k)) => k,
        _ => return None,
    };
    i.plan.points.get(k).map(|p| p.at.y)
}

/// The work plane's height (its frame): as set, or the hatch's (where the crew come in).
fn plane_of(i: &Interior, h: &Hull) -> f32 {
    i.plane.unwrap_or_else(|| i.plan.points.iter().find(|p| p.name.as_deref() == Some("HATCH")).map_or((h.lo.y + h.hi.y) * 0.5, |p| p.at.y))
}

/// Where on line `k` the cursor is: its quarter, middle or three-quarter mark if
/// within 10 px of one (which), else straight across from the cursor.
fn on_line(i: &Interior, cam: &Camera, q: Vec2, k: usize) -> Option<(Vec3, Option<usize>)> {
    let (a, b, _) = i.plan.lines[k];
    let (pa, pb) = (i.plan.points[a].at, i.plan.points[b].at);
    let (sa, sb) = (cam.project(pa)?.0, cam.project(pb)?.0);
    let marks = [0.25f32, 0.5, 0.75];
    let near = marks.iter().enumerate().map(|(m, t)| (m, sa.lerp(sb, *t).distance(q))).filter(|(_, d)| *d < 10.0).min_by(|x, y| x.1.total_cmp(&y.1));
    Some(match near {
        Some((m, _)) => (pa.lerp(pb, marks[m]), Some(m)),
        None => {
            let ab = sb - sa;
            let t = ((q - sa).dot(ab) / ab.length_squared().max(1e-6)).clamp(0.05, 0.95);
            (pa.lerp(pb, t), None)
        }
    })
}

/// Where on the work plane a click at `q` would lay a point: snapped to 25 cm;
/// with SHIFT, from the path's last point straight along the plane's x or z,
/// whichever is nearer.
fn on_plane(i: &Interior, cam: &Camera, plane: f32, q: Vec2, shift: bool) -> Option<Vec3> {
    let ray = cam.ray(q);
    let t = if ray.y.abs() > 1e-4 { (plane - cam.eye.y) / ray.y } else { -1.0 };
    if t <= 0.0 {
        return None;
    }
    let at = cam.eye + ray * t;
    let mut at = Vec3::new((at.x * 4.0).round() / 4.0, plane, (at.z * 4.0).round() / 4.0);
    if shift && let Some(a) = i.from {
        let from = i.plan.points[a].at;
        if (at.x - from.x).abs() > (at.z - from.z).abs() {
            at.z = from.z;
        } else {
            at.x = from.x;
        }
    }
    // SNAP FLOOR: down to the hull's floor under it (the first surface below, if it
    // faces up; within 8 m).
    if i.snap_floor
        && let Some((_, h)) = &i.lines
    {
        let from = universe_engine::glam::DVec3::new(f64::from(at.x), f64::from(plane) + 0.05, f64::from(at.z));
        if let Some((d, n)) = h.mesh.ray(from, universe_engine::glam::DVec3::NEG_Y, 8.0)
            && n.y > 0.6
        {
            at.y = (from.y - d) as f32;
        }
    }
    Some(at)
}

/// A run's length (m) and its slope from level (degrees).
fn slope_of(a: Vec3, b: Vec3) -> (f32, f32) {
    let d = b - a;
    (d.length(), d.y.abs().atan2(Vec2::new(d.x, d.z).length()).to_degrees())
}

/// What's under the cursor: a point (within 8 px), or else a line (within 5 px).
fn hover_at(i: &Interior, cam: &Camera, q: Vec2) -> Option<Hover> {
    // (What's hidden isn't under the cursor.)
    let screen: Vec<Option<Vec2>> = i.plan.points.iter().map(|p| if i.shown(3 + sort_index(p.name.as_deref())) { cam.project(p.at).map(|s| s.0) } else { None }).collect();
    let near = screen.iter().enumerate().filter_map(|(k, s)| s.map(|s| (k, s.distance(q)))).filter(|(_, d)| *d < 8.0).min_by(|a, b| a.1.total_cmp(&b.1));
    if let Some((k, _)) = near {
        return Some(Hover::Point(k));
    }
    if !(i.shown(layer::LINES) || i.shown(layer::ROOMS) || i.shown(layer::WALLS)) {
        return None;
    }
    i.plan.lines.iter().enumerate().filter_map(|(k, &(a, b, _))| {
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
    // (The hull's lines picked up, its plan seeded, whatever else is going on.)
    let spec = app.ship.spec();
    interior.spin += ctx.dt;
    interior.sync(&spec.key, spec.shape(), &mut app.deckplans, 0.0);
    interior.refresh();
    // Closing with unsaved changes: SAVE AND CLOSE (S, ENTER), DISCARD (D), or keep
    // working (ESC); nothing else meanwhile.
    if interior.confirm {
        let size = ctx.hud_size.as_vec2();
        let (_, buttons) = confirm_box(size);
        let click = |k: usize| input.button_pressed(MouseButton::Left) && inside(buttons[k], input.cursor);
        if input.pressed(KeyCode::KeyS) || input.pressed(KeyCode::Enter) || click(0) {
            interior.save();
            interior.confirm = false;
            return !interior.saved.as_ref().is_some_and(|s| *s == interior.plan);
        }
        if input.pressed(KeyCode::KeyD) || click(1) {
            interior.confirm = false;
            return false;
        }
        if input.pressed(KeyCode::Escape) || click(2) {
            interior.confirm = false;
        }
        return true;
    }
    // A dialog open: a row chosen, or ESC (or CANCEL) to put it away; nothing else.
    if let Some(d) = interior.dialog.clone() {
        let size = ctx.hud_size.as_vec2();
        let choices = dialog_choices(&d);
        let (_, rows) = dialog_rows(size, choices.len());
        let chosen = if input.button_pressed(MouseButton::Left) { rows.iter().position(|r| inside(*r, input.cursor)) } else { None };
        if input.pressed(KeyCode::Escape) || chosen == Some(choices.len() - 1) {
            interior.dialog = None;
        } else if let Some(k) = chosen {
            interior.dialog = None;
            match d {
                Dialog::New => interior.new_design(hulls().get(k).copied()),
                Dialog::Open(list) => interior.open_design(&list[k].0),
            }
        }
        interior.cursor = input.cursor;
        return true;
    }
    // ESC: what's in progress cancelled (a module picked, a path, member or truss
    // being laid, TRUSS waiting, WALK HERE armed); nothing in progress, the studio
    // closed.
    if input.pressed(KeyCode::Escape) {
        if interior.cancel() {
            return true;
        }
        return !interior.close();
    }
    // SAVE (CTRL+S) and OPEN (CTRL+O).
    let clicked = |b: (Vec2, Vec2)| input.button_pressed(MouseButton::Left) && inside(b, input.cursor);
    if (ctrl && input.pressed(KeyCode::KeyS)) || clicked(file_button(0)) {
        interior.save();
        return true;
    }
    // OPEN (a saved design) and NEW (in a hull, or none): with changes unsaved, not
    // till they're saved (or undone).
    let open = (ctrl && input.pressed(KeyCode::KeyO)) || clicked(file_button(1));
    let new = (ctrl && input.pressed(KeyCode::KeyN)) || clicked(file_button(2));
    if (open || new) && interior.unsaved() {
        interior.message = Some(("UNSAVED CHANGES: SAVE THEM (CTRL+S) OR UNDO THEM FIRST".into(), 4.0));
        return true;
    }
    if open {
        interior.dialog = Some(Dialog::Open(Interior::designs()));
        return true;
    }
    if new {
        interior.dialog = Some(Dialog::New);
        return true;
    }
    // REACH: the check run (on a thread); its findings picked up when done.
    if let Some((plan, rx)) = &interior.reach_job
        && let Ok(found) = rx.try_recv()
    {
        interior.reach = Some((plan.clone(), Arc::new(found)));
        interior.reach_job = None;
    }
    if clicked(reach_button()) && interior.reach_job.is_none() {
        if let Some((_, h)) = &interior.lines {
            let (mesh, walls, decks, plan) = (h.mesh.clone(), interior.walls(), interior.decks_now.clone(), interior.plan.clone());
            let (tx, rx) = mpsc::channel();
            let snapshot = plan.clone();
            job("studio-reach", move || {
                tx.send(reach(&mesh, &walls, decks.as_ref(), &plan)).ok();
            });
            interior.reach_job = Some((snapshot, rx));
        }
        return true;
    }
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
    let stay = input_plan(ctx, interior);
    // (A slide is one step, however many frames it changes the plan.)
    let changed = interior.plan != before && interior.plan.hull == before.hull;
    if changed && !interior.sliding {
        interior.undo.push(before);
        interior.redo.clear();
    }
    interior.sliding = interior.slider.is_some() && (changed || interior.sliding);
    // Its walls worked out again when it's changed.
    if interior.panelled.as_ref().is_none_or(|(p, _)| *p != interior.plan) {
        interior.panelled = Some((interior.plan.clone(), interior.plan.wall_panels()));
    }
    // The plan checked against the hull, again when it's changed.
    if interior.checked.as_ref().is_none_or(|(p, _)| *p != interior.plan)
    {
        // (No hull: nothing to clash with.)
        let found = match interior.spec().and_then(|s| s.shape().walk.as_ref()) {
            Some(mesh) => clashes(mesh, &interior.plan),
            None => vec![Vec::new(); interior.plan.lines.len()],
        };
        interior.checked = Some((interior.plan.clone(), found));
    }
    // AUTO-SIZE done: its members put in (an undo step), if the plan's as it was.
    if let Some((plan, rx)) = &interior.sizing
        && let Ok((beams, rounds, maxed)) = rx.try_recv()
    {
        if *plan == interior.plan {
            let before = interior.plan.clone();
            let mass = |bs: &[Beam]| bs.iter().filter_map(|b| stocks().iter().find(|s| s.key == b.stock).map(|s| s.per_metre * f64::from(b.a.distance(b.b)))).sum::<f64>();
            let (was, now) = (mass(&before.beams), mass(&beams));
            interior.plan.beams = beams;
            interior.undo.push(before);
            interior.redo.clear();
            let short = if maxed > 0 { format!("; {maxed} NEED MORE THAN THEIR MATERIAL'S BIGGEST TUBE: TRY ANOTHER") } else { String::new() };
            interior.message = Some((format!("SIZED IN {rounds} ROUNDS: FRAME {:.1} T (WAS {:.1} T){short}", now / 1000.0, was / 1000.0), 8.0));
        } else {
            interior.message = Some(("AUTO-SIZE DROPPED: THE PLAN CHANGED MEANWHILE".into(), 4.0));
        }
        interior.sizing = None;
    }
    // What the frame bears, worked out again (on a thread) when the plan's changed.
    if let Some((plan, rx)) = &interior.bearing_job
        && let Ok(found) = rx.try_recv()
    {
        interior.bearing = Some((plan.clone(), Arc::new(found)));
        interior.bearing_job = None;
    }
    let stale = interior.bearing.as_ref().is_none_or(|(p, _)| *p != interior.plan);
    if stale && interior.bearing_job.as_ref().is_none_or(|(p, _)| *p != interior.plan) && !interior.plan.beams.is_empty() {
        let (plan, fit, spec) = (interior.plan.clone(), interior.fit.clone(), interior.spec());
        let (tx, rx) = mpsc::channel();
        let snapshot = plan.clone();
        job("studio-frame", move || {
            tx.send(bearing(&plan, &fit, spec, STANDARD_G)).ok();
        });
        interior.bearing_job = Some((snapshot, rx));
    }
    // The modules checked too: none in the hull's own material, out of it, or in a
    // tube's room.
    if interior.block_clash.as_ref().is_none_or(|(b, _)| *b != interior.plan.blocks)
    {
        let found = block_clashes(interior.spec().and_then(|s| s.shape().walk.as_deref()), interior);
        interior.block_clash = Some((interior.plan.blocks.clone(), found));
    }
    stay
}

fn input_plan(ctx: &Context, interior: &mut Interior) -> bool {
    let input = &ctx.input;
    let Some(h) = interior.hull() else { return true };
    let size = ctx.hud_size.as_vec2();
    let cam = Camera::of(interior, &h, size);
    let cursor = input.cursor;
    interior.cursor = cursor;
    interior.shift = input.down(KeyCode::ShiftLeft) || input.down(KeyCode::ShiftRight);
    let d = input.mouse_delta;
    let pressed = input.button_pressed(MouseButton::Left);
    // A layer's row clicked: shown or hidden.
    if pressed && layers_click(interior, layers_corner(size), cursor) {
        return true;
    }
    // The globe: an axis clicked undoes the view's turn about that axis only. X:
    // levelled (no tilt up or down), its turn round the ship kept. Y: its turn round
    // the ship squared to the nearest quarter, its tilt kept. (Z would be a roll,
    // which this view never has.)
    if pressed
        && let Some((a, _, _)) = globe_ends(&cam, size).into_iter().find(|(_, q, _)| q.distance(cursor) < 9.0)
    {
        if a.x.abs() > 0.5 {
            interior.pitch = 0.0;
        } else if a.y.abs() > 0.5 {
            let quarter = std::f32::consts::FRAC_PI_2;
            interior.yaw = (interior.yaw / quarter).round() * quarter;
        }
        return true;
    }
    // The tools (toolbar or key); the panel's actions.
    let tool = if pressed { button_at(cursor) } else { None };
    for (key, t) in [(KeyCode::KeyV, Tool::Look), (KeyCode::KeyP, Tool::Path), (KeyCode::KeyD, Tool::Door), (KeyCode::KeyM, Tool::Modules), (KeyCode::KeyR, Tool::Frame)] {
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
    if action == Some(Action::SnapFloor) {
        interior.snap_floor = !interior.snap_floor;
    }
    // A cross-section, its shape (a button) or its width or height (a slider, held
    // and slid): for new lines; picked in LOOK, that line's.
    if pressed {
        interior.slider = sliders(interior.tool).iter().position(|t| inside((t.0 - Vec2::new(0.0, 4.0), t.1 + Vec2::new(0.0, 8.0)), cursor));
    }
    if !input.button_down(MouseButton::Left) {
        interior.slider = None;
    }
    // (In LOOK, every line picked; laying paths, the new lines'.)
    let picked = picked_lines(interior);
    let set = |p: &mut Profile| {
        if let Some(Action::Preset(k)) = action {
            *p = PRESETS[k].1;
        }
        if action == Some(Action::Stand) {
            p.stand = !p.stand;
        }
        if let Some(Action::Section(s)) = action {
            p.section = s;
        }
        if let Some(k) = interior.slider {
            let (at, len) = sliders(Tool::Path)[k];
            let v = ROOM_MIN + ((cursor.x - at.x) / len.x).clamp(0.0, 1.0) * (ROOM_MAX - ROOM_MIN);
            *(if k == 0 { &mut p.width } else { &mut p.height }) = (v * 10.0).round() / 10.0;
        }
    };
    match interior.tool {
        Tool::Look => {
            for &k in &picked {
                set(&mut interior.plan.lines[k].2);
            }
        }
        Tool::Path => set(&mut interior.profile),
        // (The hatch new doorways get: its shape and size as a cross-section's.)
        Tool::Door => {
            let h = &mut interior.hatch;
            let mut p = Profile { section: h.section, width: h.width, height: h.height, stand: false };
            set(&mut p);
            if p.section != Section::Line {
                h.section = p.section;
            }
            (h.width, h.height) = (p.width, p.height);
            if let Some(Action::Slide(slide)) = action {
                h.slide = slide;
            }
        }
        // (FRAME: the stock new members are cut from, a row of its list (the picked
        // member's too); the case shown; the picked member taken out.)
        Tool::Frame => {
            if pressed && let Some(k) = (0..STOCK_ROWS).find(|&n| interior.stock_top + n < stocks().len() && inside(stock_row(n), cursor)).map(|n| interior.stock_top + n) {
                interior.stock = k;
                if let Some(b) = interior.beam_pick.and_then(|j| interior.plan.beams.get_mut(j)) {
                    b.stock = stocks()[k].key.clone();
                }
                return true;
            }
            if let Some(Action::Case(c)) = action {
                interior.case = c;
            }
            if action == Some(Action::Truss) {
                interior.truss_mode = !interior.truss_mode;
                (interior.beam_from, interior.truss_from) = (None, None);
            }
            if action == Some(Action::Mix) {
                interior.mix = !interior.mix;
            }
            // (BRACE: what's past its limit as last worked out, braced.)
            if action == Some(Action::Brace) {
                match interior.bearing.as_ref().filter(|(p, _)| *p == interior.plan) {
                    Some((_, b)) => {
                        let (beams, n) = brace(&interior.plan, b);
                        interior.message = Some((if n == 0 { "NOTHING PAST ITS LIMIT TO BRACE".to_string() } else { format!("{n} MEMBERS BRACED: AUTO-SIZE THEM") }, 5.0));
                        interior.plan.beams = beams;
                    }
                    None => interior.message = Some(("STILL WORKING THE FRAME OUT: A MOMENT".into(), 3.0)),
                }
            }
            // (DEPTH and SPACING: the next truss's.)
            if let Some(k) = interior.slider {
                let (at, len) = sliders(Tool::Frame)[k];
                let (lo, hi) = slider_range(Tool::Frame);
                let v = ((lo + ((cursor.x - at.x) / len.x).clamp(0.0, 1.0) * (hi - lo)) * 2.0).round() / 2.0;
                if k == 0 { interior.depth = v } else { interior.spacing = v }
            }
            if action == Some(Action::MountAll) {
                let stock = stocks().get(interior.stock).map_or(String::new(), |s| s.key.clone());
                let (beams, added) = mounts(&interior.plan, &interior.fit, interior.spacing.max(1.0), &stock);
                interior.message = Some((if added == 0 { "EVERY MODULE IS MOUNTED (OR THERE'S NO FRAME NEAR IT)".to_string() } else { format!("{added} MOUNTING MEMBERS ADDED") }, 4.0));
                interior.plan.beams = beams;
            }
            if action == Some(Action::AutoSize) && interior.sizing.is_none() && !interior.plan.beams.is_empty() {
                let (plan, fit, spec, mix) = (interior.plan.clone(), interior.fit.clone(), interior.spec(), interior.mix);
                let (tx, rx) = mpsc::channel();
                let snapshot = plan.clone();
                job("studio-size", move || {
                    tx.send(auto_size(&plan, &fit, spec, mix)).ok();
                });
                interior.sizing = Some((snapshot, rx));
            }
            if action == Some(Action::Remove)
                && let Some(k) = interior.beam_pick.take()
                && k < interior.plan.beams.len()
            {
                interior.plan.beams.remove(k);
            }
        }
        // (The module picked: a row of the list; its width or height stretched, its
        // length then what keeps its volume; turned; taken out.)
        // (A row picks what's placed next: in a hull, its one module, picked if it's
        // placed; with none, a new one of that kind. What's done to a module is done
        // to the one picked.)
        Tool::Modules => {
            if pressed && let Some(n) = (0..FIT_ROWS).find(|&n| interior.fit_top + n < interior.fit.len() && inside(module_row(n), cursor)) {
                let k = interior.fit_top + n;
                interior.module = Some(k);
                interior.block = if interior.spec().is_some() { interior.plan.blocks.iter().position(|b| b.id == interior.fit[k].id) } else { None };
                return true;
            }
            if let Some(n) = interior.block.filter(|&n| n < interior.plan.blocks.len())
                && let Some(f) = interior.fit.iter().find(|f| f.id == kind(&interior.plan.blocks[n].id)).cloned()
            {
                let b = &mut interior.plan.blocks[n];
                if let Some(k) = interior.slider {
                    let (at, len) = sliders(Tool::Modules)[k];
                    let (lo, hi) = slider_range(Tool::Modules);
                    let v = ((lo + ((cursor.x - at.x) / len.x).clamp(0.0, 1.0) * (hi - lo)) * 10.0).round() / 10.0;
                    let floor = b.at.y - b.size.y * 0.5;
                    if k == 0 { b.size.x = v } else { b.size.y = v }
                    let shape = if f.round { std::f32::consts::PI / 6.0 } else { 1.0 };
                    b.size.z = f.volume / (shape * b.size.x * b.size.y);
                    // (Standing where it stood.)
                    b.at.y = floor + b.size.y * 0.5;
                }
                if action == Some(Action::Turn) {
                    (b.size.x, b.size.z) = (b.size.z, b.size.x);
                }
            }
            if action == Some(Action::Remove)
                && let Some(n) = interior.block.take().filter(|&n| n < interior.plan.blocks.len())
            {
                interior.plan.blocks.remove(n);
            }
        }
    }
    // Groups: the picked lines one (out of any other); their groups broken up; walled
    // off or opened up (lines in no group made one first).
    match action {
        Some(Action::Group) if !picked.is_empty() => {
            for g in &mut interior.plan.groups {
                g.lines.retain(|k| !picked.contains(k));
            }
            interior.plan.groups.retain(|g| !g.lines.is_empty());
            interior.plan.groups.push(Group { lines: picked.clone(), walled: false });
        }
        Some(Action::Ungroup) => {
            interior.plan.groups.retain(|g| !g.lines.iter().any(|k| picked.contains(k)));
        }
        Some(Action::Wall) if !picked.is_empty() => {
            if picked.iter().any(|&k| interior.plan.group_of(k).is_none()) {
                for g in &mut interior.plan.groups {
                    g.lines.retain(|k| !picked.contains(k));
                }
                interior.plan.groups.retain(|g| !g.lines.is_empty());
                interior.plan.groups.push(Group { lines: picked.clone(), walled: true });
            } else {
                let wall = !picked.iter().all(|&k| interior.plan.group_of(k).is_some_and(|g| interior.plan.groups[g].walled));
                for &k in &picked {
                    if let Some(g) = interior.plan.group_of(k) {
                        interior.plan.groups[g].walled = wall;
                    }
                }
            }
        }
        _ => {}
    }
    if interior.slider.is_some() {
        return true;
    }
    interior.hover = if matches!(interior.tool, Tool::Modules | Tool::Frame) { None } else { hover_at(interior, &cam, cursor) };
    interior.beam_hover = if interior.tool == Tool::Frame && interior.shown(layer::FRAME) && !inside(PANEL, cursor) { beam_at(interior, &cam, cursor) } else { None };
    // (MODULES: the nearest placed one under the cursor.)
    interior.block_hover = None;
    if interior.tool == Tool::Modules && interior.shown(layer::MODULES) && !inside(PANEL, cursor) {
        let ray = cam.ray(cursor);
        interior.block_hover = interior.plan.blocks.iter().enumerate().filter_map(|(n, b)| b.hit(cam.eye, ray).map(|t| (n, t))).min_by(|a, b| a.1.total_cmp(&b.1)).map(|(n, _)| n);
    }
    interior.module_hover = interior.block_hover.and_then(|n| interior.fit.iter().position(|f| f.id == kind(&interior.plan.blocks[n].id)));
    // A walk-through: F over a point or a tube, there; WALK HERE, then a click on one.
    if pressed && inside(walk_button(), cursor) {
        interior.walk_armed = !interior.walk_armed;
        return true;
    }
    let walk_here = input.pressed(KeyCode::KeyF) || (interior.walk_armed && pressed && !inside(PANEL, cursor));
    if walk_here {
        let spot = match interior.hover {
            Some(Hover::Point(k)) => {
                // (At a point: on the floor of a tube it ends, if one does.)
                let line = interior.plan.lines.iter().position(|&(a, b, _)| a == k || b == k);
                Some(interior.feet(interior.plan.points[k].at, line))
            }
            Some(Hover::Line(k)) => on_line(interior, &cam, cursor, k).map(|(at, _)| interior.feet(at, Some(k))),
            None => None,
        };
        if let Some(at) = spot {
            interior.walk = Some(at);
            interior.walk_armed = false;
            return true;
        }
    }
    // (FRAME: DEL takes out the member under the cursor.)
    if input.pressed(KeyCode::Delete)
        && let Some(k) = interior.beam_hover.take()
    {
        interior.plan.beams.remove(k);
        interior.beam_pick = None;
        return true;
    }
    // Removed: what's under the cursor (DEL), or what's picked (REMOVE); the hull's
    // own points stay; a point takes its lines with it.
    let gone = if input.pressed(KeyCode::Delete) { interior.hover.or(interior.pick) } else if action == Some(Action::Remove) { interior.pick } else { None };
    match gone {
        Some(Hover::Point(k)) if interior.plan.points[k].name.is_none() => {
            interior.plan.points.remove(k);
            interior.plan.keep_lines(|_, &(a, b, _)| a != k && b != k);
            for l in &mut interior.plan.lines {
                l.0 -= usize::from(l.0 > k);
                l.1 -= usize::from(l.1 > k);
            }
            interior.from = None;
            interior.pick = None;
            interior.more.clear();
        }
        Some(Hover::Line(k)) => {
            // (REMOVE: every line picked; DEL over a line: that one.)
            let lines = if action == Some(Action::Remove) { picked_lines(interior) } else { vec![k] };
            interior.plan.keep_lines(|j, _| !lines.contains(&j));
            interior.pick = None;
            interior.more.clear();
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
    // (The right button, pressed and not dragged: what's in progress cancelled, as
    // ESC does.)
    if input.button_pressed(MouseButton::Right) && interior.in_progress() {
        interior.cancel();
        interior.stopped = true;
    }
    if input.button_pressed(MouseButton::Right) && interior.truss_from.is_some() {
        interior.truss_from = None;
        interior.stopped = true;
    }
    if input.button_pressed(MouseButton::Right) && interior.beam_from.is_some() {
        interior.beam_from = None;
        interior.stopped = true;
    }
    if input.button_pressed(MouseButton::Right) && interior.from.is_some() {
        interior.from = None;
        interior.stopped = true;
    }
    if !input.button_down(MouseButton::Right) {
        interior.stopped = false;
    }
    // The plane's handle (laying paths): held, it goes up and down with the mouse.
    let plane = plane_of(interior, &h);
    if matches!(interior.tool, Tool::Path | Tool::Modules | Tool::Frame) {
        interior.map_hollow(plane);
    }
    if pressed && matches!(interior.tool, Tool::Path | Tool::Modules | Tool::Frame) && cam.project(plane_handle(&cam, &h, plane)).is_some_and(|(q, _)| q.distance(cursor) < 10.0) {
        interior.lifting = true;
    }
    if interior.lifting {
        if input.button_down(MouseButton::Left) {
            let handle = plane_handle(&cam, &h, plane);
            let per_pixel = (handle - cam.eye).length() / cam.focal;
            // (Where the mouse has it, unsnapped, so it moves on past a point.)
            let free = interior.lift_free.unwrap_or(plane) - d.y * per_pixel;
            interior.lift_free = Some(free.clamp(h.lo.y, h.hi.y));
            interior.plane = interior.lift_free;
            // Within 30 cm of the selected point's level: on it.
            if let Some(y) = selected_level(interior)
                && (free - y).abs() < 0.3
            {
                interior.plane = Some(y);
            }
        } else {
            interior.lifting = false;
            interior.lift_free = None;
            if interior.plane.is_some_and(|y| selected_level(interior) != Some(y)) {
                interior.plane = interior.plane.map(|y| (y * 10.0).round() / 10.0);
            }
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
            // FRAME: a click at a joint or a bearer (else on the work plane) ends the
            // member being laid there and starts the next from it; not laying one, a
            // click on a member picks it, else starts one.
            // (TRUSS: its first corner, then its second: the truss laid between.)
            Tool::Frame if interior.truss_mode => {
                if let Some(at) = on_plane(interior, &cam, plane, cursor, false) {
                    match interior.truss_from.take() {
                        None => interior.truss_from = Some(at),
                        Some(from) => {
                            let stock = stocks().get(interior.stock).map_or(String::new(), |s| s.key.clone());
                            let fresh: Vec<Beam> = truss(from, Vec3::new(at.x, from.y, at.z), interior.depth, interior.spacing, &stock).into_iter().filter(|n| !interior.plan.beams.iter().any(|b| (b.a == n.a && b.b == n.b) || (b.a == n.b && b.b == n.a))).collect();
                            interior.message = Some((format!("A TRUSS OF {} MEMBERS LAID", fresh.len()), 4.0));
                            interior.plan.beams.extend(fresh);
                        }
                    }
                }
            }
            Tool::Frame => {
                let snap = frame_snap(interior, &cam, cursor);
                if interior.beam_from.is_none() && snap.is_none() && interior.beam_hover.is_some() {
                    interior.beam_pick = interior.beam_hover;
                    if let Some(k) = interior.beam_pick.and_then(|k| stocks().iter().position(|s| s.key == interior.plan.beams[k].stock)) {
                        interior.stock = k;
                    }
                } else if let Some(to) = snap.or_else(|| on_plane(interior, &cam, plane, cursor, false)) {
                    if let Some(from) = interior.beam_from
                        && from.distance(to) > 0.05
                        && let Some(stock) = stocks().get(interior.stock)
                    {
                        interior.plan.beams.push(Beam { a: from, b: to, stock: stock.key.clone() });
                    }
                    interior.beam_from = Some(to);
                    interior.beam_pick = None;
                }
            }
            // A module under the cursor: picked. Else the one picked put where the
            // cursor is on the work plane, standing on it (if it's there already:
            // moved).
            Tool::Modules => {
                if let Some(n) = interior.block_hover.filter(|&n| Some(n) != interior.block) {
                    interior.block = Some(n);
                    interior.module = interior.module_hover;
                } else if let Some(at) = on_plane(interior, &cam, plane, cursor, false) {
                    match interior.block.filter(|&n| n < interior.plan.blocks.len()) {
                        // (The one picked: moved there.)
                        Some(n) => {
                            let b = &mut interior.plan.blocks[n];
                            b.at = at + Vec3::Y * b.size.y * 0.5;
                        }
                        // (A row picked, nothing placed of it yet (or, with no hull, a
                        // new one): put there.)
                        None => {
                            if let Some(f) = interior.module.and_then(|k| interior.fit.get(k)).cloned() {
                                let id = if interior.spec().is_some() {
                                    f.id.clone()
                                } else {
                                    let n = (1..).find(|n| !interior.plan.blocks.iter().any(|b| b.id == format!("{}#{n}", f.id))).unwrap_or(1);
                                    format!("{}#{n}", f.id)
                                };
                                interior.plan.blocks.push(Block { id, at: at + Vec3::Y * f.size.y * 0.5, size: f.size });
                                interior.block = Some(interior.plan.blocks.len() - 1);
                            }
                        }
                    }
                }
            }
            Tool::Path => {
                // On a point: joined to it, and on from it. On a line: a point there
                // (its quarter, middle or three-quarter mark if near one), the line
                // split through it. Else on the plane: a new point there. Either new
                // one joined to the last.
                let to = match interior.hover {
                    Some(Hover::Point(k)) => Some(k),
                    Some(Hover::Line(k)) => on_line(interior, &cam, cursor, k).map(|(at, _)| {
                        let (a, b, profile) = interior.plan.lines[k];
                        interior.plan.points.push(Point { at, name: None });
                        let n = interior.plan.points.len() - 1;
                        interior.plan.lines[k] = (a, n, profile);
                        interior.plan.lines.push((n, b, profile));
                        let new = interior.plan.lines.len() - 1;
                        if let Some(g) = interior.plan.group_of(k) {
                            interior.plan.groups[g].lines.push(new);
                        }
                        n
                    }),
                    None => on_plane(interior, &cam, plane, cursor, interior.shift).map(|at| {
                        interior.plan.points.push(Point { at, name: None });
                        interior.plan.points.len() - 1
                    }),
                };
                if let Some(k) = to {
                    let onto = matches!(interior.hover, Some(Hover::Point(_)));
                    if let Some(a) = interior.from
                        && a != k
                        && !interior.plan.lines.iter().any(|&(p, q, _)| (p, q) == (a, k) || (p, q) == (k, a))
                    {
                        interior.plan.lines.push((a, k, interior.profile));
                        // (A room preset's: its free ends doorways.)
                        if PRESETS.iter().any(|(_, p, doors)| *doors && *p == interior.profile) {
                            let new = interior.plan.lines.len() - 1;
                            interior.plan.ends.extend([EndSet(new, 0, End::Door, interior.hatch), EndSet(new, 1, End::Door, interior.hatch)]);
                        }
                        // (Joined to a point already there: that tunnel done, picked to
                        // be worked on, the path tool put down.)
                        if onto {
                            interior.pick = Some(Hover::Line(interior.plan.lines.len() - 1));
                            interior.from = None;
                            interior.tool = Tool::Look;
                            return true;
                        }
                    }
                    interior.from = Some(k);
                }
            }
            // A click picks what's under the cursor (a line in a group: with the rest
            // of it); with SHIFT, a line added to the picked ones, or taken out.
            // A doorway: at a tube's end (within 30 px of it on screen), that end
            // cycled; along it, one in the wall facing the camera (or that one gone).
            Tool::Door => {
                if let Some(Hover::Line(k)) = interior.hover {
                    let (a, b, _) = interior.plan.lines[k];
                    let (pa, pb) = (interior.plan.points[a].at, interior.plan.points[b].at);
                    let near_end = [pa, pb].iter().position(|p| cam.project(*p).is_some_and(|(q, _)| q.distance(cursor) < 30.0));
                    match near_end {
                        Some(e) => {
                            let next = match interior.plan.end_of(k, e) {
                                End::Closed => End::Door,
                                End::Door => End::Open,
                                End::Open => End::Closed,
                            };
                            interior.plan.ends.retain(|&EndSet(j, f, _, _)| !(j == k && f == e));
                            interior.plan.ends.push(EndSet(k, e, next, interior.hatch));
                            if !interior.plan.free_end(k, [a, b][e]) {
                                interior.message = Some(("THAT END JOINS ANOTHER TUBE: IT'S A WAY THROUGH ALREADY".into(), 3.0));
                            }
                        }
                        None => {
                            if let Some((at, _)) = on_line(interior, &cam, cursor, k) {
                                let len = pa.distance(pb).max(1e-3);
                                let d = (pb - pa) / len;
                                let u = d.cross(Vec3::Y).try_normalize().unwrap_or(Vec3::X);
                                let right = u.dot(cam.eye - at) > 0.0;
                                let t = ((at - pa).dot(d) / len).clamp(0.0, 1.0);
                                let near = interior.plan.doors.iter().position(|&SideDoor(j, s, r, _)| j == k && r == right && (s - t).abs() * len < 1.0);
                                match near {
                                    Some(n) => {
                                        interior.plan.doors.remove(n);
                                    }
                                    None => interior.plan.doors.push(SideDoor(k, t, right, interior.hatch)),
                                }
                            }
                        }
                    }
                }
            }
            Tool::Look => match (interior.hover, interior.shift) {
                (Some(Hover::Line(k)), true) => {
                    let mut lines = picked_lines(interior);
                    if let Some(at) = lines.iter().position(|&j| j == k) {
                        lines.remove(at);
                    } else {
                        lines.push(k);
                    }
                    interior.pick = lines.first().map(|&j| Hover::Line(j));
                    interior.more = lines.into_iter().skip(1).collect();
                }
                (Some(Hover::Line(k)), false) => {
                    interior.pick = Some(Hover::Line(k));
                    interior.more = interior.plan.group_of(k).map_or_else(Vec::new, |g| interior.plan.groups[g].lines.iter().copied().filter(|&j| j != k).collect());
                }
                (other, _) => {
                    interior.pick = other;
                    interior.more.clear();
                }
            },
        }
    }
    let radius = (h.hi - h.lo).length() * 0.5;
    let distance = (cam.eye - interior.target.unwrap_or((h.lo + h.hi) * 0.5)).length();
    if (input.button_down(MouseButton::Right) && !interior.stopped) || input.button_down(MouseButton::Middle) {
        let per_pixel = distance / cam.focal;
        let target = interior.target.unwrap_or((h.lo + h.hi) * 0.5);
        interior.target = Some(target + (-cam.right * d.x + cam.up * d.y) * per_pixel);
    }
    if input.scroll != 0.0 && inside(PANEL, cursor) {
        if interior.tool == Tool::Modules {
            let most = interior.fit.len().saturating_sub(FIT_ROWS);
            interior.fit_top = (interior.fit_top as f32 - input.scroll.signum() * 3.0).clamp(0.0, most as f32) as usize;
        }
        if interior.tool == Tool::Frame {
            let most = stocks().len().saturating_sub(STOCK_ROWS);
            interior.stock_top = (interior.stock_top as f32 - input.scroll.signum() * 2.0).clamp(0.0, most as f32) as usize;
        }
    } else if input.scroll != 0.0 {
        interior.distance = Some((distance * 0.88f32.powf(input.scroll)).clamp(radius * 0.05, radius * 8.0));
    }
    // HOME: the view as it was (the plan kept).
    if input.pressed(KeyCode::Home) {
        let fresh = Interior::new();
        (interior.yaw, interior.pitch, interior.distance, interior.target) = (fresh.yaw, fresh.pitch, None, None);
    }
    true
}

pub fn draw(frame: &mut Frame, _app: &App, place: &str, interior: &Interior) {
    let size = frame.size();
    let spec = interior.spec();
    frame.hud_rect(Vec2::ZERO, size, PAPER);
    let on = spec.map_or("NO HULL".to_string(), |s| s.name.to_uppercase());
    frame.text(Vec2::new(12.0, 10.0), &format!("{place}   INTERIOR STUDIO - {} ({on})", interior.plan.hull.to_uppercase()), LABEL);
    let hint = match interior.tool {
        Tool::Modules => "PICK ONE IN THE LIST, CLICK THE PLANE: IT STANDS THERE - CLICK ONE IN THE VIEW TO PICK IT - RIGHT-CLICK OR ESC DROPS IT - WHEEL OVER THE LIST SCROLLS",
        _ => "LEFT-DRAG TURNS IT - RIGHT-DRAG MOVES IT - WHEEL: NEARER, FARTHER - HOME: AS IT WAS - ESC CLOSES",
    };
    frame.text_scaled(Vec2::new(12.0, size.y - 18.0), hint, LABEL.scale(0.6), 0.7);
    // The toolbar: the tool in hand lit, the button under the cursor brighter.
    {
        use crate::hud::{draw_cell, Lamp};
        for (k, (key, name, tool)) in TOOLBAR.iter().enumerate() {
            let (p, c) = button(k);
            let lamp = if *tool == interior.tool || button_at(interior.cursor) == Some(*tool) { Lamp::On } else { Lamp::Off };
            draw_cell(frame, p, c, key, name, lamp);
        }
        {
            let (p, c) = walk_button();
            let lamp = if interior.walk_armed || inside((p, c), interior.cursor) { Lamp::On } else { Lamp::Off };
            draw_cell(frame, p, c, "F", "WALK", lamp);
        }
        {
            let (p, c) = reach_button();
            let lamp = if interior.reach_job.is_some() { Lamp::Busy } else if inside((p, c), interior.cursor) { Lamp::On } else { Lamp::Off };
            draw_cell(frame, p, c, "", if interior.reach_job.is_some() { "CHECKING" } else { "REACH" }, lamp);
        }
        for (k, (key, name)) in [("^S", if interior.unsaved() { "SAVE *" } else { "SAVE" }), ("^O", "OPEN"), ("^N", "NEW")].into_iter().enumerate() {
            let (p, c) = file_button(k);
            let lamp = if inside((p, c), interior.cursor) { Lamp::On } else if k == 0 && interior.unsaved() { Lamp::Busy } else { Lamp::Off };
            draw_cell(frame, p, c, key, name, lamp);
        }
        for (k, (key, name, redo)) in HISTORY.iter().enumerate() {
            let (p, c) = history_button(k);
            let some = if *redo { !interior.redo.is_empty() } else { !interior.undo.is_empty() };
            let lamp = if !some { Lamp::Unavailable } else if inside((p, c), interior.cursor) { Lamp::On } else { Lamp::Off };
            draw_cell(frame, p, c, key, name, lamp);
        }
    }
    let Some(h) = interior.hull() else {
        if let Some(spec) = spec.filter(|s| s.shape().walk.is_none()) {
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
    let grid = interior.shown(layer::GRID);
    for i in (x0..=x1).filter(|_| grid) {
        let x = i as f32 * step;
        if let (Some((a, _)), Some((b, _))) = (cam.project(ground(x, z0 as f32 * step)), cam.project(ground(x, z1 as f32 * step))) {
            frame.hud_line(a, b, GROUND);
        }
    }
    for i in (z0..=z1).filter(|_| grid) {
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
    let rows = if grid { ((top - lo.y) / step).round() as i32 } else { -1 };
    for k in 0..=rows {
        let y = lo.y + k as f32 * step;
        seg(frame, Vec3::new(far_x, y, gz0), Vec3::new(far_x, y, gz1), GROUND);
        seg(frame, Vec3::new(gx0, y, far_z), Vec3::new(gx1, y, far_z), GROUND);
    }
    for i in (z0..=z1).filter(|_| grid) {
        let z = i as f32 * step;
        seg(frame, Vec3::new(far_x, lo.y, z), Vec3::new(far_x, top, z), GROUND);
    }
    for i in (x0..=x1).filter(|_| grid) {
        let x = i as f32 * step;
        seg(frame, Vec3::new(x, lo.y, far_z), Vec3::new(x, top, far_z), GROUND);
    }
    // Its measures: its length along the ground on the near side, its beam across
    // the far end, its height up the far corner (against the walls); ticks at their
    // ends, the measure by their middles.
    if grid {
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
    let hull_shown = interior.shown(layer::HULL);
    for [a, b] in h.lines.iter().filter(|_| hull_shown) {
        line(frame, *a, *b);
    }
    // (Round things' outlines from here: one face toward the camera, one away.)
    for [a, b, n1, n2] in h.bends.iter().filter(|_| hull_shown) {
        let to_eye = cam.eye - *a;
        if n1.dot(to_eye) * n2.dot(to_eye) < 0.0 {
            line(frame, *a, *b);
        }
    }
    // The work plane (placing points): its outline over the hull, at its height.
    let plane = plane_of(interior, &h);
    if matches!(interior.tool, Tool::Path | Tool::Modules | Tool::Frame) {
        let c = [Vec3::new(lo.x, plane, lo.z), Vec3::new(hi.x, plane, lo.z), Vec3::new(hi.x, plane, hi.z), Vec3::new(lo.x, plane, hi.z)];
        // The hull at this height: hollow cells green, solid red (where there's room).
        if let Some((at, cells)) = interior.hollow.as_ref().filter(|_| interior.shown(layer::HOLLOW)) {
            let y = *at;
            for (c, hollow) in cells.iter() {
                let q = [Vec3::new(c.x - 0.48, y, c.y - 0.48), Vec3::new(c.x + 0.48, y, c.y - 0.48), Vec3::new(c.x + 0.48, y, c.y + 0.48), Vec3::new(c.x - 0.48, y, c.y + 0.48)];
                if let [Some((a, _)), Some((b, _)), Some((cc, _)), Some((d, _))] = q.map(|p| cam.project(p)) {
                    let fill = [if *hollow { Color([0.4, 1.0, 0.55, 0.13]) } else { Color([1.0, 0.35, 0.3, 0.16]) }; 3];
                    frame.hud_triangle_colored([a, b, cc], fill);
                    frame.hud_triangle_colored([a, cc, d], fill);
                }
            }
        }
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
            let at_point = selected_level(interior) == Some(plane);
            frame.text_scaled(q + Vec2::new(16.0, -4.0), &format!("PLANE {:.1} M{}", plane - lo.y, if at_point { "  AT POINT" } else { "" }), if at_point { PICKED } else { col }, 0.7);
        }
        // Where a click would put a point: over a line, its quarter marks (the one
        // it'd snap to lit); else on the plane (SHIFT: squared to the last point).
        // Not over a point: that joins to it.
        match interior.hover {
            Some(Hover::Line(k)) => {
                let (a, b, _) = interior.plan.lines[k];
                let (pa, pb) = (interior.plan.points[a].at, interior.plan.points[b].at);
                let snap = on_line(interior, &cam, interior.cursor, k);
                for (m, t) in [0.25f32, 0.5, 0.75].into_iter().enumerate() {
                    if let Some((q, _)) = cam.project(pa.lerp(pb, t)) {
                        let lit = snap.is_some_and(|s| s.1 == Some(m));
                        let r = if m == 1 { 5.0 } else { 3.5 };
                        let col = if lit { PICKED } else { Color([1.0, 1.0, 1.0, 0.9]) };
                        frame.hud_line(q - Vec2::new(r, r), q + Vec2::new(r, r), col);
                        frame.hud_line(q - Vec2::new(r, -r), q + Vec2::new(r, -r), col);
                    }
                }
                if let Some((at, _)) = snap
                    && let Some((q, _)) = cam.project(at)
                {
                    frame.hud_box(q - Vec2::splat(4.0), Vec2::splat(8.0), PICKED);
                }
            }
            Some(Hover::Point(_)) => {}
            // (FRAME: where a click would join, and the member being laid to it.)
            // (TRUSS: from its first corner to the cursor, its box.)
            None if interior.tool == Tool::Frame && interior.truss_mode => {
                if let Some(to) = on_plane(interior, &cam, plane, interior.cursor, false) {
                    let from = interior.truss_from.unwrap_or(to);
                    let (lo, hi) = (from.min(Vec3::new(to.x, from.y, to.z)), from.max(Vec3::new(to.x, from.y, to.z)) + Vec3::Y * interior.depth);
                    let ghost = Block { id: String::new(), at: (lo + hi) * 0.5, size: hi - lo };
                    for [a, b] in ghost.edges(false) {
                        seg(frame, a, b, PICKED.scale(0.6));
                    }
                    if let Some((q, _)) = cam.project(to) {
                        let text = if interior.truss_from.is_some() { format!("{:.1} X {:.1} X {:.1} M", hi.x - lo.x, hi.z - lo.z, interior.depth) } else { "FIRST CORNER".to_string() };
                        frame.text_scaled(q + Vec2::new(10.0, 6.0), &text, PICKED, 0.7);
                    }
                }
            }
            None if interior.tool == Tool::Frame => {
                let snap = frame_snap(interior, &cam, interior.cursor);
                let to = snap.or_else(|| on_plane(interior, &cam, plane, interior.cursor, false));
                if let Some(to) = to
                    && let Some((q, _)) = cam.project(to)
                {
                    frame.hud_box(q - Vec2::splat(4.0), Vec2::splat(8.0), if snap.is_some() { PICKED } else { PICKED.scale(0.5) });
                    if let Some(from) = interior.beam_from {
                        seg(frame, from, to, PICKED.scale(0.7));
                        frame.text_scaled(q + Vec2::new(10.0, 6.0), &format!("{:.1} M", from.distance(to)), PICKED, 0.7);
                    }
                }
            }
            // (MODULES: the picked one where a click would put it, faint.)
            None if interior.tool == Tool::Modules => {
                let picked = interior.block.and_then(|n| interior.plan.blocks.get(n));
                if interior.block_hover.is_none()
                    && let Some(f) = picked.and_then(|b| interior.fit.iter().find(|f| f.id == kind(&b.id))).or_else(|| interior.module.and_then(|k| interior.fit.get(k)))
                    && let Some(at) = on_plane(interior, &cam, plane, interior.cursor, false)
                {
                    let size = picked.map_or(f.size, |b| b.size);
                    let ghost = Block { id: f.id.clone(), at: at + Vec3::Y * size.y * 0.5, size };
                    for [a, b] in ghost.edges(f.round) {
                        seg(frame, a, b, MODULE.scale(0.45));
                    }
                }
            }
            None => {
                if let Some(at) = on_plane(interior, &cam, plane, interior.cursor, interior.shift)
                    && let Some((q, _)) = cam.project(at)
                {
                    frame.hud_box(q - Vec2::splat(3.0), Vec2::splat(6.0), PICKED.scale(0.6));
                }
            }
        }
    }
    // The deck studio's decks: their floors faintly filled, their walls outlined.
    if let Some((_, shapes)) = &interior.decks {
        for (outline, floor) in shapes {
            if !interior.shown(if *floor { layer::DECK_FLOORS } else { layer::DECK_WALLS }) {
                continue;
            }
            let on: Option<Vec<Vec2>> = outline.iter().map(|p| cam.project(*p).map(|s| s.0)).collect();
            let Some(on) = on else { continue };
            if *floor {
                for k in 1..on.len().saturating_sub(1) {
                    frame.hud_triangle_colored([on[0], on[k], on[k + 1]], [DECK_FLOOR; 3]);
                }
            } else {
                for k in 0..on.len() {
                    frame.hud_line(on[k], on[(k + 1) % on.len()], DECK_WALL);
                }
            }
        }
    }
    // The access plan: its lines bright, its points small squares (the hull's own,
    // named); what's under the cursor and a line's first point lit.
    let plan = &interior.plan;
    // Each line, and its room round it: its cross-section along it (red where it
    // clashes with the hull, its clashes marked).
    let clash = interior.checked.as_ref().filter(|(p, _)| p == plan).map(|(_, c)| c.as_slice()).unwrap_or(&[]);
    let picked = picked_lines(interior);
    // Walled groups: their tubes' walls (cut away inside one another, their ends open).
    for q in interior.panels().iter().filter(|_| interior.shown(layer::WALLS)) {
        let on: Option<Vec<Vec2>> = q.outline.iter().map(|p| cam.project(*p).map(|s| s.0)).collect();
        if let Some(on) = on {
            for k in 1..on.len().saturating_sub(1) {
                frame.hud_triangle_colored([on[0], on[k], on[k + 1]], [WALL; 3]);
            }
        }
    }
    // Hatches, closed: each leaf filled, its outline and its window (an open
    // doorway: just the hole).
    for leaf in plan.leaves().into_iter().filter(|_| interior.shown(layer::HATCHES)) {
        let on: Option<Vec<Vec2>> = leaf.outline.iter().map(|p| cam.project(*p).map(|q| q.0)).collect();
        let glass: Option<Vec<Vec2>> = leaf.window.iter().map(|p| cam.project(*p).map(|q| q.0)).collect();
        let (Some(on), Some(glass)) = (on, glass) else { continue };
        for k in 1..on.len().saturating_sub(1) {
            frame.hud_triangle_colored([on[0], on[k], on[k + 1]], [Color([1.0, 0.65, 0.2, 0.18]); 3]);
        }
        for k in 0..on.len() {
            frame.hud_line(on[k], on[(k + 1) % on.len()], DOOR_FRAME);
        }
        for k in 0..glass.len() {
            frame.hud_line(glass[k], glass[(k + 1) % glass.len()], Color([0.45, 0.9, 1.0, 1.0]));
        }
    }
    // The modules placed: faintly filled, outlined (picked or under the cursor lit;
    // clashing red), named.
    let block_clash = interior.block_clash.as_ref().filter(|(b, _)| *b == plan.blocks).map(|(_, c)| c.as_slice()).unwrap_or(&[]);
    for (n, b) in plan.blocks.iter().enumerate().filter(|_| interior.shown(layer::MODULES)) {
        let Some(k) = interior.fit.iter().position(|f| f.id == kind(&b.id)) else { continue };
        let f = &interior.fit[k];
        let lit = interior.tool == Tool::Modules && (interior.block == Some(n) || interior.block_hover == Some(n));
        let col = if lit { PICKED } else if block_clash.get(n) == Some(&true) { CLASH } else { MODULE };
        for (t, _) in b.faces(f.round) {
            if let [Some((p, _)), Some((q, _)), Some((r, _))] = t.map(|p| cam.project(p)) {
                frame.hud_triangle_colored([p, q, r], [Color([col.0[0], col.0[1], col.0[2], if f.round { 0.025 } else { 0.07 }]); 3]);
            }
        }
        for [p, q] in b.edges(f.round) {
            seg(frame, p, q, Color([col.0[0], col.0[1], col.0[2], 0.85]));
        }
        if let Some((q, _)) = cam.project(b.at + Vec3::Y * b.size.y * 0.5) {
            let w = f.name.chars().count() as f32 * universe_engine::frame::GLYPH * 0.6;
            frame.text_scaled(q + Vec2::new(-w / 2.0, -12.0), &f.name, col, 0.6);
        }
    }
    // The frame: each member coloured by how hard it's worked (the case shown, or
    // its worst): green easy, amber near its limit, red past it; broken, red and
    // crossed. Its load points: the landing pads and nozzles (lit if a joint's at
    // them).
    if interior.shown(layer::FRAME) {
        let bearing = interior.bearing.as_ref().filter(|(p, _)| *p == interior.plan).map(|(_, b)| b.clone());
        let outcome = |k: usize| -> Option<(f64, bool)> {
            let b = bearing.as_ref()?;
            let pick: Vec<&Result<universe_sim::world::frame::Collapse, String>> = match interior.case {
                Some(c) => b.cases.get(c).map(|c| &c.1).into_iter().collect(),
                None => b.cases.iter().map(|c| &c.1).collect(),
            };
            let m = (*b.index.get(k)?)?;
            let os: Vec<_> = pick.into_iter().filter_map(|r| r.as_ref().ok()).filter_map(|c| c.members.get(m)).collect();
            (!os.is_empty()).then(|| (os.iter().map(|o| o.work.design).fold(0.0, f64::max), os.iter().any(|o| o.broken.is_some())))
        };
        for (k, b) in plan.beams.iter().enumerate() {
            let lit = interior.beam_pick == Some(k) || interior.beam_hover == Some(k);
            let (col, broken) = match outcome(k) {
                _ if lit => (PICKED, false),
                Some((_, true)) => (CLASH, true),
                Some((u, _)) if u >= 1.0 => (Color([1.0, 0.3, 0.25, 1.0]), false),
                Some((u, _)) if u >= 0.5 => (Color([1.0, 0.7, 0.2, 1.0]), false),
                Some(_) => (Color([0.4, 1.0, 0.5, 1.0]), false),
                None => (Color([0.75, 0.8, 0.85, 1.0]), false),
            };
            if let (Some((pa, _)), Some((pb, _))) = (cam.project(b.a), cam.project(b.b)) {
                if broken {
                    // (Dashed, and crossed at its middle.)
                    let n = ((pb - pa).length() / 6.0).max(1.0) as usize;
                    for i in (0..n).step_by(2) {
                        frame.hud_line(pa.lerp(pb, i as f32 / n as f32), pa.lerp(pb, ((i + 1) as f32 / n as f32).min(1.0)), col);
                    }
                    let m = (pa + pb) * 0.5;
                    frame.hud_line(m - Vec2::splat(4.0), m + Vec2::splat(4.0), col);
                    frame.hud_line(m + Vec2::new(-4.0, 4.0), m + Vec2::new(4.0, -4.0), col);
                } else {
                    // (A bit heavier than a line: two side by side.)
                    let across = (pb - pa).perp().normalize_or_zero() * 0.6;
                    frame.hud_line(pa + across, pb + across, col);
                    frame.hud_line(pa - across, pb - across, col);
                }
            }
        }
        for b in &plan.beams {
            for p in [b.a, b.b] {
                if let Some((q, _)) = cam.project(p) {
                    frame.hud_rect(q - Vec2::splat(1.5), Vec2::splat(3.0), Color([0.9, 0.95, 1.0, 0.9]));
                }
            }
        }
        if interior.tool == Tool::Frame {
            use universe_sim::world::shape::Role;
            use universe_sim::world::ship::ThrusterRole;
            let joined = |p: Vec3| plan.beams.iter().any(|b| b.a.distance(p) <= 1.0 || b.b.distance(p) <= 1.0);
            let none: &[universe_sim::world::ship::Thruster] = &[];
            for p in spec.into_iter().flat_map(|s| s.shape().nodes(Role::Gear)).map(|n| n.at.as_vec3()).chain(plan.pads.iter().copied()) {
                if let Some((q, _)) = cam.project(p) {
                    let col = if joined(p) { Color([0.4, 0.9, 1.0, 1.0]) } else { Color([0.4, 0.9, 1.0, 0.45]) };
                    let d = [Vec2::new(0.0, -6.0), Vec2::new(6.0, 0.0), Vec2::new(0.0, 6.0), Vec2::new(-6.0, 0.0)];
                    for i in 0..4 {
                        frame.hud_line(q + d[i], q + d[(i + 1) % 4], col);
                    }
                    frame.text_scaled(q + Vec2::new(8.0, 2.0), "PAD", col, 0.55);
                }
            }
            for t in spec.map_or(none, |s| s.thrusters.as_slice()).iter().filter(|t| matches!(t.role, ThrusterRole::Main | ThrusterRole::Lift)) {
                let p = t.at.as_vec3();
                let col = if joined(p) { Color([1.0, 0.65, 0.2, 1.0]) } else { Color([1.0, 0.65, 0.2, 0.45]) };
                seg(frame, p, p + t.push.as_vec3() * 2.0, col);
                if let Some((q, _)) = cam.project(p) {
                    frame.hud_box(q - Vec2::splat(3.0), Vec2::splat(6.0), col);
                }
            }
        }
    }
    for (k, &(a, b, profile)) in plan.lines.iter().enumerate() {
        let lit = interior.hover == Some(Hover::Line(k)) || picked.contains(&k);
        let hits = clash.get(k).map_or(&[][..], |c| c.as_slice());
        let hits = if interior.shown(layer::CLASHES) { hits } else { &[] };
        let col = if lit { PICKED } else if hits.is_empty() { PATH } else { CLASH };
        if interior.shown(layer::LINES) {
            seg(frame, plan.points[a].at, plan.points[b].at, col);
        }
        let edge = Color([col.0[0], col.0[1], col.0[2], 0.45]);
        let (pa, pb) = plan.axis(k);
        for [p, q] in room(pa, pb, profile).0.into_iter().filter(|_| interior.shown(layer::ROOMS)) {
            seg(frame, p, q, edge);
        }
        for p in hits {
            if let Some((q, _)) = cam.project(*p) {
                frame.hud_rect(q - Vec2::splat(1.5), Vec2::splat(3.0), CLASH);
            }
        }
    }
    if interior.tool == Tool::Path
        && let Some(a) = interior.from
        && let Some((pa, _)) = cam.project(plan.points[a].at)
    {
        // (To where a click would go: a point, a line's mark, or the plane's spot.)
        let to = match interior.hover {
            Some(Hover::Point(k)) => Some(plan.points[k].at),
            Some(Hover::Line(k)) => on_line(interior, &cam, interior.cursor, k).map(|s| s.0),
            None => on_plane(interior, &cam, plane, interior.cursor, interior.shift),
        };
        let end = to.and_then(|p| cam.project(p)).map_or(interior.cursor, |(q, _)| q);
        frame.hud_line(pa, end, PICKED.scale(0.6));
        // Its length and its slope (amber past a comfortable 35°, red past the walker's
        // 50°: too steep to walk).
        if let Some(b) = to {
            let (len, slope) = slope_of(plan.points[a].at, b);
            let col = if slope > 50.0 { CLASH } else if slope > 35.0 { Color([1.0, 0.65, 0.2, 1.0]) } else { PICKED };
            let what = if slope > 50.0 { "  TOO STEEP TO WALK" } else if slope > 2.0 { "  RAMP" } else { "" };
            frame.text_scaled(end + Vec2::new(12.0, 8.0), &format!("{len:.1} M  {slope:.0}°{what}"), col, 0.75);
        }
    }
    for (k, p) in plan.points.iter().enumerate() {
        if !interior.shown(3 + sort_index(p.name.as_deref())) {
            continue;
        }
        let Some((q, _)) = cam.project(p.at) else { continue };
        let lit = interior.hover == Some(Hover::Point(k)) || interior.from == Some(k) || interior.pick == Some(Hover::Point(k));
        let col = if lit { PICKED } else { sort(p.name.as_deref()).0 };
        frame.hud_rect(q - Vec2::splat(3.0), Vec2::splat(6.0), col);
        if let Some(name) = p.name.as_ref().filter(|_| interior.shown(layer::NAMES)) {
            // (Dimmer unless it's under the cursor: there are many.)
            frame.text_scaled(q + Vec2::new(6.0, -4.0), name, if lit { col } else { Color([col.0[0], col.0[1], col.0[2], 0.55]) }, 0.7);
        }
    }
    // The globe: the axes as the camera sees them (X red, Y green, Z blue; their far
    // ends faint); X clicked levels the view, Y squares it.
    {
        let (c, r) = globe(size);
        frame.hud_box(c - Vec2::splat(r), Vec2::splat(r * 2.0), PLANE.scale(0.8));
        let mut ends = globe_ends(&cam, size);
        ends.sort_by(|a, b| a.2.total_cmp(&b.2));
        for (a, q, _) in ends {
            let (col, name) = if a.x.abs() > 0.5 { (Color([1.0, 0.35, 0.35, 1.0]), "X") } else if a.y.abs() > 0.5 { (Color([0.4, 1.0, 0.45, 1.0]), "Y") } else { (Color([0.4, 0.6, 1.0, 1.0]), "Z") };
            let plus = a.x + a.y + a.z > 0.0;
            let hover = q.distance(interior.cursor) < 9.0;
            let col = if hover { PICKED } else if plus { col } else { Color([col.0[0], col.0[1], col.0[2], 0.45]) };
            if plus {
                frame.hud_line(c, q, col);
            }
            frame.hud_rect(q - Vec2::splat(6.0), Vec2::splat(12.0), Color([0.02, 0.06, 0.13, 1.0]));
            frame.hud_box(q - Vec2::splat(6.0), Vec2::splat(12.0), col);
            if plus {
                frame.text_scaled(q - Vec2::new(3.0, 4.0), name, col, 0.7);
            }
        }
    }
    // The reach check's findings: the floor got to (dots), where it drops away, and
    // a report (out of date once the plan's changed).
    if let Some((checked, found)) = interior.reach.as_ref().filter(|_| interior.shown(layer::REACH)) {
        let fresh = *checked == interior.plan;
        if fresh {
            for p in &found.floor {
                if let Some((q, _)) = cam.project(*p) {
                    frame.hud_rect(q - Vec2::splat(0.75), Vec2::splat(1.5), Color([0.4, 1.0, 0.55, 0.5]));
                }
            }
            for p in &found.drops {
                if let Some((q, _)) = cam.project(*p) {
                    frame.hud_line(q - Vec2::splat(2.5), q + Vec2::splat(2.5), Color([1.0, 0.65, 0.2, 0.8]));
                    frame.hud_line(q + Vec2::new(-2.5, 2.5), q + Vec2::new(2.5, -2.5), Color([1.0, 0.65, 0.2, 0.8]));
                }
            }
        }
        let count = |v: &[(usize, bool)]| (v.iter().filter(|x| x.1).count(), v.len());
        let (t_ok, t_all) = count(&found.tubes);
        let (d_ok, d_all) = count(&found.decks);
        let p_ok = found.points.iter().filter(|x| x.1).count();
        let mut lines = vec![
            if fresh { "REACH FROM THE HATCH".to_string() } else { "REACH: OUT OF DATE (RUN AGAIN)".to_string() },
            format!("WALLED TUBES {t_ok} OF {t_all}"),
            format!("DECKS {d_ok} OF {d_all}"),
            format!("SERVICE, DASH, DOORS {p_ok} OF {}", found.points.len()),
            format!("DROPS {}", found.drops.len()),
        ];
        let missed: Vec<String> = found.tubes.iter().filter(|x| !x.1).map(|x| format!("TUBE {}", x.0 + 1)).chain(found.points.iter().filter(|x| !x.1).map(|x| x.0.clone())).collect();
        if !missed.is_empty() {
            lines.push("NOT GOT TO:".into());
            lines.extend(missed.into_iter().take(10));
        }
        let (w, line) = (260.0, 13.0);
        let at = Vec2::new(size.x - 12.0 - LAYERS_WIDTH - 8.0 - w, size.y - 26.0 - (lines.len() as f32 * line + 10.0));
        frame.hud_rect(at, Vec2::new(w, lines.len() as f32 * line + 10.0), Color([0.02, 0.06, 0.13, 0.85]));
        frame.hud_box(at, Vec2::new(w, lines.len() as f32 * line + 10.0), PLANE.scale(1.5));
        for (k, l) in lines.iter().enumerate() {
            let col = if k == 0 { if fresh { PATH } else { PICKED } } else { LABEL };
            frame.text_scaled(at + Vec2::new(6.0, 5.0 + k as f32 * line), l, col, 0.7);
        }
    }
    draw_layers(frame, interior, layers_corner(size), interior.cursor);
    // The tool's panel: what it does and how, its actions, how much is drawn.
    {
        use crate::hud::{draw_cell, Lamp};
        let (p, c) = PANEL;
        frame.hud_rect(p, c, Color([0.02, 0.06, 0.13, 0.92]));
        frame.hud_box(p, c, PLANE.scale(1.5));
        let (title, help) = match interior.tool {
            Tool::Look => ("LOOK", "DRAG TO TURN IT, RIGHT-DRAG TO MOVE IT. CLICK A POINT OR A TUBE TO PICK IT (A TUBE IN A GROUP: THE GROUP); SHIFT-CLICK TUBES TO PICK MORE. GROUP THEM, WALL THEM OFF. DEL TAKES OUT WHAT'S UNDER THE CURSOR."),
            Tool::Path => ("PATH", "CLICK THE PLANE TO LAY A POINT, JOINED TO THE LAST ONE; CLICK A POINT TO START THERE, OR TO JOIN TO IT (THAT TUNNEL DONE AND PICKED). RIGHT-CLICK STOPS. DRAG THE PLANE'S GRIP (ITS NEAR RIGHT CORNER) UP OR DOWN."),
            Tool::Door => ("DOOR", "CLICK NEAR A WALLED TUBE'S END: CLOSED, A HATCH, OPEN, IN TURN. CLICK ALONG A TUBE: A HATCH IN THE WALL FACING YOU (AGAIN: GONE). HATCHES SLIDE OPEN AS YOU COME NEAR. SET THEIR SHAPE, SIZE AND SLIDE BELOW."),
            Tool::Modules => ("MODULES", "PICK ONE, CLICK THE PLANE: IT STANDS THERE. CLICK ONE IN THE VIEW TO PICK IT. STRETCHED, IT KEEPS ITS VOLUME."),
            Tool::Frame => ("FRAME", "JOINT TO JOINT, OR TRUSS: TWO CORNERS. MOUNT ALL, AUTO-SIZE (MIX: ANY MATERIAL), BRACE WHAT'S OVER."),
        };
        frame.text(p + Vec2::new(8.0, 8.0), title, LABEL);
        let mut y = p.y + 28.0;
        // (MODULES: its hint on the hint line, the panel for the list and the sheet.)
        let help = if interior.tool == Tool::Modules { "" } else { help };
        for line in crate::fmt::wrap(help, ((c.x - 16.0) / 8.0 * 1.25) as usize) {
            frame.text_scaled(Vec2::new(p.x + 8.0, y), &line, LABEL.scale(0.85), 0.8);
            y += 12.0;
        }
        // FRAME: the stock (lit: what new members, or the picked one, are cut from),
        // the case shown, how each case goes, what isn't carried, its members' mass.
        if interior.tool == Tool::Frame {
            frame.text_scaled(Vec2::new(p.x + 8.0, p.y + 72.0), "STOCK                     KG/M  YIELD", LABEL.scale(0.7), 0.6);
            if stocks().len() > STOCK_ROWS {
                let shown = format!("{}-{} OF {} (WHEEL)", interior.stock_top + 1, (interior.stock_top + STOCK_ROWS).min(stocks().len()), stocks().len());
                frame.text_scaled(Vec2::new(p.x + c.x - 8.0 - shown.len() as f32 * 4.8, p.y + 62.0), &shown, LABEL.scale(0.6), 0.6);
            }
            for (n, (k, st)) in stocks().iter().enumerate().skip(interior.stock_top).take(STOCK_ROWS).enumerate() {
                let (q, qc) = stock_row(n);
                let lit = interior.stock == k;
                if lit {
                    frame.hud_rect(q, qc, PICKED.scale(0.18));
                }
                let col = if lit || inside((q, qc), interior.cursor) { PICKED } else { LABEL.scale(0.75) };
                let name: String = st.name.chars().take(24).collect();
                frame.text_scaled(Vec2::new(q.x + 4.0, q.y + 2.0), &name, col, 0.6);
                let figures = format!("{:>5.1} {:>5.0} MPA", st.per_metre, st.material.yield_strength / 1e6);
                frame.text_scaled(Vec2::new(q.x + qc.x - figures.len() as f32 * 4.8, q.y + 2.0), &figures, col, 0.6);
            }
            frame.text_scaled(Vec2::new(p.x + 8.0, p.y + 146.0), "LOAD CASE SHOWN", LABEL.scale(0.7), 0.6);
            for (r, name, a) in panel_buttons(Tool::Frame) {
                let lamp = if a == Action::AutoSize && interior.sizing.is_some() {
                    Lamp::Busy
                } else if inside(r, interior.cursor) || matches!(a, Action::Case(c) if c == interior.case) || (a == Action::Mix && interior.mix) || (a == Action::Truss && interior.truss_mode) {
                    Lamp::On
                } else {
                    Lamp::Off
                };
                let off = (a == Action::Remove && interior.beam_pick.is_none()) || (a == Action::AutoSize && plan.beams.is_empty());
                let name = if a == Action::AutoSize && interior.sizing.is_some() { "SIZING..." } else { name };
                draw_cell(frame, r.0, r.1, "", name, if off { Lamp::Unavailable } else { lamp });
            }
            // (The next truss's depth and its stations' spacing: a slider each.)
            for (k, (at, len)) in sliders(Tool::Frame).into_iter().enumerate() {
                let (name, v) = if k == 0 { ("TRUSS DEPTH", interior.depth) } else { ("SPACING", interior.spacing) };
                let col = if interior.slider == Some(k) { PICKED } else { LABEL };
                frame.text_scaled(Vec2::new(p.x + 8.0, at.y), &format!("{name} {v:.1}"), col, 0.6);
                frame.hud_line(at + Vec2::new(0.0, len.y * 0.5), at + Vec2::new(len.x, len.y * 0.5), col.scale(0.6));
                let (lo, hi) = slider_range(Tool::Frame);
                let x = at.x + (v - lo) / (hi - lo) * len.x;
                frame.hud_rect(Vec2::new(x - 3.0, at.y - 2.0), Vec2::new(6.0, len.y + 4.0), col);
            }
            let bearing = interior.bearing.as_ref().filter(|(p, _)| *p == interior.plan).map(|(_, b)| b.clone());
            let mut y = p.y + 180.0;
            match &bearing {
                _ if plan.beams.is_empty() => {
                    frame.text_scaled(Vec2::new(p.x + 8.0, y), "NO MEMBERS YET", LABEL.scale(0.7), 0.65);
                }
                None => {
                    frame.text_scaled(Vec2::new(p.x + 8.0, y), "WORKING IT OUT...", PICKED, 0.65);
                }
                Some(b) => {
                    for (k, (name, result)) in b.cases.iter().enumerate() {
                        let felt = b.felt.get(k).copied().unwrap_or(0.0) / STANDARD_G;
                        let (text, col) = match result {
                            Ok(c) => {
                                let o = &c.members;
                                let worst = o.iter().map(|o| o.work.design).fold(0.0, f64::max);
                                let broken = o.iter().filter(|o| o.broken.is_some()).count();
                                let col = if broken > 0 { CLASH } else if worst > 1.0 { Color([1.0, 0.65, 0.2, 1.0]) } else { Color([0.4, 1.0, 0.5, 1.0]) };
                                let what = match (broken, c.falls_apart) {
                                    (_, true) => format!("{broken} BREAK, IT FALLS APART"),
                                    (0, _) => format!("WORST AT {:.0}% OF ITS LIMIT", worst * 100.0),
                                    _ => format!("{broken} BREAK"),
                                };
                                (format!("{name} {felt:.1} G: {what}"), col)
                            }
                            Err(e) => (format!("{name}: {e}"), CLASH),
                        };
                        frame.text_scaled(Vec2::new(p.x + 8.0, y), &text, col, 0.6);
                        y += 11.0;
                    }
                    y += 3.0;
                    for l in b.loose.iter().take(3) {
                        frame.text_scaled(Vec2::new(p.x + 8.0, y), l, CLASH.scale(0.9), 0.6);
                        y += 11.0;
                    }
                    frame.text_scaled(Vec2::new(p.x + 8.0, p.y + 250.0), &format!("{} MEMBERS, {} JOINTS, {:.2} T", plan.beams.len(), b.joints.len(), b.mass / 1000.0), Color([0.4, 1.0, 0.5, 1.0]), 0.65);
                }
            }
        }
        // MODULES: the fit, a row each (placed: a mark; red: it clashes), what's
        // placed of it, the picked one's size.
        if interior.tool == Tool::Modules {
            let clash_of = |id: &str| interior.block_clash.as_ref().filter(|(b, _)| *b == plan.blocks).and_then(|(b, c)| b.iter().position(|x| x.id == id).map(|k| c[k])).unwrap_or(false);
            let hull = interior.spec().is_some();
            let head = if hull { "THE SHIP'S FIT              MASS   VOLUME" } else { "ALL EQUIPMENT (ANY NUMBER) MASS   VOLUME" };
            frame.text_scaled(Vec2::new(p.x + 8.0, p.y + 29.0), head, LABEL.scale(0.7), 0.6);
            // (What's placed, of everything: its mass and volume.)
            let fitted_of = |b: &Block| interior.fit.iter().find(|f| f.id == kind(&b.id));
            let (mass, volume) = plan.blocks.iter().filter_map(fitted_of).fold((0.0, 0.0), |(m, v), f| (m + f.mass, v + f.volume));
            let placed = plan.blocks.len();
            for (n, (k, f)) in interior.fit.iter().enumerate().skip(interior.fit_top).take(FIT_ROWS).enumerate() {
                let (q, qc) = module_row(n);
                let copies = plan.blocks.iter().filter(|b| kind(&b.id) == f.id).count();
                let block = plan.blocks.iter().find(|b| kind(&b.id) == f.id);
                let lit = interior.module == Some(k) || interior.module_hover == Some(k) || inside((q, qc), interior.cursor);
                if interior.module == Some(k) {
                    frame.hud_rect(q, qc, PICKED.scale(0.18));
                }
                let col = if block.is_some_and(|b| clash_of(&b.id)) { CLASH } else if lit { PICKED } else if block.is_some() { MODULE } else { LABEL.scale(0.6) };
                if block.is_some() {
                    frame.hud_rect(Vec2::new(q.x + 2.0, q.y + 3.0), Vec2::splat(5.0), col);
                } else {
                    frame.hud_box(Vec2::new(q.x + 2.0, q.y + 3.0), Vec2::splat(5.0), col);
                }
                let name: String = if copies > 1 { format!("{copies}X {}", f.name) } else { f.name.clone() }.chars().take(22).collect();
                frame.text_scaled(Vec2::new(q.x + 11.0, q.y + 2.0), &name, col, 0.6);
                let figures = if f.volume < 10.0 { format!("{:>5.2} T {:>5.1} M3", f.mass / 1000.0, f.volume) } else { format!("{:>5.2} T {:>5.0} M3", f.mass / 1000.0, f.volume) };
                frame.text_scaled(Vec2::new(q.x + qc.x - figures.len() as f32 * 4.8, q.y + 2.0), &figures, col, 0.6);
            }
            // (The sheet: the row under the cursor, else the module under it in the
            // view, else the one picked.)
            let row = (0..FIT_ROWS).find(|&n| interior.fit_top + n < interior.fit.len() && inside(module_row(n), interior.cursor)).map(|n| interior.fit_top + n);
            if let Some(f) = row.or(interior.module_hover).or(interior.module).and_then(|k| interior.fit.get(k)) {
                let block = interior.block.and_then(|n| plan.blocks.get(n)).filter(|b| kind(&b.id) == f.id);
                draw_sheet(frame, interior, f, block);
            }
            let text = if hull {
                let total: f32 = interior.fit.iter().map(|f| f.volume).sum();
                format!("PLACED {placed} OF {}: {volume:.0} OF {total:.0} M3, {:.1} T", interior.fit.len(), mass / 1000.0)
            } else {
                format!("PLACED {placed}: {volume:.0} M3, {:.1} T", mass / 1000.0)
            };
            frame.text_scaled(Vec2::new(p.x + 8.0, p.y + 245.0), &text, MODULE, 0.65);
            // (Scrolled: where in the list it is.)
            if interior.fit.len() > FIT_ROWS {
                let shown = format!("{}-{} OF {} (WHEEL)", interior.fit_top + 1, (interior.fit_top + FIT_ROWS).min(interior.fit.len()), interior.fit.len());
                frame.text_scaled(Vec2::new(p.x + c.x - 8.0 - shown.len() as f32 * 4.8, p.y + 10.0), &shown, LABEL.scale(0.6), 0.6);
            }
            let picked_block = interior.block.and_then(|n| plan.blocks.get(n));
            for (k, (at, len)) in sliders(Tool::Modules).into_iter().enumerate() {
                let value = picked_block.map(|b| if k == 0 { b.size.x } else { b.size.y });
                let name = if k == 0 { "WIDTH" } else { "HEIGHT" };
                let col = if value.is_none() { LABEL.scale(0.4) } else if interior.slider == Some(k) { PICKED } else { LABEL };
                frame.text_scaled(Vec2::new(p.x + 8.0, at.y), &value.map_or(name.to_string(), |v| format!("{name} {v:.1}")), col, 0.7);
                frame.hud_line(at + Vec2::new(0.0, len.y * 0.5), at + Vec2::new(len.x, len.y * 0.5), col.scale(0.6));
                if let Some(v) = value {
                    let (lo, hi) = slider_range(Tool::Modules);
                    let x = at.x + (v - lo) / (hi - lo) * len.x;
                    frame.hud_rect(Vec2::new(x - 3.0, at.y - 2.0), Vec2::new(6.0, len.y + 4.0), col);
                }
            }
            if let Some(b) = picked_block {
                frame.text_scaled(Vec2::new(p.x + 8.0, p.y + 298.0), &format!("LENGTH {:.1} M  ({} UP)", b.size.z.max(b.size.x), format_args!("{:.1} M", b.at.y - b.size.y * 0.5 - lo.y)), LABEL, 0.65);
            } else if interior.module.is_some() {
                frame.text_scaled(Vec2::new(p.x + 8.0, p.y + 298.0), "NOT PLACED: CLICK THE PLANE", PICKED, 0.65);
            }
            for (r, name, a) in panel_buttons(Tool::Modules) {
                let lamp = if inside(r, interior.cursor) || (a == Action::SnapFloor && interior.snap_floor) { Lamp::On } else { Lamp::Off };
                let off = matches!(a, Action::Turn | Action::Remove) && picked_block.is_none();
                draw_cell(frame, r.0, r.1, "", name, if off { Lamp::Unavailable } else { lamp });
            }
        }
        // (Else: the picked line, the cross-section, the tool's actions.)
        if !matches!(interior.tool, Tool::Modules | Tool::Frame) {
            let pick = match interior.pick {
                Some(Hover::Point(k)) => match &plan.points[k].name {
                    Some(n) => format!("PICKED: {n} (THE HULL'S: STAYS)"),
                    None => format!("PICKED: A POINT {:.1} M UP", plan.points[k].at.y - lo.y),
                },
                Some(Hover::Line(_)) if picked.len() > 1 => {
                    let len: f32 = picked.iter().map(|&k| plan.points[plan.lines[k].0].at.distance(plan.points[plan.lines[k].1].at)).sum();
                    let group = plan.group_of(picked[0]).filter(|&g| plan.groups[g].lines.len() == picked.len() && picked.iter().all(|&k| plan.group_of(k) == Some(g)));
                    match group {
                        Some(g) => format!("A GROUP OF {} {len:.0} M{}", picked.len(), if plan.groups[g].walled { "  WALLED" } else { "" }),
                        None => format!("{} LINES PICKED {len:.0} M", picked.len()),
                    }
                }
                Some(Hover::Line(k)) => {
                    let (a, b, profile) = plan.lines[k];
                    let len = plan.points[a].at.distance(plan.points[b].at);
                    let hits = clash.get(k).map_or(0, |c| c.len());
                    let slope = slope_of(plan.points[a].at, plan.points[b].at).1;
                    format!("A LINE {len:.1} M {slope:.0}°{}{}", if slope > 50.0 { " TOO STEEP" } else { "" }, if hits > 0 { "  CLASHES" } else { "" }) + &if profile.section == Section::Line { String::new() } else { format!("  {} {:.1}X{:.1}", profile.section.name(), profile.width, profile.height) }
                }
                None => {
                    let len: f32 = plan.lines.iter().map(|&(a, b, _)| plan.points[a].at.distance(plan.points[b].at)).sum();
                    let bad = clash.iter().filter(|c| !c.is_empty()).count();
                    format!("{} LINES {:.0} M  {bad} CLASH", plan.lines.len(), len.max(0.0))
                }
            };
            frame.text_scaled(Vec2::new(p.x + 8.0, p.y + 126.0), &pick, PICKED.scale(0.9), 0.8);
            if interior.tool == Tool::Path {
                frame.text_scaled(Vec2::new(p.x + 8.0, p.y + 138.0), &format!("PLANE {:.1} M UP", plane - lo.y), PATH, 0.8);
            }
            // (The cross-section lit: the picked line's in LOOK, else the one new lines get.)
            let h = interior.hatch;
            let now = match (interior.tool, interior.pick) {
                (Tool::Look, Some(Hover::Line(k))) => Some(plan.lines[k].2),
                (Tool::Look, _) => None,
                (Tool::Door, _) => Some(Profile { section: h.section, width: h.width, height: h.height, stand: false }),
                _ => Some(interior.profile),
            };
            let what = match interior.tool {
                Tool::Look => "THE PICKED LINE'S CROSS-SECTION",
                Tool::Door => "NEW HATCHES: SHAPE, SIZE, SLIDE",
                Tool::Path | Tool::Modules | Tool::Frame => "NEW LINES' CROSS-SECTION",
            };
            frame.text_scaled(Vec2::new(p.x + 8.0, p.y + 162.0), what, LABEL.scale(0.8), 0.7);
            if interior.tool == Tool::Path {
                frame.text_scaled(Vec2::new(p.x + 8.0, p.y + 295.0), "ROOM PRESETS (ROOMS: DOORWAYS AT THEIR ENDS)", LABEL.scale(0.8), 0.7);
            }
            // Its width and height: a slider each, its knob where it is.
            for (k, (at, len)) in sliders(Tool::Path).into_iter().enumerate() {
                let value = now.map(|q| if k == 0 { q.width } else { q.height });
                let name = if k == 0 { "WIDTH" } else { "HEIGHT" };
                let col = if value.is_none() { LABEL.scale(0.4) } else if interior.slider == Some(k) { PICKED } else { LABEL };
                frame.text_scaled(Vec2::new(p.x + 8.0, at.y), &value.map_or(name.to_string(), |v| format!("{name} {v:.1}")), col, 0.7);
                frame.hud_line(at + Vec2::new(0.0, len.y * 0.5), at + Vec2::new(len.x, len.y * 0.5), col.scale(0.6));
                if let Some(v) = value {
                    let x = at.x + (v - ROOM_MIN) / (ROOM_MAX - ROOM_MIN) * len.x;
                    frame.hud_rect(Vec2::new(x - 3.0, at.y - 2.0), Vec2::new(6.0, len.y + 4.0), col);
                }
            }
            let walled = !picked.is_empty() && picked.iter().all(|&k| plan.group_of(k).is_some_and(|g| plan.groups[g].walled));
            let grouped = picked.iter().any(|&k| plan.group_of(k).is_some());
            for (r, name, a) in panel_buttons(interior.tool) {
                let lamp = if inside(r, interior.cursor)
                    || matches!(a, Action::Section(s) if now.is_some_and(|p| p.section == s))
                    || matches!(a, Action::Preset(k) if now == Some(PRESETS[k].1))
                    || (a == Action::Stand && now.is_some_and(|p| p.stand))
                    || (a == Action::SnapFloor && interior.snap_floor)
                    || matches!(a, Action::Slide(sl) if interior.tool == Tool::Door && h.slide == sl)
                {
                    Lamp::On
                } else {
                    Lamp::Off
                };
                let off = match a {
                    Action::Remove => interior.pick.is_none(),
                    Action::Section(_) | Action::Stand => now.is_none(),
                    Action::Group | Action::Wall => picked.is_empty(),
                    Action::Ungroup => !grouped,
                    _ => false,
                };
                let name = if a == Action::Wall && walled { "OPEN UP" } else { name };
                // (Standing on its line: the line its floor; else its axis.)
                let name = if a == Action::Stand { if now.is_some_and(|p| !p.stand) { "AXIS ON LINE" } else { "FLOOR ON LINE" } } else { name };
                draw_cell(frame, r.0, r.1, "", name, if off { Lamp::Unavailable } else { lamp });
            }
        }
    }
    // The budgets (MODULES, FRAME): under the panel, each met in green, short in
    // amber; what fails, in red.
    if matches!(interior.tool, Tool::Modules | Tool::Frame) && !plan.blocks.is_empty() {
        let frame_mass: f64 = plan.beams.iter().filter_map(|b| stocks().iter().find(|s| s.key == b.stock).map(|s| s.per_metre * f64::from(b.a.distance(b.b)))).sum();
        let b = budget(interior, frame_mass);
        let (pp, pc) = PANEL;
        let rows = b.lines.len() + b.faults.len().min(4);
        let (p, c) = (Vec2::new(pp.x, pp.y + pc.y + 6.0), Vec2::new(pc.x + 120.0, 18.0 + rows as f32 * 10.0));
        frame.hud_rect(p, c, Color([0.02, 0.06, 0.13, 0.88]));
        frame.hud_box(p, c, PLANE.scale(1.2));
        frame.text_scaled(p + Vec2::new(6.0, 4.0), "BUDGETS AND CHECKS", LABEL.scale(0.8), 0.6);
        let mut y = p.y + 15.0;
        for (text, ok) in &b.lines {
            let col = if *ok { Color([0.4, 1.0, 0.5, 0.95]) } else { Color([1.0, 0.7, 0.2, 1.0]) };
            frame.text_scaled(Vec2::new(p.x + 6.0, y), text, col, 0.55);
            y += 10.0;
        }
        for f in b.faults.iter().take(4) {
            frame.text_scaled(Vec2::new(p.x + 6.0, y), f, CLASH, 0.55);
            y += 10.0;
        }
    }
    // A message for a while (saved, opened), under the toolbar.
    if let Some((text, _)) = &interior.message {
        frame.text_scaled(Vec2::new(button(2).0.x, 52.0), text, PICKED, 0.7);
    }
    // A dialog: its question, its choices (the one under the cursor lit).
    if let Some(d) = &interior.dialog {
        use crate::hud::{draw_cell, Lamp};
        let choices = dialog_choices(d);
        let ((p, c), rows) = dialog_rows(size, choices.len());
        frame.hud_rect(Vec2::ZERO, size, Color([0.0, 0.0, 0.0, 0.45]));
        frame.hud_rect(p, c, Color([0.02, 0.06, 0.13, 0.98]));
        frame.hud_box(p, c, PICKED);
        let title = match d {
            Dialog::New => "NEW DESIGN: IN WHICH HULL?",
            Dialog::Open(list) if list.is_empty() => "NO SAVED DESIGNS YET",
            Dialog::Open(_) => "OPEN A SAVED DESIGN",
        };
        frame.text(p + Vec2::new(10.0, 10.0), title, PICKED);
        for (r, name) in rows.iter().zip(&choices) {
            let lamp = if inside(*r, interior.cursor) { Lamp::On } else { Lamp::Off };
            draw_cell(frame, r.0, r.1, "", name, lamp);
        }
    }
    // Closing with unsaved changes: asked first.
    if interior.confirm {
        use crate::hud::{draw_cell, Lamp};
        let ((p, c), buttons) = confirm_box(size);
        frame.hud_rect(Vec2::ZERO, size, Color([0.0, 0.0, 0.0, 0.45]));
        frame.hud_rect(p, c, Color([0.02, 0.06, 0.13, 0.98]));
        frame.hud_box(p, c, PICKED);
        frame.text(p + Vec2::new(10.0, 10.0), "UNSAVED CHANGES", PICKED);
        frame.text_scaled(p + Vec2::new(10.0, 32.0), "CLOSE THE STUDIO WITHOUT SAVING THIS PLAN?", LABEL, 0.8);
        for (k, (key, name)) in [("S", "SAVE AND CLOSE"), ("D", "DISCARD"), ("ESC", "KEEP WORKING")].into_iter().enumerate() {
            let (bp, bc) = buttons[k];
            draw_cell(frame, bp, bc, key, name, if inside((bp, bc), interior.cursor) { Lamp::On } else { Lamp::Off });
        }
    }
}
