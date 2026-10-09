//! The home system and the gate network linking it to its neighbours.

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
        if !matches!(s.class, StarClass::G | StarClass::K) || s.position.distance(crate::galaxy::REGION_CENTRE) > crate::galaxy::charted().region * 0.15 {
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

/// The settled neighbours round home: one toward each of four even
/// directions round it (seen from above), near and near its plane; of the
/// ways to turn those four directions, the one with the tightest ring. Home
/// sits in the middle of its neighbourhood.
pub fn neighbours(galaxy: &Galaxy, home: usize) -> Vec<usize> {
    use std::f64::consts::{FRAC_PI_2, FRAC_PI_4, TAU};
    let at = galaxy.stars[home].position;
    let near = galaxy.nearest(home, 80);
    let mut best: Option<(f64, Vec<usize>)> = None;
    for turn in 0..18 {
        let turn = turn as f64 * 5f64.to_radians();
        let mut picked = Vec::new();
        let mut total = 0.0;
        for quarter in 0..4 {
            let centre = turn + quarter as f64 * FRAC_PI_2;
            let pick = near
                .iter()
                .filter_map(|&i| {
                    let d = galaxy.stars[i].position - at;
                    let off = (d.z.atan2(d.x) - centre + TAU + FRAC_PI_4).rem_euclid(TAU) - FRAC_PI_4;
                    // (Gates are short: 8 ly at most.)
                    (off.abs() <= FRAC_PI_4 && d.length() <= 8.0 && !picked.contains(&i)).then(|| (d.length() * (1.0 + off.abs()) + 1.5 * d.y.abs(), i))
                })
                .min_by(|a, b| a.0.total_cmp(&b.0));
            if let Some((cost, i)) = pick {
                total += cost;
                picked.push(i);
            } else if let Some(&i) = near.iter().find(|i| !picked.contains(i)) {
                // (Nothing near that way: the nearest left.)
                total += 100.0 + galaxy.stars[i].position.distance(at);
                picked.push(i);
            }
        }
        if best.as_ref().is_none_or(|b| total < b.0) {
            best = Some((total, picked));
        }
    }
    best.map(|b| b.1).unwrap_or_default()
}

/// Gate the home system to each of its neighbours (`neighbours`): home the
/// hub, plus the two shortest links between neighbours side by side for loops;
/// the hub's longest gate dropped, that neighbour reached through a loop.
pub fn build(galaxy: &Galaxy, home: usize) -> Vec<(usize, usize)> {
    let around = neighbours(galaxy, home);
    let dist = |a: usize, b: usize| galaxy.stars[a].position.distance(galaxy.stars[b].position);
    let mut links: Vec<(usize, usize)> = around.iter().map(|&n| (home, n)).collect();
    // (Loops between neighbours side by side round home, never across it.)
    let mut pairs: Vec<(usize, usize)> = (0..around.len()).map(|k| (around[k], around[(k + 1) % around.len()])).filter(|(a, b)| a != b).collect();
    pairs.sort_by(|p, q| dist(p.0, p.1).total_cmp(&dist(q.0, q.1)));
    links.extend(pairs.into_iter().take(2));
    // The hub's longest gate goes where a loop reaches that neighbour anyway:
    // it's reached through the next system (traffic and news chain through).
    let looped = |n: usize| links.iter().any(|&(a, b)| a != home && b != home && (a == n || b == n));
    if let Some(k) = (0..around.len()).filter(|&k| looped(links[k].1)).max_by(|&a, &b| dist(home, links[a].1).total_cmp(&dist(home, links[b].1))) {
        links.remove(k);
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
    fn gate_network_links_home_to_its_neighbours_each_with_one_to_three_gates() {
        let w = World::new(1984);
        let systems = w.settled();
        assert_eq!(systems.len(), 5, "links: {:?}", w.gate_links);
        assert!(systems.contains(&w.home_system));
        for &s in &systems {
            let links = w.gate_links_of(s);
            // (Home is the hub: a gate to each neighbour but the one reached through a loop.)
            let gates = if s == w.home_system { 3..=3 } else { 1..=3 };
            assert!(gates.contains(&links.len()), "system {s} has {} gates", links.len());
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
    }
}
