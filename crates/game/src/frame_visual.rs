//! A built frame's shape, from its stock (SFO 13, SFO 14): each member a length of
//! its tube, bar or box section, each node a ball of its forging, each deck a slab
//! of its plate. No model: the stock record's form and size are the shape, its
//! tint the colour (docs/ships/frames-answer.md). The studio, its stand and the
//! test drive draw a frame from this; the game will draw a saved assembly so too.
use crate::equipment_visual::{Face, Placed};
use glam::{Vec2, Vec3};
use universe_engine::Color;

/// A member: a length of `stock` from `a` to `b`, a box section turned `roll` (rad)
/// about its axis from its height standing in the plane of the member and up.
pub struct Member<'a> {
    pub a: Vec3,
    pub b: Vec3,
    pub stock: &'a str,
    pub roll: f32,
}

/// A node: a ball of `stock` (a forging) at `at`.
pub struct Node<'a> {
    pub at: Vec3,
    pub stock: &'a str,
}

/// A deck: a slab of `stock` over `lo`..`hi` (x, z), its top at `top`.
pub struct Deck<'a> {
    pub lo: Vec2,
    pub hi: Vec2,
    pub top: f32,
    pub stock: &'a str,
}

/// Sides round a tube or a ball's girth.
const ROUND: usize = 16;

/// Its stock's colour (the record's tint), else a steel grey.
fn tint(s: Option<&universe_sim::world::registry::Stock>) -> Color {
    let [r, g, b] = s.and_then(|s| s.tint).unwrap_or([0.6, 0.62, 0.65]);
    Color([r as f32, g as f32, b as f32, 1.0])
}

fn stock(key: &str) -> Option<&'static universe_sim::world::registry::Stock> {
    universe_sim::world::registry::registry().stock(key)
}

fn face(positions: [Vec3; 3], normals: [Vec3; 3], colour: Color) -> Face {
    Face { positions, normals, colour, double_sided: true, blend: false, alpha_cutoff: None }
}

/// Two triangles of the quad a, b, c, d (anticlockwise seen from its outside).
fn quad(out: &mut Vec<Face>, p: [Vec3; 4], n: [Vec3; 4], c: Color) {
    out.push(face([p[0], p[1], p[2]], [n[0], n[1], n[2]], c));
    out.push(face([p[0], p[2], p[3]], [n[0], n[2], n[3]], c));
}

/// A section's outline about its axis (x across, y up the section) with each
/// corner's outward normal per side: a ring of `ROUND` or a rectangle.
struct Section {
    /// (Outline points, outward normals at each side's two ends: smooth on a ring.)
    outer: Vec<(Vec2, Vec2, Vec2)>,
    /// The bore's outline (same count), or none: solid.
    inner: Option<Vec<Vec2>>,
}

fn ring(r: f32) -> Vec<Vec2> {
    (0..ROUND).map(|k| Vec2::from_angle(k as f32 / ROUND as f32 * std::f32::consts::TAU) * r).collect()
}

fn rect(w: f32, h: f32) -> Vec<Vec2> {
    vec![Vec2::new(w, -h), Vec2::new(w, h), Vec2::new(-w, h), Vec2::new(-w, -h)]
}

fn section(s: &universe_sim::world::registry::Stock) -> Option<Section> {
    let z = &s.size;
    match s.identity.form.as_str() {
        "box" => {
            let (w, h) = (z.width? as f32 * 0.5, z.height? as f32 * 0.5);
            let o = rect(w, h);
            let outer = (0..4).map(|k| {
                let (p, q) = (o[k], o[(k + 1) % 4]);
                let n = (q - p).perp().normalize() * -1.0;
                (p, n, n)
            }).collect();
            let inner = z.wall.map(|t| rect(w - t as f32, h - t as f32)).filter(|_| z.wall.unwrap() * 2.0 < z.width.unwrap().min(z.height.unwrap()));
            Some(Section { outer, inner })
        }
        "tube" | "bar" => {
            let r = z.diameter? as f32 * 0.5;
            let o = ring(r);
            let outer = o.iter().map(|p| (*p, p.normalize(), p.normalize())).collect();
            let inner = z.wall.filter(|t| (*t as f32) < r).map(|t| ring(r - t as f32));
            Some(Section { outer, inner })
        }
        _ => None,
    }
}

