//! The home system and the gate network linking it to its neighbours.

use std::collections::HashMap;

use crate::galaxy::{Galaxy, StarClass};
use crate::names::star_name;
use crate::rng::Rng;
use crate::system::StarSystem;

/// A sun-like star near the middle of the charted region with a station
/// round a rocky world (and room round it for neighbours to settle).
pub fn find_home(galaxy: &Galaxy, seed: u64) -> usize {
    let mut rng = Rng::new(seed ^ 0x686f_6d65);
    let n = galaxy.stars.len();
    for _ in 0..100_000 {
        let i = (rng.next_u64() % n as u64) as usize;
        let s = &galaxy.stars[i];
        if !matches!(s.class, StarClass::G | StarClass::K) || s.position.distance(crate::galaxy::REGION_CENTRE) > crate::galaxy::REGION * 0.15 {
            continue;
        }
        let sys = StarSystem::generate(i, s);
        // (An Earth-like home world: a starter ship lands on it, about 1 g at most.)
        let earthlike = sys.station().and_then(|st| sys.bodies[st].rail.parent).is_some_and(|p| sys.bodies[p].rail.mu / sys.bodies[p].rail.radius.powi(2) < 11.0);
        if earthlike && sys.planet_count() >= 4 {
            return i;
        }
    }
    0
}

/// Link the home system and its 4 nearest neighbours with gates: a
/// spanning tree (each system to the nearest already-linked one), plus up
/// to two extra short links for loops. Every system gets 1-3 gates.
pub fn build(galaxy: &Galaxy, home: usize) -> Vec<(usize, usize)> {
    const MAX_GATES: usize = 3;
    let mut nodes = vec![home];
    nodes.extend(galaxy.nearest(home, 4));
    let dist = |a: usize, b: usize| galaxy.stars[a].position.distance(galaxy.stars[b].position);
    let mut degree: HashMap<usize, usize> = HashMap::new();
    let mut links: Vec<(usize, usize)> = Vec::new();
    for k in 1..nodes.len() {
        let n = nodes[k];
        let best = nodes[..k]
            .iter()
            .copied()
            .filter(|m| degree.get(m).copied().unwrap_or(0) < MAX_GATES)
            .min_by(|&a, &b| dist(a, n).total_cmp(&dist(b, n)))
            .unwrap_or(nodes[0]);
        links.push((best, n));
        *degree.entry(best).or_default() += 1;
        *degree.entry(n).or_default() += 1;
    }
    let mut pairs: Vec<(usize, usize)> =
        nodes.iter().flat_map(|&a| nodes.iter().filter(move |&&b| b > a).map(move |&b| (a, b))).collect();
    pairs.sort_by(|p, q| dist(p.0, p.1).total_cmp(&dist(q.0, q.1)));
    let mut extra = 0;
    for (a, b) in pairs {
        let linked = links.iter().any(|&(x, y)| (x, y) == (a, b) || (y, x) == (a, b));
        let room = |n: usize| degree.get(&n).copied().unwrap_or(0) < MAX_GATES;
        if extra < 2 && !linked && room(a) && room(b) {
            links.push((a, b));
            *degree.entry(a).or_default() += 1;
            *degree.entry(b).or_default() += 1;
            extra += 1;
        }
    }
    links
}

/// Systems linked to `i` by gates, with their names.
pub fn links_of(links: &[(usize, usize)], galaxy: &Galaxy, i: usize) -> Vec<(usize, String)> {
    links
        .iter()
        .filter_map(|&(a, b)| if a == i { Some(b) } else if b == i { Some(a) } else { None })
        .map(|j| (j, star_name(galaxy.stars[j].seed)))
        .collect()
}

#[cfg(test)]
mod tests {
    use crate::system::BodyKind;
    use crate::World;

    #[test]
    fn gate_network_links_five_systems_with_one_to_three_gates_each() {
        let w = World::new(1984);
        let mut systems: Vec<usize> = w.gate_links.iter().flat_map(|&(a, b)| [a, b]).collect();
        systems.sort();
        systems.dedup();
        assert_eq!(systems.len(), 5, "links: {:?}", w.gate_links);
        assert!(systems.contains(&w.home_system));
        for &s in &systems {
            let links = w.gate_links_of(s);
            assert!((1..=3).contains(&links.len()), "system {s} has {} gates", links.len());
            let sys = w.system(s);
            for (to, _) in &links {
                let g = sys.gate_to(*to).expect("a gate for every link");
                assert_eq!(sys.bodies[g].kind, BodyKind::Gate);
            }
        }
        // Connected: walk from home.
        let mut seen = vec![w.home_system];
        let mut i = 0;
        while i < seen.len() {
            for (n, _) in w.gate_links_of(seen[i]) {
                if !seen.contains(&n) {
                    seen.push(n);
                }
            }
            i += 1;
        }
        assert_eq!(seen.len(), 5, "network must be connected");
        // Every settled system held; the first faction at home; every faction holding some.
        let n = crate::content::content().factions.iter().count();
        let held = crate::factions::territory(w.galaxy.seed, &w.gate_links, w.home_system, n);
        assert_eq!(held.len(), 5);
        assert!(held.iter().any(|&(s, k)| s == w.home_system && k == 0), "the first faction at home");
        for k in 0..n {
            assert!(held.iter().any(|h| h.1 == k), "faction {k} holds nothing");
        }
    }
}
