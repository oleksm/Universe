# 2026-09-30 — Profiler, and the first round of what it found

User: "lets review our engine and game. I noticed that FPS is now dropping to 100 fps near crowded
places. Are we using multi threading for calculations? I dont believe there are lots of triangles
to process that would hit limits of my 5090 so lets profile what could be a bottleneck. Ok to write
some instrumentation for better troubleshooting"

## Instrumentation

- **New crate `universe-prof`**:
  - `let _p = scope("sim/crafts")` or `time("draw/scene/bodies", || ...)` times a stretch of work;
  - `frame_end()` folds each frame into mean and worst time per frame, and calls per frame, over
    the last 120 frames;
  - names are paths, so the report reads as a tree, and a path with no scope of its own gets a
    row summing its children;
  - off by default, and a scope costs nothing then; on, a scope is a clock read and a lock;
  - any layer can use it (sim, world and avionics have no graphics).
- **Scopes added**:
  - the engine's frame: update, draw, render (vertex upload, waiting for the surface);
  - every part of the game's update (sim, globes, plan, contacts, fire control, collision
    warning, view, sound…);
  - the scene and HUD draws, down to planet meshes, the surface grid and the star field;
  - the sim tick: snapshot, player, crafts, then per craft sightings, hunt, avionics, world step
    (flight, landed, hyperdrive, handover, air, heat), combat (weapons, collisions), traffic
    presence, recorder.
- **F3**: the profiler and an in-game panel of the scopes taking real time.
- **`UNIVERSE_PROFILE=1`** with `UNIVERSE_SCREENSHOT`: the report is logged at the end of the run.
- **Dev scenario `crowd`**: 300 home-system crafts flying their routes within 40 km of the station.

## What it showed (headless crowd, 1,000 ships, per frame)

- **No multithreading anywhere**: one thread does everything. The GPU waits are ~0: the 5090 is
  idle, and the whole frame is CPU.
- Before: 7.75 ms. Sim 3.3 ms, draw 2.3 ms (planet meshes 1.5, star field 0.6), render 0.55 ms.

## Fixed in this round

- **Body positions solved once per system per moment** (`World::rails_now`, the last two moments
  kept, since each ship's turn spans the frame's start and end).
  - Every docked and landed ship (~700) was re-solving every orbit in its system each frame, and
    the avionics bus did it again on every call.
  - World step for 1,001 ships: 1.17 → 0.46 ms.
- **`traffic_presence`**:
  - each system's ports are worked out once a frame, not once per ship;
  - no cloned position lists;
  - only corridors actually held are checked (`TrafficControl::corridors_held`).
  - 0.61 → 0.42 ms.
- **Planets far off use a coarse mesh**: 720 triangles, not 11,500, under 90 px radius on
  screen. Planet meshes 1.45 → 0.52 ms.
- **The star field is cached per system**, rebuilt only when zoomed out toward galaxy scale.
  0.6 → 0.09 ms.
- After: about 6.2 ms per frame. Sim 2.4 ms, draw 0.8 ms (it was 2.3).

## Still to do (not in this round)

- Planet meshes, ship models and the surface grid are still transformed and lit on the CPU every
  frame and re-uploaded. Static meshes kept on the GPU, lit in a shader, would remove that work.
- Building a system's planet meshes on first sight takes ~55 ms: a hitch on arrival. It could be
  done on a background thread.
- The crafts' tick (avionics plus physics per ship) is the biggest remaining cost, and runs
  serially. It can be spread over threads, but the world holds `Rc` everywhere and ships make
  requests to traffic control mid-tick: that needs restructuring.
