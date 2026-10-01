//! Traffic control (a service): who may use which station, port or gate
//! (clearance), which landing pad and which corridor, with explicit claims
//! and releases.
//!
//! - **Pads**: a ship cleared to land is given a pad of its own
//!   (`request_pad`); when they're all taken it's queued (it holds, see
//!   avionics) and asks again. The pad is its until it's been there and gone
//!   (physically: see `presence`), or it's released (`release`: its
//!   clearance ended, it was wrecked, it left the system).
//!   A port is a spaceport or a station's deck (`Facility`).
//! - **Corridors**: a gate's run takes one ship
//!   at a time (`request_corridor`). It's held until the ship is through (it
//!   docked, it went through the gate, or it's on its final run and close in),
//!   has launched and gone, or is released.
//!
//! What's observed each frame is only physical fact (`presence`): who stands
//! on which pad or is in its column, and who has left a corridor behind.

use std::collections::HashMap;

use glam::DVec3;
use universe_world::ship::Ship;
use universe_world::spaceport::PADS;
use universe_world::system::{BodyKind, StarSystem};
use universe_protocol::{Cause, Tick};
use universe_world::traffic::Facility;

pub use universe_protocol::PadGrant;

/// A waiting ship is forgotten if it hasn't asked for this long (s).
const QUEUE_PATIENCE: f64 = 60.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Owner {
    ship: usize,
    /// It has been on the pad (or in its column).
    arrived: bool,
}

/// One port's pads (a spaceport's or a station deck's) and queue.
#[derive(Clone, Debug, Default)]
struct Port {
    owners: [Option<Owner>; PADS],
    /// Ships waiting, in order, with when each last asked.
    queue: Vec<(usize, f64)>,
}

/// Traffic control's board as it stood (see `TrafficControl::board`):
/// what a pilot reads to know where it stands, without changing anything.
#[derive(Clone, Debug, Default)]
pub struct Board {
    ports: HashMap<(usize, Facility), Port>,
    corridors: HashMap<(usize, usize), usize>,
    corridor_queues: HashMap<(usize, usize), Vec<(usize, f64)>>,
}

impl Board {
    /// What `TrafficControl::request_pad` would have answered.
    pub fn peek_pad(&self, system: usize, port: Facility, ship: usize) -> PadGrant {
        peek_pad(&self.ports, system, port, ship)
    }

    /// What `TrafficControl::request_corridor` would have answered.
    pub fn peek_corridor(&self, system: usize, body: usize, ship: usize) -> Option<usize> {
        peek_corridor(&self.corridors, &self.corridor_queues, system, body, ship)
    }
}

fn peek_pad(ports: &HashMap<(usize, Facility), Port>, system: usize, port: Facility, ship: usize) -> PadGrant {
    let Some(p) = ports.get(&(system, port)) else { return PadGrant::Queued(0) };
    if let Some(k) = p.owners.iter().position(|o| o.is_some_and(|o| o.ship == ship)) {
        return PadGrant::Pad(k);
    }
    PadGrant::Queued(p.queue.iter().position(|(s, _)| *s == ship).unwrap_or(p.queue.len()))
}

fn peek_corridor(corridors: &HashMap<(usize, usize), usize>, queues: &HashMap<(usize, usize), Vec<(usize, f64)>>, system: usize, body: usize, ship: usize) -> Option<usize> {
    let key = (system, body);
    match corridors.get(&key) {
        Some(&s) if s == ship => None,
        held => {
            let queue = queues.get(&key);
            let place = queue.and_then(|q| q.iter().position(|(s, _)| *s == ship)).unwrap_or_else(|| queue.map_or(0, |q| q.len()));
            Some(place + usize::from(held.is_some()))
        }
    }
}

/// Where a ship physically is, as far as traffic control cares, this frame.
#[derive(Clone, Debug, Default)]
pub struct Presence {
    pub ship: usize,
    pub system: usize,
    /// On a pad, or in the column over it: (port, pad).
    pub pad: Option<(Facility, usize)>,
    /// Corridors (station or gate bodies) it's done with: through or clear of.
    pub clear_of: Vec<usize>,
}

/// Traffic control for every system.
#[derive(Clone, Debug, Default)]
pub struct TrafficControl {
    ports: HashMap<(usize, Facility), Port>,
    corridors: HashMap<(usize, usize), usize>,
    /// Ships waiting for each corridor, in order, with when each last asked.
    corridor_queues: HashMap<(usize, usize), Vec<(usize, f64)>>,
    /// Every change, with its tick and cause (the last `JOURNAL`).
    pub journal: Vec<Change>,
    /// The tick now, why the next changes (other than requests) happen, and
    /// how many requests so far (each a message).
    tick: Tick,
    cause: Option<Cause>,
    requests: u64,
}

