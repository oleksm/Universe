//! Designed hulls. A design is a hull drawn up from a few numbers: the body
//! (how long, wide and high, how it tapers to the nose and the tail), wings
//! and fins, its size class and how many cargo racks, hardpoints and
//! utility slots it has, how many drive nozzles, and where things go along
//! it — the thruster quads and the belly lift, the engine room, the tank,
//! the hold and the bridge. From those, its shape (body, wings, fins: convex
//! parts), its nodes (nozzles, mounts, gear, cockpit), its frame's mass and
//! strength (from its size) and price, and a stock fit (the cheapest module
//! that fits each slot, a plant big enough for them): a hull like any other.
//!
//! Nothing is balanced for you: where the masses sit and where the
//! thrusters push from are the designer's, and the physics says what it
//! costs (see `ClassSpec::authority`). Commissioned, a design joins the
//! hulls (`content().hulls`) for good, by a key its numbers make, so a ship
//! built to it is the same hull wherever it's loaded.

use glam::DVec3;
use serde::{Deserialize, Serialize};

use crate::content::content;
use crate::modules::{Does, Module, SlotKind};
use crate::ship::{ClassSpec, Hull};

/// The frame's mass per square metre of its size (V^⅔: its skin, roughly), kg.
pub(crate) const FRAME_PER_AREA: f64 = 108.0;
/// What a kilogram of frame costs, and a size of slot (credits).
pub(crate) const PRICE_PER_KG: f64 = 3.0;
pub(crate) const PRICE_PER_SLOT_SIZE: f64 = 2000.0;
/// The energy that wrecks the hull per kilogram of frame (J).
pub(crate) const STRENGTH_PER_KG: f64 = 600.0;

/// A hull as designed (metres; positions along the body as shares of its
/// length from its middle, + aft).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Design {
    pub name: String,
    pub length: f64,
    pub width: f64,
    pub height: f64,
    /// The nose's and the tail's section, as a share of the body's.
    pub nose: f64,
    pub tail: f64,
    /// Each wing's reach out past the body (0: none), and how far aft its tip sweeps.
    pub wing_span: f64,
    pub wing_sweep: f64,
    pub fins: bool,
    /// Its size class (1..4): how big its slots are.
    pub class: u8,
    pub racks: u8,
    pub hardpoints: u8,
    pub utility: u8,
    /// Drive nozzles across its tail (1..4).
    pub mains: u8,
    /// The thruster quads: how far apart fore and aft (share of the length),
    /// and their middle (share, + aft).
    pub quads_spread: f64,
    pub quads_at: f64,
    /// The belly lift's middle (share, + aft).
    pub lift_at: f64,
    /// Where the engine room (plant, drive, hyperdrive), the tank, the hold
    /// and the bridge (computers, life support) sit (shares, + aft).
    pub engines_at: f64,
    pub tank_at: f64,
    pub hold_at: f64,
    pub bridge_at: f64,
}

impl Default for Design {
    fn default() -> Self {
        Design {
            name: "DESIGN".into(),
            // (The MC-07's size: room for equipment at its real size, a drive of 387 m³.)
            length: 66.0,
            width: 31.0,
            height: 19.0,
            nose: 0.35,
            tail: 0.85,
            wing_span: 8.0,
            wing_sweep: 6.0,
            fins: false,
            class: 2,
            racks: 1,
            hardpoints: 1,
            utility: 1,
            mains: 2,
            quads_spread: 0.32,
            quads_at: 0.05,
            lift_at: 0.05,
            engines_at: 0.35,
            tank_at: 0.1,
            hold_at: -0.15,
            bridge_at: -0.38,
        }
    }
}

/// A design's number: what it's called, its least and most, its step.
pub struct Knob {
    pub label: &'static str,
    pub min: f64,
    pub max: f64,
    pub step: f64,
    /// What it is; what turning it up does for the ship and costs it; turning it down.
    pub about: &'static str,
    pub more: &'static str,
    pub less: &'static str,
}

