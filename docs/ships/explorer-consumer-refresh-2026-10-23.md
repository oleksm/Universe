# Explorer crew-hatch-01 against main's consumers: refresh (engine, 2026-10-23, audit only)

Registry `20261023T121956-registry-180f`, for Blender's crew-hatch-01 (`explorer.blend`): a two-leaf rear ramp with its
frame, a circular pocket hatch with a folding ladder, compact folding gear and a raised floor. Old exports are invalid.
Main at eb8d79964. No consumer code (import, crew, walk, shape, mounted, ground, legs, deck plans, on foot) has
changed since the hatch audit (bbe234698), so its findings stand. Here, feature by feature:

| Feature | Main supports | Gap |
| --- | --- | --- |
| Several ways out | One: a ramp, if the model has one, else the one `hatch` node's teleport to an implied stair foot | No second exit, no choice between exits; with a ramp the hatch is inert (`explorer-crew-hatch-audit.md`) |
| Circular pocket hatch | A `hatch` node (place and direction); walk meshes; `door_*` static aperture centres | No hatch door, pocket slide or open/shut state; opening size not checked |
| Folding ladder | Deck-plan ladders inside, deck to deck (the walker holds on and climbs) | None from a hatch to the ground; no ladder markers, states, fold, foot on terrain or clearance |
| Two-leaf rear ramp + frame | One rigid leaf (every `*Ramp*` mesh), hinge at `CargoRamp` origin, +X axis, 0 or landed, ≤ 90°, toe at the gear's height | Second leaf, chain posing, > 90° swing, terrain under the toe, sweep checks (`explorer-rear-ramp-audit.md`) |
| Compact folding gear | Static: `gear_*` nodes are the feet; record legs (SFO 15) for landing limits; gear products carried in the fit with their mass (ed70fa59b) | No retraction, stowed state or fold; a gear model with motion is refused unless its fit entry is `pose: rest`, then drawn still (7b5659851) |
| Raised floor | Walk meshes: any standable surface; floor = nearest standable surface down the ray (d07c6fa4b) | None known for a floor itself; step 0.45 m, standable slope over 0.643 rise |
| Engines, RCS | Chemical and cold-gas Engine instances, record-bound nozzles, own nitrogen pool, stated limits (e4a742e1c, 97921ca0e) | Needs the record's engine slots, fit and thruster bindings and real nozzle empties |
| Mass | Registered hull refused with partial or missing mass (ed70fa59b) | The record's whole mass |

Release limits, unchanged: no motion of any mechanism in the game (ramp: two states only); no structural, lock, load
or human/suit/carry qualification; the explorer can't be commissioned until its record and export exist.
