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
    /// The player's money, and what's in the hold (good, units).
    #[serde(default = "starting_credits")]
    pub credits: f64,
    #[serde(default)]
    pub hold: Vec<(usize, u32)>,
}

fn starting_credits() -> f64 {
    crate::universe::STARTING_CREDITS
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
    #[serde(default = "starting_credits")]
    credits: f64,
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
        UniverseSave { seed: r.seed, time: r.time, ship: r.ship.ship, ship_system: r.ship_system, route: r.route, avionics, credits: r.credits, hold: Vec::new() }
    }
}

impl Universe {
    pub fn save(&self) -> UniverseSave {
        self.save_with(self.avionics().clone())
    }

    /// A save, with the player's avionics as the client's cockpit has them.
    pub fn save_with(&self, avionics: Avionics) -> UniverseSave {
        UniverseSave {
            seed: self.world.galaxy.seed,
            time: self.world.time,
            ship: self.ship.clone(),
            ship_system: self.ship_system,
            route: avionics.route.clone(),
            avionics,
            credits: self.credits(),
            hold: self.hold(),
        }
    }

    pub fn load(&mut self, save: UniverseSave) {
        if save.seed != self.world.galaxy.seed {
            *self = Universe::new(save.seed);
        }
        self.world.time = save.time;
        self.ship = save.ship;
        self.ship_system = save.ship_system.min(self.world.galaxy.stars.len() - 1);
        if let Some(c) = &mut self.cockpit {
            *c.avionics_mut() = Avionics { route: save.route, ..save.avionics };
        }
        // The ledger takes the save's word for our credits and hold.
        use universe_services::{Asset, Party};
        let me = Party::Pilot(crate::combat::PLAYER);
        let (tick, cause) = (self.tick, universe_protocol::Cause::Rules);
        self.ledger.settle(me, Asset::Credits, save.credits, tick, cause);
        self.ledger.write_off(crate::combat::PLAYER, tick, cause);
        for (good, units) in &save.hold {
            self.ledger.settle(me, Asset::Goods(*good), *units as f64, tick, cause);
        }
        self.ship.cargo = universe_services::market::cargo_mass(&self.world.goods, &self.hold());
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
            u.step_world(1.0 / 60.0, 100.0, &Controls::default());
        }
        let json = serde_json::to_string(&u.save()).unwrap();
        let mut restored = Universe::new(7);
        restored.load(serde_json::from_str(&json).unwrap());
        assert_eq!(restored.world.time, u.world.time);
        assert_eq!(restored.ship.position, u.ship.position);
        assert_eq!(restored.ship_system, u.ship_system);
        assert_eq!(restored.avionics().nav_target, u.avionics().nav_target);
    }

}
