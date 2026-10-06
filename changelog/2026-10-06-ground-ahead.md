# The ground read ahead of the physics

- Twice a second, every ship (the player's and every craft) within 30 km of a baked world's ground
  has the fine tiles under it, and where it will be 3 and 8 s on, read in the background
  (`Universe::ground_ahead`, `Terrain::prefetch`): the physics reads a tile it lacks on the spot
  and waits for it (about 10 ms each), so a ship coming down found the ground's reading in its
  frame. The heights are the same either way (the same files): the physics and its replays are as
  they were.
- The relief plan's groundwork (the lab's `terrain-relief-plan.md`): the physics' height queries
  served from the shared tile cache, ahead of need.
