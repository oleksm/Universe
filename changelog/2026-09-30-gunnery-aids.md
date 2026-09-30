# 2026-09-30 — Gunnery: boresight, gimballed gun, hit feedback

User: "what is the range of my guns? I cant seem to shot down anyone even from 3 km."

Range wasn't the problem: the gun reaches about 30 km (3 km/s for 10 s) and the laser 30 km (full
power within 2 km). The problem was aiming:
- the chase view (the default) showed no aim point at all;
- a 12 m ship at 3 km is about ±0.23°, a couple of pixels, and the gun fired exactly along the
  nose;
- there was no feedback on hits.

The laser at 3 km gets about 0.9 MW, roughly 36% of a hull before it overheats: it's a
close-range weapon.

- **Boresight in every view**: a small cross on the nose in the chase view (the cockpit has its
  crosshair).
- **Gimballed gun** (world device):
  - `Ship::gun_dir` (ship frame) swings up to `GIMBAL_LIMIT` = 4° off the nose at
    `GIMBAL_RATE` = 30°/s, toward `Ship::gun_target`, which is commanded with
    `ShipCommands::gun_target`.
  - `weapons::lay_gun` runs in the combat phase. The gun and the laser fire along
    `Ship::gun_forward()`.
  - In combat mode, fire control lays the gun on the lead (`Universe::fire_control` commands
    it), and parks it on the nose when there's no solution.
  - Slugs still fly physically. The pilot's job becomes flying the lead into the gimbal's ring,
    not pixel-perfect aim.
- **HUD**:
  - In combat mode, the gimbal's reach is a ring around the nose, and the gun's pipper shows where
    it points.
  - The lead circle is dim outside the ring, amber inside, and doubled when the gun is on it.
  - The fire-control line reads `FLY THE LEAD INTO THE RING` / `LAYING GUN` / `GUN ON TARGET`.
- **Hit feedback**:
  - `World::impacts`: where rounds and beams struck this frame, and whose.
  - Sparks at each hit, screen-sized so they show at any range, brighter for ours.
  - `HIT` next to the locked target when ours land, with a confirmation tick.
  - The radar still doesn't reveal the target's hull; you see the impacts.

Tests: `the_gimbal_lays_the_gun_within_its_cone_at_its_rate` (world),
`the_gimbal_hits_with_the_nose_two_degrees_off` (sim: a pilot holding the nose 2° off the lead
still shoots the settler down). The `gunnery` scenario now holds the nose 2° off, and the
screenshot shows `GUN ON TARGET`.

## Follow-up: no crosshair in travel

User: "in travel mode I am not sure I need a crosshair". The nose crosshair (the cockpit cross,
and the nose cross in the chase view) now shows only in combat mode or during an approach, where
the HUD asks you to put the nose on the plan's cue. In plain travel it's hidden.
