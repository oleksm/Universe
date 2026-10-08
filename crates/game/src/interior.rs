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
const LAYERS: [(&str, Option<usize>, Option<Color>); 26] = [
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
    ("CLASHES", None, Some(Color([1.0, 0.3, 0.25, 1.0]))),
    ("DECKS (2D)", None, None),
    ("FLOORS", Some(17), Some(Color([1.0, 0.8, 0.5, 0.6]))),
    ("WALLS", Some(17), Some(Color([1.0, 0.8, 0.5, 0.9]))),
    ("HOLLOW MAP", None, Some(Color([0.4, 1.0, 0.55, 0.8]))),
    ("REACH", None, Some(Color([0.4, 1.0, 0.55, 0.8]))),
    ("MODULES", None, Some(MODULE)),
    ("FRAME", None, Some(Color([0.4, 1.0, 0.5, 1.0]))),
    ("PRESSURE", None, Some(Color([1.0, 0.3, 0.25, 1.0]))),
    ("DECKS", None, Some(DECK)),
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
    pub const PRESSURE: usize = 24;
    pub const DECKS: usize = 25;
}

/// A deck's colour (its plate, filled and outlined).
const DECK: Color = Color([0.85, 0.75, 0.5, 1.0]);

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
    /// Each frame member's clash, as a solid tube (what it passes into, if anything),
    /// for which plan.
    member_clash: Option<(Plan, Vec<Option<Passes>>)>,
    /// Its nodes (each joint, its widest tube, its node), for which plan.
    node_cache: Option<(Plan, Vec<Node>)>,
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
    /// DECK: laying decks (their first corner, once clicked).
    deck_mode: bool,
    deck_from: Option<Vec3>,
    /// DECK: the stock new decks are cut from (its key; none: the lightest 6061
    /// plate), and the deck picked (its stock is changed by picking a row).
    deck_stock: Option<String>,
    deck_pick: Option<usize>,
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
    /// The test stand: a design with no hull walked in the studio itself.
    stand: Option<Stand>,
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
    /// The issue last gone to (its number in `issues`), and how far the issue list
    /// is scrolled (rows).
    issue: Option<usize>,
    issue_top: usize,
    /// Where the budgets panel was last drawn (clicks and the wheel there are its).
    budget_rect: std::cell::Cell<(Vec2, Vec2)>,
    /// MIRROR (X): edits made across the ship's middle too (0 off; 1 across x = 0,
    /// left and right; 2 across z = 0, fore and aft; 3 both); this frame's change
    /// made by a generator (MOUNT ALL, BRACE, AUTO-SIZE), not mirrored.
    mirror: u8,
    bulk: bool,
    /// SELECT: modules, members and decks picked with the one picked (SHIFT-click);
    /// what was copied (CTRL+C), and stamping it (CTRL+V: a click puts it there).
    set: Vec<Hover>,
    clip: Option<Assembly>,
    stamping: bool,
    /// COMPARE (C): the design compared with, and its figures.
    compare: Option<(String, Vec<Figure>)>,
    /// Exact values being typed for what's picked (ENTER), or for the targets (SET
    /// TARGETS: those, then); where SET TARGETS was last drawn.
    entry: Option<String>,
    entry_targets: bool,
    targets_rect: std::cell::Cell<(Vec2, Vec2)>,
    /// The grid points on the work plane snap to (m; none set: a quarter metre).
    snap: Option<f32>,
    /// MEASURE (Q): measuring (its first point, once clicked), and the last measure.
    measuring: bool,
    measure_from: Option<Vec3>,
    measured: Option<(Vec3, Vec3)>,
    /// The view: 0 as turned (in perspective); 1 front, 2 side, 3 top (flat, with
    /// the ship's sizes).
    view: u8,
    /// TEST DRIVE under way (the design flown), and its balance room before it.
    drive: Option<crate::test_drive::Drive>,
    balance: Option<crate::test_drive::Balance>,
    /// FIT FRAME under way: for which plan, its news, and its stop.
    fitting: Option<(Plan, mpsc::Receiver<Fitting>, Arc<std::sync::atomic::AtomicBool>)>,
}

/// The mirror images of `p` across the planes `mirror` names (none of them `p`).
fn images(p: Vec3, mirror: u8) -> Vec<Vec3> {
    let mut out = Vec::new();
    if mirror & 1 != 0 {
        out.push(Vec3::new(-p.x, p.y, p.z));
    }
    if mirror & 2 != 0 {
        out.push(Vec3::new(p.x, p.y, -p.z));
    }
    if mirror == 3 {
        out.push(Vec3::new(-p.x, p.y, -p.z));
    }
    out
}

/// The same mirror image of a direction, by image number (as `images` orders them).
fn image_dir(d: Vec3, mirror: u8, n: usize) -> Vec3 {
    let flip = match (mirror, n) {
        (1, _) | (3, 0) => Vec3::new(-1.0, 1.0, 1.0),
        (2, _) | (3, 1) => Vec3::new(1.0, 1.0, -1.0),
        _ => Vec3::new(-1.0, 1.0, -1.0),
    };
    d * flip
}

/// An edit (`before` to `plan`) made across the mirror planes too: modules placed,
/// moved, turned or taken out, members laid or taken out (or their stock changed),
/// decks laid or taken out, each done to its twins as well. Modules mirror only in
/// a design with no hull (a hull's slots hold one of each).
fn mirror_edit(before: &Plan, plan: &mut Plan, mirror: u8) {
    let near = |a: Vec3, b: Vec3| a.distance(b) < 0.05;
    // (Modules, by their ids.)
    if hull_of(plan).is_none() && plan.blocks.iter().all(|b| b.id.contains('#')) {
        let removed: Vec<Block> = before.blocks.iter().filter(|b| !plan.blocks.iter().any(|a| a.id == b.id)).cloned().collect();
        for r in &removed {
            for m in images(r.at, mirror) {
                plan.blocks.retain(|b| !(kind(&b.id) == kind(&r.id) && near(b.at, m)));
            }
        }
        let now = plan.blocks.clone();
        for b in &now {
            match before.blocks.iter().find(|o| o.id == b.id) {
                // (Placed: its twins placed.)
                None => {
                    for (n, m) in images(b.at, mirror).into_iter().enumerate() {
                        if near(m, b.at) || plan.blocks.iter().any(|o| kind(&o.id) == kind(&b.id) && near(o.at, m)) {
                            continue;
                        }
                        let k = (1..).find(|k| !plan.blocks.iter().any(|o| o.id == format!("{}#{k}", kind(&b.id)))).unwrap_or(1);
                        plan.blocks.push(Block { id: format!("{}#{k}", kind(&b.id)), at: m, size: b.size, push: b.push.map(|d| image_dir(d, mirror, n)) });
                    }
                }
                // (Moved, resized or turned: its twins (where its images were) too.)
                Some(o) if o != b => {
                    for (n, (was, m)) in images(o.at, mirror).into_iter().zip(images(b.at, mirror)).enumerate() {
                        if near(was, o.at) {
                            continue;
                        }
                        if let Some(t) = plan.blocks.iter_mut().find(|t| t.id != b.id && kind(&t.id) == kind(&b.id) && near(t.at, was)) {
                            (t.at, t.size, t.push) = (m, b.size, b.push.map(|d| image_dir(d, mirror, n)));
                        }
                    }
                }
                _ => {}
            }
        }
    }
    // (Members, by their ends, either way round.)
    let same = |x: &Beam, a: Vec3, b: Vec3| (near(x.a, a) && near(x.b, b)) || (near(x.a, b) && near(x.b, a));
    let removed: Vec<Beam> = before.beams.iter().filter(|o| !plan.beams.iter().any(|b| b == *o)).cloned().collect();
    let added: Vec<Beam> = plan.beams.iter().filter(|b| !before.beams.iter().any(|o| o == *b)).cloned().collect();
    for r in &removed {
        for (a, b) in images(r.a, mirror).into_iter().zip(images(r.b, mirror)) {
            if !same(r, a, b) && !added.iter().any(|x| same(x, a, b)) {
                plan.beams.retain(|x| !same(x, a, b));
            }
        }
    }
    for n in &added {
        for (a, b) in images(n.a, mirror).into_iter().zip(images(n.b, mirror)) {
            if same(n, a, b) {
                continue;
            }
            plan.beams.retain(|x| !same(x, a, b) || x.stock == n.stock);
            if !plan.beams.iter().any(|x| same(x, a, b)) {
                plan.beams.push(Beam { a, b, stock: n.stock.clone(), pinned: n.pinned });
            }
        }
    }
    // (Decks, by their corners and level.)
    let deck_image = |d: &Plate, m: Vec3| -> (Vec2, Vec2) {
        let (x0, x1) = if m.x < 0.0 { (-d.hi.x, -d.lo.x) } else { (d.lo.x, d.hi.x) };
        let (z0, z1) = if m.z < 0.0 { (-d.hi.y, -d.lo.y) } else { (d.lo.y, d.hi.y) };
        (Vec2::new(x0, z0), Vec2::new(x1, z1))
    };
    let flips: Vec<Vec3> = images(Vec3::ONE, mirror);
    let same_deck = |x: &Plate, lo: Vec2, hi: Vec2, y: f32| x.lo.distance(lo) < 0.05 && x.hi.distance(hi) < 0.05 && (x.y - y).abs() < 0.05;
    let removed: Vec<Plate> = before.plates.iter().filter(|o| !plan.plates.iter().any(|p| p == *o)).cloned().collect();
    let added: Vec<Plate> = plan.plates.iter().filter(|p| !before.plates.iter().any(|o| o == *p)).cloned().collect();
    for r in &removed {
        for &f in &flips {
            let (lo, hi) = deck_image(r, f);
            if !same_deck(r, lo, hi, r.y) {
                plan.plates.retain(|x| !same_deck(x, lo, hi, r.y));
            }
        }
    }
    for d in &added {
        for &f in &flips {
            let (lo, hi) = deck_image(d, f);
            if !same_deck(d, lo, hi, d.y) && !plan.plates.iter().any(|x| same_deck(x, lo, hi, d.y)) {
                plan.plates.push(Plate { lo, hi, y: d.y, stock: d.stock.clone() });
            }
        }
    }
}

/// Something wrong with the design, to go to: what it says, where it is (none: the
/// whole ship), and what to pick there.
struct Issue {
    text: String,
    at: Option<Vec3>,
    pick: Option<Hover>,
}

/// Rows of the issue list shown at once.
const ISSUE_ROWS: usize = 8;

/// Everything wrong with the design, worst first, from the checks as last worked
/// out (none of them worked out here): members and decks past their design limit
/// (breaking first), clashes, the budgets' faults, what's loose.
fn issues(i: &Interior, faults: &[String]) -> Vec<Issue> {
    let mut out = Vec::new();
    let plan = &i.plan;
    if let Some((_, b)) = i.bearing.as_ref().filter(|(p, _)| *p == *plan) {
        let mut over: Vec<(f64, bool, Issue)> = Vec::new();
        for (k, m) in plan.beams.iter().enumerate() {
            let os = b.outcomes(k);
            if let Some((case, o)) = b.cases.iter().map(|c| c.0.as_str()).zip(os.iter()).filter(|(_, o)| o.work.design > 1.0).max_by(|x, y| x.1.work.design.total_cmp(&y.1.work.design)) {
                let breaks = o.broken.is_some();
                let text = format!("{} MEMBER {k} {}: {case} {:.0}%", if breaks { "BREAKS" } else { "OVER" }, m.stock.trim_start_matches("stock.").to_uppercase(), o.work.design * 100.0);
                over.push((o.work.design, breaks, Issue { text, at: Some((m.a + m.b) * 0.5), pick: Some(Hover::Member(k)) }));
            }
        }
        for (n, d) in plan.plates.iter().enumerate() {
            let worst = b.strips.iter().filter(|s| s.0 == n).flat_map(|s| b.cases.iter().map(|c| c.0.as_str()).zip(b.of_member(s.4))).max_by(|x, y| x.1.work.design.total_cmp(&y.1.work.design));
            if let Some((case, o)) = worst.filter(|w| w.1.work.design > 1.0) {
                let at = Vec3::new((d.lo.x + d.hi.x) * 0.5, d.y, (d.lo.y + d.hi.y) * 0.5);
                over.push((o.work.design, o.broken.is_some(), Issue { text: format!("OVER DECK {}: {case} {:.0}%", n + 1, o.work.design * 100.0), at: Some(at), pick: Some(Hover::Deck(n)) }));
            }
        }
        over.sort_by(|x, y| y.1.cmp(&x.1).then(y.0.total_cmp(&x.0)));
        out.extend(over.into_iter().map(|o| o.2));
    }
    if let Some((_, clash)) = i.member_clash.as_ref().filter(|(p, _)| *p == *plan) {
        for (k, c) in clash.iter().enumerate() {
            let say = match c {
                Some(Passes::Module) => "THROUGH A MODULE",
                Some(Passes::Room) => "INTO A ROOM",
                Some(Passes::Deck) => "THROUGH A DECK",
                Some(Passes::Member) => "INTO A MEMBER",
                None => continue,
            };
            let m = &plan.beams[k];
            out.push(Issue { text: format!("CLASH MEMBER {k} {say}"), at: Some((m.a + m.b) * 0.5), pick: Some(Hover::Member(k)) });
        }
    }
    if let Some((_, clash)) = i.block_clash.as_ref().filter(|(b, _)| *b == plan.blocks) {
        for (k, _) in clash.iter().enumerate().filter(|(_, c)| **c) {
            let name = i.fit.iter().find(|f| f.id == kind(&plan.blocks[k].id)).map_or(plan.blocks[k].id.to_uppercase(), |f| f.name.clone());
            out.push(Issue { text: format!("CLASH {name}"), at: Some(plan.blocks[k].at), pick: Some(Hover::Module(k)) });
        }
    }
    // (A fault or a loose module naming a module: at that module.)
    let named = |text: &str| plan.blocks.iter().enumerate().find(|(_, b)| text.contains(&kind(&b.id).to_uppercase()) || i.fit.iter().find(|f| f.id == kind(&b.id)).is_some_and(|f| text.starts_with(&f.name)));
    for f in faults {
        let at = named(f);
        out.push(Issue { text: f.clone(), at: at.map(|(_, b)| b.at), pick: at.map(|(k, _)| Hover::Module(k)) });
    }
    if let Some((_, b)) = i.bearing.as_ref().filter(|(p, _)| *p == *plan) {
        for l in &b.loose {
            let at = named(l);
            out.push(Issue { text: format!("LOOSE {l}"), at: at.map(|(_, b)| b.at), pick: at.map(|(k, _)| Hover::Module(k)) });
        }
    }
    out
}

/// A box on the screen: its corner and its size.
type Rect = (Vec2, Vec2);

/// The budgets panel (along the bottom, right of the tool panel) and the issue
/// panel (under the tool panel), on a screen `size`: each one's box, and each issue
/// row shown (its number in the list, its box).
fn budget_panel(size: Vec2, lines: usize, issues: usize, top: usize) -> (Rect, Rect, Vec<(usize, Rect)>) {
    let (pp, pc) = PANEL;
    let height = 18.0 + lines as f32 * 10.0;
    let x = pp.x + pc.x + 6.0;
    let budgets = (Vec2::new(x, size.y - 22.0 - height), Vec2::new((size.x - 170.0 - x).max(380.0), height));
    let shown = issues.saturating_sub(top).min(ISSUE_ROWS);
    let list = (Vec2::new(pp.x, pp.y + pc.y + 6.0), Vec2::new(pc.x, 18.0 + ISSUE_ROWS as f32 * 10.0));
    let first = list.0.y + 15.0;
    let boxes = (0..shown).map(|n| (top + n, (Vec2::new(list.0.x + 2.0, first + n as f32 * 10.0 - 1.0), Vec2::new(list.1.x - 4.0, 10.0)))).collect();
    (budgets, list, boxes)
}

/// A piece of a design to stamp elsewhere (CTRL+C, CTRL+V; saved as an
/// assembly): its modules, members and decks, placed round its foot (x and z its
/// bounds' middle, y its lowest).
#[derive(Clone, Default, serde::Serialize, serde::Deserialize)]
struct Assembly {
    blocks: Vec<Block>,
    beams: Vec<Beam>,
    plates: Vec<Plate>,
}

impl Assembly {
    /// What `picks` names of `plan`, about its foot.
    fn of(plan: &Plan, picks: &[Hover]) -> Assembly {
        let a = Self::as_placed(plan, picks);
        a.moved(-a.foot())
    }

    /// Its foot: x and z its bounds' middle, y its lowest.
    fn foot(&self) -> Vec3 {
        let (lo, hi) = self.bounds();
        Vec3::new((lo.x + hi.x) * 0.5, lo.y, (lo.z + hi.z) * 0.5)
    }

    /// What `picks` names of `plan`, where it stands.
    fn as_placed(plan: &Plan, picks: &[Hover]) -> Assembly {
        let mut a = Assembly::default();
        for h in picks {
            match *h {
                Hover::Module(n) => a.blocks.extend(plan.blocks.get(n).cloned()),
                Hover::Member(k) => a.beams.extend(plan.beams.get(k).cloned()),
                Hover::Deck(n) => a.plates.extend(plan.plates.get(n).cloned()),
                _ => {}
            }
        }
        a
    }

    fn bounds(&self) -> (Vec3, Vec3) {
        let mut lo = Vec3::splat(f32::MAX);
        let mut hi = Vec3::splat(f32::MIN);
        for b in &self.blocks {
            let (a, z) = b.bounds();
            (lo, hi) = (lo.min(a), hi.max(z));
        }
        for b in &self.beams {
            (lo, hi) = (lo.min(b.a).min(b.b), hi.max(b.a).max(b.b));
        }
        for p in &self.plates {
            (lo, hi) = (lo.min(Vec3::new(p.lo.x, p.y, p.lo.y)), hi.max(Vec3::new(p.hi.x, p.y, p.hi.y)));
        }
        if lo.x > hi.x { (Vec3::ZERO, Vec3::ZERO) } else { (lo, hi) }
    }

    fn moved(&self, by: Vec3) -> Assembly {
        Assembly {
            blocks: self.blocks.iter().map(|b| Block { at: b.at + by, ..b.clone() }).collect(),
            beams: self.beams.iter().map(|b| Beam { a: b.a + by, b: b.b + by, ..b.clone() }).collect(),
            plates: self.plates.iter().map(|p| Plate { lo: p.lo + Vec2::new(by.x, by.z), hi: p.hi + Vec2::new(by.x, by.z), y: p.y + by.y, stock: p.stock.clone() }).collect(),
        }
    }

    fn is_empty(&self) -> bool {
        self.blocks.is_empty() && self.beams.is_empty() && self.plates.is_empty()
    }

    fn says(&self) -> String {
        format!("{} MODULES, {} MEMBERS, {} DECKS", self.blocks.len(), self.beams.len(), self.plates.len())
    }

    /// Put in `plan` with its foot at `at`: modules numbered on (a design with a
    /// hull takes no copies of its slots), members not already there, decks not
    /// already there; joints knit to the frame's.
    fn stamp(&self, plan: &mut Plan, at: Vec3) {
        let here = self.moved(at);
        if hull_of(plan).is_none() {
            for b in here.blocks {
                let k = (1..).find(|k| !plan.blocks.iter().any(|o| o.id == format!("{}#{k}", kind(&b.id)))).unwrap_or(1);
                plan.blocks.push(Block { id: format!("{}#{k}", kind(&b.id)), ..b });
            }
        }
        let near = |x: &Beam, b: &Beam| (x.a.distance(b.a) < 0.05 && x.b.distance(b.b) < 0.05) || (x.a.distance(b.b) < 0.05 && x.b.distance(b.a) < 0.05);
        for b in here.beams {
            if !plan.beams.iter().any(|x| near(x, &b)) {
                plan.beams.push(b);
            }
        }
        plan.beams = knit(std::mem::take(&mut plan.beams)).0;
        for p in here.plates {
            if !plan.plates.iter().any(|x| x.lo.distance(p.lo) < 0.05 && x.hi.distance(p.hi) < 0.05 && (x.y - p.y).abs() < 0.05) {
                plan.plates.push(p);
            }
        }
    }

    fn folder() -> std::path::PathBuf {
        crate::save::data_dir().join("freefall").join("interiors").join("assemblies")
    }

    /// Saved under the next free name (assembly-1, -2...): which.
    fn save(&self) -> Option<String> {
        let dir = Self::folder();
        std::fs::create_dir_all(&dir).ok()?;
        let name = (1..).map(|k| format!("assembly-{k}")).find(|n| !dir.join(format!("{n}.json")).exists())?;
        std::fs::write(dir.join(format!("{name}.json")), serde_json::to_string_pretty(self).ok()?).ok()?;
        Some(name)
    }

    fn load(name: &str) -> Option<Assembly> {
        serde_json::from_str(&std::fs::read_to_string(Self::folder().join(format!("{name}.json"))).ok()?).ok()
    }

    /// The saved ones: each one's name and what's in it.
    fn saved() -> Vec<(String, String)> {
        let mut out: Vec<(String, String)> = std::fs::read_dir(Self::folder()).into_iter().flatten().flatten().filter_map(|e| {
            let name = e.file_name().to_string_lossy().strip_suffix(".json")?.to_string();
            let a = Self::load(&name)?;
            Some((name, a.says()))
        }).collect();
        out.sort();
        out
    }
}

/// A deck's level: everything above it up to the next deck over it (that deck
/// too, not this one): modules standing in it, members within it (not this deck's
/// own joists), the deck over it. None: no deck over it.
fn level(plan: &Plan, deck: usize) -> Option<(Vec<Hover>, f32)> {
    let y0 = plan.plates.get(deck)?.y;
    let y1 = plan.plates.iter().map(|p| p.y).filter(|&y| y > y0 + 0.5).fold(f32::MAX, f32::min);
    if y1 == f32::MAX {
        return None;
    }
    let mut out = Vec::new();
    for (n, b) in plan.blocks.iter().enumerate() {
        let foot = b.at.y - b.size.y * 0.5;
        if foot >= y0 - 0.3 && foot < y1 - 0.3 {
            out.push(Hover::Module(n));
        }
    }
    for (k, b) in plan.beams.iter().enumerate() {
        let (lo, hi) = (b.a.y.min(b.b.y), b.a.y.max(b.b.y));
        if lo >= y0 - 0.5 && hi <= y1 + 0.5 && hi > y0 + 0.5 {
            out.push(Hover::Member(k));
        }
    }
    for (n, p) in plan.plates.iter().enumerate() {
        if (p.y - y1).abs() < 0.05 {
            out.push(Hover::Deck(n));
        }
    }
    Some((out, y1 - y0))
}

/// What's picked, as exact values to type over: what they are, and them now.
fn exact_of(i: &Interior) -> Option<(&'static str, String)> {
    let f = |v: &[f32]| v.iter().map(|x| format!("{x:.2}")).collect::<Vec<_>>().join(", ");
    if i.entry_targets {
        let t = i.plan.targets.clone().unwrap_or_default();
        let v = [t.delta_v.map_or(0.0, |v| v / 1000.0), t.crew.unwrap_or(0.0), t.cargo.map_or(0.0, |v| v / 1000.0), t.trip.unwrap_or(0.0)];
        return Some(("TARGETS: DELTA-V KM/S, CREW, CARGO T, LONGEST TRIP H (0: NOT ASKED)", v.iter().map(|x| format!("{x}")).collect::<Vec<_>>().join(", ")));
    }
    match i.pick? {
        Hover::Module(n) => {
            let b = i.plan.blocks.get(n)?;
            Some(("MODULE: X, FOOT Y, Z", f(&[b.at.x, b.at.y - b.size.y * 0.5, b.at.z])))
        }
        Hover::Member(k) => {
            let b = i.plan.beams.get(k)?;
            Some(("MEMBER: FROM X, Y, Z TO X, Y, Z", f(&[b.a.x, b.a.y, b.a.z, b.b.x, b.b.y, b.b.z])))
        }
        Hover::Deck(n) => {
            let p = i.plan.plates.get(n)?;
            Some(("DECK: FROM X, Z TO X, Z, AT Y", f(&[p.lo.x, p.lo.y, p.hi.x, p.hi.y, p.y])))
        }
        _ => None,
    }
}

/// The values typed set on what's picked; what happened.
fn set_exact(i: &mut Interior, v: &[f32]) -> String {
    if i.entry_targets {
        i.entry_targets = false;
        let get = |k: usize, scale: f64| v.get(k).map(|&x| f64::from(x) * scale).filter(|&x| x > 0.0);
        let t = Targets { delta_v: get(0, 1000.0), crew: get(1, 1.0), cargo: get(2, 1000.0), trip: get(3, 1.0) };
        let any = t.delta_v.is_some() || t.crew.is_some() || t.cargo.is_some() || t.trip.is_some();
        i.plan.targets = any.then_some(t);
        return if any { "TARGETS SET: SEE THE BUDGETS".into() } else { "TARGETS CLEARED".into() };
    }
    match i.pick {
        Some(Hover::Module(n)) if v.len() == 3 && n < i.plan.blocks.len() => {
            let b = &mut i.plan.blocks[n];
            b.at = Vec3::new(v[0], v[1] + b.size.y * 0.5, v[2]);
            "MODULE MOVED".into()
        }
        Some(Hover::Member(k)) if v.len() == 6 && k < i.plan.beams.len() => {
            let b = &mut i.plan.beams[k];
            (b.a, b.b) = (Vec3::new(v[0], v[1], v[2]), Vec3::new(v[3], v[4], v[5]));
            "MEMBER MOVED".into()
        }
        Some(Hover::Deck(n)) if v.len() == 5 && n < i.plan.plates.len() => {
            let p = &mut i.plan.plates[n];
            (p.lo, p.hi, p.y) = (Vec2::new(v[0].min(v[2]), v[1].min(v[3])), Vec2::new(v[0].max(v[2]), v[1].max(v[3])), v[4]);
            "DECK MOVED".into()
        }
        _ => format!("NOT SET: {} VALUES TYPED, {} WANTED", v.len(), match i.pick {
            Some(Hover::Module(_)) => 3,
            Some(Hover::Member(_)) => 6,
            Some(Hover::Deck(_)) => 5,
            _ => 0,
        }),
    }
}

/// The ship's sizes in a flat view: across the bottom and up the side of all it
/// is (modules, members, decks), each with its length.
fn draw_sizes(frame: &mut Frame, i: &Interior, cam: &Camera) {
    let a = Assembly { blocks: i.plan.blocks.clone(), beams: i.plan.beams.clone(), plates: i.plan.plates.clone() };
    let (lo, hi) = a.bounds();
    if lo == hi {
        return;
    }
    // (Across: x, except side on (z); up: y, except from the top (z).)
    let (across, up) = match i.view {
        2 => (Vec3::Z, Vec3::Y),
        3 => (Vec3::X, Vec3::NEG_Z),
        _ => (Vec3::X, Vec3::Y),
    };
    let col = Color([1.0, 0.85, 0.4, 0.95]);
    let size = hi - lo;
    let pad = size.max_element() * 0.06;
    let corner = |u: f32, v: f32| {
        let base = Vec3::new(if across.x != 0.0 { u } else { (lo.x + hi.x) * 0.5 }, if up.y != 0.0 { v } else { (lo.y + hi.y) * 0.5 }, if across.z != 0.0 { u } else if up.z != 0.0 { -v } else { (lo.z + hi.z) * 0.5 });
        cam.project(base).map(|p| p.0)
    };
    let (u0, u1) = if across.x != 0.0 { (lo.x, hi.x) } else { (lo.z, hi.z) };
    let (v0, v1) = if up.y != 0.0 { (lo.y, hi.y) } else { (-hi.z, -lo.z) };
    // (Each label outside its line: under the one across, left of the one up.)
    let mut line = |a: Option<Vec2>, b: Option<Vec2>, text: String| {
        if let (Some(a), Some(b)) = (a, b) {
            frame.hud_line(a, b, col);
            let n = (b - a).perp().normalize_or_zero() * 4.0;
            frame.hud_line(a - n, a + n, col);
            frame.hud_line(b - n, b + n, col);
            let mid = (a + b) * 0.5;
            let across = (b - a).x.abs() > (b - a).y.abs();
            let off = if across { Vec2::new(-(text.len() as f32) * 3.5, 8.0) } else { Vec2::new(-(text.len() as f32) * 7.5 - 8.0, -4.0) };
            frame.hud_rect(mid + off - Vec2::new(2.0, 2.0), Vec2::new(text.len() as f32 * 7.2 + 4.0, 13.0), Color([0.02, 0.04, 0.08, 0.9]));
            frame.text_scaled(mid + off, &text, col, 0.85);
        }
    };
    line(corner(u0, v0 - pad), corner(u1, v0 - pad), format!("{:.1} M", u1 - u0));
    line(corner(u0 - pad, v0), corner(u0 - pad, v1), format!("{:.1} M", v1 - v0));
}

impl Interior {
    /// The modules, members and decks picked in SELECT (the one picked and the
    /// rest picked with it).
    fn picked_items(&self) -> Vec<Hover> {
        let mut out: Vec<Hover> = self.pick.into_iter().filter(|h| matches!(h, Hover::Module(_) | Hover::Member(_) | Hover::Deck(_))).collect();
        for &h in &self.set {
            if !out.contains(&h) {
                out.push(h);
            }
        }
        out
    }

    /// A flat view (dev: `freefall --studio` with UNIVERSE_VIEW): 1 front, 2 side,
    /// 3 top.
    pub fn flat_view(&mut self, k: u8) {
        let (yaw, pitch) = [(0.0, 0.0), (std::f32::consts::FRAC_PI_2, 0.0), (0.0, 1.5695)][(k.clamp(1, 3) - 1) as usize];
        (self.yaw, self.pitch, self.view) = (yaw, pitch, k.clamp(1, 3));
    }

    /// Compared with saved design `name` (dev: `freefall --studio` with
    /// UNIVERSE_COMPARE).
    pub fn compare_with(&mut self, name: &str) {
        self.compare = Some((name.to_string(), key_figures(&checked(name))));
    }

    /// The tool in hand set by name (dev: `freefall --studio` with UNIVERSE_TOOL):
    /// select, path, door, modules, frame.
    pub fn use_tool(&mut self, name: &str) {
        self.tool = match name {
            "path" => Tool::Path,
            "door" => Tool::Door,
            "modules" => Tool::Modules,
            "frame" => Tool::Frame,
            _ => Tool::Look,
        };
    }

    /// Go to issue `n` (dev: `freefall --studio` with UNIVERSE_ISSUE), once the
    /// checks list that many and the frame's loads are worked out; gone to?
    pub fn go_to_issue(&mut self, n: usize) -> bool {
        if !self.bearing.as_ref().is_some_and(|(p, _)| *p == self.plan) {
            return false;
        }
        let b = budget(self);
        let list = issues(self, &b.faults);
        if list.len() <= n {
            return false;
        }
        self.go_to(&list, n);
        true
    }

    /// Go to issue `n`: the view turned on where it is, close in, and what it's
    /// about picked (in SELECT, to see it).
    fn go_to(&mut self, list: &[Issue], n: usize) {
        let Some(is) = list.get(n) else { return };
        self.issue = Some(n);
        self.issue_top = if n < self.issue_top { n } else if n >= self.issue_top + ISSUE_ROWS { n + 1 - ISSUE_ROWS } else { self.issue_top };
        if let Some(at) = is.at {
            self.target = Some(at);
            self.distance = Some(14.0);
        }
        if let Some(h) = is.pick {
            self.cancel();
            self.tool = Tool::Look;
            self.pick = Some(h);
            self.more.clear();
        }
        self.message = Some((is.text.clone(), 6.0));
    }
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
/// What a frame member, a solid tube, passes into.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Passes {
    Module,
    Room,
    Deck,
    Member,
}

/// The closest two points of segments `a`-`b` and `c`-`d` are this far apart.
fn segment_gap(a: Vec3, b: Vec3, c: Vec3, d: Vec3) -> f32 {
    let (u, v, w) = (b - a, d - c, a - c);
    let (aa, bb, cc, dd, ee) = (u.dot(u), u.dot(v), v.dot(v), u.dot(w), v.dot(w));
    let den = aa * cc - bb * bb;
    let mut s = if den > 1e-9 { ((bb * ee - cc * dd) / den).clamp(0.0, 1.0) } else { 0.0 };
    let t = if cc > 1e-9 { ((bb * s + ee) / cc).clamp(0.0, 1.0) } else { 0.0 };
    if aa > 1e-9 {
        s = ((bb * t - dd) / aa).clamp(0.0, 1.0);
    }
    (a + u * s).distance(c + v * t)
}

/// The radius of a member of `stock` (m: half its tube's diameter).
fn tube_radius(stock: &str) -> f32 {
    stocks().iter().find(|s| s.key == stock).map_or(0.03, |s| s.section.diameter as f32 * 0.5)
}

/// Clear of contact by a centimetre: touching isn't passing into (m).
const TOUCH: f32 = 0.01;

