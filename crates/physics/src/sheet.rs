//! A sheet of constants (Dogma's laws, `laws`; a world's fixed numbers,
//! `universe_world::sheet`): each with its unit, kind and reason.

/// What kind of thing a constant is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// Measured nature (a constant, a real material's property).
    Real,
    /// Real physics, speculative engineering.
    Grounded,
    /// An approximation of the real, to refine.
    Simplified,
    /// The hyper layer: beyond known physics, by written rule.
    Invented,
    /// A designer's choice within the rules.
    Tuning,
    /// A reference, not used by the code yet.
    Planned,
}

/// One constant of a sheet.
#[derive(Clone, Copy, Debug)]
pub struct Entry {
    pub section: &'static str,
    pub name: &'static str,
    pub value: f64,
    pub unit: &'static str,
    pub kind: Kind,
    pub note: &'static str,
}
