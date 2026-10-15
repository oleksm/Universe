# The Planet LOD app

*2026-10-14, planets. The owner: migrate the browser Planet LOD viewer (planet-trees-pgs2) to a mini app, then make
it 120 fps.*

- `freefall --app planetlod [VIEW_DIR]` (default `~/planet-data/views/earth_s15-pattern`; Yosemite from USGS 3DEP at
  `~/planet-data/views/earth_s15-yosemite`): the pattern's progressive mesh and the planet around it, the page's
  panel (`game::hud_kit`, a shared panel of widgets), pins, crest and valley lines, Mesh and Real views, height x,
  flight keys, the observer's `/status`.
- Per-vertex mode: chunks sized by triangle count, decisions in a thread of their own, chunk meshes built by a
  worker and put on screen together once all are on the GPU.
- Cluster mode (with `<view>-clusters` beside the view, built by planet-trees-pgs2 `tools/planet_clusters.py`):
  fixed clusters of about 128 triangles, a Nanite-style cut by on-screen error (watertight, under a pixel), on the
  engine's mesh pool; the cut for a second ahead uploaded but not drawn.
- Measured on screen: Yosemite at 3 km in Real view, 2,630 clusters, 333k triangles, 120 fps (the refresh cap);
  3x3 grid at 65 km: app 0.7 ms, render 0.7 ms, GPU 0.4 ms.
- Not yet: the clusters' top levels stop halving (the pattern's rim stays locked); no morph between clusters.
