//! Saving and restoring a game: the player's ship and its avionics, the
//! clock, the player's credits and hold, and what's been dug out of
//! asteroids. Star systems regenerate from the seed. The settlers aren't
//! saved: loading keeps the crafts already flying (none, if the seed changes).
//!
//! A save records its `version` and the content it was made with (its
//! hash). Content is stored by key (goods, hulls), so saves survive content
//! growing; older saves are read through `SaveRecord`, which takes every
//! earlier form.

use serde::{Deserialize, Serialize};
use universe_avionics::route::Route;
use universe_avionics::{Avionics, Clearance, NavTarget};
use universe_world::Ship;

use crate::universe::Universe;

/// Everything needed to restore a game. Star systems regenerate from the seed.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(from = "SaveRecord")]
pub struct UniverseSave {
    /// The save format's version (`SAVE_VERSION` when written).
    pub version: u32,
    /// The hash of the content it was made with (0: not recorded).
    pub content: u64,
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
    pub hold: Vec<(GoodsRef, u32)>,
    /// What's been dug out of asteroids: ((system, field, rock), kg).
    #[serde(default)]
    pub mined: Vec<((usize, usize, usize), f64)>,
}

/// The save format's version: 1, content by key (0: goods by catalogue position).
pub const SAVE_VERSION: u32 = 1;

/// A good in a save: by key; saves before keys had its place in the catalogue.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum GoodsRef {
    Key(String),
    Index(usize),
}

fn starting_credits() -> f64 {
    crate::universe::STARTING_CREDITS
}

/// A save as stored. Saves from before the avionics had their own record kept
/// the nav state inside the ship; those fields are picked up so an old game
/// resumes with its target, clearance and hyperdrive autopilot.
#[derive(Deserialize)]
struct SaveRecord {
    #[serde(default)]
    version: u32,
    #[serde(default)]
    content: u64,
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
    #[serde(default)]
    hold: Vec<(GoodsRef, u32)>,
    #[serde(default)]
    mined: Vec<((usize, usize, usize), f64)>,
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
        UniverseSave {
            version: r.version,
            content: r.content,
            seed: r.seed,
            time: r.time,
            ship: r.ship.ship,
            ship_system: r.ship_system,
            route: r.route,
            avionics,
            credits: r.credits,
            hold: r.hold,
            mined: r.mined,
        }
    }
}

/// A save's ship whose hull (or a fitted module) is gone from the content
/// loads as the starting hull, stock fit: what's gone is lost, the rest kept.
pub fn forget_missing(save: &mut serde_json::Value) {
    let c = universe_world::content::content();
    let Some(ship) = save.get_mut("ship").and_then(|s| s.as_object_mut()) else { return };
    let hull_gone = ship.get("class").and_then(|k| k.as_str()).is_some_and(|k| c.handle::<universe_world::ship::ClassSpec>(k).is_none());
    let fit_gone = ship.get("fit").is_some_and(|f| {
        let mut keys = Vec::new();
        strings(f, &mut keys);
        keys.iter().any(|k| k.contains('.') && c.handle::<universe_world::modules::Module>(k).is_none() && !k.starts_with("slot"))
    });
    if hull_gone {
        ship.remove("class");
    }
    if hull_gone || fit_gone {
        ship.remove("fit");
    }
}

fn strings(v: &serde_json::Value, out: &mut Vec<String>) {
    match v {
        serde_json::Value::String(s) => out.push(s.clone()),
        serde_json::Value::Array(a) => a.iter().for_each(|x| strings(x, out)),
        serde_json::Value::Object(o) => o.values().for_each(|x| strings(x, out)),
        _ => {}
    }
}

impl Universe {
    pub fn save(&self) -> UniverseSave {
        self.save_with(self.avionics().clone())
    }

    /// A save, with the player's avionics as the client's cockpit has them.
    pub fn save_with(&self, avionics: Avionics) -> UniverseSave {
        UniverseSave {
            version: SAVE_VERSION,
            content: universe_world::content::content().hash(),
            seed: self.world.galaxy.seed,
            time: self.world.time,
            ship: self.ship.clone(),
            ship_system: self.ship_system,
            route: avionics.route.clone(),
            avionics,
            credits: self.credits(),
            hold: self.hold().into_iter().map(|(good, units)| (GoodsRef::Key(self.world.goods[good].key.clone()), units)).collect(),
            mined: {
                let mut m: Vec<_> = self.world.mined.iter().map(|(&k, &v)| (k, v)).collect();
                m.sort_by_key(|e| e.0);
                m
            },
        }
    }

