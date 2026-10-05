//! Load bearing: a ship's frame of members (beams welded at their joints), the
//! forces a load case puts through it, and what breaks.
//!
//! **The frame** is joints (points in the ship's frame, m) and members between
//! them, each a section of a material: a 3D frame of beams, stiff at their
//! joints (each joint moves and turns: six ways), so a member carries a push or
//! pull along it, bending and twist. It is solved the standard way: each
//! member's stiffness put together into the whole frame's, the loads at the
//! joints, the joints held where something holds them, and the stiffness
//! inverted for how far each joint moves; each member's forces follow from how
//! its ends moved.
//!
//! **A load case** is the loads at the joints (weights, inertia, thrust) and
//! what holds it: joints held where they stand (on the ground: its landing
//! pads), or, free in flight with its loads in balance, one joint held only to
//! keep it from drifting (it takes next to nothing).
//!
//! **Breaking** is first-order, as an engineer sizes a member: its stress (its
//! push or pull over its area, and its bending at its surface) against its
//! material's tensile strength, and a member pushed along against the load it
//! buckles at (Euler's, for a column its length). A member past either breaks;
//! the frame is solved again without it (its load moved onto the rest) until
//! nothing more breaks, or it falls apart. The design limit is the same with
//! the registry's safety factor, against its yield strength.

use glam::DVec3;

/// A member's cross-section: a round tube or bar.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Section {
    /// Its outer diameter and wall (m; a bar: its wall half its diameter).
    pub diameter: f64,
    pub wall: f64,
}

impl Section {
    fn radii(&self) -> (f64, f64) {
        let ro = self.diameter * 0.5;
        (ro, (ro - self.wall).max(0.0))
    }

    /// Its area (m²).
    pub fn area(&self) -> f64 {
        let (ro, ri) = self.radii();
        std::f64::consts::PI * (ro * ro - ri * ri)
    }

    /// Its second moment of area about a line across it (m⁴); twice that is its
    /// torsion constant, a round section's.
    pub fn inertia(&self) -> f64 {
        let (ro, ri) = self.radii();
        std::f64::consts::PI / 4.0 * (ro.powi(4) - ri.powi(4))
    }
}

/// What a member is made of, as far as bearing goes.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Material {
    /// Young's modulus and the shear modulus (Pa).
    pub stiffness: f64,
    pub shear: f64,
    /// The stress it yields at, and breaks at (Pa).
    pub yield_strength: f64,
    pub tensile_strength: f64,
    /// kg/m³.
    pub density: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Member {
    pub a: usize,
    pub b: usize,
    pub section: Section,
    pub material: Material,
}

impl Member {
    /// Its mass (kg), its ends `pa` and `pb` apart.
    pub fn mass(&self, pa: DVec3, pb: DVec3) -> f64 {
        self.section.area() * pa.distance(pb) * self.material.density
    }
}

/// A frame: its joints and its members.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Frame {
    pub joints: Vec<DVec3>,
    pub members: Vec<Member>,
}

/// A load case: forces at joints (N), and what holds it: joints held where they
/// stand (they don't move; they turn), and one joint held entirely (in flight,
/// its loads in balance: it keeps the frame from drifting).
#[derive(Clone, Debug, Default)]
pub struct Case {
    pub loads: Vec<(usize, DVec3)>,
    pub held: Vec<usize>,
    pub anchor: Option<usize>,
}

/// What a member carries: the pull along it (N; a push is less than nothing),
/// its biggest bending moment and its twist (N m).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Forces {
    pub axial: f64,
    pub bending: f64,
    pub torsion: f64,
}

/// How hard a member is worked: its stress over what it breaks at (its tensile
/// strength, or its buckling load if it's pushed), and over its design limit
/// (its yield strength, with the safety factor). Past 1 it breaks; past 1 on the
/// design limit, it's over what it's designed for.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Work {
    pub forces: Forces,
    pub breaking: f64,
    pub design: f64,
    /// Buckling, not strength, is what limits it.
    pub buckles: bool,
}

