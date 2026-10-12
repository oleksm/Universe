# Asynchronous asset loading and residency

Proposal, 2026-10-11. Scope: faster first usable scenes, fewer loading stalls, and bounded asset memory across all game modes. The proposal below preserves the original design rationale. Implementation and measured scope are recorded in `../ships/async-visual-consumers.md` and `../diagnostics/asset-loader-finish-v1.json`.

## Recommendation

Keep a small, validated registry/content snapshot available for simulation and catalogue queries. Put heavyweight visual assets behind one engine-owned asynchronous asset service used by every game mode. Request assets by registry identity, render a suitable fallback immediately, then promote representations as they become ready.

Schedule in two stages: **urgency from predicted time until needed**, then **cost and visual benefit within that urgency**. Cheap assets should fill a scene quickly, while large imminent assets start early enough to finish. Manage CPU and GPU residency independently, using explicit budgets and predicted reuse rather than distance or “behind the camera” alone.

Studio is a game mode, whether entered in-game or launched standalone. Loading, caching, profiling and residency are engine services with one implementation. Consolidate existing duplicate paths as part of this work; mode-specific code supplies demand and camera information only. Standalone launch changes the entry point, not the architecture.

## Baseline before implementation

| Evidence | Implication |
| --- | --- |
| [Registry](../../crates/core/registry/src/lib.rs) is compiled into the binary and decoded once; [content](../../crates/core/world/src/content.rs) is validated and shared. | Runtime YAML discovery is not the main visual-loading problem. Measure decode/world construction before changing this contract. |
| [World visuals](../../crates/game/src/visuals.rs) reads, verifies and decodes a model synchronously on first lookup, retaining results by key. | First sight of a new model can block its caller; the cache has no byte budget. |
| [Equipment visuals](../../crates/game/src/equipment_visual.rs) has another cache, holding its mutex through package loading, decoding and construction of CPU faces. | Duplicate consumer caches can repeat work; misses serialize and CPU representations accumulate. Consolidate them into the engine service. |
| [PBR upload](../../crates/engine/src/pbr.rs) creates GPU models on first submission; texture upload generates mipmaps and compresses images. GPU models expire after 600 unused frames. | Moving file reads alone will not remove first-use stalls. Retention varies with frame rate and does not enforce a memory budget. |
| [Terrain LOD](../../crates/game/src/terrain_lod.rs) has worker jobs, coarse fallbacks and predicted camera positions. | Reuse this design experience; coordinate resource budgets rather than replacing terrain with a model loader. |
| [Application construction](../../crates/game/src/lib.rs) constructs the universe and spawns settlers before completing application creation. | Startup may also include simulation work. Instrument it separately from assets. |

These are identifiable synchronous paths, not a measured ranking of startup costs. Pending Studio material changes must be preserved during integration.

## Loading pipeline and ownership

`request → resolve → read/verify → decode/prepare → upload → resident`

Each request returns a stable handle immediately. Its state distinguishes queued, loading, CPU-ready, GPU-ready, failed and cancelled. Rendering never waits on disk, decoding or compression. The service deduplicates requests by package content hash plus decode options; registry keys resolve to that identity. Texture identity also includes colour-space and format use.

A single asset may have several consumers. Aggregate their urgency and retain it while any consumer needs it. Instance transforms, animation state and material overrides remain per instance; immutable geometry and textures are shared. Simulation/collision dependencies have separate readiness and lifetime rules from cosmetic detail.

Bounded workers perform reads, existing manifest/hash verification, glTF decoding, mip generation and compression. They publish immutable prepared results through bounded queues. The renderer admits GPU uploads within per-frame byte and time budgets. Large work is split at safe boundaries such as textures, mip levels and mesh buffers; a monolithic upload cannot be allowed to defeat the frame budget. Atomic publication occurs only when the chosen representation is drawable.

Reserve estimated peak memory before starting a job, including compressed input, decoded images, temporary mip/compression buffers, completed results and upload staging. Reconcile estimates against actual allocations. A bounded job count alone is insufficient.

Use scene generations to discard stale completion results after a jump, new design or scene change. Cancellation removes unstarted work and stops running work at stage boundaries. Never hold the shared cache lock during I/O or expensive processing. Corrupt packages retain their fallback and a useful error; transient failures retry with backoff. Installing a different package hash invalidates an old negative result.

## Predicting when an asset matters

