//! What a pilot's postings carry to the world: traffic control requests,
//! made in order once the tick's postings are in, and each ship's inbox of
//! commands on their way to its devices (see `pilots`).

use universe_protocol::ShipId;
use universe_world::{Controls, Ship, ShipCommands, ShipEvent, World};

/// A traffic control request from a pilot: made in order with the tick's
/// postings (so the outcome doesn't depend on thread timing). Meanwhile the
/// pilot has the board's answer as it stood.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub(crate) enum Request {
    /// To traffic control: a pad at a port, a station's or gate's corridor.
    Pad { system: usize, port: universe_world::Facility, ship: ShipId, now: f64 },
    Corridor { system: usize, body: usize, ship: ShipId, now: f64 },
    /// To the market service: the quotes in this system (and the asker's
    /// account), from the market it's at; a trade there; a plan, declared.
    Quotes { system: usize, market: universe_world::Facility },
    Trade { market: universe_world::Facility, item: usize, units: i64 },
    Declare { market: universe_world::Facility, deal: universe_services::records::Deal },
    /// Fill the tank at the market it's at.
    Refuel { market: universe_world::Facility },
    /// Have the hull repaired (at a station).
    Repair { market: universe_world::Facility },
    /// Take aboard people waiting to leave; land those aboard to settle.
    Board { market: universe_world::Facility, to: (usize, universe_world::Facility) },
    Land { market: universe_world::Facility },
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
    // (Held as they are: the queue keeps its room, so posting allocates nothing.)
    Devices(ShipCommands),
    Turn(Option<Controls>),
}

impl Inbox {
    /// Commands for the devices, and maybe the turn, due at `due`.
    /// (`seen`: when its pilot saw the world it was ordered against.)
    pub(crate) fn post(&mut self, due: u64, seen: f64, devices: Vec<ShipCommands>, turn: Option<Option<Controls>>) {
        let at = self.pending.iter().position(|(d, _, _)| *d > due).unwrap_or(self.pending.len());
        let items = devices.into_iter().map(Pending::Devices).chain(turn.map(Pending::Turn));
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

/// A ship in the world with its pilot's side of it: the player's (vessel 0) and every NPC craft's
/// alike.
pub struct Vessel {
    pub name: std::sync::Arc<str>,
    pub ship: Ship,
    /// Galaxy index of the system it's in (its coordinates are relative to that star).
    pub system: usize,
    /// What its pilot shows (posted with its commands).
    pub status: crate::contract::Status,
    /// When its pilot last posted (world time), and whether the dead-man rule has cut in since.
    pub last_posted: f64,
    pub dead_man: bool,
    /// Its pilot sleeps till this tick (it's not in the views till then, unless a message wakes
    /// it).
    pub(crate) asleep_until: u64,
    /// Its pilot's commands on their way to the devices.
    pub inbox: Inbox,
}

impl Vessel {
    /// A vessel named `name` with `ship` in `system`, its pilot just in.
    pub fn new(name: &str, ship: Ship, system: usize, now: f64) -> Self {
        Vessel { name: name.into(), ship, system, status: Default::default(), last_posted: now, dead_man: false, asleep_until: 0, inbox: Default::default() }
    }
}

/// Every ship in the world, by id: the player's first (`ShipId::PLAYER`), then the crafts in the
/// order they came (craft `i` is `ShipId::craft(i)`).
pub struct Vessels(Vec<Vessel>);

impl Vessels {
    /// Just the player's.
    pub fn new(player: Vessel) -> Self {
        Vessels(vec![player])
    }

    /// A craft added: its id.
    pub fn push(&mut self, v: Vessel) -> ShipId {
        self.0.push(v);
        ShipId(self.0.len() - 1)
    }

    /// The NPC crafts, by craft index.
    pub fn crafts(&self) -> &[Vessel] {
        &self.0[1..]
    }
    pub fn crafts_mut(&mut self) -> &mut [Vessel] {
        &mut self.0[1..]
    }

    /// Every ship, with its id.
    pub fn iter(&self) -> impl Iterator<Item = (ShipId, &Vessel)> {
        self.0.iter().enumerate().map(|(i, v)| (ShipId(i), v))
    }
    pub fn iter_mut(&mut self) -> impl Iterator<Item = (ShipId, &mut Vessel)> {
        self.0.iter_mut().enumerate().map(|(i, v)| (ShipId(i), v))
    }
    pub fn all(&self) -> &[Vessel] {
        &self.0
    }
    pub fn all_mut(&mut self) -> &mut [Vessel] {
        &mut self.0
    }

    /// How many, the player's with them.
    pub fn len(&self) -> usize {
        self.0.len()
    }
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Ship `id`'s (none for a turret's, or an id no ship has).
    pub fn get(&self, id: ShipId) -> Option<&Vessel> {
        self.0.get(id.0).filter(|_| id.turret_of().is_none())
    }
    pub fn get_mut(&mut self, id: ShipId) -> Option<&mut Vessel> {
        if id.turret_of().is_some() { None } else { self.0.get_mut(id.0) }
    }
}

impl std::ops::Index<ShipId> for Vessels {
    type Output = Vessel;
    fn index(&self, id: ShipId) -> &Vessel {
        &self.0[id.0]
    }
}

impl std::ops::IndexMut<ShipId> for Vessels {
    fn index_mut(&mut self, id: ShipId) -> &mut Vessel {
        &mut self.0[id.0]
    }
}
