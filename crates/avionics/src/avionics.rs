//! A ship's avionics: what its navigation computer remembers — the nav
//! target, the clearance traffic control granted, the autopilots' state and
//! the route — and the programs that act on it once a frame. None of it is
//! physics or the world's business: the ship carries none of it, and the
//! world never reads it. The avionics see the ship through a `Bus`, fly it
//! only by sending `ShipCommands`, and learn what happened to it from the
//! world's physical events (`Avionics::observe`).

use glam::DVec3;
use serde::{Deserialize, Serialize};
use universe_world::{Controls, GateFrame, HyperdriveCommand, Ship, ShipCommands, ShipEvent, ShipState, StarSystem, StationFrame, TrafficEvent};

use crate::bus::Bus;
use crate::docking::{self, DockingStatus};
use crate::events::Event;
use crate::gate::{self, GateStatus};
use crate::hyperdrive;
use crate::landing::{self, LandingStatus, PadFrame};
use crate::nav::{Clearance, NavTarget, PadSlot, Phase};
use universe_protocol::PadGrant;
use crate::plan::{self, Plan};
use crate::route::{self, Route};

/// One ship's avionics state.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Avionics {
    /// Target locked on the navigation map.
    #[serde(default)]
    pub nav_target: Option<NavTarget>,
    /// Clearance to dock, land or transit (as granted by traffic control), and the autopilot's state.
    #[serde(default)]
    pub clearance: Option<Clearance>,
    /// Hyperdrive autopilot: steer to the nav target (only when the pilot turns it on).
    #[serde(default)]
    pub hyper_autopilot: bool,
    /// The multi-stop route, and the route autopilot's progress. (Saved on
    /// its own, next to the rest.)
    #[serde(skip)]
    pub route: Route,
    /// Where the hyperdrive autopilot is steering (for debugging).
    #[serde(skip)]
    pub debug_way: Option<DVec3>,
    /// The pirate's program is installed (see `hunter`), and the hunt under way.
    #[serde(default)]
    pub pirate: bool,
    #[serde(default)]
    pub hunting: Option<crate::hunter::Hunt>,
    /// A pirate rests until this world time after a hunt.
    #[serde(default)]
    pub rest_until: f64,
    /// Traffic control keeps us out of the corridor (station or gate) for
    /// now, and our place on the waiting ring meanwhile.
    #[serde(skip)]
    pub corridor_denied: bool,
    /// Ships ahead of us in line for the corridor, while waiting.
    #[serde(skip)]
    pub corridor_ahead: Option<usize>,
    #[serde(skip)]
    pub wait_place: usize,
    /// The collision warning is switched on (see `collision`).
    #[serde(default)]
    pub collision_warning: bool,
    /// The radar contact locked on (the id the radar reports it by).
    #[serde(skip)]
    pub contact: Option<usize>,
    /// The rock locked on (in mining): body `.1` among field `.0`'s bodies.
    #[serde(default)]
    pub rock_lock: Option<(usize, usize)>,
    /// Fire control's track on it.
    #[serde(skip)]
    pub track: Option<crate::fire_control::Track>,
    /// The follow program (keep at range, orbit), if engaged (see `follow`).
    #[serde(skip)]
    pub following: Option<crate::follow::Follow>,
}

/// Live guidance for the current clearance.
#[derive(Clone, Debug)]
pub enum Approach {
    Dock { station: usize, status: DockingStatus },
    Land { port: usize, status: Box<LandingStatus> },
    Transit { gate: usize, status: GateStatus },
}

impl Avionics {
    /// The dock/land/gate autopilot is flying.
    pub fn autopilot_engaged(&self) -> bool {
        self.clearance.is_some_and(|c| c.autopilot)
    }

    /// A computer, rather than the pilot's stick, turns the ship this frame.
    pub fn flies(&self, ship: &Ship) -> bool {
        match ship.state {
            ShipState::Flying if ship.hyperdrive => self.hyper_autopilot,
            ShipState::Flying => self.autopilot_engaged(),
            _ => false,
        }
    }


