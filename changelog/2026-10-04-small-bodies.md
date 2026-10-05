# Small bodies in every system

- **Every system the seed makes now has its small bodies**, by the rules the registry's seeder
  followed (each after what is known of the Sun's):
  - the main belt's largest body (39% of the belt's mass; a dwarf planet if it's over 400 km);
  - five crossing asteroids knocked onto orbits among the rocky planets;
  - captured moons far out round each giant, tilted and mostly going round backward;
  - centaurs among the giants;
  - the outer belt's dwarf planets;
  - four returning comets and one from the far cloud.

  The figures the registry holds come from `seeding.asteroids` (the belts' mass and edges, comet
  density, the zones). The rest are the seeder's own, in `small_bodies.rs`, to move to a seeding
  record.
- **New kinds of body:** dwarf planet (a round, cratered world you can land on), crossing asteroid,
  captured moon, centaur and comet. The four rock-like ones are rocks: lumpy, with a rock class and
  make-up, drawn, scanned, mined and touched as asteroids are. A class whose record lacks a colour
  or albedo isn't made: captured moons are carbonaceous until the registry's primitive class has
  its colour.
- **Near-parabolic orbits solve** (a far-cloud comet's eccentricity of 0.99999): Kepler's equation
  from Danby's starting guess, in up to 60 steps.
- They come after everything else a system has, so nothing made before them moves.
- **The registry's small-body records** (seeded by its own Python seeder, `in_game: not made`) are
  not the engine's yet. The celestial guard holds small bodies to their records once those records
  are the engine's. The exporter marks each small body (`"small": true`, with its rock class and
  density) for the registry to write them out.
