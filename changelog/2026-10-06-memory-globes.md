# 4 GB less: whole globes from the coarse heights

- The game held about 5.4 GB from a baked world's first sight: each globe mesh (`terrain_view::globe`)
  took its heights in full, and so read every fine tile of every baked world (Harvest's 635 and
  Cinder's 1,536, about 2.1 MB each in memory) and kept them. A globe's vertices are hundreds of
  kilometres apart: they take the 5 km heights alone now (`Terrain::surface_coarse`).
- Measured over 25 s: low over Hearth 5.4 GB to 1.05 GB (it was 0.87 GB before the bakes); low over
  Harvest at 200 m/s 1.3 GB (peaks to 1.8 GB while its maps are decoded).
- Next: fine tiles in a cache by bytes, visual data only for the world the eye is near, and none at
  all where nothing looks.
