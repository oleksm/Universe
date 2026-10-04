# Settlements' ground in the world: Port Trethi's facilities, streets and power line

- The game loads `content/base/settlements.ron`, generated from Local Administration by
  `tools/standards/build.py`: each settlement's zones, parcels, streets, power lines and
  facilities, all in metres east and north of its spaceport's pad grid centre
  (`world::settlements`, `Content::settlement`). Facts only; what is made there stays the
  economy's.
- The registry build lays each facility's modules out on its parcel (rows from the side facing
  its street, in step order, 20 m of ground round each: an invented rule) and rejects a
  layout that doesn't fit. Trethi Foundry needed 620 m of depth, so parcel 1 was deepened to
  650 m.
- Drawn near the port (within 60 km): each module a solid, lit box on its footprint, standing
  on the ground as it falls away with the body's curve; streets as paving 20 m wide; power
  lines on 25 m poles. Checked from above against the registry's map (warehouse south past the
  hangar, foundry east along East Road).
- Treistun's ten ports in the game (seed 1984) match the registry's names, bodies, latitudes
  and longitudes exactly.
- Dev scenario `settlement` (our ship on a pad at `UNIVERSE_PORT`, default Port Trethi; camera
  `UNIVERSE_DIST`, `UNIVERSE_YAW`, `UNIVERSE_PITCH`).
- Not yet: zones and parcels as a map layer; landing or flying into buildings (no collision);
  the economy reading facilities.
