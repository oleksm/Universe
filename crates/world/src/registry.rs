//! The registry (Freefall Facts, `standards/`): read and checked when the game
//! was built, carried in the binary, decoded once on first use. The records
//! are the truth; the world takes what it needs from them as it's made.

use std::sync::OnceLock;

pub use universe_registry::*;

pub(crate) static ENCODED: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/registry.bin"));

/// The registry the game was built with.
pub fn registry() -> &'static Registry {
    static REGISTRY: OnceLock<Registry> = OnceLock::new();
    REGISTRY.get_or_init(|| Registry::decode(ENCODED))
}
