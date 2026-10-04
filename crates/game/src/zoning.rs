//! The zoning view (Enter on a place in the economy panel): a settlement's
//! ground from above, as the registry records it (`world::settlements`):
//! its zones, parcels, streets, power lines and facilities, each a layer to
//! show or hide (Z P S W F), with the port's pads and hangar for scale. Click
//! a parcel or a facility for its facts. Esc goes back to the list.

use universe_engine::glam::Vec2;
use universe_engine::{Color, Context, Frame, KeyCode, MouseButton};
use universe_sim::world::settlements::{area, inside, Settlement};
use universe_sim::world::spaceport::{GRID, PAD_RADIUS, PAD_SPACING};

use crate::App;

const TEXT: Color = Color::hex(0xdcebf2);
const DIM: Color = Color::hex(0x7d93a0);
const CYAN: Color = Color::hex(0x60e0ff);
const AMBER: Color = Color::hex(0xffb040);

/// The layers, their keys and names, in the order they're drawn.
const LAYERS: [(KeyCode, &str); 5] = [(KeyCode::KeyZ, "ZONES"), (KeyCode::KeyP, "PARCELS"), (KeyCode::KeyS, "STREETS"), (KeyCode::KeyW, "POWER"), (KeyCode::KeyF, "FACILITIES")];
const ZONES: usize = 0;
const PARCELS: usize = 1;
const STREETS: usize = 2;
const POWER: usize = 3;
const FACILITIES: usize = 4;

/// What's been clicked.
#[derive(Clone, Copy, PartialEq)]
enum Pick {
    Parcel(u32),
    Facility(usize),
}

/// The view's state: whose ground, which layers show, what's picked.
pub struct Zoning {
    system: usize,
    port: usize,
    layers: [bool; 5],
    picked: Option<Pick>,
}

impl Zoning {
    /// The view of the ground at `place`, if the registry records any there.
    pub fn open(app: &App, place: &universe_sim::services::economy::Place) -> Option<Self> {
        let universe_sim::world::Facility::Spaceport(port) = place.facility else { return None };
        let z = Zoning { system: place.system, port, layers: [true; 5], picked: None };
        z.settlement(app).map(|_| z)
    }

    /// Pick by name, for dev scenarios: `f<k>` facility k, `p<n>` parcel n.
    pub fn pick(&mut self, what: &str) {
        let n = what.get(1..).and_then(|v| v.parse().ok());
        self.picked = match (what.chars().next(), n) {
            (Some('f'), Some(k)) => Some(Pick::Facility(k as usize)),
            (Some('p'), Some(n)) => Some(Pick::Parcel(n)),
            _ => None,
        };
    }

    fn settlement(&self, app: &App) -> Option<&'static Settlement> {
        let sys = app.charts.system(self.system);
        let sp = sys.spaceports.get(self.port)?;
        universe_sim::world::content::content().settlement(&sys.name, &sys.bodies[sp.body].name, &sp.name)
    }
}

/// Where the plan is drawn on the HUD (`size`): (top left, bottom right).
fn plan_area(size: Vec2) -> (Vec2, Vec2) {
    (Vec2::new(12.0, 40.0), Vec2::new(size.x - 12.0, size.y - 64.0))
}

/// The ground to screen: metres [east, north] of the port to HUD pixels,
/// the whole settlement (and the port's pads and hangar) fitted in.
struct Plan {
    centre: (f64, f64),
    scale: f64,
    at: Vec2,
}

