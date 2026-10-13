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