    pub fn load(&mut self, save: UniverseSave) {
        if save.seed != self.world.galaxy.seed {
            *self = Universe::new(save.seed);
        }
        self.world.time = save.time;
        self.ship = save.ship;
        self.ship.refresh();
        self.ship_system = save.ship_system.min(self.world.galaxy.stars.len() - 1);
        // Docked at a station but off its pads (saved when stations were
        // otherwise): onto its middle pad.
        if let universe_world::ShipState::Landed { body, local_position, .. } = self.ship.state
            && self.world.system(self.ship_system).bodies.get(body).is_some_and(|b| b.kind == universe_world::BodyKind::Station)
            && self.ship.hangar.is_none()
            && universe_world::station::pad_at(local_position).is_none()
        {
            let placed = self.world.ship_on(self.ship_system, universe_world::Facility::Station(body), universe_world::spaceport::CENTER_PAD);
            (self.ship.position, self.ship.velocity, self.ship.orientation, self.ship.state) = (placed.position, placed.velocity, placed.orientation, placed.state);
        }
        if let Some(c) = self.player.as_mut().and_then(|p| p.as_any_mut().downcast_mut::<crate::cockpit::Cockpit>()) {
            *c.avionics_mut() = Avionics { route: save.route, ..save.avionics };
        }
        // The ledger takes the save's word for our credits and hold.
        use universe_services::{Asset, Party};
        let me = Party::Pilot(crate::combat::PLAYER);
        let (tick, cause) = (self.tick, universe_protocol::Cause::Rules);
        self.ledger.settle(me, Asset::Credits, save.credits, tick, cause);
        self.ledger.write_off(crate::combat::PLAYER, tick, cause);
        // (Goods the content no longer has are lost with it.)
        for (good, units) in &save.hold {
            let id = match good {
                GoodsRef::Key(k) => self.world.goods.iter().position(|i| &i.key == k),
                GoodsRef::Index(i) => (*i < self.world.goods.len()).then_some(*i),
            };
            if let Some(id) = id {
                self.ledger.settle(me, Asset::Goods(id), *units as f64, tick, cause);
            }
        }
        self.ship.cargo = universe_services::market::cargo_mass(&self.world.goods, &self.hold());
        self.ship.cargo_volume = universe_services::market::cargo_volume(&self.world.goods, &self.hold());
        self.world.mined = save.mined.into_iter().collect();
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
        // Something in the hold, and a rock dug into.
        use universe_services::{Asset, Party};
        let ore = universe_world::goods::Ore::Stony.item();
        u.ledger.settle(Party::Pilot(crate::combat::PLAYER), Asset::Goods(ore), 3.0, u.tick, universe_protocol::Cause::Rules);
        u.world.mined.insert((u.ship_system, 0, 1), 1500.0);
        let save = u.save();
        assert_eq!((save.version, save.content), (SAVE_VERSION, universe_world::content::content().hash()));
        let json = serde_json::to_string(&save).unwrap();
        assert!(json.contains("\"ore.stony\"") && json.contains("\"hull.drover\""), "content by key: {json}");
        let mut restored = Universe::new(7);
        restored.load(serde_json::from_str(&json).unwrap());
        assert_eq!(restored.world.time, u.world.time);
        assert_eq!(restored.ship.position, u.ship.position);
        assert_eq!(restored.ship_system, u.ship_system);
        assert_eq!(restored.avionics().nav_target, u.avionics().nav_target);
        assert_eq!(restored.hold(), vec![(ore, 3)], "the hold comes back");
        assert_eq!(restored.world.mined.get(&(u.ship_system, 0, 1)), Some(&1500.0), "and the dug rock");
    }

    #[test]
    fn a_save_from_before_keys_still_loads() {
        let u = Universe::new(7);
        let mut json: serde_json::Value = serde_json::to_value(u.save()).unwrap();
        // As written before: no version or content, goods by catalogue position, no hull.
        let o = json.as_object_mut().unwrap();
        o.remove("version");
        o.remove("content");
        o["ship"].as_object_mut().unwrap().remove("class");
        let ore = universe_world::goods::Ore::Pgm.item();
        o.insert("hold".into(), serde_json::json!([[ore, 2]]));
        let save: UniverseSave = serde_json::from_value(json).unwrap();
        assert_eq!(save.version, 0);
        let mut restored = Universe::new(7);
        restored.load(save);
        assert_eq!(restored.hold(), vec![(ore, 2)]);
        assert_eq!(restored.ship.class, universe_world::ship::starting_hull());
    }

    #[test]
    fn a_hull_gone_from_the_content_loads_as_the_starting_hull() {
        let u = Universe::new(42);
        let mut json = serde_json::to_value(u.save()).unwrap();
        json["ship"]["class"] = "hull.long_gone".into();
        assert!(serde_json::from_value::<UniverseSave>(json.clone()).is_err());
        forget_missing(&mut json);
        let save: UniverseSave = serde_json::from_value(json).unwrap();
        assert_eq!(save.ship.class, universe_world::ship::starting_hull());
    }

}
