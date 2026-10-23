# Explorer rear ramp (two leaves) against the game's ramp consumer (engine, 2026-10-22)

Registry `20261022T202822-registry-cff0`: Blender access-revision-01's rear ramp, a 1.69 m panel and a 1.4 m extension,
endpoint geometry only (`rear-ramp-contact.json`: hinge (4.95, 0, 1.75), 124.8° to touch the ground). An audit;
nothing implemented. Read from main at 6ab56878b.

## What the game supports (one rigid ramp)

- Import (`world/src/import.rs`): every mesh whose name contains `Ramp`, and everything under it, is one rigid group
  (`walk`, line 279). Its hinge is the `CargoRamp` mesh's origin and its axis that mesh's local +X (else the `hatch`
  node, the axis across the line to the group's middle). `length` is the farthest point of the group from the
  hinge along the horizontal "out" direction (`ramp`, line 198), as the ramp lies in the exported pose.
- Pose (`world/src/crew.rs:193`): shut (0) unless landed where its hatch opens; then swung by
  `asin((hinge.y − lowest gear foot) / length)`, clamped to at most 90°: its far end level with the feet. Two
  states, no motion between them, no animation curve; the ground is the gear's height, not the terrain under it.
- Walking (`ship_colliders`): the group's triangles turned rigidly about the hinge, walked like any floor (the floor
  rule since d07c6fa4b: the nearest standable surface).

## Gaps for the two-leaf rear ramp

1. One leaf only. An extension named with `Ramp`, or under `CargoRamp`, is bound rigidly to the panel in its
   exported pose. A folded extension stays folded, and an extended one is part of one long straight leaf. No second
   hinge, no relative angle, no sequence.
2. 90° at most. The study's 124.8° swing (a leaf stowed past vertical) can't be reached: `asin` of a clamped ratio
   tops at 90°. And with the leaf stowed near vertical, the horizontal `length` is near zero, so the angle rule
   breaks before the clamp does.
3. Ground by gear height. The toe is put level with the lowest foot, not on the terrain under the toe: a slope,
   a rock or a pad edge is not seen.
4. No intermediate poses. Nothing checks the sweep against the hull, the gear or the ground, so self-collision and
   clearance can't be tested in the game.
5. No support or strength. The leaves are walk surfaces only: no hinge loads, no toe bearing, no locks.
6. Side compartments: no consumer for service or human entry beyond the `hatch` node and walk meshes.

## The contract a compound ramp would need (if the registry chooses it)

- Nodes: `CargoRamp` (panel) and `CargoRampExtension`, its child, each a mesh whose origin is its hinge and whose
  local +X is its axis, exported in the stowed pose. The meshes under each move with it.
- Record: per leaf its range (stowed 0 to deployed angle, either sign) and the order (panel, then extension), the
  deployed angles from the endpoint study, and which leaf's toe meets the ground.
- Engine: leaves posed by their own angles (a chain of hinges); the deploy solved against the terrain under the
  toe, not the gear; the sweep stepped through intermediate poses for collision with hull, gear and ground (refused
  where it hits); the walk mesh as the posed leaves. Tests: closed, intermediate, deployed; a slope; a collision
  refused.
- Not in it: hinge loads, locks, actuators and strength (structure), human envelope and carry (crew route).

Until then the explorer's rear ramp can only be drawn closed, or as one rigid leaf if the extension is merged into
it; neither is the study's mechanism.
