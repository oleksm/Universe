//! The follow program in the tick: where an anchoring ship is (as the
//! follower's radar would measure it), and the pilot's commands to keep at a
//! range from the locked contact or the nav target, or to orbit it.

use glam::DVec3;
use universe_avionics::follow::{Anchor, Manoeuvre};
use universe_world::radar::RADAR_RANGE;

use crate::universe::Universe;

/// Which kind of follow the pilot asked for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FollowKind {
    KeepAt,
    Orbit,
}

impl Universe {
    /// Keep at a range from, or orbit, the locked contact (else the nav
    /// target, a station or gate): the cockpit's follow program.
    pub fn follow(&mut self, kind: FollowKind) {
        self.in_cockpit(|k| k.follow(kind));
    }

    /// What we're following, for the display: the manoeuvre, the anchor's
    /// name, and how far it is now (m).
    pub fn following_status(&mut self) -> Option<(Manoeuvre, String, f64)> {
        self.cockpit_now().following_status()
    }

    /// Stop following.
    pub fn stop_following(&mut self) {
        self.in_cockpit(|k| k.stop_following());
    }

    /// Craft `i` follows `anchor`.
    pub fn craft_follow(&mut self, i: usize, anchor: Anchor, manoeuvre: Manoeuvre) {
        self.craft_run(i, |a, link, events| a.follow(link, anchor, manoeuvre, events));
    }
}

/// Ship `id` as the frame's snapshot has it, seen from `from` in `system`.
pub(crate) fn mark_in(snaps: &[crate::traffic::Snap], system: usize, from: DVec3, id: usize) -> Option<(DVec3, DVec3)> {
    let s = snaps.get(id)?;
    (s.system == system && s.flying && !s.hyperdrive && s.position.distance(from) < RADAR_RANGE).then_some((s.position, s.velocity))
}

