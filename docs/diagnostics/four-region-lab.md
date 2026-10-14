# Four-region lab: the S15 package in the game

2026-10-13, planets' request `planet-unfold/docs/requests/2026-10-13-four-region-lab.md` (plan step 3). Source:
`~/planet-data/pgs2/s15-4regions-b` (manifest SHA256 `bbe1ffa65345…`, the trace's `source_sha256`), the S15 root and
four non-adjacent regions. The two v6 regions (20079, 7717) are **v6 construction, never accepted: scaffolding**, in
every capture and number below. Host: Treistun E, radius 6,281,370 m; source radius 6,371,000 m; directions and
heights preserved. Branch `engine-claude-lab4`. Evidence: `out/review/lab4/` (local, kept for planets).

## What was built

- `worlds::pgs::Surface::open_package` (`crates/core/world/src/worlds/pgs/package.rs`): the revision-4 reader behind
  the engine's `Surface`, so physics, water, categories, LOD and the contact group read the composed surface with no
  other change. Files mapped; the manifest's opening records checked at open; each 64 KiB block checked against its
  kind-19 hash on first touch; regions opened on their first query (header, table, hash section); manifest counts and
  height bounds held (a point outside the bounds is refused when read); composed face ids and cross-file neighbours
  (kind 18, including the "across is itself replaced" case) for `face_geometry`. The canonical order is checked on
  touch. Nothing resampled into collision.
- `pgs2_surface` example: the lab's goldens through `Surface` (226 of 226, 5 of 5 refused) and every face's neighbours
  reciprocal over the same edge (goldens: 604 faces, two regions sharing an edge; s15: the first 50,000 composed faces).
- Dev lab: `UNIVERSE_PGS1=<package folder>` loads a package; `UNIVERSE_PGS1_FLIGHT=<file>` flies the camera through
  keyframes (`frame dx dy dz alt[g] pitch heading [shot]`, body-frame directions as planets' `places.json` gives them)
  with captures; the package's reads, page-cache residency and process RSS logged every 300 frames
  (`PGS_PACKAGE`); `UNIVERSE_PGS1_NO_AIR=1` drops the aerial haze for diagnostic captures.
- `pgs_walk_audit` reads packages and makes the 81-probe line corridor (`PGS_WALK_LINE`).

## Loading

| | first touch | open | bytes read at first touch | read by the end of the flight |
|---|---|---|---|---|
| package (manifest, root) | start | 0.16–0.29 ms | 1.9 MB (root) | root 2.7 MB |
| v8 41014 | frame 0 (orbit) | 0.08 ms | 1.1 MB | 191.5 MB (whole) |
| steep 35607 | ≤ frame 300 (orbit) | 0.12 ms | 4.9 MB | 191.9 MB (whole) |
| v6 20079 (scaffolding) | ≤ frame 300 (orbit) | 0.06 ms | 4.4 MB | 53.7 MB (whole) |
| v6 7717 (scaffolding) | ≤ frame 300 (orbit) | 0.05 ms | 4.8 MB | 53.9 MB (whole) |

- **Every region is touched from orbit**: the globe's coarse patches sample the composed surface, and some of their
  vertices fall inside each footprint. So no region stays unopened while the globe is drawn.
- **Each region is read whole on approach.** At 800 km over the v8 region 90 MB were read; by 2 km all of it. Coarse
  patches sample the full-resolution region at scattered points, so their queries land in nearly every 64 KiB block.
  Spatial order helps a walker's queries, not this. A region needs a coarse representation for distant views (render
  tiles are the engine's; built once from the region, or a coarse level in the package). This is the main finding.
- Process RSS rose from 661 MB at start to 1,507 MB with all four regions read. About 491 MB of that is the
  package's mapped pages: file-backed and reclaimable, counted in RSS. After leaving the regions (back at
  9,000 km) nothing is released: the reader keeps no copies, but the mapped pages stay resident until the kernel
  needs them. A residency policy (drop untouched regions' pages) is not built.
- Loading states: a region opens synchronously on its first query (≤ 0.12 ms for the header; first blocks follow),
  so no query is ever answered from provisional ground. The readiness trace's `ready` covers the walker's contact
  group (planet-readiness-trace/1); nothing provisional enters collision.

## Descent, landing, contact

One flight (`flight/four-regions.flight`, 42 keyframes, 22,300 frames): orbit 9,000 km, then for each region 800 km,
150 km, 15 km, 2 km over the ground, 1.7 m at the cap centre, and for the v8 and steep regions 1.7 m at the crest and
the steep face. 27 captures in `flight/`; the haze-free opening in `flight/noair/`.

Landings (`land/<region>/`, groundwalk at the cap centre, the walker walking on, contact group on, readiness trace):

| region | sun | frames | ready from | not ready after | contact (dry, ready) | walked | groups | peak extra GPU |
|---|---|---|---|---|---|---|---|---|
| v8 41014 | 55° | 6,008 | 88 | 0 | ≤ 1.4 µm | 227 m | 36 | 12.0 MB |
| steep 35607 | 71° | 1,275 | 147 | 0 | ≤ 1.5 µm | 82 m | 13 | 11.1 MB |
| v6 20079 (scaffolding) | 67° | 14,400 | 98 | 0 | ≤ 2.1 µm | 273 m | 51 | 9.4 MB |
| v6 7717 (scaffolding) | 30° | 4,912 | 224 | 0 | ≤ 1.4 µm | 105 m | 17 | 7.4 MB |

Planets' checker (`tools/planet_runtime_trace_check.py`, `check.json` beside each trace) fails only frames 2 to the
frame before ready (startup, no group) in all four. The 20079 run hit its 300 s timeout (startup took 110 s at 12 h
on that spot, against about 30 s elsewhere; not investigated); its `trace.json` is assembled from the batches the trace
appends as it runs (`trace.frames.jsonl`), and says so. Its landing capture is `flight/v6gentle-day/`.

**1.7 m contact screen** (`contact/`, the settled-patch reconstruction at 1.7 m over each of 81 probes, ±0.10 m; the
contact group the walker stands on is not in it): the v8 region on the handoff corridor passes as before (max 0.0754 m,
RMS 0.0379 m, 0 over). The others, on 81 probes at 100 m along the local downhill through the cap centre (directions
in `<region>.json.corridor.json`), fail: steep 35607 max 2.61 m, RMS 0.40 m, 9 over; v6 20079 max 0.39 m, 7 over;
v6 7717 max 0.98 m, 7 over. Level-15 patches (18.8 m cells) miss steep features; the contact group carries the walker.

## Seams and appearance

- **Root/region seam**: by construction none. A region's boundary edges are the root's edges with byte-identical
  points (checked on touch), so the two sides' planes agree along the edge exactly. The 15 km and 2 km captures show no
  crack at the footprints, but neutral grey terrain under heavy haze is a weak screen: from 15 km to orbit the haze
  hides most relief, the same with the monolithic v8 (`flight/mono/`, pixel-identical views), so it is not the package.
- **Dotted dark lines** along ridges and across slopes in some 1.7 m captures (`v8-41014-crest-1.7m.png` taken just
  after a 1.7 m-high traverse, `v6gentle-day/`): not at a footprint (the boundary is beyond 50 km there). The crest
  re-taken from a standing start shows none, and package and monolithic give pixel-identical frames there
  (`flight/crest-ab/`). Likely patch skirts or cracks at LOD borders in rough ground; **not identified**.

## Not done

- **Frame p95 at 1280×720**: needs a visible window on the owner's screen (workspace 8 is compositor-throttled).
- The 6,561-probe regional screen.
- Residency policy, coarse region representation for distance.
