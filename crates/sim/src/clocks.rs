//! The engine's clocks (the tick tree: `docs/tick-tree.md`), their periods read from their records
//! (`standards/Engine/metadata/scheduling`) and nowhere else, and the work each runs bound to it
//! by `ClockHandler`: a clock added to the registry doesn't build until it's bound here.
//!
//! A world clock catches up in game time at its record's period; engine work inside the realtime
//! step runs on the ticks a whole multiple of its period (`due`). (`docs/tick-tree.md` §8.)

use universe_world::registry::{ClockHandler, ClockKey};
#[cfg(test)]
use universe_world::registry::registry;
pub use universe_world::registry::clock_every as every;

/// Clock `k`'s period, which it must have (a clock the engine steps by time).
pub fn period(k: ClockKey) -> f64 {
    every(k).unwrap_or_else(|| panic!("{} has no period in its record", k.key()))
}

/// `seconds` of game time in realtime ticks (at least one).
pub fn ticks(seconds: f64) -> u64 {
    (seconds / tick()).round().max(1.0) as u64
}

/// Whether work every `seconds` is due on realtime tick `tick` (the engine's way to run work at a
/// period: on the ticks that are whole multiples of it).
pub fn due(tick: u64, seconds: f64) -> bool {
    tick.is_multiple_of(ticks(seconds))
}

/// The realtime tick (s): every other period is a whole number of it.
pub fn tick() -> f64 {
    static TICK: std::sync::OnceLock<f64> = std::sync::OnceLock::new();
    *TICK.get_or_init(|| period(ClockKey::Realtime))
}

/// Where each clock's work is in the engine now: what runs on it, and how much of the tree's
/// design for it is built. (Bound exhaustively: a new clock is a compile error here.)
pub struct Binding;

impl ClockHandler for Binding {
    type Out = &'static str;
    fn galaxy(&mut self) -> &'static str {
        "the calendar (world time, fixed 1x); the star chart a function of time"
    }
    fn region(&mut self) -> &'static str {
        "hypernet delivery; transit through gates; not yet its own step"
    }
    fn system(&mut self) -> &'static str {
        "the per-system freeze each tick (the group of its clocks)"
    }
    fn celestial(&mut self) -> &'static str {
        "orbits and rotation as functions of time (rails): nothing to step"
    }
    fn planetary(&mut self) -> &'static str {
        "climate and clouds as functions of time (generators); no stepped state yet"
    }
    fn administration(&mut self) -> &'static str {
        "services::land: the land levy at its period (`land::levy_every`); zoning, census, insurers to come"
    }
    fn economy(&mut self) -> &'static str {
        "services::economy, stepped at its period (`economy::step`: recipes, people, wear, mines)"
    }
    fn market(&mut self) -> &'static str {
        "commerce: price boards published, and standings from law's events, at its period"
    }
    fn machinery(&mut self) -> &'static str {
        "the dead-man rule checked at its period; power, fuel and heat still inside each craft's realtime step"
    }
    fn rails(&mut self) -> &'static str {
        "not yet: coasting crafts still step every tick (long substeps from their orbit)"
    }
    fn npc_lane(&mut self) -> &'static str {
        "not yet: unwatched crafts still step on the realtime tick"
    }
    fn realtime(&mut self) -> &'static str {
        "every craft every tick (200 Hz as recorded), side by side on a frozen world: bubbles not yet"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every clock has a record, the stepped ones a period that is a whole number of the realtime
    /// tick, and a binding.
    #[test]
    fn every_clock_is_recorded_and_bound() {
        let t = tick();
        assert!(t > 0.0);
        for &k in ClockKey::ALL {
            assert!(registry().clock(k.key()).is_some(), "{} has no record", k.key());
            if let Some(e) = every(k) {
                let n = e / t;
                assert!((n - n.round()).abs() < 1e-6, "{} every {e} s isn't whole ticks of {t}", k.key());
            }
            assert!(!k.handle(&mut Binding).is_empty());
        }
    }
}
