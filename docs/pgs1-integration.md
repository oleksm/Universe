# PGS1 engine acceptance

2026-10-10. The engine reads the canonical root subset of PGS1 version 1,
revision 3 (`planet-unfold` contract commit `42bde8a`). This is development
acceptance, not a registry installation.

`crates/core/world/src/worlds/pgs.rs` owns decoding and queries. It checks the
header, provenance JSON/source digest, reserved bytes, section ranges, alignment,
strides, counts and SHA-256 hashes. Unknown optional sections are checked and
skipped; unknown required sections and unsupported capabilities are refused.
Canonical points, faces, optional water bodies and owner-face category IDs are
supported. Children, compact points, overlays, anchors and the serialized cube
index are not supported yet. Category IDs remain qualified by the two header
vocabularies; the reader does not turn them into gameplay materials.

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
The preview accepts capability 1 (terrain, unknown water), 3 (terrain plus water),
33 (terrain plus categories) and 35 (terrain/water/categories). Other capabilities
remain rejected. Categories are retained for queries; the current preview draws
gray terrain and blue known water, without category colours or material textures.

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
Capability 3 adds diagnostic blue for known ocean/lake domains, at their exported
absolute levels. Categories remain unknown. Local patches shade their actual mesh slopes. Old contour overlays and bake maps
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

## Cross-reader recheck, 2026-10-10 16:57

For planets task `20261010T165643-planets-8d30`, rechecked the current lab
`tests/fixtures/pgs1-v1/` against the engine copies. All six binary/query files
and the manifest are byte-identical; all manifest hashes and byte counts pass.
The eight PGS integration tests pass, including 123 analytical golden queries,
2,000 split-invariance directions, unknown-water semantics and shared contacts.

Rebuilt the acceptance example from current engine source with
`~/bin/capped cargo build -p universe-world --example pgs_accept`, then ran
`target/debug/examples/pgs_accept` on both directories listed above. Each passed
all 27 world queries with exact face IDs and unknown water. Maximum height error:
S13 `9.094947017729282e-12 m`; S15 `7.275957614183426e-12 m`. Both whole-file hashes
remain those recorded above; manifest and section integrity checks passed.
No mismatches or reader changes. Real water-domain/shoreline exports remain a
separate forthcoming acceptance input; the terrain-only baseline is preserved.

## Water-domain agreement, 2026-10-10

For planets task `20261010T170628-planets-89d9`, the current reader passed all
**381 new queries** without decoder/query changes. Vendored the three small water
fixtures, query files and manifest verbatim from
`planet-trees/tests/fixtures/pgs1-water-v1/` into the engine tests. The new
`independent_water_domain_goldens` regression checks both binary and query-file
hashes/byte counts, exact face owners, unknown categories, water state and body
IDs, and terrain height/water level/depth within 0.001 m. All nine PGS integration
tests pass, preserving the earlier terrain-only and split-invariance checks.

| Input | Queries passed | Maximum terrain-height error (m) |
|---|---:|---:|
| ocean_cut | 61 | 3.498e-13 |
| lake_cut | 85 | 2.843e-13 |
| lake_spill_cap | 61 | 2.307e-13 |
| S15 surface_model_005 | 174 | 1.410e-10 |

The small-case error measurements used unchanged binary/JSON bytes through the
acceptance CLI, with temporary surface/query filenames and corresponding manifest
entries. Their checked-in original manifest remains unchanged.

S15 is 2,398,728 bytes, capability 3, 23,048 points and 46,092 faces. The acceptance
CLI verifies manifest entries for the surface and queries, internal section
hashes, and all query expectations, including known standing water. Its surface
SHA-256 is `c20fde3bd5662e53b86b8106c4a5938a15858f742351120c4bc3ae7198cabc79`.
Reproduce after building the example:

```sh
cargo test -p universe-world --test pgs
~/bin/capped cargo run -p universe-world --example pgs_accept -- \
  ../planet-trees/out/planet/earth_s15/surface_model_005
```

This agrees with the exported water interpretation; it does not certify the
spill-cap hydrology or independently reconstruct the lake assignment. Planets'
reported S13 multi-level conflict remains unresolved, and no S13 water package
was accepted. Shore-chain sidecars are diagnostic; feature sections remain absent.
This CPU water-reader pass preceded the development rendering extension below;
it does not authorize installation.

## Water development rendering, 2026-10-10

