//! The world's institutions: they read what the core reports (its events,
//! by tick and number) and the requests made to them, decide their own
//! domain, and record why — every authoritative change carries its `Cause`
//! (see `docs/rearchitecture.md` §5).

pub mod atc;
pub mod economy;
pub mod land;
pub mod lots;
pub mod law;
pub mod ledger;
pub mod market;
pub mod outfitter;
pub mod records;

pub use atc::{Board, PadGrant, Presence, TrafficControl};
pub use law::{Law, Ruling, AGGRESSION};
pub use ledger::{Asset, Ledger, Party};
pub use market::{Markets, Order};
pub use records::{Deal, Kill, Records, TradeRecord, TrafficStats};
