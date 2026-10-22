# Engine review: fixes and consolidation, one by one

From the engine review of 2026-10-21 (duplication, fragmentation, control, performance).

1. A frame replaced on its way to the render thread (not taken within its patience) dropped what it asked of
   the GPU besides drawing: mesh uploads, pooled uploads and the releases sent only once (the land pyramid's
   evictions), so ranges stayed resident till pool pressure. The new frame takes them on
   (`Frame::carry_requests`); the old frame is freed after the slot's lock is let go.
