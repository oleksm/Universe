> Provenance: copied from `/home/alexm/git/universe-ships-studio-perf/docs/diagnostics/studio-walk-investigation.md`; original diagnostic commit `dd94f934`. The investigation below records its own runtime baseline and measurement conditions.

# Studio WALK CPU investigation — 2026-10-12

Runtime baseline c9b2089c; isolated release build changes only timing scopes in draw_stand.
Main and installed binaries untouched. The subsequent main test purge has no runtime changes.

Fixture: commuter-3.ship.yaml from ships-loading-audit/out/review/loading-audit.
Warm assets, RTX 5090 Vulkan, native window, 60 FPS cap; five observer samples per pose.
Another user-owned freefall process remained running; timings are diagnostic, not an isolated
benchmark or reproduction of the owner's exact 30 FPS design/pose. No build ran during captures.
Raw profiles: universe-ships-studio-perf/out/review/walk-investigation/{facing,away,inside,design}.

Facing pose feet(0,0,12), yaw0 submits about 176,000 triangles. WALK CPU profile:
- Whole WALK: 15.857 ms mean, 24.293 ms maximum.
- Mixed list stable depth sort: 8.642 ms mean, 11.682 ms maximum.
- Equipment projection and list insertion: 4.833 ms mean.
- Geometry submission/list consumption: 2.344 ms mean.
- Walls: 0.015 ms; labels/members: 0.016 ms.
- Update: 0.016 ms; total draw: 18.244 ms; render thread: 4.512 ms.
These overlap/nest; do not sum total draw, WALK and render-thread times.
Median observer facing FPS54.1; away60, inside60. Exact reported30 FPS not reproduced.
See accompanying JSON for sample medians. DESIGN sends about181,000 triangles
but only about5.8 ms CPU draw: raw triangle count alone is not the WALK penalty.

Cause: draw_stand accumulates walls, equipment triangles, members and labels into a fresh
Vec<(f32, Item)> and stable-sorts ALL items far-to-near each frame. Large Item values
include full Projected triangles; sorting moves large records. The renderer now depth-tests
opaque/mask canvas geometry and independently sorts blended triangles in canvas_batches.
The WALK opaque sort is therefore redundant for visibility; transparent sort is repeated.
Labels/members are renderer UI overlays, so removing order blindly could still change UI
overlap or equal-depth ties. Those need explicit review in a fix.

Other costs: equipment_visual::project_each transforms and lights triangles on CPU every
frame, culls backfaces/near plane but no side-frustum or wall occlusion before submission.
Walls still allocate clipped polygons and projected point vectors, but measured wall time
is negligible in this fixture. Loading scopes are zero at steady state.

Recommended order:
1. Stream opaque/mask geometry; keep only a small explicitly ordered label/member pass
and let shared renderer own blend ordering. Review glass, overlapping labels and equal-depth surfaces.
2. Add conservative module bounds/frustum rejection before triangle projection.
3. Longer term, use resident GPU mesh rendering for shared Studio equipment while retaining
the same model resolver, material rules and transforms.
No visual behavior was changed in this investigation. Instrumentation compiled in release;
no broad test run needed for timing scopes. Main installation is unchanged.
