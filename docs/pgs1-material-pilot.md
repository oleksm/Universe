# PGS material pilot: input audit and bounded implementation

First report for `20261010T174926-planets-2da3`, following the complete
`planet-trees/docs/articles/the-way-to-great-scenery.md` (read 2026-10-10).
The initial input audit is followed by the implemented pilot and validation below.

## Available inputs and reusable code

| Input or mechanism | Current state |
| --- | --- |
| Canonical height | PGS CPU sampling and local meshes agree with physics; no finer relief is implied. |
| Canonical slope | Derivable from canonical height samples with a stated sampling distance. Existing patch field code uses 600 m central differences for legacy worlds, but canonical patches return early with water fields, so their slope is not currently supplied. |
| Rock identity | Frozen `ground-vocabulary.rock@1`, nonzero u16 IDs plus explicit unknown/unassigned semantics; reader agreement passes. |
| Known water | Ocean/lake owner, absolute level and depth are available; the material pilot must leave this rendering and physics unchanged. Unknown water stays unknown. |
| Display controls | Neutral/category modes, manifest-bound vocabulary/legend validation, scoped development settings and an independent shadow switch exist. |
| Rock appearance code | `ground_material.wgsl` has a 20-entry linear-light rock colour table and pixel-filtered colour noise helpers. This table is legacy visual tuning, not measured PGS reflectance. |
| Fixed comparison | `UNIVERSE_ENGINE_THREAD=0`, paused scenario, fixed camera/time and frame number provide repeatable Vulkan captures. |

PGS frozen IDs are **not** the old zero-based `geology.UNITS` indices. ID 0
must never become basalt by default or clamp to another material. Any reuse
needs an explicit mapping by the frozen source names and verified namespace,
not unchecked subtraction/indexing. Diagnostic legend colours are not physical
albedos and must remain a separate comparison mode.

## Missing or unsuitable inputs

`ground_material(GroundIn)` cannot be called unchanged: it assumes ground/biome
colour, sea-level mean temperature, rainfall, river-derived wetness/scree/bare
fields and a legacy rock texture. It also invents woodland mosaics, bed banding,
beaches and snow from those inputs and thresholds. Supplying zero-filled climate
would still invent a climate and snow regime. PGS category 35 supplies none of
those fields. A known lake does not establish soil moisture; elevation alone
does not establish a snow line. Landform patterns on S15 006 are explicitly
unassigned. No features or finer geometry are available for this pilot.

## Bounded first implementation

1. Add explicit development `materials` alongside `neutral` and `categories`;
   label the HUD and evidence **rock appearance pilot — visual tuning**.
2. Factor a small rock-only entry from the existing material shader. Reuse its
   linear rock colour table through a reviewed source-name mapping; bypass the
   climate, vegetation, beach, snow, scree and strata rules entirely. Keep the
   legacy shader unchanged for legacy worlds.
3. Initial proposal: supply canonical slope from CPU height samples at a documented fixed distance,
   independent of render LOD; preserve the existing known-water field. For the
   first pilot use slope only for a clearly labelled visual blend between two
   rock brightness/roughness looks, not a claim of bare-rock coverage or scree.
   No bump, displacement, added relief or noise-derived geometry.
4. Resolve rock IDs on CPU before producing appearance data. Unknown, unassigned,
   unmapped or wrong-namespace categories retain neutral appearance. Interpolated
   display colours are an approximation at category boundaries, never new IDs.
5. Compare neutral/category/material at identical summit and lake-shore cameras,
   time and shadow setting. Check water pixels/levels, height-query agreement,
   unknown fallbacks, WGSL validation and Vulkan timing. Report colour boundary
   resolution and slope sampling limits; do not call this article-wide scenery
   completion or a demonstrated mountain-shadow result.

The rock lookup and slope/brightness response require explicit visual-tuning
labels. Exact shader parameters should be recorded with the pilot images, not
represented as sourced geological or climate measurements. No registry install,
main merge or new physical surface is needed for this bounded experiment.

## Implemented bounded pilot

Task `20261010T175140-planets-6a07` enables
`UNIVERSE_PGS1_COLOURS=materials` in the explicit PGS development preview.
`neutral` remains the default and `categories` remains the diagnostic legend.
A persistent HUD label identifies the material view as **visual tuning**.
No registry installation, branch merge, climate assignment or geometry detail
accompanies this mode. Existing shadows remain an independent opt-in switch.

