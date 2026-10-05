//! The shipyard's layout studio: a ship's inside drawn as a blueprint. The
//! plan of one deck (the hull's cross-section at its height, a grid, the
//! floors and walls), and below it the ship from the side with its decks.
//! Floors and walls are trimmed to the hull, so they follow its curves; a
//! wall's segments can be bent into arcs. Plans last the session (not saved),
//! one per hull; see `world::deckplan`.
//!
//! Worked with the mouse: the toolbar along the top (each button's key too),
//! drawing and picking on the plan.
//!
//! Tools: S select (drag a point; drag a segment's middle to bend it), P
//! plane (click its corners; the first again or ENTER closes it), W wall
//! (click its points; ENTER ends it), D door (click a wall; again removes
//! it), L ladder (click: up to the deck above, through a hatch), T stair
//! (click its foot, then its head: up to the deck above). DELETE removes what's selected; BACKSPACE the last point placed; ESC
//! stops drawing. N the next deck (round from the top to the bottom), PGUP/
//! PGDN up and down (round too), a click on a deck in the side view picks it,
//! SHIFT+N adds a deck above the top one (if the hull has room), +/- its floor, [ ] its height
//! (the decks above move with either; SHIFT:
//! more), CTRL+DELETE removes it. Wheel zooms, right drag
//! pans, HOME fits. Points snap to a quarter metre (ALT: free).

use universe_engine::glam::{DVec2, Vec2};
use universe_engine::{Color, Context, Frame, KeyCode, MouseButton};
use universe_sim::world::deckplan::{self, Deck, DeckPlan, Door, Sides, Wall};

use crate::App;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Tool {
    #[default]
    Select,
    Plane,
    Wall,
    Door,
    Ladder,
    Stair,
}

/// What's picked: a wall or a plane of the current deck.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Pick {
    Wall(usize),
    Plane(usize),
    Ladder(usize),
    Stair(usize),
}

/// What a drag moves.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Drag {
    Pan,
    /// A deck's floor, dragged up or down in the side view.
    Floor(usize),
    /// The side view (0) or the end view (1), moved.
    View(u8),
    Point(Pick, usize),
    Bend(usize, usize),
}

/// The hull as the studio draws it, worked out once per hull and deck.
#[derive(Default)]
struct Hull {
    key: String,
    /// The deck height it's cut at (m), and the cut.
    y: f64,
    section: Vec<[DVec2; 2]>,
    sides: Sides,
    /// Its side view: the cut along its centre line (z, y), and the ground it
    /// stands on (its lowest point: its feet).
    profile: Vec<[DVec2; 2]>,
    /// Which way its nose is along z (+1 or -1: where its cockpit is).
    nose: f64,
    /// Its name, and its mass as built (kg: its frame and what's fitted, dry).
    name: String,
    dry_mass: f64,
    keel: f64,
    /// Its lowest floor inside (well above its feet), for a first deck.
    first_floor: f64,
    /// Its extent in plan (x, z) and in height.
    lo: universe_engine::glam::DVec3,
    hi: universe_engine::glam::DVec3,
}

/// A hull's elevations (the edges seen from outside): from either side (z, y:
/// from -x, from +x) and either end (x, y: from its nose, from its tail).
pub struct Elevations {
    side: [Vec<[DVec2; 2]>; 2],
    end: [Vec<[DVec2; 2]>; 2],
}

#[derive(Default)]
pub struct Studio {
    deck: usize,
    pub tool: Tool,
    /// Points placed so far (a plane's corners or a wall's points).
    drawing: Vec<DVec2>,
    pick: Option<Pick>,
    drag: Option<Drag>,
    /// The view: the plan point at the middle and pixels a metre (none: fit it).
    view: Option<(DVec2, f64)>,
    hull: Option<Hull>,
    /// The cursor as input last saw it (HUD pixels): what's being drawn runs to it.
    pub cursor: Vec2,
    /// A repeating step held (floor up/down, height +/-: its number), how long it's
    /// been held and when it steps next (s).
    held: Option<(usize, f32, f32)>,
    /// WALK HERE pressed away from the plan: the next click on it is where.
    walk_armed: bool,
    /// The hull's elevations, for which hull; and being worked out (away from the
    /// frame: they take a moment), for which.
    elev: Option<(String, std::sync::Arc<Elevations>)>,
    elev_job: Option<(String, std::sync::mpsc::Receiver<Elevations>)>,
    /// The side and end views: seen from the other side or end, and moved (pixels).
    pub(crate) side_flip: bool,
    pub(crate) end_flip: bool,
    side_pan: Vec2,
    end_pan: Vec2,
    /// Time, for the spinner while the elevations are worked out (s).
    spin: f32,
    /// A walk-through asked for: feet (the hull's frame) and facing (see `shipyard`).
    pub walk: Option<(universe_engine::glam::DVec3, f64)>,
}

/// The studio's regions on screen: the plan, the side view.
fn regions(size: Vec2, panel: bool) -> ((Vec2, Vec2), (Vec2, Vec2)) {
    // (Under the title, the toolbar and the line saying what's on; the plan right of
    // the tool's panel when it's open, the side view always the full width.)
    let top = 86.0;
    let left = if panel { 12.0 + PANEL_WIDTH + 10.0 } else { 12.0 };
    let bottom = size.y - 12.0;
    let split = top + (bottom - top) * 0.62;
    ((Vec2::new(left, top), Vec2::new(size.x - 12.0, split - 6.0)), (Vec2::new(12.0, split), Vec2::new(size.x - 12.0, bottom)))
}

/// The tool's panel, at the left of the plan (px).
const PANEL_WIDTH: f32 = 240.0;
/// A row of the panel's list (px).
const ROW: f32 = 16.0;

/// Is the tool's panel open: a tool that makes things, or something picked?
fn panel_open(studio: &Studio) -> bool {
    studio.tool != Tool::Select || studio.pick.is_some()
}

/// The kind of thing the panel is about: the tool's, or what's picked.
fn panel_kind(studio: &Studio) -> Tool {
    match (studio.tool, studio.pick) {
        (Tool::Select, Some(Pick::Wall(_))) => Tool::Wall,
        (Tool::Select, Some(Pick::Plane(_))) => Tool::Plane,
        (Tool::Select, Some(Pick::Ladder(_))) => Tool::Ladder,
        (Tool::Select, Some(Pick::Stair(_))) => Tool::Stair,
        (t, _) => t,
    }
}

/// Where the panel's list starts (y): under its title, where the tool is at, its
/// AUTO FILL (floors) and the list's heading. (Its help comes after the list.)
fn panel_list_top(studio: &Studio) -> f32 {
    let width = (PANEL_WIDTH / 8.0) as usize - 2;
    let state = tool_state(studio).map_or(0, |s| crate::fmt::wrap(&s, width).len() + 1);
    86.0 + 8.0 + 22.0 + state as f32 * 14.0 + if auto_shown(studio) { AUTO_ROW } else { 0.0 } + 18.0
}

/// Is the floors' FILL button shown (the plane tool in hand)?
fn auto_shown(studio: &Studio) -> bool {
    studio.tool == Tool::Plane
}

/// The floors' panel's FILL button: its place (above the list), and the room it takes.
const AUTO_ROW: f32 = 28.0;
fn auto_button(studio: &Studio) -> (Vec2, Vec2) {
    (Vec2::new(20.0, panel_list_top(studio) - 18.0 - AUTO_ROW + 2.0), Vec2::new(PANEL_WIDTH - 16.0, 18.0))
}

/// The panel's list: the things of its kind on this deck, each a row (what it is, what's picked).
fn panel_list(studio: &Studio, deck: &Deck, sides: Option<&Sides>, holes: &[Vec<DVec2>]) -> Vec<(Pick, String)> {
    match panel_kind(studio) {
        Tool::Plane => deck.planes.iter().enumerate().map(|(k, poly)| {
            let area: f64 = sides.map_or(0.0, |sd| deckplan::floor_strips(poly, sd, holes).iter().map(|s| (s.1 - s.0) * (s.3 - s.2)).sum());
            (Pick::Plane(k), format!("FLOOR {}  {:.0} M2", k + 1, area))
        }).collect(),
        Tool::Wall | Tool::Door => deck.walls.iter().enumerate().map(|(k, w)| {
            let doors = match w.doors.len() { 0 => String::new(), 1 => "  1 DOOR".into(), n => format!("  {n} DOORS") };
            (Pick::Wall(k), format!("{} {}  {:.1} M{doors}", if w.rail { "RAILING" } else { "WALL" }, k + 1, w.length()))
        }).collect(),
        Tool::Ladder => deck.ladders.iter().enumerate().map(|(k, _)| (Pick::Ladder(k), format!("LADDER {}", k + 1))).collect(),
        Tool::Stair => deck.stairs.iter().enumerate().map(|(k, st)| (Pick::Stair(k), format!("STAIR {}  {:.1} M RUN", k + 1, (st.to - st.from).length()))).collect(),
        Tool::Select => Vec::new(),
    }
}

// Blueprint colours.
const PAPER: Color = Color([0.04, 0.14, 0.28, 1.0]);
const GRID: Color = Color([0.35, 0.6, 0.9, 0.18]);
const GRID5: Color = Color([0.45, 0.7, 1.0, 0.35]);
const HULL: Color = Color([0.75, 0.88, 1.0, 0.8]);
const INK: Color = Color([0.95, 0.98, 1.0, 1.0]);
const FLOOR: Color = Color([0.55, 0.78, 1.0, 0.22]);
const PICKED: Color = Color([1.0, 0.85, 0.35, 1.0]);
/// The 3D studio's tunnels, shown here.
const TUNNEL: Color = Color([0.4, 1.0, 0.75, 0.95]);
const OUT: Color = Color([1.0, 0.45, 0.4, 0.55]);
const LABEL: Color = Color([0.7, 0.85, 1.0, 1.0]);

