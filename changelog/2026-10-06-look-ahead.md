# The near ground looks ahead

- The lab's R3: from the eye's way since the last frame, the patches it will want 1, 2 and 4 s on
  (`terrain_lod::ideal` at the eye's place then) are made after those it wants now (coarse first,
  8 a frame), so they're ready before they're first drawn.
- Measured flying at 2 km/s, coming down from 40 km over Harvest's mountains: the worst frame 19 ms
  of 300, no hitches, no holes.
