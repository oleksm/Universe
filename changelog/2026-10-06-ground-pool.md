# The ground's patches made on a pool of their own: no frame waits

- The lab's streaming requirement R1 (`~/git/planet-sim/docs/streaming-requirements.md`): the
  near ground's patches (`terrain_lod`) are made on a pool of their own (two cores left to the
  rest), nearest first in their own sizes, at most 48 at once, and taken in at most 24 a frame as
  they're done. A frame used to wait on every patch it lacked.
- Until a patch is ready, the nearest made above it stands in; only a world's six whole faces are
  made on the spot (nothing stands in for them). A patch made before its ground was all read is
  made again on the pool, and swapped in when done.
- Low over Harvest at 200 m/s: 144 frames a second, the worst of 300 at 8 ms, no hitches.
- Next (R5, R6): patches brought in blended from what stood in, so a swap doesn't pop.