/// How a member does under a load case: its forces and how hard it's worked;
/// broken (and in which round of breaking: the first to go is 1).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Outcome {
    pub work: Work,
    pub broken: Option<usize>,
}

/// Why a frame can't be solved: some of it isn't held (it would move freely:
/// not joined to the rest, or turning loose about a joint).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Loose;

/// Its stress at the surface (Pa): its pull or push over its area, and its
/// bending at its outer edge.
fn stress(f: &Forces, s: &Section) -> f64 {
    f.axial.abs() / s.area() + f.bending * s.diameter * 0.5 / s.inertia()
}

/// How hard member `m` is worked by `f`, `length` long, with safety factor `sf`.
pub fn work(m: &Member, f: Forces, length: f64, sf: f64) -> Work {
    let sigma = stress(&f, &m.section);
    let euler = std::f64::consts::PI.powi(2) * m.material.stiffness * m.section.inertia() / (length * length).max(1e-9);
    let push = (-f.axial).max(0.0);
    let (strength, buckling) = (sigma / m.material.tensile_strength, push / euler);
    Work {
        forces: f,
        breaking: strength.max(buckling),
        design: (sigma * sf / m.material.yield_strength).max(push * sf / euler),
        buckles: buckling > strength,
    }
}

/// The forces in each member of `frame` under `case` (none for a member left out
/// by `gone`).
pub fn solve(frame: &Frame, case: &Case, gone: &[bool]) -> Result<Vec<Option<Forces>>, Loose> {
    // (Only the joints members reach: six ways each.)
    let live: Vec<usize> = (0..frame.members.len()).filter(|&k| !gone.get(k).copied().unwrap_or(false)).collect();
    let mut index = vec![usize::MAX; frame.joints.len()];
    let mut n = 0;
    for &k in &live {
        for j in [frame.members[k].a, frame.members[k].b] {
            if index[j] == usize::MAX {
                index[j] = n;
                n += 1;
            }
        }
    }
    let dof = n * 6;
    let mut k_all = vec![0.0f64; dof * dof];
    let mut locals = Vec::with_capacity(live.len());
    for &k in &live {
        let m = &frame.members[k];
        let (pa, pb) = (frame.joints[m.a], frame.joints[m.b]);
        let (kl, t) = element(m, pa, pb);
        let kg = to_global(&kl, &t);
        let at = [index[m.a] * 6, index[m.b] * 6];
        for r in 0..12 {
            for c in 0..12 {
                let (gr, gc) = (at[r / 6] + r % 6, at[c / 6] + c % 6);
                k_all[gr * dof + gc] += kg[r][c];
            }
        }
        locals.push((k, kl, t, at));
    }
    let mut f = vec![0.0f64; dof];
    for &(j, load) in &case.loads {
        if index.get(j).is_some_and(|&i| i != usize::MAX) {
            for d in 0..3 {
                f[index[j] * 6 + d] += load[d];
            }
        }
    }
    // (Held: those ways fixed, by a stiff spring there; the rest free.)
    let big = k_all.iter().fold(0.0f64, |a, v| a.max(v.abs())).max(1.0) * 1e8;
    let mut fix = |dof_at: usize| k_all[dof_at * dof + dof_at] += big;
    for &j in &case.held {
        if index[j] != usize::MAX {
            for d in 0..3 {
                fix(index[j] * 6 + d);
            }
        }
    }
    if let Some(j) = case.anchor.filter(|&j| index[j] != usize::MAX) {
        for d in 0..6 {
            fix(index[j] * 6 + d);
        }
    }
    let u = gauss(k_all, f, dof)?;
    let mut out = vec![None; frame.members.len()];
    for (k, kl, t, at) in locals {
        let mut ug = [0.0; 12];
        for r in 0..12 {
            ug[r] = u[at[r / 6] + r % 6];
        }
        // (Its ends' moves in its own axes, and the forces at its ends.)
        let mut ul = [0.0; 12];
        for blk in 0..4 {
            for r in 0..3 {
                ul[blk * 3 + r] = (0..3).map(|c| t[r][c] * ug[blk * 3 + c]).sum();
            }
        }
        let fl: Vec<f64> = (0..12).map(|r| (0..12).map(|c| kl[r][c] * ul[c]).sum()).collect();
        let moment = |a: f64, b: f64| (a * a + b * b).sqrt();
        out[k] = Some(Forces { axial: fl[6], bending: moment(fl[4], fl[5]).max(moment(fl[10], fl[11])), torsion: fl[3].abs() });
    }
    Ok(out)
}

