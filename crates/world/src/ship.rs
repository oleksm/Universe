//! Ships: a rigid body with devices (main engine, thrusters and lift,
//! attitude control, hyperdrive, gun and laser), a hull, fuel, ammunition and
//! cargo.
//!
//! Whatever flies a ship — a pilot's hands or a flight computer — does it
//! through `ShipCommands`, and the devices turn those into forces within their
//! limits. The ship carries no navigation state: no target, no clearance.

use glam::{DMat3, DQuat, DVec3};
use serde::{Deserialize, Serialize};
use universe_physics::RigidBody;

/// The hull a new ship is built as, unless it's told otherwise.
pub const STARTING_HULL: &str = "hull.drover";

/// What a hull is built with (content: `content/*/hulls.ron`). Everything
/// about how a ship flies follows from these and the physics: its
/// accelerations are its thrusts over its mass as loaded, so a heavy ship is
/// slow, and one whose lift can't carry its weight can't hover or land on a
/// big world.
#[derive(Clone, Debug, PartialEq)]
pub struct ClassSpec {
    pub key: String,
    pub name: String,
    /// Its shape (content key), and the shape itself.
    pub shape: String,
    pub shape_ref: crate::content::Handle<crate::shape::Shape>,
    /// A designed hull's own shape (not in the content: see `design`).
    pub shape_own: Option<&'static crate::shape::Shape>,
    /// The frame alone (kg), its slots, and what's fitted in them.
    pub frame_mass: f64,
    pub slots: Vec<Slot>,
    pub fit: Vec<(String, crate::content::Handle<crate::modules::Module>)>,
    /// Mass without fuel or cargo (kg): the frame and its modules.
    pub dry_mass: f64,
    /// Fuel tank (kg), and the most cargo the hold carries (kg) and the
    /// room in it (m³: its racks').
    pub fuel_capacity: f64,
    pub hold_capacity: f64,
    pub hold_volume: f64,
    /// Passenger seats (its cabins').
    pub seats: u32,
    /// The power its plant makes, and its modules draw at work (W).
    pub power_output: f64,
    pub power_draw: f64,
    /// The autopilots its nav computers run, and the gear fitted.
    pub features: Vec<crate::modules::Feature>,
    pub gear: Vec<crate::modules::Gear>,
    /// Its thrusters: where each sits on the shape, which way it pushes, how hard.
    pub thrusters: Vec<Thruster>,
    /// What they add up to (derived from `thrusters`): the main drive's push
    /// along the nose (N); the translation thrusters', in the weakest of
    /// their directions (N); the belly lift's, along the ship's +Y (N).
    pub main_thrust: f64,
    pub rcs_thrust: f64,
    pub lift_thrust: f64,
    /// How fast its thrusters turn it about each body axis (rad/s², pitch,
    /// yaw, roll; the weaker way), loaded with a full tank and an empty hold:
    /// a lighter ship turns faster in proportion (see `Ship::turn_accel`).
    pub turn_accel: DVec3,
    /// Pitch and yaw rate, and roll rate, at full stick (rad/s): the flight
    /// computer's limits.
    pub turn_rate: f64,
    pub roll_rate: f64,
    /// Where its mass is (shape frame): the frame spread through the shape,
    /// each module at its mount (`mount_<slot>`; the shape's centre if it
    /// has none). Dry: the centre of mass and the inertia about it (kg·m²).
    /// Fuel sits at the tanks, cargo in the holds (capacity-weighted).
    pub dry_com: DVec3,
    pub dry_inertia: DMat3,
    pub tank_at: DVec3,
    pub hold_at: DVec3,
    /// Its trim cells, at the hull's ends (see `trim::cells`).
    pub trim_cells: [DVec3; 6],
    /// Where each module sits, and what has no room (see `place`).
    pub placed: Vec<Placed>,
    pub crowded: Vec<String>,
    /// Collision radius (m).
    pub radius: f64,
    /// Drag coefficient × frontal area (m²).
    pub drag_area: f64,
    /// Energy that wrecks the hull (J): see `damage`.
    pub hull_strength: f64,
    /// Its frame (to fit anew from).
    pub frame: HullFrame,
}

/// What a ship's thrusters can give without turning it (N): see `Ship::authority`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Authority {
    pub main: f64,
    pub lift: f64,
    pub side: f64,
}

/// The inertia (kg·m²) of a mass `m` at `r` from the axis point.
fn point_inertia(m: f64, r: DVec3) -> DMat3 {
    (DMat3::IDENTITY * r.length_squared() - DMat3::from_cols(r * r.x, r * r.y, r * r.z)) * m
}

/// What a thruster is for: the main drive (the throttle), translation (the
/// thruster controls), or the belly lift (translation up).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ThrusterRole {
    Main,
    Rcs,
    Lift,
}

/// A passenger's mass (kg), with their things (their seat is a cabin's).
pub const PASSENGER_MASS: f64 = 100.0;

/// A thruster on a hull: at a nozzle of its shape (metres, shape frame),
/// pushing along `push` (unit: opposite its exhaust) with up to `thrust` (N).
#[derive(Clone, Debug, PartialEq)]
pub struct Thruster {
    pub nozzle: String,
    pub role: ThrusterRole,
    pub at: DVec3,
    pub push: DVec3,
    pub thrust: f64,
}

/// A hull as `hulls.ron` has it: its thrusters by nozzle name.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct HullDef {
    key: String,
    name: String,
    shape: String,
    /// The frame alone (kg), and its price (credits).
    frame_mass: f64,
    price: f64,
    slots: Vec<(String, crate::modules::SlotKind, u8)>,
    /// What it's sold with: (slot, module).
    fit: Vec<(String, String)>,
    thrusters: Vec<ThrusterDef>,
    radius: f64,
    drag_area: f64,
    hull_strength: f64,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ThrusterDef {
    nozzle: String,
    slot: String,
    share: f64,
}

/// A slot of a hull: its name, its kind, its size class.
#[derive(Clone, Debug, PartialEq)]
pub struct Slot {
    pub name: String,
    pub kind: crate::modules::SlotKind,
    pub size: u8,
}

impl HullDef {
    /// One made in code (a design's).
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn made(key: String, name: String, shape: String, frame_mass: f64, price: f64, slots: Vec<(String, crate::modules::SlotKind, u8)>, fit: Vec<(String, String)>, thrusters: Vec<(String, String, f64)>, radius: f64, drag_area: f64, hull_strength: f64) -> Self {
        let thrusters = thrusters.into_iter().map(|(nozzle, slot, share)| ThrusterDef { nozzle, slot, share }).collect();
        HullDef { key, name, shape, frame_mass, price, slots, fit, thrusters, radius, drag_area, hull_strength }
    }

    pub(crate) fn key(&self) -> &str {
        &self.key
    }

    pub(crate) fn shape_key(&self) -> &str {
        &self.shape
    }

