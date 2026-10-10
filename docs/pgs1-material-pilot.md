# PGS material pilot: input audit and bounded implementation

First report for `20261010T174926-planets-2da3`, following the complete
`planet-trees/docs/articles/the-way-to-great-scenery.md` (read 2026-10-10).
This is the implementation boundary, not a claim that materials are delivered.

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
3. Supply canonical slope from CPU height samples at a documented fixed distance,
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
