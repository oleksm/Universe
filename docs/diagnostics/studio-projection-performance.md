# Studio projection performance

The CPU projector used two heap vectors per visible triangle, then built a full
projected mesh before copying it into the canvas. With dense installed equipment,
that overhead dominated Studio drawing.

`equipment_visual::project_each` uses fixed triangle/quad scratch storage, rejects
triangles behind the near plane before shading, and streams output to the caller.
DESIGN, catalogue, BALANCE and TEST DRIVE submit to the same canvas directly.
WALK retains its existing mixed scene ordering but skips the intermediate projected
mesh. Geometry, asset readiness/leases, shading and alpha handling are unchanged.
No geometry reduction, new frame cache, physics or saved-design changes are involved.

## Verification

The pre-change projector remains test-only as a reference. The regression compares
exact positions, colours, depths and material flags across every below/on/above
near-plane combination, both sidedness modes, three material modes and rotated
placement/parent transforms. Existing pending-promotion and clipping tests remain.
Engine independently reviewed the change and checked 10,027 clipping cases with
no blocking regression (`out/review/studio-projection-review/review.md` in the main
worktree).

`draw/studio/project-equipment` now includes consumer submission; its old and new
values are not directly equivalent. Compare the outer `design-models` or `draw`
scopes. Existing transparent-intersection sorting and neutral-preview limitations
are unchanged.

## Measurement method

The native observer runs use `tools/diagnostics/studio-observer-check.py` with
`--warmup 8 --samples 5 --debug 0`, the unchanged isolated export of the owner's
`commuter-3`, and a 60 FPS cap. Both use the shared observer/profiler and warm OS
caches. No concurrent builds run during the final pairs. Performance samples
precede recording/screenshots. The tiled viewport varied in WALK, so its native
pair is not used as a controlled comparison.

The final scripted Vulkan captures use the existing fixed 1920x1080 screenshot
surface, frame 300, profiling enabled and input ignored. The profile reports the
rolling frame window at capture; the profiler overlay remains visible. Screenshot
mode disables the HTTP observer; an attempted combined run is retained as failed
harness evidence, not included in performance claims.

| Mode | Baseline draw CPU | Optimized draw CPU |
|---|---:|---:|
| DESIGN | 11.132 ms | 6.332 ms |
| WALK (default, facing away) | 4.394 ms | 1.010 ms |
| WALK (ship-facing at 0,0,12; yaw 0) | 18.139 ms | 16.262 ms |
| BALANCE | 10.299 ms | 4.990 ms |
| TEST DRIVE | 11.599 ms | 4.894 ms |

DESIGN's native observer median was 11.217 -> 5.681 ms. Both baseline and optimized
reach the 60 FPS cap with idle-system headroom; the earlier ~40-47 FPS readings
were under different system load. This change removes the measured bottleneck;
it is not a promise of 60 FPS for every ship/camera/machine. The outer drawing
scope improves about 43-58% in the three dense fixed views.

DESIGN, default WALK and BALANCE captures have identical pixels outside the
changing profiler HUD region. DRIVE has time-step-dependent pose/HUD differences;
its full installed ship is present, with the exact projector regression covering
transform/clipping equality. Default WALK faces away from the ship, so it measures
behind-camera rejection; a separate ship-facing pair supplements that check and also matches pixels outside
the profiler HUD. WALK retains its large mixed-scene sorting cost, so its dense
ship-facing improvement is smaller (~10%); further sorting optimization remains.

235 workspace tests pass (one existing ignored GPU test), along with the registry
build, player/no-default-features check and both release builds. Four native
observer/recording checks and ten fixed Vulkan captures pass. Full paths and
binary SHA256 values are in `studio-projection-performance.json`.

Raw evidence: `out/review/studio-perf/` in the `universe-ships-studio-perf` worktree.
The cap makes this a latency/headroom comparison, not a maximum-FPS benchmark.