/// The numbers a designer turns, in order (see `Design::knob`).
pub const KNOBS: &[Knob] = &[
    Knob { label: "LENGTH M", min: 16.0, max: 120.0, step: 2.0, about: "THE BODY'S LENGTH, NOSE TO TAIL.", more: "THRUSTERS AND LIFT SIT FARTHER APART: MORE LEVER TO TURN AND TRIM. BUT A HEAVIER, DEARER FRAME, AND MORE MASS TO SWING.", less: "A LIGHT, CHEAP FRAME, QUICK TO SWING ROUND. BUT SHORT LEVERS: LESS TURN FROM THE SAME THRUSTERS, TOUCHIER BALANCE." },
    Knob { label: "WIDTH M", min: 4.0, max: 40.0, step: 1.0, about: "THE BODY'S WIDTH.", more: "THE TOP THRUSTERS SIT WIDER: MORE ROLL. BUT A HEAVIER FRAME, MORE DRAG IN AIR (FRONTAL AREA).", less: "LESS DRAG, A LIGHTER FRAME. BUT WEAKER ROLL." },
    Knob { label: "HEIGHT M", min: 3.0, max: 30.0, step: 1.0, about: "THE BODY'S HEIGHT.", more: "A HEAVIER FRAME WITH MORE DRAG IN AIR, FOR LITTLE BACK (NO EXTRA ROOM IS COUNTED YET).", less: "A FLATTER HULL: LIGHTER, LESS DRAG. NOTHING LOST." },
    Knob { label: "NOSE TAPER", min: 0.15, max: 1.0, step: 0.05, about: "HOW THE BODY NARROWS TO THE NOSE (1: NOT AT ALL).", more: "A BLUNT NOSE: MORE FRAME FORWARD, THE CENTRE OF MASS MOVES FORWARD.", less: "A POINTED NOSE: LESS FRAME FORWARD, THE CENTRE OF MASS MOVES AFT." },
    Knob { label: "TAIL TAPER", min: 0.4, max: 1.2, step: 0.05, about: "HOW THE BODY NARROWS (UNDER 1) OR FLARES (OVER 1) AT THE TAIL.", more: "A BROAD TAIL: MORE FRAME AFT (THE CENTRE OF MASS AFT), ROOM FOR BIGGER DRIVE NOZZLES.", less: "A SLIM TAIL: LESS FRAME AFT (THE CENTRE OF MASS FORWARD)." },
    Knob { label: "WING SPAN M", min: 0.0, max: 40.0, step: 1.0, about: "HOW FAR EACH WING REACHES PAST THE BODY (0: NONE).", more: "LOOKS; NO LIFT IN AIR YET. COSTS FRAME MASS FAR OUT (SLOWER ROLL) AND A WIDER HULL TO CLIP OTHERS.", less: "A COMPACT, LIGHTER HULL THAT ROLLS FASTER AND FITS TIGHT PLACES." },
    Knob { label: "WING SWEEP M", min: 0.0, max: 30.0, step: 1.0, about: "HOW FAR AFT THE WINGTIPS SIT.", more: "THE WINGS' MASS MOVES AFT (THE CENTRE OF MASS WITH IT A LITTLE).", less: "STRAIGHTER WINGS: THEIR MASS FURTHER FORWARD." },
    Knob { label: "FINS", min: 0.0, max: 1.0, step: 1.0, about: "A TAIL FIN ON TOP.", more: "LOOKS; A LITTLE FRAME MASS HIGH AND AFT.", less: "NONE: A LITTLE LIGHTER." },
    Knob { label: "SIZE CLASS", min: 1.0, max: 4.0, step: 1.0, about: "HOW BIG A MODULE ITS SLOTS TAKE (1-4).", more: "BIGGER DRIVES, PLANTS, TANKS AND RACKS FIT: MORE PUSH, RANGE AND HOLD. THE FRAME COSTS MORE (PRICED BY SLOT SIZE).", less: "A CHEAPER FRAME. BUT ONLY SMALL MODULES FIT." },
    Knob { label: "CARGO RACKS", min: 0.0, max: 3.0, step: 1.0, about: "HOW MANY CARGO SLOTS (0-3).", more: "MORE HOLD (WITH RACKS FITTED): MORE TO HAUL. EACH SLOT ADDS TO THE PRICE; A FULL HOLD IS MASS TO PUSH AND TO BALANCE.", less: "A LIGHTER, CHEAPER SHIP THAT CARRIES LITTLE OR NOTHING." },
    Knob { label: "HARDPOINTS", min: 0.0, max: 3.0, step: 1.0, about: "HOW MANY GUN SLOTS (0-3), UNDER THE NOSE.", more: "MORE GUNS TO FIT. EACH ADDS TO THE PRICE, AND GUNS ARE MASS AT THE NOSE.", less: "CHEAPER; NOTHING TO FIGHT WITH (THE SAMS AND YOUR SPEED ARE ALL)." },
    Knob { label: "UTILITY SLOTS", min: 0.0, max: 2.0, step: 1.0, about: "HOW MANY SLOTS FOR GEAR LIKE A MINING RIG (0-2), ON THE SPINE.", more: "ROOM FOR GEAR: A MINING RIG TO DIG. EACH ADDS TO THE PRICE.", less: "CHEAPER; NO GEAR (NO MINING)." },
    Knob { label: "DRIVE NOZZLES", min: 1.0, max: 4.0, step: 1.0, about: "HOW MANY NOZZLES THE MAIN DRIVE FIRES THROUGH, ACROSS THE TAIL.", more: "THE SAME PUSH SHARED WIDER: SMALLER NOZZLES ACROSS THE TAIL. LOOKS, FOR NOW.", less: "THE SAME PUSH THROUGH FEWER, BIGGER NOZZLES." },
    Knob { label: "THRUSTER SPREAD", min: 0.1, max: 0.48, step: 0.02, about: "HOW FAR THE FRONT AND BACK THRUSTER SETS SIT FROM THEIR CENTRE (SHARE OF THE LENGTH).", more: "A LONGER LEVER: FASTER PITCH AND YAW FROM THE SAME THRUSTERS. TOO FAR AND THEY CROWD THE NOSE AND TAIL.", less: "SLUGGISH TURNING: THE SETS PUSH AGAINST EACH OTHER WITH LITTLE LEVER." },
    Knob { label: "THRUSTERS CENTRE", min: -0.4, max: 0.4, step: 0.01, about: "WHERE THE MIDDLE OF THE THRUSTER SETS SITS (+: AFT). PUT IT ON THE CENTRE OF MASS (+).", more: "AFT OF THE CENTRE OF MASS: SHOVES UP OR SIDEWAYS TURN THE SHIP, SO THE FRONT SET WORKS HARDER AND YOU LOSE PUSH.", less: "FORWARD OF IT: THE SAME, THE OTHER WAY. ON IT: FULL PUSH, NO TURN." },
    Knob { label: "LIFT CENTRE", min: -0.4, max: 0.4, step: 0.01, about: "WHERE THE BELLY LIFT'S MIDDLE SITS (+: AFT). PUT IT ON THE CENTRE OF MASS TO HOVER LEVEL.", more: "AFT OF THE CENTRE OF MASS: THE NOSE DIPS UNDER LIFT; TRIMMING IT LEVEL COSTS LIFT (HOVERING, LANDING).", less: "FORWARD OF IT: THE TAIL DIPS. ON IT: ALL THE LIFT, LEVEL." },
    Knob { label: "ENGINE ROOM AT", min: -0.48, max: 0.48, step: 0.02, about: "WHERE THE PLANT, DRIVE AND HYPERDRIVE SIT (+: AFT). THE HEAVIEST THINGS ABOARD.", more: "FURTHER AFT: THE CENTRE OF MASS MOVES AFT. MOVE THE THRUSTERS AND LIFT AFT WITH IT.", less: "FURTHER FORWARD: THE CENTRE OF MASS MOVES FORWARD." },
    Knob { label: "TANK AT", min: -0.48, max: 0.48, step: 0.02, about: "WHERE THE FUEL SITS (+: AFT). ITS MASS GOES AS YOU BURN IT.", more: "OFF THE CENTRE OF MASS (EITHER WAY) THE BALANCE SHIFTS AS THE TANK EMPTIES. KEEP IT NEAR THE CENTRE.", less: "(THE SAME: CLOSE TO THE CENTRE OF MASS, BURNING FUEL BARELY MOVES IT.)" },
    Knob { label: "HOLD AT", min: -0.48, max: 0.48, step: 0.02, about: "WHERE THE CARGO SITS (+: AFT). FULL OR EMPTY, A BIG MASS THAT COMES AND GOES.", more: "OFF THE CENTRE OF MASS (EITHER WAY) THE SHIP FLIES DIFFERENTLY LOADED AND EMPTY. KEEP IT NEAR THE CENTRE.", less: "(THE SAME: NEAR THE CENTRE, LOADED OR EMPTY IT KEEPS ITS BALANCE.)" },
    Knob { label: "BRIDGE AT", min: -0.48, max: 0.48, step: 0.02, about: "WHERE THE COMPUTERS, SENSORS AND LIFE SUPPORT SIT (+: AFT). LIGHT: A FINE TRIM.", more: "AFT: A LITTLE MASS AFT.", less: "FORWARD: A LITTLE MASS FORWARD (THE CLASSIC PLACE, UP FRONT)." },
];

