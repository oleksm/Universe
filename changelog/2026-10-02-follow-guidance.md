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

## Orbit range: hold to choose

- The orbit key: a tap orbits at the range chosen last (before any is chosen, the preset
  nearest how far the anchor is; again, the next one out). Held, it lists the ranges — 500 m,
  1, 2, 3, 5, 10, 20, 30 km — the mouse or wheel moves the cursor, and letting go orbits at the
  one under it, which becomes the default (`orbitpick`). The engine takes the range with the
  command (`Command::Follow(kind, Some(range))`), no closer than the anchor allows.
- Ranges now run 500 m to 30 km (were 1 to 20 km), for keeping at range too.
- Ranges read 500 M, 1 KM … 30 KM on the button, the list and the messages.
- An orbit's guide now includes the frame you're about to fly through: its path starts at the
  grid mark behind the ship (the path's own start has no frame), so the next mark always has
  one, and it goes once you're through it. (It started a step ahead, ~500 m at 3 km.)
- Dev scenario `orbitpick` (the list up).

## Thrusters commanded per axis

- The follow program (and the hunter's `thrust_for`) turned a wanted acceleration into a
  thruster command dividing by the side thrusters' push on every axis; the lift thrusters
  push up about four times harder, so an upward command came out ~4× too strong. Banked in
  orbit (the push to the anchor is up) feedback hid it on average, at four times the loop gain
  and 10 m inside the range. They use `Ship::thruster_command` now (per axis).
- Measured: orbiting at 3 km, a 90 t ship holds itself on the circle with ~216 kN up, 0.022 kg/s
  of fuel (thrust over the exhaust velocity): about 80 kg an hour, the main engine off.

## Arches the ship's size

- Evenly spaced guide frames (orbit, keep at range) were sized from their spacing (10° of the
  orbit), so they grew with the range: ~600 m wide at 10 km round a gate, smaller than the ship
  at 500 m round a rock. They're now arches of the ship's size at any range (3.5 ship radii
  half-width, 0.4 of that tall: ~84 × 34 m), wide along the wings of a ship banked into the
  turn. The landing/docking ladder keeps its sizes.
- Dev scenario `orbitrock1k`.
