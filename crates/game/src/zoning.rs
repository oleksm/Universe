//! The zoning view (Enter on a place in the economy panel): a settlement's
//! ground from above, as the land office has it now (`services::land`, from
//! the registry's records and what's been done since): its zones, lots,
//! streets, power lines and facilities, each a layer to show or hide
//! (Z P S W F), with the port's pads and hangar for scale.
//!
//! What can be done here, as anyone can: drag a rectangle on free ground and
//! Enter to claim it; click a vacant lot and Enter to buy it; click a lot of
//! yours and B to build on it (one of the registry's facilities, laid out on
//! your lot). Each shows its price, or why it can't be, before you commit.
//! Esc steps back (menu, then the marked ground, then the list).

use universe_engine::glam::Vec2;
use universe_engine::{text_size, Color, Context, Frame, KeyCode, MouseButton};
use universe_sim::services::land::{Ground, LandOffice, Owner, Works};
use universe_sim::services::Party;
use universe_sim::world::settlements::{area, inside};
use universe_sim::world::spaceport::{GRID, PAD_RADIUS, PAD_SPACING};
use universe_sim::Command;

use crate::App;

const TEXT: Color = Color::hex(0xdcebf2);
const DIM: Color = Color::hex(0x7d93a0);
const CYAN: Color = Color::hex(0x60e0ff);
const AMBER: Color = Color::hex(0xffb040);
const RED: Color = Color::hex(0xff5040);

/// The layers, their keys and names, in the order they're drawn.
const LAYERS: [(KeyCode, &str); 5] = [(KeyCode::KeyZ, "ZONES"), (KeyCode::KeyP, "PARCELS"), (KeyCode::KeyS, "STREETS"), (KeyCode::KeyW, "POWER"), (KeyCode::KeyF, "FACILITIES")];
const ZONES: usize = 0;
const PARCELS: usize = 1;
const STREETS: usize = 2;
const POWER: usize = 3;
const FACILITIES: usize = 4;
/// Marked ground snaps to this (m).
const SNAP: f64 = 10.0;

/// What's been clicked.
#[derive(Clone, Copy, PartialEq)]
enum Pick {
    Parcel(u32),
    Facility(usize),
}

/// The view's state: whose ground, which layers show, what's picked, the
/// ground being marked, the blueprint menu.
pub struct Zoning {
    system: usize,
    port: usize,
    layers: [bool; 5],
    picked: Option<Pick>,
    /// Where a press began (screen), while the button's held.
    press: Option<Vec2>,
    /// Ground marked for a claim (a rectangle, metres from the port).
    marked: Option<Vec<(f64, f64)>>,
    /// The blueprint menu, while open: the one under the cursor.
    menu: Option<usize>,
}

impl Zoning {
    /// The view of the ground at `place`, if the land office has any there.
    pub fn open(app: &App, place: &universe_sim::services::economy::Place) -> Option<Self> {
        let universe_sim::world::Facility::Spaceport(port) = place.facility else { return None };
        let z = Zoning { system: place.system, port, layers: [true; 5], picked: None, press: None, marked: None, menu: None };
        z.ground(app).map(|_| z)
    }

    /// Pick by name, for dev scenarios: `f<k>` facility k, `p<n>` parcel n,
    /// `m` the blueprint menu (on a picked lot).
    pub fn pick(&mut self, what: &str) {
        for part in what.split(',') {
            let n = part.get(1..).and_then(|v| v.parse().ok());
            match (part.chars().next(), n) {
                (Some('f'), Some(k)) => self.picked = Some(Pick::Facility(k as usize)),
                (Some('p'), Some(n)) => self.picked = Some(Pick::Parcel(n)),
                (Some('m'), _) => self.menu = Some(0),
                _ => {}
            }
        }
    }

    /// Mark ground, for dev scenarios.
    pub fn mark(&mut self, outline: Vec<(f64, f64)>) {
        self.marked = Some(outline);
    }

    fn ground<'a>(&self, app: &'a App) -> Option<&'a Ground> {
        app.v.land.ground(self.system, self.port)
    }
}

const ME: Party = Party::Pilot(universe_sim::PLAYER);

