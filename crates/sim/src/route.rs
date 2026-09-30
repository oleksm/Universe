//! Multi-stop routes, flown by the route autopilot (see `Universe::route_step`).

use std::collections::VecDeque;

use serde::{Deserialize, Serialize};

use crate::ship::NavTarget;

/// A place to visit: a station (dock) or spaceport (land) in some star system.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Stop {
    /// Galaxy index of the star system.
    pub system: usize,
    pub target: NavTarget,
}

/// How long the route autopilot waits at each stop (game seconds).
pub const DWELL: f64 = 20.0;

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Route {
    pub stops: Vec<Stop>,
    /// Index of the stop we're heading for (or dwelling at).
    pub next: usize,
    /// The route autopilot is flying.
    pub active: bool,
    /// Waiting at a stop until this game time.
    pub dwell_until: Option<f64>,
    /// Just lifted off a surface: climbing before anything else.
    pub departing: bool,
}

impl Route {
    pub fn current(&self) -> Option<Stop> {
        self.stops.get(self.next).copied()
    }

    pub fn clear(&mut self) {
        *self = Route::default();
    }
}

/// Shortest chain of systems from `from` to `to` over gate links, excluding
/// `from` itself. Empty if they're the same system; None if unreachable.
pub fn gate_path(links: &[(usize, usize)], from: usize, to: usize) -> Option<Vec<usize>> {
    if from == to {
        return Some(Vec::new());
    }
    let mut prev = std::collections::HashMap::new();
    let mut queue = VecDeque::from([from]);
    prev.insert(from, from);
    while let Some(s) = queue.pop_front() {
        for &(a, b) in links {
            let n = if a == s { b } else if b == s { a } else { continue };
            if prev.contains_key(&n) {
                continue;
            }
            prev.insert(n, s);
            if n == to {
                let mut path = vec![to];
                let mut at = to;
                while prev[&at] != from {
                    at = prev[&at];
                    path.push(at);
                }
                path.reverse();
                return Some(path);
            }
            queue.push_back(n);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gate_paths() {
        let links = [(1, 2), (2, 3), (1, 4), (4, 5), (3, 5)];
        assert_eq!(gate_path(&links, 1, 1), Some(vec![]));
        assert_eq!(gate_path(&links, 1, 2), Some(vec![2]));
        assert_eq!(gate_path(&links, 1, 3), Some(vec![2, 3]));
        assert_eq!(gate_path(&links, 1, 5), Some(vec![4, 5]));
        assert_eq!(gate_path(&links, 1, 9), None);
    }
}
