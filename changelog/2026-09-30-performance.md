# 2026-09-30 — Navigation performance

## Measured (release, one core of a Ryzen 9 9950X3D)

`cargo test -p universe-sim --release bench_navigation -- --ignored --nocapture`

| One ship, one frame (60 fps)          | Before  | After   |
|---------------------------------------|---------|---------|
| Coasting in orbit, warp 1             | 3.1 µs  | 1.9 µs  |
| Coasting in orbit, warp 1000          | 4.8 µs  | 4.6 µs  |
| Docking / landing autopilot, warp 1   | 5.0 µs  | 3.6 µs  |
| Landing autopilot, warp 100           | 69 µs   | 22 µs   |
| Hyperdrive                            | 1.8 µs  | 1.7 µs  |
| Flight planner (landing / docking)    | 4.5 / 2.5 ms | 2.0 / 1.1 ms |

1,000 ships at normal speed ≈ 2–4 ms/frame on one core (budget 16.7 ms).

## Changes

- **Per-frame ephemeris** (`StarSystem::ephemeris`): exact body positions, velocities and
  accelerations once per frame, extrapolated for each physics substep (p + v·τ + ½·a·τ²; error
  ~millimetres over a frame). Falls back to exact solving when a frame covers more than 5 s of
  game time. This is also the shared data many ships would read.
- **Kepler fast path**: circular orbits skip the solve; a better first guess for the rest.
- **Planner**: sphere pre-check before terrain lookups; the game rebuilds the plan 10×/s (or at
  once when clearance/target/phase changes).
- **Plans are anchored**: a plan remembers its reference (station, gate, or a port's planet),
  where it was and how it spins; drawing carries an older plan along with it. (Without this a
  0.1 s-old plan was drawn ~3 km off, since everything moves at km/s.)

## Not done yet (needs multiple ships)

- The `Universe` has one ship; traffic needs a list of ships sharing the per-frame ephemeris.
- Coasting ships "on rails" (exact Kepler around the dominant body), spreading ships over
  threads, and coarser steps for ships far from any player.

## World time scale (for a shared multiplayer world)

- Decision: one fixed time scale for the whole world instead of per-player warp. Speeding up
  time is physically the same as rescaling the universe; the user tried 10× ("pretty fluid") and
  chose a **5× default, configurable**: `UNIVERSE_TIME_SCALE=10 cargo run --release` (1–100).
- Game time runs at that scale for everything (orbits, flight, the clock); ship turning stays at
  human speed. The single-player warp keys multiply on top and would go away in multiplayer.
- The hyperdrive (speed defined per real second) and the 5 s gate transit keep the world clock
  at the same pace for everyone. HUD shows `TIME X5`; the ETA is now in real seconds.

## Fewer guidance frames

- Fewer tunnel frames: first every other one, then (at the user's request) every fourth, keeping the exponential spacing.

## Q&A: do stations and gates have mass?

- Yes: a station is 10⁹ kg, a gate 10¹⁰ kg. Their gravity is deliberately left out: 500 m from
  a station it's ~3×10⁻⁷ m/s², about 10⁸ times weaker than the thrusters. They matter as
  obstacles, not as gravity sources.
- Later the same day: default lowered to **2×** at the user's request (`UNIVERSE_TIME_SCALE` still overrides).
