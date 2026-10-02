//! The charts: what's fixed about a galaxy once it's generated from its seed
//! — its stars, the gate network, the goods traded, and each star system as
//! generated (on first look, then kept). Read-only and shareable between
//! threads: the world engine and the client each look things up here.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use crate::galaxy::Galaxy;
use crate::goods::Item;
use crate::system::StarSystem;
use crate::units::LIGHT_YEAR;

pub struct Charts {
    pub seed: u64,
    pub galaxy: Galaxy,
    pub gate_links: Vec<(usize, usize)>,
    pub goods: Vec<Item>,
    pub home_system: usize,
    systems: Mutex<HashMap<usize, Arc<StarSystem>>>,
    /// Who holds each settled system (see `factions::territory`).
    territory: Vec<(usize, usize)>,
}

impl Charts {
    pub fn new(seed: u64, galaxy: Galaxy, gate_links: Vec<(usize, usize)>, goods: Vec<Item>, home_system: usize) -> Self {
        let territory = crate::factions::territory(seed, &gate_links, home_system, crate::content::content().factions.iter().count());
        Charts { seed, galaxy, gate_links, goods, home_system, systems: Mutex::new(HashMap::new()), territory }
    }

    /// The faction holding system `i` (None: nobody's).
    pub fn holder(&self, i: usize) -> Option<&'static crate::factions::Faction> {
        let k = self.territory.iter().find(|t| t.0 == i)?.1;
        crate::content::content().factions.iter().nth(k).map(|(_, f)| f)
    }

    /// The holder of system `i`, by its place among the factions (content order).
    pub fn holder_index(&self, i: usize) -> Option<usize> {
        self.territory.iter().find(|t| t.0 == i).map(|t| t.1)
    }

    /// Every held system, and who holds it.
    pub fn territory(&self) -> impl Iterator<Item = (usize, &'static crate::factions::Faction)> + '_ {
        self.territory.iter().filter_map(|&(s, _)| Some((s, self.holder(s)?)))
    }

    /// Star system `i`, generated on first look (with its gates).
    pub fn system(&self, i: usize) -> Arc<StarSystem> {
        let mut cache = self.systems.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(s) = cache.get(&i) {
            return s.clone();
        }
        let sys = Arc::new(generate(&self.galaxy, &self.gate_links, i));
        cache.insert(i, sys.clone());
        sys
    }

    /// The gate links of star `i`: where each goes, and its name.
    pub fn gate_links_of(&self, i: usize) -> Vec<(usize, String)> {
        crate::network::links_of(&self.gate_links, &self.galaxy, i)
    }

    /// Distance in light years between two stars.
    pub fn distance_ly(&self, a: usize, b: usize) -> f64 {
        self.galaxy.offset(a, b).length() / LIGHT_YEAR
    }
}

/// Star system `i` of `galaxy`, with the gates of the network.
pub fn generate(galaxy: &Galaxy, gate_links: &[(usize, usize)], i: usize) -> StarSystem {
    let star = &galaxy.stars[i];
    let mut sys = StarSystem::generate(i, star);
    let links = crate::network::links_of(gate_links, galaxy, i);
    if !links.is_empty() {
        // Each gate faces the star it leads to.
        let links: Vec<(usize, String, glam::DVec3)> = links.into_iter().map(|(d, n)| (d, n, galaxy.offset(i, d))).collect();
        sys.add_gates(&links, star.seed);
    }
    sys
}
