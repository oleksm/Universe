# MC-07 cargo ramp runtime integration

Task `20261010T173807-ships-ac86`, installed static package `304edf1c`.
The replacement is **not enabled**: ships' final hull-fit gate fails pending
classification/repair of hinge-region contacts. Static package acceptance does
not certify moving hull fit. No asset installation or branch merge is made here.

## Corrected native hinge

The importer used `hatch` as the ramp hinge. In the current hull that marker is
approximately 3.84 m from the authored `CargoRamp` origin. Ships confirmed the
mesh origin is the hinge, while the marker serves boarding. The importer now
uses the authored mesh origin and transformed local +X axis when CargoRamp
exists. Legacy unnamed ramps retain their hatch convention. Rendering and the
moving ramp collision mesh both consume the same corrected `Shape::ramp`.

The root transform is applied once. Model-frame hinge is `(0,-7.15,+2.6)` m,
axis approximately `(-1,0,0)`; the shape's centre of mass is subtracted once for
simulation coordinates. The imported hatch remains separate. A regression test
checks the actual current hull's hinge, axis, full ramp length and downward
rotation; the full workspace tests pass.

## Rigid evaluator agreement

`crates/engine/examples/cargo_ramp_fixture.rs` independently evaluates the
source's `Rx(0.5176*c)` ramp and two body/rod DAMPED_TRACK pairs, without scaling.
Blender local +Y becomes glTF local -Z. Each track rotates its parent-transformed
unconstrained axis toward the opposite anchor by the shortest arc. The solver
refuses the ambiguous antiparallel case. It does not reuse the evaluated bind
rotation as the unconstrained orientation.

The 65 five-group source poses agree within `3.832e-7` per matrix component;
exported rest matrices agree within `3.259e-7`. The optional new full-node
fixture checks all ten nodes at all 65 poses, including both attachment nodes,
the moving boarding toe, static mount and source root; maximum error is
`8.386e-7` per component. PBR loading confirms
all five moving groups are selectable and no unclassified mesh remains.
This is a diagnostic consumer, not an installed motion schema or live drawing.

```sh
~/bin/capped cargo run -p universe-engine --example cargo_ramp_fixture -- \
 ../blender/mc07-equipment/cargo-ramp/candidate-v01/review/source-motion.json \
 ../freefall-assets/models/access-cargo-ramp-mc07/v1/model.glb \
 ../blender/mc07-equipment/cargo-ramp/candidate-v01/review/runtime-fixtures.json
```

Authoritative handoffs:

- `../blender/mc07-ramp-fit/motion-handoff.yaml`: anchor frames, axes, placement.
- `../blender/mc07-ramp-fit/fit-review.yaml`: failed final fit, action-cleared sweep.
- `../blender/mc07-equipment/cargo-ramp/candidate-v01/review/runtime-fixtures.json`:
  complete glTF binds and pose references, supplied by Blender.

## Replacement mapping and remaining work

The hull-model mount must be translation `(0,-7.15,+2.6)`, identity rotation.
The asset already carries the source root turn. It must replace exactly
`CargoRamp`, `CargoRam_L_Body`, `CargoRam_L_Rod`, `CargoRam_R_Body` and
`CargoRam_R_Rod`; fixed supports remain hull-owned. Avoid drawing those native
meshes alongside the equipment. The current hull has no `mount_cargo_ramp`.

Live replacement must draw each asset group using evaluated-pose times inverse
export-bind, then mount, then the ship transform with COM subtraction. Its
`door_cargo_ramp` follows the ramp, distinct from its hinge. No authored timing
or interpolation curve is provided. Preserve existing landing/boarding actions;
check boarding against the moving toe and collision surfaces rather than
inventing a deployment duration. Existing ram rendering/collision is not a
complete articulated actuator consumer, and needs replacement together.

Next gates: ships resolves hull seating/pockets and repeats fit; runtime mounts
and five-part drawing/collision/boarding integration use this evaluator and
fixtures; Vulkan closed/deployed views and boarding checks verify the assembled
result. Passing source matrices alone does not close those gates.
