//! The law's charges (see `universe_services::law`): after the combat phase,
//! each offence the tick's log shows in a system whose administration has a
//! law (`world::order::law`), charged against whoever did it, and the pilot
//! told. Firing on the innocent is piracy (charged where the law rules on the
//! hit, `combat`); an innocent brought down by one who was fair game,
//! murder; a ship wrecked with nobody firing on it, reckless flying.

use universe_protocol::Cause;
use universe_services::law::Charge;
use universe_world::registry::Offence;
use universe_world::ShipEvent;

use crate::universe::Universe;
use crate::Event;

impl Universe {
    /// Is there a law in `system`?
    pub(crate) fn has_law(&self, system: usize) -> bool {
        universe_world::order::law(&self.world.system(system).name).is_some()
    }

    /// The tick's wrecks, judged.
    pub(crate) fn judge(&mut self, now: f64) {
        let mut charges: Vec<Charge> = Vec::new();
        for (index, (id, e)) in self.log.iter().enumerate() {
            let ShipEvent::Crashed { .. } = e else { continue };
            let Some(system) = self.ship_by_id(*id).map(|s| s.1) else { continue };
            if !self.has_law(system) {
                continue;
            }
            let cause = Cause::Event { tick: self.tick, index: index as u32 };
            // (Who brought it down: the last weapon hit on it this tick.)
            let killer = self.log[..index].iter().rev().find_map(|(v, e)| match e {
                ShipEvent::Hit { by, weapon: true, .. } if v == id => Some(*by),
                _ => None,
            });
            let charge = |ship, offence, against| Charge { ship, system, offence, time: now, cause, against };
            match killer {
                Some(k) if universe_world::turrets::turret_of(k).is_none() && self.law.aggressed(k as _, now) && !self.law.aggressed(*id as _, now) => charges.push(charge(k as _, Offence::Murder, Some(*id as _))),
                Some(_) => {}
                None => charges.push(charge(*id as _, Offence::RecklessFlying, None)),
            }
        }
        for c in charges {
            self.charge(c);
        }
    }

    /// Charge `c`, and tell the player if it's against us.
    pub(crate) fn charge(&mut self, c: Charge) {
        if c.ship as usize == crate::combat::PLAYER {
            let system = self.world.system(c.system).name.clone();
            self.events.push(Event::Charged { offence: universe_world::order::offence_name(c.offence), system });
        }
        self.law.charge(c);
    }

    /// The offences ship `id` as it is now has been charged with; and fair
    /// game for firing on the innocent is piracy, law or none.
    pub(crate) fn brought_on(&self, id: usize) -> Vec<Offence> {
        let mut out: Vec<Offence> = self.law.charges_of(id as _).map(|c| c.offence).collect();
        if self.law.aggressed(id as _, self.world.time) {
            out.push(Offence::Piracy);
        }
        out
    }
}
