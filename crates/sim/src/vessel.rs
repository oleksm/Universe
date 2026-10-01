//! What a pilot's postings carry to the world: traffic control requests,
//! made in order once the tick's postings are in, and each ship's inbox of
//! commands on their way to its devices (see `pilots`).

use universe_world::{Controls, Ship, ShipCommands, ShipEvent, World};

/// A traffic control request from a pilot: made in order with the tick's
/// postings (so the outcome doesn't depend on thread timing). Meanwhile the
/// pilot has the board's answer as it stood.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub(crate) enum Request {
    /// To traffic control: a pad at a port, a station's or gate's corridor.
    Pad { system: usize, port: usize, ship: usize, now: f64 },
    Corridor { system: usize, body: usize, ship: usize, now: f64 },
    /// To the market service: the quotes in this system (and the asker's
    /// account), from the market it's at; a trade there; a plan, declared.
    Quotes { system: usize, market: universe_world::Facility },
    Trade { market: universe_world::Facility, item: usize, units: i64 },
    Declare { market: universe_world::Facility, deal: universe_services::records::Deal },
    /// Fill the tank at the market it's at.
    Refuel { market: universe_world::Facility },
}

/// A ship's commands on their way to its devices: its pilot's postings,
/// each taking effect at its due tick (see `pilots`).
#[derive(Clone, Debug, Default)]
pub struct Inbox {
    /// Due tick, and when its pilot saw the world (world time), for each.
    pending: std::collections::VecDeque<(u64, f64, Pending)>,
    /// The turn commanded, held until its pilot changes it.
    turn: Option<Controls>,
}

#[derive(Clone, Debug)]
enum Pending {
    Devices(Box<ShipCommands>),
    Turn(Option<Controls>),
}

impl Inbox {
    /// Commands for the devices, and maybe the turn, due at `due`.
    /// (`seen`: when its pilot saw the world it was ordered against.)
    pub(crate) fn post(&mut self, due: u64, seen: f64, devices: Vec<ShipCommands>, turn: Option<Option<Controls>>) {
        let at = self.pending.iter().position(|(d, _, _)| *d > due).unwrap_or(self.pending.len());
        let items = devices.into_iter().map(|c| Pending::Devices(Box::new(c))).chain(turn.map(Pending::Turn));
        for (k, item) in items.enumerate() {
            self.pending.insert(at + k, (due, seen, item));
        }
    }

    /// Hand the devices what's due by `tick`; the turn now held. An
    /// order given before the ship locked down (its pilot hadn't seen it
    /// land) doesn't touch its engine or thrusters.
    pub(crate) fn deliver(&mut self, world: &World, ship: &mut Ship, system: usize, t: f64, tick: u64, events: &mut Vec<ShipEvent>) -> Option<Controls> {
        while self.pending.front().is_some_and(|(due, _, _)| *due <= tick) {
            let (_, seen, item) = self.pending.pop_front().expect("due");
            match item {
                Pending::Devices(mut c) => {
                    if matches!(ship.state, universe_world::ShipState::Landed { .. }) && seen < ship.locked_at {
                        (c.throttle, c.rcs) = (ship.throttle, ship.rcs);
                    }
                    world.command_at(ship, system, &c, t, events);
                }
                Pending::Turn(c) => self.turn = c,
            }
        }
        self.turn
    }
}
