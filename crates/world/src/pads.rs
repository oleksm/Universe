//! Traffic control's books. Corridors: a station's docking corridor or a
//! gate's run takes one ship at a time (on its final run, or launching out of
//! the slot); the rest wait at the entry. Pads: each spaceport's pads, who they're given to,
//! and who's waiting. A ship cleared to land is given a pad of its own; when
//! they're all taken it waits its turn in the queue (holding, see avionics),
//! and asks again. A pad stays booked while its ship is on its way and while
//! it stands on it; the orchestration tells the book each frame which claims
//! still hold (`keep`), and the rest are freed.

use std::collections::{HashMap, HashSet};

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

/// One spaceport's pads and queue.
#[derive(Clone, Debug, Default)]
struct Port {
    owners: [Option<usize>; PADS],
    /// Ships waiting, in order, with when each last asked.
    queue: Vec<(usize, f64)>,
}

/// Every spaceport's pads, by (system, port).
#[derive(Clone, Debug, Default)]
pub struct PadBook {
    ports: HashMap<(usize, usize), Port>,
}

impl PadBook {
    /// Ship `ship` asks for a pad at `port` in `system` at time `now`.
    pub fn request(&mut self, system: usize, port: usize, ship: usize, now: f64) -> PadGrant {
        let p = self.ports.entry((system, port)).or_default();
        if let Some(k) = p.owners.iter().position(|o| *o == Some(ship)) {
            return PadGrant::Pad(k);
        }
        p.queue.retain(|(_, t)| now - *t < QUEUE_PATIENCE);
        let free: Vec<usize> = (0..PADS).filter(|&k| p.owners[k].is_none()).collect();
        let place = p.queue.iter().position(|(s, _)| *s == ship).unwrap_or(p.queue.len());
        if place < free.len() {
            // Its turn: the free pad nearest the middle.
            let k = *free.iter().min_by_key(|&&k| (k as i32 - 4).abs()).expect("free pad");
            p.owners[k] = Some(ship);
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

    /// Keep only the pads whose claims still hold: `(system, port, pad,
    /// ship)`. A claim on a free pad (a ship standing on it, or just over it,
    /// that never asked) takes it: it's occupied either way.
    pub fn keep(&mut self, claims: &HashSet<(usize, usize, usize, usize)>) {
        for ((system, port), p) in self.ports.iter_mut() {
            for (k, owner) in p.owners.iter_mut().enumerate() {
                if let Some(ship) = *owner
                    && !claims.contains(&(*system, *port, k, ship))
                {
                    *owner = None;
                }
            }
        }
        let mut claims: Vec<_> = claims.iter().copied().collect();
        claims.sort();
        for (system, port, k, ship) in claims {
            let p = self.ports.entry((system, port)).or_default();
            if p.owners[k].is_none() && !p.owners.contains(&Some(ship)) {
                p.owners[k] = Some(ship);
            }
        }
    }

    /// Who has each pad at a port (for display).
    pub fn owners(&self, system: usize, port: usize) -> [Option<usize>; PADS] {
        self.ports.get(&(system, port)).map_or([None; PADS], |p| p.owners)
    }

    /// How many are waiting at a port.
    pub fn waiting(&self, system: usize, port: usize) -> usize {
        self.ports.get(&(system, port)).map_or(0, |p| p.queue.len())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pads_go_one_per_ship_and_the_rest_wait_their_turn() {
        let mut book = PadBook::default();
        let pads: Vec<PadGrant> = (0..PADS).map(|s| book.request(1, 0, s, 0.0)).collect();
        assert!(pads.iter().all(|g| matches!(g, PadGrant::Pad(_))));
        assert_eq!(book.request(1, 0, 3, 0.0), pads[3], "asking again: the same pad");
        assert_eq!(book.request(1, 0, 100, 1.0), PadGrant::Queued(0));
        assert_eq!(book.request(1, 0, 101, 1.0), PadGrant::Queued(1));
        // Ship 0 leaves: its pad goes to the first in line, not the second.
        let claims: HashSet<_> = (1..PADS).map(|s| (1, 0, match pads[s] { PadGrant::Pad(k) => k, _ => 0 }, s)).collect();
        book.keep(&claims);
        assert_eq!(book.request(1, 0, 101, 2.0), PadGrant::Queued(1));
        assert!(matches!(book.request(1, 0, 100, 2.0), PadGrant::Pad(_)));
        assert_eq!(book.request(1, 0, 101, 3.0), PadGrant::Queued(0));
        // Another port has its own pads.
        assert!(matches!(book.request(1, 1, 101, 3.0), PadGrant::Pad(_)));
    }
}

/// Traffic control's corridor book: who has each station's corridor or
/// gate's run (by system and body).
#[derive(Clone, Debug, Default)]
pub struct CorridorBook {
    holders: HashMap<(usize, usize), usize>,
}

impl CorridorBook {
    /// Ship `ship` asks to use the corridor of `body` in `system`: yes if it's
    /// free (it's now this ship's) or already this ship's.
    pub fn request(&mut self, system: usize, body: usize, ship: usize) -> bool {
        *self.holders.entry((system, body)).or_insert(ship) == ship
    }

    /// Keep only the holds whose claims still hold: `(system, body, ship)`.
    pub fn keep(&mut self, claims: &HashSet<(usize, usize, usize)>) {
        self.holders.retain(|(system, body), ship| claims.contains(&(*system, *body, *ship)));
    }
}
