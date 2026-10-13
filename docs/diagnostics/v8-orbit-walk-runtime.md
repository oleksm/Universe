# v8 orbit-to-walk runtime pilot

2026-10-13, isolated `engine-pgs-walk`, based on main `0c484a56`. No installation or main merge. This bounded pilot does **not** pass the complete playable-journey, regional walking-geometry or frame-time gates.

## Frozen input and lab policy

Package: `/home/alexm/git/planet-trees/out/planet/landscape_v8_seed83_20/landscape_002/review/reader`.
Surface SHA256 `4b30e7eda8e707dff50498f0be80476bf82d5dfd4b8160c662b1a2113dadfe80`, 167,175,056 bytes, 1,607,328 points and 3,214,652 faces. The independent Rust acceptance CLI verifies manifest and fixture hashes, passes all 4,352 owner/height/water/category queries, maximum height difference 1.147e-8 m. Decode/index took 2.753 s; coherent/distributed query batches averaged 0.154/0.740 microseconds. These are local warm-cache measurements, not latency guarantees.

Explicit temporary host is Treistun E, radius 6,281,370 m and unchanged host physics/rotation/air. Source radius is 6,371,000 m. Unit directions and absolute heights are preserved; horizontal lengths scale by 0.9859315649 and slopes change. The full globe contains **one** detailed landscape, not planet-wide detailed coverage. Source and registry remain unchanged.

## Loading and visual runs

Existing main release ran Vulkan orbit and lowflight captures at 1280x720, render scale 1, 60 FPS cap, neutral palette, terrain shadows off, settlers 0, paused. Orbit first shell was 257 ms and CPU/physics-ready 4,372 ms. Peak child RSS was about 1.91 GiB. This includes renderer/process allocations; it is not PGS resident memory alone. The full PGS/index stays resident; only render patches stream. No canonical disk-paging claim.

Time-zero lowflight was dark (sun -7.47 degrees); retained as diagnostic evidence. Separate daylight capture uses 6.3 hours, sun elevation 55.02 degrees, identical site and source. It shows the regional geometry. These fixed captures do not establish continuous descent or walking. Final near-ground capture uses altitude 1.7 m at corridor latitude -50.235008063051644, longitude -19.39232336991895. Actual camera datum height is logged, rather than inferred from the spawn argument.

## Found and fixed: elevated terrain selected coarse patches

`terrain_lod.rs` previously measured split distance from the datum sphere (`mid * radius`). The corridor is about 5 km above that sphere, so a camera at the feet selected only level 13: nominal 75.28 m cells. At 81 provided corridor directions with an eye 1.7 m above each canonical height, settled mesh reconstruction failed the 0.10 m screen at 76 positions: maximum 0.7830 m, RMS 0.4608 m.

The isolated change measures canonical LOD distance from cached patch origins; missing child predictions inherit the nearest resident ancestor's elevation. No terrain query runs on the draw thread for this decision. Existing noncanonical selection, worker count, maximum level, queue/residency limits, physics and shader morph remain unchanged. Actual patches refine with their own sampled centre once ready. This is a centre-distance improvement, not an error-bounded LOD selector.

With terrain-centred distance and unchanged level-15 cap, the 81 settled corridor probes have maximum 0.07542 m, RMS 0.03793 m, zero over 0.10 m. Diagnostic caps 17/19 give maximum 0.01640/0.00423 m, but those caps are **not enabled in the runtime patch**.

The broader 20 km square (6,561 probes, 250 m spacing, independent 1.7 m eye at each point) still fails: maximum 2.99299 m, RMS 0.11330 m, 217 probes over 0.10 m. Worst local offset is (-5250,250) m on the camera tangent plane. Grid resampling still misses canonical features; passing the easy corridor does not establish walking agreement at crests, joins or shores.

An opt-in `UNIVERSE_PGS1_CONTACT_AUDIT=1` logs radial intersections with the **actual submitted resident grid vertices**, reproducing the shader's f32 geomorph and excluding skirts. It is CPU reconstruction of submitted geometry, not GPU depth readback. Final fixed-point snapshots reached level15, 1,018 resident patches, pending0; canonical height 5172.089497742 m, render height 5172.100410355 m, gap +0.010912613 m. Earlier snapshot had 12 pending patches. This single position passes; it does not supersede the regional failures or verify the entire loading interval.

## Frame gate remains failed

Final candidate's raw last-300-frame window: median 51.147 ms, p95 51.591 ms, maximum 90.324 ms, failing the declared p95 <=33.3 ms screen. Profiler scopes identify roughly 50 ms surface acquisition; CPU drawing is around 1 ms, GPU passes small. Historical title/compositor placement confounds remain; desktop settings were not changed and no causal display fix is claimed. Some validation/build jobs overlapped under separate shared guards, so these are bounded correctness/diagnostic runs, not controlled throughput benchmarks. Raw timing counters and profiler scopes have different windows; retain the originals rather than conflating them.

Generic `UNIVERSE_FRAME_TIMINGS=1` now logs raw history and existing resource/timing counters at an automatic screenshot. No timing behavior changes.

## Physics journey scope

Ships owns the separate command-driven shared World/Person replay on `ships-pgs-journey`. Initial 100 km circular-orbit attempt crashed in Approach at 711.02 simulated seconds. A separate orbit run with a 30-second coast before unchanged landing guidance landed at 2330.14 simulated seconds, then exited, walked 12.82 m and jumped about 0.445 m. A separate 1000 m lowflight run landed in 72.78 s. These are independent trials with disclosed initial/command timing, not a successful replay of the failed trial.

Reboarding remains under investigation at the Drover ramp: `inside_hatch` finds no walk floor. No invented floor, state reset, teleport to seat or alternate walker is accepted. Headless command replay and fixed Vulkan captures are separate evidence, not one GUI journey. Return to orbit and revisit after eviction remain unverified.

## Validation and next bounded work

Release build, registry build and 68 workspace tests pass, including a regression that elevated canonical ground refines near the feet and pending children inherit known elevation. The registry build retains its pre-existing report gaps. Full source/binary/artifact hashes and commands are saved in the runtime directory. Player-build check (`--no-default-features`) also passes; its existing dead-code warning is retained in the log.

Next: resolve the actual shared-hull boarding path and approach reliability, then run one continuous GUI journey with repeatable inputs; preserve canonical crease/shore geometry near the feet or introduce a measured-error refinement policy; separate surface-presentation waits from terrain cost in a controlled frame test; exercise orbit return/patch eviction. No further bakery terrain edits, global coverage expansion or production installation are justified by this pilot.

Evidence root: `/home/alexm/git/universe-engine-pgs-walk/out/review/v8-walk-runtime` (`runs.json`, `day-runs.json`, `candidate-runs.json`, `reader.log`, contact JSONs, `summary.json`, screenshots, build/test logs). `run.py`, `run-day.py`, `run-candidate.py` and `controls.py` record reproduction commands. All heavy jobs use the shared scenery launcher with 2 CPU, <=8 GiB, <=600 seconds per job. CPU audit source: `crates/core/world/examples/pgs_walk_audit.rs`.