impl Plan {
    fn of(s: &Settlement, size: Vec2) -> Plan {
        let hangar = -((GRID as f64 / 2.0 + 1.0) * PAD_SPACING);
        let mut pts: Vec<(f64, f64)> = vec![(-PAD_RADIUS, -PAD_RADIUS), (PAD_RADIUS, PAD_RADIUS), (0.0, hangar - 60.0)];
        pts.extend(s.zones.iter().flat_map(|z| z.outline.iter().copied()));
        pts.extend(s.parcels.iter().flat_map(|p| p.outline.iter().copied()));
        pts.extend(s.streets.iter().flat_map(|st| st.line.iter().copied()));
        let (lo, hi) = pts.iter().fold(((f64::MAX, f64::MAX), (f64::MIN, f64::MIN)), |(lo, hi), p| ((lo.0.min(p.0), lo.1.min(p.1)), (hi.0.max(p.0), hi.1.max(p.1))));
        let (a, b) = plan_area(size);
        let scale = 0.94 * ((b.x - a.x) as f64 / (hi.0 - lo.0).max(1.0)).min((b.y - a.y) as f64 / (hi.1 - lo.1).max(1.0));
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
fn footprints(f: &universe_sim::world::settlements::Facility) -> impl Iterator<Item = [(f64, f64); 4]> + '_ {
    f.blocks.iter().map(|b| {
        let ((e, n), (he, hn)) = (b.centre, b.half_extent());
        [(e - he, n - hn), (e + he, n - hn), (e + he, n + hn), (e - he, n + hn)]
    })
}

/// Keys and clicks while open. False to go back to the list.
pub fn input(app: &App, z: &mut Zoning, ctx: &Context) -> bool {
    let input = &ctx.input;
    if input.pressed(KeyCode::Escape) {
        return false;
    }
    for (k, (key, _)) in LAYERS.iter().enumerate() {
        if input.pressed(*key) {
            z.layers[k] = !z.layers[k];
        }
    }
    if input.button_pressed(MouseButton::Left) {
        let Some(s) = z.settlement(app) else { return false };
        let size = ctx.hud_size.as_vec2();
        let (a, b) = plan_area(size);
        let q = input.cursor;
        if q.x >= a.x && q.y >= a.y && q.x <= b.x && q.y <= b.y {
            let g = Plan::of(s, size).ground(q);
            // (A facility's module before the parcel it stands on.)
            let facility = if z.layers[FACILITIES] { s.facilities.iter().position(|f| footprints(f).any(|o| inside(&o, g))) } else { None };
            let parcel = if z.layers[PARCELS] { s.parcels.iter().find(|p| inside(&p.outline, g)).map(|p| p.number) } else { None };
            z.picked = facility.map(Pick::Facility).or(parcel.map(Pick::Parcel));
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
        _ => Color::hex(0x1a2420),
    }
}

pub fn draw(frame: &mut Frame, app: &App, z: &Zoning) {
    let size = frame.size();
    frame.hud_rect(Vec2::ZERO, size, Color([0.012, 0.018, 0.026, 1.0]));
    let Some(s) = z.settlement(app) else { return };
    let line = 11.0;
    let mut x = 12.0;
    x = frame.text(Vec2::new(x, 12.0), &format!("ZONING - {}, {}   ", s.name.to_uppercase(), s.body.to_uppercase()), TEXT).x;
    for (k, (key, name)) in LAYERS.iter().enumerate() {
        let k_name = format!("{:?}", key).trim_start_matches("Key").to_string();
        x = frame.text(Vec2::new(x, 12.0), &format!("{k_name} {name}  "), if z.layers[k] { CYAN } else { DIM }).x;
    }
    let _ = x;
    let hint = "CLICK A PARCEL OR FACILITY: ITS FACTS   ESC: BACK";
    frame.text(Vec2::new(size.x - 12.0 - universe_engine::text_size(hint).x, size.y - 24.0), hint, DIM);
    let plan = Plan::of(s, size);
    let (a, b) = plan_area(size);
    frame.hud_box(a, b - a, Color::hex(0x1e2a33));
    let on = |k: usize| z.layers[k];

    if on(ZONES) {
        for zn in &s.zones {
            fill(frame, &plan, &zn.outline, tint(&zn.use_));
            outline(frame, &plan, &zn.outline, tint(&zn.use_).scale(2.2));
            let top = zn.outline.iter().fold((0.0, f64::MIN), |m, p| if p.1 > m.1 { *p } else { m });
            let left = zn.outline.iter().map(|p| p.0).fold(f64::MAX, f64::min);
            frame.text(plan.screen((left, top.1)) + Vec2::new(4.0, 3.0), &zn.name.to_uppercase(), DIM);
        }
    }
    // The port's own: its pads and its hangar, for scale.
    {
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
                let w = universe_engine::text_size(&label).x;
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
    if on(PARCELS) {
        for p in &s.parcels {
            let lit = z.picked == Some(Pick::Parcel(p.number));
            outline(frame, &plan, &p.outline, if lit { CYAN } else { TEXT.scale(0.7) });
        }
    }
    if on(FACILITIES) {
        for (k, f) in s.facilities.iter().enumerate() {
            let lit = z.picked == Some(Pick::Facility(k));
            for o in footprints(f) {
                fill(frame, &plan, &o, if lit { Color::hex(0x2a5866) } else { Color::hex(0x4a5058) });
                outline(frame, &plan, &o, if lit { CYAN } else { Color::hex(0x9aa0a6) });
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
    // Parcels' numbers and owners, over everything.
    if on(PARCELS) {
        for p in &s.parcels {
            let lit = z.picked == Some(Pick::Parcel(p.number));
            // Its number inside its top left corner; who owns it under, if that fits across it.
            let (w, e) = p.outline.iter().fold((f64::MAX, f64::MIN), |m, q| (m.0.min(q.0), m.1.max(q.1)));
            let top = p.outline.iter().map(|q| q.1).fold(f64::MIN, f64::max);
            let corner = plan.screen((w, top)) + Vec2::new(4.0, 3.0);
            let c = if lit { CYAN } else { TEXT.scale(0.8) };
            frame.text(corner, &format!("{}", p.number), c);
            let owner = p.owner_name.to_uppercase();
            if universe_engine::text_size(&owner).x + 8.0 < ((e - w) * plan.scale) as f32 {
                frame.text(corner + Vec2::new(0.0, line), &owner, c.scale(0.8));
            }
        }
    }
    // Scale: a bar of a round length, bottom left of the plan; north up.
    let metres = [50.0, 100.0, 200.0, 500.0, 1000.0, 2000.0].into_iter().find(|m| m * plan.scale > 60.0).unwrap_or(5000.0);
    let base = Vec2::new(a.x + 12.0, b.y - 14.0);
    frame.hud_line(base, base + Vec2::new((metres * plan.scale) as f32, 0.0), TEXT);
    frame.text(base + Vec2::new(0.0, -13.0), &format!("{metres:.0} M   N UP"), DIM);

    // What's picked, in full.
    let mut y = b.y + 8.0;
    let zone_of = |o: &[(f64, f64)]| {
        let c = o.iter().fold((0.0, 0.0), |m, q| (m.0 + q.0 / o.len() as f64, m.1 + q.1 / o.len() as f64));
        s.zones.iter().find(|zn| inside(&zn.outline, c)).map_or("NONE".to_string(), |zn| zn.name.to_uppercase())
    };
    match z.picked {
        Some(Pick::Parcel(n)) => {
            let Some(p) = s.parcels.iter().find(|p| p.number == n) else { return };
            frame.text(Vec2::new(12.0, y), &format!("PARCEL {n}   OWNER {}   ZONE {}   {:.0} M2", p.owner_name.to_uppercase(), zone_of(&p.outline), area(&p.outline)), TEXT);
            y += line;
            let on_it: Vec<String> = s.facilities.iter().filter(|f| f.parcel == n).map(|f| f.name.to_uppercase()).collect();
            frame.text(Vec2::new(12.0, y), &format!("ON IT: {}", if on_it.is_empty() { "NOTHING BUILT".to_string() } else { on_it.join(", ") }), DIM);
        }
        Some(Pick::Facility(k)) => {
            let Some(f) = s.facilities.get(k) else { return };
            let owner = s.parcels.iter().find(|p| p.number == f.parcel).map_or(String::new(), |p| p.owner_name.to_uppercase());
            let n = f.blocks.len();
            frame.text(Vec2::new(12.0, y), &format!("{}   {}   ON PARCEL {}   {owner}   {n} MODULE{}", f.name.to_uppercase(), f.kind.to_uppercase(), f.parcel, if n == 1 { "" } else { "S" }), TEXT);
            y += line;
            let mut most: Vec<String> = f.makes.iter().map(|(what, t)| format!("MAKES {} UP TO {t:.1} T/H", what.to_uppercase())).collect();
            if f.draws > 0.0 {
                most.push(format!("DRAWS {:.0} MW FLAT OUT", f.draws));
            }
            if f.supplies > 0.0 {
                most.push(format!("SUPPLIES UP TO {:.0} MW", f.supplies));
            }
            if f.holds > 0.0 {
                most.push(format!("HOLDS UP TO {:.0} T", f.holds));
            }
            frame.text(Vec2::new(12.0, y), &format!("AT MOST: {}", most.join("   ")), DIM);
        }
        None => {
            frame.text(Vec2::new(12.0, y), &format!("{} ZONES, {} PARCELS, {} FACILITIES, {} STREETS, {} POWER LINES   AS THE LAND OFFICE RECORDS THEM", s.zones.len(), s.parcels.len(), s.facilities.len(), s.streets.len(), s.power_lines.len()), DIM);
        }
    }
}
