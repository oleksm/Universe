# Contact-band representation witnesses

This CPU diagnostic attributes the refusals from `7888737b`; it does not enable the overlay. The corrected version-2 plan preserves all caps, the 64 m contact core, 128 m footprint and unchanged source/physics. It replaces the old plan's stale dense-overlay/full-footprint wording in a new file, leaving the frozen original untouched. Evidence is under `out/review/v8-representation`, bound by `v8-representation.json`.

## Parent: mandatory source boundary versus optional sampling

The parent witness is source face 40805, whose shared edge with 41014 passes about **2.000 nanometres** from the chart origin. Its intersections with x=0 and y=0 are y=2.066 nm and x=-7.974 nm. The previous 1e-8 m rounded cell coordinates produce the saved (-1e-8,0)/(0,0) sliver. The exact saved f32 coincidence reproduces. Both faces are dry; this is not permission to delete a wet/dry boundary.

A tiny control coalesces the four optional 8 m sampling cells in the local 16 m square. It keeps both source faces and their mandatory common edge; no source height or edge moves. Across reversed traversal and chart-coordinate translations of 0, 0.125 and 128 m, the resulting eight triangles have no f32 collapse and area 256 m². The source edge uses one identity from the same two original source vertices in both incident faces, independent of traversal.

This is a **candidate subdivision policy**, not an implemented shared-boundary mesh correction. The local control also changes triangulation to centroid fans: its optional-lines-on controls do not reproduce the old triangulator's collapse. Consequently it does not isolate grid removal as the sole sufficient change. Full shared polygon construction, attachment to neighbouring unmodified cells, mandatory boundary representability and field/interior-error checks remain required. A separate distinct-mandatory-constraint fixture retains two distinct coordinates that collapse in f32 and explicitly requires refusal; it must not be resolved by optional-grid coalescing.

## Retained edges: different finite approximations

Both conflicts pair a level-18 edge with one of its level-19 half-edges. The infinite grid-line label groups them but does not establish physical equality.

| Witness | Coarse patch (face,level,x,y) | Fine patch | Fine midpoint gap after morph |
|---|---|---|---:|
| Crest 1 | 3,18,242300,86017 | 3,19,484601,172033 | 1.490702 mm |
| Crest 2 | 3,18,242201,86144 | 3,19,484401,172288 | 0.235673 mm |

Crest 1 intervals are [7753624,7753626] and [7753625,7753626]; crest 2 intervals are [2756614,2756616] and [2756615,2756616]. The common even endpoint differs by 0.743/1.044 micrometres due to separate patch representations. At the odd fine endpoint, unmorphed differences are 6.697142/0.636660 mm. Fine morph factors are approximately .777/.750 and .629/.661; coarse factors are zero. The fine edge has not fully collapsed to the coarse edge. Boundary-plane intersections therefore disagree by the previously measured .720/.141 mm.

Each witness retains both origins, finite endpoint IDs/intervals, raw positions, parent targets, morph factors, physical triangles, intersection fractions and points. The mesh parity targets and diagonal agree with `terrain_lod.rs::make`; the shader uses `smoothstep` on camera-relative distance. A shader-like f32 reconstruction using the f32 patch size changes these factors slightly but changes **none of these final endpoint coordinates**. Thus that rounding difference does not explain away the recorded gaps.

This establishes an incompatible coarse/fine **top-surface reconstruction** at the frozen selection, not a clipping-predicate error. The current terrain renderer has skirts, which these contact-surface fixtures exclude. Actual GPU transforms/residency have not been replayed; do not present this as a measured visible GPU crack. Choosing either height as a common intersection would change the other surface, so the overlay still refuses. Runtime-compatible boundary ownership or a coordinated retained-grid change needs a separate implementation gate.

## Strict support: actual stored-value gaps

All 26 misses are endpoint samples, representing **13 unique points**, each seen from two output edges. Their two nearby supporting triangles lie in separate patches (mostly equal levels; two coarse/fine pairs). Independent exact rational determinant signs agree with an 80-digit calculation using exact conversions of the stored binary64 values and homogeneous 3D determinants, avoiding the rounded chart projection. Every point remains outside both saved support cones. None becomes a pass through a more accurate orientation predicate.

Nearest finite 3D edge distances range from **0.031 to 1.223 micrometres**. The report also records distance to each supporting plane. This is physical stored-edge separation, not just similar radial heights; the points are not exactly covered by the nearby top triangles. The check is local to the extracted candidates, not a search of every triangle on the globe. No tolerance increase, weld or exact-seam acceptance follows.

## Normals and scope

The saved-mesh audit now separates matching-source-owner triangle centroids, matching-owner edge/vertex samples, and different query-owner samples. It retains all errors. The largest crest-3 band error remains **78.079 degrees even at same-owner centroids**, so it cannot all be dismissed as a canonical tie-owner discontinuity. Matching-owner centroid maxima at shore, sampling join and relief/native join are .321/.710/1.934 degrees. In the crest-3 core, the 34.476-degree maximum belongs to different query owners; same-owner centroid maximum is 0.045831 degrees and same-owner boundary maximum remains 4.521155 degrees. Thus the large core tie effect is separated without hiding the remaining boundary error. Triangle centroids are an interior sample classification, not a lower bound on distance to a source edge.

No seven-site construction rerun, GPU job, runtime publication, terrain/source change, installation or main merge was performed. The extra provenance is diagnostic allocation; transient worker allocation and GPU lifetime accounting remain unverified. Prior memory/readiness gates remain held. Next work should implement shared optional-subdivision policy and explicit finite-edge ownership, then repeat the small regressions before expanding to the full CPU screen.

Validation: 68 workspace tests pass; registry build exits 0; diagnostic scripts and stored-output replay checks pass.