Every demand carries its role, minimum representation, projected importance and estimated time-to-need. Flight predicts a swept region using relative position/velocity, camera view and planned arrival where available. Use time until entering a useful visibility or interaction region, not simply distance divided by ship speed. Stationary nearby objects, lateral motion and large distant silhouettes need explicit handling.

Include a surrounding region and a short rear retention region for turns and backing up. Camera angular motion broadens the prediction. Uncertain motion widens the cheap fallback coverage while limiting speculative expensive detail. Recompute demand as movement changes; do not repeatedly cancel and restart near a boundary.

Studio uses the same service with different demand signals: selected/placed equipment first, visible catalogue cards next, likely next cards after that. There is no meaningful world-distance priority for a catalogue. A jump or teleport explicitly requests the destination set and cancels obsolete speculation.

Use wall-clock loading time. Convert simulated travel into expected wall time when time acceleration is active, and rebuild predictions when that rate changes.

## Two-stage scheduling

1. **Choose an urgency class.** Required to enter/play the scene; visible or actively selected now; predicted near-future demand; speculative surroundings. Physics prerequisites outrank cosmetic detail. Reserve capacity for current-scene work so prefetch cannot monopolize workers or uploads.
2. **Choose work within the class.** Estimate remaining read, decode and upload latency from observed size/type/device statistics. Define `slack = time_to_need - remaining_latency - uncertainty_margin`. Low or negative slack promotes a job. Among similarly urgent jobs, prefer high screen benefit per estimated cost and cheap minimum representations. Aging and reserved progress prevent large jobs from starving.

File size is only an initial estimate of “lightness”: a small compressed image may be expensive to decode and large in memory. Track disk bytes, CPU time, expanded bytes, GPU bytes and upload cost separately. Dependencies inherit the strongest dependent deadline; a cheap child must not obscure an expensive critical dependency.

Example: a hull needed in two seconds with an estimated 1.5-second preparation should begin before a small prop needed in ten seconds. Other capacity can complete cheap visible props while the hull prepares. When already late, show the hull fallback immediately and defer its highest texture detail.

Do not invent LODs at runtime as the first milestone. Existing envelopes provide an initial fallback. Authored coarse meshes and separately streamable texture tiers can follow through the asset contract; until then a GLB remains a whole decode unit.

## Memory and eviction

Maintain separate budgets and accounting for CPU assets, GPU resources and transient work. Include terrain and other major consumers in an aggregate coordinator, even if they retain specialised queues. Configured GPU budgets must work without reliable free-VRAM telemetry; device capability limits are not free memory measurements.

- **Pinned:** current frame resources, required simulation/collision data, current scene minimums and explicit active interactions.
- **Warm:** recently visible or predicted to return soon; useful during turns and mode switches.
- **Cold:** no active demand and low predicted reuse; first eviction candidates.

Use high/low watermarks and a minimum residency grace period to avoid oscillation. Prefer eviction of large, cheap-to-recreate, unlikely-to-return representations. Demote high detail before losing all visual coverage. Being behind the camera lowers demand but is not itself an eviction instruction: shadows, mirrors, rapid turns and other consumers may still need it.

CPU decoded data and GPU data can be evicted separately. Drop redundant source buffers after preparation where possible. CPU projected faces are a separately accounted representation, built only when a consumer needs them. Track strong references outside the service: removing a cache entry does not reclaim memory while a consumer retains an Arc. Consumers should hold leases with clear lifetimes, not permanent model ownership.

GPU retirement follows safe submission lifetimes, and pending retirement remains accounted until reclaimable. On pressure, stop speculation, release cold assets, downgrade optional detail and reduce in-flight reservations. If pinned requirements alone exceed the budget, report the condition and enter a controlled reduced-detail/loading state; never silently discard necessary collision or allow unbounded growth.

## Startup and player experience

Show a responsive shell/loading view before expensive scene preparation. Measure registry decode, content validation, world construction, traffic creation, graphics setup and minimum visual preparation independently. Publish the registry/content snapshot atomically; do not expose partially valid simulation data.

Enter the scene when simulation prerequisites and safe fallback coverage are ready. Optional textures and distant equipment fill in afterward. A jump may require a short explicit readiness gate for collision and canonical terrain; missing visuals alone should not stop play. Model replacement must preserve transforms, selection bounds, material semantics and mechanism state. Unknown terrain must not temporarily become flat physical ground.

## Engine asset profiler

Deliver the profiler with the first loading slice, using the same instrumentation in every game mode. Its core counters are **loads, cache hits and unloads**, visible as session totals, interval rates and per-asset histories. Identify assets by registry key, content hash and representation; show shared use without counting each instance as a separate loaded asset.

