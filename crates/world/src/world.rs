//! The world ships move through — the galaxy, its gate network, the clock and
//! the star systems generated so far — and the ship step, where commands meet
//! the devices, the physics kernel and the world's contact rules.
//!
//! The ship step reads the ship and its commands, and whatever the kernel
//! reports; nothing about where the ship is trying to go. A flight computer
//! can take part while the ship moves (`FlightComputer`), but only by giving
//! commands.

use std::collections::HashMap;
use std::rc::Rc;

use glam::{DQuat, DVec3};
use universe_physics::integrate::FINE_STEP;
use universe_physics::{integrate, Driver, Ephemeris, Fact, Feature, Response, RigidBody, Span, Weld};

use crate::damage;
use crate::events::ShipEvent;
use crate::galaxy::{Galaxy, GALAXY_STARS};
use crate::gate::{self, GateFrame};
use crate::hyperdrive;
use crate::names::star_name;
use crate::network;
use crate::ship::{upright, Controls, HyperdriveCommand, Ship, ShipCommands, ShipState, SHIP_RADIUS};
use crate::spaceport;
use crate::station::{self, DOCKED_HEIGHT, STATION_SIZE};
use crate::system::{BodyKind, StarSystem};
use crate::traffic::Facility;
use crate::units::LIGHT_YEAR;
use crate::goods::Item;
use crate::market::{Market, MarketState, Quote};
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

/// A ship's flight computer, as the world sees it: something that gives the
/// devices commands while the ship moves. (The pilot's own commands come
/// with each step; see `World::step_ship`.)
pub trait FlightComputer {
    /// Its control rate: the longest substep it can fly with (s), since it can
    /// only change its commands between substeps. It doesn't change the physics:
    /// a powered ship already takes fine substeps whoever flies it.
    fn interval(&self) -> f64 {
        f64::INFINITY
    }

    /// Commands for the substep of `h` seconds from `t` (the ship in `sys`,
    /// rail bodies at `positions`), or None to leave the devices as they are.
    fn substep(&mut self, _sys: &StarSystem, _ship: &Ship, _t: f64, _h: f64, _positions: &[DVec3]) -> Option<ShipCommands> {
        None
    }

    /// In hyperdrive, this frame's commands, the world having moved on to
    /// `t` (rail bodies at `positions`). By default: carry on as set.
    fn hyperdrive(&mut self, _sys: &StarSystem, ship: &Ship, _t: f64, _positions: &[DVec3]) -> ShipCommands {
        ShipCommands { hyperdrive: Some(HyperdriveCommand::CRUISE), ..ship.holding() }
    }
}

/// No flight computer: the pilot's commands stand.
pub struct Manual;

impl FlightComputer for Manual {}