/// Journal entries kept.
const JOURNAL: usize = 10_000;

/// A change traffic control made, when, to whom, and why.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Change {
    pub tick: Tick,
    pub ship: usize,
    pub what: What,
    pub cause: Cause,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum What {
    PadGranted { system: usize, port: Facility, pad: usize },
    /// Waiting for a pad: first in line at `place` (0: next).
    PadQueued { system: usize, port: Facility, place: usize },
    /// Taken by a ship standing on it unasked.
    PadOccupied { system: usize, port: Facility, pad: usize },
    PadFreed { system: usize, port: Facility, pad: usize },
    CorridorGranted { system: usize, body: usize },
    CorridorFreed { system: usize, body: usize },
}

impl TrafficControl {
    /// The tick now, and why the changes from here on (other than requests,
    /// which are messages) happen: an event that's ended a ship's business
    /// here, the rules at work on the facts observed…
    pub fn because(&mut self, tick: Tick, cause: Cause) {
        (self.tick, self.cause) = (tick, Some(cause));
    }

    fn note(&mut self, ship: usize, what: What, cause: Cause) {
        self.journal.push(Change { tick: self.tick, ship, what, cause });
        let excess = self.journal.len().saturating_sub(JOURNAL);
        self.journal.drain(..excess);
    }

    fn noted(&mut self, ship: usize, what: What) {
        let cause = self.cause.unwrap_or(Cause::Rules);
        self.note(ship, what, cause);
    }

    /// A request from `ship`, as a message (sender, number).
    pub fn request_from(&mut self, ship: usize) -> Cause {
        self.requests += 1;
        Cause::Message { sender: ship as u64, id: self.requests }
    }

    /// Ship `ship` asks for a pad at `port` in `system` at time `now`.
    pub fn request_pad(&mut self, system: usize, port: Facility, ship: usize, now: f64) -> PadGrant {
        let cause = self.request_from(ship);
        let p = self.ports.entry((system, port)).or_default();
        if let Some(k) = p.owners.iter().position(|o| o.is_some_and(|o| o.ship == ship)) {
            return PadGrant::Pad(k);
        }
        p.queue.retain(|(_, t)| now - *t < QUEUE_PATIENCE);
        let free: Vec<usize> = (0..PADS).filter(|&k| p.owners[k].is_none()).collect();
        let place = p.queue.iter().position(|(s, _)| *s == ship).unwrap_or(p.queue.len());
        if place < free.len() {
            // Its turn: the free pad nearest the middle.
            let k = *free.iter().min_by_key(|&&k| (k as i32 - 4).abs()).expect("free pad");
            p.owners[k] = Some(Owner { ship, arrived: false });
            if place < p.queue.len() {
                p.queue.remove(place);
            }
            self.note(ship, What::PadGranted { system, port, pad: k }, cause);
            return PadGrant::Pad(k);
        }
        if place < p.queue.len() {
            p.queue[place].1 = now;
        } else {
            p.queue.push((ship, now));
            self.note(ship, What::PadQueued { system, port, place }, cause);
        }
        PadGrant::Queued(place)
    }

    /// What `request_pad` would answer now, changing nothing (a ship asking
    /// while ships step side by side: its request is made after).
    pub fn peek_pad(&self, system: usize, port: Facility, ship: usize) -> PadGrant {
        peek_pad(&self.ports, system, port, ship)
    }

    /// What `request_corridor` would answer now, changing nothing.
    pub fn peek_corridor(&self, system: usize, body: usize, ship: usize) -> Option<usize> {
        peek_corridor(&self.corridors, &self.corridor_queues, system, body, ship)
    }

    /// The board as it stands: who holds which pad and corridor, and who
    /// waits, for pilots to read (they ask by request; see `Board`).
    pub fn board(&self) -> Board {
        Board { ports: self.ports.clone(), corridors: self.corridors.clone(), corridor_queues: self.corridor_queues.clone() }
    }

