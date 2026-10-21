# Retain terrain while moving through detailed land

Raise the land-pyramid cache target from 256 MiB to 8 GiB and its per-frame GPU
upload budget from 4 MiB to 32 MiB, as requested by the owner for the RTX 5090.
The reported 10 km view needs 330 tiles and about 904 MB: retaining that working
set and nearby terrain avoids repeatedly loading it as the camera moves. The
larger upload budget lets newly selected tiles become ready in larger batches.

Existing LRU eviction, coarse fallback, worker streaming and generation handling
continue to apply. The cache target protects selected geometry as before; it is
not a hard total-process or total-VRAM cap. Pyramid regression tests and the game
build verify the change.
