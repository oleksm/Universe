# Cape Fold v1: independent constraint witness

The failure is real in the stored linear system, but the short contradictory
rows come from false endpoint events. This is not evidence that the150m height
bound is too tight or that the chosen source basin cannot fit.

Rows152 and301 describe source tributaries14->20 and32->20 entering the same
outlet from opposing directions. Both use canonical owner184817, vertices
24676/132062/43264; only132062 is adjustable. Positive weights of approximately
one half cancel its coefficient and leave RHS -9.999999995e-8. The checked-in
witness additionally uses exact rational representations of the stored binary
float coefficients: coefficient sum is exactly zero and RHS strictly negative.
Thus the pair is infeasible even with unlimited variable adjustment while the
existing pins remain fixed. No numerical LP status is needed for this proof.

The claimed intervals are0.402 and0.497 micrometres long. Their midpoints fall
inside the1e-12-radian ownership tie band of eight neighbouring faces, which
selects the same lowest-ID face for both opposite approach directions. This
makes the minimum negative gradient constraints contradictory on that face.

Independent extended-precision edge/route intersections put the two incident
edge events at t=1±1.5e-13. The existing trace admits interior events only below
1-1e-12, so these should not be interior breakpoints. The common endpoint
direction agrees with canonical vertex43264 to1.04e-16 in vector norm. Ordinary
float dot/cross cancellation has moved endpoint events into the admitted range,
creating fictitious short intervals. The precision used here is recorded in
`diagnostics/cape-fold-v1/witness.json`; this is an event diagnosis for the two
rows, not a proof that every route or constraint is correct.

Recommended correction, owned by planets: preserve known endpoint vertex
identity and remove incident-edge intersections that are endpoint events.
Do not discard arbitrary short intervals or loosen the downhill, water, native
route or height-bound gates. Preserve genuine near-endpoint crossings when the
endpoint is not that vertex. Regression checks should cover forward/reversed
routes, endpoints at vertices and edge interiors, incident/collinear edges,
and a distinct nearby endpoint with a real interior crossing. Re-run all fit
and canonical downhill/water checks after correcting topology; do not infer
acceptance from eliminating this pair alone.

Reproduce the independent stored-row and endpoint check using
`tools/review/cape_fold_witness.py OUTPUT_JSON` under the shared2CPU/4GiB
launcher. Inputs are the frozen Cape Fold v1 constraint dump and unchanged
1.25km control5029fe82. It reads data and does not edit terrain or constraints.

## Bounded v2 collar witness (independent review)

`tools/review/cape_fold_bounded_witness.py` verifies the seven trace rows against
`scenery-basin-s15-cape-fold-v2/constraint_system.npz`, then uses exact rational
arithmetic on the stored binary floats. Divide each collar row by its positive
receiver coefficient and add the two stated 150 m bounds. Every variable cancels
exactly and the right hand side is **-217.42337923958848 m**. This is a real bounded
contradiction, independent of solver tolerance; no river rows participate.

The path is 54323 → 26877 → 54335 → 32262 → 67514 → 32268 → 62665 → 29751.
Its target rises 517.4225234955111 m, while the old surface descends. The required
positive descent adds about 0.856 mm. This path alone requires at least
258.71168961979424 m symmetric adjustment, exceeding 150 m. This is a lower bound
from this witness, **not** the minimum for the entire system. Exact certificate,
input hashes and row weights are in `diagnostics/cape-fold-v1/bounded-v2-witness.json`.
The checker ran under the shared 2 CPU / 4 GiB launcher; no solve or render ran.

Preserving every old selected receiver edge as downhill is sufficient for local
escape, but is not necessary for a sink-free transition. It does not require the
old receiver to remain the steepest edge; nevertheless choosing those particular
edges excludes otherwise possible routes. The engine drainage review requires
sink, basin, native-channel and water gates, not this particular collar edge set.
The witness proves this chosen routing conflicts with the bounds. It does not
prove an alternative is feasible. Dropping collar rows alone is not acceptance.

A target-aware forest is a reasonable explicitly named experimental policy with
the old-edge policy kept as the default. Build its roots from allowed unchanged
terminals or fixed boundary drainage, forbid cycles and unintended basin exits,
and report terminal/catchment changes. Re-evaluate actual steepest receivers on
the decoded output: a constrained forest proves the existence of descent, not
which receiver the sampler selects. Audit the entire changed footprint and its
collar, including routes into existing dry sinks, rather than only newly created
sink vertices. Retain the 150 m bound, native-route, water/spill/connectivity and
mapped-basin outlet gates unchanged. These are review conditions, not an approval
of a new fit or any assertion of continuous sub-face drainage.
