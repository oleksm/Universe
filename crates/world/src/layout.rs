//! Ship layouts (the registry's SFO 18, generated into `content/base/layouts.ron`
//! by `tools/standards/build.py`): a hull's inside as compartments and the
//! openings between them. From a layout, a modelled hull gets its interior:
//! each compartment lined (floor, ceiling, walls; openings cut through), walked
//! on and bumped into like the hull, drawn; ladder shafts and hatches climbed.
//!
//! A layout's frame is the registry's: metres back from the nose, above the
//! keel, from the centre line (starboard positive). In the hull's own frame
//! (−Z forward, +Y up, centred on its centre of mass) that's placed by the
//! model's nose and keel (its `Hull_*` meshes' foremost and lowest points).

use glam::DVec3;
use serde::Deserialize;

/// A hull's layout.
#[derive(Clone, Debug, Deserialize)]
pub struct Layout {
    /// The hull it lays out (the registry's file name: `mc-07`).
    pub hull: String,
    pub compartments: Vec<Compartment>,
    pub openings: Vec<Opening>,
}

/// A space: its purpose, address, whether it's sealed, and its boxes
/// (aft from, aft to, up from, up to, side from, side to; m).
#[derive(Clone, Debug, Deserialize)]
pub struct Compartment {
    pub name: String,
    pub function: String,
    pub deck: u32,
    pub section: String,
    pub unit: u32,
    pub pressurised: bool,
    pub boxes: Vec<(f64, f64, f64, f64, f64, f64)>,
}

/// A way between compartments `a` and `b` (or `outside`), the middle of its
/// sill at `at` (aft, up, side): a doorway `width` across and `height` tall in
/// a wall; in a floor or ceiling, `width` across the ship and `height` along it.
#[derive(Clone, Debug, Deserialize)]
pub struct Opening {
    pub kind: String,
    pub a: String,
    pub b: String,
    pub at: (f64, f64, f64),
    pub width: f64,
    pub height: f64,
    pub seals: bool,
}

/// How far each compartment's lining stands in from its boxes (m): two
/// compartments' walls back to back, not in one plane.
const INSET: f64 = 0.02;

/// A compartment placed in the hull's frame.
#[derive(Clone, Debug, PartialEq)]
pub struct Room {
    pub name: String,
    pub function: String,
    /// Its address (SFO 9): unit, section, deck.
    pub address: String,
    pub pressurised: bool,
    /// Its boxes (low and high corners, hull frame).
    pub boxes: Vec<(DVec3, DVec3)>,
}

/// What lines a surface: a floor, a wall or a ceiling.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Face {
    Floor,
    Wall,
    Ceiling,
}

/// A hull's interior, in its own frame: its rooms, the lining (each piece a
/// rectangle: its corners in order, what it is, whose room), where one climbs.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Interior {
    pub rooms: Vec<Room>,
    pub panels: Vec<([DVec3; 4], Face, usize)>,
    /// Ladder shafts and the columns under hatches: boxes (low, high).
    pub climbs: Vec<(DVec3, DVec3)>,
}

impl Interior {
    /// The lining as triangles (for walking on).
    pub fn triangles(&self) -> Vec<[DVec3; 3]> {
        self.panels.iter().flat_map(|(q, _, _)| [[q[0], q[1], q[2]], [q[0], q[2], q[3]]]).collect()
    }

    /// The room `p` (hull frame) is in, if any.
    pub fn room_at(&self, p: DVec3) -> Option<&Room> {
        self.rooms.iter().find(|r| r.boxes.iter().any(|(lo, hi)| p.cmpge(*lo).all() && p.cmple(*hi).all()))
    }

    /// Is `p` (hull frame) where one climbs (a ladder, a hatch's column)?
    pub fn climbing(&self, p: DVec3) -> bool {
        self.climbs.iter().any(|(lo, hi)| p.cmpge(*lo).all() && p.cmple(*hi).all())
    }
}

/// An axis-aligned rectangle in a face's own two coordinates.
#[derive(Clone, Copy, Debug)]
struct Rect {
    u0: f64,
    u1: f64,
    v0: f64,
    v1: f64,
}

