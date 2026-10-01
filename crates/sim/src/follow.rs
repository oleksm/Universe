//! The follow program in the tick: where an anchoring ship is (as the
//! follower's radar would measure it), and the pilot's commands to keep at a
//! range from the locked contact or the nav target, or to orbit it.

use glam::DVec3;
use universe_avionics::follow::{self, Anchor, Manoeuvre};
use universe_avionics::{Event, NavTarget};
use universe_world::radar::RADAR_RANGE;
use universe_world::{Controls, Ship};

use crate::combat::{craft_id, PLAYER};
use crate::universe::Universe;

/// Which kind of follow the pilot asked for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FollowKind {
    KeepAt,
    Orbit,
}

impl Universe {
    /// Ship `id` (the player's 0, craft i: i + 1), as seen from `from` in
    /// `system`: where it is and how it moves, if it's there to be seen
    /// (flying in normal space, in radar range).
    pub(crate) fn ship_mark(&self, system: usize, from: DVec3, id: usize) -> Option<(DVec3, DVec3)> {
        let (s, ship): (usize, &Ship) = if id == PLAYER { (self.ship_system, &self.ship) } else { self.crafts.get(id - 1).map(|c| (c.system, &c.ship))? };
        (s == system && ship.is_flying() && !ship.hyperdrive && ship.position.distance(from) < RADAR_RANGE).then_some((ship.position, ship.velocity))
    }

    /// The follow program's stick for the player this frame, if it's engaged.
    pub(crate) fn player_follow(&mut self) -> Option<Controls> {
        let f = self.avionics.following?;
        let mark = match f.anchor {
            Anchor::Ship(id) => self.ship_mark(self.ship_system, self.ship.position, id),
            Anchor::Place(_) => None,
        };
        let (world, mut player) = self.player();
        player.run(world, |a, link, events| a.follow_step(link, mark, events))
    }

    /// Keep at a range from, or orbit, the locked contact (else the nav
    /// target, a station or gate). Asked again for the same kind: the next
    /// range out. The first range is the preset nearest where we are.
    pub fn follow(&mut self, kind: FollowKind) {
        let anchor = match (self.avionics.contact, self.avionics.nav_target) {
            (Some(c), _) => Anchor::Ship(craft_id(c)),
            (None, Some(t @ (NavTarget::Station(_) | NavTarget::Gate(_)))) => Anchor::Place(t),
            _ => {
                self.events.push(Event::Refused { reason: "FOLLOW: LOCK A SHIP (T), OR A STATION OR GATE (M)".into() });
                return;
            }
        };
        let sys = self.ship_system();
        let min = follow::min_range(&sys, anchor);
        let range = match self.avionics.following {
            Some(f) if f.anchor == anchor && same_kind(f.manoeuvre, kind) => follow::next_range(f.manoeuvre.range(), min),
            _ => {
                let at = match anchor {
                    Anchor::Ship(id) => self.ship_mark(self.ship_system, self.ship.position, id).map(|m| m.0),
                    Anchor::Place(t) => self.target_position(t),
                };
                let Some(at) = at else {
                    self.events.push(Event::Refused { reason: "FOLLOW: TARGET NOT IN SIGHT".into() });
                    return;
                };
                follow::nearest_range(at.distance(self.ship.position), min)
            }
        };
        let manoeuvre = match kind {
            FollowKind::KeepAt => Manoeuvre::KeepAt(range),
            FollowKind::Orbit => Manoeuvre::Orbit(range),
        };
        let (world, mut player) = self.player();
        player.run(world, |a, link, events| a.follow(link, anchor, manoeuvre, events));
    }

    /// What we're following, for the display: the manoeuvre, the anchor's
    /// name, and how far it is now (m).
    pub fn following_status(&mut self) -> Option<(Manoeuvre, String, f64)> {
        let f = self.avionics.following?;
        let (name, at) = match f.anchor {
            Anchor::Ship(id) => {
                let name = self.ship_name(id);
                (name, self.ship_mark(self.ship_system, self.ship.position, id)?.0)
            }
            Anchor::Place(t) => (self.target_name(t).to_uppercase(), self.target_position(t)?),
        };
        Some((f.manoeuvre, name, at.distance(self.ship.position)))
    }

    /// Stop following.
    pub fn stop_following(&mut self) {
        let (world, mut player) = self.player();
        player.run(world, |a, link, events| a.stop_following(link, events));
    }

    /// Craft `i` follows `anchor`.
    pub fn craft_follow(&mut self, i: usize, anchor: Anchor, manoeuvre: Manoeuvre) {
        self.craft_run(i, |a, link, events| a.follow(link, anchor, manoeuvre, events));
    }
}

fn same_kind(m: Manoeuvre, kind: FollowKind) -> bool {
    matches!((m, kind), (Manoeuvre::KeepAt(_), FollowKind::KeepAt) | (Manoeuvre::Orbit(_), FollowKind::Orbit))
}