/// Who owns a lot, as shown.
fn owner_name(o: &Owner) -> String {
    match o {
        Owner::Company { name, .. } => name.to_uppercase(),
        Owner::Party(p) if *p == ME => "YOURS".into(),
        Owner::Party(Party::Pilot(id)) => format!("PILOT {id}"),
        Owner::Party(_) => "SOMEONE".into(),
        Owner::Vacant => "FOR SALE".into(),
    }
}

/// Where the plan is drawn on the HUD (`size`): (top left, bottom right).
fn plan_area(size: Vec2) -> (Vec2, Vec2) {
    (Vec2::new(12.0, 40.0), Vec2::new(size.x - 12.0, size.y - 88.0))
}

/// The ground to screen: metres [east, north] of the port to HUD pixels,
/// the whole settlement (and the port's pads and hangar) fitted in.
struct Plan {
    centre: (f64, f64),
    scale: f64,
    at: Vec2,
}

impl Plan {
    fn of(g: &Ground, size: Vec2) -> Plan {
        let s = g.recorded;
        let hangar = -((GRID as f64 / 2.0 + 1.0) * PAD_SPACING);
        let mut pts: Vec<(f64, f64)> = vec![(-PAD_RADIUS, -PAD_RADIUS), (PAD_RADIUS, PAD_RADIUS), (0.0, hangar - 60.0)];
        pts.extend(s.zones.iter().flat_map(|z| z.outline.iter().copied()));
        pts.extend(g.lots.iter().flat_map(|p| p.outline.iter().copied()));
        pts.extend(s.streets.iter().flat_map(|st| st.line.iter().copied()));
        let (lo, hi) = pts.iter().fold(((f64::MAX, f64::MAX), (f64::MIN, f64::MIN)), |(lo, hi), p| ((lo.0.min(p.0), lo.1.min(p.1)), (hi.0.max(p.0), hi.1.max(p.1))));
        // (Room round it to mark more ground.)
        let pad = 0.12 * (hi.0 - lo.0).max(hi.1 - lo.1);
        let (lo, hi) = ((lo.0 - pad, lo.1 - pad), (hi.0 + pad, hi.1 + pad));
        let (a, b) = plan_area(size);
        let scale = ((b.x - a.x) as f64 / (hi.0 - lo.0).max(1.0)).min((b.y - a.y) as f64 / (hi.1 - lo.1).max(1.0));
        Plan { centre: ((lo.0 + hi.0) / 2.0, (lo.1 + hi.1) / 2.0), scale, at: (a + b) / 2.0 }
    }

    fn screen(&self, p: (f64, f64)) -> Vec2 {
        self.at + Vec2::new(((p.0 - self.centre.0) * self.scale) as f32, (-(p.1 - self.centre.1) * self.scale) as f32)
    }

    fn ground(&self, q: Vec2) -> (f64, f64) {
        let d = q - self.at;
        (self.centre.0 + d.x as f64 / self.scale, self.centre.1 - d.y as f64 / self.scale)
    }
}

/// A facility's module footprints, as outlines.
fn footprints(w: &Works) -> impl Iterator<Item = [(f64, f64); 4]> + '_ {
    w.blocks.iter().map(|b| {
        let ((e, n), (he, hn)) = (b.centre, b.half_extent());
        [(e - he, n - hn), (e + he, n - hn), (e + he, n + hn), (e - he, n + hn)]
    })
}

/// A lot that's yours with nothing on it, picked: the one to build on.
fn buildable(g: &Ground, picked: Option<Pick>) -> Option<u32> {
    let Some(Pick::Parcel(n)) = picked else { return None };
    let lot = g.lots.iter().find(|l| l.number == n)?;
    (lot.owner == Owner::Party(ME) && !g.works.iter().any(|w| w.parcel == n)).then_some(n)
}