pub struct World {
    pub galaxy: Galaxy,
    /// Seconds since the epoch.
    pub time: f64,
    pub home_system: usize,
    /// Gate links between star systems (galaxy indices).
    pub gate_links: Vec<(usize, usize)>,
    systems: HashMap<usize, Rc<StarSystem>>,
    /// Nearest stars of each system visited (see `NEIGHBOURS`).
    neighbours: HashMap<usize, Rc<[usize]>>,
    /// Body snapshots per system for the current moment, shared by every ship there.
    ephemerides: HashMap<usize, (f64, Rc<Ephemeris>)>,
    positions: Vec<DVec3>,
    /// Slugs in flight (see `weapons`).
    pub slugs: Vec<Slug>,
    /// Laser beams fired in the last combat phase.
    pub beams: Vec<Beam>,
    /// Hits in the last combat phase.
    pub impacts: Vec<Impact>,
    /// The goods traded in this galaxy (see `goods`).
    pub goods: Vec<Item>,
    /// Markets met so far, and the state of those traded with (see `market`).
    markets: HashMap<(usize, Facility), Rc<Market>>,
    market_states: HashMap<(usize, Facility), MarketState>,
    /// Traffic control: pads and corridors (see `pads`).
    pub traffic: crate::pads::TrafficControl,
    /// Defence turrets by system, met so far, and their guns' cooldowns (see `turrets`).
    pub(crate) turrets: HashMap<usize, Rc<Vec<crate::turrets::Turret>>>,
    pub(crate) turret_cooldowns: HashMap<usize, f64>,
}

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
            systems: HashMap::new(),
            neighbours: HashMap::new(),
            ephemerides: HashMap::new(),
            positions: Vec::new(),
            slugs: Vec::new(),
            beams: Vec::new(),
            impacts: Vec::new(),
            goods: crate::goods::catalog(seed),
            markets: HashMap::new(),
            market_states: HashMap::new(),
            traffic: Default::default(),
            turrets: HashMap::new(),
            turret_cooldowns: HashMap::new(),
        }
    }

    /// The market at facility `f` in `system` (None for gates).
    pub fn market(&mut self, system: usize, f: Facility) -> Option<Rc<Market>> {
        if let Some(m) = self.markets.get(&(system, f)) {
            return Some(m.clone());
        }
        let sys = self.system(system);
        let m = Rc::new(crate::market::market(self.galaxy.seed, system, &sys, f, &self.goods)?);
        self.markets.insert((system, f), m.clone());
        Some(m)
    }

    /// The market's quotes now.
    pub fn quotes(&mut self, system: usize, f: Facility) -> Vec<Quote> {
        let Some(m) = self.market(system, f) else { return Vec::new() };
        let now = self.time;
        let mut state = self.market_states.get(&(system, f)).cloned().unwrap_or(MarketState { updated: now, ..Default::default() });
        m.quotes(&mut state, now)
    }

    /// The market's quote for one item, if it trades it (see `Market::quote_for`).
    pub fn quote_for(&mut self, system: usize, f: Facility, item: usize) -> Option<Quote> {
        let m = self.market(system, f)?;
        let now = self.time;
        let mut state = self.market_states.get(&(system, f)).cloned().unwrap_or(MarketState { updated: now, ..Default::default() });
        m.quote_for(&mut state, now, &self.goods[item])
    }

    /// The market's quotes for several items at once (None: not traded there).
    pub fn quotes_for(&mut self, system: usize, f: Facility, items: &[usize]) -> Vec<Option<Quote>> {
        let Some(m) = self.market(system, f) else { return vec![None; items.len()] };
        let now = self.time;
        let mut state = self.market_states.get(&(system, f)).cloned().unwrap_or(MarketState { updated: now, ..Default::default() });
        items.iter().map(|&i| m.quote_for(&mut state, now, &self.goods[i])).collect()
    }

    /// Trade at the market at `f` in `system` (see `Market::trade`): only
    /// for a ship docked or landed there.
    pub fn trade(&mut self, system: usize, f: Facility, item: usize, units: i64, ship: &mut Ship, credits: &mut f64) -> Result<f64, String> {
        let sys = self.system(system);
        if crate::market::docked_at(&sys, ship) != Some(f) {
            return Err("DOCK OR LAND HERE TO TRADE".into());
        }
        let m = self.market(system, f).ok_or("NO MARKET")?;
        let now = self.time;
        let state = self.market_states.entry((system, f)).or_insert_with(|| MarketState { updated: now, ..Default::default() });
        let goods = &self.goods;
        m.trade(state, now, &goods[item], units, ship, credits)
    }

    /// Systems linked to `i` by gates, with their names.
    pub fn gate_links_of(&self, i: usize) -> Vec<(usize, String)> {
        network::links_of(&self.gate_links, &self.galaxy, i)
    }

    /// Get (generating and caching if needed) the star system at galaxy index `i`.
    /// A star system already generated, if it is (no generating from `&self`).
    pub fn system_if_known(&self, i: usize) -> Option<Rc<StarSystem>> {
        self.systems.get(&i).cloned()
    }

    pub fn system(&mut self, i: usize) -> Rc<StarSystem> {
        if self.systems.len() > 64 {
            // Keep the gate network's systems, where the traffic is.
            self.systems.retain(|&k, _| self.gate_links.iter().any(|&(a, b)| a == k || b == k));
        }
        if let Some(sys) = self.systems.get(&i) {
            return sys.clone();
        }
        let star = &self.galaxy.stars[i];
        let mut sys = StarSystem::generate(i, star);
        let links = self.gate_links_of(i);
        if !links.is_empty() {
            sys.add_gates(&links, star.seed);
        }
        let sys = Rc::new(sys);
        self.systems.insert(i, sys.clone());
        sys
    }

    /// The stars nearest to star `i`, nearest first (see `NEIGHBOURS`).
    pub fn neighbours(&mut self, i: usize) -> Rc<[usize]> {
        self.neighbours.entry(i).or_insert_with(|| self.galaxy.nearest(i, NEIGHBOURS).into()).clone()
    }

    /// Distance in light years between two stars.
    pub fn distance_ly(&self, a: usize, b: usize) -> f64 {
        self.galaxy.offset(a, b).length() / LIGHT_YEAR
    }

    /// The body snapshot for `sys` at time `t`, computed once and shared by
    /// every ship stepping from that moment.
    fn ephemeris(&mut self, sys: &StarSystem, t: f64) -> Rc<Ephemeris> {
        match self.ephemerides.get(&sys.index) {
            Some((at, e)) if *at == t => e.clone(),
            _ => {
                let e = Rc::new(sys.ephemeris(t));
                self.ephemerides.insert(sys.index, (t, e.clone()));
                e
            }
        }
    }

    fn advance_clock(&mut self, dt: f64) -> StepResult {
        self.time += dt;
        StepResult { simulated: dt, warp_limited: false }
    }

    /// Hand the ship's devices new commands, with no time passing: the engine
    /// and thrusters take their settings, then the hyperdrive engages or
    /// disengages if told to. (Turning takes time: see `step_ship`.)
    pub fn command(&mut self, ship: &mut Ship, system: usize, c: &ShipCommands, events: &mut Vec<ShipEvent>) {
        ship.set_controls(c);
        if let Some(on) = c.arm {
            crate::weapons::master_arm(ship, on, events);
        }
        if let Some(h) = &c.hyperdrive
            && h.engage != ship.hyperdrive
        {
            let sys = self.system(system);
            sys.positions(self.time, &mut self.positions);
            hyperdrive::switch(&sys, ship, h, self.time, &self.positions, events);
        }
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
        computer: &mut impl FlightComputer,
        real_dt: f64,
        warp: f64,
        events: &mut Vec<ShipEvent>,
    ) -> StepResult {
        ship.hyper_jam = (ship.hyper_jam - real_dt * warp).max(0.0);
        self.command(ship, *system, commands, events);
        let sys = self.system(*system);
        // Nearby stars, when already looked up for this system.
        let mut near = None;
        let result = match ship.state.clone() {
            ShipState::Destroyed { respawn_in } => {
                let left = respawn_in - real_dt;
                ship.state = ShipState::Destroyed { respawn_in: left };
                if left <= 0.0 {
                    self.respawn(ship, system, events);
                }
                self.advance_clock(real_dt * warp)
            }
            ShipState::Landed { body, local_position, local_orientation } => {
                let weld = Weld { body, local_position, local_orientation };
                self.landed_step(&sys, ship, weld, commands.turn, real_dt, warp, events)
            }
            ShipState::Transit { to, from, remaining, local_velocity, local_offset, local_orientation } => {
                // The transit takes a few real seconds; the world clock keeps its pace.
                let result = self.advance_clock(real_dt * warp);
                let left = remaining - real_dt;
                if left > 0.0 {
                    ship.state = ShipState::Transit { to, from, remaining: left, local_velocity, local_offset, local_orientation };
                } else {
                    self.arrive_through_gate(ship, system, to, from, local_velocity, local_offset, local_orientation, events);
                }
                result
            }
            ShipState::Flying if ship.hyperdrive => {
                if let Some(turn) = &commands.turn {
                    ship.steer(turn, real_dt);
                }
                let neighbours = self.neighbours(*system);
                let result = self.hyperdrive_step(&sys, ship, *system, &neighbours, computer, real_dt, warp, events);
                near = Some(neighbours);
                result
            }
            ShipState::Flying => {
                if let Some(turn) = &commands.turn {
                    ship.steer(turn, real_dt);
                }
                self.flight_step(&sys, ship, *system, computer, real_dt * warp, events)
            }
        };
        if ship.is_flying() {
            let neighbours = near.unwrap_or_else(|| self.neighbours(*system));
            self.handover(ship, system, &neighbours, events);
        }
        result
    }

    /// Welded to the body: carried round with its orbit and spin, turning in
    /// place if commanded; the docking port launches, the landing gear lifts off.
    #[allow(clippy::too_many_arguments)]
    fn landed_step(
        &mut self,
        sys: &StarSystem,
        ship: &mut Ship,
        weld: Weld,
        turn: Option<Controls>,
        real_dt: f64,
        warp: f64,
        events: &mut Vec<ShipEvent>,
    ) -> StepResult {
        let result = self.advance_clock(real_dt * warp);
        let b = &sys.bodies[weld.body];
        let rot = b.rotation(self.time);
        sys.positions(self.time, &mut self.positions);

        let mut rigid = ship.rigid();
        weld.place(&sys.bodies, self.time, &self.positions, &mut rigid);
        ship.set_rigid(&rigid);
        // Allow turning in place on the pad.
        if let Some(turn) = &turn {
            ship.steer(turn, real_dt);
        }
        let local_orientation = rot.inverse() * ship.orientation;
        ship.state = ShipState::Landed { body: weld.body, local_position: weld.local_position, local_orientation };
        let offset = rot * weld.local_position;

        if b.kind == BodyKind::Station {
            station::launch(sys, ship, weld.body, self.time, &self.positions, events);
            return result;
        }
        spaceport::lift_off(ship, offset.normalize(), events);
        result
    }

    /// Under hyperdrive: the flight computer (if any) has its say once the
    /// clock has moved on, then the drive carries the ship.
    #[allow(clippy::too_many_arguments)]
    fn hyperdrive_step(
        &mut self,
        sys: &StarSystem,
        ship: &mut Ship,
        system: usize,
        neighbours: &[usize],
        computer: &mut impl FlightComputer,
        real_dt: f64,
        warp: f64,
        events: &mut Vec<ShipEvent>,
    ) -> StepResult {
        let result = self.advance_clock(real_dt * warp);
        sys.positions(self.time, &mut self.positions);
        let c = computer.hyperdrive(sys, ship, self.time, &self.positions);
        ship.set_controls(&c);
        if let Some(turn) = &c.turn {
            ship.steer(turn, real_dt);
        }
        let orders = c.hyperdrive.unwrap_or(HyperdriveCommand::CRUISE);
        hyperdrive::cruise(&self.galaxy, neighbours, system, sys, ship, &orders, self.time, &self.positions, real_dt, warp, events);
        result
    }

    /// Free flight: the physics kernel moves the ship under gravity and the
    /// thrust of its devices, then the world's rules judge whatever it touched.
    fn flight_step(&mut self, sys: &StarSystem, ship: &mut Ship, system: usize, computer: &mut impl FlightComputer, dt: f64, events: &mut Vec<ShipEvent>) -> StepResult {
        // For short frames, snapshot the bodies once and extrapolate for each
        // substep instead of re-solving every orbit (see `Ephemeris`).
        let ephemeris = (dt <= Ephemeris::SPAN).then(|| self.ephemeris(sys, self.time));
        // Small steps while any device pushes (engine or thrusters), whoever
        // is flying: a powered ship is integrated alike under a pilot or a
        // computer. The kernel also takes small steps near any station or gate,
        // for contact. Separately, a computer's commands can only change between
        // substeps, so its control rate also bounds them, just as the pilot's
        // commands change once per step.
        let powered = ship.throttle > 0.0 || ship.rcs != DVec3::ZERO;
        let physics = if powered { FINE_STEP } else { f64::INFINITY };
        let span = Span { t: self.time, dt, max_h: physics.min(computer.interval()), contact_step: FINE_STEP };
        let mut rigid = ship.rigid();
        let mut devices = Devices::new(sys, &mut *ship, computer, &mut *events);
        let out = integrate(&sys.bodies, ephemeris.as_deref(), &mut self.positions, &mut rigid, span, &mut devices);
        ship.set_rigid(&rigid);
        self.time = out.time;
        if let Some(fact) = out.fact {
            self.react(sys, ship, system, fact, events);
        }
        StepResult { simulated: out.simulated, warp_limited: out.limited }
    }

    /// A flight step stopped on a physical fact: what it means for the ship.
    fn react(&mut self, sys: &StarSystem, ship: &mut Ship, system: usize, fact: Fact, events: &mut Vec<ShipEvent>) {
        match fact {
            Fact::Trigger { body, .. } => self.enter_gate(sys, ship, system, body, events),
            Fact::Contact(c) => match c.feature {
                Feature::Surface { .. } => spaceport::touch_down(sys, ship, &c, self.time, events),
                // The station: its docking slot, or (too fast for a bump) its hull.
                Feature::Hull | Feature::CutOut(_) => station::contact(sys, ship, &c, self.time, &self.positions, events),
                Feature::Ring => damage::destroy(ship, &sys.bodies[c.body].name, events),
            },
        }
    }

    /// Through a gate's opening: the gate device starts the transit to the
    /// linked system, unless the ship is going too fast for it.
    fn enter_gate(&mut self, sys: &StarSystem, ship: &mut Ship, system: usize, gate: usize, events: &mut Vec<ShipEvent>) {
        let b = &sys.bodies[gate];
        let frame = GateFrame::new(sys, gate, self.time, &self.positions);
        let to = b.link.unwrap_or(system);
        match gate::enter(&frame, ship, to, system) {
            Err(speed) => {
                events.push(ShipEvent::GateTooFast { speed });
                damage::destroy(ship, &b.name, events);
            }
            Ok(transit) => {
                ship.state = transit;
                ship.rcs = DVec3::ZERO;
                ship.throttle = 0.0;
                events.push(ShipEvent::GateEntered { to: star_name(self.galaxy.stars[to].seed) });
            }
        }
    }

    /// Come out of the gate in system `to` that leads back to `from`, with the
    /// same motion relative to it as we had going into the other one.
    #[allow(clippy::too_many_arguments)]
    fn arrive_through_gate(
        &mut self,
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
        sys.positions(self.time, &mut self.positions);
        let Some(g) = sys.gate_to(from) else {
            // No return gate (shouldn't happen): a new ship at home instead.
            self.respawn(ship, system, events);
            return;
        };
        let frame = GateFrame::new(&sys, g, self.time, &self.positions);
        let mut rigid = ship.rigid();
        gate::emerge(&frame, local_velocity, local_offset, local_orientation, &mut rigid);
        ship.set_rigid(&rigid);
        ship.state = ShipState::Flying;
        events.push(ShipEvent::GateArrived { system: sys.name.clone() });
    }

    /// When a flying ship is closer to one of its system's `neighbours` than
    /// to its own star, move it into that star's frame (a floating origin at
    /// interstellar scale).
    fn handover(&mut self, ship: &mut Ship, system: &mut usize, neighbours: &[usize], events: &mut Vec<ShipEvent>) {
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
        *system = self.home_system;
        let sys = self.system(*system);
        let station = sys.station().unwrap_or(0);
        sys.positions(self.time, &mut self.positions);
        let pos = self.positions[station];
        let vel = sys.velocity(station, self.time);
        let parent = sys.bodies[station].rail.parent.unwrap_or(0);
        let rel_vel = vel - sys.velocity(parent, self.time);
        let prograde = rel_vel.normalize();
        let radial = (pos - self.positions[parent]).normalize();

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
        sys.positions(self.time, &mut self.positions);
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
        Weld { body, local_position, local_orientation }.place(&sys.bodies, self.time, &self.positions, &mut rigid);
        ship.set_rigid(&rigid);
        ship.state = ShipState::Landed { body, local_position, local_orientation };
        ship
    }
}