    /// What a physical event means for the navigation state: arriving,
    /// being destroyed or leaving through a gate ends a clearance; leaving
    /// the system forgets the target (it was in the old one); any change of
    /// the hyperdrive hands its steering back to the pilot; a new ship starts
    /// with nothing set.
    pub fn observe(&mut self, event: &ShipEvent) {
        match event {
            ShipEvent::Landed { .. } | ShipEvent::LandedAtPort { .. } | ShipEvent::Crashed { .. } | ShipEvent::Anchored { .. } => {
                self.clearance = None;
                self.following = None;
            }
            ShipEvent::GateEntered { .. } | ShipEvent::EnteredSystem { .. } => {
                self.clearance = None;
                self.following = None;
                self.nav_target = None;
                self.contact = None;
                self.rock_lock = None;
                self.track = None;
            }
            // (Switching the drive on hands the pilot the stick when it's
            // ordered: see `toggle_hyperdrive`. The news comes later.)
            ShipEvent::HyperdriveEngaged => self.following = None,
            ShipEvent::HyperdriveDisengaged => self.hyper_autopilot = false,
            ShipEvent::Respawned => {
                let mut route = std::mem::take(&mut self.route);
                // A hunt that ended in the wreck: back to the route.
                route.active |= self.hunting.is_some();
                *self = Avionics { route, pirate: self.pirate, collision_warning: self.collision_warning, ..Avionics::default() };
            }
            ShipEvent::TookOff
            | ShipEvent::Launched { .. }
            | ShipEvent::Bumped
            | ShipEvent::StruckRock { .. }
            | ShipEvent::AnchorFailed { .. }
            | ShipEvent::AnchorReleased
            | ShipEvent::Mined { .. }
            | ShipEvent::OutOfFuel
            | ShipEvent::ExcavatorStopped { .. }
            | ShipEvent::GateArrived { .. }
            | ShipEvent::GateTooFast { .. }
            | ShipEvent::Hit { .. }
            | ShipEvent::Collided { .. }
            | ShipEvent::Aggressed { .. }
            | ShipEvent::HyperdriveJammed { .. }
            | ShipEvent::RuleFired { .. }
            | ShipEvent::WeaponsArming
            | ShipEvent::WeaponsHot
            | ShipEvent::WeaponsSafe => {}
        }
    }

    /// Pass the world's events for the ship on to the pilot, taking note of them.
    pub fn record(&mut self, ship_events: Vec<ShipEvent>, events: &mut Vec<Event>) {
        for e in ship_events {
            self.observe(&e);
            events.push(Event::Ship(e));
        }
    }

    /// Set the ship's devices, and take note of what the feed reports.
    pub fn command(&mut self, bus: &mut impl Bus, c: &ShipCommands, events: &mut Vec<Event>) {
        bus.actuate(c);
        let happened = bus.feed();
        self.record(happened, events);
    }

    /// Change some of the engine and thruster settings, keeping the rest.
    pub(crate) fn set_controls(&mut self, bus: &mut impl Bus, events: &mut Vec<Event>, change: impl FnOnce(&mut ShipCommands)) {
        let mut c = bus.ship().holding();
        change(&mut c);
        self.command(bus, &c, events);
    }

    /// Engage or disengage the hyperdrive. Dropping out, the ship is left
    /// co-moving with the nav target if it's close, otherwise with whatever
    /// dominates gravity there.
    pub fn toggle_hyperdrive(&mut self, bus: &mut impl Bus, events: &mut Vec<Event>) {
        if !bus.ship().is_flying() {
            return;
        }
        let engage = !bus.ship().hyperdrive;
        let exit_velocity = if engage {
            None
        } else {
            let (sys, positions) = bus.positions();
            let (t, p) = (bus.time(), bus.ship().position);
            let aim = self.nav_target.and_then(|target| hyperdrive::aim(&sys, target, t, &positions, p));
            hyperdrive::exit_velocity(aim.as_ref(), p)
        };
        let orders = HyperdriveCommand { engage, start: engage, exit_velocity, ..Default::default() };
        let c = ShipCommands { hyperdrive: Some(orders), ..bus.ship().holding() };
        self.command(bus, &c, events);
        // (Both ways, the drive starts from zero throttle, and the pilot has the stick.)
        self.hyper_autopilot = false;
    }

