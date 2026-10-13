# Bounded v8 near-ground refinement

2026-10-13, isolated `engine-pgs-walk`, follow-up to `ac27d5a5`. **Not accepted for installation or main merge.** Source, host physics and registry are unchanged. This experiment improves settled contact at the original witnesses, but still fails sharp-crest and loading-contact gates.

## Frozen scope

Surface SHA256 `4b30e7eda8e707dff50498f0be80476bf82d5dfd4b8160c662b1a2113dadfe80`; source radius 6,371,000 m, explicit Treistun E host radius 6,281,370 m. Directions and heights remain unchanged; horizontal lengths retain the previously declared scale difference. Full canonical PGS/index remains resident; only render patches stream.

Before running, `out/review/v8-near-ground/plan.json` fixed canonical level cap 19, grid 16, 3,000 cached patches, 48 pending requests, two workers and 8 GiB per guarded job. No increase beyond that cap follows a failed gate. Noncanonical LOD is unchanged. This is a distance-selected uniform-grid prototype, not a certified error-bounded representation.

## Contact evidence

The settled ideal reconstruction over the same 6,561 regional probes gives maximum 0.060414 m, RMS 0.001494 m, zero above 0.10 m, versus level-15 maximum 2.99299 m and 217 failures. It reproduces f32 patch coordinates and geomorph but assumes settled refinement; it is not proof of asynchronous residency.

Planets then supplied 49 hash-bound boundary queries (`b782208`, `planet-scale-boundary-probes-v1/handoff.json`). At level 19:

| Probe group | Count | Maximum gap | Above 0.10 m |
|---|---:|---:|---:|
| Core/native collar interface | 7 | 0.000000063 m | 0 |
| Parent-face interface | 7 | 0.000147 m | 0 |
| Relief/exact-native-face join | 7 | 0.00000482 m | 0 |
| Local crests | 21 | 0.167564 m | 5 |
| Known wet/dry shore | 7 | 0.001195 m | 0 |

These targeted failures demonstrate why the broad grid is insufficient. The shore result concerns geometric height agreement only, not shoreline topology, water animation, or walking across water. Source queries and visible-water-level sampling are retained separately.

Actual Vulkan runs log radial intersections against the submitted resident grid, reconstructing shader morph and excluding skirts. This is CPU inspection of submitted geometry, not GPU depth readback. Fixed cameras are paused at 1.7 m, daylight 6.3 hours, neutral mode, shadows off, 1280x720 and 60 FPS cap. Each capture is an independent launch, not a continuous walking journey. All per-frame samples and final residency counts are in `summary.json`. The 12 original worst sites settle within 0.10 m (largest measured absolute gap 0.053660 m); the targeted crest independently reproduces -0.167564 m and the shore +0.001195 m. Resident peaks are 1,302–1,390 patches. A signal-9 interruption at worst-09 is retained in runs.json and followed by a successful separate retry; its cause was not established.

Loading is a separate failure: the first visible parent at worst-01 is about 3,864 m below canonical contact and worst-02 about 4,138 m below. Missing roots draw the existing base globe and are explicitly logged without a fictitious intersection. Canonical physics does not move to these approximations. A settled pass must not be described as contact agreement throughout loading.

The declared count/queue bounds remain unchanged. They do not constitute a process/GPU byte ceiling: the canonical source is resident and GPU mesh retention has its own lifetime. Eviction/revisit and continuous walking through the transition are not verified by fixed captures. The failed crest gate prevents acceptance without those remaining checks anyway.

## Presentation experiment

Diagnostic-only overrides `UNIVERSE_DIAGNOSTIC_TITLE` and `UNIVERSE_DIAGNOSTIC_PRESENT=auto-vsync|auto-no-vsync` apply only to screenshot/test launches. Normal launch configuration remains unchanged. Surface size and configured mode are logged. The fixed screenshot size is held constant even if compositor placement changes window size. Title comparisons test the combined window-placement/presentation path; they cannot by themselves establish a driver defect.