/// The ship's devices during a flight step, as the kernel's force callback:
/// the flight computer's commands (if any) set them each substep, and they
/// push with the thrust they're set to. Contacts are judged by world rules.
/// (The same devices fly a copy of the ship when a flight is simulated
/// ahead, e.g. by a flight planner through `universe_physics::simulate`.)
pub struct Devices<'a, C> {
    sys: &'a StarSystem,
    ship: &'a mut Ship,
    computer: &'a mut C,
    events: &'a mut Vec<ShipEvent>,
}

impl<'a, C: FlightComputer> Devices<'a, C> {
    /// `ship`'s devices in `sys`, set each substep by `computer`; what they
    /// do goes to `events`. The kernel's body stands for the ship's motion
    /// (see `Ship::rigid`/`set_rigid`); the ship keeps the device settings.
    pub fn new(sys: &'a StarSystem, ship: &'a mut Ship, computer: &'a mut C, events: &'a mut Vec<ShipEvent>) -> Self {
        Self { sys, ship, computer, events }
    }
}

impl<C: FlightComputer> Driver for Devices<'_, C> {
    fn applied(&mut self, body: &mut RigidBody, t: f64, h: f64, positions: &[DVec3]) -> DVec3 {
        let ship = &mut *self.ship;
        ship.position = body.position;
        ship.velocity = body.velocity;
        if let Some(c) = self.computer.substep(self.sys, ship, t, h, positions) {
            ship.set_controls(&c);
            if let Some(turn) = &c.turn {
                ship.steer(turn, h);
            }
            body.orientation = ship.orientation;
            body.angular_velocity = ship.angular_velocity;
        }
        ship.thrust()
    }

    fn respond(&mut self, fact: &Fact, _: &RigidBody) -> Response {
        match fact {
            // A gentle scrape on a station's hull: bounce off and keep flying.
            Fact::Contact(c) if station::bounces(c) => {
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
    use crate::ship::{DRY_MASS, FUEL_CAPACITY};
    use crate::testkit::Probe;
    use crate::units::AU;

    #[test]
    fn home_system_has_station_and_planets() {
        let mut w = World::new(42);
        let sys = w.system(w.home_system);
        assert!(sys.station().is_some());
        assert!(sys.planet_count() >= 4);
        for (i, b) in sys.bodies.iter().enumerate() {
            if let Some(p) = b.rail.parent {
                assert!(p < i, "parent must precede child");
            }
        }
    }

    #[test]
    fn ship_keeps_orbit_under_warp() {
        let mut p = Probe::new(42);
        let sys = p.sys();
        let station = sys.station().unwrap();
        let planet = sys.bodies[station].rail.parent.unwrap();
        let mut pos = Vec::new();
        sys.positions(p.world.time, &mut pos);
        let r0 = p.ship.position.distance(pos[planet]);
        // Two simulated hours at 1000x.
        for _ in 0..(7200 / 16) {
            p.step(0.016, 1000.0);
        }
        sys.positions(p.world.time, &mut pos);
        let r1 = p.ship.position.distance(pos[planet]);
        assert!(p.ship.is_flying());
        assert!((r1 - r0).abs() / r0 < 0.01, "orbit radius drifted: {r0} -> {r1}");
    }

    #[test]
    fn heavier_ship_accelerates_less() {
        let mut p = Probe::new(42);
        let full = p.ship.main_accel();
        assert!((full - 30.0).abs() < 1e-9, "a fully fuelled, empty ship keeps the old 30 m/s^2");
        p.ship.cargo = DRY_MASS + FUEL_CAPACITY; // double the mass
        assert!((p.ship.main_accel() - 15.0).abs() < 1e-9);
        // And it shows in flight: burn for 10 s far from anything.
        let burn = |cargo: f64| {
            let mut p = Probe::new(42);
            p.ship.position = DVec3::new(0.0, 5.0 * AU, 0.0);
            p.ship.velocity = DVec3::ZERO;
            p.ship.cargo = cargo;
            p.set_throttle(1.0);
            let v0 = p.ship.velocity;
            for _ in 0..600 {
                p.step(1.0 / 60.0, 1.0);
            }
            (p.ship.velocity - v0).dot(p.ship.forward())
        };
        let (light, heavy) = (burn(0.0), burn(90_000.0));
        eprintln!("10 s full burn: empty {light:.1} m/s, loaded {heavy:.1} m/s");
        assert!((light / heavy - 2.0).abs() < 0.02);
    }

    /// A flight computer burning at full throttle every substep. Like the
    /// real one, it declares the control rate it commands at.
    struct Burn;

    impl FlightComputer for Burn {
        fn interval(&self) -> f64 {
            FINE_STEP
        }

        fn substep(&mut self, _: &StarSystem, ship: &Ship, _: f64, _: f64, _: &[DVec3]) -> Option<ShipCommands> {
            Some(ShipCommands { throttle: 1.0, ..ship.holding() })
        }
    }

    #[test]
    fn a_flight_computer_flies_only_through_commands() {
        // The same commands from a computer (every substep) or from the pilot
        // (once, beforehand) fly the ship exactly the same.
        let run = |by_computer: bool| {
            let mut p = Probe::new(42);
            p.ship.position = DVec3::new(0.0, 5.0 * AU, 0.0);
            p.ship.velocity = DVec3::ZERO;
            if !by_computer {
                p.set_throttle(1.0);
            }
            for _ in 0..600 {
                let c = ShipCommands { turn: Some(Controls::default()), ..p.ship.holding() };
                if by_computer {
                    p.world.step_ship(&mut p.ship, &mut p.system, &c, &mut Burn, 1.0 / 60.0, 5.0, &mut p.events);
                } else {
                    p.world.step_ship(&mut p.ship, &mut p.system, &c, &mut Manual, 1.0 / 60.0, 5.0, &mut p.events);
                }
            }
            (p.ship.position, p.ship.velocity, p.ship.throttle, p.world.time)
        };
        let (computer, pilot) = (run(true), run(false));
        assert_eq!(computer, pilot);
        assert_eq!(computer.2, 1.0, "the engine keeps the computer's last setting");
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
