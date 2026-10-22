//! The world ships move through — the galaxy, its gate network, the clock and
//! the star systems generated so far — and the ship step, where commands meet
//! the devices, Dogma and the world's contact rules.
//!
//! The ship step reads the ship and its commands, and whatever Dogma
//! reports; nothing about where the ship is trying to go. Whatever flies it
//! (a pilot, a program) has had its say before the step, as device settings
//! that hold through it: the physics integrates, nothing else runs inside.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use glam::{DQuat, DVec3};
use universe_physics::integrate::FINE_STEP;
use universe_physics::{integrate, Driver, Ephemeris, Fact, Response, RigidBody, Span, Weld};

use crate::events::ShipEvent;
use crate::galaxy::Galaxy;
use crate::gate::{self, GateFrame};
use crate::hyperdrive;
use crate::network;
use crate::ship::{upright, Controls, HyperdriveCommand, Ship, ShipCommands, ShipState, TAXI_SPEED};
use crate::system::StarSystem;
use crate::traffic::Facility;
use crate::units::LIGHT_YEAR;
use crate::goods::Item;
use crate::weapons::{Beam, Impact, Slug};

/// Neighbouring stars checked for hyperdrive obstacles and system hand-over.
pub const NEIGHBOURS: usize = 24;

#[derive(Clone, Copy, Debug, Default)]
pub struct StepResult {
    /// Simulated seconds this step.
    pub simulated: f64,
    /// True if the requested warp could not be reached.
    pub warp_limited: bool,
}

pub struct World {
    pub galaxy: Galaxy,
    isolated: bool,
    surfaces: std::sync::Arc<crate::worlds::pgs::Sources>,
    /// Seconds since the epoch.
    pub time: f64,
    pub home_system: usize,
    /// Gate links between star systems (galaxy indices).
    pub gate_links: Vec<(usize, usize)>,
    // Caches, filled on first use, shared by every ship (behind locks, so
    // ships can step side by side on several threads):
    systems: Mutex<HashMap<usize, Arc<StarSystem>>>,
    /// Nearest stars of each system visited (see `NEIGHBOURS`).
    neighbours: Mutex<HashMap<usize, Arc<[usize]>>>,
    /// Body snapshots per system for the current moment, shared by every ship there.
    ephemerides: Mutex<HashMap<usize, (f64, Arc<Ephemeris>)>>,
    /// The same for an asteroid field's bodies, by (system, field).
    field_ephemerides: Mutex<HashMap<(usize, usize), EphemerisAt>>,
    /// Fields' bodies with what's been dug out of their rocks gone, by
    /// (system, field), with the total dug they were made for (see `field_bodies_now`).
    depleted: Mutex<HashMap<(usize, usize), (f64, FieldBodies)>>,
    /// Body positions per system at the last two moments asked for: see `rails_at`.
    rails: Mutex<HashMap<usize, [RailsAt; 2]>>,
    /// Each system's contact rules, as their owners registered them (see `rules`).
    rules: Mutex<HashMap<usize, Arc<crate::rules::Rules>>>,
    /// While ships step side by side: what they'll look up, gathered first
    /// and read without locks (see `freeze`).
    frozen: Option<Frozen>,
    /// Slugs in flight (see `weapons`).
    pub slugs: Vec<Slug>,
    /// Missiles in flight (see `missiles`).
    pub missiles: Vec<crate::missiles::Missile>,
    /// Laser beams fired in the last combat phase.
    pub beams: Vec<Beam>,
    /// Hits in the last combat phase.
    pub impacts: Vec<Impact>,
    /// The goods traded in this galaxy (see `goods`).
    pub goods: Vec<Item>,
    /// Ore dug out of asteroids so far (kg), by (system, field, body among
    /// the field's bodies): see `dug`.
    pub mined: HashMap<(usize, usize, usize), f64>,
    /// Defence turrets by system, met so far, and their guns' cooldowns (see `turrets`).
    pub(crate) turrets: Mutex<HashMap<usize, Arc<Vec<crate::turrets::Turret>>>>,
    pub(crate) turret_guns: HashMap<universe_protocol::ShipId, crate::turrets::TurretGun>,
}

/// What ships stepping side by side will look up, gathered before they
/// start (see `World::freeze`): read by every thread without locks.
#[derive(Default)]
struct Frozen {
    systems: HashMap<usize, Arc<StarSystem>>,
    neighbours: HashMap<usize, Arc<[usize]>>,
    rails: HashMap<(usize, u64), Arc<Vec<DVec3>>>,
    ephemerides: HashMap<(usize, u64), Arc<Ephemeris>>,
    turrets: HashMap<usize, Arc<Vec<crate::turrets::Turret>>>,
    rules: HashMap<usize, Arc<crate::rules::Rules>>,
}

/// Lock a cache (a poisoned one is still good: caches hold no invariants).
fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

/// A ship step's clock moved on by `dt`.
fn advance(clock: &mut f64, dt: f64) -> StepResult {
    *clock += dt;
    StepResult { simulated: dt, warp_limited: false }
}

/// A field's bodies.
type FieldBodies = Arc<Vec<crate::system::Body>>;

/// A body snapshot and its moment.
type EphemerisAt = (f64, Arc<Ephemeris>);

/// Body positions at a moment (see `World::rails_now`).
type RailsAt = (f64, Arc<Vec<DVec3>>);

impl World {
    /// The settled systems (the gate network's), in order.
    pub fn settled(&self) -> Vec<usize> {
        crate::gate::settled(&self.gate_links)
    }

    pub fn new(seed: u64) -> Self {
        Self::with_surfaces(seed, Default::default())
    }

    /// Construct an instance with explicit immutable terrain sources, shared with its charts.
    pub fn with_surfaces(seed: u64, surfaces: std::sync::Arc<crate::worlds::pgs::Sources>) -> Self {
        let galaxy = Galaxy::generate(seed);
        let home_system = network::find_home(&galaxy, seed);
        let gate_links = network::build(&galaxy, home_system);
        Self::empty(galaxy, home_system, gate_links, surfaces, false)
    }

