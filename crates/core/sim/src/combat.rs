//! The combat phase of the tick, and fire control for the player's ship.
//!
//! After every ship has taken its turn, the world runs weapons for all of
//! them at once (guns fire, slugs fly, lasers cut, hits land), and what
//! happened goes to each ship's avionics like any other physical event.
//! Ships are known to weapons by id: the player's ship is `PLAYER`, craft
//! `i` is `i + 1`.

use universe_avionics::fire_control::{Solution, Track};
use universe_world::weapons::Armed;
use universe_world::ShipEvent;
use universe_protocol::ShipId;

use crate::contacts::Contact;
use universe_services::records::Kill;
use crate::universe::Universe;

/// The player's ship's id.
pub const PLAYER: ShipId = ShipId::PLAYER;

/// Craft `i`'s id.
pub fn craft_id(i: usize) -> ShipId {
    ShipId::craft(i)
}

impl Universe {
    /// Weapons for every ship, over the `dt` game seconds the frame covered.
    pub(crate) fn combat(&mut self, dt: f64) {
        // The turrets' gunners have their say (the defence service's pool).
        universe_prof::time("sim/combat/gunners", || self.gunners(dt));
        // (What happens to each ship, by id: the player's first.)
        let mut events: Vec<Vec<ShipEvent>> = (0..self.vessels.len()).map(|_| Vec::new()).collect();
        let now = self.world.time;
        {
            let mut armed: Vec<Armed> = self.vessels.all_mut().iter_mut().zip(events.iter_mut()).enumerate().map(|(i, (v, e))| Armed { id: ShipId(i), system: v.system, ship: &mut v.ship, events: e }).collect();
            universe_prof::time("sim/combat/weapons", || self.world.combat(&mut armed, dt));
            universe_prof::time("sim/combat/collisions", || self.world.collide(&mut armed, dt));
        }
        // The law rules on the hits, as logged: firing on a ship that isn't
        // fair game makes the shooter fair game, and it's told so.
        universe_prof::time("sim/combat/law", || self.rule_on_hits(now, &mut events));
        for (i, events) in events.into_iter().enumerate() {
            let id = ShipId(i);
            if events.is_empty() && !id.is_player() {
                continue;
            }
            self.traffic_events(id, &events.iter().cloned().map(universe_avionics::Event::Ship).collect::<Vec<_>>());
            if let Some(kill) = self.kill_in(id, self.vessels[id].system, &events) {
                if kill.weapon == "COLLISION" {
                    self.records.stats.collision_losses += 1;
                } else {
                    self.records.stats.shot_down += 1;
                    // By the law's evidence: the downed was fair game, or the
                    // killer was (an innocent killed by an aggressor).
                    if self.services.law.aggressed(id, now) {
                        self.records.stats.aggressors_downed += 1;
                    } else if self.services.law.aggressed(kill.killer, now) {
                        self.records.stats.innocents_killed += 1;
                    }
                }
                self.record_kill(kill);
            }
            self.records.stats.collisions += events.iter().filter(|e| matches!(e, ShipEvent::Collided { .. })).count() as u64;
            // And to its pilot, by its sensors.
            match id.craft_index() {
                None => self.player_events(events),
                Some(c) => self.tell(c, crate::contract::Msg::Feed(events)),
            }
        }
    }

    /// Turret gunners' orders due now reach the guns (the gunners are
    /// clients: see `pilots::aim_guns`), after the command delay like a pilot's.
    fn gunners(&mut self, _dt: f64) {
        let mut due: Vec<(ShipId, universe_protocol::TurretCommand)> = Vec::new();
        self.turret_orders.make_contiguous().sort_by_key(|o| o.0);
        while self.turret_orders.front().is_some_and(|(d, _, _)| *d <= self.tick) {
            let (_, id, c) = self.turret_orders.pop_front().expect("due");
            due.push((id, c));
        }
        for (id, c) in due {
            self.world.command_turret(id, c);
        }
    }

