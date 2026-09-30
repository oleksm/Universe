//! Saving and restoring a game: the player's ship and its avionics, and the
//! clock. Star systems regenerate from the seed. The settlers aren't saved:
//! loading keeps the crafts already flying (none, if the seed changes).

use serde::{Deserialize, Serialize};
use universe_avionics::route::Route;
use universe_avionics::{Avionics, Clearance, NavTarget};
use universe_world::Ship;

use crate::universe::Universe;

/// Everything needed to restore a game. Star systems regenerate from the seed.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(from = "SaveRecord")]
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

/// A save as stored. Saves from before the avionics had their own record kept
/// the nav state inside the ship; those fields are picked up so an old game
/// resumes with its target, clearance and hyperdrive autopilot.
#[derive(Deserialize)]
struct SaveRecord {
    seed: u64,
    time: f64,
    ship: ShipRecord,
    ship_system: usize,
    #[serde(default)]
    route: Route,
    #[serde(default)]
    avionics: Option<Avionics>,
}

#[derive(Deserialize)]
struct ShipRecord {
    #[serde(flatten)]
    ship: Ship,
    #[serde(default)]
    nav_target: Option<NavTarget>,
    #[serde(default)]
    clearance: Option<Clearance>,
    #[serde(default)]
    hyper_autopilot: bool,
}

impl From<SaveRecord> for UniverseSave {
    fn from(r: SaveRecord) -> Self {
        let avionics = r.avionics.unwrap_or_else(|| Avionics {
            nav_target: r.ship.nav_target,
            clearance: r.ship.clearance,
            hyper_autopilot: r.ship.hyper_autopilot,
            ..Avionics::default()
        });
        UniverseSave { seed: r.seed, time: r.time, ship: r.ship.ship, ship_system: r.ship_system, route: r.route, avionics }
    }
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

    /// Saves from before the refactor kept the nav state inside the ship.
    #[test]
    fn old_saves_keep_their_nav_state() {
        let mut u = Universe::new(7);
        let station = u.ship_system().station().unwrap();
        u.set_nav_target(Some(NavTarget::Station(station)));
        u.avionics.hyper_autopilot = true;
        let mut json: serde_json::Value = serde_json::to_value(u.save()).unwrap();
        let old = json.as_object_mut().unwrap();
        let avionics = old.remove("avionics").unwrap();
        let ship = old["ship"].as_object_mut().unwrap();
        ship.insert("nav_target".into(), avionics["nav_target"].clone());
        ship.insert("hyper_autopilot".into(), true.into());
        let mut restored = Universe::new(7);
        restored.load(serde_json::from_value(json).unwrap());
        assert_eq!(restored.avionics.nav_target, Some(NavTarget::Station(station)));
        assert!(restored.avionics.hyper_autopilot);
        assert_eq!(restored.ship.position, u.ship.position);
    }
}
