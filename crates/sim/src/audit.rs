//! Persistence and audit (re-architecture R9). The world is a function of
//! its seed and its inputs: the pilots' postings (applied at their ticks) and
//! a few operations on it (settlers spawned, a respawn, trades, walking). The
//! `InputLog` records them tick by tick; `Universe::replay` runs a world from
//! the seed and the log alone, no pilots at all, and `Universe::state_hash`
//! says whether two worlds are the same.
//!
//! A `WorldSave` is that log (the checkpoint is the seed) and each pilot's
//! own state, the cockpit's included: loading replays the world, then hands
//! the pilots back what they knew, and play goes on.

use serde::{Deserialize, Serialize};
use universe_world::{Controls, Facility, WalkCommands};

use crate::contract::Posting;
use crate::universe::Universe;

/// The world's inputs, tick by tick, from its seed.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct InputLog {
    pub seed: u64,
    pub ticks: Vec<TickInputs>,
}

/// One tick's inputs: what came in since the last (postings applied at once,
/// operations), the tick's step, and the postings due at it.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TickInputs {
    pub tick: u64,
    pub before: Vec<Input>,
    pub real_dt: f64,
    pub warp: f64,
    pub due: Vec<Posting>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Input {
    Post(Vec<Posting>),
    Op(Op),
}

/// An operation on the world (not by posting).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Op {
    /// Ships registered by their operator.
    Register(Vec<crate::contract::Registration>),
    Respawn,
    Trade { market: Facility, item: usize, units: i64 },
    Walk(WalkCommands, f64),
    /// Land at a settlement: claimed, bought, built on.
    Claim { system: usize, port: usize, outline: Vec<(f64, f64)> },
    BuyParcel { system: usize, port: usize, number: u32 },
    Build { system: usize, port: usize, number: u32, blueprint: String },
    /// A works' module set to one of its recipes (or to nothing).
    SetUp { works: usize, setup: usize, recipe: Option<usize> },
    /// The player's dealings docked: a full tank, a mended hull, a module fitted, a hull bought,
    /// something from the vending machine, the drive's trim, passengers taken on.
    Refuel,
    Repair,
    Refit { slot: String, module: Option<universe_world::content::Handle<universe_world::modules::Module>> },
    BuyHull(universe_world::ship::Hull),
    Vend(usize),
    Trim(universe_world::trim::Trim),
    Passengers(Option<(usize, Facility)>),
}

impl Universe {
    /// Record the world's inputs from now on (from its start, for a replay
    /// or save to hold).
    pub fn record_inputs(&mut self) {
        self.replay.input_log = Some(InputLog { seed: self.world.galaxy.seed, ticks: Vec::new() });
    }

    /// Note an input that came in between ticks (if recording).
    pub(crate) fn note(&mut self, input: impl FnOnce() -> Input) {
        if self.replay.input_log.is_some() {
            self.replay.between.push(input());
        }
    }

    /// Carry out an operation (as recorded).
    pub fn op(&mut self, op: Op) {
        match op {
            Op::Register(ships) => self.register(ships),
            Op::Respawn => self.respawn(),
            Op::Trade { market, item, units } => {
                let _ = self.trade(market, item, units);
            }
            Op::Walk(c, dt) => self.walk(&c, dt),
            Op::Claim { system, port, outline } => {
                let _ = self.pilot_claim(crate::combat::PLAYER, system, port, outline);
            }
            Op::BuyParcel { system, port, number } => {
                let _ = self.pilot_buy_parcel(crate::combat::PLAYER, system, port, number);
            }
            Op::Build { system, port, number, blueprint } => {
                let _ = self.pilot_build(crate::combat::PLAYER, system, port, number, &blueprint);
            }
            Op::SetUp { works, setup, recipe } => {
                let _ = self.pilot_set_up(crate::combat::PLAYER, works, setup, recipe);
            }
            Op::Refuel => self.refuel_player(),
            Op::Repair => self.repair_player(),
            Op::Refit { slot, module } => {
                let _ = self.refit(&slot, module);
            }
            Op::BuyHull(hull) => {
                let _ = self.buy_hull(hull);
            }
            Op::Vend(item) => self.vend(item),
            Op::Trim(t) => {
                let _ = self.set_trim(t);
            }
            Op::Passengers(to) => self.passengers(to),
        }
    }

    /// A world run from `log` alone: its seed, its operations and postings,
    /// tick by tick. (No pilots think: what they did is in the log.)
    pub fn replay(log: &InputLog) -> Universe {
        let mut u = Universe::bare(log.seed);
        u.replay.replaying = true;
        for t in &log.ticks {
            for input in &t.before {
                match input {
                    Input::Post(p) => u.post(p.clone()),
                    Input::Op(o) => u.op(o.clone()),
                }
            }
            u.replay.replay_due = t.due.clone();
            u.tick(t.real_dt, t.warp, &Controls::default());
        }
        u.replay.replaying = false;
        u
    }

    /// What the world is, as a number: every ship (where, how fast, which
    /// way, its state, hull and settings), the clock, and the services'
    /// records.
    pub fn state_hash(&self) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut h = std::collections::hash_map::DefaultHasher::new();
        let mut ship = |system: usize, s: &universe_world::Ship| {
            system.hash(&mut h);
            for v in [s.position, s.velocity] {
                v.to_array().map(f64::to_bits).hash(&mut h);
            }
            s.orientation.to_array().map(f64::to_bits).hash(&mut h);
            format!("{:?}", s.state).hash(&mut h);
            (s.hull.to_bits(), s.throttle.to_bits(), s.rcs.to_array().map(f64::to_bits), s.armed, s.hyperdrive, s.ammo).hash(&mut h);
            // (What it is and carries: its hull, fit, fuel and cargo.)
            (universe_world::content::content().get(s.class).key.as_str(), s.fuel.to_bits(), s.cargo.to_bits(), s.passengers).hash(&mut h);
            format!("{:?}", s.fit).hash(&mut h);
        };
        ship(self.vessels[crate::combat::PLAYER].system, &self.vessels[crate::combat::PLAYER].ship);
        for c in self.vessels.crafts() {
            ship(c.system, &c.ship);
        }
        (self.tick, self.world.time.to_bits()).hash(&mut h);
        format!("{:?}", self.records.stats).hash(&mut h);
        self.services.atc.journal.len().hash(&mut h);
        self.services.ledger.journal.len().hash(&mut h);
        // (The land: every lot's owner, every facility's modules and when they're done.)
        for g in &self.services.land.grounds {
            for l in &g.lots {
                (l.number, format!("{:?}", l.owner)).hash(&mut h);
            }
            for w in &g.works {
                (w.parcel, w.done_at.iter().map(|t| t.to_bits()).collect::<Vec<_>>()).hash(&mut h);
            }
        }
        let mut mined: Vec<_> = self.world.mined.iter().map(|(&k, v)| (k, v.to_bits())).collect();
        mined.sort_unstable();
        mined.hash(&mut h);
        h.finish()
    }

}