/// What member `b`, the solid tube it is, passes into, if anything: a module it
/// doesn't mount (one with neither of its ends at it), a walled room, a deck (a
/// slab its depth down from its top) it doesn't end on, or one of `others` it
/// shares no end with.
fn passes(plan: &Plan, b: &Beam, others: &[Beam]) -> Option<Passes> {
    let r = tube_radius(&b.stock);
    // (Points along it, clear of its ends by its radius and a little: its ends are
    // joints, where it meets what it's joined to.)
    let len = b.a.distance(b.b);
    let n = ((len / 0.1).ceil() as usize).max(2);
    let pts: Vec<Vec3> = (0..=n).map(|k| k as f32 / n as f32).filter(|t| t * len > r + 0.05 && (1.0 - t) * len > r + 0.05).map(|t| b.a.lerp(b.b, t)).collect();
    let module = plan.blocks.iter().any(|blk| {
        let (lo, hi) = blk.bounds();
        let near = |p: Vec3| p.cmpge(lo - 0.3).all() && p.cmple(hi + 0.3).all();
        !near(b.a) && !near(b.b) && pts.iter().any(|&p| p.clamp(lo, hi).distance(p) < r - TOUCH)
    });
    if module {
        return Some(Passes::Module);
    }
    let room = (0..plan.lines.len()).filter(|&k| plan.group_of(k).is_some_and(|g| plan.groups[g].walled) && plan.lines[k].2.section != Section::Line).any(|k| {
        let (p, e) = plan.axis(k);
        let pr = plan.lines[k].2;
        let d = e - p;
        // (Its walls' thickness round it; its floor the deck it stands on.)
        let wall = wall_of(pr).map_or(0.0, |w| w.0.depth as f32);
        pts.iter().any(|&q| {
            let t = (q - p).dot(d) / d.length_squared().max(1e-6);
            let off = q - (p + d * t.clamp(0.0, 1.0));
            (0.0..=1.0).contains(&t) && off.y > -(pr.height * 0.5) - r + TOUCH && off.y < pr.height * 0.5 + wall + r - TOUCH && Vec3::new(off.x, 0.0, off.z).length() < pr.width * 0.5 + wall + r - TOUCH
        })
    });
    if room {
        return Some(Passes::Room);
    }
    // (A deck is a slab from its top down its depth; a member that ends on it holds
    // it up, joined there; one standing steeply through it is a pillar through a
    // hole cut for it. One lying in it is in the way.)
    let steep = (b.b - b.a).normalize_or_zero().y.abs() > 0.5;
    let deck = !steep && plan.plates.iter().any(|pl| {
        let (x0, x1, z0, z1) = (pl.lo.x.min(pl.hi.x), pl.lo.x.max(pl.hi.x), pl.lo.y.min(pl.hi.y), pl.lo.y.max(pl.hi.y));
        let on = |p: Vec3| (p.y - pl.y).abs() < 0.05 && p.x >= x0 - 0.05 && p.x <= x1 + 0.05 && p.z >= z0 - 0.05 && p.z <= z1 + 0.05;
        if on(b.a) || on(b.b) {
            return false;
        }
        let depth = deck_depth(pl);
        pts.iter().any(|p| p.x > x0 - r + TOUCH && p.x < x1 + r - TOUCH && p.z > z0 - r + TOUCH && p.z < z1 + r - TOUCH && p.y > pl.y - depth - r + TOUCH && p.y < pl.y + r - TOUCH)
    });
    if deck {
        return Some(Passes::Deck);
    }
    let (lo, hi) = (b.a.min(b.b) - Vec3::splat(r), b.a.max(b.b) + Vec3::splat(r));
    let member = others.iter().any(|o| {
        if (o.a == b.a && o.b == b.b) || (o.a == b.b && o.b == b.a) {
            return false;
        }
        let ro = tube_radius(&o.stock);
        let shares = [b.a, b.b].iter().any(|p| p.distance(o.a) < 0.05 || p.distance(o.b) < 0.05);
        if shares || (o.a.min(o.b) - Vec3::splat(ro)).cmpgt(hi).any() || lo.cmpgt(o.a.max(o.b) + Vec3::splat(ro)).any() {
            return false;
        }
        segment_gap(b.a, b.b, o.a, o.b) < r + ro - TOUCH
    });
    member.then_some(Passes::Member)
}

/// Each member of the plan's frame as the solid tube it is: what it passes into.
fn member_clashes(plan: &Plan) -> Vec<Option<Passes>> {
    plan.beams.iter().map(|b| passes(plan, b, &plan.beams)).collect()
}

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
        // (Fit-out stands inside a room: no clash there.)
        let inside = i.fit.iter().find(|f| f.id == kind(&b.id)).and_then(figures).is_some_and(|(k, ..)| INSIDE.contains(&k.as_str()));
        hull || (!inside && rooms.iter().any(|p| b.holds(*p, round)))
    }).collect()
}

/// The kinds of module that stand inside a pressurised room: what the crew sit,
/// sleep, eat and wash at, and what keeps them safe there.
const INSIDE: [&str; 6] = ["command_station", "berths", "galley", "head", "fire_unit", "suit_locker"];

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
    /// Its decks: plates laid level, borne by the frame they touch.
    #[serde(default)]
    plates: Vec<Plate>,
    /// The gravity it's designed to land in (m/s²; not said: a standard g).
    #[serde(default)]
    gravity: Option<f64>,
    /// The job it's designed for, if said: checked in the budgets.
    #[serde(default)]
    targets: Option<Targets>,
}

/// A design's job: the delta-v it's to have (m/s) carrying its cargo (kg), and the
/// people it's to carry (each none: not asked).
#[derive(Clone, Default, PartialEq, serde::Serialize, serde::Deserialize)]
struct Targets {
    delta_v: Option<f64>,
    crew: Option<f64>,
    cargo: Option<f64>,
    /// Its longest trip (h): a short hop needs no berths, galley or head.
    #[serde(default)]
    trip: Option<f64>,
}

/// The trip lengths (h) past which those aboard need a head, a galley, berths
/// (invented thresholds, to review: an airliner's head, a working day's meals, a
/// night's sleep).
const NEEDS_HEAD: f64 = 2.0;
const NEEDS_GALLEY: f64 = 8.0;
const NEEDS_BERTHS: f64 = 16.0;

impl Plan {
    /// The gravity it's designed to land in (m/s²).
    fn gravity(&self) -> f64 {
        self.gravity.unwrap_or(STANDARD_G)
    }
}

/// A deck: a plate laid level at height `y` from `lo` to `hi` (x, z), cut from plate
/// stock (by its key in the registry).
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
struct Plate {
    lo: Vec2,
    hi: Vec2,
    y: f32,
    stock: String,
}

/// Plate stock a deck can be cut from: the registry's plates and sheets whose
/// material says how it bears; its thickness (m), what it's made of, its mass a
/// square metre (kg).
#[derive(Clone)]
pub struct PlateStock {
    key: String,
    name: String,
    of: String,
    /// A plate's thickness; a sandwich panel's each face's.
    thickness: f64,
    /// A sandwich panel's core (none: a solid plate).
    core: Option<universe_sim::world::frame::Core>,
    /// How thick it is in all (m): a plate's thickness; a panel's core and faces.
    depth: f64,
    material: universe_sim::world::frame::Material,
    per_square_metre: f64,
}

impl PlateStock {
    /// A strip of it `width` wide, as the frame bears it.
    fn section(&self, width: f32) -> universe_sim::world::frame::Section {
        use universe_sim::world::frame::Section;
        match self.core {
            Some(c) => Section::panel(f64::from(width), self.thickness, c),
            None => Section::strip(f64::from(width), self.thickness),
        }
    }
}

fn plate_stocks() -> &'static [PlateStock] {
    static PLATES: std::sync::OnceLock<Vec<PlateStock>> = std::sync::OnceLock::new();
    PLATES.get_or_init(|| {
        use universe_sim::world::frame::Material;
        let reg = universe_sim::world::registry::registry();
        let mut out: Vec<PlateStock> = reg.stock.iter().filter(|s| matches!(s.identity.form.as_str(), "plate" | "sheet") && !s.identity.code.ends_with("BLANK") && !s.identity.code.ends_with("PANEL")).filter_map(|s| {
            let t = s.size.thickness?;
            let of = &s.made_from.first()?.item;
            let m = reg.material(of)?;
            let k = &m.mechanical;
            let (e, y, ts, rho) = (k.youngs_modulus?, k.yield_strength?, k.tensile_strength?, m.mass.density?);
            let shear = k.shear_modulus.unwrap_or(e / (2.0 * (1.0 + k.poissons_ratio.unwrap_or(0.3))));
            Some(PlateStock { key: s.identity.key.clone(), name: s.identity.name.clone(), of: of.clone(), thickness: t, core: None, depth: t, material: Material { stiffness: e, shear, yield_strength: y, tensile_strength: ts, density: rho }, per_square_metre: t * rho })
        }).collect();
        // (Sandwich panels: their faces' material bears, their core shears.)
        out.extend(reg.stock.iter().filter_map(|s| {
            let sw = s.sandwich.as_ref()?;
            let m = reg.materials.iter().find(|m| m.identity.key == sw.faces.material)?;
            let k = &m.mechanical;
            let (e, y, ts, rho) = (k.youngs_modulus?, k.yield_strength?, k.tensile_strength?, m.mass.density?);
            let shear = k.shear_modulus.unwrap_or(e / (2.0 * (1.0 + k.poissons_ratio.unwrap_or(0.3))));
            let per_square_metre = sw.mass_per_area.unwrap_or(2.0 * sw.faces.thickness * rho + sw.core.depth * sw.core.density + sw.bond);
            Some(PlateStock {
                key: s.identity.key.clone(),
                name: s.identity.name.clone(),
                of: format!("{} on {}", sw.faces.material, sw.core.material),
                thickness: sw.faces.thickness,
                depth: s.size.thickness.unwrap_or(sw.core.depth + 2.0 * sw.faces.thickness),
                core: Some(universe_sim::world::frame::Core { depth: sw.core.depth, shear_strength: sw.core.shear_strength }),
                material: Material { stiffness: e, shear, yield_strength: y, tensile_strength: ts, density: rho },
                per_square_metre,
            })
        }));
        out.sort_by(|a, b| a.of.cmp(&b.of).then(a.per_square_metre.total_cmp(&b.per_square_metre)));
        out
    })
}

/// The stock a new deck is cut from unless another is picked: the lightest 6061
/// plate, else the first there is.
fn default_deck_stock() -> String {
    plate_stocks().iter().find(|s| s.key.contains("al6061-pl")).or(plate_stocks().first()).map_or(String::new(), |s| s.key.clone())
}

/// How thick a deck is (m): its stock's whole thickness. Its `y` is its top, the
/// floor walked on; it runs down from there.
fn deck_depth(p: &Plate) -> f32 {
    plate_stocks().iter().find(|s| s.key == p.stock).map_or(0.01, |s| s.depth as f32)
}

/// The pressure a ship's rooms hold (Pa): the registry's hulls' design cabin
/// pressure (sea-level air, chosen there), 101.3 kPa if none says.
fn cabin_pressure() -> f64 {
    universe_sim::world::registry::registry().hulls.iter().find_map(|h| h.design.cabin_pressure).unwrap_or(101_300.0)
}

/// How hard a wall of `ps` is worked holding pressure `p` round a room of profile
/// `pr` (1: at its design limit, safety factor 1.5): a round room in hoop tension
/// (p r / t); a many-sided one so, and each flat side bent between its corners; a
/// square one its flat sides bent, continuous round its corners (p L^2 / 12 a metre
/// of wall); a sandwich's faces carrying the bending as a couple, its core the
/// shear (p L / 2).
fn wall_work(ps: &PlateStock, pr: Profile, p: f64) -> f64 {
    const SF: f64 = 1.5;
    let limit = ps.material.yield_strength / SF;
    // (What carries tension: a plate its thickness, a sandwich its two faces.)
    let skin = ps.thickness * if ps.core.is_some() { 2.0 } else { 1.0 };
    let r = f64::from(pr.width.max(pr.height)) * 0.5;
    let hoop = p * r / skin / limit;
    let corners = pr.corners();
    let side = (0..corners.len()).map(|k| f64::from(corners[k].distance(corners[(k + 1) % corners.len()]))).fold(0.0, f64::max);
    let bend = |l: f64| -> f64 {
        let m = p * l * l / 12.0;
        let v = p * l / 2.0;
        match ps.core {
            Some(c) => {
                let d = c.depth + ps.thickness;
                (m / (ps.thickness * d) / limit).max(v / d * SF / c.shear_strength.max(1.0))
            }
            None => m * 6.0 / (ps.thickness * ps.thickness) / limit,
        }
    };
    match pr.section {
        Section::Round => hoop,
        Section::Square => bend(side),
        Section::Hex | Section::Oct => hoop.max(bend(side)),
        Section::Line => 0.0,
    }
}

/// A walled room's wall: the lightest stock (plate or panel) that holds the cabin
/// pressure round it, and how hard it's worked; none holds: the strongest, worked
/// past its limit.
fn wall_of(pr: Profile) -> Option<(&'static PlateStock, f64)> {
    let p = cabin_pressure();
    let mut all: Vec<(&PlateStock, f64)> = plate_stocks().iter().map(|s| (s, wall_work(s, pr, p))).collect();
    all.sort_by(|a, b| a.0.per_square_metre.total_cmp(&b.0.per_square_metre));
    all.iter().find(|w| w.1 <= 1.0).copied().or_else(|| all.iter().min_by(|a, b| a.1.total_cmp(&b.1)).copied())
}

/// Each walled room's wall: its line, its area (m²: round it, and across its walled
/// ends), its stock and how hard that's worked.
fn walls(plan: &Plan) -> Vec<(usize, f64, &'static PlateStock, f64)> {
    (0..plan.lines.len())
        .filter(|&k| plan.group_of(k).is_some_and(|g| plan.groups[g].walled) && plan.lines[k].2.section != Section::Line)
        .filter_map(|k| {
            let pr = plan.lines[k].2;
            let (a, b) = plan.axis(k);
            let c = pr.corners();
            let round: f32 = (0..c.len()).map(|j| c[j].distance(c[(j + 1) % c.len()])).sum();
            let ends = [plan.lines[k].0, plan.lines[k].1].iter().enumerate().filter(|&(e, &pt)| plan.walled_end(k, pt, e)).count();
            let area = f64::from(round * a.distance(b) + section_area(pr) * ends as f32);
            let (s, w) = wall_of(pr)?;
            Some((k, area, s, w))
        })
        .collect()
}

/// Lines across `a`..`b` on the ship's 2 m lattice: its two ends, and every even
/// metre between (none within half a metre of an end), so decks and frames laid on
/// the same lattice meet on the same lines.
fn lattice(a: f32, b: f32) -> Vec<f32> {
    let (lo, hi) = (a.min(b), a.max(b).max(a.min(b) + 0.1));
    let mut out = vec![lo];
    let mut k = (lo / 2.0).floor() + 1.0;
    while k * 2.0 < hi - 0.5 {
        if k * 2.0 > lo + 0.5 {
            out.push(k * 2.0);
        }
        k += 1.0;
    }
    out.push(hi);
    out
}

/// A deck's grid: its lines along x and along z (the 2 m lattice), and its points,
/// row by row (a row each z, along x).
fn plate_grid(p: &Plate) -> (Vec<Vec3>, Vec<f32>, Vec<f32>) {
    let (xs, zs) = (lattice(p.lo.x, p.hi.x), lattice(p.lo.y, p.hi.y));
    let nodes = zs.iter().flat_map(|&z| xs.iter().map(move |&x| Vec3::new(x, p.y, z))).collect();
    (nodes, xs, zs)
}

/// The width each of `lines` stands for: half the gap to each neighbour.
fn shares(lines: &[f32]) -> Vec<f32> {
    (0..lines.len()).map(|k| (lines[(k + 1).min(lines.len() - 1)] - lines[k.saturating_sub(1)]) * 0.5).collect()
}

/// The load a deck is designed to carry on top of its own weight (kg a square
/// metre: a crowd, or stores stacked; a figure chosen).
const DECK_LOAD: f64 = 500.0;

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
/// Placed modules whose key isn't in `fit` but is an old key the registry renamed
/// (its `renames.yaml`, followed through renames of renames): given the new key,
/// keeping their number. How many.
fn rename(blocks: &mut [Block], fit: &[Fitted]) -> usize {
    let renames = &universe_sim::world::registry::registry().renames;
    let mut renamed = 0;
    for b in blocks.iter_mut() {
        let k = kind(&b.id).to_string();
        if fit.iter().any(|f| f.id == k) {
            continue;
        }
        let mut now = k.clone();
        for _ in 0..8 {
            match renames.get(&now) {
                Some(next) => now = next.clone(),
                None => break,
            }
        }
        if now != k && fit.iter().any(|f| f.id == now) {
            b.id = format!("{now}{}", &b.id[k.len()..]);
            renamed += 1;
        }
    }
    renamed
}

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
    /// Pinned at its end at `a`, at `b` (free to turn there, carrying no bending
    /// into what it meets: a strut's rod end).
    #[serde(default, skip_serializing_if = "is_unpinned")]
    pinned: [bool; 2],
}

fn is_unpinned(p: &[bool; 2]) -> bool {
    !p[0] && !p[1]
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
        let mut out: Vec<Stock> = reg.stock.iter().filter(|s| matches!(s.identity.form.as_str(), "tube" | "bar" | "box")).filter_map(|s| {
            // (A box section by its width, height and wall; a tube or bar by its diameter.)
            let section = if s.identity.form == "box" {
                Section::boxed(s.size.width?, s.size.height?, s.size.wall?)
            } else {
                let d = s.size.diameter?;
                Section::round(d, s.size.wall.unwrap_or(d * 0.5).min(d * 0.5))
            };
            let of = &s.made_from.first()?.item;
            let m = reg.material(of)?;
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

/// A node fitting, where a frame's members meet: one of the registry's forgings.
pub struct NodeStock {
    /// Hollow (a shell, its wall on record), or a solid forging.
    hollow: bool,
    /// Across (m), and what it weighs (kg: a solid ball of its material).
    diameter: f32,
    mass: f64,
}

/// The registry's node forgings, smallest first.
fn node_stocks() -> &'static [NodeStock] {
    static NODES: std::sync::OnceLock<Vec<NodeStock>> = std::sync::OnceLock::new();
    NODES.get_or_init(|| {
        let reg = universe_sim::world::registry::registry();
        let mut out: Vec<NodeStock> = reg.stock.iter().filter(|s| s.identity.form == "forging" && s.identity.code.contains("NODE")).filter_map(|s| {
            let d = s.size.diameter?;
            let of = &s.made_from.first()?.item;
            let rho = reg.materials.iter().find(|m| m.identity.key == *of)?.mass.density?;
            // (Solid: a ball of its steel; hollow: a shell, 4 pi r^2 wall (1 - wall / d).)
            let mass = match s.size.wall {
                Some(w) => 4.0 * std::f64::consts::PI * (d * 0.5).powi(2) * w * rho * (1.0 - w / d),
                None => std::f64::consts::PI / 6.0 * d.powi(3) * rho,
            };
            Some(NodeStock { hollow: s.size.wall.is_some(), diameter: d as f32, mass })
        }).collect();
        out.sort_by(|a, b| a.mass.total_cmp(&b.mass));
        out
    })
}

/// How much wider a node is than the widest tube meeting at it (the registry's
/// rule, the MERO ratio).
const NODE_RATIO: f32 = 1.2;

/// The node for a joint whose widest tube is `widest` across: the lightest at least
/// `NODE_RATIO` as wide; at a root (a landing leg's mount) a solid one. None: none in stock is wide enough.
fn node_for(widest: f32, root: bool) -> Option<&'static NodeStock> {
    node_stocks().iter().filter(|n| !root || !n.hollow).find(|n| n.diameter >= widest * NODE_RATIO - 1e-4)
}

/// The boxes of a plan's roots: where a landing leg is held, its landing's jolt
/// all in a few joints (an engine pushes through many: no root).
fn roots(plan: &Plan, fit: &[Fitted]) -> Vec<(Vec3, Vec3)> {
    plan.blocks.iter().filter(|b| fit.iter().find(|f| f.id == kind(&b.id)).is_some_and(|f| f.gear.is_some())).map(|b| b.bounds()).collect()
}

/// A joint of a frame, the widest tube meeting there (m across), and its node.
type Node = (Vec3, f32, Option<&'static NodeStock>);

/// Each joint of a frame (its members' ends as they meet), the widest tube meeting
/// there (m across), and its node (`node_for`; a root: within `roots`' boxes).
fn nodes(beams: &[Beam], roots: &[(Vec3, Vec3)]) -> Vec<Node> {
    let mut out: Vec<(Vec3, f32)> = Vec::new();
    for b in beams {
        let d = tube_radius(&b.stock) * 2.0;
        for p in [b.a, b.b] {
            match out.iter_mut().find(|j| j.0.distance(p) < 0.05) {
                Some(j) => j.1 = j.1.max(d),
                None => out.push((p, d)),
            }
        }
    }
    out.into_iter().map(|(p, d)| (p, d, node_for(d, roots.iter().any(|(lo, hi)| p.cmpge(*lo - 0.3).all() && p.cmple(*hi + 0.3).all())))).collect()
}

/// The frame knit together at its nodes: joints nearer each other than a node
/// across (or 0.3 m) made one; a member passing through a node it doesn't end at jointed there
/// (as a space frame is built). The members, and how many joinings it made.
fn knit(mut beams: Vec<Beam>) -> (Vec<Beam>, usize) {
    let mut made = 0;
    for _ in 0..6 {
        let js = nodes(&beams, &[]);
        let reach = |n: &Node| n.2.map_or(n.1 * NODE_RATIO, |s| s.diameter) * 0.5;
        let mut changed = false;
        // (Joints within a node of each other: the later moved onto the earlier.)
        for a in 0..js.len() {
            for b in a + 1..js.len() {
                let (pa, pb) = (js[a].0, js[b].0);
                let d = pa.distance(pb);
                // (Nearer than a node across, or than the seat tolerance decks are laid
                // to: one node.)
                if d >= 0.05 && d < (2.0 * reach(&js[a]).max(reach(&js[b]))).max(0.3) {
                    for m in beams.iter_mut() {
                        if m.a.distance(pb) < 0.05 {
                            m.a = pa;
                        }
                        if m.b.distance(pb) < 0.05 {
                            m.b = pa;
                        }
                    }
                    changed = true;
                    made += 1;
                }
            }
        }
        beams.retain(|m| m.a.distance(m.b) > 0.05);
        let mut seen: Vec<(Vec3, Vec3)> = Vec::new();
        beams.retain(|m| {
            let dup = seen.iter().any(|&(a, b)| (a.distance(m.a) < 0.05 && b.distance(m.b) < 0.05) || (a.distance(m.b) < 0.05 && b.distance(m.a) < 0.05));
            seen.push((m.a, m.b));
            !dup
        });
        // (A member through a node it doesn't end at: jointed there.)
        let js = nodes(&beams, &[]);
        for j in &js {
            let r = reach(j);
            if let Some(k) = beams.iter().position(|m| {
                let d = m.b - m.a;
                let t = (j.0 - m.a).dot(d) / d.length_squared().max(1e-6);
                m.a.distance(j.0) > 0.05 && m.b.distance(j.0) > 0.05 && t > 0.0 && t < 1.0 && (m.a + d * t).distance(j.0) < r
            }) {
                let m = beams.remove(k);
                beams.push(Beam { a: m.a, b: j.0, stock: m.stock.clone(), pinned: [m.pinned[0], false] });
                beams.push(Beam { a: j.0, b: m.b, stock: m.stock, pinned: [false, m.pinned[1]] });
                changed = true;
                made += 1;
            }
        }
        // (Members that cross, nearer than their two radii, jointed where they come
        // nearest; a member's end against another's side, put on it and jointed there.)
        let n = crossings(&mut beams);
        changed |= n > 0;
        made += n;
        if !changed {
            break;
        }
    }
    (beams, made)
}

/// The nearest points of segments `p1 q1` and `p2 q2`: how far along each (0 to 1).
fn nearest(p1: Vec3, q1: Vec3, p2: Vec3, q2: Vec3) -> (f32, f32) {
    let (d1, d2, r) = (q1 - p1, q2 - p2, p1 - p2);
    let (a, e, f, c, b) = (d1.length_squared(), d2.length_squared(), d2.dot(r), d1.dot(r), d1.dot(d2));
    let den = a * e - b * b;
    let mut s = if den > 1e-9 { ((b * f - c * e) / den).clamp(0.0, 1.0) } else { 0.0 };
    let mut t = (b * s + f) / e.max(1e-9);
    if t < 0.0 {
        t = 0.0;
        s = (-c / a.max(1e-9)).clamp(0.0, 1.0);
    } else if t > 1.0 {
        t = 1.0;
        s = ((b - c) / a.max(1e-9)).clamp(0.0, 1.0);
    }
    (s, t)
}

/// Crossing members jointed (each pair once a pass; a member in one pair at most):
/// both split at the middle of their nearest points; a member's end against
/// another's side moved onto it, that one split there. How many joinings.
fn crossings(beams: &mut Vec<Beam>) -> usize {
    let mut used = vec![false; beams.len()];
    let mut cuts: Vec<(usize, Vec3)> = Vec::new();
    let mut moves: Vec<(usize, bool, Vec3)> = Vec::new();
    let shares = |x: &Beam, y: &Beam| [x.a, x.b].iter().any(|p| y.a.distance(*p) < 0.05 || y.b.distance(*p) < 0.05);
    for i in 0..beams.len() {
        for j in i + 1..beams.len() {
            if used[i] || used[j] || shares(&beams[i], &beams[j]) {
                continue;
            }
            let (x, y) = (&beams[i], &beams[j]);
            let (s, t) = nearest(x.a, x.b, y.a, y.b);
            let (pi, pj) = (x.a.lerp(x.b, s), y.a.lerp(y.b, t));
            if pi.distance(pj) >= tube_radius(&x.stock) + tube_radius(&y.stock) - TOUCH {
                continue;
            }
            let inner = |u: f32, len: f32| u * len > 0.1 && (1.0 - u) * len > 0.1;
            let (li, lj) = (x.a.distance(x.b), y.a.distance(y.b));
            match (inner(s, li), inner(t, lj)) {
                (true, true) => {
                    let c = (pi + pj) * 0.5;
                    cuts.push((i, c));
                    cuts.push((j, c));
                }
                // (j's end against i's side: that end onto i, i cut there.)
                (true, false) => {
                    moves.push((j, t < 0.5, pi));
                    cuts.push((i, pi));
                }
                (false, true) => {
                    moves.push((i, s < 0.5, pj));
                    cuts.push((j, pj));
                }
                _ => continue,
            }
            (used[i], used[j]) = (true, true);
        }
    }
    for &(k, at_a, p) in &moves {
        if at_a { beams[k].a = p } else { beams[k].b = p }
    }
    let made = cuts.len();
    cuts.sort_by_key(|c| std::cmp::Reverse(c.0));
    for (k, c) in cuts {
        let m = beams.remove(k);
        beams.push(Beam { a: m.a, b: c, stock: m.stock.clone(), pinned: [m.pinned[0], false] });
        beams.push(Beam { a: c, b: m.b, stock: m.stock, pinned: [false, m.pinned[1]] });
    }
    made
}

/// The gravity the frame is worked out under, standing (m/s²: a standard g; each
/// port's own is to come).
const STANDARD_G: f64 = 9.80665;

/// The load cases, by name: standing after a landing (the jolt of the hull's design
/// landing on top of the weight), at full main thrust, hovering at full lift.
const CASES: [&str; 4] = ["LANDING", "THRUST", "LIFT", "FLOOR"];

/// A case a design isn't built for (no legs: it never lands or stands; no lift: it
/// never hovers): said, not a fault.
const NOT_LANDING: &str = "IT DOESN'T LAND";
const NOT_STANDING: &str = "IT DOESN'T STAND";
const NOT_HOVERING: &str = "IT DOESN'T HOVER";

/// Is this case's error only that the design isn't built for it?
fn not_built_for(e: &str) -> bool {
    [NOT_LANDING, NOT_STANDING, NOT_HOVERING].contains(&e)
}

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
    /// Each deck's strips: its deck, its ends, its width (m), which of the frame's
    /// members it is.
    strips: Vec<(usize, Vec3, Vec3, f32, usize)>,
}

impl Bearing {
    /// How the frame's member `m` does in each case.
    fn of_member(&self, m: usize) -> Vec<&universe_sim::world::frame::Outcome> {
        self.cases.iter().filter_map(|(_, r)| r.as_ref().ok()).filter_map(|c| c.members.get(m)).collect()
    }

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
    // (Every member welded at its joints: a welded tube frame is nearer rigid than
    // pinned. Whether a member is a diagonal is kept for a pinned frame to come.)
    let mut add = |p: Vec3, q: Vec3, diagonal: bool| {
        if p.distance(q) > 0.05 && !out.iter().any(|x| (x.a == p && x.b == q) || (x.a == q && x.b == p)) {
            let _ = diagonal;
            out.push(Beam { a: p, b: q, stock: stock.to_string(), pinned: [false; 2] });
        }
    };
    let at = |x: f32, y: f32, z: f32| Vec3::new(x, y, z);
    for w in zs.windows(2) {
        for &x in &xs {
            for &y in &ys {
                add(at(x, y, w[0]), at(x, y, w[1]), false);
            }
            // (Each line's bay braced up its side.)
            add(at(x, ys[0], w[0]), at(x, ys[1], w[1]), true);
        }
        for xw in xs.windows(2) {
            for &y in &ys {
                add(at(xw[0], y, w[0]), at(xw[1], y, w[1]), true);
            }
        }
    }
    for &z in &zs {
        for &x in &xs {
            add(at(x, ys[0], z), at(x, ys[1], z), false);
        }
        for xw in xs.windows(2) {
            for &y in &ys {
                add(at(xw[0], y, z), at(xw[1], y, z), false);
            }
        }
    }
    out
}

/// Members mounting each placed module the frame doesn't hold yet (no joint in
/// it): joints along its foot (one each `spacing` m; over a tonne full, at its
/// foot's corners, each braced to the nearest joint below as well, or hung from
/// the nearest above if there's none below), each strutted
/// to the nearest two joints of the frame outside it; a landing leg by its top, to
/// four; never through a room or further than 15 m; and each landing pad with
/// nothing at it, on four legs to the nearest joints; each deck's grid point with
/// nothing at it, a post to the nearest joint below; of `stock`.
/// Can a member of `stock` run from `a` to `b` clear of everything (modules, rooms,
/// decks, the members of `frame` and `added`)?
fn clear(plan: &Plan, a: Vec3, b: Vec3, stock: &str, frame: &[Beam], added: &[Beam]) -> bool {
    let m = Beam { a, b, stock: stock.to_string(), pinned: [false; 2] };
    passes(plan, &m, frame).is_none() && passes(&Plan { blocks: Vec::new(), lines: Vec::new(), plates: Vec::new(), ..Plan::default() }, &m, added).is_none()
}

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
    // (Each deck's joists: under each row of its grid not already over a member, a
    // joist across it at the frame's level, from member to member it crosses (each
    // crossed member jointed there): what its points rest on between the frame's
    // own lines.)
    for plate in &plan.plates {
        let rows = plate_grid(plate).2;
        let (x0, x1) = (plate.lo.x.min(plate.hi.x), plate.lo.x.max(plate.hi.x));
        for z in rows {
            // (Where members under the deck, a seat's height at most, cross the row.)
            let mut cross: Vec<(f32, Vec3, usize)> = beams.iter().enumerate().filter_map(|(k, b)| {
                let under = |p: Vec3| (-0.05..=0.3).contains(&(plate.y - p.y));
                if !under(b.a) || !under(b.b) || (b.a.z - z) * (b.b.z - z) > 0.0 || (b.a.z - b.b.z).abs() < 1e-3 {
                    return None;
                }
                let at = b.a.lerp(b.b, (z - b.a.z) / (b.b.z - b.a.z));
                (at.x >= x0 - 0.05 && at.x <= x1 + 0.05).then_some((at.x, at, k))
            }).collect();
            cross.sort_by(|a, b| a.0.total_cmp(&b.0));
            cross.dedup_by(|a, b| (a.0 - b.0).abs() < 0.05);
            if cross.len() < 2 || beams.iter().any(|b| (b.a.z - z).abs() < 0.05 && (b.b.z - z).abs() < 0.05 && (plate.y - b.a.y) <= 0.3 && (plate.y - b.a.y) >= -0.05) {
                continue;
            }
            // (Each crossed member jointed where the joist meets it.)
            let ats: Vec<Vec3> = cross.iter().map(|c| c.1).collect();
            for &at in &ats {
                if let Some(k) = beams.iter().position(|b| {
                    let d = b.b - b.a;
                    let t = (at - b.a).dot(d) / d.length_squared().max(1e-6);
                    t > 0.02 && t < 0.98 && (b.a + d * t).distance(at) < 0.05
                }) {
                    let b = beams.remove(k);
                    beams.push(Beam { a: b.a, b: at, stock: b.stock.clone(), pinned: [b.pinned[0], false] });
                    beams.push(Beam { a: at, b: b.b, stock: b.stock, pinned: [false, b.pinned[1]] });
                }
            }
            for w in ats.windows(2) {
                beams.push(Beam { a: w[0], b: w[1], stock: stock.to_string(), pinned: [false; 2] });
            }
        }
    }
    for blk in &plan.blocks {
        let (lo, hi) = blk.bounds();
        let inside = |q: Vec3| q.cmpge(lo - 0.3).all() && q.cmple(hi + 0.3).all();
        // (A big engine (over half a meganewton): a thrust structure, a cone of struts
        // from the joints it pushes on to the four nearest joints the way it pushes,
        // within 7 m (the frame as it stands, joists and all), so its push runs along
        // them into the frame, as a rocket's thrust structure carries it.)
        let push = fit.iter().find(|f| f.id == kind(&blk.id)).and_then(|f| push_of(blk, f));
        if let Some((dir, thrust)) = push.filter(|p| p.1 > 5.0e5) {
            let now: Vec<Vec3> = beams.iter().flat_map(|b| [b.a, b.b]).fold(Vec::new(), |mut v, p| {
                if !v.iter().any(|q: &Vec3| q.distance(p) < 0.05) {
                    v.push(p);
                }
                v
            });
            let _ = thrust;
            for m in now.iter().copied().filter(|&q| inside(q)) {
                let mut round: Vec<Vec3> = now.iter().copied().filter(|&q| !inside(q) && (q - m).dot(dir) > 1.0 && q.distance(m) <= 7.0).collect();
                round.sort_by(|a, b| a.distance(m).total_cmp(&b.distance(m)));
                let chosen: Vec<Vec3> = round.into_iter().filter(|&q| clear(plan, m, q, stock, &beams, &out)).filter(|&q| !beams.iter().chain(&out).any(|b| (b.a.distance(m) < 0.05 && b.b.distance(q) < 0.05) || (b.b.distance(m) < 0.05 && b.a.distance(q) < 0.05))).take(4).collect();
                for q in chosen {
                    out.push(Beam { a: m, b: q, stock: stock.to_string(), pinned: [false; 2] });
                }
            }
        }
        if joints.iter().any(|&q| inside(q)) {
            continue;
        }
        // (Standing on a deck, light: the deck carries it, no struts. Heavy (over
        // HEAVY full): mounted to the frame, as a big tank is strapped to it.)
        let full_mass = fit.iter().find(|f| f.id == kind(&blk.id)).map_or(0.0, |f| f.mass + f.load);
        let on_deck = plan.plates.iter().any(|pl| (lo.y - pl.y).abs() < 0.1 && lo.x < pl.lo.x.max(pl.hi.x) && hi.x > pl.lo.x.min(pl.hi.x) && lo.z < pl.lo.y.max(pl.hi.y) && hi.z > pl.lo.y.min(pl.hi.y));
        if on_deck && full_mass <= HEAVY {
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
                beams.push(Beam { a: b.a, b: m, stock: b.stock.clone(), pinned: [b.pinned[0], false] });
                beams.push(Beam { a: m, b: b.b, stock: b.stock, pinned: [false, b.pinned[1]] });
            }
            let mut near: Vec<Vec3> = joints.iter().copied().filter(|&q| !inside(q) && q.distance(m) <= 15.0 && !along.contains(&q)).collect();
            near.sort_by(|a, b| a.distance(m).total_cmp(&b.distance(m)));
            // (Only where it can run clear: no module, room, deck or member in its way.)
            let near: Vec<Vec3> = near.into_iter().filter(|&q| clear(plan, m, q, stock, &beams, &out)).take(8).collect();
            // (Braced to the nearest joint below; nothing below, hung from the nearest
            // above.)
            let below = near.iter().copied().find(|q| q.y < m.y - 1.0).or_else(|| near.iter().copied().find(|q| q.y > m.y + 1.0));
            let take = if leg { 4 } else { 2 };
            let mut chosen: Vec<Vec3> = near.into_iter().take(take).collect();
            if let Some(q) = below.filter(|q| heavy && !leg && !chosen.contains(q)) {
                chosen.push(q);
            }
            // (Three struts or more: a tripod of rod ends, each pinned where it meets
            // the frame, bringing it no bending; fewer stand by their stiffness.)
            let pin = chosen.len() >= 3;
            for q in chosen {
                out.push(Beam { a: m, b: q, stock: stock.to_string(), pinned: [false, pin] });
            }
        }
    }
    // (Each deck's grid point with nothing at it: a post down to the nearest joint
    // below it, within 8 m and not through a room; none below, the nearest at all.)
    // (A point near one already held, another deck's, is held by it.)
    let mut held: Vec<Vec3> = Vec::new();
    for plate in &plan.plates {
        for node in plate_grid(plate).0 {
            if joints.iter().chain(&held).any(|q| q.distance(node) <= 0.6) {
                continue;
            }
            // (Resting on a member, part way along it (the member under it, its top
            // at the deck's underside, a seat's height at most): a joint made there,
            // the member in two; it bears the deck there.)
            if let Some((k, under)) = beams.iter().enumerate().find_map(|(k, b)| {
                let d = b.b - b.a;
                let flat = Vec3::new(d.x, 0.0, d.z);
                let t = (Vec3::new(node.x - b.a.x, 0.0, node.z - b.a.z)).dot(flat) / flat.length_squared().max(1e-6);
                let at = b.a + d * t;
                let gap = node.y - at.y;
                (t > 0.02 && t < 0.98 && Vec3::new(at.x - node.x, 0.0, at.z - node.z).length() < 0.05 && (-0.05..=0.3).contains(&gap)).then_some((k, at))
            }) {
                let b = beams.remove(k);
                beams.push(Beam { a: b.a, b: under, stock: b.stock.clone(), pinned: [b.pinned[0], false] });
                beams.push(Beam { a: under, b: b.b, stock: b.stock, pinned: [false, b.pinned[1]] });
                held.push(node);
                continue;
            }
            // (A post: steep, down if it can, up if not; never a strut lying under the
            // floor across its joists. None: it's left for the checks to say.)
            let ok = |q: &&Vec3| q.distance(node) <= 8.0 && ((**q - node).normalize_or_zero().y.abs() > 0.7) && clear(plan, node, **q, stock, &beams, &out);
            let below = joints.iter().filter(ok).filter(|q| q.y < node.y).min_by(|a, b| a.distance(node).total_cmp(&b.distance(node)));
            if let Some(&q) = below.or_else(|| joints.iter().filter(ok).min_by(|a, b| a.distance(node).total_cmp(&b.distance(node)))) {
                out.push(Beam { a: node, b: q, stock: stock.to_string(), pinned: [false; 2] });
                held.push(node);
            }
        }
    }
    for &pad in &plan.pads {
        if joints.iter().any(|q| q.distance(pad) <= 1.0) {
            continue;
        }
        let mut near: Vec<Vec3> = joints.iter().copied().filter(|&q| q.distance(pad) <= 15.0).collect();
        near.sort_by(|a, b| a.distance(pad).total_cmp(&b.distance(pad)));
        let near: Vec<Vec3> = near.into_iter().filter(|&q| clear(plan, pad, q, stock, &beams, &out)).take(4).collect();
        for q in near.into_iter().take(4) {
            out.push(Beam { a: pad, b: q, stock: stock.to_string(), pinned: [false; 2] });
        }
    }
    let added = out.len();
    beams.extend(out);
    // (Knit at its nodes.)
    let (beams, joined) = knit(beams);
    (beams, added + joined)
}

