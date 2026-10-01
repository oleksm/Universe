# Guidance for the follow programs

Keep at range (N), orbit (U) and closing on a rock (3, mining) now guide like landing and
docking do:

- **A phase banner** across the top: what's followed, the steps — CLOSE IN > MATCH DRIFT >
  ANCHOR (Y); JOIN ORBIT > ORBIT; CLOSE TO RANGE > HOLD — the current one boxed, the distance
  to go and the ETA.
- **In the view**: a dashed path from the ship to where the program is taking it, the goal
  marked (green once there); the orbit's circle; the range shell kept at; on a rock, the
  anchor's reach ringed on the surface under the ship (green when in reach and drifting slowly
  enough).
- **The numbers**: distance to go and closing speed; on a rock, the gap to the surface and the
  drift against it, against the anchor's limits (within 30 m, under 0.5 m/s) — Y TO ANCHOR
  when both are met. X lets go of any of them.
- Dev scenarios `closing10`, `closing`, `orbitrock`.

## One guidance system

- The follow programs now guide with **the same guide frames and path** as landing, docking
  and gates (`scene::Guide`, `scene::guided_path`): the way the program takes us is drawn as
  a plan (straight to the goal; for an orbit, on round the circle; over a rock, turning with
  it), and the frames are laid along it the one way. The dashed line and goal diamond drawn
  only for follow are gone; what's particular stays (the orbit's circle, the range shell, the
  anchor's reach on the rock).
- **Guidance shows in the observer's view too**, wherever it's on.
- The guidance banners sit below the mode bar.
- Every message that names a key takes it from the bindings (and the engine's own messages no
  longer name keys at all: they're the client's).
