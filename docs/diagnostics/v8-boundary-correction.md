# v8 boundary correction — bounded CPU and runtime prerequisites

Task `20261013T125016-planets-a178`, isolated `engine-pgs-walk`. No installation, default change, main merge, canonical source edit or physics change.

The optional-cell control keeps the same centroid triangulator and local origin. A detected f32 collapse can trigger one shared 16 m block replacing four optional 8 m cells. Its boundary splits propagate to adjacent cells. Source-face and water constraints remain mandatory; remaining collapse refuses. Reversed traversal must preserve identities, counts and positions within the existing seam tolerance.

Finite retained edges use the lowest-level active owner, with lexicographic equal-level ownership and a common lattice parameter. Corrections validate every overlapping finite interval and final corner dependencies before mutating the supplied group. Missing incident owners inside the footprint and stale generations refuse. Whole-interval separation is bounded by its endpoint differences because both retained edges are affine there.

The saved fixed seven-site meshes have complete outer-segment support bounds below 4.2 micrometres. Each segment has both endpoints within that distance of one corrected retained triangle; convexity bounds the whole segment. This is a directed support bound, supplemented by mesh topology checks, not a substitute for runtime replacement topology. All seven +16 m anchors pass topology and complete outer-segment bounds below 4.5 micrometres; their sampled core errors are below 3.8 micrometres. Working allocations stay below 12.2 MB in these cases, excluding resident source/input data. The earlier approximately 19 MB refusal remains archived.

The 64 m contact core is the only walk-ready candidate. The outer transition remains outside that claim, and the older full-128 m contact gate remains failed. Wet visible geometry uses the water level and is not dry walking contact. High band normal differences remain an appearance limitation pending inspection.

Evidence: `out/review/v8-boundary-correction/{fixed-current,neighbour-current}` and their contact/topology/seam reports; exact bindings are in `v8-boundary-correction.json`. These settled-grid controls do not establish actual resident readiness.

An additional reader accepts exported actual selected meshes, their origins, immutable IDs, eye/morph inputs, source hash and generation. It rejects partial patches and selected ancestor/descendant overlap rather than reconstructing absent neighbors. Such output remains explicitly `runtime_publishable: false` until the full retained replacement, incident-vertex closure and GPU gates pass.

Runtime publication must retain existing terrain while pending or refused. Locking a whole patch at one eye can create new outer seams as the eye moves: lock only crossed vertices and their finite-owner/incident closure, leaving untouched exterior morphs intact. No partial patch replacement is admissible. Actual support snapshots and the atomic game adapter are being prepared by ships. GPU checks remain held behind these prerequisites.

Validation: 69 workspace tests, the explicit missing-owner/no-partial-mutation regression, and registry build pass. GPU validation has not run.
