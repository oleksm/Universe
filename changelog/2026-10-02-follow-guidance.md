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

## One guide at a time

- A follow program can run while a clearance stands (keeping station on a station you're
  cleared to dock at). Both then drove the one set of guide frames: the follow program re-keyed
  it every frame, the clearance's new plans keyed it back, and it was drawn twice, against two
  centres. Now one guide shows: the follow program's while it flies the ship, else the
  clearance's, which is rebuilt as soon as it takes the guide back.
- Dev scenario `approachkeep` (cleared to dock and keeping station).

## Orbit guide on station; Cancel on X

- On station, the follow program holds within a metre or so either side of its goal. The
  plan no longer includes a join that short (one that would turn the first frames about every
  frame): the path starts on the orbit, and a keep-at-range hold has no path, only its shell.
- The follow program's "LET GO" is now **CANCEL**, on **X**: a key can be pinned
  (`keys::PINNED`), given out before the first-letter assignment. EXCAVATE, which had X, is
  now **DIG** (G).
- Dev scenarios `gateorbit`, `gateorbit60`, `gateorbitwatch` (orbiting a gate, from above).

## Orbit: nose along the way, frames on a grid

- In orbit the ship flies nose first along the circle, banked into the turn as an aircraft
  is (its top to the anchor): what holds it on the circle pushes up, through one axis of the
  thrusters, and trims along the way can go on the main engine. (It was nose on the anchor.)
- The guide can space its frames evenly (`Guide::update_spaced`). An orbit's frames sit on a
  fixed grid of angles round the anchor, one every 10°, over the next 150° of the way: they hold
  still, you fly through them, and one more appears at the far end as you pass one. Keeping at
  range spaces its frames evenly too; closing on a rock keeps the landing ladder.
- An orbit round a rock no longer turns its frames with the rock (only its surface turns).
