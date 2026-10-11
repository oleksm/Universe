//! Key-based inventory snapshots. Runtime catalogue indices never cross the save boundary.
use crate::universe::Universe;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use universe_protocol::{Cause, ShipId};
use universe_services::{
    economy::{Pool, Site, Works},
    packages::Unit,
    Asset, Party,
};
use universe_world::{
    goods::Item,
    registry::{Batch, Mark},
};

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Stock {
    /// Ledger units for a hold; kilograms for a works pool.
    pub quantity: f64,
    #[serde(default)]
    pub marks: Vec<Mark>,
    #[serde(default)]
    pub lots: Vec<(Batch, f64)>,
    #[serde(default)]
    pub packages: Vec<Unit>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Hold {
    pub pilot: usize,
    pub name: String,
    pub hull: String,
    pub stock: BTreeMap<String, Stock>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WorksSave {
    pub key: String,
    pub site: String,
    /// For unrecorded works only; recorded works match by key and site.
    pub slot: usize,
    pub stock: BTreeMap<String, Stock>,
    pub open_lots: BTreeMap<String, Batch>,
    pub progress: BTreeMap<String, f64>,
    pub parts_in: Vec<String>,
    pub deposit: Option<(String, f64)>,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct PackingSave {
    pub holds: Vec<Hold>,
    pub works: Vec<WorksSave>,
    pub sequences: BTreeMap<String, u32>,
    pub package_sequences: BTreeMap<String, u16>,
    pub stepped_to: f64,
}
fn by_key<T: Clone>(values: &BTreeMap<usize, T>, goods: &[Item]) -> BTreeMap<String, T> {
    values
        .iter()
        .filter_map(|(i, v)| Some((goods.get(*i)?.key.clone(), v.clone())))
        .collect()
}
fn by_index<T>(values: BTreeMap<String, T>, goods: &[Item]) -> BTreeMap<usize, T> {
    values
        .into_iter()
        .filter_map(|(k, v)| Some((goods.iter().position(|g| g.key == k)?, v)))
        .collect()
}
fn site(u: &Universe, w: &Works) -> String {
    match w.site {
        Site::Ground(n) => u.services.land.grounds.get(n).map_or_else(
            || format!("unrecorded-ground:{n}"),
            |g| g.recorded.key.clone(),
        ),
        Site::Rig(system, body) => format!("rig:{system}:{body}"),
    }
}
fn pool_save(pool: &Pool, goods: &[Item]) -> BTreeMap<String, Stock> {
    let keys: BTreeSet<_> = pool
        .stock
        .keys()
        .chain(pool.marks.keys())
        .chain(pool.lots.keys())
        .chain(pool.packages.keys())
        .copied()
        .collect();
    keys.into_iter()
        .filter_map(|i| {
            Some((
                goods.get(i)?.key.clone(),
                Stock {
                    quantity: pool.stock.get(&i).copied().unwrap_or(0.),
                    marks: pool.marks.get(&i).into_iter().flatten().cloned().collect(),
                    lots: pool.lots.get(&i).into_iter().flatten().cloned().collect(),
                    packages: pool
                        .packages
                        .get(&i)
                        .into_iter()
                        .flatten()
                        .cloned()
                        .collect(),
                },
            ))
        })
        .collect()
}
fn pool_load(pool: &mut Pool, stock: BTreeMap<String, Stock>, goods: &[Item]) {
    pool.stock.clear();
    pool.marks.clear();
    pool.lots.clear();
    pool.packages.clear();
    for (i, s) in by_index(stock, goods) {
        pool.stock.insert(i, s.quantity);
        if !s.marks.is_empty() {
            pool.marks.insert(i, s.marks.into());
        }
        if !s.lots.is_empty() {
            pool.lots.insert(i, s.lots.into());
        }
        if !s.packages.is_empty() {
            pool.packages.insert(i, s.packages.into());
        }
    }
}
impl PackingSave {
    pub(super) fn capture(u: &Universe) -> Self {
        let ledger = &u.services.ledger;
        let e = &u.services.markets.economy;
        let goods = &u.world.goods;
        Self {
            holds: u
                .vessels
                .iter()
                .map(|(id, v)| Hold {
                    pilot: id.0,
                    name: v.name.to_string(),
                    hull: v.ship.spec().key.clone(),
                    stock: ledger
                        .hold_quantities(id)
                        .into_iter()
                        .flatten()
                        .map(|(&i, &quantity)| {
                            (
                                goods[i].key.clone(),
                                Stock {
                                    quantity,
                                    marks: ledger
                                        .hold_marks(id)
                                        .and_then(|m| m.get(&i))
                                        .into_iter()
                                        .flatten()
                                        .cloned()
                                        .collect(),
                                    lots: ledger
                                        .hold_lots(id)
                                        .and_then(|m| m.get(&i))
                                        .into_iter()
                                        .flatten()
                                        .cloned()
                                        .collect(),
                                    packages: ledger
                                        .hold_packages(id)
                                        .and_then(|m| m.get(&i))
                                        .into_iter()
                                        .flatten()
                                        .cloned()
                                        .collect(),
                                },
                            )
                        })
                        .collect(),
                })
                .collect(),
            works: e
                .works
                .iter()
                .map(|w| WorksSave {
                    key: w.key.clone(),
                    site: site(u, w),
                    slot: w.works,
                    stock: pool_save(&w.pool, goods),
                    open_lots: by_key(&w.open_lots, goods),
                    progress: by_key(&w.progress, goods),
                    parts_in: w.parts_in.clone(),
                    deposit: w.deposit.clone(),
                })
                .collect(),
            sequences: e.sequences.clone(),
            package_sequences: e.package_sequences.clone(),
            stepped_to: e.stepped_to,
        }
    }
    pub(super) fn restore(self, u: &mut Universe) {
        let tick = u.tick;
        // Clear live inventories first: a package cannot remain on its newer carrier
        // while the same historical package returns to a saved warehouse.
        let ids: Vec<_> = u.vessels.iter().map(|(id, _)| id).collect();
        for &id in &ids {
            u.services.ledger.write_off(id, tick, Cause::Rules);
        }
        for h in self.holds {
            let id = ShipId(h.pilot);
            let matches = u.vessels.get(id).is_some_and(|v| {
                id == crate::combat::PLAYER
                    || (v.name.as_ref() == h.name && v.ship.spec().key == h.hull)
            });
            if !matches {
                continue;
            }
            for (i, s) in by_index(h.stock, &u.world.goods) {
                u.services.ledger.settle(
                    Party::Pilot(id),
                    Asset::Goods(i),
                    s.quantity,
                    tick,
                    Cause::Rules,
                );
                u.services
                    .ledger
                    .load_cargo(id, i, s.marks, s.lots, s.packages);
            }
        }
        let sites: Vec<_> = u
            .services
            .markets
            .economy
            .works
            .iter()
            .map(|w| site(u, w))
            .collect();
        let e = &mut u.services.markets.economy;
        // Works absent from the historical snapshot must not retain newer packages
        // while the global allocator returns to the saved point in time.
        for live in &mut e.works {
            pool_load(&mut live.pool, BTreeMap::new(), &u.world.goods);
            live.open_lots.clear();
            live.progress.clear();
            live.parts_in.clear();
            live.last = None;
        }
        for w in self.works {
            let Some(n) = e.works.iter().enumerate().position(|(n, live)| {
                live.key == w.key
                    && sites[n] == w.site
                    && (!w.key.is_empty() || live.works == w.slot)
            }) else {
                continue;
            };
            let live = &mut e.works[n];
            pool_load(&mut live.pool, w.stock, &u.world.goods);
            live.open_lots = by_index(w.open_lots, &u.world.goods);
            live.progress = by_index(w.progress, &u.world.goods);
            live.parts_in = w.parts_in;
            live.deposit = w.deposit;
            live.last = None;
        }
        e.sequences = self.sequences;
        e.package_sequences = self.package_sequences;
        e.stepped_to = self.stepped_to;
        e.invalidate_snapshots();
        for id in ids {
            u.vessels[id].ship.cargo = u.services.ledger.cargo_mass(id, &u.world.goods);
            u.vessels[id].ship.cargo_volume = u.services.ledger.cargo_volume(id, &u.world.goods);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{combat::PLAYER, vessel::Vessel};
    fn fixture() -> (Universe, usize, Unit) {
        let mut u = Universe::new(1984);
        let good = u
            .world
            .goods
            .iter()
            .position(|g| g.key == "stock.stony-ore-bulk")
            .unwrap();
        // Deliberately invented test package values, not production specifications.
        let batch = Batch {
            lot: "TEST-000001".into(),
            heat: None,
            made_at: "facility.test".into(),
            made_on: 10.,
        };
        let mut unit = Unit {
            batch: batch.clone(),
            product: u.world.goods[good].key.clone(),
            brand: "org.test".into(),
            template: "package.test".into(),
            net_mass: u.world.goods[good].mass,
            capacity: u.world.goods[good].mass,
            tare: 7.,
            volume: 5.,
            label: None,
        };
        unit.seal(20., &mut u.services.markets.economy.package_sequences);
        u.services
            .markets
            .economy
            .sequences
            .insert("TEST".into(), 1);
        u.services.ledger.settle(
            Party::Pilot(PLAYER),
            Asset::Goods(good),
            1.,
            u.tick,
            Cause::Rules,
        );
        u.services.ledger.load_cargo(
            PLAYER,
            good,
            vec![],
            vec![(batch, unit.net_mass)],
            vec![unit.clone()],
        );
        (u, good, unit)
    }
    fn json<T: Serialize>(x: T) -> serde_json::Value {
        serde_json::to_value(x).unwrap()
    }
    #[test]
    fn labels_lots_tare_and_high_water_counters_survive_repeated_load() {
        let (mut u, g, unit) = fixture();
        // No extant unit has suffix 9998: the high-water counter must still survive.
        u.services
            .markets
            .economy
            .package_sequences
            .insert(unit.batch.lot.clone(), 9998);
        let saved = serde_json::to_string(&u.save()).unwrap();
        let mut loaded = Universe::new(99);
        for _ in 0..2 {
            loaded.load(serde_json::from_str(&saved).unwrap());
            assert_eq!(
                json(loaded.services.ledger.hold_packages(PLAYER).unwrap()[&g].clone()),
                json(vec![unit.clone()])
            );
            assert_eq!(
                loaded.services.ledger.hold_lots(PLAYER).unwrap()[&g][0].1,
                unit.net_mass
            );
            assert_eq!(loaded.vessels[PLAYER].ship.cargo, unit.net_mass + 7.);
            assert_eq!(loaded.vessels[PLAYER].ship.cargo_volume, 5.);
            assert_eq!(
                loaded.services.markets.economy.package_sequences[&unit.batch.lot],
                9998
            );
            assert!(loaded.services.ledger.balanced());
        }
        let mut next = unit.clone();
        next.label = None;
        assert!(next.seal(30., &mut loaded.services.markets.economy.package_sequences));
        assert!(next.label.unwrap().serial.ends_with("-9999"));
        let mut exhausted = unit;
        exhausted.label = None;
        assert!(!exhausted.seal(31., &mut loaded.services.markets.economy.package_sequences));
    }
    #[test]
    fn works_partial_packages_and_keyed_inventory_restore_with_fresh_views() {
        let (mut u, g, mut unit) = fixture();
        unit.label = None;
        unit.net_mass /= 2.;
        let key = u.services.markets.economy.works[0].key.clone();
        let site_key = site(&u, &u.services.markets.economy.works[0]);
        let w = &mut u.services.markets.economy.works[0];
        w.pool.stock.clear();
        w.pool.lots.clear();
        w.pool.marks.clear();
        w.pool.packages.clear();
        w.pool.put_cargo(
            g,
            unit.net_mass,
            vec![],
            vec![(unit.batch.clone(), unit.net_mass)],
            vec![unit.clone()],
        );
        w.open_lots.insert(g, unit.batch.clone());
        w.progress.insert(g, 12.5);
        w.parts_in.push("part-test".into());
        let mut save = u.save();
        save.packing.as_mut().unwrap().works.reverse();
        u.services.markets.economy.works[0]
            .pool
            .stock
            .insert(g, 9000.);
        u.services.markets.economy.works_snapshot();
        u.load(save);
        let w = u
            .services
            .markets
            .economy
            .works
            .iter()
            .find(|w| w.key == key && site(&u, w) == site_key)
            .unwrap();
        assert_eq!(w.pool.stock[&g], unit.net_mass);
        assert_eq!(json(&w.pool.packages[&g]), json(vec![unit.clone()]));
        assert_eq!(w.progress[&g], 12.5);
        assert_eq!(w.open_lots[&g].lot, unit.batch.lot);
        let view = u.services.markets.economy.works_snapshot();
        let w = view.iter().find(|w| w.key == key).unwrap();
        assert_eq!(w.pool.stock[&g], unit.net_mass);
    }
    #[test]
    fn fractional_cargo_survives_save_and_clear() {
        let (mut u, g, _) = fixture();
        u.services.ledger.write_off(PLAYER, u.tick, Cause::Rules);
        u.services.ledger.settle(
            Party::Pilot(PLAYER),
            Asset::Goods(g),
            0.25,
            u.tick,
            Cause::Rules,
        );
        let saved = u.save();
        assert_eq!(
            saved.packing.as_ref().unwrap().holds[0].stock[&u.world.goods[g].key].quantity,
            0.25
        );
        u.load(saved.clone());
        assert_eq!(
            u.services
                .ledger
                .balance(Party::Pilot(PLAYER), Asset::Goods(g)),
            0.25
        );
        assert_eq!(u.vessels[PLAYER].ship.cargo, u.world.goods[g].mass * 0.25);
        u.load(saved);
        assert_eq!(
            u.services
                .ledger
                .balance(Party::Pilot(PLAYER), Asset::Goods(g)),
            0.25
        );
        u.services.ledger.write_off(PLAYER, u.tick, Cause::Rules);
        assert_eq!(
            u.services
                .ledger
                .balance(Party::Pilot(PLAYER), Asset::Goods(g)),
            0.
        );
        assert_eq!(u.services.ledger.cargo_mass(PLAYER, &u.world.goods), 0.);
    }
    #[test]
    fn legacy_save_does_not_invent_labels() {
        let (u, g, _) = fixture();
        let mut save = json(u.save());
        save.as_object_mut().unwrap().remove("packing");
        save["version"] = json(2);
        let mut loaded = Universe::new(7);
        loaded.load(serde_json::from_value(save).unwrap());
        assert_eq!(loaded.hold(), vec![(g, 1)]);
        assert!(loaded.services.ledger.hold_packages(PLAYER).is_none());
        assert_eq!(
            loaded.vessels[PLAYER].ship.cargo,
            loaded.world.goods[g].mass
        );
    }
    #[test]
    fn npc_holds_replace_and_recycled_identity_does_not_inherit() {
        let (mut u, g, unit) = fixture();
        u.services.ledger.write_off(PLAYER, u.tick, Cause::Rules);
        let npc = u.vessels.push(Vessel::new(
            "carrier-a",
            u.vessels[PLAYER].ship.clone(),
            0,
            0.,
        ));
        u.services
            .ledger
            .settle(Party::Pilot(npc), Asset::Goods(g), 1., u.tick, Cause::Rules);
        u.services.ledger.load_cargo(
            npc,
            g,
            vec![],
            vec![(unit.batch.clone(), unit.net_mass)],
            vec![unit.clone()],
        );
        let save = u.save();
        u.services.ledger.write_off(npc, u.tick, Cause::Rules);
        u.load(save.clone());
        assert_eq!(u.services.ledger.hold(npc), vec![(g, 1)]);
        assert_eq!(u.vessels[npc].ship.cargo, unit.gross_mass());
        u.vessels[npc].name = "replacement".into();
        u.load(save);
        assert!(u.services.ledger.hold(npc).is_empty());
        assert_eq!(u.vessels[npc].ship.cargo, 0.);
    }
    #[test]
    fn stock_keys_resolve_after_catalogue_reordering_and_missing_stock_is_dropped() {
        let (u, g, _) = fixture();
        let saved = PackingSave::capture(&u);
        let key = u.world.goods[g].key.clone();
        let mut goods = u.world.goods.clone();
        goods.reverse();
        let mapped = by_index(saved.holds[0].stock.clone(), &goods);
        let new = goods.iter().position(|i| i.key == key).unwrap();
        assert_eq!(mapped[&new].quantity, 1.);
        goods.retain(|i| i.key != key);
        assert!(by_index(saved.holds[0].stock.clone(), &goods).is_empty());
    }
    #[test]
    fn trade_keeps_label_and_recomputes_gross_hold_mass() {
        let (mut u, g, unit) = fixture();
        let place = u
            .services
            .markets
            .economy
            .places
            .iter()
            .find(|p| p.warehouse.is_some())
            .unwrap()
            .clone();
        let warehouse = place.warehouse.unwrap();
        u.services.ledger.write_off(PLAYER, u.tick, Cause::Rules);
        u.vessels[PLAYER].system = place.system;
        u.vessels[PLAYER].ship = u.world.ship_on(place.system, place.facility, 0);
        let pool = &mut u.services.markets.economy.works[warehouse].pool;
        pool.stock.remove(&g);
        pool.lots.remove(&g);
        pool.marks.remove(&g);
        pool.packages.remove(&g);
        pool.put_cargo(
            g,
            unit.net_mass,
            vec![],
            vec![(unit.batch.clone(), unit.net_mass)],
            vec![unit.clone()],
        );
        u.services.markets.economy.invalidate_snapshots();
        u.pilot_trade(PLAYER, place.facility, g, 1).unwrap();
        assert_eq!(u.vessels[PLAYER].ship.cargo, unit.gross_mass());
        assert_eq!(u.vessels[PLAYER].ship.cargo_volume, 5.);
        assert_eq!(
            json(&u.services.ledger.hold_packages(PLAYER).unwrap()[&g][0]),
            json(&unit)
        );
        u.pilot_trade(PLAYER, place.facility, g, -1).unwrap();
        assert_eq!(u.vessels[PLAYER].ship.cargo, 0.);
        assert_eq!(
            json(&u.services.markets.economy.works[warehouse].pool.packages[&g][0]),
            json(&unit)
        );
    }
    #[test]
    fn breakage_opens_packages_and_removes_tare() {
        for quantity in [1., 0.25] {
            let (mut u, _, mut unit) = fixture();
            let g = u
                .world
                .goods
                .iter()
                .position(|g| g.shock_limit.is_some())
                .unwrap();
            unit.product = u.world.goods[g].key.clone();
            unit.net_mass = u.world.goods[g].mass * quantity;
            unit.label = None;
            u.services.ledger.write_off(PLAYER, u.tick, Cause::Rules);
            u.services.ledger.settle(
                Party::Pilot(PLAYER),
                Asset::Goods(g),
                quantity,
                u.tick,
                Cause::Rules,
            );
            u.services.ledger.load_cargo(
                PLAYER,
                g,
                vec![],
                vec![(unit.batch.clone(), unit.net_mass)],
                vec![unit],
            );
            let events = u.book_jolts(
                PLAYER,
                &[crate::Event::Ship(universe_world::ShipEvent::HardLanding {
                    sink: 100.,
                    jolt: 1e9,
                })],
            );
            assert!(!events.is_empty());
            assert!(u.services.ledger.hold(PLAYER).is_empty());
            assert_eq!(u.services.ledger.cargo_mass(PLAYER, &u.world.goods), 0.);
            assert_eq!(u.vessels[PLAYER].ship.cargo_volume, 0.);
        }
    }
}
