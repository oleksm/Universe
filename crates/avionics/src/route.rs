//! Multi-stop routes, and the route autopilot that flies them: leave, cross
//! systems through gates, travel by hyperdrive, dock or land with the regular
//! autopilots, wait, repeat.

use std::collections::VecDeque;

use glam::DVec3;
use serde::{Deserialize, Serialize};
use universe_world::{BodyKind, ShipState, StarSystem};

use crate::avionics::{target_position, Avionics};
use crate::bus::Bus;
use crate::events::Event;
use crate::nav::NavTarget;

/// A place to visit: a station (dock) or spaceport (land) in some star system.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Stop {
    /// Galaxy index of the star system.
    pub system: usize,
    pub target: NavTarget,
}

/// How long the route autopilot waits at each stop (game seconds).
pub const DWELL: f64 = 20.0;

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Route {
    pub stops: Vec<Stop>,
    /// Index of the stop we're heading for (or dwelling at).
    pub next: usize,
    /// The route autopilot is flying.
    pub active: bool,
    /// Waiting at a stop until this game time.
    pub dwell_until: Option<f64>,
    /// Just lifted off a surface: climbing before anything else.
    pub departing: bool,
}

impl Route {
    pub fn current(&self) -> Option<Stop> {
        self.stops.get(self.next).copied()
    }

    pub fn clear(&mut self) {
        *self = Route::default();
    }

    /// Take the last stop off the route (if any), keeping the progress within it.
    pub fn pop(&mut self) -> Option<Stop> {
        let stop = self.stops.pop()?;
        self.next = self.next.min(self.stops.len());
        Some(stop)
    }
}

/// Name of a stop, with its system (`sys`, the stop's).
pub fn stop_name(sys: &StarSystem, stop: Stop) -> String {
    let name = match stop.target {
        NavTarget::Station(b) | NavTarget::Gate(b) => sys.bodies.get(b).map_or_else(String::new, |b| b.name.clone()),
        NavTarget::Spaceport(p) => sys.spaceports.get(p).map_or_else(String::new, |p| p.name.clone()),
    };
    format!("{name} ({})", sys.name)
}

/// How close the hyperdrive gets before an autopilot takes over (m). The
/// hyperdrive drops out a little closer still (120 km / 20 km).
pub(crate) fn hyperjump_limit(target: NavTarget) -> f64 {
    match target {
        NavTarget::Spaceport(_) => 200_000.0,
        NavTarget::Station(_) | NavTarget::Gate(_) => 30_000.0,
    }
}

/// Is a ship landed on `body` at `local_position` (in `sys`) at this target?
fn landed_at(sys: &StarSystem, target: NavTarget, body: usize, local_position: DVec3) -> bool {
    match target {
        NavTarget::Station(s) => s == body,
        NavTarget::Spaceport(p) => sys.on_pad(p, body, local_position.normalize()),
        NavTarget::Gate(_) => false,
    }
}

/// Height above the ground of the dominant body (m), if it has a surface.
fn ground_altitude(bus: &mut impl Bus) -> f64 {
    let (sys, positions) = bus.positions();
    let (p, t) = (bus.ship().position, bus.time());
    let d = sys.dominant(p, &positions);
    let b = &sys.bodies[d];
    p.distance(positions[d]) - b.surface_radius_at(positions[d], p, t)
}

impl Avionics {
    /// The route autopilot, once a frame.
    pub(crate) fn route_step(&mut self, bus: &mut impl Bus, events: &mut Vec<Event>) {
        if !self.route.active {
            return;
        }
        let Some(stop) = self.route.current() else {
            self.route.active = false;
            events.push(Event::RouteComplete);
            return;
        };
        match bus.ship().state.clone() {
            ShipState::Destroyed { .. } | ShipState::Transit { .. } => {}
            ShipState::Landed { body, local_position, .. } => {
                let sys = bus.star_system();
                if bus.system() == stop.system && landed_at(&sys, stop.target, body, local_position) {
                    match self.route.dwell_until {
                        None => {
                            self.route.dwell_until = Some(bus.time() + DWELL);
                            events.push(Event::RouteStop { number: self.route.next + 1, name: stop_name(&sys, stop) });
                        }
                        Some(t) if bus.time() >= t => {
                            self.route.dwell_until = None;
                            self.route.next += 1;
                            if self.route.next >= self.route.stops.len() {
                                self.route.active = false;
                                events.push(Event::RouteComplete);
                                return;
                            }
                            self.leave(bus, &sys, body, events);
                        }
                        Some(_) => {}
                    }
                } else {
                    self.leave(bus, &sys, body, events);
                }
            }
            ShipState::Flying => self.route_fly(bus, stop, events),
        }
    }

