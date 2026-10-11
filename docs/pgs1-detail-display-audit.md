# Canonical detail and the development renderer

Read-only audit of candidate `b84f9ef3` against unchanged 1.25 km control
`5029fe82`, at the frozen summit (-50.927589054482716, -19.166702846855568),
heading90°, down0.25, time7200s. No terrain, climate, material or geometry tuning.
The host radius is6281370m; source radius6371000m is provenance, not the
preview body size. Earlier source-radius estimates were superseded by this host-radius run.
Results and reproducible camera settings: `diagnostics/scenery-detail-v1/`.

The local renderer samples canonical visible heights (explicit water level where
wet) into 16×16 cube-sphere patches. It does not render the PGS face topology.
The material slope can use the analytic canonical derivative; PGS lighting on
patches instead uses the **rendered, geomorphed triangle normal** from screen
position derivatives. This overrides the mesh's smoothed vertex normal. The
512×512 globe map is not the source of local patch geometry or its PGS normal.

The independent CPU reproduction of settled patch selection, f32 local vertices
and shader morph sampled a 20 km square at250m spacing (6561 dry samples):

| Quantity | Control | Candidate |
|---|---:|---:|
| Nominal cell sizes |151–301m|151–301m|
| Height RMS / maximum error |0.0292 /0.9286m|0.2294 /2.8299m|
| Normal RMS / maximum angle error |0.0362 /0.9391°|0.2498 /2.7135°|
| Height absolute95th percentile |0.00320m|0.42248m|
| Normal95th percentile |0.00165°|0.54696°|

These measure resampling/morph differences, not source-detail quality. They are
not a full-footprint bound, exact GPU floating-point equality, or a shoreline
error result. The CPU audit uses the nominal ship position at absolute
height6456.78422877m; runtime pilot eye is6456.257570m (about0.527m lower).
The CPU model assumes resident ideal LOD. The actual overlay capture confirms
stable residency at frames120 and240, no pending patches, with levels6–12 over
the visible horizon and levels11–12 in the audited window. Runtime wire/normal
glyphs reconstruct the actual resident patches, excluding skirts.

Canonical edge lengths near the summit are measured independently in
`canonical-edges.json`. The regional canonical mesh is much coarser than the
render grid; more render vertices cannot create absent source relief. Creases
crossing render cells can still be flattened, explaining localized normal and
height errors. This does not prove that every ridge/valley elsewhere is retained.

The changed footprint extends about136km from the frozen summit. Of21364 changed
vertices,9966 are inside the original60° vertical/16:9 camera frustum;3499 are
behind it. Only3947/8457 intended-basin vertices lie in that frustum. These are
point counts, not visible area: terrain occlusion is not included. The canonical
centre sightline meets the candidate at about3.09km and the control at4.26km.
The fixed view therefore compares different surface locations along the same
ray; it is a valid matched view, but weak evidence about the whole basin.

Two proposed local views in `cameras.json` look between real high/low canonical
vertices within10km of the frozen summit: point82290 (5751.402m) and86772
(5562.324m), about11.7km camera-to-target distance. They use the same absolute
camera height6751.402m, time7200s, neutral colour and shadows off; headings are
132.4558° and312.3593°, down slopes0.103268 and0.086992. These are reproducible
local high/low-ground diagnostics, **not surveyed ridge crests**. No measured
ridge-crest vectors exist in this candidate. Before treating either as a scenic
comparison, inspect source river overlays and terrain occlusion; retain the
original fixed view and use identical absolute camera positions across variants.

Reproduce the numerical audit using `pgs_render_audit` (world example) with
`surface.pgs lat lon absolute_height heading down output.json render_radius_m`. The companion
`tools/review/scenery_camera_audit.py PACKAGE OUTPUT --render-radius 6281370` reads exact PGS point
sections and saved masks, normalizes their direction vectors, and computes
frustum counts and camera settings. These are bounded sub-second runs after
building here, under the shared two-CPU/4-GiB launcher. GPU overlay evidence is
under `out/review/pgs-debug-v1/`; launch and limits are in
`pgs1-debug-overlays.md`. The audit quantifies local loss; no acceptance threshold was assigned. The
mountain-scenery visual goal remains unmet.
