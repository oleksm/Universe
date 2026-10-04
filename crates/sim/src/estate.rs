//! Land at settlements: claim free ground, buy a vacant lot, build a facility
//! on one's own lot. Each the same action for anyone (`pilot_*`, by pilot
//! id); the player's are recorded for replay. The land office keeps the facts
//! and its terms (`universe_services::land`); the money goes through the
//! ledger: land to the system's administration, building to the world (no
//! builders in the game's economy yet).

use universe_services::ledger::Asset;
use universe_services::Party;

use crate::audit::{Input, Op};
use crate::universe::Universe;

impl Universe {
    /// Claim `outline` (a rectangle in metres east and north of the port) at
    /// spaceport `port` of `system`, as the player.
    pub fn claim_land(&mut self, system: usize, port: usize, outline: Vec<(f64, f64)>) -> Result<String, String> {
        self.note(|| Input::Op(Op::Claim { system, port, outline: outline.clone() }));
        self.pilot_claim(crate::combat::PLAYER, system, port, outline)
    }

    pub(crate) fn pilot_claim(&mut self, id: usize, system: usize, port: usize, outline: Vec<(f64, f64)>) -> Result<String, String> {
        let price = self.land.quote_claim(system, port, &outline)?;
        self.ledger.transfer(Party::Pilot(id), Party::Administration(system), Asset::Credits, price, self.tick, universe_protocol::Cause::Rules)?;
        let number = self.land.claim(system, port, outline, Party::Pilot(id))?;
        Ok(format!("PARCEL {number} FILED AS YOURS FOR {price:.0} CR"))
    }

    /// Buy vacant lot `number` at spaceport `port` of `system`, as the player.
    pub fn buy_parcel(&mut self, system: usize, port: usize, number: u32) -> Result<String, String> {
        self.note(|| Input::Op(Op::BuyParcel { system, port, number }));
        self.pilot_buy_parcel(crate::combat::PLAYER, system, port, number)
    }

    pub(crate) fn pilot_buy_parcel(&mut self, id: usize, system: usize, port: usize, number: u32) -> Result<String, String> {
        let price = self.land.quote_buy(system, port, number)?;
        self.ledger.transfer(Party::Pilot(id), Party::Administration(system), Asset::Credits, price, self.tick, universe_protocol::Cause::Rules)?;
        self.land.buy(system, port, number, Party::Pilot(id))?;
        Ok(format!("PARCEL {number} BOUGHT FOR {price:.0} CR"))
    }

    /// Build on lot `number` what the registry's facility `blueprint` is
    /// built of (its modules, laid out on this lot), as the player.
    pub fn build_facility(&mut self, system: usize, port: usize, number: u32, blueprint: String) -> Result<String, String> {
        self.note(|| Input::Op(Op::Build { system, port, number, blueprint: blueprint.clone() }));
        self.pilot_build(crate::combat::PLAYER, system, port, number, &blueprint)
    }

    pub(crate) fn pilot_build(&mut self, id: usize, system: usize, port: usize, number: u32, blueprint: &str) -> Result<String, String> {
        let (kind, modules) = blueprint_of(blueprint).ok_or_else(|| format!("NO BLUEPRINT '{blueprint}'"))?;
        let (blocks, cost, time) = self.land.quote_build(system, port, number, Party::Pilot(id), &modules)?;
        self.ledger.transfer(Party::Pilot(id), Party::World, Asset::Credits, cost, self.tick, universe_protocol::Cause::Rules)?;
        let name = match kind {
            "power" => "POWER STATION".to_string(),
            k => k.to_uppercase(),
        };
        self.land.build(system, port, number, name, kind.to_string(), blueprint.to_string(), blocks, self.world.time)?;
        Ok(format!("BUILDING STARTED FOR {cost:.0} CR: DONE IN {}", crate::estate::duration(time)))
    }
}

/// The registry's facility `name`, as a blueprint: its kind and its modules
/// (in the order they're laid out).
pub fn blueprint_of(name: &str) -> Option<(&'static str, Vec<(&'static universe_world::settlements::IndustrialModule, u32)>)> {
    let c = universe_world::content::content();
    let f = c.settlements.iter().flat_map(|s| &s.facilities).find(|f| f.name.eq_ignore_ascii_case(name))?;
    let modules = f.modules.iter().map(|(m, n)| c.industrial(m).map(|m| (m, *n))).collect::<Option<Vec<_>>>()?;
    Some((f.kind.as_str(), modules))
}

/// Every blueprint: the registry's facilities, by name.
pub fn blueprints() -> Vec<&'static str> {
    universe_world::content::content().settlements.iter().flat_map(|s| &s.facilities).map(|f| f.name.as_str()).collect()
}

/// A time as hours and minutes.
pub fn duration(s: f64) -> String {
    let m = (s / 60.0).ceil() as u64;
    if m >= 60 { format!("{}H {:02}M", m / 60, m % 60) } else { format!("{m} MIN") }
}