    /// Launch from a station, or lift off a surface and climb.
    fn leave(&mut self, bus: &mut impl Bus, sys: &StarSystem, body: usize, events: &mut Vec<Event>) {
        if sys.bodies[body].kind == BodyKind::Station {
            self.set_controls(bus, events, |c| c.throttle = 0.2);
        } else {
            self.set_controls(bus, events, |c| c.rcs = DVec3::Y);
            self.route.departing = true;
        }
    }

    fn route_fly(&mut self, bus: &mut impl Bus, stop: Stop, events: &mut Vec<Event>) {
        if bus.ship().hyperdrive {
            return; // the hyperdrive autopilot flies and drops out on arrival
        }
        if self.route.departing {
            // Straight up on the lift thrusters until clear of the ground.
            if ground_altitude(bus) < 3000.0 {
                self.set_controls(bus, events, |c| {
                    c.rcs = DVec3::Y;
                    c.throttle = 0.0;
                });
                return;
            }
            self.route.departing = false;
            self.set_controls(bus, events, |c| c.rcs = DVec3::ZERO);
        }
        // Next hop: the stop itself, or the gate toward its system.
        let hop = if bus.system() == stop.system {
            stop.target
        } else {
            let Some(next) = gate_path(bus.gate_links(), bus.system(), stop.system).and_then(|p| p.first().copied()) else {
                self.route.active = false;
                events.push(Event::RouteBlocked { reason: "NO GATE PATH".into() });
                return;
            };
            let Some(g) = bus.star_system().gate_to(next) else {
                self.route.active = false;
                events.push(Event::RouteBlocked { reason: "GATE MISSING".into() });
                return;
            };
            NavTarget::Gate(g)
        };
        if self.nav_target != Some(hop) {
            self.nav_target = Some(hop);
            self.clearance = None;
        }
        let Some(at) = target_position(bus, hop) else {
            self.route.active = false;
            events.push(Event::RouteBlocked { reason: "STOP NOT FOUND".into() });
            return;
        };
        // Hyperdrive (which steers around anything in the way) until close:
        // the landing and docking approaches only mind their own target.
        let far = at.distance(bus.ship().position) > hyperjump_limit(hop);
        match self.clearance {
            Some(c) if !c.autopilot => self.toggle_autopilot(bus, events),
            Some(_) => {}
            None if far => {
                // Far: hyperdrive there, steered by its autopilot.
                self.toggle_hyperdrive(bus, events);
                self.hyper_autopilot = true;
                self.set_controls(bus, events, |c| c.throttle = 1.0);
            }
            None => {
                if self.request_clearance(bus, events) {
                    self.toggle_autopilot(bus, events);
                }
            }
        }
    }
}

/// Shortest chain of systems from `from` to `to` over gate links, excluding
/// `from` itself. Empty if they're the same system; None if unreachable.
pub fn gate_path(links: &[(usize, usize)], from: usize, to: usize) -> Option<Vec<usize>> {
    if from == to {
        return Some(Vec::new());
    }
    let mut prev = std::collections::HashMap::new();
    let mut queue = VecDeque::from([from]);
    prev.insert(from, from);
    while let Some(s) = queue.pop_front() {
        for &(a, b) in links {
            let n = if a == s { b } else if b == s { a } else { continue };
            if prev.contains_key(&n) {
                continue;
            }
            prev.insert(n, s);
            if n == to {
                let mut path = vec![to];
                let mut at = to;
                while prev[&at] != from {
                    at = prev[&at];
                    path.push(at);
                }
                path.reverse();
                return Some(path);
            }
            queue.push_back(n);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gate_paths() {
        let links = [(1, 2), (2, 3), (1, 4), (4, 5), (3, 5)];
        assert_eq!(gate_path(&links, 1, 1), Some(vec![]));
        assert_eq!(gate_path(&links, 1, 2), Some(vec![2]));
        assert_eq!(gate_path(&links, 1, 3), Some(vec![2, 3]));
        assert_eq!(gate_path(&links, 1, 5), Some(vec![4, 5]));
        assert_eq!(gate_path(&links, 1, 9), None);
    }
}
