# MC-07 cargo ramp runtime integration

Task `20261010T173807-ships-ac86`, installed static package `304edf1c`.
The replacement is **not enabled**. The isolated heel-pocket interface is now
accepted by ships and Blender, superseding the old native hinge-contact failure.
Combined-hull and live five-part drawing/collision/boarding gates remain open.
No asset installation or branch merge is made here.

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
- `../blender/mc07-ramp-fit/integration-plan.yaml`: accepted isolated interface,
  pinned heel-pocket fixture and remaining combined-hull gates.
- `../blender/mc07-ramp-fit/runtime-collision-v01/handoff.yaml`: exact collision
  meshes and ray-measured diagnostic walking path.
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

Next gates: ships assembles and verifies the combined hull; runtime mounts
and five-part drawing/collision/boarding integration use this evaluator and
fixtures; Vulkan closed/deployed views and boarding checks verify the assembled
result. Passing source matrices alone does not close those gates.

## Heel threshold consumer check

Registry task `20261010T183252-registry-02b4`: **walker heel crossing passes**
for both closed/deployed snapshots, both directions, at 30/60/120 Hz (12 cases).
The real `walk::Walker` consumes the exact exported GLB triangles: 14 meshes,
28,029 triangles per pose. This includes the nominal 30 mm cover rise and the
actual deployed chamfer transition (approximately 58 mm over the final 20 mm,
per ships), rather than a synthetic step. All heel cases remain grounded.

The test uses existing 0.45 m step, 0.35 m body radius, 1.8 m height and 0.643
floor-normal threshold. Speed 1.5 m/s and standard gravity are diagnostic choices,
not new gameplay tuning. The ray-measured centreline runs model z=4 to z=2 m
through the heel; the longer route continues to z=-4 m. No jump/climb input.

All 12 full-route cases reach the opposite endpoint. Closed cases remain grounded;
deployed cases briefly lose grounded status: about 67 ms outbound and 33 ms
inbound at each tested rate. Downward rays near z=0.475 and -3 m encounter faces
with normal.y about 0.265, below the controller floor threshold. These are outside
the heel window. Thus the longer deployed route fails the conservative continuous
support gate despite completing traversal. No controller tuning or geometry fix
is included. This is a static snapshot test, not live boarding/toe behavior.

**Cargo crossing is UNSUPPORTED**, not passed: cargo currently has aggregate
mass/volume inventory, without a traversing body, wheels, pallet or clearance
contract. Registry confirmed this classification in `20261010T183429-registry-cf82`;
the ramp's 40 t rating alone cannot define a collision test.

Reproduce after building `cargo build -p universe-world --example ramp_threshold`:

```sh
/home/alexm/git/planet-trees/.venv/bin/python \
 /home/alexm/git/planet-trees/tools/scenery_job.py --memory-gib 8 --threads 2 -- \
 python3 tools/review/ramp_threshold.py \
 ../blender/mc07-ramp-fit/runtime-collision-v01 /tmp/ramp-threshold.json
```

`mc07-ramp-threshold-validation.json` retains fixture and final GLB hashes,
controller outputs and unsupported-frame evidence. The runner verifies the owner
manifest and triangle counts; its `reached_end` and `continuous_support` results
are separate. The example exits nonzero when continuous supported traversal
fails. Combined-hull geometry, off-centre routes, moving articulation, live
boarding actions and cargo traversal remain outside this acceptance.

Validation: example builds and executes all 24 snapshot/route cases; registry
build passes. Full workspace testing hits the unrelated existing
`ship::classes::every_hull_is_balanced_and_sized_for_its_job` assertion: MC-07
empty-hold lift is 8.2 m/s², below the required 1.1 g margin. Reported to registry
as `20261010T183954-engine-1a81`; no rating or assertion changed for this review.
The remaining workspace tests pass with only that named assertion excluded.