Capability 3 is now accepted by the explicit preview. `canonical_texel` supplies
absolute visible height (the exported level for wet samples, terrain height
otherwise) plus a known-water mask. Palette 4 draws that mask blue; palette 3
preserves unknown water. Both keep categories unknown and disable synthetic
terrain/material detail. Nearby LOD patches sample the mask directly from PGS1
rather than inheriting the 512-square orbit map's resolution. Their geometry
continues to use the same `Terrain::surface_view_to` source as before; the
physics/contact scalar surface also uses the known level, while `surface_sample`
and raw-height queries retain the underlying terrain. No buoyancy, underwater
physics, rivers or categorical materials are introduced.

Blue is an invented diagnostic display colour, not a physical water material.
The filtered mask and existing mesh LOD approximate shores between samples;
small domains can disappear at coarse resolution. This is not an exact
face-edge shoreline renderer or a millimetre geometry claim. The CPU agreement
and its exact ownership rules are unchanged.

Reproduce S15 water orbit and elevated-lake views with the prior screenshot
controls and `surface_model_005`. Lake body 1504 has absolute level
620.6131151627992 m; one golden query inside it is at engine latitude
59.01000627043136, longitude -103.1714991841128 (engine longitude is atan2(-z,x)).
Use `UNIVERSE_SCENARIO=lowflight UNIVERSE_LAT=59.01000627043136
UNIVERSE_LON=-103.1714991841128 UNIVERSE_HOURS=9.3065
UNIVERSE_ALT=10000 UNIVERSE_DOWN=0.6` to inspect it.
All prior save/load/replay and non-dev-build restrictions still apply.

Validation: full `cargo test -q --workspace` (including WGSL validation and the
new renderer sample regression), `cargo check -p universe --no-default-features`,
registry generation and the release game build pass. Inspected Vulkan captures
`/tmp/pgs-water-orbit.png` (240 frames) and `/tmp/pgs-water-lake-day.png`
(480 frames): known water blue, neighboring uncategorized terrain gray, visible
sampled shore. The daylight lake capture uses zero speed, paused, 9.3065 hours.
This visual check does not establish shoreline accuracy between mesh samples.

## Category reader agreement, 2026-10-10

Planets task `20261010T172314-planets-97c6`: the reader now accepts bit 5 and returns
`Some((rock, pattern))` from the selected terrain owner face. Water's boundary
ownership remains independently evaluated. Without bit 5, stored label bytes are
ignored and categories remain `None`; ID 0 with bit 5 means explicitly unassigned,
not unknown and not basalt. Versioned vocabulary identifiers are required when
categories are present. IDs are opaque values qualified by the header names;
this reader does not resolve names, validate membership in an external vocabulary,
reintegrate source Voronoi support or assign materials/gameplay properties.
`Terrain::category_vocabularies` exposes those names alongside `surface_sample`.

Vendored the three shared category fixtures unchanged, with their manifest.
All **399 shared queries** pass, including nonzero pattern IDs and edge/vertex
ownership. Additional regressions cover explicit `(0,0)`, absent capabilities
with nonzero stored labels, and invalid/missing-version namespace identifiers.
The prior terrain-only and water fixtures remain passing.

The acceptance CLI now compares category IDs/unknown semantics as well as
terrain and water. S15 `surface_model_006` passes **185 world queries**, with
exact face owners, category IDs and water body IDs; maximum height error is
`1.4097167877480388e-10 m`. SHA-256:
`444d983b703e6a2c9629c41a75be720ef5c15f16bb4616b9d3989e9b0500f616`.
The 2,399,408-byte file contains 23,048 points and 46,092 faces, capability 35.
Independent byte comparison against 005 confirms identical point/water payloads
and identical vertex/neighbour/body fields on every face. S15 patterns are all
explicitly unassigned, per its declared source policy.

Both export vocabulary snapshots match the registry YAML byte-for-byte and match
their export manifest entries:

- `ground-vocabulary.rock@1`: `7d3835cbcfa04d13deceb8038d6a2757e8fcafd51aabbd87e92e6d45fe777c9b`.
- `ground-vocabulary.pattern@1`: `011b65eb9982c5593614fe30fae26a0e124ec4de7136bfa708fa487f0b4896e9`.

Use the existing explicit `UNIVERSE_PGS1` commands with `surface_model_006`.
This enables terrain/water preview of the category-bearing package, preserving
labels in canonical queries; category colour/texturing is not implemented. No
registry installation, feature/river export or main-branch merge is involved.

Validation passes: full workspace tests, the final 11-test PGS suite including
namespace propagation through Terrain, non-dev compilation and release build.
The S15 cap35 Vulkan orbit capture `/tmp/pgs-categories-orbit.png` loads correctly
and preserves the neutral terrain/known-water presentation. Earlier registry
generation passed unchanged; this work does not modify registry data.