    /// The hull, on `shape`, with its stock fit (modules found by `module`):
    /// its nozzles found on the shape, its fit resolved, then assembled (see
    /// `ClassSpec::assemble`).
    pub(crate) fn build<'m>(
        self,
        shape_ref: crate::content::Handle<crate::shape::Shape>,
        shape: &crate::shape::Shape,
        module: impl Fn(&str) -> Option<(crate::content::Handle<crate::modules::Module>, &'m crate::modules::Module)>,
    ) -> Result<ClassSpec, String> {
        if !(self.frame_mass.is_finite() && self.frame_mass > 0.0) {
            return Err(format!("frame_mass must be positive ({})", self.frame_mass));
        }
        let slots: Vec<Slot> = self.slots.into_iter().map(|(name, kind, size)| Slot { name, kind, size }).collect();
        let mut nozzles = Vec::new();
        for t in self.thrusters {
            let n = shape.node(&t.nozzle).filter(|n| n.role == crate::shape::Role::Nozzle).ok_or_else(|| format!("no nozzle '{}' on {}", t.nozzle, shape.key))?;
            if !(t.share.is_finite() && t.share > 0.0) {
                return Err(format!("thruster at {}: share must be positive ({})", t.nozzle, t.share));
            }
            if !slots.iter().any(|s| s.name == t.slot) {
                return Err(format!("thruster at {}: no slot '{}'", t.nozzle, t.slot));
            }
            nozzles.push(NozzleLink { nozzle: t.nozzle, slot: t.slot, share: t.share, at: n.at, push: -n.dir });
        }
        let mut fit = Vec::new();
        let mut found = std::collections::HashMap::new();
        for (slot, key) in &self.fit {
            let (h, m) = module(key).ok_or_else(|| format!("fit: no module '{key}'"))?;
            found.insert(h, m);
            fit.push((slot.clone(), h));
        }
        let frame = HullFrame { frame_mass: self.frame_mass, price: self.price, slots, nozzles, radius: self.radius, drag_area: self.drag_area, hull_strength: self.hull_strength };
        ClassSpec::assemble(self.key, self.name, self.shape, shape_ref, shape, frame, fit, |h| found[&h])
    }
}

/// A nozzle of a hull, driven by a slot's module at a share of its rating
/// (where it sits on the shape and which way it pushes).
#[derive(Clone, Debug, PartialEq)]
pub struct NozzleLink {
    pub nozzle: String,
    pub slot: String,
    pub share: f64,
    pub at: DVec3,
    pub push: DVec3,
}

/// What a hull is before anything's fitted.
#[derive(Clone, Debug, PartialEq)]
pub struct HullFrame {
    pub frame_mass: f64,
    /// The frame's price (credits): what's fitted is priced on top.
    pub price: f64,
    pub slots: Vec<Slot>,
    pub nozzles: Vec<NozzleLink>,
    pub radius: f64,
    pub drag_area: f64,
    pub hull_strength: f64,
}

/// A fit: a module in each of its slots (by name).
pub type Fit = Vec<(String, crate::content::Handle<crate::modules::Module>)>;

/// Modules placed in `shape`, each where it has room nearest where it's
/// wanted (`wants`: slot, module, the module, its mount): the biggest
/// first, each box wholly inside the hull and clear of those already in.
/// One with no room anywhere stays at its mount, and is said.
#[allow(clippy::type_complexity)]
fn place(shape: &crate::shape::Shape, wants: &[(String, crate::content::Handle<crate::modules::Module>, &crate::modules::Module, DVec3)]) -> (Vec<Placed>, Vec<String>) {
    let mut order: Vec<usize> = (0..wants.len()).collect();
    order.sort_by(|&a, &b| wants[b].2.volume.total_cmp(&wants[a].2.volume));
    let (lo, hi) = shape.mesh.extent();
    let reach = (hi - lo).length() * 0.5;
    let sign = |k: i32, bit: i32| if k & bit == 0 { -1.0 } else { 1.0 };
    let inside = |at: DVec3, half: DVec3| (0..8).all(|k| shape.inside(at + DVec3::new(sign(k, 1), sign(k, 2), sign(k, 4)) * half, DVec3::ZERO).is_some());
    let clear = |at: DVec3, half: DVec3, placed: &[Placed]| placed.iter().all(|p| ((at - p.at).abs() - (half + p.half)).max_element() >= 0.0);
    let mut placed: Vec<Placed> = Vec::new();
    let mut crowded = Vec::new();
    for &i in &order {
        let (slot, h, m, want) = &wants[i];
        let half = m.dims() * 0.5;
        // Offsets in widening shells round the mount, nearest first — along
        // the ship before up or down, and sideways last (a layout kept
        // symmetric keeps the ship's balance on its centreline).
        let step = half.min_element().clamp(0.25, 1.0);
        let n = (reach / step).ceil() as i32;
        let around = |k: i32| (0..=2 * k).map(move |i| if i % 2 == 0 { -(i / 2) } else { (i + 1) / 2 });
        let mut found = None;
        'shells: for r in 0..=4 * n {
            for dz in around(r) {
                for dy in around(r / 2) {
                    for dx in around(r / 4) {
                        if dz.abs().max(2 * dy.abs()).max(4 * dx.abs()) != r {
                            continue;
                        }
                        let at = *want + DVec3::new(dx as f64, dy as f64, dz as f64) * step;
                        if clear(at, half, &placed) && inside(at, half) {
                            found = Some(at);
                            break 'shells;
                        }
                    }
                }
            }
        }
        if found.is_none() {
            crowded.push(format!("NO ROOM FOR {} ({:.0} M3)", m.name, m.volume));
        }
        placed.push(Placed { slot: slot.clone(), module: *h, at: found.unwrap_or(*want), half });
    }
    // (Back in the fit's order.)
    placed.sort_by_key(|p| wants.iter().position(|w| w.0 == p.slot));
    (placed, crowded)
}

/// A module where it sits in a hull: its slot, the module, its box's centre
/// and half-size (shape frame, axis-aligned).
#[derive(Clone, Debug, PartialEq)]
pub struct Placed {
    pub slot: String,
    pub module: crate::content::Handle<crate::modules::Module>,
    pub at: DVec3,
    pub half: DVec3,
}

impl ClassSpec {
    /// Where each fitted module sits (see `place`).
    pub fn layout(&self) -> &[Placed] {
        &self.placed
    }

    /// What has no room (empty: all fits).
    pub fn room(&self) -> &[String] {
        &self.crowded
    }

    /// Its centre of mass with `fuel` and `load` (kg) aboard (shape frame).
    pub fn centre_of_mass(&self, fuel: f64, load: f64) -> DVec3 {
        (self.dry_com * self.dry_mass + self.tank_at * fuel + self.hold_at * load) / (self.dry_mass + fuel + load)
    }

    /// Its inertia about its centre of mass with `fuel` and `load` aboard (kg·m²).
    pub fn inertia(&self, fuel: f64, load: f64) -> DMat3 {
        let com = self.centre_of_mass(fuel, load);
        self.dry_inertia + point_inertia(self.dry_mass, self.dry_com - com) + point_inertia(fuel, self.tank_at - com) + point_inertia(load, self.hold_at - com)
    }