/// How a frame does under a load case: each member's outcome, and whether what
/// broke left it in pieces (what's left loose: it falls apart).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Collapse {
    pub members: Vec<Outcome>,
    pub falls_apart: bool,
}

/// `frame` under `case`, breaking: each round, every member past what it breaks
/// at goes, and the rest is solved again, until nothing more breaks, or it falls
/// apart. Loose before anything breaks: `Err`.
pub fn collapse(frame: &Frame, case: &Case, sf: f64) -> Result<Collapse, Loose> {
    let mut gone = vec![false; frame.members.len()];
    let mut out = vec![Outcome::default(); frame.members.len()];
    for round in 1..=frame.members.len().max(1) {
        let forces = match solve(frame, case, &gone) {
            Ok(f) => f,
            Err(e) if round == 1 => return Err(e),
            Err(_) => return Ok(Collapse { members: out, falls_apart: true }),
        };
        let mut broke = false;
        for (k, f) in forces.iter().enumerate() {
            let Some(f) = f else { continue };
            let m = &frame.members[k];
            let w = work(m, *f, frame.joints[m.a].distance(frame.joints[m.b]), sf);
            out[k].work = w;
            if w.breaking >= 1.0 {
                out[k].broken = Some(round);
                gone[k] = true;
                broke = true;
            }
        }
        if !broke {
            return Ok(Collapse { members: out, falls_apart: false });
        }
    }
    Ok(Collapse { members: out, falls_apart: false })
}

/// A member's stiffness in its own axes (along it, and two across), and the
/// turn from the ship's axes into those.
fn element(m: &Member, pa: DVec3, pb: DVec3) -> ([[f64; 12]; 12], [[f64; 3]; 3]) {
    let d = pb - pa;
    let l = d.length().max(1e-6);
    let x = d / l;
    let up = if x.y.abs() > 0.95 { DVec3::X } else { DVec3::Y };
    let z = x.cross(up).normalize();
    let y = z.cross(x);
    let t = [x.to_array(), y.to_array(), z.to_array()];
    let (e, g) = (m.material.stiffness, m.material.shear);
    let (a, i) = (m.section.area(), m.section.inertia());
    let j = 2.0 * i;
    let mut k = [[0.0; 12]; 12];
    let mut set = |r: usize, c: usize, v: f64| {
        k[r][c] += v;
        if r != c {
            k[c][r] += v;
        }
    };
    let (ea, gj) = (e * a / l, g * j / l);
    set(0, 0, ea);
    set(6, 6, ea);
    set(0, 6, -ea);
    set(3, 3, gj);
    set(9, 9, gj);
    set(3, 9, -gj);
    // (Bending in its two planes: across y (turning about z), across z (about y).)
    let (k1, k2, k3, k4) = (12.0 * e * i / l.powi(3), 6.0 * e * i / (l * l), 4.0 * e * i / l, 2.0 * e * i / l);
    for (v, w, s) in [(1usize, 5usize, 1.0f64), (2, 4, -1.0)] {
        set(v, v, k1);
        set(v + 6, v + 6, k1);
        set(v, v + 6, -k1);
        set(v, w, s * k2);
        set(v, w + 6, s * k2);
        set(v + 6, w, -s * k2);
        set(v + 6, w + 6, -s * k2);
        set(w, w, k3);
        set(w + 6, w + 6, k3);
        set(w, w + 6, k4);
    }
    (k, t)
}