    /// Ship `ship` asks at `now` to use the corridor of station or gate
    /// `body` in `system`: `None` if it's its (it was free and this ship's
    /// turn, or already its), otherwise how many are ahead of it in line.
    pub fn request_corridor(&mut self, system: usize, body: usize, ship: usize, now: f64) -> Option<usize> {
        let key = (system, body);
        if self.corridors.get(&key) == Some(&ship) {
            return None;
        }
        let queue = self.corridor_queues.entry(key).or_default();
        queue.retain(|(_, t)| now - *t < QUEUE_PATIENCE);
        let place = match queue.iter().position(|(s, _)| *s == ship) {
            Some(k) => {
                queue[k].1 = now;
                k
            }
            None => {
                queue.push((ship, now));
                queue.len() - 1
            }
        };
        if place == 0 && !self.corridors.contains_key(&key) {
            queue.remove(0);
            self.corridors.insert(key, ship);
            let cause = self.request_from(ship);
            self.note(ship, What::CorridorGranted { system, body }, cause);
            return None;
        }
        // Ahead: the one in the corridor, and those before it in line.
        Some(place + usize::from(self.corridors.contains_key(&key)))
    }

    /// Ship `ship` is done with its corridor at `body` (it docked, went
    /// through the gate, launched and left).
    pub fn release_corridor(&mut self, system: usize, body: usize, ship: usize) {
        if self.corridors.get(&(system, body)) == Some(&ship) {
            self.corridors.remove(&(system, body));
            self.noted(ship, What::CorridorFreed { system, body });
        }
    }

    /// Everything ship `ship` holds or waits for is given up (its clearance
    /// ended, it was wrecked, it left the system). Where it physically stands
    /// is taken again by `presence`.
    pub fn release(&mut self, ship: usize) {
        let mut freed = Vec::new();
        for (&(system, port), p) in self.ports.iter_mut() {
            for (k, o) in p.owners.iter_mut().enumerate() {
                if o.is_some_and(|o| o.ship == ship) {
                    *o = None;
                    freed.push(What::PadFreed { system, port, pad: k });
                }
            }
            p.queue.retain(|(s, _)| *s != ship);
        }
        for (&(system, body), _) in self.corridors.iter().filter(|(_, s)| **s == ship) {
            freed.push(What::CorridorFreed { system, body });
        }
        freed.sort_by_key(|w| format!("{w:?}"));
        for w in freed {
            self.noted(ship, w);
        }
        self.corridors.retain(|_, s| *s != ship);
        for q in self.corridor_queues.values_mut() {
            q.retain(|(s, _)| *s != ship);
        }
    }

    /// This frame's physical facts: a pad's ship on it (or in its column)
    /// has arrived; one that has arrived and is gone frees it; a ship on a
    /// pad nobody holds occupies it. Corridors are freed by ships done with them.
    pub fn presence(&mut self, present: &[Presence]) {
        let mut at: HashMap<usize, (usize, Facility, usize)> = HashMap::new();
        for p in present {
            if let Some((port, pad)) = p.pad {
                at.insert(p.ship, (p.system, port, pad));
            }
            for &body in &p.clear_of {
                self.release_corridor(p.system, body, p.ship);
            }
        }
        let mut changes = Vec::new();
        for ((system, port), p) in self.ports.iter_mut() {
            for (k, o) in p.owners.iter_mut().enumerate() {
                if let Some(owner) = o {
                    if at.get(&owner.ship) == Some(&(*system, *port, k)) {
                        owner.arrived = true;
                    } else if owner.arrived {
                        changes.push((owner.ship, What::PadFreed { system: *system, port: *port, pad: k }));
                        *o = None;
                    }
                }
            }
        }
        let mut squatters: Vec<_> = at.into_iter().collect();
        squatters.sort();
        for (ship, (system, port, k)) in squatters {
            let p = self.ports.entry((system, port)).or_default();
            if p.owners[k].is_none() && !p.owners.iter().any(|o| o.is_some_and(|o| o.ship == ship)) {
                p.owners[k] = Some(Owner { ship, arrived: true });
                changes.push((ship, What::PadOccupied { system, port, pad: k }));
            }
        }
        changes.sort_by_key(|(s, w)| (*s, format!("{w:?}")));
        for (ship, w) in changes {
            self.noted(ship, w);
        }
    }

    /// Who has each pad at a port (for display).
    pub fn owners(&self, system: usize, port: Facility) -> [Option<usize>; PADS] {
        self.ports.get(&(system, port)).map_or([None; PADS], |p| p.owners.map(|o| o.map(|o| o.ship)))
    }

    /// Every corridor held: (system, body, ship).
    pub fn corridors_held(&self) -> impl Iterator<Item = (usize, usize, usize)> + '_ {
        self.corridors.iter().map(|(&(system, body), &ship)| (system, body, ship))
    }

    /// Who holds the corridor of `body` in `system`.
    pub fn corridor(&self, system: usize, body: usize) -> Option<usize> {
        self.corridors.get(&(system, body)).copied()
    }

    /// How many are waiting at a port.
    pub fn waiting(&self, system: usize, port: Facility) -> usize {
        self.ports.get(&(system, port)).map_or(0, |p| p.queue.len())
    }
}