Define events precisely:

- **Requests:** consumer requests, including repeated requests for a resident asset.
- **Hits:** a request satisfied immediately by an already-ready representation. Report CPU and GPU hits separately; CPU readiness is not a GPU hit.
- **Misses and coalesced requests:** distinguish a new job from a request joining an existing in-flight job. Joining a job is not a ready-cache hit or another load.
- **Loads:** successful preparation/residency transitions, separately for disk reads, CPU decode and GPU upload. Count attempts, failures and cancellations separately. A GPU re-upload from retained CPU data is not another disk load.
- **Unloads:** actual release of a tracked CPU/GPU representation, including released bytes and reason. Track eviction requests and pending retirement separately from completed release; retained references must not produce fictitious reclaimed bytes.
- **Reloads:** repeated loading of a previously released representation, with time since unload. Highlight frequent eviction/reload cycles and redundant decoding.

Alongside counts, record queue wait and stage durations, bytes read/uploaded, current and peak resident/transient/reserved memory, active leases, last use, priority/deadline, fallback duration and whether a prefetched asset was ever used before eviction. Break down hit rates by representation and define their denominator as eligible requests; report coalescing independently.

Provide an in-engine panel with sortable asset rows and a frame-correlated timeline. A developer should be able to answer: “Which asset stalled this frame?”, “Why was this loaded?”, “Why is it still resident?” and “Did we unload it too soon?” Export a bounded trace plus machine-readable counter snapshots for cold/warm comparisons. Include build, device, scene, package hashes and budget settings so captures are comparable.

Use inexpensive aggregate counters normally, with detailed event capture opt-in and held in a bounded ring buffer. Measure profiler overhead. Record renderer CPU upload time separately from GPU execution time; only label timings GPU measurements when timestamp instrumentation supports them. Mode changes must not reset the session history or instantiate another profiler/cache.

Regression checks cover one preparation for simultaneous duplicate requests, a ready hit without a load, a GPU-only reload, a failed load, cancellation, eviction delayed by a lease, and eventual release. Verify counter accounting against the event trace and memory ledger, including device recreation.

## Delivery plan

1. **Baseline and instrumentation:** cold/warm startup, first input, first useful scene, frame stalls, per-stage timings and peak CPU/GPU/transient memory. Add the engine asset profiler described above, including loads, hits, unloads and reasons for priority and eviction.
2. **First vertical slice:** engine asynchronous model service for all visual consumers, consolidating existing duplicate caches, existing verified packages and fallbacks, bounded workers, deduplication, explicit errors. Move texture preparation off the render path and budget uploads in this slice.
3. **Predictive demand:** camera/ship time-to-need, Studio demand adapters, dependency deadlines, cancellation and anti-starvation. Reuse terrain look-ahead signals and coordinate worker capacity.
4. **Residency:** byte accounting, leases, pressure response, demotion and safe GPU retirement; remove legacy permanent caches and frame-count-only retention.
5. **Optional package improvements:** registry-owned dependency/size/bounds metadata and authored coarse representations. Keep measured runtime cost estimates in a disposable local cache keyed by content hash and preparation version, not as physical registry facts.

Engine owns scheduler, residency and renderer upload work. Game consumers supply demand and fallback policies. Registry owns any asset-contract additions; asset producers supply optional representation tiers. Agree those additions before implementation. No registry migration is required to begin the first slice.

## Acceptance and unresolved measurements

Set numerical budgets after baseline captures on the target machine; this proposal does not claim tested worker counts, millisecond budgets or memory ceilings.

Acceptance scenarios: cold/warm launch; dense port approach; rapid 180-degree turn; jump and cancellation; Studio catalogue scrolling and repeated mode switches; constrained memory; missing/corrupt package; very large texture; GPU resource recreation.

Verify that steady rendering performs no synchronous asset I/O/decode/compression; identical concurrent requests prepare once; large urgent assets make progress; stale work never replaces current content; queues and resident bytes stay bounded including retained references and retirement; returning to a warm scene avoids repeated uploads. Compare first-use latency, frame-time percentiles and worst stalls, allocation peaks, deadline misses and fallback duration against baseline. Separately regression-test physics, saved keys, transforms, transparency and motion contracts.

The first implementation decision should follow the startup trace: asset streaming fixes identifiable first-use stalls, but it will not by itself solve expensive simulation construction or shader/pipeline setup.
