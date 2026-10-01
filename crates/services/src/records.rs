//! Records: what happened, kept for anyone to read — kills (with the event
//! each came of), trades and trading decisions, and traffic's totals. An
//! observer of the core's events and the services' decisions; it decides
//! nothing.

use universe_protocol::Cause;

/// Kills kept, and trades.
pub const KILLS: usize = 50;
pub const TRADES: usize = 100;

/// A ship destroyed by weapons fire: who fired the last hit, at whom, with
/// what ("GUNFIRE", "LASER FIRE"), where and when.
#[derive(Clone, Debug)]
pub struct Kill {
    pub time: f64,
    pub system: usize,
    pub killer: usize,
    pub victim: usize,
    pub killer_name: String,
    pub victim_name: String,
    pub weapon: String,
    /// What it came of: the wreck, as the core logged it.
    pub cause: Cause,
}

/// What a trade record is.
#[derive(Clone, Debug, PartialEq)]
pub enum Deal {
    Bought,
    Sold,
    /// Heading for another market here, expecting this profit (credits).
    Heading { to: String, expect: f64 },
    /// Nothing worth it here: moving on to another system.
    MovingOn { to: String },
}

/// One trade (or trading decision): who bought or sold what, where, for how
/// much, and how they stood after it.
#[derive(Clone, Debug)]
pub struct TradeRecord {
    pub time: f64,
    pub system: usize,
    pub market: String,
    pub trader: String,
    pub deal: Deal,
    pub bought: bool,
    pub item: String,
    pub units: u32,
    /// Credits paid (bought) or received (sold).
    pub amount: f64,
    /// Cargo aboard after it (kg), and credits.
    pub cargo: f64,
    pub credits: f64,
}

/// Totals across all crafts.
#[derive(Clone, Copy, Debug, Default)]
pub struct TrafficStats {
    pub stops: u64,
    pub transits: u64,
    pub crashes: u64,
    pub routes_completed: u64,
    /// Crafts destroyed by weapons fire.
    pub shot_down: u64,
    /// Hunts by pirates: begun, and ended in a kill.
    pub hunts: u64,
    pub pirate_kills: u64,
    /// Lawful ships taking on an aggressor (each ship counted), and aggressors they shot down.
    pub defences: u64,
    pub aggressors_downed: u64,
    /// Ship-to-ship collisions (each ship's side counted), and ships wrecked by them.
    pub collisions: u64,
    pub collision_losses: u64,
    /// Trades by settlers, and the credits that changed hands.
    pub trades: u64,
    pub turnover: f64,
}

/// The records.
#[derive(Clone, Debug, Default)]
pub struct Records {
    pub kills: Vec<Kill>,
    pub trades: Vec<TradeRecord>,
    pub stats: TrafficStats,
}

impl Records {
    pub fn kill(&mut self, k: Kill) {
        self.kills.push(k);
        let excess = self.kills.len().saturating_sub(KILLS);
        self.kills.drain(..excess);
    }

    pub fn trade(&mut self, t: TradeRecord) {
        self.trades.push(t);
        let excess = self.trades.len().saturating_sub(TRADES);
        self.trades.drain(..excess);
    }
}