    /// A single explicit system with independent time, caches and combat state.
    /// No galaxy generation, gate network, or implicit system generation occurs.
    /// Events remain caller-owned, as with `step_ship` and `command`.
    /// The existing integrator requires body 0 as a root (it may be inert).
    pub fn isolated(galaxy: Galaxy, system: StarSystem) -> Result<Self, String> {
        if galaxy.stars.len() != 1 || system.index != 0 {
            return Err("an isolated world requires one galaxy star and system index 0".into());
        }
        if system.bodies.first().is_none_or(|b| b.rail.parent.is_some()) {
            return Err("an isolated system requires body 0 as its root".into());
        }
        let world = Self::empty(galaxy, 0, Vec::new(), Default::default(), true);
        lock(&world.systems).insert(0, Arc::new(system));
        Ok(world)
    }

    fn empty(galaxy: Galaxy, home_system: usize, gate_links: Vec<(usize, usize)>, surfaces: Arc<crate::worlds::pgs::Sources>, isolated: bool) -> Self {
        Self {
            isolated,
            surfaces,
            galaxy,
            time: 0.0,
            home_system,
            gate_links,
            systems: Default::default(),
            neighbours: Default::default(),
            ephemerides: Default::default(),
            field_ephemerides: Default::default(),
            depleted: Default::default(),
            rails: Default::default(),
            rules: Default::default(),
            frozen: None,
            slugs: Vec::new(),
            missiles: Vec::new(),
            beams: Vec::new(),
            impacts: Vec::new(),
            goods: if isolated { Vec::new() } else { crate::goods::catalog() },
            mined: HashMap::new(),
            turrets: Default::default(),
            turret_guns: HashMap::new(),
        }
    }

    pub fn gate_links_of(&self, i: usize) -> Vec<(usize, String)> {
        network::links_of(&self.gate_links, &self.galaxy, i)
    }

    /// Get (generating and caching if needed) the star system at galaxy index `i`.
    /// A star system already generated, if it is (no generating from `&self`).
    pub fn system_if_known(&self, i: usize) -> Option<Arc<StarSystem>> {
        lock(&self.systems).get(&i).cloned()
    }

    pub fn system(&self, i: usize) -> Arc<StarSystem> {
        if let Some(s) = self.frozen.as_ref().and_then(|f| f.systems.get(&i)) {
            return s.clone();
        }
        if let Some(sys) = lock(&self.systems).get(&i) {
            return sys.clone();
        }
        assert!(!self.isolated, "system {i} is not in this isolated world");
        // (Made with the cache let go: making one reads its worlds from disk, and other ships' steps ask the
        // cache meanwhile. Two made at once: the first kept.)
        let mut sys = crate::charts::generate(&self.galaxy, &self.gate_links, i);
        crate::worlds::pgs::apply_sources(&mut sys, &self.surfaces);
        let sys = Arc::new(sys);
        let mut systems = lock(&self.systems);
        if systems.len() > 64 {
            // Keep the gate network's systems, where the traffic is.
            systems.retain(|&k, _| self.gate_links.iter().any(|&(a, b)| a == k || b == k));
        }
        systems.entry(i).or_insert(sys).clone()
    }

    /// The charts of this galaxy, to share (with the client).
    pub fn charts(&self) -> crate::charts::Charts {
        if self.isolated {
            return crate::charts::Charts::isolated(self.galaxy.clone(), self.system(0));
        }
        crate::charts::Charts::new(self.galaxy.seed, self.galaxy.clone(), self.gate_links.clone(), self.goods.clone(), self.home_system).with_surfaces(self.surfaces.clone())
    }

    /// The stars nearest to star `i`, nearest first (see `NEIGHBOURS`).
    pub fn neighbours(&self, i: usize) -> Arc<[usize]> {
        if let Some(n) = self.frozen.as_ref().and_then(|f| f.neighbours.get(&i)) {
            return n.clone();
        }
        lock(&self.neighbours).entry(i).or_insert_with(|| self.galaxy.nearest(i, NEIGHBOURS).into()).clone()
    }

    /// Distance in light years between two stars.
    pub fn distance_ly(&self, a: usize, b: usize) -> f64 {
        self.galaxy.offset(a, b).length() / LIGHT_YEAR
    }

    /// The body snapshot for `sys` at time `t`, computed once and shared by
    /// every ship stepping from that moment.
    fn ephemeris(&self, sys: &StarSystem, t: f64) -> Arc<Ephemeris> {
        if let Some(e) = self.frozen.as_ref().and_then(|f| f.ephemerides.get(&(sys.index, t.to_bits()))) {
            return e.clone();
        }
        if let Some((at, e)) = lock(&self.ephemerides).get(&sys.index)
            && *at == t
        {
            return e.clone();
        }
        // (Worked out with the cache let go.)
        let e = Arc::new(sys.ephemeris(t));
        lock(&self.ephemerides).insert(sys.index, (t, e.clone()));
        e
    }

    /// What's been dug out of rock `rock` among field `field`'s bodies in `system` (kg).
    pub fn dug(&self, system: usize, field: usize, rock: usize) -> f64 {
        self.mined.get(&(system, field, rock)).copied().unwrap_or(0.0)
    }

    /// A tonne dug out of that rock (see `ShipEvent::Mined`), to be remembered.
    pub fn dig(&mut self, system: usize, field: usize, rock: usize) {
        *self.mined.entry((system, field, rock)).or_default() += crate::goods::TONNE;
    }

    /// `ephemeris` for field `f`'s bodies (see `StarSystem::field_bodies`).
    fn field_ephemeris(&self, sys: &StarSystem, f: usize, t: f64) -> Arc<Ephemeris> {
        if let Some((at, e)) = lock(&self.field_ephemerides).get(&(sys.index, f))
            && *at == t
        {
            return e.clone();
        }
        let e = Arc::new(Ephemeris::new(&sys.field_bodies(f)[..], t));
        lock(&self.field_ephemerides).insert((sys.index, f), (t, e.clone()));
        e
    }

