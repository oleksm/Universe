//! The combat phase of the tick, and fire control for the player's ship.
//!
//! After every ship has taken its turn, the world runs weapons for all of
//! them at once (guns fire, slugs fly, lasers cut, hits land), and what
//! happened goes to each ship's avionics like any other physical event.
//! Ships are known to weapons by id: the player's ship is `PLAYER`, craft
//! `i` is `i + 1`.

use universe_avionics::fire_control::{lead, Solution, Track};
use universe_world::weapons::{Armed, GUN_MUZZLE, SLUG_LIFETIME};
use universe_world::ShipEvent;

use crate::contacts::Contact;
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
        let mut player_events = Vec::new();
        let mut craft_events: Vec<Vec<ShipEvent>> = (0..self.crafts.len()).map(|_| Vec::new()).collect();
        {
            let mut armed = Vec::with_capacity(self.crafts.len() + 1);
            armed.push(Armed { id: PLAYER, system: self.ship_system, ship: &mut self.ship, events: &mut player_events });
            for (i, (c, e)) in self.crafts.iter_mut().zip(craft_events.iter_mut()).enumerate() {
                armed.push(Armed { id: craft_id(i), system: c.system, ship: &mut c.ship, events: e });
            }
            self.world.combat(&mut armed, dt);
        }
        self.avionics.record(player_events, &mut self.events);
        let mut ignored = Vec::new();
        for (i, events) in craft_events.into_iter().enumerate() {
            if events.is_empty() {
                continue;
            }
            if events.iter().any(|e| matches!(e, ShipEvent::Crashed { .. })) {
                self.traffic.shot_down += 1;
            }
            self.crafts[i].avionics.record(events, &mut ignored);
            ignored.clear();
        }
    }

    /// Name of the ship with combat id `id`.
    pub fn ship_name(&self, id: usize) -> String {
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
            return None;
        };
        Track::update(&mut self.avionics.track, c.blip.id, c.blip.position, c.blip.velocity, self.world.time);
        let track = self.avionics.track?;
        let solution = track
            .ready()
            .then(|| lead(self.ship.position, self.ship.velocity, c.blip.position, c.blip.velocity, track.acceleration, GUN_MUZZLE, SLUG_LIFETIME))
            .flatten();
        Some((track, solution))
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
        let (sys, pos, vel) = (u.ship_system, u.ship.position, u.ship.velocity);
        let c = &mut u.crafts[0];
        c.system = sys;
        c.ship.state = ShipState::Flying;
        c.ship.hyperdrive = false;
        c.ship.position = pos + DVec3::new(0.0, 3_000.0, 0.0);
        c.ship.velocity = vel + DVec3::new(40.0, 0.0, 0.0);
        c.avionics = Default::default();
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
