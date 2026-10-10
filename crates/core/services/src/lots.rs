//! A good's lots (SFO 21) where it lies, oldest first: a pool's or a hold's. The same two moves
//! wherever stock goes: pieces in (a piece of a lot already there joins it), and kilograms out,
//! the oldest lots first.

use std::collections::VecDeque;
use universe_world::registry::Batch;

/// The lots of one good somewhere, oldest first, with their kilograms.
pub type Lots = VecDeque<(Batch, f64)>;

/// `pieces` in: a piece of a lot already here joins it, a new lot goes last.
pub fn put(lots: &mut Lots, pieces: Vec<(Batch, f64)>) {
    for (b, kg) in pieces {
        match lots.iter_mut().find(|(x, _)| x.lot == b.lot) {
            Some((_, have)) => *have += kg,
            None => lots.push_back((b, kg)),
        }
    }
}

/// Up to `kg` out, the oldest lots first: the pieces taken.
pub fn take(lots: &mut Lots, kg: f64) -> Vec<(Batch, f64)> {
    let mut out = Vec::new();
    let mut want = kg;
    while want > 1e-9 {
        let Some((b, have)) = lots.front_mut() else { break };
        let k = want.min(*have);
        *have -= k;
        want -= k;
        out.push((b.clone(), k));
        if *have <= 1e-9 {
            lots.pop_front();
        }
    }
    out
}
