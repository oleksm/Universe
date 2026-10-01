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
pub const STARTING_HULL: &str = "hull.cobra";

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
    /// The frame alone (kg), its slots, and what's fitted in them.
    pub frame_mass: f64,
    pub slots: Vec<Slot>,
    pub fit: Vec<(String, crate::content::Handle<crate::modules::Module>)>,
    /// Mass without fuel or cargo (kg): the frame and its modules.
    pub dry_mass: f64,
    /// Fuel tank (kg), and the most cargo the hold carries (kg).
    pub fuel_capacity: f64,
    pub hold_capacity: f64,
    /// The power its plant makes, and its modules draw at work (W).
    pub power_output: f64,
    pub power_draw: f64,
    /// The autopilots its nav computers run.
    pub features: Vec<crate::modules::Feature>,
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
    /// Collision radius (m).
    pub radius: f64,
    /// Drag coefficient × frontal area (m²).
    pub drag_area: f64,
    /// Energy that wrecks the hull (J): see `damage`.
    pub hull_strength: f64,
}

/// What a thruster is for: the main drive (the throttle), translation (the
/// thruster controls), or the belly lift (translation up).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ThrusterRole {
    Main,
    Rcs,
    Lift,
}

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
    /// The frame alone (kg).
    frame_mass: f64,
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
    pub(crate) fn key(&self) -> &str {
        &self.key
    }

    pub(crate) fn shape_key(&self) -> &str {
        &self.shape
    }

    /// The hull, on `shape`, with its stock fit (modules found by `module`):
    /// every number from the frame and what's fitted — the mass, the tank
    /// and hold, each thruster's thrust (its slot's module's rating × its
    /// share), the turning limits (the flight computer's), the power
    /// budget, the autopilots it runs; checked that each module fits its
    /// slot, the base blocks are all there, and the plant carries the load.
    pub(crate) fn build<'m>(
        self,
        shape_ref: crate::content::Handle<crate::shape::Shape>,
        shape: &crate::shape::Shape,
        module: impl Fn(&str) -> Option<(crate::content::Handle<crate::modules::Module>, &'m crate::modules::Module)>,
    ) -> Result<ClassSpec, String> {
        use crate::modules::{Does, SlotKind, BASE_BLOCKS};
        if !(self.frame_mass.is_finite() && self.frame_mass > 0.0) {
            return Err(format!("frame_mass must be positive ({})", self.frame_mass));
        }
        let slots: Vec<Slot> = self.slots.into_iter().map(|(name, kind, size)| Slot { name, kind, size }).collect();
        // The fit, slot by slot.
        let mut fit: Vec<(String, crate::content::Handle<crate::modules::Module>)> = Vec::new();
        let mut fitted: Vec<(&Slot, &crate::modules::Module)> = Vec::new();
        for (slot_name, key) in &self.fit {
            let slot = slots.iter().find(|s| &s.name == slot_name).ok_or_else(|| format!("fit: no slot '{slot_name}'"))?;
            let (h, m) = module(key).ok_or_else(|| format!("fit: no module '{key}'"))?;
            if m.does.slot() != slot.kind {
                return Err(format!("fit: {} ({:?}) doesn't go in slot '{}' ({:?})", m.key, m.does.slot(), slot.name, slot.kind));
            }
            if m.size > slot.size {
                return Err(format!("fit: {} (size {}) is too big for slot '{}' (size {})", m.key, m.size, slot.name, slot.size));
            }
            if fit.iter().any(|(s, _)| s == slot_name) {
                return Err(format!("fit: slot '{slot_name}' twice"));
            }
            fit.push((slot_name.clone(), h));
            fitted.push((slot, m));
        }
        for base in BASE_BLOCKS {
            if !fitted.iter().any(|(s, _)| s.kind == base) {
                return Err(format!("no {base:?}: every ship must carry one"));
            }
        }
        let modules = || fitted.iter().map(|(_, m)| *m);
        let dry_mass = self.frame_mass + modules().map(|m| m.mass).sum::<f64>();
        let fuel_capacity: f64 = modules().filter_map(|m| if let Does::Tank { capacity } = m.does { Some(capacity) } else { None }).sum();
        let hold_capacity: f64 = modules().filter_map(|m| if let Does::Rack { capacity } = m.does { Some(capacity) } else { None }).sum();
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
        // The thrusters: each nozzle by its slot's module.
        let mut thrusters = Vec::new();
        for t in self.thrusters {
            let n = shape.node(&t.nozzle).filter(|n| n.role == crate::shape::Role::Nozzle).ok_or_else(|| format!("no nozzle '{}' on {}", t.nozzle, shape.key))?;
            if !(t.share.is_finite() && t.share > 0.0) {
                return Err(format!("thruster at {}: share must be positive ({})", t.nozzle, t.share));
            }
            let slot = slots.iter().find(|s| s.name == t.slot).ok_or_else(|| format!("thruster at {}: no slot '{}'", t.nozzle, t.slot))?;
            // (An empty slot: the nozzle's there, nothing drives it.)
            let Some((_, m)) = fitted.iter().find(|(s, _)| s.name == slot.name) else { continue };
            let (role, rating) = match (slot.kind, &m.does) {
                (SlotKind::Drive, Does::Drive { thrust }) => (ThrusterRole::Main, *thrust),
                (SlotKind::Thrusters, Does::Thrusters { thrust }) => (ThrusterRole::Rcs, *thrust),
                (SlotKind::Lift, Does::Lift { thrust }) => (ThrusterRole::Lift, *thrust),
                _ => return Err(format!("thruster at {}: slot '{}' doesn't drive nozzles", t.nozzle, t.slot)),
            };
            thrusters.push(Thruster { nozzle: t.nozzle, role, at: n.at, push: -n.dir, thrust: rating * t.share });
        }
        // Each role's push along a direction (only thrusters pushing that way count).
        let along = |role: ThrusterRole, d: DVec3| thrusters.iter().filter(|t| t.role == role).map(|t| t.thrust * t.push.dot(d).max(0.0)).sum::<f64>();
        let main_thrust = along(ThrusterRole::Main, DVec3::NEG_Z);
        let lift_thrust = along(ThrusterRole::Lift, DVec3::Y);
        let rcs_thrust = [DVec3::X, DVec3::NEG_X, DVec3::NEG_Y, DVec3::Z, DVec3::NEG_Z].iter().map(|&d| along(ThrusterRole::Rcs, d)).fold(f64::INFINITY, f64::min);
        let mass = dry_mass + fuel_capacity;
        let inertia = shape.solid.inertia * (mass / shape.solid.volume);
        let turn_accel = crate::thrusters::turn_envelope(&thrusters, shape.solid.centroid, mass, inertia);
        Ok(ClassSpec {
            key: self.key,
            name: self.name,
            shape: self.shape,
            shape_ref,
            frame_mass: self.frame_mass,
            slots,
            fit,
            dry_mass,
            fuel_capacity,
            hold_capacity,
            power_output,
            power_draw,
            features,
            thrusters,
            main_thrust,
            rcs_thrust,
            lift_thrust,
            turn_accel,
            turn_rate,
            roll_rate,
            radius: self.radius,
            drag_area: self.drag_area,
            hull_strength: self.hull_strength,
        })
    }
}

