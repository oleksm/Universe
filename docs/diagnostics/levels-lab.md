# Levels lab: PGS1 revision 5 on s15-4regions-c (stopped)

2026-10-13, planets' request `planet-unfold/docs/requests/2026-10-13-levels-lab.md` (agentmsg
20261013T204353-planets-2d24). Package `~/planet-data/pgs2/s15-4regions-c`, written under planet-trees-pgs2
29c873c/9c57600: the S15 root and four regions, each with levels 1 to 6 (40 to 1,280 m spacing). The two v6 regions
are scaffolding. Branch `engine-claude-levels`. Evidence: `out/review/levels/` (local).

**The owner stopped the lab here** ("we should start over; store this work, we will need it"). Below is only what
was measured. Gates 3 to 6 and the frame p95 were **not measured**.

## What was built

- Revision-5 reader behind `Surface` (`worlds/pgs/package.rs`): header version 2, kind 20, levels checked at open
  (order, declared deviations monotone, root above the last level), each level held to level 0 on touch.
  `query_to(dir, tolerance)` picks the coarsest representation (root, level k, level 0) whose declared deviation is
  within the tolerance; composed ids follow the rev-4 rule over that mixture. Tolerance 0 is revision 4 exactly.
- Drawn tolerance: half a pixel (4.1e-4 rad) times the patch's distance, so `patch_tolerance(cell)` = 9.84e-4 × the
  patch size. The globe map uses half a texel. Physics and the contact group use 0.
  `UNIVERSE_PGS1_LEVEL0_ONLY=1` forces 0 for matched captures.
- Residency: region files untouched for 20 s, or over a 256 MB budget, are released (madvise and fadvise DONTNEED);
  their blocks are checked again on the next touch.
- `verify_levels`: the gate 1/2 check, using planets' counting rules (f02bcc4, 29c873c).
- `pgs2_levels` example: the check plus a 301×301 sweep per region. Diagnostics: `PGS_VERIFY_NEAR=1` lists
  decisions near the thresholds; `PGS_VERIFY_DUMP=<dir>` writes every counted crossing.
- Goldens `pgs2-v2-levels` (29c873c): 2,508 of 2,508 queries, 5 of 5 refused. Rev-4 goldens: 226 of 226.

## Gates 1 and 2 on -c (measured)

In all four regions, at every level, the level points are byte-identical to level 0. The declared max is never
below the measured one, and the measured max equals the manifest's `deviation_measured_max_m` to the last digit.
Overlay counts (mine / manifest) and RMS (mine / manifest):

| region | L1 | L2 | L3 | L4 to L6 |
|---|---|---|---|---|
| v8 41014 | 3,457,399 / 3,457,400; 0.600172 / 0.600168 | 3,034,208 / 3,034,210; 1.491426 / 1.491447 | exact | exact |
| steep 35607 | 3,046,899 / 3,046,898; 0.640344 / 0.640336 | 3,371,302 / 3,371,303; 1.535493 / 1.535492 | exact | exact |
| v6 20079 | 734,602 / 734,601; 0.811397 / 0.811398 | exact | 647,985 / 647,984; 4.257063 / 4.257035 | exact |
| v6 7717 | 803,768 / 803,766; 0.830112 / 0.830113 | 721,460 / 721,458; 1.835101 / 1.835137 | 688,032 / 688,026; 4.486676 / 4.486482 | L4 662,292 / 662,293, 10.280279 / 10.280272; L5, L6 exact |

So the counts differ by up to 6 overlay vertices and the RMS by up to 1.9e-4 m. The maximum is exact everywhere.

**Why the counts differ** (v8, from both readers' crossing dumps: mine in `out/review/levels/crossings/`, planets'
in the package's `crossings/`). L1: 4 crossings only planets counts, 3 only I count. L2: 5 only planets, 3 only
mine. Nearly all of them lie on a threshold:
- the END margin (t or u near 1e-9 from an end: margins 1.0e-9 to 1.07e-9, one at 7.9e-9), or
- the relative collinear test (denominator ratio 1.07e-9 to 2.1e-9).

The two readers' 2D coordinates differ by enough to move such cases across. The vertex-touch cluster of end margins
(1e-11 to 1e-10) has a tail through 1e-9, so END = 1e-9 sits inside it. One case is not a threshold case: v8 L1, level
edge (196791, 196811) against fine edge (842306, 842308), t 0.5002, u 0.0328, 5.34 m. I count it; planets' dump
does not have it. Not identified.

A proposal was put to planets: END as a distance along the edge in metres, e.g. 1e-4 m, and likewise a collinear
test away from the roundoff. It is held until the owner sets a direction.

## Sweep (measured)

301×301 directions over each cap. At each level's declared tolerance, the worst difference from level 0 (m), shown
as worst / tolerance. Tolerance 0 gives level 0 exactly (difference 0) in all four regions.

| region | L1 | L2 | L3 | L4 | L5 | L6 | root |
|---|---|---|---|---|---|---|---|
| v8 41014 | 6.1 / 11.0 | 6.9 / 21.3 | 20.4 / 40.7 | 54.3 / 80.1 | 109.2 / 150.1 | 178.4 / 293.2 | 857.5 / 987.3 |
| steep 35607 | 4.9 / 10.8 | 11.9 / 20.3 | 32.1 / 39.3 | 56.1 / 82.4 | 139.6 / 170.7 | 204.2 / 252.8 | 891.9 / 941.3 |
| v6 20079 | 2.8 / 10.3 | 6.3 / 21.3 | 27.3 / 40.7 | 60.4 / 83.4 | 119.4 / 157.8 | 234.1 / 295.2 | 829.5 / 941.3 |
| v6 7717 | 5.0 / 10.4 | 9.7 / 20.4 | 20.9 / 40.9 | 62.2 / 78.2 | 130.5 / 149.6 | 221.8 / 279.8 | 907.5 / 958.6 |

## Observed, not measured

The owner flew `levels.flight` on -c in their own window (21:42 to 21:44, closed at frame 9,600 after the v8 and
steep legs; "not bad"). Planets' first look at that log:
- residency releases happened; the v8 fine file was 0.5 MB resident after the flight left it;
- the steep fine file was read to 189.9 MB during its 2 km hold.

This was not a lab run, and no gate is judged on it.

## Not measured

- Gate 3: the representation served and bytes read per altitude, and level 0's first touch. Judged against the patch
  rule (format-v4.md, planet-unfold 94913ba): above 92 km at most a tenth of the v8 fine file, and level 0 first
  touched below 46 km. The level each patch size uses:

  | patch | tolerance | v8 level |
  |---|---|---|
  | 9.6 km | 9.5 m | level 0 |
  | 19.3 km | 19.0 m | L1 (11.03) |
  | 38.5 km | 37.9 m | L2 (21.3) |
  | 77 km | 75.8 m | L3 (40.7) |
  | 154 km | 152 m | L5 (150.05) |
  | 308 km and 617 km | | L6 |
  | 1,233 km | | root |

- Gate 4 (matched captures, levels vs `UNIVERSE_PGS1_LEVEL0_ONLY`).
- Gate 5 (the contact screen on -c; the rev-4 goldens pass).
- Gate 6 (residency at 9,000 km).
- Frame p95 at 1280×720.