/// A member's faces: its section drawn along it, its ends capped (a tube's as rings,
/// its bore lined).
fn member(out: &mut Vec<Face>, m: &Member) {
    let s = stock(m.stock);
    let along = m.b - m.a;
    if along.length_squared() < 1e-8 {
        return;
    }
    let dir = along.normalize();
    // (Its section's x: level and across it; y: up the section, in the plane of the
    // member and up. Upright members take z for up.)
    let up = if dir.y.abs() > 0.99 { Vec3::Z } else { Vec3::Y };
    let x0 = dir.cross(up).normalize();
    let y0 = x0.cross(dir);
    let (sr, cr) = m.roll.sin_cos();
    let (x, y) = (x0 * cr + y0 * sr, y0 * cr - x0 * sr);
    let c = tint(s);
    let sec = s.and_then(section).unwrap_or_else(|| Section { outer: ring(0.03).iter().map(|p| (*p, p.normalize(), p.normalize())).collect(), inner: None });
    let to = |p: Vec2| x * p.x + y * p.y;
    let n = sec.outer.len();
    // (The outside: per side, its own normals; a ring's are smooth.)
    for k in 0..n {
        let (p, na, _) = sec.outer[k];
        let (q, nb, _) = sec.outer[(k + 1) % n];
        let (na, nb) = if sec.outer.len() == 4 { (na, na) } else { (na, nb) };
        quad(out, [m.a + to(p), m.a + to(q), m.b + to(q), m.b + to(p)], [to(na), to(nb), to(nb), to(na)], c);
    }
    match &sec.inner {
        Some(bore) => {
            // (The bore, facing in, and the two ends as rings.)
            for k in 0..n {
                let (p, q) = (bore[k], bore[(k + 1) % n]);
                let (np, nq) = if n == 4 { let e = -(q - p).perp().normalize() * -1.0; (e, e) } else { (-p.normalize(), -q.normalize()) };
                quad(out, [m.a + to(q), m.a + to(p), m.b + to(p), m.b + to(q)], [to(nq), to(np), to(np), to(nq)], c);
                let (op, oq) = (sec.outer[k].0, sec.outer[(k + 1) % n].0);
                quad(out, [m.a + to(op), m.a + to(p), m.a + to(q), m.a + to(oq)], [-dir; 4], c);
                quad(out, [m.b + to(oq), m.b + to(q), m.b + to(p), m.b + to(op)], [dir; 4], c);
            }
        }
        None => {
            for k in 0..n {
                let (p, q) = (sec.outer[k].0, sec.outer[(k + 1) % n].0);
                out.push(face([m.a, m.a + to(q), m.a + to(p)], [-dir; 3], c));
                out.push(face([m.b, m.b + to(p), m.b + to(q)], [dir; 3], c));
            }
        }
    }
}

/// A node's faces: a ball of its forging's diameter.
fn node(out: &mut Vec<Face>, n: &Node) {
    let s = stock(n.stock);
    let r = s.and_then(|s| s.size.diameter).unwrap_or(0.1) as f32 * 0.5;
    let c = tint(s);
    let rows = ROUND / 2;
    let at = |i: usize, j: usize| {
        let (st, ct) = (i as f32 / rows as f32 * std::f32::consts::PI).sin_cos();
        let (sp, cp) = (j as f32 / ROUND as f32 * std::f32::consts::TAU).sin_cos();
        Vec3::new(st * cp, ct, st * sp)
    };
    for i in 0..rows {
        for j in 0..ROUND {
            let v = [at(i, j), at(i + 1, j), at(i + 1, j + 1), at(i, j + 1)];
            quad(out, v.map(|u| n.at + u * r), v, c);
        }
    }
}

