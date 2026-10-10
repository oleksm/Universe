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
vertices. Supply the pivot and axes in exported glTF/model-root coordinates.
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

## Feed ducts and installation — supersedes the hose route

2026-10-10 update for Blender task `20261010T140310-blender-6982`, based on
registry commit `b5951123`, `docs/ships/ch-s2-feed-lines.md` on fso. Stop the
previous CH-S2 procedural-hose implementation plan. The v10 hose fixtures fail
bend/length checks and remain negative reference cases, not enabled motion.

Registry now recommends two tied bellows joints and a rigid duct between them
for each feed, with fixed inlet flanges at the mount face and pump inlets near
the gimbal plane. The stated bore ranges are hydrogen 85–120 mm and oxygen
50–70 mm. These are registry design inputs, not renderer constants; the inlet
velocity rule and joint ratings remain provisional pending registry/source work.

**Runtime decision:** reuse rigid-part transforms for the central ducts, flanges,
gimbal rings and tie hardware. No animation clips or morph playback is required
for these parts. Reusing the aiming calculation from an actuator is appropriate;
reusing its telescoping behavior for a fixed-length duct is not.

The blockout/export must provide:

- Exact node names, neutral binds, local longitudinal and roll-reference axes;
  both joint centres and attachment orientations in explicitly named parent
  frames, converted to the exported coordinate basis.
- Fixed centre-to-centre length and closure tolerance for each rigid link. At
  every pose, check `abs(length(B - A) - rest_length) <= tolerance`. A rigid
  transform cannot satisfy two anchors whose separation changes. Reject such a
  layout or explicitly model the additional rated degree of freedom; do not
  scale the duct or silently allow actuator-like sliding.
- Each joint's actual degrees of freedom, hinge axes/order, neutral alignment,
  angular limits and any permitted axial/lateral travel and torsion. A two-axis
  gimbal is not an unrestricted ball joint. Do not infer joint ratings from the
  engine's 0.07 rad steering limit. Include source/provisional status for ratings.
- A deterministic roll rule from the joint frames, including singular cases.
  A direction between two points alone leaves roll undetermined; tie rods and
  gimbal axes must follow the same solved frames, not independent look-at rules.
- Bore, outside envelope, bellows active span, tie attachment frames and lengths,
  and collision envelopes. Provide neutral, signed axis/combined limits and a
  dense steering sweep with worst-case closure, joint travel, tie closure and
  clearance residuals. Identify sampled clearance evidence as sampled.

Evaluate the engine pose first, then attachment frames, rigid-link closure and
joint orientation, and finally the hardware transforms. Apply the existing
`placement * current * inverse(bind)` drawing rule. This is kinematic geometry
validation, not pressure, fatigue or cryogenic qualification.

**Bellows surface is a separate representation decision.** A corrugated mesh
cannot stay attached to two relatively rotating flanges through one rigid
transform. Blender should provide the end frames, rest profile and an explicit
visual proposal. A rigid overlapping cover may suffice only if that approximation
is accepted and its seam/clearance sweep passes; otherwise the bellows needs a
small procedural deformation path. Neither generic hose routing nor skin/morph
support is a prerequisite we should implement speculatively. Do not call the
whole assembly motion-complete while the bellows visibly separates or intersects.

Registry's proposed reuse of a rigid-link schema is acceptable only if it encodes
these length, frame, roll and joint constraints. A joint angle field alone is
insufficient. Registry owns the schema; these are consumer requirements, not a
new shipped format. Missing ratings permit labelled blockout review only.

A static preview uses the complete neutral assembly. Failed motion validation
prevents enabling motion for that package. Technical runtime agreement does not
accept the asset or authorize installation.

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
5. **Step 5: tied feed ducts (revised).** Validate Blender's new joint layout
   against fixed-length closure, allowed joint travel, deterministic roll, tie
   closure and clearances before enabling rigid duct motion. Reuse the part
   evaluator from step 4b. Settle and validate the bellows surface representation
   separately; implement a bounded procedural update only if that representation
   requires it. The previous routed-hose solver is removed from this sequence.
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

## Proposal-v03 runtime/schema review

2026-10-10, Blender task `20261010T141306-blender-c688`. Reviewed
`~/git/blender/engine-ch-s2/proposal-v03/delivery.yaml`, `review/duct-study.json`,
`source/build_duct_study.py` and the neutral GLB against registry `c5ae8c1a`
(`docs/formats/freefall-motion-1.schema.yaml`, `tools/standards/assets.py`).

