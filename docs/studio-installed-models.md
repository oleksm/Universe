# Installed models in Ship Studio DESIGN

Run `./target/release/freefall-studio` (or `./target/release/freefall --studio`).
Start a new design, choose MODULES, select Chemical engine S2 (hydrolox), and
click the work plane to place it. T rotates a selected engine through its six
thrust directions. Existing save/open and envelope selection remain available.

Catalogue `Fitted.key` resolves the installed registry visual through the checked
asset manifest and GLB loader. The shared immutable cache also supplies mounted
flight models. Unsupported skin/morph/clip data or failed package checks retain
the envelope fallback and log once. A motion sidecar alone is allowed for neutral
studio drawing; flight's articulated-motion gate remains in place.

The mesh's baked root transforms are retained. Its actual bounds centre is
subtracted once, then its +Y thrust axis is rotated to the block's saved thrust
direction and translated to the existing block centre. Geometry remains in
metres with no stretching to match a resized design envelope. The envelope
continues to represent the saved planning dimensions, not a new mesh collision
shape. No record sizes, physical calculations, saved schema or model packages
were changed.

The design canvas draws actual mesh triangles, material base colours and
camera-relative studio lighting. It sorts all placed module triangles together;
this painter-based canvas is not a depth-buffered PBR view, and intersecting
triangles can have ordering artifacts. Textures, operational animation and
physical collision are not enabled by this change. Selection/clash envelopes
remain overlaid, and unmodelled catalogue entries retain their box/ellipsoid.

Validation capture: `out/review/studio-equipment-v1/design.png` shows a fresh
hull-free saved design containing two CH-S2 instances, one upright and one
forward-thrusting. Vulkan completed 120 frames; both real meshes, feed ducts and
nozzles were visually inspected. Fixture/save, GLB, binary and PNG hashes are in
that folder's `validation.json`. The six-axis orientation/centering and save
round-trip test passes. Registry build passes. A live window was launched using
an isolated review data directory so existing owner saves remain untouched:

```
XDG_DATA_HOME="$PWD/out/review/studio-equipment-v1/data" UNIVERSE_TOOL=modules \
  ./target/release/freefall-studio installed-equipment-review
```

Without the isolated data override, the standard launcher uses your normal
saved designs. New designs use the same installed-model drawing path.

Workspace validation: 133 tests passed, including the new six-axis design test;
both release launchers built successfully. No operational-motion gate changed.
