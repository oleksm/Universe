# PGS1 water development preview

The explicit `UNIVERSE_PGS1` planet/lowflight preview accepts capability 3 as
well as terrain-only capability 1. Known ocean/lake domains draw in diagnostic
blue at their exported absolute levels. The globe map and local patches sample
the same canonical source as physics; below-datum unknown terrain stays gray.
Categories remain unknown, with no procedural relief/materials or feature export.

Shorelines are sampled through the existing cube map/mesh LOD, not rendered from
exact canonical face edges. Small domains can disappear at coarse resolutions.
No installation, registry change or main-branch merge is part of this preview.

Validation: shared-fixture renderer regression, full workspace tests including
shader validation, non-dev check, registry generation and release build pass.
Inspected S15 Vulkan orbit and daylight elevated-lake captures.
