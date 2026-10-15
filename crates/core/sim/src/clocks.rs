//! The engine's clocks (the tick tree: `docs/tick-tree.md`), their periods read from their records
//! (`standards/Engine/metadata/scheduling`) and nowhere else, and the work each runs bound to it
//! by `ClockHandler`: a clock added to the registry doesn't build until it's bound here.
//!
//! A world clock catches up in game time at its record's period; engine work inside the realtime
//! step runs on the ticks a whole multiple of its period (`due`). (`docs/tick-tree.md` §8.)

use universe_world::registry::{ClockHandler, ClockKey};
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

/// The clocks an app runs (`docs/miniapps-plan.md` §2.2): a set of the tree's clocks, each with a
/// period in its record. A clock not in the set does not run at all.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Subscriptions(pub std::collections::BTreeSet<ClockKey>);

impl Subscriptions {
    pub fn of(keys: &[ClockKey]) -> Self {
        Subscriptions(keys.iter().copied().collect())
    }
}

/// What a runtime's clocks step: one call a clock a step, at the step's end time.
pub trait Clocked {
    /// Clock `k` steps from `t - dt` to `t` (game seconds).
    fn step(&mut self, k: ClockKey, t: f64, dt: f64);
}

impl<F: FnMut(ClockKey, f64, f64)> Clocked for F {
    fn step(&mut self, k: ClockKey, t: f64, dt: f64) {
        self(k, t, dt)
    }
}

/// The tick tree as a scheduler over one world instance (its own time and time scale): each
/// subscribed clock steps at its record's period, in time order (at the same moment, the slower
/// clock first, so faster ones read its published state). A clock gets at most `budget` steps an
/// `advance`; the rest waits for the next one (never dropped), as the realtime tick does.
pub struct Runtime {
    clocks: Vec<(ClockKey, f64, f64)>,
    /// Game seconds per real second; 0 stands still.
    pub scale: f64,
    time: f64,
    budget: u32,
}

impl Runtime {
    /// Refused: a clock that steps on events or is a group (a runtime steps by time only).
    pub fn new(subs: &Subscriptions, scale: f64, budget: u32) -> Result<Self, String> {
        let mut clocks = vec![];
        for &k in &subs.0 {
            let every = every(k).ok_or_else(|| format!("{} has no period: it isn't stepped by time", k.key()))?;
            clocks.push((k, every, 0.0));
        }
        Self::with(clocks, scale, budget)
    }

    fn with(mut clocks: Vec<(ClockKey, f64, f64)>, scale: f64, budget: u32) -> Result<Self, String> {
        if !(scale.is_finite() && scale >= 0.0) || budget == 0 {
            return Err("a runtime needs a finite scale and a budget".into());
        }
        // (Slowest first: at the same moment, it steps before the faster ones.)
        clocks.sort_by(|a, b| b.1.total_cmp(&a.1));
        Ok(Runtime { clocks, scale, time: 0.0, budget })
    }

    /// The world instance's time (game seconds).
    pub fn time(&self) -> f64 {
        self.time
    }

    /// Clock `k`'s last step's end, if it is subscribed.
    pub fn done_to(&self, k: ClockKey) -> Option<f64> {
        self.clocks.iter().find(|c| c.0 == k).map(|c| c.2)
    }

    /// Game seconds clock `k` has still to step (0 when not subscribed).
    pub fn pending(&self, k: ClockKey) -> f64 {
        self.done_to(k).map_or(0.0, |d| self.time - d)
    }

    /// Real seconds `real_dt` pass: world time moves on by `real_dt × scale`, and every clock
    /// steps that is due by then (within its budget). How many steps each clock took.
    pub fn advance(&mut self, real_dt: f64, w: &mut impl Clocked) -> Vec<(ClockKey, u32)> {
        self.time += real_dt.max(0.0) * self.scale;
        let mut steps = vec![0u32; self.clocks.len()];
        loop {
            // The next due step: earliest end; ties to the slowest (first in the list).
            let next = (0..self.clocks.len())
                .filter(|&i| steps[i] < self.budget && self.clocks[i].2 + self.clocks[i].1 <= self.time + 1e-9)
                .min_by(|&a, &b| (self.clocks[a].2 + self.clocks[a].1).total_cmp(&(self.clocks[b].2 + self.clocks[b].1)).then(a.cmp(&b)));
            let Some(i) = next else { break };
            let (k, every, done) = &mut self.clocks[i];
            *done += *every;
            w.step(*k, *done, *every);
            steps[i] += 1;
        }
        self.clocks.iter().zip(steps).map(|(c, n)| (c.0, n)).collect()
    }

    /// World time jumps to `t` (a timeline scrubbed): no steps between, every clock done to `t`.
    pub fn scrub(&mut self, t: f64) {
        self.time = t;
        for c in &mut self.clocks {
            c.2 = t;
        }
    }
}

#[cfg(test)]
mod runtime_tests {
    use super::*;
    use ClockKey::*;

    #[derive(Default)]
    struct Log(Vec<(ClockKey, f64)>);
    impl Clocked for Log {
        fn step(&mut self, k: ClockKey, t: f64, _: f64) {
            self.0.push((k, t));
        }
    }

    #[test]
    fn subscribed_clocks_step_in_time_order_within_budget() {
        // Only subscribed clocks step, at their record periods.
        let mut r = Runtime::new(&Subscriptions::of(&[Economy]), 1.0, 1000).unwrap();
        let mut log = Log::default();
        r.advance(35.0, &mut log);
        assert_eq!(log.0, vec![(Economy, 10.0), (Economy, 20.0), (Economy, 30.0)]);
        assert!(r.done_to(Realtime).is_none());
        assert!(Runtime::new(&Subscriptions::of(&[Rails]), 1.0, 1).is_err(), "an event clock isn't stepped by time");
        // At the same moment the slower clock steps first.
        let mut r = Runtime::with(vec![(Market, 1.0, 0.0), (Economy, 2.0, 0.0)], 1.0, 100).unwrap();
        let mut log = Log::default();
        r.advance(2.0, &mut log);
        assert_eq!(log.0, vec![(Market, 1.0), (Economy, 2.0), (Market, 2.0)]);
        // Over budget, steps wait (not dropped); scale 0 stands still; scrub jumps.
        let mut r = Runtime::with(vec![(Realtime, 0.25, 0.0)], 1.0, 2).unwrap();
        let mut log = Log::default();
        r.advance(1.0, &mut log);
        assert_eq!((log.0.len(), r.pending(Realtime)), (2, 0.5));
        r.advance(0.0, &mut log);
        assert_eq!(log.0.len(), 4);
        r.scale = 0.0;
        r.advance(10.0, &mut log);
        assert_eq!(log.0.len(), 4);
        r.scrub(100.0);
        r.scale = 1.0;
        r.advance(0.25, &mut log);
        assert_eq!(log.0.last(), Some(&(Realtime, 100.25)));
    }
}
