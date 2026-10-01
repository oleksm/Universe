//! The world's institutions: they read what the core reports (its events,
//! by tick and number) and the requests made to them, decide their own
//! domain, and record why — every authoritative change carries its `Cause`
//! (see `docs/rearchitecture.md` §5).

pub mod law;

pub use law::{Law, Ruling, AGGRESSION};