impl ClassSpec {
    /// Its shape.
    pub fn shape(&self) -> &'static crate::shape::Shape {
        crate::content::content().get(self.shape_ref)
    }
}

/// A hull of the loaded content.
pub type Hull = crate::content::Handle<ClassSpec>;

/// The starting hull.
pub fn starting_hull() -> Hull {
    crate::content::content().handle(STARTING_HULL).expect("the content has the starting hull (checked at load)")
}

/// The Cobra's numbers (for tests and defaults).
pub fn cobra() -> &'static ClassSpec {
    crate::content::content().get(starting_hull())
}

/// A ship's size as traffic lays out room for it (pads, docking slots,
/// corridors), and the Cobra's collision radius (m).
pub const SHIP_RADIUS: f64 = 12.0;
/// The drives' exhaust velocity (m/s): a torch drive (a few percent of
/// light speed), so a tank lasts a day of burning. Each device burns its
/// thrust / this, in kg/s. (The economy's knob: see `economy`.)
pub const EXHAUST_VELOCITY: f64 = 1.0e7;
/// The hyperdrive's draw at full throttle (kg/s), while engaged.
pub const HYPER_FUEL_FLOW: f64 = 0.2;
fn full_tank() -> f64 {
    cobra().fuel_capacity
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
    /// In this spaceport's hangar (off the pads, out of sight; still
    /// `Landed`, parked at the hangar): see `spaceport::hangar`.
    #[serde(default)]
    pub hangar: Option<usize>,
    /// The hull it's built as (stored by its content key).
    #[serde(default = "starting_hull")]
    pub class: Hull,
    /// Fuel on board (kg).
    #[serde(default = "full_tank")]
    pub fuel: f64,
    /// Cargo on board (kg): the mass of what's in the hold.
    #[serde(default)]
    pub cargo: f64,
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
            fuel: cobra().fuel_capacity,
            class: starting_hull(),
            hangar: None,
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
        self.spec().dry_mass + self.fuel + self.cargo + self.hopper
    }

    /// What its class is built with.
    pub fn spec(&self) -> &'static ClassSpec {
        crate::content::content().get(self.class)
    }

    /// Its centre of mass (shape frame, m): its shape's, as a solid.
    /// (Fuel and cargo are taken as spread like the hull, for now.)
    pub fn centre_of_mass(&self) -> DVec3 {
        self.spec().shape().solid.centroid
    }

    /// Its inertia tensor about its centre of mass (kg·m², body frame): its
    /// shape's solid, as heavy as the ship is now.
    pub fn inertia(&self) -> glam::DMat3 {
        let solid = &self.spec().shape().solid;
        solid.inertia * (self.mass() / solid.volume)
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
        let s = self.spec();
        let inertia = self.inertia();
        let w = self.angular_velocity;
        // The push: the main drive along the nose, the thrusters as set.
        let c = self.rcs.clamp(DVec3::splat(-1.0), DVec3::ONE);
        let force = if push { DVec3::NEG_Z * (self.throttle.clamp(0.0, 1.0) * s.main_thrust) + DVec3::new(c.x * s.rcs_thrust, c.y * if c.y > 0.0 { s.lift_thrust } else { s.rcs_thrust }, c.z * s.rcs_thrust) } else { DVec3::ZERO };
        // The turn: toward the rates asked, as fast as the span allows.
        let want = turn.map(|t| DVec3::new(t.pitch.clamp(-1.0, 1.0) * s.turn_rate, t.yaw.clamp(-1.0, 1.0) * s.turn_rate, t.roll.clamp(-1.0, 1.0) * s.roll_rate));
        let torque = want.map_or(DVec3::ZERO, |want| inertia * ((want - w) / dt.max(TURN_RESPONSE)));
        if self.fuel <= 0.0 || (force == DVec3::ZERO && torque.length_squared() < 1e-6) {
            // (Nothing asked, or nothing to burn: every jet off.)
            self.jets.iter_mut().for_each(|j| *j = 0.0);
            self.applied = (DVec3::ZERO, DVec3::ZERO);
        } else {
            self.applied = crate::thrusters::allocate(&s.thrusters, self.centre_of_mass(), self.mass(), inertia, force, torque, &mut self.jets);
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

    /// Room left in the hold (kg).
    pub fn hold_room(&self) -> f64 {
        (self.spec().hold_capacity - self.cargo - self.hopper).max(0.0)
    }

    /// Main engine acceleration at full throttle (m/s^2): thrust / mass.
    pub fn main_accel(&self) -> f64 {
        self.spec().main_thrust / self.mass()
    }

    /// Translation thruster acceleration, per axis (m/s^2).
    pub fn side_accel(&self) -> f64 {
        self.spec().rcs_thrust / self.mass()
    }

    /// Lift thruster acceleration (m/s^2).
    pub fn lift_accel(&self) -> f64 {
        self.spec().lift_thrust / self.mass()
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
        ShipCommands { throttle: self.throttle, rcs: self.rcs, turn: None, hyperdrive: None, weapons: None, arm: None, gun_target: None, anchor: None, excavate: None, hangar: None }
    }

    /// The main engine and thrusters take their new settings.
    pub(crate) fn set_controls(&mut self, c: &ShipCommands) {
        self.throttle = c.throttle;
        self.rcs = c.rcs;
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
    fn the_cobras_thrusters_add_up_to_its_envelope() {
        let c = cobra();
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
        assert!((s.inertia().y_axis.y / yaw - (s.mass() / (s.mass() + s.spec().fuel_capacity))).abs() < 1e-9, "lighter, easier to turn");
    }
}