/// A deck's faces: a slab as thick as its plate, its top at `top`.
fn deck(out: &mut Vec<Face>, d: &Deck) {
    let s = stock(d.stock);
    let t = s.and_then(|s| s.size.thickness).unwrap_or(0.01) as f32;
    let c = tint(s);
    let (lo, hi) = (Vec3::new(d.lo.x.min(d.hi.x), d.top - t, d.lo.y.min(d.hi.y)), Vec3::new(d.lo.x.max(d.hi.x), d.top, d.lo.y.max(d.hi.y)));
    let p = |x: bool, y: bool, z: bool| Vec3::new(if x { hi.x } else { lo.x }, if y { hi.y } else { lo.y }, if z { hi.z } else { lo.z });
    let (f, t_) = (false, true);
    quad(out, [p(f, t_, f), p(f, t_, t_), p(t_, t_, t_), p(t_, t_, f)], [Vec3::Y; 4], c);
    quad(out, [p(f, f, f), p(t_, f, f), p(t_, f, t_), p(f, f, t_)], [Vec3::NEG_Y; 4], c);
    quad(out, [p(f, f, f), p(f, f, t_), p(f, t_, t_), p(f, t_, f)], [Vec3::NEG_X; 4], c);
    quad(out, [p(t_, f, f), p(t_, t_, f), p(t_, t_, t_), p(t_, f, t_)], [Vec3::X; 4], c);
    quad(out, [p(f, f, f), p(f, t_, f), p(t_, t_, f), p(t_, f, f)], [Vec3::NEG_Z; 4], c);
    quad(out, [p(f, f, t_), p(t_, f, t_), p(t_, t_, t_), p(f, t_, t_)], [Vec3::Z; 4], c);
}

/// The faces of a frame's members and nodes, and of its decks.
pub fn faces(members: &[Member], nodes: &[Node], decks: &[Deck]) -> Vec<Face> {
    let mut out = Vec::new();
    members.iter().for_each(|m| member(&mut out, m));
    nodes.iter().for_each(|n| node(&mut out, n));
    decks.iter().for_each(|d| deck(&mut out, d));
    out
}

/// The same as one drawable structure (none: nothing to draw).
pub fn placed(members: &[Member], nodes: &[Node], decks: &[Deck]) -> Option<Placed> {
    let f = faces(members, nodes, decks);
    (!f.is_empty()).then(|| Placed::structure(f))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_tube_is_drawn_round_and_hollow_and_a_box_square() {
        let reg = universe_sim::world::registry::registry();
        let tube = reg.stock.iter().find(|s| s.identity.form == "tube" && s.size.wall.is_some()).unwrap();
        let boxed = reg.stock.iter().find(|s| s.identity.form == "box").unwrap();
        fn m(k: &str) -> Member<'_> { Member { a: Vec3::ZERO, b: Vec3::new(2.0, 0.0, 0.0), stock: k, roll: 0.0 } }
        let t = faces(&[m(&tube.identity.key)], &[], &[]);
        // (Outside, bore, two rings: four quads a side.)
        assert_eq!(t.len(), ROUND * 4 * 2);
        let r = tube.size.diameter.unwrap() as f32 * 0.5;
        let widest = t.iter().flat_map(|f| f.positions).map(|p| Vec2::new(p.y, p.z).length()).fold(0.0, f32::max);
        assert!((widest - r).abs() < 1e-4);
        let b = faces(&[m(&boxed.identity.key)], &[], &[]);
        let h = boxed.size.height.unwrap() as f32 * 0.5;
        let top = b.iter().flat_map(|f| f.positions).map(|p| p.y).fold(0.0, f32::max);
        assert!((top - h).abs() < 1e-4, "a level box stands its height up");
    }
}