impl Studio {
    /// The plan for `hull` (made when first drawn on).
    fn plan<'a>(app: &'a mut App, hull: &str) -> &'a mut DeckPlan {
        if let Some(k) = app.deckplans.iter().position(|p| p.hull == hull) {
            return &mut app.deckplans[k];
        }
        app.deckplans.push(DeckPlan { hull: hull.into(), decks: Vec::new() });
        app.deckplans.last_mut().expect("just added")
    }

    fn plan_of<'a>(app: &'a App, hull: &str) -> Option<&'a DeckPlan> {
        app.deckplans.iter().find(|p| p.hull == hull)
    }

    /// Plan point ↔ screen, in the plan region (nose to the left, starboard up).
    fn to_screen(&self, region: (Vec2, Vec2), p: DVec2) -> Vec2 {
        let (c, s) = self.view.unwrap_or((DVec2::ZERO, 10.0));
        let mid = (region.0 + region.1) * 0.5;
        Vec2::new(mid.x + ((p.y - c.y) * s) as f32, mid.y - ((p.x - c.x) * s) as f32)
    }

    fn to_plan(&self, region: (Vec2, Vec2), q: Vec2) -> DVec2 {
        let (c, s) = self.view.unwrap_or((DVec2::ZERO, 10.0));
        let mid = (region.0 + region.1) * 0.5;
        DVec2::new(c.x - (q.y - mid.y) as f64 / s, c.y + (q.x - mid.x) as f64 / s)
    }

    /// The hull cut at the current deck (worked out again when the hull or deck changes).
    fn refresh(&mut self, app: &App, hull_key: &str, shape: &universe_sim::world::shape::Shape) {
        let Some(mesh) = shape.walk.as_ref() else {
            self.hull = None;
            return;
        };
        // The elevations: asked for once a hull, worked out on a thread of their own.
        if let Some((key, rx)) = &self.elev_job
            && let Ok(e) = rx.try_recv()
        {
            self.elev = Some((key.clone(), std::sync::Arc::new(e)));
            self.elev_job = None;
        }
        if self.elev.as_ref().is_none_or(|(k, _)| k != hull_key) && self.elev_job.as_ref().is_none_or(|(k, _)| k != hull_key) {
            let (tx, rx) = std::sync::mpsc::channel();
            let (mesh, nose) = (mesh.clone(), nose_of(shape));
            std::thread::spawn(move || {
                let e = Elevations { side: [mesh.elevation(0, -1.0), mesh.elevation(0, 1.0)], end: [mesh.elevation(2, nose), mesh.elevation(2, -nose)] };
                tx.send(e).ok();
            });
            self.elev_job = Some((hull_key.into(), rx));
        }
        let decks = Self::plan_of(app, hull_key).map(|p| p.decks.clone()).unwrap_or_default();
        let y = decks.get(self.deck).map_or_else(|| first_floor(mesh), |d| d.floor);
        // (Cut just over the floor: the hull as it is at the deck, its walls solid under a doorway that starts higher.)
        if self.hull.as_ref().is_some_and(|h| h.key == hull_key && (h.y - y).abs() < 1e-9) {
            return;
        }
        let section = mesh.section_y(y + 0.05);
        let sides = deckplan::deck_sides(mesh, y);
        let same = self.hull.as_ref().filter(|h| h.key == hull_key);
        let profile = same.map_or_else(|| mesh.section_x(0.0), |h| h.profile.clone());
        let first_floor = first_floor(mesh);
        self.hull = Some(Hull { key: hull_key.into(), y, section, sides, profile, nose: nose_of(shape), name: app.ship.spec().name.clone(), dry_mass: app.ship.spec().dry_mass, keel: mesh.lo.y, first_floor, lo: mesh.lo, hi: mesh.hi });
    }
}

/// A hull's lowest floor inside: looking down its centre line, the lowest
/// level surface more than 3 m over its feet (below that, its legs).
fn first_floor(mesh: &universe_sim::world::walk::WalkMesh) -> f64 {
    use universe_sim::world::walk::{ray, Collider};
    let cols = [Collider::Mesh { mesh, at: universe_engine::glam::DVec3::ZERO, rot: universe_engine::glam::DQuat::IDENTITY }];
    let mut best: Option<f64> = None;
    for k in 1..8 {
        let z = mesh.lo.z + (mesh.hi.z - mesh.lo.z) * k as f64 / 8.0;
        let mut y = mesh.hi.y;
        // (Down through every surface in turn.)
        while let Some((d, n)) = ray(&cols, universe_engine::glam::DVec3::new(0.0, y, z), universe_engine::glam::DVec3::NEG_Y, y - mesh.lo.y) {
            y -= d + 0.01;
            if n.y > 0.9 && y > mesh.lo.y + 3.0 {
                best = Some(best.map_or(y, |b: f64| b.min(y)));
            }
        }
    }
    best.unwrap_or(mesh.lo.y + 1.0)
}

/// What a toolbar button (or its key) does.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Action {
    Tool(Tool),
    NextDeck,
    AddDeck,
    DropDeck,
    Floor(f64),
    Headroom(f64),
    Finish,
    Remove,
    Fit,
    Close,
    Walk,
}

/// The toolbar: each button's key, name and what it does.
const TOOLBAR: [(&str, &str, Action); 18] = [
    ("S", "SELECT", Action::Tool(Tool::Select)),
    ("P", "PLANE", Action::Tool(Tool::Plane)),
    ("W", "WALL", Action::Tool(Tool::Wall)),
    ("D", "DOOR", Action::Tool(Tool::Door)),
    ("L", "LADDER", Action::Tool(Tool::Ladder)),
    ("T", "STAIR", Action::Tool(Tool::Stair)),
    ("ENT", "FINISH", Action::Finish),
    ("DEL", "REMOVE", Action::Remove),
    ("HOME", "FIT VIEW", Action::Fit),
    ("F", "WALK HERE", Action::Walk),
    ("ESC", "CLOSE", Action::Close),
    ("N", "NEXT DECK", Action::NextDeck),
    ("S+N", "ADD DECK", Action::AddDeck),
    ("^DEL", "DROP DECK", Action::DropDeck),
    ("-", "FLOOR DOWN", Action::Floor(-0.1)),
    ("+", "FLOOR UP", Action::Floor(0.1)),
    ("[", "HEIGHT -", Action::Headroom(-0.1)),
    ("]", "HEIGHT +", Action::Headroom(0.1)),
];

/// Where toolbar button `k` is on screen (two rows of nine along the top).
fn button(size: Vec2, k: usize) -> (Vec2, Vec2) {
    let w = ((size.x - 24.0 - 8.0 * 4.0) / 9.0).floor();
    (Vec2::new(12.0 + (k % 9) as f32 * (w + 4.0), 30.0 + (k / 9) as f32 * 20.0), Vec2::new(w, 16.0))
}

/// What the tool does and how it's used (the tools' column, under them).
fn tool_help(tool: Tool) -> &'static str {
    match tool {
        Tool::Select => "PICK SOMETHING TO CHANGE OR REMOVE. CLICK A WALL, A FLOOR, A LADDER OR A STAIR. A PICKED WALL SHOWS ITS POINTS (SQUARES: DRAG TO MOVE) AND THE MIDDLE OF EACH SEGMENT (RINGS: DRAG SIDEWAYS TO BEND IT INTO AN ARC). A PICKED FLOOR SHOWS ITS CORNERS. R MAKES A PICKED WALL A RAILING (1.1 M HIGH) OR BACK. DEL REMOVES WHAT'S PICKED.",
        Tool::Plane => "A FLOOR ON THIS DECK. CLICK ITS CORNERS ONE BY ONE; CLICK THE FIRST AGAIN, OR ENTER, TO CLOSE IT. IT'S TRIMMED TO THE HULL: DRAW IT LARGE AND ONLY WHAT'S INSIDE IS FLOOR. BACKSPACE TAKES THE LAST CORNER BACK. FILL: A FLOOR OVER THE WHOLE DECK AT ONCE, FOLLOWING THE HULL (ONE FOR EACH PART OF IT AT THIS HEIGHT).",
        Tool::Wall => "A WALL ON THIS DECK, AS TALL AS THE DECK. CLICK ITS POINTS ONE BY ONE; ENTER ENDS IT. IT STOPS WHERE IT MEETS THE HULL (BEYOND, FAINT RED). TO CURVE A SEGMENT, PICK THE WALL WITH SELECT AND DRAG THE RING AT ITS MIDDLE. BACKSPACE TAKES THE LAST POINT BACK.",
        Tool::Door => "A DOORWAY IN A WALL, 0.9 M WIDE AND 2.1 M TALL. CLICK ON A WALL WHERE IT GOES; CLICK AN EXISTING DOOR TO REMOVE IT.",
        Tool::Ladder => "A LADDER UP TO THE DECK ABOVE, THROUGH A 0.9 M HATCH CUT IN ITS FLOOR. CLICK WHERE IT STANDS. ABOARD: WALK INTO IT, W CLIMBS (LOOK DOWN TO CLIMB DOWN). NEEDS A DECK ABOVE.",
        Tool::Stair => "A STAIR UP TO THE DECK ABOVE, 1 M WIDE, ITS OPENING CUT IN THAT DECK'S FLOOR. CLICK ITS FOOT, THEN ITS HEAD: THE LONGER THE RUN, THE GENTLER. ABOARD: WALK UP IT. NEEDS A DECK ABOVE.",
    }
}

