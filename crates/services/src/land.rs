//! The land office: at each settlement with ground recorded, who owns which
//! lot and what stands on it, seeded from the registry (Local Administration,
//! `world::settlements`) and changed by the same three actions for anyone:
//! claim free ground, buy a vacant lot, build a facility on one's own lot.
//! It keeps the facts and the office's terms (prices, how long building
//! takes); who does what is the pilots' and companies' business.
//!
//! Zoning is the administration's law, not a wall: nothing here refuses a
//! use a zone doesn't permit (policies and their enforcement come later).
//! What is refused is physical: ground already taken or paved, too far from
//! the port's flat ground, modules that don't fit the lot.

use std::sync::Arc;

use universe_world::settlements::{lay_out, rects_overlap, area, Block, IndustrialModule, Settlement};

use crate::ledger::Party;

/// What the administration asks for land (credits per m²). Invented, to be
/// tuned with the economy.
pub const LAND_PRICE: f64 = 2.0;
/// What a module costs to build (credits per m³ of it: materials and work,
/// bought from outside the game's economy for now). Invented.
pub const MODULE_PRICE: f64 = 0.5;
/// How fast a facility goes up: m³ of module a second, one module after
/// another (one crew). Invented: an ore yard in 6 minutes, a shaft furnace
/// in 36.
pub const BUILD_RATE: f64 = 50_000.0 / 60.0;
/// The ground round a port that is flat enough to build on (m): the
/// terrain's own (`terrain::PAD_FLAT_INNER`).
pub const REACH: f64 = 4_000.0;
/// A claim's least side (m).
pub const LEAST_SIDE: f64 = 50.0;
/// Paving each side of a street's line (m).
const STREET_HALF: f64 = 10.0;

/// Who owns a lot.
#[derive(Clone, Debug, PartialEq)]
pub enum Owner {
    /// A company of the registry's Maker House (its key and name).
    Company { key: String, name: String },
    /// A pilot or anyone else the ledger knows.
    Party(Party),
    /// No one: the land office's to sell.
    Vacant,
}

/// A lot of land.
#[derive(Clone, Debug)]
pub struct Lot {
    pub number: u32,
    pub owner: Owner,
    pub outline: Vec<(f64, f64)>,
}

/// A facility: its modules where they stand, and when each is (or was) done
/// building (world time, s).
#[derive(Clone, Debug)]
pub struct Works {
    pub name: String,
    pub kind: String,
    /// The registry's facility it's built as (what it takes, gives, needs).
    pub blueprint: String,
    pub parcel: u32,
    pub blocks: Vec<Block>,
    pub done_at: Vec<f64>,
    /// How it ran over the last step, once built.
    pub last: Option<Run>,
}

/// How a facility ran over a step: how fast against flat out (0 to 1), what
/// held it back, and what it earned (credits, less what it paid).
#[derive(Clone, Debug, Default)]
pub struct Run {
    pub rate: f64,
    pub held_by: Option<String>,
    pub earned: f64,
}

impl Works {
    /// How far module `k` is built at `now` (0 to 1), given when it started.
    pub fn progress(&self, k: usize, now: f64) -> f64 {
        let start = if k == 0 { self.done_at[0] - build_time(&self.blocks[0]) } else { self.done_at[k - 1] };
        ((now - start) / (self.done_at[k] - start).max(1e-9)).clamp(0.0, 1.0)
    }

    /// Is all of it built at `now`?
    pub fn built(&self, now: f64) -> bool {
        self.done_at.last().is_none_or(|&t| now >= t)
    }
}

/// How long a module takes to build (s).
pub fn build_time(b: &Block) -> f64 {
    b.length * b.width * b.height / BUILD_RATE
}

/// A settlement's ground: what the registry recorded (zones, streets, power
/// lines: fixed for now) and its lots and facilities as they are now.
#[derive(Clone, Debug)]
pub struct Ground {
    pub system: usize,
    pub port: usize,
    pub recorded: &'static Settlement,
    pub lots: Vec<Lot>,
    pub works: Vec<Works>,
}

/// The land office: every settlement's ground.
#[derive(Clone, Debug, Default)]
pub struct LandOffice {
    pub grounds: Vec<Arc<Ground>>,
    /// The companies that own land, by their number as ledger parties: (key, name).
    pub companies: Vec<(String, String)>,
    /// World time its facilities have run to (s).
    pub ran_to: f64,
    /// World time the land rate has been levied to (s).
    pub levied_to: f64,
}

/// How often the land rate is levied (s): daily.
const LEVY_EVERY: f64 = 86_400.0;

