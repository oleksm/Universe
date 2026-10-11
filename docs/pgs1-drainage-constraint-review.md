# Drainage-unit constraint construction review

Task `20261010T193254-planets-0db1`; inspected the v8 saved script/rejection and
current `../planet-trees/tools/scenery_basin_pilot.py` (identical at review time).
No candidate reconstructed or rendered. The v8 infeasibility is real solver
output, but does not yet establish that a nonconforming mesh is its cause.

## Confirmed numerical diagnostic mismatch

The primary solve requests primal/dual feasibility tolerance 1e-9; the unbounded
adjustment and separate-group diagnoses omit those options. Their default primal
tolerance is 1e-7, equal to `design_gradient`. Thus the claim that each group is
feasible alone is not currently tested at the primary solve's accuracy.

`tools/review/drainage_constraint_examples.py` reproduces an impossible fixed-only
row `0*x <= -1e-7`: default HiGHS returns success with residual +1e-7; at 1e-9 it
reports infeasible. This is a numerical counterexample, not evidence that v8
contains that specific row. The script also tests the logical example below.

Use identical options for all solves, explicitly verify maximum residual and
per-group grade/drop after reconstruction, and distinguish infeasible from a
solver time limit. Inspect zero/near-zero rows after fixed-variable elimination.
Before division, report duplicate or extremely short river intervals; a zero
interval must not be required to descend. The 1e-6 length clamp does not remove
a contradictory zero row. Do not mask a meaningful short interval solely to
obtain feasibility.

## Confirmed stronger-than-connectivity construction

The basin code selects one receiver tree using Dijkstra with travel/height
penalties, then requires strict descent along every selected receiver edge.
Allowing the final steepest-flow audit to use another downhill edge does not
make these LP constraints equivalent to "there exists a route to the outlet."
The LP still requires every chosen tree edge. A different drainage tree could
be compatible even when this one is not. The tree is acyclic by positive cost,
but its union with the river inequalities need not be consistent.

A synthetic triangle makes the issue exact. Let a canonical river run from
vertex A to midpoint(B,C), and choose receiver edges B->A and C->A. In arbitrary
height units, impose:

- `-hA + 0.5*hB + 0.5*hC <= -1` (river descends).
- `hA - hB <= -1` and `hA - hC <= -1` (receiver tree descends).

River and tree groups are each feasible. Adding the river row to half of each
tree row gives `0 <= -2`, a positive infeasibility certificate. This requires
neither a cyclic receiver tree nor an invalid triangle. The unit drops are
invented demonstration values; any positive required drops give the same
contradiction. It is not an identified witness from the actual v8 rows.

## Signs and assembly checked

The river row is `(downstream-upstream)/length`, with negative RHS: correct.
Receiver rows likewise enforce receiver below donor. Exterior-edge rows enforce
outside neighbour above inside, preventing an exit. Fixed heights are included
through `rhs = -gradient - grade@target`; variable deltas use `grade[:,variable]`.
The L1 absolute-value constraints and relaxed L-infinity diagnostic dimensions
are consistent. No simple reversed sign or RHS indexing error was found here.

Boundary rows are stricter than necessary: they require the exterior to be
strictly higher, whereas non-lower prevents downhill exit under the intended
rule. Treat that distinction as a separate diagnostic, not an unreviewed fix.
The closure vertex skips both its receiver and all its exterior-edge rows:
verify its actual chosen receiver remains on the pinned connector. A pinned
connector alone does not prevent another adjacent vertex becoming a steeper
alternative. The final traversal audit helps catch this; the LP alone does not.
Nearest-cell basin membership and continuous river traces are built separately;
record every channel crossing of an inside/outside edge, especially near the
outlet. Those are natural places for river and boundary inequalities to oppose
one another. Also preserve explicit row IDs for pinned connector constraints.

## Smallest useful next experiment

1. Export the assembled sparse grade/A, RHS, target, variable/fixed IDs, interval
   lengths, receiver tree and row provenance. Preserve exact script/control hashes.
   The rejected artifacts currently retain source but not the assembled system.
   Requested this from planets as `20261010T193339-engine-9de9`.
2. Re-run all diagnoses at the same tolerance, check zero rows, and compare
   river+receiver, river+exterior and receiver+exterior subsets. Separate positive
   margin conflict from ordering conflict with a labelled zero-margin diagnostic;
   a flat feasible result is not drainage acceptance.
3. Extract a small infeasible subset or normalized Farkas certificate from the
   fixed-eliminated unbounded system: nonnegative y with A^T y=0, sum(y)=1,
   and b^T y<0. Verify residuals independently at the coefficient scale. Map
   support rows back to actual triangles, channel intervals, receivers, pinned
   vertices and boundary crossings; include the accumulated contradiction margin.
4. If that witness implicates selected receiver edges, try a river-compatible
   receiver assignment or targeted local structural change. If it implicates
   a true divide/channel sampling conflict, then use the witness to justify
   conforming refinement. Halving mesh spacing without this diagnosis cannot
   distinguish numerical trouble from an incompatible chosen tree.

All acceptance gates from `pgs1-drainage-unit-review.md` remain. No height bounds,
water pins, source grades or preview criteria were relaxed by this review.
The small synthetic checks passed under the shared 2-CPU/4-GiB launcher; they do
not establish which rows caused v8's failure. Actual-row diagnosis awaits the dump.

## Analytic rows and conforming footprint follow-up

Review task `20261010T194759-planets-74c8`: analytic along-arc derivative rows
are a sensible way to avoid cancellation from nearly identical endpoint queries.
Use the actual minor-arc tangent, consistent direction and units, and a face
owning the open interval. An edge-coincident arc has the shared-edge derivative;
do not introduce a discontinuity by switching tie owners. Form coefficients
from height differences so their sum is zero by construction; validate constant
height gives zero and compare finite differences away from vertices. Keep the
geometric trace audit: tiny duplicate intervals should not become new rivers.

A single derivative sample proves the derivative sign within one face on a
great-circle segment, but does not automatically prove the requested *minimum
grade* everywhere. The derivative's numerator has constant sign while its
squared projection denominator varies. For a uniform margin, bound that
geometry-dependent denominator over the interval (including interior extrema),
or explicitly certify only sign and separately measure the minimum grade.
Do not silently replace endpoint drop-per-length with a midpoint slope test.

Including all crossed-face vertices can remove the witnessed contradictory
inside/outside edge labels. It also expands the intended catchment: publish the
old/new masks, added area and boundary, preserve the source mapping/provenance,
and call it a conforming candidate footprint rather than unchanged measured
basin extent. A connected channel corridor does not certify the whole catchment;
rerun sinks, extra outlets, native routes, collar and all water/spill gates.
Do not absorb pinned wet/exterior vertices or another outlet just to satisfy rows.
A native prefix may be included only with the declared outlet anchored on that
same directed native arc and a verified downstream connector to known water;
check branching, upstream exits and crossings of the new divide explicitly.
The existing bounds and failure gates stay in force; no visual acceptance follows
from changing the footprint or the row formula alone.