/// The members and decks as AUTO-SIZE leaves them, the rounds it took, and how many
/// need more than the biggest tube (or plate) their material comes in.
type Sized = (Vec<Beam>, Vec<Plate>, usize, usize);

/// FIT FRAME's news: a step under way, or done (the plan fitted, none: stopped;
/// and the rounds it took).
enum Fitting {
    Step(String),
    Done(Option<Box<Plan>>, usize),
}

/// How FIT FRAME mounts and sizes: the stock new mounts are cut from, their
/// spacing (m), and whether sizing may pick any material (MIX).
struct FitHow {
    stock: String,
    spacing: f32,
    mix: bool,
}

/// The frame fitted in one go: joints knit, every module mounted (members of
/// `stock`, `spacing` apart), then sized and braced in turn until bracing adds
/// nothing (six rounds at most), and sized once more after the last brace. Each
/// step said through `say`; `stop` set: given up (none).
fn fit_frame(plan: &Plan, fit: &[Fitted], spec: Option<&universe_sim::world::ship::ClassSpec>, how: &FitHow, stop: &std::sync::atomic::AtomicBool, say: &dyn Fn(Fitting)) -> Option<(Plan, usize)> {
    let (mix, stock, spacing) = (how.mix, how.stock.as_str(), how.spacing);
    use std::sync::atomic::Ordering;
    let mut plan = plan.clone();
    say(Fitting::Step("MOUNTING EVERY MODULE".into()));
    plan.beams = knit(std::mem::take(&mut plan.beams)).0;
    plan.beams = mounts(&plan, fit, spacing, stock).0;
    plan.beams = knit(std::mem::take(&mut plan.beams)).0;
    let mut rounds = 0;
    let mut braced = true;
    while braced && rounds < 6 {
        if stop.load(Ordering::Relaxed) {
            return None;
        }
        rounds += 1;
        say(Fitting::Step(format!("ROUND {rounds}: SIZING")));
        let (bs, ps, _, _) = auto_size(&plan, fit, spec, mix);
        (plan.beams, plan.plates) = (bs, ps);
        if stop.load(Ordering::Relaxed) {
            return None;
        }
        say(Fitting::Step(format!("ROUND {rounds}: BRACING WHAT'S OVER")));
        let b = bearing(&plan, fit, spec, plan.gravity());
        let (bs, n) = brace(&plan, &b);
        braced = n > 0;
        plan.beams = bs;
    }
    if braced {
        say(Fitting::Step("SIZING THE LAST BRACES".into()));
        let (bs, ps, _, _) = auto_size(&plan, fit, spec, mix);
        (plan.beams, plan.plates) = (bs, ps);
    }
    Some((plan, rounds))
}

/// Fit a saved design's frame from the command line (`freefall --fit <design>`):
/// each step timed as it goes; the design as it was kept beside it
/// (`backups/<design>.json`), then saved fitted. What it says.
pub fn fit_design(name: &str) -> String {
    let mut i = Interior::new();
    i.open_design(name);
    i.refit();
    if i.plan.blocks.is_empty() {
        return format!("no design named {name} (or it has no modules)\n");
    }
    let backup = Interior::file(name).with_file_name("backups").join(format!("{name}.json"));
    if let Err(e) = std::fs::create_dir_all(backup.parent().expect("a folder")).and_then(|_| std::fs::copy(Interior::file(name), &backup)) {
        return format!("not fitted: couldn't keep the design as it was ({e})\n");
    }
    let start = std::time::Instant::now();
    let say = |f: Fitting| {
        if let Fitting::Step(s) = f {
            println!("{:6.1} s  {s}", start.elapsed().as_secs_f64());
        }
    };
    let stock = stocks().first().map_or(String::new(), |s| s.key.clone());
    let stop = std::sync::atomic::AtomicBool::new(false);
    let was = frame_masses(&i);
    let how = FitHow { stock, spacing: i.spacing.max(1.0), mix: true };
    let Some((plan, rounds)) = fit_frame(&i.plan, &i.fit, i.spec(), &how, &stop, &say) else { return "stopped\n".into() };
    i.plan = plan;
    i.save();
    let now = frame_masses(&i);
    format!("{:6.1} s  FITTED IN {rounds} ROUNDS: FRAME {:.1} T (WAS {:.1} T); SAVED, THE OLD ONE IN {}\n", start.elapsed().as_secs_f64(), (now.0 + now.1 + now.2) / 1000.0, (was.0 + was.1 + was.2) / 1000.0, backup.display())
}