    /// What its thrusters can give without turning it with `fuel` and
    /// `load` aboard (about its centre of mass then): the main drive along
    /// the nose, the belly lift up, the thrusters in their weakest direction
    /// (N). Off balance, less than they're rated. Worked out per tonne of
    /// fuel and load (a tonne either way changes little).
    pub fn authority(&'static self, fuel: f64, load: f64) -> Authority {
        self.authority_trimmed(fuel, load, &crate::trim::Trim::default())
    }

    /// `authority`, with the ship trimmed by `trim` (see `trim`).
    pub fn authority_trimmed(&'static self, fuel: f64, load: f64, trim: &crate::trim::Trim) -> Authority {
        let key = (self as *const ClassSpec as usize, (fuel / 1000.0).round() as i64, (load / 1000.0).round() as i64, trim.key());
        thread_local! {
            static CACHE: std::cell::RefCell<std::collections::HashMap<(usize, i64, i64, i64), Authority>> = Default::default();
        }
        if let Some(a) = CACHE.with(|c| c.borrow().get(&key).copied()) {
            return a;
        }
        // (At the tonne the key stands for, so it's the same answer whoever
        // asks first: the world stays the same to the bit.)
        let (fuel, load) = (key.1 as f64 * 1000.0, key.2 as f64 * 1000.0);
        let (com, mass, inertia) = (crate::trim::centre_of_mass(self, fuel, load, trim), self.dry_mass + fuel + load, crate::trim::inertia(self, fuel, load, trim));
        let straight = crate::thrusters::STRAIGHT * self.turn_accel.min_element();
        let thrusters = crate::trim::thrusters(self, trim);
        let push = |d: DVec3, full: f64| crate::thrusters::balanced(&thrusters, com, mass, inertia, d, full, straight);
        let a = Authority {
            main: crate::thrusters::balanced_with(&thrusters, com, mass, inertia, DVec3::NEG_Z, self.main_thrust, straight, true),
            lift: push(DVec3::Y, self.lift_thrust),
            side: [DVec3::X, DVec3::NEG_X, DVec3::NEG_Y, DVec3::Z, DVec3::NEG_Z].iter().map(|&d| push(d, self.rcs_thrust)).fold(f64::INFINITY, f64::min),
        };
        CACHE.with(|c| {
            let mut c = c.borrow_mut();
            if c.len() > 4096 {
                c.clear();
            }
            c.insert(key, a);
        });
        a
    }

    /// A hull's numbers from its frame and `fit` (modules looked up by
    /// `get`): the mass, tank and hold, each thruster's thrust (its slot's
    /// module's rating × its share), the turning limits (the flight
    /// computer's), the power budget, the autopilots it runs. Refused if a
    /// module doesn't fit its slot, a base block is missing, or the plant
    /// can't carry the load.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn assemble<'m>(
        key: String,
        name: String,
        shape_key: String,
        shape_ref: crate::content::Handle<crate::shape::Shape>,
        shape: &crate::shape::Shape,
        frame: HullFrame,
        fit: Fit,
        get: impl Fn(crate::content::Handle<crate::modules::Module>) -> &'m crate::modules::Module,
    ) -> Result<ClassSpec, String> {
        use crate::modules::{Does, SlotKind, BASE_BLOCKS};
        let mut fitted: Vec<(&Slot, &crate::modules::Module)> = Vec::new();
        for (slot_name, h) in &fit {
            let slot = frame.slots.iter().find(|s| &s.name == slot_name).ok_or_else(|| format!("fit: no slot '{slot_name}'"))?;
            let m = get(*h);
            if m.does.slot() != slot.kind {
                return Err(format!("fit: {} ({:?}) doesn't go in slot '{}' ({:?})", m.key, m.does.slot(), slot.name, slot.kind));
            }
            if m.size > slot.size {
                return Err(format!("fit: {} (size {}) is too big for slot '{}' (size {})", m.key, m.size, slot.name, slot.size));
            }
            if fitted.iter().any(|(s, _)| &s.name == slot_name) {
                return Err(format!("fit: slot '{slot_name}' twice"));
            }
            fitted.push((slot, m));
        }
        for base in BASE_BLOCKS {
            if !fitted.iter().any(|(s, _)| s.kind == base) {
                return Err(format!("no {base:?}: every ship must carry one"));
            }
        }
        let modules = || fitted.iter().map(|(_, m)| *m);
        let dry_mass = frame.frame_mass + modules().map(|m| m.mass).sum::<f64>();
        let fuel_capacity: f64 = modules().filter_map(|m| if let Does::Tank { capacity } = m.does { Some(capacity) } else { None }).sum();
        let hold_capacity: f64 = modules().filter_map(|m| if let Does::Rack { capacity } = m.does { Some(capacity) } else { None }).sum();
        let seats: u32 = modules().filter_map(|m| if let Does::Cabin { seats } = m.does { Some(seats) } else { None }).sum();
        let hold_volume: f64 = modules().filter_map(|m| if let Does::Rack { .. } = m.does { Some(m.volume) } else { None }).sum();
        let power_output: f64 = modules().filter_map(|m| if let Does::PowerPlant { output } = m.does { Some(output) } else { None }).sum();
        let power_draw: f64 = modules().map(|m| m.power).sum();
        if power_draw > power_output {
            return Err(format!("its modules draw {:.1} MW, its plant makes {:.1} MW", power_draw / 1e6, power_output / 1e6));
        }
        let (turn_rate, roll_rate) = modules()
            .find_map(|m| if let Does::FlightComputer { turn_rate, roll_rate } = m.does { Some((turn_rate, roll_rate)) } else { None })
            .expect("a flight computer (a base block)");
        let mut features: Vec<crate::modules::Feature> = modules().flat_map(|m| if let Does::NavComputer { features } = &m.does { features.clone() } else { Vec::new() }).collect();
        features.sort();
        features.dedup();
        let mut gear: Vec<crate::modules::Gear> = modules().filter_map(|m| m.does.gear()).collect();
        gear.sort();
        gear.dedup();
        // The thrusters: each nozzle by its slot's module (an empty slot drives none).
        let mut thrusters = Vec::new();
        for n in &frame.nozzles {
            let Some((slot, m)) = fitted.iter().find(|(s, _)| s.name == n.slot) else { continue };
            let (role, rating) = match (slot.kind, &m.does) {
                (SlotKind::Drive, Does::Drive { thrust }) => (ThrusterRole::Main, *thrust),
                (SlotKind::Thrusters, Does::Thrusters { thrust }) => (ThrusterRole::Rcs, *thrust),
                (SlotKind::Lift, Does::Lift { thrust }) => (ThrusterRole::Lift, *thrust),
                _ => return Err(format!("thruster at {}: slot '{}' doesn't drive nozzles", n.nozzle, n.slot)),
            };
            thrusters.push(Thruster { nozzle: n.nozzle.clone(), role, at: n.at, push: n.push, thrust: rating * n.share });
        }
        // Each role's push along a direction (only thrusters pushing that way count).
        let along = |role: ThrusterRole, d: DVec3| thrusters.iter().filter(|t| t.role == role).map(|t| t.thrust * t.push.dot(d).max(0.0)).sum::<f64>();
        let main_thrust = along(ThrusterRole::Main, DVec3::NEG_Z);
        let lift_thrust = along(ThrusterRole::Lift, DVec3::Y);
        let rcs_thrust = [DVec3::X, DVec3::NEG_X, DVec3::NEG_Y, DVec3::Z, DVec3::NEG_Z].iter().map(|&d| along(ThrusterRole::Rcs, d)).fold(f64::INFINITY, f64::min);
        // Where the mass is: the frame through the shape, the modules at their mounts.
        // Each module where it has room, nearest its mount; its mass there.
        let wants: Vec<(String, crate::content::Handle<crate::modules::Module>, &crate::modules::Module, DVec3)> = fit
            .iter()
            .filter_map(|(slot, h)| fitted.iter().find(|(s, _)| &s.name == slot).map(|(s, m)| (slot.clone(), *h, *m, shape.node(&format!("mount_{}", s.name)).map_or(shape.solid.centroid, |n| n.at))))
            .collect();
        let (placed, crowded) = place(shape, &wants);
        // (A module with no room anywhere in the hull: it won't go together.)
        if !crowded.is_empty() {
            return Err(crowded.join(", "));
        }
        let mount = |slot: &str| placed.iter().find(|p| p.slot == slot).map_or(shape.solid.centroid, |p| p.at);
        let parts: Vec<(f64, DVec3)> = std::iter::once((frame.frame_mass, shape.solid.centroid)).chain(fitted.iter().map(|(s, m)| (m.mass, mount(&s.name)))).collect();
        let dry_com = parts.iter().map(|(m, at)| *at * *m).sum::<DVec3>() / dry_mass;
        let dry_inertia = shape.solid.inertia * (frame.frame_mass / shape.solid.volume)
            + parts.iter().map(|&(m, at)| point_inertia(m, at - dry_com)).fold(DMat3::ZERO, |a, b| a + b);
        let weighted = |kind: SlotKind, cap: fn(&Does) -> f64| {
            let w: Vec<(f64, DVec3)> = fitted.iter().filter(|(s, _)| s.kind == kind).map(|(s, m)| (cap(&m.does), mount(&s.name))).collect();
            let total: f64 = w.iter().map(|x| x.0).sum();
            if total > 0.0 { w.iter().map(|(c, at)| *at * *c).sum::<DVec3>() / total } else { dry_com }
        };
        let tank_at = weighted(SlotKind::Tank, |d| if let Does::Tank { capacity } = d { *capacity } else { 0.0 });
        let hold_at = weighted(SlotKind::Cargo, |d| if let Does::Rack { capacity } = d { *capacity } else { 0.0 });
        let mass = dry_mass + fuel_capacity;
        let com = (dry_com * dry_mass + tank_at * fuel_capacity) / mass;
        let inertia = dry_inertia + point_inertia(dry_mass, dry_com - com) + point_inertia(fuel_capacity, tank_at - com);
        let turn_accel = crate::thrusters::turn_envelope(&thrusters, com, mass, inertia);
        Ok(ClassSpec {
            key,
            name,
            shape: shape_key,
            shape_ref,
            shape_own: None,
            frame_mass: frame.frame_mass,
            slots: frame.slots.clone(),
            fit,
            dry_mass,
            fuel_capacity,
            hold_capacity,
            hold_volume,
            seats,
            power_output,
            power_draw,
            features,
            gear,
            thrusters,
            main_thrust,
            rcs_thrust,
            lift_thrust,
            turn_accel,
            turn_rate,
            roll_rate,
            dry_com,
            dry_inertia,
            tank_at,
            hold_at,
            trim_cells: crate::trim::cells(shape, tank_at),
            placed,
            crowded,
            radius: frame.radius,
            drag_area: frame.drag_area,
            hull_strength: frame.hull_strength,
            frame,
        })
    }

    /// This hull with another fit (refused, with the reason, if it won't do).
    pub fn refit(&self, fit: Fit) -> Result<ClassSpec, String> {
        let c = crate::content::content();
        let mut s = ClassSpec::assemble(self.key.clone(), self.name.clone(), self.shape.clone(), self.shape_ref, self.shape(), self.frame.clone(), fit, |h| c.get(h))?;
        s.shape_own = self.shape_own;
        Ok(s)
    }

    /// Has it this gear fitted?
    pub fn has(&self, g: crate::modules::Gear) -> bool {
        self.gear.contains(&g)
    }

    /// Does its nav computer run this autopilot?
    pub fn runs(&self, f: crate::modules::Feature) -> bool {
        self.features.contains(&f)
    }
}