/// Keys and clicks while open. False to go back to the list.
pub fn input(app: &mut App, z: &mut Zoning, ctx: &Context) -> bool {
    let input = &ctx.input;
    let Some(g) = z.ground(app) else { return false };
    let blueprints = universe_sim::estate::blueprints();
    // The blueprint menu has the keys while it's open.
    if let Some(sel) = z.menu.as_mut() {
        if input.pressed(KeyCode::Escape) {
            z.menu = None;
        } else if input.pressed(KeyCode::ArrowDown) {
            *sel = (*sel + 1) % blueprints.len().max(1);
        } else if input.pressed(KeyCode::ArrowUp) {
            *sel = (*sel + blueprints.len().max(1) - 1) % blueprints.len().max(1);
        } else if input.pressed(KeyCode::Enter) {
            if let (Some(n), Some(b)) = (buildable(g, z.picked), blueprints.get(*sel)) {
                let cmd = Command::Build { system: z.system, port: z.port, number: n, blueprint: b.to_string() };
                z.menu = None;
                app.engine.send(cmd);
                return true;
            }
            z.menu = None;
        }
        return true;
    }
    if input.pressed(KeyCode::Escape) {
        return z.marked.take().is_some();
    }
    for (k, (key, _)) in LAYERS.iter().enumerate() {
        if input.pressed(*key) {
            z.layers[k] = !z.layers[k];
        }
    }
    if input.pressed(KeyCode::Enter) {
        if let Some(outline) = z.marked.take() {
            app.engine.send(Command::ClaimLand { system: z.system, port: z.port, outline });
        } else if let Some(Pick::Parcel(n)) = z.picked
            && g.lots.iter().any(|l| l.number == n && l.owner == Owner::Vacant)
        {
            app.engine.send(Command::BuyParcel { system: z.system, port: z.port, number: n });
        }
        return true;
    }
    if input.pressed(KeyCode::KeyB) && buildable(g, z.picked).is_some() {
        z.menu = Some(0);
        return true;
    }
    // A press: a click picks; a drag marks ground.
    let size = ctx.hud_size.as_vec2();
    let (a, b) = plan_area(size);
    let q = input.cursor;
    let over = q.x >= a.x && q.y >= a.y && q.x <= b.x && q.y <= b.y;
    if input.button_pressed(MouseButton::Left) && over {
        z.press = Some(q);
    }
    let plan = Plan::of(g, size);
    if let Some(from) = z.press {
        let snap = |p: (f64, f64)| ((p.0 / SNAP).round() * SNAP, (p.1 / SNAP).round() * SNAP);
        let dragged = (q - from).length() > 6.0;
        if dragged {
            let (p, r) = (snap(plan.ground(from)), snap(plan.ground(q)));
            let (w, e, s, n) = (p.0.min(r.0), p.0.max(r.0), p.1.min(r.1), p.1.max(r.1));
            z.marked = Some(vec![(w, s), (e, s), (e, n), (w, n)]);
            z.picked = None;
        }
        if !input.button_down(MouseButton::Left) {
            z.press = None;
            if !dragged {
                let p = plan.ground(q);
                z.marked = None;
                let facility = if z.layers[FACILITIES] { g.works.iter().position(|w| footprints(w).any(|o| inside(&o, p))) } else { None };
                let parcel = if z.layers[PARCELS] { g.lots.iter().find(|l| inside(&l.outline, p)).map(|l| l.number) } else { None };
                z.picked = facility.map(Pick::Facility).or(parcel.map(Pick::Parcel));
            }
        }
    }
    true
}

/// Fill a convex outline on the HUD.
fn fill(frame: &mut Frame, plan: &Plan, o: &[(f64, f64)], c: Color) {
    let pts: Vec<Vec2> = o.iter().map(|&p| plan.screen(p)).collect();
    for k in 1..pts.len().saturating_sub(1) {
        frame.hud_triangle_colored([pts[0], pts[k], pts[k + 1]], [c, c, c]);
    }
}

fn outline(frame: &mut Frame, plan: &Plan, o: &[(f64, f64)], c: Color) {
    for k in 0..o.len() {
        frame.hud_line(plan.screen(o[k]), plan.screen(o[(k + 1) % o.len()]), c);
    }
}

/// A zone's tint by its use.
fn tint(use_: &str) -> Color {
    match use_ {
        "port" => Color::hex(0x0d2830),
        "industrial" => Color::hex(0x2c2412),
        "commercial" => Color::hex(0x1c1a30),
        "civic" => Color::hex(0x23202a),
        "agricultural" => Color::hex(0x1a2c14),
        _ => Color::hex(0x1a2420),
    }
}

