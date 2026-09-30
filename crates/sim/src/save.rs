//! Saving and restoring a game: the player's ship and its avionics, and the
//! clock. Star systems regenerate from the seed. The settlers aren't saved:
//! loading keeps the crafts already flying (none, if the seed changes).

use serde::{Deserialize, Serialize};
use universe_avionics::route::Route;
use universe_avionics::Avionics;
use universe_world::Ship;

use crate::universe::Universe;

/// Everything needed to restore a game. Star systems regenerate from the seed.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct UniverseSave {
    pub seed: u64,
    pub time: f64,
    pub ship: Ship,
    pub ship_system: usize,
    #[serde(default)]
    pub route: Route,
    #[serde(default)]
    pub avionics: Avionics,
}

impl Universe {
    pub fn save(&self) -> UniverseSave {
        UniverseSave {
            seed: self.world.galaxy.seed,
            time: self.world.time,
            ship: self.ship.clone(),
            ship_system: self.ship_system,
            route: self.avionics.route.clone(),
            avionics: self.avionics.clone(),
        }
    }

    pub fn load(&mut self, save: UniverseSave) {
        if save.seed != self.world.galaxy.seed {
            *self = Universe::new(save.seed);
        }
        self.world.time = save.time;
        self.ship = save.ship;
        self.ship_system = save.ship_system.min(self.world.galaxy.stars.len() - 1);
        self.avionics = Avionics { route: save.route, ..save.avionics };
        self.events.clear();
    }
}

#[cfg(test)]
mod tests {
    use universe_avionics::NavTarget;
    use universe_world::Controls;

    use super::*;

    #[test]
    fn save_round_trips_through_json() {
        let mut u = Universe::new(7);
        u.ship.throttle = 0.5;
        let station = u.ship_system().station().unwrap();
        u.set_nav_target(Some(NavTarget::Station(station)));
        for _ in 0..60 {
            u.step(1.0 / 60.0, 100.0, &Controls::default());
        }
        let json = serde_json::to_string(&u.save()).unwrap();
        let mut restored = Universe::new(7);
        restored.load(serde_json::from_str(&json).unwrap());
        assert_eq!(restored.world.time, u.world.time);
        assert_eq!(restored.ship.position, u.ship.position);
        assert_eq!(restored.ship_system, u.ship_system);
        assert_eq!(restored.avionics.nav_target, u.avionics.nav_target);
    }
}
