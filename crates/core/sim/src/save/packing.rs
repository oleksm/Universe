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
    use crate::combat::PLAYER;
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

}