The 20 legacy `ROCK` entries are reused through explicit frozen source names
(the order was checked against planet-sim's `stages/geology.py`). A test ties the
CPU colour values to `ground_material.wgsl`; no category ID is used as a legacy
array index. Only names in the verified legend and mapping are eligible.
Missing/zero/unassigned/unmapped labels and unsupported rock namespaces remain
neutral. The existing manifest, surface hash and vocabulary checks still apply.
Linear colours are encoded as sRGB bytes for the existing colour cubemap and
hardware-decoded when sampled; byte quantization remains a display limitation.

The rock-only WGSL entry is deliberately small: multiply that colour by
`mix(0.62, 0.8, clamp(slope / 0.5, 0, 1))`. The two multipliers reuse old shader
brightness choices; the blend and threshold are **invented visual tuning**.
This makes no claim about measured albedo, roughness, bare-rock coverage, scree,
strata or soil. It uses no noise, vegetation, snow, rainfall or temperature.
The bound body's global climate remains outside this S15 shader path (ordinary
ship/environment HUD values are not S15 surface measurements).

`Surface::query_differential(direction, radius_m)` differentiates the same
stored chord-plane barycentric height selected by the canonical query. It returns
tangent height gradient in metres/radian, physical normal and rise/run slope.
At boundaries it uses the same lowest-index owner, not an average. The derivative
remains ground-only under water; the existing water query is unchanged. It is
read-only, has no binary-format changes and accepts the actual bound body's
radius; the denominator is `radius + height`, not the header radius silently
applied to another body. Invalid radii are refused.

Local vertices carry analytic owner-face slope. The shader interpolates that
scalar across render triangles; it is not an exact per-pixel derivative at
creases. The 512-square colour cube faces also filter category boundaries and
can miss small units. The orbit view uses the same rock colours with zero slope
in this pilot. These are display/LOD limits, never changes to the canonical IDs
or physics. Known ocean/lake colour and absolute levels take precedence.

### Validation and reproduction

- 330 independent shared slope/normal fixtures pass, including split, shoreline,
  ocean, lake and spill-cap cases. Tests also retain original height/water/category
  query results and verify explicit radius scaling and invalid-radius refusal.
- S15: 185 queries pass; maximum height error `1.40517e-10 m`, slope error
  `2.21351e-15`, normal-component error `6.32827e-15`, within lab gates of 1 mm
  and `1e-10` for slope/normal. The comparison CLI is `pgs_slope_accept`.
- Workspace tests include WGSL validation and material lookup/namespace/fallback
  checks. Registry generation, release build and player compilation pass.
- `tools/review/pgs_material_captures.sh` captures neutral/category/material at
  fixed summit and lake cameras, sun, time, shadow-off setting and frame480,
  with simulation threading disabled and paused. Default output directory:
  `/tmp/pgs-material-pilot`. These are Vulkan comparisons, not calibrated
  physical appearance or a demonstration of mountain shadows.

```sh
~/bin/capped cargo build --release -p universe --bin freefall
bash tools/review/pgs_material_captures.sh
~/bin/capped cargo run -p universe-world --example pgs_slope_accept -- \
 ../planet-trees/out/benchmarks/scenery-slope-s15-v1/s15.pgs \
 ../planet-trees/out/benchmarks/scenery-slope-s15-v1/s15.json
```

Final inspected Vulkan captures and hashes are recorded in
`docs/pgs1-material-pilot-validation.json`. Summit comparisons all have time
7200 s, surface5456.784229 m, sun15.684057 degrees; lake comparisons all have time
33503.4 s, water surface620.613115 m, sun12.521021 degrees. The material summit
is darker brown than neutral/diagnostic views, with the same gentle silhouette;
this is a bounded colour pilot, not realistic mountain scenery completion.
The lake remains blue, with the shore changing appearance. In a 200x200 water-only
rectangle, material vs neutral differs in 8 pixels by at most one byte-channel
step; no large colour change is present. This screenshot check supplements,
not replaces, canonical water-query tests.

Measured frame averages (neutral/category/material) are 24.54/24.26/23.56 ms at
the summit and 30.41/29.99/30.13 ms at the lake. These single 480-frame runs
include warmup and asynchronous terrain work: they are not evidence of a
speedup or a controlled overhead benchmark. GPU scene timings are in the JSON.
