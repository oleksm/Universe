//! Deck plans: a ship's inside as its owner lays it out in the shipyard's
//! layout studio. Decks at heights, each with floor planes (outlines) and
//! walls (chains of points, any segment bent into an arc), doors in walls.
//! Floors and walls are trimmed to the hull: they go only where its sides
//! are, row by row, so they follow its curves.
//!
//! All in metres in the hull's own frame (−Z forward, +Y up, X starboard):
//! a plan point is (x, z), a deck's floor a height y.

use glam::{DVec2, DVec3};
use serde::{Deserialize, Serialize};

/// A wall's thickness (m).
pub const WALL: f64 = 0.12;
/// A door's opening (m).
pub const DOOR_WIDTH: f64 = 0.9;
pub const DOOR_HEIGHT: f64 = 2.1;
/// A new deck's headroom (m).
pub const HEADROOM: f64 = 2.6;
/// How finely floors are trimmed to the hull and arcs are drawn (m).
const STEP: f64 = 0.1;

/// A hull's inside as laid out.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct DeckPlan {
    /// The hull it's for (its content key).
    pub hull: String,
    pub decks: Vec<Deck>,
}

/// A level: its floor's height and headroom, its floors and walls.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Deck {
    pub floor: f64,
    pub headroom: f64,
    /// Floor planes: outlines (x, z), each closed.
    pub planes: Vec<Vec<DVec2>>,
    pub walls: Vec<Wall>,
}

impl Deck {
    pub fn at(floor: f64) -> Self {
        Deck { floor, headroom: HEADROOM, planes: Vec::new(), walls: Vec::new() }
    }
}

/// A wall along `points`; segment k bent by `bulges[k]` (the arc's middle
/// that far off the straight line, to the left going along; 0 straight).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Wall {
    pub points: Vec<DVec2>,
    pub bulges: Vec<f64>,
    pub doors: Vec<Door>,
}

/// A doorway `at` metres along its wall (its middle), `width` across, `height` tall.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Door {
    pub at: f64,
    pub width: f64,
    pub height: f64,
}

impl Wall {
    /// Segment k's bulge (0 if none given).
    pub fn bulge(&self, k: usize) -> f64 {
        self.bulges.get(k).copied().unwrap_or(0.0)
    }

    /// Its line, finely: points along it (arcs drawn as many short pieces),
    /// each with how far along the wall it is.
    pub fn path(&self) -> Vec<(DVec2, f64)> {
        let mut out: Vec<(DVec2, f64)> = Vec::new();
        let mut along = 0.0;
        for k in 0..self.points.len().saturating_sub(1) {
            let seg = arc(self.points[k], self.points[k + 1], self.bulge(k));
            for (i, p) in seg.into_iter().enumerate() {
                if i == 0 && !out.is_empty() {
                    continue;
                }
                if let Some(&(q, _)) = out.last() {
                    along += p.distance(q);
                }
                out.push((p, along));
            }
        }
        out
    }

    /// How long it is (m).
    pub fn length(&self) -> f64 {
        self.path().last().map_or(0.0, |p| p.1)
    }

    /// The point `along` metres along it.
    pub fn point_at(&self, along: f64) -> Option<DVec2> {
        let path = self.path();
        path.windows(2).find(|w| along >= w[0].1 && along <= w[1].1).map(|w| {
            let s = (along - w[0].1) / (w[1].1 - w[0].1).max(1e-9);
            w[0].0.lerp(w[1].0, s)
        })
    }

    /// How far along it the point nearest `p` is, and how far off it `p` is.
    pub fn nearest(&self, p: DVec2) -> Option<(f64, f64)> {
        let path = self.path();
        path.windows(2)
            .map(|w| {
                let (a, b) = (w[0].0, w[1].0);
                let s = ((p - a).dot(b - a) / (b - a).length_squared().max(1e-12)).clamp(0.0, 1.0);
                let q = a.lerp(b, s);
                (w[0].1 + s * (w[1].1 - w[0].1), q.distance(p))
            })
            .min_by(|x, y| x.1.total_cmp(&y.1))
    }
}

/// The segment from `a` to `b` bent by `bulge` (see `Wall`), as points.
pub fn arc(a: DVec2, b: DVec2, bulge: f64) -> Vec<DVec2> {
    let chord = b - a;
    let len = chord.length();
    if bulge.abs() < 1e-3 || len < 1e-6 {
        return vec![a, b];
    }
    // (The sagitta s over a chord L: radius (L²/4 + s²) / 2s, turning through 4·atan(2s/L).)
    let left = DVec2::new(-chord.y, chord.x) / len;
    let r = (len * len / 4.0 + bulge * bulge) / (2.0 * bulge.abs());
    let centre = (a + b) * 0.5 + left * (bulge - bulge.signum() * r);
    let turn = -4.0 * (2.0 * bulge / len).atan();
    let n = ((r * turn.abs()) / STEP).ceil().max(2.0) as usize;
    (0..=n).map(|i| centre + DVec2::from_angle(turn * i as f64 / n as f64).rotate(a - centre)).collect()
}

