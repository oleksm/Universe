//! Recipes as the engine runs them: everything a module can be set to make,
//! with what that takes, by stock item. A module's own recipes are the
//! registry's (`module.recipes`). A shop module's (one that cuts, machines,
//! welds or assembles whatever it is given, `module.throughput`) are what
//! names it as the module that makes it: a part, from what it is made of,
//! what is cut away coming out as scrap of its metal; a piece of equipment or
//! a hull, from its parts (`Registry::built_of`), at the module's rate.

use std::collections::HashMap;

use crate::registry::Registry;

/// One thing a module can be set to make. Amounts are kg for each kg of what
/// it makes.
#[derive(Clone, Debug, PartialEq)]
pub struct Recipe {
    /// What it makes (a stock item).
    pub makes: usize,
    /// What goes in from stock (what's drawn where the module stands, its
    /// air, rain or ground water, the world gives: not listed).
    pub inputs: Vec<(usize, f64)>,
    /// What else comes out.
    pub outputs: Vec<(usize, f64)>,
    /// What each kg made draws of itself from the ground where the module stands (a mine's ore
    /// in place: a claimed deposit's, worked out in the end); 0: none.
    pub from_ground: f64,
    /// kg/s of what it makes, at full rate; W drawn.
    pub rate: f64,
    pub power: f64,
    /// Its place in the module's registry recipes (None: a shop recipe).
    pub index: Option<usize>,
}

/// Every module's recipes, by module key, for the stock catalogue `index`
/// (item key to item). A recipe naming something not in it is left out.
pub(crate) fn build(reg: &Registry, index: &HashMap<String, usize>, mass: &dyn Fn(usize) -> f64) -> HashMap<String, Vec<Recipe>> {
    let id = |k: &str| index.get(k).copied();
    let mut out: HashMap<String, Vec<Recipe>> = HashMap::new();
    for m in &reg.modules {
        let list = out.entry(m.identity.key.clone()).or_default();
        for (k, r) in m.recipes.iter().enumerate() {
            let Some(makes) = id(&r.makes) else { continue };
            let amounts = |a: &[crate::registry::Amount], stock_only: bool| a.iter().filter(|x| !(stock_only && crate::goods::from_place(x))).filter_map(|x| Some((id(&x.item)?, x.quantity))).collect::<Vec<_>>();
            let from_ground = r.inputs.iter().filter(|x| crate::goods::from_place(x) && x.item == r.makes).map(|x| x.quantity).sum();
            list.push(Recipe { makes, inputs: amounts(&r.inputs, true), outputs: amounts(&r.outputs, false), from_ground, rate: r.rate, power: r.power, index: Some(k) });
        }
    }
    // The scrap of a stock item's metal: the stock of form scrap made from the same material.
    let scrap_of = |stock: &str| -> Option<usize> {
        let s = reg.stock(&stock)?;
        let material = &s.made_from.first()?.item;
        let scrap = reg.stock.iter().find(|x| x.identity.form == "scrap" && x.made_from.iter().any(|m| &m.item == material))?;
        id(&scrap.identity.key)
    };
    let shop = |module: &Option<String>| module.as_deref().and_then(|k| reg.module(k)).and_then(|m| Some((m.identity.key.clone(), m.throughput.as_ref()?)));
    // Parts: from what each is made of (kg for one), the rest scrap.
    for p in &reg.parts {
        let (Some(makes), Some((module, t))) = (id(&p.identity.key), shop(&p.making.module)) else { continue };
        let each = mass(makes);
        if each <= 0.0 {
            continue;
        }
        let inputs: Vec<(usize, f64)> = p.made_from.iter().filter_map(|x| Some((id(&x.item)?, x.quantity? / each))).collect();
        if inputs.is_empty() {
            continue;
        }
        let cut: f64 = inputs.iter().map(|i| i.1).sum::<f64>() - 1.0;
        let outputs = p.made_from.iter().find_map(|x| scrap_of(&x.item)).filter(|_| cut > 1e-9).map(|s| vec![(s, cut)]).unwrap_or_default();
        out.entry(module).or_default().push(Recipe { makes, inputs, outputs, from_ground: 0.0, rate: t.rate, power: t.power.unwrap_or(0.0), index: None });
    }
    // Equipment, hulls and parts made of parts: from their parts.
    let assemblies = reg.parts.iter().filter(|p| p.made_from.is_empty()).map(|p| (&p.identity.key, &p.making.module));
    let products = reg.equipment.iter().map(|e| (&e.identity.key, &e.making.module)).chain(reg.hulls.iter().map(|h| (&h.identity.key, &h.making.module))).chain(assemblies);
    for (key, module) in products {
        let (Some(makes), Some((module, t))) = (id(key), shop(module)) else { continue };
        let each = mass(makes);
        // (A part the registry hasn't described yet, with no mass and nothing it's made of,
        // isn't built into it: there's nothing to build it from.)
        let parts: Vec<_> = reg.built_of(key).into_iter().filter(|(p, _)| p.physical.mass.is_some() || !p.made_from.is_empty() || !reg.built_of(&p.identity.key).is_empty()).collect();
        let inputs: Vec<(usize, f64)> = parts.iter().filter_map(|(p, n)| Some((id(&p.identity.key)?, mass(id(&p.identity.key)?) * *n as f64 / each))).collect();
        if each <= 0.0 || inputs.is_empty() || inputs.len() < parts.len() {
            continue;
        }
        out.entry(module).or_default().push(Recipe { makes, inputs, outputs: Vec::new(), from_ground: 0.0, rate: t.rate, power: t.power.unwrap_or(0.0), index: None });
    }
    out
}

/// What module `key` can be set to make.
pub fn of(key: &str) -> &'static [Recipe] {
    crate::content::content().recipes.get(key).map_or(&[], Vec::as_slice)
}
