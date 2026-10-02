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

use crate::contacts::Contact;
use universe_services::records::Kill;
use crate::universe::Universe;

/// The player's ship's id in combat.
pub const PLAYER: usize = 0;

/// Combat id of craft `i`.
pub fn craft_id(i: usize) -> usize {
    i + 1
}

impl Universe {
    /// Weapons for every ship, over the `dt` game seconds the frame covered.
    pub(crate) fn combat(&mut self, dt: f64) {
        // The turrets' gunners have their say (the defence service's pool).
        universe_prof::time("sim/combat/gunners", || self.gunners(dt));
        let mut player_events = Vec::new();
        let mut craft_events: Vec<Vec<ShipEvent>> = (0..self.crafts.len()).map(|_| Vec::new()).collect();
        let now = self.world.time;
        {
            let mut armed = Vec::with_capacity(self.crafts.len() + 1);
            armed.push(Armed { id: PLAYER, system: self.ship_system, ship: &mut self.ship, events: &mut player_events });
            for (i, (c, e)) in self.crafts.iter_mut().zip(craft_events.iter_mut()).enumerate() {
                armed.push(Armed { id: craft_id(i), system: c.system, ship: &mut c.ship, events: e });
            }
            universe_prof::time("sim/combat/weapons", || self.world.combat(&mut armed, dt));
            universe_prof::time("sim/combat/collisions", || self.world.collide(&mut armed, dt));
        }
        // The law rules on the hits, as logged: firing on a ship that isn't
        // fair game makes the shooter fair game, and it's told so.
        universe_prof::time("sim/combat/law", || self.rule_on_hits(now, &mut player_events, &mut craft_events));
        self.traffic_events(PLAYER, &player_events.iter().cloned().map(universe_avionics::Event::Ship).collect::<Vec<_>>());
        if let Some(kill) = self.kill_in(PLAYER, self.ship_system, &player_events) {
            self.record_kill(kill);
        }
        self.player_events(player_events);
        for (i, events) in craft_events.into_iter().enumerate() {
            if events.is_empty() {
                continue;
            }
            self.traffic_events(craft_id(i), &events.iter().cloned().map(universe_avionics::Event::Ship).collect::<Vec<_>>());
            if let Some(kill) = self.kill_in(craft_id(i), self.crafts[i].system, &events) {
                if kill.weapon == "COLLISION" {
                    self.records.stats.collision_losses += 1;
                } else {
                    self.records.stats.shot_down += 1;
                    // By the law's evidence: the downed was fair game, or the
                    // killer was (an innocent killed by an aggressor).
                    if self.law.aggressed(craft_id(i), now) {
                        self.records.stats.aggressors_downed += 1;
                    } else if self.law.aggressed(kill.killer, now) {
                        self.records.stats.innocents_killed += 1;
                    }
                }
                self.record_kill(kill);
            }
            self.records.stats.collisions += events.iter().filter(|e| matches!(e, ShipEvent::Collided { .. })).count() as u64;
            // And to its pilot, by its sensors.
            self.tell(i, crate::contract::Msg::Feed(events));
        }
    }

