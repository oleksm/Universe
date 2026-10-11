# Drainage-unit pilot: engine pre-preview review

Review of planets request `20261010T191221-planets-028b` and
`../planet-trees/docs/requests/2026-10-10-drainage-unit-pilot.md`.
Decision: the bounded cap3 experiment is suitable for local construction with the
gates below. This is design review, not artifact acceptance. No render, build,
installation or main merge is requested. Baseline 006 and conforming control v2
remain immutable. Source analogy and vertical fitting remain modelling hypotheses.

## Canonical field and immutable boundary

- Pin baseline/control hashes, source DEM hash, complete catchment selection rule,
  selected outlet, XY transform/projection, positive vertical scale/offset and
  solver constraints/tolerances before assessing the result visually. Report
  projection distortion: a rigid planar transform is not automatically an
  isometry on the sphere. Reject a fit that leaves its declared support/collar.
- Compare decoded point positions, point ordering, face indices, winding,
  neighbours and ownership against control v2, not directly against 006's
  coarser face IDs. They must remain exact; do not normalize chord points.
  Heights are the only editable geometric field. Publish changed vertices and
  affected faces, and all fixed-vertex sets. Pin every vertex incident to an
  unchanged exterior face, plus every wet-face vertex and shore vertex, exactly.
- Reject nonfinite heights and invalid host radius + height. Independently
  re-evaluate the canonical ray/plane barycentric field after solving and after
  serialization, rather than accepting solver residuals alone. Engine readback
  requires exact owner IDs and <=0.001 m height/level/depth discrepancy. This
  tolerance is interop error allowance, not permission to change pinned values.
- Regenerate candidate goldens inside changed faces. Unchanged exterior answers
  must still match the control; changed heights must match the new independent
  model, not baseline heights. Cover face interiors, crossed edges, vertices and
  full tie fans, collar boundaries and both sides at 0.5/2/50 times the 1e-12
  angular tie tolerance. Shared heights imply continuity, not matching slopes:
  report collar slope/normal jumps and extrema; do not claim C1 continuity.

## Drainage, beyond descending vector endpoints

- Retain the full source catchment and raw DEM grades; filled routing heights
  are not measured terrain. Reject uphill source channels, cycles, inconsistent
  junction elevations and truncated upstream catchments. Record source channel
  identities, junction identities and selection/rejection evidence.
- Audit every transformed channel and the entire connector to first known-water
  entry against canonical triangle crossings, including collinear edges,
  vertex hits and zero-length pieces. Shared junctions must share coordinates
  and height. Comparing endpoints alone misses interior ridges. On a straight
  chord segment within one face the query height is a ratio of affine functions
  with constant-sign derivative; split at every face crossing. If projection
  creates a curved path, that proof no longer suffices: declare the actual path
  representation and use a valid bound or split into the exported segments.
- State the required downhill margin and its numerical uncertainty. A route
  whose drop is swallowed by the solve/readback error is not certified strictly
  downhill. Do not invent a universal physical minimum slope here. Report flats,
  maximum uphill rise, worst residual and relief lost to the constraints; reject
  infeasible solves rather than relaxing fixed boundaries or flattening silently.
- Descending selected lines alone do not establish a basin. Recompute candidate
  catchment connectivity under the declared routing rule: all intended branches
  reach the selected outlet, with no new interior sinks, disconnected tributaries
  or extra boundary exits. Check the entire changed footprint and collar, not
  just source vectors. Edge-graph drainage is only an edge-graph guarantee;
  distinguish it from continuous face-gradient drainage. At minimum explicitly
  report that distinction before calling the candidate a single-outlet basin.
- Recheck native routes intersecting affected faces and the outlet connector,
  including routes with both endpoints outside the footprint. Catalogue existing
  global failures by stable route IDs and pre/post values; no new local failure
  may be hidden behind the existing nine-route global conflict count.

## Fixed water domains require spill checks too

- Body IDs, absolute levels, wet-face assignments, shore topology and wet heights
  remain exact. Check all wet vertices <= level, shore edges at level, and exact
  body/state ownership at wet/dry ties. Water depth stays level minus terrain;
  do not substitute water height for terrain physics. Cap3 categories/features
  remain unknown/absent, with no stale cap35 category provenance claim.
- Recompute minimax spill/connectivity for standing bodies against candidate
  heights. Pinning wet vertices alone does not prevent a lowered dry saddle from
  opening a new escape or joining bodies with different levels. Audit affected
  dry connectivity and any body whose spill path touches it, extending beyond
  the local patch when necessary. With fixed levels/domains, reject newly
  incompatible spills, wet/dry crossings or flooded connectivity; do not silently
  recap lakes, relabel faces, or change water volume to make the export pass.
- Reject new dry ground below ocean level outside the shoreline tolerance band.
  For lakes use connectivity to the body, not a global comparison of all dry
  heights to every lake level: isolated lower dry basins need not be flooded.
  Check the outlet meets the intended body's actual shore at its absolute level;
  trim the above-water route there and distinguish underwater ground grades.
- The Rust reader validates wet vertices and shore edges but does not prove dry
  domain spill consistency or drainage. A successfully loaded PGS file alone is
  not evidence for these gates. Existing depth clamping is not a repair rule.

## Handoff before a preview

Supply a versioned cap3 surface, manifest and source/control hashes, changed/fixed
sets, independent candidate query and derivative fixtures, route/crossing audit,
pre/post water/spill checks and a gate report with explicit failures. Include
solver time/memory, source-versus-fit relief distributions and residuals, signed
volume-change method/units (not erosion mass conservation), and collar slope
measurements. Negative cases should expose hidden ridge crossings, wet-boundary
edits and a newly lowered dry spill saddle, even when selected rivers descend.

Engine will then verify hashes, load the candidate and run independent readback
before any fixed-view neutral comparison. Baseline/candidate must share camera,
sun, host radius, LOD and shadow settings, with the same canonical field used by
physics and drawing; materials/categories cannot be the comparison variable.
This review launches no workload. The proposed first run remains two CPU
equivalents and 4 GiB through the shared scenery launcher/headroom guard;
queue jobs exceeding ten minutes. No resource expansion implied.
