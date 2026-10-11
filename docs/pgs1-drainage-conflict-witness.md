# Actual drainage constraint witnesses

Tasks `20261010T193956-planets-37cf` and `20261010T194339-planets-9d96`.
No fitting, geometry change or rendering. Sparse dual and diagnostic solves ran
under the shared two-CPU/4-GiB launcher in under a second per invocation.
Row numbers below are zero-based in the delivered NPZ/JSON.

## Coarse: river versus the imposed exterior boundary

Dump `scenery-basin-s15-coarse-diagnostic-v1/constraint_system.npz`, SHA256
`273765a6a3bffe98907718c628e969b5ee3f819bd0764322a705550a6703465f`.
Only three rows are needed:

| Row | Provenance |
| --- | --- |
| 225 | Reference river 45->29, queries 269->270 |
| 233 | Reference river 46->45, queries 278->279 |
| 1388 | Basin exterior: inside vertex 31775, outside vertex 29161 |

All involve only vertices 29161, 31775, 45907. Positive normalized weights
`[0.1657493858597794, 0.35130865033678604, 0.48294196380343457]`
cancel their variable coefficients to <=4.13e-20 while summing RHS to
`-1.00000000175e-7`. With ±150 m adjustments, residual influence is bounded by
`1.06e-17`. Thus the demanded positive grades contradict one another by a clear
margin. This witness does not involve a transferred receiver row or pinned
heights. It is the sampled exterior boundary opposing two consecutive reference
river intervals, not evidence that finer triangles alone are necessary.
A zero-grade solution might be flat; it would not meet the requested descent.

Full coefficients/provenance and two normalizations are retained in
`diagnostics/drainage-coarse-witness.json` and `drainage-coarse-small.json`.

## River-only success is numerically unusable

At 1e-9 solver tolerances, the raw river-only solve reproduces success with
maximum absolute adjustment `1.897161704e9 m` and maximum grade residual
`0.00102985354`, row 454 (river 88->83, queries 541->542, length 701.1098 m).
That row itself is not a zero-length interval. The river matrix has 600 nonzero
coefficients below 1e-9 (minimum 3.96e-16), and interval lengths range from
2.97e-8 to 1926.14 m. These scales and unbounded variables make solver status
unreliable without an original-matrix residual check.

Scaling each row by its largest coefficient alone also returns success, with
adjustments `1.21128e20 m` and residual `624311.9863` (row 622). It is not a fix.
Do not accept either group as feasible. Preserve reasonable diagnostic bounds,
inspect near-duplicate trace points and partition-of-unity cancellation, and
verify original grade/drop residuals. Cleanup needs an explicit geometric/error
budget; silently dropping small coefficients can change an unbounded problem.

## Fine v3: native route versus divide plus receiver

Dump `scenery-basin-s15-1250-v3/constraint_system.npz`, SHA256
`0971f5d6adb5a7471278bd0816e2f215e4f23cb6ecefa3fbcf3a5fe864eaeddc`.
A small **bounded** witness uses:

| Row | Provenance |
| --- | --- |
| 2620 | Basin exterior: inside 41812, outside 24410 |
| 8569 | Transferred receiver 129194->41812 |
| 24387 | Native river 17763->17852, interval 23 |

All involve vertices 24410, 41812, 129194; only the first is outside.
Weights `[0.49972604939701376, 0.00031891314921592124, 0.49995503745377035]`
give RHS `-4.36397594e-7` and coefficient residuals about `2.027e-11` each.
Under the actual ±150 m adjustment bounds their total effect is <=`9.1214e-9`,
well below the negative margin. These three rows therefore already preclude the
bounded candidate: the native route must descend across an imposed rise, with
the receiver preventing the third vertex from compensating. No gate should be
weakened to fit it. Evidence: `diagnostics/drainage-fine-small.json`.

There is a numerical caveat important to the *unbounded* interpretation: native
row 24387 sums to `1.2163e-10`, where a pure height difference should have zero
sum when all participating heights are variable. That small constant-height
sensitivity means the three rows are not an exact unbounded certificate. A
three-row normalized dual has no feasible exact cancellation; do not call this
an exact unbounded proof or attribute it solely to zero-length rows. A larger
full-system numerical dual also finds a negative bound (residual ~1e-16), but
its tiny-weight support and many collar rows are unnecessary for the decisive
bounded result. The small bounded certificate is the evidence to act on.

Next step: inspect those actual boundary/channel intersections and receiver
choices before changing placement. Stabilize query-matrix partition of unity
and duplicate-crossing handling, then reassemble/recheck original residuals;
retain the same native/water/basin gates. The different fine snapping/tree means
coarse versus fine success cannot isolate mesh resolution as the cause.
