# 2026-09-30 — Smooth drawing between world ticks

User: "shaded side of planets are flickering badly / target guidance, render settlers are jumping
frame to frame pretty badly, there is some desync shaking so I see sort of 5 blue targets on
screen flickering"

## Cause

The world ticks at 60 Hz on its own thread. The client (drawing at 160+ Hz) carried positions
forward from the latest view by the time since it *picked that view up* (phase 3). Two faults
followed:
- **Shaky time.** The pickup time jitters by up to a client frame, so the drawn world time and
  everything placed by it (ships, bodies, planet rotation) jumped back and forth a little each
  frame.
- **Inconsistent frames.** The forward step was recomputed from the clock on every call, so the
  camera, each craft and each marker in one frame were drawn at slightly different moments.
  Measured on the nearest craft: its speed relative to the camera varied by **56 m/s mean,
  936 m/s max** from frame to frame, at a real relative speed of about 30 m/s.
- **Markers not carried forward.** The HUD markers (contacts, the lock bracket, the scanner)
  were drawn at the tick's positions while the camera had been carried forward, so they shook
  against everything else: the "5 blue targets".

## Fix: interpolate, don't extrapolate

- **Each view is stamped** with when the engine made it (`View::made`).
- **The client keeps the previous view** and draws **one tick behind real time, between the
  two**, by those stamps.
  - The blend (`App::alpha`) is taken **once per frame**, so everything in a frame is drawn at
    the same moment.
  - A view late in coming holds at the latest rather than guessing ahead.
- **Blended:**
  - world time (`App::now`, so bodies and their rotation);
  - our ship and every craft (`App::place(Who)`: position lerp, orientation slerp; a ship that
    jumped between views, by a new ship, a gate or another star, isn't blended);
  - the HUD markers (contacts, lock bracket, lead tick, scanner);
  - turret markers (computed from the charts at the drawn time);
  - tracers (carried to the drawn time by their own velocity).
- **After:** the same craft's speed relative to the camera varies by **0.29 m/s mean, 0.47 m/s
  max**, which is its real acceleration. The drawn time advances evenly (3.6–4.9 ms steps at
  ~200 fps, never backwards).

## Tools

- `UNIVERSE_DEBUG_MOTION=1`: logs, each frame, the drawn world time and where the nearest
  craft is drawn relative to the camera (to measure smoothness).
- `UNIVERSE_SCREENSHOT_FRAMES=n`: with `UNIVERSE_SCREENSHOT`, also captures the n frames after
  it (`name_1.png`…), to compare frame to frame.

## Planets' dark sides

I compared consecutive frames (planet from orbit, noon, dusk, night, low flight, landing
approach) after the fix and found no flicker: at most a few hundred pixels changing during a
landing approach, which is real motion. The jitter above moved bodies and their rotation too,
and is the likely cause. If it persists, it needs the exact situation to reproduce.
