//! The combat phase of the tick, and fire control for the player's ship.
//!
//! After every ship has taken its turn, the world runs weapons for all of
//! them at once (guns fire, slugs fly, lasers cut, hits land), and what
//! happened goes to each ship's avionics like any other physical event.
//! Ships are known to weapons by id: the player's ship is `PLAYER`, craft
//! `i` is `i + 1`.

use universe_avionics::fire_control::{lead, Solution, Track};
use universe_world::weapons::{Armed, GUN_MUZZLE, SLUG_LIFETIME};
use universe_world::{ShipCommands, ShipEvent};

use crate::contacts::Contact;
use crate::universe::Universe;

/// The player's ship's id in combat.
pub const PLAYER: usize = 0;

/// Kills kept in `Universe::kills`.
const KILL_LOG: usize = 50;

/// A ship destroyed by weapons fire: who fired the last hit, at whom, with
/// what ("GUNFIRE", "LASER FIRE"), where and when.
#[derive(Clone, Debug)]
pub struct Kill {
    pub time: f64,
    pub system: usize,
    pub killer: usize,
    pub victim: usize,
    pub killer_name: String,
    pub victim_name: String,
    pub weapon: String,
}

/// Combat id of craft `i`.
pub fn craft_id(i: usize) -> usize {
    i + 1
}

