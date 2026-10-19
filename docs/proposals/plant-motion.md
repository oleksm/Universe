# Plant motion in freefall-motion/1 (agreed 2026-10-18)

Blender's rework queue has 21 plant units to rig: the furnaces tilt, cranes traverse and hoist, the mill and the
balling discs spin, the tipper tips, the ladle furnace's roof lifts. Their motion is already data on the unit records
(SFO 26 `motion`: id, kind, point, direction, angle or distance, what moves). But `freefall-motion/1` only knows
ship things (a gimbal, actuators, flexible lines). It has no plain "this node turns about this axis through this range"
or "this node slides along this axis", and no continuous spin. Rigging the 21 against today's format would add nodes
and binds without the engine ever moving them (blender, thread 20261017T103307-blender-e2ba).

## Proposed: `axes`, an optional block of freefall-motion/1

```json
"axes": [
  {"motion": "tilt", "node": "vessel", "kind": "rotate", "pivot": [0, 1.0, 0], "axis": [0, 0, -1], "range": [-0.2618, 0.2618]},
  {"motion": "spin", "node": "drum", "kind": "rotate", "pivot": [0, 3.2, 0], "axis": [1, 0, 0], "continuous": true, "rate": 1.6},
  {"motion": "hoist", "node": "hook", "kind": "translate", "axis": [0, 1, 0], "range": [-16.0, 0.0]}
]
```

| Field | Holds |
|---|---|
| `motion` | the unit record's motion id it carries out (required: every axis answers one record motion) |
| `node` | the glTF node that moves, with everything under it (it is listed in `nodes` with its bind) |
| `kind` | `rotate` or `translate` |
| `pivot` | rotate only: m, a point on the axis, in model-root space (glTF, +Y up) |
| `axis` | unit vector, model-root space |
| `range` | rad or m from the neutral pose, least and most; left out with `continuous` |
| `continuous` | true: turns without end (a mill's drum, a disc) |
| `rate` | rad/s or m/s in service, where the record gives one |
| `parent_axis` | an axis this one rides on (a crane's hoist rides its trolley's traverse) |

The engine drives each axis by one number (an angle or a distance, or a rate for a continuous one) and needs nothing
else to pose the unit. The registry's motion stays the source: `check_motion` in `assets.py` checks each axis against
its record motion. The kind must match; the pivot must lie on the record's axis (the record's frame turned to glTF:
x, z, -y) within 0.1 m; the axis must be parallel; the range must equal the record's.

A unit that travels whole (a car, a crane's run along its rails, a forklift driving) needs no axis: the layout moves it.

## Agreed (engine, 2026-10-18, thread 20261017T104216-registry-79e0)

- Angles in rad, positive right-hand about `axis`; the value 0 is the pose as modelled.
- The moving node's parent in `nodes` (its glTF parent) must be its `parent_axis` axis's node: the engine composes by
  hierarchy, and `parent_axis` is a check.
- A translate has no pivot.
- In the schema (`docs/formats/freefall-motion-1.schema.yaml`, `axes`) and `check_motion` (`assets.py _check_axes`).
  `precheck_model.py` skips the game consumer for plain axes: the engine has none yet and builds it when units go in.

## With actuators and hoses (2026-10-18, blender's EAF, thread 20261018T125615-blender-b71e)

A unit's axes may carry `actuators` and `hoses` as freefall-motion/1 defines them: their anchors and end frames are
parented to an axis's node (or the root), and they follow it. The axes drive; an actuator only follows its two
anchors, and a hose only its two ends. `check_motion` (`assets.py _sweep_axes`) sweeps them over the axes: neutral,
each axis through its range in 8 steps with the others at 0, and every corner of the ranges together. It checks each
actuator's overlap and closed length and each hose's reach. **References** give one input value per axis, in the order
of `axes`, and every listed node's transform must match within 2e-6. Links, bellows and a gimbal together with plain
axes are refused until they are needed. The engine's runtime is the same hierarchy: the axis poses its node, and an
actuator's body and rod aim at their anchors, as on the gimbal (engine's convention v11).

## Ratios (2026-10-18, blender's forklift, thread 20261018T134752-blender-8337)

An axis may carry `ratio`: it moves by that many times its record motion's value. Several axes may answer one record
motion only when each has a ratio, and one input drives them all. A duplex mast's inner mast and its carriage (a child
of the inner mast) each take 0.5 of `mast`, so the forks rise by the whole 3 m. Each axis's range is the record's range
times its ratio. An axis rides the nearest axis above its node in the hierarchy. References give one input per record
motion the axes answer, in the order they first appear. Chain strands that change length are not motion: draw each as
a rigid piece riding the carriage or the inner mast, its excess hidden in the upright's channel past the sheave.

## Spans (2026-10-18, blender's 150 t moon gantry, thread 20261018T231801-blender-1c16)

A hoist rope's falls lengthen as the hook lowers, so a rigid piece will not do. `spans` lists straight members whose
length follows the pose. Each is one node, modelled `unit_length` long along its local `axis`. At a pose its origin sits
at `from`; it is turned toward `to` by the shortest rotation, as an actuator is (v11); then it is stretched along
`axis` to the anchors' distance: M = T(from) R S. Each fall of the reeving is one span (eight falls, eight spans), from
the trolley's sheave block to the hook block's. The check sweeps the axes and holds the anchors' distance within
`min_length` to `max_length`. References check the stretched transform. No axis moves a span's node, and it is listed
under the root only, if at all. The engine draws a span by that transform; it needs no other rope solver. Chains stay
as before: rigid strands, or covered.

## Still asked
- Registry (mine): give the record motions their `rate` where known, and keep `moves` naming the parts so blender knows
  which geometry to separate.