Controlled runs follow completion of correctness captures and builds. One factor changes at a time: baseline, alternate title with the same mode, alternate title with no-vsync, then baseline repeat. Raw frame windows, exact environments, binary hashes and surface/profiler evidence are archived. Desktop configuration is untouched. At frame 1500, the last-300-frame results are:

| Run | p95 | Gate |
|---|---:|---|
| Baseline title, AutoVsync | 52.104 ms | fail |
| Alternate title, AutoVsync | 49.930 ms | fail |
| Alternate title, AutoNoVsync | 16.701 ms | pass in this run |
| Baseline repeat, AutoVsync | 49.722 ms | fail |

All four finish level19/pending0, with settled samples more than 300 terrain draws before the final sample. **Correction (follow-up crease audit): same launch parameters, source and binary were used, but measured eye heights differ between runs and baseline-repeat moves inside its final timing window. Full camera transforms were not logged. The fixed-camera claim is withdrawn;** asynchronous cache contents are not asserted byte-identical. No other engine build or capture overlaps this set; ships/planets/Blender agreed to hold heavy work. Existing interactive desktop processes remain. The order is not randomized and compositor state is not controlled, so this is bounded evidence of presentation sensitivity, not a universal driver diagnosis or justification for changing the default.

The first frame480 comparison was retained but rejected as a settled-geometry comparison: faster presentation reached the screenshot while terrain was still level7. The longer warmup addresses that specific confound. Profiler captures show approximately 1–2 ms CPU drawing and sub-millisecond-to-low-millisecond render work, while vsync surface acquisition can stall near50 ms. Raw frame-history and smoothed wait/resource counters use different windows; their numbers are not interchangeable. No-vsync remains a diagnostic override, not an installed fix.

## Decision

Do not increase the refinement cap opportunistically or claim a complete fix. Next geometry work needs measured local approximation error and preservation of canonical creases, with a clear loading/readiness contract and bounded eviction/revisit checks. Simply sampling smaller regular cells reduces error but does not remove its cause. Readiness must not silently redefine physics or fabricate ground.

Ships independently owns supported-hull exit/reboarding and a real shared command journey. The Drover lacks its walk floor; the existing ORE CUTTER can land but its tested walker route meets a cockpit wall. Neither is repaired by this visual prototype. Main, installed assets and binaries are untouched. Ships records `04b47fb6` and `1dc4906c` preserve the separate command journey and native access failures.

Evidence root: `/home/alexm/git/universe-engine-pgs-walk/out/review/v8-near-ground`. `sites.json`, `runs.json`, `presentation-runs.json`, `summary.json`, `hashes.json`, logs and images preserve exact commands, outcomes and failures. The regional settled result is `out/review/v8-walk-runtime/region-contact19.json`.

## Build validation

Registry build, 68 release-workspace tests and player release check (`--no-default-features`) pass. The existing player dead-code warning remains. Debug dependency warmup was stopped in favor of the cached release test profile. An optional post-test audit metadata cleanup rebuild hit its 60-second bound; the cleanup was reverted and the previously successful exact audit source/output retained. In the tool JSON, legacy `dry_samples` counts all included samples when water inclusion is enabled; `wet_queries=4` identifies the four wet queries out of 49. No terrain result depends on this field name.

### Camera correction, subsequent saved-log audit

Baseline changes from eye/canonical height 5214.612914/5213.701666 m to 5223.964697/5211.180623 m at terrain frame540, before its final timing window. Baseline-repeat changes at frame1140, inside its final window, with a further ~12 micrometre radial shift at1230. Both alternate-title runs retain the earlier scalar heights at logged samples. Thus baseline and title runs differ by ~9.352 m radially and sample different canonical directions; the repeat is not stationary. Full orientation, position and simulation-time traces are absent, so scalar constancy cannot prove an identical view. Saved source shows paused mode still updates the camera from asynchronous snapshots/prediction and only sends zero warp after the engine starts; this is an investigative lead, not an established cause of the later jump. Presentation numbers remain observations with this confound, not isolated attribution. See `v8-crease-cpu.json` and `out/review/v8-crease-cpu/camera-drift.json`. No GPU rerun or default-mode change follows this correction.