**Conclusion: the axis-aligned rigid mechanism is suitable for a runtime fixture.**
It is not an accepted production engine or a schema-conforming package. Both
spools inherit pitch; the first hinge joins root to pitch on the pitch axis,
the second joins pitch to yaw on the yaw axis. Use inherited transforms for the
spools, cuffs and hinge hardware. No independent look-at or roll solver is needed
for this layout, and no part should receive both inherited and solved motion.

Independent check using the exported pivot/axes, Rodrigues rotations in double
precision, radial inputs at radii 0, .25, .5, .75 and 1 (128 directions per nonzero
radius), and the nine supplied reference matrices:

| Check | Engine result |
|---|---|
| Sampled poses | 513 |
| Maximum chord-length residual | 3.033e-8 m |
| Maximum paired joint-centre residual | 2.503e-10 m |
| Maximum component error against supplied stage matrices | 2.371e-8 |
| Projected first-hinge roll vs inherited pitch, maximum vector residual | 2.087e-10 |
| GLB bind-node names | All five present |
| GLB animations / skins | 0 / 0 |

Input GLB SHA-256 matches the delivery:
`5579cfc4037acf54762e2cb1c39431f5ad58024e9c4a69105d6a7e9a05b6f096`.
This independently supports the kinematics; it does not rerun Blender's mesh
clearance or validate bellows deformation. Reference matrices currently cover
only the two stages. Add spool, cuff, hinge and bellows-part references in the
next fixture so the complete exported hierarchy can be compared.

### Schema mapping and validator amendments

- Represent each rigid spool as a pitch child with its actual root-space bind.
  The study's spool binds are identity, whereas `links.node` currently requires
  an origin at joint a. For a links export either rebase the mesh and bind
  together, or have registry explicitly support local joint attachment frames;
  do not merely replace the bind translation. Preserve the working source study.
- The projected `joint_a_hinge` roll agrees with inherited pitch in this layout
  (see the numeric check), but schema/consumer must state whether the link is
  solved or inherited and checked. There must be exactly one pose producer.
- `check_motion` at c5ae8c1a checks chord length, projected chord hinge turns,
  single-hinge direction constraints, roll singularity and sampled tie lengths.
  It does not evaluate full link orientations or consume `roll_axis`, `torsion`,
  `axial`, or reference transforms. Its two-hinge angle projections are not an
  ordered rotation decomposition. Passing it is therefore not complete joint
  validation, particularly for twist or a curved spool's end frames.
- Extend the contract to make both joint-side neutral frames unambiguous, and
  validate their relative rotation against the ordered permitted hinges, with
  residual torsion/translation checks. Check the solved/inherited link pose and
  ties against that same evaluation. Add negative fixtures with unchanged joint
  centres but forbidden twist, wrong roll and disallowed second-axis rotation.
  Include signed limit poses and full node-transform reference comparisons.
- The 65-pose installer sweep is sampled evidence, not proof of clearance or
  compliance throughout travel. Retain the denser exporter sweep and final-hull
  clearance checks. A limit means maximum turn either way from neutral in the
  current schema; do not import a catalogue total-travel number as that limit.

### Bellows representation decision

The study's Hermite surface is explicit and useful review evidence, but its mesh
is rebuilt by Blender; the neutral GLB cannot reproduce that behavior in the
current renderer. Keep it as the reference surface for now.

Prefer a **rigid-ring fan prototype** as the next consumer experiment because it
uses the existing rigid-part draw path. This is conditional technical preference,
not approval of its appearance or engineering. A usable fixture must define each
ring node/bind, both cuff frames, hinge centre/axis, ordered interpolation weights
including the end-ring rules, and the position as well as rotation formula. An
instruction to rotate ring i by i/n alone does not establish a sealed surface.
Show end seating, overlaps/gaps, self-intersection, bounds and silhouette through
signed/combined limits, compared with the Hermite reference. Count parts/draws;
the current part selector uses u8 IDs, with part 0 reserved.

Registry's current schema has no ring-fan evaluator fields. Agree those fields
with the consumer before calling the ring export runtime-ready. If rigid rings
fail the visual checks, use a bounded procedural bellows surface update with
reused buffers; do not revive the infeasible free-hose route. The Hermite method
would also need explicit active span, tangent magnitudes and profile parameters
instead of treating the entire catalogue joint length as the active bellows.