/// Where the tool is at now, if it's partway through something.
fn tool_state(studio: &Studio) -> Option<String> {
    if studio.walk_armed {
        return Some("WALK HERE: CLICK A SPOT ON THE PLAN".into());
    }
    let n = studio.drawing.len();
    match studio.tool {
        Tool::Plane if n > 0 => Some(format!("{n} CORNER{} PLACED: {}", if n == 1 { "" } else { "S" }, if n >= 3 { "CLICK THE FIRST AGAIN OR ENTER TO CLOSE" } else { "CLICK THE NEXT" })),
        Tool::Wall if n > 0 => Some(format!("{n} POINT{} PLACED: {}", if n == 1 { "" } else { "S" }, if n >= 2 { "CLICK ON, OR ENTER TO END" } else { "CLICK THE NEXT" })),
        Tool::Stair if n > 0 => Some("FOOT PLACED: CLICK ITS HEAD".into()),
        Tool::Select => studio.pick.map(|p| match p {
            Pick::Wall(_) => "A WALL PICKED: DRAG A SQUARE OR A RING, OR DEL".into(),
            Pick::Plane(_) => "A FLOOR PICKED: DRAG A CORNER, OR DEL".into(),
            Pick::Ladder(_) => "A LADDER PICKED: DEL REMOVES IT".into(),
            Pick::Stair(_) => "A STAIR PICKED: DEL REMOVES IT".into(),
        }),
        _ => None,
    }
}

/// The toolbar button under `q`, if any.
fn button_at(size: Vec2, q: Vec2) -> Option<Action> {
    (0..TOOLBAR.len()).find(|&k| {
        let (p, c) = button(size, k);
        q.x >= p.x && q.x <= p.x + c.x && q.y >= p.y && q.y <= p.y + c.y
    }).map(|k| TOOLBAR[k].2)
}

/// Is there room for a deck between these sides: at least 3 m across, for at
/// least 3 m of the ship's length (not just its masts and fittings)?
fn roomy(sides: &Sides) -> bool {
    sides.rows().filter(|(_, a, b)| b - a >= 3.0).count() as f64 * 0.1 >= 3.0
}

/// Is `q` in region `r`?
fn in_rect(r: (Vec2, Vec2), q: Vec2) -> bool {
    q.x >= r.0.x && q.x <= r.1.x && q.y >= r.0.y && q.y <= r.1.y
}

/// Which way a hull's nose is along z: where its cockpit is (+z if it has none).
fn nose_of(shape: &universe_sim::world::shape::Shape) -> f64 {
    shape.nodes.iter().find(|n| n.role == universe_sim::world::shape::Role::Cockpit).map_or(1.0, |n| if n.at.z < 0.0 { -1.0 } else { 1.0 })
}

/// The strip under the plan, shared: the side view at the left, the end view at
/// the right.
fn split_views(r: (Vec2, Vec2)) -> ((Vec2, Vec2), (Vec2, Vec2)) {
    let split = r.0.x + (r.1.x - r.0.x) * 0.7;
    ((r.0, Vec2::new(split - 5.0, r.1.y)), (Vec2::new(split + 5.0, r.0.y), r.1))
}

/// The side and end views' scale (pixels a metre, one for both, so their decks
/// line up), for the hull in the strip `r`.
fn views_scale(h: &Hull, r: (Vec2, Vec2)) -> f64 {
    let (side, end) = split_views(r);
    let (ws, we, ht) = ((side.1.x - side.0.x) as f64, (end.1.x - end.0.x) as f64, (r.1.y - r.0.y) as f64);
    (ws / (h.hi.z - h.lo.z)).min(we / (h.hi.x - h.lo.x)).min(ht / (h.hi.y - h.lo.y)) * 0.8
}

/// The side view's scale and middle (moved by `pan`), for the hull in the strip `r`.
fn side_view(h: &Hull, r: (Vec2, Vec2), pan: Vec2) -> (f64, Vec2) {
    let side = split_views(r).0;
    (views_scale(h, r), (side.0 + side.1) * 0.5 + pan)
}

/// The end view's scale and middle (moved by `pan`).
fn end_view(h: &Hull, r: (Vec2, Vec2), pan: Vec2) -> (f64, Vec2) {
    let end = split_views(r).1;
    (views_scale(h, r), (end.0 + end.1) * 0.5 + pan)
}

/// The height (hull frame) at screen row `y` of the side view.
fn side_y(h: &Hull, r: (Vec2, Vec2), pan: Vec2, y: f32) -> f64 {
    let (k, mid) = side_view(h, r, pan);
    (h.lo.y + h.hi.y) / 2.0 - (y - mid.y) as f64 / k
}

/// The 3D studio's tunnels and points in a side or end view (`at`: where a point
/// is on screen; `k`: pixels a metre): each tunnel's line and the band of its room
/// round it, each point.
fn access_side(frame: &mut Frame, access: &crate::interior::Access, at: impl Fn(universe_engine::glam::Vec3) -> Vec2, k: f32) {
    for &(a, b, room, walled) in &access.tunnels {
        let (pa, pb) = (at(a), at(b));
        frame.hud_line(pa, pb, TUNNEL);
        if let Some((w, h)) = room {
            let along = (pb - pa).normalize_or_zero();
            // (Across the line on screen: its height if it runs level, its width if
            // it climbs.)
            let level = (b - a).y.abs() < universe_engine::glam::Vec2::new((b - a).x, (b - a).z).length();
            let side = Vec2::new(-along.y, along.x) * if level { h } else { w } * 0.5 * k;
            let edge = Color([TUNNEL.0[0], TUNNEL.0[1], TUNNEL.0[2], if walled { 0.8 } else { 0.4 }]);
            frame.hud_line(pa + side, pb + side, edge);
            frame.hud_line(pa - side, pb - side, edge);
        }
    }
    for &(p, c) in &access.points {
        frame.hud_rect(at(p) - Vec2::splat(2.0), Vec2::splat(4.0), c);
    }
}

/// A dimension: a line from `a` to `b` with ticks across its ends, and its
/// measure beside its middle (over it if it runs across, right of it if up).
fn dimension(frame: &mut Frame, a: Vec2, b: Vec2, metres: f64) {
    let along = (b - a).normalize_or_zero();
    let across = Vec2::new(-along.y, along.x) * 4.0;
    frame.hud_line(a, b, LABEL.scale(0.8));
    for p in [a, b] {
        frame.hud_line(p - across, p + across, LABEL.scale(0.8));
    }
    let text = format!("{metres:.1} M");
    let w = text.chars().count() as f32 * universe_engine::frame::GLYPH * 0.7;
    let mid = (a + b) * 0.5;
    let at = if along.x.abs() > along.y.abs() { mid + Vec2::new(-w / 2.0, -12.0) } else { mid + Vec2::new(6.0, -4.0) };
    frame.text_scaled(at, &text, LABEL, 0.7);
}

/// A view's grid: a metre (every fifth brighter) across `r`, `to_u`/`to_v` the
/// screen x of an across measure and the screen y of a height over the keel.
fn view_grid(frame: &mut Frame, r: (Vec2, Vec2), k: f64, to_u: impl Fn(f64) -> f32, to_v: impl Fn(f64) -> f32, u: (f64, f64), v: (f64, f64)) {
    if k < 4.0 {
        return;
    }
    for i in (u.0.floor() as i64)..=(u.1.ceil() as i64) {
        let x = to_u(i as f64);
        if x >= r.0.x && x <= r.1.x {
            frame.hud_line(Vec2::new(x, r.0.y), Vec2::new(x, r.1.y), if i % 5 == 0 { GRID5 } else { GRID });
        }
    }
    for i in (v.0.floor() as i64)..=(v.1.ceil() as i64) {
        let y = to_v(i as f64);
        if y >= r.0.y && y <= r.1.y {
            frame.hud_line(Vec2::new(r.0.x, y), Vec2::new(r.1.x, y), if i % 5 == 0 { GRID5 } else { GRID });
        }
    }
}

/// A view's FLIP button: at its top right.
fn flip_button(r: (Vec2, Vec2)) -> (Vec2, Vec2) {
    (Vec2::new(r.1.x - 42.0, r.0.y + 4.0), Vec2::new(r.1.x - 4.0, r.0.y + 18.0))
}

/// Snapped to a quarter metre (ALT: as it is).
fn snap(p: DVec2, free: bool) -> DVec2 {
    if free { p } else { (p * 4.0).round() / 4.0 }
}

