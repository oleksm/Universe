# Installed equipment across Ship Studio

Run `./target/release/freefall-studio` (or `./target/release/freefall --studio`).
In a new design, choose MODULES, select Chemical engine S2 (hydrolox), and click
the work plane. T rotates a selected engine through six thrust directions.
The same placed equipment appears in DESIGN, walk/PREVIEW, BALANCE and TEST DRIVE.

`equipment_visual::Placed` is the common visual representation. It resolves
`Fitted.key` through one hash-checked asset cache, selects an installed mesh or
fallback once, and stores its material colours and design-space transform.
The rotating catalogue card uses this same representation too. Every mode uses
the same projector, lighting, near-plane clipping and GPU depth test; only the
camera and parent pose differ. Mounted flight shares the
checked package/PBR cache but keeps its operational-motion acceptance gate.

Authored GLB root transforms and metres are preserved. A model's bounds centre
is subtracted once, its +Y thrust axis is rotated to the saved engine direction,
and it is translated to the design block centre. Walk and DESIGN use that frame
directly. BALANCE subtracts physical COM once. TEST DRIVE adds craft rotation
and position after that subtraction. Members and physical boxes already stored
relative to COM are not recentered again.

Block dimensions, selection, placement, saved schema and physical calculations
remain independent of the visual mesh. The walker retains the previous physical
triangles; test drive retains its existing contact boxes. Neither acquires mesh
collision or operational animation from this change. Skin/morph/clip data and
failed packages fall back with a once-only warning. A motion sidecar alone does
not prevent drawing the neutral rigid GLB.

The shared studio canvas draws actual triangles with material base colours and
camera-relative studio shading. Per-vertex eye distance reaches the GPU: reversed
depth resolves each pixel, and colours interpolate perspectively. Geometry does
not sample the font atlas. Each independent camera (including the catalogue
card) starts fresh depth; ordinary UI never writes it. Walk walls and equipment
share depth. This is still base-colour shading, not flight PBR: textures and
material reflections are not sampled. Selection/clash envelopes remain
available in DESIGN. Missing models share one box/ellipsoid visual fallback;
physical airlock passages and ramps retain their separate collision treatment.

Validation uses one hull-free saved design with two CH-S2 instances, upright and
forward-thrusting, across all modes. The isolated review data directory leaves
normal owner saves untouched. Reproduce the review design with:

```
XDG_DATA_HOME="$PWD/out/review/studio-equipment-v1/data" UNIVERSE_TOOL=modules \
  ./target/release/freefall-studio installed-equipment-review
```

Review-only startup controls select existing modes: `UNIVERSE_BALANCE=trim`,
`UNIVERSE_DRIVE=0`, or `UNIVERSE_STUDIO_WALK=0,0.45,8,0` (feet x,y,z and yaw).
Ordinary UI controls use the same paths. Without XDG_DATA_HOME, the launcher uses
normal saved designs. Earlier DESIGN-only capture evidence is in
`out/review/studio-equipment-v1`; consolidated captures are in
`out/review/studio-equipment-v2`.

Final validation: 135 workspace tests passed, the registry build passed, and both
release binaries rebuilt. All four Vulkan captures were inspected; their binary,
saved-design and image hashes are recorded in `out/review/studio-equipment-v2/captures.json`.
The test-drive fixture has only engines, so its missing fuel/gear and contact
warnings are expected; this validates drawing, not flight readiness. The rebuilt
Studio was launched with the isolated review design above.

Depth-rendering follow-up: the unchanged comm dish was captured with the old and
new binaries at the same default camera. The interior speckles and incorrectly
visible rear parts disappeared. Six Vulkan captures (dish, crossing-triangle
regression, DESIGN, walk, balance, drive) are in `out/review/studio-depth-v1`.
The GPU fixture produces exactly identical pixels for opposite triangle orders,
checks UI overlay colour and a fresh camera depth clear. Run
`tools/review/studio_depth.py` under the shared 2 CPU / 8 GiB launcher after
building both binaries and the engine's `canvas_depth` example. All 135 workspace
tests and the registry build passed. A rebuilt dish-review window is open; the
owner's earlier window was left intact because it contains unsaved edits.

Editor grid/selection lines remain deliberate overlays. Textures, reflections,
transparent-material sorting and operational motion are separate work.
