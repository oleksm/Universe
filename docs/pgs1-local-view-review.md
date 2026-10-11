# Frozen local high/low view review

Both proposed ground targets pass a canonical sampled occlusion screen on the
control and candidate. The high-to-low view is the useful channel diagnostic:
178 source-channel points and 143 native-channel points lie in its frustum and
pass that screen. The reverse view includes 29 native points, but zero source
channel points. No connector points lie within the 30km display radius.
Counts are sidecar points, not unique channel length or a continuous visibility
proof. Control evaluates the same channel directions on its own field; it does
not claim those transplanted source channels exist in the control.

All six Vulkan captures were inspected. Blue source and yellow native lines are
visible high-to-low; reverse shows the native line. Neutral views remain broad,
faintly shaded terrain with no convincing mountain silhouette. These views
remove target occlusion as the explanation at these two sites, within the
sampled test's limits; they do not establish a global source-detail diagnosis.

Settings are unchanged from `diagnostics/scenery-detail-v1/cameras.json`:

| View | Latitude / longitude | Heading | Down slope | Control / candidate altitude m |
|---|---|---:|---:|---:|
| High to low | -50.930818875 / -19.207806685 |132.455774406|0.1032684903|1301.977429670 /1000.084144317|
| Low to high | -51.002226990 / -19.083665221 |312.359343999|0.0869918957|1408.967977580 /1189.114890971|

Nominal absolute eye height6751.402172317m, host radius6281370m, source
radius6371000m, time7200s, neutral colours, shadows off, paused, zero speed,
60deg vertical FOV,1920x1080. Capture manifests retain full precision settings,
renderer/PNG hashes, actual logged eye heights and frame120/240 LOD residency.
Terrain-relative launch altitude differs to keep the absolute eye fixed.
The first run exposed up to8cm eye differences from frame/surface lookup.
Calibrated final captures match logged eye height within1 micrometre per pair:
high-to-low6751.402175m; reverse6751.402171/6751.402172m.
Neutral runs use an empty diagnostic sidecar solely to expose residency/eye
logs; they draw no geometry or markers. Candidate channel overlays retain the
producer's frozen-summit marker and 2m display-only lift. No appearance tuning.

`pgs_view_visibility` queries the actual engine reader at <=25m steps, excluding
the last25m to avoid self-contact, using1mm negative clearance as obstruction.
Candidate target minimum sampled clearances are4.061m and4.264m; control5.453m
and5.115m. Narrow obstructions between samples or within the last25m are not
excluded; neither are subpixel raster/LOD occlusions. The target is ground,
not a labelled ridge crest. These screens use nominal ship eyes; runtime logs
record the final calibrated heights above.

Reproduce with the world example `pgs_view_visibility surface.pgs cameras.json
scenery-debug.json output.json`, once per variant, followed by
`tools/review/scenery_local_captures.py` under the shared2CPU/8GiB scenery
launcher. Capture files are under `out/review/pgs-local-views-v1/`; audit and
capture manifests are under `docs/diagnostics/scenery-local-views-v1/`.