impl Universe {
    /// Weapons for every ship, over the `dt` game seconds the frame covered.
    pub(crate) fn combat(&mut self, dt: f64) {
        let mut player_events = Vec::new();
        let mut craft_events: Vec<Vec<ShipEvent>> = (0..self.crafts.len()).map(|_| Vec::new()).collect();
        {
            let mut armed = Vec::with_capacity(self.crafts.len() + 1);
            armed.push(Armed { id: PLAYER, system: self.ship_system, ship: &mut self.ship, events: &mut player_events });
            for (i, (c, e)) in self.crafts.iter_mut().zip(craft_events.iter_mut()).enumerate() {
                armed.push(Armed { id: craft_id(i), system: c.system, ship: &mut c.ship, events: e });
            }
            self.world.combat(&mut armed, dt);
            self.world.collide(&mut armed, dt);
        }
        self.traffic_events(PLAYER, &player_events.iter().cloned().map(universe_avionics::Event::Ship).collect::<Vec<_>>());
        if let Some(kill) = self.kill_in(PLAYER, self.ship_system, &player_events) {
            self.record_kill(kill);
        }
        self.avionics.record(player_events, &mut self.events);
        let mut ignored = Vec::new();
        for (i, events) in craft_events.into_iter().enumerate() {
            if events.is_empty() {
                continue;
            }
            self.traffic_events(craft_id(i), &events.iter().cloned().map(universe_avionics::Event::Ship).collect::<Vec<_>>());
            // Fired on (and not a hunter itself, nor standing to fight with
            // hull to spare): run for the guns.
            let c = &self.crafts[i];
            let fighting = c.avionics.hunting.is_some_and(|h| h.lawful) && c.ship.hull >= universe_avionics::hunter::FLEE_HULL;
            if !c.avionics.pirate && !fighting && events.iter().any(|e| matches!(e, ShipEvent::Hit { .. })) {
                self.flee(i);
            }
            if let Some(kill) = self.kill_in(craft_id(i), self.crafts[i].system, &events) {
                if kill.weapon == "COLLISION" {
                    self.traffic.collision_losses += 1;
                } else {
                    self.traffic.shot_down += 1;
                    let killer = if kill.killer == PLAYER { None } else { self.crafts.get(kill.killer - 1) };
                    if killer.is_some_and(|c| c.avionics.pirate) {
                        self.traffic.pirate_kills += 1;
                    } else if killer.is_some_and(|c| c.avionics.hunting.is_some_and(|h| h.lawful)) {
                        self.traffic.aggressors_downed += 1;
                    }
                }
                self.record_kill(kill);
            }
            self.traffic.collisions += events.iter().filter(|e| matches!(e, ShipEvent::Collided { .. })).count() as u64;
            self.crafts[i].avionics.record(events, &mut ignored);
            ignored.clear();
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
        Some(Kill { time: self.world.time, system, killer, victim, killer_name: self.ship_name(killer), victim_name: self.ship_name(victim), weapon })
    }

    fn record_kill(&mut self, kill: Kill) {
        self.recorder.file(kill.time, kill.victim, kill.victim_name.clone(), kill.weapon.clone(), Some((kill.killer, kill.killer_name.clone())));
        self.kills.push(kill);
        if self.kills.len() > KILL_LOG {
            self.kills.remove(0);
        }
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
        let r = &mut c.avionics.route;
        if r.stops.get(r.next) == Some(&stop) {
            return;
        }
        r.stops.insert(r.next.min(r.stops.len()), stop);
        r.active = true;
        r.dwell_until = None;
        r.departing = false;
        if c.avionics.clearance.take().is_some() {
            self.world.traffic.release(craft_id(i));
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

    /// Fire control on the locked radar contact: keep the track going from
    /// this frame's `contacts`, and once it's settled, the gun's lead.
    pub fn fire_control(&mut self, contacts: &[Contact]) -> Option<(Track, Option<Solution>)> {
        let Some(c) = self.locked_contact_in(contacts) else {
            self.avionics.track = None;
            if self.ship.gun_target.is_some() {
                self.command(&ShipCommands { gun_target: Some(None), ..self.ship.holding() });
            }
            return None;
        };
        Track::update(&mut self.avionics.track, c.blip.id, c.blip.position, c.blip.velocity, self.world.time);
        let track = self.avionics.track?;
        // Gravity pulls the round as it does the target: lead on the rest of its acceleration.
        let sys = self.ship_system();
        let mut positions = Vec::new();
        sys.positions(self.world.time, &mut positions);
        let accel = track.acceleration - sys.gravity(c.blip.position, &positions);
        let solution = track
            .ready()
            .then(|| lead(self.ship.position, self.ship.velocity, c.blip.position, c.blip.velocity, accel, GUN_MUZZLE, SLUG_LIFETIME))
            .flatten();
        // In combat mode, fire control lays the gun on the lead (its gimbal
        // reaches a few degrees off the nose).
        let lay = solution.filter(|_| self.ship.armed).map(|s| s.aim);
        if lay != self.ship.gun_target {
            self.command(&ShipCommands { gun_target: Some(lay), ..self.ship.holding() });
        }
        Some((track, solution))
    }
}


#[cfg(test)]
mod tests {
    use glam::{DQuat, DVec3};
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
        c.avionics = Default::default();
        c.avionics.pirate = true;
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
            if u.traffic.shot_down > 0 {
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
    fn the_gimbal_hits_with_the_nose_two_degrees_off() {
        let mut u = Universe::new(1984);
        u.spawn_settlers(1, 1);
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
        c.avionics = Default::default();
        c.avionics.pirate = true;
        u.command(&ShipCommands { arm: Some(true), ..u.ship.holding() });
        u.lock_next_contact();
        let two = DQuat::from_rotation_z(2f64.to_radians());
        for frame in 0..600 {
            let contacts = u.contacts();
            if let Some((_, Some(sol))) = u.fire_control(&contacts) {
                // A pilot holding the nose 2° off the lead.
                let off = two * sol.aim;
                u.ship.orientation = universe_world::ship::facing(off, off.any_orthonormal_vector());
                if frame % 60 == 0 {
                    u.command(&ShipCommands { weapons: Some(Triggers { gun: true, laser: false }), ..u.ship.holding() });
                }
            }
            u.step_world(1.0 / 60.0, 1.0, &Controls::default());
            if u.traffic.shot_down > 0 {
                break;
            }
        }
        assert_eq!(u.traffic.shot_down, 1, "the gimbal lays the gun on the lead");
        assert!(u.world.impacts.len() <= 1);
    }

    #[test]
    fn combat_mode_and_clearance_exclude_each_other() {
        let mut u = Universe::new(1984);
        assert!(u.request_clearance(), "cleared to dock");
        u.command(&ShipCommands { arm: Some(true), ..u.ship.holding() });
        u.step_world(1.0 / 60.0, 1.0, &Controls::default());
        assert!(u.avionics.clearance.is_none(), "arming gives the clearance up");
        assert!(!u.request_clearance(), "no clearance while armed");
        u.command(&ShipCommands { arm: Some(false), ..u.ship.holding() });
        assert!(u.request_clearance(), "safe again: cleared");
        u.toggle_autopilot();
        u.cancel_clearance();
        assert!(u.avionics.clearance.is_none(), "given up");
        assert_eq!(u.ship.throttle, 0.0, "its autopilot stopped, engines idle");
    }
}