impl Design {
    /// Number `k` of `KNOBS`, as it stands.
    pub fn knob(&self, k: usize) -> f64 {
        match k {
            0 => self.length,
            1 => self.width,
            2 => self.height,
            3 => self.nose,
            4 => self.tail,
            5 => self.wing_span,
            6 => self.wing_sweep,
            7 => f64::from(u8::from(self.fins)),
            8 => f64::from(self.class),
            9 => f64::from(self.racks),
            10 => f64::from(self.hardpoints),
            11 => f64::from(self.utility),
            12 => f64::from(self.mains),
            13 => self.quads_spread,
            14 => self.quads_at,
            15 => self.lift_at,
            16 => self.engines_at,
            17 => self.tank_at,
            18 => self.hold_at,
            _ => self.bridge_at,
        }
    }

    /// Turn number `k` by `steps` of its step (kept within its bounds).
    pub fn turn(&mut self, k: usize, steps: f64) {
        let Some(knob) = KNOBS.get(k) else { return };
        let v = ((self.knob(k) + steps * knob.step) / knob.step).round() * knob.step;
        let v = v.clamp(knob.min, knob.max);
        let n = v.round() as u8;
        match k {
            0 => self.length = v,
            1 => self.width = v,
            2 => self.height = v,
            3 => self.nose = v,
            4 => self.tail = v,
            5 => self.wing_span = v,
            6 => self.wing_sweep = v,
            7 => self.fins = n > 0,
            8 => self.class = n,
            9 => self.racks = n,
            10 => self.hardpoints = n,
            11 => self.utility = n,
            12 => self.mains = n,
            13 => self.quads_spread = v,
            14 => self.quads_at = v,
            15 => self.lift_at = v,
            16 => self.engines_at = v,
            17 => self.tank_at = v,
            18 => self.hold_at = v,
            _ => self.bridge_at = v,
        }
    }

