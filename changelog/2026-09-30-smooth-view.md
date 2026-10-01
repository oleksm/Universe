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

## Second pass (user: "The flicker and blue target multi jitter did not go away"; "THe issue first noticed when we first make multithreading split")

The first pass measured a craft's position, which was fixed, but not what was actually seen.
Consecutive captured frames showed the light-blue nav target marker on the station jumping
between 4105 and 4435 m frame to frame.

- **Nav target marker and approach guidance.** Both still came from the view, at the tick.
  Drawn a tick earlier than that (between views), a station orbiting at ~50 km/s is ~835 m off.
  This is why it began with the threading split: before it, the world and the drawing were the
  same moment. Now the client works both out at the drawn moment from the charts and the shared
  code: `App::nav_marker_now`, and `App::approach_now` (avionics `approach` for our ship as
  drawn). Same six frames after: 4000 m, steady.
- **The dark sides of planets.** The planet light (planetshine) for an object was worked out
  from its camera-relative position in single precision. For the reflecting planet itself, near
  it, that came out as up to a metre from its own centre, in a random direction that changed as
  the camera moved. So the planet randomly lit its own night side (up to ~60% brightness), frame
  to frame. Now planetshine is worked out from the object's true world position, and the
  reflector never lights itself (`Frame::fill_at`).

## Third pass: HUD showing through our hull

User: "my ship is transparent so everything goes through it" / "not everything, just text is
coming through but it looks wierd". Then, after a first attempt: "stil wiered even worse - text
kees overlapping to some level then hides. Target locks and guidance are still coming through."

The HUD (text, brackets, markers) is its own layer over the scene, without depth, so whatever it
marks behind our ship was drawn over the hull. The first attempt (hiding a whole label when the
point it marks was behind the hull, by a CPU ray test) was wrong: a label overhung the hull,
then popped out all at once.

**Fix: per-pixel hull mask.**
- **Occluders:** `Frame::occluder` registers our ship (it's drawn as usual too). The renderer
  draws it again into a low-res mask (`R8Unorm`), depth-tested against the scene's depth, which
  is now stored for this. So the mask is where the hull is actually seen.
- **Anchored HUD:** HUD drawn inside `Frame::anchored(...)` carries a flag in its vertices' z.
  The HUD shader (`fs_hud`) discards those pixels wherever the mask is set: text, brackets and
  boxes are cut along the hull's silhouette, as if behind it.
- **Anchored:**
  - labels (bodies, gates, stars), craft names, the guide path's distance;
  - contact boxes and ranges, the locked target's bracket and motion arrow;
  - the nav target marker, turret markers, the impact marker, the reference body's bracket.
- **Not anchored (instruments, always on top):** the gunsight and gimbal ring, the lead pip,
  the velocity / prograde / retrograde markers, the nose cue, and the status text.

## Fourth pass: one ship as drawn

User: "yellow direction cursor is till coming through" / "how was it working fine on before we
started making multithreading changes, can you go back and review that?"

- **Review.** I built two old versions (`82cdcf4`, fight or flight, and `f26ca25`, just before
  Phase 1) and captured the collision scene. Both draw the HUD (text, the gunsight cross) over
  the hull exactly as now: the HUD layer has never had depth.
- **What the split did change** is motion. Before it, one ship state drove the hull, the camera
  and the HUD, so the overlays sat still on the hull. Since then, the hull and camera come from
  the blended ship, while 36 HUD and scene reads (the gunsight, lead, prograde, scanner, guidance)
  read the latest tick (`app.v.ship`). That is a tick ahead, so they slid against the hull
  whenever the ship turned or accelerated.
- **Fix (as a class).** `App::ship` is our ship as drawn this frame: position, turn and velocity
  at the moment drawn, everything else from the view. All drawing (hud, scene, on foot, nav map,
  approach guidance, the eye on foot) reads it. Only commands still use the latest view, since
  they go to the world.
