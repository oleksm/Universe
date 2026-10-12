# Shared asynchronous visual consumers

Implemented through `ships-asset-consumers` and `engine-asset-loader`, using engine `AssetCache` and draw-time residency leases. World registry models, Studio catalogue/design/walk/balance/test drive, and mounted equipment request the same immutable hash-keyed `Visual`. Manifest/hash verification and glTF decoding run in engine-owned workers. A missing or pending model retains its existing envelope; a corrupt package remains a fallback and exposes its error through `/resources`.

`Placed` retains a full asset lease whenever it holds cached faces. A stand or test craft created while loading retains its key and transform intent, and the shared projector resolves it again each draw. Completion changes visual triangles only: saved blocks, centring, rotation, contact boxes and walker triangles do not change. World and mounted drawing retain the same lease through engine render-frame/worker ownership. Mounted equipment no longer permanently caches a pending result as failure. Motion/native-suppression gates still apply.

World demand now includes measured camera motion, angular motion, relative asset velocity and predicted time to visibility. Selected catalogue cards remain required; nearby rows are speculative. Queued speculation expires or cancels on scene changes. Only a short rear neighbourhood is renewed, allowing distant rear assets to become idle. Cost estimates learn preparation time; small fitting work can proceed while a larger request waits for leased memory.

CPU visual and texture preparation each have a separate 512 MiB default payload budget. Package sizes, GLB hierarchy, embedded buffers, image dimensions and expanded payload estimates are checked before large allocations. These are payload and admission limits, not a hard process RSS or driver-memory ceiling. Unsupported external glTF resources are refused with a visible fallback/error. GPU publication is incremental and separately budgeted; its scheduling time target is soft because allocation and publication have indivisible steps.

F3 performance HUD (including Studio) and observer `/resources.assets` expose CPU loads, hits, misses, coalescing, unloads, reloads, queue/running counts, retained/reserved bytes, retries and failures. Observer entries include immutable keys, states and errors. Existing renderer resource data is exposed alongside it. Standalone Studio cycles the same counters and profiler with F3 and runs cache maintenance without constructing an App/universe; its observer port is not implemented.

Startup follow-up: both game entrypoints and standalone Studio now use engine `run_prepared`, which draws a responsive shell while a worker constructs the application. `Models::new`, world construction and Studio MC-07 commissioning remain atomic prerequisites inside that worker. No partial collision/physics data reaches play. Errors remain visible; Escape or closing the window does not join the preparation worker.

Native hull PBR, explicit showcases and mount frames now request the same visual service as installed equipment. Native path plus decode options identify immutable per-session data; workers publish its actual SHA-256 for overlay validation. Hull drawing and mounted sockets share the `Ramp` decode entry. Frame discovery does not reread the file; draw clones retain the returned lease. Native replacement still requires its existing motion/contact acceptance. Overlay metadata is read in the startup factory, limited to 4 MiB, and remains hash-bound. Native files changed on disk require restarting the process.

Startup logs distinguish first shell CPU draw submission (not GPU presentation) from CPU/physics-ready publication. Registry/content and model construction still take time; optional models may remain envelopes after readiness. Standalone Studio still commissions MC-07 for hull-backed editing even when opening a hull-free design. The follow-up moves that prerequisite off the window thread; it does not remove it.

Ground fallback meshes replace their previous readiness revision, so a newly ready model is not covered by an old box and eviction restores a box without retaining every readiness permutation.

## Validation

Registry build and 175 workspace tests pass. The GPU test normally ignored by the suite was run explicitly and passes. Four standalone Vulkan captures (DESIGN, walk, balance, test drive) show both CH-S2 instances with one shared load, correct orientations, and zero asset errors; DESIGN also speculatively loads an air tank from its visible catalogue rows. Captures were visually inspected. This small fixture has no fuel/gear and is not flight acceptance.

An actual CH-S2 cache test covers first load, warm reuse, lease-pinned idle, eviction and scene return: two total loads, one unload, one reload, 8,757,417 retained bytes, zero failures. Cold here means an empty process cache, not flushed OS file caches. A combined engine Vulkan fixture survives ten frames absent and returns with one CPU load, 199 CPU hits and one GPU upload (2,147,808 bytes); its unlit capture is only a rendering/residency check. Additional regressions cover corrupt manifest failure, shared world/Studio identity, pending stand/drive promotion without physics mutation, and removal/restoration of cached world fallback boxes.

Machine-readable results: `docs/diagnostics/async-visual-consumers-v1.json`; local logs, screenshots, reproduction script and diagnostic binary: `out/review/async-visual-consumers/`. Both main release binary hashes still match the previous validated asset merge.

Completion checks and GPU eviction/return evidence: `docs/diagnostics/asset-loader-finish-v1.json`. The earlier validation above records the consumer milestone; final workspace validation covers the startup and prediction additions.


Startup validation (`docs/diagnostics/startup-loader-v1.json`): final source `ce297c52` passed 186 workspace tests, registry build and both release binaries. Native MC-07 plus computer/transponder rendered with three CPU loads, three GPU uploads and zero asset errors. Publication now runs the first update before the first game draw; its regression prevents the empty render-position failure caught by that capture.

On the two-engine Studio fixture, main's first-frame capture plus process exit took 0.818/0.821 s; the earlier candidate's shell capture plus exit took 0.293/0.263 s, and its CPU-ready event arrived at 0.793/0.788 s from process spawn. These are different milestones, measured with unflushed OS/driver caches, not a claim of faster final readiness. That Studio timing revision predates the publication fix; final native-hull capture uses the final source.

A remaining world-preview stall is explicitly outside the constructor gate: `update/globes` took 4480.671 ms of a 4485.923 ms first update. The main-thread globe initialization can still freeze the shell during handover. Native/equipment decoding was successful; this is not stutter-free world startup. See the scoped profiler output in `out/review/startup-loader/native-profile.log`.