impl LandOffice {
    /// Seeded from the registry: each settlement at its system and port,
    /// with its lots and facilities as recorded (all built).
    pub fn seed(at: impl IntoIterator<Item = (usize, usize, &'static Settlement)>) -> Self {
        let grounds = at
            .into_iter()
            .map(|(system, port, s)| {
                let lots = s
                    .parcels
                    .iter()
                    .map(|p| Lot { number: p.number, owner: if p.owner.is_empty() { Owner::Vacant } else { Owner::Company { key: p.owner.clone(), name: p.owner_name.clone() } }, outline: p.outline.clone() })
                    .collect();
                let works = s.facilities.iter().map(|f| Works { name: f.name.clone(), kind: f.kind.clone(), blueprint: f.name.clone(), parcel: f.parcel, blocks: f.blocks.clone(), done_at: vec![0.0; f.blocks.len()], last: None }).collect();
                Arc::new(Ground { system, port, recorded: s, lots, works })
            })
            .collect::<Vec<Arc<Ground>>>();
        let mut companies: Vec<(String, String)> = Vec::new();
        for l in grounds.iter().flat_map(|g| &g.lots) {
            if let Owner::Company { key, name } = &l.owner
                && !companies.iter().any(|c| &c.0 == key)
            {
                companies.push((key.clone(), name.clone()));
            }
        }
        LandOffice { grounds, companies, ran_to: 0.0, levied_to: 0.0 }
    }

    /// The land rate, levied up to `now` a day at a time: each owned lot's
    /// owner pays its system's law's rate on the lot's worth (its area at
    /// the office's price) to the administration. (Where there's no law,
    /// none.)
    pub fn levy(&mut self, ledger: &mut crate::ledger::Ledger, now: f64, tick: universe_protocol::Tick) {
        if self.levied_to == 0.0 {
            self.levied_to = now;
        }
        while self.levied_to + LEVY_EVERY <= now {
            self.levied_to += LEVY_EVERY;
            for g in &self.grounds {
                let Some(rate) = universe_world::order::law(&g.recorded.system).and_then(|l| l.policies.land_rate) else { continue };
                for l in &g.lots {
                    let Some(owner) = self.party(&l.owner) else { continue };
                    let due = area(&l.outline) * LAND_PRICE * rate * LEVY_EVERY;
                    let _ = ledger.transfer(owner, Party::Administration(g.system), crate::ledger::Asset::Credits, due, tick, universe_protocol::Cause::Rules);
                }
            }
        }
    }

    /// The company `key` as a ledger party, made one of the office's
    /// companies if it isn't yet (named as the registry names it).
    pub fn enlist(&mut self, key: &str) -> Party {
        let i = match self.companies.iter().position(|c| c.0 == key) {
            Some(i) => i,
            None => {
                let name = universe_world::registry::registry().names.get(key).cloned().unwrap_or_else(|| key.to_string());
                self.companies.push((key.to_string(), name));
                self.companies.len() - 1
            }
        };
        Party::Company(i as u32)
    }

    /// Who an owner is to the ledger.
    pub fn party(&self, o: &Owner) -> Option<Party> {
        match o {
            Owner::Company { key, .. } => self.companies.iter().position(|c| &c.0 == key).map(|i| Party::Company(i as u32)),
            Owner::Party(p) => Some(*p),
            Owner::Vacant => None,
        }
    }

    pub fn ground(&self, system: usize, port: usize) -> Option<&Ground> {
        self.grounds.iter().find(|g| g.system == system && g.port == port).map(|g| &**g)
    }

    fn ground_mut(&mut self, system: usize, port: usize) -> Result<&mut Ground, String> {
        let g = self.grounds.iter_mut().find(|g| g.system == system && g.port == port).ok_or("NO LAND OFFICE HERE")?;
        Ok(Arc::make_mut(g))
    }

    /// What claiming `outline` (a rectangle squared to east and north, in
    /// metres from the port) at a settlement would cost, or why it can't be.
    pub fn quote_claim(&self, system: usize, port: usize, outline: &[(f64, f64)]) -> Result<f64, String> {
        let g = self.ground(system, port).ok_or("NO LAND OFFICE HERE")?;
        let (w, s, e, n) = outline.iter().fold((f64::MAX, f64::MAX, f64::MIN, f64::MIN), |m, p| (m.0.min(p.0), m.1.min(p.1), m.2.max(p.0), m.3.max(p.1)));
        if outline.len() != 4 || e - w < LEAST_SIDE || n - s < LEAST_SIDE {
            return Err(format!("A CLAIM IS A RECTANGLE AT LEAST {LEAST_SIDE:.0} M EACH WAY"));
        }
        if outline.iter().any(|p| (p.0 * p.0 + p.1 * p.1).sqrt() > REACH) {
            return Err(format!("ONLY GROUND WITHIN {:.0} KM OF THE PORT IS FLAT ENOUGH TO BUILD ON", REACH / 1000.0));
        }
        // (The port's own: its pads and its hangar, south of them.)
        let port = [(-500.0, -560.0), (500.0, -560.0), (500.0, 450.0), (-500.0, 450.0)];
        if rects_overlap(outline, &port) {
            return Err("THAT IS THE PORT'S GROUND".into());
        }
        if let Some(l) = g.lots.iter().find(|l| rects_overlap(outline, &l.outline)) {
            return Err(format!("IT OVERLAPS PARCEL {}", l.number));
        }
        for st in &g.recorded.streets {
            for seg in st.line.windows(2) {
                let (a, b) = (seg[0], seg[1]);
                let paved = [(a.0.min(b.0) - STREET_HALF, a.1.min(b.1) - STREET_HALF), (a.0.max(b.0) + STREET_HALF, a.1.min(b.1) - STREET_HALF), (a.0.max(b.0) + STREET_HALF, a.1.max(b.1) + STREET_HALF), (a.0.min(b.0) - STREET_HALF, a.1.max(b.1) + STREET_HALF)];
                if rects_overlap(outline, &paved) {
                    return Err(format!("IT IS OVER {}", st.name.to_uppercase()));
                }
            }
        }
        Ok(area(outline) * LAND_PRICE)
    }

    /// File a claim as `by`'s, once paid for: its new lot number.
    pub fn claim(&mut self, system: usize, port: usize, outline: Vec<(f64, f64)>, by: Party) -> Result<u32, String> {
        let g = self.ground_mut(system, port)?;
        let number = g.lots.iter().map(|l| l.number).max().unwrap_or(0) + 1;
        g.lots.push(Lot { number, owner: Owner::Party(by), outline });
        Ok(number)
    }

    /// What vacant lot `number` costs, or why it can't be bought.
    pub fn quote_buy(&self, system: usize, port: usize, number: u32) -> Result<f64, String> {
        let g = self.ground(system, port).ok_or("NO LAND OFFICE HERE")?;
        let l = g.lots.iter().find(|l| l.number == number).ok_or_else(|| format!("NO PARCEL {number}"))?;
        if l.owner != Owner::Vacant {
            return Err(format!("PARCEL {number} IS NOT FOR SALE"));
        }
        Ok(area(&l.outline) * LAND_PRICE)
    }

    /// Lot `number` is `by`'s, once paid for.
    pub fn buy(&mut self, system: usize, port: usize, number: u32, by: Party) -> Result<(), String> {
        let g = self.ground_mut(system, port)?;
        let l = g.lots.iter_mut().find(|l| l.number == number).ok_or_else(|| format!("NO PARCEL {number}"))?;
        l.owner = Owner::Party(by);
        Ok(())
    }

    /// Building `modules` (in order) on lot `number` as `by`: laid out as
    /// the registry lays facilities out; their cost and how long they take,
    /// or why not.
    pub fn quote_build(&self, system: usize, port: usize, number: u32, by: Party, modules: &[(&IndustrialModule, u32)]) -> Result<(Vec<Block>, f64, f64), String> {
        let g = self.ground(system, port).ok_or("NO LAND OFFICE HERE")?;
        let l = g.lots.iter().find(|l| l.number == number).ok_or_else(|| format!("NO PARCEL {number}"))?;
        if l.owner != Owner::Party(by) {
            return Err(format!("PARCEL {number} ISN'T YOURS"));
        }
        if g.works.iter().any(|w| w.parcel == number) {
            return Err(format!("SOMETHING ALREADY STANDS ON PARCEL {number}"));
        }
        let blocks = lay_out(&l.outline, &g.recorded.streets, modules)?;
        let cost = blocks.iter().map(|b| b.length * b.width * b.height * MODULE_PRICE).sum();
        let time = blocks.iter().map(build_time).sum();
        Ok((blocks, cost, time))
    }

    /// Start building, once paid for: one module after another from `now`.
    pub fn build(&mut self, system: usize, port: usize, number: u32, name: String, kind: String, blueprint: String, blocks: Vec<Block>, now: f64) -> Result<(), String> {
        let g = self.ground_mut(system, port)?;
        let mut t = now;
        let done_at = blocks.iter().map(|b| { t += build_time(b); t }).collect();
        g.works.push(Works { name, kind, blueprint, parcel: number, blocks, done_at, last: None });
        Ok(())
    }
}

/// The registry's facility a blueprint is.
fn blueprint(name: &str) -> Option<&'static universe_world::settlements::Facility> {
    universe_world::content::content().settlements.iter().flat_map(|s| &s.facilities).find(|f| f.name.eq_ignore_ascii_case(name))
}

/// The registry key of the facility a blueprint (by name) is.
pub fn blueprint_key(name: &str) -> Option<String> {
    blueprint(name).map(|f| f.key.clone())
}