    /// Field `f`'s bodies as they are now: rocks dug into are the smaller
    /// for it (what's seen is what's touched).
    pub fn field_bodies_now(&self, sys: &StarSystem, f: usize) -> Arc<Vec<crate::system::Body>> {
        let bodies = sys.field_bodies(f);
        let here = |((s, field, _), _): &(&(usize, usize, usize), &f64)| *s == sys.index && *field == f;
        // (Digging only ever adds: the total says whether it's changed. Asked every flight step: nothing
        // gathered unless it has.)
        let total: f64 = self.mined.iter().filter(here).map(|(_, &kg)| kg).sum();
        if total == 0.0 {
            return bodies;
        }
        if let Some((at, b)) = lock(&self.depleted).get(&(sys.index, f))
            && *at == total
        {
            return b.clone();
        }
        let dug: Vec<(usize, f64)> = self.mined.iter().filter(here).map(|(&(_, _, b), &kg)| (b, kg)).collect();
        let mut now = (*bodies).clone();
        for (i, kg) in dug {
            if let Some(b) = now.get_mut(i) {
                crate::belt::shrink(b, kg);
            }
        }
        crate::system::settle_bodies(&mut now);
        let now = Arc::new(now);
        lock(&self.depleted).insert((sys.index, f), (total, now.clone()));
        now
    }