Compact flight-joint targets, one-sided ratings, actual route-length mass,
pump supports/outlets and full-assembly/hull clearance remain registry/Blender
work. This review neither selects those provisional engineering values nor
changes the production installation gate.

## Proposal-v04 rigid-sleeve consumer check

2026-10-10, Blender task `20261010T143628-blender-663a`. Engine agrees to the
prototype evaluator below; registry agreement/schema publication remains pending
on task `20261010T143719-engine-68d2`. This does not implement production equipment
motion loading or approve installation.

The repeatable Rust diagnostic uses the actual `PbrModel::load_gltf_parts` loader,
its baked vertices and its part IDs. Run from the engine checkout:

```sh
cargo run -p universe-engine --example ch_s2_fixture -- \
  /home/alexm/git/blender/engine-ch-s2/proposal-v04/source/ring-fan-fixture.json \
  /home/alexm/git/blender/engine-ch-s2/proposal-v04/exports/duct-ring-rest.glb
```

The fixture is external review input, not installed game content. The example is
a diagnostic of this provisional format, not a hardened package parser. It uses
18 explicit moving selectors (pitch, yaw, 16 sleeves), plus static part 0. The
63 primitives retain their immutable geometry and model ID. Stage-owned hardware
inherits its stage part; each sleeve has its own part. Selector overlap is rejected.

Evaluator semantics to preserve in the registry contract:

1. Attachment points, tangents and the upstream hinge axis are local to their
   named parent. Transform them using the evaluated parent root-space frame.
   Both endpoint tangents point downstream along increasing Hermite parameter;
   the downstream tangent is not an outward-facing end normal.
2. Use cubic Hermite interpolation with the supplied `tangent_magnitude` for
   both endpoint derivatives. Each sleeve origin is `H(t)` at its supplied `t`.
   Frame Z is the normalized derivative; X follows the upstream hinge axis,
   and Y is Z cross X. This prototype requires a planar single-hinge arrangement:
   reject degenerate derivatives/axes and nonperpendicular hinge/tangent pairs,
   rather than silently selecting a different roll. Orthonormalize numerical
   roundoff after checking perpendicularity.
3. Let F be that root-space frame, F0 its evaluated neutral frame, and B the
   exported sleeve bind. The current exported node frame is `F * inverse(F0) * B`.
   This neutral calibration preserves the exported mesh's local-axis convention;
   it avoids an implicit Blender-to-glTF conversion in the consumer. The draw
   delta is then `current * inverse(B)`, with equipment placement applied once.
4. Other nodes use their parent's evaluated frame times inverse parent bind
   times their own bind. Sleeve frames have one procedural pose producer; do not
   apply stage motion to them a second time. Bind and pose references remain in
   model-root space. Production validation must reject cyclic dependencies and
   define evaluation order before allowing attachments to other procedural nodes.

Observed CPU results on v04:

| Check | Result |
|---|---|
| Nodes / sleeves / primitives / part groups including static | 66 / 16 / 63 / 19 |
| All-node reference poses | 9 passed |
| Additional finite, rigid-frame sweep | 513 poses passed |
| GLB vs fixture bind maximum component error | 2.385e-7 |
| Evaluated vs reference pose maximum component error | 3.703e-7 |
| Maximum placed vertex difference vs reference (metres) | 5.841e-7 |

The vertex comparison includes a rotated/translated equipment placement and
uses independent GLB local vertices with Blender's reference matrices as the
expected result. This tests the bind-delta and inherited-part conventions, not
just the Hermite formula. Acceptance thresholds are 2e-6 for matrix components
and 3e-6 m for placed vertices, accommodating exported single precision.

Blender reports sampled sleeve overlap/cuff seating success. This diagnostic
does not independently reproduce that surface QC, issue GPU draws, measure GPU
cost, or check full-engine/hull collisions. Those gates remain open; the current
catalogue-sized study is not the compact production design.

Input SHA-256 values were checked against the delivery: fixture
`da819e1ab60bf60d3dcbf6def872bcad097532a4d2a6cd1f4518dc93179f4a58`, GLB
`78019c2b3c2713e61fc0f526c6bfd75698e69b4714a67846214a31d3156e603d`.
A deliberately displaced reference sleeve and a nonplanar hinge-axis mutation
were each rejected as expected. Registry build and workspace tests passed after
adding the diagnostic (serde_json is a development dependency only).
