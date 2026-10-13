# Frozen Cape Fold view: coverage and lighting diagnosis

The frozen summit view covers the new terrain, but predominantly gentle slopes.
There is no evidence here that large normal smoothing is the primary reason it
looks faint. Sun direction and the low-contrast neutral presentation limit relief
readability in this particular camera. No terrain, lighting, material, camera or
frozen capture was changed for this audit.

`pgs_visibility_light` samples 202 unobscured screen-grid locations below the
horizon, omitting the instruments, radar and nearby navigation marker. Each ray
finds its first canonical terrain crossing with 25 m marching to 100 km and
bisection. All 202 hit dry ground. Each candidate owner face touches changed
vertices; 201 touch the mapped basin. This is a sampled face-touch count, not an
area integral. Candidate ranges are 2.12–56.34 km, median 4.28 km.

| Quantity | Control | Candidate |
|---|---:|---:|
| Canonical slope median | 0.549° | 1.810° |
| Canonical slope p95 | 0.694° | 3.344° |
| Canonical slope maximum | 0.694° | 4.476° |
| Render slope median | 0.549° | 1.793° |
| Render slope p95 | 0.694° | 3.049° |
| Normal error median | 0.00086° | 0.01457° |
| Normal error p95 / maximum | 0.00494° / 0.311° | 0.712° / 1.532° |
| Render sun cosine p05–p95 | 0.2753–0.2833 | 0.2728–0.3205 |
| PNG luminance p05–p95 (0–255) | 65.00–75.60 | 66.00–73.71 |

Rendered normals are an independent reproduction of the settled LOD's f32
vertices and shader morph, **not a GPU normal-buffer readback**. Nominal cells
range 151–1204 m; candidate median 151 m. Some local normal errors are meaningful,
especially at canonical face boundaries, but median/p95 slopes retain the main
signal. Shader `scene.wgsl` explicitly derives canonical patch lighting normals
from screen derivatives of the drawn triangles; the legacy 600 m material slope
estimate is not used for this neutral lighting path.

Visible point checks, in original 1920×1080 pixel coordinates:

| Pixel | Range | Canonical/render slope | Normal difference |
|---|---:|---:|---:|
| 1320,600 | 4.16 km | 1.529° / 1.555° | 0.0265° |
| 1720,655 | 4.20 km | 1.8685° / 1.8687° | 0.00065° |
| 1240,765 | 2.52 km | 3.923° / 3.677° | 0.645° |

The actual host system at 7200 s gives sun elevation **15.684057°**, azimuth
**94.985686°**, matching the original log. Camera heading is 90°, down slope .25
(14.04° downward), putting the sun 30.126° from the view axis and nearly straight
ahead horizontally. It supplies little cross-view side lighting. Every sampled
normal still faces the sun; the frozen runs disable terrain shadows. Their
neutral palette does not provide rock/cover differentiation to make slopes read.
A unit-sun Lambert-plus-0.06-ambient proxy ranges p05–p95 0.316–0.361 for the
candidate. This is not the actual pixel shader result: irradiance, fill,
atmosphere and tone mapping are excluded. Final PNG 5×5 median luminance is
reported separately and cannot isolate which of those stages compresses contrast.

The supported diagnosis is visible but gentle canonical geometry plus weak
contrast in this frozen view, with local LOD deviations rather than wholesale
loss of normals. An alternate downward wire view can reveal triangulation and
shape; it is a separate diagnostic, not a replacement for these frozen images.
No visual tuning is warranted by a claim that the mesh missed the entire basin.

Evidence: `diagnostics/cape-fold-light-v1/{control,candidate,summary,coverage}.json`;
reproducers `crates/core/world/examples/pgs_visibility_light.rs`,
`tools/review/cape_fold_light_summary.py`, `tools/review/cape_fold_coverage.py`.
All work used the shared 2 CPU / 8 GiB launcher. PGS SHA remains f3df5e59; original
PNG hashes remain recorded in the previous capture manifest. Ray marching,
screen sampling and ideal LOD are bounded diagnostics, not continuous visibility
or actual GPU-residency proofs.