    /// The asteroid field ship `p` is among or near in `system` at `t`, if any:
    /// a field's swarm, else the belt patch of the nearest belt rock close by.
    pub fn field_at(&self, sys: &StarSystem, system: usize, p: DVec3, t: f64) -> Option<usize> {
        if !sys.fields.is_empty()
            && let Some(f) = sys.field_near(p, &self.rails_at(system, t))
        {
            return Some(f);
        }
        let close = universe_physics::integrate::FINE_RANGE;
        // (Belt rocks go round the star, which itself wobbles round the system's middle.)
        let star = self.rails_at(system, t).first().copied().unwrap_or_default();
        let p = p - star;
        // (Most ships are nowhere near a belt: by their distance from the star alone.)
        let r = p.length();
        if !sys.belts.iter().any(|b| r + close >= b.inner * (1.0 - crate::belts::rocks().most_eccentric) && r - close <= b.outer * (1.0 + crate::belts::rocks().most_eccentric)) {
            return None;
        }
        let n = sys.bodies.len();
        crate::belts::patches_near(&sys.belts, p, t, close, 0.0)
            .into_iter()
            .filter_map(|patch| {
                let f = crate::belts::patch_field(patch);
                let bodies = sys.field_bodies(f);
                (n..bodies.len()).filter_map(|i| Some(bodies[i].rail.orbit.as_ref()?.position(t).distance(p) - bodies[i].rail.radius)).filter(|&gap| gap < close).map(|gap| (f, gap)).min_by(|a, b| a.1.total_cmp(&b.1))
            })
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(f, _)| f)
    }

    /// Hand the ship's devices new commands, with no time passing: the engine
    /// and thrusters take their settings, then the hyperdrive engages or
    /// disengages if told to. (Turning takes time: see `step_ship`.)
    pub fn command(&mut self, ship: &mut Ship, system: usize, c: &ShipCommands, events: &mut Vec<ShipEvent>) {
        self.command_at(ship, system, c, self.time, events);
    }

    /// `command` at time `t`.
    pub fn command_at(&self, ship: &mut Ship, system: usize, c: &ShipCommands, t: f64, events: &mut Vec<ShipEvent>) {
        match c.power {
            Some(true) if !ship.powered => {
                ship.powered = true;
                events.push(ShipEvent::SystemsOn);
            }
            // Powering down is for a parked ship only.
            Some(false) if ship.powered && matches!(ship.state, ShipState::Landed { .. }) => {
                ship.powered = false;
                events.push(ShipEvent::SystemsOff);
            }
            Some(false) if ship.powered => events.push(ShipEvent::SystemsRefused { why: "NOT WHILE FLYING".into() }),
            _ => {}
        }
        ship.set_controls(c);
        if let Some(on) = c.arm {
            crate::weapons::master_arm(ship, on, events);
        }
        use crate::modules::Gear;
        match c.anchor {
            Some(true) if !ship.spec().has(Gear::MiningRig) => events.push(ShipEvent::NotFitted { what: "MINING RIG".into() }),
            Some(true) if !matches!(ship.state, ShipState::Anchored { .. }) => {
                let sys = self.system(system);
                let field = self.field_at(&sys, system, ship.position, t);
                let bodies = field.map(|f| (f, self.field_bodies_now(&sys, f)));
                crate::mining::anchor(bodies, ship, t, events);
            }
            Some(false) => crate::mining::release(ship, events),
            _ => {}
        }
        if let Some(h) = c.hangar {
            self.hangar_move(ship, system, h, t, events);
        }
        if let Some(on) = c.excavate {
            ship.excavator = on && matches!(ship.state, ShipState::Anchored { .. });
            if on && !ship.excavator {
                events.push(ShipEvent::ExcavatorStopped { why: "NOT ANCHORED".into() });
            }
        }
        match &c.hyperdrive {
            Some(h) if h.engage && h.start && !ship.hyperdrive && !ship.spec().has(Gear::Hyperdrive) => events.push(ShipEvent::NotFitted { what: "HYPERDRIVE".into() }),
            // (Off always; on only by the engage control.)
            Some(h) if h.engage != ship.hyperdrive && (h.start || !h.engage) => {
                let sys = self.system(system);
                let positions = self.rails_at(system, t);
                hyperdrive::switch(&sys, ship, h, t, &positions, events);
                ship.hyper_orders = HyperdriveCommand::CRUISE;
            }
            // Engaged: how to fly, held until changed.
            Some(h) if ship.hyperdrive => ship.hyper_orders = *h,
            _ => {}
        }
    }

    /// A move between a spaceport's pads and its hangar: from the pad it's
    /// landed on, it taxis into the hangar (and is in it, out of sight, once
    /// there); from the hangar it rolls out of the door and taxis to pad
    /// `pad` (traffic control's to give; the world sees only that it's a pad
    /// of the port). See `taxi_step`.
    fn hangar_move(&self, ship: &mut Ship, system: usize, h: universe_protocol::HangarCommand, t: f64, events: &mut Vec<ShipEvent>) {
        use crate::spaceport::PADS;
        use universe_protocol::HangarCommand;
        let ShipState::Landed { body, local_position, .. } = ship.state.clone() else {
            return events.push(ShipEvent::HangarRefused { why: "NOT LANDED".into() });
        };
        if ship.taxi.is_some() {
            return events.push(ShipEvent::HangarRefused { why: "TAXIING".into() });
        }
        let sys = self.system(system);
        match (h, ship.hangar) {
            (HangarCommand::Enter, None) => {
                let Some(port) = crate::port::at(&sys, body, local_position).filter(|&p| crate::port::pad_at(&sys, p, local_position).is_some()) else {
                    return events.push(ShipEvent::HangarRefused { why: "NOT ON A PAD".into() });
                };
                ship.taxi = Some(crate::ship::Taxi { port, pad: None });
            }
            (HangarCommand::Leave { pad }, Some(port)) if pad < PADS && crate::port::body(&sys, port) == Some(body) => {
                // Out of the door, in sight again, and taxiing.
                let local_position = crate::port::hangar(&sys, port, ship.rest_height());
                let local_orientation = crate::port::resting(port, local_position, crate::port::pad(&sys, port, pad, ship.rest_height()) - local_position);
                let mut rigid = ship.rigid();
                Weld { body, local_position, local_orientation }.place(&sys.bodies, t, &self.rails_at(system, t), &mut rigid);
                ship.set_rigid(&rigid);
                ship.state = ShipState::Landed { body, local_position, local_orientation };
                ship.hangar = None;
                ship.taxi = Some(crate::ship::Taxi { port, pad: Some(pad) });
            }
            (HangarCommand::Enter, Some(_)) => events.push(ShipEvent::HangarRefused { why: "IN THE HANGAR ALREADY".into() }),
            _ => events.push(ShipEvent::HangarRefused { why: "NOT IN A HANGAR".into() }),
        }
    }

    /// A taxiing ship, `dt` seconds on: along the ground (or deck) toward
    /// the hangar or its pad at `TAXI_SPEED`, nose the way it goes; there,
    /// into the hangar (out of sight) or on the pad (ready to lift off).
    fn taxi_step(&self, sys: &StarSystem, ship: &mut Ship, dt: f64, events: &mut Vec<ShipEvent>) {
        let (Some(taxi), ShipState::Landed { body, local_position, local_orientation }) = (ship.taxi, ship.state.clone()) else { return };
        let to = match taxi.pad {
            Some(pad) => crate::port::pad(sys, taxi.port, pad, ship.rest_height()),
            None => crate::port::hangar(sys, taxi.port, ship.rest_height()),
        };
        let step = TAXI_SPEED * dt;
        let name = taxi.port.name(sys);
        let arrived = local_position.distance(to) <= step;
        let local_position = if arrived { to } else { crate::port::settle(sys, taxi.port, local_position + (to - local_position).normalize() * step, ship.rest_height()) };
        let local_orientation = if arrived { local_orientation } else { crate::port::resting(taxi.port, local_position, to - local_position) };
        ship.state = ShipState::Landed { body, local_position, local_orientation };
        if arrived {
            ship.taxi = None;
            match taxi.pad {
                None => {
                    ship.hangar = Some(taxi.port);
                    events.push(ShipEvent::EnteredHangar { port: name });
                }
                Some(pad) => events.push(ShipEvent::LeftHangar { port: name, pad }),
            }
        }
    }

    /// Ships are about to step side by side from `t0` to `t1` in `systems`:
    /// gather what they'll look up there (the systems and their neighbours,
    /// the bodies at both moments, the snapshot for the integrator, the
    /// turrets), to be read without locks until `thaw`.
    pub fn freeze(&mut self, systems: &[usize], t0: f64, t1: f64) {
        self.frozen = None;
        let mut f = Frozen::default();
        for &i in systems {
            let sys = self.system(i);
            f.neighbours.insert(i, self.neighbours(i));
            for t in [t0, t1] {
                f.rails.insert((i, t.to_bits()), self.rails_at(i, t));
            }
            f.ephemerides.insert((i, t0.to_bits()), self.ephemeris(&sys, t0));
            f.turrets.insert(i, self.turrets_of(i));
            f.rules.insert(i, self.rules_of(i));
            f.systems.insert(i, sys);
        }
        self.frozen = Some(f);
    }

    /// The contact rules of `system` (see `rules`), registered on first look.
    pub fn rules_of(&self, system: usize) -> Arc<crate::rules::Rules> {
        if let Some(r) = self.frozen.as_ref().and_then(|f| f.rules.get(&system)) {
            return r.clone();
        }
        if let Some(r) = lock(&self.rules).get(&system) {
            return r.clone();
        }
        let r = Arc::new(crate::structures::rules(&self.galaxy, &self.system(system)));
        lock(&self.rules).insert(system, r.clone());
        r
    }

    /// Back to the locked caches (see `freeze`).
    pub fn thaw(&mut self) {
        self.frozen = None;
    }

    pub(crate) fn frozen_turrets(&self, system: usize) -> Option<Arc<Vec<crate::turrets::Turret>>> {
        self.frozen.as_ref().and_then(|f| f.turrets.get(&system)).cloned()
    }

    /// Where the bodies of `system` are now: solved once per moment and
    /// shared (a thousand ships in a system ask every frame).
    pub fn rails_now(&self, system: usize) -> Arc<Vec<DVec3>> {
        self.rails_at(system, self.time)
    }

    /// Where the bodies of `system` are at `t`: kept for the last two
    /// moments asked for (each ship's turn runs from the frame's start to its
    /// end, so both are asked for, turn after turn).
    pub fn rails_at(&self, system: usize, t: f64) -> Arc<Vec<DVec3>> {
        if let Some(p) = self.frozen.as_ref().and_then(|f| f.rails.get(&(system, t.to_bits()))) {
            return p.clone();
        }
        if let Some(slots) = lock(&self.rails).get(&system)
            && let Some((_, p)) = slots.iter().find(|(at, _)| *at == t)
        {
            return p.clone();
        }
        let sys = self.system(system);
        let mut p = Vec::with_capacity(sys.bodies.len());
        sys.positions(t, &mut p);
        let p = Arc::new(p);
        let mut rails = lock(&self.rails);
        let slots = rails.entry(system).or_insert_with(|| [(f64::NAN, p.clone()), (f64::NAN, p.clone())]);
        if !slots.iter().any(|(at, _)| *at == t) {
            slots[1] = std::mem::replace(&mut slots[0], (t, p.clone()));
        }
        p
    }

    /// Advance one ship in `system` by `real_dt` real seconds, the world
    /// clock by `real_dt * warp` (less if that can't be reached in flight).
    /// `commands` are the pilot's for this frame (its `turn` over `real_dt`);
    /// `computer` gives commands while the ship moves, if it is flying it.
    #[allow(clippy::too_many_arguments)]
    pub fn step_ship(
        &mut self,
        ship: &mut Ship,
        system: &mut usize,
        commands: &ShipCommands,
        real_dt: f64,
        warp: f64,
        events: &mut Vec<ShipEvent>,
    ) -> StepResult {
        let mut clock = self.time;
        let r = self.step_ship_at(&mut clock, ship, system, commands, real_dt, warp, events);
        self.time = clock;
        r
    }

    /// `step_ship` from time `clock`, which it moves on (the world's own
    /// clock untouched: ships can step side by side).
    #[allow(clippy::too_many_arguments)]
    pub fn step_ship_at(
        &self,
        clock: &mut f64,
        ship: &mut Ship,
        system: &mut usize,
        commands: &ShipCommands,
        real_dt: f64,
        warp: f64,
        events: &mut Vec<ShipEvent>,
    ) -> StepResult {
        ship.hyper_jam = (ship.hyper_jam - real_dt * warp).max(0.0);
        self.command_at(ship, *system, commands, *clock, events);
        let sys = self.system(*system);
        if ship.devices.is_some() {
            let positions = self.rails_at(*system, *clock);
            let density = universe_physics::air_at(&sys.bodies, ship.position, &positions, *clock).map_or(0., |(rho,_)|rho);
            ship.devices.as_mut().unwrap().density = density;
        }
        // Nearby stars, when already looked up for this system.
        let mut near = None;
        let result = match ship.state.clone() {
            ShipState::Destroyed { respawn_in } => {
                let left = respawn_in - real_dt;
                ship.state = ShipState::Destroyed { respawn_in: left };
                if left <= 0.0 {
                    self.respawn_at(ship, system, *clock, events);
                }
                advance(clock, real_dt * warp)
            }
            ShipState::Landed { body, local_position, local_orientation } => {
                let weld = Weld { body, local_position, local_orientation };
                universe_prof::time("sim/crafts/tick/world step/landed", || self.landed_step(clock, &sys, *system, ship, weld, commands.turn, real_dt, warp, events))
            }
            ShipState::Anchored { field, body, .. } => {
                let result = advance(clock, real_dt * warp);
                crate::mining::hold(&sys, ship, *clock);
                crate::mining::excavate(&sys, ship, self.dug(*system, field, body), real_dt * warp, events);
                result
            }
            ShipState::Transit { to, from, remaining, duration, local_velocity, local_offset, local_orientation } => {
                // Through the tube in game time: its natural crossing for the ship's mass.
                let result = advance(clock, real_dt * warp);
                let left = remaining - real_dt * warp;
                if left > 0.0 {
                    ship.state = ShipState::Transit { to, from, remaining: left, duration, local_velocity, local_offset, local_orientation };
                } else {
                    self.arrive_through_gate(*clock, ship, system, to, from, local_velocity, local_offset, local_orientation, events);
                }
                result
            }
            ShipState::Flying if ship.hyperdrive => {
                ship.drive(commands.turn.as_ref(), real_dt, false);
                let neighbours = self.neighbours(*system);
                let result = universe_prof::time("sim/crafts/tick/world step/hyperdrive", || self.hyperdrive_step(clock, &sys, ship, *system, &neighbours, real_dt, warp, events));
                near = Some(neighbours);
                result
            }
            ShipState::Flying => {
                // The flight computer: the push and the turn, through the
                // thrusters, over the game time the step covers.
                ship.drive(commands.turn.as_ref(), real_dt * warp, true);
                universe_prof::time("sim/crafts/tick/world step/flight (physics)", || self.flight_step(clock, &sys, ship, *system, real_dt * warp, events))
            }
        };
        // The skin: its energy balance in the light and air where it is.
        ship.heat_owed += real_dt * warp;
        let every = if ship.heat_in_air { 0.1 } else { 1.0 };
        if (ship.is_flying() || matches!(ship.state, ShipState::Landed { .. })) && ship.heat_owed >= every {
            let around = universe_prof::time("sim/crafts/tick/world step/surroundings", || {
                let positions = self.rails_at(*system, *clock);
                let (sun, world) = crate::climate::light_at(&sys, &positions, ship.position);
                let air = if ship.hyperdrive {
                    None
                } else {
                    universe_physics::air_at(&sys.bodies, ship.position, &positions, *clock).map(|(density, v)| {
                        let over = sys.bodies.iter().enumerate().take(positions.len()).filter(|(_, b)| b.rail.atmosphere.is_some()).min_by(|a, b| positions[a.0].distance(ship.position).total_cmp(&positions[b.0].distance(ship.position))).map(|(i, _)| i);
                        let temp = over.map_or(250.0, |i| crate::climate::air_temperature(&sys, i, &positions, *clock, ship.position));
                        // (Resting on the ground: still in the air.)
                        let v = if ship.is_flying() { v } else { ship.velocity };
                        (density, v, temp)
                    })
                };
                crate::heat::Surroundings { sun, world, air }
            });
            ship.heat_in_air = around.air.is_some();
            let owed = std::mem::take(&mut ship.heat_owed);
            universe_prof::time("sim/crafts/tick/world step/heat", || crate::heat::heat(ship, &around, owed, events));
        }
        if ship.is_flying() {
            let _p = universe_prof::scope("sim/crafts/tick/world step/handover");
            let neighbours = near.unwrap_or_else(|| self.neighbours(*system));
            self.handover(ship, system, &neighbours, events);
        }
        result
    }

    /// The world clock, moved on by a ship step (see `step_ship_at`).
    pub fn set_time(&mut self, t: f64) {
        self.time = t;
    }

    /// Welded to the body: carried round with its orbit and spin, turning in
    /// place if commanded; the docking port launches, the landing gear lifts off.
    #[allow(clippy::too_many_arguments)]
    fn landed_step(
        &self,
        clock: &mut f64,
        sys: &StarSystem,
        system: usize,
        ship: &mut Ship,
        weld: Weld,
        turn: Option<Controls>,
        real_dt: f64,
        warp: f64,
        events: &mut Vec<ShipEvent>,
    ) -> StepResult {
        let result = advance(clock, real_dt * warp);
        let t = *clock;
        let b = &sys.bodies[weld.body];
        let rot = b.rotation(t);
        let positions = self.rails_at(system, t);
        // Taxiing: on along the ground, then placed where it's got to.
        self.taxi_step(sys, ship, real_dt * warp, events);
        let weld = match ship.state {
            ShipState::Landed { body, local_position, local_orientation } => Weld { body, local_position, local_orientation },
            _ => weld,
        };

        let mut rigid = ship.rigid();
        weld.place(&sys.bodies, t, &positions, &mut rigid);
        ship.set_rigid(&rigid);
        // On ground that takes ships by contact (a world's): asleep where it
        // lies, never turned in place; any push or turn commanded wakes it,
        // to fly from there under the ground's push (see `ground`).
        let rules = self.rules_of(system);
        if crate::ground::lands(&rules, weld.body) && ship.hangar.is_none() && ship.taxi.is_none() {
            // (What the commands really give, after allocation and device
            // limits: a fan in vacuum or an empty tank wakes nothing.)
            let (orientation, spin) = (ship.orientation, ship.angular_velocity);
            if ship.powered {
                ship.drive(turn.as_ref(), real_dt, true);
            }
            (ship.orientation, ship.angular_velocity) = (orientation, spin);
            if crate::ground::pushed(ship) {
                ship.angular_velocity = DVec3::ZERO;
                ship.state = ShipState::Flying;
                let mut touching = ship.ground.take().unwrap_or_default();
                (touching.body, touching.woken, touching.still, touching.touched) = (weld.body, true, 0.0, true);
                ship.ground = Some(touching);
                events.push(ShipEvent::RuleFired { rule: "ground".to_string(), outcome: "woken".to_string() });
            } else {
                ship.applied = (DVec3::ZERO, DVec3::ZERO);
                ship.jets.clear();
            }
            if ship.powered { ship.burn(real_dt); }
            return result;
        }
        // Allocate actual commands while the landing constraint holds position.
        // Manual jets and assisted translation use the same resource limits as flight.
        if !ship.powered {
            (ship.throttle, ship.rcs) = (0.0, DVec3::ZERO);
            ship.held = 0;
            ship.jets.clear();
            ship.applied = (DVec3::ZERO, DVec3::ZERO);
        } else {
            ship.drive(turn.as_ref(), real_dt, true);
        }
        let local_orientation = rot.inverse() * ship.orientation;
        ship.state = ShipState::Landed { body: weld.body, local_position: weld.local_position, local_orientation };
        // Let go, if the structure's release says so (its owner's rule).
        crate::rules::release(&rules, sys, weld.body, weld.local_position, ship, t, events);
        if ship.powered { ship.burn(real_dt); }
        result
    }

    /// Under hyperdrive: the drive carries the ship as last ordered.
    #[allow(clippy::too_many_arguments)]
    fn hyperdrive_step(
        &self,
        clock: &mut f64,
        sys: &StarSystem,
        ship: &mut Ship,
        system: usize,
        neighbours: &[usize],
        real_dt: f64,
        warp: f64,
        events: &mut Vec<ShipEvent>,
    ) -> StepResult {
        let result = advance(clock, real_dt * warp);
        let t = *clock;
        let positions = self.rails_at(system, t);
        let orders = ship.hyper_orders;
        hyperdrive::cruise(&self.galaxy, neighbours, system, sys, ship, &orders, t, &positions, real_dt, warp, events);
        result
    }

    /// Free flight: Dogma moves the ship under gravity and the
    /// thrust of its devices, then the world's rules judge whatever it touched.
    #[allow(clippy::too_many_arguments)]
    fn flight_step(&self, clock: &mut f64, sys: &StarSystem, ship: &mut Ship, system: usize, dt: f64, events: &mut Vec<ShipEvent>) -> StepResult {
        // Among an asteroid field, its swarm's bodies too.
        let field = self.field_at(sys, system, ship.position, *clock);
        let local = field.map(|f| self.field_bodies_now(sys, f));
        let bodies = local.as_deref().map_or(&sys.bodies[..], |b| &b[..]);
        // For short frames, snapshot the bodies once and extrapolate for each
        // substep instead of re-solving every orbit (see `Ephemeris`).
        let ephemeris = (dt <= Ephemeris::SPAN).then(|| match field {
            Some(f) => self.field_ephemeris(sys, f, *clock),
            None => self.ephemeris(sys, *clock),
        });
        // Small steps while any device pushes (engine or thrusters), whoever
        // is flying. Dogma also takes small steps near any station or
        // gate, for contact.
        let powered = ship.throttle > 0.0 || ship.rcs != DVec3::ZERO;
        let mut rigid = ship.rigid();
        let rules = self.rules_of(system);
        // Within reach of ground that takes ships by contact: its contacts,
        // in substeps short enough for a leg's spring.
        let ground = {
            let mut now = Vec::with_capacity(bodies.len());
            match &ephemeris {
                Some(e) => e.positions(*clock, &mut now),
                None => universe_physics::positions(bodies, *clock, &mut now),
            }
            let reach = 2.0 + rigid.velocity.length() * dt * 2.0;
            crate::ground::near(bodies, &rules, &now, &rigid, reach)
        };
        let physics = if ground.is_some() { crate::ground::CONTACT_STEP } else if powered { FINE_STEP } else { f64::INFINITY };
        let span = Span { t: *clock, dt, max_h: physics, contact_step: FINE_STEP };
        let mut devices = Devices::new(&mut *ship, &rules, &mut *events).among(bodies);
        if let Some(g) = ground {
            devices = devices.on_ground(g);
        }
        let mut positions = Vec::with_capacity(bodies.len());
        let out = integrate(bodies, ephemeris.as_deref(), &mut positions, &mut rigid, span, &mut devices);
        let step = devices.into_ground();
        ship.set_rigid(&rigid);
        *clock = out.time;
        if step.is_none() {
            ship.ground = None;
        }
        let rock = |f: &Fact| matches!(f, Fact::Contact(c) if bodies[c.body].kind.is_rock());
        // Struck the ground too hard to stand: wrecked, by its rule.
        if let (Some(g), Some(Fact::Contact(c))) = (&step, &out.fact)
            && c.body == g.body
            && crate::rules::Part::of(c.feature) == crate::rules::Part::Ground
        {
            if let Some(crate::rules::Rule::Lock { name, otherwise, .. }) = rules.get(c.body, crate::rules::Part::Ground) {
                events.push(ShipEvent::RuleFired { rule: name.clone(), outcome: "struck too hard: wrecked".to_string() });
                crate::damage::destroy(ship, otherwise, events);
            }
            ship.ground = None;
            return StepResult { simulated: out.simulated, warp_limited: out.limited };
        }
        if let Some(g) = step {
            g.finish(sys, &rules, ship, out.time, &positions, out.simulated, events);
        }
        if let Some(fact) = out.fact.filter(|f| !rock(f)) {
            // What it touched decides, by its owner's rule.
            crate::rules::apply(&rules, sys, system, ship, &fact, out.time, &positions, events);
        }
        StepResult { simulated: out.simulated, warp_limited: out.limited }
    }

    /// Come out of the gate in system `to` that leads back to `from`, with the
    /// same motion relative to it as we had going into the other one.
    #[allow(clippy::too_many_arguments)]
    fn arrive_through_gate(
        &self,
        t: f64,
        ship: &mut Ship,
        system: &mut usize,
        to: usize,
        from: usize,
        local_velocity: DVec3,
        local_offset: DVec3,
        local_orientation: DQuat,
        events: &mut Vec<ShipEvent>,
    ) {
        *system = to;
        let sys = self.system(to);
        let positions = self.rails_at(to, t);
        let Some(g) = sys.gate_to(from) else {
            // No return gate (shouldn't happen): a new ship at home instead.
            self.respawn_at(ship, system, t, events);
            return;
        };
        let frame = GateFrame::new(&sys, g, t, &positions);
        let mut rigid = ship.rigid();
        gate::emerge(&frame, local_velocity, local_offset, local_orientation, &mut rigid);
        ship.set_rigid(&rigid);
        ship.state = ShipState::Flying;
        events.push(ShipEvent::GateArrived { system: sys.name.clone() });
    }

    /// When a flying ship is closer to one of its system's `neighbours` than
    /// to its own star, move it into that star's frame (a floating origin at
    /// interstellar scale).
    fn handover(&self, ship: &mut Ship, system: &mut usize, neighbours: &[usize], events: &mut Vec<ShipEvent>) {
        let p = ship.position;
        let mut best = (*system, p.length());
        for &n in neighbours {
            let d = self.galaxy.offset(*system, n).distance(p);
            if d < best.1 {
                best = (n, d);
            }
        }
        if best.0 != *system {
            let offset = self.galaxy.offset(*system, best.0);
            ship.position -= offset;
            *system = best.0;
            let name = self.system(best.0).name.clone();
            events.push(ShipEvent::EnteredSystem { name });
        }
    }

    /// A new ship next to the home station, matching its orbit.
    pub fn respawn(&mut self, ship: &mut Ship, system: &mut usize, events: &mut Vec<ShipEvent>) {
        self.respawn_at(ship, system, self.time, events);
    }

    /// `respawn` at time `t`.
    pub fn respawn_at(&self, ship: &mut Ship, system: &mut usize, t: f64, events: &mut Vec<ShipEvent>) {
        *system = self.home_system;
        let sys = self.system(*system);
        let station = sys.station().unwrap_or(0);
        let positions = self.rails_at(*system, t);
        let pos = positions[station];
        let vel = sys.velocity(station, t);
        let parent = sys.bodies[station].rail.parent.unwrap_or(0);
        let rel_vel = vel - sys.velocity(parent, t);
        let prograde = rel_vel.normalize();
        let radial = (pos - positions[parent]).normalize();

        // 4 km behind the station on the same orbit, nose pointing at it:
        // the same hull and fit as the one lost (its insurer's; whether the
        // excess is paid is the ledger's business), a full tank.
        let ship_pos = pos - prograde * 4000.0;
        let orientation = upright(radial, prograde);
        let (class, fit, trim) = (ship.class, ship.fit.clone(), ship.trim.clone());
        *ship = Ship::new(ship_pos, vel, orientation);
        ship.class = class;
        ship.trim = trim;
        if let Some(fit) = fit {
            let _ = ship.refit((*fit).clone());
        }
        ship.fuel = ship.spec().fuel_capacity;
        ship.energy = ship.spec().capacitor_capacity;
        events.push(ShipEvent::Respawned);
    }

    /// A new ship docked at a station or landed at a spaceport in `system`.
    pub fn ship_at(&mut self, system: usize, at: Facility) -> Ship {
        self.ship_on(system, at, crate::spaceport::CENTER_PAD)
    }

    /// A ship resting at `at` in `system`: on pad `pad` of a station's deck
    /// or a spaceport.
    /// A ship set down (on a port or open ground) stood again at its own
    /// height (`Ship::rest_height`): after its hull changed under it (a new
    /// one bought, another commissioned), on its feet, not in the deck or over it.
    pub fn resettle(&self, system: usize, ship: &mut Ship) {
        let ShipState::Landed { body, local_position, local_orientation } = ship.state else { return };
        let sys = self.system(system);
        let h = ship.rest_height();
        let local_position = match crate::port::at(&sys, body, local_position) {
            Some(port) => crate::port::settle(&sys, port, local_position, h),
            None => {
                let dir = local_position.normalize();
                dir * (sys.bodies[body].surface_radius(dir) + h)
            }
        };
        let mut rigid = ship.rigid();
        Weld { body, local_position, local_orientation }.place(&sys.bodies, self.time, &self.rails_now(system), &mut rigid);
        ship.set_rigid(&rigid);
        ship.state = ShipState::Landed { body, local_position, local_orientation };
    }

    pub fn ship_on(&mut self, system: usize, at: Facility, pad: usize) -> Ship {
        let sys = self.system(system);
        let positions = self.rails_now(system);
        let mut ship = Ship::new(DVec3::ZERO, DVec3::ZERO, DQuat::IDENTITY);
        let at = match at {
            Facility::Station(_) | Facility::Spaceport(_) if crate::port::body(&sys, at).is_some() => at,
            _ => Facility::Spaceport(0),
        };
        let body = crate::port::body(&sys, at).expect("a port");
        let local_position = crate::port::pad(&sys, at, pad, ship.rest_height());
        let local_orientation = match at {
            Facility::Station(_) => crate::station::parked(),
            _ => crate::port::resting(at, local_position, local_position.any_orthonormal_vector()),
        };
        let mut rigid = ship.rigid();
        Weld { body, local_position, local_orientation }.place(&sys.bodies, self.time, &positions, &mut rigid);
        ship.set_rigid(&rigid);
        ship.state = ShipState::Landed { body, local_position, local_orientation };
        // (Parked: powered down.)
        ship.powered = false;
        ship
    }
}

