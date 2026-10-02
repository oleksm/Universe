//! The physics sheet: the game's dogma as numbers (see `config/physics.ron`,
//! and the charter, `docs/physics.md`). Its constants are generated from the
//! sheet at build time; the world's code uses them from here.

/// What kind of thing a constant is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// Measured nature.
    Real,
    /// Real physics, speculative engineering.
    Grounded,
    /// An approximation of the real, to refine.
    Simplified,
    /// The hyper layer: beyond known physics, by written rule.
    Invented,
    /// A designer's choice within the rules.
    Tuning,
    /// In the charter, not yet used by the code.
    Planned,
}

/// One constant of the sheet.
#[derive(Clone, Copy, Debug)]
pub struct Entry {
    pub section: &'static str,
    pub name: &'static str,
    pub value: f64,
    pub unit: &'static str,
    pub kind: Kind,
    pub note: &'static str,
}

include!(concat!(env!("OUT_DIR"), "/sheet.rs"));
