//! Factions (content: `factions.ron`; `docs/factions.md`): the powers of
//! the seeded world, and the territory each starts with. A faction holds a
//! system when it owns its station, ports and gates; there it keeps its law
//! (to come). The first faction sits at the home system; each other's seat
//! is a settled system as far by gates from the seats before it as can be
//! (picked from the seed among the farthest); every settled system goes to
//! the nearest seat. Territory changes hands later: nothing's held for good.

use serde::Deserialize;

/// A faction of the loaded content.
#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Faction {
    pub key: String,
    pub name: String,
    /// Three letters, for tight places.
    pub tag: String,
    /// Its colour on maps.
    pub color: [f32; 3],
    pub note: String,
}

/// Gate hops from `from` to every system it reaches (unreached: absent).
fn hops(links: &[(usize, usize)], from: usize) -> std::collections::HashMap<usize, usize> {
    let mut seen = std::collections::HashMap::from([(from, 0)]);
    let mut edge = vec![from];
    while !edge.is_empty() {
        let mut next = Vec::new();
        for &s in &edge {
            for &(a, b) in links {
                let other = if a == s { b } else if b == s { a } else { continue };
                if !seen.contains_key(&other) {
                    seen.insert(other, seen[&s] + 1);
                    next.push(other);
                }
            }
        }
        edge = next;
    }
    seen
}

/// Who holds each settled system at the start: (system, faction by content
/// order), by system. `factions`: how many there are.
pub fn territory(seed: u64, links: &[(usize, usize)], home: usize, factions: usize) -> Vec<(usize, usize)> {
    let mut systems: Vec<usize> = links.iter().flat_map(|&(a, b)| [a, b]).chain([home]).collect();
    systems.sort();
    systems.dedup();
    let away: Vec<_> = systems.iter().map(|&s| (s, hops(links, s))).collect();
    let far = |a: usize, b: usize| away.iter().find(|x| x.0 == a).and_then(|x| x.1.get(&b).copied()).unwrap_or(usize::MAX / 2);
    // The seats: home first, then each as far from those before as can be.
    let mut seats = vec![home];
    while seats.len() < factions.min(systems.len()) {
        let distance = |s: usize| seats.iter().map(|&t| far(s, t)).min().unwrap_or(0);
        let best = systems.iter().filter(|s| !seats.contains(s)).map(|&s| distance(s)).max().unwrap_or(0);
        let farthest: Vec<usize> = systems.iter().copied().filter(|s| !seats.contains(s) && distance(*s) == best).collect();
        seats.push(farthest[(crate::rng::mix(seed, seats.len() as u64) % farthest.len() as u64) as usize]);
    }
    systems.iter().map(|&s| (s, (0..seats.len()).min_by_key(|&k| far(s, seats[k])).unwrap_or(0))).collect()
}