/// The plan's members sized: each round every load case worked out, and each
/// member cut from the lightest tube of its material that would carry the forces
/// it carries in each case at four fifths of its design limit (the biggest if none
/// would); again, as the forces shift with the members' stiffness, until nothing
/// changes (or `ROUNDS`; after the sixth, only bigger). The members, and the
/// rounds it took.
fn auto_size(plan: &Plan, fit: &[Fitted], spec: Option<&universe_sim::world::ship::ClassSpec>, mix: bool) -> Sized {
    // (Its decks' plates sized too: they come back with the plan's members.)
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
        let b = bearing(&plan, fit, spec, plan.gravity());
        if !b.cases.iter().any(|(_, r)| r.is_ok()) {
            return (plan.beams, plan.plates, round, 0);
        }
        let mut changed = false;
        let mut maxed = 0;
        // (A deck's supports no less stiff than its plate: a bendy joist under a stiff
        // plate sags within its own limits and bends the plate far past its.)
        let floor_stiffness = |p: Vec3| -> f64 {
            plan.plates.iter().filter(|pl| plate_grid(pl).0.iter().any(|q| q.distance(p) < 0.05)).filter_map(|pl| plate_stocks().iter().find(|s| s.key == pl.stock)).map(|s| s.material.stiffness).fold(0.0, f64::max)
        };
        let decks_on: Vec<f64> = plan.beams.iter().map(|b| floor_stiffness(b.a).max(floor_stiffness(b.b))).collect();
        // (A member lying under a deck: no fatter than the room between it and the
        // deck's underside (the deck's depth below its top), or it runs into it.)
        let under: Vec<Option<f64>> = plan.beams.iter().map(|b| {
            let level = (b.b.y - b.a.y).abs() < 0.3;
            let top = b.a.y.max(b.b.y);
            plan.plates.iter().filter(|pl| {
                let inside = |p: Vec3| p.x >= pl.lo.x.min(pl.hi.x) - 0.05 && p.x <= pl.lo.x.max(pl.hi.x) + 0.05 && p.z >= pl.lo.y.min(pl.hi.y) - 0.05 && p.z <= pl.lo.y.max(pl.hi.y) + 0.05;
                level && top < pl.y - 0.02 && top > pl.y - 1.0 && inside((b.a + b.b) * 0.5)
            }).map(|pl| f64::from(2.0 * (pl.y - deck_depth(pl) - top + TOUCH))).reduce(f64::min)
        }).collect();
        for (k, beam) in plan.beams.iter_mut().enumerate() {
            let Some(now) = stocks().iter().find(|s| s.key == beam.stock) else { continue };
            let length = f64::from(beam.a.distance(beam.b));
            let forces: Vec<universe_sim::world::frame::Forces> = b.outcomes(k).iter().map(|o| o.work.forces).collect();
            if forces.is_empty() {
                continue;
            }
            let needed = |s: &Stock| forces.iter().map(|f| work(&Member { pinned: [false; 2], a: 0, b: 1, section: s.section, material: s.material }, *f, length, sf).design).fold(0.0, f64::max);
            let ladder: Vec<&Stock> = ladder_of(&beam.stock).into_iter().filter(|s| s.material.stiffness >= decks_on[k] * 0.9).collect();
            let ladder = if ladder.is_empty() { ladder_of(&beam.stock) } else { ladder };
            // (None that fits under strong enough: a break is worse than a clash, so
            // any; the clash is said.)
            let fits: Vec<&Stock> = ladder.iter().copied().filter(|s| under[k].is_none_or(|room| s.section.diameter <= room)).collect();
            let ladder = if fits.iter().any(|s| needed(s) <= 1.0) { fits } else { ladder };
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
        // (Each deck: the lightest plate of its material (MIX: of any) whose strips
        // carry their forces at four fifths of their limit; none, the heaviest.)
        for (n, plate) in plan.plates.iter_mut().enumerate() {
            let Some(now) = plate_stocks().iter().find(|s| s.key == plate.stock) else { continue };
            let strips: Vec<_> = b.strips.iter().filter(|s| s.0 == n).collect();
            let needed = |ps: &PlateStock| strips.iter().flat_map(|&&(_, a, e, w, m)| b.of_member(m).into_iter().map(move |o| work(&Member { pinned: [false; 2], a: 0, b: 1, section: ps.section(w), material: ps.material }, o.work.forces, f64::from(a.distance(e)), sf).design)).fold(0.0, f64::max);
            // (MIX: any plate, lightest first; else its own material's.)
            let mut ladder: Vec<&PlateStock> = plate_stocks().iter().filter(|s| mix || s.of == now.of).collect();
            ladder.sort_by(|a, b| a.per_square_metre.total_cmp(&b.per_square_metre));
            let pick = ladder.iter().find(|s| needed(s) <= 0.8).copied().or_else(|| {
                maxed += usize::from(ladder.last().is_some_and(|s| needed(s) > 1.0));
                ladder.last().copied()
            });
            if let Some(pick) = pick.filter(|p| p.key != plate.stock && (round <= 6 || p.per_square_metre > now.per_square_metre)) {
                plate.stock = pick.key.clone();
                changed = true;
            }
        }
        if !changed || round == ROUNDS {
            return (plan.beams, plan.plates, round, maxed);
        }
    }
    (plan.beams, plan.plates, ROUNDS, 0)
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
        let mut ties: Vec<Vec3> = joints.iter().copied().filter(|q| q.distance(m.a) > 0.1 && q.distance(m.b) > 0.1).collect();
        ties.sort_by(|x, y| x.distance(mid).total_cmp(&y.distance(mid)));
        let tie = ties.into_iter().find(|&q| clear(plan, mid, q, &m.stock, &beams, &extra));
        extra.push(Beam { a: m.a, b: mid, stock: m.stock.clone(), pinned: [m.pinned[0], false] });
        extra.push(Beam { a: mid, b: m.b, stock: m.stock.clone(), pinned: [false, m.pinned[1]] });
        if let Some(q) = tie {
            extra.push(Beam { a: mid, b: q, stock: m.stock, pinned: [false; 2] });
        }
    }
    // (A short one past its limit: halving it wouldn't help; its weaker end (the
    // one fewer members meet at) tied to the nearest joint it isn't joined to that a
    // tube reaches clear, one below first, to hold that end still.)
    let short: Vec<usize> = (0..plan.beams.len()).filter(|&k| plan.beams[k].a.distance(plan.beams[k].b) < 2.0 && b.outcomes(k).iter().any(|o| o.work.design > 1.0 || o.broken.is_some())).collect();
    let mut tied = 0;
    for k in short {
        let m = &plan.beams[k];
        let meets = |p: Vec3| plan.beams.iter().filter(|x| x.a.distance(p) < 0.05 || x.b.distance(p) < 0.05).count();
        let end = if meets(m.a) <= meets(m.b) { m.a } else { m.b };
        let joined: Vec<Vec3> = plan.beams.iter().filter(|x| x.a.distance(end) < 0.05 || x.b.distance(end) < 0.05).map(|x| if x.a.distance(end) < 0.05 { x.b } else { x.a }).collect();
        let mut ties: Vec<Vec3> = joints.iter().copied().filter(|q| q.distance(end) > 0.1 && q.distance(end) < 8.0 && !joined.iter().any(|j| j.distance(*q) < 0.05)).collect();
        ties.sort_by(|x, y| (x.y >= end.y - 0.5).cmp(&(y.y >= end.y - 0.5)).then(x.distance(end).total_cmp(&y.distance(end))));
        if let Some(q) = ties.into_iter().find(|&q| clear(plan, end, q, &m.stock, &beams, &extra)) {
            extra.push(Beam { a: end, b: q, stock: m.stock.clone(), pinned: [false; 2] });
            tied += 1;
        }
    }
    beams.extend(extra);
    (beams, over.len() + tied)
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
        let m = Member { a: ja, b: jb, section: s.section, material: s.material, pinned: b.pinned };
        let mass = m.mass(b.a.as_dvec3(), b.b.as_dvec3());
        out.mass += mass;
        // (A member's own weight: half at each end.)
        weight.extend([(ja, mass * 0.5), (jb, mass * 0.5)]);
        frame.members.push(m);
    }
    // Its nodes: each joint's forging, its weight there.
    for (at, _, node) in nodes(&plan.beams, &roots(plan, fit)) {
        if let (Some(n), Some(j)) = (node, joints.iter().position(|q| q.distance(at) < 0.05)) {
            weight.push((j, n.mass));
            out.mass += n.mass;
        }
    }
    // Its decks: each a grid of strips of its plate, both ways, carrying its own
    // weight and its design load at its grid's points; each point tied to a joint
    // of the frame within half a metre of it (by a fitting: the stoutest steel tube).
    let frame_joints = joints.len();
    // (Each deck point's share of the design floor load: the FLOOR case's, not the
    // flight's; what is aboard is in its modules.)
    let mut floor: Vec<(usize, f64)> = Vec::new();
    let mut deck_points: Vec<usize> = Vec::new();
    let fitting = stocks().iter().filter(|s| s.of.contains("4340") && s.section.wall < s.section.diameter * 0.5).max_by(|a, b| a.per_metre.total_cmp(&b.per_metre));
    for (n, plate) in plan.plates.iter().enumerate() {
        let Some(ps) = plate_stocks().iter().find(|s| s.key == plate.stock) else { continue };
        let (nodes, xs, zs) = plate_grid(plate);
        let (nx, nz) = (xs.len() - 1, zs.len() - 1);
        let (wx, wz) = (shares(&xs), shares(&zs));
        // (A point within 0.3 m of a joint already there, another deck's or the
        // frame's, is that joint: decks meeting share their edge.)
        let at: Vec<usize> = nodes
            .iter()
            .map(|&q| {
                let near = joints.iter().enumerate().filter(|(_, p)| p.distance(q) < 0.3).min_by(|a, b| a.1.distance(q).total_cmp(&b.1.distance(q))).map(|(k, _)| k);
                near.unwrap_or_else(|| joint(q, &mut joints))
            })
            .collect();
        let node = |i: usize, j: usize| at[j * (nx + 1) + i];
        let strip = |a: usize, b: usize, width: f32, frame: &mut Frame, out: &mut Bearing| {
            out.strips.push((n, joints[a], joints[b], width, frame.members.len()));
            frame.members.push(Member { a, b, section: ps.section(width), material: ps.material, pinned: [false; 2] });
        };
        // (Strips along each line, as wide as the deck the line stands for.)
        for (j, &w) in wz.iter().enumerate() {
            for i in 0..nx {
                strip(node(i, j), node(i + 1, j), w, &mut frame, &mut out);
            }
        }
        for (i, &w) in wx.iter().enumerate() {
            for j in 0..nz {
                strip(node(i, j), node(i, j + 1), w, &mut frame, &mut out);
            }
        }
        // (Each point's share of the deck: the area its lines stand for.)
        let mut touched = false;
        for (j, &dz) in wz.iter().enumerate() {
            for (i, &dx) in wx.iter().enumerate() {
                let share = f64::from(dx * dz);
                let p = node(i, j);
                weight.push((p, share * ps.per_square_metre));
                floor.push((p, share * DECK_LOAD));
                out.mass += share * ps.per_square_metre;
                if let Some(q) = (0..frame_joints).filter(|&q| q != p && joints[q].distance(joints[p]) <= 0.6).min_by(|&a, &b| joints[a].distance(joints[p]).total_cmp(&joints[b].distance(joints[p]))) {
                    if let Some(f) = fitting {
                        frame.members.push(Member { a: p, b: q, section: f.section, material: f.material, pinned: [false; 2] });
                    }
                    touched = true;
                } else if p < frame_joints {
                    touched = true;
                }
            }
        }
        if !touched {
            out.loose.push(format!("DECK {} TOUCHES NO STRUCTURE", n + 1));
        }
        deck_points.extend(at);
    }
    // (A deck rests on what holds it: a strip between two points that both rest on
    // something is pinned at both, free to turn on them (it can't be bent by what
    // it rests on moving); a strip reaching out over nothing stays continuous.)
    let mut rests = vec![false; joints.len()];
    for m in frame.members.iter().filter(|m| m.section.flat.is_none()) {
        rests[m.a] = true;
        rests[m.b] = true;
    }
    for m in frame.members.iter_mut().filter(|m| m.section.flat.is_some()) {
        let both = rests[m.a] && rests[m.b];
        m.pinned = [both, both];
    }
    frame.joints = joints.iter().map(|p| p.as_dvec3()).collect();
    out.joints = joints.clone();
    let near = |at: Vec3, r: f32| -> Vec<usize> { joints.iter().enumerate().filter(|(_, q)| q.distance(at) <= r).map(|(k, _)| k).collect() };
    // (The joints a module is held at: those in it, or by it; a deck's floor points
    // (held up by nothing but posts) only under its foot, where it stands on the
    // deck: floor, not frame; no engine pushes on one.)
    let on_deck: Vec<bool> = (0..joints.len()).map(|j| deck_points.contains(&j)).collect();
    let mut framed = vec![false; joints.len()];
    for m in frame.members.iter().filter(|m| m.section.flat.is_none()) {
        let d = (joints[m.b] - joints[m.a]).normalize_or_zero();
        if d.y.abs() < 0.9 {
            framed[m.a] = true;
            framed[m.b] = true;
        }
    }
    let floor_only = |j: usize| on_deck[j] && !framed[j];
    let mounted = |lo: Vec3, hi: Vec3| -> Vec<usize> {
        let at: Vec<usize> = joints.iter().enumerate().filter(|&(k, q)| q.cmpge(lo - 0.3).all() && q.cmple(hi + 0.3).all() && (!floor_only(k) || q.y <= lo.y + 0.3)).map(|(k, _)| k).collect();
        if !at.is_empty() {
            return at;
        }
        // (Smaller than the deck's grid, standing on it: on the deck's points round
        // its foot, up to four, within 2 m.)
        let foot = Vec3::new((lo.x + hi.x) * 0.5, lo.y, (lo.z + hi.z) * 0.5);
        let mut near: Vec<usize> = (0..joints.len()).filter(|&k| on_deck[k] && (joints[k].y - foot.y).abs() < 0.35 && joints[k].distance(foot) < 2.0).collect();
        near.sort_by(|&a, &b| joints[a].distance(foot).total_cmp(&joints[b].distance(foot)));
        near.truncate(4);
        near
    };
    // The masses it carries: each module's (the ore bay: its hold full), shared
    // between the joints in it.
    let full = record.and_then(|h| h.capacity.hold).unwrap_or(0.0);
    let mut masses: Vec<(Vec<usize>, f64)> = weight.iter().map(|&(j, m)| (vec![j], m)).collect();
    for blk in &plan.blocks {
        let Some(f) = fit.iter().find(|f| f.id == kind(&blk.id)) else { continue };
        let (lo, hi) = blk.bounds();
        let mut at: Vec<usize> = mounted(lo, hi);
        // (Nothing at it, but in a walled room: the room carries it, on its floor.)
        if at.is_empty()
            && let Some(k) = (0..plan.lines.len()).filter(|&k| plan.group_of(k).is_some_and(|g| plan.groups[g].walled) && plan.lines[k].2.section != Section::Line).find(|&k| {
                let (p, e) = plan.axis(k);
                let pr = plan.lines[k].2;
                let d = e - p;
                let t = (blk.at - p).dot(d) / d.length_squared().max(1e-6);
                let off = blk.at - (p + d * t.clamp(0.0, 1.0));
                (0.0..=1.0).contains(&t) && off.y.abs() < pr.height * 0.5 && Vec3::new(off.x, 0.0, off.z).length() < pr.width * 0.5
            })
        {
            let (a, b) = plan.axis(k);
            let pr = plan.lines[k].2;
            let half = Vec3::new(pr.width.max(pr.height) * 0.5, pr.height * 0.5, pr.width.max(pr.height) * 0.5);
            at = mounted(a.min(b) - half, a.max(b) + half);
        }
        // (Its weight rests on its foot: the joints there, if it has any; else (hung)
        // the joints round it.)
        if f.gear.is_none() {
            let foot: Vec<usize> = at.iter().copied().filter(|&j| joints[j].y <= lo.y + 0.3).collect();
            if !foot.is_empty() {
                at = foot;
            }
        }
        // (Full: its ore, cargo or fuel too. Heavy: on the frame, not on floor.)
        let m = if blk.id == HOLD { full } else { f.mass + f.load };
        if m > HEAVY {
            let framed_at: Vec<usize> = at.iter().copied().filter(|&j| !floor_only(j)).collect();
            if !framed_at.is_empty() {
                at = framed_at;
            }
        }
        if at.is_empty() {
            out.loose.push(format!("{} IS NOT MOUNTED", f.name));
        } else {
            masses.push((at, m));
        }
    }
    // (Each walled room's walls, on what's under and in it.)
    for (k, area, ws, _) in walls(plan) {
        let (a, b) = plan.axis(k);
        let pr = plan.lines[k].2;
        let half = Vec3::new(pr.width.max(pr.height) * 0.5, pr.height * 0.5, pr.width.max(pr.height) * 0.5);
        let at = mounted(a.min(b) - half, a.max(b) + half);
        if !at.is_empty() {
            masses.push((at, area * ws.per_square_metre));
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
            let Some((dir, thrust)) = fit.iter().find(|f| f.id == kind(&blk.id)).and_then(|f| push_of(blk, f)) else { continue };
            let (lo, hi) = blk.bounds();
            // (Its push goes through its mount into the frame, never a deck.)
            let at: Vec<usize> = mounted(lo, hi).into_iter().filter(|&j| !floor_only(j)).collect();
            let role = if dir.y > 0.5 { ThrusterRole::Lift } else { ThrusterRole::Main };
            // (Swivelled, pushing some other way: it turns up to land too.)
            if role == ThrusterRole::Main && swivelled(plan, fit, blk) {
                pushers.push((ThrusterRole::Lift, DVec3::Y * thrust, at.clone()));
            }
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
        let at: Vec<usize> = mounted(lo, hi);
        if at.is_empty() {
            out.loose.push(format!("{name} HOLDS UP NOTHING"));
        }
        pads.extend(at);
    }
    // (No legs, no pads: it never lands; not a fault.)
    let lands = !legs_placed.is_empty() || !plan.pads.is_empty() || spec.is_some();
    let landing = if !lands {
        Err(NOT_LANDING.to_string())
    } else if pads.is_empty() {
        out.loose.push("NOTHING STANDS ON THE LANDING PADS".into());
        Err("NOTHING ON THE PADS".to_string())
    } else {
        Ok(Case { loads: loads(DVec3::new(0.0, jolt, 0.0)), held: pads.clone(), anchor: None })
    };
    out.felt.push(g + jolt);
    // Floor: standing on its pads at its design gravity, full (what it really
    // carries). The design floor load (a crowd, stores stacked) is local: a floor
    // must hold it anywhere, but the whole ship is never a crowd; it's added below
    // to each deck panel and to what holds each deck point up.
    let floored = if !lands { Err(NOT_STANDING.to_string()) } else if pads.is_empty() { Err("NOTHING ON THE PADS".to_string()) } else { Ok(Case { loads: loads(DVec3::ZERO), held: pads, anchor: None }) };
    // Full main thrust, and hovering on full lift: the pushes balanced by every
    // mass's inertia, held only to keep it from drifting.
    // (Nothing pushing it along but what lifts it (under a tenth of it: attitude
    // thrusters): a tail-sitter, flying along the way it lands; its lift its drive.)
    let along: f64 = pushers.iter().filter(|p| p.0 == ThrusterRole::Main).map(|p| p.1.length()).sum();
    let up: f64 = pushers.iter().filter(|p| p.0 == ThrusterRole::Lift).map(|p| p.1.length()).sum();
    let (main, total) = if along >= 0.1 * up { pushes(ThrusterRole::Main, &mut out, "THE DRIVE") } else { pushes(ThrusterRole::Lift, &mut out, "THE DRIVE") };
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
    // (The cases worked out side by side, each on a thread of its own.)
    let (no_main, no_lift, decked) = (main.is_empty(), lift.is_empty(), !floor.is_empty());
    let (landed, thrusting, hovering, standing) = std::thread::scope(|s| {
        let l = s.spawn(|| run(landing, true));
        let t = s.spawn(|| run(Ok(thrust), !no_main));
        let h = s.spawn(|| if no_lift { Err(NOT_HOVERING.to_string()) } else { run(Ok(hover), true) });
        let f = decked.then(|| s.spawn(|| run(floored, true)));
        let done = |r: std::thread::Result<_>| r.unwrap_or_else(|_| Err("THE SOLVE FAILED".to_string()));
        (done(l.join()), done(t.join()), done(h.join()), f.map(|f| done(f.join())))
    });
    out.cases = vec![(CASES[0].into(), landed), (CASES[1].into(), thrusting), (CASES[2].into(), hovering)];
    // (No decks: no floor to check.)
    if let Some(standing) = standing {
        out.felt.push(g);
        out.cases.push((CASES[3].into(), standing));
    }
    // (Standing, each deck point's floor load pushes on what holds it there: its
    // posts and fittings, shared.)
    let mut holders = vec![0usize; frame.joints.len()];
    for m in frame.members.iter().filter(|m| m.section.flat.is_none()) {
        holders[m.a] += 1;
        holders[m.b] += 1;
    }
    let mut floor_at = vec![0.0f64; frame.joints.len()];
    for &(j, m) in &floor {
        floor_at[j] += m * g;
    }
    if let Some((_, Ok(col))) = out.cases.iter_mut().find(|c| c.0 == CASES[3]) {
        for (k, m) in frame.members.iter().enumerate().filter(|(_, m)| m.section.flat.is_none()) {
            let push: f64 = [m.a, m.b].iter().filter(|&&j| floor_at[j] > 0.0).map(|&j| floor_at[j] / holders[j].max(1) as f64).sum();
            if push > 0.0 {
                let o = &mut col.members[k];
                let mut f = o.work.forces;
                f.axial -= push;
                o.work = universe_sim::world::frame::work(m, f, frame.joints[m.a].distance(frame.joints[m.b]), sf);
                if o.work.breaking >= 1.0 && o.broken.is_none() {
                    o.broken = Some(0);
                }
            }
        }
    }
    // (Each deck strip's own bending between its ends: its share of the deck's load
    // (half each way), simply supported, q L^2 / 8, and its shear q L / 2; the deck's own weight as it is
    // pushed in flight, its weight and floor load standing.)
    for (k, (name, result)) in out.cases.iter_mut().enumerate() {
        let Ok(col) = result else { continue };
        let felt = out.felt.get(k).copied().unwrap_or(g);
        for &(n, a, b, width, m) in &out.strips {
            let Some(ps) = plan.plates.get(n).and_then(|p| plate_stocks().iter().find(|s| s.key == p.stock)) else { continue };
            let per_area = if name.as_str() == CASES[3] { (ps.per_square_metre + DECK_LOAD) * g } else { ps.per_square_metre * felt };
            let l = f64::from(a.distance(b));
            let local = per_area * 0.5 * f64::from(width) * l * l / 8.0;
            let o = &mut col.members[m];
            let mut f = o.work.forces;
            f.across += local;
            // (And its shear at its ends, q L / 2: what a sandwich's core takes.)
            f.shear += per_area * 0.5 * f64::from(width) * l / 2.0;
            o.work = universe_sim::world::frame::work(&frame.members[m], f, l, sf);
            if o.work.breaking >= 1.0 && o.broken.is_none() {
                o.broken = Some(0);
            }
        }
    }
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
    /// An engine: which way it pushes the ship, as placed (none: its kind's way: a
    /// drive forward, a lift up, an engine forward).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    push: Option<Vec3>,
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
    let mut out: Vec<Fitted> = hull.fit.iter().filter_map(|f| fitted(reg.equipment(&f.item)?, &f.slot)).collect();
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
            // (An engine: pointed where it's placed; forward till then.)
            EquipmentFunction::Engine(g) => Some((Vec3::NEG_Z, g.thrust)),
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

/// Which way a placed module pushes the ship and how hard (N), if it does: as placed,
/// else its kind's way.
fn push_of(b: &Block, f: &Fitted) -> Option<(Vec3, f64)> {
    f.push.map(|(d, t)| (b.push.unwrap_or(d), t))
}

/// Whether a placed engine has a swivel at it that bears its thrust: it can turn to
/// push the ship up (to land), whichever way it's placed.
fn swivelled(plan: &Plan, fit: &[Fitted], b: &Block) -> bool {
    let thrust = fit.iter().find(|f| f.id == kind(&b.id)).and_then(|f| f.push).map_or(0.0, |p| p.1);
    let (lo, hi) = b.bounds();
    plan.blocks.iter().any(|s| {
        let (slo, shi) = s.bounds();
        let touch = slo.cmple(hi + 0.3).all() && shi.cmpge(lo - 0.3).all();
        touch && fit.iter().find(|f| f.id == kind(&s.id)).and_then(figures).is_some_and(|(k, m, _)| k == "swivel" && m.get("bears").and_then(|v| v.as_f64()).unwrap_or(0.0) >= thrust)
    })
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
    /// How far in from each end its end wall stands (a door where it meets a tube
    /// across it: at that tube's side, not in its middle).
    caps: [f32; 2],
}

impl Tube {
    /// Where along it its end `e`'s wall stands.
    fn cap(&self, e: usize) -> f32 {
        if e == 0 { self.caps[0] } else { self.len - self.caps[1] }
    }

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
            // (A door where it joins another tube: its wall across it, at the side of a
            // tube that crosses it there, square.)
            let junction = |p: usize, e: usize| !self.free_end(k, p) && self.walled_end(k, p, e);
            let cap = |p: usize, e: usize| {
                if !junction(p, e) {
                    return 0.0;
                }
                walled.iter().copied().filter(|&j| j != k && (self.lines[j].0 == p || self.lines[j].1 == p)).filter(|&j| out_of(j, p).dot(d).abs() < 0.7).map(|j| self.lines[j].2.width * 0.5).fold(0.0, f32::max)
            };
            let mitres = [mitre(a, -d).map(|n| -n), mitre(b, d)];
            let ends = [self.walled_end(k, a, 0).then(|| self.end_of(k, 0)), self.walled_end(k, b, 1).then(|| self.end_of(k, 1))];
            Tube { a: pa, d, len, u, v: u.cross(d), corners: profile.corners(), mitres, line: k, ends, hatches: [self.end_hatch(k, 0), self.end_hatch(k, 1)], caps: [cap(a, 0), cap(b, 1)] }
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
                    doorways.push(t.door_cut(t.cap(e), None, &t.hatches[e]));
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
                let s = t.cap(e);
                let mut cap: Vec<Vec3> = t.corners.iter().map(|c| t.at(s, *c)).collect();
                if e == 1 {
                    cap.reverse();
                }
                let mut pieces = vec![cap];
                // (A door where it joins another tube: the wall stands across the way
                // through, only its doorway cut.)
                let point = if e == 0 { self.lines[t.line].0 } else { self.lines[t.line].1 };
                if self.free_end(t.line, point) {
                    for (o, room) in rooms.iter().enumerate() {
                        if o != i {
                            pieces = pieces.into_iter().flat_map(|p| outside(p, room)).collect();
                        }
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
                    out.extend(t.leaf(t.cap(e), None, &t.hatches[e]));
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

    /// Whether line `k`'s end `e` (at point `p`) is walled across: a free end, or
    /// one where it joins another tube that's set a door (a door between the two).
    fn walled_end(&self, k: usize, p: usize, e: usize) -> bool {
        self.free_end(k, p) || self.ends.iter().any(|&EndSet(j, f, how, _)| j == k && f == e && how == End::Door)
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
    /// The assemblies saved, to stamp one (its name, what's in it).
    Stamp(Vec<(String, String)>),
    /// The designs saved, to compare this one with.
    Compare(Vec<(String, String)>),
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
    /// SELECT: a module placed, a frame member, a deck, a node (at its joint).
    Module(usize),
    Member(usize),
    Deck(usize),
    Node(Vec3),
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
                    self.plan.blocks.push(Block { id: format!("{}#{copy}", f.id), at: Vec3::new(x + f.size.x * 0.5, f.size.y * 0.5, 0.0), size: f.size, push: None });
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
            self.plan.blocks.push(Block { id: f.id.clone(), at: Vec3::new(0.0, floor + f.size.y * 0.5, z + f.size.z * 0.5), size: f.size, push: None });
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
        // (A module whose key the registry has renamed: given the new key, the same
        // way: the registry's change.)
        let renamed = rename(&mut self.plan.blocks, &self.fit);
        if let Some(saved) = self.saved.as_mut() {
            rename(&mut saved.blocks, &self.fit);
        }
        let resized = resize(&mut self.plan.blocks, &self.fit);
        if let Some(saved) = self.saved.as_mut() {
            resize(&mut saved.blocks, &self.fit);
        }
        let unknown = self.plan.blocks.iter().filter(|b| !self.fit.iter().any(|f| f.id == kind(&b.id))).count();
        if unknown > 0 {
            self.message = Some((format!("{unknown} MODULES NOT IN THE REGISTRY: SEE THE CHECKS"), 8.0));
        } else if renamed > 0 {
            self.message = Some((format!("{renamed} MODULES GIVEN THE KEYS THE REGISTRY RENAMED THEM TO"), 6.0));
        } else if resized > 0 {
            self.message = Some((format!("{resized} MODULES MADE THE SIZE THE REGISTRY NOW GIVES THEM"), 6.0));
        }
    }

    /// The test stand (dev scenarios): stood up, the walker at `at` (x, y, z, yaw)
    /// or at the ramp's foot.
    pub fn stand_test(&mut self, at: Option<[f64; 4]>) {
        self.refit();
        let mut s = self.stand_up();
        if let Some([x, y, z, yaw]) = at {
            (s.feet, s.yaw) = (universe_engine::glam::DVec3::new(x, y, z), yaw);
        }
        self.stand = Some(s);
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

    /// SELECT with `what` picked (dev scenarios): member:N, module:N, deck:N, room:N
    /// (a line), node (the first joint of member 0).
    pub fn select(&mut self, what: &str) {
        self.tool = Tool::Look;
        let (kind, n) = what.split_once(':').unwrap_or((what, "0"));
        let n: usize = n.parse().unwrap_or(0);
        self.pick = match kind {
            "member" => Some(Hover::Member(n)),
            "module" => Some(Hover::Module(n)),
            "deck" => Some(Hover::Deck(n)),
            "room" => Some(Hover::Line(n)),
            "node" => self.plan.beams.get(n).map(|b| Hover::Node(b.a)),
            _ => None,
        };
    }

    /// The FRAME tool in DECK mode, laying decks (dev scenarios).
    pub fn deck_tool(&mut self) {
        (self.tool, self.deck_mode) = (Tool::Frame, true);
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
        self.plan = Plan { hull: id.into(), points, lines: Vec::new(), groups: Vec::new(), ends: Vec::new(), doors: Vec::new(), blocks: Vec::new(), beams: Vec::new(), on, pads: Vec::new(), plates: Vec::new(), gravity: None, targets: None };
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
    pub(crate) fn new_design(&mut self, hull: Option<&'static universe_sim::world::ship::ClassSpec>) {
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
    pub(crate) fn open_design(&mut self, id: &str) {
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
        self.module.is_some() || self.block.is_some() || self.from.is_some() || self.beam_from.is_some() || self.truss_from.is_some() || self.truss_mode || self.deck_mode || self.beam_pick.is_some() || self.walk_armed || self.stamping
    }

    /// What's in progress cancelled; was anything?
    fn cancel(&mut self) -> bool {
        let any = self.in_progress();
        (self.module, self.block, self.from, self.beam_from, self.truss_from, self.beam_pick) = (None, None, None, None, None, None);
        (self.truss_mode, self.deck_mode, self.walk_armed, self.deck_from, self.deck_pick) = (false, false, false, None, None);
        let any = any || self.stamping || self.measuring;
        (self.stamping, self.measuring, self.measure_from) = (false, false, None);
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

/// The test stand: a design with no hull stood on the ground (its landing legs'
/// feet) and walked, first person, in the studio. What's solid: its rooms' walls and
/// floors, its modules, the ground; a ramp a slope from the ground up to the
/// airlock it serves; an airlock a passage (floor, walls, roof, open at its ends);
/// a vertical room a shaft climbed as a ladder. The walk is the game's own walker.
struct Stand {
    mesh: universe_sim::world::walk::WalkMesh,
    /// What's drawn: each face and its colour (the first two the ground, beneath
    /// all); the frame's members as lines; the rooms (axis ends, half width,
    /// half height), to know when the walker's inside.
    faces: Vec<([Vec3; 3], Color)>,
    rooms: Vec<(Vec3, Vec3, f32, f32)>,
    /// Each module's name on each face of its box: the face's middle, which way it
    /// faces, how wide it is (m), the name.
    labels: Vec<(Vec3, Vec3, f32, String)>,
    members: Vec<[Vec3; 2]>,
    /// The shafts climbed: each one's axis and radius.
    shafts: Vec<(Vec3, Vec3, f32)>,
    feet: universe_engine::glam::DVec3,
    velocity: universe_engine::glam::DVec3,
    yaw: f64,
    pitch: f64,
}

/// A box's faces (`lo` to `hi`), less those `skip` says (0..6: -y, +y, -z, +z,
/// -x, +x).
fn box_faces(lo: Vec3, hi: Vec3, skip: &[usize]) -> Vec<[Vec3; 3]> {
    let c = |x: f32, y: f32, z: f32| Vec3::new(if x < 0.5 { lo.x } else { hi.x }, if y < 0.5 { lo.y } else { hi.y }, if z < 0.5 { lo.z } else { hi.z });
    let quads = [
        [c(0., 0., 0.), c(1., 0., 0.), c(1., 0., 1.), c(0., 0., 1.)],
        [c(0., 1., 0.), c(0., 1., 1.), c(1., 1., 1.), c(1., 1., 0.)],
        [c(0., 0., 0.), c(0., 1., 0.), c(1., 1., 0.), c(1., 0., 0.)],
        [c(0., 0., 1.), c(1., 0., 1.), c(1., 1., 1.), c(0., 1., 1.)],
        [c(0., 0., 0.), c(0., 0., 1.), c(0., 1., 1.), c(0., 1., 0.)],
        [c(1., 0., 0.), c(1., 1., 0.), c(1., 1., 1.), c(1., 0., 1.)],
    ];
    quads.iter().enumerate().filter(|(k, _)| !skip.contains(k)).flat_map(|(_, q)| [[q[0], q[1], q[2]], [q[0], q[2], q[3]]]).collect()
}

impl Interior {
    /// The design stood on its test stand, ready to walk from the foot of its
    /// boarding ramp (or, with none, on the ground before it).
    fn stand_up(&self) -> Stand {
        use universe_engine::glam::DVec3;
        let kind_of = |b: &Block| self.fit.iter().find(|f| f.id == kind(&b.id)).and_then(figures).map(|(k, ..)| k).unwrap_or_default();
        let ground = self.plan.blocks.iter().filter(|b| kind_of(b) == "landing_gear").map(|b| b.at.y - b.size.y * 0.5).fold(f32::INFINITY, f32::min);
        let ground = if ground.is_finite() { ground } else { self.plan.blocks.iter().map(|b| b.at.y - b.size.y * 0.5).fold(0.0, f32::min) };
        // The ground: wide, flat, at the legs' feet (drawn first: beneath all).
        let g = 300.0;
        let ground_colour = Color([0.24, 0.27, 0.22, 1.0]);
        let mut faces: Vec<([Vec3; 3], Color)> = vec![
            ([Vec3::new(-g, ground, -g), Vec3::new(g, ground, -g), Vec3::new(g, ground, g)], ground_colour),
            ([Vec3::new(-g, ground, -g), Vec3::new(g, ground, g), Vec3::new(-g, ground, g)], ground_colour),
        ];
        // Its rooms' walls and floors.
        for p in self.panels().iter() {
            let q = &p.outline;
            let c = p.colour();
            for k in 1..q.len().saturating_sub(1) {
                faces.push(([q[0], q[k], q[k + 1]], Color(c)));
            }
        }
        // Its decks: floors (but one under a walkway: its floor is drawn with it).
        let under_walkway = |plate: &Plate| {
            let m = Vec3::new((plate.lo.x + plate.hi.x) * 0.5, plate.y, (plate.lo.y + plate.hi.y) * 0.5);
            self.plan.lines.iter().enumerate().any(|(k, &(a, b, pr))| {
                let (pa, pb) = (self.plan.points[a].at, self.plan.points[b].at);
                let d = pb - pa;
                let t = (m - pa).dot(d) / d.length_squared().max(1e-6);
                let off = m - (pa + d * t.clamp(0.0, 1.0));
                pr.stand && self.plan.group_of(k).is_some_and(|g| self.plan.groups[g].walled) && (0.0..=1.0).contains(&t) && off.y.abs() < 0.3 && Vec3::new(off.x, 0.0, off.z).length() < pr.width * 0.5
            })
        };
        for plate in self.plan.plates.iter().filter(|p| !under_walkway(p)) {
            let c = [Vec3::new(plate.lo.x, plate.y, plate.lo.y), Vec3::new(plate.hi.x, plate.y, plate.lo.y), Vec3::new(plate.hi.x, plate.y, plate.hi.y), Vec3::new(plate.lo.x, plate.y, plate.hi.y)];
            let colour = Color([0.6, 0.55, 0.45, 1.0]);
            faces.push(([c[0], c[1], c[2]], colour));
            faces.push(([c[0], c[2], c[3]], colour));
        }
        // Its modules: solid, but an airlock a passage and a ramp a slope; each one's
        // name on every face of its box, to know what's walked past.
        let airlocks: Vec<&Block> = self.plan.blocks.iter().filter(|b| kind_of(b) == "airlock").collect();
        let mut start = None;
        let mut labels = Vec::new();
        for b in &self.plan.blocks {
            if let Some(f) = self.fit.iter().find(|f| f.id == kind(&b.id)) {
                let h = b.size * 0.5;
                for n in [Vec3::X, Vec3::NEG_X, Vec3::Y, Vec3::NEG_Y, Vec3::Z, Vec3::NEG_Z] {
                    // (Its width across: a side's along the ground, the top's longer way.)
                    let wide = if n.y != 0.0 { b.size.x.max(b.size.z) } else if n.x != 0.0 { b.size.z } else { b.size.x };
                    labels.push((b.at + n * (h.dot(n.abs()) + 0.02), n, wide, f.name.clone()));
                }
            }
            let (lo, hi) = b.bounds();
            let k = kind_of(b);
            let along_z = b.size.z >= b.size.x;
            match k.as_str() {
                "airlock" => {
                    // (Open at its ends, along its length.)
                    let open = if along_z { [2, 3] } else { [4, 5] };
                    for t in box_faces(lo, hi, &open) {
                        faces.push((t, Color([0.62, 0.66, 0.7, 1.0])));
                    }
                }
                "ramp" => {
                    // (From the end nearest an airlock, at that airlock's floor, down
                    // to the ground at the far end.)
                    let lock = airlocks.iter().min_by(|a, c| a.at.distance(b.at).total_cmp(&c.at.distance(b.at)));
                    let top = lock.map_or(hi.y, |l| l.at.y - l.size.y * 0.5);
                    let (near, far, half) = if along_z {
                        let to = lock.map_or(1.0, |l| (l.at.z - b.at.z).signum());
                        (Vec3::new(b.at.x, top, b.at.z + to * b.size.z * 0.5), Vec3::new(b.at.x, ground, b.at.z - to * b.size.z * 0.5), Vec3::X * b.size.x * 0.5)
                    } else {
                        let to = lock.map_or(1.0, |l| (l.at.x - b.at.x).signum());
                        (Vec3::new(b.at.x + to * b.size.x * 0.5, top, b.at.z), Vec3::new(b.at.x - to * b.size.x * 0.5, ground, b.at.z), Vec3::Z * b.size.z * 0.5)
                    };
                    let (a, c, d, e) = (near - half, near + half, far + half, far - half);
                    let colour = Color([0.85, 0.7, 0.35, 1.0]);
                    faces.push(([a, c, d], colour));
                    faces.push(([a, d, e], colour));
                    // (Walked up from its foot, facing up it.)
                    let up = near - far;
                    start = Some((far + (far - near).normalize_or_zero() * 1.5, f64::from((-up.x).atan2(-up.z))));
                }
                _ => {
                    let round = self.round(&b.id);
                    let colour = if k == "landing_gear" { Color([0.4, 0.42, 0.46, 1.0]) } else { Color([0.55, 0.5, 0.62, 1.0]) };
                    for (t, _) in b.faces(round) {
                        faces.push((t, colour));
                    }
                }
            }
        }
        let tris: Vec<[DVec3; 3]> = faces.iter().map(|(t, _)| t.map(|p| p.as_dvec3())).collect();
        let mesh = universe_sim::world::walk::WalkMesh::new(&tris);
        // The shafts: walled rooms standing upright.
        let shafts = (0..self.plan.lines.len()).filter(|&k| self.plan.group_of(k).is_some_and(|g| self.plan.groups[g].walled)).filter_map(|k| {
            let (a, b) = self.plan.axis(k);
            ((b - a).normalize_or_zero().y.abs() > 0.9).then_some((a.min(b), a.max(b), self.plan.lines[k].2.width * 0.5))
        }).collect();
        let (feet, yaw) = start.unwrap_or_else(|| (Vec3::new(0.0, ground, -40.0), 0.0));
        Stand {
            mesh,
            faces,
            labels,
            members: self.plan.beams.iter().map(|b| [b.a, b.b]).collect(),
            // (And each airlock, a room too: along its length.)
            rooms: (0..self.plan.lines.len()).filter(|&k| self.plan.group_of(k).is_some_and(|g| self.plan.groups[g].walled)).map(|k| {
                let (a, b) = self.plan.axis(k);
                let p = self.plan.lines[k].2;
                (a, b, p.width * 0.5, p.height * 0.5)
            }).chain(airlocks.iter().map(|b| {
                let along = if b.size.z >= b.size.x { Vec3::Z * b.size.z * 0.5 } else { Vec3::X * b.size.x * 0.5 };
                (b.at - along, b.at + along, b.size.x.min(b.size.z) * 0.5, b.size.y * 0.5)
            })).collect(),
            shafts,
            feet: DVec3::new(f64::from(feet.x), f64::from(ground) + 0.05, f64::from(feet.z)),
            velocity: DVec3::ZERO,
            yaw,
            pitch: 0.0,
        }
    }
}

/// Where a walker clicked at would stand on the stand: the floor of the first room
/// the ray from `eye` along `dir` passes into; else the first floor it meets (a
/// side met: what's under that spot, backed off it a little); none if it meets
/// nothing.
fn stand_spot(s: &Stand, eye: Vec3, dir: Vec3) -> Option<universe_engine::glam::DVec3> {
    use universe_engine::glam::DVec3;
    let d = dir.as_dvec3();
    let mut from = eye.as_dvec3();
    // (Along the ray, surface after surface: the first room's floor it passes over,
    // or else the first floor it meets; a side met, what's under that spot.)
    let mut first: Option<DVec3> = None;
    for _ in 0..16 {
        let Some((t, n)) = s.mesh.ray(from, d, 2000.0) else { break };
        let hit = from + d * t;
        let level = n.y.abs() > 0.6;
        let spot = if level && d.y < 0.0 {
            Some(hit + DVec3::Y * 0.05)
        } else if !level {
            let back = hit - d * 0.4;
            s.mesh.ray(back, DVec3::NEG_Y, 2000.0).map(|(down, _)| back - DVec3::Y * (down - 0.05))
        } else {
            None
        };
        if let Some(p) = spot {
            if in_rooms(s, (p + DVec3::Y * 0.9).as_vec3()) {
                return Some(p);
            }
            first = first.or(Some(p));
        }
        from = hit + d * 0.05;
    }
    first
}

/// Is `p` inside one of the stand's rooms (or airlocks)?
fn in_rooms(s: &Stand, p: Vec3) -> bool {
    s.rooms.iter().any(|&(a, b, hw, hh)| {
        let d = b - a;
        let t = ((p - a).dot(d) / d.length_squared().max(1e-6)).clamp(0.0, 1.0);
        let off = p - (a + d * t);
        if d.normalize_or_zero().y.abs() > 0.9 { Vec2::new(off.x, off.z).length() < hw } else { off.y.abs() < hh && Vec3::new(off.x, 0.0, off.z).length() < hw }
    })
}

/// The test stand walked for a frame: WASD, the arrows or a drag to look, SPACE to
/// jump, SHIFT to run (up a shaft: forward climbs, looking down goes down); ESC
/// back to the studio.
fn stand_input(ctx: &Context, s: &mut Stand, g: f64) {
    use universe_engine::glam::DVec3;
    use universe_sim::world::walk::{Collider, Stride, Walker};
    let input = &ctx.input;
    let dt = f64::from(ctx.dt).min(0.05);
    let key = |k: KeyCode| if input.down(k) { 1.0 } else { 0.0 };
    let turn = key(KeyCode::ArrowRight) - key(KeyCode::ArrowLeft);
    let tilt = key(KeyCode::ArrowUp) - key(KeyCode::ArrowDown);
    s.yaw -= turn * 1.8 * dt;
    s.pitch = (s.pitch + tilt * 1.2 * dt).clamp(-1.4, 1.4);
    if input.button_down(MouseButton::Left) {
        s.yaw -= f64::from(input.mouse_delta.x) * 0.006;
        s.pitch = (s.pitch - f64::from(input.mouse_delta.y) * 0.006).clamp(-1.4, 1.4);
    }
    let forward = key(KeyCode::KeyW) - key(KeyCode::KeyS);
    let right = key(KeyCode::KeyD) - key(KeyCode::KeyA);
    let run = input.down(KeyCode::ShiftLeft) || input.down(KeyCode::ShiftRight);
    let (sn, co) = s.yaw.sin_cos();
    let (fwd, side) = (DVec3::new(-sn, 0.0, -co), DVec3::new(co, 0.0, -sn));
    let speed = if run { universe_sim::world::crew::RUN } else { universe_sim::world::crew::WALK };
    let wish = (fwd * forward + side * right).clamp_length_max(1.0) * speed;
    let jump = if input.pressed(KeyCode::Space) { universe_sim::world::crew::JUMP } else { 0.0 };
    let climb = forward * universe_sim::world::crew::CLIMB * if s.pitch < -0.5 { -1.0 } else { 1.0 };
    // (A shaft's ladder runs a metre past its top, so the climber's feet come level
    // with the floor there to step off.)
    let shafts = s.shafts.clone();
    let climbable = move |p: DVec3| shafts.iter().any(|&(lo, hi, r)| {
        let q = p.as_vec3();
        q.y >= lo.y - 0.3 && q.y <= hi.y + 1.0 && Vec2::new(q.x - lo.x, q.z - lo.z).length() < r
    });
    let colliders = [Collider::Mesh { mesh: &s.mesh, at: DVec3::ZERO, rot: universe_engine::glam::DQuat::IDENTITY }];
    let mut w = Walker { feet: s.feet, velocity: s.velocity };
    w.step(&colliders, &|_| DVec3::Y, g, &climbable, &Stride { wish, jump, climb }, dt);
    s.feet = w.feet;
    s.velocity = w.velocity;
}

/// The test stand seen first person: every face shaded and drawn far to near (the
/// near ones over the far), cut where it passes behind the eye; the frame's
/// members as lines among them; a cross at the middle; how to walk.
fn draw_stand(frame: &mut Frame, s: &Stand, place: &str) {
    let size = frame.size();
    frame.hud_rect(Vec2::ZERO, size, Color([0.02, 0.03, 0.06, 1.0]));
    let eye = (s.feet + universe_engine::glam::DVec3::Y * 1.65).as_vec3();
    let (sy, cy) = (s.yaw as f32).sin_cos();
    let (sp, cp) = (s.pitch as f32).sin_cos();
    let forward = Vec3::new(-sy * cp, sp, -cy * cp);
    let right = Vec3::new(cy, 0.0, -sy);
    let up = right.cross(forward);
    let focal = size.y * 0.5 / (FOV * 0.5).tan();
    let near = 0.08;
    let view = |p: Vec3| {
        let d = p - eye;
        Vec3::new(d.dot(right), d.dot(up), d.dot(forward))
    };
    let screen = |v: Vec3| size * 0.5 + Vec2::new(v.x, -v.y) * (focal / v.z);
    // (A polygon in view space cut at the near plane.)
    let clip = |poly: &[Vec3]| -> Vec<Vec3> {
        let mut out = Vec::new();
        for k in 0..poly.len() {
            let (a, b) = (poly[k], poly[(k + 1) % poly.len()]);
            if a.z >= near {
                out.push(a);
            }
            if (a.z >= near) != (b.z >= near) {
                let t = (near - a.z) / (b.z - a.z);
                out.push(a.lerp(b, t));
            }
        }
        out
    };
    let light = Vec3::new(0.4, 0.8, 0.45).normalize();
    // (Everything to draw, with its distance: faces, then the members.)
    enum Item {
        Face(Vec<Vec3>, Color),
        Member(Vec3, Vec3),
        Label(Vec2, String, f32),
    }
    let mut items: Vec<(f32, Item)> = Vec::new();
    // (The ground first, beneath all; then the rest far to near.)
    let mut ground = Vec::new();
    for (k, (t, colour)) in s.faces.iter().enumerate() {
        let v = t.map(view);
        if v.iter().all(|p| p.z < near) {
            continue;
        }
        let n = (t[1] - t[0]).cross(t[2] - t[0]).normalize_or_zero();
        let shade = 0.45 + 0.55 * n.dot(light).abs();
        let c = Color([colour.0[0] * shade, colour.0[1] * shade, colour.0[2] * shade, 1.0]);
        let poly = clip(&v);
        if poly.len() >= 3 {
            if k < 2 {
                ground.push((poly, c));
                continue;
            }
            let depth = poly.iter().map(|p| p.z).sum::<f32>() / poly.len() as f32;
            items.push((depth, Item::Face(poly, c)));
        }
    }
    // (Inside a room, its walls hide the frame round it: no members.)
    let head = eye;
    let inside_room = s.rooms.iter().any(|&(a, b, hw, hh)| {
        let d = b - a;
        let t = ((head - a).dot(d) / d.length_squared().max(1e-6)).clamp(0.0, 1.0);
        let off = head - (a + d * t);
        if d.normalize_or_zero().y.abs() > 0.9 { Vec2::new(off.x, off.z).length() < hw } else { off.y.abs() < hh + 0.3 && Vec3::new(off.x, 0.0, off.z).length() < hw }
    });
    for [a, b] in s.members.iter().filter(|_| !inside_room) {
        let (va, vb) = (view(*a), view(*b));
        if va.z < near && vb.z < near {
            continue;
        }
        let poly = clip(&[va, vb]);
        if let [p, q, ..] = poly[..] {
            items.push(((p.z + q.z) * 0.5, Item::Member(p, q)));
        }
    }
    // (Each module's name on its faces turned this way, near enough to read, drawn
    // just in front of its face: what's in front of the face hides it too.)
    for (at, n, wide, name) in &s.labels {
        if n.dot(eye - *at) <= 0.0 || at.distance(eye) > 40.0 {
            continue;
        }
        let v = view(*at);
        if v.z < near + 0.2 {
            continue;
        }
        // (About 0.3 m tall where it is, but no wider than nine tenths of its face;
        // too small then to read: none.)
        let fit = 0.9 * wide * focal / v.z / (name.chars().count() as f32 * universe_engine::frame::GLYPH);
        let scale = (0.3 * focal / v.z / 8.0).min(fit).min(2.5);
        if scale < 0.4 {
            continue;
        }
        items.push((v.z - 0.05, Item::Label(screen(v), name.clone(), scale)));
    }
    items.sort_by(|a, b| b.0.total_cmp(&a.0));
    for (poly, c) in ground {
        let q: Vec<Vec2> = poly.iter().map(|p| screen(*p)).collect();
        for k in 1..q.len() - 1 {
            frame.hud_triangle_colored([q[0], q[k], q[k + 1]], [c; 3]);
        }
    }
    for (_, item) in items {
        match item {
            Item::Face(poly, c) => {
                let q: Vec<Vec2> = poly.iter().map(|p| screen(*p)).collect();
                for k in 1..q.len() - 1 {
                    frame.hud_triangle_colored([q[0], q[k], q[k + 1]], [c; 3]);
                }
            }
            Item::Member(p, q) => frame.hud_line(screen(p), screen(q), Color([0.4, 0.95, 0.55, 0.9])),
            Item::Label(at, name, scale) => {
                let w = name.chars().count() as f32 * universe_engine::frame::GLYPH * scale;
                frame.text_scaled(at - Vec2::new(w * 0.5, 4.0 * scale), &name, Color([1.0, 0.92, 0.6, 1.0]), scale);
            }
        }
    }
    let c = size * 0.5;
    frame.hud_line(c - Vec2::new(6.0, 0.0), c + Vec2::new(6.0, 0.0), LABEL);
    frame.hud_line(c - Vec2::new(0.0, 6.0), c + Vec2::new(0.0, 6.0), LABEL);
    frame.text(Vec2::new(12.0, 10.0), &format!("{place}   TEST STAND"), LABEL);
    frame.text_scaled(Vec2::new(12.0, size.y - 18.0), "WASD WALK - DRAG OR ARROWS LOOK - SPACE JUMP - SHIFT RUN - UP A SHAFT: FORWARD CLIMBS, LOOK DOWN TO GO DOWN - ESC BACK TO THE STUDIO", LABEL.scale(0.7), 0.7);
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
        // (No hull: the design itself framed, once it has something in it; else the
        // room the studio gives it.)
        let (lo, hi) = match hull_of(&i.plan) {
            None if !i.plan.blocks.is_empty() || !i.plan.beams.is_empty() => {
                let a = Assembly { blocks: i.plan.blocks.clone(), beams: i.plan.beams.clone(), plates: i.plan.plates.clone() };
                let (lo, hi) = a.bounds();
                (lo - Vec3::splat(1.0), hi + Vec3::splat(1.0))
            }
            _ => (h.lo, h.hi),
        };
        let middle = (lo + hi) * 0.5;
        let target = i.target.unwrap_or(middle);
        let radius = (hi - lo).length() * 0.5;
        // (A flat view: from far off with a long lens, so near enough flat.)
        let flat = if i.view > 0 { 25.0 } else { 1.0 };
        let focal = size.y * 0.5 / (FOV * 0.5).tan() * flat;
        // (Far enough back that the whole hull fits, unless zoomed.)
        let distance = i.distance.unwrap_or(radius / (FOV * 0.5).tan() * 0.8) * flat;
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
const TOOLBAR: [(&str, &str, Tool); 5] = [("V", "SELECT", Tool::Look), ("P", "PATH", Tool::Path), ("D", "DOOR", Tool::Door), ("M", "MODULES", Tool::Modules), ("R", "FRAME", Tool::Frame)];

/// Where toolbar button `k` is (one row along the top).
fn button(k: usize) -> (Vec2, Vec2) {
    (Vec2::new(12.0 + k as f32 * 72.0, 30.0), Vec2::new(70.0, 16.0))
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

/// TEST DRIVE, after WALK.
fn drive_button() -> (Vec2, Vec2) {
    button(TOOLBAR.len() + HISTORY.len() + 1)
}

/// SAVE (0), OPEN (1) and NEW (2), after TEST DRIVE.
fn file_button(k: usize) -> (Vec2, Vec2) {
    button(TOOLBAR.len() + HISTORY.len() + 2 + k)
}

/// REACH, after SAVE, OPEN and NEW.
fn reach_button() -> (Vec2, Vec2) {
    button(TOOLBAR.len() + HISTORY.len() + 5)
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
        Dialog::Open(list) | Dialog::Stamp(list) | Dialog::Compare(list) => list.iter().map(|(id, on)| format!("{}   ({on})", id.to_uppercase())).collect(),
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
    FitFrame,
    /// SELECT: the picked deck's level picked; the picks copied; the copy stamped;
    /// saved as an assembly; a saved one to stamp; the level copied on top.
    Level,
    Copy,
    Paste,
    SaveAssembly,
    Assemblies,
    LevelUp,
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
    /// Laying decks (two corners on the plane; a click on one: gone), or not.
    Deck,
    /// The design's gravity down or up a tenth of a g.
    GravityDown,
    GravityUp,
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

/// What a person aboard uses a day (kg) of `item`, by the registry's needs that
/// take it (`needs`: their keys): oxygen breathed (need.air); water drunk and
/// washed in (need.water, need.washing).
fn a_day(needs: &[&str], item: &str) -> f64 {
    let reg = universe_sim::world::registry::registry();
    reg.needs.iter().filter(|n| needs.contains(&n.identity.key.as_str())).flat_map(|n| n.takes.iter()).filter(|t| t.item == item).map(|t| t.rate).sum::<f64>() * 86_400.0
}

/// A propellant's bare name, however a record names it (a material's key, a stock's:
/// "material.methalox", "stock.methalox-liq" both "methalox").
fn bare(key: &str) -> &str {
    let name = key.rsplit('.').next().unwrap_or(key);
    name.trim_end_matches("-liq").trim_end_matches("-gas")
}

/// A module heavier than this full (kg) is mounted to the frame, never just stood on
/// a deck: a deck's panels are floors, not foundations.
const HEAVY: f64 = 2000.0;

/// How often each airlock cycles a day (a figure chosen: a working ship's crew in
/// and out once), and the share of air's mass that is oxygen.
const AIRLOCK_CYCLES: f64 = 1.0;
const OXYGEN_IN_AIR: f64 = 0.232;

/// The share of a jet's power that comes aboard as heat, where its record doesn't
/// say (`function.heat_to_hull`, a product figure; the registry's first guess).
const HEAT_TO_HULL: f64 = 1e-6;

/// The design's budgets and what doesn't work: its mass (dry, full, the frame's),
/// lift against its full weight, power drawn against supplied, heat to shed
/// against radiators and coolant loops, crew and their days of air and water; and
/// each check that fails (a ramp or lift that doesn't reach the ground, an airlock
/// on no room, no way in, an ore path with a gap).
struct Budget {
    lines: Vec<(String, bool)>,
    faults: Vec<String>,
}

/// What a walled room's opening leads to: space through nothing (an open end: a
/// leak), space through a hatch alone (opening it vents the room), space through
/// an airlock (the way in), or another room.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Opening {
    Leak,
    ToSpace,
    Airlock,
    Inner,
}

/// The plan's pressure: its sealed spaces (walled rooms joined end to end or
/// through a hatch: each one's volume, m³, and whether an airlock leads into it),
/// and each opening of a room to space or another room (where, what it leads to).
struct Pressure {
    spaces: Vec<(f64, bool)>,
    openings: Vec<(Vec3, Opening)>,
}

/// A cross-section's area (m²): its shape at its width and height.
fn section_area(p: Profile) -> f32 {
    let wh = p.width * p.height;
    match p.section {
        Section::Line => 0.0,
        Section::Round => std::f32::consts::PI / 4.0 * wh,
        Section::Square => wh,
        Section::Hex => 0.75 * wh,
        Section::Oct => 0.83 * wh,
    }
}

fn pressure(i: &Interior) -> Pressure {
    let plan = &i.plan;
    let walled: Vec<usize> = (0..plan.lines.len()).filter(|&k| plan.group_of(k).is_some_and(|g| plan.groups[g].walled) && plan.lines[k].2.section != Section::Line).collect();
    // (Which walled room a point is in, other than `not`.)
    let room_at = |q: Vec3, not: usize| walled.iter().copied().find(|&k| {
        if k == not {
            return false;
        }
        let (a, b) = plan.axis(k);
        let pr = plan.lines[k].2;
        let d = b - a;
        let t = (q - a).dot(d) / d.length_squared().max(1e-6);
        let off = q - (a + d * t.clamp(0.0, 1.0));
        (-0.05..=1.05).contains(&t) && off.y.abs() < pr.height * 0.5 && Vec3::new(off.x, 0.0, off.z).length() < pr.width * 0.5
    });
    let airlocks: Vec<&Block> = plan.blocks.iter().filter(|b| i.fit.iter().find(|f| f.id == kind(&b.id)).and_then(figures).is_some_and(|(k, ..)| k == "airlock")).collect();
    // (A way in: an airlock placed at it, or the hull's own hatch or door.)
    let entries: Vec<Vec3> = plan.points.iter().filter(|q| q.name.as_deref().is_some_and(|n| n == "HATCH" || n.starts_with("DOOR"))).map(|q| q.at).collect();
    let at_airlock = |q: Vec3| airlocks.iter().any(|b| {
        let (lo, hi) = b.bounds();
        q.cmpge(lo - 1.2).all() && q.cmple(hi + 1.2).all()
    }) || entries.iter().any(|e| e.distance(q) < 2.5);
    // (A cabin is pressurised itself: a hatch onto one opens into it.)
    let cabins: Vec<&Block> = plan.blocks.iter().filter(|b| i.fit.iter().find(|f| f.id == kind(&b.id)).and_then(figures).is_some_and(|(k, ..)| k == "cabin")).collect();
    let in_cabin = |q: Vec3| cabins.iter().any(|b| {
        let (lo, hi) = b.bounds();
        q.cmpge(lo - 0.5).all() && q.cmple(hi + 0.5).all()
    });
    // (Spaces: rooms sharing a point, or a hatch between them.)
    let mut root: Vec<usize> = (0..plan.lines.len()).collect();
    fn find(root: &mut [usize], k: usize) -> usize {
        if root[k] != k {
            let r = find(root, root[k]);
            root[k] = r;
        }
        root[k]
    }
    let join = |root: &mut Vec<usize>, a: usize, b: usize| {
        let (x, y) = (find(root, a), find(root, b));
        root[x] = y;
    };
    for (n, &a) in walled.iter().enumerate() {
        for &b in &walled[n + 1..] {
            let (la, lb) = (plan.lines[a], plan.lines[b]);
            if la.0 == lb.0 || la.0 == lb.1 || la.1 == lb.0 || la.1 == lb.1 {
                join(&mut root, a, b);
            }
        }
    }
    let mut openings = Vec::new();
    let mut ways_in: Vec<usize> = Vec::new();
    // (Each opening: where it is, which way is out of its room, its line, a hatch?)
    let mut holes: Vec<(Vec3, Vec3, usize, bool)> = Vec::new();
    for &k in &walled {
        let (a, b) = plan.axis(k);
        let d = (b - a).normalize_or_zero();
        for e in 0..2 {
            let point = if e == 0 { plan.lines[k].0 } else { plan.lines[k].1 };
            if !plan.walled_end(k, point, e) {
                continue;
            }
            let (at, out) = if e == 0 { (a, -d) } else { (b, d) };
            // (A door where it joins another walkway: into that walkway.)
            if !plan.free_end(k, point)
                && let Some(&j) = walled.iter().find(|&&j| j != k && (plan.lines[j].0 == point || plan.lines[j].1 == point))
            {
                join(&mut root, k, j);
                openings.push((at, Opening::Inner));
                continue;
            }
            match plan.end_of(k, e) {
                End::Closed => {}
                End::Open => holes.push((at, out, k, false)),
                End::Door => holes.push((at, out, k, true)),
            }
        }
    }
    for &SideDoor(k, t, right, _) in &plan.doors {
        if !walled.contains(&k) {
            continue;
        }
        let (a, b) = plan.axis(k);
        let d = (b - a).normalize_or_zero();
        let u = d.cross(Vec3::Y).try_normalize().unwrap_or(Vec3::X);
        let side = if right { u } else { -u };
        let w = plan.lines[k].2.width * 0.5;
        holes.push((a.lerp(b, t) + side * w, side, k, true));
    }
    for (at, out, k, hatch) in holes {
        let beyond = at + out * 0.8;
        let what = match room_at(beyond, k) {
            Some(other) => {
                join(&mut root, k, other);
                Opening::Inner
            }
            None if hatch && in_cabin(beyond) => Opening::Inner,
            None if !hatch && at_airlock(beyond) => {
                ways_in.push(k);
                Opening::Airlock
            }
            None if !hatch => Opening::Leak,
            None if at_airlock(beyond) => {
                ways_in.push(k);
                Opening::Airlock
            }
            None => Opening::ToSpace,
        };
        openings.push((at, what));
    }
    // (Each space's volume, and whether a way in leads to it.)
    let mut spaces: Vec<(usize, f64, bool)> = Vec::new();
    for &k in &walled {
        let r = find(&mut root, k);
        let (a, b) = plan.axis(k);
        let v = f64::from(section_area(plan.lines[k].2) * a.distance(b));
        let way = ways_in.iter().any(|&w| find(&mut root, w) == r);
        match spaces.iter_mut().find(|s| s.0 == r) {
            Some(s) => {
                s.1 += v;
                s.2 |= way;
            }
            None => spaces.push((r, v, way)),
        }
    }
    Pressure { spaces: spaces.into_iter().map(|s| (s.1, s.2)).collect(), openings }
}

/// The oxygen in a cubic metre of air at a ship's pressure (kg: 21 kPa of it).
const OXYGEN_A_CUBIC_METRE: f64 = 0.28;

/// A record's function's fields, by name.
type Fields = serde_json::Map<String, serde_json::Value>;

/// A placed module's record's figures: its function's kind and fields, and the
/// power it draws (W).
fn figures(f: &Fitted) -> Option<(String, Fields, f64)> {
    let reg = universe_sim::world::registry::registry();
    let e = reg.equipment(&f.key)?;
    let serde_json::Value::Object(mut map) = serde_json::to_value(&e.function).ok()? else { return None };
    let kind = map.get("kind").and_then(|v| v.as_str()).unwrap_or("").to_string();
    let power = e.needs.power.unwrap_or(0.0);
    // (A ducted fan draws its power only while it lifts: kept out of the steady
    // draw, its full-thrust draw kept for hovering's sums.)
    if map.get("cycle").and_then(|v| v.as_str()) == Some("ducted_fan") {
        map.insert("full_power".into(), serde_json::json!(power));
        return Some((kind, map, 0.0));
    }
    // (A jump drive draws while it runs a jump, not all the while: likewise.)
    if kind == "hyperdrive" {
        map.insert("run_power".into(), serde_json::json!(power));
        return Some((kind, map, 0.0));
    }
    Some((kind, map, power))
}

/// A battery pack's heat capacity (J/kg K) and how far it may warm in a hover (K),
/// for a pack whose record doesn't say (its specific heat; its hottest): assumed,
/// lithium cells' (about 1 kJ/kg K; kept below about 60 °C), to review. The dock's
/// temperature (K), the packs' at the start.
const PACK_HEAT_CAPACITY: f64 = 1000.0;
const PACK_WARMING: f64 = 40.0;
const DOCK_TEMPERATURE: f64 = 293.0;

/// The design as a craft to test drive: every part's mass where it sits (modules
/// full, members, decks), its middle of mass and inertia from them (each module a
/// solid box); each engine and fan pushing from where it is; each leg a spring at
/// its foot; the batteries and plants; what the rest draws.
fn craft(i: &Interior) -> crate::test_drive::Craft {
    use crate::test_drive::{Actuator, Craft, Kind, Leg};
    use universe_engine::glam::{DMat3, DVec3};
    let num = |m: &Fields, k: &str| m.get(k).and_then(|v| v.as_f64()).unwrap_or(0.0);
    let reg = universe_sim::world::registry::registry();
    let mut placed: Vec<(&Block, &Fitted, String, Fields, f64)> = Vec::new();
    for b in &i.plan.blocks {
        if let Some(f) = i.fit.iter().find(|f| f.id == kind(&b.id))
            && let Some((k, m, draw)) = figures(f)
        {
            placed.push((b, f, k, m, draw));
        }
    }
    // (Each mass: where, how much, its box's size (none: a point).)
    let mut masses: Vec<(DVec3, f64, DVec3)> = placed.iter().map(|p| (p.0.at.as_dvec3(), p.1.mass + p.1.load, p.0.size.as_dvec3())).collect();
    masses.extend(i.plan.beams.iter().filter_map(|b| stocks().iter().find(|s| s.key == b.stock).map(|s| (((b.a + b.b) * 0.5).as_dvec3(), s.per_metre * f64::from(b.a.distance(b.b)), DVec3::ZERO))));
    masses.extend(i.plan.plates.iter().filter_map(|p| plate_stocks().iter().find(|s| s.key == p.stock).map(|s| (DVec3::new(f64::from(p.lo.x + p.hi.x) * 0.5, f64::from(p.y), f64::from(p.lo.y + p.hi.y) * 0.5), s.per_square_metre * f64::from(((p.hi.x - p.lo.x) * (p.hi.y - p.lo.y)).abs()), DVec3::new(f64::from(p.hi.x - p.lo.x).abs(), 0.02, f64::from(p.hi.y - p.lo.y).abs())))));
    let (m_nodes, n_nodes) = { let ms = frame_masses(i); (ms.1, i.plan.beams.len().max(1)) };
    let _ = n_nodes;
    let total: f64 = masses.iter().map(|m| m.1).sum::<f64>() + m_nodes;
    let middle = masses.iter().fold(DVec3::ZERO, |c, m| c + m.0 * m.1) / masses.iter().map(|m| m.1).sum::<f64>().max(1.0);
    // (Inertia: each box's own, and its mass at its distance.)
    let mut inertia = DMat3::ZERO;
    for (at, m, s) in &masses {
        let r = *at - middle;
        let own = DMat3::from_diagonal(DVec3::new(s.y * s.y + s.z * s.z, s.x * s.x + s.z * s.z, s.x * s.x + s.y * s.y) * (m / 12.0));
        let shift = (DMat3::IDENTITY * r.length_squared() - DMat3::from_cols(r * r.x, r * r.y, r * r.z)) * *m;
        inertia += own + shift;
    }
    let mid32 = middle.as_vec3();
    let actuators = placed.iter().filter_map(|p| {
        let (dir, thrust) = push_of(p.0, p.1)?;
        let kind = if p.3.get("full_power").is_some() { Kind::Fan { full_power: num(&p.3, "full_power"), min_density: num(&p.3, "min_density") } } else { Kind::Rocket { exhaust: num(&p.3, "exhaust").max(1.0) } };
        Some(Actuator { at: (p.0.at - mid32).as_dvec3(), dir: dir.normalize_or_zero().as_dvec3(), thrust, kind })
    }).collect();
    let legs = placed.iter().filter_map(|p| {
        let (holds, stroke, _, sink) = p.1.gear?;
        let (lo, _) = p.0.bounds();
        Some(Leg { foot: (Vec3::new(p.0.at.x, lo.y, p.0.at.z) - mid32).as_dvec3(), holds, stroke, sink })
    }).collect();
    let of = |k: &'static str| placed.iter().filter(move |p| p.2 == k);
    let rate: f64 = of("battery").map(|p| num(&p.3, "rate")).sum();
    let efficiency = if rate > 0.0 { of("battery").map(|p| num(&p.3, "rate") * p.3.get("efficiency").and_then(|v| v.as_f64()).unwrap_or(1.0)).sum::<f64>() / rate } else { 1.0 };
    let phys = |p: &&(&Block, &Fitted, String, Fields, f64)| reg.equipment(&p.1.key).map(|e| (e.physical.specific_heat, e.physical.operating_max_temperature));
    let warming = of("battery").filter_map(|p| phys(&p).and_then(|x| x.1)).fold(f64::MAX, f64::min);
    let rockets: Vec<String> = placed.iter().filter(|p| p.1.push.is_some() && p.3.get("full_power").is_none()).filter_map(|p| p.3.get("propellant").filter(|v| !v.is_null()).or(p.3.get("burns")).and_then(|v| v.as_str()).map(|s| bare(s).to_string())).collect();
    let boxes: Vec<(Vec3, Vec3)> = i.plan.blocks.iter().map(|b| { let (lo, hi) = b.bounds(); (lo - mid32, hi - mid32) }).collect();
    let (lo, hi) = boxes.iter().fold((Vec3::splat(f32::MAX), Vec3::splat(f32::MIN)), |(a, z), b| (a.min(b.0), z.max(b.1)));
    Craft {
        name: i.plan.hull.clone(),
        mass: total,
        inertia,
        actuators,
        legs,
        stored: of("battery").map(|p| num(&p.3, "stores")).sum(),
        rate,
        efficiency,
        heat_capacity: of("battery").map(|p| p.1.mass * phys(&p).and_then(|x| x.0).unwrap_or(PACK_HEAT_CAPACITY)).sum(),
        warming: if warming == f64::MAX { PACK_WARMING } else { warming - DOCK_TEMPERATURE },
        plants: of("power_plant").map(|p| num(&p.3, "output")).sum(),
        steady: placed.iter().map(|p| p.4).sum(),
        propellant: of("tank").filter(|t| t.3.get("holds").and_then(|v| v.as_str()).is_some_and(|h| rockets.iter().any(|r| r == bare(h)))).map(|t| num(&t.3, "capacity")).sum(),
        leg_boxes: i.plan.blocks.iter().map(|b| i.fit.iter().find(|f| f.id == kind(&b.id)).is_some_and(|f| f.gear.is_some())).collect(),
        boxes,
        members: i.plan.beams.iter().map(|b| [b.a - mid32, b.b - mid32]).collect(),
        gravity: i.plan.gravity(),
        area: f64::from(((hi.x - lo.x) * (hi.z - lo.z)).max(1.0)),
    }
}

impl Interior {
    /// The balance room opened (dev: `freefall --studio` with UNIVERSE_BALANCE: T
    /// trims it if set to "trim").
    pub fn balance_room(&mut self, trimmed: bool) {
        let mut b = crate::test_drive::Balance::new(craft(self));
        b.set_trim(trimmed);
        self.balance = Some(b);
    }

    /// TEST DRIVE started (dev: `freefall --studio` with UNIVERSE_DRIVE: its
    /// collective set there).
    pub fn test_drive(&mut self, collective: Option<f64>) {
        let mut d = crate::test_drive::Drive::new(craft(self));
        if let Some(c) = collective {
            d.set_collective(c);
            d.set_assist(std::env::var_os("UNIVERSE_DRIVE_ASSIST").is_some());
        }
        self.drive = Some(d);
    }
}

/// A craft that never leaves the air: lifted on fans, with no jump drive and no
/// rocket (an engine throwing propellant faster than 500 m/s, over 10 kN). Its doors
/// open to the air, not to space: no airlock needed.
fn air_only(placed: &[(&Block, &Fitted, String, Fields, f64)]) -> bool {
    let num = |m: &Fields, k: &str| m.get(k).and_then(|v| v.as_f64()).unwrap_or(0.0);
    let rocket = placed.iter().any(|p| p.1.push.is_some() && p.3.get("full_power").is_none() && num(&p.3, "exhaust") > 500.0 && num(&p.3, "thrust") > 10_000.0);
    placed.iter().any(|p| p.3.get("full_power").is_some()) && !rocket && !placed.iter().any(|p| p.2 == "hyperdrive")
}

/// Days, to a tenth when under ten.
fn short_days(d: f64) -> String {
    if d < 10.0 { format!("{d:.1}") } else { format!("{d:.0}") }
}

/// The budgets' target lines: the delta-v asked (with the cargo asked aboard)
/// against what its main engines' propellant gives, and the propellant (and tanks,
/// at the tanks' own mass for what they hold) that would give it, and its thrust
/// over weight then; the crew asked against its seats and its life support.
fn target_lines(t: &Targets, placed: &[(&Block, &Fitted, String, Fields, f64)], dry: f64, full: f64, g: f64) -> Vec<(String, bool)> {
    let mut out = Vec::new();
    let num = |m: &Fields, k: &str| m.get(k).and_then(|v| v.as_f64()).unwrap_or(0.0);
    let of = |k: &'static str| placed.iter().filter(move |p| p.2 == k);
    let cargo = t.cargo.unwrap_or(0.0);
    let thrust: f64 = placed.iter().filter_map(|p| p.1.push.map(|x| x.1)).sum();
    // (The propellant its strongest engines throw: theirs, and the tanks that hold it.)
    let engines: Vec<_> = placed.iter().filter(|p| p.1.push.is_some() && p.3.get("exhaust").is_some() && p.3.get("full_power").is_none()).collect();
    let strongest = engines.iter().map(|e| num(&e.3, "thrust")).fold(0.0, f64::max);
    let main: Vec<_> = engines.iter().filter(|e| num(&e.3, "thrust") >= strongest * 0.5).collect();
    let fuel = main.iter().find_map(|e| e.3.get("propellant").filter(|v| !v.is_null()).or(e.3.get("burns")).and_then(|v| v.as_str())).map(bare);
    let tanks: Vec<_> = of("tank").filter(|p| p.3.get("holds").and_then(|v| v.as_str()).map(bare) == fuel).collect();
    let held: f64 = tanks.iter().map(|p| num(&p.3, "capacity")).sum();
    let k = if held > 0.0 { tanks.iter().map(|p| p.1.mass).sum::<f64>() / held } else { 0.1 };
    let pushing: f64 = main.iter().map(|e| num(&e.3, "thrust")).sum();
    let exhaust = main.iter().map(|e| num(&e.3, "thrust") * num(&e.3, "exhaust")).sum::<f64>() / pushing.max(1.0);
    let name = fuel.unwrap_or("PROPELLANT").to_uppercase();
    let has = if held > 0.0 && exhaust > 0.0 { exhaust * ((full + cargo) / (full + cargo - held)).ln() } else { 0.0 };
    let with = if cargo > 0.0 { format!(" WITH {:.1} T CARGO", cargo / 1000.0) } else { String::new() };
    if let Some(dv) = t.delta_v {
        let text = if has >= dv {
            format!("TARGET DELTA-V {:.2} KM/S{with}: HAS {:.2}; MET", dv / 1000.0, has / 1000.0)
        } else if exhaust <= 0.0 {
            format!("TARGET DELTA-V {:.2} KM/S: NO ENGINE THROWS PROPELLANT", dv / 1000.0)
        } else {
            // (Without its tanks of it, then the propellant P that gives dv: (D + (1 + k)P) / (D + kP) = r.)
            let bare_ship = dry + cargo - k * held;
            let r = (dv / exhaust).exp();
            let room = 1.0 + k - r * k;
            if room <= 0.0 {
                format!("TARGET DELTA-V {:.2} KM/S{with}: HAS {:.2}; NOT ON {name} IN THESE TANKS, ANY AMOUNT: STAGE IT", dv / 1000.0, has / 1000.0)
            } else {
                let p = bare_ship * (r - 1.0) / room;
                let tw = thrust / ((bare_ship + (1.0 + k) * p) * g);
                let lifts = if tw < 1.0 { ", TOO LITTLE TO LIFT IT: STAGE IT OR ADD THRUST" } else { "" };
                format!("TARGET DELTA-V {:.2} KM/S{with}: HAS {:.2}; +{:.1} T {name} IN +{:.1} T OF TANKS: T/W THEN {tw:.2}{lifts}", dv / 1000.0, has / 1000.0, (p - held) / 1000.0, k * (p - held) / 1000.0)
            }
        };
        out.push((text, has >= dv));
    } else if cargo > 0.0 {
        let tw = thrust / ((full + cargo) * g);
        out.push((format!("TARGET CARGO {:.1} T: DELTA-V WITH IT {:.2} KM/S, T/W {tw:.2}", cargo / 1000.0, has / 1000.0), tw > 1.0));
    }
    if let Some(n) = t.crew {
        let seats = of("command_station").map(|p| num(&p.3, "persons")).sum::<f64>() + of("cabin").map(|p| num(&p.3, "seats")).sum::<f64>();
        let keeps: f64 = of("life_support").map(|p| num(&p.3, "persons")).sum();
        let mut need = Vec::new();
        if seats < n {
            need.push(format!("{:.0} MORE SEATS (A CABIN)", n - seats));
        }
        if keeps < n {
            need.push(format!("LIFE SUPPORT FOR {:.0} MORE", n - keeps));
        }
        let gap = if need.is_empty() { "MET".to_string() } else { need.join(", ") };
        out.push((format!("TARGET CREW {n:.0}: SEATS {seats:.0}, LIFE SUPPORT KEEPS {keeps:.0}; {gap}"), seats >= n && keeps >= n));
    }
    if let Some(h) = t.trip {
        let needs: Vec<&str> = [(NEEDS_HEAD, "A HEAD"), (NEEDS_GALLEY, "A GALLEY"), (NEEDS_BERTHS, "BERTHS")].iter().filter(|n| h > n.0).map(|n| n.1).collect();
        out.push((format!("TARGET TRIP {h:.1} H: {}", if needs.is_empty() { "NO HEAD, GALLEY OR BERTHS NEEDED".to_string() } else { format!("NEEDS {}", needs.join(", ")) }), true));
    }
    out
}

/// A design's figure to compare: its name, its value, its unit (as shown: T, M,
/// KM/S...), and how many places to show.
type Figure = (&'static str, f64, &'static str, usize);

/// A design's key figures, to compare designs by: masses, size, thrust and T/W,
/// delta-v on its main engines' propellant, lift, crew and life support, power and
/// heat in hand, and how many issues it has (the checks as worked out here).
fn key_figures(i: &Interior) -> Vec<Figure> {
    let num = |m: &Fields, k: &str| m.get(k).and_then(|v| v.as_f64()).unwrap_or(0.0);
    let mut placed: Vec<(&Block, &Fitted, String, Fields, f64)> = Vec::new();
    for b in &i.plan.blocks {
        if let Some(f) = i.fit.iter().find(|f| f.id == kind(&b.id))
            && let Some((k, m, draw)) = figures(f)
        {
            placed.push((b, f, k, m, draw));
        }
    }
    let of = |k: &'static str| placed.iter().filter(move |p| p.2 == k);
    let (members, nodes, decks) = frame_masses(i);
    let frame = members + nodes + decks;
    let walls: f64 = walls(&i.plan).iter().map(|w| w.1 * w.2.per_square_metre).sum();
    let mods: f64 = i.plan.blocks.iter().filter_map(|b| i.fit.iter().find(|f| f.id == kind(&b.id))).map(|f| f.mass).sum();
    let load: f64 = i.plan.blocks.iter().filter_map(|b| i.fit.iter().find(|f| f.id == kind(&b.id))).map(|f| f.load).sum();
    let (dry, full) = (mods + frame + walls, mods + frame + walls + load);
    let a = Assembly { blocks: i.plan.blocks.clone(), beams: i.plan.beams.clone(), plates: i.plan.plates.clone() };
    let (lo, hi) = a.bounds();
    let g = i.plan.gravity();
    let thrust: f64 = placed.iter().filter_map(|p| p.1.push.map(|x| x.1)).sum();
    let lift: f64 = placed.iter().filter(|p| push_of(p.0, p.1).is_some_and(|(d, _)| d.y > 0.5) || (p.1.push.is_some() && swivelled(&i.plan, &i.fit, p.0))).filter_map(|p| p.1.push.map(|x| x.1)).sum();
    // (Delta-v: what target_lines works out, with nothing asked.)
    let dv = target_lines(&Targets { delta_v: Some(f64::MAX), ..Default::default() }, &placed, dry, full, g).first().and_then(|l| l.0.split("HAS ").nth(1)).and_then(|t| t.split(';').next()).and_then(|t| t.trim().parse::<f64>().ok()).unwrap_or(0.0);
    let seats = of("command_station").map(|p| num(&p.3, "persons")).sum::<f64>() + of("cabin").map(|p| num(&p.3, "seats")).sum::<f64>();
    let keeps: f64 = of("life_support").map(|p| num(&p.3, "persons")).sum();
    let supply: f64 = of("power_plant").map(|p| num(&p.3, "output")).sum::<f64>() + of("solar_array").map(|p| num(&p.3, "output")).sum::<f64>();
    let draw: f64 = placed.iter().map(|p| p.4).sum();
    let shed: f64 = of("radiator").map(|p| num(&p.3, "rejects")).sum();
    let b = budget(i);
    let issues = issues(i, &b.faults).len() as f64;
    vec![
        ("DRY MASS", dry / 1000.0, "T", 1),
        ("FULL MASS", full / 1000.0, "T", 1),
        ("FRAME", frame / 1000.0, "T", 1),
        ("WIDE", f64::from(hi.x - lo.x), "M", 1),
        ("TALL", f64::from(hi.y - lo.y), "M", 1),
        ("DEEP", f64::from(hi.z - lo.z), "M", 1),
        ("THRUST", thrust / 1e6, "MN", 2),
        ("T/W FULL", thrust / (full * g).max(1.0), "", 2),
        ("T/W DRY", thrust / (dry * g).max(1.0), "", 2),
        ("DELTA-V", dv, "KM/S", 2),
        ("LIFT / WEIGHT", lift / (full * g).max(1.0), "", 2),
        ("SEATS", seats, "", 0),
        ("LIFE SUPPORT KEEPS", keeps, "", 0),
        ("POWER SPARE", (supply - draw) / 1e6, "MW", 2),
        ("RADIATORS", shed / 1e6, "MW", 1),
        ("ISSUES", issues, "", 0),
    ]
}

/// Two designs' figures as rows of a table: the figure, this one's, the other's,
/// the difference (other less this).
fn compare_rows(ours: &[Figure], theirs: &[Figure]) -> Vec<[String; 4]> {
    ours.iter().zip(theirs).map(|(a, b)| {
        let f = |v: f64| format!("{v:.p$} {}", a.2, p = a.3).trim().to_string();
        let d = b.1 - a.1;
        [a.0.to_string(), f(a.1), f(b.1), if d.abs() < 1e-9 { "=".into() } else { format!("{}{}", if d > 0.0 { "+" } else { "" }, f(d)) }]
    }).collect()
}

/// A saved design opened and checked (its loads, clashes and nodes worked out, as
/// the studio works them out in the background).
fn checked(name: &str) -> Interior {
    let mut i = Interior::new();
    i.open_design(name);
    i.refit();
    i.node_cache = Some((i.plan.clone(), nodes(&i.plan.beams, &roots(&i.plan, &i.fit))));
    i.member_clash = Some((i.plan.clone(), member_clashes(&i.plan)));
    let b = bearing(&i.plan, &i.fit, i.spec(), i.plan.gravity());
    i.bearing = Some((i.plan.clone(), Arc::new(b)));
    i
}

/// Two saved designs compared, as text (`freefall --compare <a> <b>`).
pub fn compare_designs(a: &str, b: &str) -> String {
    let (x, y) = (checked(a), checked(b));
    let mut out = format!("{:<20} {:>14} {:>14} {:>14}\n", "FIGURE", a.to_uppercase(), b.to_uppercase(), "DIFFERENCE");
    for r in compare_rows(&key_figures(&x), &key_figures(&y)) {
        out += &format!("{:<20} {:>14} {:>14} {:>14}\n", r[0], r[1], r[2], r[3]);
    }
    out
}

/// The frame's mass by part: its members, its nodes, its decks (kg). One figure
/// for the studio and the report alike.
fn frame_masses(i: &Interior) -> (f64, f64, f64) {
    let members: f64 = i.plan.beams.iter().filter_map(|m| stocks().iter().find(|s| s.key == m.stock).map(|s| s.per_metre * f64::from(m.a.distance(m.b)))).sum();
    let decks: f64 = i.plan.plates.iter().filter_map(|p| plate_stocks().iter().find(|s| s.key == p.stock).map(|s| s.per_square_metre * f64::from(((p.hi.x - p.lo.x) * (p.hi.y - p.lo.y)).abs()))).sum();
    let node = |ns: &[Node]| ns.iter().filter_map(|n| n.2).map(|n| n.mass).sum::<f64>();
    let nodes = match i.node_cache.as_ref().filter(|(p, _)| *p == i.plan) {
        Some((_, ns)) => node(ns),
        None => node(&nodes(&i.plan.beams, &roots(&i.plan, &i.fit))),
    };
    (members, nodes, decks)
}

fn budget(i: &Interior) -> Budget {
    let (members, nodes, decks) = frame_masses(i);
    let frame_mass = members + nodes + decks;
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
    // (The rooms' walls, each the lightest stock that holds the cabin pressure.)
    let ws = walls(&i.plan);
    let wall_mass: f64 = ws.iter().map(|w| w.1 * w.2.per_square_metre).sum();
    let dry: f64 = i.plan.blocks.iter().filter_map(|b| i.fit.iter().find(|f| f.id == kind(&b.id))).map(|f| f.mass).sum::<f64>() + frame_mass + wall_mass;
    let full = dry + i.plan.blocks.iter().filter_map(|b| i.fit.iter().find(|f| f.id == kind(&b.id))).map(|f| f.load).sum::<f64>();
    let mut lines = Vec::new();
    let t = |kg: f64| format!("{:.1} T", kg / 1000.0);
    lines.push((format!("MASS  DRY {}  FULL {}  (FRAME {})", t(dry), t(full), t(frame_mass)), true));
    if !ws.is_empty() {
        // (The walls: their area and mass, the stock most of it is, and whether every
        // room's holds the cabin pressure.)
        let area: f64 = ws.iter().map(|w| w.1).sum();
        let worst = ws.iter().map(|w| w.3).fold(0.0, f64::max);
        let mut by: Vec<(&str, f64)> = Vec::new();
        for w in &ws {
            match by.iter_mut().find(|b| b.0 == w.2.name.as_str()) {
                Some(b) => b.1 += w.1,
                None => by.push((w.2.name.as_str(), w.1)),
            }
        }
        by.sort_by(|a, b| b.1.total_cmp(&a.1));
        let most = by.first().map_or(String::new(), |b| b.0.to_uppercase());
        lines.push((format!("WALLS {area:.0} M2, {} AT {:.0} KPA: MOSTLY {most}; WORST AT {:.0}% OF ITS LIMIT", t(wall_mass), cabin_pressure() / 1000.0, worst * 100.0), worst <= 1.0));
    }
    // (How far its propellant takes it: for each propellant aboard, the engines that
    // throw it (their exhaust, thrust-weighted), the rocket equation over the whole
    // ship full and without that propellant.)
    {
        let mut by: Vec<(String, f64)> = Vec::new();
        for t in of("tank") {
            let holds = t.3.get("holds").and_then(|v| v.as_str()).unwrap_or("").to_string();
            match by.iter_mut().find(|b| b.0 == holds) {
                Some(b) => b.1 += num(&t.3, "capacity"),
                None => by.push((holds, num(&t.3, "capacity"))),
            }
        }
        let mut parts = Vec::new();
        for (holds, kg) in by {
            let throws: Vec<_> = of("engine").filter(|e| [e.3.get("propellant"), e.3.get("burns")].iter().flatten().any(|v| v.as_str().is_some_and(|p| bare(p) == bare(&holds)))).collect();
            let thrust: f64 = throws.iter().map(|e| num(&e.3, "thrust")).sum();
            if thrust <= 0.0 || kg <= 0.0 {
                continue;
            }
            let exhaust = throws.iter().map(|e| num(&e.3, "thrust") * num(&e.3, "exhaust")).sum::<f64>() / thrust;
            let dv = exhaust * (full / (full - kg).max(1.0)).ln();
            let shown = if dv < 100.0 { format!("{dv:.0} M/S") } else { format!("{:.2} KM/S", dv / 1000.0) };
            parts.push(format!("{shown} ON {}", bare(&holds).to_uppercase()));
        }
        if !parts.is_empty() {
            lines.push((format!("DELTA-V  {}", parts.join(", ")), true));
        }
    }
    // (What pushes it up: whatever is placed pushing up, or swivelled to turn up.)
    let lifting: Vec<&(&Block, &Fitted, String, Fields, f64)> = placed.iter().filter(|p| push_of(p.0, p.1).is_some_and(|(d, _)| d.y > 0.5) || (p.1.push.is_some() && swivelled(&i.plan, &i.fit, p.0))).collect();
    let lift: f64 = lifting.iter().filter_map(|p| p.1.push.map(|x| x.1)).sum();
    // (Against its weight in the gravity it's designed for; and the strongest
    // gravity it can lift itself in, full.)
    let design = i.plan.gravity();
    // (The whole vehicle: its size over everything placed, framed and decked; its
    // masses; its thrust (every engine, lifting or not) over its weight, full and
    // dry, at the gravity it's designed for.)
    {
        let mut lo = Vec3::splat(f32::MAX);
        let mut hi = Vec3::splat(f32::MIN);
        for b in &i.plan.blocks {
            let (a, z) = b.bounds();
            lo = lo.min(a);
            hi = hi.max(z);
        }
        for b in &i.plan.beams {
            lo = lo.min(b.a).min(b.b);
            hi = hi.max(b.a).max(b.b);
        }
        for p in &i.plan.plates {
            lo = lo.min(Vec3::new(p.lo.x, p.y, p.lo.y));
            hi = hi.max(Vec3::new(p.hi.x, p.y, p.hi.y));
        }
        let thrust: f64 = placed.iter().filter_map(|p| p.1.push.map(|x| x.1)).sum();
        if hi.x >= lo.x {
            let d = hi - lo;
            let tw = |m: f64| thrust / (m * design);
            lines.insert(1, (format!("VEHICLE  {:.1} WIDE × {:.1} TALL × {:.1} DEEP M; THRUST {}: T/W {:.2} FULL, {:.2} DRY AT {:.2} G", d.x, d.y, d.z, si(thrust, "N"), tw(full), tw(dry), design / STANDARD_G), true));
        }
    }
    if lift > 0.0 {
        let g = lift / (full * design);
        let most = lift / (full * STANDARD_G);
        lines.push((format!("LIFT  {}: {g:.2} OF ITS FULL WEIGHT AT {:.2} G; HOVERS FULL UP TO {most:.2} G", si(lift, "N"), design / STANDARD_G), g > 1.0));
        // (Landing on them: hovering, their jet on the pad (half the thrust times the
        // exhaust speed, thrust-weighted), and which pad takes it (SFO 23: 100 MW,
        // 1 GW, 10 GW; nothing lands above); what hovering burns; whether it still
        // hovers with its strongest one out; whether they throttle down to hover.)
        let weight = full * design;
        let exhaust = lifting.iter().map(|p| num(&p.3, "thrust") * num(&p.3, "exhaust")).sum::<f64>() / lift.max(1.0);
        let pad = 0.5 * weight * exhaust;
        let class = [(1e8, "A PAD OF 100 MW"), (1e9, "A PAD OF 1 GW"), (1e10, "A PAD OF 10 GW")].iter().find(|c| pad <= c.0).map(|c| c.1);
        let burn = weight / exhaust.max(1.0);
        let strongest = lifting.iter().map(|p| num(&p.3, "thrust")).fold(0.0, f64::max);
        let one_out = lift - strongest >= weight;
        let least: f64 = lifting.iter().map(|p| num(&p.3, "thrust") * p.3.get("throttle").and_then(|v| v.as_f64()).unwrap_or(0.0)).sum();
        let burned = if burn < 10.0 { format!("{burn:.2}") } else { format!("{burn:.0}") };
        // (And how long its landing propellant lets it hover: what its lifting
        // engines throw, held in its tanks.)
        let throws: Vec<String> = lifting.iter().filter_map(|p| p.3.get("propellant").filter(|v| !v.is_null()).or(p.3.get("burns")).and_then(|v| v.as_str()).map(String::from)).collect();
        let held: f64 = of("tank").filter(|t| t.3.get("holds").and_then(|v| v.as_str()).is_some_and(|h| throws.iter().any(|p| bare(p) == bare(h)))).map(|t| num(&t.3, "capacity")).sum();
        let hover_s = if held > 0.0 && burn > 0.0 { format!(" FOR {:.0} S", held / burn) } else { String::new() };

        // (Lifted on fans alone: what hovering draws (each fan's full draw times its
        // share of the weight over its thrust, to the 1.5: momentum theory), against
        // what the power plants and batteries give, and how long the batteries hold it.)
        if lifting.iter().all(|p| p.3.get("full_power").is_some()) {
            let full_power: f64 = lifting.iter().map(|p| num(&p.3, "full_power")).sum();
            let hover = full_power * (weight / lift).min(1.0).powf(1.5);
            let plants: f64 = of("power_plant").map(|p| num(&p.3, "output")).sum();
            let (rate, stored): (f64, f64) = of("battery").fold((0.0, 0.0), |(r, s), p| (r + num(&p.3, "rate"), s + num(&p.3, "stores")));
            let steady: f64 = placed.iter().map(|p| p.4).sum();
            let short = (hover + steady - plants).max(0.0);
            let lasts = if short > 0.0 { format!("; THE BATTERIES HOLD IT {:.0} S", stored / short) } else { String::new() };
            let out_one = {
                let strongest = lifting.iter().max_by(|a, b| num(&a.3, "thrust").total_cmp(&num(&b.3, "thrust")));
                strongest.is_some_and(|s| lift - num(&s.3, "thrust") >= weight)
            };
            let min_air = lifting.iter().map(|p| num(&p.3, "min_density")).fold(0.0, f64::max);
            let text = format!("LAND  ON FANS: HOVER DRAWS {} OF {} THE BATTERIES AND PLANTS GIVE{lasts}{}; IN AIR OF {min_air:.1} KG/M3 OR MORE", si(hover + steady, "W"), si(rate + plants, "W"), if lifting.len() > 1 { if out_one { ", HOVERS ONE OUT" } else { ", NOT ONE OUT" } } else { ", ONE FAN" });
            lines.push((text, g > 1.0 && hover + steady <= rate + plants && stored > 0.0));
            // (Hovering's heat: the fans' share of their jet aboard (motor and inverter;
            // jet goes as power, with thrust to the 1.5) and the packs' discharge losses;
            // for as long as it hovers, the heat the packs take up, at an assumed
            // 1 kJ/kg K, against an assumed 40 K they may warm.)
            let share = (weight / lift).min(1.0).powf(1.5);
            let fans_heat: f64 = lifting.iter().map(|p| 0.5 * num(&p.3, "thrust") * num(&p.3, "exhaust") * share * p.3.get("heat_to_hull").and_then(|v| v.as_f64()).unwrap_or(0.0)).sum();
            let loss = if rate > 0.0 { of("battery").map(|p| num(&p.3, "rate") * (1.0 - p.3.get("efficiency").and_then(|v| v.as_f64()).unwrap_or(1.0))).sum::<f64>() / rate } else { 0.0 };
            let heat = fans_heat + short * loss;
            let seconds = if short > 0.0 { stored / short } else { 0.0 };
            // (The packs warm by their own losses (the fans' heat is in their motors),
            // taken up by their heat capacity (their records' specific heat, by mass);
            // from the dock's temperature they may warm to their records' hottest.)
            let reg = universe_sim::world::registry::registry();
            let phys = |p: &&(&Block, &Fitted, String, Fields, f64)| reg.equipment(&p.1.key).map(|e| (e.physical.specific_heat, e.physical.operating_max_temperature));
            let capacity: f64 = of("battery").map(|p| p.1.mass * phys(&p).and_then(|x| x.0).unwrap_or(PACK_HEAT_CAPACITY)).sum();
            let room = of("battery").filter_map(|p| phys(&p).and_then(|x| x.1)).fold(f64::MAX, f64::min);
            let room = if room == f64::MAX { PACK_WARMING } else { room - DOCK_TEMPERATURE };
            let warms = short * loss * seconds / capacity.max(1.0);
            lines.push((format!("HOVER HEAT  {} WHILE IT HOVERS ({} IN THE PACKS, {} IN THE FAN MOTORS): {:.0} MJ OVER {seconds:.0} S; THE PACKS WARM {warms:.0} OF THE {room:.0} K THEY MAY", si(heat, "W"), si(short * loss, "W"), si(fans_heat, "W"), heat * seconds / 1e6), warms <= room));
            let _ = (pad, class, burned, hover_s, least);
        } else {
        let text = format!("LAND  {} ON THE PAD: {}; HOVER BURNS {burned} KG/S{hover_s}{}{}", si(pad, "W"), class.unwrap_or("NO PAD TAKES IT"), if lifting.len() > 1 { if one_out { ", HOVERS ONE OUT" } else { ", NOT ONE OUT" } } else { ", ONE ENGINE" }, if least > weight { "; CAN'T THROTTLE DOWN TO HOVER" } else { "" });
        lines.push((text, g > 1.0 && class.is_some() && least <= weight));
        }
        // (Balanced: its lift's middle under its weight's.)
        let mass_at = |p: &&(&Block, &Fitted, String, Fields, f64)| (p.0.at, p.1.mass + p.1.load);
        // (Its frame and decks too: each member at its middle, each deck at its own.)
        let mut all: Vec<(Vec3, f64)> = placed.iter().map(|p| mass_at(&p)).collect();
        all.extend(i.plan.beams.iter().filter_map(|b| stocks().iter().find(|s| s.key == b.stock).map(|s| ((b.a + b.b) * 0.5, s.per_metre * f64::from(b.a.distance(b.b))))));
        all.extend(i.plan.plates.iter().filter_map(|p| plate_stocks().iter().find(|s| s.key == p.stock).map(|s| (Vec3::new((p.lo.x + p.hi.x) * 0.5, p.y, (p.lo.y + p.hi.y) * 0.5), s.per_square_metre * f64::from(((p.hi.x - p.lo.x) * (p.hi.y - p.lo.y)).abs())))));
        let total: f64 = all.iter().map(|a| a.1).sum::<f64>().max(1.0);
        let centre = all.iter().fold(Vec3::ZERO, |c, a| c + a.0 * (a.1 / total) as f32);
        let lifts = lifting.iter().fold(Vec3::ZERO, |c, p| c + p.0.at * (num(&p.3, "thrust") / lift.max(1.0)) as f32);
        let off = Vec3::new(lifts.x - centre.x, 0.0, lifts.z - centre.z).length();
        if off > 1.0 {
            lines.push((format!("BALANCE  ITS LIFT IS {off:.1} M OFF ITS MIDDLE OF MASS: IT TIPS"), false));
        }
    }
    // (Batteries supply too, up to their rate, for as long as they hold: what they
    // store over what's drawn, when nothing else supplies it.)
    let plants: f64 = of("power_plant").map(|p| num(&p.3, "output")).sum::<f64>() + of("solar_array").map(|p| num(&p.3, "output")).sum::<f64>();
    let (rate, stored): (f64, f64) = of("battery").fold((0.0, 0.0), |(r, s), p| (r + num(&p.3, "rate"), s + num(&p.3, "stores")));
    let supply = plants + rate;
    let draw: f64 = placed.iter().map(|p| p.4).sum();
    let gear: f64 = of("switchgear").map(|p| num(&p.3, "carries")).sum();
    let lasts = if draw > plants && stored > 0.0 { format!(", BATTERIES LAST {:.1} H", stored / (draw - plants) / 3600.0) } else { String::new() };
    // (A jump drive: what it draws running, against what's given; how long the
    // batteries run it.)
    let jump: f64 = of("hyperdrive").map(|p| num(&p.3, "run_power")).sum();
    if jump > 0.0 {
        let short = (jump + draw - plants).max(0.0);
        let runs = if short > 0.0 && stored > 0.0 { format!("; THE BATTERIES RUN IT {:.0} MIN", stored / short / 60.0) } else { String::new() };
        lines.push((format!("JUMP  THE JUMP DRIVE DRAWS {} RUNNING, OF {}{runs}", si(jump, "W"), si(supply, "W")), jump + draw <= supply));
    }
    lines.push((format!("POWER {} DRAWN OF {}{lasts}{}", si(draw, "W"), si(supply, "W"), if gear > 0.0 { format!(", SWITCHGEAR {}", si(gear, "W")) } else { String::new() }), draw <= supply && (gear == 0.0 || gear >= supply)));
    // (Heat, as the registry's Budgets reckon it: a plant's loss; each drive's,
    // lift's and thrusters' jet power (half its thrust times its exhaust speed, a
    // thruster block four nozzles) at full burn, times the share its record says
    // reaches the hull; and every watt drawn, which ends as heat aboard.)
    let plants: f64 = of("power_plant").map(|p| { let e = num(&p.3, "efficiency").max(0.01); num(&p.3, "output") * (1.0 / e - 1.0) }).sum();
    let burn: f64 = placed
        .iter()
        .filter(|p| matches!(p.2.as_str(), "drive" | "lift" | "thrusters" | "engine") && p.3.get("full_power").is_none())
        .map(|p| {
            let nozzles = if p.2 == "thrusters" { 4.0 } else { 1.0 };
            let share = p.3.get("heat_to_hull").and_then(|v| v.as_f64()).unwrap_or(HEAT_TO_HULL);
            0.5 * num(&p.3, "thrust") * num(&p.3, "exhaust") * nozzles * share
        })
        .sum();
    // (And the people aboard: the heat their food leaves as.)
    let people = of("command_station").map(|p| num(&p.3, "persons")).sum::<f64>() + of("cabin").map(|p| num(&p.3, "seats")).sum::<f64>();
    let heat = plants + burn + draw + universe_sim::world::registry::registry().needs.iter().filter_map(|n| n.heat).sum::<f64>() * people;
    // (In vacuum a radiator gives what its two faces radiate, when its record says
    // its area and temperature (0.9 emissive); its rating is in air. In air, air
    // coolers give theirs too. A craft lifted on fans flies in air: judged by that.)
    let shed: f64 = of("radiator").map(|p| num(&p.3, "rejects")).sum();
    let vacuum: f64 = of("radiator").map(|p| match (p.3.get("area").and_then(|v| v.as_f64()), p.3.get("temperature").and_then(|v| v.as_f64())) {
        (Some(a), Some(t)) => (2.0 * a * 0.9 * 5.670_374e-8 * t.powi(4)).min(num(&p.3, "rejects")),
        _ => num(&p.3, "rejects"),
    }).sum();
    let air = shed + of("heat_exchanger").map(|p| num(&p.3, "transfers")).sum::<f64>();
    // (A craft that never leaves the air is judged in air; one that goes into
    // space (a jump drive, a rocket), in vacuum.)
    let loops: f64 = of("coolant_loop").map(|p| num(&p.3, "carries")).sum();
    let rejected = if air_only(&placed) { air } else { vacuum };
    let both = if (air - vacuum).abs() > 1.0 { format!("IN VACUUM {}, IN AIR {}{}", si(vacuum, "W"), si(air, "W"), if air_only(&placed) { " (IT STAYS IN AIR)" } else { " (IT GOES INTO SPACE: VACUUM COUNTS)" }) } else { format!("RADIATORS {}", si(shed, "W")) };
    lines.push((format!("HEAT  {} TO SHED: {both}, LOOPS {}", si(heat, "W"), si(loops, "W")), rejected >= heat && loops >= heat));
    // (Who's aboard: the crew, the command stations' seats; the passengers, the
    // cabins'.)
    let crew_seats: f64 = of("command_station").map(|p| num(&p.3, "persons")).sum::<f64>() + 0.0;
    let passengers: f64 = of("cabin").map(|p| num(&p.3, "seats")).sum::<f64>() + 0.0;
    let crew = crew_seats + passengers;
    if crew > 0.0 {
        // (An empty sum is -0: plus 0, it's 0.)
        let air: f64 = of("store").filter(|p| p.3.get("holds").and_then(|v| v.as_str()) == Some("element.o")).map(|p| num(&p.3, "capacity")).sum::<f64>() + of("life_support").map(|p| num(&p.3, "air_store")).sum::<f64>() + 0.0;
        let water: f64 = of("store").filter(|p| p.3.get("holds").and_then(|v| v.as_str()) == Some("good.water")).map(|p| num(&p.3, "capacity")).sum::<f64>() + of("life_support").map(|p| num(&p.3, "water_store")).sum::<f64>() + 0.0;
        // (What the stores make up: what the crew use less what the life support
        // recovers, its record's air_recovery and water_recovery, the best fitted.)
        let recovers = |k: &str| of("life_support").map(|p| num(&p.3, k)).fold(0.0, f64::max).min(0.999);
        let oxygen = a_day(&["need.air"], "element.o") * (1.0 - recovers("air_recovery"));
        let drunk = a_day(&["need.water", "need.washing"], "good.water") * (1.0 - recovers("water_recovery"));
        // (Air lost through the airlocks: each one's record's air lost a cycle, at
        // AIRLOCK_CYCLES a day; its oxygen made up from the store.)
        let vented = of("airlock").map(|p| num(&p.3, "air_lost")).sum::<f64>() * AIRLOCK_CYCLES * OXYGEN_IN_AIR;
        // (And what the hull leaks: its leak rate (the registry's hulls', a station's
        // figure if none says) over the sealed volume, its oxygen made up.)
        let sealed: f64 = pressure(i).spaces.iter().map(|s| s.0).sum();
        let leak = universe_sim::world::registry::registry().hulls.iter().find_map(|h| h.design.leak_rate).unwrap_or(3.4e-9) * sealed * 86_400.0 * OXYGEN_IN_AIR;
        let (ad, wd) = (air / (oxygen * crew + vented + leak), water / (drunk * crew));
        // (Food: the stores that hold it against the need's rate.)
        let food: f64 = of("store").filter(|p| p.3.get("holds").and_then(|v| v.as_str()).is_some_and(|h| h.contains("food"))).map(|p| num(&p.3, "capacity")).sum::<f64>() + 0.0;
        let fd = food / (a_day(&["need.food"], "market.food") * crew).max(1e-9);
        let life = of("life_support").count();
        let food_text = if food > 0.0 { format!(", FOOD {fd:.0} DAYS") } else { ", NO FOOD STORED".into() };
        // (Enough for its longest trip, if said (food only past a galley's); else a day.)
        let trip = i.plan.targets.as_ref().and_then(|t| t.trip);
        let days = trip.map_or(1.0, |h| h / 24.0);
        let fed = trip.is_some_and(|h| h <= NEEDS_GALLEY) || fd >= days;
        lines.push((format!("ABOARD {crew:.0} ({crew_seats:.0} CREW): AIR {} DAYS, WATER {} DAYS{food_text}{}", short_days(ad), short_days(wd), if life == 0 { ", NO LIFE SUPPORT" } else { "" }), ad >= days && wd >= days && fed && life > 0));
        // (Life support: the people it keeps, and the heat it carries out of the
        // cabin: theirs, by the need that gives it.)
        let keeps: f64 = of("life_support").map(|p| num(&p.3, "persons")).sum();
        let cools: f64 = of("life_support").map(|p| num(&p.3, "cooling")).sum();
        let body: f64 = universe_sim::world::registry::registry().needs.iter().filter_map(|n| n.heat).sum::<f64>() * crew;
        lines.push((format!("LIFE  KEEPS {keeps:.0} OF {crew:.0}; COOLS {} OF THEIR {}", si(cools, "W"), si(body, "W")), keeps >= crew && cools >= body));
    }
    // Checks: sealed spaces, the way in, the way down, the ore's path.
    let mut faults = Vec::new();
    // (Modules the registry doesn't know (renamed or taken out): not counted in
    // anything; said, not dropped.)
    let mut unknown: Vec<&str> = i.plan.blocks.iter().map(|b| kind(&b.id)).filter(|k| !i.fit.iter().any(|f| f.id == *k)).collect();
    unknown.sort_unstable();
    unknown.dedup();
    for k in unknown {
        let n = i.plan.blocks.iter().filter(|b| kind(&b.id) == k).count();
        faults.push(format!("{n} × {} NOT IN THE REGISTRY: NOT COUNTED (RENAMED? TAKEN OUT?)", k.to_uppercase()));
    }
    let pr = pressure(i);
    // (In a hull, its rooms open into it: the hull holds the air, not checked here.)
    if !pr.spaces.is_empty() && i.spec().is_some() {
        let volume: f64 = pr.spaces.iter().map(|s| s.0).sum();
        lines.push((format!("ROOMS {} SPACE{}, {volume:.0} M3, IN THE HULL", pr.spaces.len(), if pr.spaces.len() == 1 { "" } else { "S" }), true));
    } else if !pr.spaces.is_empty() {
        let volume: f64 = pr.spaces.iter().map(|s| s.0).sum();
        let leaks = pr.openings.iter().filter(|o| o.1 == Opening::Leak).count();
        let locks = pr.openings.iter().filter(|o| o.1 == Opening::Airlock).count();
        let air: f64 = of("store").filter(|p| p.3.get("holds").and_then(|v| v.as_str()) == Some("element.o")).map(|p| num(&p.3, "capacity")).sum::<f64>() + of("life_support").map(|p| num(&p.3, "air_store")).sum::<f64>() + 0.0;
        let fills = air / (volume * OXYGEN_A_CUBIC_METRE).max(1e-9);
        let air_only = air_only(&placed);
        let sealed = leaks == 0 && (air_only || pr.spaces.iter().all(|s| s.1));
        lines.push((format!("SEALED {} SPACE{}, {volume:.0} M3, {locks} AIRLOCK DOOR{}; AIR FILLS IT {fills:.1} TIMES", pr.spaces.len(), if pr.spaces.len() == 1 { "" } else { "S" }, if locks == 1 { "" } else { "S" }), sealed && fills >= 1.0));
        if leaks > 0 {
            faults.push(format!("{leaks} OPEN END{} TO SPACE: IT LEAKS", if leaks == 1 { "" } else { "S" }));
        }
        let vents = pr.openings.iter().filter(|o| o.1 == Opening::ToSpace).count();
        if vents > 0 && air_only {
            lines.push((format!("DOORS {vents} TO THE AIR: IT NEVER LEAVES THE AIR, SO NO AIRLOCK"), true));
        } else if vents > 0 {
            faults.push(format!("{vents} HATCH{} TO SPACE WITH NO AIRLOCK: OPENING IT VENTS THE ROOM", if vents == 1 { "" } else { "ES" }));
        }
        let shut = pr.spaces.iter().filter(|s| !s.1).count();
        if shut > 0 && !(air_only && vents > 0) {
            faults.push(format!("{shut} SEALED SPACE{} NO AIRLOCK LEADS INTO", if shut == 1 { "" } else { "S" }));
        }
    }
    // (The frame's members, solid tubes, passing into what they mustn't.)
    if let Some((_, clash)) = i.member_clash.as_ref().filter(|(p, _)| *p == i.plan) {
        let count = |w: Passes| clash.iter().filter(|c| **c == Some(w)).count();
        let parts: Vec<String> = [(Passes::Module, "THROUGH MODULES"), (Passes::Room, "INTO ROOMS"), (Passes::Deck, "THROUGH DECKS"), (Passes::Member, "INTO OTHER MEMBERS")].iter().filter(|(w, _)| count(*w) > 0).map(|(w, say)| format!("{} {say}", count(*w))).collect();
        if !parts.is_empty() {
            faults.push(format!("MEMBERS CLASH: {}", parts.join(", ")));
        }
    }
    // (Crewing: someone to fly it, seated for its jolts, inside; berths for the crew;
    // galley and head for all aboard; fire units for the air; suits for the crew;
    // a ship that lands, something to land by.)
    {
        let persons = |k: &'static str| of(k).map(|p| num(&p.3, "persons")).sum::<f64>();
        let crew = persons("command_station");
        let aboard = crew + of("cabin").map(|p| num(&p.3, "seats")).sum::<f64>();
        let in_room = |b: &Block| (0..i.plan.lines.len()).filter(|&k| i.plan.group_of(k).is_some_and(|g| i.plan.groups[g].walled) && i.plan.lines[k].2.section != Section::Line).any(|k| {
            let (p, e) = i.plan.axis(k);
            let pr = i.plan.lines[k].2;
            let d = e - p;
            let t = (b.at - p).dot(d) / d.length_squared().max(1e-6);
            let off = b.at - (p + d * t.clamp(0.0, 1.0));
            (0.0..=1.0).contains(&t) && off.y.abs() < pr.height * 0.5 && Vec3::new(off.x, 0.0, off.z).length() < pr.width * 0.5
        });
        if aboard > 0.0 && crew == 0.0 {
            faults.push("NO COMMAND STATION: NOBODY TO FLY IT".into());
        }
        for p in placed.iter().filter(|p| INSIDE.contains(&p.2.as_str()) && !in_room(p.0)) {
            faults.push(format!("{} IS OUTSIDE THE PRESSURISED ROOMS", p.1.name));
        }
        if let Some(b) = i.bearing.as_ref().filter(|(p, _)| *p == i.plan) {
            let worst = b.1.felt.iter().copied().fold(0.0, f64::max);
            for p in of("command_station").filter(|p| num(&p.3, "g_rating") < worst) {
                faults.push(format!("{}'S SEATS ARE RATED {:.1} G; IT FEELS {:.1} G", p.1.name, num(&p.3, "g_rating") / STANDARD_G, worst / STANDARD_G));
            }
        }
        // (A trip said shorter than a need's threshold: that need not asked.)
        let trip = i.plan.targets.as_ref().and_then(|t| t.trip).unwrap_or(f64::MAX);
        let berths = persons("berths") + 0.0;
        if crew > berths && trip > NEEDS_BERTHS {
            faults.push(format!("CREW {crew:.0}, BERTHS FOR {berths:.0}"));
        }
        for (k, what, past) in [("galley", "GALLEY", NEEDS_GALLEY), ("head", "HEAD", NEEDS_HEAD)] {
            let n = persons(k) + 0.0;
            if aboard > n && trip > past {
                faults.push(format!("{aboard:.0} ABOARD, {what} FOR {n:.0}"));
            }
        }
        let protected: f64 = of("fire_unit").map(|p| num(&p.3, "protects")).sum();
        let sealed: f64 = pr.spaces.iter().map(|s| s.0).sum();
        if sealed > protected {
            faults.push(format!("FIRE UNITS COVER {protected:.0} OF {sealed:.0} M3"));
        }
        let suits = persons("suit_locker");
        if crew > suits {
            faults.push(format!("CREW {crew:.0}, SUITS FOR {:.0}", suits + 0.0));
        }
        if of("landing_gear").count() > 0 && of("altimeter").count() == 0 {
            faults.push("IT LANDS BLIND: NO ALTIMETER".into());
        }
    }
    // (Joints with no node in stock wide enough for their widest tube.)
    if let Some((_, ns)) = i.node_cache.as_ref().filter(|(p, _)| *p == i.plan) {
        let short = ns.iter().filter(|n| n.2.is_none()).count();
        if short > 0 {
            let most = node_stocks().iter().map(|n| n.diameter).fold(0.0, f32::max);
            faults.push(format!("{short} JOINT{} NEED A NODE WIDER THAN {:.0} MM: NONE IN STOCK", if short == 1 { "" } else { "S" }, most * 1000.0));
        }
    }
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
    if crew > 0.0 && locks.is_empty() && !air_only(&placed) {
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
    // (The job asked of it: each target against what it has, and what would close
    // the gap.)
    if let Some(t) = &i.plan.targets {
        lines.extend(target_lines(t, &placed, dry, full, design));
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
/// A design's report, without a window (`freefall --report <design>`): its load cases,
/// budgets and faults, what's past its design limit, its clashes, what isn't held,
/// its mass by part. What the studio shows, as text, for checking a change fast.
pub fn report(name: &str) -> String {
    use std::fmt::Write as _;
    let mut i = Interior::new();
    i.open_design(name);
    i.refit();
    let mut out = String::new();
    if i.plan.blocks.is_empty() && i.plan.beams.is_empty() {
        return format!("no design named {name} (or it's empty)\n");
    }
    let b = bearing(&i.plan, &i.fit, i.spec(), i.plan.gravity());
    let clash = member_clashes(&i.plan);
    i.member_clash = Some((i.plan.clone(), clash.clone()));
    i.node_cache = Some((i.plan.clone(), nodes(&i.plan.beams, &roots(&i.plan, &i.fit))));
    i.bearing = Some((i.plan.clone(), std::sync::Arc::new(b.clone())));
    let _ = writeln!(out, "DESIGN {name}: {} modules, {} members, {} decks, design gravity {:.2} g", i.plan.blocks.len(), i.plan.beams.len(), i.plan.plates.len(), i.plan.gravity() / STANDARD_G);
    // (The load cases: each one's worst member, against its design limit and breaking.)
    for ((n, r), felt) in b.cases.iter().zip(b.felt.iter()) {
        let _ = match r {
            Ok(c) => {
                let worst = c.members.iter().map(|o| o.work.design).fold(0.0, f64::max);
                let near = c.members.iter().map(|o| o.work.breaking).fold(0.0, f64::max);
                let broken = c.members.iter().filter(|o| o.broken.is_some()).count();
                writeln!(out, "CASE {n} {:.2} g: worst {:.0}% of design limit, {:.0}% of breaking{}", felt / STANDARD_G, worst * 100.0, near * 100.0, if broken > 0 { format!(", {broken} break") } else { String::new() })
            }
            Err(e) => writeln!(out, "CASE {n}: {e}"),
        };
    }
    let bu = budget(&i);
    for (l, ok) in &bu.lines {
        let _ = writeln!(out, "{} {l}", if *ok { "ok   " } else { "SHORT" });
    }
    for f in &bu.faults {
        let _ = writeln!(out, "FAULT {f}");
    }

    for l in &b.loose {
        let _ = writeln!(out, "LOOSE {l}");
    }
    // (Members past their design limit: where, what, how hard, in which case.)
    for (k, m) in i.plan.beams.iter().enumerate() {
        let os = b.outcomes(k);
        if let Some((case, o)) = b.cases.iter().map(|c| c.0.as_str()).zip(os.iter()).filter(|(_, o)| o.work.design > 1.0).max_by(|a, b| a.1.work.design.total_cmp(&b.1.work.design)) {
            let _ = writeln!(out, "OVER member {k} {:.2},{:.2},{:.2} -> {:.2},{:.2},{:.2} ({:.2} m, {}): {case} {:.0}% design, {:.0}% break, push/pull {:.0} kN, bending {:.1} kN m", m.a.x, m.a.y, m.a.z, m.b.x, m.b.y, m.b.z, m.a.distance(m.b), m.stock.trim_start_matches("stock."), o.work.design * 100.0, o.work.breaking * 100.0, o.work.forces.axial / 1000.0, o.work.forces.bending / 1000.0);
        }
    }
    // (Decks past their design limit: each one's hardest worked strip, and in which
    // case; and anything else of the frame that isn't one of its members (a deck's
    // fittings).)
    for n in 0..i.plan.plates.len() {
        let worst = b.strips.iter().filter(|s| s.0 == n).flat_map(|s| b.cases.iter().map(|c| c.0.as_str()).zip(b.of_member(s.4))).max_by(|x, y| x.1.work.design.total_cmp(&y.1.work.design));
        if let Some((case, o)) = worst.filter(|w| w.1.work.design > 1.0) {
            let _ = writeln!(out, "OVER deck {} ({}): {case} its hardest strip {:.0}% design, {:.0}% break", n + 1, i.plan.plates[n].stock.trim_start_matches("stock."), o.work.design * 100.0, o.work.breaking * 100.0);
        }
    }
    let planned: std::collections::HashSet<usize> = b.index.iter().flatten().copied().chain(b.strips.iter().map(|s| s.4)).collect();
    for (case, r) in &b.cases {
        if let Ok(c) = r {
            let other = c.members.iter().enumerate().filter(|(m, o)| !planned.contains(m) && o.work.design > 1.0).count();
            if other > 0 {
                let _ = writeln!(out, "OVER {other} deck fittings in {case}");
            }
        }
    }
    // (Clashes: how many of each, and the first few of each.)
    for (w, say) in [(Passes::Module, "through a module"), (Passes::Room, "into a room"), (Passes::Deck, "through a deck"), (Passes::Member, "into another member")] {
        let ks: Vec<usize> = (0..clash.len()).filter(|&k| clash[k] == Some(w)).collect();
        if !ks.is_empty() {
            let eg: Vec<String> = ks.iter().take(3).map(|&k| { let m = &i.plan.beams[k]; format!("{k} {:.1},{:.1},{:.1}->{:.1},{:.1},{:.1}", m.a.x, m.a.y, m.a.z, m.b.x, m.b.y, m.b.z) }).collect();
            let _ = writeln!(out, "CLASH {} members {say}: {}", ks.len(), eg.join("; "));
        }
    }
    let modules: Vec<String> = i.plan.blocks.iter().zip(block_clashes(i.spec().and_then(|s| s.shape().walk.as_deref()), &i)).filter(|(_, c)| *c).map(|(b, _)| b.id.clone()).collect();
    if !modules.is_empty() {
        let _ = writeln!(out, "CLASH modules: {}", modules.join(", "));
    }
    // (Its mass by part.)
    let (members, node_mass, decks) = frame_masses(&i);
    let wall_mass: f64 = walls(&i.plan).iter().map(|w| w.1 * w.2.per_square_metre).sum();
    let mods: f64 = i.plan.blocks.iter().filter_map(|b| i.fit.iter().find(|f| f.id == kind(&b.id))).map(|f| f.mass).sum();
    let _ = writeln!(out, "MASS modules {:.1} t, members {:.1} t, nodes {:.1} t, decks {:.1} t, walls {:.1} t", mods / 1000.0, members / 1000.0, node_mass / 1000.0, decks / 1000.0, wall_mass / 1000.0);
    out
}

/// A sheet beside the panel: a title, a line under it, and rows of text, each in its
/// colour (the module sheet's place and look).
fn draw_rows(frame: &mut Frame, down: f32, title: &str, under: &str, colour: Color, rows: &[(String, Color)]) {
    let (pp, pc) = PANEL;
    let (p, c) = (Vec2::new(pp.x + pc.x + 8.0, pp.y + down), Vec2::new(340.0, 40.0 + rows.len() as f32 * 11.0));
    frame.hud_rect(p, c, Color([0.02, 0.06, 0.13, 0.92]));
    frame.hud_box(p, c, colour.scale(0.7));
    frame.text_scaled(p + Vec2::new(8.0, 6.0), &title.chars().take(40).collect::<String>(), colour, 0.8);
    frame.text_scaled(p + Vec2::new(8.0, 19.0), &under.chars().take(60).collect::<String>(), LABEL.scale(0.7), 0.55);
    for (k, (text, col)) in rows.iter().enumerate() {
        frame.text_scaled(p + Vec2::new(8.0, 34.0 + k as f32 * 11.0), &text.chars().take(60).collect::<String>(), *col, 0.58);
    }
}

/// A sheet's line for one load case: its text, how hard it's worked, whether it
/// breaks (none: nothing to say in that case).
type CaseLine<'a> = dyn Fn(&universe_sim::world::frame::Collapse) -> Option<(String, f64, bool)> + 'a;

/// A force in kN, a moment in kN m.
fn kn(f: f64) -> String {
    format!("{:.1}", f / 1000.0)
}

/// SELECT: what's picked, on a sheet: what it is (its stock or product, size, mass)
/// and what it carries (each load case: its forces, how hard it's worked, whether
/// it breaks).
fn draw_selected(frame: &mut Frame, interior: &Interior) {
    let plan = &interior.plan;
    let reg = universe_sim::world::registry::registry();
    let name = |key: &str| reg.names.get(key).map_or(key.trim_start_matches("material.").replace('-', " ").to_uppercase(), |n| n.to_uppercase());
    let bearing = interior.bearing.as_ref().filter(|(p, _)| *p == interior.plan).map(|(_, b)| b.clone());
    let pct = |w: f64| format!("{:.0}%", w * 100.0);
    let hue = |w: f64, broken: bool| if broken || w > 1.0 { CLASH } else if w >= 0.5 { Color([1.0, 0.7, 0.2, 1.0]) } else { Color([0.4, 1.0, 0.5, 1.0]) };
    let cases = |b: &Bearing, of: &CaseLine<'_>| -> Vec<(String, Color)> {
        b.cases.iter().map(|(n, r)| match r {
            Ok(c) => match of(c) {
                Some((text, w, broken)) => (format!("{n}: {text}"), hue(w, broken)),
                None => (format!("{n}: -"), LABEL.scale(0.7)),
            },
            Err(e) => (format!("{n}: {e}"), if not_built_for(e) { LABEL.scale(0.7) } else { CLASH }),
        }).collect()
    };
    match interior.pick {
        Some(Hover::Module(n)) => {
            let Some(b) = plan.blocks.get(n) else { return };
            if let Some(f) = interior.fit.iter().find(|f| f.id == kind(&b.id)) {
                draw_sheet(frame, interior, f, Some(b));
                // (Under it, what the frame takes from it: its weight full, as each
                // case presses it, and where it's held.)
                let full = f.mass + f.load;
                let mut rows = vec![(format!("{} FULL ({} EMPTY)", si(full, "KG"), si(f.mass, "KG")), LABEL)];
                if let Some(bb) = &bearing {
                    for ((n, r), felt) in bb.cases.iter().zip(&bb.felt) {
                        rows.push(match r {
                            Ok(_) => (format!("{n}: {:.2} G, {} KN ON THE FRAME", felt / STANDARD_G, kn(full * felt)), LABEL),
                            Err(e) => (format!("{n}: {e}"), if not_built_for(e) { LABEL.scale(0.7) } else { CLASH }),
                        });
                    }
                    if bb.loose.iter().any(|l| l.starts_with(&f.name.to_uppercase()) && l.contains("NOT MOUNTED")) {
                        rows.push(("NOT MOUNTED: NOTHING HOLDS IT".into(), CLASH));
                    }
                }
                draw_rows(frame, 308.0, "ON THE FRAME", "WHAT IT PRESSES ON WHAT HOLDS IT", MODULE, &rows);
            }
        }
        Some(Hover::Member(k)) => {
            let Some(m) = plan.beams.get(k) else { return };
            let Some(st) = stocks().iter().find(|s| s.key == m.stock) else { return };
            let len = m.a.distance(m.b);
            let mut rows = vec![
                (format!("TUBE {:.0} X {:.0} MM, {:.2} M LONG", st.section.diameter * 1000.0, st.section.wall * 1000.0, len), LABEL),
                (format!("MASS {:.1} KG ({:.2} KG/M)", st.per_metre * f64::from(len), st.per_metre), LABEL),
                (format!("YIELDS {:.0} MPA, BREAKS {:.0} MPA, E {:.0} GPA", st.material.yield_strength / 1e6, st.material.tensile_strength / 1e6, st.material.stiffness / 1e9), LABEL),
                (format!("FROM {:.2},{:.2},{:.2} TO {:.2},{:.2},{:.2}", m.a.x, m.a.y, m.a.z, m.b.x, m.b.y, m.b.z), LABEL.scale(0.75)),
            ];
            let clash = interior.member_clash.as_ref().filter(|(p, _)| *p == interior.plan).and_then(|(_, c)| c.get(k).copied().flatten());
            rows.push(match clash {
                Some(w) => (format!("PASSES INTO {}", match w { Passes::Module => "A MODULE", Passes::Room => "A ROOM", Passes::Deck => "A DECK", Passes::Member => "ANOTHER MEMBER" }), CLASH),
                None => ("CLEAR OF EVERYTHING".into(), LABEL.scale(0.75)),
            });
            rows.push(("CASE: PULL+/PUSH- KN, BENDING KN M, % OF DESIGN LIMIT".into(), LABEL.scale(0.75)));
            rows.push(("(YIELD WITH A 1.5 MARGIN), % OF BREAKING".into(), LABEL.scale(0.75)));
            if let Some(b) = &bearing {
                let idx = b.index.get(k).copied().flatten();
                rows.extend(cases(b, &|c| {
                    let o = c.members.get(idx?)?;
                    let f = o.work.forces;
                    let text = format!("{}, {}, {} ({} BREAK){}{}", kn(f.axial), kn(f.bending), pct(o.work.design), pct(o.work.breaking), if o.work.buckles { " BUCKLING" } else { "" }, if o.broken.is_some() { " BREAKS" } else { "" });
                    Some((text, o.work.design, o.broken.is_some()))
                }));
            }
            draw_rows(frame, 0.0, &st.name, &format!("FRAME MEMBER - {}", name(&st.of)), Color([0.4, 1.0, 0.5, 1.0]), &rows);
        }
        Some(Hover::Deck(n)) => {
            let Some(pl) = plan.plates.get(n) else { return };
            let Some(ps) = plate_stocks().iter().find(|s| s.key == pl.stock) else { return };
            let (w, d) = ((pl.hi.x - pl.lo.x).abs(), (pl.hi.y - pl.lo.y).abs());
            let area = f64::from(w * d);
            let mut rows = vec![
                (match ps.core {
                    Some(c) => format!("PANEL {:.1} MM: FACES {:.1} MM, CORE {:.1} MM", ps.depth * 1000.0, ps.thickness * 1000.0, c.depth * 1000.0),
                    None => format!("PLATE {:.1} MM", ps.depth * 1000.0),
                }, LABEL),
                (format!("{w:.1} X {d:.1} M = {area:.0} M2, TOP AT {:.2} M", pl.y), LABEL),
                (format!("MASS {:.0} KG ({:.1} KG/M2)", area * ps.per_square_metre, ps.per_square_metre), LABEL),
                (format!("FLOOR LOAD {DECK_LOAD:.0} KG/M2; YIELDS {:.0} MPA", ps.material.yield_strength / 1e6), LABEL),
                ("CASE: ITS HARDEST WORKED STRIP, % OF DESIGN LIMIT (OF BREAKING)".into(), LABEL.scale(0.75)),
            ];
            if let Some(b) = &bearing {
                let strips: Vec<usize> = b.strips.iter().filter(|s| s.0 == n).map(|s| s.4).collect();
                rows.extend(cases(b, &|c| {
                    let worst = strips.iter().filter_map(|&m| c.members.get(m)).max_by(|a, b| a.work.design.total_cmp(&b.work.design))?;
                    let broken = strips.iter().filter_map(|&m| c.members.get(m)).filter(|o| o.broken.is_some()).count();
                    Some((format!("{} ({}), {} STRIPS{}", pct(worst.work.design), pct(worst.work.breaking), strips.len(), if broken > 0 { format!(", {broken} BREAK") } else { String::new() }), worst.work.design, broken > 0))
                }));
            }
            draw_rows(frame, 0.0, &ps.name.to_uppercase(), &format!("DECK {} - {}", n + 1, name(&ps.of)), DECK, &rows);
        }
        Some(Hover::Node(at)) => {
            let Some((_, ns)) = interior.node_cache.as_ref().filter(|(p, _)| *p == interior.plan) else { return };
            let Some(&(_, widest, node)) = ns.iter().find(|n| n.0.distance(at) < 0.05) else { return };
            let meeting: Vec<usize> = (0..plan.beams.len()).filter(|&k| plan.beams[k].a.distance(at) < 0.05 || plan.beams[k].b.distance(at) < 0.05).collect();
            let mut rows = vec![
                (match node {
                    Some(nd) => format!("{:.0} MM {}, {:.1} KG", nd.diameter * 1000.0, if nd.hollow { "HOLLOW SHELL" } else { "SOLID FORGING" }, nd.mass),
                    None => format!("NONE IN STOCK {:.0} MM WIDE", widest * NODE_RATIO * 1000.0),
                }, if node.is_some() { LABEL } else { CLASH }),
                (format!("WIDEST TUBE {:.0} MM (A NODE {NODE_RATIO:.1} TIMES IT)", widest * 1000.0), LABEL),
                (format!("{} MEMBERS MEET HERE", meeting.len()), LABEL),
                (format!("AT {:.2},{:.2},{:.2}", at.x, at.y, at.z), LABEL.scale(0.75)),
                ("CASE: ITS HARDEST WORKED MEMBER, % OF DESIGN LIMIT (OF BREAKING)".into(), LABEL.scale(0.75)),
            ];
            if let Some(b) = &bearing {
                rows.extend(cases(b, &|c| {
                    let worst = meeting.iter().filter_map(|&k| b.index.get(k).copied().flatten()).filter_map(|m| c.members.get(m)).max_by(|a, b| a.work.design.total_cmp(&b.work.design))?;
                    Some((format!("{} ({})", pct(worst.work.design), pct(worst.work.breaking)), worst.work.design, worst.broken.is_some()))
                }));
            }
            draw_rows(frame, 0.0, "NODE", "4340 STEEL, WHERE MEMBERS MEET", Color([0.9, 0.95, 1.0, 1.0]), &rows);
        }
        Some(Hover::Line(k)) => {
            // (A walled room: its size, its air, its walls against the cabin pressure.)
            let Some(&(a, b, pr)) = plan.lines.get(k) else { return };
            if pr.section == Section::Line || !plan.group_of(k).is_some_and(|g| plan.groups[g].walled) {
                return;
            }
            let len = plan.points[a].at.distance(plan.points[b].at);
            let mut rows = vec![
                (format!("{} {:.1} X {:.1} M, {len:.1} M LONG", pr.section.name(), pr.width, pr.height), LABEL),
                (format!("AIR {:.0} M3", section_area(pr) * len), LABEL),
            ];
            if let Some((_, area, ws, w)) = walls(plan).into_iter().find(|w| w.0 == k) {
                rows.push((format!("WALLS {}", ws.name.to_uppercase()), LABEL));
                rows.push((format!("{area:.0} M2, {:.0} KG, {:.1} MM THICK", area * ws.per_square_metre, ws.depth * 1000.0), LABEL));
                rows.push((format!("AT {:.0} KPA: {} OF ITS LIMIT", cabin_pressure() / 1000.0, pct(w)), hue(w, false)));
            }
            draw_rows(frame, 0.0, "ROOM", "A WALLED TUBE", Color([0.75, 0.9, 1.0, 1.0]), &rows);
        }
        _ => {}
    }
}

fn draw_sheet(frame: &mut Frame, interior: &Interior, f: &Fitted, block: Option<&Block>) {
    let reg = universe_sim::world::registry::registry();
    let e = reg.equipment(&f.key);
    let (pp, pc) = PANEL;
    let (p, c) = (Vec2::new(pp.x + pc.x + 8.0, pp.y), Vec2::new(236.0, 300.0));
    frame.hud_rect(p, c, Color([0.02, 0.06, 0.13, 0.92]));
    frame.hud_box(p, c, MODULE.scale(0.7));
    frame.text_scaled(p + Vec2::new(8.0, 6.0), &f.name, MODULE, 0.8);
    let name = |key: &str| reg.names.get(key).map_or(key.to_string(), |n| n.to_uppercase());
    let line = match e {
        Some(e) => {
            let fits = reg.equipment(&f.key).and_then(|x| x.fits.clone()).map_or(String::new(), |m| format!(" - FITS {}", name(&m)));
            format!("{}{}", name(&e.identity.maker), fits)
        }
        None => "THE HULL'S OWN".to_string(),
    };
    frame.text_scaled(p + Vec2::new(8.0, 19.0), &line.chars().take(44).collect::<String>(), LABEL.scale(0.7), 0.55);
    // Its shape, turning (a box, or a tank's ball or egg), in a window of its own.
    let (wp, wc) = (p + Vec2::new(8.0, 32.0), Vec2::new(c.x - 16.0, 84.0));
    frame.hud_box(wp, wc, LABEL.scale(0.25));
    let size = block.map_or(f.size, |b| b.size);
    let shape = Block { id: String::new(), at: Vec3::ZERO, size, push: None };
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
                    // (A share as a percent, or as a power of ten when tiny; a speed in
                    // km/s; the rest with its unit's prefix.)
                    match (field.as_str(), unit(field)) {
                        ("efficiency", _) => format!("{:.0}%", x * 100.0),
                        (_, "M/S") if x >= 1e4 => format!("{:.0} KM/S", x / 1000.0),
                        (_, "") if x != 0.0 && x.abs() < 0.01 => format!("{x:.0E}"),
                        (_, u) => si(x, u),
                    }
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
    // (A deck's grid points: to hold it up from.)
    at.extend(i.plan.plates.iter().flat_map(|p| plate_grid(p).0));
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
            let w5 = (c.x - 16.0 - 24.0) / 5.0;
            let cases = [("WORST", Action::Case(None)), ("LAND", Action::Case(Some(0))), ("THRUST", Action::Case(Some(1))), ("LIFT", Action::Case(Some(2))), ("FLOOR", Action::Case(Some(3)))];
            // (DESIGN GRAVITY's - and +: small, at the end of its row.)
            let small = |x: f32| (Vec2::new(p.x + c.x - 8.0 - x, p.y + 294.0), Vec2::new(20.0, 13.0));
            cases.into_iter().enumerate().map(|(k, (n, a))| (at(156.0, k as f32 * (w5 + 6.0), w5), n, a)).chain([(at(310.0, 0.0, w4), "TRUSS", Action::Truss), (at(310.0, w4 + 6.0, w4), "BRACE", Action::Brace), (at(310.0, 2.0 * (w4 + 6.0), w4), "DECK", Action::Deck), (at(310.0, 3.0 * (w4 + 6.0), w4), "MIX", Action::Mix), (at(332.0, 0.0, w3), "MOUNT ALL", Action::MountAll), (at(332.0, w3 + 6.0, w3), "AUTO-SIZE", Action::AutoSize), (at(332.0, 2.0 * (w3 + 6.0), w3), "FIT FRAME", Action::FitFrame), (small(44.0), "-", Action::GravityDown), (small(20.0), "+", Action::GravityUp)]).collect()
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
            shapes.chain([(at(246.0, 0.0, w3), "GROUP", Action::Group), (at(246.0, w3 + 6.0, w3), "UNGROUP", Action::Ungroup), (at(246.0, 2.0 * (w3 + 6.0), w3), "WALL OFF", Action::Wall), (at(270.0, 0.0, w), "REMOVE PICKED", Action::Remove), (at(270.0, w + 6.0, w), "ON LINE", Action::Stand), (at(294.0, 0.0, w3), "LEVEL", Action::Level), (at(294.0, w3 + 6.0, w3), "COPY", Action::Copy), (at(294.0, 2.0 * (w3 + 6.0), w3), "STAMP", Action::Paste), (at(316.0, 0.0, w3), "SAVE ASSY", Action::SaveAssembly), (at(316.0, w3 + 6.0, w3), "ASSEMBLIES", Action::Assemblies), (at(316.0, 2.0 * (w3 + 6.0), w3), "LEVEL UP", Action::LevelUp)]).collect()
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
    let step = i.snap.unwrap_or(0.25);
    let mut at = Vec3::new((at.x / step).round() * step, plane, (at.z / step).round() * step);
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
    let select = i.tool == Tool::Look;
    // (SELECT: a node, as wide as it's drawn.)
    if select && i.shown(layer::FRAME)
        && let Some((_, ns)) = i.node_cache.as_ref().filter(|(p, _)| *p == i.plan)
        && let Some(n) = ns.iter().filter_map(|n| {
            let (s, z) = cam.project(n.0)?;
            let w = (n.2.map_or(n.1, |x| x.diameter) * cam.focal / z).max(6.0) * 0.5;
            (s.distance(q) < w).then_some((n.0, s.distance(q)))
        }).min_by(|a, b| a.1.total_cmp(&b.1))
    {
        return Some(Hover::Node(n.0));
    }
    let line = (i.shown(layer::LINES) || i.shown(layer::ROOMS) || i.shown(layer::WALLS)).then(|| i.plan.lines.iter().enumerate().filter_map(|(k, &(a, b, _))| {
        let (pa, pb) = (screen[a]?, screen[b]?);
        let ab = pb - pa;
        let t = ((q - pa).dot(ab) / ab.length_squared().max(1e-6)).clamp(0.0, 1.0);
        let d = (pa + ab * t).distance(q);
        (d < 5.0).then_some((k, d))
    }).min_by(|a, b| a.1.total_cmp(&b.1)).map(|(k, _)| Hover::Line(k))).flatten();
    if line.is_some() || !select {
        return line;
    }
    // (SELECT: a member; else the nearest module or deck along the ray.)
    if i.shown(layer::FRAME) && let Some(k) = beam_at(i, cam, q) {
        return Some(Hover::Member(k));
    }
    let ray = cam.ray(q);
    let module = i.plan.blocks.iter().enumerate().filter(|_| i.shown(layer::MODULES)).filter_map(|(n, b)| b.hit(cam.eye, ray).map(|t| (Hover::Module(n), t)));
    let deck = i.plan.plates.iter().enumerate().filter(|_| i.shown(layer::DECKS)).filter_map(|(n, p)| {
        let t = (p.y - cam.eye.y) / ray.y;
        let at = cam.eye + ray * t;
        (t > 0.0 && at.x >= p.lo.x.min(p.hi.x) && at.x <= p.lo.x.max(p.hi.x) && at.z >= p.lo.y.min(p.hi.y) && at.z <= p.lo.y.max(p.hi.y)).then_some((Hover::Deck(n), t))
    });
    module.chain(deck).min_by(|a, b| a.1.total_cmp(&b.1)).map(|(h, _)| h)
}

/// This frame's input. False: close it. (A change to the plan is kept for UNDO.)
pub fn input(app: &mut App, ctx: &Context, interior: &mut Interior) -> bool {
    let spec = app.ship.spec();
    input_with(spec, &mut app.deckplans, ctx, interior)
}

/// This frame's input, given the ship's hull spec and the deck plans (what the game
/// keeps; the studio alone keeps its own). False: close it.
pub fn input_with(spec: &universe_sim::world::ship::ClassSpec, deckplans: &mut Vec<universe_sim::world::deckplan::DeckPlan>, ctx: &Context, interior: &mut Interior) -> bool {
    let input = &ctx.input;
    let ctrl = input.down(KeyCode::ControlLeft) || input.down(KeyCode::ControlRight);
    let shift = input.down(KeyCode::ShiftLeft) || input.down(KeyCode::ShiftRight);
    // (The hull's lines picked up, its plan seeded, whatever else is going on.)
    interior.spin += ctx.dt;
    interior.sync(&spec.key, spec.shape(), deckplans, 0.0);
    interior.refresh();
    // The balance room: ENTER flies it, ESC back to the studio.
    if let Some(b) = interior.balance.as_mut() {
        match b.input(ctx) {
            crate::test_drive::Asked::Fly => {
                let craft = b.craft.clone();
                interior.balance = None;
                interior.drive = Some(crate::test_drive::Drive::new(craft));
            }
            crate::test_drive::Asked::Back => interior.balance = None,
            crate::test_drive::Asked::Stay => {}
        }
        return true;
    }
    // TEST DRIVE: flown; ESC back to the studio.
    if let Some(d) = interior.drive.as_mut() {
        if input.pressed(KeyCode::Escape) {
            interior.drive = None;
        } else {
            d.input(ctx);
        }
        return true;
    }
    // On the test stand: walked; ESC back to the studio.
    if let Some(s) = interior.stand.as_mut() {
        if input.pressed(KeyCode::Escape) {
            interior.stand = None;
        } else {
            let g = interior.plan.gravity();
            stand_input(ctx, s, g);
        }
        return true;
    }
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
                Dialog::Compare(list) => {
                    let name = list[k].0.clone();
                    interior.compare = Some((name.clone(), key_figures(&checked(&name))));
                }
                Dialog::Stamp(list) => match Assembly::load(&list[k].0) {
                    Some(a) => {
                        interior.message = Some((format!("{}: CLICK WHERE ITS FOOT GOES ON THE PLANE; RIGHT-CLICK OR ESC STOPS", list[k].0.to_uppercase()), 8.0));
                        (interior.clip, interior.stamping, interior.tool) = (Some(a), true, Tool::Look);
                    }
                    None => interior.message = Some(("THAT ASSEMBLY WOULDN'T OPEN".into(), 4.0)),
                },
            }
        }
        interior.cursor = input.cursor;
        return true;
    }
    // Typing exact values: ESC puts it away (not the studio).
    if interior.entry.is_some() && input.pressed(KeyCode::Escape) {
        interior.entry = None;
        interior.entry_targets = false;
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
    // TEST DRIVE (on the budgets panel, or CTRL+T): the design flown.
    if (input.button_pressed(MouseButton::Left) && inside(drive_button(), input.cursor)) || (ctrl && input.pressed(KeyCode::KeyT)) {
        interior.balance = Some(crate::test_drive::Balance::new(craft(interior)));
        return true;
    }
    // SET TARGETS (on the budgets panel): the job typed in.
    if input.button_pressed(MouseButton::Left) && inside(interior.targets_rect.get(), input.cursor) {
        interior.entry_targets = true;
        interior.entry = exact_of(interior).map(|e| e.1);
        return true;
    }
    // The issue list: PGDN / PGUP (next, back) anywhere; a row clicked, the wheel
    // over the panel (and nothing else under it).
    let over = inside(interior.budget_rect.get(), input.cursor);
    let paged = input.pressed(KeyCode::PageDown) || input.pressed(KeyCode::PageUp);
    if paged || (over && (input.button_pressed(MouseButton::Left) || input.scroll != 0.0)) {
        let b = budget(interior);
        let list = issues(interior, &b.faults);
        let (_, _, boxes) = budget_panel(ctx.hud_size.as_vec2(), b.lines.len(), list.len(), interior.issue_top);
        if paged && !list.is_empty() {
            let n = match interior.issue {
                Some(n) if input.pressed(KeyCode::PageUp) => (n + list.len() - 1) % list.len(),
                Some(n) => (n + 1) % list.len(),
                None => 0,
            };
            interior.go_to(&list, n);
        } else if let Some((n, _)) = boxes.iter().find(|(_, r)| input.button_pressed(MouseButton::Left) && inside(*r, input.cursor)) {
            interior.go_to(&list, *n);
        } else if input.scroll != 0.0 {
            interior.issue_top = (interior.issue_top as f32 - input.scroll.signum() * 2.0).clamp(0.0, list.len().saturating_sub(ISSUE_ROWS) as f32) as usize;
        }
        interior.cursor = input.cursor;
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
    // MIRROR (X): off, across x = 0, across z = 0, both.
    if input.pressed(KeyCode::KeyX) && !ctrl {
        interior.mirror = (interior.mirror + 1) % 4;
        let say = ["MIRROR OFF", "MIRROR: LEFT AND RIGHT (ACROSS X = 0)", "MIRROR: FORE AND AFT (ACROSS Z = 0)", "MIRROR: BOTH WAYS"][interior.mirror as usize];
        interior.message = Some((say.into(), 4.0));
    }
    let before = interior.plan.clone();
    interior.bulk = false;
    let stay = input_plan(ctx, interior);
    // (An edit made across the mirror planes too, unless a generator made it.)
    if interior.mirror > 0 && !interior.bulk && interior.plan != before && interior.plan.hull == before.hull {
        let mut plan = interior.plan.clone();
        mirror_edit(&before, &mut plan, interior.mirror);
        interior.plan = plan;
    }
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
    // FIT FRAME's news: each step said; done, its frame put in (an undo step), if the
    // plan's as it was.
    let mut fitted = None;
    if let Some((plan, rx, _)) = &interior.fitting {
        while let Ok(f) = rx.try_recv() {
            match f {
                Fitting::Step(s) => interior.message = Some((format!("FIT FRAME: {s} (CLICK AGAIN TO STOP)"), 600.0)),
                Fitting::Done(p, rounds) => fitted = Some((plan.clone(), p, rounds)),
            }
        }
    }
    if let Some((was, p, rounds)) = fitted {
        interior.fitting = None;
        match p {
            Some(p) if was == interior.plan => {
                let mass = |i: &Interior| {
                    let m = frame_masses(i);
                    (m.0 + m.1 + m.2) / 1000.0
                };
                let before = mass(interior);
                interior.undo.push(interior.plan.clone());
                interior.redo.clear();
                interior.plan = *p;
                interior.bulk = true;
                interior.message = Some((format!("FRAME FITTED IN {rounds} ROUNDS: {:.1} T (WAS {before:.1} T); WHAT'S LEFT IS IN THE ISSUES", mass(interior)), 10.0));
            }
            Some(_) => interior.message = Some(("FIT FRAME DROPPED: THE PLAN CHANGED MEANWHILE".into(), 5.0)),
            None => interior.message = Some(("FIT FRAME STOPPED: NOTHING CHANGED".into(), 4.0)),
        }
    }
    // AUTO-SIZE done: its members put in (an undo step), if the plan's as it was.
    if let Some((plan, rx)) = &interior.sizing
        && let Ok((beams, plates, rounds, maxed)) = rx.try_recv()
    {
        if *plan == interior.plan {
            let before = interior.plan.clone();
            let mass = |bs: &[Beam]| bs.iter().filter_map(|b| stocks().iter().find(|s| s.key == b.stock).map(|s| s.per_metre * f64::from(b.a.distance(b.b)))).sum::<f64>();
            let (was, now) = (mass(&before.beams), mass(&beams));
            interior.plan.beams = beams;
            interior.plan.plates = plates;
            interior.bulk = true;
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
            let g = plan.gravity();
            tx.send(bearing(&plan, &fit, spec, g)).ok();
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
    // (And the frame's members, as the solid tubes they are.)
    if interior.member_clash.as_ref().is_none_or(|(p, _)| *p != interior.plan) {
        interior.member_clash = Some((interior.plan.clone(), member_clashes(&interior.plan)));
        interior.node_cache = Some((interior.plan.clone(), nodes(&interior.plan.beams, &roots(&interior.plan, &interior.fit))));
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
    // Typing exact values for what's picked: digits, signs, points, commas and
    // spaces; BACKSPACE; ENTER sets them (and nothing else meanwhile).
    if let Some(text) = interior.entry.as_mut() {
        text.extend(input.typed.chars().filter(|c| c.is_ascii_digit() || matches!(c, '-' | '.' | ',' | ' ')));
        if input.pressed(KeyCode::Backspace) {
            text.pop();
        }
        if input.pressed(KeyCode::Enter) || input.pressed(KeyCode::NumpadEnter) {
            let values: Vec<f32> = text.split([',', ' ']).filter_map(|v| v.trim().parse().ok()).collect();
            let said = set_exact(interior, &values);
            interior.message = Some((said, 5.0));
            interior.entry = None;
        }
        return true;
    }
    if input.pressed(KeyCode::Enter) {
        match exact_of(interior) {
            Some((_, now)) => interior.entry = Some(now),
            None => interior.message = Some(("PICK A MODULE, A MEMBER OR A DECK (SELECT) TO TYPE WHERE IT IS".into(), 4.0)),
        }
        return true;
    }
    // Views: 1 front, 2 side, 3 top (flat, the ship's sizes shown), 4 as it was.
    for (k, key) in [KeyCode::Digit1, KeyCode::Digit2, KeyCode::Digit3, KeyCode::Digit4].into_iter().enumerate() {
        if input.pressed(key) {
            let (yaw, pitch) = [(0.0, 0.0), (std::f32::consts::FRAC_PI_2, 0.0), (0.0, 1.5695), (0.7, 0.35)][k];
            (interior.yaw, interior.pitch, interior.view) = (yaw, pitch, if k < 3 { k as u8 + 1 } else { 0 });
            interior.distance = None;
        }
    }
    // C: COMPARE with another saved design (C again: put away).
    if input.pressed(KeyCode::KeyC) && !(input.down(KeyCode::ControlLeft) || input.down(KeyCode::ControlRight)) {
        if interior.compare.take().is_none() {
            interior.dialog = Some(Dialog::Compare(Interior::designs().into_iter().filter(|d| d.0 != interior.plan.hull).collect()));
        }
        return true;
    }
    // G: the grid the work plane snaps to (a quarter, half, one, two metres).
    if input.pressed(KeyCode::KeyG) {
        let next = match interior.snap.unwrap_or(0.25) {
            s if s < 0.3 => 0.5,
            s if s < 0.6 => 1.0,
            s if s < 1.5 => 2.0,
            _ => 0.25,
        };
        interior.snap = Some(next);
        interior.message = Some((format!("THE PLANE SNAPS TO {next} M"), 3.0));
    }
    // Q: MEASURE: two clicks (at joints, else on the work plane), the distance
    // between them; Q again: off.
    if input.pressed(KeyCode::KeyQ) {
        interior.measuring = !interior.measuring;
        interior.measure_from = None;
        interior.message = Some((if interior.measuring { "MEASURE: CLICK TWO POINTS (JOINTS SNAP); Q OR ESC STOPS" } else { "MEASURE OFF" }.into(), 4.0));
    }
    if interior.measuring && pressed && !inside(PANEL, cursor) && cursor.y > 50.0 {
        let plane = plane_of(interior, &h);
        if let Some(at) = frame_snap(interior, &cam, cursor).or_else(|| on_plane(interior, &cam, plane, cursor, false)) {
            match interior.measure_from.take() {
                None => interior.measure_from = Some(at),
                Some(from) => {
                    interior.measured = Some((from, at));
                    let d = at - from;
                    interior.message = Some((format!("{:.2} M (ACROSS {:.2}, UP {:.2}, ALONG {:.2})", d.length(), d.x.abs(), d.y.abs(), d.z.abs()), 10.0));
                }
            }
        }
        return true;
    }
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
    // Assemblies: LEVEL (the picked deck's level picked), COPY (CTRL+C), STAMP
    // (CTRL+V), SAVE ASSY, ASSEMBLIES (a saved one to stamp), LEVEL UP (the picked
    // deck's level copied on top of the ship).
    let ctrl = input.down(KeyCode::ControlLeft) || input.down(KeyCode::ControlRight);
    let deck = match interior.pick {
        Some(Hover::Deck(n)) => Some(n),
        _ => None,
    };
    if action == Some(Action::Level) || action == Some(Action::LevelUp) {
        match deck.and_then(|n| level(&interior.plan, n)) {
            None => interior.message = Some(("PICK A DECK WITH ANOTHER DECK OVER IT: ITS LEVEL IS WHAT'S BETWEEN".into(), 5.0)),
            Some((picks, height)) if action == Some(Action::LevelUp) => {
                // (Its foot where it stands, raised so this deck's level sits on the
                // top deck.)
                let placed = Assembly::as_placed(&interior.plan, &picks);
                let a = placed.moved(-placed.foot());
                let y0 = interior.plan.plates[deck.unwrap_or(0)].y;
                let top = interior.plan.plates.iter().map(|p| p.y).fold(f32::MIN, f32::max);
                a.stamp(&mut interior.plan, placed.foot() + Vec3::Y * (top - y0));
                interior.bulk = true;
                interior.message = Some((format!("LEVEL COPIED ON TOP: {} ({height:.1} M HIGH)", a.says()), 5.0));
            }
            Some((picks, height)) => {
                interior.pick = picks.first().copied();
                interior.set = picks.into_iter().skip(1).collect();
                interior.message = Some((format!("THE LEVEL PICKED: {} ITEMS, {height:.1} M HIGH; COPY OR LEVEL UP", interior.set.len() + 1), 5.0));
            }
        }
        return true;
    }
    if action == Some(Action::Copy) || (ctrl && input.pressed(KeyCode::KeyC)) {
        let a = Assembly::of(&interior.plan, &interior.picked_items());
        interior.message = Some((if a.is_empty() { "PICK MODULES, MEMBERS OR DECKS FIRST (SHIFT-CLICK FOR MORE, OR LEVEL)".to_string() } else { format!("COPIED: {}", a.says()) }, 4.0));
        if !a.is_empty() {
            interior.clip = Some(a);
        }
        return true;
    }
    if action == Some(Action::Paste) || (ctrl && input.pressed(KeyCode::KeyV)) {
        if interior.clip.is_some() {
            interior.stamping = true;
            interior.message = Some(("STAMP: CLICK WHERE ITS FOOT GOES ON THE WORK PLANE ([ AND ] MOVE IT); RIGHT-CLICK OR ESC STOPS".into(), 8.0));
        } else {
            interior.message = Some(("NOTHING COPIED YET".into(), 3.0));
        }
        return true;
    }
    if action == Some(Action::SaveAssembly) {
        let a = interior.clip.clone().unwrap_or_else(|| Assembly::of(&interior.plan, &interior.picked_items()));
        interior.message = Some((match (a.is_empty(), a.save()) {
            (true, _) => "COPY OR PICK SOMETHING FIRST".to_string(),
            (false, Some(name)) => format!("SAVED AS {}: {}", name.to_uppercase(), a.says()),
            (false, None) => "COULDN'T SAVE IT".to_string(),
        }, 5.0));
        return true;
    }
    if action == Some(Action::Assemblies) {
        interior.dialog = Some(Dialog::Stamp(Assembly::saved()));
        return true;
    }
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
            // (DECK: a row of deck stock: what new decks, or the picked one, are cut from.)
            if interior.deck_mode
                && pressed
                && let Some(k) = (0..STOCK_ROWS).find(|&n| interior.stock_top + n < plate_stocks().len() && inside(stock_row(n), cursor)).map(|n| interior.stock_top + n)
            {
                let key = plate_stocks()[k].key.clone();
                if let Some(p) = interior.deck_pick.and_then(|j| interior.plan.plates.get_mut(j)) {
                    p.stock = key.clone();
                }
                interior.deck_stock = Some(key);
                return true;
            }
            if pressed && !interior.deck_mode && let Some(k) = (0..STOCK_ROWS).find(|&n| interior.stock_top + n < stocks().len() && inside(stock_row(n), cursor)).map(|n| interior.stock_top + n) {
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
                (interior.deck_mode, interior.beam_from, interior.truss_from, interior.deck_from) = (false, None, None, None);
            }
            if action == Some(Action::Mix) {
                interior.mix = !interior.mix;
            }
            if action == Some(Action::Deck) {
                interior.deck_mode = !interior.deck_mode;
                (interior.stock_top, interior.deck_pick) = (0, None);
                (interior.truss_mode, interior.beam_from, interior.truss_from, interior.deck_from) = (false, None, None, None);
            }
            if let Some(a @ (Action::GravityDown | Action::GravityUp)) = action {
                let g = interior.plan.gravity() / STANDARD_G + if a == Action::GravityUp { 0.1 } else { -0.1 };
                interior.plan.gravity = Some((g * 10.0).round().clamp(1.0, 30.0) / 10.0 * STANDARD_G);
            }
            // (BRACE: what's past its limit as last worked out, braced.)
            if action == Some(Action::Brace) {
                match interior.bearing.as_ref().filter(|(p, _)| *p == interior.plan) {
                    Some((_, b)) => {
                        let (beams, n) = brace(&interior.plan, b);
                        interior.message = Some((if n == 0 { "NOTHING PAST ITS LIMIT TO BRACE".to_string() } else { format!("{n} MEMBERS BRACED: AUTO-SIZE THEM") }, 5.0));
                        interior.plan.beams = beams;
                        interior.bulk = true;
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
                interior.bulk = true;
            }
            // (FIT FRAME: run (on a thread), or, running, stopped.)
            if action == Some(Action::FitFrame) {
                if let Some((_, _, stop)) = &interior.fitting {
                    stop.store(true, std::sync::atomic::Ordering::Relaxed);
                    interior.message = Some(("FIT FRAME STOPPING".into(), 3.0));
                } else if interior.plan.blocks.is_empty() {
                    interior.message = Some(("NOTHING TO FIT A FRAME TO: PLACE MODULES FIRST".into(), 4.0));
                } else {
                    let (plan, fit, spec) = (interior.plan.clone(), interior.fit.clone(), interior.spec());
                    let how = FitHow { stock: stocks().get(interior.stock).map_or(String::new(), |s| s.key.clone()), spacing: interior.spacing.max(1.0), mix: interior.mix };
                    let stop = Arc::new(std::sync::atomic::AtomicBool::new(false));
                    let (tx, rx) = mpsc::channel();
                    let (snapshot, halt) = (plan.clone(), stop.clone());
                    job("studio-fit", move || {
                        let say = |f: Fitting| {
                            tx.send(f).ok();
                        };
                        let done = fit_frame(&plan, &fit, spec, &how, &halt, &say);
                        let rounds = done.as_ref().map_or(0, |d| d.1);
                        tx.send(Fitting::Done(done.map(|d| Box::new(d.0)), rounds)).ok();
                    });
                    interior.fitting = Some((snapshot, rx, stop));
                    interior.message = Some(("FIT FRAME: STARTING (CLICK AGAIN TO STOP)".into(), 30.0));
                }
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
    // (No hull: the test stand. WALK arms it, and a click puts the walker where it
    // lands on the ship or the ground; F there at once. Nothing under the cursor:
    // the ramp's foot.)
    if interior.spec().is_none() {
        if pressed && inside(walk_button(), cursor) {
            interior.walk_armed = !interior.walk_armed;
            return true;
        }
        if input.pressed(KeyCode::KeyF) || (interior.walk_armed && pressed && !inside(PANEL, cursor)) {
            interior.refit();
            let mut s = interior.stand_up();
            if let Some(feet) = stand_spot(&s, cam.eye, cam.ray(cursor)) {
                s.feet = feet;
                s.yaw = f64::from((-cam.forward.x).atan2(-cam.forward.z));
            }
            interior.stand = Some(s);
            interior.walk_armed = false;
            return true;
        }
    }
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
            _ => None,
        };
        if let Some(at) = spot {
            interior.walk = Some(at);
            interior.walk_armed = false;
            return true;
        }
    }
    // (MODULES: T turns the picked engine: which way it pushes the ship, forward, up
    // (its jet down: to land), back, down, to starboard, to port.)
    if interior.tool == Tool::Modules
        && input.pressed(KeyCode::KeyT)
        && let Some(n) = interior.block.filter(|&n| n < interior.plan.blocks.len())
        && let Some(f) = interior.fit.iter().find(|f| f.id == kind(&interior.plan.blocks[n].id))
        && let Some((now, _)) = push_of(&interior.plan.blocks[n], f)
    {
        const WAYS: [Vec3; 6] = [Vec3::NEG_Z, Vec3::Y, Vec3::Z, Vec3::NEG_Y, Vec3::X, Vec3::NEG_X];
        let at = WAYS.iter().position(|w| w.distance(now) < 0.1).unwrap_or(0);
        interior.plan.blocks[n].push = Some(WAYS[(at + 1) % WAYS.len()]);
        return true;
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
        if d != Vec2::ZERO {
            interior.view = 0;
        }
        interior.pitch = (interior.pitch + d.y * 0.008).clamp(-1.5, 1.5);
    }
    // Stamping: a click puts the copy's foot there on the work plane (to the half
    // metre); again and again till ESC or the right button.
    if interior.stamping
        && !input.button_down(MouseButton::Left)
        && interior.press.is_some()
        && !dragged
        && let Some(at) = on_plane(interior, &cam, plane, cursor, false)
        && let Some(clip) = interior.clip.clone()
    {
        interior.press = None;
        let at = Vec3::new((at.x * 2.0).round() / 2.0, at.y, (at.z * 2.0).round() / 2.0);
        clip.stamp(&mut interior.plan, at);
        interior.message = Some((format!("STAMPED: {}", clip.says()), 4.0));
    }
    if !input.button_down(MouseButton::Left)
        && interior.press.take().is_some()
        && !dragged
    {
        match interior.tool {
            // FRAME: a click at a joint or a bearer (else on the work plane) ends the
            // member being laid there and starts the next from it; not laying one, a
            // click on a member picks it, else starts one.
            // (DECK: on a deck at the plane's height, it's taken up; else its first
            // corner, then its second: the deck laid between, of the lightest plate.)
            Tool::Frame if interior.deck_mode => {
                if let Some(at) = on_plane(interior, &cam, plane, cursor, false) {
                    let on = interior.plan.plates.iter().position(|p| (p.y - at.y).abs() < 0.3 && at.x >= p.lo.x.min(p.hi.x) && at.x <= p.lo.x.max(p.hi.x) && at.z >= p.lo.y.min(p.hi.y) && at.z <= p.lo.y.max(p.hi.y));
                    match (on, interior.deck_from.take()) {
                        // (A deck: picked; picked already: taken up.)
                        (Some(k), None) if interior.deck_pick == Some(k) => {
                            interior.plan.plates.remove(k);
                            interior.deck_pick = None;
                        }
                        (Some(k), None) => interior.deck_pick = Some(k),
                        (_, None) => {
                            interior.deck_pick = None;
                            interior.deck_from = Some(at);
                        }
                        (_, Some(from)) => {
                            let stock = interior.deck_stock.clone().unwrap_or_else(default_deck_stock);
                            interior.plan.plates.push(Plate { lo: Vec2::new(from.x, from.z), hi: Vec2::new(at.x, at.z), y: from.y, stock });
                        }
                    }
                }
            }
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
                        interior.plan.beams.push(Beam { a: from, b: to, stock: stock.key.clone(), pinned: [false; 2] });
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
                                interior.plan.blocks.push(Block { id, at: at + Vec3::Y * f.size.y * 0.5, size: f.size, push: None });
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
                    _ => on_plane(interior, &cam, plane, cursor, interior.shift).map(|at| {
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
                                let say = if next == End::Door { "A DOOR WHERE IT JOINS THE NEXT TUBE" } else { "THAT END JOINS ANOTHER TUBE: A WAY THROUGH" };
                                interior.message = Some((say.into(), 3.0));
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
                // (SHIFT: a module, member or deck picked with the rest, or put back.)
                (Some(h @ (Hover::Module(_) | Hover::Member(_) | Hover::Deck(_))), true) => {
                    if interior.pick == Some(h) {
                        interior.pick = interior.set.pop();
                    } else if let Some(at) = interior.set.iter().position(|&x| x == h) {
                        interior.set.remove(at);
                    } else if interior.pick.is_none() {
                        interior.pick = Some(h);
                    } else {
                        interior.set.push(h);
                    }
                    interior.more.clear();
                }
                (other, _) => {
                    interior.pick = other;
                    interior.more.clear();
                    interior.set.clear();
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
            let most = if interior.deck_mode { plate_stocks().len() } else { stocks().len() }.saturating_sub(STOCK_ROWS);
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

pub fn draw(frame: &mut Frame, place: &str, interior: &Interior) {
    if let Some(b) = &interior.balance {
        crate::test_drive::draw_balance(frame, b);
        return;
    }
    if let Some(d) = &interior.drive {
        crate::test_drive::draw(frame, d);
        return;
    }
    if let Some(s) = &interior.stand {
        draw_stand(frame, s, place);
        return;
    }
    let size = frame.size();
    let spec = interior.spec();
    frame.hud_rect(Vec2::ZERO, size, PAPER);
    let on = spec.map_or("NO HULL".to_string(), |s| s.name.to_uppercase());
    frame.text(Vec2::new(12.0, 10.0), &format!("{place}   INTERIOR STUDIO - {} ({on})", interior.plan.hull.to_uppercase()), LABEL);
    let hint = match interior.tool {
        _ if interior.walk_armed && interior.spec().is_none() => "CLICK WHERE TO START WALKING (A ROOM: ON ITS FLOOR) - RIGHT-CLICK OR ESC CANCELS",
        Tool::Modules => "PICK ONE IN THE LIST, CLICK THE PLANE: IT STANDS THERE - CLICK ONE IN THE VIEW TO PICK IT - RIGHT-CLICK OR ESC DROPS IT - WHEEL OVER THE LIST SCROLLS",
        _ => "LEFT-DRAG TURNS IT - RIGHT-DRAG MOVES IT - WHEEL: NEARER, FARTHER - HOME: AS IT WAS - ESC CLOSES",
    };
    let mirror = ["X MIRROR", "X MIRROR (LEFT-RIGHT)", "X MIRROR (FORE-AFT)", "X MIRROR (BOTH)"][interior.mirror as usize];
    let snap = interior.snap.unwrap_or(0.25);
    frame.text_scaled(Vec2::new(12.0, size.y - 18.0), &format!("{hint} - {mirror} - G SNAP {snap} M - Q MEASURE - 1 2 3 FRONT SIDE TOP, 4 TURNED - ENTER TYPES WHERE"), LABEL.scale(0.6), 0.7);
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
            let (p, c) = drive_button();
            let lamp = if inside((p, c), interior.cursor) { Lamp::On } else { Lamp::Off };
            draw_cell(frame, p, c, "^T", "DRIVE", lamp);
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
    // (The mirror planes, MIRROR on: each outlined up to the grid's top, crossed.)
    let plane_col = Color([1.0, 0.45, 0.85, 0.7]);
    if interior.mirror & 1 != 0 {
        for (a, b) in [((gz0, lo.y), (gz1, lo.y)), ((gz1, lo.y), (gz1, top)), ((gz1, top), (gz0, top)), ((gz0, top), (gz0, lo.y)), ((gz0, lo.y), (gz1, top))] {
            seg(frame, Vec3::new(0.0, a.1, a.0), Vec3::new(0.0, b.1, b.0), plane_col);
        }
    }
    if interior.mirror & 2 != 0 {
        for (a, b) in [((gx0, lo.y), (gx1, lo.y)), ((gx1, lo.y), (gx1, top)), ((gx1, top), (gx0, top)), ((gx0, top), (gx0, lo.y)), ((gx0, lo.y), (gx1, top))] {
            seg(frame, Vec3::new(a.0, a.1, 0.0), Vec3::new(b.0, b.1, 0.0), plane_col);
        }
    }
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
    // (Stamping: the copy where a click would put it, faint.)
    if interior.stamping
        && let Some(clip) = &interior.clip
        && let Some(at) = on_plane(interior, &cam, plane, interior.cursor, false)
    {
        let ghost = clip.moved(Vec3::new((at.x * 2.0).round() / 2.0, at.y, (at.z * 2.0).round() / 2.0));
        for b in &ghost.blocks {
            let round = interior.fit.iter().find(|f| f.id == kind(&b.id)).is_some_and(|f| f.round);
            for [a, z] in b.edges(round) {
                seg(frame, a, z, MODULE.scale(0.45));
            }
        }
        for b in &ghost.beams {
            seg(frame, b.a, b.b, PICKED.scale(0.45));
        }
        for p in &ghost.plates {
            let c = [Vec3::new(p.lo.x, p.y, p.lo.y), Vec3::new(p.hi.x, p.y, p.lo.y), Vec3::new(p.hi.x, p.y, p.hi.y), Vec3::new(p.lo.x, p.y, p.hi.y)];
            for k in 0..4 {
                seg(frame, c[k], c[(k + 1) % 4], DECK.scale(0.45));
            }
        }
    }
    // (A flat view: the ship's sizes. MEASURE: the last measure, and from its first
    // point to the cursor.)
    if interior.view > 0 {
        draw_sizes(frame, interior, &cam);
    }
    let ruler = Color([1.0, 0.85, 0.4, 0.95]);
    let measure = |frame: &mut Frame, a: Vec3, b: Vec3| {
        seg(frame, a, b, ruler);
        if let (Some((pa, _)), Some((pb, _))) = (cam.project(a), cam.project(b)) {
            frame.hud_box(pa - Vec2::splat(3.0), Vec2::splat(6.0), ruler);
            frame.hud_box(pb - Vec2::splat(3.0), Vec2::splat(6.0), ruler);
            frame.text_scaled((pa + pb) * 0.5 + Vec2::new(6.0, -10.0), &format!("{:.2} M", a.distance(b)), ruler, 0.75);
        }
    };
    if let Some((a, b)) = interior.measured {
        measure(frame, a, b);
    }
    if interior.measuring
        && let Some(from) = interior.measure_from
        && let Some(to) = frame_snap(interior, &cam, interior.cursor).or_else(|| on_plane(interior, &cam, plane, interior.cursor, false))
    {
        measure(frame, from, to);
    }
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
            Some(Hover::Point(_) | Hover::Module(_) | Hover::Member(_) | Hover::Deck(_) | Hover::Node(_)) => {}
            // (FRAME: where a click would join, and the member being laid to it.)
            // (DECK: from its first corner to the cursor, its plate.)
            None if interior.tool == Tool::Frame && interior.deck_mode => {
                if let Some(to) = on_plane(interior, &cam, plane, interior.cursor, false) {
                    let from = interior.deck_from.unwrap_or(to);
                    let c = [Vec3::new(from.x, from.y, from.z), Vec3::new(to.x, from.y, from.z), Vec3::new(to.x, from.y, to.z), Vec3::new(from.x, from.y, to.z)];
                    for k in 0..4 {
                        seg(frame, c[k], c[(k + 1) % 4], PICKED.scale(0.7));
                    }
                    if let Some((q, _)) = cam.project(to) {
                        let text = if interior.deck_from.is_some() { format!("DECK {:.1} X {:.1} M", (to.x - from.x).abs(), (to.z - from.z).abs()) } else { "FIRST CORNER (ON A DECK: TAKE IT UP)".to_string() };
                        frame.text_scaled(q + Vec2::new(10.0, 6.0), &text, PICKED, 0.7);
                    }
                }
            }
            // (TRUSS: from its first corner to the cursor, its box.)
            None if interior.tool == Tool::Frame && interior.truss_mode => {
                if let Some(to) = on_plane(interior, &cam, plane, interior.cursor, false) {
                    let from = interior.truss_from.unwrap_or(to);
                    let (lo, hi) = (from.min(Vec3::new(to.x, from.y, to.z)), from.max(Vec3::new(to.x, from.y, to.z)) + Vec3::Y * interior.depth);
                    let ghost = Block { id: String::new(), at: (lo + hi) * 0.5, size: hi - lo, push: None };
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
                    let ghost = Block { id: f.id.clone(), at: at + Vec3::Y * size.y * 0.5, size, push: None };
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
    // The rooms' openings (PRESSURE): a leak to space red and crossed, a hatch to
    // space with no airlock amber, an airlock's door green.
    if interior.shown(layer::PRESSURE) && interior.spec().is_none() {
        for (at, what) in pressure(interior).openings {
            let Some((q, _)) = cam.project(at) else { continue };
            let col = match what {
                Opening::Leak => CLASH,
                Opening::ToSpace => Color([1.0, 0.7, 0.2, 1.0]),
                Opening::Airlock => Color([0.4, 1.0, 0.5, 1.0]),
                Opening::Inner => continue,
            };
            let n = 16;
            for k in 0..n {
                let (a, b) = (k as f32 / n as f32 * std::f32::consts::TAU, (k + 1) as f32 / n as f32 * std::f32::consts::TAU);
                frame.hud_line(q + Vec2::new(a.cos(), a.sin()) * 7.0, q + Vec2::new(b.cos(), b.sin()) * 7.0, col);
            }
            if what == Opening::Leak {
                frame.hud_line(q - Vec2::splat(5.0), q + Vec2::splat(5.0), col);
                frame.hud_line(q + Vec2::new(-5.0, 5.0), q + Vec2::new(5.0, -5.0), col);
            }
        }
    }
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
        let lit = (interior.tool == Tool::Modules && (interior.block == Some(n) || interior.block_hover == Some(n))) || [interior.pick, interior.hover].contains(&Some(Hover::Module(n))) || interior.set.contains(&Hover::Module(n));
        // (An engine: an arrow from its middle, the way it pushes the ship; a second
        // up, if a swivel turns it to land.)
        if let Some((d, _)) = push_of(b, f) {
            let reach = b.size.max_element() * 0.6 + 1.0;
            let tip = b.at + d * reach;
            let col = Color([1.0, 0.85, 0.3, 1.0]);
            seg(frame, b.at, tip, col);
            let side = d.cross(if d.y.abs() > 0.9 { Vec3::X } else { Vec3::Y }).normalize_or_zero() * 0.4;
            seg(frame, tip, tip - d * 0.6 + side, col);
            seg(frame, tip, tip - d * 0.6 - side, col);
            if d.y < 0.5 && swivelled(plan, &interior.fit, b) {
                seg(frame, b.at, b.at + Vec3::Y * reach, col.scale(0.6));
            }
        }
        let col = if lit { PICKED } else if block_clash.get(n) == Some(&true) && interior.shown(layer::CLASHES) { CLASH } else { MODULE };
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
    // The decks: each plate filled and outlined (its own layer, frame shown or not).
    for (n, plate) in plan.plates.iter().enumerate().filter(|_| interior.shown(layer::DECKS)) {
        let deck_col = if [interior.pick, interior.hover].contains(&Some(Hover::Deck(n))) || interior.set.contains(&Hover::Deck(n)) { PICKED } else { DECK };
        let c = [Vec3::new(plate.lo.x, plate.y, plate.lo.y), Vec3::new(plate.hi.x, plate.y, plate.lo.y), Vec3::new(plate.hi.x, plate.y, plate.hi.y), Vec3::new(plate.lo.x, plate.y, plate.hi.y)];
        if let [Some((a, _)), Some((b, _)), Some((cc, _)), Some((d, _))] = c.map(|p| cam.project(p)) {
            let fill = [Color([DECK.0[0], DECK.0[1], DECK.0[2], 0.45]); 3];
            frame.hud_triangle_colored([a, b, cc], fill);
            frame.hud_triangle_colored([a, cc, d], fill);
        }
        // (Its underside too: it is as thick as its stock.)
        let under = c.map(|p| p - Vec3::Y * deck_depth(plate));
        for k in 0..4 {
            seg(frame, c[k], c[(k + 1) % 4], deck_col);
            seg(frame, under[k], under[(k + 1) % 4], deck_col.scale(0.6));
            seg(frame, c[k], under[k], deck_col.scale(0.6));
        }
    }
    // The frame: each member coloured by how hard it's worked (the case shown, or
    // its worst): green easy, amber near its limit, red past it; broken, red and
    // crossed. Its load points: the landing pads and nozzles (lit if a joint's at
    // them).
    if interior.shown(layer::FRAME) {
        let bearing = interior.bearing.as_ref().filter(|(p, _)| *p == interior.plan).map(|(_, b)| b.clone());
        let member_clash: &[Option<Passes>] = if interior.shown(layer::CLASHES) { interior.member_clash.as_ref().filter(|(p, _)| *p == interior.plan).map_or(&[], |(_, c)| c.as_slice()) } else { &[] };
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
            let lit = interior.beam_pick == Some(k) || interior.beam_hover == Some(k) || [interior.pick, interior.hover].contains(&Some(Hover::Member(k))) || interior.set.contains(&Hover::Member(k));
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
                    // (The solid tube it is: as wide as its diameter at each end, seen
                    // from here; at least a line. Clashing (CLASHES shown): outlined red.)
                    let r = stocks().iter().find(|s| s.key == b.stock).map_or(0.03, |s| s.section.diameter as f32 * 0.5);
                    let (za, zb) = (cam.project(b.a).map_or(1.0, |p| p.1), cam.project(b.b).map_or(1.0, |p| p.1));
                    let side = (pb - pa).perp().normalize_or_zero();
                    let (wa, wb) = ((r * cam.focal / za).max(0.6), (r * cam.focal / zb).max(0.6));
                    if member_clash.get(k).is_some_and(|c| c.is_some()) {
                        let (oa, ob) = (side * (wa + 1.5), side * (wb + 1.5));
                        frame.hud_triangle_colored([pa + oa, pb + ob, pb - ob], [CLASH; 3]);
                        frame.hud_triangle_colored([pa + oa, pb - ob, pa - oa], [CLASH; 3]);
                    }
                    let (oa, ob) = (side * wa, side * wb);
                    frame.hud_triangle_colored([pa + oa, pb + ob, pb - ob], [col; 3]);
                    frame.hud_triangle_colored([pa + oa, pb - ob, pa - oa], [col; 3]);
                }
            }
        }
        // (Its nodes, as wide as they are; red where none in stock is wide enough.)
        if let Some((_, ns)) = interior.node_cache.as_ref().filter(|(p, _)| *p == interior.plan) {
            for (at, widest, node) in ns {
                if let Some((q, z)) = cam.project(*at) {
                    let d = node.map_or(*widest, |n| n.diameter);
                    let w = (d * cam.focal / z).max(3.0);
                    let lit = [interior.pick, interior.hover].contains(&Some(Hover::Node(*at)));
                    frame.hud_rect(q - Vec2::splat(w * 0.5), Vec2::splat(w), if lit { PICKED } else if node.is_some() { Color([0.9, 0.95, 1.0, 0.9]) } else { CLASH });
                }
            }
        }
        // (The decks' strips coloured as members are.)
        if let Some(b) = &bearing {
            for &(_, sa, sb, _, m) in &b.strips {
                let os = b.of_member(m);
                let pick: Vec<_> = match interior.case {
                    Some(c) => b.cases.get(c).and_then(|c| c.1.as_ref().ok()).and_then(|c| c.members.get(m)).into_iter().collect(),
                    None => os,
                };
                let (u, broken) = (pick.iter().map(|o| o.work.design).fold(0.0, f64::max), pick.iter().any(|o| o.broken.is_some()));
                let col = if broken { CLASH } else if u >= 1.0 { Color([1.0, 0.3, 0.25, 1.0]) } else if u >= 0.5 { Color([1.0, 0.7, 0.2, 1.0]) } else { Color([0.4, 1.0, 0.5, 0.8]) };
                seg(frame, sa, sb, col);
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
            _ => on_plane(interior, &cam, plane, interior.cursor, interior.shift),
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
            Tool::Look => ("SELECT", "CLICK ANYTHING TO SEE WHAT IT IS AND WHAT IT CARRIES: A MODULE, A MEMBER, A NODE, A DECK, A ROOM. A TUBE IN A GROUP PICKS THE GROUP; SHIFT-CLICK TUBES TO PICK MORE. GROUP THEM, WALL THEM OFF. DRAG TO TURN, RIGHT-DRAG TO MOVE. DEL TAKES OUT WHAT'S UNDER THE CURSOR."),
            Tool::Path => ("PATH", "CLICK THE PLANE TO LAY A POINT, JOINED TO THE LAST ONE; CLICK A POINT TO START THERE, OR TO JOIN TO IT (THAT TUNNEL DONE AND PICKED). RIGHT-CLICK STOPS. DRAG THE PLANE'S GRIP (ITS NEAR RIGHT CORNER) UP OR DOWN."),
            Tool::Door => ("DOOR", "CLICK NEAR A WALLED TUBE'S END: CLOSED, A HATCH, OPEN, IN TURN. CLICK ALONG A TUBE: A HATCH IN THE WALL FACING YOU (AGAIN: GONE). HATCHES SLIDE OPEN AS YOU COME NEAR. SET THEIR SHAPE, SIZE AND SLIDE BELOW."),
            Tool::Modules => ("MODULES", "PICK ONE, CLICK THE PLANE: IT STANDS THERE. CLICK ONE IN THE VIEW TO PICK IT. STRETCHED, IT KEEPS ITS VOLUME. T TURNS A PICKED ENGINE (ITS ARROW: WHICH WAY IT PUSHES)."),
            Tool::Frame => ("FRAME", "JOINT TO JOINT, OR TRUSS: TWO CORNERS. MOUNT ALL, AUTO-SIZE (MIX: ANY MATERIAL), BRACE WHAT'S OVER."),
        };
        frame.text(p + Vec2::new(8.0, 8.0), title, LABEL);
        let mut y = p.y + 28.0;
        // (SELECT: what's picked, on a sheet beside the panel.)
        if interior.tool == Tool::Look {
            draw_selected(frame, interior);
        }
        // (MODULES: its hint on the hint line, the panel for the list and the sheet.)
        let help = if interior.tool == Tool::Modules { "" } else { help };
        for line in crate::fmt::wrap(help, ((c.x - 16.0) / 8.0 * 1.25) as usize) {
            frame.text_scaled(Vec2::new(p.x + 8.0, y), &line, LABEL.scale(0.85), 0.8);
            y += 12.0;
        }
        // FRAME: the stock (lit: what new members, or the picked one, are cut from),
        // the case shown, how each case goes, what isn't carried, its members' mass.
        if interior.tool == Tool::Frame {
            if interior.deck_mode {
                // (DECK: the deck stock, plates and panels; lit: what new decks, or the
                // picked one, are cut from.)
                frame.text_scaled(Vec2::new(p.x + 8.0, p.y + 72.0), "DECK STOCK               KG/M2  YIELD", LABEL.scale(0.7), 0.6);
                if plate_stocks().len() > STOCK_ROWS {
                    let shown = format!("{}-{} OF {} (WHEEL)", interior.stock_top + 1, (interior.stock_top + STOCK_ROWS).min(plate_stocks().len()), plate_stocks().len());
                    frame.text_scaled(Vec2::new(p.x + c.x - 8.0 - shown.len() as f32 * 4.8, p.y + 62.0), &shown, LABEL.scale(0.6), 0.6);
                }
                let now = interior.deck_pick.and_then(|j| plan.plates.get(j)).map(|p| p.stock.clone()).or_else(|| interior.deck_stock.clone()).unwrap_or_else(default_deck_stock);
                for (n, st) in plate_stocks().iter().skip(interior.stock_top).take(STOCK_ROWS).enumerate() {
                    let (q, qc) = stock_row(n);
                    let lit = st.key == now;
                    if lit {
                        frame.hud_rect(q, qc, PICKED.scale(0.18));
                    }
                    let col = if lit || inside((q, qc), interior.cursor) { PICKED } else { LABEL.scale(0.75) };
                    let name: String = st.name.to_uppercase().chars().take(24).collect();
                    frame.text_scaled(Vec2::new(q.x + 4.0, q.y + 2.0), &name, col, 0.6);
                    let figures = format!("{:>5.1} {:>5.0} MPA", st.per_square_metre, st.material.yield_strength / 1e6);
                    frame.text_scaled(Vec2::new(q.x + qc.x - figures.len() as f32 * 4.8, q.y + 2.0), &figures, col, 0.6);
                }
            } else {
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
            }
            frame.text_scaled(Vec2::new(p.x + 8.0, p.y + 146.0), "LOAD CASE SHOWN", LABEL.scale(0.7), 0.6);
            for (r, name, a) in panel_buttons(Tool::Frame) {
                let lamp = if a == Action::AutoSize && interior.sizing.is_some() {
                    Lamp::Busy
                } else if inside(r, interior.cursor) || matches!(a, Action::Case(c) if c == interior.case) || (a == Action::Mix && interior.mix) || (a == Action::Truss && interior.truss_mode) || (a == Action::Deck && interior.deck_mode) {
                    Lamp::On
                } else {
                    Lamp::Off
                };
                let off = (a == Action::Remove && interior.beam_pick.is_none()) || (a == Action::AutoSize && plan.beams.is_empty());
                let name = if a == Action::AutoSize && interior.sizing.is_some() { "SIZING..." } else { name };
                draw_cell(frame, r.0, r.1, "", name, if off { Lamp::Unavailable } else { lamp });
            }
            // (The gravity it's designed to land in.)
            frame.text_scaled(Vec2::new(p.x + 8.0, p.y + 296.0), &format!("DESIGN GRAVITY {:.1} G", interior.plan.gravity() / STANDARD_G), LABEL, 0.6);
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
                                let nearest = o.iter().map(|o| o.work.breaking).fold(0.0, f64::max);
                                let broken = o.iter().filter(|o| o.broken.is_some()).count();
                                // (Past its design limit (yield, with the safety margin) is to be
                                // fixed: red, as breaking is.)
                                let col = if broken > 0 || worst > 1.0 { CLASH } else { Color([0.4, 1.0, 0.5, 1.0]) };
                                let what = match (broken, c.falls_apart) {
                                    (_, true) => format!("{broken} BREAK, IT FALLS APART"),
                                    (0, _) => format!("WORST {:.0}% DESIGN, {:.0}% BREAK", worst * 100.0, nearest * 100.0),
                                    _ => format!("{broken} BREAK"),
                                };
                                (format!("{name} {felt:.1} G: {what}"), col)
                            }
                            Err(e) => (format!("{name}: {e}"), if not_built_for(e) { LABEL.scale(0.7) } else { CLASH }),
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
                Some(Hover::Module(n)) => format!("PICKED: {}", interior.fit.iter().find(|f| plan.blocks.get(n).is_some_and(|b| f.id == kind(&b.id))).map_or("A MODULE".into(), |f| f.name.clone())),
                Some(Hover::Member(_)) => "PICKED: A FRAME MEMBER".into(),
                Some(Hover::Deck(n)) => format!("PICKED: DECK {}", n + 1),
                Some(Hover::Node(_)) => "PICKED: A NODE".into(),
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
    // The budgets, in every tool: under the panel, each met in green, short in
    // amber; what fails, in red.
    interior.budget_rect.set((Vec2::ZERO, Vec2::ZERO));
    interior.targets_rect.set((Vec2::ZERO, Vec2::ZERO));
    if !plan.blocks.is_empty() || !plan.groups.is_empty() {
        let b = budget(interior);
        let list = issues(interior, &b.faults);
        let ((p, c), (lp, lc), boxes) = budget_panel(size, b.lines.len(), list.len(), interior.issue_top.min(list.len().saturating_sub(1)));
        frame.hud_rect(p, c, Color([0.02, 0.06, 0.13, 0.88]));
        frame.hud_box(p, c, PLANE.scale(1.2));
        frame.text_scaled(p + Vec2::new(6.0, 4.0), "BUDGETS AND CHECKS", LABEL.scale(0.8), 0.6);
        // (SET TARGETS: at the header's end.)
        let tb = (Vec2::new(p.x + c.x - 96.0, p.y + 2.0), Vec2::new(92.0, 11.0));
        interior.targets_rect.set(tb);
        let lit = inside(tb, interior.cursor);
        frame.hud_rect(tb.0, tb.1, if lit { Color([0.15, 0.2, 0.35, 0.95]) } else { Color([0.05, 0.1, 0.2, 0.95]) });
        frame.hud_box(tb.0, tb.1, PLANE.scale(1.2));
        frame.text_scaled(tb.0 + Vec2::new(5.0, 2.0), "SET TARGETS", LABEL, 0.55);
        let mut y = p.y + 15.0;
        for (text, ok) in &b.lines {
            let col = if *ok { Color([0.4, 1.0, 0.5, 0.95]) } else { Color([1.0, 0.7, 0.2, 1.0]) };
            frame.text_scaled(Vec2::new(p.x + 6.0, y), text, col, 0.55);
            y += 10.0;
        }
        // (The issues, under the tool panel: each one to go to, by a click or PGDN /
        // PGUP; none: said so.)
        interior.budget_rect.set((lp, lc));
        frame.hud_rect(lp, lc, Color([0.02, 0.06, 0.13, 0.88]));
        frame.hud_box(lp, lc, PLANE.scale(1.2));
        let head = if list.is_empty() { "NO ISSUES".to_string() } else { format!("{} ISSUES: CLICK, OR PGDN / PGUP", list.len()) };
        frame.text_scaled(lp + Vec2::new(6.0, 4.0), &head, LABEL.scale(0.8), 0.6);
        for (n, (q, sz)) in &boxes {
            if interior.issue == Some(*n) {
                frame.hud_rect(*q, *sz, Color([0.3, 0.12, 0.08, 0.9]));
            } else if inside((*q, *sz), interior.cursor) {
                frame.hud_rect(*q, *sz, Color([0.12, 0.12, 0.2, 0.9]));
            }
            let text: String = format!("{:>3} {}", n + 1, list[*n].text).chars().take(46).collect();
            frame.text_scaled(Vec2::new(lp.x + 6.0, q.y + 1.0), &text, CLASH, 0.5);
        }
    }
    // COMPARE: this design's figures against the other's, a table at the top right
    // (left of the layers).
    if let Some((name, theirs)) = &interior.compare {
        let rows = compare_rows(&key_figures(interior), theirs);
        let (p, c) = (Vec2::new(size.x - 175.0 - 380.0, 56.0), Vec2::new(380.0, 22.0 + (rows.len() + 1) as f32 * 10.0));
        frame.hud_rect(p, c, Color([0.02, 0.06, 0.13, 0.9]));
        frame.hud_box(p, c, PLANE.scale(1.2));
        frame.text_scaled(p + Vec2::new(6.0, 4.0), &format!("COMPARED WITH {} (C PUTS IT AWAY)", name.to_uppercase()), LABEL.scale(0.8), 0.6);
        let cols = [6.0, 150.0, 230.0, 310.0];
        let head = ["FIGURE", "THIS", "THAT", "DIFFERENCE"];
        for (x, h) in cols.iter().zip(head) {
            frame.text_scaled(p + Vec2::new(*x, 16.0), h, LABEL.scale(0.8), 0.55);
        }
        for (n, r) in rows.iter().enumerate() {
            let y = p.y + 26.0 + n as f32 * 10.0;
            for (x, cell) in cols.iter().zip(r) {
                frame.text_scaled(Vec2::new(p.x + x, y), cell, Color([0.85, 0.9, 1.0, 0.95]), 0.55);
            }
        }
    }
    // Typing exact values: what they are, as typed, under the toolbar.
    if let Some(text) = &interior.entry {
        let what = exact_of(interior).map_or("", |e| e.0);
        let (p, c) = (Vec2::new(button(2).0.x, 66.0), Vec2::new(560.0, 20.0));
        frame.hud_rect(p, c, Color([0.02, 0.06, 0.13, 0.95]));
        frame.hud_box(p, c, PICKED);
        frame.text_scaled(p + Vec2::new(6.0, 5.0), &format!("{what}: {text}_   ENTER SETS, ESC DROPS"), PICKED, 0.7);
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
            Dialog::Stamp(list) if list.is_empty() => "NO ASSEMBLIES SAVED YET: COPY SOMETHING, THEN SAVE ASSEMBLY",
            Dialog::Stamp(_) => "STAMP WHICH ASSEMBLY?",
            Dialog::Compare(_) => "COMPARE THIS DESIGN WITH WHICH?",
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





