/// The studio's input (the shipyard's layout page). False: ESC with nothing to stop (leave).
pub fn input(app: &mut App, ctx: &Context, hull_key: &str, shape: &universe_sim::world::shape::Shape, studio: &mut Studio) -> bool {
    let input = &ctx.input;
    let size = ctx.hud_size.as_vec2();
    let (plan_r, low_r) = regions(size, panel_open(studio));
    let (side_r, end_r) = split_views(low_r);
    studio.spin += ctx.dt;
    studio.refresh(app, hull_key, shape);
    let Some(h) = studio.hull.as_ref() else {
        return !input.pressed(KeyCode::Escape);
    };
    // A toolbar button clicked: as its key.
    let clicked = if input.button_pressed(MouseButton::Left) { button_at(size, input.cursor) } else { None };
    // Fit the view to the hull.
    if input.pressed(KeyCode::Home) || clicked == Some(Action::Fit) {
        studio.side_pan = Vec2::ZERO;
        studio.end_pan = Vec2::ZERO;
    }
    if studio.view.is_none() || input.pressed(KeyCode::Home) || clicked == Some(Action::Fit) {
        let span = (plan_r.1 - plan_r.0).as_dvec2();
        let s = (span.x / (h.hi.z - h.lo.z)).min(span.y / (h.hi.x - h.lo.x)) * 0.92;
        studio.view = Some((DVec2::new((h.lo.x + h.hi.x) / 2.0, (h.lo.z + h.hi.z) / 2.0), s));
    }
    let shift = input.down(KeyCode::ShiftLeft) || input.down(KeyCode::ShiftRight);
    let ctrl = input.down(KeyCode::ControlLeft) || input.down(KeyCode::ControlRight);
    let alt = input.down(KeyCode::AltLeft) || input.down(KeyCode::AltRight);
    let cursor = input.cursor;
    studio.cursor = cursor;
    let over = cursor.x >= plan_r.0.x && cursor.x <= plan_r.1.x && cursor.y >= plan_r.0.y && cursor.y <= plan_r.1.y;
    let at = studio.to_plan(plan_r, cursor);
    let px = studio.view.map_or(10.0, |v| v.1);
    if clicked == Some(Action::Close) {
        return false;
    }
    // ESC: stop drawing, drop the pick, else leave.
    if input.pressed(KeyCode::Escape) {
        if !studio.drawing.is_empty() || studio.pick.is_some() {
            studio.drawing.clear();
            studio.pick = None;
            return true;
        }
        return false;
    }
    // Tools.
    for (k, t) in [(KeyCode::KeyS, Tool::Select), (KeyCode::KeyP, Tool::Plane), (KeyCode::KeyW, Tool::Wall), (KeyCode::KeyD, Tool::Door), (KeyCode::KeyL, Tool::Ladder), (KeyCode::KeyT, Tool::Stair)] {
        if input.pressed(k) || clicked == Some(Action::Tool(t)) {
            studio.tool = t;
            studio.drawing.clear();
        }
    }
    // Decks.
    let first_floor = h.first_floor;
    let plan = Studio::plan(app, hull_key);
    let n = plan.decks.len();
    // Another deck: on the first floor, or above the top one with its headroom
    // and a deck's thickness between (not past the hull's top).
    let mut went: Option<usize> = None;
    if (shift && input.pressed(KeyCode::KeyN)) || clicked == Some(Action::AddDeck) {
        let floor = if n == 0 { first_floor } else { plan.decks.iter().map(|d| d.floor + d.headroom + deckplan::DECK).fold(first_floor, f64::max) };
        // (Only where there's hull round it, at a person's waist and under its ceiling.)
        let room = |y: f64| shape.walk.as_ref().is_some_and(|m| roomy(&Sides::of(&m.section_y(y))));
        if room(floor + 1.0) && room(floor + deckplan::HEADROOM - 0.2) {
            plan.decks.push(Deck::at(floor));
            plan.decks.sort_by(|a, b| a.floor.total_cmp(&b.floor));
            went = plan.decks.iter().position(|d| (d.floor - floor).abs() < 1e-9);
        }
    } else if n > 0 {
        // The next deck, up or down: round from one end to the other.
        if (input.pressed(KeyCode::KeyN) && !shift) || clicked == Some(Action::NextDeck) || input.pressed(KeyCode::PageUp) {
            went = Some((studio.deck + 1) % n);
        } else if input.pressed(KeyCode::PageDown) {
            went = Some((studio.deck + n - 1) % n);
        } else if input.button_pressed(MouseButton::Left) && in_rect(side_r, cursor) && !in_rect(flip_button(side_r), cursor) {
            // A deck clicked in the side view: its floor or its headroom; on its floor's
            // line (within a few pixels), grabbed to drag up or down.
            let y = side_y(h, low_r, studio.side_pan, cursor.y);
            let near = 6.0 / side_view(h, low_r, studio.side_pan).0;
            if let Some(k) = plan.decks.iter().position(|d| (y - d.floor).abs() <= near) {
                went = Some(k);
                studio.drag = Some(Drag::Floor(k));
            } else {
                went = plan.decks.iter().position(|d| y >= d.floor - 0.4 && y <= d.floor + d.headroom).or(went);
            }
        }
    }
    if let Some(k) = went {
        studio.deck = k;
        studio.drawing.clear();
        studio.pick = None;
    }
    if let Some(Drag::Floor(_)) = studio.drag
        && !input.button_down(MouseButton::Left)
    {
        studio.drag = None;
    }
    let step: f64 = if shift { 0.5 } else { 0.1 };
    let k = studio.deck;
    if k < plan.decks.len() {
        // The floor moved: the decks above with it (the stack keeps together); not
        // down into the deck below (its height and a deck's thickness kept).
        // The steps that repeat while held (key or button): once at the press, then
        // after a pause, over and over.
        let steps = [
            ([KeyCode::Equal, KeyCode::NumpadAdd], Action::Floor(0.1)),
            ([KeyCode::Minus, KeyCode::NumpadSubtract], Action::Floor(-0.1)),
            ([KeyCode::BracketRight, KeyCode::BracketRight], Action::Headroom(0.1)),
            ([KeyCode::BracketLeft, KeyCode::BracketLeft], Action::Headroom(-0.1)),
        ];
        let mut fire = [false; 4];
        for (n, (keys, action)) in steps.iter().enumerate() {
            let start = keys.iter().any(|k| input.pressed(*k)) || clicked == Some(*action);
            let down = keys.iter().any(|k| input.down(*k)) || (input.button_down(MouseButton::Left) && button_at(size, cursor) == Some(*action));
            if start {
                studio.held = Some((n, 0.0, 0.4));
                fire[n] = true;
            } else if let Some((h, t, next)) = studio.held.as_mut()
                && *h == n
            {
                if !down {
                    studio.held = None;
                } else {
                    *t += ctx.dt;
                    if *t >= *next {
                        *next += 0.06;
                        fire[n] = true;
                    }
                }
            }
        }
        let mut lift = 0.0;
        // Dragged in the side view: to the cursor's height (5 cm steps).
        if studio.drag == Some(Drag::Floor(k)) {
            let to = (side_y(h, low_r, studio.side_pan, cursor.y) / 0.05).round() * 0.05;
            let lowest = k.checked_sub(1).map_or(f64::MIN, |b| plan.decks[b].floor + plan.decks[b].headroom + deckplan::DECK);
            lift = to.max(lowest) - plan.decks[k].floor;
        }
        if fire[0] {
            lift = step;
        }
        if fire[1] {
            let lowest = k.checked_sub(1).map_or(f64::MIN, |b| plan.decks[b].floor + plan.decks[b].headroom + deckplan::DECK);
            lift = -step.min(plan.decks[k].floor - lowest).max(0.0);
        }
        // Its height changed: the decks above moved by as much.
        let mut taller = 0.0;
        if fire[2] {
            taller = step;
        }
        if fire[3] {
            taller = -step.min(plan.decks[k].headroom - 1.0).max(0.0);
        }
        plan.decks[k].headroom += taller;
        for (j, d) in plan.decks.iter_mut().enumerate().skip(k) {
            d.floor += lift + if j > k { taller } else { 0.0 };
        }
    }
    if ((ctrl && input.pressed(KeyCode::Delete)) || clicked == Some(Action::DropDeck)) && studio.deck < plan.decks.len() {
        plan.decks.remove(studio.deck);
        studio.deck = studio.deck.min(plan.decks.len().saturating_sub(1));
        studio.pick = None;
        return true;
    }
    // Zoom (about the cursor) and pan.
    if over && input.scroll != 0.0
        && let Some((c, s)) = studio.view
    {
        let k = 1.15f64.powf(input.scroll as f64);
        let ns = (s * k).clamp(2.0, 400.0);
        // (The point under the cursor stays under it.)
        studio.view = Some((at + (c - at) * (s / ns), ns));
    }
    if over && (input.button_pressed(MouseButton::Right) || input.button_pressed(MouseButton::Middle)) {
        studio.drag = Some(Drag::Pan);
    }
    if studio.drag == Some(Drag::Pan) {
        if input.button_down(MouseButton::Right) || input.button_down(MouseButton::Middle) {
            if let Some((c, s)) = studio.view {
                let d = input.mouse_delta.as_dvec2();
                studio.view = Some((c + DVec2::new(d.y, -d.x) / s, s));
            }
        } else {
            studio.drag = None;
        }
    }
    // The side and end views: dragged with the right (or middle) button; FLIP
    // shows the other side, the other end.
    let grab = input.button_pressed(MouseButton::Right) || input.button_pressed(MouseButton::Middle);
    let held = input.button_down(MouseButton::Right) || input.button_down(MouseButton::Middle);
    if grab && in_rect(side_r, cursor) {
        studio.drag = Some(Drag::View(0));
    } else if grab && in_rect(end_r, cursor) {
        studio.drag = Some(Drag::View(1));
    }
    if let Some(Drag::View(w)) = studio.drag {
        if held {
            *(if w == 0 { &mut studio.side_pan } else { &mut studio.end_pan }) += input.mouse_delta;
        } else {
            studio.drag = None;
        }
    }
    if input.button_pressed(MouseButton::Left) {
        let flip = |r: (Vec2, Vec2)| { let b = flip_button(r); in_rect(b, cursor) };
        if flip(side_r) {
            studio.side_flip = !studio.side_flip;
            return true;
        }
        if flip(end_r) {
            studio.end_flip = !studio.end_flip;
            return true;
        }
    }
    let Some(deck) = plan.decks.get_mut(studio.deck) else { return true };
    // A walk-through: F over the plan, there; WALK HERE (or F off it), then a click on it.
    let walk_key = input.pressed(KeyCode::KeyF) || clicked == Some(Action::Walk);
    if walk_key && over {
        studio.walk = Some((universe_engine::glam::DVec3::new(at.x, deck.floor + 0.05, at.y), 0.0));
        return true;
    }
    if walk_key {
        studio.walk_armed = true;
        return true;
    }
    if studio.walk_armed && input.button_pressed(MouseButton::Left) {
        studio.walk_armed = false;
        if over {
            studio.walk = Some((universe_engine::glam::DVec3::new(at.x, deck.floor + 0.05, at.y), 0.0));
            return true;
        }
    }
    let finish = input.pressed(KeyCode::Enter) || clicked == Some(Action::Finish);
    let remove = (input.pressed(KeyCode::Delete) && !ctrl) || clicked == Some(Action::Remove);
    let p = snap(at, alt);
    let near = |q: DVec2| q.distance(at) * px < 8.0;
    match studio.tool {
        Tool::Plane | Tool::Wall => {
            if over && input.button_pressed(MouseButton::Left) {
                // (A plane closes on its first corner again.)
                if studio.tool == Tool::Plane && studio.drawing.len() >= 3 && near(studio.drawing[0]) {
                    deck.planes.push(std::mem::take(&mut studio.drawing));
                } else {
                    studio.drawing.push(p);
                }
            }
            if input.pressed(KeyCode::Backspace) {
                studio.drawing.pop();
            }
            if finish {
                if studio.tool == Tool::Plane && studio.drawing.len() >= 3 {
                    deck.planes.push(std::mem::take(&mut studio.drawing));
                } else if studio.tool == Tool::Wall && studio.drawing.len() >= 2 {
                    let points = std::mem::take(&mut studio.drawing);
                    let bulges = vec![0.0; points.len() - 1];
                    deck.walls.push(Wall { points, bulges, doors: Vec::new(), rail: false });
                }
            }
        }
        Tool::Ladder => {
            if over && input.button_pressed(MouseButton::Left) {
                deck.ladders.push(deckplan::Ladder { at: p });
            }
        }
        Tool::Stair => {
            if over && input.button_pressed(MouseButton::Left) {
                // Its foot, then its head.
                if let Some(&from) = studio.drawing.first() {
                    if from.distance(p) >= 0.5 {
                        deck.stairs.push(deckplan::Stair { from, to: p, width: deckplan::STAIR_WIDTH });
                    }
                    studio.drawing.clear();
                } else {
                    studio.drawing.push(p);
                }
            }
        }
        Tool::Door => {
            if over && input.button_pressed(MouseButton::Left) {
                // On the wall nearest the click (within a few pixels): a door there, or the one there removed.
                let hit = deck.walls.iter().enumerate().filter_map(|(k, w)| w.nearest(at).map(|(along, off)| (k, along, off))).filter(|h| h.2 * px < 10.0).min_by(|a, b| a.2.total_cmp(&b.2));
                if let Some((k, along, _)) = hit {
                    let w = &mut deck.walls[k];
                    if let Some(d) = w.doors.iter().position(|d| (d.at - along).abs() <= d.width / 2.0) {
                        w.doors.remove(d);
                    } else {
                        let len = w.length();
                        let half = deckplan::DOOR_WIDTH / 2.0;
                        w.doors.push(Door { at: along.clamp(half.min(len / 2.0), (len - half).max(len / 2.0)), width: deckplan::DOOR_WIDTH, height: deckplan::DOOR_HEIGHT });
                    }
                }
            }
        }
        Tool::Select => {
            if over && input.button_pressed(MouseButton::Left) {
                // A handle of what's picked first (its points, its segments' middles), else pick anew.
                let mut grabbed = None;
                if let Some(Pick::Wall(k)) = studio.pick
                    && let Some(w) = deck.walls.get(k)
                {
                    if let Some(i) = w.points.iter().position(|q| near(*q)) {
                        grabbed = Some(Drag::Point(Pick::Wall(k), i));
                    } else if let Some(i) = (0..w.points.len().saturating_sub(1)).find(|&i| near(bend_handle(w, i))) {
                        grabbed = Some(Drag::Bend(k, i));
                    }
                }
                if let Some(Pick::Plane(k)) = studio.pick
                    && let Some(poly) = deck.planes.get(k)
                    && let Some(i) = poly.iter().position(|q| near(*q))
                {
                    grabbed = Some(Drag::Point(Pick::Plane(k), i));
                }
                if grabbed.is_none() {
                    let wall = deck.walls.iter().enumerate().filter_map(|(k, w)| w.nearest(at).map(|(_, off)| (k, off))).filter(|h| h.1 * px < 8.0).min_by(|a, b| a.1.total_cmp(&b.1)).map(|h| Pick::Wall(h.0));
                    let plane = || deck.planes.iter().position(|poly| inside(poly, at)).map(Pick::Plane);
                    let ladder = || deck.ladders.iter().position(|l| inside(&l.outline(), at)).map(Pick::Ladder);
                    let stair = || deck.stairs.iter().position(|st| inside(&st.outline(), at)).map(Pick::Stair);
                    studio.pick = ladder().or_else(stair).or(wall).or_else(plane);
                }
                studio.drag = grabbed;
            }
            match studio.drag {
                Some(Drag::Point(pick, i)) if input.button_down(MouseButton::Left) => match pick {
                    Pick::Wall(k) => {
                        if let Some(q) = deck.walls.get_mut(k).and_then(|w| w.points.get_mut(i)) {
                            *q = p;
                        }
                    }
                    Pick::Plane(k) => {
                        if let Some(q) = deck.planes.get_mut(k).and_then(|poly| poly.get_mut(i)) {
                            *q = p;
                        }
                    }
                    // (Ladders and stairs: placed anew, not dragged.)
                    Pick::Ladder(_) | Pick::Stair(_) => {}
                },
                Some(Drag::Bend(k, i)) if input.button_down(MouseButton::Left) => {
                    // The arc's middle under the cursor: its bulge, how far that is off the chord.
                    if let Some(w) = deck.walls.get_mut(k) {
                        let (a, b) = (w.points[i], w.points[i + 1]);
                        let chord = (b - a).normalize_or_zero();
                        let left = DVec2::new(-chord.y, chord.x);
                        let bulge = (at - (a + b) * 0.5).dot(left);
                        w.bulges.resize(w.points.len() - 1, 0.0);
                        w.bulges[i] = if bulge.abs() < 0.05 { 0.0 } else { bulge };
                    }
                }
                Some(Drag::Point(..) | Drag::Bend(..)) => studio.drag = None,
                _ => {}
            }
        }
    }
    // The floors' panel's FILL: floors over the whole deck, now.
    if auto_shown(studio) && input.button_pressed(MouseButton::Left) && in_rect({ let (p, c) = auto_button(studio); (p, p + c) }, cursor) {
        if let Some(mesh) = shape.walk.as_ref() {
            deck.planes.extend(deckplan::fill(mesh, deck.floor));
            studio.pick = deck.planes.len().checked_sub(1).map(Pick::Plane);
        }
        return true;
    }
    // The panel's list: a row clicked picks that thing (REMOVE then takes it out).
    if panel_open(studio) && input.button_pressed(MouseButton::Left) && cursor.x >= 12.0 && cursor.x <= 12.0 + PANEL_WIDTH {
        let top = panel_list_top(studio);
        let list = panel_list(studio, deck, None, &[]);
        let row = ((cursor.y - top) / ROW).floor();
        if row >= 0.0
            && let Some((pick, _)) = list.get(row as usize)
        {
            studio.pick = Some(*pick);
        }
    }
    // R: a picked wall made a railing (RAIL high, not to the ceiling), or back.
    if input.pressed(KeyCode::KeyR)
        && let Some(Pick::Wall(k)) = studio.pick
        && let Some(w) = deck.walls.get_mut(k)
    {
        w.rail = !w.rail;
    }
    // What's picked removed (with any tool).
    if remove {
        match studio.pick.take() {
            Some(Pick::Wall(k)) if k < deck.walls.len() => {
                deck.walls.remove(k);
            }
            Some(Pick::Plane(k)) if k < deck.planes.len() => {
                deck.planes.remove(k);
            }
            Some(Pick::Ladder(k)) if k < deck.ladders.len() => {
                deck.ladders.remove(k);
            }
            Some(Pick::Stair(k)) if k < deck.stairs.len() => {
                deck.stairs.remove(k);
            }
            _ => {}
        }
    }
    true
}