// Clearance: traffic control's rules on who may use a station, a port or a gate.

/// The station nearest to `p`, if the system has one.
pub fn nearest_station(sys: &StarSystem, p: DVec3, positions: &[DVec3]) -> Option<Facility> {
    sys.bodies
        .iter()
        .enumerate()
        .filter(|(_, b)| b.kind == BodyKind::Station)
        .map(|(i, _)| (i, positions[i].distance(p)))
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(i, _)| Facility::Station(i))
}

/// A ship asks for clearance to use `target` (if it names one) at `t`
/// (`positions` at `t`): granted, or refused with the reason.
pub fn request(sys: &StarSystem, ship: &Ship, target: Option<Facility>, t: f64, positions: &[DVec3]) -> Result<Facility, String> {
    if !ship.is_flying() {
        return Err("NOT IN FLIGHT".into());
    }
    if ship.hyperdrive {
        return Err("DISENGAGE HYPERDRIVE FIRST".into());
    }
    if ship.armed {
        return Err("WEAPONS ARMED - DISARM FIRST (B)".into());
    }
    let Some(target) = target else {
        return Err("NO TARGET - PICK ONE ON THE MAP (M)".into());
    };
    let Some(at) = target.position(sys, t, positions) else {
        return Err("TARGET NOT IN THIS SYSTEM".into());
    };
    let range = target.clearance_range(sys);
    if at.distance(ship.position) > range {
        return Err(format!("OUT OF RANGE - CLOSE TO {:.0} KM", range / 1000.0));
    }
    Ok(target)
}

/// A clearance for `target` lapses if the ship wanders more than twice the
/// granting range away, the target is gone, or the ship arms its weapons.
pub fn lapsed(sys: &StarSystem, ship: &Ship, target: Facility, t: f64, positions: &[DVec3]) -> bool {
    if ship.armed {
        return true;
    }
    let range = target.clearance_range(sys);
    target.position(sys, t, positions).is_none_or(|p| p.distance(ship.position) > 2.0 * range)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pads_go_one_per_ship_and_the_rest_wait_their_turn() {
        let mut tc = TrafficControl::default();
        let pads: Vec<PadGrant> = (0..PADS).map(|s| tc.request_pad(1, Facility::Spaceport(0), s, 0.0)).collect();
        assert!(pads.iter().all(|g| matches!(g, PadGrant::Pad(_))));
        assert_eq!(tc.request_pad(1, Facility::Spaceport(0), 3, 0.0), pads[3], "asking again: the same pad");
        assert_eq!(tc.request_pad(1, Facility::Spaceport(0), 100, 1.0), PadGrant::Queued(0));
        assert_eq!(tc.request_pad(1, Facility::Spaceport(0), 101, 1.0), PadGrant::Queued(1));
        // Ship 0 gives up: its pad goes to the first in line, not the second.
        tc.release(0);
        assert_eq!(tc.request_pad(1, Facility::Spaceport(0), 101, 2.0), PadGrant::Queued(1));
        assert!(matches!(tc.request_pad(1, Facility::Spaceport(0), 100, 2.0), PadGrant::Pad(_)));
        assert_eq!(tc.request_pad(1, Facility::Spaceport(0), 101, 3.0), PadGrant::Queued(0));
        // Another port has its own pads.
        assert!(matches!(tc.request_pad(1, Facility::Spaceport(1), 101, 3.0), PadGrant::Pad(_)));
    }

    #[test]
    fn a_corridor_takes_one_ship_until_it_is_done() {
        let mut tc = TrafficControl::default();
        assert_eq!(tc.request_corridor(1, 5, 1, 0.0), None);
        assert_eq!(tc.request_corridor(1, 5, 2, 0.0), Some(1), "taken: one ahead");
        assert_eq!(tc.request_corridor(1, 5, 3, 0.0), Some(2), "and two for the next");
        tc.presence(&[Presence { ship: 1, system: 1, pad: None, clear_of: vec![5] }]);
        assert_eq!(tc.request_corridor(1, 5, 3, 1.0), Some(1), "not its turn yet: 2 was first");
        assert_eq!(tc.request_corridor(1, 5, 2, 1.0), None, "free once it's through, and 2's turn");
        tc.release(2);
        assert_eq!(tc.corridor(1, 5), None);
        assert_eq!(tc.request_corridor(1, 5, 3, 2.0), None);
    }
}

