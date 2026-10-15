//! Excise the contact footprint from the actual resident patches, and lock what must stay still.
//!
//! The band (`sparse_band`) covers the chart square `[-HALF, HALF]²` exactly; its outer edge lies
//! on the retained triangles as they stood at the job's eye. Every resident triangle inside the
//! square is dropped, every crossed one keeps only its part outside, and each skirt along an edge
//! inside the square goes too. The vertices those parts use stop geomorphing (their delta set to
//! zero, their size kept), and so does every vertex another patch's edge depends on through them:
//! equal points on neighbouring patches, and the fine points along a coarser neighbour's segment.
//! The closure stops at vertices that never move (zero delta). Nothing absent is synthesised: a
//! dependency on a patch not supplied, or across a cube face, refuses the whole group.
//!
//! Checked before anything is returned: the footprint's area is covered once, the exterior meets
//! the band's outer edge point for point within `SEAM`, and no seam between patches is wider at
//! any of several eyes than the original renderer's at that eye or at the job's.
use crate::{
    boundary_correction::retained::{RetainedPatch, morphed},
    contact_core::{Chart, grid::Triangle},
    crease::{Tile, area, rect},
    sparse_band::crossing,
};
use glam::{DVec2, DVec3, Vec3};
use serde_json::{Value, json};
use std::collections::{BTreeSet, HashMap, HashSet};

/// Half the footprint (m, chart units).
pub const HALF: f64 = 64.;
/// The declared inter-representation seam budget (m).
pub const SEAM: f64 = 1e-5;
/// Patches one group may change, at most.
const MAX_PATCHES: usize = 96;
/// Grid vertices a patch side (17) and a patch (289).
const SIDE: u32 = 17;
const GRID_VERTICES: usize = 289;
const GRID_FACES: usize = 512;

/// One patch as it must be drawn instead of the resident one.
pub struct Replacement {
    pub key: [u32; 4],
    pub mesh_id: u64,
    pub origin: [f64; 3],
    pub positions: Vec<[f32; 3]>,
    pub morph: Vec<[f32; 4]>,
    /// Each vertex's attributes: the resident vertices (up to three) and weights to blend.
    pub blend: Vec<[(u32, f32); 3]>,
    /// The ground's triangles first (`ground` of them), then the skirts'.
    pub faces: Vec<[u32; 3]>,
    pub ground: usize,
}

type V = (usize, u32);

fn step(key: [u32; 4]) -> u32 {
    1 << (19 - key[1])
}
fn tag(key: [u32; 4], i: u32) -> [u32; 3] {
    let s = step(key);
    [key[0], (key[2] * 16 + i % SIDE) * s, (key[3] * 16 + i / SIDE) * s]
}
fn local(key: [u32; 4], t: [u32; 3]) -> Option<u32> {
    let s = step(key);
    if t[0] != key[0] || t[1] % s != 0 || t[2] % s != 0 {
        return None;
    }
    let i = (t[1] / s).checked_sub(key[2] * 16)?;
    let j = (t[2] / s).checked_sub(key[3] * 16)?;
    (i < SIDE && j < SIDE).then_some(j * SIDE + i)
}
/// The skirt's edge walk, as the patch maker lays it out: grid index per edge point.
fn edge_walk() -> Vec<u32> {
    let g = SIDE;
    (0..g)
        .map(|i| i)
        .chain((0..g).map(|j| j * g + g - 1))
        .chain((0..g).rev().map(|i| (g - 1) * g + i))
        .chain((0..g).rev().map(|j| j * g))
        .collect()
}

/// A seam relation between resident boundary vertices of different patches.
enum Relation {
    /// The same lattice point.
    Same(Vec<V>),
    /// A fine point on a coarser neighbour's segment, at `t` from `a` to `b`.
    Span { f: V, a: V, b: V, t: f64 },
}
impl Relation {
    fn members(&self) -> Vec<V> {
        match self {
            Relation::Same(v) => v.clone(),
            Relation::Span { f, a, b, .. } => vec![*f, *a, *b],
        }
    }
    fn gap(&self, at: &dyn Fn(V) -> DVec3) -> f64 {
        match self {
            Relation::Same(v) => {
                let p = at(v[0]);
                v[1..].iter().map(|u| at(*u).distance(p)).fold(0., f64::max)
            }
            Relation::Span { f, a, b, t } => at(*f).distance(at(*a).lerp(at(*b), *t)),
        }
    }
}

