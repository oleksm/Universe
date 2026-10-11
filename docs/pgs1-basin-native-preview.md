# Native-connected drainage pilot: engine readback and fixed views

Candidate `b84f9ef3` from planets' `scenery-basin-s15-native-connector-v2`, compared
with unchanged 1.25 km control `5029fe82`. Source and independent review manifests
were checked in full. The candidate manifest lacks the preview's required
`format` discriminator; a temporary staging manifest adds it while retaining
identical hashed PGS/fixture bytes. No source package or registry installation
was modified. The engine's reader passes all 198 owner/height/water queries and
198 derivative queries: max height error 1.27e-11 m, slope error 1.04e-17,
normal component error 2.22e-16. Cap3 preserves unknown categories.

Four Vulkan PNGs and logs are under `out/review/pgs-basin-native-v1/`:
`control-summit`, `candidate-summit`, `control-lake`, `candidate-lake`.
Exact paths, hashes, renderer hash and metrics are in
`pgs1-basin-native-validation.json`. Same release renderer, neutral mode,
shadows off, fixed heading/down angle, paused time and camera frame. Terrain
and known water remain canonical. All captures ran under the shared 2-CPU/8-GiB
launcher with GPU enabled.

At the frozen summit, surface height changes 5456.78422877 -> 5746.57790378 m.
The candidate uses altitude 710.206324998 m rather than 1000 m to retain the
same absolute camera radius; time7200 s and sun elevation15.684057 degrees
match. At the lake, water surface620.613115 m and altitude10000 m are unchanged;
time33503.4 s and sun elevation12.521021 degrees match. The candidate/control
lake frames differ by at most one byte per channel. The logged lake surface is
water level, not the underlying 590.379461 m terrain bed.

Visual inspection: the fixed summit view remains very smooth, with only subtle
relief shading changes. The shared lower-middle terrain crop changes by mean
0.229 byte/channel, with no pixel exceeding two bytes. The full-image difference
includes the deliberately different terrain-relative HUD altitude. This is
numeric and fixed-view agreement, not a claim of convincing mountain scenery,
visible cast mountain shadows, physical hydrology or production acceptance.
