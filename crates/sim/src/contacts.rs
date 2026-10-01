//! Radar contacts for the player's ship: what its radar sees (a world
//! device), who each is (the crafts' transponders: name, what it's doing,
//! where it's bound), and the lock its avionics hold on one of them.

use universe_avionics::route;
use universe_avionics::NavTarget;
use universe_world::radar::{self, Blip};
use universe_world::ShipState;

use crate::traffic::Craft;
use crate::universe::Universe;

/// A ship on the radar, with its transponder's answer.
#[derive(Clone, Debug)]
pub struct Contact {
    /// Position, velocity and distance, as the radar measures them; `blip.id`
    /// is the craft's index.
    pub blip: Blip,
    pub name: String,
    /// What it's doing ("DOCKING", "HYPERDRIVE", ...).
    pub activity: &'static str,
    /// Where it's bound, if it says.
    pub destination: Option<String>,
    /// Hull integrity 0..1, as a combat scan reads it (shown in combat mode).
    pub hull: f64,
    /// Aggressed: fair game (it opened fire on someone).
    pub aggressed: bool,
}

/// Half-angle of the lock beam around the nose (rad): 6°.
pub const LOCK_BEAM: f64 = 6.0 * std::f64::consts::PI / 180.0;

/// What a craft's transponder says it's doing.
pub(crate) fn activity(craft: &Craft) -> &'static str {
    let a = &craft.avionics;
    // Weapons hot is plain to see, whatever the transponder says.
    if craft.ship.weapons_hot() {
        return "WEAPONS HOT";
    }
    match craft.ship.state {
        ShipState::Landed { .. } if a.route.dwell_until.is_some() => "AT STOP",
        ShipState::Landed { .. } => "PARKED",
        _ if a.route.departing => "DEPARTING",
        _ if craft.ship.hyperdrive => "HYPERDRIVE",
        _ => match a.clearance.map(|c| c.target) {
            Some(NavTarget::Station(_)) => "DOCKING",
            Some(NavTarget::Spaceport(_)) => "LANDING",
            Some(NavTarget::Gate(_)) => "GATE RUN",
            None => "UNDERWAY",
        },
    }
}

impl Universe {
    /// Ships the radar sees, nearest first.
    pub fn contacts(&mut self) -> Vec<Contact> {
        let blips = radar::sweep(&self.ship, self.ship_system, self.crafts.iter().enumerate().map(|(i, c)| (i, c.system, &c.ship)));
        blips
            .into_iter()
            .map(|blip| {
                let craft = &self.crafts[blip.id];
                let (name, activity) = (craft.name.to_uppercase(), activity(craft));
                let r = &craft.avionics.route;
                let stop = r.stops.get(r.next).copied();
                let destination = stop.map(|s| route::stop_name(&self.world.system(s.system), s).to_uppercase());
                Contact { blip, name, activity, destination, hull: craft.ship.hull, aggressed: self.law.aggressed(crate::combat::craft_id(blip.id), self.world.time) }
            })
            .collect()
    }

    /// Lock the next contact out from the one locked (the nearest, if none);
    /// past the farthest, unlock. Returns the new lock.
    pub fn lock_next_contact(&mut self) -> Option<Contact> {
        let contacts = self.contacts();
        let from = self.locked_contact_in(&contacts).map(|c| (c.blip.distance, c.blip.id));
        let next = contacts.into_iter().find(|c| from.is_none_or(|(d, id)| (c.blip.distance, c.blip.id) > (d, id)));
        self.avionics.contact = next.as_ref().map(|c| c.blip.id);
        next
    }

    /// With the collision warning on, and flying in normal space: the path
    /// ahead and what it would hit (radar contacts count, as they're moving).
    pub fn collision_warning(&mut self, contacts: &[Contact]) -> Option<universe_avionics::collision::Prediction> {
        use universe_avionics::collision::{predict, Traffic, RANGE};
        if !self.avionics.collision_warning || !self.ship.is_flying() || self.ship.hyperdrive {
            return None;
        }
        let traffic: Vec<Traffic> = contacts
            .iter()
            .filter(|c| c.blip.distance < RANGE)
            .map(|c| Traffic { name: c.name.clone(), position: c.blip.position, velocity: c.blip.velocity })
            .collect();
        let sys = self.ship_system();
        let mut positions = Vec::new();
        sys.positions(self.world.time, &mut positions);
        Some(predict(&sys, &self.ship, self.world.time, &positions, &traffic))
    }