impl ClassSpec {
    /// Its shape.
    pub fn shape(&self) -> &'static crate::shape::Shape {
        self.shape_own.unwrap_or_else(|| crate::content::content().get(self.shape_ref))
    }
}

/// A hull of the loaded content.
pub type Hull = crate::content::Handle<ClassSpec>;

/// A hull's numbers with `fit`: worked out once per hull and fit, then kept
/// (identical fits share them, however many ships fly them).
pub fn fitted(hull: Hull, fit: &Fit) -> Result<&'static ClassSpec, String> {
    use std::collections::HashMap;
    use std::sync::{Mutex, OnceLock};
    type Cache = Mutex<HashMap<(Hull, Fit), &'static ClassSpec>>;
    static FITTED: OnceLock<Cache> = OnceLock::new();
    let cache = FITTED.get_or_init(Default::default);
    if let Some(s) = cache.lock().unwrap_or_else(|e| e.into_inner()).get(&(hull, fit.clone())) {
        return Ok(s);
    }
    let spec: &'static ClassSpec = Box::leak(Box::new(crate::content::content().get(hull).refit(fit.clone())?));
    cache.lock().unwrap_or_else(|e| e.into_inner()).insert((hull, fit.clone()), spec);
    Ok(spec)
}

/// The starting hull.
pub fn starting_hull() -> Hull {
    crate::content::content().handle(STARTING_HULL).expect("the content has the starting hull (checked at load)")
}

/// The starting hull's numbers (for tests and defaults).
pub fn starter() -> &'static ClassSpec {
    crate::content::content().get(starting_hull())
}

/// A ship's size as traffic lays out room for it (pads, docking slots,
/// corridors), and the starting hull's collision radius (m).
pub const SHIP_RADIUS: f64 = 12.0;
/// The drives' exhaust velocity (m/s): a torch drive (a few percent of
/// light speed), so a tank lasts a day of burning. Each device burns its
/// thrust / this, in kg/s. (The economy's knob: see `economy`.)
pub const EXHAUST_VELOCITY: f64 = 1.0e7;
/// The hyperdrive's draw at full throttle (kg/s), while engaged.
pub const HYPER_FUEL_FLOW: f64 = 0.2;
fn full_tank() -> f64 {
    starter().fuel_capacity
}

fn powered() -> bool {
    true
}

fn intact() -> f64 {
    1.0
}