    /// A design after hull `s` (a copy to change): its body's size and
    /// tapers, its wings and fins, its slots, its drive nozzles, and where
    /// its thrusters and masses sit — read off its shape, nodes and slots.
    /// Close, not the same: this draws every body the one way.
    pub fn after(s: &ClassSpec) -> Design {
        use crate::ship::ThrusterRole;
        let shape = s.shape();
        let pts = &shape.mesh.points;
        let body: Vec<DVec3> = shape.part_points.first().map_or_else(|| pts.clone(), |r| pts[r.clone()].to_vec());
        let (lo, hi) = body.iter().fold((DVec3::splat(f64::INFINITY), DVec3::splat(f64::NEG_INFINITY)), |(a, b), p| (a.min(*p), b.max(*p)));
        let (length, width, height) = ((hi.z - lo.z).max(4.0), (hi.x - lo.x).max(2.0), (hi.y - lo.y).max(2.0));
        let mid = 0.5 * (lo.z + hi.z);
        // How wide the body is near its nose and its tail, against its widest.
        let width_near = |z: f64| body.iter().filter(|p| (p.z - z).abs() < length * 0.08).map(|p| p.x.abs()).fold(0.0, f64::max) * 2.0;
        let share = |z: f64| ((z - mid) / length).clamp(-0.48, 0.48);
        // Its other parts: wide and flat, wings; thin and tall, a fin.
        let (mut wing_span, mut wing_sweep, mut fins) = (0.0f64, 0.0f64, false);
        for r in shape.part_points.iter().skip(1) {
            let part = &pts[r.clone()];
            let (plo, phi) = part.iter().fold((DVec3::splat(f64::INFINITY), DVec3::splat(f64::NEG_INFINITY)), |(a, b), p| (a.min(*p), b.max(*p)));
            if phi.x - plo.x < 1.0 {
                fins = true;
            } else {
                wing_span = wing_span.max(phi.x.max(-plo.x) - width * 0.5);
                let tip = part.iter().map(|p| p.x.abs()).fold(0.0, f64::max);
                let tip_fore = part.iter().filter(|p| p.x.abs() > tip - 0.5).map(|p| p.z).fold(f64::INFINITY, f64::min);
                let root_fore = part.iter().filter(|p| p.x.abs() < tip * 0.6).map(|p| p.z).fold(f64::INFINITY, f64::min);
                if tip_fore.is_finite() && root_fore.is_finite() {
                    wing_sweep = wing_sweep.max(tip_fore - root_fore);
                }
            }
        }
        let count = |k: SlotKind| s.slots.iter().filter(|sl| sl.kind == k).count() as u8;
        let size = |k: SlotKind| s.slots.iter().filter(|sl| sl.kind == k).map(|sl| sl.size).max().unwrap_or(1);
        let mount = |slot: &str| shape.node(&format!("mount_{slot}")).map(|n| share(n.at.z));
        let mean = |k: SlotKind| {
            let z: Vec<f64> = s.slots.iter().filter(|sl| sl.kind == k).filter_map(|sl| mount(&sl.name)).collect();
            (!z.is_empty()).then(|| z.iter().sum::<f64>() / z.len() as f64)
        };
        let rcs: Vec<f64> = s.thrusters.iter().filter(|t| t.role == ThrusterRole::Rcs).map(|t| t.at.z).collect();
        let lift: Vec<f64> = s.thrusters.iter().filter(|t| t.role == ThrusterRole::Lift).map(|t| t.at.z).collect();
        let (fore, aft) = (rcs.iter().copied().fold(f64::INFINITY, f64::min), rcs.iter().copied().fold(f64::NEG_INFINITY, f64::max));
        let d = Design::default();
        let mut out = Design {
            name: format!("COPY OF {}", s.name),
            length,
            width,
            height,
            nose: (width_near(lo.z) / width).clamp(0.15, 1.0),
            tail: (width_near(hi.z) / width).clamp(0.4, 1.2),
            wing_span: wing_span.clamp(0.0, 40.0),
            wing_sweep: wing_sweep.clamp(0.0, 30.0),
            fins,
            class: size(SlotKind::Drive).clamp(1, 4),
            racks: count(SlotKind::Cargo).min(3),
            hardpoints: count(SlotKind::Hardpoint).min(3),
            utility: count(SlotKind::Utility).min(2),
            mains: (s.thrusters.iter().filter(|t| t.role == ThrusterRole::Main).count() as u8).clamp(1, 4),
            quads_spread: if rcs.is_empty() { d.quads_spread } else { ((aft - fore) / 2.0 / length).clamp(0.1, 0.48) },
            quads_at: if rcs.is_empty() { d.quads_at } else { share(0.5 * (fore + aft)) },
            lift_at: if lift.is_empty() { d.lift_at } else { share(lift.iter().sum::<f64>() / lift.len() as f64) },
            engines_at: mount("power").unwrap_or(d.engines_at),
            tank_at: mean(SlotKind::Tank).unwrap_or(d.tank_at),
            hold_at: mean(SlotKind::Cargo).unwrap_or(d.hold_at),
            bridge_at: mount("computer").unwrap_or(d.bridge_at),
        };
        // (On the knobs' steps, within their bounds.)
        for k in 0..KNOBS.len() {
            out.turn(k, 0.0);
        }
        out
    }

