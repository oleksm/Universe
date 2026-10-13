# Bounded canonical-crease CPU prototype

2026-10-13, `engine-pgs-walk`, after `a7baecdc`. **CPU sampled-geometry pass at eight local fixtures; no runtime, GPU, loading or journey acceptance.** No installation/main merge or canonical source/physics change.

## Correction to the presentation comparison

The preceding report's equal-camera claim was wrong. Saved `PGS_CONTACT` samples show:

| Run | Eye/canonical height initially | Later height | Change frame |
|---|---|---|---|
| Baseline | 5214.612914 / 5213.701666 m | 5223.964697 / 5211.180623 m | 540 |
| Alternate title, vsync | same initial pair | unchanged at logged samples | — |
| Alternate title, no-vsync | same initial pair | unchanged at logged samples | — |
| Baseline repeat | same initial pair | same later pair | 1140; another ~12 µm radial change at1230 |

Baseline's change precedes its final timing window. The repeat's change occurs inside its final window even allowing the 30-draw logging phase. Consequently the baseline and alternate-title views differ by about 9.352 m radially and query different canonical directions. Full position/orientation/time was not captured; equal scalar heights do not establish transform identity. The numerical p95 observations remain, but neither fixed-camera attribution nor a default presentation change is justified.

The saved source still updates cameras from asynchronous snapshots and cockpit prediction when paused; zero warp is sent by the UI after the engine starts. These are possible diagnostic paths, not a demonstrated cause of the late jump. No new GPU batch or speculative simulation change was used to explain it. `pgs_camera_drift.py` reproduces the evidence and the preceding report is corrected.

## Budget and method

`out/review/v8-crease-cpu/plan.json` was written before implementation. Per fixture: one 128×128 m tangent-plane square, split into two adjacent 64×128 m tiles; 8 m clipping cells; at most 4,096 examined faces, 32,768 output vertices, 32,768 output triangles, 2 MiB serialized geometry, 16 MiB traced temporary allocations excluding the resident source, and 300,000 audit queries. Jobs use two CPU, 8 GiB and at most540 seconds. Exceeding a cap refuses the result rather than enlarging it.

The source remains SHA256 `4b30e7eda8e707dff50498f0be80476bf82d5dfd4b8160c662b1a2113dadfe80`; host radius6,281,370 m with the earlier explicit source-radius policy. The Python prototype reads checked sections from that exact source. The independent Rust audit runs the production strict PGS reader, including full topology validation. This prototype's short section reader is not a replacement loader.

A bounded adjacency walk starts from the frozen feature's known face(s). In a shared gnomonic tangent chart, canonical spherical edges are straight. Each source-face footprint is clipped against the pair bounds and 8 m cells, then triangulated **inside that face**. Stored chord-plane barycentric heights are evaluated at the new directions, followed by radial embedding at the host radius. Wet faces use their body level, with body and face IDs retained on every output triangle. No two overlapping terrain sheets or changed collision field are introduced.

Intersection keys are rounded to10 nm in tangent metres, far below the declared sampled target; this is numerical edge retention, not an exact-arithmetic theorem. Each tile evaluates heights with its own vertex cache and uses a separate origin with f32 local coordinates. Shared boundary split-key sets must match; their reconstructed world positions are compared after f32 conversion. No cache is shared between tiles to force the seam result. Coverage area and emitted area are checked; a separate saved-mesh tool verifies edge incidence across both tiles.

The native-face control is the centroid of the larger adjacent shore face, selected from the frozen fixture, not an unbounded global search. Its longest source edge is194.2 km. Other native join faces reach213.6 km; only their local clipped portions are emitted. This exercises radial embedding of large source faces without drawing them whole.

## Frozen results

All eight fixtures build within the original limits. The independent Rust audit checks **127,374** samples at f32 triangle vertices, edge fractions/midpoints and interiors, plus all49 frozen boundary probes by radial triangle intersections. Maximum sampled error is **4.514 µm**; maximum error on the frozen queries is **2.237 µm**. All triangle-centroid query owners equal their stored face provenance. These are finite samples, not a continuous geometric-error bound.

| Fixture | Searched faces | Vertices / triangles | Instrumented Python build |
|---|---:|---:|---:|
| Lake shore | 6 | 328 / 554 | 205 ms |
| Parent-face join | 8 | 331 / 558 | 212 ms |
| Core/native collar | 85 | 676 / 1196 | 458 ms |
| Relief/native join | 146 | 836 / 1494 | 540 ms |
| Crest1 | 235 | 1027 / 1849 | 690 ms |
| Crest2 | 229 | 1020 / 1840 | 670 ms |
| Crest3 | 219 | 994 / 1791 | 660 ms |
| Large native face | 8 | 310 / 516 | 206 ms |

Largest packed position/index/provenance payload is49,304 bytes; largest JSON geometry artifact is142,996 bytes. Maximum traced temporary allocation is1,291,252 bytes. Whole Python-process peak RSS is201,756 KiB, including the167,175,056-byte resident source. Tracemalloc covers Python/NumPy allocations used here, not a portable whole-process ceiling; the separate guarded job limits still apply. Instrumented timings include allocation tracing and Python overhead and are not predictions of a Rust worker's cost. Initial section read/hash is74 ms; independent Rust full decode/index/validation is2.726 s on this local run.

Maximum adjacent-tile seam displacement is **2.725 µm**, below the frozen10 µm seam target. Independent edge-incidence checks find zero unmatched/nonmanifold interior edges in all pairs. The source-backed crest now passes where the level19 regular grid missed by0.167564 m. This isolates representation at the frozen points; it does not establish visual appeal, normals, LOD seams against the existing outer renderer or walking during loading.

Search cap1, output-vertex cap1 and serialized-byte cap32 all refuse explicitly. A separate independent-reader query-cap1 run exits with `sample cap` and publishes no report. No cap is raised after a failure. Final source hashes bind the successful artifacts.

## What this permits next

Report the cost before GPU integration. The prototype uses frozen seed faces; runtime needs a bounded read-only face-access/search API starting from the existing canonical owner query. A compiled worker, cancellation and source-generation checks, atomic publication of compatible tile groups, retained old coverage while replacement builds, explicit missing/over-budget readiness, and a separate GPU byte/lifetime budget remain to be designed and measured. Shader geomorph must not erase the retained crease. Normals/material ownership and interfaces to distant patches also need validation.

No uniform LOD-cap increase beyond the previous held experiment, no simulation reset, no new hull/access work, and no change to accepted water or terrain occurred. Full return/eviction and a continuous rendered descent/walk demonstration remain held; previous headless command evidence is separate.

## Reproduction and evidence

Tools: `tools/diagnostics/pgs_crease_cpu.py`, `pgs_crease_topology.py`, `pgs_camera_drift.py`; independent Rust example `crates/core/world/examples/pgs_crease_audit.rs`. Runtime files are unchanged in this step. Exact commands, plans, original/final artifacts, negative cases and logs are under `/home/alexm/git/universe-engine-pgs-walk/out/review/v8-crease-cpu`. `geometry-final` and `rust-audit-final.json` are the delivered geometry/audit; earlier generations are retained but superseded. Machine summary and hashes: `docs/diagnostics/v8-crease-cpu.json`.

Validation: registry build and 68 release-workspace tests pass; the new independent Rust example compiles and runs successfully. Runtime source is unchanged in this follow-up.