fn full_magazine() -> u32 {
    crate::weapons::GUN_AMMO
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum ShipState {
    Flying,
    /// Resting on a body, stored in that body's rotating frame.
    Landed { body: usize, local_position: DVec3, local_orientation: DQuat },
    Destroyed { respawn_in: f64 },
    /// Held to an asteroid by the anchor: body `body` among field `field`'s
    /// (see `StarSystem::field_bodies`), stored in its rotating frame.
    Anchored { field: usize, body: usize, local_position: DVec3, local_orientation: DQuat },
    /// Between gates. The ship's motion relative to the entry gate is kept and
    /// reapplied at the paired gate on arrival.
    Transit {
        /// Galaxy index of the system we're going to, and the one we came from.
        to: usize,
        from: usize,
        remaining: f64,
        local_velocity: DVec3,
        local_offset: DVec3,
        local_orientation: DQuat,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Ship {
    /// Relative to the star of the ship's current system (m).
    pub position: DVec3,
    pub velocity: DVec3,
    /// Local -Z is forward, +Y is up.
    pub orientation: DQuat,
    /// Body-frame angular velocity (x = pitch, y = yaw, z = roll), rad/s.
    pub angular_velocity: DVec3,
    /// Main engine setting, 0..1, as last commanded.
    pub throttle: f64,
    /// The hyperdrive is engaged.
    pub hyperdrive: bool,
    pub state: ShipState,
    /// When it last locked to a structure or the ground (world time): orders
    /// given before then don't touch its engine or thrusters (see
    /// `sim::vessel::Inbox`), as their pilot hadn't seen it land. (Not saved:
    /// it matters only for the ticks an order is on its way.)
    #[serde(skip, default = "never")]
    pub locked_at: f64,
    /// Translation thruster setting, body frame, each axis -1..1 (x right, y up, z back), as last commanded.
    #[serde(default)]
    pub rcs: DVec3,
    /// In this port's hangar (a spaceport's or a station's: off the pads,
    /// out of sight; still `Landed`, parked at the hangar): see `port`.
    #[serde(default)]
    pub hangar: Option<crate::traffic::Facility>,
    /// Flight systems on: the drives, thrusters and turn answer. A ship
    /// that sets down powers down (parked); its pilot powers it up to fly.
    #[serde(default = "powered")]
    pub powered: bool,
    /// The flight computer off: each thruster fires only while the pilot
    /// holds it (`held`, bit `k` its thruster `k`, at full); no stick, no
    /// throttle, nothing to steady it.
    #[serde(default)]
    pub manual: bool,
    /// Its trim (set at a shipyard: see `trim`).
    #[serde(default)]
    pub trim: crate::trim::Trim,
    #[serde(default)]
    pub held: u64,
    /// Taxiing on the ground between a pad and a spaceport's hangar (see
    /// `World::hangar_move`): out of the pads' count while it does.
    #[serde(default)]
    pub taxi: Option<Taxi>,
    /// The hull it's built as (stored by its content key).
    #[serde(default = "starting_hull")]
    pub class: Hull,
    /// What's fitted, if not its hull's stock fit (modules by content key).
    #[serde(default)]
    pub fit: Option<Fit>,
    /// Its numbers, hull and fit together (kept to hand; see `spec`).
    #[serde(skip)]
    spec_ref: Option<&'static ClassSpec>,
    /// Fuel on board (kg).
    #[serde(default = "full_tank")]
    pub fuel: f64,
    /// Cargo on board (kg): the mass of what's in the hold; and the room it
    /// takes (m³).
    #[serde(default)]
    pub cargo: f64,
    #[serde(default)]
    pub cargo_volume: f64,
    /// Passengers aboard (each `PASSENGER_MASS`, in a cabin's seat), and
    /// where they've booked passage to (system, market) for what fare each.
    #[serde(default)]
    pub passengers: u32,
    #[serde(default)]
    pub bound_for: Option<(usize, crate::traffic::Facility)>,
    #[serde(default)]
    pub fare: f64,
    /// The excavator is switched on (it digs while anchored: see `mining`).
    #[serde(default)]
    pub excavator: bool,
    /// Ore dug but not yet a whole tonne in the hold (kg).
    #[serde(default)]
    pub hopper: f64,
    /// Hull integrity, 1 (intact) .. 0 (destroyed): see `damage`.
    #[serde(default = "intact")]
    pub hull: f64,
    /// Gun rounds left.
    #[serde(default = "full_magazine")]
    pub ammo: u32,
    /// Laser heat, 0 (cold) .. 1 (too hot to fire).
    #[serde(default)]
    pub laser_heat: f64,
    /// The laser overheated and is locked out until it has cooled (see `weapons::LASER_RESET`).
    #[serde(default)]
    pub laser_overheated: bool,
    /// Combat mode: the master arm is on (see `weapons`). The weapons are
    /// hot once `arming` has run down.
    #[serde(default)]
    pub armed: bool,
    /// Seconds until the weapons are primed.
    #[serde(default)]
    pub arming: f64,
    /// Weapon triggers, as last commanded.
    #[serde(skip)]
    pub triggers: Triggers,
    /// Seconds until the gun can fire again.
    #[serde(skip)]
    pub gun_cooldown: f64,
    /// Where the gun points, in the ship's frame (unit; -Z is the nose). Its
    /// gimbal swings it a few degrees off the nose (see `weapons`).
    #[serde(skip, default = "boresight")]
    pub gun_dir: DVec3,
    /// Where the gun is to be laid, in the world (unit), as last commanded.
    /// None: along the nose.
    #[serde(skip)]
    pub gun_target: Option<DVec3>,
    /// How the hyperdrive flies while engaged, as last ordered (held like any
    /// device setting).
    #[serde(skip, default = "cruise")]
    pub hyper_orders: HyperdriveCommand,
    /// When the drive was last switched on (world time), and the throttle and
    /// velocity the ship had then: a drive that drops straight out (a planet
    /// in the way) gives them back. (Not saved.)
    #[serde(skip)]
    pub hyper_engaged: Option<(f64, f64, DVec3)>,
    /// How hard each thruster fires now (0..1, the hull's `thrusters` in
    /// order): the flight computer's allocation (see `thrusters`). (Not saved.)
    #[serde(skip)]
    pub jets: Vec<f64>,
    /// What they give (body frame): force (N) and torque about the centre of mass (N·m).
    #[serde(skip)]
    pub applied: (DVec3, DVec3),
    /// The hull's skin temperature (K): see `heat`.
    #[serde(default = "skin_ambient")]
    pub skin_temp: f64,
    /// Seconds the hyperdrive stays jammed (hits disrupt it: see `damage::HYPER_JAM`).
    #[serde(skip)]
    pub hyper_jam: f64,
}

fn boresight() -> DVec3 {
    DVec3::NEG_Z
}

pub use universe_protocol::{Controls, Destination, HyperdriveCommand, ShipCommands, Triggers};

fn never() -> f64 {
    f64::NEG_INFINITY
}

impl Ship {
    pub fn new(position: DVec3, velocity: DVec3, orientation: DQuat) -> Self {
        Self {
            position,
            velocity,
            orientation,
            angular_velocity: DVec3::ZERO,
            throttle: 0.0,
            hyperdrive: false,
            state: ShipState::Flying,
            rcs: DVec3::ZERO,
            fuel: starter().fuel_capacity,
            class: starting_hull(),
            fit: None,
            spec_ref: None,
            hangar: None,
            powered: true,
            manual: false,
            trim: Default::default(),
            held: 0,
            taxi: None,
            passengers: 0,
            bound_for: None,
            fare: 0.0,
            cargo_volume: 0.0,
            cargo: 0.0,
            excavator: false,
            hopper: 0.0,
            hull: 1.0,
            ammo: crate::weapons::GUN_AMMO,
            laser_heat: 0.0,
            laser_overheated: false,
            armed: false,
            arming: 0.0,
            triggers: Triggers::default(),
            gun_cooldown: 0.0,
            gun_dir: DVec3::NEG_Z,
            gun_target: None,
            hyper_jam: 0.0,
            skin_temp: crate::heat::AMBIENT,
            hyper_orders: HyperdriveCommand::CRUISE,
            locked_at: f64::NEG_INFINITY,
            hyper_engaged: None,
            jets: Vec::new(),
            applied: (DVec3::ZERO, DVec3::ZERO),
        }
    }

    /// Total mass right now (kg).
    pub fn mass(&self) -> f64 {
        self.spec().dry_mass + self.fuel + self.load()
    }

    /// What its class is built with.
    pub fn spec(&self) -> &'static ClassSpec {
        match (self.spec_ref, &self.fit) {
            (Some(s), _) => s,
            (None, None) => crate::content::content().get(self.class),
            // (Loaded with a fit of its own and not yet refreshed: looked up.)
            (None, Some(fit)) => fitted(self.class, fit).unwrap_or_else(|_| crate::content::content().get(self.class)),
        }
    }

    /// Back to its hull's stock fit, with a full tank (a basic ship).
    pub fn refresh_stock(&mut self) {
        self.fit = None;
        self.spec_ref = None;
        self.fuel = self.spec().fuel_capacity;
        self.cargo = self.cargo.min(self.spec().hold_capacity);
    }

    /// Its numbers to hand again (after loading).
    pub fn refresh(&mut self) {
        self.spec_ref = self.fit.as_ref().and_then(|f| fitted(self.class, f).ok());
    }

    /// Fit it anew: refused if the fit won't do (see `ClassSpec::assemble`)
    /// or the hold it leaves can't take what's in it; a smaller tank keeps
    /// what it holds.
    pub fn refit(&mut self, fit: Fit) -> Result<(), String> {
        let spec = fitted(self.class, &fit)?;
        if self.cargo_volume > spec.hold_volume + 1e-6 {
            return Err(format!("THE HOLD WOULD HOLD {:.0} M3, THERE'S {:.1} M3 IN IT", spec.hold_volume, self.cargo_volume));
        }
        if self.cargo + self.hopper > spec.hold_capacity + 1e-6 {
            return Err(format!("THE HOLD WOULD TAKE {:.0} T, THERE'S {:.1} T IN IT", spec.hold_capacity / 1000.0, (self.cargo + self.hopper) / 1000.0));
        }
        self.fuel = self.fuel.min(spec.fuel_capacity);
        let stock = &crate::content::content().get(self.class).fit;
        if &fit == stock {
            self.fit = None;
            self.spec_ref = None;
        } else {
            self.fit = Some(fit);
            self.spec_ref = Some(spec);
        }
        self.jets.clear();
        Ok(())
    }

    /// Its centre of mass (shape frame, m): its shape's, as a solid.
    /// (Fuel and cargo are taken as spread like the hull, for now.)
    pub fn centre_of_mass(&self) -> DVec3 {
        crate::trim::centre_of_mass(self.spec(), self.fuel, self.load(), &self.trim)
    }

    /// Its inertia tensor about its centre of mass (kg·m², body frame).
    pub fn inertia(&self) -> glam::DMat3 {
        crate::trim::inertia(self.spec(), self.fuel, self.load(), &self.trim)
    }

    /// What its thrusters can give without turning it, loaded as it is (see
    /// `ClassSpec::authority`).
    pub fn authority(&self) -> Authority {
        self.spec().authority_trimmed(self.fuel, self.load(), &self.trim)
    }

    /// How fast its thrusters turn it now (rad/s² about each body axis).
    pub fn turn_accel(&self) -> DVec3 {
        let s = self.spec();
        s.turn_accel * ((s.dry_mass + s.fuel_capacity) / self.mass())
    }

    /// The flight computer, for a span of `dt` seconds: the push the engine
    /// and thrusters are set to, and the turn wanted (rates, `turn`; None: no
    /// turn worked), allocated to the thrusters; then the ship turns as their
    /// torque turns it (Euler's equations, its inertia from its shape). Its
    /// `applied` force is what `thrust` gives over the span.
    /// (`push` false: the turn alone — in the hyperdrive, where the throttle
    /// sets the drive's speed, or on the ground, where the landing gear's
    /// rules decide lift-off.)
    pub fn drive(&mut self, turn: Option<&Controls>, dt: f64, push: bool) {
        if self.manual && push {
            return self.drive_manual(dt);
        }
        let s = self.spec();
        let inertia = self.inertia();
        let w = self.angular_velocity;
        // The push: the main drive along the nose, the thrusters as set — as
        // much as they give without turning the ship (off balance, less: the
        // flight computer keeps it straight first).
        let c = self.rcs.clamp(DVec3::splat(-1.0), DVec3::ONE);
        let a = if push { self.authority() } else { Authority { main: 0.0, lift: 0.0, side: 0.0 } };
        let force = if push { DVec3::NEG_Z * (self.throttle.clamp(0.0, 1.0) * a.main) + DVec3::new(c.x * a.side, c.y * if c.y > 0.0 { a.lift } else { a.side }, c.z * a.side) } else { DVec3::ZERO };
        // The turn: toward the rates asked, as fast as the span allows.
        let want = turn.map(|t| DVec3::new(t.pitch.clamp(-1.0, 1.0) * s.turn_rate, t.yaw.clamp(-1.0, 1.0) * s.turn_rate, t.roll.clamp(-1.0, 1.0) * s.roll_rate));
        let torque = want.map_or(DVec3::ZERO, |want| inertia * ((want - w) / dt.max(TURN_RESPONSE)));
        if self.fuel <= 0.0 || (force == DVec3::ZERO && torque.length_squared() < 1e-6) {
            // (Nothing asked, or nothing to burn: every jet off.)
            self.jets.iter_mut().for_each(|j| *j = 0.0);
            self.applied = (DVec3::ZERO, DVec3::ZERO);
        } else {
            // The drive at the throttle (its nozzles together, as far as it
            // goes straight); the thrusters and lift do the rest.
            let level = if push { self.throttle.clamp(0.0, 1.0) * a.main / s.main_thrust.max(1.0) } else { 0.0 };
            let thrusters = crate::trim::thrusters(s, &self.trim);
            self.applied = crate::thrusters::allocate_with(&thrusters, self.centre_of_mass(), self.mass(), inertia, Some(level), force, torque, &mut self.jets);
        }
        // Turning: I ω̇ = τ. (The spin's own coupling, ω × Iω, is left out:
        // the flight computer holds against it when it turns the ship, and a
        // ship tumbling free keeps its spin about its own axes — near enough
        // for a hull this close to symmetric, and steady over the long steps
        // the planner takes.) Never past the rates asked in one span.
        let accel = inertia.inverse() * self.applied.1;
        let mut next = w + accel * dt;
        if let Some(want) = want {
            let toward = |w: f64, n: f64, t: f64| if (t - w) * (t - n) < 0.0 { t } else { n };
            next = DVec3::new(toward(w.x, next.x, want.x), toward(w.y, next.y, want.y), toward(w.z, next.z, want.z));
        }
        self.angular_velocity = next;
        self.orientation = (self.orientation * DQuat::from_scaled_axis(self.angular_velocity * dt)).normalize();
    }

    /// Manual flight, for `dt`: the held thrusters at full (while there's
    /// fuel), every other off; the ship pushed and turned by just what
    /// they give about its centre of mass, nothing held against it.
    fn drive_manual(&mut self, dt: f64) {
        let s = self.spec();
        let com = self.centre_of_mass();
        self.jets.resize(s.thrusters.len(), 0.0);
        let (mut force, mut torque) = (DVec3::ZERO, DVec3::ZERO);
        let thrusters = crate::trim::thrusters(s, &self.trim);
        for (k, t) in thrusters.iter().enumerate() {
            let on = k < 64 && self.held & (1 << k) != 0 && self.fuel > 0.0;
            self.jets[k] = if on { 1.0 } else { 0.0 };
            if on {
                let f = t.push * t.thrust;
                force += f;
                torque += (t.at - com).cross(f);
            }
        }
        self.applied = (force, torque);
        self.angular_velocity += self.inertia().inverse() * torque * dt;
        self.orientation = (self.orientation * DQuat::from_scaled_axis(self.angular_velocity * dt)).normalize();
    }

    /// Room left in the hold (kg).
    pub fn hold_room(&self) -> f64 {
        (self.spec().hold_capacity - self.cargo - self.hopper).max(0.0)
    }

    /// Space left in the hold (m³).
    pub fn hold_space(&self) -> f64 {
        (self.spec().hold_volume - self.cargo_volume).max(0.0)
    }

    /// How much of something this dense (t/m³) still goes in (kg): what the
    /// hold's weight and its space each allow, the less.
    pub fn takes(&self, density: f64) -> f64 {
        self.hold_room().min(self.hold_space() * density * 1000.0)
    }

    /// What's aboard besides fuel (kg): cargo, ore, passengers.
    pub fn load(&self) -> f64 {
        self.cargo + self.hopper + self.passengers as f64 * PASSENGER_MASS
    }

    /// How many more passengers it has seats for (its cabins').
    pub fn passenger_room(&self) -> u32 {
        self.spec().seats.saturating_sub(self.passengers)
    }

    /// Main engine acceleration at full throttle without turning the ship (m/s^2).
    pub fn main_accel(&self) -> f64 {
        self.authority().main / self.mass()
    }

    /// Translation thruster acceleration, per axis, without turning it (m/s^2).
    pub fn side_accel(&self) -> f64 {
        self.authority().side / self.mass()
    }

    /// Lift thruster acceleration without turning it (m/s^2).
    pub fn lift_accel(&self) -> f64 {
        self.authority().lift / self.mass()
    }

    /// Body-frame acceleration from a thruster command (each axis -1..1).
    pub fn thruster_accel(&self, rcs: DVec3) -> DVec3 {
        let c = rcs.clamp(DVec3::splat(-1.0), DVec3::ONE);
        let (side, lift) = (self.side_accel(), self.lift_accel());
        DVec3::new(c.x * side, c.y * if c.y > 0.0 { lift } else { side }, c.z * side)
    }

    /// The thruster command that best produces a body-frame acceleration.
    pub fn thruster_command(&self, accel: DVec3) -> DVec3 {
        let (side, lift) = (self.side_accel(), self.lift_accel());
        let up = if accel.y > 0.0 { lift } else { side };
        DVec3::new(accel.x / side, accel.y / up, accel.z / side).clamp(DVec3::splat(-1.0), DVec3::ONE)
    }

    pub fn forward(&self) -> DVec3 {
        self.orientation * DVec3::NEG_Z
    }

    pub fn up(&self) -> DVec3 {
        self.orientation * DVec3::Y
    }

    pub fn is_flying(&self) -> bool {
        matches!(self.state, ShipState::Flying)
    }

    /// The ship as the physics kernel sees it.
    pub fn rigid(&self) -> RigidBody {
        // Drag in air (none in the hyperdrive's field).
        let ballistic = if self.hyperdrive { 0.0 } else { self.mass() / self.spec().drag_area };
        RigidBody { position: self.position, velocity: self.velocity, orientation: self.orientation, angular_velocity: self.angular_velocity, radius: self.spec().radius, parts: &self.spec().shape().spheres, ballistic }
    }

    /// Take the kernel's word for where the ship is and how it moves.
    pub fn set_rigid(&mut self, body: &RigidBody) {
        self.position = body.position;
        self.velocity = body.velocity;
        self.orientation = body.orientation;
        self.angular_velocity = body.angular_velocity;
    }

    /// Commands that keep the engine and thrusters as they are, turn nothing
    /// and leave the hyperdrive alone: a starting point for new commands.
    pub fn holding(&self) -> ShipCommands {
        ShipCommands { throttle: self.throttle, rcs: self.rcs, turn: None, hyperdrive: None, weapons: None, arm: None, gun_target: None, anchor: None, excavate: None, hangar: None, power: None, manual: None, jets: None }
    }

    /// The main engine and thrusters take their new settings.
    pub(crate) fn set_controls(&mut self, c: &ShipCommands) {
        // (Powered down, nothing answers; that's for a ship set down:
        // anything off the ground is flying, its systems on.)
        if !matches!(self.state, ShipState::Landed { .. }) {
            self.powered = true;
        }
        let on = self.powered || c.power == Some(true);
        if let Some(m) = c.manual {
            self.manual = m;
            self.held = 0;
        }
        if let Some(j) = c.jets {
            self.held = j;
        }
        // (Manual: the stick and throttle answer nothing; the held jets do.)
        let on = on && !self.manual;
        self.throttle = if on { c.throttle } else { 0.0 };
        self.rcs = if on { c.rcs } else { DVec3::ZERO };
        if let Some(t) = c.weapons {
            self.triggers = t;
        }
        if let Some(g) = c.gun_target {
            self.gun_target = g;
        }
    }

    /// Where the gun points, in the world.
    pub fn gun_forward(&self) -> DVec3 {
        self.orientation * self.gun_dir
    }

    /// Weapons can fire: combat mode on, primed.
    pub fn weapons_hot(&self) -> bool {
        self.armed && self.arming <= 0.0
    }

    /// Acceleration from the main engine and thrusters as set (m/s^2): thrust / current mass.
    pub fn thrust(&self) -> DVec3 {
        // (No fuel, no thrust.)
        if self.fuel <= 0.0 {
            return DVec3::ZERO;
        }
        self.orientation * self.applied.0 / self.mass()
    }

    /// What the engine and thrusters burn as set (kg/s): their thrust over
    /// the exhaust velocity.
    pub fn fuel_flow(&self) -> f64 {
        self.spec().thrusters.iter().zip(&self.jets).map(|(t, &u)| t.thrust * u).sum::<f64>() / EXHAUST_VELOCITY
    }

    /// `dt` seconds of the drives as set: the fuel they burn.
    pub fn burn(&mut self, dt: f64) {
        self.fuel = (self.fuel - self.fuel_flow() * dt).max(0.0);
    }

}

/// A taxi on a port's ground (or deck): into `port`'s hangar, or out to pad `pad`.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Taxi {
    pub port: crate::traffic::Facility,
    /// None: into the hangar.
    pub pad: Option<usize>,
}

/// How fast a ship taxis on the ground (m/s).
pub const TAXI_SPEED: f64 = 15.0;

/// How quickly the flight computer brings the turn to the rates asked (s),
/// as far as the thrusters allow.
pub const TURN_RESPONSE: f64 = 0.12;

/// Orientation with the nose exactly along `forward` and the ship's top as close
/// as possible to `up_hint`. A fixed reference keeps the roll steady instead of
/// drifting with whatever the last turn left behind.
pub fn facing(forward: DVec3, up_hint: DVec3) -> DQuat {
    let f = forward.normalize();
    let right = f.cross(up_hint).try_normalize().unwrap_or_else(|| f.any_orthonormal_vector());
    let up = right.cross(f);
    DQuat::from_mat3(&DMat3::from_cols(right, up, -f))
}

/// Orientation with `up` along `normal` and forward as close as possible to `forward`.
pub fn upright(normal: DVec3, forward: DVec3) -> DQuat {
    let tangent = (forward - normal * forward.dot(normal)).try_normalize().unwrap_or_else(|| normal.any_orthonormal_vector());
    let right = tangent.cross(normal);
    DQuat::from_mat3(&DMat3::from_cols(right, normal, -tangent))
}

fn skin_ambient() -> f64 {
    crate::heat::AMBIENT
}

fn cruise() -> HyperdriveCommand {
    HyperdriveCommand::CRUISE
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_starters_thrusters_add_up_to_its_envelope() {
        let c = starter();
        assert_eq!(c.thrusters.len(), 18);
        assert!((c.main_thrust - 2.7e6).abs() < 1.0, "{}", c.main_thrust);
        assert!((c.lift_thrust - 2.25e6).abs() < 1.0, "{}", c.lift_thrust);
        // The same push every way the thrusters push (down, the sides, fore and aft).
        for d in [DVec3::X, DVec3::NEG_X, DVec3::NEG_Y, DVec3::Z, DVec3::NEG_Z] {
            let sum: f64 = c.thrusters.iter().filter(|t| t.role == ThrusterRole::Rcs).map(|t| t.thrust * t.push.dot(d).max(0.0)).sum();
            assert!((sum - 5.4e5).abs() < 1.0, "{d}: {sum}");
        }
        assert!((c.rcs_thrust - 5.4e5).abs() < 1.0);
    }

    #[test]
    fn a_ships_inertia_follows_its_shape_and_its_load() {
        let mut s = Ship::new(DVec3::ZERO, DVec3::ZERO, DQuat::IDENTITY);
        let full = s.inertia();
        // Wider than it's long: hardest to roll? No — it's flat: yaw (about Y) is hardest.
        let (pitch, yaw, roll) = (full.x_axis.x, full.y_axis.y, full.z_axis.z);
        assert!(yaw > pitch && yaw > roll, "pitch {pitch:.3e} yaw {yaw:.3e} roll {roll:.3e}");
        s.fuel = 0.0;
        assert!(s.inertia().y_axis.y < yaw && s.inertia().x_axis.x < pitch, "lighter, easier to turn");
    }
}

#[cfg(test)]
mod classes {
    use super::*;
    use crate::content::content;

    #[test]
    fn every_hull_is_balanced_and_sized_for_its_job() {
        for (_, h) in content().hulls.iter().filter(|(_, h)| h.key.starts_with("hull.")) {
            let h: &'static ClassSpec = h;
            // Every way it pushes, nearly all of it without turning (at the
            // load it's balanced for: a full tank, the hold half full).
            let a = h.authority(h.fuel_capacity, h.hold_capacity / 2.0);
            for (what, got, full) in [("drive", a.main, h.main_thrust), ("lift", a.lift, h.lift_thrust), ("thrusters", a.side, h.rcs_thrust)] {
                assert!(got > 0.9 * full, "{}: {what} only {:.0}%", h.key, 100.0 * got / full);
            }
            let loaded = h.dry_mass + h.fuel_capacity;
            eprintln!(
                "{:<16} dry {:>5.1} t  tank {:>4.0} t  hold {:>5.0} t  main {:>4.1} m/s²  lift {:>4.1} m/s² (full hold {:>4.1})  turns {:.1}/{:.1}/{:.1}  {:.0} CR",
                h.name, h.dry_mass / 1e3, h.fuel_capacity / 1e3, h.hold_capacity / 1e3, h.main_thrust / loaded, h.lift_thrust / loaded, h.lift_thrust / (loaded + h.hold_capacity), h.turn_accel.x, h.turn_accel.y, h.turn_accel.z, h.frame.price
            );
            assert!(h.lift_thrust / loaded > 9.81 * 1.1, "{}: can't hover at 1 g with an empty hold", h.key);
        }
    }
}

#[cfg(test)]
mod balance {
    use super::*;
    use crate::content::content;

    #[test]
    fn loading_moves_the_centre_of_mass_and_off_balance_costs_authority() {
        for (_, h) in content().hulls.iter().filter(|(_, h)| h.key.starts_with("hull.")) {
            let h: &'static ClassSpec = h;
            // Built balanced about its usual load: empty to full, it keeps nearly all its push.
            let (empty, full) = (h.authority(h.fuel_capacity, 0.0), h.authority(h.fuel_capacity, h.hold_capacity));
            for (what, a) in [("empty", empty), ("full", full)] {
                assert!(a.lift > 0.9 * h.lift_thrust && a.main > 0.9 * h.main_thrust && a.side > 0.85 * h.rcs_thrust, "{} {what}: {a:?}", h.key);
            }
        }
        // The Drover's hold is forward: loading it moves its centre of mass forward.
        let d = starter();
        let (empty, full) = (d.centre_of_mass(d.fuel_capacity, 0.0), d.centre_of_mass(d.fuel_capacity, d.hold_capacity));
        assert!(empty.z - full.z > 1.5, "{empty} -> {full}");
        // 3 m off its balance, the lift can't push straight with all it has.
        let (com, m) = (d.centre_of_mass(d.fuel_capacity, 0.0), d.dry_mass + d.fuel_capacity);
        let i = d.inertia(d.fuel_capacity, 0.0);
        let straight = crate::thrusters::STRAIGHT * d.turn_accel.min_element();
        let off = crate::thrusters::balanced(&d.thrusters, com + DVec3::Z * 3.0, m, i, DVec3::Y, d.lift_thrust, straight);
        assert!(off < 0.85 * d.lift_thrust, "{:.0}%", 100.0 * off / d.lift_thrust);
    }
}