    /// The key its numbers make (the same design, the same hull).
    pub fn key(&self) -> String {
        let text = ron::to_string(self).unwrap_or_default();
        let hash = text.bytes().fold(0xcbf2_9ce4_8422_2325u64, |h, b| (h ^ b as u64).wrapping_mul(0x100_0000_01b3));
        format!("design.{hash:016x}")
    }

    /// Its slots: (name, kind, size).
    fn slots(&self) -> Vec<(String, SlotKind, u8)> {
        standard_slots(self.class, self.racks, self.hardpoints, self.utility)
    }

    /// The body's half-width and half-height at `z` (it tapers to the nose
    /// and the tail).
    fn section(&self, z: f64) -> (f64, f64) {
        let (l, w, h) = (self.length, self.width * 0.5, self.height * 0.5);
        let f = if z < -l / 6.0 {
            let t = (z + l / 2.0) / (l / 3.0);
            self.nose + (1.0 - self.nose) * t.clamp(0.0, 1.0)
        } else if z > l / 3.0 {
            let t = (z - l / 3.0) / (l / 6.0);
            1.0 + (self.tail - 1.0) * t.clamp(0.0, 1.0)
        } else {
            1.0
        };
        (w * f, h * f)
    }

    /// Its shape and hull, built (not commissioned: see `commission`).
    pub fn build(&self) -> Result<ClassSpec, String> {
        let (l, half_h) = (self.length, self.height * 0.5);
        if !(self.length > 0.0 && self.width > 0.0 && self.height > 0.0) {
            return Err("it needs a size".into());
        }
        // The body: hexagonal sections at the nose, forward, aft and the tail.
        let mut body = Vec::new();
        for z in [-l / 2.0, -l / 6.0, l / 3.0, l / 2.0] {
            let (w, h) = self.section(z);
            for (x, y) in [(w, 0.0), (-w, 0.0), (w * 0.55, h), (-w * 0.55, h), (w * 0.55, -h), (-w * 0.55, -h)] {
                body.push(DVec3::new(x, y, z));
            }
        }
        let mut parts = Vec::new();
        let (wing_fore, wing_aft) = (-l * 0.1, l * 0.42);
        let wing_y = -half_h * 0.2;
        if self.wing_span > 0.5 {
            let thick = (self.height * 0.06).max(0.3);
            for side in [-1.0, 1.0] {
                let (root_fore, root_aft) = (self.section(wing_fore).0 * 0.9, self.section(wing_aft).0 * 0.9);
                let tip = self.section(0.0).0 + self.wing_span;
                let tip_fore = (wing_fore + self.wing_sweep).min(wing_aft - 1.0);
                let tip_aft = (wing_aft).min(l / 2.0);
                let mut w = Vec::new();
                for (x, z, t) in [(root_fore, wing_fore, thick), (root_aft, wing_aft, thick), (tip, tip_fore, thick * 0.5), (tip, tip_aft, thick * 0.5)] {
                    w.push(DVec3::new(side * x, wing_y + t, z));
                    w.push(DVec3::new(side * x, wing_y - t, z));
                }
                parts.push(w);
            }
        }
        if self.fins {
            let (z0, z1) = (l * 0.22, l * 0.5);
            let top = self.section(z1).1;
            let fin_h = self.height * 0.6;
            parts.push(vec![
                DVec3::new(0.15, top * 0.8, z0),
                DVec3::new(-0.15, top * 0.8, z0),
                DVec3::new(0.15, top * 0.8, z1),
                DVec3::new(-0.15, top * 0.8, z1),
                DVec3::new(0.15, top + fin_h, z1 - 0.15 * l * 0.3),
                DVec3::new(-0.15, top + fin_h, z1 - 0.15 * l * 0.3),
                DVec3::new(0.15, top + fin_h, z1),
                DVec3::new(-0.15, top + fin_h, z1),
            ]);
        }
        // The nodes: the drive across the tail, the quads and the lift, the
        // gear, the cockpit, a mount for each slot.
        let mut nodes = Vec::new();
        let mut thrusters = Vec::new();
        let (tail_w, _) = self.section(l / 2.0);
        let n = self.mains.clamp(1, 4) as usize;
        let mut loops = Vec::new();
        let r = (tail_w * 1.1 / n as f64).min(self.section(l / 2.0).1 * 0.8) * 0.8;
        for k in 0..n {
            let x = if n == 1 { 0.0 } else { -tail_w * 0.55 + tail_w * 1.1 * k as f64 / (n - 1) as f64 };
            let at = DVec3::new(x, 0.0, l / 2.0);
            nodes.push((format!("nozzle_main_{k}"), at, DVec3::Z));
            // (The drive's push shared between its nozzles: two nozzles' worth
            // however many, as a stock hull's two.)
            thrusters.push((format!("nozzle_main_{k}"), "drive".to_string(), 2.0 / n as f64));
            loops.push((0..16).map(|i| at + DVec3::new((i as f64 * std::f64::consts::TAU / 16.0).cos() * r, (i as f64 * std::f64::consts::TAU / 16.0).sin() * r, 0.0)).collect());
        }
        let at = |share: f64| (share * l).clamp(-l / 2.0, l / 2.0);
        let spread = self.quads_spread * l;
        for (end, z, along) in [("nose", at(self.quads_at) - spread, -1.0), ("tail", at(self.quads_at) + spread, 1.0)] {
            let z = z.clamp(-l / 2.0, l / 2.0);
            let (w, h) = self.section(z);
            for (sx, side) in [(-1.0, "left"), (1.0, "right")] {
                let x = sx * w * 0.6;
                nodes.push((format!("nozzle_{end}_{side}_up"), DVec3::new(x, h, z), DVec3::Y));
                nodes.push((format!("nozzle_{end}_{side}_side"), DVec3::new(sx * w, 0.0, z), DVec3::new(sx, 0.0, 0.0)));
                let along_name = if along < 0.0 { "fore" } else { "aft" };
                nodes.push((format!("nozzle_{end}_{side}_{along_name}"), DVec3::new(x, 0.0, z + along), DVec3::new(0.0, 0.0, along)));
                thrusters.push((format!("nozzle_{end}_{side}_up"), "thrusters".to_string(), 0.5));
                thrusters.push((format!("nozzle_{end}_{side}_side"), "thrusters".to_string(), 1.0));
                thrusters.push((format!("nozzle_{end}_{side}_{along_name}"), "thrusters".to_string(), 1.0));
            }
        }
        let lift_spread = 0.25 * l;
        for (end, z) in [("nose", at(self.lift_at) - lift_spread), ("tail", at(self.lift_at) + lift_spread)] {
            let z = z.clamp(-l / 2.0, l / 2.0);
            let (w, h) = self.section(z);
            for (sx, side) in [(-1.0, "left"), (1.0, "right")] {
                nodes.push((format!("nozzle_lift_{end}_{side}"), DVec3::new(sx * w * 0.5, -h, z), DVec3::NEG_Y));
                thrusters.push((format!("nozzle_lift_{end}_{side}"), "lift".to_string(), 1.0));
            }
        }
        let (gw, gh) = self.section(l * 0.25);
        nodes.push(("gear_0".into(), DVec3::new(-gw * 0.5, -gh, l * 0.25), DVec3::NEG_Y));
        nodes.push(("gear_1".into(), DVec3::new(gw * 0.5, -gh, l * 0.25), DVec3::NEG_Y));
        nodes.push(("gear_2".into(), DVec3::new(0.0, -self.section(-l * 0.3).1, -l * 0.3), DVec3::NEG_Y));
        nodes.push(("cockpit".into(), DVec3::new(0.0, self.section(-l * 0.35).1 * 0.6, -l * 0.35), DVec3::NEG_Z));
        let slots = self.slots();
        for (name, kind, _) in &slots {
            let z = match kind {
                SlotKind::Power | SlotKind::Hyperdrive | SlotKind::Capacitor => at(self.engines_at),
                SlotKind::Drive => at(self.engines_at + 0.05),
                SlotKind::Tank => at(self.tank_at),
                SlotKind::Cargo => at(self.hold_at),
                SlotKind::Computer | SlotKind::Transponder | SlotKind::Sensors | SlotKind::Comm | SlotKind::Avionics | SlotKind::LifeSupport => at(self.bridge_at),
                SlotKind::Thrusters => at(self.quads_at),
                SlotKind::Lift => at(self.lift_at),
                SlotKind::Hardpoint => at(-0.42),
                SlotKind::Utility | SlotKind::Relay => 0.0,
            };
            let y = match kind {
                SlotKind::Lift | SlotKind::Hardpoint => -self.section(z).1 * 0.7,
                SlotKind::Utility => self.section(z).1 * 0.9,
                _ => 0.0,
            };
            nodes.push((format!("mount_{name}"), DVec3::new(0.0, y, z), DVec3::NEG_Z));
        }
        let key = self.key();
        let shape = crate::shape::ShapeDef::made(format!("shape.{key}"), body, parts, loops, nodes).build()?;
        // The frame: its mass from its size, its strength and price from that.
        let frame_mass = FRAME_PER_AREA * shape.solid.volume.powf(2.0 / 3.0);
        let price = PRICE_PER_KG * frame_mass + PRICE_PER_SLOT_SIZE * slots.iter().map(|s| f64::from(s.2)).sum::<f64>();
        let fit = stock_fit(&slots)?;
        let radius = 0.3 * self.length.max(self.width + 2.0 * self.wing_span) * 0.5 + 6.0;
        let drag = self.width * self.height * 0.6;
        let def = crate::ship::HullDef::made(key, self.name.to_uppercase(), shape.key.clone(), frame_mass, price, slots, fit, thrusters, radius, drag, STRENGTH_PER_KG * frame_mass);
        let shape: &'static crate::shape::Shape = Box::leak(Box::new(shape));
        let any = content().shapes.iter().next().map(|(h, _)| h).expect("the content has shapes");
        let module = |k: &str| content().handle::<Module>(k).map(|h| (h, content().get(h)));
        let mut spec = def.build(any, shape, module)?;
        spec.shape_own = Some(shape);
        Ok(spec)
    }