/// The ship's devices during a flight step, as Dogma's force callback:
/// they push with the thrust they're set to (held through the step).
/// Contacts are judged by world rules. (The same devices fly a copy of the
/// ship when a flight is simulated ahead, e.g. by a flight planner through
/// `universe_physics::simulate`.)
pub struct Devices<'a> {
    ship: &'a mut Ship,
    rules: &'a crate::rules::Rules,
    events: &'a mut Vec<ShipEvent>,
    /// The bodies flown among (for what an asteroid is: see `among`).
    bodies: &'a [crate::system::Body],
    /// Near ground that takes ships by contact: its contacts, each substep (see `ground`).
    ground: Option<crate::ground::Step>,
}

impl<'a> Devices<'a> {
    /// `ship`'s devices, among parts with `rules`; what they do goes to
    /// `events`. Dogma's body stands for the ship's motion (see
    /// `Ship::rigid`/`set_rigid`); the ship keeps the device settings.
    pub fn new(ship: &'a mut Ship, rules: &'a crate::rules::Rules, events: &'a mut Vec<ShipEvent>) -> Self {
        Self { ship, rules, events, bodies: &[], ground: None }
    }

    /// Near the ground of `body` (among `among`'s bodies): touching it pushes
    /// back at every point (see `ground`).
    pub fn on_ground(self, body: usize) -> Self {
        let step = crate::ground::Step::new(self.ship, body);
        Self { ground: Some(step), ..self }
    }