/// Where a hull's sides are at one height: for each row along it (z, every
/// `STEP`), from its leftmost to its rightmost surface there.
#[derive(Clone, Debug, Default)]
pub struct Sides {
    z0: f64,
    rows: Vec<Option<(f64, f64)>>,
}

impl Sides {
    /// From a cross-section (segments in x, z: see `WalkMesh::section_y`).
    pub fn of(section: &[[DVec2; 2]]) -> Self {
        let (lo, hi) = section.iter().flatten().fold((f64::MAX, f64::MIN), |m, p| (m.0.min(p.y), m.1.max(p.y)));
        if lo > hi {
            return Sides::default();
        }
        let n = ((hi - lo) / STEP).ceil() as usize + 1;
        let mut rows = vec![None; n];
        for [a, b] in section {
            let (za, zb) = (a.y.min(b.y), a.y.max(b.y));
            let (r0, r1) = (((za - lo) / STEP).ceil() as usize, ((zb - lo) / STEP).floor() as usize);
            for (r, row) in rows.iter_mut().enumerate().take(r1.min(n - 1) + 1).skip(r0) {
                let z = lo + r as f64 * STEP;
                let x = if (b.y - a.y).abs() < 1e-9 { a.x } else { a.x + (b.x - a.x) * (z - a.y) / (b.y - a.y) };
                let e: &mut (f64, f64) = row.get_or_insert((x, x));
                *e = (e.0.min(x), e.1.max(x));
            }
        }
        Sides { z0: lo, rows }
    }

    /// From its leftmost to its rightmost surface at `z`, if it's there.
    pub fn span(&self, z: f64) -> Option<(f64, f64)> {
        let r = ((z - self.z0) / STEP).round();
        if r < 0.0 {
            return None;
        }
        *self.rows.get(r as usize)?
    }

    /// Is `p` (x, z) between its sides?
    pub fn contains(&self, p: DVec2) -> bool {
        self.span(p.y).is_some_and(|(a, b)| p.x >= a && p.x <= b)
    }

    /// Its rows: (z, from, to).
    pub fn rows(&self) -> impl Iterator<Item = (f64, f64, f64)> + '_ {
        self.rows.iter().enumerate().filter_map(|(r, s)| s.map(|(a, b)| (self.z0 + r as f64 * STEP, a, b)))
    }
}

/// Where outline `poly` (x, z) is crossed at `z`: its spans inside (pairs of x).
fn crossings(poly: &[DVec2], z: f64) -> Vec<(f64, f64)> {
    let mut xs = Vec::new();
    for i in 0..poly.len() {
        let (a, b) = (poly[i], poly[(i + 1) % poly.len()]);
        if (a.y <= z) != (b.y <= z) {
            xs.push(a.x + (b.x - a.x) * (z - a.y) / (b.y - a.y));
        }
    }
    xs.sort_by(f64::total_cmp);
    xs.as_chunks::<2>().0.iter().map(|c| (c[0], c[1])).collect()
}

/// A floor plane trimmed to the hull's sides: strips (z from, z to, x from, x to).
pub fn floor_strips(poly: &[DVec2], sides: &Sides) -> Vec<(f64, f64, f64, f64)> {
    if poly.len() < 3 {
        return Vec::new();
    }
    let (lo, hi) = poly.iter().fold((f64::MAX, f64::MIN), |m, p| (m.0.min(p.y), m.1.max(p.y)));
    let mut out = Vec::new();
    let mut z = (lo / STEP).floor() * STEP;
    while z < hi {
        let mid = z + STEP / 2.0;
        if let Some((a, b)) = sides.span(mid) {
            for (x0, x1) in crossings(poly, mid) {
                let (x0, x1) = (x0.max(a), x1.min(b));
                if x1 > x0 {
                    out.push((z.max(lo), (z + STEP).min(hi), x0, x1));
                }
            }
        }
        z += STEP;
    }
    out
}

