//! The outfitter: what modules a station's shipyard carries, at what
//! price, and what fitting one takes from the place's stock. All from the
//! seed and the charts (local knowledge, worked out the same by anyone):
//!
//! - a brand's **home** is a settled system with a station, picked by its key
//!   and the galaxy's seed; there, its whole range is carried;
//! - farther off (in gate hops), a module is carried less often
//!   (`CARRIED_PER_HOP` a hop, down to `CARRIED_FAR`) and costs more
//!   (`MARKUP_PER_HOP` a hop: shipping); unbranded modules are made anywhere;
//! - a module is **made of** goods — machinery, or electronics for the
//!   computing kind — by its mass: fitting one draws that from the station's
//!   stock (none in stock, none to fit), and one taken out puts half back.

use universe_world::goods::Category;
use universe_world::modules::{Module, SlotKind};
use universe_world::rng::{mix, Rng};
use universe_world::Facility;

/// The chance a module is carried falls by this a hop from its brand's home…
pub const CARRIED_PER_HOP: f64 = 0.8;
/// …to no less than this.
pub const CARRIED_FAR: f64 = 0.15;
/// Its price rises by this share a hop (shipping).
pub const MARKUP_PER_HOP: f64 = 0.05;

/// FNV-1a: the same everywhere.
fn hash(s: &str) -> u64 {
    s.bytes().fold(0xcbf2_9ce4_8422_2325, |h, b| (h ^ b as u64).wrapping_mul(0x100_0000_01b3))
}

/// Brand `brand`'s home among the settled systems with a station (`settled`,
/// sorted), for the galaxy `seed`.
pub fn brand_home(seed: u64, brand: &str, settled: &[usize]) -> Option<usize> {
    (!settled.is_empty()).then(|| settled[(mix(seed, hash(brand)) % settled.len() as u64) as usize])
}

/// Gate hops from `from` to `to` (None: not linked).
pub fn hops(links: &[(usize, usize)], from: usize, to: usize) -> Option<usize> {
    if from == to {
        return Some(0);
    }
    let mut seen = std::collections::HashSet::from([from]);
    let mut frontier = vec![from];
    for d in 1.. {
        let mut next = Vec::new();
        for &s in &frontier {
            for &(a, b) in links {
                let n = if a == s { b } else if b == s { a } else { continue };
                if n == to {
                    return Some(d);
                }
                if seen.insert(n) {
                    next.push(n);
                }
            }
        }
        if next.is_empty() {
            return None;
        }
        frontier = next;
    }
    None
}

/// How a station stands to a module: carried or not, and its price there.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Offer {
    pub carried: bool,
    pub price: f64,
    /// Gate hops from its brand's home (0: made here, or unbranded).
    pub hops: usize,
}

/// Module `m` at station `station` of system `system`.
pub fn offer(seed: u64, links: &[(usize, usize)], settled: &[usize], system: usize, station: Facility, m: &Module) -> Offer {
    let hops = if m.brand.is_empty() { 0 } else { brand_home(seed, &m.brand, settled).and_then(|home| hops(links, home, system)).unwrap_or(12) };
    let chance = if hops == 0 { 1.0 } else { CARRIED_PER_HOP.powi(hops as i32).max(CARRIED_FAR) };
    let key = match station {
        Facility::Station(i) | Facility::Spaceport(i) | Facility::Gate(i) | Facility::Asteroid(i) => i as u64,
    };
    let mut rng = Rng::new(mix(mix(seed, 0x0f17_7e45 + system as u64), key ^ hash(&m.key)));
    Offer { carried: rng.range(0.0, 1.0) < chance, price: m.price * (1.0 + MARKUP_PER_HOP * hops as f64), hops }
}

/// What module `m` is made of: (kind of goods, tonnes).
pub fn materials(m: &Module) -> (Category, f64) {
    let electronic = matches!(m.does.slot(), SlotKind::Computer | SlotKind::Transponder | SlotKind::Sensors | SlotKind::Comm | SlotKind::Avionics);
    let kind = if electronic { "goods.electronics" } else { "goods.machinery" };
    (Category::of(kind).expect("machinery and electronics are kinds of goods"), m.mass / 1000.0)
}

/// The settled systems with a station, sorted (where brands make their home).
pub fn settled(places: &[crate::economy::Place]) -> Vec<usize> {
    let mut s: Vec<usize> = places.iter().filter(|p| matches!(p.facility, Facility::Station(_))).map(|p| p.system).collect();
    s.sort_unstable();
    s.dedup();
    s
}