    /// The ground contact's state after the step.
    pub fn into_ground(self) -> Option<crate::ground::Step> {
        self.ground
    }

    /// Flying among `bodies`: touching an asteroid among them is a
    /// collision (see `mining::strike`), whoever's rules.
    pub fn among(self, bodies: &'a [crate::system::Body]) -> Self {
        Self { bodies, ..self }
    }
}

impl Driver for Devices<'_> {
    fn applied(&mut self, body: &mut RigidBody, t: f64, h: f64, positions: &[DVec3]) -> DVec3 {
        let ship = &mut *self.ship;
        ship.position = body.position;
        ship.velocity = body.velocity;
        ship.orientation = body.orientation;
        let thrust = ship.thrust();
        ship.burn(h);
        let pushed = match &mut self.ground {
            Some(g) => g.apply(self.bodies, t, positions, body, ship, self.events),
            None => DVec3::ZERO,
        };
        thrust + pushed
    }

    fn respond(&mut self, fact: &Fact, _: &RigidBody) -> Response {
        // Ground that takes ships by contact: its push holds the ship (see
        // `applied`), unless it struck too hard (its rule's limit: wrecked).
        if let (Some(g), Fact::Contact(c)) = (&self.ground, fact)
            && c.body == g.body
            && crate::rules::Part::of(c.feature) == crate::rules::Part::Ground
        {
            let limit = match self.rules.get(c.body, crate::rules::Part::Ground) {
                Some(crate::rules::Rule::Lock { max_speed, .. }) => *max_speed,
                _ => 0.0,
            };
            let into = (-c.relative_velocity.dot(c.normal)).max(g.touching.last.strike);
            return if into < limit { Response::Continue } else { Response::Stop };
        }
        match fact {
            Fact::Contact(c) if self.bodies.get(c.body).is_some_and(|b| b.kind.is_rock()) => {
                if crate::mining::strike(self.ship, &self.bodies[c.body], c, self.events) {
                    Response::Bounce { restitution: crate::mining::RESTITUTION, separation: 0.1, push: 0.2 }
                } else {
                    Response::Stop
                }
            }
            // A part that bounces gentle contact (its rule): off it, and keep flying.
            Fact::Contact(c) if self.rules.bounces(c.body, crate::rules::Part::of(c.feature), c.relative_velocity.length()) => {
                self.events.push(ShipEvent::Bumped);
                Response::Bounce { restitution: 0.4, separation: 0.5, push: 2.0 }
            }
            _ => Response::Stop,
        }
    }
}
