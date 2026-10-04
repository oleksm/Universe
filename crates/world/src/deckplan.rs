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
/// A ladder's hatch, square (m).
pub const HATCH: f64 = 0.9;
/// A stair's width (m), and the most its steps rise (m).
pub const STAIR_WIDTH: f64 = 1.0;
const STAIR_RISE: f64 = 0.22;
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
    /// Ladders up to the deck above (each through a hatch in its floor).
    #[serde(default)]
    pub ladders: Vec<Ladder>,
    /// Stairs up to the deck above (each through an opening in its floor).
    #[serde(default)]
    pub stairs: Vec<Stair>,
}

impl Deck {
    pub fn at(floor: f64) -> Self {
        Deck { floor, headroom: HEADROOM, planes: Vec::new(), walls: Vec::new(), ladders: Vec::new(), stairs: Vec::new() }
    }
}

/// A ladder at `at`, up to the deck above.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Ladder {
    pub at: DVec2,
}

impl Ladder {
    /// Its hatch, as an outline.
    pub fn outline(&self) -> Vec<DVec2> {
        let h = HATCH / 2.0;
        vec![self.at + DVec2::new(-h, -h), self.at + DVec2::new(h, -h), self.at + DVec2::new(h, h), self.at + DVec2::new(-h, h)]
    }
}

/// A stair from its foot `from` to its head `to` (along the floor), `width`
/// across, up to the deck above.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Stair {
    pub from: DVec2,
    pub to: DVec2,
    pub width: f64,
}

impl Stair {
    /// Its footprint (and the opening above it), as an outline.
    pub fn outline(&self) -> Vec<DVec2> {
        let side = (self.to - self.from).perp().normalize_or_zero() * (self.width / 2.0);
        vec![self.from - side, self.to - side, self.to + side, self.from + side]
    }
}

