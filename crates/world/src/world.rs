//! The world ships move through — the galaxy, its gate network, the clock and
//! the star systems generated so far — and the ship step, where commands meet
//! the devices, the physics kernel and the world's contact rules.
//!
//! The ship step reads the ship and its commands, and whatever the kernel
//! reports; nothing about where the ship is trying to go. Whatever flies it
//! (a pilot, a program) has had its say before the step, as device settings
//! that hold through it: the physics integrates, nothing else runs inside.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use glam::{DQuat, DVec3};
use universe_physics::integrate::FINE_STEP;
use universe_physics::{integrate, Driver, Ephemeris, Fact, Response, RigidBody, Span, Weld};

use crate::events::ShipEvent;
use crate::galaxy::{Galaxy, GALAXY_STARS};
use crate::gate::{self, GateFrame};
use crate::hyperdrive;
use crate::network;
use crate::ship::{upright, Controls, HyperdriveCommand, Ship, ShipCommands, ShipState, SHIP_RADIUS};
use crate::station::{DOCKED_HEIGHT, STATION_SIZE};
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
    /// Body positions per system at the last two moments asked for: see `rails_at`.
    rails: Mutex<HashMap<usize, [RailsAt; 2]>>,
    /// Each system's contact rules, as their owners registered them (see `rules`).
    rules: Mutex<HashMap<usize, Arc<crate::rules::Rules>>>,
    /// While ships step side by side: what they'll look up, gathered first
    /// and read without locks (see `freeze`).
    frozen: Option<Frozen>,
    /// Slugs in flight (see `weapons`).
    pub slugs: Vec<Slug>,
    /// Laser beams fired in the last combat phase.
    pub beams: Vec<Beam>,
    /// Hits in the last combat phase.
    pub impacts: Vec<Impact>,
    /// The goods traded in this galaxy (see `goods`).
    pub goods: Vec<Item>,
    /// Defence turrets by system, met so far, and their guns' cooldowns (see `turrets`).
    pub(crate) turrets: Mutex<HashMap<usize, Arc<Vec<crate::turrets::Turret>>>>,
    pub(crate) turret_guns: HashMap<usize, crate::turrets::TurretGun>,
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

/// A body snapshot and its moment.
type EphemerisAt = (f64, Arc<Ephemeris>);

/// Body positions at a moment (see `World::rails_now`).
type RailsAt = (f64, Arc<Vec<DVec3>>);