    /// Before the ship moves this frame: the route autopilot, then (unless
    /// it is flying a route) the dock/land/gate autopilot's hyperjump.
    pub fn prepare(&mut self, bus: &mut impl Bus, dt: f64, events: &mut Vec<Event>) -> Option<Controls> {
        // Close to a station's or gate's corridor: ask traffic control whether it's ours.
        // Near it (approaching or lining up), ask; waiting ships keep their own place.
        self.wait_place = bus.id();
        self.corridor_ahead = match self.clearance {
            Some(Clearance { target: NavTarget::Station(b) | NavTarget::Gate(b), phase: Phase::Approach | Phase::Align | Phase::Final, .. })
                if target_position(bus, NavTarget::Gate(b)).or_else(|| target_position(bus, NavTarget::Station(b))).is_some_and(|p| p.distance(bus.ship().position) < 12_000.0) =>
            {
                bus.request_corridor(b)
            }
            _ => None,
        };
        self.corridor_denied = self.corridor_ahead.is_some();
        // A hunt flies the ship itself (see `hunter`); the route waits.
        if self.hunting.is_some() {
            return None;
        }
        self.route_step(bus, events);
        if !self.route.active {
            self.hyperjump(bus, events);
        }
        self.fly(bus, dt, events)
    }

    /// The autopilots' say for the next `dt` seconds (a tick), before the
    /// world steps the ship: in hyperdrive, its navigation (the drive's
    /// orders, and the stick if the hyperdrive autopilot steers); otherwise
    /// the dock/land/gate autopilot, if it's flying. They set the devices,
    /// which hold through the step; the stick (if they hold it) is returned.
    pub fn fly(&mut self, bus: &mut impl Bus, dt: f64, events: &mut Vec<Event>) -> Option<Controls> {
        let ship = bus.ship().clone();
        if !ship.is_flying() {
            return None;
        }
        let (sys, positions) = bus.positions();
        let t = bus.time();
        if ship.hyperdrive {
            let (commands, arrived) = hyperdrive::navigate(&sys, &ship, t, &positions, self.nav_target, self.hyper_autopilot, &mut self.debug_way);
            let turn = commands.turn;
            self.command(bus, &ShipCommands { turn: None, ..commands }, events);
            if let Some(target) = arrived {
                events.push(Event::HyperdriveArrived { target });
            }
            return turn.filter(|_| self.hyper_autopilot);
        }
        let c = self.clearance.filter(|c| c.autopilot)?;
        let wait = self.corridor_denied.then_some(self.wait_place);
        let cmd = crate::computer::autopilot(&crate::computer::AutopilotInput { sys: &sys, ship: &ship, target: c.target, phase: c.phase, pad: c.pad, wait, t, h: dt, positions: &positions });
        if cmd.phase != c.phase {
            self.clearance = Some(Clearance { phase: cmd.phase, ..c });
        }
        self.command(bus, &ShipCommands { turn: None, ..cmd.commands() }, events);
        Some(cmd.controls)
    }

    /// After the ship moved: let a clearance lapse.
    pub fn conclude(&mut self, bus: &mut impl Bus, events: &mut Vec<Event>) {
        self.check_clearance(bus, events);
    }

    /// Lock (or clear) the navigation target.
    pub fn set_nav_target(&mut self, bus: &mut impl Bus, target: Option<NavTarget>, events: &mut Vec<Event>) {
        if self.clearance.is_some_and(|c| Some(c.target) != target) {
            self.clearance = None;
            self.set_controls(bus, events, |c| c.rcs = DVec3::ZERO);
            events.push(Event::Traffic(TrafficEvent::ClearanceCancelled));
        }
        self.nav_target = target;
        let name = target.map(|t| t.name(&bus.star_system()));
        events.push(Event::NavTargetSet { name });
    }

    /// Ask traffic control for permission to dock/land at the locked nav
    /// target (or the nearest station).
    pub fn request_clearance(&mut self, bus: &mut impl Bus, events: &mut Vec<Event>) -> bool {
        let sys = bus.star_system();
        match bus.request_clearance(self.nav_target) {
            Ok(target) => {
                // Landing: a pad of our own, or a place in the holding ring.
                let pad = match target {
                    NavTarget::Spaceport(p) => match bus.request_pad(p) {
                        PadGrant::Pad(k) => PadSlot::Pad(k),
                        PadGrant::Queued(n) => PadSlot::Hold(n),
                    },
                    _ => PadSlot::Center,
                };
                let phase = if matches!(pad, PadSlot::Hold(_)) { Phase::Hold } else { Phase::Approach };
                self.clearance = Some(Clearance { target, autopilot: false, phase, pad });
                if let Some(kind) = target.kind() {
                    events.push(Event::Traffic(TrafficEvent::ClearanceGranted { target: target.name(&sys), kind }));
                }
                match pad {
                    PadSlot::Pad(k) => events.push(Event::Traffic(TrafficEvent::PadAssigned { pad: k })),
                    PadSlot::Hold(n) => events.push(Event::Traffic(TrafficEvent::Holding { ahead: n })),
                    PadSlot::Center => {}
                }
                true
            }
            Err(reason) => {
                events.push(Event::Traffic(TrafficEvent::ClearanceDenied { reason }));
                false
            }
        }
    }

