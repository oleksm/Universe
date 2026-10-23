# Planet LOD parity: light exaggeration, a planet-fixed sun, a nadir camera

Planets `20261023T120619-planets-37bb` (one-ground plan, stage 3; `planet-forge/docs/lod-parity-requests-2026-10-23.md`).

- A pyramid's `light_exaggeration: {land, sea}` (`pyramid.json`, `world::land::Meta`) lights the ground steeper than
  it stands: tile normals from heights scaled by `land` over 0 m and `sea` under, the positions at their own height
  x. Absent: as before.
- `UNIVERSE_PLANETLOD_AT` takes `sun=e,n,u` (a direction in the east-north-up frame at the start's place, fixed to the
  planet from then on; else the eye-relative north-west sun as before), `pitch=nadir` or `pitch=<deg down>` (held at
  every altitude; else by altitude as before) and `fov=<deg>` (vertical; 50 as before).
- The near plane comes in to a tenth of the clearance over the ground below where that's less than a fifth of the
  altitude: at 2 km over e104's belt it cut nearby peaks (16 % of a nadir frame black). Settled frames otherwise
  identical.

Test: `pyramid_light_exaggeration_lights_steeper_at_true_height`; the workspace's library tests pass.
