//! The companies' business (a client's, as the miners' and traders' is): what
//! a company that owns a works does with it, by the same action a player has
//! (setting a module, `Economy::set_up`). The engine never chooses for it.
//!
//! A yard builds the hulls its building dock can make: the dock set to its
//! hull; its assembly shop to whichever of that hull's fitted products (its
//! drive, tank, plant...) its store has least of; each welding bay and
//! machining centre to the part the hull and those products lack most. A
//! module keeps to what it's making until its store has what's wanted of
//! it; one whose line makes what none of that goes into stands idle (it
//! would only use up the stock the rest needs).

use std::collections::HashMap;

use universe_services::Party;
use universe_world::recipes::{self, Recipe};

use crate::universe::Universe;

/// What a works wants on hand of each thing it builds toward (kg): one of each.
fn wanted(goals: &[usize], mass: &dyn Fn(usize) -> f64, made_by: &dyn Fn(usize) -> Option<&'static Recipe>) -> HashMap<usize, f64> {
    let mut want: HashMap<usize, f64> = HashMap::new();
    for &g in goals {
        *want.entry(g).or_default() += mass(g);
        if let Some(r) = made_by(g) {
            for &(i, q) in &r.inputs {
                *want.entry(i).or_default() += q * mass(g);
            }
        }
    }
    want
}

/// Each company's works, set once a step of the economy.
pub fn run(u: &mut Universe) {
    let goods = u.world.goods.clone();
    let mass = |i: usize| goods[i].mass;
    for k in 0..u.markets.economy.works.len() {
        let w = &u.markets.economy.works[k];
        let g = &u.land.grounds[w.ground];
        let Some(owner @ Party::Company(_)) = g.works.get(w.works).and_then(|x| g.lots.iter().find(|l| l.number == x.parcel)).and_then(|l| u.land.party(&l.owner)) else { continue };
        let shop: Vec<usize> = (0..w.setups.len()).filter(|&s| w.setups[s].module.throughput.is_some()).collect();
        if shop.is_empty() {
            continue;
        }
        let module = |s: usize| w.setups[s].module.identity.key.as_str();
        // What it builds toward: the hull its dock makes, and that hull's fitted products.
        let reg = universe_world::registry::registry();
        let docks: Vec<usize> = shop.iter().copied().filter(|&s| module(s) == "module.building-dock").collect();
        let hulls: Vec<usize> = docks.iter().filter_map(|&s| recipes::of(module(s)).first()).map(|r| r.makes).collect();
        let fitted: Vec<usize> = hulls.iter().filter_map(|&h| reg.hulls.iter().find(|x| x.identity.key == goods[h].key)).flat_map(|h| h.fit.iter().filter_map(|f| universe_world::goods::item(&f.item))).collect();
        let goals: Vec<usize> = hulls.iter().chain(&fitted).copied().collect();
        if goals.is_empty() {
            continue;
        }
        // (What makes each thing, among this works' shop modules.)
        let made_by = |i: usize| shop.iter().find_map(|&s| recipes::of(module(s)).iter().find(|r| r.makes == i));
        let want = wanted(&goals, &mass, &made_by);
        let short = |i: usize| want.get(&i).copied().unwrap_or(0.0) - w.pool.of(i);
        let mut picks: Vec<(usize, Option<usize>)> = Vec::new();
        let mut taken: Vec<usize> = Vec::new();
        for &s in &shop {
            let list = recipes::of(module(s));
            let now = w.setups[s].recipe;
            let pick = if module(s) == "module.building-dock" {
                (!list.is_empty()).then_some(0)
            } else if now.is_some_and(|r| short(list[r].makes) > 0.0) {
                now
            } else {
                // (The most lacking it can make, that no other module of this works is on.)
                let at = |i: usize| want.get(&i).is_some_and(|_| short(i) > 0.0 && !taken.contains(&i));
                let best = (0..list.len()).filter(|&r| at(list[r].makes)).max_by(|&a, &b| short(list[a].makes).total_cmp(&short(list[b].makes)));
                best.or(None)
            };
            if let Some(r) = pick {
                taken.push(list[r].makes);
            }
            if pick != now {
                picks.push((s, pick));
            }
        }
        // (A module making what none of this goes into stands idle: it would only use up stock.)
        for s in (0..w.setups.len()).filter(|s| !shop.contains(s)) {
            if w.setups[s].recipe().is_some_and(|r| !want.contains_key(&r.makes)) {
                picks.push((s, None));
            }
        }
        for (s, r) in picks {
            let _ = u.markets.economy.set_up(&u.land, k, s, r, owner);
        }
    }
}
