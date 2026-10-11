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
