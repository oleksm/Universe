//! Traffic control's replies to pilots.

/// What traffic control says to a request for a landing pad.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PadGrant {
    /// This pad is yours.
    Pad(usize),
    /// None free: you're waiting, with this many ahead of you.
    Queued(usize),
}
