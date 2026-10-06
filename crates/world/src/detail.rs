//! The ground's runtime detail (layer C of the lab's terrain relief plan): ~150 m down to ~1 m,
//! grown where it's wanted from the ~150 m heights and their fields (flow, drainage, the rock's
//! threshold slope, ice), the rock and the world's seed. One pure function: this one, in f64, is
//! the truth the physics stands on; its WGSL twin (`engine/src/shaders/detail.wgsl`) is held to
//! it within `AGREE_M` by the engine's test. The generator is the lab's; until it lands this is a
//! stand-in that adds nothing (`ACTIVE` false: the ground patches go no finer than before).

/// The generator is in (the patches go to its finest levels, the physics reads it).
pub const ACTIVE: bool = false;

/// The most the WGSL twin may differ from this (m).
pub const AGREE_M: f64 = 0.05;

/// The band the physics reads the detail at (m): the finest cells.
pub const PHYSICS_CELL_M: f64 = 0.5;

/// Where the detail is wanted, and what it's grown from.
pub struct Site<'a> {
    /// The place within its ~150 m tile (m from the tile's origin, along its u and v): the twin
    /// works in the same, so f32 keeps it to millimetres.
    pub at: [f64; 2],
    /// The tile's sample spacing there (m).
    pub spacing: f64,
    /// The ~150 m heights by sample (m, as the ground reads them), indices running two samples
    /// past the tile's edges into its neighbours (the halo).
    pub height: &'a dyn Fn(i64, i64) -> f64,
    /// The fields by their own samples (`fd150`, half as many a side), raw 0..255: flow
    /// direction, drainage area, the rock's threshold slope, ice; with the same halo.
    pub fields: &'a dyn Fn(i64, i64) -> [f64; 4],
    /// The rock unit under it (the 5 km rock map's number).
    pub rock: u32,
    /// The world's seed.
    pub seed: u64,
    /// The band: what's finer than about two of these cells is left out (m), so a coarse patch
    /// gets the smoothed ground and the physics `PHYSICS_CELL_M`.
    pub cell: f64,
}

/// The height the detail adds at `site` (m) over the ~150 m ground read bilinearly.
pub fn offset(_site: &Site) -> f64 {
    0.0
}

/// The world's seed from its id (`TRD1`): the same in every build (FNV-1a).
pub fn seed(world_id: &str) -> u64 {
    world_id.bytes().fold(0xcbf2_9ce4_8422_2325, |h, b| (h ^ b as u64).wrapping_mul(0x0100_0000_01b3))
}