    /// Give the clearance up (and stop its autopilot, leaving the engines idle).
    pub fn cancel_clearance(&mut self, bus: &mut impl Bus, events: &mut Vec<Event>) {
        let Some(c) = self.clearance.take() else { return };
        self.set_controls(bus, events, |cmd| {
            cmd.rcs = DVec3::ZERO;
            if c.autopilot {
                cmd.throttle = 0.0;
            }
        });
        events.push(Event::Traffic(TrafficEvent::ClearanceCancelled));
    }

    /// Engage or release the autopilot. In hyperdrive it steers to the nav
    /// target; otherwise it docks or lands (requesting clearance if needed).
    pub fn toggle_autopilot(&mut self, bus: &mut impl Bus, events: &mut Vec<Event>) {
        if bus.ship().hyperdrive {
            if self.nav_target.is_none() && !self.hyper_autopilot {
                events.push(Event::Refused { reason: "LOCK A NAV TARGET FIRST".into() });
                return;
            }
            self.hyper_autopilot = !self.hyper_autopilot;
            events.push(Event::Autopilot { on: self.hyper_autopilot });
            return;
        }
        if self.clearance.is_none() && !self.request_clearance(bus, events) {
            return;
        }
        let Some(c) = &mut self.clearance else { return };
        c.autopilot = !c.autopilot;
        if c.autopilot && self.following.take().is_some() {
            events.push(Event::Following { what: None });
        }
        let Some(c) = &mut self.clearance else { return };
        c.phase = if matches!(c.pad, PadSlot::Hold(_)) { Phase::Hold } else { Phase::Approach };
        let on = c.autopilot;
        if !on {
            self.set_controls(bus, events, |c| {
                c.rcs = DVec3::ZERO;
                c.throttle = 0.0;
            });
        }
        events.push(Event::Autopilot { on });
    }

    /// Start or stop the route autopilot.
    pub fn toggle_route(&mut self, bus: &mut impl Bus, events: &mut Vec<Event>) {
        if self.route.stops.is_empty() {
            return;
        }
        self.route.active = !self.route.active;
        if self.route.active && self.following.take().is_some() {
            events.push(Event::Following { what: None });
        }
        if !self.route.active {
            if let Some(c) = &mut self.clearance {
                c.autopilot = false;
            }
            self.hyper_autopilot = false;
            self.set_controls(bus, events, |c| {
                c.rcs = DVec3::ZERO;
                c.throttle = 0.0;
            });
        }
        events.push(Event::Autopilot { on: self.route.active });
    }

    /// Clearance lapses if the ship wanders far away or the target vanishes
    /// (traffic control's rule).
    fn check_clearance(&mut self, bus: &mut impl Bus, events: &mut Vec<Event>) {
        let Some(mut c) = self.clearance else { return };
        if !bus.ship().is_flying() {
            return;
        }
        // Holding for a pad: ask again; our turn gives us one.
        if let (NavTarget::Spaceport(p), PadSlot::Hold(was)) = (c.target, c.pad) {
            match bus.request_pad(p) {
                PadGrant::Pad(k) => {
                    c.pad = PadSlot::Pad(k);
                    c.phase = Phase::Approach;
                    events.push(Event::Traffic(TrafficEvent::PadAssigned { pad: k }));
                }
                PadGrant::Queued(n) if n != was => c.pad = PadSlot::Hold(n),
                PadGrant::Queued(_) => {}
            }
            self.clearance = Some(c);
        }
        if !bus.clearance_holds(c.target) {
            self.clearance = None;
            self.set_controls(bus, events, |c| c.rcs = DVec3::ZERO);
            events.push(Event::Traffic(TrafficEvent::ClearanceCancelled));
        }
    }

