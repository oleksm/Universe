# Steadier land LOD with the branch-four pyramid

Prefer a view's `pyramid4/pyramid.json` when present. Read its branch factor for level
spacing, parent links, child traversal and raster sampling. The new 192-cell tiles
have 193 shared samples and sixteen children; older pyramids default to branch two.
Validate divisible tile dimensions rather than requiring powers of two.

Morph vertices to the parent's bilinear surface over the last half of the geometric
range. Keep the parent unmorphed across both hierarchy boundaries. Remember split
state across frames and view changes: split above the existing error threshold and
merge only below 0.7 of it. Hole boundaries retain their fine geometry and skirts.

Tests cover branch-two compatibility, branch-four parent targets, the sixteen-child
resident cover, invalid dimensions and split/merge hysteresis. Full-package selection,
streaming and native flight evidence: docs/diagnostics/planetlod-branch4-20261021.json.
The pixel threshold remains 0.0007. Branch-four data has larger orbital working sets;
the cache target remains soft when selected geometry itself exceeds 256 MiB.

The orbital capture also revealed diagonal climate-colour bands already in the producer
fine RGB raster. An independent raster preview confirms their source; producer repair
was requested separately. This patch does not alter the view data.
