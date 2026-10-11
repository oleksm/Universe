# Cape Fold v4: independent engine review

Development-only candidate PGS SHA256
`f3df5e5920dc69fa0742dfaa5c002e07456309c96fd38db77d580afba8cd4899`.
No installation, physics changes, registry edits or main merge.

The existing Rust cross-reader passes all 198 owner/height/water fixtures:
maximum height discrepancy 2.774e-10 m. Analytic derivatives pass 198 checks:
height 1.383e-10 m, slope 1.111e-16, normal component 2.221e-16.
Capability 3 retains unknown categories. The review-v1 input manifest hashes
were checked; evidence and binary hashes are in `diagnostics/cape-fold-v4`.
The local capture package copies the unchanged PGS and fixture bytes into the
manifest shape required by the reader; it does not rewrite terrain.

Independent traversal of the review-v2 receiver arrays reproduces all terminal
IDs: all 21,142 affected vertices reach wet vertices on both surfaces, with
1,037 changes in first wet vertex and no dry termination. This verifies the
supplied graph traversal, not reconstruction of steepest slopes or continuous
water-domain intersection. Planets separately reports canonical terminal queries
classify all of these as ocean 0, including changed termini 22839 and 23000.
Do not substitute forest-root eligibility for these decoded route checks.

## Frozen neutral Vulkan comparison

`tools/review/cape_fold_captures.py` runs the existing release binary sequentially
under the shared 2 CPU / 8 GiB launcher. Both use host radius 6,281,370 m (source
radius 6,371,000 m), heading 90 degrees, zero speed, paused time, neutral colours,
shadows off, no geometry or line overlays, and capture frame 480 at 1920x1080.
An empty hash-bound debug sidecar enables absolute-eye and LOD logging; its
on-screen lift legend does not mean any debug geometry is drawn.

Summit: latitude -50.927589054482716, longitude -19.166702846855568, time 7200 s,
down 0.25. Initial equal relative launch altitudes produced eyes differing by
1309.127425 m. The final control launch is compensated upward; final absolute
eyes are 7765.607267 m control and 7765.607268 m candidate. This preserves at
least the initial nominal 1000 m launch clearance on both surfaces. HUD ground
clearance differs because the surfaces differ; that is not a camera mismatch.

Lake: latitude 59.01000627043136, longitude -103.1714991841128, time 33503.4 s,
down 0.6. Initial eyes already match at 10620.613114 m, with nominal 10000 m
launch clearance; final repetitions use the same settings.

The inspected summit pair shows visible but faint additional foreground ridges
and troughs, with a similarly flat distant silhouette. This fixed neutral view
supports visible geometric change, not mountain-scenery visual acceptance.
No sun, material or relief tuning was used. Raw captures, exact launch settings,
PNG hashes and logs are under `out/review/cape-fold-v4`; these include clearly
labelled initial calibration captures, which are not matched comparisons.

All four final Vulkan runs completed and screenshots were inspected. The lake
pair retains the blue water and matching shore placement; no obvious local
change is visible. Final lake eyes match exactly at logged precision. All runs
have zero pending LOD work at capture, but resident tile counts differ slightly;
this is not a forced-identical render tessellation comparison. Mean frame times
(control/candidate) were summit 34.64/39.98 ms and lake 47.69/44.73 ms. These
single warmup-inclusive runs under shared load do not establish a performance
regression. Exact final tile counts and timings are in
`diagnostics/cape-fold-v4/runtime.json`.