    /// With the dock/land/gate autopilot on and the target far away, cover
    /// the distance by hyperdrive first (its autopilot steers around planets),
    /// then carry on: the clearance and autopilot stay on through the jump.
    fn hyperjump(&mut self, bus: &mut impl Bus, events: &mut Vec<Event>) {
        let Some(c) = self.clearance.filter(|c| c.autopilot) else { return };
        if !bus.ship().is_flying() || bus.ship().hyperdrive || bus.ship().hyper_jam > 0.0 {
            return;
        }
        let Some(at) = target_position(bus, c.target) else { return };
        if at.distance(bus.ship().position) > route::hyperjump_limit(c.target) {
            self.nav_target = Some(c.target);
            self.toggle_hyperdrive(bus, events);
            self.hyper_autopilot = true;
            self.set_controls(bus, events, |c| c.throttle = 1.0);
        }
    }

    /// Guidance numbers for the HUD, if cleared to dock, land or transit (the
    /// ship in `sys` at `t`, bodies at `positions`).
    pub fn approach(&self, sys: &StarSystem, ship: &Ship, t: f64, positions: &[DVec3]) -> Option<Approach> {
        let c = self.clearance?;
        Some(match c.target {
            NavTarget::Station(station) => {
                let frame = StationFrame::new(sys, station, t, positions);
                Approach::Dock { station, status: docking::status(&frame, ship, &c) }
            }
            NavTarget::Spaceport(port) => {
                let pad = PadFrame::for_slot(sys, port, c.pad, t, positions);
                let terrain = sys.bodies[sys.spaceports[port].body].terrain.as_ref();
                Approach::Land { port, status: Box::new(landing::status(&pad, ship, &c, terrain)) }
            }
            NavTarget::Gate(g) => {
                let frame = GateFrame::new(sys, g, t, positions);
                Approach::Transit { gate: g, status: gate::status(&frame, ship, &c) }
            }
            // (No one clears a ship for an asteroid.)
            NavTarget::Asteroid(_) => return None,
        })
    }

    /// The flight plan to the cleared target: what to do from here, as the
    /// autopilot would do it (see `plan`). None unless flying, and not in
    /// hyperdrive.
    pub fn plan(&self, sys: &StarSystem, rules: &universe_world::rules::Rules, ship: &Ship, t: f64) -> Option<Plan> {
        let c = self.clearance?;
        if !ship.is_flying() || ship.hyperdrive {
            return None;
        }
        Some(plan::plan(sys, rules, ship, c.target, c.phase, c.pad, t))
    }
}

/// Where `target` is now, if it's still there.
pub(crate) fn target_position(bus: &mut impl Bus, target: NavTarget) -> Option<DVec3> {
    let (sys, positions) = bus.positions();
    target.position(&sys, bus.time(), &positions)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn physical_events_end_clearances_and_forget_targets() {
        let target = NavTarget::Station(3);
        let set = || Avionics {
            nav_target: Some(target),
            clearance: Some(Clearance { target, autopilot: true, phase: Phase::Final, pad: PadSlot::Center }),
            hyper_autopilot: true,
            ..Default::default()
        };
        let after = |e: ShipEvent| {
            let mut a = set();
            a.observe(&e);
            a
        };
        let docked = after(ShipEvent::Landed { body: "Station".into(), station: true });
        assert!(docked.clearance.is_none() && docked.nav_target == Some(target), "arrived: the clearance is used up");
        let crashed = after(ShipEvent::Crashed { body: "Station".into() });
        assert!(crashed.clearance.is_none());
        let gone = after(ShipEvent::GateEntered { to: "Lave".into() });
        assert!(gone.clearance.is_none() && gone.nav_target.is_none(), "the target was in the old system");
        let dropped = after(ShipEvent::HyperdriveDisengaged);
        assert!(!dropped.hyper_autopilot && dropped.autopilot_engaged());
        assert!(after(ShipEvent::Bumped).autopilot_engaged(), "a bump changes nothing");
        let new = after(ShipEvent::Respawned);
        assert!(new.nav_target.is_none() && new.clearance.is_none() && !new.hyper_autopilot);
    }
}