    /// The combat phase's events go into the tick's log; the law rules on
    /// each weapon hit there, in order (so a ship made fair game is fair game
    /// for the hits after), and a ruling that's news goes to the shooter.
    fn rule_on_hits(&mut self, now: f64, events: &mut [Vec<ShipEvent>]) {
        let tick = self.tick;
        // (The law where the struck ship is: in a settled system, a station's or a port's.)
        let lasts = |u: &Self, system: usize| {
            let sys = u.world.system(system);
            (sys.station().is_some() || !sys.spaceports.is_empty()).then_some(crate::standing::FAIR_GAME)
        };
        let system_of = |u: &Self, id: ShipId| u.ship_by_id(id).map(|s| s.1);
        let mut notices: Vec<(ShipId, ShipEvent)> = Vec::new();
        let mut pirates: Vec<universe_services::law::Charge> = Vec::new();
        for (i, list) in events.iter().enumerate() {
            let id = ShipId(i);
            for e in list {
                let index = self.log.len() as u32;
                self.log.push((id, e.clone()));
                if let ShipEvent::Hit { by, weapon: true, .. } = *e {
                    let answers = universe_world::turrets::turret_of(by).is_none();
                    let hit = universe_services::law::Hit { shooter: by, target: id, time: now };
                    let law = system_of(self, id).and_then(|s| lasts(self, s));
                    if let Some(r) = self.services.law.hit(hit, answers, law, universe_protocol::Cause::Event { tick, index })
                        && r.new
                    {
                        notices.push((r.ship, ShipEvent::Aggressed { until: r.until }));
                        // (Where the system has a law: charged with piracy.)
                        if let Some(system) = system_of(self, id).filter(|&s| self.has_law(s)) {
                            pirates.push(universe_services::law::Charge { ship: r.ship, system, offence: universe_world::registry::Offence::Piracy, time: now, cause: r.cause, against: Some(id) });
                        }
                    }
                }
            }
        }
        for c in pirates {
            self.charge(c);
        }
        for (ship, notice) in notices {
            if let Some(e) = events.get_mut(ship.0).filter(|_| ship.turret_of().is_none()) {
                e.push(notice);
            }
        }
    }

    /// Did ship `victim`'s `events` end in its destruction by weapons fire?
    /// Who fired the last hit, and with what.
    fn kill_in(&self, victim: ShipId, system: usize, events: &[ShipEvent]) -> Option<Kill> {
        let weapon = events.iter().find_map(|e| match e {
            ShipEvent::Crashed { body } => Some(body.clone()),
            _ => None,
        })?;
        let killer = events.iter().rev().find_map(|e| match e {
            ShipEvent::Hit { by, .. } => Some(*by),
            _ => None,
        })?;
        // The wreck, as logged this tick.
        let index = self.log.iter().rposition(|(id, e)| *id == victim && matches!(e, ShipEvent::Crashed { .. })).unwrap_or(0) as u32;
        let cause = universe_protocol::Cause::Event { tick: self.tick, index };
        let at = self.ship_by_id(victim).map_or(glam::DVec3::ZERO, |(_, _, s)| s.position);
        Some(Kill { time: self.world.time, system, killer, victim, killer_name: self.ship_name(killer), victim_name: self.ship_name(victim), weapon, at, cause })
    }

    fn record_kill(&mut self, kill: Kill) {
        self.recorder.file(kill.time, kill.victim, kill.victim_name.clone(), kill.weapon.clone(), Some((kill.killer, kill.killer_name.clone())));
        self.records.kill(kill);
    }

    /// Name of the ship with combat id `id`.
    pub fn ship_name(&self, id: ShipId) -> String {
        if let Some((system, k)) = universe_world::turrets::turret_of(id) {
            let sys = self.world.system_if_known(system);
            let place = sys.and_then(|sys| universe_world::turrets::turrets(self.world.galaxy.seed, system, &sys).get(k).map(|t| t.facility.name(&sys)));
            return format!("SAM TURRET ({})", place.unwrap_or_default().to_uppercase());
        }
        self.vessels.get(id).map_or_else(|| "UNKNOWN".into(), |v| v.name.to_uppercase())
    }

    /// Fire control on the locked radar contact (the cockpit's, now).
    pub fn fire_control(&mut self, contacts: &[Contact]) -> Option<(Track, Option<Solution>)> {
        self.in_cockpit(|k| k.fire_control_on(contacts))
    }
}
