# Shared scene views

`scene::Scene` is an isolated retained presentation graph. It owns stable opaque
`ObjectId`s, parent-local f64 `Pose`s, optional `Geometry::Model`/`Mesh`, layer masks,
and optional authoring pick bounds. The client owns ID allocation, asset requests,
placeholder selection and the domain simulation. `set` validates before replacing;
`remove` removes a subtree and releases its handles. Ready model handles should carry
the existing asset owner lease. There is no registry, ship, Studio or physics policy
in this graph.

`TickClock::advance` schedules the caller's existing tick and retains excess elapsed
time. It does not implement integration, allocation, gravity or contacts. Pausing is
not calling advance; a session owns and resets its own clock.

`viewport::View` contains `camera`, `projection`, `rect` and `layers`. Cameras look
along local -Z, +Y up, +X right. `Projection::Orthographic { height, far }` is a true
reversed-Z orthographic camera. Rectangles use the parent frame's HUD coordinates.
`View::project` and `View::ray` use this exact projection; ray origins are on the near
plane. `Scene::pick` returns sorted world-distance OBB proxy hits, not mesh-accurate
physics contacts.

```
frame.scene_view(view_id, &scene, &view, |child| {
    child.ambient = 0.2;
    child.light = light;
    child.line(a, b, colour); // actual scene depth
    child.world_text(anchor, "name", colour, 1.0, true); // actual scene depth
    child.text(label_pixel, "overlay", colour); // on top
});
```

Each view has an independent color/depth target and camera-relative submissions.
The shared renderer submits each view before reusing scratch globals and instance
buffers; it does not create another device, model cache or shader implementation.
Views are composited in submission order below the host's UI. A view's own UI is
local to its rectangle. Duplicate IDs, out-of-frame rectangles and nested views are
refused. Closing a view drops its target; resize recreates that target. Per-frame
model demand is the union of all views and the host. PBR upload byte/time budgets
are shared across them. World labels sample opaque/masked scene depth, not glass.

PBR draws opaque/masked geometry before a far-to-near queue across all model
primitives and both sidedness pipelines. The queue uses transformed cached bounds
centres. It does not solve intersecting glass or per-triangle self-sorting inside a
single primitive; those require split primitives or a future order-independent
transparency algorithm. Masked shadow casters sample the same base texture/factor
and cutoff. Transparent primitives do not cast solid shadows.

The `scene_views` example is a bounded acceptance fixture outside the 50-test fast
suite. `--cpu` checks graph transactions/transforms, scheduling and perspective/ortho
ray roundtrips at large coordinates. The GPU mode renders three simultaneous views,
opaque/transparent overlap and world labels. `REVERSE=1` reverses object submission
identity for an image-equivalence check. Use the usual screenshot environment flags.

This API does not automatically migrate a client, delete its old projector or
establish its gameplay parity; clients must switch their scene submission and
validate their own interactions.

Validation (2026-10-12): 17 existing engine tests pass, including all WGSL module
validation. The CPU harness passes; three simultaneous Vulkan view captures pass,
and reversed model submission is pixel-identical. `--mask` produces the expected
checker shadow; `SOLID=1 --mask` produces the solid control. Captures and hashes are
in `out/review/scene-runtime/validation.json` in the engine-scene-runtime worktree.
This evidence does not establish client-mode parity or a performance budget.
