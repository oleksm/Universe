# PGS1 engine acceptance

2026-10-10. The engine reads the canonical root subset of PGS1 version 1,
revision 3 (`planet-unfold` contract commit `42bde8a`). This is development
acceptance, not a registry installation.

`crates/core/world/src/worlds/pgs.rs` owns decoding and queries. It checks the
header, provenance JSON/source digest, reserved bytes, section ranges, alignment,
strides, counts and SHA-256 hashes. Unknown optional sections are checked and
skipped; unknown required sections and unsupported capabilities are refused.
Canonical points, faces and optional water bodies are supported. Children,
compact points, overlays, anchors, categories and the serialized cube index are
not supported yet.

Roots must be connected, closed, outward-facing spherical meshes with reciprocal
edge neighbours. Wet faces must name an existing body, stay below its level and
meet dry faces at that level. Different bodies cannot meet at a vertex or edge.
Stored positions remain unchanged, including points inserted on a chord plane.
The query projects onto that plane and blends heights in f64. A runtime centroid
KD tree seeds a neighbour walk; a bounded walk has an exhaustive fallback.
Boundary traversal visits the whole tied fan and selects the lowest face index.
Water queries examine all tied faces, including when the terrain owner is dry.

`Terrain::from_surface` attaches the result for explicit development use.
`Terrain::surface_sample` retains unknown water and categories; scalar surface
queries use a water level only when a domain is present. The same field serves
physics, coarse and local drawing queries. No previous bake, contour field,
procedural relief or port flattening is mixed into it. `classify` uses a neutral
lowland drawing fallback for unknown categories; it is not categorical data.
The legacy `is_ocean` boolean only returns true for known water on this path;
callers needing unknown versus dry must use `surface_sample`.

## Reproduce acceptance

The small lab goldens are vendored verbatim under
`crates/core/world/tests/fixtures/pgs1-v1/`, including their manifest and reference
queries. Tests check all 123 queries, exact owner IDs, heights within 1 mm, shore
semantics, 2,000 analytical split-invariance directions, malformed files, unknown
optional/required sections and the shared Terrain query paths.

```sh
cargo test -p universe-world --test pgs
cargo test -q --workspace
cargo run -p universe-world --release --example pgs_accept -- \
  ../planet-trees/out/benchmarks/s13-pgs1-v1 /tmp/pgs1-engine-s13.png
cargo run -p universe-world --release --example pgs_accept -- \
  ../planet-trees/out/planet/earth_s15/surface_model_004 /tmp/pgs1-engine-s15.png
```

The acceptance command checks surface and fixture hashes and byte counts against
the delivered manifest, then checks the fixture's surface hash, face IDs, heights
and water state. It reports load time and two fixed 10,000-query batches: coherent
nearby directions and directions distributed over the sphere. These include
water evaluation. Local timings are measurements, not runtime guarantees.

The optional PNG is a 512×256 equirectangular grayscale height preview generated
through `Terrain::surface`. Its metadata records the height range. Dark pixels
mean lower elevations, not water. This is not a flight/rendering acceptance test.

## Measured acceptance, 2026-10-10

The full workspace tests passed, including eight PGS1 integration tests.
Both external world exports passed all 27 reference queries with exact face IDs,
unknown water and unknown categories. Release-build results on this machine:

| Export | Bytes | Maximum height error (m) | Load (ms) | Coherent query (µs) | Distributed query (µs) |
|---|---:|---:|---:|---:|---:|
| S13 | 2,080,336 | 9.095e-12 | 19.11 | 0.139 | 0.310 |
| S15 | 2,080,344 | 7.276e-12 | 17.99 | 0.134 | 0.295 |

Each contains 20,000 points and 39,996 faces. The measured inputs were:

- S13: `4e171a2cbf217b0ab0b4471c58ce7528999b0429a7e05e0b3fa0d941b8ccfca6`
- S15: `995c63bb6b71eee4d88aa9c76eb94a162e80224d12f3e2d6d34f6376c2d4ae36`

## In-game development preview

The dev build accepts `UNIVERSE_PGS1`, a lab export directory, with an explicit
`UNIVERSE_BODY` and either the `planet` or `lowflight` scenario. It checks the
manifest format, surface hash/size, file validity and any existing body binding.
This preview currently requires **terrain-only** capability 1. Other capabilities
are rejected instead of being drawn incorrectly.

```sh
cargo build --release -p universe --bin freefall
UNIVERSE_PGS1=../planet-trees/out/planet/earth_s15/surface_model_004 \
UNIVERSE_BODY=body.treistun.treistun-e UNIVERSE_SCENARIO=planet \
UNIVERSE_SETTLERS=0 UNIVERSE_PAUSED=1 target/release/freefall

UNIVERSE_PGS1=../planet-trees/out/planet/earth_s15/surface_model_004 \
UNIVERSE_BODY=body.treistun.treistun-e UNIVERSE_SCENARIO=lowflight \
UNIVERSE_ALT=6000 UNIVERSE_SPEED=200 UNIVERSE_SETTLERS=0 \
target/release/freefall
```

The host supplies an immutable source map to the world before simulation setup.
Physics and independently generated client/NPC charts receive that same map;
revisiting a system regenerates it from the same source. Heights remain in metres,
on the selected body's existing radius, rotation and gravity. The map is scoped
to this world instance; neither registry records nor global terrain state change.
A contact test checks the physics surface against chart/render queries, including
negative terrain, and contacts 5 cm above/below the expected contact boundary.

The globe shader's canonical palette is grayscale by measured elevation (clamped
to ± the terrain relief scale). It preserves negative
heights and suppresses inferred oceans, materials, procedural grain and relief.
Local patches shade their actual mesh slopes. Old contour overlays and bake maps
are not used. Brightness is a diagnostic elevation tint, not a material or water classification.
Atmosphere still comes from the selected body's record. The existing mesh LOD
still approximates the canonical field between mesh vertices.

Preview saves/loads and seed-only recordings are refused: their formats do not
carry this external source yet. Player builds without the dev feature reject the
preview option. The standalone studio and normal game startup remain available.

## Preview validation, 2026-10-10

The full workspace tests, shader validation, release game build, player-build
check (`cargo check -p universe --no-default-features`) and registry build passed.
The registry build still reports its existing open report gaps; no registry files
changed. S15 ran on Vulkan in orbital and lowflight scenarios, including an
unpaused 240-frame flight above below-datum terrain. Screenshots were inspected
for terrain visibility, inferred water/materials and old overlays; no GPU
validation errors were reported. These are short smoke runs, not a flight soak.

Reproduce the inspected daytime orbital view by adding `UNIVERSE_YAW=3.74`
and `UNIVERSE_DIST=2.5` to the orbital command. The below-datum flight used
`UNIVERSE_LAT=-46 UNIVERSE_LON=178 UNIVERSE_ALT=3000 UNIVERSE_HEADING=0
UNIVERSE_DOWN=0.1 UNIVERSE_SPEED=200 UNIVERSE_TIME_SCALE=1`.
For an automatic capture and exit, set `UNIVERSE_SCREENSHOT=/tmp/pgs1.png`
and `UNIVERSE_SCREENSHOT_AT=240`.

The root's relief is broad and shallow at this scale; the preview does not add
fine landscape that the export does not contain.

## Production installation remains separate

S13/S15 currently carry terrain only and no registry body binding. Production
installation needs the registry ground format, installer and game loading path
to agree on a PGS1 package, plus rendering for its real water/material capabilities.
The current preview deliberately cannot install a world. Existing registered PTL2
worlds remain the normal game's source.
