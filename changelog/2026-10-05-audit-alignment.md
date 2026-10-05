# The game against the registry: what it held of its own

From an audit of the game against the registry (three passes: what the game reads, what it still
keeps a copy of, where it behaves otherwise). Moved to the registry's figures, nothing else
changed:

- **Small bodies and belt rocks** are seeded by `seeding.small-bodies`: the largest body, crossing
  asteroids, captured moons, centaurs, outer dwarfs, comets, spins, and the belt rocks' patch,
  sizes, tilts, swarm width and rubble cut-off. Every body comes out as before (the celestial
  guard holds them to their records).
- **A ship's sensors see as far as its fitted radar's `range`** (500 km, its record's), not a
  constant; with none fitted, nothing.
- **A gate ring's size is its records'**: half the opening and half the thickness (1,500 m and
  60 m, as before); the physics sheet no longer holds them.
- **An imported hull the registry describes flies by its record's `flight`**: the MC-07's radius
  15.9 m and drag area 399 m², not worked out from its size.