/// A point of an exterior polygon: where it is in the chart and in the world, and which
/// resident vertices (and weights) its attributes come from.
#[derive(Clone)]
struct Point {
    uv: DVec2,
    world: DVec3,
    /// Lines it lies on: triangle edges 0..3, then chart lines 3 + axis*2 + (side > 0).
    on: u8,
    /// A resident grid vertex, if it is one.
    vertex: Option<u32>,
}

fn line_bit(axis: usize, side: f64) -> u8 {
    1 << (3 + axis * 2 + usize::from(side > 0.))
}

/// The crossed triangles' parts outside the square, drawn in the band's frame.
pub struct Rim {
    pub origin: [f64; 3],
    pub positions: Vec<[f32; 3]>,
    pub morph: Vec<[f32; 4]>,
    /// Each vertex's attributes: the supplied patch (its index) and its vertices and weights.
    pub blend: Vec<(u32, [(u32, f32); 3])>,
    pub faces: Vec<[u32; 3]>,
}

pub struct Prepared {
    pub patches: Vec<Replacement>,
    pub rim: Rim,
    pub report: Value,
}

/// `grid`: the footprint's resident triangles as corrected (`ownership::correct`) at `eye`.
/// `patches`: the actual resident meshes supplied; every one a closure may reach must be here.
/// `band`: the contact band, its outer chain built from the same `grid`.
pub fn excise(
    chart: &Chart,
    eye: DVec3,
    grid: Vec<Triangle>,
    patches: &[RetainedPatch],
    band: &Tile,
) -> Result<Prepared, Value> {
    let by_key: HashMap<[u32; 4], usize> =
        patches.iter().enumerate().map(|(i, p)| (p.key, i)).collect();
    let walk = edge_walk();
    for p in patches {
        let n = p.mesh.vertex_count();
        let faces = p.mesh.all_faces();
        let skirt = n.checked_sub(GRID_VERTICES).map(|s| s / 2);
        let layout = skirt.is_some_and(|e| {
            (n - GRID_VERTICES) % 2 == 0
                && (e == 0 || e == walk.len())
                && faces.len() == GRID_FACES + 4 * e.saturating_sub(1)
        });
        if !layout || p.mesh.faces().len() != GRID_FACES {
            return Err(json!({"reason":"resident patch layout","patch":p.key}));
        }
    }
    let origin = |pi: usize| DVec3::from_array(patches[pi].origin);
    let resident = |(pi, i): V, at: DVec3| -> DVec3 {
        let p = &patches[pi];
        origin(pi) + morphed(origin(pi), Vec3::from_array(p.mesh.position(i as usize)), p.mesh.morph(i as usize), at).as_dvec3()
    };
    let inside = |uv: DVec2| uv.x.abs() <= HALF && uv.y.abs() <= HALF;

    // Each footprint triangle: its patch, resident face and grid vertices.
    let mut excised: HashMap<usize, BTreeSet<usize>> = HashMap::new();
    let mut locked: HashMap<V, DVec3> = HashMap::new();
    let mut crossed = vec![];
    let mut square_area = 0.;
    let footprint_triangles = grid.len();
    for t in &grid {
        let pi = *by_key
            .get(&t.patch)
            .ok_or_else(|| json!({"reason":"footprint triangle of an unsupplied patch","patch":t.patch}))?;
        let ids = t.tags.map(|g| local(t.patch, g));
        let [Some(a), Some(b), Some(c)] = ids else {
            return Err(json!({"reason":"footprint triangle off its patch lattice","patch":t.patch}));
        };
        // The grid's own layout: cell (i, j) holds faces 2(16j+i) and 2(16j+i)+1.
        let mut k = [a, b, c];
        k.sort_unstable();
        let (i, j) = (k.iter().map(|v| v % SIDE).min().unwrap_or(0), k.iter().map(|v| v / SIDE).min().unwrap_or(0));
        let cell = (2 * (j * 16 + i)) as usize;
        let face = if k.contains(&(j * SIDE + i + 1)) { cell } else { cell + 1 };
        let mut resident_face = patches[pi].mesh.faces().get(face).copied().unwrap_or([u32::MAX; 3]);
        resident_face.sort_unstable();
        if i >= 16 || j >= 16 || resident_face != k {
            return Err(json!({"reason":"footprint triangle is not a resident face","patch":t.patch}));
        }
        if !excised.entry(pi).or_default().insert(face) {
            return Err(json!({"reason":"footprint triangle twice","patch":t.patch,"face":face}));
        }
        for (i, v) in [a, b, c].into_iter().enumerate() {
            if let Some(old) = locked.insert((pi, v), t.world[i]) {
                if old != t.world[i] {
                    return Err(json!({"reason":"footprint vertex inconsistent","patch":t.patch,"vertex":v}));
                }
            }
        }
        let uv: Vec<DVec2> = t.world.iter().map(|p| chart.project(*p)).collect::<Result<_, _>>().map_err(|e| json!({"reason":e}))?;
        square_area += area(&rect(uv.clone(), [-HALF, HALF, -HALF, HALF]));
        if !uv.iter().all(|p| inside(*p)) {
            crossed.push((pi, face, [a, b, c], uv));
        }
    }
    // Only the locked places and the crossed triangles are needed from here.
    drop(grid);
    if (square_area - 4. * HALF * HALF).abs() > 1e-3 {
        return Err(json!({"reason":"resident coverage of the footprint","area_m2":square_area}));
    }

    // Seam relations among every supplied patch's boundary.
    let mut same: HashMap<[u32; 3], Vec<V>> = HashMap::new();
    // Line (face, axis 1 = constant u-tag / 2 = constant v-tag, coordinate) -> sides on it:
    // (patch, which side of the line the patch lies, its points as (parameter, vertex)).
    let mut lines: HashMap<(u32, usize, u32), Vec<(usize, bool, Vec<(u32, u32)>)>> = HashMap::new();
    for (pi, p) in patches.iter().enumerate() {
        let side = |fixed: usize, at: u32| -> Vec<(u32, u32)> {
            (0..SIDE)
                .map(|k| {
                    let i = if fixed == 1 { k * SIDE + at } else { at * SIDE + k };
                    (tag(p.key, i)[3 - fixed], i)
                })
                .collect()
        };
        for (fixed, at, plus) in [(1, 0, true), (1, SIDE - 1, false), (2, 0, true), (2, SIDE - 1, false)] {
            let points = side(fixed, at);
            let coord = tag(p.key, points[0].1)[fixed];
            for &(_, i) in &points {
                let list = same.entry(tag(p.key, i)).or_default();
                if !list.contains(&(pi, i)) {
                    list.push((pi, i));
                }
            }
            lines.entry((p.key[0], fixed, coord)).or_default().push((pi, plus, points));
        }
    }
    let mut relations: Vec<Relation> = same.into_values().filter(|v| v.len() > 1).map(Relation::Same).collect();
    for sides in lines.values() {
        for (fi, (fp, fplus, fine)) in sides.iter().enumerate() {
            for (ci, (cp, cplus, coarse)) in sides.iter().enumerate() {
                if fi == ci || fplus == cplus || step(patches[*fp].key) >= step(patches[*cp].key) {
                    continue;
                }
                for &(q, f) in fine {
                    let Some(w) = coarse.windows(2).find(|w| w[0].0 < q && q < w[1].0) else { continue };
                    let t = (q - w[0].0) as f64 / (w[1].0 - w[0].0) as f64;
                    relations.push(Relation::Span { f: (*fp, f), a: (*cp, w[0].1), b: (*cp, w[1].1), t });
                }
            }
        }
    }
    let mut touching: HashMap<V, Vec<usize>> = HashMap::new();
    for (r, rel) in relations.iter().enumerate() {
        for v in rel.members() {
            touching.entry(v).or_default().push(r);
        }
    }

    // Close the locks: a relation holding a locked vertex locks its every moving member.
    let moves = |(pi, i): V| patches[pi].mesh.morph(i as usize)[..3].iter().any(|d| *d != 0.);
    let mut work: Vec<V> = locked.keys().copied().collect();
    work.sort_unstable();
    let footprint_locks = work.len();
    while let Some(v) = work.pop() {
        for &r in touching.get(&v).into_iter().flatten() {
            for u in relations[r].members() {
                if locked.contains_key(&u) || !moves(u) {
                    continue;
                }
                // Equal points take the locked partner's (corrected) place; others their own.
                let world = match &relations[r] {
                    Relation::Same(_) => locked[&v],
                    Relation::Span { .. } => resident(u, eye),
                };
                locked.insert(u, world);
                work.push(u);
            }
        }
    }
    // Every locked boundary point needs its neighbour across the line among those supplied.
    let face_edge = 16u32 << 19;
    for (&(pi, i), _) in &locked {
        let key = patches[pi].key;
        let (ii, jj) = (i % SIDE, i / SIDE);
        let t = tag(key, i);
        for (fixed, on, plus) in [(1, ii == 0, true), (1, ii == SIDE - 1, false), (2, jj == 0, true), (2, jj == SIDE - 1, false)] {
            if !on {
                continue;
            }
            if t[fixed] == 0 || t[fixed] == face_edge {
                return Err(json!({"reason":"lock closure crosses a cube face","patch":key,"vertex":i}));
            }
            let q = t[3 - fixed];
            let covered = lines.get(&(key[0], fixed, t[fixed])).is_some_and(|sides| {
                sides.iter().any(|(_, p, pts)| *p != plus && pts[0].0 <= q && q <= pts[pts.len() - 1].0)
            });
            if !covered {
                return Err(json!({"reason":"lock closure needs a patch not supplied","patch":key,"vertex":i,"tag":t}));
            }
        }
    }

    // The band's outer points, by side line, to stitch into the exterior.
    let band_origin = DVec3::from_array(band.origin);
    let band_world = |k: usize| band_origin + Vec3::from_array(band.vertices[k]).as_dvec3();
    let mut band_outer: Vec<(DVec2, DVec3)> = vec![];
    for (k, xy) in band.uv.iter().enumerate() {
        let q = DVec2::from_array(*xy);
        if (q.x.abs() == HALF && q.y.abs() <= HALF) || (q.y.abs() == HALF && q.x.abs() <= HALF) {
            band_outer.push((q, band_world(k)));
        }
    }

    // Each modified patch: its vertices (locked ones still), faces and the exterior polygons.
    let mut touched: BTreeSet<usize> = excised.keys().copied().collect();
    touched.extend(locked.keys().map(|v| v.0));
    if touched.len() > MAX_PATCHES {
        return Err(json!({"reason":"group patch cap","patches":touched.len()}));
    }
    let mut out = vec![];
    let mut exterior_boundary: Vec<(DVec3, DVec3)> = vec![];
    let mut new_tris = 0;
    let mut stitched = 0;
    for &pi in &touched {
        let p = &patches[pi];
        let o = origin(pi);
        let n = p.mesh.vertex_count();
        let mut positions: Vec<[f32; 3]> = (0..n).map(|i| p.mesh.position(i)).collect();
        let mut morph: Vec<[f32; 4]> = (0..n).map(|i| p.mesh.morph(i)).collect();
        let mut blend: Vec<[(u32, f32); 3]> = (0..n as u32).map(|i| [(i, 1.), (i, 0.), (i, 0.)]).collect();
        for i in 0..GRID_VERTICES as u32 {
            if let Some(w) = locked.get(&(pi, i)) {
                positions[i as usize] = (*w - o).as_vec3().to_array();
                morph[i as usize] = [0., 0., 0., morph[i as usize][3]];
            }
        }
        // Skirt aliases follow their grid point; the bottom keeps its drop below it.
        let skirts = (n - GRID_VERTICES) / 2;
        for w in 0..skirts {
            let g = walk[w];
            let Some(at) = locked.get(&(pi, g)) else { continue };
            let (top, bottom) = (GRID_VERTICES + 2 * w, GRID_VERTICES + 2 * w + 1);
            let shift = *at - resident((pi, g), eye);
            for k in [top, bottom] {
                let place = resident((pi, k as u32), eye) + shift;
                positions[k] = (place - o).as_vec3().to_array();
                morph[k] = [0., 0., 0., morph[k][3]];
            }
        }
        let gone = excised.get(&pi);
        let all = p.mesh.all_faces();
        let mut faces: Vec<[u32; 3]> =
            all[..GRID_FACES].iter().enumerate().filter(|(f, _)| !gone.is_some_and(|g| g.contains(f))).map(|(_, f)| *f).collect();
        let ground = faces.len();
        // Skirts: keep each segment's part outside the square.
        let uv_of = |w: DVec3| chart.project(w).ok();
        for w in 0..skirts.saturating_sub(1) {
            let quad = &all[GRID_FACES + 4 * w..GRID_FACES + 4 * w + 4];
            let (ta, tb) = ((GRID_VERTICES + 2 * w) as u32, (GRID_VERTICES + 2 * w + 2) as u32);
            let top = |k: u32| o + Vec3::from_array(positions[k as usize]).as_dvec3();
            let (Some(a), Some(b)) = (uv_of(top(ta)), uv_of(top(tb))) else {
                faces.extend_from_slice(quad);
                continue;
            };
            // The segment's parameters inside the square (Liang-Barsky).
            let d = b - a;
            let (mut lo, mut hi) = (0f64, 1f64);
            for (pk, qk) in [(-d.x, a.x + HALF), (d.x, HALF - a.x), (-d.y, a.y + HALF), (d.y, HALF - a.y)] {
                if pk == 0. {
                    if qk < 0. {
                        hi = -1.;
                    }
                } else if pk < 0. {
                    lo = lo.max(qk / pk);
                } else {
                    hi = hi.min(qk / pk);
                }
            }
            if hi - lo <= 1e-12 {
                faces.extend_from_slice(quad);
                continue;
            }
            let mut keep = vec![];
            if lo > 1e-12 {
                keep.push((0., lo));
            }
            if hi < 1. - 1e-12 {
                keep.push((hi, 1.));
            }
            for (s, e) in keep {
                let mut ends = [0u32; 4];
                for (slot, (t, k)) in [(s, 0), (e, 0), (s, 1), (e, 1)].into_iter().enumerate() {
                    let (x, y) = (ta + k, tb + k);
                    let at = |q: u32| o + Vec3::from_array(positions[q as usize]).as_dvec3();
                    ends[slot] = if t == 0. {
                        x
                    } else if t == 1. {
                        y
                    } else {
                        let place = at(x).lerp(at(y), t);
                        positions.push((place - o).as_vec3().to_array());
                        morph.push([0., 0., 0., morph[x as usize][3]]);
                        blend.push([(x, (1. - t) as f32), (y, t as f32), (x, 0.)]);
                        (positions.len() - 1) as u32
                    };
                }
                let [a, b, c, d] = ends;
                faces.extend([[a, b, d], [a, d, c], [a, d, b], [a, c, d]]);
            }
        }
        out.push(Replacement { key: p.key, mesh_id: p.mesh_id, origin: p.origin, positions, morph, blend, faces, ground });
    }

    // Crossed triangles: their parts outside the square, in one rim mesh in the band's own
    // frame (a coarse patch's f32 frame cannot hold the band's points to the seam budget),
    // stitched to the band's outer points.
    let mut rim = Rim { origin: band.origin, positions: vec![], morph: vec![], blend: vec![], faces: vec![] };
    let mut rim_index: HashMap<[u64; 3], u32> = HashMap::new();
    let mut degenerate = 0;
    let (mut merged, mut merges) = (0f64, 0);
    let mut rim_patch = 0f64;
    let mut rim_worst = Value::Null;
    let bo = band_origin;
    for (pi, face, ids, uv) in &crossed {
        let (pi, face) = (*pi, *face);
        let p = &patches[pi];
        let o = origin(pi);
        let resident_face = p.mesh.all_faces()[face];
        let world = |i: u32| locked[&(pi, i)];
        let size = p.mesh.morph(ids[0] as usize)[3];
            let tri_world = ids.map(&world);
            let tri_tags = ids.map(|i| tag(p.key, i));
            let orientation = (uv[1] - uv[0]).perp_dot(uv[2] - uv[0]).signum();
            let normal = (tri_world[1] - tri_world[0]).cross(tri_world[2] - tri_world[0]);
            let on_plane = |q: DVec2| {
                let d = chart.direction(q);
                d * (normal.dot(tri_world[0]) / normal.dot(d))
            };
            let bary = |q: DVec2| {
                let det = (uv[1] - uv[0]).perp_dot(uv[2] - uv[0]);
                let v = (q - uv[0]).perp_dot(uv[2] - uv[0]) / det;
                let w = (uv[1] - uv[0]).perp_dot(q - uv[0]) / det;
                [(ids[0], (1. - v - w) as f32), (ids[1], v as f32), (ids[2], w as f32)]
            };
            let start: Vec<Point> = (0..3)
                .map(|k| Point { uv: uv[k], world: tri_world[k], on: (1 << k) | (1 << ((k + 2) % 3)), vertex: Some(ids[k]) })
                .collect();
            // Triangle edge k runs from vertex k to k+1.
            let edge_of = |m: u8| (0..3).find(|k| m & (1 << k) != 0);
            let clip = |poly: Vec<Point>, axis: usize, side: f64, keep_below: bool| -> Vec<Point> {
                let keep = |p: &Point| if keep_below { p.uv[axis] <= side } else { p.uv[axis] >= side };
                let mut next = vec![];
                for k in 0..poly.len() {
                    let (a, b) = (&poly[k], &poly[(k + 1) % poly.len()]);
                    if keep(a) {
                        next.push(a.clone());
                    }
                    if keep(a) != keep(b) && a.uv[axis] != side && b.uv[axis] != side {
                        let common = a.on & b.on;
                        let point = if let Some(e) = edge_of(common) {
                            let (x, y) = (e, (e + 1) % 3);
                            let (x, y) = if tri_tags[x] < tri_tags[y] { (x, y) } else { (y, x) };
                            match crossing(chart, tri_world[x], tri_world[y], axis, side) {
                                Some((q, w)) => Point { uv: q, world: w, on: (1 << e) | line_bit(axis, side), vertex: None },
                                None => {
                                    let t = (side - a.uv[axis]) / (b.uv[axis] - a.uv[axis]);
                                    let mut q = a.uv.lerp(b.uv, t);
                                    q[axis] = side;
                                    Point { uv: q, world: on_plane(q), on: (1 << e) | line_bit(axis, side), vertex: None }
                                }
                            }
                        } else {
                            // Along another chart line: the square's corner.
                            let t = (side - a.uv[axis]) / (b.uv[axis] - a.uv[axis]);
                            let mut q = a.uv.lerp(b.uv, t);
                            q[axis] = side;
                            for x in [axis, 1 - axis] {
                                if (q[x].abs() - HALF).abs() < 1e-9 {
                                    q[x] = HALF.copysign(q[x]);
                                }
                            }
                            Point { uv: q, world: on_plane(q), on: (common & !7) | line_bit(axis, side), vertex: None }
                        };
                        next.push(point);
                    }
                }
                next
            };
            let regions = [
                vec![(0, -HALF, true)],
                vec![(0, HALF, false)],
                vec![(0, -HALF, false), (0, HALF, true), (1, -HALF, true)],
                vec![(0, -HALF, false), (0, HALF, true), (1, HALF, false)],
            ];
            for region in regions {
                let mut poly = start.clone();
                for &(axis, side, below) in &region {
                    if poly.len() < 3 {
                        break;
                    }
                    poly = clip(poly, axis, side, below);
                }
                poly.dedup_by(|a, b| a.uv == b.uv);
                while poly.len() > 1 && poly[0].uv == poly[poly.len() - 1].uv {
                    poly.pop();
                }
                if poly.len() < 3 || area(&poly.iter().map(|p| p.uv).collect::<Vec<_>>()) < 1e-12 {
                    continue;
                }
                // Snap square-edge points to the band's own, and add the band's between them.
                let on_square = |q: DVec2| (q.x.abs() == HALF && q.y.abs() <= HALF) || (q.y.abs() == HALF && q.x.abs() <= HALF);
                for pt in poly.iter_mut() {
                    if pt.vertex.is_none() && on_square(pt.uv) {
                        if let Some((_, w)) = band_outer.iter().find(|(q, _)| q.distance(pt.uv) < 1e-4) {
                            pt.world = *w;
                        }
                    }
                }
                let mut full = vec![];
                for k in 0..poly.len() {
                    let (a, b) = (&poly[k], &poly[(k + 1) % poly.len()]);
                    full.push(a.clone());
                    for axis in [0, 1] {
                        let side = a.uv[axis];
                        if a.uv[axis] != b.uv[axis] || side.abs() != HALF || a.on & b.on & line_bit(axis, side) == 0 {
                            continue;
                        }
                        let (lo, hi) = (a.uv[1 - axis].min(b.uv[1 - axis]), a.uv[1 - axis].max(b.uv[1 - axis]));
                        let mut between: Vec<&(DVec2, DVec3)> = band_outer
                            .iter()
                            .filter(|(q, _)| q[axis] == side && q[1 - axis] > lo + 1e-4 && q[1 - axis] < hi - 1e-4)
                            .collect();
                        between.sort_by(|x, y| x.0[1 - axis].total_cmp(&y.0[1 - axis]));
                        if b.uv[1 - axis] < a.uv[1 - axis] {
                            between.reverse();
                        }
                        for (q, w) in between {
                            stitched += 1;
                            full.push(Point { uv: *q, world: *w, on: line_bit(axis, side), vertex: None });
                        }
                    }
                }
                // As drawn: f32 local to the band's origin.
                let drawn = |w: DVec3| bo + (w - bo).as_vec3().as_dvec3();
                for k in 0..full.len() {
                    let (a, b) = (&full[k], &full[(k + 1) % full.len()]);
                    if on_square(a.uv) && on_square(b.uv) && a.on & b.on & !7 != 0 {
                        exterior_boundary.push((drawn(a.world), drawn(b.world)));
                    }
                }
                let mut index = |pt: &Point| -> u32 {
                    let key = pt.world.to_array().map(f64::to_bits);
                    if let Some(&k) = rim_index.get(&key) {
                        return k;
                    }
                    rim.positions.push((pt.world - bo).as_vec3().to_array());
                    rim.morph.push([0., 0., 0., size]);
                    rim.blend.push((pi as u32, match pt.vertex { Some(v) => [(v, 1.), (v, 0.), (v, 0.)], None => bary(pt.uv) }));
                    let k = (rim.positions.len() - 1) as u32;
                    rim_index.insert(key, k);
                    if let Some(v) = pt.vertex {
                        // The same point as the patch draws it, in its own frame.
                        let mine = bo + Vec3::from_array(rim.positions[k as usize]).as_dvec3();
                        let theirs = o + (world(v) - o).as_vec3().as_dvec3();
                        let gap = mine.distance(theirs);
                        if gap > rim_patch {
                            rim_patch = gap;
                            rim_worst = json!({"patch":p.key,"vertex":v,"gap_m":gap,"from_band_origin_m":(world(v) - bo).length(),"from_patch_origin_m":(world(v) - o).length()});
                        }
                    }
                    k
                };
                // A resident vertex an f32 step from where its edge crosses the square is, as drawn,
                // the crossing: keep the band's point, and say how far apart the two were.
                let as_drawn = |w: DVec3| (w - bo).as_vec3();
                let mut k = 0;
                while full.len() > 3 && k < full.len() {
                    let next = (k + 1) % full.len();
                    if as_drawn(full[k].world) == as_drawn(full[next].world) {
                        let gap = full[k].world.distance(full[next].world);
                        if gap > SEAM {
                            return Err(json!({"reason":"distinct exterior points collapse in f32","patch":p.key,"face":resident_face,"gap_m":gap}));
                        }
                        merged = merged.max(gap);
                        merges += 1;
                        let drop = if full[k].vertex.is_some() { k } else { next };
                        full.remove(drop);
                        continue;
                    }
                    k += 1;
                }
                let ring: Vec<u32> = full.iter().map(&mut index).collect();
                let fan: Vec<[u32; 3]> = if ring.len() == 3 {
                    vec![[ring[0], ring[1], ring[2]]]
                } else {
                    let centre = full.iter().map(|p| p.uv).sum::<DVec2>() / full.len() as f64;
                    let c = index(&Point { uv: centre, world: on_plane(centre), on: 0, vertex: None });
                    (0..ring.len()).map(|k| [c, ring[k], ring[(k + 1) % ring.len()]]).collect()
                };
                let flat = |q: u32| chart.project(bo + Vec3::from_array(rim.positions[q as usize]).as_dvec3()).unwrap_or(DVec2::ZERO);
                for mut t in fan {
                    let s = (flat(t[1]) - flat(t[0])).perp_dot(flat(t[2]) - flat(t[0]));
                    if s.signum() != orientation {
                        t.swap(1, 2);
                    }
                    let q = t.map(|k| Vec3::from_array(rim.positions[k as usize]));
                    if q[0] == q[1] || q[1] == q[2] || q[0] == q[2] {
                        return Err(json!({"reason":"distinct exterior points collapse in f32","patch":p.key,"face":resident_face}));
                    }
                    if (q[1] - q[0]).cross(q[2] - q[0]).length_squared() == 0. {
                        // Collinear in f32: it covers nothing, and its neighbours share its edges.
                        degenerate += 1;
                        continue;
                    }
                    rim.faces.push(t);
                    new_tris += 1;
                }
            }
                let _ = o;
    }

    // The rim meets each patch where they share a vertex. Over a coarse patch (a loading one
    // spanning the whole footprint) the shared vertex can lie kilometres from both origins, and
    // two f32 frames cannot hold it to the seam budget there: refuse, as any other seam.
    if rim_patch > SEAM {
        return Err(json!({"reason":"rim meets a patch beyond the seam budget (coarse support)","worst":rim_worst}));
    }

    // The exterior meets the band's outer edge, point for point.
    let mut band_edges: Vec<(DVec3, DVec3)> = vec![];
    for t in &band.triangles {
        for k in 0..3 {
            let (a, b) = (t[k] as usize, t[(k + 1) % 3] as usize);
            let (qa, qb) = (DVec2::from_array(band.uv[a]), DVec2::from_array(band.uv[b]));
            if (qa.x == qb.x && qa.x.abs() == HALF) || (qa.y == qb.y && qa.y.abs() == HALF) {
                band_edges.push((band_world(a), band_world(b)));
            }
        }
    }
    // Separation of the two outer polylines: each vertex of one to the nearest segment of the
    // other, both ways. For piecewise-linear curves that bounds every point of either.
    let to_segment = |p: DVec3, e: &(DVec3, DVec3)| {
        let d = e.1 - e.0;
        let t = if d.length_squared() > 0. { ((p - e.0).dot(d) / d.length_squared()).clamp(0., 1.) } else { 0. };
        p.distance(e.0 + d * t)
    };
    let nearest = |p: DVec3, set: &[(DVec3, DVec3)]| set.iter().map(|e| to_segment(p, e)).fold(f64::INFINITY, f64::min);
    let mut band_seam = 0f64;
    let mut worst_edge = None;
    let mut unmatched_edges = 0;
    for (set, other, name) in [(&band_edges, &exterior_boundary, "band"), (&exterior_boundary, &band_edges, "exterior")] {
        for e in set.iter() {
            let d = nearest(e.0, other).max(nearest(e.1, other));
            let exact = other.iter().any(|f| (e.0 == f.0 && e.1 == f.1) || (e.0 == f.1 && e.1 == f.0));
            unmatched_edges += usize::from(!exact);
            if d > band_seam {
                band_seam = d;
                worst_edge = Some(json!({"side":name,"edge":[chart.project(e.0).ok().map(|p|p.to_array()),chart.project(e.1).ok().map(|p|p.to_array())]}));
            }
        }
    }
    if band_edges.is_empty() || band_seam > SEAM {
        return Err(json!({"reason":"exterior does not meet the band's outer edge","max_m":band_seam,"worst":worst_edge,"band_edges":band_edges.len(),"exterior_edges":exterior_boundary.len(),"merged":merges,"merged_gap_m":merged}));
    }

    // No seam between patches wider than the original renderer's, at the job's eye or another.
    let by_patch: HashMap<usize, &Replacement> = touched.iter().copied().zip(out.iter()).collect();
    let mut used: HashMap<usize, HashSet<u32>> =
        out.iter().zip(&touched).map(|(r, &pi)| (pi, r.faces.iter().flatten().copied().collect())).collect();
    for (slot, b) in &rim.blend {
        if b[0].1 == 1. {
            used.entry(*slot as usize).or_default().insert(b[0].0);
        }
    }
    let now = |(pi, i): V, at: DVec3| -> DVec3 {
        match by_patch.get(&pi) {
            Some(r) => {
                let o = origin(pi);
                o + morphed(o, Vec3::from_array(r.positions[i as usize]), r.morph[i as usize], at).as_dvec3()
            }
            None => resident((pi, i), at),
        }
    };
    let up = chart.c;
    let eyes = [eye, eye + chart.e * 30., eye - chart.e * 30., eye + chart.n * 30., eye - chart.n * 30., eye + up * 20., eye + chart.e * 90. + up * 5.];
    let mut worst = 0f64;
    let mut checked = 0;
    for rel in &relations {
        let members = rel.members();
        if !members.iter().any(|v| by_patch.contains_key(&v.0)) {
            continue;
        }
        if members.iter().any(|v| used.get(&v.0).is_some_and(|u| !u.contains(&v.1))) {
            continue;
        }
        checked += 1;
        let before = rel.gap(&|v| resident(v, eye));
        for at in eyes {
            let new = rel.gap(&|v| now(v, at));
            let old = rel.gap(&|v| resident(v, at)).max(before).max(SEAM);
            worst = worst.max(new - old);
            if new > old + 1e-7 {
                return Err(json!({"reason":"patch seam widened by the replacement","new_m":new,"allowed_m":old,"members":members.iter().map(|v|json!([patches[v.0].key,v.1])).collect::<Vec<_>>(),"eye":at.to_array()}));
            }
        }
    }
    let report = json!({
        "patches_changed":out.len(),"patches_supplied":patches.len(),
        "footprint_triangles":footprint_triangles,"crossed_triangles":crossed.len(),
        "footprint_locks":footprint_locks,"closure_locks":locked.len()-footprint_locks,
        "exterior_triangles":new_tris,"exterior_degenerate_dropped":degenerate,"f32_coincident_points_merged":merges,"f32_coincident_max_gap_m":merged,
        "rim_vertices":rim.positions.len(),"rim_patch_seam_max_m":rim_patch,"rim_patch_worst":rim_worst,"band_points_stitched":stitched,
        "band_outer_edges":band_edges.len(),"exterior_outer_edges":exterior_boundary.len(),
        "band_seam_max_m":band_seam,"band_edges_not_shared_exactly":unmatched_edges,"relations_checked":checked,"eyes_checked":eyes.len(),
        "max_seam_widening_m":worst.max(0.),
        "coverage_m2":square_area,
    });
    Ok(Prepared { patches: out, rim, report })
}
