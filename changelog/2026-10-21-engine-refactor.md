# Engine review: fixes and consolidation, one by one

From the engine review of 2026-10-21 (duplication, fragmentation, control, performance).

1. A frame replaced on its way to the render thread (not taken within its patience) dropped what it asked of
   the GPU besides drawing: mesh uploads, pooled uploads and the releases sent only once (the land pyramid's
   evictions), so ranges stayed resident till pool pressure. The new frame takes them on
   (`Frame::carry_requests`); the old frame is freed after the slot's lock is let go.
2. Contact asks each substep's body motion once: `ground::GroundFrame` holds the body's centre, velocity (its
   orbit chain solved once), spin and turn, and answers every contact point's three height samples; before,
   each sample solved the chain and the rotation again. The land pyramid's raster cache is in 16 shards locked
   apart (the parallel ships' queries no longer queue on one lock); a query no longer allocates. Not done:
   sharing tile reads between the renderer and physics (they read different rectangles; the OS page cache
   already shares the file reads), nor one copy of the anchor mesh (the renderer's f64 vertices would need
   rewriting for about 0.5 GB).
3. The pair grid's walk took every cell in a mover's reach: by the cube of its sweep (at warp or high speed,
   millions of empty cells a mover). It now takes the group's occupied cells in that box when they are fewer,
   in the walk's order so the contacts are the same. Collisions take the world's cached body positions
   (`rails_at`) instead of solving them again. (Rebuilding the grid each tick stays: everything moved.)
