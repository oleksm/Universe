//! The developer's kit, apart from the game: named start-up scenarios (`UNIVERSE_SCENARIO`) and the
//! live observer port and recordings (SHIFT+F3, for whoever's helping). Built with the `dev` feature,
//! on by default; a player build leaves it out (`--no-default-features`), and `off` stands in with the
//! same names doing nothing, so the game's code reads the same either way.

#[cfg(feature = "dev")]
pub mod dev;
#[cfg(feature = "dev")]
pub mod observe;

#[cfg(not(feature = "dev"))]
mod off;
#[cfg(not(feature = "dev"))]
pub use off::{dev, observe};