    /// Built, kept (per set of numbers: turning a knob back finds it again).
    pub fn spec(&self) -> Result<&'static ClassSpec, String> {
        use std::collections::HashMap;
        use std::sync::{Mutex, OnceLock};
        static BUILT: OnceLock<Mutex<HashMap<String, Result<&'static ClassSpec, String>>>> = OnceLock::new();
        let key = self.key();
        let cache = BUILT.get_or_init(Default::default);
        if let Some(r) = cache.lock().unwrap_or_else(|e| e.into_inner()).get(&key) {
            return r.clone();
        }
        let built = self.build().map(|s| &*Box::leak(Box::new(s)));
        cache.lock().unwrap_or_else(|e| e.into_inner()).insert(key, built.clone());
        built
    }

    /// Commissioned: a hull among the others for good (the same numbers,
    /// the same hull).
    pub fn commission(&self) -> Result<Hull, String> {
        let spec = self.build()?;
        content().hulls.add(spec)
    }
}

/// A hull's slots by its size class (1..4: how big its slots are), with its
/// cargo racks, hardpoints and utility slots: the base blocks, the lift, the
/// tank, the hyperdrive.
pub(crate) fn standard_slots(class: u8, racks: u8, hardpoints: u8, utility: u8) -> Vec<(String, SlotKind, u8)> {
    let c = class.clamp(1, 4);
    let big = (c + 1).min(4);
    let mut s = vec![
        ("power".to_string(), SlotKind::Power, c),
        ("drive".into(), SlotKind::Drive, c),
        ("thrusters".into(), SlotKind::Thrusters, c),
        ("lift".into(), SlotKind::Lift, c),
        ("tank".into(), SlotKind::Tank, big),
        ("hyperdrive".into(), SlotKind::Hyperdrive, c),
        ("computer".into(), SlotKind::Computer, 1),
        ("transponder".into(), SlotKind::Transponder, 1),
        ("sensors".into(), SlotKind::Sensors, 1),
        ("comm".into(), SlotKind::Comm, 1),
        ("life".into(), SlotKind::LifeSupport, c.min(2)),
        ("avionics".into(), SlotKind::Avionics, 1),
    ];
    for k in 0..racks {
        s.push((if k == 0 { "cargo".to_string() } else { format!("cargo_{}", k + 1) }, SlotKind::Cargo, big));
    }
    for k in 0..hardpoints {
        s.push((format!("hardpoint_{}", k + 1), SlotKind::Hardpoint, 1));
    }
    for k in 0..utility {
        s.push((if k == 0 { "utility".to_string() } else { format!("utility_{}", k + 1) }, SlotKind::Utility, c.min(2)));
    }
    s
}

