//! Traffic control: who may use which landing pad and which corridor, as one
//! service with explicit claims and releases.
//!
//! - **Pads**: a ship cleared to land is given a pad of its own
//!   (`request_pad`); when they're all taken it's queued (it holds, see
//!   avionics) and asks again. The pad is its until it's been there and gone
//!   (physically: see `presence`), or it's released (`release`: its
//!   clearance ended, it was wrecked, it left the system).
//! - **Corridors**: a station's docking corridor or a gate's run takes one ship
//!   at a time (`request_corridor`). It's held until the ship is through (it
//!   docked, it went through the gate, or it's on its final run and close in),
//!   has launched and gone, or is released.
//!
//! What's observed each frame is only physical fact (`presence`): who stands
//! on which pad or is in its column, and who has left a corridor behind.

use std::collections::HashMap;

use crate::spaceport::PADS;

/// What traffic control says to a request for a pad.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PadGrant {
    /// This pad is yours.
    Pad(usize),
    /// None free: you're waiting, with this many ahead of you.
    Queued(usize),
}

/// A waiting ship is forgotten if it hasn't asked for this long (s).
const QUEUE_PATIENCE: f64 = 60.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Owner {
    ship: usize,
    /// It has been on the pad (or in its column).
    arrived: bool,
}

/// One spaceport's pads and queue.
#[derive(Clone, Debug, Default)]
struct Port {
    owners: [Option<Owner>; PADS],
    /// Ships waiting, in order, with when each last asked.
    queue: Vec<(usize, f64)>,
}

/// Where a ship physically is, as far as traffic control cares, this frame.
#[derive(Clone, Debug, Default)]
pub struct Presence {
    pub ship: usize,
    pub system: usize,
    /// On a pad, or in the column over it: (port, pad).
    pub pad: Option<(usize, usize)>,
    /// Corridors (station or gate bodies) it's done with: through or clear of.
    pub clear_of: Vec<usize>,
}

/// Traffic control for every system.
#[derive(Clone, Debug, Default)]
pub struct TrafficControl {
    ports: HashMap<(usize, usize), Port>,
    corridors: HashMap<(usize, usize), usize>,
    /// Ships waiting for each corridor, in order, with when each last asked.
    corridor_queues: HashMap<(usize, usize), Vec<(usize, f64)>>,
}

impl TrafficControl {
    /// Ship `ship` asks for a pad at `port` in `system` at time `now`.
    pub fn request_pad(&mut self, system: usize, port: usize, ship: usize, now: f64) -> PadGrant {
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
            return PadGrant::Pad(k);
        }
        if place < p.queue.len() {
            p.queue[place].1 = now;
        } else {
            p.queue.push((ship, now));
        }
        PadGrant::Queued(place)
    }

    /// What `request_pad` would answer now, changing nothing (a ship asking
    /// while ships step side by side: its request is made after).
    pub fn peek_pad(&self, system: usize, port: usize, ship: usize) -> PadGrant {
        let Some(p) = self.ports.get(&(system, port)) else { return PadGrant::Queued(0) };
        if let Some(k) = p.owners.iter().position(|o| o.is_some_and(|o| o.ship == ship)) {
            return PadGrant::Pad(k);
        }
        PadGrant::Queued(p.queue.iter().position(|(s, _)| *s == ship).unwrap_or(p.queue.len()))
    }

    /// What `request_corridor` would answer now, changing nothing.
    pub fn peek_corridor(&self, system: usize, body: usize, ship: usize) -> Option<usize> {
        let key = (system, body);
        match self.corridors.get(&key) {
            Some(&s) if s == ship => None,
            held => {
                let queue = self.corridor_queues.get(&key);
                let place = queue.and_then(|q| q.iter().position(|(s, _)| *s == ship)).unwrap_or_else(|| queue.map_or(0, |q| q.len()));
                Some(place + usize::from(held.is_some()))
            }
        }
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
        }
    }

    /// Everything ship `ship` holds or waits for is given up (its clearance
    /// ended, it was wrecked, it left the system). Where it physically stands
    /// is taken again by `presence`.
    pub fn release(&mut self, ship: usize) {
        for p in self.ports.values_mut() {
            for o in p.owners.iter_mut() {
                if o.is_some_and(|o| o.ship == ship) {
                    *o = None;
                }
            }
            p.queue.retain(|(s, _)| *s != ship);
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
        let mut at: HashMap<usize, (usize, usize, usize)> = HashMap::new();
        for p in present {
            if let Some((port, pad)) = p.pad {
                at.insert(p.ship, (p.system, port, pad));
            }
            for &body in &p.clear_of {
                self.release_corridor(p.system, body, p.ship);
            }
        }
        for ((system, port), p) in self.ports.iter_mut() {
            for (k, o) in p.owners.iter_mut().enumerate() {
                if let Some(owner) = o {
                    if at.get(&owner.ship) == Some(&(*system, *port, k)) {
                        owner.arrived = true;
                    } else if owner.arrived {
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
            }
        }
    }

    /// Who has each pad at a port (for display).
    pub fn owners(&self, system: usize, port: usize) -> [Option<usize>; PADS] {
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
    pub fn waiting(&self, system: usize, port: usize) -> usize {
        self.ports.get(&(system, port)).map_or(0, |p| p.queue.len())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(ship: usize, pad: usize) -> Presence {
        Presence { ship, system: 1, pad: Some((0, pad)), clear_of: Vec::new() }
    }

    #[test]
    fn pads_go_one_per_ship_and_the_rest_wait_their_turn() {
        let mut tc = TrafficControl::default();
        let pads: Vec<PadGrant> = (0..PADS).map(|s| tc.request_pad(1, 0, s, 0.0)).collect();
        assert!(pads.iter().all(|g| matches!(g, PadGrant::Pad(_))));
        assert_eq!(tc.request_pad(1, 0, 3, 0.0), pads[3], "asking again: the same pad");
        assert_eq!(tc.request_pad(1, 0, 100, 1.0), PadGrant::Queued(0));
        assert_eq!(tc.request_pad(1, 0, 101, 1.0), PadGrant::Queued(1));
        // Ship 0 gives up: its pad goes to the first in line, not the second.
        tc.release(0);
        assert_eq!(tc.request_pad(1, 0, 101, 2.0), PadGrant::Queued(1));
        assert!(matches!(tc.request_pad(1, 0, 100, 2.0), PadGrant::Pad(_)));
        assert_eq!(tc.request_pad(1, 0, 101, 3.0), PadGrant::Queued(0));
        // Another port has its own pads.
        assert!(matches!(tc.request_pad(1, 1, 101, 3.0), PadGrant::Pad(_)));
    }

    #[test]
    fn a_pad_is_held_until_its_ship_has_been_and_gone() {
        let mut tc = TrafficControl::default();
        let PadGrant::Pad(k) = tc.request_pad(1, 0, 7, 0.0) else { panic!() };
        tc.presence(&[]);
        assert_eq!(tc.owners(1, 0)[k], Some(7), "still on its way: held");
        tc.presence(&[at(7, k)]);
        tc.presence(&[at(7, k)]);
        assert_eq!(tc.owners(1, 0)[k], Some(7), "standing on it");
        tc.presence(&[]);
        assert_eq!(tc.owners(1, 0)[k], None, "been and gone: free");
        // A ship that lands without asking occupies the pad.
        tc.presence(&[at(9, 2)]);
        assert_eq!(tc.owners(1, 0)[2], Some(9));
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
