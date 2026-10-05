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
const LAYERS: [(&str, Option<usize>, Option<Color>); 23] = [
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
    block_clash: Option<(Vec<Block>, Vec<bool>)>,
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
fn block_clashes(mesh: &universe_sim::world::walk::WalkMesh, i: &Interior) -> Vec<bool> {
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
        let hull = marks.into_iter().flat_map(|x| marks.into_iter().flat_map(move |y| marks.into_iter().map(move |z| Vec3::new(x, y, z)))).any(|m| {
            // (A hair off the hull's middle line: rays straight down it slip through the
            // seam between its mirrored halves.)
            let p = b.at + m * h + Vec3::new(0.0137, 0.0, 0.0071);
            let flat = DVec2::new(f64::from(p.x), f64::from(p.z));
            // (Outside: not closed in by the hull's skin all round, any face of it.)
            in_material(mesh, flat, f64::from(p.y)) || !enclosed(mesh, flat, f64::from(p.y), 0.0, true)
        });
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
    name: String,
    mass: f64,
    volume: f32,
    round: bool,
    size: Vec3,
}

/// The ore bay's id among the placed blocks.
const HOLD: &str = "HOLD";

/// What `spec` is fitted with, and its ore bay, to be placed: as its hull's record
/// in the registry fits it (found by its model), each item's mass, volume and size
/// from its equipment record; a hull the registry doesn't describe, as the game fits
/// it.
fn fit_of(spec: &universe_sim::world::ship::ClassSpec) -> Vec<Fitted> {
    use universe_sim::world::registry::{registry, EquipmentFunction};
    let reg = registry();
    let Some(hull) = spec.visual.as_deref().and_then(|v| reg.hulls.iter().find(|h| h.model.as_deref() == Some(v))) else { return game_fit(spec) };
    let content = universe_sim::world::content::content();
    let mut out: Vec<Fitted> = hull.fit.iter().filter_map(|f| {
        let e = reg.equipment.iter().find(|e| e.identity.key == f.item)?;
        let p = &e.physical;
        let round = matches!(e.function, EquipmentFunction::Tank(_));
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
        Some(Fitted { id: f.slot.clone(), name: e.identity.name.to_uppercase(), mass: p.mass.unwrap_or(0.0), volume, round, size })
    }).collect();
    // (Its ore bay: its hold; broad and low, under doors.)
    if let Some(volume) = hull.capacity.hold_volume.filter(|v| *v > 0.0).map(|v| v as f32) {
        let a = Vec3::new(1.2, 0.8, 1.0);
        out.push(Fitted { id: HOLD.into(), name: "ORE BAY".into(), mass: 0.0, volume, round: false, size: a * (volume / (a.x * a.y * a.z)).cbrt() });
    }
    out
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
        Fitted { id: slot.clone(), name: m.name.to_uppercase(), mass: m.mass, volume, round, size }
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
        Interior { yaw: 0.9, pitch: 0.35, snap_floor: true, ..Default::default() }
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
        self.fit.iter().any(|f| f.id == id && f.round)
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
        Interior { yaw, pitch, ..Default::default() }
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
    pub fn sync(&mut self, key: &str, shape: &universe_sim::world::shape::Shape, deckplans: &mut Vec<universe_sim::world::deckplan::DeckPlan>, dt: f32) {
        self.seed(key, shape);
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
            self.decks = match (&now, shape.walk.as_ref()) {
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

    /// The saved plan opened over this one (an undo step: UNDO brings this back).
    fn open(&mut self) {
        // (Its decks too, put in the deck studio when next in step.)
        if let Some(d) = self.read_decks() {
            self.saved_decks = Some(Some(d.clone()));
            self.load_decks = Some(d);
        }
        match self.read_saved() {
            Some(plan) => {
                self.undo.push(std::mem::replace(&mut self.plan, plan));
                self.redo.clear();
                self.saved = Some(self.plan.clone());
                self.pick = None;
                self.more.clear();
                self.from = None;
                self.message = Some(("OPENED THE SAVED PLAN (UNDO TO GO BACK)".into(), 4.0));
            }
            None => self.message = Some(("NO SAVED PLAN FOR THIS HULL YET".into(), 4.0)),
        }
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
            modules: plan.blocks.iter().filter_map(|b| self.fit.iter().find(|f| f.id == b.id).map(|f| (b.at, b.size, f.round, f.name.clone()))).collect(),
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
    pub fn refit(&mut self, spec: &universe_sim::world::ship::ClassSpec) {
        self.fit = fit_of(spec);
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

    fn seed(&mut self, key: &str, shape: &universe_sim::world::shape::Shape) {
        if self.plan.hull == key {
            return;
        }
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
        self.plan = Plan { hull: key.into(), points, lines: Vec::new(), groups: Vec::new(), ends: Vec::new(), doors: Vec::new(), blocks: Vec::new() };
        // The plan saved for this hull, if there is one (its hull's own points where
        // the model has them now).
        if let Some(saved) = self.read_saved() {
            self.plan = saved;
        }
        self.saved = Some(self.plan.clone());
        // Its saved decks put in the deck studio (none saved: what's there is new).
        let decks = self.read_decks();
        self.saved_decks = Some(decks.clone());
        self.load_decks = decks;
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
const TOOLBAR: [(&str, &str, Tool); 4] = [("V", "LOOK", Tool::Look), ("P", "PATH", Tool::Path), ("D", "DOOR", Tool::Door), ("M", "MODULES", Tool::Modules)];

/// Where toolbar button `k` is (one row along the top).
fn button(k: usize) -> (Vec2, Vec2) {
    (Vec2::new(12.0 + k as f32 * 94.0, 30.0), Vec2::new(90.0, 16.0))
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

/// SAVE (0) and OPEN (1), after WALK HERE.
fn file_button(k: usize) -> (Vec2, Vec2) {
    button(TOOLBAR.len() + HISTORY.len() + 1 + k)
}

/// REACH, after SAVE and OPEN.
fn reach_button() -> (Vec2, Vec2) {
    button(TOOLBAR.len() + HISTORY.len() + 3)
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
    /// The picked lines made a group; their groups broken up; walled off (or open).
    Group,
    Ungroup,
    Wall,
}

/// The panel's sliders: the cross-section's width (0) and height (1) (a module's,
/// lower down, under its list), each its track (where, its size).
fn sliders(tool: Tool) -> [(Vec2, Vec2); 2] {
    let (p, c) = PANEL;
    let top = if tool == Tool::Modules { 264.0 } else { 202.0 };
    [0.0, 22.0].map(|dy| (Vec2::new(p.x + 92.0, p.y + top + dy), Vec2::new(c.x - 100.0, 10.0)))
}

/// A slider's range (m): a room's, or a module's (a hold is broad).
fn slider_range(tool: Tool) -> (f32, f32) {
    if tool == Tool::Modules { (0.3, 16.0) } else { (ROOM_MIN, ROOM_MAX) }
}

/// The MODULES panel's list: row `k`'s place (the fit, one a row).
fn module_row(k: usize) -> (Vec2, Vec2) {
    let (p, c) = PANEL;
    (Vec2::new(p.x + 6.0, p.y + 82.0 + k as f32 * 10.5), Vec2::new(c.x - 12.0, 10.5))
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
    interior.refresh(&spec.key, spec.shape());
    interior.sync(&spec.key, spec.shape(), &mut app.deckplans, 0.0);
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
    if input.pressed(KeyCode::Escape) {
        return !interior.close();
    }
    // SAVE (CTRL+S) and OPEN (CTRL+O).
    let clicked = |b: (Vec2, Vec2)| input.button_pressed(MouseButton::Left) && inside(b, input.cursor);
    if (ctrl && input.pressed(KeyCode::KeyS)) || clicked(file_button(0)) {
        interior.save();
        return true;
    }
    if (ctrl && input.pressed(KeyCode::KeyO)) || clicked(file_button(1)) {
        interior.open();
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
    let stay = input_plan(app, ctx, interior);
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
        && let Some(mesh) = app.ship.spec().shape().walk.as_ref()
    {
        interior.checked = Some((interior.plan.clone(), clashes(mesh, &interior.plan)));
    }
    // The modules checked too: none in the hull's own material, out of it, or in a
    // tube's room.
    if interior.block_clash.as_ref().is_none_or(|(b, _)| *b != interior.plan.blocks)
        && let Some(mesh) = app.ship.spec().shape().walk.as_ref()
    {
        let found = block_clashes(mesh, interior);
        interior.block_clash = Some((interior.plan.blocks.clone(), found));
    }
    stay
}

fn input_plan(app: &mut App, ctx: &Context, interior: &mut Interior) -> bool {
    let input = &ctx.input;
    let spec = app.ship.spec();
    let Some(h) = interior.hull(&spec.key) else { return true };
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
    for (key, t) in [(KeyCode::KeyV, Tool::Look), (KeyCode::KeyP, Tool::Path), (KeyCode::KeyD, Tool::Door), (KeyCode::KeyM, Tool::Modules)] {
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
        // (The module picked: a row of the list; its width or height stretched, its
        // length then what keeps its volume; turned; taken out.)
        Tool::Modules => {
            if pressed && let Some(k) = (0..interior.fit.len()).find(|&k| inside(module_row(k), cursor)) {
                interior.module = Some(k);
                return true;
            }
            if let Some(f) = interior.module.and_then(|k| interior.fit.get(k)).cloned()
                && let Some(b) = interior.plan.blocks.iter_mut().find(|b| b.id == f.id)
            {
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
                && let Some(f) = interior.module.and_then(|k| interior.fit.get(k))
            {
                let id = f.id.clone();
                interior.plan.blocks.retain(|b| b.id != id);
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
    interior.hover = if interior.tool == Tool::Modules { None } else { hover_at(interior, &cam, cursor) };
    // (MODULES: the nearest placed one under the cursor.)
    interior.module_hover = None;
    if interior.tool == Tool::Modules && interior.shown(layer::MODULES) && !inside(PANEL, cursor) {
        let ray = cam.ray(cursor);
        interior.module_hover = interior.plan.blocks.iter().filter_map(|b| b.hit(cam.eye, ray).map(|t| (b, t))).min_by(|a, b| a.1.total_cmp(&b.1)).and_then(|(b, _)| interior.fit.iter().position(|f| f.id == b.id));
    }
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
    if input.button_pressed(MouseButton::Right) && interior.from.is_some() {
        interior.from = None;
        interior.stopped = true;
    }
    if !input.button_down(MouseButton::Right) {
        interior.stopped = false;
    }
    // The plane's handle (laying paths): held, it goes up and down with the mouse.
    let plane = plane_of(interior, &h);
    if matches!(interior.tool, Tool::Path | Tool::Modules) {
        interior.map_hollow(plane);
    }
    if pressed && matches!(interior.tool, Tool::Path | Tool::Modules) && cam.project(plane_handle(&cam, &h, plane)).is_some_and(|(q, _)| q.distance(cursor) < 10.0) {
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
            // A module under the cursor: picked. Else the one picked put where the
            // cursor is on the work plane, standing on it (if it's there already:
            // moved).
            Tool::Modules => {
                if let Some(k) = interior.module_hover.filter(|&k| Some(k) != interior.module) {
                    interior.module = Some(k);
                } else if let Some(f) = interior.module.and_then(|k| interior.fit.get(k)).cloned()
                    && let Some(at) = on_plane(interior, &cam, plane, cursor, false)
                {
                    let size = interior.plan.blocks.iter().find(|b| b.id == f.id).map_or(f.size, |b| b.size);
                    let block = Block { id: f.id.clone(), at: at + Vec3::Y * size.y * 0.5, size };
                    match interior.plan.blocks.iter_mut().find(|b| b.id == f.id) {
                        Some(b) => *b = block,
                        None => interior.plan.blocks.push(block),
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
        {
            let (p, c) = walk_button();
            let lamp = if interior.walk_armed || inside((p, c), interior.cursor) { Lamp::On } else { Lamp::Off };
            draw_cell(frame, p, c, "F", "WALK HERE", lamp);
        }
        {
            let (p, c) = reach_button();
            let lamp = if interior.reach_job.is_some() { Lamp::Busy } else if inside((p, c), interior.cursor) { Lamp::On } else { Lamp::Off };
            draw_cell(frame, p, c, "", if interior.reach_job.is_some() { "CHECKING" } else { "REACH" }, lamp);
        }
        for (k, (key, name)) in [("^S", if interior.unsaved() { "SAVE *" } else { "SAVE" }), ("^O", "OPEN")].into_iter().enumerate() {
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
    if matches!(interior.tool, Tool::Path | Tool::Modules) {
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
            // (MODULES: the picked one where a click would put it, faint.)
            None if interior.tool == Tool::Modules => {
                if interior.module_hover.is_none()
                    && let Some(f) = interior.module.and_then(|k| interior.fit.get(k))
                    && let Some(at) = on_plane(interior, &cam, plane, interior.cursor, false)
                {
                    let size = interior.plan.blocks.iter().find(|b| b.id == f.id).map_or(f.size, |b| b.size);
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
        let Some(k) = interior.fit.iter().position(|f| f.id == b.id) else { continue };
        let f = &interior.fit[k];
        let lit = interior.tool == Tool::Modules && (interior.module == Some(k) || interior.module_hover == Some(k));
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
        };
        frame.text(p + Vec2::new(8.0, 8.0), title, LABEL);
        let mut y = p.y + 28.0;
        for line in crate::fmt::wrap(help, ((c.x - 16.0) / 8.0 * 1.25) as usize) {
            frame.text_scaled(Vec2::new(p.x + 8.0, y), &line, LABEL.scale(0.85), 0.8);
            y += 12.0;
        }
        // MODULES: the fit, a row each (placed: a mark; red: it clashes), what's
        // placed of it, the picked one's size.
        if interior.tool == Tool::Modules {
            let clash_of = |id: &str| interior.block_clash.as_ref().filter(|(b, _)| *b == plan.blocks).and_then(|(b, c)| b.iter().position(|x| x.id == id).map(|k| c[k])).unwrap_or(false);
            frame.text_scaled(Vec2::new(p.x + 8.0, p.y + 72.0), "THE SHIP'S FIT              MASS   VOLUME", LABEL.scale(0.7), 0.6);
            let (mut mass, mut volume, mut placed) = (0.0, 0.0, 0);
            for (k, f) in interior.fit.iter().enumerate() {
                let (q, qc) = module_row(k);
                let block = plan.blocks.iter().find(|b| b.id == f.id);
                let lit = interior.module == Some(k) || interior.module_hover == Some(k) || inside((q, qc), interior.cursor);
                if interior.module == Some(k) {
                    frame.hud_rect(q, qc, PICKED.scale(0.18));
                }
                let col = if block.is_some_and(|b| clash_of(&b.id)) { CLASH } else if lit { PICKED } else if block.is_some() { MODULE } else { LABEL.scale(0.6) };
                if block.is_some() {
                    frame.hud_rect(Vec2::new(q.x + 2.0, q.y + 3.0), Vec2::splat(5.0), col);
                    mass += f.mass;
                    volume += f.volume;
                    placed += 1;
                } else {
                    frame.hud_box(Vec2::new(q.x + 2.0, q.y + 3.0), Vec2::splat(5.0), col);
                }
                let name: String = f.name.chars().take(22).collect();
                frame.text_scaled(Vec2::new(q.x + 11.0, q.y + 2.0), &name, col, 0.6);
                let figures = if f.volume < 10.0 { format!("{:>5.2} T {:>5.1} M3", f.mass / 1000.0, f.volume) } else { format!("{:>5.2} T {:>5.0} M3", f.mass / 1000.0, f.volume) };
                frame.text_scaled(Vec2::new(q.x + qc.x - figures.len() as f32 * 4.8, q.y + 2.0), &figures, col, 0.6);
            }
            let total: f32 = interior.fit.iter().map(|f| f.volume).sum();
            let text = format!("PLACED {placed} OF {}: {volume:.0} OF {total:.0} M3, {:.1} T", interior.fit.len(), mass / 1000.0);
            frame.text_scaled(Vec2::new(p.x + 8.0, p.y + 245.0), &text, MODULE, 0.65);
            let picked_block = interior.module.and_then(|k| interior.fit.get(k)).and_then(|f| plan.blocks.iter().find(|b| b.id == f.id));
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
        if interior.tool != Tool::Modules {
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
                Tool::Path | Tool::Modules => "NEW LINES' CROSS-SECTION",
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
    // A message for a while (saved, opened), under the toolbar.
    if let Some((text, _)) = &interior.message {
        frame.text_scaled(Vec2::new(button(2).0.x, 52.0), text, PICKED, 0.7);
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
