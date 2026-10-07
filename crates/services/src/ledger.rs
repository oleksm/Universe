//! The ledger: who holds what — credits, and goods in holds — kept double
//! entry. Every movement is a transfer from one party to another with its
//! cause, so nothing appears or vanishes: across all parties, every asset
//! sums to zero (the world's account is where starting funds come from).
//! Pilots can't go below zero; markets (backed by their whole economy) can.

use std::collections::{BTreeMap, HashMap, VecDeque};

use universe_protocol::{BodyId, Cause, Tick};
use universe_world::Facility;
use universe_world::registry::{Batch, Mark};

/// Who holds things.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Party {
    /// The pilot of a ship (its purse, and its hold).
    Pilot(BodyId),
    /// A market, by system and place.
    Market(usize, Facility),
    /// The world: where starting funds come from (and what's written off goes).
    World,
    /// A settled system's administration (what its land office takes in).
    Administration(usize),
    /// A company of the registry's Maker House that owns land (by its number
    /// in the land office's list).
    Company(u32),
}

/// What's held.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Asset {
    Credits,
    /// Units of a good (by catalog index).
    Goods(usize),
}

/// One transfer, as journalled.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Entry {
    pub tick: Tick,
    pub from: Party,
    pub to: Party,
    pub asset: Asset,
    pub amount: f64,
    pub cause: Cause,
}

#[derive(Clone, Debug, Default)]
pub struct Ledger {
    balances: HashMap<(Party, Asset), f64>,
    /// Every pilot's goods (for listing a hold), kept alongside.
    holds: HashMap<BodyId, BTreeMap<usize, f64>>,
    /// The marks of the products in each pilot's hold, and the lots of its bulk (SFO 21), by good,
    /// oldest first.
    marks: HashMap<BodyId, BTreeMap<usize, VecDeque<Mark>>>,
    lots: HashMap<BodyId, BTreeMap<usize, crate::lots::Lots>>,
    /// The latest transfers, oldest first (the last `KEEP`).
    pub journal: Vec<Entry>,
}

/// Journal entries kept.
const KEEP: usize = 10_000;

impl Ledger {
    pub fn balance(&self, party: Party, asset: Asset) -> f64 {
        self.balances.get(&(party, asset)).copied().unwrap_or(0.0)
    }

    pub fn credits(&self, party: Party) -> f64 {
        self.balance(party, Asset::Credits)
    }

    /// What a pilot's hold holds: (good, units), by good.
    pub fn hold(&self, pilot: BodyId) -> Vec<(usize, u32)> {
        self.holds.get(&pilot).map_or_else(Vec::new, |h| h.iter().filter(|(_, n)| **n >= 0.5).map(|(&g, &n)| (g, n.round() as u32)).collect())
    }

    /// Move `amount` of `asset` from `from` to `to` (at `tick`, because of
    /// `cause`). A pilot can't give what it hasn't got.
    pub fn transfer(&mut self, from: Party, to: Party, asset: Asset, amount: f64, tick: Tick, cause: Cause) -> Result<(), String> {
        if amount < 0.0 {
            return self.transfer(to, from, asset, -amount, tick, cause);
        }
        if let Party::Pilot(_) = from
            && self.balance(from, asset) + 1e-6 < amount
        {
            return Err(match asset {
                Asset::Credits => "NOT ENOUGH CREDITS".into(),
                Asset::Goods(_) => "NOT IN THE HOLD".into(),
            });
        }
        self.post(from, asset, -amount);
        self.post(to, asset, amount);
        self.journal.push(Entry { tick, from, to, asset, amount, cause });
        let excess = self.journal.len().saturating_sub(KEEP);
        self.journal.drain(..excess);
        Ok(())
    }

    /// Set `party`'s holding of `asset` to `amount`, the difference coming
    /// from (or going to) the world: starting funds, a loaded save.
    pub fn settle(&mut self, party: Party, asset: Asset, amount: f64, tick: Tick, cause: Cause) {
        let d = amount - self.balance(party, asset);
        if d.abs() > 1e-9 {
            let _ = self.transfer(Party::World, party, asset, d, tick, cause);
        }
    }

    /// Into pilot `pilot`'s hold with good `good`: its marks and lots.
    pub fn load_marked(&mut self, pilot: BodyId, good: usize, marks: Vec<Mark>, lots: Vec<(Batch, f64)>) {
        if !marks.is_empty() {
            self.marks.entry(pilot).or_default().entry(good).or_default().extend(marks);
        }
        if !lots.is_empty() {
            crate::lots::put(self.lots.entry(pilot).or_default().entry(good).or_default(), lots);
        }
    }

    /// Out of pilot `pilot`'s hold: `units` of good `good` (`kg` of it), their marks and lots,
    /// oldest first.
    pub fn unload_marked(&mut self, pilot: BodyId, good: usize, units: usize, kg: f64) -> (Vec<Mark>, Vec<(Batch, f64)>) {
        let marks = self.marks.get_mut(&pilot).and_then(|m| m.get_mut(&good)).map_or(Vec::new(), |m| m.drain(..units.min(m.len())).collect());
        let lots = self.lots.get_mut(&pilot).and_then(|l| l.get_mut(&good)).map_or(Vec::new(), |l| crate::lots::take(l, kg));
        (marks, lots)
    }

    /// The marks of the products in pilot `pilot`'s hold, by good.
    pub fn hold_marks(&self, pilot: BodyId) -> Option<&BTreeMap<usize, VecDeque<Mark>>> {
        self.marks.get(&pilot)
    }

    /// Everything a pilot holds goes back to the world (its ship is gone).
    pub fn write_off(&mut self, pilot: BodyId, tick: Tick, cause: Cause) {
        self.marks.remove(&pilot);
        self.lots.remove(&pilot);
        for (good, _) in self.hold(pilot) {
            self.settle(Party::Pilot(pilot), Asset::Goods(good), 0.0, tick, cause);
        }
    }

    /// Does every asset sum to zero across all parties (nothing made or lost)?
    pub fn balanced(&self) -> bool {
        let mut sums: HashMap<Asset, f64> = HashMap::new();
        for (&(_, asset), &v) in &self.balances {
            *sums.entry(asset).or_default() += v;
        }
        sums.values().all(|s| s.abs() < 1e-6)
    }

    fn post(&mut self, party: Party, asset: Asset, d: f64) {
        *self.balances.entry((party, asset)).or_default() += d;
        if let (Party::Pilot(p), Asset::Goods(g)) = (party, asset) {
            let hold = self.holds.entry(p).or_default();
            let n = hold.entry(g).or_default();
            *n += d;
            if n.abs() < 1e-9 {
                hold.remove(&g);
            }
        }
    }
}
