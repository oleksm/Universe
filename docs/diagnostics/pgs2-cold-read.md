# PGS1 revision 4: cold open and first query

The v8 package at `~/planet-data/pgs2/v8` (planet-trees `pgs2` 2c33017; contract planet-unfold 2f73d6f
`format-v4.md`) read as the engine would: `crates/core/world/examples/pgs2_cold.rs`. Files mapped
(`MADV_RANDOM`); the manifest's opening records checked at open (header and table sha256, the kind-19 section's
sha256); each 64 KiB block checked against its kind-19 hash on first touch; candidates from the cell index (kinds
17, 11, 12); the footprint looked up in `footprints.bin`; a region opened on first touch. No tree; no section read
whole. Report: `out/review/pgs2/cold-v8-{1,2,3}.json` (local).

**Cold** means the package's pages were dropped from the page cache (`posix_fadvise DONTNEED`; `mincore` shows 0
resident before each run). File-system metadata stays cached: a cold boot may be slower. NVMe (Corsair MP700 PRO),
btrfs, Linux 7.2.

| first query | open + query | bytes read | blocks checked |
|---|---|---|---|
| region cap centre | 11.2–21.7 ms | 3.92 MB | root 6, region 52 (points 25, faces 24) |
| antipode, root only | 1.3–1.4 ms | 0.45 MB | root 7 |

Before each block was asked for in one read (`MADV_WILLNEED`), its sixteen pages faulted in one at a time: the
region query took 41–61 ms and the root query 6.4–6.8 ms.

**Answers.** All 4,352 v8 fixtures from the package against the canonical reader (`worlds::pgs::Surface`) on the
monolithic v8 file: composed face, height (bit for bit), water and categories identical. Against the fixture file's
own heights (written by the lab): 481 bit-identical, the rest within 1.2e-8 m (the fixtures' tolerance is 1 mm).
Ties: every root and region face whose cell lists it and whose closed triangle holds the direction; the owner is the
least composed id, computed from the region file's canonical order without reading it whole. The direction is
divided by its length, as the canonical reader does (multiplying by the reciprocal moved heights by up to 4.5e-10 m).

**What costs.** The region's cell holds 19 faces, but those faces and their points lie in 49 different 64 KiB
blocks: the canonical order (faces sorted by vertex triples, added points in the writer's order) is not spatial. With
added points along a space-filling curve over the cube-face cells, and faces following by the same canonical sort,
a cell's faces and points would sit in a few blocks: a few hundred KB per first query instead of 3.9 MB.

## Goldens (planet-trees `pgs2` 8f93131, `tests/fixtures/pgs2-v1`)

`pgs2_cold --goldens`: root of 300 points and two regions sharing a root edge. All 226 queries pass: owner
(file, local face), tie set (15 ties), composed face id, categories, height within the 1 mm tolerance (51 bit
identical, the rest within 2.4e-12 m of the lab's heights). The five broken packages are refused for their reasons:
footprints hash, block 0 hash (on touch), header/table hash, overlapping footprints, faces out of canonical order.
The reader checks the region id in a region's header against the manifest, and the canonical order on touch (each
held face against its stored neighbours, by composed vertex keys: kept points by root id, then added points in
stored order). With these checks the v8 figures above hold (region 11–17 ms, root 1.3 ms, 4.0 MB / 0.45 MB).

## Spatial canonical order (planet-trees `pgs2` 8d54089, planet-unfold 9ddb838)

Kept points by root id; added points along the cell curve (cube face, Morton code of (i, j) on the file's n); faces
by the curve key of their centroid, then their composed triple. The reader's on-touch check uses that key (stored
order non-decreasing). Goldens rebuilt under the rule: 226 of 226 pass, 5 of 5 refused.

| first query, cold | open + query | bytes read | region blocks checked |
|---|---|---|---|
| region cap centre | 5.6–10.5 ms (38.5 ms once, run right after reading the 167 MB monolithic file) | 1.54 MB (was 3.99) | 16: points 7, faces 5, cells 1, cell faces 1, root ids 2 (was 52) |
| antipode, root only | 1.3–1.4 ms | 0.45 MB | — |

All 4,352 v8 fixtures against the canonical reader on the monolithic file: height bit for bit, and the owning
triangle (its stored vertices) identical; face ids differ (313 equal) because the region's faces are reordered.

Most of the bytes are the 64 KiB hash blocks: a cell's 19 faces are 608 B and their points about 2.3 KB, but each
touch checks a whole block. A finer grid would not change that; smaller hash blocks would (16 KiB blocks: a hash
section four times larger, 373 KB to check at open for this region).