/// What the registry says a facility built like `w` can do at most (the
/// registry's facility of the same modules), and the power its modules
/// supply.
fn maxima(w: &Works) -> Vec<String> {
    let c = universe_sim::world::content::content();
    let mut out = Vec::new();
    let same = c.settlements.iter().flat_map(|s| &s.facilities).find(|f| f.blocks.len() == w.blocks.len() && f.blocks.iter().zip(&w.blocks).all(|(a, b)| a.module == b.module));
    if let Some(f) = same {
        out.extend(f.makes.iter().map(|(what, rate)| format!("MAKES {} UP TO {:.1} T/H", what.to_uppercase(), rate * 3.6)));
        if f.draws > 0.0 {
            out.push(format!("DRAWS {:.0} MW FLAT OUT", f.draws / 1e6));
        }
    }
    let supplies: f64 = w.blocks.iter().filter_map(|b| c.industrial(&b.module)).map(|m| m.supplies).sum();
    if supplies > 0.0 {
        out.push(format!("SUPPLIES UP TO {:.0} MW", supplies / 1e6));
    }
    out
}

pub fn draw(frame: &mut Frame, app: &App, z: &Zoning) {
    let size = frame.size();
    frame.hud_rect(Vec2::ZERO, size, Color([0.012, 0.018, 0.026, 1.0]));
    let Some(g) = z.ground(app) else { return };
    let s = g.recorded;
    let now = app.now();
    let line = 11.0;
    let mut x = 12.0;
    x = frame.text(Vec2::new(x, 12.0), &format!("ZONING - {}, {}   ", s.name.to_uppercase(), s.body.to_uppercase()), TEXT).x;
    for (k, (key, name)) in LAYERS.iter().enumerate() {
        let k_name = format!("{:?}", key).trim_start_matches("Key").to_string();
        x = frame.text(Vec2::new(x, 12.0), &format!("{k_name} {name}  "), if z.layers[k] { CYAN } else { DIM }).x;
    }
    let _ = x;
    let credits = format!("{:.0} CR", app.v.credits);
    frame.text(Vec2::new(size.x - 12.0 - text_size(&credits).x, 12.0), &credits, TEXT);
    let plan = Plan::of(g, size);
    let (a, b) = plan_area(size);
    frame.hud_box(a, b - a, Color::hex(0x1e2a33));
    // (The menu stands alone: the plan's lines would show through it.)
    let menu_open = z.menu.is_some() && buildable(g, z.picked).is_some();
    let on = |k: usize| z.layers[k] && !menu_open;

    if on(ZONES) {
        for zn in &s.zones {
            fill(frame, &plan, &zn.outline, tint(&zn.use_));
            outline(frame, &plan, &zn.outline, tint(&zn.use_).scale(2.2));
            let top = zn.outline.iter().map(|p| p.1).fold(f64::MIN, f64::max);
            let left = zn.outline.iter().map(|p| p.0).fold(f64::MAX, f64::min);
            frame.text(plan.screen((left, top)) + Vec2::new(4.0, 3.0), &zn.name.to_uppercase(), DIM);
        }
    }
    // The port's own: its pads and its hangar, for scale.
    if !menu_open {
        let half = (GRID as f64 - 1.0) / 2.0;
        for row in 0..GRID {
            for col in 0..GRID {
                let (e, n) = ((col as f64 - half) * PAD_SPACING, -(row as f64 - half) * PAD_SPACING);
                outline(frame, &plan, &[(e - 35.0, n - 35.0), (e + 35.0, n - 35.0), (e + 35.0, n + 35.0), (e - 35.0, n + 35.0)], Color::hex(0x4090b0));
            }
        }
        let h = -((GRID as f64 / 2.0 + 1.0) * PAD_SPACING);
        outline(frame, &plan, &[(-40.0, h - 30.0), (40.0, h - 30.0), (40.0, h + 30.0), (-40.0, h + 30.0)], Color::hex(0x4090b0));
    }
    if on(STREETS) {
        for st in &s.streets {
            for w in st.line.windows(2) {
                let (p, q) = (w[0], w[1]);
                let (dx, dy) = (q.0 - p.0, q.1 - p.1);
                let len = (dx * dx + dy * dy).sqrt().max(1e-6);
                let (sx, sy) = (-dy / len * 10.0, dx / len * 10.0);
                fill(frame, &plan, &[(p.0 + sx, p.1 + sy), (q.0 + sx, q.1 + sy), (q.0 - sx, q.1 - sy), (p.0 - sx, p.1 - sy)], Color::hex(0x3a4048));
            }
            // Its name: above it if it runs east-west, beside it (west) if north-south.
            if let (Some(first), Some(last)) = (st.line.first(), st.line.last()) {
                let label = st.name.to_uppercase();
                let w = text_size(&label).x;
                if (last.0 - first.0).abs() >= (last.1 - first.1).abs() {
                    let at = plan.screen((first.0 + (last.0 - first.0) * 0.6, first.1 + (last.1 - first.1) * 0.6));
                    frame.text(at + Vec2::new(-w / 2.0, -14.0 - (10.0 * plan.scale) as f32), &label, DIM);
                } else {
                    let at = plan.screen((first.0 + (last.0 - first.0) * 0.7, first.1 + (last.1 - first.1) * 0.7));
                    frame.text(at + Vec2::new(-w - 6.0 - (10.0 * plan.scale) as f32, -5.0), &label, DIM);
                }
            }
        }
    }
    let lot_colour = |o: &Owner, lit: bool| {
        if lit {
            CYAN
        } else if *o == Owner::Party(ME) {
            AMBER
        } else if *o == Owner::Vacant {
            DIM
        } else {
            TEXT.scale(0.7)
        }
    };
    if on(PARCELS) {
        for l in &g.lots {
            outline(frame, &plan, &l.outline, lot_colour(&l.owner, z.picked == Some(Pick::Parcel(l.number))));
        }
    }
    if on(FACILITIES) {
        for (k, w) in g.works.iter().enumerate() {
            let lit = z.picked == Some(Pick::Facility(k));
            for (j, o) in footprints(w).enumerate() {
                let p = w.progress(j, now);
                if p >= 1.0 {
                    fill(frame, &plan, &o, if lit { Color::hex(0x2a5866) } else { Color::hex(0x4a5058) });
                    outline(frame, &plan, &o, if lit { CYAN } else { Color::hex(0x9aa0a6) });
                } else {
                    // (Going up: filled from its south edge as far as it's built.)
                    if p > 0.0 {
                        let (s0, n0) = (o[0].1, o[2].1);
                        let top = s0 + (n0 - s0) * p;
                        fill(frame, &plan, &[o[0], o[1], (o[1].0, top), (o[0].0, top)], Color::hex(0x3a3420));
                    }
                    outline(frame, &plan, &o, if lit { CYAN } else { AMBER.scale(0.6) });
                }
            }
        }
    }
    if on(POWER) {
        for pw in &s.power_lines {
            for w in pw.line.windows(2) {
                frame.hud_line(plan.screen(w[0]), plan.screen(w[1]), AMBER);
            }
            for p in &pw.line {
                frame.hud_rect(plan.screen(*p) - Vec2::splat(1.5), Vec2::splat(3.0), AMBER);
            }
        }
    }
    // Lots' numbers and owners, over everything.
    if on(PARCELS) {
        for l in &g.lots {
            let c = lot_colour(&l.owner, z.picked == Some(Pick::Parcel(l.number)));
            let (w, e) = l.outline.iter().fold((f64::MAX, f64::MIN), |m, q| (m.0.min(q.0), m.1.max(q.0)));
            let top = l.outline.iter().map(|q| q.1).fold(f64::MIN, f64::max);
            let corner = plan.screen((w, top)) + Vec2::new(4.0, 3.0);
            frame.text(corner, &format!("{}", l.number), c);
            let owner = owner_name(&l.owner);
            if text_size(&owner).x + 8.0 < ((e - w) * plan.scale) as f32 {
                frame.text(corner + Vec2::new(0.0, line), &owner, c.scale(0.8));
            }
        }
    }
    // Ground marked for a claim.
    let office: &LandOffice = &app.v.land;
    let claim = z.marked.as_ref().map(|o| (o, office.quote_claim(z.system, z.port, o)));
    if let Some((o, quote)) = claim.as_ref().filter(|_| !menu_open) {
        fill(frame, &plan, o, if quote.is_ok() { Color::hex(0x3a2c10) } else { Color::hex(0x3a1410) });
        outline(frame, &plan, o, if quote.is_ok() { AMBER } else { RED });
    }
    // Scale: a bar of a round length, bottom left of the plan; north up.
    let metres = [50.0, 100.0, 200.0, 500.0, 1000.0, 2000.0].into_iter().find(|m| m * plan.scale > 60.0).unwrap_or(5000.0);
    let base = Vec2::new(a.x + 12.0, b.y - 14.0);
    frame.hud_line(base, base + Vec2::new((metres * plan.scale) as f32, 0.0), TEXT);
    frame.text(base + Vec2::new(0.0, -13.0), &format!("{metres:.0} M   N UP"), DIM);

    // What's picked or marked, in full, and what can be done.
    let mut y = b.y + 8.0;
    let zone_of = |o: &[(f64, f64)]| {
        let c = o.iter().fold((0.0, 0.0), |m, q| (m.0 + q.0 / o.len() as f64, m.1 + q.1 / o.len() as f64));
        s.zones.iter().find(|zn| inside(&zn.outline, c)).map_or("NONE".to_string(), |zn| zn.name.to_uppercase())
    };
    let mut hint = "CLICK: A LOT OR FACILITY   DRAG: MARK GROUND TO CLAIM   ESC: BACK".to_string();
    if let Some((o, quote)) = &claim {
        let (w, e, s0, n0) = o.iter().fold((f64::MAX, f64::MIN, f64::MAX, f64::MIN), |m, p| (m.0.min(p.0), m.1.max(p.0), m.2.min(p.1), m.3.max(p.1)));
        frame.text(Vec2::new(12.0, y), &format!("MARKED {:.0} X {:.0} M   {:.0} M2   ZONE {}", e - w, n0 - s0, area(o), zone_of(o)), TEXT);
        y += line;
        match quote {
            Ok(price) => {
                frame.text(Vec2::new(12.0, y), &format!("THE LAND OFFICE ASKS {price:.0} CR"), AMBER);
                hint = "ENTER: CLAIM IT   ESC: CLEAR".into();
            }
            Err(why) => {
                frame.text(Vec2::new(12.0, y), &format!("CAN'T BE CLAIMED: {why}"), RED);
                hint = "DRAG AGAIN   ESC: CLEAR".into();
            }
        }
    } else {
        match z.picked {
            Some(Pick::Parcel(n)) => {
                if let Some(l) = g.lots.iter().find(|l| l.number == n) {
                    frame.text(Vec2::new(12.0, y), &format!("PARCEL {n}   {}   ZONE {}   {:.0} M2", owner_name(&l.owner), zone_of(&l.outline), area(&l.outline)), TEXT);
                    y += line;
                    let on_it: Vec<String> = g.works.iter().filter(|w| w.parcel == n).map(|w| w.name.to_uppercase()).collect();
                    if l.owner == Owner::Vacant {
                        match office.quote_buy(z.system, z.port, n) {
                            Ok(price) => {
                                frame.text(Vec2::new(12.0, y), &format!("FOR SALE: {price:.0} CR"), AMBER);
                                hint = "ENTER: BUY IT   ESC: BACK".into();
                            }
                            Err(why) => {
                                frame.text(Vec2::new(12.0, y), &why, RED);
                            }
                        }
                    } else {
                        frame.text(Vec2::new(12.0, y), &format!("ON IT: {}", if on_it.is_empty() { "NOTHING BUILT".to_string() } else { on_it.join(", ") }), DIM);
                        if buildable(g, z.picked).is_some() {
                            hint = "B: BUILD ON IT   ESC: BACK".into();
                        }
                    }
                }
            }
            Some(Pick::Facility(k)) => {
                if let Some(w) = g.works.get(k) {
                    let owner = g.lots.iter().find(|l| l.number == w.parcel).map_or(String::new(), |l| owner_name(&l.owner));
                    let n = w.blocks.len();
                    frame.text(Vec2::new(12.0, y), &format!("{}   {}   ON PARCEL {}   {owner}   {n} MODULE{}", w.name.to_uppercase(), w.kind.to_uppercase(), w.parcel, if n == 1 { "" } else { "S" }), TEXT);
                    y += line;
                    if w.built(now) {
                        frame.text(Vec2::new(12.0, y), &format!("AT MOST: {}", maxima(w).join("   ")), DIM);
                        y += line;
                        // How it ran over the last step (the economy's), and what of it isn't traded yet.
                        if let Some(r) = &w.last {
                            let held = r.held_by.as_ref().map_or(String::new(), |h| format!(", HELD BY {h}"));
                            frame.text(Vec2::new(12.0, y), &format!("LAST 10 MIN: RAN AT {:.0}%{held}   EARNED {:+.0} CR", r.rate * 100.0, r.earned), if r.earned < 0.0 { AMBER } else { TEXT });
                        } else {
                            frame.text(Vec2::new(12.0, y), "NOT RUN YET", DIM);
                        }
                        let c = universe_sim::world::content::content();
                        if let Some(f) = c.settlements.iter().flat_map(|s| &s.facilities).find(|f| Some(&f.key) == w.facility.as_ref()) {
                            let untraded: Vec<String> = f.takes.iter().chain(&f.gives).chain(&f.burns).filter(|t| t.1.is_empty()).map(|t| t.0.to_uppercase()).collect();
                            if !untraded.is_empty() {
                                let mut text = format!("NOT TRADED YET (NO GAME KIND): {}", untraded.join(", "));
                                while text_size(&text).x > size.x - 24.0 && text.len() > 4 {
                                    text.truncate(text.len() - 4);
                                    text.push_str("...");
                                }
                                frame.text(Vec2::new(12.0, y + line), &text, DIM);
                            }
                        }
                    } else {
                        let done = (0..n).filter(|&j| w.progress(j, now) >= 1.0).count();
                        let left = w.done_at.last().copied().unwrap_or(now) - now;
                        frame.text(Vec2::new(12.0, y), &format!("BUILDING: {done} OF {n} MODULES UP, DONE IN {}", universe_sim::estate::duration(left)), AMBER);
                    }
                }
            }
            None => {
                frame.text(Vec2::new(12.0, y), &format!("{} ZONES, {} LOTS, {} FACILITIES, {} STREETS, {} POWER LINES   AS THE LAND OFFICE HAS THEM", s.zones.len(), g.lots.len(), g.works.len(), s.streets.len(), s.power_lines.len()), DIM);
            }
        }
    }
    frame.text(Vec2::new(size.x - 12.0 - text_size(&hint).x, size.y - 24.0), &hint, DIM);

    // The blueprint menu: the registry's facilities, each with what it would
    // cost on this lot and how long it would take, or why it won't fit.
    if let (Some(sel), Some(n)) = (z.menu, buildable(g, z.picked)) {
        let rows = universe_sim::estate::blueprints();
        let (w, h) = (900.0, 30.0 + (rows.len() as f32 + 1.0) * line + 20.0);
        let at = Vec2::new((size.x - w) / 2.0, (size.y - h) / 2.0);
        frame.hud_rect(at, Vec2::new(w, h), Color([0.02, 0.03, 0.04, 0.97]));
        frame.hud_box(at, Vec2::new(w, h), CYAN.scale(0.6));
        frame.text(at + Vec2::new(12.0, 10.0), &format!("BUILD ON PARCEL {n}   UP/DOWN, ENTER: BUILD, ESC: CANCEL"), TEXT);
        let mut ry = at.y + 30.0;
        frame.text(Vec2::new(at.x + 12.0, ry), &format!(" {:<22} {:>12} {:>9}  {}", "BLUEPRINT", "COST", "TIME", "ON THIS LOT"), DIM);
        ry += line;
        for (k, name) in rows.iter().enumerate() {
            let quote = universe_sim::estate::blueprint_of(name).ok_or_else(|| "NO SUCH BLUEPRINT".to_string()).and_then(|(_, m)| office.quote_build(z.system, z.port, n, ME, &m));
            let mark = if k == sel { ">" } else { " " };
            let (text, c) = match quote {
                Ok((_, cost, time)) => (format!("{mark}{:<22} {:>9.0} CR {:>9}  FITS", name.to_uppercase(), cost, universe_sim::estate::duration(time)), if k == sel { TEXT } else { DIM }),
                Err(why) => (format!("{mark}{:<22} {:>12} {:>9}  {why}", name.to_uppercase(), "-", "-"), if k == sel { RED } else { RED.scale(0.6) }),
            };
            frame.text(Vec2::new(at.x + 12.0, ry), &text, c);
            ry += line;
        }
    }
}
