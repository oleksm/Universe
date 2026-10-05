//! The companies' business (a client's, as the miners' and traders' is): what
//! a company that owns a works does with it, by the same action a player has
//! (setting a module, `Economy::set_up`). The engine never chooses for it.
//!
//! A yard builds the hulls its building dock can make: the dock set to its
//! hull; every other module that can be set to anything (assembly shop,
//! welding bays, machining centres, the cutting table and panel former) to
//! what the hull and its fitted products (drive, tank, plant...) lack most,
//! down the whole bill (a part's panels, a panel's blanks). A module keeps to
//! what it's making until its store has what's wanted of it; one with
//! nothing wanted to make stands idle (it would only use up stock).

use std::collections::HashMap;

use universe_services::Party;
use universe_world::recipes::{self, Recipe};

use crate::universe::Universe;

/// What a works wants on hand of each thing it builds toward (kg): one of
/// each, and what goes into it, down the bill (a hull's parts, a leg's own).
fn wanted(goals: &[usize], mass: &dyn Fn(usize) -> f64, made_by: &dyn Fn(usize) -> Option<&'static Recipe>) -> HashMap<usize, f64> {
    let mut want: HashMap<usize, f64> = HashMap::new();
    let mut open: Vec<(usize, f64, usize)> = goals.iter().map(|&g| (g, mass(g), 0)).collect();
    while let Some((i, kg, depth)) = open.pop() {
        *want.entry(i).or_default() += kg;
        if let Some(r) = made_by(i).filter(|_| depth < 8) {
            open.extend(r.inputs.iter().map(|&(x, q)| (x, q * kg, depth + 1)));
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
        // (A works with a building dock is a yard: each of its modules that can be set to
        // anything is set to what the build needs, the cutting table and panel former too.)
        if !w.setups.iter().any(|s| s.module.identity.key == "module.building-dock") {
            continue;
        }
        let shop: Vec<usize> = (0..w.setups.len()).filter(|&s| !recipes::of(&w.setups[s].module.identity.key).is_empty()).collect();
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
        // (Something to make it from: every input in store.)
        let ready = |r: &Recipe| r.inputs.iter().all(|&(i, _)| w.pool.of(i) > 0.0);
        let mut picks: Vec<(usize, Option<usize>)> = Vec::new();
        let mut taken: Vec<usize> = Vec::new();
        for &s in &shop {
            let list = recipes::of(module(s));
            let now = w.setups[s].recipe;
            let pick = if module(s) == "module.building-dock" {
                (!list.is_empty()).then_some(0)
            } else if now.is_some_and(|r| short(list[r].makes) > 0.0 && ready(&list[r])) {
                now
            } else {
                // (The most lacking it can make, and has the makings of, that no other module of
                // this works is on.)
                let at = |r: &Recipe| want.get(&r.makes).is_some_and(|_| short(r.makes) > 0.0 && !taken.contains(&r.makes)) && ready(r);
                let best = (0..list.len()).filter(|&r| at(&list[r])).max_by(|&a, &b| short(list[a].makes).total_cmp(&short(list[b].makes)));
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