    /// The defence service's gunners, for the tick just run (`dt` game s):
    /// in every system with someone fair game in it, each turret's gunner
    /// looks (the frame's snapshot, the law's standings, its gun as its
    /// sensors read it) and orders its gun. Orders reach the guns after the
    /// command delay, like a pilot's; where no one's fair game any more, the
    /// Turret gunners' orders due now reach the guns (the gunners are
    /// clients: see `pilots::aim_guns`).
    fn gunners(&mut self, _dt: f64) {
        let mut due: Vec<(usize, universe_protocol::TurretCommand)> = Vec::new();
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
    fn rule_on_hits(&mut self, now: f64, player: &mut Vec<ShipEvent>, crafts: &mut [Vec<ShipEvent>]) {
        let tick = self.tick;
        let charts = self.charts();
        // (The law of whoever holds the space the struck ship's in.)
        let lasts = |system: usize| charts.holder(system).map(|f| f.aggression);
        let system_of = |u: &Self, id: usize| u.ship_by_id(id).map(|s| s.1);
        let mut notices: Vec<(usize, ShipEvent)> = Vec::new();
        for (id, events) in std::iter::once((PLAYER, &*player)).chain(crafts.iter().enumerate().map(|(i, e)| (craft_id(i), e))) {
            for e in events {
                let index = self.log.len() as u32;
                self.log.push((id, e.clone()));
                if let ShipEvent::Hit { by, weapon: true, .. } = *e {
                    let answers = universe_world::turrets::turret_of(by).is_none();
                    let hit = universe_services::law::Hit { shooter: by, target: id, time: now };
                    let law = system_of(self, id).and_then(lasts);
                    if let Some(r) = self.law.hit(hit, answers, law, universe_protocol::Cause::Event { tick, index })
                        && r.new
                    {
                        notices.push((r.ship, ShipEvent::Aggressed { until: r.until }));
                    }
                }
            }
        }
        for (ship, notice) in notices {
            match ship {
                PLAYER => player.push(notice),
                id => {
                    if let Some(e) = crafts.get_mut(id - 1) {
                        e.push(notice);
                    }
                }
            }
        }
    }

    /// Did ship `victim`'s `events` end in its destruction by weapons fire?
    /// Who fired the last hit, and with what.
    fn kill_in(&self, victim: usize, system: usize, events: &[ShipEvent]) -> Option<Kill> {
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
    pub fn ship_name(&self, id: usize) -> String {
        if let Some((system, k)) = universe_world::turrets::turret_of(id) {
            let sys = self.world.system_if_known(system);
            let place = sys.and_then(|sys| universe_world::turrets::turrets(self.world.galaxy.seed, system, &sys).get(k).map(|t| t.facility.name(&sys)));
            return format!("SAM TURRET ({})", place.unwrap_or_default().to_uppercase());
        }
        match id {
            PLAYER => "YOU".into(),
            _ => self.crafts.get(id - 1).map_or_else(|| "UNKNOWN".into(), |c| c.name.to_uppercase()),
        }
    }

    /// Fire control on the locked radar contact (the cockpit's, now).
    pub fn fire_control(&mut self, contacts: &[Contact]) -> Option<(Track, Option<Solution>)> {
        self.in_cockpit(|k| k.fire_control_on(contacts))
    }
}

#[cfg(test)]
mod tests {
    use glam::DVec3;
    use universe_world::{Controls, ShipCommands, ShipState, Triggers};

    use crate::universe::Universe;

    #[test]
    fn the_player_shoots_down_a_settler_on_the_lead() {
        let mut u = Universe::new(1984);
        // (In flight by the home station, not parked on its deck; the respawn
        // settled, a step on, before the lock is taken.)
        u.respawn();
        u.step_world(1.0 / 60.0, 1.0, &Controls::default());
        u.spawn_settlers(1, 1);
        // A settler 3 km ahead, crossing at 40 m/s; we're drifting with it.
        // Well away from any defence turrets (we'll be the aggressor), and the
        // target one that won't run (a pirate, with nothing to hunt).
        u.ship.position += DVec3::new(1.5e6, 0.0, 0.0);
        let (sys, pos, vel) = (u.ship_system, u.ship.position, u.ship.velocity);
        let c = &mut u.crafts[0];
        c.system = sys;
        c.ship.state = ShipState::Flying;
        c.ship.hyperdrive = false;
        c.ship.position = pos + DVec3::new(0.0, 3_000.0, 0.0);
        c.ship.velocity = vel + DVec3::new(40.0, 0.0, 0.0);
        u.pilots()[0].avionics = universe_avionics::Avionics { pirate: true, ..Default::default() };
        u.command(&ShipCommands { arm: Some(true), ..u.ship.holding() });
        u.lock_next_contact();
        let mut destroyed = false;
        for frame in 0..600 {
            let contacts = u.contacts();
            let fc = u.fire_control(&contacts);
            if let Some((_, Some(sol))) = fc {
                u.ship.orientation = universe_world::ship::facing(sol.aim, sol.aim.any_orthonormal_vector());
                if frame % 60 == 0 {
                    u.command(&ShipCommands { weapons: Some(Triggers { gun: true, laser: false }), ..u.ship.holding() });
                }
            }
            u.step_world(1.0 / 60.0, 1.0, &Controls::default());
            if u.records.stats.shot_down > 0 {
                destroyed = true;
                break;
            }
        }
        let fired = universe_world::weapons::GUN_AMMO - u.ship.ammo;
        eprintln!("rounds fired {fired}, shot down {destroyed}");
        assert!(destroyed, "should be shot down; fired {fired}");
        assert!(fired < 30, "most rounds on target: {fired}");
    }

}
