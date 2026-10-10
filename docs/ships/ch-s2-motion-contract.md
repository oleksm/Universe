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
The v09 delivery updates the Blender-space pivot to `[0, 0, -0.43]`, superseding
the earlier JSON value `[0, 0, -0.3]`. Neither is an exported pivot: regenerate
the converted motion evidence from the latest source. Export conventions are
metres, +Y up.

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

The selected CH-S2 runtime route is a procedural tube with fixed topology and
updated vertex positions/normals. No clips, skinning or morph playback is needed.
This requires a new dynamic PBR mesh update path; today's rigid-part support alone
cannot do it. Keep the Blender hoses and evaluated sweeps as reference evidence.

End frames, diameter and free length are necessary but do not uniquely specify a
routed hose. Supply the rest centreline, end tangent axes, routing guides and their
parent frames, minimum bend radius and clearance constraints. Use a deterministic
curve solver and stable transported cross-section frames. Preserve the declared
free length within an agreed tolerance; reject infeasible endpoint distances,
bend radii or routing instead of silently stretching the hose. Compare endpoints,
tangents, arc length, bends and clearances throughout the steering envelope.

A static preview uses the complete neutral assembly. Do not steer rigid parts
while leaving their hoses frozen. A failed motion validation prevents enabling
motion for that package. The latest v09 delivery is Building, accepted=false and
installed=false; a technically loadable neutral export is not asset acceptance.

## Confirmed engine sequence (registry steps 3–5)

Reply to registry task `20261010T134430-registry-c09b`, reviewing
`~/git/universe-fso/docs/ships/ch-s2-motion-plan.md`. This confirms the clipless
approach with the amendments below; it is an implementation plan, not a claim
that these consumers exist yet.

1. **Agree the data contract before the consumer.** Move registry step 6 ahead of
   runtime motion loading. Registry owns `freefall-motion/1`, installer validation
   and the equipment limit; engine and Blender provide consumer/export fixtures.
   Distinguish model-root bind transforms from attachment transforms local to a
   named parent. Both use the exported basis, metres and radians, but are not the
   same coordinate frame. Validate unique nodes, parent acyclicity, finite rigid
   transforms, frame references, dimensions and package/model association.
2. **Step 3: mount placement and neutral equipment.** Resolve each fitted device
   to its model and hull mount. Compose the full hull mount frame with the inverse
   equipment mount frame, preserving roll and the authored datum. Retain any
   orientation missing from the current import path; do not centre by bounds.
   Add part-aware loading, neutral bounds and a neutral placement comparison on
   the target hull. This can proceed alongside the schema/fixture work.
3. **Step 4a: actual device state.** There is no engine gimbal command/state today
   (`protocol::ShipCommands` has throttle and attitude-rate commands, not a
   per-engine pitch/yaw setting). Define the device command and applied state,
   core clamping from the registry limit, view exposure and save/replay behavior.
   Do not directly reinterpret the attitude-rate stick as a gimbal angle. Client
   flight control owns command allocation; core applies device commands. Use the
   applied angle for both thrust direction/torque and rendered nozzle direction.
   Any slew/response law requires specification; do not invent a rate.
4. **Step 4b: rigid motion.** Evaluate nested pitch/yaw and the rigid sliding
   actuator linkage from that state, using the bind-space deltas above. Verify
   exported neutral, axis-limit and combined-limit reference poses, actuator
   attachment agreement, stroke/overlap and clearance. Independent instances
   must not share mutable pose state. Reference fixture poses can be evaluated
   before 4a, but live motion depends on it.
5. **Step 5: flexible motion.** Implement the routed tube evaluator and bounded
   per-instance dynamic vertex updates, reusing topology/buffers rather than
   generating new model IDs each frame. Include normals, shadow rendering and
   motion bounds. Validate hose seating, tangents, length, bend radius and
   interference over the same full-envelope fixtures, then measure update cost.
6. **Integration gate.** Check mounted motion on the actual hull, command/render/
   force agreement, independent instances and save/replay. Enable moving
   installation only after the schema, exporter, runtime and asset QC agree.
   Static installation remains a separately accepted neutral package after
   Blender's open pipe/manifold/datum/mass/thermal checks and owner acceptance.

Turbulence remains an owner/spec decision. Do not silently add invented noise to
the command or applied thrust. A physically moving nozzle must remain consistent
with its thrust direction; any approved purely cosmetic vibration needs its own
explicit scope and attachment behavior. The v09 turbulence clip is review
reference, and the steering sweep remains QC evidence.
