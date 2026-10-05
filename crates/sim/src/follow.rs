//! The follow program in the tick: where an anchoring ship is (as the
//! follower's radar would measure it), and the pilot's commands to keep at a
//! range from the locked contact or the nav target, or to orbit it.

use glam::DVec3;


/// Which kind of follow the pilot asked for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FollowKind {
    KeepAt,
    Orbit,
}


/// Ship `id` as the frame's snapshot has it, seen from `from` in `system` by
/// sensors that see `range` (m).
pub(crate) fn mark_in(snaps: &[crate::traffic::Snap], system: usize, from: DVec3, range: f64, id: usize) -> Option<(DVec3, DVec3)> {
    let s = snaps.get(id)?;
    (s.system == system && s.flying && !s.hyperdrive && s.position.distance(from) < range).then_some((s.position, s.velocity))
}