/// The openings in deck `d`'s floor: the hatches and stairwells of the deck below.
pub fn openings(plan: &DeckPlan, d: usize) -> Vec<Vec<DVec2>> {
    let Some(below) = d.checked_sub(1).and_then(|k| plan.decks.get(k)) else { return Vec::new() };
    below.ladders.iter().map(Ladder::outline).chain(below.stairs.iter().map(Stair::outline)).collect()
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

/// A floor plane trimmed to the hull's sides, `holes` cut out of it (outlines):
/// strips (z from, z to, x from, x to).
pub fn floor_strips(poly: &[DVec2], sides: &Sides, holes: &[Vec<DVec2>]) -> Vec<(f64, f64, f64, f64)> {
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
                // (Less the holes crossing this strip.)
                let mut spans = vec![(x0, x1)];
                for hole in holes {
                    for (h0, h1) in crossings(hole, mid) {
                        spans = spans.into_iter().flat_map(|(s0, s1)| [(s0, s1.min(h0)), (s0.max(h1), s1)]).filter(|(s0, s1)| s1 > s0).collect();
                    }
                }
                for (x0, x1) in spans {
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

/// A floor carved to fill the space around `at`: within the hull's sides and the
/// walls (closed in by them, as far as it reaches), run on a little under the
/// walls so floors either side of one meet; its outline, smoothed. None if `at`
/// is outside the hull or on a wall.
pub fn carve(sides: &Sides, walls: &[Wall], at: DVec2) -> Option<Vec<DVec2>> {
    const CELL: f64 = 0.1;
    let rows: Vec<(f64, f64, f64)> = sides.rows().collect();
    let (z0, z1) = (rows.first()?.0, rows.last()?.0);
    let (x0, x1) = rows.iter().fold((f64::MAX, f64::MIN), |m, r| (m.0.min(r.1), m.1.max(r.2)));
    let (nx, nz) = (((x1 - x0) / CELL).ceil() as usize + 3, ((z1 - z0) / CELL).ceil() as usize + 3);
    let centre = |i: usize, j: usize| DVec2::new(x0 + (i as f64 - 1.0 + 0.5) * CELL, z0 + (j as f64 - 1.0 + 0.5) * CELL);
    // Each wall as short pieces, to keep clear of.
    let pieces: Vec<(DVec2, DVec2)> = walls.iter().flat_map(|w| w.path().windows(2).map(|p| (p[0].0, p[1].0)).collect::<Vec<_>>()).collect();
    let near_wall = |p: DVec2| {
        pieces.iter().any(|&(a, b)| {
            let s = ((p - a).dot(b - a) / (b - a).length_squared().max(1e-12)).clamp(0.0, 1.0);
            a.lerp(b, s).distance(p) < WALL / 2.0 + CELL * 0.5
        })
    };
    let open: Vec<bool> = (0..nz).flat_map(|j| (0..nx).map(move |i| (i, j))).map(|(i, j)| {
        let p = centre(i, j);
        sides.contains(p) && !near_wall(p)
    }).collect();
    let start = (((at.x - x0) / CELL).floor() as isize + 1, ((at.y - z0) / CELL).floor() as isize + 1);
    if start.0 < 0 || start.1 < 0 || start.0 as usize >= nx || start.1 as usize >= nz || !open[start.1 as usize * nx + start.0 as usize] {
        return None;
    }
    // Flooded from there.
    let mut fill = vec![false; nx * nz];
    let mut todo = vec![(start.0 as usize, start.1 as usize)];
    fill[start.1 as usize * nx + start.0 as usize] = true;
    while let Some((i, j)) = todo.pop() {
        for (di, dj) in [(1isize, 0isize), (-1, 0), (0, 1), (0, -1)] {
            let (a, b) = (i as isize + di, j as isize + dj);
            if a < 0 || b < 0 || a as usize >= nx || b as usize >= nz {
                continue;
            }
            let k = b as usize * nx + a as usize;
            if open[k] && !fill[k] {
                fill[k] = true;
                todo.push((a as usize, b as usize));
            }
        }
    }
    // Run on under the walls (two cells), so floors either side of one meet.
    for _ in 0..2 {
        let was = fill.clone();
        for j in 1..nz - 1 {
            for i in 1..nx - 1 {
                if !was[j * nx + i] && (was[j * nx + i - 1] || was[j * nx + i + 1] || was[(j - 1) * nx + i] || was[(j + 1) * nx + i]) {
                    fill[j * nx + i] = true;
                }
            }
        }
    }
    // Its edges, each cell's sides facing out, wound round it; chained into loops.
    let corner = |i: usize, j: usize| (i, j);
    let mut next: std::collections::HashMap<(usize, usize), (usize, usize)> = std::collections::HashMap::new();
    let filled = |i: isize, j: isize| i >= 0 && j >= 0 && (i as usize) < nx && (j as usize) < nz && fill[j as usize * nx + i as usize];
    for j in 0..nz {
        for i in 0..nx {
            if !fill[j * nx + i] {
                continue;
            }
            let (ii, jj) = (i as isize, j as isize);
            if !filled(ii, jj - 1) {
                next.insert(corner(i, j), corner(i + 1, j));
            }
            if !filled(ii + 1, jj) {
                next.insert(corner(i + 1, j), corner(i + 1, j + 1));
            }
            if !filled(ii, jj + 1) {
                next.insert(corner(i + 1, j + 1), corner(i, j + 1));
            }
            if !filled(ii - 1, jj) {
                next.insert(corner(i, j + 1), corner(i, j));
            }
        }
    }
    let point = |c: (usize, usize)| DVec2::new(x0 + (c.0 as f64 - 1.0) * CELL, z0 + (c.1 as f64 - 1.0) * CELL);
    let mut best: Vec<DVec2> = Vec::new();
    let mut best_area = 0.0;
    while let Some(&first) = next.keys().next() {
        let mut lp = Vec::new();
        let mut c = first;
        while let Some(n) = next.remove(&c) {
            lp.push(point(c));
            c = n;
        }
        let area = (0..lp.len()).map(|k| lp[k].perp_dot(lp[(k + 1) % lp.len()])).sum::<f64>().abs() / 2.0;
        if area > best_area {
            best_area = area;
            best = lp;
        }
    }
    (best.len() >= 3).then(|| simplify(&best, CELL * 0.8))
}

/// A closed outline with points that hardly matter dropped (Douglas–Peucker,
/// within `tolerance`): a staircase of cells becomes its slope.
fn simplify(lp: &[DVec2], tolerance: f64) -> Vec<DVec2> {
    fn dp(pts: &[DVec2], tol: f64, out: &mut Vec<DVec2>) {
        let (a, b) = (pts[0], pts[pts.len() - 1]);
        let (k, d) = pts.iter().enumerate().skip(1).take(pts.len().saturating_sub(2)).map(|(k, p)| {
            let ab = b - a;
            let d = if ab.length_squared() < 1e-12 { p.distance(a) } else { (p - a).perp_dot(ab).abs() / ab.length() };
            (k, d)
        }).fold((0, 0.0), |m, x| if x.1 > m.1 { x } else { m });
        if d > tol {
            dp(&pts[..=k], tol, out);
            out.pop();
            dp(&pts[k..], tol, out);
        } else {
            out.push(a);
            out.push(b);
        }
    }
    // (Split at the point farthest from the first, so the closed loop is two open runs.)
    let far = (0..lp.len()).max_by(|&a, &b| lp[a].distance(lp[0]).total_cmp(&lp[b].distance(lp[0]))).unwrap_or(0);
    let mut out = Vec::new();
    let first: Vec<DVec2> = lp[..=far].to_vec();
    let mut second: Vec<DVec2> = lp[far..].to_vec();
    second.push(lp[0]);
    dp(&first, tolerance, &mut out);
    out.pop();
    dp(&second, tolerance, &mut out);
    out.pop();
    out
}

/// What a plan builds: for walking on (triangles) and drawing (each a
/// rectangle, its corners in order, and whether it's floor (or tread) or
/// wall), and where one climbs (ladders: boxes, low and high corners).
#[derive(Clone, Debug, Default)]
pub struct Built {
    pub panels: Vec<([DVec3; 4], bool)>,
    pub climbs: Vec<(DVec3, DVec3)>,
}

impl Built {
    pub fn triangles(&self) -> Vec<[DVec3; 3]> {
        self.panels.iter().flat_map(|(q, _)| [[q[0], q[1], q[2]], [q[0], q[2], q[3]]]).collect()
    }
}

/// A plan built to walk in: its surfaces, and where one climbs.
#[derive(Debug)]
pub struct Walkable {
    pub mesh: crate::walk::WalkMesh,
    pub climbs: Vec<(DVec3, DVec3)>,
}

impl Walkable {
    /// Is `p` where one climbs (on a ladder)?
    pub fn climbing(&self, p: DVec3) -> bool {
        self.climbs.iter().any(|(lo, hi)| p.cmpge(*lo).all() && p.cmple(*hi).all())
    }
}

impl From<&Built> for Walkable {
    fn from(b: &Built) -> Self {
        Walkable { mesh: crate::walk::WalkMesh::new(&b.triangles()), climbs: b.climbs.clone() }
    }
}

/// A plan built, its decks trimmed by `sides` (one for each deck, as the hull is at that height).
pub fn build(plan: &DeckPlan, sides: &[Sides]) -> Built {
    let mut b = Built::default();
    for (d, deck) in plan.decks.iter().enumerate() {
        let Some(sd) = sides.get(d) else { continue };
        let y = deck.floor;
        let holes = openings(plan, d);
        for poly in &deck.planes {
            for (z0, z1, x0, x1) in floor_strips(poly, sd, &holes) {
                b.panels.push(([DVec3::new(x0, y, z0), DVec3::new(x1, y, z0), DVec3::new(x1, y, z1), DVec3::new(x0, y, z1)], true));
            }
        }
        for wall in &deck.walls {
            for run in wall_runs(wall, sd) {
                // Its pieces (a doorway: the wall only above it), those running on
                // straight and alike as one.
                let mut pieces: Vec<(DVec2, DVec2, f64)> = Vec::new();
                for w in run.windows(2) {
                    let ((a, sa), (c, sc)) = (w[0], w[1]);
                    let mid = (sa + sc) / 2.0;
                    let bottom = wall.doors.iter().find(|dr| (mid - dr.at).abs() <= dr.width / 2.0).map_or(0.0, |dr| dr.height.min(deck.headroom));
                    if let Some(last) = pieces.last_mut()
                        && last.2 == bottom
                        && (last.1 - last.0).normalize_or_zero().dot((c - a).normalize_or_zero()) > 0.99995
                    {
                        last.1 = c;
                        continue;
                    }
                    pieces.push((a, c, bottom));
                }
                for (a, c, bottom) in pieces {
                    let dir = (c - a).normalize_or_zero();
                    let off = DVec2::new(-dir.y, dir.x) * (WALL / 2.0);
                    for side in [off, -off] {
                        let (p, q) = (a + side, c + side);
                        b.panels.push(([DVec3::new(p.x, y + bottom, p.y), DVec3::new(q.x, y + bottom, q.y), DVec3::new(q.x, y + deck.headroom, q.y), DVec3::new(p.x, y + deck.headroom, p.y)], false));
                    }
                }
            }
        }
        // Up to the deck above (none: nowhere to go).
        let Some(above) = plan.decks.get(d + 1) else { continue };
        let top = above.floor;
        for l in &deck.ladders {
            // Climbed from the floor to a step out over the hatch; its rails and rungs on the far side.
            let h = HATCH / 2.0;
            b.climbs.push((DVec3::new(l.at.x - h, y, l.at.y - h), DVec3::new(l.at.x + h, top + 1.5, l.at.y + h)));
            let back = l.at.y + h;
            for x in [l.at.x - 0.25, l.at.x + 0.2] {
                b.panels.push(([DVec3::new(x, y, back), DVec3::new(x + 0.05, y, back), DVec3::new(x + 0.05, top + 1.0, back), DVec3::new(x, top + 1.0, back)], false));
            }
            let mut ry = y + 0.3;
            while ry < top + 0.9 {
                b.panels.push(([DVec3::new(l.at.x - 0.25, ry, back), DVec3::new(l.at.x + 0.25, ry, back), DVec3::new(l.at.x + 0.25, ry + 0.04, back), DVec3::new(l.at.x - 0.25, ry + 0.04, back)], false));
                ry += 0.3;
            }
        }
        for st in &deck.stairs {
            // Steps of equal rise and run from the foot to the head, each a tread and its riser.
            let run = st.to - st.from;
            let rise = top - y;
            if run.length() < 0.3 || rise <= 0.0 {
                continue;
            }
            let n = (rise / STAIR_RISE).ceil().max(1.0) as usize;
            let side = run.perp().normalize_or_zero() * (st.width / 2.0);
            for k in 0..n {
                let (a, c) = (st.from + run * (k as f64 / n as f64), st.from + run * ((k + 1) as f64 / n as f64));
                let (lo, hi) = (y + rise * k as f64 / n as f64, y + rise * (k + 1) as f64 / n as f64);
                let p = |q: DVec2, h: f64| DVec3::new(q.x, h, q.y);
                b.panels.push(([p(a - side, lo), p(a + side, lo), p(a + side, hi), p(a - side, hi)], false));
                b.panels.push(([p(a - side, hi), p(a + side, hi), p(c + side, hi), p(c - side, hi)], true));
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
        let floor = [DVec2::new(-5.0, -2.0), DVec2::new(5.0, -2.0), DVec2::new(5.0, 2.0), DVec2::new(-5.0, 2.0)];
        let area = |holes: &[Vec<DVec2>]| floor_strips(&floor, &sides, holes).iter().map(|s| (s.1 - s.0) * (s.3 - s.2)).sum::<f64>();
        assert!((area(&[]) - 24.0).abs() < 0.3, "{}", area(&[]));
        // A ladder's hatch through it: 0.81 m² less.
        let hatch = Ladder { at: DVec2::ZERO }.outline();
        assert!((area(&[]) - area(&[hatch]) - HATCH * HATCH).abs() < 0.1);
        // A floor carved round a point: the whole hull (60 m²), or with the wall across
        // it, the half the point's in (30), each run on under the wall a little.
        let carved = |walls: &[Wall], at: DVec2| {
            let poly = carve(&sides, walls, at).expect("a floor");
            floor_strips(&poly, &sides, &[]).iter().map(|s| (s.1 - s.0) * (s.3 - s.2)).sum::<f64>()
        };
        assert!((carved(&[], DVec2::new(0.0, 3.0)) - 60.0).abs() < 1.5, "{}", carved(&[], DVec2::new(0.0, 3.0)));
        let across = Wall { points: vec![DVec2::new(-5.0, 0.0), DVec2::new(5.0, 0.0)], ..Default::default() };
        let half = carved(&[across], DVec2::new(0.0, 3.0));
        assert!((half - 30.0).abs() < 1.5, "{half}");
        // A wall across, 10 m: kept inside, 6 m of it.
        let wall = Wall { points: vec![DVec2::new(-5.0, 0.0), DVec2::new(5.0, 0.0)], ..Default::default() };
        let runs = wall_runs(&wall, &sides);
        let kept: f64 = runs.iter().map(|r| r.last().unwrap().1 - r[0].1).sum();
        assert!((kept - 6.0).abs() < 0.25, "{kept}");
    }

    #[test]
    fn a_stair_is_walked_up_and_a_ladder_climbed_to_the_deck_above() {
        use crate::walk::{Collider, Stride, Walker};
        // A hull 20 m square; two decks 3 m apart, each a floor across it; a stair 6 m
        // long and a ladder from the lower to the upper.
        let c = [DVec2::new(-10.0, -10.0), DVec2::new(10.0, -10.0), DVec2::new(10.0, 10.0), DVec2::new(-10.0, 10.0)];
        let section: Vec<[DVec2; 2]> = (0..4).map(|i| [c[i], c[(i + 1) % 4]]).collect();
        let sides = [Sides::of(&section), Sides::of(&section)];
        let floor = c.to_vec();
        let mut lower = Deck::at(0.0);
        lower.planes.push(floor.clone());
        lower.stairs.push(Stair { from: DVec2::new(0.0, 6.0), to: DVec2::new(0.0, 0.0), width: STAIR_WIDTH });
        lower.ladders.push(Ladder { at: DVec2::new(5.0, 0.0) });
        let mut upper = Deck::at(3.0);
        upper.planes.push(floor);
        let plan = DeckPlan { hull: "test".into(), decks: vec![lower, upper] };
        let walk = Walkable::from(&build(&plan, &sides));
        let cols = [Collider::Mesh { mesh: &walk.mesh, at: DVec3::ZERO, rot: glam::DQuat::IDENTITY }];
        let go = |from: DVec3, wish: DVec3, climb: f64, secs: f64| {
            let mut w = Walker { feet: from, velocity: DVec3::ZERO };
            for _ in 0..(secs * 60.0) as usize {
                w.step(&cols, &|_| DVec3::Y, crate::units::STANDARD_GRAVITY, &|p| walk.climbing(p), &Stride { wish, jump: 0.0, climb }, 1.0 / 60.0);
            }
            w.feet
        };
        // Up the stair (walking from its foot toward its head, -z): on the upper floor past it.
        let top = go(DVec3::new(0.0, 0.0, 8.0), DVec3::new(0.0, 0.0, -1.6), 0.0, 8.0);
        assert!((top.y - 3.0).abs() < 0.05 && top.z < 0.0, "{top:?}");
        // Up the ladder, then a step off it: on the upper floor.
        let mut at = go(DVec3::new(5.0, 0.0, 0.0), DVec3::ZERO, 1.0, 5.0);
        at = go(at, DVec3::new(-1.0, 0.0, 0.0), 0.0, 1.5);
        assert!((at.y - 3.0).abs() < 0.05, "{at:?}");
    }
}
