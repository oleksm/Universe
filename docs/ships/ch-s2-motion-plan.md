# ch.s2 delivery: moving parts without clips (proposal, 2026-10-10)

For `equipment.engine.ch.s2` refinement-v09 (blender `engine-ch-s2/refinement-v09/delivery.yaml`), after engine's
consumer review (`docs/ships/ch-s2-motion-contract.md` on main, 462a7b45). Status: **proposed**; engine to confirm
items 3-5.

## The physics view

The gimbal's angle is ship state, not a film: the game's flight model commands pitch and yaw within the record's limit
(`function.gimbal` 0.07 rad), and everything that moves with the gimbal follows from that angle. So nothing needs a clip:

- the two gimbal stages are rigid parts turned about the pivot by the commanded pitch and yaw (nested: pitch parent, yaw
  child, Blender's normalisation `d = max(1, sqrt(pitch^2 + yaw^2))`, stage angles `0.07 pitch/d`, `0.07 yaw/d`);
- each actuator is two rigid parts (body on its fixed anchor, rod on its moving anchor), each aimed at the other's anchor;
  its length follows from the anchors, so it needs no keys;
- each flexible hose is a tube between two end frames (one on the fixed structure, one on the gimbal) with its diameter
  and free length: drawn by the game as a bent tube between them, so it needs no morphs;
- the turbulence (0.02 rad peak, 1.625 to 5.125 Hz in v09's preview) is invented visual motion. If the owner keeps it, it is a small
  noise on the commanded angle in the game (labelled invented), not a clip in the model.

## The package

1. **model.glb, static** (what the installer takes today): the rest pose, every moving group its own rigid mesh under an
   unambiguous node (static body; gimbal stages; each actuator's body and rod), hoses drawn at rest as a fallback.
   Clips, skins and morphs stay in the review export only.
2. **motion data** beside it: the pivot, axes and order, stage limits and normalisation, each moving node's parent and
   bind transform, each actuator's two anchors (parent frame, point) and lengths, each hose's two end frames, diameter
   and free length; all in model-root coordinates (metres, +Y up). For now in the modeller's `--about` account
   (`evidence.motion`); when engine reads it, it becomes a checked package file (`motion.json`, format
   `freefall-motion/1`, schema in the registry, checked by the installer).

## Next steps and owners

| # | Step | Owner |
|---|---|---|
| 1 | Static rest-pose export with the named rigid groups; motion data in model-root coordinates (from motion-contract.json, Blender-space values converted); finish the open QC (pipes, manifolds, datum, mass, thermal) | blender |
| 2 | Accept the candidate (turbulence: keep as an in-game noise or drop) | owner |
| 3 | Draw equipment at its mounts (models exist for settlement things only) | engine |
| 4 | Drive the gimbal nodes from the commanded angle; aim the actuator parts at their anchors | engine |
| 5 | Hoses: a bent tube between two end frames (preferred: no morphs to carry), or morph support | engine decides |
| 6 | `freefall-motion/1`: schema, installer check, the record stays the authority on the limit | registry, once 4-5 are agreed |
| 7 | Install (static package now is fine: the game draws it rigid when 3 lands); validate: loader test, frames once drawn | blender installs, registry validates |
