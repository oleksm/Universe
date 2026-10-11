# PGS scenery diagnostics (development only)

Explicitly enabled overlays attach to the same body and immutable PGS used by
physics. Default rendering, terrain, water and materials are unchanged. The
sidecar is `scenery-debug/1` (discriminator `version`, also accepts `format`),
with surface SHA256, source radius and layers of labelled RGB0..1 colours,
unit directions/canonical terrain heights, and point-index segments. Optional
markers use the same point representation. Supplemental provenance/normal
samples are retained by the producer; runtime normals are evaluated independently.

The reader checks surface hash, source radius, finite/unit directions, unique
layer IDs, RGB range, segment indices and every height against the consumer
(1mm height tolerance). It refuses more than200000 segments. Source radius
validates the package; **display positions use the actual host body's radius**,
just like the terrain renderer. S15 source radius6371000m and TreistunE host
radius6281370m differ. Applying the source radius to overlays would put them
89630m above the terrain. Angular directions and canonical heights are unchanged;
this fix does not rescale terrain or change physics.

Controls (environment, before launch):

- `UNIVERSE_PGS1_DEBUG=/absolute/path/scenery-debug.json` enables sidecar lines
  and markers. Omit to leave sidecar overlays off.
- `UNIVERSE_PGS1_DEBUG_LAYERS=changed_footprint,mapped_basin,reference_rivers`
  selects exact layer IDs; otherwise all layers. Unknown IDs fail explicitly.
- `UNIVERSE_PGS1_GEOMETRY=off|wire|normals|both` independently selects the
  actual resident rendered triangles and normal/slope glyphs. Default `off`.
- `UNIVERSE_PGS1_DEBUG_LIFT=2` is a radial **display-only** lift in metres
  (default2, allowed0–100). It neither edits source heights nor certifies clearance.

Rendered triangles are magenta; the portable `canonical_mesh` layer supplies
canonical edges in its own labelled colour. The renderer reconstructs its vertex
geomorph before drawing diagnostics; skirts are excluded. Cyan glyphs are
canonical terrain normals, magenta glyphs rendered visible-surface normals.
On water those intentionally compare the underlying terrain with the displayed
water surface. Both glyphs are100m long for visibility; a tangent glyph's length
and green-to-red colour encode canonical rise/run0–1 (capped at1). Markers have
100m stems and20m rings. These are visual scales, not landforms or physical sizes.

Lines/glyphs are depth-tested and limited to30km from the eye. A line below a
coarser resampled triangle can be occluded; increase the explicitly labelled
lift when inspecting such differences. Dense full wireframes alias at distance;
use separate line-only/geometry-only views. No inferred ridge crests are added.
Every120 terrain frames, debug logs report actual drawn LOD levels, outstanding
patch jobs and eye height. The debug legend replaces the redundant initial
centre toast; the permanent PGS/water/category caption remains.

Example frozen summit on the reviewed candidate:

```sh
UNIVERSE_PGS1=/home/alexm/git/planet-trees/out/planet/earth_s15/basin_review_001 \
UNIVERSE_PGS1_DEBUG=/home/alexm/git/planet-trees/out/planet/earth_s15/debug_review_002/scenery-debug.json \
UNIVERSE_PGS1_GEOMETRY=both UNIVERSE_PGS1_COLOURS=neutral \
UNIVERSE_PGS1_SHADOWS=off UNIVERSE_BODY=body.treistun.treistun-e \
UNIVERSE_SCENARIO=lowflight UNIVERSE_ENGINE_THREAD=0 \
UNIVERSE_LAT=-50.927589054482716 UNIVERSE_LON=-19.166702846855568 \
UNIVERSE_HOURS=2 UNIVERSE_ALT=710.2063249977064 \
UNIVERSE_HEADING=90 UNIVERSE_DOWN=0.25 UNIVERSE_SPEED=0 \
UNIVERSE_SETTLERS=0 UNIVERSE_PAUSED=1 \
/home/alexm/git/planet-trees/.venv/bin/python \
/home/alexm/git/planet-trees/tools/scenery_job.py --memory-gib 8 --threads 2 -- \
target/debug/freefall
```

Build with the ordinary game build, or the explicit fso preview build documented
in `mounted-equipment-preview.md`. For a neutral comparison remove DEBUG and
GEOMETRY; preserve package, absolute camera position/time and all other settings.
For line-only views set GEOMETRY=off and omit canonical_mesh from the layer list.
For geometry-only omit DEBUG and keep GEOMETRY=both. Screenshots and logs:
`out/review/pgs-debug-v1/{wire,lines,geometry,neutral}.{png,log}`. Hashes, actual
residency and validation results are recorded with the detail audit. No registry
installation or branch merge is part of this preview.

Validation:131 workspace tests pass, repository registry build passes, and the
non-development configuration compiles. Final four Vulkan captures (RTX5090,
shared2CPU/8GiB,240frames each) were inspected; the native river/source-mask lines,
coarse canonical edges, fine rendered grid and normal glyphs are visible, with
no centre-caption collision. Runtime residency is unchanged at120/240 with no
pending jobs. Whole-run averages include startup and are not steady-state
benchmarks; recorded GPU scene passes are approximately0.22–0.29ms here.
See `diagnostics/scenery-detail-v1/runtime-validation.json` for hashes and timings.