/// The cheapest module of each kind that fits each slot (a base block, the
/// lift, a hold, a hyperdrive: what a ship flies on; guns and gear left
/// empty), with a plant big enough for the lot.
pub(crate) fn stock_fit(slots: &[(String, SlotKind, u8)]) -> Result<Vec<(String, String)>, String> {
    let c = content();
    let cheapest = |kind: SlotKind, size: u8| c.modules.iter().map(|(_, m)| m).filter(|m| m.does.slot() == kind && m.size <= size).min_by(|a, b| a.price.total_cmp(&b.price));
    let mut fit = Vec::new();
    let mut draw = 0.0;
    // (The tank must hold what the drive burns: the registry has propellant tanks for other engines (SFO 22), and the cheapest tank is a nitrogen bottle.)
    let mut fuel: Option<String> = None;
    for (name, kind, size) in slots {
        if matches!(kind, SlotKind::Power | SlotKind::Hardpoint | SlotKind::Utility) {
            continue;
        }
        let m = match (kind, &fuel) {
            (SlotKind::Tank, Some(f)) => c
                .modules
                .iter()
                .map(|(_, m)| m)
                .filter(|m| m.does.slot() == SlotKind::Tank && m.size <= *size && matches!(&m.does, Does::Tank { holds, .. } if holds == f))
                .min_by(|a, b| a.price.total_cmp(&b.price)),
            _ => cheapest(*kind, *size),
        }
        .ok_or_else(|| format!("no module fits its {} slot", name))?;
        if let Does::Drive { burns, .. } = &m.does {
            fuel = Some(burns.clone());
        }
        draw += m.power;
        fit.push((name.clone(), m.key.clone()));
    }
    let (power, size) = slots.iter().find(|s| s.1 == SlotKind::Power).map(|s| (s.0.clone(), s.2)).ok_or("no power slot")?;
    let plant = c
        .modules
        .iter()
        .map(|(_, m)| m)
        .filter(|m| m.size <= size && matches!(m.does, Does::PowerPlant { output, .. } if output >= draw))
        .min_by(|a, b| a.price.total_cmp(&b.price))
        .ok_or_else(|| format!("no plant that fits makes the {:.1} MW its modules draw", draw / 1e6))?;
    fit.push((power, plant.key.clone()));
    Ok(fit)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_design_builds_a_hull_that_flies_and_commissions_once() {
        let d = Design::default();
        let s = d.spec().expect("the default design goes together");
        let loaded = s.dry_mass + s.fuel_capacity;
        eprintln!(
            "{}: frame {:.1} t, dry {:.1} t, tank {:.0} t, hold {:.0} t, main {:.1} m/s², lift {:.1} m/s², price {:.0}",
            s.key, s.frame.frame_mass / 1e3, s.dry_mass / 1e3, s.fuel_capacity / 1e3, s.hold_capacity / 1e3, s.main_thrust / loaded, s.lift_thrust / loaded, s.frame.price
        );
        assert!(s.main_thrust > 0.0 && s.lift_thrust > 0.0 && s.rcs_thrust > 0.0);
        let a = s.authority(s.fuel_capacity, 0.0);
        assert!(a.lift > 0.0 && a.main > 0.0);
        // Commissioned: a hull among the others, found by its key; again, the same one.
        let h = d.commission().unwrap();
        assert_eq!(content().get(h).key, d.key());
        assert_eq!(d.commission().unwrap(), h);
        assert_eq!(content().handle::<ClassSpec>(&d.key()), Some(h));
        // A ship built to it flies it.
        let mut ship = crate::ship::Ship::new(DVec3::ZERO, DVec3::ZERO, glam::DQuat::IDENTITY);
        ship.class = h;
        ship.refresh();
        ship.fuel = content().get(h).fuel_capacity;
        ship.throttle = 1.0;
        ship.drive(None, 1.0 / 60.0, true);
        assert!(ship.applied.0.z < -0.9 * content().get(h).main_thrust * 0.5, "{:?}", ship.applied);
        // A hull modelled in Blender, imported the same way: it flies too. (The MC-07's model: the
        // little test hull of tools/blender/test_hull.py has no room for equipment at real size.)
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets/models/mc07.glb");
        let visual = "assets/models/mc07.glb";
        let h = crate::import::commission(&std::fs::read(path).unwrap(), visual).unwrap();
        let s = content().get(h);
        let a = s.authority(s.fuel_capacity, 0.0);
        eprintln!("{}: frame {:.1} t, dry {:.1} t, main {:.1} m/s², lift {:.1} m/s², {} nozzles", s.name, s.frame.frame_mass / 1e3, s.dry_mass / 1e3, s.main_thrust / (s.dry_mass + s.fuel_capacity), s.lift_thrust / (s.dry_mass + s.fuel_capacity), s.thrusters.len());
        assert_eq!(s.name, "MC-07");
        assert!(s.main_thrust > 0.0 && s.lift_thrust > 0.0 && s.rcs_thrust > 0.0 && a.lift > 0.0, "{a:?}");
        assert_eq!(s.visual.as_deref(), Some(visual));
        assert_eq!(crate::import::commission(&std::fs::read(path).unwrap(), visual).unwrap(), h, "the same file, the same hull");
    }

}