/// A member's stiffness turned into the ship's axes.
fn to_global(kl: &[[f64; 12]; 12], t: &[[f64; 3]; 3]) -> [[f64; 12]; 12] {
    // (T is four of the 3x3 turns down its diagonal: K = Tᵀ k T.)
    let tt = |r: usize, c: usize| if r / 3 == c / 3 { t[r % 3][c % 3] } else { 0.0 };
    let kt: [[f64; 12]; 12] = std::array::from_fn(|r| std::array::from_fn(|c| (0..12).map(|m| kl[r][m] * tt(m, c)).sum()));
    std::array::from_fn(|r| std::array::from_fn(|c| (0..12).map(|m| tt(m, r) * kt[m][c]).sum()))
}

/// `a x = b` for x (`a` n by n), by elimination with the largest pivot; a pivot
/// next to nothing (beside the matrix's biggest) means something moves freely.
fn gauss(mut a: Vec<f64>, mut b: Vec<f64>, n: usize) -> Result<Vec<f64>, Loose> {
    let scale = a.iter().fold(0.0f64, |m, v| m.max(v.abs())).max(1e-30);
    for col in 0..n {
        let p = (col..n).max_by(|&r, &s| a[r * n + col].abs().total_cmp(&a[s * n + col].abs())).unwrap_or(col);
        if a[p * n + col].abs() < scale * 1e-13 {
            return Err(Loose);
        }
        if p != col {
            for c in 0..n {
                a.swap(p * n + c, col * n + c);
            }
            b.swap(p, col);
        }
        let piv = a[col * n + col];
        for r in col + 1..n {
            let f = a[r * n + col] / piv;
            if f != 0.0 {
                for c in col..n {
                    a[r * n + c] -= f * a[col * n + c];
                }
                b[r] -= f * b[col];
            }
        }
    }
    let mut x = vec![0.0; n];
    for r in (0..n).rev() {
        let s: f64 = (r + 1..n).map(|c| a[r * n + c] * x[c]).sum();
        x[r] = (b[r] - s) / a[r * n + r];
    }
    Ok(x)
}

#[cfg(test)]
mod tests {
    use super::*;

    const STEEL: Material = Material { stiffness: 200e9, shear: 77e9, yield_strength: 1300e6, tensile_strength: 1420e6, density: 7850.0 };
    const TUBE: Section = Section { diameter: 0.15, wall: 0.01 };

    /// A cantilever bends as the textbook has it (tip load P, length L: the
    /// moment at its root P L), a column pulled carries its load, and one past
    /// its strength breaks; one held at nothing is loose.
    #[test]
    fn beams_bear_and_break() {
        let frame = Frame { joints: vec![DVec3::ZERO, DVec3::new(2.0, 0.0, 0.0)], members: vec![Member { a: 0, b: 1, section: TUBE, material: STEEL }] };
        let p = 1000.0;
        let case = Case { loads: vec![(1, DVec3::new(0.0, -p, 0.0))], anchor: Some(0), ..Default::default() };
        let f = solve(&frame, &case, &[]).unwrap()[0].unwrap();
        assert!((f.bending - p * 2.0).abs() < 1.0, "{f:?}");
        let pull = Case { loads: vec![(1, DVec3::new(5000.0, 0.0, 0.0))], anchor: Some(0), ..Default::default() };
        assert!((solve(&frame, &pull, &[]).unwrap()[0].unwrap().axial - 5000.0).abs() < 1.0);
        let hard = Case { loads: vec![(1, DVec3::new(TUBE.area() * 1500e6, 0.0, 0.0))], anchor: Some(0), ..Default::default() };
        assert_eq!(collapse(&frame, &hard, 1.5).ok().map(|o| o.members[0].broken), Some(Some(1)));
        assert_eq!(solve(&frame, &Case { loads: vec![(1, DVec3::Y)], ..Default::default() }, &[]), Err(Loose));
    }
}
