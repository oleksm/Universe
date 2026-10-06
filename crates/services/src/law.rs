//! The law: who is fair game, and who is charged with what. Opening fire on a
//! ship that isn't makes the shooter fair game (aggressed) for `AGGRESSION`
//! seconds; shooting the aggressed is no crime. The law rules on the core's
//! hit events, in the order they were logged, and keeps the evidence with
//! every ruling. Where a system's administration has a law
//! (`world::order::law`), an offence it names is charged (`Charge`), with its
//! evidence; what its penalties do is the sim's.

use std::collections::HashMap;

use universe_protocol::{BodyId, Cause};
use universe_world::registry::Offence;

/// How long a ship stays aggressed after hitting one that wasn't (s), where
/// a holder's law doesn't say: 10 minutes.
pub const AGGRESSION: f64 = 600.0;

/// A hit the core reported: who fired, who was struck, when.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Hit {
    pub shooter: BodyId,
    pub target: BodyId,
    pub time: f64,
}

/// The law's decision, with its evidence.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Ruling {
    /// Who's aggressed, and until when.
    pub ship: BodyId,
    pub until: f64,
    /// What it came of: the hit event, and the hit itself.
    pub cause: Cause,
    pub evidence: Hit,
    /// The ship wasn't aggressed before (it's news to it).
    pub new: bool,
}

/// An offence charged against a ship under its system's law.
#[derive(Clone, Debug, PartialEq)]
pub struct Charge {
    pub ship: BodyId,
    pub system: usize,
    pub offence: Offence,
    pub time: f64,
    /// What it came of (the event logged), and who suffered by it, if anyone.
    pub cause: Cause,
    pub against: Option<BodyId>,
}

/// Who's aggressed, until when, and why; who's charged with what.
#[derive(Clone, Debug, Default)]
pub struct Law {
    standing: HashMap<BodyId, Ruling>,
    /// Every ruling made, oldest first (the last `KEEP`).
    pub rulings: Vec<Ruling>,
    /// Every charge made, oldest first (the last `KEEP`).
    pub charges: Vec<Charge>,
    /// When each ship's record was last wiped (a new ship): its charges since are its own.
    cleared: HashMap<BodyId, f64>,
}

/// Rulings kept on record.
const KEEP: usize = 1000;

impl Law {
    /// Is `ship` fair game at `now`?
    pub fn aggressed(&self, ship: BodyId, now: f64) -> bool {
        self.standing.get(&ship).is_some_and(|r| now < r.until)
    }

    /// Until when `ship` is fair game, if it is at `now`.
    pub fn until(&self, ship: BodyId, now: f64) -> Option<f64> {
        self.standing.get(&ship).map(|r| r.until).filter(|&u| now < u)
    }

    /// The ruling that made `ship` fair game, if it is at `now`.
    pub fn standing(&self, ship: BodyId, now: f64) -> Option<&Ruling> {
        self.standing.get(&ship).filter(|r| now < r.until)
    }

    /// A hit, as the core logged it (`cause`): firing on a ship that isn't
    /// fair game makes the shooter fair game for `lasts` (s: the law of
    /// whoever holds the space; None: unclaimed, no law) — if
    /// `shooter_answers` (a turret, say, doesn't). The ruling, if one was made.
    pub fn hit(&mut self, hit: Hit, shooter_answers: bool, lasts: Option<f64>, cause: Cause) -> Option<Ruling> {
        let lasts = lasts?;
        if !shooter_answers || self.aggressed(hit.target, hit.time) {
            return None;
        }
        let new = !self.aggressed(hit.shooter, hit.time);
        let ruling = Ruling { ship: hit.shooter, until: hit.time + lasts, cause, evidence: hit, new };
        self.record(ruling);
        Some(ruling)
    }

    /// Declare `ship` fair game until `until` (by `cause`: a scenario, a test,
    /// a court).
    pub fn declare(&mut self, ship: BodyId, until: f64, now: f64, cause: Cause) {
        let evidence = Hit { shooter: ship, target: ship, time: now };
        self.record(Ruling { ship, until, cause, evidence, new: !self.aggressed(ship, now) });
    }

    /// Wipe the record (a new ship at `now`, the old one gone). The charges stay on the record.
    pub fn forget(&mut self, ship: BodyId, now: f64) {
        self.standing.remove(&ship);
        self.cleared.insert(ship, now);
    }

    /// Charge a ship with an offence.
    pub fn charge(&mut self, c: Charge) {
        self.charges.push(c);
        let excess = self.charges.len().saturating_sub(KEEP);
        self.charges.drain(..excess);
    }

    /// The charges against `ship` as it is now (since its record was last wiped).
    pub fn charges_of(&self, ship: BodyId) -> impl Iterator<Item = &Charge> {
        let since = self.cleared.get(&ship).copied().unwrap_or(f64::NEG_INFINITY);
        self.charges.iter().filter(move |c| c.ship == ship && c.time >= since)
    }

    fn record(&mut self, r: Ruling) {
        self.standing.insert(r.ship, r);
        self.rulings.push(r);
        let excess = self.rulings.len().saturating_sub(KEEP);
        self.rulings.drain(..excess);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const EV: Cause = Cause::Event { tick: 7, index: 3 };

    #[test]
    fn opening_fire_on_the_innocent_makes_the_shooter_fair_game_with_the_evidence() {
        let mut law = Law::default();
        let r = law.hit(Hit { shooter: 1, target: 2, time: 10.0 }, true, Some(AGGRESSION), EV).expect("ruled");
        // Unclaimed space: no law.
        assert!(Law::default().hit(Hit { shooter: 1, target: 2, time: 10.0 }, true, None, EV).is_none());
        assert!(law.aggressed(1, 10.0) && !law.aggressed(2, 10.0));
        assert_eq!(r.until, 10.0 + AGGRESSION);
        assert_eq!(r.cause, EV);
        assert_eq!(r.evidence, Hit { shooter: 1, target: 2, time: 10.0 });
        assert!(r.new);
        // It lapses.
        assert!(!law.aggressed(1, 10.0 + AGGRESSION + 1.0));
    }

}
