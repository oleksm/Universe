# Zooming in and out no longer leaves the continent blurred

The 8 GiB land cache kept every visited tile's GPU copy alive too, so the 1 GiB mesh
pool (the device's buffer limit) filled after a few zooms and new covers could not be
uploaded: the view stayed on one coarse root tile with a storm of "mesh pool full".

The pool now evicts the least recently used GPU copies when it is full, independently
of the CPU cache. Evicted tiles keep their packed CPU mesh and are uploaded again from
it when needed, with no file read. Everything the current cover needs (drawn tiles,
uploads, fallback ancestors and prepared children, `Frame::retain_pooled`) is never
evicted. If that working set alone is larger than the pool, the warning is logged at
most once every 120 frames.

`UNIVERSE_PLANETLOD_FLY=zoom` loops 10 km -> 6,300 km -> 350 km -> 10 km (6 s holds,
3 s moves) to repeat the owner's zooming in a test window.

Engine pool and viewer tests pass. Native A/B on the zoom loop: old build 120,304
pool-full lines, 6,300 km stuck at 12/101 tiles and 350 km at 41/272; fixed build
3 warnings, every hold after the first cold cycle 101/101, 272/272, 330/330 with
nothing pending. docs/diagnostics/planetlod-gpu-residency-20261021.json.
