# 2026-09-30 — Ship mass; sketchy but physical terrain

## Ship mass

- The ship has mass: 60 t dry + 30 t fuel (+ cargo, 0 for now). Engines are defined as thrust
  (main 2.7 MN, thrusters 540 kN per axis, lift 2.25 MN), so acceleration = thrust / mass:
  30 / 6 / 25 m/s² at full load, as before; a heavier ship is slower (test: a 10 s full burn
  gives 300 m/s empty and 150 m/s at double mass).
- Autopilots, guidance speed profiles and the flight planner use the ship's actual capability.
- HUD: `MASS 90.0 T  FUEL 30.0 T  MAX ACC 30.0 M/S2`. Fuel isn't consumed yet (next: fuel +
  rocket equation, which also forces the real deorbit landing).
- (Gravity doesn't depend on the ship's mass — it cancels — so orbits were already right.)

## Terrain (sim)

- New `terrain` module: a deterministic height function per rocky planet / moon (value-noise fBm
  continents, ridged mountain ranges on high ground, craters with rims), amplitude scaled to the
  body (~1.5–7 km). Kinds: Terran (oceans at sea level 0, ~half the surface), Dry, Cratered.
- The ground around every spaceport is flattened (flat to 4 km, blending back by 40 km).
- Physics uses it: collisions are against the actual surface (sphere pre-check with the maximum
  terrain height), landing puts you on the terrain, touching down on an ocean is fatal
  ("<planet> ocean"), the landing impact prediction and the planner's crash check follow the
  terrain, and the landing route clears the tallest mountains.
- Bug caught by a new test in the game's own universe: over the pad, the "clear of the tallest
  mountain" check flip-flopped around the entry altitude and the autopilot oscillated for hours.
  The check is skipped over the flattened ground around the port.
- 23 sim tests pass (new: terrain sanity + flat pads, heavier ship, autoland in the game universe).

## Terrain (visuals)

- Engine: models can carry per-vertex colors (`WireModel::colors`, `Frame::model_colored`), plus
  `Frame::line2` / `triangle3` with per-vertex colors.
- **From orbit**: a 24×14 latitude/longitude globe displaced by the terrain, lines colored by the
  ground under them (blue ocean, green/olive land, white peaks, dark craters) over a dark tint of
  the same colors. Built once per body.
- **Near the surface** (below 30% of the radius, max 1,500 km): a local grid on the ground, fixed to
  the planet on a cube-sphere lattice with power-of-two spacing (stable, no swimming), sized to
  reach the horizon, fading at the edges, colored by terrain, with dark filled cells. It's the
  "surface starts here" cue.
- **Craters**: rims drawn as circles on the surface when the body is large on screen.
- Performance (release): ~5 ms/frame including startup in the terrain scenarios.

## Dev

- Scenarios: `lowflight` (6 km over the home planet, toward its highest nearby ground), `moon`.

## Fix: hyperdrive couldn't catch gates; gates drifted away after drop-out

- Playtest: "hyperdrive never stops, hovers at 20 km to the gate; if I turn it off the gate is
  flying away; I can't get closer on normal drive".
- Cause: hyperdrive motion was measured against the star, but targets ride along with their
  planet (~30 km/s) and gates also orbit it (~3 km/s). Since hyperdrive speed shrinks with
  distance, below full throttle the ship settled at the distance where its speed matched the
  target's. Manual drop-out matched the planet's velocity, not the gate's.
- Hyperdrive now moves relative to the target (a port: its planet), or to the dominant body with
  no target; manual drop-out within 1,000 km of the nav target matches the target's velocity.
- Test: at 30% throttle the hyperdrive reaches a gate and stops at rest relative to it; a
  manual drop-out near it also matches it. 24 sim tests pass.

## Change: no automatic clearance request on hyperdrive arrival

- Playtest: "When I come to the gate it automatically requested pass through. I still want to do
  it myself." Arriving by hyperdrive at a station, port or gate no longer requests clearance; the
  message says `ARRIVED AT <TARGET>` / `R TO REQUEST CLEARANCE`. (Fits "manual by default".)