/// Where segment `i` of wall `w` is bent from: the middle of its arc.
fn bend_handle(w: &Wall, i: usize) -> DVec2 {
    let (a, b) = (w.points[i], w.points[i + 1]);
    let chord = (b - a).normalize_or_zero();
    (a + b) * 0.5 + DVec2::new(-chord.y, chord.x) * w.bulge(i)
}

/// Is `p` inside outline `poly`?
fn inside(poly: &[DVec2], p: DVec2) -> bool {
    let mut odd = false;
    for i in 0..poly.len() {
        let (a, b) = (poly[i], poly[(i + 1) % poly.len()]);
        if (a.y > p.y) != (b.y > p.y) && p.x < a.x + (b.x - a.x) * (p.y - a.y) / (b.y - a.y) {
            odd = !odd;
        }
    }
    odd
}

/// The studio drawn (the shipyard's layout page).
pub fn draw(frame: &mut Frame, app: &App, place: &str, hull_key: &str, hull_name: &str, studio: &Studio, access: &crate::interior::Access) {
    let size = frame.size();
    let (plan_r, low_r) = regions(size, panel_open(studio));
    let (side_r, end_r) = split_views(low_r);
    frame.text(Vec2::new(12.0, 10.0), &format!("{place}   LAYOUT STUDIO - {hull_name}   {:.0} CR", app.v.credits), LABEL);
    // The toolbar: the tool in use lit, the button under the cursor brighter.
    {
        use crate::hud::{draw_cell, Lamp};
        for (k, (key, name, action)) in TOOLBAR.iter().enumerate() {
            let (p, c) = button(size, k);
            let hover = button_at(size, studio.cursor) == Some(*action);
            let lamp = if *action == Action::Tool(studio.tool) || hover { Lamp::On } else { Lamp::Off };
            draw_cell(frame, p, c, key, name, lamp);
        }
    }
    for r in [plan_r, side_r, end_r] {
        frame.hud_rect(r.0, r.1 - r.0, PAPER);
    }
    let Some(h) = studio.hull.as_ref() else {
        frame.text(plan_r.0 + Vec2::new(16.0, 16.0), &format!("{hull_name} HAS NO MODEL TO LAY OUT: THE STUDIO NEEDS A MODELLED HULL"), LABEL);
        return;
    };
    let plan = Studio::plan_of(app, hull_key);
    let decks: &[Deck] = plan.map_or(&[], |p| &p.decks);
    let deck = decks.get(studio.deck);
    let px = studio.view.map_or(10.0, |v| v.1);
    let to = |p: DVec2| studio.to_screen(plan_r, p);
    frame.hud_clipped(plan_r.0, plan_r.1, |frame| {
        // The grid: a metre, every fifth brighter.
        let (lo, hi) = (studio.to_plan(plan_r, Vec2::new(plan_r.0.x, plan_r.1.y)), studio.to_plan(plan_r, Vec2::new(plan_r.1.x, plan_r.0.y)));
        if px > 4.0 {
            for z in (lo.y.floor() as i64)..=(hi.y.ceil() as i64) {
                let x = to(DVec2::new(0.0, z as f64)).x;
                frame.hud_line(Vec2::new(x, plan_r.0.y), Vec2::new(x, plan_r.1.y), if z % 5 == 0 { GRID5 } else { GRID });
            }
            for xx in (lo.x.floor() as i64)..=(hi.x.ceil() as i64) {
                let y = to(DVec2::new(xx as f64, 0.0)).y;
                frame.hud_line(Vec2::new(plan_r.0.x, y), Vec2::new(plan_r.1.x, y), if xx % 5 == 0 { GRID5 } else { GRID });
            }
        }
        // The hull, cut at this deck.
        for [a, b] in &h.section {
            frame.hud_line_smooth(to(*a), to(*b), HULL);
        }
        // The 3D studio's access plan, seen from above: each tunnel's line and its
        // sides (its width), each point; bright where it passes this deck, faint
        // elsewhere.
        if let Some(d) = deck {
            let (y0, y1) = (d.floor as f32, (d.floor + d.headroom) as f32);
            for &(a, b, room, walled) in &access.tunnels {
                let half = room.map_or(0.0, |r| r.1 * 0.5);
                let here = a.y.min(b.y) - half <= y1 && a.y.max(b.y) + half >= y0;
                let col = Color([TUNNEL.0[0], TUNNEL.0[1], TUNNEL.0[2], if here { 0.95 } else { 0.25 }]);
                let flat = |p: universe_engine::glam::Vec3| to(DVec2::new(f64::from(p.x), f64::from(p.z)));
                let (pa, pb) = (flat(a), flat(b));
                frame.hud_line(pa, pb, col);
                if let Some((w, _)) = room {
                    let along = DVec2::new(f64::from(b.x - a.x), f64::from(b.z - a.z));
                    if along.length() > 0.2 {
                        let side = DVec2::new(-along.y, along.x).normalize() * f64::from(w) * 0.5;
                        let (a2, b2) = (DVec2::new(f64::from(a.x), f64::from(a.z)), DVec2::new(f64::from(b.x), f64::from(b.z)));
                        let edge = Color([col.0[0], col.0[1], col.0[2], col.0[3] * if walled { 0.9 } else { 0.5 }]);
                        frame.hud_line(to(a2 + side), to(b2 + side), edge);
                        frame.hud_line(to(a2 - side), to(b2 - side), edge);
                    } else {
                        // (Upright: a shaft, its square seen from above.)
                        let r = f64::from(w) * 0.5;
                        let c = DVec2::new(f64::from(a.x), f64::from(a.z));
                        let q = [c + DVec2::new(-r, -r), c + DVec2::new(r, -r), c + DVec2::new(r, r), c + DVec2::new(-r, r)].map(to);
                        for k in 0..4 {
                            frame.hud_line(q[k], q[(k + 1) % 4], col);
                        }
                    }
                }
            }
            for &(p, c) in &access.points {
                let here = p.y >= y0 - 0.5 && p.y <= y1 + 0.5;
                let q = to(DVec2::new(f64::from(p.x), f64::from(p.z)));
                frame.hud_rect(q - Vec2::splat(2.5), Vec2::splat(5.0), Color([c.0[0], c.0[1], c.0[2], if here { 1.0 } else { 0.3 }]));
            }
        }
        // Its length along its foot, its beam beside it (the whole hull's).
        {
            let corners = [to(DVec2::new(h.lo.x, h.lo.z)), to(DVec2::new(h.hi.x, h.hi.z))];
            let (lo, hi) = (corners[0].min(corners[1]), corners[0].max(corners[1]));
            dimension(frame, Vec2::new(lo.x, hi.y - 6.0), Vec2::new(hi.x, hi.y - 6.0), h.hi.z - h.lo.z);
            dimension(frame, Vec2::new(hi.x + 12.0, hi.y), Vec2::new(hi.x + 12.0, lo.y), h.hi.x - h.lo.x);
        }
        let Some(deck) = deck else { return };
        // Floors: trimmed to the hull (filled), the openings from the deck below cut out, their outlines as drawn.
        let holes = plan.map(|p| deckplan::openings(p, studio.deck)).unwrap_or_default();
        for hole in &holes {
            for i in 0..hole.len() {
                dashed(frame, to(hole[i]), to(hole[(i + 1) % hole.len()]), LABEL);
            }
        }
        for (k, poly) in deck.planes.iter().enumerate() {
            for q in deckplan::floor_pieces(poly, &h.sides, &holes) {
                let q = q.map(to);
                frame.hud_triangle_colored([q[0], q[1], q[2]], [FLOOR; 3]);
                frame.hud_triangle_colored([q[0], q[2], q[3]], [FLOOR; 3]);
            }
            let col = if studio.pick == Some(Pick::Plane(k)) { PICKED } else { INK.scale(0.8) };
            for i in 0..poly.len() {
                frame.hud_line_smooth(to(poly[i]), to(poly[(i + 1) % poly.len()]), col);
            }
            if studio.pick == Some(Pick::Plane(k)) {
                for q in poly {
                    handle(frame, to(*q), PICKED, false);
                }
            }
        }
        // Walls: where they're inside the hull in ink (thick), beyond it faint red; doors as gaps with a swing.
        for (k, w) in deck.walls.iter().enumerate() {
            let picked = studio.pick == Some(Pick::Wall(k));
            let path = w.path();
            for s in path.windows(2) {
                frame.hud_line(to(s[0].0), to(s[1].0), OUT);
            }
            let col = if picked { PICKED } else { INK };
            let half = ((deckplan::WALL / 2.0) * px).max(1.0) as f32;
            for run in deckplan::wall_runs(w, &h.sides) {
                for s in run.windows(2) {
                    let ((a, sa), (b, sb)) = (s[0], s[1]);
                    if w.doors.iter().any(|d| ((sa + sb) / 2.0 - d.at).abs() <= d.width / 2.0) {
                        continue;
                    }
                    let (pa, pb) = (to(a), to(b));
                    let n = (pb - pa).perp().normalize_or_zero() * half;
                    frame.hud_line_smooth(pa + n, pb + n, col);
                    frame.hud_line_smooth(pa - n, pb - n, col);
                }
            }
            for d in &w.doors {
                if let (Some(a), Some(b)) = (w.point_at(d.at - d.width / 2.0), w.point_at(d.at + d.width / 2.0)) {
                    // (A door's leaf, open, and its swing.)
                    let (pa, pb) = (to(a), to(b));
                    let leaf = (pb - pa).perp();
                    frame.hud_line_smooth(pa, pa + leaf, LABEL);
                    let r = leaf.length();
                    let a0 = leaf.y.atan2(leaf.x);
                    let a1 = (pb - pa).y.atan2((pb - pa).x);
                    let mut da = a1 - a0;
                    if da > std::f32::consts::PI {
                        da -= std::f32::consts::TAU;
                    } else if da < -std::f32::consts::PI {
                        da += std::f32::consts::TAU;
                    }
                    for i in 0..8 {
                        let (t0, t1) = (a0 + da * i as f32 / 8.0, a0 + da * (i + 1) as f32 / 8.0);
                        frame.hud_line_smooth(pa + Vec2::from_angle(t0) * r, pa + Vec2::from_angle(t1) * r, LABEL.scale(0.6));
                    }
                }
            }
            if picked {
                for q in &w.points {
                    handle(frame, to(*q), PICKED, false);
                }
                for i in 0..w.points.len().saturating_sub(1) {
                    handle(frame, to(bend_handle(w, i)), PICKED, true);
                }
            }
        }
        // Ladders and stairs, up to the deck above (none there: in red, going nowhere).
        let up = decks.get(studio.deck + 1).is_some();
        for (k, l) in deck.ladders.iter().enumerate() {
            let col = if studio.pick == Some(Pick::Ladder(k)) { PICKED } else if up { INK } else { OUT };
            let o = l.outline();
            for i in 0..4 {
                frame.hud_line_smooth(to(o[i]), to(o[(i + 1) % 4]), col);
            }
            // (Its rungs.)
            for j in 1..4 {
                let f = j as f64 / 4.0;
                frame.hud_line(to(o[0].lerp(o[3], f)), to(o[1].lerp(o[2], f)), col.scale(0.7));
            }
            if !up {
                frame.text(to(o[2]) + Vec2::new(4.0, -6.0), "NO DECK ABOVE", OUT);
            }
        }
        for (k, st) in deck.stairs.iter().enumerate() {
            let col = if studio.pick == Some(Pick::Stair(k)) { PICKED } else if up { INK } else { OUT };
            let o = st.outline();
            for i in 0..4 {
                frame.hud_line_smooth(to(o[i]), to(o[(i + 1) % 4]), col);
            }
            // (Its treads, and an arrow up its run.)
            let n = ((st.to - st.from).length() / 0.28).round().max(2.0) as usize;
            for j in 1..n {
                let f = j as f64 / n as f64;
                frame.hud_line(to(o[0].lerp(o[1], f)), to(o[3].lerp(o[2], f)), col.scale(0.6));
            }
            let (a, b) = (to(st.from), to(st.to));
            frame.hud_line_smooth(a, b, col);
            let d = (b - a).normalize_or_zero() * 8.0;
            frame.hud_line_smooth(b, b - d + d.perp() * 0.6, col);
            frame.hud_line_smooth(b, b - d - d.perp() * 0.6, col);
            frame.text(a + Vec2::new(4.0, 4.0), if up { "UP" } else { "NO DECK ABOVE" }, col);
        }
        // What's being drawn, to the cursor.
        if !studio.drawing.is_empty() {
            let cursor = to(snap(studio.to_plan(plan_r, studio.cursor), false));
            let pts: Vec<Vec2> = studio.drawing.iter().map(|q| to(*q)).collect();
            for s in pts.windows(2) {
                frame.hud_line_smooth(s[0], s[1], PICKED);
            }
            frame.hud_line_smooth(*pts.last().expect("a point"), cursor, PICKED.scale(0.6));
            for q in &pts {
                handle(frame, *q, PICKED, false);
            }
        }
    });
    // The side view: the hull seen from a side (its edges, faint: doors, windows,
    // frames) and cut along its centre line, and the decks (this one bright), to
    // scale; the end view beside it, at the same scale.
    let elev = studio.elev.as_ref().filter(|(k, _)| k == hull_key).map(|(_, e)| e.clone());
    let (zc, xc, yc) = ((h.lo.z + h.hi.z) / 2.0, (h.lo.x + h.hi.x) / 2.0, (h.lo.y + h.hi.y) / 2.0);
    let (k, mid) = side_view(h, low_r, studio.side_pan);
    // (Seen from -x, the nose... to the right if it's +z; flipped, from +x.)
    let sdir = if studio.side_flip { -1.0 } else { 1.0 };
    let zs = |z: f64| mid.x + ((z - zc) * k * sdir) as f32;
    let sy = |y: f64| mid.y - ((y - yc) * k) as f32;
    const SCALE: f32 = 0.7;
    // While the elevations are worked out: a spinner, and what's happening.
    let spinner = |frame: &mut Frame, r: (Vec2, Vec2)| {
        let c = (r.0 + r.1) * 0.5;
        for i in 0..12 {
            let a = i as f32 / 12.0 * std::f32::consts::TAU + studio.spin * 6.0;
            let lit = ((i as f32 / 12.0 + studio.spin) % 1.0).max(0.15);
            let d = Vec2::new(a.cos(), a.sin());
            frame.hud_line(c + d * 9.0, c + d * 16.0, INK.scale(lit));
        }
        frame.text_scaled(c + Vec2::new(-60.0, 24.0), "DRAWING THE HULL", LABEL.scale(0.8), SCALE);
    };
    // (What the screen spans in metres, for the grids: across and up from the keel.)
    let span = |c: f32, m: f32| ((c - m) as f64 / k, 0.0);
    frame.hud_clipped(side_r.0, side_r.1, |frame| {
        {
            let (a, b) = (span(side_r.0.x, mid.x).0 * sdir + zc, span(side_r.1.x, mid.x).0 * sdir + zc);
            let top = yc + (mid.y - side_r.0.y) as f64 / k - h.keel;
            let bottom = yc - (side_r.1.y - mid.y) as f64 / k - h.keel;
            view_grid(frame, side_r, k, zs, |v| sy(v + h.keel), (a.min(b), a.max(b)), (bottom, top));
        }
        match &elev {
            Some(e) => {
                for [a, b] in &e.side[studio.side_flip as usize] {
                    frame.hud_line(Vec2::new(zs(a.x), sy(a.y)), Vec2::new(zs(b.x), sy(b.y)), HULL.scale(0.35));
                }
            }
            None => spinner(frame, side_r),
        }
        for [a, b] in &h.profile {
            frame.hud_line_smooth(Vec2::new(zs(a.x), sy(a.y)), Vec2::new(zs(b.x), sy(b.y)), HULL.scale(0.8));
        }
        // The 3D studio's access plan from the side.
        access_side(frame, access, |p| Vec2::new(zs(f64::from(p.z)), sy(f64::from(p.y))), k as f32);
        // Its length under it, its height beside it.
        let (z0, z1) = (zs(h.lo.z).min(zs(h.hi.z)), zs(h.lo.z).max(zs(h.hi.z)));
        dimension(frame, Vec2::new(z0, sy(h.lo.y) + 10.0), Vec2::new(z1, sy(h.lo.y) + 10.0), h.hi.z - h.lo.z);
        dimension(frame, Vec2::new(z1 + 12.0, sy(h.lo.y)), Vec2::new(z1 + 12.0, sy(h.hi.y)), h.hi.y - h.lo.y);
        // The hull's sheet: its measures and its mass.
        let sheet = format!("{}   L {:.1} M   B {:.1} M   H {:.1} M   DRY MASS {:.1} T", h.name, h.hi.z - h.lo.z, h.hi.x - h.lo.x, h.hi.y - h.lo.y, h.dry_mass / 1000.0);
        frame.text_scaled(Vec2::new(side_r.0.x + 6.0, side_r.0.y + 8.0), &sheet, INK.scale(0.8), SCALE);
        // The decks' lines, from just right of their labels (so they don't run through them).
        let label = |k: usize, d: &Deck| format!("DECK {}  {:.1} M UP  {:.1} M HIGH", k + 1, d.floor - h.keel, d.headroom);
        let labels_w = decks.iter().enumerate().map(|(k, d)| label(k, d).chars().count()).max().unwrap_or(0) as f32 * universe_engine::frame::GLYPH * SCALE;
        let from = side_r.0.x + 6.0 + labels_w + 20.0;
        for (k, d) in decks.iter().enumerate() {
            let col = if k == studio.deck { PICKED } else { INK.scale(0.6) };
            frame.hud_line(Vec2::new(from, sy(d.floor)), Vec2::new(side_r.1.x, sy(d.floor)), col);
            // (The deck's slab under its floor.)
            frame.hud_line(Vec2::new(from, sy(d.floor - deckplan::DECK)), Vec2::new(side_r.1.x, sy(d.floor - deckplan::DECK)), col.scale(0.5));
            frame.hud_line(Vec2::new(from, sy(d.floor + d.headroom)), Vec2::new(side_r.1.x, sy(d.floor + d.headroom)), col.scale(0.4));
        }
        // Their labels, small, each level with the middle of its deck; where decks are
        // closer than a line, pushed apart (top down) with a leader to the deck.
        let line = 18.0 * SCALE;
        let mut order: Vec<usize> = (0..decks.len()).collect();
        order.sort_by(|&a, &b| decks[b].floor.total_cmp(&decks[a].floor));
        let mut next_free = f32::MIN;
        for k in order {
            let d = &decks[k];
            let col = if k == studio.deck { PICKED } else { INK.scale(0.6) };
            let want = sy(d.floor + d.headroom / 2.0) - line / 2.0;
            let y = want.max(next_free);
            next_free = y + line;
            let text = label(k, d);
            let end = frame.text_scaled(Vec2::new(side_r.0.x + 6.0, y), &text, col, SCALE);
            if (y - want).abs() > 1.0 {
                let mid_y = y + line / 2.0;
                frame.hud_line(Vec2::new(end.x + 4.0, mid_y), Vec2::new(from, sy(d.floor + d.headroom / 2.0)), col.scale(0.7));
            }
        }
        // The height under the cursor: a guide across, and how far up it is (from the
        // keel), to line a deck up with a door or a window.
        if in_rect(side_r, studio.cursor) {
            let y = studio.cursor.y;
            let up = side_y(h, low_r, studio.side_pan, y) - h.keel;
            frame.hud_line(Vec2::new(from, y), Vec2::new(side_r.1.x, y), PICKED.scale(0.45));
            frame.text_scaled(Vec2::new(studio.cursor.x + 10.0, y - 13.0), &format!("{up:.2} M UP"), PICKED, SCALE);
        }
    });
    // The end view: from the nose (its right to the right) or, flipped, the tail.
    let (_, emid) = end_view(h, low_r, studio.end_pan);
    let edir = h.nose * if studio.end_flip { -1.0 } else { 1.0 };
    let xs = |x: f64| emid.x + ((x - xc) * k * edir) as f32;
    let ey = |y: f64| emid.y - ((y - yc) * k) as f32;
    frame.hud_clipped(end_r.0, end_r.1, |frame| {
        {
            let (a, b) = (span(end_r.0.x, emid.x).0 * edir + xc, span(end_r.1.x, emid.x).0 * edir + xc);
            let top = yc + (emid.y - end_r.0.y) as f64 / k - h.keel;
            let bottom = yc - (end_r.1.y - emid.y) as f64 / k - h.keel;
            view_grid(frame, end_r, k, xs, |v| ey(v + h.keel), (a.min(b), a.max(b)), (bottom, top));
            let (x0, x1) = (xs(h.lo.x).min(xs(h.hi.x)), xs(h.lo.x).max(xs(h.hi.x)));
            dimension(frame, Vec2::new(x0, ey(h.lo.y) + 10.0), Vec2::new(x1, ey(h.lo.y) + 10.0), h.hi.x - h.lo.x);
            dimension(frame, Vec2::new(x1 + 12.0, ey(h.lo.y)), Vec2::new(x1 + 12.0, ey(h.hi.y)), h.hi.y - h.lo.y);
        }
        match &elev {
            Some(e) => {
                for [a, b] in &e.end[studio.end_flip as usize] {
                    frame.hud_line(Vec2::new(xs(a.x), ey(a.y)), Vec2::new(xs(b.x), ey(b.y)), HULL.scale(0.5));
                }
            }
            None => spinner(frame, end_r),
        }
        access_side(frame, access, |p| Vec2::new(xs(f64::from(p.x)), ey(f64::from(p.y))), k as f32);
        for (k, d) in decks.iter().enumerate() {
            let col = if k == studio.deck { PICKED } else { INK.scale(0.6) };
            frame.hud_line(Vec2::new(end_r.0.x, ey(d.floor)), Vec2::new(end_r.1.x, ey(d.floor)), col);
            frame.hud_line(Vec2::new(end_r.0.x, ey(d.floor - deckplan::DECK)), Vec2::new(end_r.1.x, ey(d.floor - deckplan::DECK)), col.scale(0.5));
            frame.hud_line(Vec2::new(end_r.0.x, ey(d.floor + d.headroom)), Vec2::new(end_r.1.x, ey(d.floor + d.headroom)), col.scale(0.4));
        }
        if in_rect(end_r, studio.cursor) {
            let y = studio.cursor.y;
            let up = yc - (y - emid.y) as f64 / k - h.keel;
            frame.hud_line(Vec2::new(end_r.0.x, y), Vec2::new(end_r.1.x, y), PICKED.scale(0.45));
            frame.text_scaled(Vec2::new(studio.cursor.x + 10.0, y - 13.0), &format!("{up:.2} M UP"), PICKED, SCALE);
        }
    });
    // The views' FLIP buttons: which side, which end, is shown.
    {
        use crate::hud::{draw_cell, Lamp};
        // (Seen from -x with the nose at +z: its starboard side.)
        let starboard = (if studio.side_flip { 1.0 } else { -1.0 }) == -h.nose;
        let views = [(side_r, if starboard { "STARBOARD SIDE" } else { "PORT SIDE" }), (end_r, if studio.end_flip { "FROM THE TAIL" } else { "FROM THE NOSE" })];
        for (r, what) in views {
            let (p, q) = flip_button(r);
            let lamp = if in_rect((p, q), studio.cursor) { Lamp::On } else { Lamp::Off };
            draw_cell(frame, p, q - p, "", "FLIP", lamp);
            let w = what.chars().count() as f32 * universe_engine::frame::GLYPH * 0.7;
            frame.text_scaled(Vec2::new(p.x - w - 5.0, p.y + 3.0), what, LABEL.scale(0.8), 0.7);
        }
    }
    // The tool's panel: what its kind of thing is and how it's made, where it's at,
    // and the ones on this deck (a row clicked picks it).
    if panel_open(studio) {
        let bottom = plan_r.1.y;
        frame.hud_rect(Vec2::new(12.0, 86.0), Vec2::new(PANEL_WIDTH, bottom - 86.0), Color([0.03, 0.09, 0.17, 1.0]));
        frame.hud_box(Vec2::new(12.0, 86.0), Vec2::new(PANEL_WIDTH, bottom - 86.0), GRID5);
        let kind = panel_kind(studio);
        let name = match kind {
            Tool::Select => "SELECT",
            Tool::Plane => "FLOORS",
            Tool::Wall => "WALLS",
            Tool::Door => "DOORS",
            Tool::Ladder => "LADDERS",
            Tool::Stair => "STAIRS",
        };
        let x = 20.0;
        let mut y = 86.0 + 8.0;
        frame.text(Vec2::new(x, y), name, INK);
        y += 22.0;
        let width = (PANEL_WIDTH / 8.0) as usize - 2;
        if let Some(state) = tool_state(studio) {
            for line in crate::fmt::wrap(&state, width) {
                frame.text(Vec2::new(x, y), &line, PICKED);
                y += 14.0;
            }
        }
        if auto_shown(studio) {
            let (p, c) = auto_button(studio);
            let hover = in_rect((p, p + c), studio.cursor);
            crate::hud::draw_cell(frame, p, c, "", "FILL THE DECK", if hover { crate::hud::Lamp::On } else { crate::hud::Lamp::Off });
        }
        let top = panel_list_top(studio);
        frame.text(Vec2::new(x, top - 18.0), "ON THIS DECK", INK.scale(0.8));
        let holes = plan.map(|p| deckplan::openings(p, studio.deck)).unwrap_or_default();
        let list = deck.map(|d| panel_list(studio, d, Some(&h.sides), &holes)).unwrap_or_default();
        if list.is_empty() {
            frame.text(Vec2::new(x, top), "NONE YET", LABEL.scale(0.6));
        }
        let rows = list.len().max(1);
        for (k, (pick, text)) in list.iter().enumerate() {
            let ry = top + k as f32 * ROW;
            if ry + ROW > bottom {
                break;
            }
            let picked = studio.pick == Some(*pick);
            let hover = studio.cursor.x >= 12.0 && studio.cursor.x <= 12.0 + PANEL_WIDTH && studio.cursor.y >= ry && studio.cursor.y < ry + ROW;
            if picked || hover {
                frame.hud_rect(Vec2::new(14.0, ry - 1.0), Vec2::new(PANEL_WIDTH - 4.0, ROW), if picked { PICKED.scale(0.25) } else { GRID });
            }
            frame.text(Vec2::new(x, ry + 1.0), text, if picked { PICKED } else { LABEL });
        }
        // How it's used, under the list (as much as there's room for).
        y = top + rows as f32 * ROW + 16.0;
        for line in crate::fmt::wrap(tool_help(kind), width) {
            if y + 14.0 > bottom - 20.0 {
                break;
            }
            frame.text(Vec2::new(x, y), &line, LABEL.scale(0.85));
            y += 14.0;
        }
        if studio.pick.is_some() {
            frame.text(Vec2::new(x, bottom - 18.0), "DEL OR REMOVE: TAKE IT OUT", LABEL.scale(0.7));
        }
    }
    // What's on: the deck, the tool.
    let tool = match studio.tool {
        Tool::Select => "SELECT",
        Tool::Plane => "PLANE",
        Tool::Wall => "WALL",
        Tool::Door => "DOOR",
        Tool::Ladder => "LADDER",
        Tool::Stair => "STAIR",
    };
    let deck_line = match deck {
        Some(d) => format!("DECK {} OF {}   FLOOR {:.1} M UP   HEIGHT {:.1} M", studio.deck + 1, decks.len(), d.floor - h.keel, d.headroom),
        None => "NO DECKS YET: N ADDS ONE".into(),
    };
    frame.text(Vec2::new(plan_r.0.x, plan_r.0.y - 16.0), &format!("{deck_line}   TOOL: {tool}"), LABEL);
}

/// A dashed line (an opening in the floor, from the deck below).
fn dashed(frame: &mut Frame, a: Vec2, b: Vec2, col: Color) {
    let n = ((b - a).length() / 6.0).ceil().max(1.0) as usize;
    for i in (0..n).step_by(2) {
        frame.hud_line(a.lerp(b, i as f32 / n as f32), a.lerp(b, ((i + 1) as f32 / n as f32).min(1.0)), col);
    }
}

/// A drag handle: a square (a point) or a ring (a bend).
fn handle(frame: &mut Frame, at: Vec2, col: Color, ring: bool) {
    if ring {
        frame.hud_ellipse(at, Vec2::splat(5.0), 12, col);
    } else {
        frame.hud_box(at - Vec2::splat(4.0), Vec2::splat(8.0), col);
    }
}