/// A wall's pieces inside the hull, each a run of its line: [(point, along)].
pub fn wall_runs(wall: &Wall, sides: &Sides) -> Vec<Vec<(DVec2, f64)>> {
    let mut runs: Vec<Vec<(DVec2, f64)>> = Vec::new();
    let mut open = false;
    let path = wall.path();
    // (Finely, so a wall stops close to where it meets the hull.)
    for w in path.windows(2) {
        let n = (w[0].0.distance(w[1].0) / STEP).ceil().max(1.0) as usize;
        for i in 0..=n {
            let s = i as f64 / n as f64;
            let p = (w[0].0.lerp(w[1].0, s), w[0].1 + s * (w[1].1 - w[0].1));
            if sides.contains(p.0) {
                if !open {
                    runs.push(Vec::new());
                    open = true;
                }
                let run = runs.last_mut().expect("a run");
                if run.last().is_none_or(|q: &(DVec2, f64)| q.0.distance(p.0) > 1e-9) {
                    run.push(p);
                }
            } else {
                open = false;
            }
        }
    }
    runs.retain(|r| r.len() >= 2);
    runs
}

/// What a plan builds: for walking on (triangles) and drawing (each a
/// rectangle, its corners in order, and whether it's floor or wall).
#[derive(Clone, Debug, Default)]
pub struct Built {
    pub panels: Vec<([DVec3; 4], bool)>,
}

impl Built {
    pub fn triangles(&self) -> Vec<[DVec3; 3]> {
        self.panels.iter().flat_map(|(q, _)| [[q[0], q[1], q[2]], [q[0], q[2], q[3]]]).collect()
    }
}

/// A plan built, its decks trimmed by `sides` (one for each deck, as the hull is at that height).
pub fn build(plan: &DeckPlan, sides: &[Sides]) -> Built {
    let mut b = Built::default();
    for (d, deck) in plan.decks.iter().enumerate() {
        let Some(sd) = sides.get(d) else { continue };
        let y = deck.floor;
        for poly in &deck.planes {
            for (z0, z1, x0, x1) in floor_strips(poly, sd) {
                b.panels.push(([DVec3::new(x0, y, z0), DVec3::new(x1, y, z0), DVec3::new(x1, y, z1), DVec3::new(x0, y, z1)], true));
            }
        }
        for wall in &deck.walls {
            for run in wall_runs(wall, sd) {
                for w in run.windows(2) {
                    let ((a, sa), (c, sc)) = (w[0], w[1]);
                    // A doorway here: the wall only above it.
                    let mid = (sa + sc) / 2.0;
                    let bottom = wall.doors.iter().find(|dr| (mid - dr.at).abs() <= dr.width / 2.0).map_or(0.0, |dr| dr.height.min(deck.headroom));
                    let dir = (c - a).normalize_or_zero();
                    let off = DVec2::new(-dir.y, dir.x) * (WALL / 2.0);
                    for side in [off, -off] {
                        let (p, q) = (a + side, c + side);
                        b.panels.push(([DVec3::new(p.x, y + bottom, p.y), DVec3::new(q.x, y + bottom, q.y), DVec3::new(q.x, y + deck.headroom, q.y), DVec3::new(p.x, y + deck.headroom, p.y)], false));
                    }
                }
            }
        }
    }
    b
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_arc_bulges_by_its_sagitta_and_a_floor_and_wall_stop_at_the_hull() {
        // A segment 4 m long bent 1 m to the left: its middle 1 m off the chord.
        let pts = arc(DVec2::new(0.0, 0.0), DVec2::new(4.0, 0.0), 1.0);
        let mid = pts[pts.len() / 2];
        assert!((mid - DVec2::new(2.0, 1.0)).length() < 0.05, "{mid:?}");
        assert!(pts.last().unwrap().distance(DVec2::new(4.0, 0.0)) < 1e-9);
        // A hull 6 m wide (x -3..3) from z -5 to 5: a square of segments.
        let c = [DVec2::new(-3.0, -5.0), DVec2::new(3.0, -5.0), DVec2::new(3.0, 5.0), DVec2::new(-3.0, 5.0)];
        let section: Vec<[DVec2; 2]> = (0..4).map(|i| [c[i], c[(i + 1) % 4]]).collect();
        let sides = Sides::of(&section);
        // A floor drawn 10 m wide: trimmed to 6.
        let strips = floor_strips(&[DVec2::new(-5.0, -2.0), DVec2::new(5.0, -2.0), DVec2::new(5.0, 2.0), DVec2::new(-5.0, 2.0)], &sides);
        let area: f64 = strips.iter().map(|s| (s.1 - s.0) * (s.3 - s.2)).sum();
        assert!((area - 24.0).abs() < 0.3, "{area}");
        // A wall across, 10 m: kept inside, 6 m of it.
        let wall = Wall { points: vec![DVec2::new(-5.0, 0.0), DVec2::new(5.0, 0.0)], ..Default::default() };
        let runs = wall_runs(&wall, &sides);
        let kept: f64 = runs.iter().map(|r| r.last().unwrap().1 - r[0].1).sum();
        assert!((kept - 6.0).abs() < 0.25, "{kept}");
    }
}
