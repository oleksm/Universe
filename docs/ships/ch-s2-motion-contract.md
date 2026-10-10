# CH-S2 runtime motion: engine reply

2026-10-10. Reply to Blender task `20261010T131922-blender-f497`, after reading
`~/git/blender/engine-ch-s2/refinement-v09/source/motion-contract.json` and
`build_motion.py`. This confirms existing support and the proposed integration
boundary. It does not implement motion or approve an animated installation.

## Current consumer

| Capability | Current state |
|---|---|
| Rigid moving parts | Supported by `PbrModel::load_gltf_parts` and `Frame::model_pbr_part`; used for the ship ramp |
| Registry equipment motion | Not wired: `game::visuals` uses `load_gltf` with no moving-part selection |
| glTF animation clips | No playback/evaluation |
| Bones, skin weights, morph targets | Not evaluated by the PBR loader/vertex path |
| Blender drivers, constraints, curve hooks | No runtime evaluation |
| Runtime nonuniform scale | Not supported by `Transform` (translation, quaternion, uniform scale only) |
| Flexible hoses | No deforming-hose consumer; a static exported mesh is possible, but does not follow motion |

Evidence: `crates/engine/src/pbr.rs` (`PbrVertex`, `load_gltf_parts`, `walk`),
`crates/engine/src/model.rs` (`Transform`), `crates/engine/src/frame.rs`
(`model_pbr_part`), `crates/game/src/models.rs` and `scene.rs` (Ramp), and
`crates/game/src/visuals.rs` (registry packages).

## Rigid-parts contract to prepare

Use live pitch/yaw values to compute rigid part transforms. The frame keys at
24 fps are a verification sweep, not a runtime clip contract. Keep the Blender
rig and sweep as authoring/reference data.

The current loader accumulates the entire glTF node transform into each mesh's
vertices at load time. Part selection matches node-name substrings, inherits the
parent part and lets a matching child select another part. It does not retain a
live transform hierarchy. Give each moving group an unambiguous name and provide
its exported bind transform and parent. Avoid overlapping part-selector names.
The loader reserves part 0 for the static body and numbers selected parts from 1.
The renderer alone does not automatically apply parent-part motion.

If `B` is a part's bind transform in model-root space and `A` its current transform
in that same space (including moving ancestors), draw the baked vertices with
`equipment_placement * A * inverse(B)`. For a pivot-only rotation the delta is
`T(pivot) * R * T(-pivot)`. Do not reapply the bind transform to already baked
vertices. Supply the pivot and axes in exported glTF/model-root coordinates;
`[0, 0, -0.3]` is currently an unlabelled Blender-space value in the JSON, not an
established exported pivot. Export conventions are metres, +Y up.

Add these fields to the handoff, without treating them as a shipped schema:

- Exact static/moving group names, parent names and neutral bind transforms.
- Export coordinate basis, pivot, positive pitch/yaw axes and composition order.
- Input range, normalization and radians mapping. The Blender script already uses
  `d = max(1, sqrt(pitch*pitch + yaw*yaw))`, with stage angles
  `0.07*pitch/d` and `0.07*yaw/d`. Record that rule explicitly, together with the
  nested pitch-parent/yaw-child order. Do not change it to independent ±0.07 rad
  stage limits. The registry remains authority for the equipment limit.
- For each actuator: fixed and moving attachment points in named parent frames,
  cylinder/rod group names, longitudinal axis, neutral transforms, stroke limits,
  body/rod lengths and required minimum overlap. Existing body/rest lengths and
  overlap alone do not locate the linkage. Track both attachment points, rotating
  and sliding rigid cylinder/rod meshes without stretching their cross sections.
- Evaluated reference transforms and actuator endpoint positions at neutral,
  positive/negative single-axis limits and combined limits, after export-axis
  conversion, for engine agreement checks.

## Hoses and installation

The present renderer cannot reproduce the rig's deforming feed hoses. Baking a
clip, exporting an armature, or retaining hooks will not make that work today.
Do not mark the asset motion-complete with a fixed hose that detaches when steered.

Keep the flexible hoses in the Blender source. A runtime route still has to be
chosen and implemented: a deformable mesh/skin path, or an explicitly accepted
articulated approximation with its own clearance and endpoint checks. Neither is
promised by this reply. Hose neutral centreline, end frames and bend constraints
will be needed for that choice. A neutral-pose static preview can be reviewed
separately and must be labelled static.

The next integration work is part-aware equipment loading/placement, live gimbal
state and rigid-linkage evaluation, followed by an agreed hose representation and
motion/clearance tests. No package is installed by this confirmation.