    /// Lock in the beam: of the contacts within `LOCK_BEAM` of the nose, the
    /// one nearest the crosshair, or, if that's already locked, the next one
    /// out from it. Nothing in the beam: the lock is released.
    pub fn lock_in_beam(&mut self) -> Option<Contact> {
        let nose = self.ship.forward();
        let own = self.ship.position;
        let off = |c: &Contact| (c.blip.position - own).angle_between(nose);
        let mut beam: Vec<Contact> = self.contacts().into_iter().filter(|c| off(c) <= LOCK_BEAM).collect();
        beam.sort_by(|a, b| off(a).total_cmp(&off(b)).then(a.blip.id.cmp(&b.blip.id)));
        let next = match self.avionics.contact.and_then(|id| beam.iter().position(|c| c.blip.id == id)) {
            Some(i) => beam.into_iter().cycle().nth(i + 1),
            None => beam.into_iter().next(),
        };
        let next = next.filter(|c| Some(c.blip.id) != self.avionics.contact || self.avionics.contact.is_none());
        self.avionics.contact = next.as_ref().map(|c| c.blip.id);
        next
    }

    /// The locked contact, if it's still on the radar.
    pub fn locked_contact_in<'a>(&self, contacts: &'a [Contact]) -> Option<&'a Contact> {
        let id = self.avionics.contact?;
        contacts.iter().find(|c| c.blip.id == id)
    }
}

#[cfg(test)]
mod tests {
    use glam::DVec3;

    use crate::universe::Universe;

    #[test]
    fn t_locks_what_is_in_the_beam_nearest_the_crosshair() {
        let mut u = Universe::new(1984);
        u.spawn_settlers(3, 1);
        let (sys, pos, nose) = (u.ship_system, u.ship.position, u.ship.forward());
        let side = nose.any_orthonormal_vector();
        // Two ahead (1° and 3° off the nose), one 20° off.
        let place = [(1.0f64, 5_000.0), (3.0, 2_000.0), (20.0, 1_000.0)];
        for (c, (deg, d)) in u.crafts.iter_mut().zip(place) {
            c.system = sys;
            c.ship.state = universe_world::ShipState::Flying;
            let dir = (nose + side * deg.to_radians().tan()).normalize();
            c.ship.position = pos + dir * d;
        }
        assert_eq!(u.lock_in_beam().map(|c| c.blip.id), Some(0), "nearest the crosshair");
        assert_eq!(u.lock_in_beam().map(|c| c.blip.id), Some(1), "next in the beam");
        assert_eq!(u.lock_in_beam().map(|c| c.blip.id), Some(0), "round again");
        // Turn away: nothing in the beam, the lock goes.
        u.ship.orientation = universe_world::ship::facing(-nose, side);
        assert!(u.lock_in_beam().is_none());
        assert!(u.avionics.contact.is_none());
    }

    #[test]
    fn radar_locks_contacts_nearest_first_and_loses_them_out_of_range() {
        let mut u = Universe::new(1984);
        u.spawn_settlers(3, 1);
        let (sys, pos) = (u.ship_system, u.ship.position);
        for (i, c) in u.crafts.iter_mut().enumerate() {
            c.system = sys;
            c.ship.state = universe_world::ShipState::Flying;
            c.ship.position = pos + DVec3::X * 1_000.0 * (3 - i) as f64;
        }
        let order: Vec<usize> = u.contacts().iter().map(|c| c.blip.id).collect();
        assert_eq!(order, [2, 1, 0]);
        assert_eq!(u.lock_next_contact().map(|c| c.blip.id), Some(2));
        assert_eq!(u.lock_next_contact().map(|c| c.blip.id), Some(1));
        assert_eq!(u.lock_next_contact().map(|c| c.blip.id), Some(0));
        assert!(u.lock_next_contact().is_none(), "past the farthest: unlocked");
        u.lock_next_contact();
        u.crafts[2].ship.position = pos + DVec3::X * 1.0e6;
        let contacts = u.contacts();
        assert!(u.locked_contact_in(&contacts).is_none(), "out of range: lost");
    }
}