impl World {
    pub fn new(seed: u64) -> Self {
        let galaxy = Galaxy::generate(seed, GALAXY_STARS);
        let home_system = network::find_home(&galaxy, seed);
        let gate_links = network::build(&galaxy, home_system);
        Self {
            galaxy,
            time: 0.0,
            home_system,
            gate_links,
            systems: Default::default(),
            neighbours: Default::default(),
            ephemerides: Default::default(),
            field_ephemerides: Default::default(),
            rails: Default::default(),
            rules: Default::default(),
            frozen: None,
            slugs: Vec::new(),
            beams: Vec::new(),
            impacts: Vec::new(),
            goods: crate::goods::catalog(seed),
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
        let mut systems = lock(&self.systems);
        if systems.len() > 64 {
            // Keep the gate network's systems, where the traffic is.
            systems.retain(|&k, _| self.gate_links.iter().any(|&(a, b)| a == k || b == k));
        }
        if let Some(sys) = systems.get(&i) {
            return sys.clone();
        }
        let sys = Arc::new(crate::charts::generate(&self.galaxy, &self.gate_links, i));
        systems.insert(i, sys.clone());
        sys
    }

    /// The charts of this galaxy, to share (with the client).
    pub fn charts(&self) -> crate::charts::Charts {
        crate::charts::Charts::new(self.galaxy.seed, self.galaxy.clone(), self.gate_links.clone(), self.goods.clone(), self.home_system)
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
        let mut cache = lock(&self.ephemerides);
        match cache.get(&sys.index) {
            Some((at, e)) if *at == t => e.clone(),
            _ => {
                let e = Arc::new(sys.ephemeris(t));
                cache.insert(sys.index, (t, e.clone()));
                e
            }
        }
    }

    /// `ephemeris` for field `f`'s bodies (see `StarSystem::field_bodies`).
    fn field_ephemeris(&self, sys: &StarSystem, f: usize, t: f64) -> Arc<Ephemeris> {
        let mut cache = lock(&self.field_ephemerides);
        match cache.get(&(sys.index, f)) {
            Some((at, e)) if *at == t => e.clone(),
            _ => {
                let e = Arc::new(Ephemeris::new(&sys.field_bodies(f)[..], t));
                cache.insert((sys.index, f), (t, e.clone()));
                e
            }
        }
    }

    /// The asteroid field ship `p` is among or near in `system` at `t`, if any.
    pub fn field_at(&self, sys: &StarSystem, system: usize, p: DVec3, t: f64) -> Option<usize> {
        if sys.fields.is_empty() {
            return None;
        }
        sys.field_near(p, &self.rails_at(system, t))
    }

    /// Hand the ship's devices new commands, with no time passing: the engine
    /// and thrusters take their settings, then the hyperdrive engages or
    /// disengages if told to. (Turning takes time: see `step_ship`.)
    pub fn command(&mut self, ship: &mut Ship, system: usize, c: &ShipCommands, events: &mut Vec<ShipEvent>) {
        self.command_at(ship, system, c, self.time, events);
    }

    /// `command` at time `t`.
    pub fn command_at(&self, ship: &mut Ship, system: usize, c: &ShipCommands, t: f64, events: &mut Vec<ShipEvent>) {
        ship.set_controls(c);
        if let Some(on) = c.arm {
            crate::weapons::master_arm(ship, on, events);
        }
        match c.anchor {
            Some(true) if !matches!(ship.state, ShipState::Anchored { .. }) => {
                let sys = self.system(system);
                let field = self.field_at(&sys, system, ship.position, t);
                crate::mining::anchor(&sys, field, ship, t, events);
            }
            Some(false) => crate::mining::release(ship, events),
            _ => {}
        }
        match &c.hyperdrive {
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
            ShipState::Anchored { .. } => {
                let result = advance(clock, real_dt * warp);
                crate::mining::hold(&sys, ship, *clock);
                result
            }
            ShipState::Transit { to, from, remaining, local_velocity, local_offset, local_orientation } => {
                // The transit takes a few real seconds; the world clock keeps its pace.
                let result = advance(clock, real_dt * warp);
                let left = remaining - real_dt;
                if left > 0.0 {
                    ship.state = ShipState::Transit { to, from, remaining: left, local_velocity, local_offset, local_orientation };
                } else {
                    self.arrive_through_gate(*clock, ship, system, to, from, local_velocity, local_offset, local_orientation, events);
                }
                result
            }
            ShipState::Flying if ship.hyperdrive => {
                if let Some(turn) = &commands.turn {
                    ship.steer(turn, real_dt);
                }
                let neighbours = self.neighbours(*system);
                let result = universe_prof::time("sim/crafts/tick/world step/hyperdrive", || self.hyperdrive_step(clock, &sys, ship, *system, &neighbours, real_dt, warp, events));
                near = Some(neighbours);
                result
            }
            ShipState::Flying => {
                // Turning is physics: over the game time the step covers.
                if let Some(turn) = &commands.turn {
                    ship.steer(turn, real_dt * warp);
                }
                universe_prof::time("sim/crafts/tick/world step/flight (physics)", || self.flight_step(clock, &sys, ship, *system, real_dt * warp, events))
            }
        };
        // The skin in air (and cooling after).
        if ship.is_flying() && !ship.hyperdrive {
            let _air = universe_prof::scope("sim/crafts/tick/world step/air lookup");
            let air = if sys.bodies.iter().any(|b| b.rail.atmosphere.is_some()) {
                let positions = self.rails_at(*system, *clock);
                universe_physics::air_at(&sys.bodies, ship.position, &positions, *clock)
            } else {
                None
            };
            universe_prof::time("sim/crafts/tick/world step/heat", || crate::heat::heat(ship, air, real_dt * warp, events));
        } else if ship.hyperdrive {
            crate::heat::heat(ship, None, real_dt * warp, events);
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

        let mut rigid = ship.rigid();
        weld.place(&sys.bodies, t, &positions, &mut rigid);
        ship.set_rigid(&rigid);
        // Allow turning in place on the pad.
        if let Some(turn) = &turn {
            ship.steer(turn, real_dt);
        }
        let local_orientation = rot.inverse() * ship.orientation;
        ship.state = ShipState::Landed { body: weld.body, local_position: weld.local_position, local_orientation };
        // Let go, if the structure's release says so (its owner's rule).
        let rules = self.rules_of(system);
        crate::rules::release(&rules, sys, weld.body, weld.local_position, ship, t, &positions, events);
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

    /// Free flight: the physics kernel moves the ship under gravity and the
    /// thrust of its devices, then the world's rules judge whatever it touched.
    #[allow(clippy::too_many_arguments)]
    fn flight_step(&self, clock: &mut f64, sys: &StarSystem, ship: &mut Ship, system: usize, dt: f64, events: &mut Vec<ShipEvent>) -> StepResult {
        // Among an asteroid field, its swarm's bodies too.
        let field = self.field_at(sys, system, ship.position, *clock);
        let local = field.map(|f| sys.field_bodies(f));
        let bodies = local.as_deref().map_or(&sys.bodies[..], |b| &b[..]);
        // For short frames, snapshot the bodies once and extrapolate for each
        // substep instead of re-solving every orbit (see `Ephemeris`).
        let ephemeris = (dt <= Ephemeris::SPAN).then(|| match field {
            Some(f) => self.field_ephemeris(sys, f, *clock),
            None => self.ephemeris(sys, *clock),
        });
        // Small steps while any device pushes (engine or thrusters), whoever
        // is flying. The kernel also takes small steps near any station or
        // gate, for contact.
        let powered = ship.throttle > 0.0 || ship.rcs != DVec3::ZERO;
        let physics = if powered { FINE_STEP } else { f64::INFINITY };
        let span = Span { t: *clock, dt, max_h: physics, contact_step: FINE_STEP };
        let mut rigid = ship.rigid();
        let rules = self.rules_of(system);
        let mut devices = Devices::new(&mut *ship, &rules, &mut *events).among(bodies);
        let mut positions = Vec::with_capacity(bodies.len());
        let out = integrate(bodies, ephemeris.as_deref(), &mut positions, &mut rigid, span, &mut devices);
        ship.set_rigid(&rigid);
        *clock = out.time;
        let rock = |f: &Fact| matches!(f, Fact::Contact(c) if bodies[c.body].kind == crate::system::BodyKind::Asteroid);
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

        // 4 km behind the station on the same orbit, nose pointing at it.
        let ship_pos = pos - prograde * 4000.0;
        let orientation = upright(radial, prograde);
        *ship = Ship::new(ship_pos, vel, orientation);
        events.push(ShipEvent::Respawned);
    }

    /// A new ship docked at a station or landed at a spaceport in `system`.
    pub fn ship_at(&mut self, system: usize, at: Facility) -> Ship {
        self.ship_on(system, at, crate::spaceport::CENTER_PAD)
    }

    /// A ship resting at `at` in `system`: docked in a station's slot, or on
    /// pad `pad` of a spaceport.
    pub fn ship_on(&mut self, system: usize, at: Facility, pad: usize) -> Ship {
        let sys = self.system(system);
        let positions = self.rails_now(system);
        let mut ship = Ship::new(DVec3::ZERO, DVec3::ZERO, DQuat::IDENTITY);
        let (body, local_position, local_orientation) = match at {
            Facility::Station(s) => {
                // In the slot, nose in (the station's local frame: axis +Y, slot along X).
                let docked = DQuat::from_mat3(&glam::DMat3::from_cols(DVec3::X, DVec3::NEG_Z, DVec3::Y));
                (s, DVec3::Y * STATION_SIZE * DOCKED_HEIGHT, docked)
            }
            Facility::Spaceport(p) | Facility::Gate(p) => {
                let p = p.min(sys.spaceports.len().saturating_sub(1));
                let sp = &sys.spaceports[p];
                let b = &sys.bodies[sp.body];
                let d = crate::spaceport::pad_direction(&sys, p, pad.min(crate::spaceport::PADS - 1));
                (sp.body, d * (b.surface_radius(d) + SHIP_RADIUS), upright(d, d.any_orthonormal_vector()))
            }
        };
        let mut rigid = ship.rigid();
        Weld { body, local_position, local_orientation }.place(&sys.bodies, self.time, &positions, &mut rigid);
        ship.set_rigid(&rigid);
        ship.state = ShipState::Landed { body, local_position, local_orientation };
        ship
    }
}

/// The ship's devices during a flight step, as the kernel's force callback:
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
}

impl<'a> Devices<'a> {
    /// `ship`'s devices, among parts with `rules`; what they do goes to
    /// `events`. The kernel's body stands for the ship's motion (see
    /// `Ship::rigid`/`set_rigid`); the ship keeps the device settings.
    pub fn new(ship: &'a mut Ship, rules: &'a crate::rules::Rules, events: &'a mut Vec<ShipEvent>) -> Self {
        Self { ship, rules, events, bodies: &[] }
    }

    /// Flying among `bodies`: touching an asteroid among them is a
    /// collision (see `mining::strike`), whoever's rules.
    pub fn among(self, bodies: &'a [crate::system::Body]) -> Self {
        Self { bodies, ..self }
    }
}

impl Driver for Devices<'_> {
    fn applied(&mut self, body: &mut RigidBody, _t: f64, _h: f64, _positions: &[DVec3]) -> DVec3 {
        let ship = &mut *self.ship;
        ship.position = body.position;
        ship.velocity = body.velocity;
        ship.thrust()
    }

    fn respond(&mut self, fact: &Fact, _: &RigidBody) -> Response {
        match fact {
            Fact::Contact(c) if self.bodies.get(c.body).is_some_and(|b| b.kind == crate::system::BodyKind::Asteroid) => {
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

#[cfg(test)]
mod tests {
    use super::*;
    
    use crate::testkit::Probe;
    use crate::units::AU;

    #[test]
    fn device_settings_hold_through_the_step() {
        // Commanded once, or again before every tick: the ship flies exactly the same.
        let run = |every_tick: bool| {
            let mut p = Probe::new(42);
            p.ship.position = DVec3::new(0.0, 5.0 * AU, 0.0);
            p.ship.velocity = DVec3::ZERO;
            for k in 0..600 {
                let throttle = if every_tick || k == 0 { 1.0 } else { p.ship.throttle };
                let c = ShipCommands { throttle, turn: Some(Controls::default()), ..p.ship.holding() };
                p.world.step_ship(&mut p.ship, &mut p.system, &c, 1.0 / 60.0, 5.0, &mut p.events);
            }
            (p.ship.position, p.ship.velocity, p.ship.throttle, p.world.time)
        };
        let (each, once) = (run(true), run(false));
        assert_eq!(each, once);
        assert_eq!(once.2, 1.0, "the engine keeps its last setting");
    }

    #[test]
    fn landing_gear_lands_gently_and_breaks_on_a_hard_touchdown() {
        for (label, sink, lands) in [("gentle", 5.0, true), ("hard", 100.0, false)] {
            let mut p = Probe::new(42);
            let sys = p.sys();
            let planet = sys.bodies[sys.station().unwrap()].rail.parent.unwrap();
            let port = sys.spaceports.iter().position(|sp| sp.body == planet).expect("home planet has a spaceport");
            let pos = p.positions();
            let (b, t) = (&sys.bodies[planet], p.world.time);
            let up = b.rotation(t) * sys.spaceports[port].direction;
            // Just above the pad, moving with the ground and sinking.
            p.ship.position = pos[planet] + up * (b.rail.radius + SHIP_RADIUS + 3.0);
            p.ship.velocity = sys.velocity(planet, t) + b.angular_velocity().cross(p.ship.position - pos[planet]) - up * sink;
            p.ship.orientation = upright(up, up.any_orthonormal_vector());
            for _ in 0..120 {
                p.step(1.0 / 60.0, 1.0);
            }
            if lands {
                assert!(matches!(p.ship.state, ShipState::Landed { body, .. } if body == planet), "{label}: {:?}", p.events);
                assert!(p.events.iter().any(|e| matches!(e, ShipEvent::LandedAtPort { .. })), "{label}: on the pad: {:?}", p.events);
            } else {
                assert!(p.crashed() && matches!(p.ship.state, ShipState::Destroyed { .. }), "{label}: {:?}", p.events);
            }
        }
    }
}
