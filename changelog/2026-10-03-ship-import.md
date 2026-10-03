# Ships from Blender: imported hulls that fly

- `world::import`: a glTF ship becomes a hull like any other: collision from its `COL_*` meshes
  (convex parts; none: the model's convex hull), nozzles, gear, docks, cockpit and mounts from
  named empties, standard slots for its size class with as many hardpoints, racks and utility
  slots as it has mounts, a stock fit, its frame's mass from its size; its mass, inertia and
  thrust then from the physics. The same file, the same hull. Conventions: `docs/ship-import.md`.
- Hulls carry their model (`ClassSpec::visual`): imported ones render as made (PBR), the rest as
  before. The PBR loader skips `COL_*` meshes.
- The test hull now follows the conventions (nose +Y, collision boxes, 26 nozzles: main, lift,
  four manoeuvring quads; gear, cockpit, hardpoints, cargo, utility; name and class): it imports
  as the TEST MINER, 26 t dry, 59 m/s² main, 29 m/s² lift.
- `UNIVERSE_HULL=file.glb`: fly an imported hull.
- `design::standard_slots` shared by designs and imports.
