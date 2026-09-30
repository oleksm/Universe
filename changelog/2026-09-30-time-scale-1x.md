# 2026-09-30 — World time scale back to 1×

User: "lets get back to 1x". `DEFAULT_TIME_SCALE` is now 1.0, down from 2.0. It is still one fixed
scale for the whole (multiplayer) world, and `UNIVERSE_TIME_SCALE` still overrides it (1–100).
History: 1× at first, then "10x seems pretty fluid, 1x is a bit boring" led to 5×, then 2×, and now
back to real time.

## Settlers: 10,000 by default

User: "I asked for 10000". `UNIVERSE_SETTLERS` now defaults to 10,000, up from 50. Measured in the
game over the first 120 frames, including spawning: 14.8 ms/frame, against 6.0 ms with 50. That is
under the 16.7 ms budget for 60 fps, but with little headroom. `UNIVERSE_SETTLERS=50` restores
the old default.

Then: "ok lets do 1,000 for now". The default is 1,000, the size of the largest traffic test run
(0 crashes).
