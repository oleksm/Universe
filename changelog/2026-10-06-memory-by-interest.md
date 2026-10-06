# Memory by interest: what's near is held, the rest let go

- **Fine height tiles in one cache by bytes** (`worlds::Heights`): read as they're wanted (by the
  physics, waiting; by drawing, in the background) and kept while there's room (384 MB,
  `UNIVERSE_TILE_MB`); past it the least lately used go, and are read again if wanted again (the same
  files: the same heights, so a run is the same). They used to be kept forever once read.
- **The 5 km heights read on the first height wanted**, not when the system is made; the highest
  ground from the bake's peak list. A process that never asks a baked world's height (a headless
  pilot far away) holds only its manifest.
- **A baked world's full maps only near it** (`WORLD_MAPS_NEAR`: within 8 radii; let go past 16),
  each image encoded as soon as it's read (one held at a time, `WorldMaps::encode`): they used to be
  made for every baked world at first sight, all decoded at once.
- **The near ground's patches at most 3,000** (`terrain_lod::MAX_PATCHES`), the least lately used
  going first, besides those unused 600 frames.
- Measured over 25 s: low over Hearth 0.87 GB (as before the bakes; it was 1.05); low over Harvest
  at 200 m/s 1.27 GB, peaking 1.54 while its maps are read (it was 1.3, peaking 1.9).
