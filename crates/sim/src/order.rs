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
        let mut bounties: Vec<(usize, usize)> = Vec::new();
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
            // (A bounty on it: to whoever brought it down, from the administration that posted it.)
            if let Some(k) = killer {
                bounties.push((k, *id));
            }
            match killer {
                Some(k) if universe_world::turrets::turret_of(k).is_none() && self.law.aggressed(k as _, now) && !self.law.aggressed(*id as _, now) => charges.push(charge(k as _, Offence::Murder, Some(*id as _))),
                Some(_) => {}
                None => charges.push(charge(*id as _, Offence::RecklessFlying, None)),
            }
        }
        for c in charges {
            self.charge(c);
        }
        for (killer, victim) in bounties {
            for (system, credits) in self.law.claim_bounties(victim as _) {
                // (A turret is the administration's own: nobody is paid.)
                if universe_world::turrets::turret_of(killer).is_some() {
                    continue;
                }
                let admin = universe_services::Party::Administration(system);
                let _ = self.ledger.transfer(admin, universe_services::Party::Pilot(killer), universe_services::Asset::Credits, credits, self.tick, universe_protocol::Cause::Rules);
                if killer == crate::combat::PLAYER {
                    self.events.push(Event::Bounty { credits, on: self.ship_name(victim) });
                }
            }
        }
    }

    /// Outside the law where ship `id` is: no one there deals with it.
    pub(crate) fn barred(&self, id: usize, system: usize) -> Result<(), String> {
        if self.law.outlawed(id as _, system, self.world.time) {
            return Err(format!("YOU ARE OUTSIDE {}'S LAW: NO ONE HERE DEALS WITH YOU", self.world.system(system).name.to_uppercase()));
        }
        Ok(())
    }

    /// Charge `c` and apply what its law gives for it: outlawry for its
    /// term, a bounty (its share of the offender's ship's worth), a fine (its
    /// share of the worth at stake: here, the offender's ship; as far as its
    /// credits go); tell the player if it's against us. (Forfeiture, gaol and
    /// a warning have nothing to act on yet.)
    pub(crate) fn charge(&mut self, c: Charge) {
        use universe_world::registry::Penalty;
        let sys_name = self.world.system(c.system).name.clone();
        let worth = self.ship_by_id(c.ship as usize).map_or(0.0, |(_, _, s)| Universe::ship_value(s));
        let me = universe_services::Party::Pilot(c.ship as usize);
        let admin = universe_services::Party::Administration(c.system);
        let penalties = universe_world::order::law(&sys_name).and_then(|l| l.offences.iter().find(|o| o.offence == c.offence)).map(|o| o.penalties.clone()).unwrap_or_default();
        let mut said: Vec<String> = Vec::new();
        for p in &penalties {
            match p.kind {
                Penalty::Outlawry => {
                    let term = p.term.unwrap_or(0.0);
                    self.law.outlaw(c.ship, c.system, c.time + term);
                    said.push(format!("OUTLAWED {:.0} YEARS", term / 31_557_600.0));
                }
                Penalty::Bounty => {
                    let credits = p.share.unwrap_or(0.0) * worth;
                    self.law.post_bounty(c.ship, c.system, credits);
                    said.push(format!("{credits:.0} CR ON YOUR HEAD"));
                }
                Penalty::Fine => {
                    let fine = (p.share.unwrap_or(0.0) * worth).min(self.ledger.credits(me).max(0.0));
                    let _ = self.ledger.transfer(me, admin, universe_services::Asset::Credits, fine, self.tick, c.cause);
                    said.push(format!("FINED {fine:.0} CR"));
                }
                _ => {}
            }
        }
        if c.ship as usize == crate::combat::PLAYER {
            self.events.push(Event::Charged { offence: universe_world::order::offence_name(c.offence), system: sys_name, penalties: said.join(", ") });
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
