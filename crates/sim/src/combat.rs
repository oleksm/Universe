//! The combat phase of the tick, and fire control for the player's ship.
//!
//! After every ship has taken its turn, the world runs weapons for all of
//! them at once (guns fire, slugs fly, lasers cut, hits land), and what
//! happened goes to each ship's avionics like any other physical event.
//! Ships are known to weapons by id: the player's ship is `PLAYER`, craft
//! `i` is `i + 1`.

use glam::DVec3;
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
            // Fired on (and not a hunter itself, nor standing to fight with
            // hull to spare): run for the guns.
            let c = &self.crafts[i];
            let fighting = c.status.hunting.is_some_and(|h| h.lawful) && c.ship.hull >= universe_avionics::hunter::FLEE_HULL;
            if !c.status.pirate && !fighting && events.iter().any(|e| matches!(e, ShipEvent::Hit { .. })) {
                self.flee(i);
            }
            if let Some(kill) = self.kill_in(craft_id(i), self.crafts[i].system, &events) {
                if kill.weapon == "COLLISION" {
                    self.records.stats.collision_losses += 1;
                } else {
                    self.records.stats.shot_down += 1;
                    let killer = if kill.killer == PLAYER { None } else { self.crafts.get(kill.killer - 1) };
                    if killer.is_some_and(|c| c.status.pirate) {
                        self.records.stats.pirate_kills += 1;
                    } else if killer.is_some_and(|c| c.status.hunting.is_some_and(|h| h.lawful)) {
                        self.records.stats.aggressors_downed += 1;
                    }
                }
                self.record_kill(kill);
            }
            self.records.stats.collisions += events.iter().filter(|e| matches!(e, ShipEvent::Collided { .. })).count() as u64;
            // And to its pilot, by its sensors.
            self.pool.send(i, crate::pilots::Msg::Feed(events));
        }
    }

    /// The defence service's gunners, for the tick just run (`dt` game s):
    /// in every system with someone fair game in it, each turret's gunner
    /// looks (the frame's snapshot, the law's standings, its gun as its
    /// sensors read it) and orders its gun. Orders reach the guns after the
    /// command delay, like a pilot's; where no one's fair game any more, the
    /// gunners stand down.
    fn gunners(&mut self, dt: f64) {
        use universe_avionics::gunner::Quarry;
        use universe_protocol::TurretCommand;
        let delay = self.command_delay as u64;
        // Orders due now reach the guns.
        while self.turret_orders.front().is_some_and(|(due, _, _)| *due <= self.tick) {
            let (_, id, c) = self.turret_orders.pop_front().expect("due");
            self.world.command_turret(id, c);
        }
        let mut systems: Vec<usize> = self.snaps.iter().filter(|s| s.aggressed && (s.flying || s.landed)).map(|s| s.system).collect();
        systems.sort_unstable();
        systems.dedup();
        // What they see is the frame's snapshot (the turret too, at that
        // moment: one picture), a tick old; their orders land `delay` ticks on.
        let seen = self.snap_time;
        let latency = dt * (delay + 1) as f64;
        let mut orders = Vec::new();
        for &system in &systems {
            let sys = self.world.system(system);
            let positions = self.world.rails_at(system, seen);
            let quarry: Vec<Quarry> = self
                .snaps
                .iter()
                .enumerate()
                .filter(|(_, s)| s.system == system && s.aggressed && (s.flying || s.landed))
                .map(|(id, s)| Quarry { id, position: s.position, velocity: s.velocity })
                .collect();
            for (k, (_, at, velocity)) in self.world.turret_motions_at(system, seen).into_iter().enumerate() {
                let id = universe_world::turrets::turret_id(system, k);
                let gun = self.world.turret_gun(id).map_or_else(|| (quarry.first().map_or(at, |q| q.position) - at).normalize_or(DVec3::Y), |g| g.aim);
                let clear = |p: DVec3| {
                    let d = p - at;
                    universe_physics::ray(&sys.bodies, &positions, at + d.normalize() * 10.0, d.normalize(), d.length() - 30.0, seen, &[]).is_none()
                };
                let gravity = |p: DVec3| sys.gravity(p, &positions);
                let c = self.gunners.entry(id).or_default().orders(seen, at, velocity, gun, &quarry, clear, gravity, latency);
                orders.push((id, c));
            }
        }
        // Gunners with no one left to shoot at stand down.
        let idle: Vec<usize> = self.gunners.keys().copied().filter(|id| universe_world::turrets::turret_of(*id).is_some_and(|(s, _)| !systems.contains(&s))).collect();
        for id in idle {
            self.gunners.remove(&id);
            orders.push((id, TurretCommand { aim: None, fire: false }));
        }
        orders.sort_by_key(|o| o.0);
        for (id, c) in orders {
            if delay == 0 {
                self.world.command_turret(id, c);
            } else {
                self.turret_orders.push_back((self.tick + delay, id, c));
            }
        }
    }

    /// The combat phase's events go into the tick's log; the law rules on
    /// each weapon hit there, in order (so a ship made fair game is fair game
    /// for the hits after), and a ruling that's news goes to the shooter.
    fn rule_on_hits(&mut self, now: f64, player: &mut Vec<ShipEvent>, crafts: &mut [Vec<ShipEvent>]) {
        let tick = self.tick;
        let mut notices: Vec<(usize, ShipEvent)> = Vec::new();
        for (id, events) in std::iter::once((PLAYER, &*player)).chain(crafts.iter().enumerate().map(|(i, e)| (craft_id(i), e))) {
            for e in events {
                let index = self.log.len() as u32;
                self.log.push((id, e.clone()));
                if let ShipEvent::Hit { by, weapon: true, .. } = *e {
                    let answers = universe_world::turrets::turret_of(by).is_none();
                    let hit = universe_services::law::Hit { shooter: by, target: id, time: now };
                    if let Some(r) = self.law.hit(hit, answers, universe_protocol::Cause::Event { tick, index })
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
        Some(Kill { time: self.world.time, system, killer, victim, killer_name: self.ship_name(killer), victim_name: self.ship_name(victim), weapon, cause })
    }

    fn record_kill(&mut self, kill: Kill) {
        self.recorder.file(kill.time, kill.victim, kill.victim_name.clone(), kill.weapon.clone(), Some((kill.killer, kill.killer_name.clone())));
        self.records.kill(kill);
    }

    /// Craft `i` is under fire: it heads for the nearest defended station or
    /// spaceport in its system (next on its route), to dock or land under the
    /// turrets' guns. Already heading there, or nowhere defended: as it was.
    pub(crate) fn flee(&mut self, i: usize) {
        let c = &self.crafts[i];
        if !c.ship.is_flying() || c.ship.hyperdrive {
            return;
        }
        let (system, pos) = (c.system, c.ship.position);
        let mut havens: Vec<(f64, universe_world::Facility)> = self
            .world
            .turret_motions(system)
            .into_iter()
            .filter(|(t, _, _)| !matches!(t.facility, universe_world::Facility::Gate(_)))
            .map(|(t, p, _)| (p.distance(pos), t.facility))
            .collect();
        havens.sort_by(|a, b| a.0.total_cmp(&b.0));
        let Some(&(_, haven)) = havens.first() else { return };
        let stop = universe_avionics::Stop { system, target: haven };
        let c = &mut self.crafts[i];
        if c.status.next_stop == Some(stop) && c.status.route_active {
            return;
        }
        self.pool.send(i, crate::pilots::Msg::Order(crate::pilots::Order::Flee(stop)));
        let c = &mut self.crafts[i];
        c.status.next_stop = Some(stop);
        c.status.route_active = true;
        if c.status.clearance.take().is_some() {
            // Its pilot gives the clearance up, running.
            let cause = self.atc.request_from(craft_id(i));
            self.atc.because(self.tick, cause);
            self.atc.release(craft_id(i));
            self.atc.because(self.tick, universe_protocol::Cause::Rules);
        }
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
            if let Some((_, Some(sol))) = u.fire_control(&contacts) {
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

    #[test]
    fn combat_mode_and_clearance_exclude_each_other() {
        let mut u = Universe::new(1984);
        assert!(u.request_clearance(), "cleared to dock");
        u.command(&ShipCommands { arm: Some(true), ..u.ship.holding() });
        u.step_world(1.0 / 60.0, 1.0, &Controls::default());
        assert!(u.avionics().clearance.is_none(), "arming gives the clearance up");
        assert!(!u.request_clearance(), "no clearance while armed");
        u.command(&ShipCommands { arm: Some(false), ..u.ship.holding() });
        assert!(u.request_clearance(), "safe again: cleared");
        u.toggle_autopilot();
        u.cancel_clearance();
        assert!(u.avionics().clearance.is_none(), "given up");
        assert_eq!(u.ship.throttle, 0.0, "its autopilot stopped, engines idle");
    }
}