#[cfg(test)]
mod room {
    use super::*;

    #[test]
    fn every_module_sits_inside_its_hull_and_none_overlaps() {
        // (Not the hulls the registry marks outdated: never sized for real equipment, they fly crowded.)
        let outdated = |k: &str| crate::registry::registry().hulls.iter().any(|h| h.identity.key == k && h.identity.revision == Some(crate::registry::DesignStage::Outdated));
        for (_, h) in content().hulls.iter().filter(|(_, h)| h.key.starts_with("hull.") && !outdated(&h.key)) {
            let shape = h.shape();
            let placed = h.layout();
            assert_eq!(placed.len(), h.fit.len(), "{}: every module placed", h.key);
            for p in placed {
                for k in 0..8 {
                    let q = p.at + DVec3::new(if k & 1 == 0 { -1.0 } else { 1.0 }, if k & 2 == 0 { -1.0 } else { 1.0 }, if k & 4 == 0 { -1.0 } else { 1.0 }) * p.half;
                    assert!(shape.inside(q, DVec3::ZERO).is_some(), "{}: {} sticks out at {q}", h.key, p.slot);
                }
            }
            for (i, a) in placed.iter().enumerate() {
                for b in &placed[i + 1..] {
                    assert!(((a.at - b.at).abs() - (a.half + b.half)).max_element() >= -1e-9, "{}: {} and {} overlap", h.key, a.slot, b.slot);
                }
            }
        }
    }

}
