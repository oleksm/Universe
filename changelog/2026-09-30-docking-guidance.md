# 2026-09-30 — Docking guidance that starts from where you are

## Summary

Playtest feedback: with only a fixed corridor, there was no sense of your own direction and speed
relative to the station, so the corridor could never be caught. Guidance now starts at the ship,
from its actual position and motion.

## Changes

- **Station-relative velocity markers** (cyan) once cleared to dock. Previously the markers showed
  motion relative to the planet, which is meaningless for docking.
- **Predicted path** (cyan, dashed): where you'll drift over the next 30 s relative to the station,
  with a tick every 5 s so the spacing shows speed.
- **Guidance path** (magenta, dashed): a curve from the ship, leaving along its current motion and
  bending into the next waypoint along the corridor direction. Recomputed every frame.
- **Fly-to cue** (magenta square) where your velocity should point; a small circle marks the waypoint.
- **Thrust hint**: which Shift+key thrusters to fire and how much, e.g. `SHIFT+ D 9  E 101  S 31 M/S`,
  or `THRUST: NONE - ON THE PATH`. Plus a `REL SPEED` readout.
- **Shared guidance** in `sim::docking::guidance`: the HUD and the docking computer now use the same
  waypoints and desired velocity. Routing goes straight to the corridor entrance when that path stays
  1.5 km clear of the station; otherwise it goes around.

## Dev

- `offcourse` scenario: cleared but 6 km off-axis, drifting sideways at 40 m/s.
- `UNIVERSE_CAM=cockpit|map` overrides the camera for any scenario (map = observer watching the
  ship from 7 km). Scenario screenshots no longer include the startup messages.

## Follow-up: guidance led away from the station

- Playtest: "it seems like always pointing away of station". The guidance always routed to the
  corridor entrance 4 km out along the docking axis; from the spawn point that's 4 km *above* the
  station, so the cues led away first.
- Guidance now joins the corridor at the ship's own height (clamped to 1.2–4 km), flying straight
  there whenever the path stays 1 km clear of the station. Pointing toward the station, measured
  as the cosine between the guidance direction and the direction to the station, went from 0.67 to
  0.96 at spawn and from 0.83 to 0.98 from 6 km to the side.
- Recommended approach speed lowered (max 250 m/s, gentler braking profile).
- The docking computer uses the same entry point: 150 s from spawn instead of 229 s.
- HUD shows `GO <speed>` (recommended speed) outside the corridor and `LIMIT <speed>` only inside it.
- New `cleared` scenario (spawn point with clearance) and an ignored `print_guidance` test for
  probing guidance direction from sample positions.