impl Rect {
    /// What's left of `self` with `hole` taken out: up to four pieces.
    fn minus(self, hole: Rect) -> Vec<Rect> {
        let (u0, u1, v0, v1) = (hole.u0.max(self.u0), hole.u1.min(self.u1), hole.v0.max(self.v0), hole.v1.min(self.v1));
        if u0 >= u1 - 1e-6 || v0 >= v1 - 1e-6 {
            return vec![self];
        }
        let mut out = Vec::new();
        let mut push = |r: Rect| {
            if r.u1 - r.u0 > 1e-6 && r.v1 - r.v0 > 1e-6 {
                out.push(r);
            }
        };
        push(Rect { u0: self.u0, u1: u0, v0: self.v0, v1: self.v1 });
        push(Rect { u0: u1, u1: self.u1, v0: self.v0, v1: self.v1 });
        push(Rect { u0, u1, v0: self.v0, v1: v0 });
        push(Rect { u0, u1, v0: v1, v1: self.v1 });
        out
    }
}

impl Layout {
    /// Its interior in the hull's frame, the model's nose and keel at `nose`
    /// (its foremost z) and `keel` (its lowest y) there.
    pub fn interior(&self, nose: f64, keel: f64) -> Interior {
        // (Layout frame (aft, up, side) → hull frame (x side, y up, z forward−).)
        let at = |aft: f64, up: f64, side: f64| DVec3::new(side, up + keel, aft + nose);
        let mut interior = Interior::default();
        // Each box as [(lo, hi); 3] over aft, up, side.
        let boxes: Vec<Vec<[(f64, f64); 3]>> = self.compartments.iter().map(|c| c.boxes.iter().map(|b| [(b.0, b.1), (b.2, b.3), (b.4, b.5)]).collect()).collect();
        for (ci, c) in self.compartments.iter().enumerate() {
            let placed = boxes[ci].iter().map(|b| (at(b[0].0, b[1].0, b[2].0), at(b[0].1, b[1].1, b[2].1))).collect();
            interior.rooms.push(Room { name: c.name.clone(), function: c.function.clone(), address: format!("UNIT {}, SECTION {}, DECK {}", c.unit, c.section, c.deck), pressurised: c.pressurised, boxes: placed });
            if c.function == "shaft" {
                for b in &boxes[ci] {
                    interior.climbs.push((at(b[0].0, b[1].0, b[2].0), at(b[0].1, b[1].1, b[2].1)));
                }
            }
            for (bi, b) in boxes[ci].iter().enumerate() {
                // Its six faces: along axis k (0 aft, 1 up, 2 side), at its low or high end.
                for k in 0..3 {
                    for end in 0..2 {
                        let plane = if end == 0 { b[k].0 } else { b[k].1 };
                        // The face's own axes: the other two, in order.
                        let (ua, va) = match k {
                            0 => (2, 1),
                            1 => (2, 0),
                            _ => (0, 1),
                        };
                        let mut pieces = vec![Rect { u0: b[ua].0, u1: b[ua].1, v0: b[va].0, v1: b[va].1 }];
                        // Not where it meets another of its own boxes (the room goes on).
                        for (oj, o) in boxes[ci].iter().enumerate() {
                            if oj == bi {
                                continue;
                            }
                            let other = if end == 0 { o[k].1 } else { o[k].0 };
                            if (other - plane).abs() < 1e-6 {
                                let hole = Rect { u0: o[ua].0, u1: o[ua].1, v0: o[va].0, v1: o[va].1 };
                                pieces = pieces.into_iter().flat_map(|r| r.minus(hole)).collect();
                            }
                        }
                        // Openings of this room on this face: cut through.
                        for op in self.openings.iter().filter(|o| o.a == c.name || o.b == c.name) {
                            let p = [op.at.0, op.at.1, op.at.2];
                            let on = (p[k] - plane).abs() < 0.011 && (b[ua].0 - 0.011..=b[ua].1 + 0.011).contains(&p[ua]) && (b[va].0 - 0.011..=b[va].1 + 0.011).contains(&p[va]);
                            // (Doors are in walls; hatches, chutes and ramps in floors and ceilings.)
                            let level = !matches!(op.kind.as_str(), "door" | "pressure-door");
                            if !on || level != (k == 1) {
                                continue;
                            }
                            let hole = if k == 1 {
                                // In a floor or ceiling: across (side) and along (aft).
                                Rect { u0: p[2] - op.width / 2.0, u1: p[2] + op.width / 2.0, v0: p[0] - op.height / 2.0, v1: p[0] + op.height / 2.0 }
                            } else {
                                // In a wall: across its face, up from the sill.
                                Rect { u0: p[ua] - op.width / 2.0, u1: p[ua] + op.width / 2.0, v0: p[1], v1: p[1] + op.height }
                            };
                            pieces = pieces.into_iter().flat_map(|r| r.minus(hole)).collect();
                            // A hatch's column is climbed: from this room's floor below it up through.
                            if k == 1 && end == 1 && matches!(op.kind.as_str(), "hatch" | "cargo-hatch" | "chute") {
                                interior.climbs.push((at(hole.v0 - 0.3, b[1].0, hole.u0 - 0.3), at(hole.v1 + 0.3, plane + 2.2, hole.u1 + 0.3)));
                            }
                        }
                        // Each piece, set in from the box, facing into the room.
                        let face = match (k, end) {
                            (1, 0) => Face::Floor,
                            (1, _) => Face::Ceiling,
                            _ => Face::Wall,
                        };
                        let inward = if end == 0 { INSET } else { -INSET };
                        for r in pieces {
                            let corner = |u: f64, v: f64| {
                                let mut q = [0.0; 3];
                                q[k] = plane + inward;
                                q[ua] = u;
                                q[va] = v;
                                at(q[0], q[1], q[2])
                            };
                            let mut quad = [corner(r.u0, r.v0), corner(r.u1, r.v0), corner(r.u1, r.v1), corner(r.u0, r.v1)];
                            // (Wound to face into the room.)
                            let n = (quad[1] - quad[0]).cross(quad[2] - quad[0]);
                            let into = {
                                let mut d = [0.0; 3];
                                d[k] = if end == 0 { 1.0 } else { -1.0 };
                                DVec3::new(d[2], d[1], d[0])
                            };
                            if n.dot(into) < 0.0 {
                                quad.reverse();
                            }
                            interior.panels.push((quad, face, ci));
                        }
                    }
                }
            }
        }
        interior
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn two_rooms_and_a_door_line_each_other_with_the_doorway_cut_through() {
        let room = |name: &str, a0: f64, a1: f64| Compartment { name: name.into(), function: "corridor".into(), deck: 1, section: "A".into(), unit: 1, pressurised: true, boxes: vec![(a0, a1, 0.0, 2.5, -1.0, 1.0)] };
        let layout = Layout {
            hull: "test".into(),
            compartments: vec![room("Fore", 0.0, 4.0), room("Aft", 4.0, 8.0)],
            openings: vec![Opening { kind: "door".into(), a: "Fore".into(), b: "Aft".into(), at: (4.0, 0.0, 0.0), width: 0.8, height: 2.0, seals: false }],
        };
        let i = layout.interior(0.0, 0.0);
        // Each room: floor, ceiling, three whole walls, and the wall with the door in three pieces.
        assert_eq!(i.panels.iter().filter(|p| p.2 == 0).count(), 2 + 3 + 3);
        // Through the doorway's middle (a metre up, at the boundary): no panel there.
        let through = DVec3::new(0.0, 1.0, 4.0);
        assert!(!i.panels.iter().any(|(q, f, _)| *f == Face::Wall && (q[0].z - 4.0).abs() < 0.05 && {
            let (lo, hi) = (q.iter().fold(DVec3::MAX, |a, b| a.min(*b)), q.iter().fold(DVec3::MIN, |a, b| a.max(*b)));
            through.x >= lo.x && through.x <= hi.x && through.y >= lo.y && through.y <= hi.y
        }));
        assert_eq!(i.room_at(DVec3::new(0.0, 1.0, 6.0)).map(|r| r.name.as_str()), Some("Aft"));
    }
}
